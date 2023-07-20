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
    /// Instead of using `unwrap` or `expect`, use `context` from the `anyhow` crate.
    fn process(&mut self, ctx: &mut Context) -> Result<(), Error>;

    /// Clean up the actor when stopping explicitly, or due to an error.
    ///
    /// You can use this to propagate stop messages to other actors.
    ///
    /// This function will **not** be called on `World` drop.
    /// If you need to clean up resources external to the `World`, use `drop` instead.
    ///
    /// TODO: This is fragile and prone to mistakes as a mechanism to cleaning up dependencies.
    /// Maybe we should have automatic stop-on-drop handles? This would require a mechanism for
    /// notifying actors without directly receiving the world as mutable reference.
    fn stop(&mut self, _ctx: &mut Context) {}
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
