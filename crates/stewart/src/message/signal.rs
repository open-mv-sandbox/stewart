use std::{cell::Cell, rc::Rc};

use anyhow::{anyhow, Context, Error};
use thiserror::Error;
use tracing::instrument;

use crate::{Id, World};

/// Shared rebindable handle to for sending a wakeup signal.
///
/// Can be bound *after* creation.
/// This makes initializing actors a lot easier, as you only get the id *after* inserting the actor
/// into the world.
///
/// While every messaging primitive could implement this internally, this instead lets you rebind
/// multiple at once.
#[derive(Default, Clone)]
pub struct Signal {
    shared: Rc<Cell<Option<Id>>>,
}

impl Signal {
    /// Set the id to enqueue on signal send.
    pub fn set_id(&self, id: Id) {
        self.shared.set(Some(id));
    }

    /// Send the signal.
    #[instrument("Signal::send", level = "debug", skip_all)]
    pub fn send(&self, world: &mut World) -> Result<(), SendError> {
        let Some(id) = self.shared.get() else {
            return Err(anyhow!("no id set").into());
        };

        world.enqueue(id).context("failed to enqueue target")?;

        Ok(())
    }
}

/// Failed to send signal.
#[derive(Error, Debug)]
#[error("failed to send signal")]
pub struct SendError {
    #[from]
    source: Error,
}
