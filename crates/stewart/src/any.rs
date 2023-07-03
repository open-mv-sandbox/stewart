use std::{any::Any, collections::VecDeque};

use anyhow::{Context as _, Error};
use tracing::{event, Level};

use crate::{Actor, Context, Id, World};

pub trait AnyActorEntry {
    fn is_stop_requested(&self) -> bool;

    /// Add a message to be handled to the actor's internal queue.
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error>;

    /// Process pending messages.
    fn process(&mut self, world: &mut World);
}

pub struct ActorEntry<A>
where
    A: Actor,
{
    id: Id,
    actor: A,
    queue: VecDeque<A::Message>,
    is_stop_requested: bool,
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
            is_stop_requested: false,
        }
    }
}

impl<A> AnyActorEntry for ActorEntry<A>
where
    A: Actor,
{
    fn is_stop_requested(&self) -> bool {
        // TODO: Just return from process
        self.is_stop_requested
    }

    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the message out
        let slot: &mut Option<A::Message> =
            slot.downcast_mut().context("incorrect message type")?;
        let message = slot.take().context("message not in slot")?;

        self.queue.push_back(message);

        Ok(())
    }

    fn process(&mut self, world: &mut World) {
        let cx = Context::actor(self.id, &mut self.queue, &mut self.is_stop_requested);

        // Let the actor's implementation process
        let result = self.actor.process(world, cx);

        // Check if processing failed
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "error while processing");

            // If a processing error happens, the actor should be stopped.
            // It's better to stop than to potentially retain inconsistent state.
            self.is_stop_requested = true;
            return;
        }

        // Sanity warning, these are things a correctly processing actor should do
        if !self.queue.is_empty() {
            event!(Level::WARN, "actor did not process all pending messages");
        }
    }
}
