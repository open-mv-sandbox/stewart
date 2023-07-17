use std::ops::{Deref, DerefMut};

use anyhow::Error;

use crate::{Id, World};

/// Actor processing implementation trait.
pub trait Actor: 'static {
    /// Perform a processing step.
    ///
    /// This function can return `Err` to signal a fatal error to the system.
    /// If this happens, the actor will be stopped and cleaned up appropriately to protect against
    /// inconsistent state.
    ///
    /// You should *always* prefer this over panicking, as this crashes the entire runtime.
    fn process(&mut self, ctx: &mut Context) -> Result<(), Error>;
}

/// The context of an actor.
///
/// Bundles information and state of the actor for processing.
pub struct Context<'a> {
    world: &'a mut World,
    id: Id,
    stop: &'a mut bool,
}

impl<'a> Context<'a> {
    pub(crate) fn actor(world: &'a mut World, id: Id, stop: &'a mut bool) -> Self {
        Self { world, id, stop }
    }

    /// Get the ID of the current actor.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Schedule the actor to be stopped.
    ///
    /// The stop will be applied at the end of the process step.
    pub fn stop(&mut self) {
        *self.stop = true;
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
