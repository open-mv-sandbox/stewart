use std::ops::{Deref, DerefMut};

use thiserror::Error;
use thunderdome::Index;
use tracing::instrument;

use crate::{Actor, InternalError, Sender, World};

/// Context for world operations.
///
/// This includes:
/// - The current actor performing operations, or root.
pub struct Context<'a> {
    world: &'a mut World,
    current: Option<Index>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(world: &'a mut World, current: Option<Index>) -> Self {
        Self { world, current }
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    #[instrument("Context::create", skip_all)]
    pub fn create<M>(&mut self) -> Result<(Context, Sender<M>), InternalError>
    where
        M: 'static,
    {
        // TODO: Ensure correct message type and actor are associated

        let index = self.world.create(self.current)?;
        let sender = Sender::direct(index);

        let ctx = Context::new(self.world, Some(index));
        Ok((ctx, sender))
    }

    /// Start the current actor instance, making it available for handling messages.
    #[instrument("Context::start", skip_all)]
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

/// Error on actor starting.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum StartError {
    /// Can't start root.
    #[error("cant start root")]
    CantStartRoot,
    /// The actor has already been started.
    #[error("actor already started")]
    ActorAlreadyStarted,
    /// The actor couldn't be found.
    #[error("actor not found")]
    ActorNotFound,
}
