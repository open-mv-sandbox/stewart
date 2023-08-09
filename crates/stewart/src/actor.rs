use anyhow::Error;

use crate::{Id, World};

/// Actor identity and implementation trait.
pub trait Actor: 'static {
    /// Called when an actor is inserted into a `World`.
    ///
    /// This is useful to receive the `Signal` for this actor.
    #[allow(unused_variables)]
    fn register(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        Ok(())
    }

    /// Perform a processing step.
    ///
    /// This function can return `Err` to signal a fatal error to the system.
    /// If this happens, the actor will be stopped and cleaned up appropriately to protect against
    /// inconsistent state.
    ///
    /// You should *always* prefer this over panicking, as this crashes the entire runtime.
    /// Instead of using `unwrap` or `expect`, use `context` from the `anyhow` crate.
    fn process(&mut self, world: &mut World, id: Id) -> Result<(), Error>;
}
