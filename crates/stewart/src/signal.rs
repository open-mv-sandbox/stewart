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
pub struct SignalReceiver {
    shared: Rc<RefCell<SignalShared>>,
}

#[derive(Default)]
struct SignalShared {
    state: Arena<bool>,
    queue: VecDeque<Index>,
}

impl SignalReceiver {
    pub fn track(&self, index: Index) {
        let mut shared = self.shared.borrow_mut();
        shared.state.insert_at(index, false);
    }

    pub fn untrack(&self, index: Index) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();

        let value = shared
            .state
            .remove(index)
            .context("attempted to unregister actor that's not registered")?;
        if value {
            shared.queue.retain(|v| *v != index);
        }

        Ok(())
    }

    pub fn signal(&self, index: Index) -> Signal {
        Signal {
            shared: Rc::downgrade(&self.shared),
            index,
        }
    }

    pub fn next(&self) -> Result<Option<Index>, Error> {
        let mut shared = self.shared.borrow_mut();

        let result = shared.queue.pop_front();

        if let Some(index) = result {
            let state = shared
                .state
                .get_mut(index)
                .context("failed to get state for next in queue")?;
            *state = false;
        }

        Ok(result)
    }
}

/// Sends a signal to schedule an actor for processing in a `World`.
#[derive(Clone)]
pub struct Signal {
    shared: Weak<RefCell<SignalShared>>,
    index: Index,
}

impl Signal {
    /// Send the signal.
    #[instrument("Signal::notify", level = "debug", skip_all)]
    pub fn send(&self) -> Result<(), SendError> {
        event!(Level::DEBUG, "notifying actor");

        let shared = self.shared.upgrade().context("world no longer exists")?;
        let mut shared = shared.borrow_mut();

        // Check actor exists
        let Some(state) = shared.state.get_mut(self.index) else {
            return Err(anyhow!("attempted to signal actor that does not exist").into());
        };

        // Don't double-schedule
        if *state {
            event!(Level::TRACE, "actor already scheduled");
            return Ok(());
        }

        // Add to the end of the queue
        *state = true;
        shared.queue.push_back(self.index);

        Ok(())
    }
}

/// Error while sending signal.
#[derive(Error, Debug)]
#[error("sending signal failed")]
pub struct SendError {
    #[from]
    source: Error,
}
