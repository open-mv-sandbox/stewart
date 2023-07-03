use anyhow::{Context as _, Error};
use thunderdome::Index;
use tracing::{event, instrument, span, Level};

use crate::{
    any::ActorEntry,
    schedule::Schedule,
    tree::{Node, Tree},
    Actor, InternalError, StartError,
};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    tree: Tree,
    schedule: Schedule,
    pending_start: Vec<Index>,
}

impl World {
    /// Create a new actor.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::create", level = "debug", skip_all)]
    pub fn create(&mut self, parent: Option<Id>, name: &'static str) -> Result<Id, InternalError> {
        event!(Level::DEBUG, name, "creating actor");

        let node = Node {
            name,
            parent: parent.map(|v| v.index),
            entry: None,
        };
        let index = self.tree.insert(node)?;

        // Track that the actor has to be started
        self.pending_start.push(index);

        let id = Id { index };
        Ok(id)
    }

    /// Start an actor instance at an `Id`.
    #[instrument("World::start", level = "debug", skip_all)]
    pub fn start<A>(&mut self, id: Id, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        // Find the node for the actor
        let node = self
            .tree
            .get_mut(id.index)
            .ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == id.index);
        let Some(pending_index) = maybe_index else {
            return Err(StartError::ActorAlreadyStarted);
        };

        // Give the actor to the node
        let entry = ActorEntry::new(id, actor);
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

    /// Send a message to the actor at the ID.
    pub fn send<M>(&mut self, id: Id, message: M)
    where
        M: 'static,
    {
        let result = self.try_send(id, message);

        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to send message");
        }
    }

    fn try_send<M>(&mut self, id: Id, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Get the actor in tree
        let node = self
            .tree
            .get_mut(id.index)
            .context("failed to find actor")?;

        // Hand the message to the actor
        let entry = node.entry.as_mut().context("actor unavailable")?;
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        // Queue for processing
        self.schedule.queue_process(id.index);

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
        let stop = actor.process(self);

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

/// ID of an actor slot in a `World`.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Id {
    index: Index,
}
