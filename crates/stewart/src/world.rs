use anyhow::{Context as _, Error};
use thunderdome::Index;
use tracing::{event, instrument, span, Level};

use crate::{
    any::ActorEntry,
    schedule::Schedule,
    tree::{Node, Tree},
    Actor, Context, Handle, InternalError, StartError,
};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    tree: Tree,
    schedule: Schedule,
    pending_start: Vec<Index>,
}

impl World {
    pub(crate) fn create<A>(
        &mut self,
        name: &'static str,
        parent: Option<Index>,
    ) -> Result<Handle<A>, Error>
    where
        A: Actor,
    {
        let node = Node {
            name,
            parent,
            entry: None,
        };
        let index = self.tree.insert(node)?;

        // Track that the actor has to be started
        self.pending_start.push(index);

        let handle = Handle::new(index);
        Ok(handle)
    }

    /// Start the current actor instance, making it available for handling messages.
    #[instrument("World::start", level = "debug", skip_all)]
    pub fn start<A>(&mut self, hnd: Handle<A>, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        // Find the node for the actor
        let node = self
            .tree
            .get_mut(hnd.index)
            .ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == hnd.index);
        let pending_index = if let Some(value) = maybe_index {
            value
        } else {
            return Err(StartError::ActorAlreadyStarted);
        };

        // Give the actor to the node
        let entry = ActorEntry::new(actor);
        node.entry = Some(Box::new(entry));

        // Finalize remove pending
        self.pending_start.remove(pending_index);

        Ok(())
    }

    /// Remove any actors that weren't started in time.
    fn timeout_starting(&mut self) -> Result<(), Error> {
        while let Some(actor) = self.pending_start.pop() {
            event!(Level::DEBUG, "actor start timed out");
            self.remove(actor)?;
        }

        Ok(())
    }

    pub(crate) fn send<M>(&mut self, index: Index, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Get the actor in tree
        let node = self.tree.get_mut(index).context("failed to find actor")?;

        // Hand the message to the actor
        let entry = node.entry.as_mut().context("actor unavailable")?;
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        // Queue for processing
        self.schedule.queue_process(index);

        Ok(())
    }

    /// Process all pending messages, until none are left.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), InternalError> {
        self.timeout_starting()?;

        while let Some(index) = self.schedule.next() {
            self.process(index).context("failed to process")?;

            self.timeout_starting()?;
        }

        Ok(())
    }

    pub(crate) fn process(&mut self, index: Index) -> Result<(), Error> {
        // Borrow the actor
        let node = self.tree.get_mut(index).context("failed to find actor")?;
        let mut actor = node.entry.take().context("actor unavailable")?;

        let span = span!(Level::INFO, "actor", name = node.name);
        let _entered = span.enter();
        event!(Level::DEBUG, "processing actor");

        // Run the process sender
        let mut cx = Context::new(self, Some(index));
        actor.process(&mut cx);
        let stop = actor.is_stop_requested();

        // Return the actor
        let node = self
            .tree
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.entry = Some(actor);

        // If the actor requested to remove itself, remove it
        if stop {
            self.remove(index)?;
        }

        Ok(())
    }

    /// Remove actor and its hierarchy.
    fn remove(&mut self, index: Index) -> Result<(), Error> {
        self.tree.remove(index, |index| {
            // Clean up pending messages
            self.schedule.dequeue_process(index);
        })?;
        Ok(())
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let debug_names = self.tree.query_debug_names();

        if !debug_names.is_empty() {
            event!(
                Level::WARN,
                ?debug_names,
                "actors not cleaned up before world drop",
            );
        }
    }
}
