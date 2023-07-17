use tracing::{event, Level};

use crate::{Actor, Context, Id, World};

/// TODO: We'll be able to remove the any wrapper when mailboxes are done and actors become
/// object-safe.
pub trait AnyDynActor {
    /// Process pending messages.
    fn process(&mut self, id: Id, world: &mut World) -> bool;
}

pub struct DynActor<A>
where
    A: Actor,
{
    actor: A,
}

impl<A> DynActor<A>
where
    A: Actor,
{
    pub fn new(actor: A) -> Self {
        Self { actor }
    }
}

impl<A> AnyDynActor for DynActor<A>
where
    A: Actor,
{
    fn process(&mut self, id: Id, world: &mut World) -> bool {
        let mut is_stop_requested = false;
        let cx = Context::actor(id, &mut is_stop_requested);

        // Let the actor's implementation process
        let result = self.actor.process(world, cx);

        // Check if processing failed
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "error while processing");

            // If a processing error happens, the actor should be stopped.
            // It's better to stop than to potentially retain inconsistent state.
            return true;
        }

        is_stop_requested
    }
}
