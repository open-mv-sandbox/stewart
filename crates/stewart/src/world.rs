use std::collections::VecDeque;

use anyhow::{anyhow, Context, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{Actor, Metadata};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    queue: VecDeque<Index>,
}

struct ActorEntry {
    name: &'static str,
    actor: Option<Box<dyn Actor>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let mut names = Vec::new();

        for (_, node) in &self.actors {
            names.push(node.name);
        }

        if !names.is_empty() {
            event!(
                Level::WARN,
                ?names,
                "actors not cleaned up before world drop",
            );
        }
    }
}

impl World {
    /// Insert an actor into the world.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::insert", level = "debug", skip_all)]
    pub fn insert<A>(&mut self, name: &'static str, actor: A) -> Id
    where
        A: Actor,
    {
        event!(Level::DEBUG, name, "inserting actor");

        // Create and insert the actor itself
        let entry = ActorEntry {
            name,
            actor: Some(Box::new(actor)),
        };
        let index = self.actors.insert(entry);

        Id { index }
    }

    /// Remove an actor from the world.
    #[instrument("World::remove", level = "debug", skip_all)]
    pub fn remove(&mut self, id: Id) -> Result<(), RemoveError> {
        event!(Level::DEBUG, "removing actor");

        // Remove from the queue if it's there
        self.queue.retain(|i| *i != id.index);

        // Remove the actor itself
        self.actors
            .remove(id.index)
            .context("failed to find actor")?;

        Ok(())
    }

    /// Enqueue the actor for processing.
    pub fn enqueue(&mut self, id: Id) -> Result<(), EnqueueError> {
        event!(Level::TRACE, "enqueuing actor");

        // Validate actor exists
        if !self.actors.contains(id.index) {
            return Err(anyhow!("tried to enqueue actor that doesn't exist").into());
        }

        // Don't double-enqueue
        if self.queue.iter().any(|i| *i == id.index) {
            return Ok(());
        }

        // Add to the end
        self.queue.push_back(id.index);

        Ok(())
    }

    /// Process all pending signalled actors, until none are left pending.
    #[instrument("World::process", level = "debug", skip_all)]
    pub fn process(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.queue.pop_front() {
            self.process_actor(index).context("failed to process")?;
        }

        Ok(())
    }

    fn process_actor(&mut self, index: Index) -> Result<(), Error> {
        let (name, mut actor) = self.borrow(index)?;

        // TODO: Re-think our usage of tracing, we maybe should use an actor-native logging system.
        let span = span!(Level::INFO, "actor", name);
        let _entered = span.enter();

        // Let the actor's implementation process
        event!(Level::TRACE, "calling actor");
        let id = Id { index };
        let mut meta = Metadata::new(id);
        let result = actor.process(self, &mut meta);

        // Check if processing failed
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "error while calling actor");

            // If a processing error happens, the actor should be stopped.
            // It's better to stop than to potentially retain inconsistent state.
            meta.set_stop();
        };

        self.unborrow(index, actor)?;

        // Stop if necessary
        if meta.stop() {
            self.remove(id)?;
        }

        Ok(())
    }

    fn borrow(&mut self, index: Index) -> Result<(&'static str, Box<dyn Actor>), Error> {
        let entry = self.actors.get_mut(index).context("failed to find actor")?;
        let actor = entry.actor.take().context("actor unavailable")?;

        Ok((entry.name, actor))
    }

    fn unborrow(&mut self, index: Index, actor: Box<dyn Actor>) -> Result<(), Error> {
        let entry = self
            .actors
            .get_mut(index)
            .context("failed to find actor for return")?;
        entry.actor = Some(actor);

        Ok(())
    }
}

/// Identifier of an actor inserted into a world.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Id {
    index: Index,
}

/// failed to remove actor.
#[derive(Error, Debug)]
#[error("failed to remove actor")]
pub struct RemoveError {
    #[from]
    source: Error,
}

/// Failed to enqueue actor.
#[derive(Error, Debug)]
#[error("failed to enqueue actor")]
pub struct EnqueueError {
    #[from]
    source: Error,
}

/// Failed to process actors.
#[derive(Error, Debug)]
#[error("failed to process actors")]
pub struct ProcessError {
    #[from]
    source: Error,
}
