use std::collections::BTreeSet;

use anyhow::{Context, Error};

pub struct UniqueQueue<T> {
    queue: Vec<T>,
    set: BTreeSet<T>,
}

impl<T> UniqueQueue<T>
where
    T: Ord + Clone + Copy,
{
    pub fn push(&mut self, value: T) -> Result<(), Error> {
        // Check if it's already in the queue, if it is remove it so we can move it to the end
        if !self.set.insert(value) {
            let index = self
                .queue
                .iter()
                .position(|v| *v == value)
                .context("value in pending stop set, but not in list")?;
            self.queue.remove(index);
        }

        // Add to end of queue
        self.queue.push(value);

        Ok(())
    }

    pub fn peek(&self) -> Option<T> {
        self.queue.last().cloned()
    }

    pub fn pop(&mut self) {
        self.queue.pop();
    }

    pub fn contains(&self, value: T) -> bool {
        self.set.contains(&value)
    }
}

impl<T> Default for UniqueQueue<T> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            set: Default::default(),
        }
    }
}
