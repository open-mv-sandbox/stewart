use std::collections::VecDeque;

use anyhow::Error;

use crate::Context;

/// Actor identity trait.
pub trait Actor: Sized + 'static {
    /// The type of messages this actor receives.
    type Message;

    /// Perform a processing step.
    fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error>;
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

/// World of an actor, such as its pending messages.
pub struct State<A>
where
    A: Actor,
{
    pub(crate) queue: VecDeque<A::Message>,
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
