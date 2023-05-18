use std::{collections::VecDeque, hash::Hash, marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{bail, Context, Error};
use thunderdome::{Arena, Index};
use tracing::{event, Level};

use crate::{
    system::{AnySystemEntry, SystemEntry},
    tree::{Node, Tree},
    unique_queue::UniqueQueue,
    ActorId, CreateError, InternalError, StartError, System, SystemOptions,
};

/// Thread-local system and actor scheduler.
#[derive(Default)]
pub struct World {
    tree: Tree,
    // TODO: Systems collection
    systems: Arena<SystemSlot>,
    queue: VecDeque<SystemId>,

    pending_start: Vec<ActorId>,
    pending_stop: UniqueQueue<ActorId>,
    pending_unregister: Vec<SystemId>,
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
    pub fn register<S>(&mut self, system: S, _parent: ActorId, options: SystemOptions) -> SystemId
    where
        S: System,
    {
        // TODO: Implement auto-cleanup.

        let entry = Box::new(SystemEntry::new(system));
        let slot = SystemSlot {
            options,
            entry: Some(entry),
        };
        let index = self.systems.insert(slot);

        SystemId { index }
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    pub fn create(&mut self, parent: Option<ActorId>) -> Result<ActorId, CreateError> {
        event!(Level::INFO, "creating actor");

        let node = Node::new(parent);
        let actor = self.tree.insert(node)?;

        self.pending_start.push(actor);

        Ok(actor)
    }

    /// Start an actor instance, making it available for handling messages.
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
        self.pending_stop.push(actor)?;
        Ok(())
    }

    /// Send a message to an actor.
    ///
    /// This will never be handled in-place. The system will queue up the message to be processed
    /// at a later time.
    pub fn send<M>(&mut self, addr: Addr<M>, message: impl Into<M>)
    where
        M: 'static,
    {
        let result = self.try_send(addr.actor, message.into());

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
        if !self.queue.contains(&system_id) {
            if !slot.options.high_priority {
                self.queue.push_back(system_id);
            } else {
                self.queue.push_front(system_id);
            }
        }

        Ok(())
    }

    /// Process all pending messages, until none are left.
    pub fn run_until_idle(&mut self) -> Result<(), InternalError> {
        self.process_pending()
            .context("failed to process pending")?;

        while let Some(system_id) = self.queue.pop_front() {
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

    fn process_pending(&mut self) -> Result<(), Error> {
        // Remove any actors that weren't started in time
        while let Some(actor) = self.pending_start.pop() {
            self.stop(actor)?;
        }

        self.process_stop()?;

        // Finalize unregisters
        for system in self.pending_unregister.drain(..) {
            self.systems.remove(system.index);
        }

        Ok(())
    }

    fn process_stop(&mut self) -> Result<(), Error> {
        // Process stop queue in reverse order
        while let Some(actor) = self.pending_stop.peek() {
            // Check if this actor has any children to process first
            let mut has_children = false;
            self.tree.query_children(actor, |child| {
                self.pending_stop.push(child)?;
                has_children = true;
                Ok(())
            })?;

            // If we do have children, stop those first
            if has_children {
                continue;
            }

            // We verified, we can remove this now
            self.pending_stop.pop();
            let node = self
                .tree
                .remove(actor)
                .context("node for actor not found")?;

            // Remove it from the system it's in, if it is in one
            if let Some(system) = node.system() {
                let slot = self
                    .systems
                    .get_mut(system.index)
                    .context("failed to find system")?;
                let system = slot.entry.as_mut().context("system unavailable")?;

                system.remove(actor);
            }
        }

        Ok(())
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
    options: SystemOptions,
    entry: Option<Box<dyn AnySystemEntry>>,
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
