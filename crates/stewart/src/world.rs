use std::collections::VecDeque;

use anyhow::{bail, Context as _, Error};
use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::{
    actor::Actor,
    any::ActorEntry,
    tree::{Node, Tree},
    unique_queue::UniqueQueue,
    Context, InternalError, Options, StartError,
};

/// Thread-local actor world.
#[derive(Default)]
pub struct World {
    tree: Tree,

    pending_process: VecDeque<Index>,
    pending_start: Vec<Index>,
    pending_stop: UniqueQueue<Index, StopReason>,
}

impl World {
    /// Create a new empty `World`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get root context, not associated with any actor.
    ///
    /// You can use this to create actors with no parents, letting them manage their own lifetime
    /// entirely. Be careful with this, as parent cleanup prevents using resources unnecessarily,
    /// even in case of errors.
    pub fn root(&mut self) -> Context {
        Context::new(self, None)
    }

    pub(crate) fn create(
        &mut self,
        parent: Option<Index>,
        options: Options,
    ) -> Result<Index, Error> {
        event!(Level::DEBUG, "creating actor");

        let node = Node::new(parent, options);
        let actor = self.tree.insert(node)?;

        self.pending_start.push(actor);

        Ok(actor)
    }

    pub(crate) fn start<A>(&mut self, index: Index, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        // Find the node for the actor
        let node = self.tree.get_mut(index).ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == index);
        let pending_index = if let Some(value) = maybe_index {
            value
        } else {
            return Err(StartError::ActorAlreadyStarted);
        };

        // Give the actor to the node
        let entry = ActorEntry::new(actor);
        *node.entry_mut() = Some(Box::new(entry));

        // Finalize remove pending
        self.pending_start.remove(pending_index);

        Ok(())
    }

    pub(crate) fn stop(&mut self, index: Index) -> Result<(), InternalError> {
        self.pending_stop.enqueue(index, StopReason::StopCalled)?;
        Ok(())
    }

    pub(crate) fn send<M>(&mut self, index: Index, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Make sure the actor's not already being stopped
        if self.pending_stop.contains(index) {
            bail!("actor stopping");
        }

        // Get the actor in tree
        let node = self.tree.get_mut(index).context("actor not found")?;
        let entry = node.entry_mut().as_mut().context("actor unavailable")?;

        // Hand the message to the actor
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        // Queue for later processing
        let high_priority = node.options().high_priority;
        self.queue_process(index, high_priority);

        Ok(())
    }

    fn queue_process(&mut self, index: Index, high_priority: bool) {
        if self.pending_process.contains(&index) {
            event!(Level::TRACE, "actor already queued for processing");
            return;
        }

        event!(Level::TRACE, high_priority, "queueing actor for processing");
        if !high_priority {
            self.pending_process.push_back(index);
        } else {
            self.pending_process.push_front(index);
        }
    }

    /// Process all pending messages, until none are left.
    pub fn run_until_idle(&mut self) -> Result<(), InternalError> {
        self.process_pending()
            .context("failed to process pending")?;

        while let Some(id) = self.pending_process.pop_front() {
            self.process_actor(id).context("failed to process")?;

            self.process_pending()
                .context("failed to process pending")?;
        }

        Ok(())
    }

    fn process_actor(&mut self, index: Index) -> Result<(), Error> {
        // Borrow the actor
        let node = self.tree.get_mut(index).context("failed to find actor")?;
        let mut actor = node.entry_mut().take().context("actor unavailable")?;

        // Run the process handler
        let mut ctx = Context::new(self, Some(index));
        actor.process(&mut ctx);

        // Return the actor
        let slot = self
            .tree
            .get_mut(index)
            .context("failed to find actor for return")?;
        *slot.entry_mut() = Some(actor);

        Ok(())
    }

    #[instrument(skip_all)]
    fn process_pending(&mut self) -> Result<(), Error> {
        // Remove any actors that weren't started in time
        while let Some(actor) = self.pending_start.pop() {
            self.stop(actor)?;
        }

        self.process_stop_actors()?;

        Ok(())
    }

    fn process_stop_actors(&mut self) -> Result<(), Error> {
        // Process stop queue in reverse order intentionally
        while let Some((id, reason)) = self.pending_stop.peek() {
            // Check if all dependents have already stopped
            if !self.check_stop_dependents(id)? {
                continue;
            }

            // We verified this actor can be removed, so pop it from the queue
            self.pending_stop.pop()?;
            self.process_stop_actor(id, reason)?;
        }

        Ok(())
    }

    fn process_stop_actor(&mut self, index: Index, reason: StopReason) -> Result<(), Error> {
        event!(Level::DEBUG, ?reason, "stopping actor");

        let _node = self.tree.remove(index).context("actor not found")?;

        // Remove queue entries for the actor
        self.pending_process.retain(|pid| *pid != index);

        Ok(())
    }

    fn check_stop_dependents(&mut self, index: Index) -> Result<bool, Error> {
        let mut ready = true;

        // Check if this actor has any children to process first
        self.tree.query_children(index, |child| {
            self.pending_stop
                .enqueue(child, StopReason::ParentStopping)?;
            ready = false;
            Ok(())
        })?;

        Ok(ready)
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

#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    StopCalled,
    ParentStopping,
}
