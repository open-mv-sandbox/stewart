use std::collections::VecDeque;

use anyhow::Context;
use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::{InternalError, World};

/// Schedule of tasks to perform on a world and its actors.
#[derive(Default)]
pub struct Schedule {
    process: VecDeque<Index>,
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

    /// Process all pending messages, until none are left.
    #[instrument("Schedule::run_until_idle", skip_all)]
    pub fn run_until_idle(&mut self, world: &mut World) -> Result<(), InternalError> {
        world.timeout_starting();

        while let Some(index) = self.process.pop_front() {
            world.process(self, index).context("failed to process")?;

            world.timeout_starting();
        }

        Ok(())
    }
}
