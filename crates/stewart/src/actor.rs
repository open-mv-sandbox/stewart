use anyhow::Error;

use crate::{Id, World};

/// Actor processing implementation trait.
pub trait Actor: Sized + 'static {
    /// Perform a processing step.
    ///
    /// This function can return `Err` to signal a fatal error to the system.
    /// If this happens, the actor will be stopped and cleaned up appropriately to protect against
    /// inconsistent state.
    ///
    /// You should *always* prefer this over panicking, as this crashes the entire runtime.
    fn process(&mut self, world: &mut World, cx: Context) -> Result<(), Error>;
}

/// The context of an actor.
///
/// Bundles information and state of the actor for processing.
///
/// TODO: Just return if we need to stop, we don't need context anymore.
pub struct Context<'a> {
    id: Id,
    is_stop_requested: &'a mut bool,
}

impl<'a> Context<'a> {
    pub(crate) fn actor(id: Id, is_stop_requested: &'a mut bool) -> Self {
        Self {
            id,
            is_stop_requested,
        }
    }

    /// Get the ID of the current actor.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Schedule the actor to be stopped.
    ///
    /// The stop will be applied at the end of the process step.
    pub fn stop(&mut self) {
        *self.is_stop_requested = true;
    }
}
