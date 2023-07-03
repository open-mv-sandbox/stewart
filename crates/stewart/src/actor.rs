use std::collections::VecDeque;

use anyhow::Error;

use crate::{Id, World};

/// Actor processing implementation trait.
pub trait Actor: Sized + 'static {
    /// The type of messages this actor receives.
    type Message;

    /// Perform a processing step.
    ///
    /// This function can return `Err` to signal a fatal error to the system.
    /// If this happens, the actor will be stopped and cleaned up appropriately to protect against
    /// inconsistent state.
    ///
    /// You should *always* prefer this over panicking, as this crashes the entire runtime.
    fn process(&mut self, world: &mut World, cx: Context<Self>) -> Result<(), Error>;
}

/// The context of an actor.
///
/// Bundles information and state of the actor for processing.
pub struct Context<'a, A>
where
    A: Actor,
{
    id: Id,
    queue: &'a mut VecDeque<A::Message>,
    is_stop_requested: &'a mut bool,
}

impl<'a, A> Context<'a, A>
where
    A: Actor,
{
    pub(crate) fn actor(
        id: Id,
        queue: &'a mut VecDeque<A::Message>,
        is_stop_requested: &'a mut bool,
    ) -> Self {
        Self {
            id,
            queue,
            is_stop_requested,
        }
    }

    /// Get the ID of the current actor.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Get the next queued message, along with the actor ID it's for.
    pub fn next(&mut self) -> Option<A::Message> {
        self.queue.pop_front()
    }

    /// Schedule the actor to be stopped.
    ///
    /// The stop will be applied at the end of the process step.
    pub fn stop(&mut self) {
        *self.is_stop_requested = true;
    }
}
