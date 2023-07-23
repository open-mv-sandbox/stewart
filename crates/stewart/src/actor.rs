use anyhow::Error;

use crate::Context;

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
}
