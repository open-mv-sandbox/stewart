use thiserror::Error;
use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::{Actor, InternalError, Schedule, Sender, World};

/// Context to perform operations in.
///
/// This type bundles the other types the actions will be operated on, the world and the schedule,
/// and the context it will be operated in, such as the current actor.
pub struct Context<'a> {
    pub(crate) world: &'a mut World,
    schedule: &'a mut Schedule,
    current: Option<Index>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        world: &'a mut World,
        schedule: &'a mut Schedule,
        current: Option<Index>,
    ) -> Self {
        Self {
            world,
            schedule,
            current,
        }
    }

    /// Create a 'root' context, not associated with an actor.
    pub fn root(world: &'a mut World, schedule: &'a mut Schedule) -> Self {
        Self {
            world,
            schedule,
            current: None,
        }
    }

    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub(crate) fn schedule_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }

    /// Create a new actor.
    ///
    /// The actor's address will not be available for handling messages until `start` is called.
    #[instrument("Context::create", skip_all)]
    pub fn create<M>(&mut self) -> Result<(Context, Sender<M>), InternalError>
    where
        M: 'static,
    {
        event!(Level::DEBUG, "creating actor");

        // TODO: Ensure correct message type and actor are associated

        let index = self.world.create(self.current)?;
        let sender = Sender::direct(index);

        let ctx = Context {
            world: self.world,
            schedule: self.schedule,
            current: Some(index),
        };
        Ok((ctx, sender))
    }

    /// Start the current actor instance, making it available for handling messages.
    #[instrument("Context::start", skip_all)]
    pub fn start<A>(&mut self, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        let index = self.current.ok_or(StartError::CantStartRoot)?;
        self.world.start(index, actor)?;

        Ok(())
    }

    /// Queue an actor for stopping.
    ///
    /// After stopping an actor will no longer accept messages, but can still process them.
    /// After the current process step is done, the actor and all remaining pending messages will
    /// be dropped.
    pub fn stop(&mut self) -> Result<(), InternalError> {
        if let Some(index) = self.current {
            self.world.mark_stopping(index)?;
            self.schedule.queue_stop(index);
        }

        Ok(())
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
