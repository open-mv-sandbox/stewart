use anyhow::{Context as _, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{signal::SignalReceiver, Actor, Metadata, Signal};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    receiver: SignalReceiver,
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
    pub fn insert<A>(&mut self, name: &'static str, actor: A) -> Result<Id, Error>
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

        // Track it in the receiver
        self.receiver.track(index);

        // Call the `start` callback to let the actor bind its `Signal`
        self.call_actor(index, Actor::register)
            .context("failed to start")?;

        Ok(Id { index })
    }

    /// Remove an actor from the world.
    #[instrument("World::remove", level = "debug", skip_all)]
    pub fn remove(&mut self, id: Id) -> Result<(), Error> {
        event!(Level::DEBUG, "removing actor");

        self.receiver.untrack(id.index)?;
        self.actors
            .remove(id.index)
            .context("failed to find actor")?;

        Ok(())
    }

    /// Create a signal for the given actor.
    pub fn signal(&self, id: Id) -> Signal {
        self.receiver.signal(id.index)
    }

    /// Process all pending signalled actors, until none are left pending.
    #[instrument("World::process", level = "debug", skip_all)]
    pub fn process(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.receiver.next()? {
            self.call_actor(index, Actor::process)
                .context("failed to process")?;
        }

        Ok(())
    }

    fn call_actor<F>(&mut self, index: Index, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut dyn Actor, &mut World, &mut Metadata) -> Result<(), Error>,
    {
        let (name, mut actor) = self.borrow(index)?;

        // TODO: Re-think our usage of tracing, we maybe should use an actor-native logging system.
        let span = span!(Level::INFO, "actor", name);
        let _entered = span.enter();

        // Let the actor's implementation process
        event!(Level::TRACE, "calling actor");
        let id = Id { index };
        let mut meta = Metadata::new(id);
        let result = f(actor.as_mut(), self, &mut meta);

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

/// Error while processing actors.
#[derive(Error, Debug)]
#[error("failed to process world")]
pub struct ProcessError {
    #[from]
    source: Error,
}
