use std::{
    any::{type_name, Any},
    collections::VecDeque,
};

use anyhow::{Context as _, Error};
use tracing::{event, Level};

use crate::{Actor, Context, State};

pub trait AnyActorEntry {
    fn debug_name(&self) -> &'static str;

    /// Add a message to be handled to the actor's internal queue.
    fn enqueue(&mut self, slot: &mut dyn Any) -> Result<(), Error>;

    /// Process pending messages.
    fn process(&mut self, ctx: &mut Context);
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

        self.state.queue.push_back(message);

        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) {
        let result = self.actor.process(ctx, &mut self.state);

        if !self.state.queue.is_empty() {
            event!(Level::WARN, "actor did not process all pending messages");
        }

        match result {
            Ok(value) => value,
            Err(error) => {
                // TODO: What to do with this?
                event!(Level::ERROR, ?error, "actor failed while processing");
            }
        }
    }
}
