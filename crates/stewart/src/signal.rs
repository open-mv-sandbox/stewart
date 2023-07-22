use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Context, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, Level};

#[derive(Default)]
struct ScheduleBackend {
    state: Arena<bool>,
    queue: VecDeque<Index>,
}

/// Sends a signal to schedule an actor for processing in a `World`.
#[derive(Clone)]
pub struct Signal {
    backend: Weak<RefCell<ScheduleBackend>>,
    index: Index,
}

impl Signal {
    /// Send the signal.
    #[instrument("Signal::notify", level = "debug", skip_all)]
    pub fn send(&self) -> Result<(), SignalError> {
        event!(Level::DEBUG, "notifying actor");

        let backend = self.backend.upgrade().context("world no longer exists")?;
        let mut backend = backend.borrow_mut();

        // Check actor exists
        let Some(state) = backend.state.get_mut(self.index) else {
            return Err(anyhow!("attempted to schedule actor that does not exist").into());
        };

        // Don't double-schedule
        if *state {
            event!(Level::TRACE, "actor already scheduled");
            return Ok(());
        }

        // Add to the end of the queue
        *state = true;
        backend.queue.push_back(self.index);

        Ok(())
    }
}

#[derive(Default)]
pub struct SignalReceiver {
    backend: Rc<RefCell<ScheduleBackend>>,
}

impl SignalReceiver {
    pub fn register(&self, index: Index) {
        let mut backend = self.backend.borrow_mut();
        backend.state.insert_at(index, false);
    }

    pub fn unregister(&self, index: Index) -> Result<(), Error> {
        let mut backend = self.backend.borrow_mut();

        let value = backend
            .state
            .remove(index)
            .context("attempted to unregister actor that's not registered")?;
        if value {
            backend.queue.retain(|v| *v != index);
        }

        Ok(())
    }

    pub fn signal(&self, index: Index) -> Signal {
        Signal {
            backend: Rc::downgrade(&self.backend),
            index,
        }
    }

    pub fn next(&self) -> Result<Option<Index>, Error> {
        let mut backend = self.backend.borrow_mut();

        let result = backend.queue.pop_front();

        if let Some(index) = result {
            let state = backend
                .state
                .get_mut(index)
                .context("failed to get state for next in queue")?;
            *state = false;
        }

        Ok(result)
    }
}

/// Error while sending signal.
#[derive(Error, Debug)]
#[error("sending signal failed")]
pub struct SignalError {
    #[from]
    source: Error,
}
