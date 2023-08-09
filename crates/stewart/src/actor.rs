use anyhow::Error;

use crate::{Signal, World};

/// Actor identity and implementation trait.
pub trait Actor: 'static {
    /// Called when an actor is inserted into a `World`.
    ///
    /// This is useful to receive the `Signal` for this actor.
    #[allow(unused_variables)]
    fn register(&mut self, world: &mut World, meta: &mut Meta) -> Result<(), Error> {
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
    fn process(&mut self, world: &mut World, meta: &mut Meta) -> Result<(), Error>;
}

/// Metadata of an `Actor` in a `World`.
pub struct Meta {
    signal: Signal,
    stop: bool,
}

impl Meta {
    pub(crate) fn new(signal: Signal) -> Self {
        Self {
            signal,
            stop: false,
        }
    }

    pub(crate) fn stop(&self) -> bool {
        self.stop
    }

    /// Get a `Signal` that wakes the actor.
    pub fn signal(&self) -> Signal {
        self.signal.clone()
    }

    /// At the end of this processing step, stop the actor.
    pub fn set_stop(&mut self) {
        self.stop = true;
    }
}
