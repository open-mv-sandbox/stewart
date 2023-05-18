use std::{
    any::{type_name, Any},
    collections::VecDeque,
};

use anyhow::{Context, Error};
use tracing::{event, Level};

use crate::World;

/// Actor processing system trait.
pub trait Actor: Sized + 'static {
    /// The type of messages this actor receives.
    type Message;

    /// Perform a processing step.
    fn process(&mut self, world: &mut World, state: &mut State<Self>) -> Result<(), Error>;
}

/// Options to inform the world on how to schedule an actor.
#[derive(Default, Debug, Clone)]
#[non_exhaustive]
pub struct Options {
    /// Sets if this actor is 'high-priority'.
    ///
    /// Typically, this means the world will always place the actor at the *start* of the queue.
    /// This is useful for actors that simply relay messages to other actors.
    /// In those cases, the message waiting at the end of the queue would hurt performance by
    /// fragmenting more impactful batches, and increase latency drastically.
    pub high_priority: bool,
}

impl Options {
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

pub trait AnyActorEntry {
    fn debug_name(&self) -> &'static str;

    /// Add a message to be handled to the actor's internal queue.
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error>;

    /// Process pending messages.
    fn process(&mut self, world: &mut World);
}

pub struct ActorEntry<S>
where
    S: Actor,
{
    actor: S,
    state: State<S>,
}

impl<A> ActorEntry<A>
where
    A: Actor,
{
    pub fn new(actor: A) -> Self {
        Self {
            actor,
            state: State {
                queue: VecDeque::new(),
            },
        }
    }
}

impl<S> AnyActorEntry for ActorEntry<S>
where
    S: Actor,
{
    fn debug_name(&self) -> &'static str {
        type_name::<S>()
    }

    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the message out
        let slot: &mut Option<S::Message> =
            slot.downcast_mut().context("incorrect message type")?;
        let message = slot.take().context("message not in slot")?;

        self.state.queue.push_front(message);

        Ok(())
    }

    fn process(&mut self, world: &mut World) {
        let result = self.actor.process(world, &mut self.state);

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

/// State of an actor, such as its pending messages.
pub struct State<A>
where
    A: Actor,
{
    queue: VecDeque<A::Message>,
}

impl<A> State<A>
where
    A: Actor,
{
    /// Get the next queued message, along with the actor ID it's for.
    pub fn next(&mut self) -> Option<A::Message> {
        self.queue.pop_front()
    }
}
