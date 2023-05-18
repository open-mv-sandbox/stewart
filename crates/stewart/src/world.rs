use std::{collections::VecDeque, hash::Hash, marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{bail, Context, Error};
use thunderdome::{Arena, Index};
use tracing::{event, instrument, Level};

use crate::{
    stop_queue::{StopQueue, StopReason},
    system::{AnySystemEntry, SystemEntry},
    tree::{Node, Tree},
    ActorId, CreateError, InternalError, StartError, System, SystemOptions,
};

/// Thread-local system and actor scheduler.
#[derive(Default)]
pub struct World {
    tree: Tree,
    // TODO: Systems collection
    systems: Arena<SystemSlot>,

    pending_process: VecDeque<SystemId>,
    pending_start: Vec<ActorId>,
    pending_stop: StopQueue,
}

impl World {
    /// Create a new empty `System`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an actor processing system.
    ///
    /// Actor processing systems are linked to actors. This means when an actor is cleaned up, the
    /// processing systems linked to it will also be cleaned up.
    #[instrument(skip_all)]
    pub fn register<S>(&mut self, system: S, parent: ActorId, options: SystemOptions) -> SystemId
    where
        S: System,
    {
        let entry = Box::new(SystemEntry::new(system));
        let slot = SystemSlot {
            entry: Some(entry),
            parent,
            options,
        };
        let index = self.systems.insert(slot);

        SystemId { index }
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    #[instrument(skip_all)]
    pub fn create(&mut self, parent: Option<ActorId>) -> Result<ActorId, CreateError> {
        event!(Level::INFO, "creating actor");

        let node = Node::new(parent);
        let actor = self.tree.insert(node)?;

        self.pending_start.push(actor);

        Ok(actor)
    }

    /// Start an actor instance, making it available for handling messages.
    #[instrument(skip_all)]
    pub fn start<I>(
        &mut self,
        actor: ActorId,
        system: SystemId,
        instance: I,
    ) -> Result<(), StartError>
    where
        I: 'static,
    {
        event!(Level::INFO, "starting actor");

        // Find the node for the actor
        let node = self.tree.get_mut(actor).ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == actor);
        let pending_index = if let Some(value) = maybe_index {
            value
        } else {
            return Err(StartError::ActorAlreadyStarted);
        };

        // Find the system
        let slot = self
            .systems
            .get_mut(system.index)
            .ok_or(StartError::SystemNotFound)?;
        let system_entry = slot.entry.as_mut().ok_or(StartError::SystemUnavailable)?;

        // Give the instance to the system
        let mut instance = Some(instance);
        system_entry
            .insert(actor, &mut instance)
            .map_err(|_| StartError::InstanceWrongType)?;

        // Link the node to the system
        node.set_system(Some(system));

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

    fn try_send<M>(&mut self, actor: ActorId, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Make sure the actor's not already being stopped
        if self.pending_stop.contains(actor) {
            bail!("actor stopping");
        }

        // Get the actor in tree
        let node = self.tree.get(actor).context("actor not found")?;

        // Find the system associated with this node
        let system_id = node.system().context("actor not started")?;
        let slot = self
            .systems
            .get_mut(system_id.index)
            .context("system not found")?;
        let system = slot.entry.as_mut().context("system unavailable")?;

        // Hand the message to the system
        let mut message = Some(message);
        system.enqueue(actor, &mut message)?;

        // Queue for later processing
        if !self.pending_process.contains(&system_id) {
            if !slot.options.high_priority {
                self.pending_process.push_back(system_id);
            } else {
                self.pending_process.push_front(system_id);
            }
        }

        Ok(())
    }

    /// Process all pending messages, until none are left.
    pub fn run_until_idle(&mut self) -> Result<(), InternalError> {
        self.process_pending()
            .context("failed to process pending")?;

        while let Some(system_id) = self.pending_process.pop_front() {
            self.process_system(system_id)
                .context("failed to process")?;

            self.process_pending()
                .context("failed to process pending")?;
        }

        Ok(())
    }

    fn process_system(&mut self, system_id: SystemId) -> Result<(), Error> {
        // Borrow the system
        let slot = self
            .systems
            .get_mut(system_id.index)
            .context("failed to find system")?;
        let mut system = slot.entry.take().context("system unavailable")?;

        // Run the process handler
        system.process(self);

        // Return the system
        let slot = self
            .systems
            .get_mut(system_id.index)
            .context("failed to find system for return")?;
        slot.entry = Some(system);

        Ok(())
    }

    #[instrument(skip_all)]
    fn process_pending(&mut self) -> Result<(), Error> {
        // Remove any actors that weren't started in time
        while let Some(actor) = self.pending_start.pop() {
            self.stop(actor)?;
        }

        self.process_drop_actors()?;

        Ok(())
    }

    fn process_drop_actors(&mut self) -> Result<(), Error> {
        // Process stop queue in reverse order intentionally
        while let Some((actor_id, reason)) = self.pending_stop.peek() {
            let systems = self.find_actor_owned_systems(actor_id);

            // Check if all dependents have already stopped
            if !self.check_stop_dependents(actor_id, &systems)? {
                continue;
            }

            // We verified this actor can be removed, so pop it from the queue
            self.pending_stop.pop();
            self.process_stop_actor(actor_id, reason, systems)?;
        }

        Ok(())
    }

    fn find_actor_owned_systems(&self, actor_id: ActorId) -> Vec<SystemId> {
        let mut systems = Vec::new();

        for (index, system) in &self.systems {
            if system.parent == actor_id {
                systems.push(SystemId { index });
            }
        }

        systems
    }

    fn process_stop_actor(
        &mut self,
        actor_id: ActorId,
        reason: StopReason,
        systems: Vec<SystemId>,
    ) -> Result<(), Error> {
        event!(Level::INFO, ?reason, "stopping actor");

        let node = self
            .tree
            .remove(actor_id)
            .context("node for actor not found")?;

        // Remove it from the system it's in, if it is in one
        if let Some(system) = node.system() {
            let slot = self
                .systems
                .get_mut(system.index)
                .context("failed to find system")?;
            let system = slot.entry.as_mut().context("system unavailable")?;

            system.remove(actor_id);
        }

        // Remove systems owned by this actor
        for system_id in systems {
            event!(Level::DEBUG, "removing system");
            self.systems.remove(system_id.index);

            // Remove system from queue too, if it's in it
            self.pending_process.retain(|v| *v != system_id);
        }

        Ok(())
    }

    fn check_stop_dependents(
        &mut self,
        actor_id: ActorId,
        systems: &[SystemId],
    ) -> Result<bool, Error> {
        let mut ready = true;

        // Check if this actor has any children to process first
        self.tree.query_children(actor_id, |child| {
            self.pending_stop
                .enqueue(child, StopReason::ParentStopping)?;
            ready = false;
            Ok(())
        })?;

        // Check if this actor owns systems that have other children, that thus need to be stopped
        // first
        for system_id in systems {
            let slot = self
                .systems
                .get(system_id.index)
                .context("failed to find system")?;

            // Check if there's other actors alive that need to be stopped first for system stop
            let ids = slot
                .entry
                .as_ref()
                .context("system unavailable")?
                .actor_ids_except(actor_id);
            for id in ids {
                self.pending_stop.enqueue(id, StopReason::SystemStopping)?;
                ready = false;
            }
        }

        Ok(ready)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if !self.systems.is_empty() {
            let counts: Vec<_> = self
                .systems
                .iter()
                .map(|(_, system)| {
                    system
                        .entry
                        .as_ref()
                        .map(|v| v.debug_name())
                        .unwrap_or("Unknown")
                })
                .collect();

            event!(
                Level::WARN,
                "systems not cleaned up before world drop\n{:#?}",
                counts
            );
        }
    }
}

struct SystemSlot {
    entry: Option<Box<dyn AnySystemEntry>>,
    parent: ActorId,
    options: SystemOptions,
}

/// Handle referencing a system on a `World`.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct SystemId {
    index: Index,
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
