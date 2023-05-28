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
