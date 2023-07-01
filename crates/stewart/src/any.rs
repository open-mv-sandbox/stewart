use std::any::Any;

use anyhow::{Context as _, Error};
use tracing::{event, Level};

use crate::{Actor, Context, State};

pub trait AnyActorEntry {
    fn is_stop_requested(&self) -> bool;

    /// Add a message to be handled to the actor's internal queue.
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error>;

    /// Process pending messages.
    fn process(&mut self, cx: &mut Context);
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
            state: State::default(),
        }
    }
}

impl<S> AnyActorEntry for ActorEntry<S>
where
    S: Actor,
{
    fn is_stop_requested(&self) -> bool {
        self.state.is_stop_requested()
    }

    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error> {
        // Take the message out
        let slot: &mut Option<S::Message> =
            slot.downcast_mut().context("incorrect message type")?;
        let message = slot.take().context("message not in slot")?;

        self.state.enqueue(message);

        Ok(())
    }

    fn process(&mut self, cx: &mut Context) {
        // Let the actor's implementation process
        let result = self.actor.process(cx, &mut self.state);

        // Check if processing failed
        match result {
            Ok(value) => value,
            Err(error) => {
                event!(Level::ERROR, ?error, "error while processing");

                // If a processing error happens, the actor should be stopped.
                // It's better to stop than to potentially retain inconsistent state.
                self.state.stop();
                return;
            }
        }

        // Sanity warning, these are things a correctly processing actor should do
        if !self.state.is_queue_empty() {
            event!(Level::WARN, "actor did not process all pending messages");
        }
    }
}
