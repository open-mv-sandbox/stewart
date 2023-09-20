use anyhow::Error;

use crate::{Id, World};

/// Actor identity and processing implementation trait.
///
/// Implementations can return `Err` to signal a fatal error to the system.
/// If this happens, the actor will be stopped and cleaned up appropriately to protect against
/// inconsistent state.
///
/// You should *always* prefer this over panicking, as this crashes the entire runtime.
/// Instead of using `unwrap` or `expect`, use `context` from the `anyhow` crate.
pub trait Actor: 'static {
    /// Processing step implementation.
    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error>;
}

/// Metadata of an `Actor` in a world.
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
