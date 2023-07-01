use std::ops::{Deref, DerefMut};

use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::{Actor, Handle, InternalError, World};

/// Context to perform operations in.
///
/// This type bundles the other types the actions will be operated on, the world and the schedule,
/// and the context it will be operated in, such as the current actor.
pub struct Context<'a> {
    pub(crate) world: &'a mut World,
    current: Option<Index>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(world: &'a mut World, current: Option<Index>) -> Self {
        Self { world, current }
    }

    /// Create a 'root' context, not associated with an actor.
    pub fn root(world: &'a mut World) -> Self {
        Self {
            world,
            current: None,
        }
    }

    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    ///
    /// The given `name` will be used in logging.
    #[instrument("Context::create", level = "debug", skip_all)]
    pub fn create<A>(&mut self, name: &'static str) -> Result<(Context, Handle<A>), InternalError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, name, "creating actor");

        // TODO: Ensure correct message type and actor are associated

        let hnd = self.world.create(name, self.current)?;

        let cx = Context {
            world: self.world,
            current: Some(hnd.index),
        };
        Ok((cx, hnd))
    }
}

impl<'a> Deref for Context<'a> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl<'a> DerefMut for Context<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
