use std::collections::VecDeque;

use anyhow::Error;

use crate::Context;

/// Actor processing implementation trait.
pub trait Actor: Sized + 'static {
    /// The type of messages this actor receives.
    type Message;

    /// Perform a processing step.
    fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error>;
}

/// World state of an actor, such as its pending messages.
pub struct State<A>
where
    A: Actor,
{
    queue: VecDeque<A::Message>,
    stop_requested: bool,
}

impl<A> Default for State<A>
where
    A: Actor,
{
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            stop_requested: false,
        }
    }
}

impl<A> State<A>
where
    A: Actor,
{
    pub(crate) fn enqueue(&mut self, message: A::Message) {
        self.queue.push_back(message);
    }

    pub(crate) fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(crate) fn is_stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// Get the next queued message, along with the actor ID it's for.
    pub fn next(&mut self) -> Option<A::Message> {
        self.queue.pop_front()
    }

    /// Schedule the actor to be stopped.
    ///
    /// The stop will be applied at the end of the process step.
    pub fn stop(&mut self) {
        self.stop_requested = true;
    }
}
