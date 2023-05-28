use std::collections::VecDeque;

use anyhow::{Context, Error};
use thunderdome::Index;
use tracing::{event, Level};

use crate::{InternalError, World};

/// Schedule of tasks to perform on a world and its actors.
#[derive(Default)]
pub struct Schedule {
    process: VecDeque<Index>,
    stop: Vec<Index>,
}

impl Schedule {
    pub(crate) fn queue_process(&mut self, index: Index) {
        event!(Level::TRACE, "queueing actor for processing");

        if self.process.contains(&index) {
            event!(Level::TRACE, "actor already queued");
            return;
        }

        self.process.push_back(index);
    }

    pub(crate) fn queue_stop(&mut self, index: Index) {
        self.stop.push(index);
    }

    /// Process all pending messages, until none are left.
    pub fn run_until_idle(&mut self, world: &mut World) -> Result<(), InternalError> {
        self.process_pending(world)
            .context("failed to process pending")?;

        while let Some(index) = self.process.pop_front() {
            world.process(self, index).context("failed to process")?;

            self.process_pending(world)
                .context("failed to process pending")?;
        }

        Ok(())
    }

    fn process_pending(&mut self, world: &mut World) -> Result<(), Error> {
        world.timeout_starting();

        for index in self.stop.drain(..) {
            world.remove(index)
        }

        Ok(())
    }
}
