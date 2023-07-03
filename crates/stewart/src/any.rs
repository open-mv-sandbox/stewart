use std::{any::Any, collections::VecDeque};

use anyhow::{Context as _, Error};
use tracing::{event, Level};

use crate::{Actor, Context, Id, World};

pub trait AnyActorEntry {
    /// Add a message to be handled to the actor's internal queue.
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error>;

    /// Process pending messages.
    fn process(&mut self, world: &mut World) -> bool;
}

pub struct ActorEntry<A>
where
    A: Actor,
{
    id: Id,
    actor: A,
    queue: VecDeque<A::Message>,
}

impl<A> ActorEntry<A>
where
    A: Actor,
{
    pub fn new(id: Id, actor: A) -> Self {
        Self {
            id,
            actor,
            queue: VecDeque::new(),
        }
    }
}

impl<A> AnyActorEntry for ActorEntry<A>
where
    A: Actor,
{
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the message out
        let slot: &mut Option<A::Message> =
            slot.downcast_mut().context("incorrect message type")?;
        let message = slot.take().context("message not in slot")?;

        self.queue.push_back(message);

        Ok(())
    }

    fn process(&mut self, world: &mut World) -> bool {
        let mut is_stop_requested = false;
        let cx = Context::actor(self.id, &mut self.queue, &mut is_stop_requested);

        // Let the actor's implementation process
        let result = self.actor.process(world, cx);

        // Check if processing failed
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "error while processing");

            // If a processing error happens, the actor should be stopped.
            // It's better to stop than to potentially retain inconsistent state.
            return true;
        }

        // Sanity warning, these are things a correctly processing actor should do
        if !self.queue.is_empty() {
            event!(Level::WARN, "actor did not process all pending messages");
        }

        is_stop_requested
    }
}
