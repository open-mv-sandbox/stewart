use std::rc::{Rc, Weak};

use anyhow::{Context, Error};
use thiserror::Error;
use thunderdome::Index;
use tracing::instrument;

use crate::schedule::Schedule;

/// Sends a signal to schedule an actor for processing in a `World`.
#[derive(Clone)]
pub struct Signal {
    schedule: Weak<Schedule>,
    index: Index,
}

impl Signal {
    pub(crate) fn new(schedule: &Rc<Schedule>, index: Index) -> Self {
        Self {
            schedule: Rc::downgrade(schedule),
            index,
        }
    }

    /// Send the signal.
    #[instrument("Signal::notify", level = "debug", skip_all)]
    pub fn notify(&self) -> Result<(), NotifyError> {
        let schedule = self.schedule.upgrade().context("world no longer exists")?;
        schedule.schedule(self.index)?;
        Ok(())
    }
}

/// Error while notifying actor.
#[derive(Error, Debug)]
#[error("notifying actor failed")]
pub struct NotifyError {
    #[from]
    source: Error,
}
