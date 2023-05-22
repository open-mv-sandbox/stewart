use std::ops::{Deref, DerefMut};

use anyhow::{Context as _, Error};

use crate::{tree::Id, Actor, Addr, CreateError, InternalError, Options, StartError, World};

/// Context for world operations.
///
/// This includes:
/// - The current actor performing operations, or root.
pub struct Context<'a> {
    world: &'a mut World,
    current: Option<Id>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(world: &'a mut World, current: Option<Id>) -> Self {
        Self { world, current }
    }

    /// Create a new typed address for an actor.
    ///
    /// Message type is not checked here, but will be validated on sending.
    pub fn addr<M>(&self) -> Result<Addr<M>, Error> {
        // TODO: Custom error
        let id = self.current.context("cant get addr of root")?;
        Ok(Addr::new(id))
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    pub fn create(&mut self, options: Options) -> Result<Context, CreateError> {
        let id = self.world.create(self.current, options)?;
        Ok(Context::new(self.world, Some(id)))
    }

    /// Start the current actor instance, making it available for handling messages.
    pub fn start<A>(&mut self, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        let id = self.current.ok_or(StartError::CantStartRoot)?;
        self.world.start(id, actor)
    }

    /// Queue an actor for stopping.
    ///
    /// After stopping an actor will no longer accept messages, but can still process them.
    /// After the current process step is done, the actor and all remaining pending messages will
    /// be dropped.
    pub fn stop(&mut self) -> Result<(), InternalError> {
        self.current.map(|id| self.world.stop(id)).unwrap_or(Ok(()))
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
