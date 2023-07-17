use anyhow::{Context as _, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{
    any::{AnyDynActor, DynActor},
    schedule::Schedule,
    Actor,
};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    schedule: Schedule,
}

struct ActorEntry {
    name: &'static str,
    slot: Option<Box<dyn AnyDynActor>>,
}

impl World {
    /// Create a new actor.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::create", level = "debug", skip_all)]
    pub fn create<A>(&mut self, name: &'static str, actor: A) -> Id
    where
        A: Actor,
    {
        event!(Level::DEBUG, name, "creating actor");

        let actor = DynActor::new(actor);
        let entry = ActorEntry {
            name,
            slot: Some(Box::new(actor)),
        };
        let index = self.actors.insert(entry);

        Id { index }
    }

    /// Notify an actor to be processed.
    #[instrument("World::notify", level = "debug", skip_all)]
    pub fn notify(&mut self, id: Id) {
        event!(Level::DEBUG, "notifying actor");

        // Check actor exists
        if !self.actors.contains(id.index) {
            event!(Level::WARN, "attempted to notify actor that does not exist");
            return;
        }

        // Queue for processing
        self.schedule.queue_process(id.index);
    }

    /// Process all pending messages, until none are left.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.schedule.next() {
            self.process(index).context("failed to process")?;
        }

        Ok(())
    }

    fn process(&mut self, index: Index) -> Result<(), Error> {
        // Borrow the actor
        let node = self.actors.get_mut(index).context("failed to find actor")?;
        let mut actor = node.slot.take().context("actor unavailable")?;

        // TODO: Re-think our usage of tracing, we maybe should use an actor-native logging system.
        let span = span!(Level::INFO, "actor", name = node.name);
        let _entered = span.enter();

        // Process the actor
        event!(Level::DEBUG, "processing actor");
        let id = Id { index };
        let stop = actor.process(id, self);

        // Return the actor
        let node = self
            .actors
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.slot = Some(actor);

        // If the actor requested to remove itself, remove it
        if stop {
            self.remove(index)?;
        }

        Ok(())
    }

    fn remove(&mut self, index: Index) -> Result<(), Error> {
        self.schedule.dequeue_process(index);
        self.actors.remove(index);
        Ok(())
    }
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

/// ID of an actor in a `World`.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Id {
    index: Index,
}

/// Error while processing actors.
#[derive(Error, Debug)]
#[error("processing world failed")]
pub struct ProcessError {
    #[from]
    source: Error,
}
