use std::collections::VecDeque;

use thunderdome::Index;
use tracing::{event, Level};

/// Schedule of tasks to perform on a world and its actors.
#[derive(Default)]
pub struct Schedule {
    process: VecDeque<Index>,
}

impl Schedule {
    pub fn queue_process(&mut self, index: Index) {
        event!(Level::TRACE, "queueing actor for processing");

        if self.process.contains(&index) {
            event!(Level::TRACE, "actor already queued");
            return;
        }

        self.process.push_back(index);
    }

    pub fn dequeue_process(&mut self, index: Index) {
        self.process.retain(|v| *v != index);
    }

    pub fn next(&mut self) -> Option<Index> {
        self.process.pop_front()
    }
}
