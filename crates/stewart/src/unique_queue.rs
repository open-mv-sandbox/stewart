use std::{collections::HashSet, hash::Hash};

use anyhow::{Context, Error};

pub struct UniqueQueue<V, R> {
    queue: Vec<(V, R)>,
    set: HashSet<V>,
}

impl<V, R> UniqueQueue<V, R>
where
    V: Hash + Eq + PartialEq + Clone,
    R: Clone,
{
    // Push or bump an entry in the queue.
    pub fn enqueue(&mut self, value: V, reason: R) -> Result<(), Error> {
        // Check if it's already in the queue, if it is remove it so we can move it to the end
        if !self.set.insert(value.clone()) {
            let index = self
                .queue
                .iter()
                .position(|(v, _)| *v == value)
                .context("value in pending stop set, but not in list")?;
            self.queue.remove(index);
        }

        // Add to end of queue
        self.queue.push((value, reason));

        Ok(())
    }

    pub fn peek(&self) -> Option<(V, R)> {
        self.queue.last().cloned()
    }

    pub fn pop(&mut self) -> Result<(), Error> {
        let (value, _) = self.queue.pop().context("failed to pop queue value")?;
        self.set.remove(&value);
        Ok(())
    }

    pub fn contains(&self, value: V) -> bool {
        self.set.contains(&value)
    }
}

impl<V, R> Default for UniqueQueue<V, R> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            set: Default::default(),
        }
    }
}
