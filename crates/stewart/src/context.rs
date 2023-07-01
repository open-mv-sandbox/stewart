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
    /// Create a 'root' context, not associated with an actor.
    pub fn root(world: &'a mut World) -> Self {
        Self {
            world,
            current: None,
        }
    }

    /// Create a context, based on this context, with an actor as the current parent.
    pub fn with<A>(&mut self, hnd: Handle<A>) -> Context {
        Context {
            world: self.world,
            current: Some(hnd.index),
        }
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    ///
    /// The given `name` will be used in logging.
    #[instrument("Context::create", level = "debug", skip_all)]
    pub fn create<A>(&mut self, name: &'static str) -> Result<Handle<A>, InternalError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, name, "creating actor");

        // TODO: Ensure correct message type and actor are associated

        let hnd = self.world.create(name, self.current)?;

        Ok(hnd)
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
