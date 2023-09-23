use anyhow::Context;
use std::any::Any;
use std::collections::VecDeque;
use std::ops::ControlFlow;

use crate::{Actor, ActorError, InternalError, Runtime};

pub trait AnyActorContainer {
    fn push_message(&mut self, slot: &mut dyn Any) -> Result<(), InternalError>;

    fn process(&mut self, rt: &mut Runtime) -> Result<ControlFlow<()>, ActorError>;
}

pub struct ActorContainer<A>
where
    A: Actor,
{
    queue: VecDeque<A::Message>,
    actor: A,
}

impl<A> ActorContainer<A>
where
    A: Actor,
{
    pub fn new(actor: A) -> Self {
        Self {
            queue: VecDeque::new(),
            actor,
        }
    }
}

impl<A> AnyActorContainer for ActorContainer<A>
where
    A: Actor,
{
    fn push_message(&mut self, slot: &mut dyn Any) -> Result<(), InternalError> {
        // Try downcasting the slot
        let Some(slot) = slot.downcast_mut::<Option<A::Message>>() else {
            return Ok(());
        };

        // Take the message out, and store it
        let message = slot.take().context("no message in slot")?;
        self.queue.push_back(message);

        Ok(())
    }

    fn process(&mut self, rt: &mut Runtime) -> Result<ControlFlow<()>, ActorError> {
        while let Some(message) = self.queue.pop_front() {
            let flow = self.actor.handle(rt, message)?;

            if flow.is_break() {
                return Ok(ControlFlow::Break(()));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
