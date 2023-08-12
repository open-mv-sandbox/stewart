use anyhow::Error;

use crate::{Id, World};

/// Actor identity and implementation trait.
///
/// Implementations can return `Err` to signal a fatal error to the system.
/// If this happens, the actor will be stopped and cleaned up appropriately to protect against
/// inconsistent state.
///
/// You should *always* prefer this over panicking, as this crashes the entire runtime.
/// Instead of using `unwrap` or `expect`, use `context` from the `anyhow` crate.
pub trait Actor: 'static {
    /// Called when an actor is inserted into a `World`.
    ///
    /// This is useful to receive the `Signal` for this actor.
    #[allow(unused_variables)]
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        Ok(())
    }

    /// Perform a processing step, after being signalled.
    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error>;
}

/// Metadata of an `Actor` in a `World`.
pub struct Metadata {
    id: Id,
    stop: bool,
}

impl Metadata {
    pub(crate) fn new(id: Id) -> Self {
        Self { id, stop: false }
    }

    /// Get the identifier of this actor.
    pub fn id(&self) -> Id {
        self.id
    }

    pub(crate) fn stop(&self) -> bool {
        self.stop
    }

    /// At the end of this processing step, stop the actor.
    pub fn set_stop(&mut self) {
        self.stop = true;
    }
}
