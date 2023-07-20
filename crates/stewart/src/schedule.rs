use std::{cell::RefCell, collections::VecDeque};

use anyhow::{bail, Context, Error};
use thunderdome::{Arena, Index};
use tracing::{event, Level};

#[derive(Default)]
pub struct Schedule {
    inner: RefCell<ScheduleInner>,
}

#[derive(Default)]
struct ScheduleInner {
    state: Arena<bool>,
    queue: VecDeque<Index>,
}

impl Schedule {
    pub fn register(&self, index: Index) {
        let mut inner = self.inner.borrow_mut();
        inner.state.insert_at(index, false);
    }

    pub fn unregister(&self, index: Index) -> Result<(), Error> {
        let mut inner = self.inner.borrow_mut();

        let value = inner
            .state
            .remove(index)
            .context("attempted to unregister actor that's not registered")?;
        if value {
            inner.queue.retain(|v| *v != index);
        }

        Ok(())
    }

    pub fn schedule(&self, index: Index) -> Result<(), Error> {
        event!(Level::DEBUG, "scheduling actor");
        let mut inner = self.inner.borrow_mut();

        // Check actor exists
        let Some(state) = inner.state.get_mut(index) else {
            bail!("attempted to schedule actor that does not exist");
        };

        // Don't double-schedule
        if *state {
            event!(Level::TRACE, "actor already scheduled");
            return Ok(());
        }

        // Add to the end of the queue
        *state = true;
        inner.queue.push_back(index);
        Ok(())
    }

    pub fn next(&self) -> Result<Option<Index>, Error> {
        let mut inner = self.inner.borrow_mut();

        let result = inner.queue.pop_front();

        if let Some(index) = result {
            let state = inner
                .state
                .get_mut(index)
                .context("failed to get state for next in queue")?;
            *state = false;
        }

        Ok(result)
    }
}
