use std::{collections::VecDeque, marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{bail, Context, Error};
use tracing::{event, instrument, Level};

use crate::{
    actor::{Actor, ActorEntry},
    stop_queue::{StopQueue, StopReason},
    tree::{Node, Tree},
    ActorId, CreateError, InternalError, Options, StartError,
};

/// Thread-local system and actor scheduler.
#[derive(Default)]
pub struct World {
    tree: Tree,

    pending_process: VecDeque<ActorId>,
    pending_start: Vec<ActorId>,
    pending_stop: StopQueue,
}

impl World {
    /// Create a new empty `System`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    #[instrument(skip_all)]
    pub fn create(
        &mut self,
        parent: Option<ActorId>,
        options: Options,
    ) -> Result<ActorId, CreateError> {
        event!(Level::DEBUG, "creating actor");

        let node = Node::new(parent, options);
        let actor = self.tree.insert(node)?;

        self.pending_start.push(actor);

        Ok(actor)
    }

    /// Start an actor instance, making it available for handling messages.
    #[instrument(skip_all)]
    pub fn start<A>(&mut self, id: ActorId, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        // Find the node for the actor
        let node = self.tree.get_mut(id).ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == id);
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

    /// Stop an actor immediately, and queue it for removal from systems later.
    ///
    /// After stopping an actor will no longer accept messages, but can still process them.
    /// After the current process step is done, the actor and all remaining pending messages will
    /// be dropped.
    pub fn stop(&mut self, actor: ActorId) -> Result<(), Error> {
        self.pending_stop.enqueue(actor, StopReason::StopCalled)?;
        Ok(())
    }

    /// Send a message to an actor.
    ///
    /// This will never be handled in-place. The system will queue up the message to be processed
    /// at a later time.
    #[instrument(skip_all)]
    pub fn send<M>(&mut self, addr: Addr<M>, message: M)
    where
        M: 'static,
    {
        let result = self.try_send(addr.actor, message);

        // TODO: What to do with this error?
        // Sending failures are currently ignored, but maybe we should have a unified message
        // error system.
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to send message");
        }
    }

    fn try_send<M>(&mut self, actor_id: ActorId, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Make sure the actor's not already being stopped
        if self.pending_stop.contains(actor_id) {
            bail!("actor stopping");
        }

        // Get the actor in tree
        let node = self.tree.get_mut(actor_id).context("actor not found")?;
        let entry = node.entry_mut().as_mut().context("actor unavailable")?;

        // Hand the message to the system
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        // Queue for later processing
        if !self.pending_process.contains(&actor_id) {
            if !node.options().high_priority {
                self.pending_process.push_back(actor_id);
            } else {
                self.pending_process.push_front(actor_id);
            }
        }

        Ok(())
    }

    /// Process all pending messages, until none are left.
    pub fn run_until_idle(&mut self) -> Result<(), InternalError> {
        self.process_pending()
            .context("failed to process pending")?;

        while let Some(actor_id) = self.pending_process.pop_front() {
            self.process_actor(actor_id).context("failed to process")?;

            self.process_pending()
                .context("failed to process pending")?;
        }

        Ok(())
    }

    fn process_actor(&mut self, actor_id: ActorId) -> Result<(), Error> {
        // Borrow the actor
        let node = self
            .tree
            .get_mut(actor_id)
            .context("failed to find actor")?;
        let mut actor = node.entry_mut().take().context("system unavailable")?;

        // Run the process handler
        actor.process(self);

        // Return the system
        let slot = self
            .tree
            .get_mut(actor_id)
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
        while let Some((actor_id, reason)) = self.pending_stop.peek() {
            // Check if all dependents have already stopped
            if !self.check_stop_dependents(actor_id)? {
                continue;
            }

            // We verified this actor can be removed, so pop it from the queue
            self.pending_stop.pop();
            self.process_stop_actor(actor_id, reason)?;
        }

        Ok(())
    }

    fn process_stop_actor(&mut self, actor_id: ActorId, reason: StopReason) -> Result<(), Error> {
        event!(Level::DEBUG, ?reason, "stopping actor");

        let _node = self.tree.remove(actor_id).context("actor not found")?;

        // Remove queue entries for the actor
        self.pending_process.retain(|id| *id != actor_id);

        Ok(())
    }

    fn check_stop_dependents(&mut self, actor_id: ActorId) -> Result<bool, Error> {
        let mut ready = true;

        // Check if this actor has any children to process first
        self.tree.query_children(actor_id, |child| {
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
                "systems not cleaned up before world drop",
            );
        }
    }
}

/// Typed system address of an actor, used for sending messages to the actor.
///
/// This address can only be used with one specific system. Using it with another system is
/// not unsafe, but may result in unexpected behavior.
///
/// When distributing work between systems, you can use an 'envoy' actor that relays messages from
/// one system to another. For example, using an MPSC channel, or even across network.
pub struct Addr<M> {
    actor: ActorId,
    _m: PhantomData<AtomicPtr<M>>,
}

impl<M> Addr<M> {
    /// Create a new typed address for an actor.
    ///
    /// Message type is not checked here, but will be validated on sending.
    pub fn new(actor: ActorId) -> Self {
        Self {
            actor,
            _m: PhantomData,
        }
    }
}

impl<M> Clone for Addr<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for Addr<M> {}
