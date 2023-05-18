use std::collections::BTreeSet;

use anyhow::{Context, Error};

use crate::ActorId;

pub struct StopQueue {
    queue: Vec<(ActorId, StopReason)>,
    set: BTreeSet<ActorId>,
}

impl StopQueue {
    // Push or bump an entry in the queue.
    pub fn enqueue(&mut self, value: ActorId, reason: StopReason) -> Result<(), Error> {
        // Check if it's already in the queue, if it is remove it so we can move it to the end
        if !self.set.insert(value) {
            let index = self
                .queue
                .iter()
                .position(|v| v.0 == value)
                .context("value in pending stop set, but not in list")?;
            self.queue.remove(index);
        }

        // Add to end of queue
        self.queue.push((value, reason));

        Ok(())
    }

    pub fn peek(&self) -> Option<(ActorId, StopReason)> {
        self.queue.last().cloned()
    }

    pub fn pop(&mut self) {
        self.queue.pop();
    }

    pub fn contains(&self, value: ActorId) -> bool {
        self.set.contains(&value)
    }
}

impl Default for StopQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            set: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    StopCalled,
    ParentStopping,
    SystemStopping,
}
