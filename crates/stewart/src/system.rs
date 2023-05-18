use std::{
    any::{type_name, Any},
    collections::{BTreeMap, VecDeque},
};

use anyhow::{Context, Error};
use tracing::{event, Level};

use crate::{ActorId, World};

/// Actor processing system trait.
pub trait System: Sized + 'static {
    /// The type of actor instances this system processes.
    type Instance;

    /// The type of messages actors of this system receive.
    type Message;

    /// Perform a processing step.
    fn process(&mut self, world: &mut World, state: &mut State<Self>) -> Result<(), Error>;
}

/// Options to inform the world on how to schedule a system.
#[derive(Default, Debug, Clone)]
#[non_exhaustive]
pub struct SystemOptions {
    /// Sets if this system is 'high-priority'.
    ///
    /// Typically, this means the world will always place the system at the *start* of the queue.
    /// This is useful for systems that simply relay messages to other systems.
    /// In those cases, the message waiting at the end of the queue would hurt performance by
    /// fragmenting more impactful batches, and increase latency drastically.
    pub high_priority: bool,
}

impl SystemOptions {
    /// Convenience alias for `Self::default().with_high_priority()`.
    pub fn high_priority() -> Self {
        Self::default().with_high_priority()
    }

    /// Sets `high_priority` to true.
    pub fn with_high_priority(mut self) -> Self {
        self.high_priority = true;
        self
    }
}

pub trait AnySystemEntry {
    fn debug_name(&self) -> &'static str;

    fn actor_ids_except(&self, actor_id: ActorId) -> Vec<ActorId>;

    /// Insert an actor into the system.
    fn insert(&mut self, actor: ActorId, slot: &mut dyn Any) -> Result<(), Error>;

    /// Remove an actor from the system.
    fn remove(&mut self, actor: ActorId);

    /// Add a message to be handled to the system's internal queue.
    fn enqueue(&mut self, actor: ActorId, slot: &mut dyn Any) -> Result<(), Error>;

    fn process(&mut self, world: &mut World);
}

pub struct SystemEntry<S>
where
    S: System,
{
    system: S,
    state: State<S>,
}

impl<S> SystemEntry<S>
where
    S: System,
{
    pub fn new(system: S) -> Self {
        Self {
            system,
            state: State {
                instances: BTreeMap::new(),
                queue: VecDeque::new(),
            },
        }
    }
}

impl<S> AnySystemEntry for SystemEntry<S>
where
    S: System,
{
    fn debug_name(&self) -> &'static str {
        type_name::<S>()
    }

    fn actor_ids_except(&self, actor_id: ActorId) -> Vec<ActorId> {
        let mut ids = Vec::new();

        for (id, _) in &self.state.instances {
            if *id != actor_id {
                ids.push(*id);
            }
        }

        ids
    }

    fn insert(&mut self, actor: ActorId, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the instance out
        let slot: &mut Option<S::Instance> =
            slot.downcast_mut().context("incorrect instance type")?;
        let instance = slot.take().context("instance not in slot")?;

        self.state.instances.insert(actor, instance);

        Ok(())
    }

    fn remove(&mut self, actor: ActorId) {
        self.state.instances.remove(&actor);

        // Drop pending messages of this instance
        self.state.queue.retain(|(i, _)| *i != actor);
    }

    fn enqueue(&mut self, actor: ActorId, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the message out
        let slot: &mut Option<S::Message> =
            slot.downcast_mut().context("incorrect message type")?;
        let message = slot.take().context("message not in slot")?;

        self.state.queue.push_front((actor, message));

        Ok(())
    }

    fn process(&mut self, world: &mut World) {
        let result = self.system.process(world, &mut self.state);

        if !self.state.queue.is_empty() {
            event!(Level::WARN, "system did not process all pending messages");
        }

        match result {
            Ok(value) => value,
            Err(error) => {
                // TODO: What to do with this?
                event!(Level::ERROR, ?error, "system failed while processing");
            }
        }
    }
}

/// State of a system, such as its actor instances and pending messages.
pub struct State<S>
where
    S: System,
{
    instances: BTreeMap<ActorId, S::Instance>,
    queue: VecDeque<(ActorId, S::Message)>,
}

impl<S> State<S>
where
    S: System,
{
    /// Get the next queued message, along with the actor ID it's for.
    pub fn next(&mut self) -> Option<(ActorId, S::Message)> {
        self.queue.pop_front()
    }

    /// Get a reference to an actor's instance.
    pub fn get(&self, actor: ActorId) -> Option<&S::Instance> {
        self.instances.get(&actor)
    }

    /// Get a mutable reference to an actor's instance.
    pub fn get_mut(&mut self, actor: ActorId) -> Option<&mut S::Instance> {
        self.instances.get_mut(&actor)
    }
}
