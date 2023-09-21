use std::ops::ControlFlow;

use crate::Runtime;

/// Actor identity and processing implementation trait.
///
/// Implementations can return `Err` to signal a fatal error to the system.
/// If this happens, the actor will be stopped and cleaned up appropriately to protect against
/// inconsistent state.
///
/// You should *always* prefer this over panicking, as this crashes the entire runtime.
/// Instead of using `unwrap` or `expect`, use `context` from the `anyhow` crate.
pub trait Actor: 'static {
    // TODO: Re-introduce one-message-per-call.

    /// Processing step implementation.
    fn process(&mut self, world: &mut Runtime) -> ControlFlow<()>;
}
