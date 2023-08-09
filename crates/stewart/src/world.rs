use std::collections::VecDeque;

use anyhow::{Context as _, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{signal::SignalReceiver, Actor, Signal};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    receiver: SignalReceiver,
    pending_stop: VecDeque<Index>,
}

struct ActorEntry {
    name: &'static str,
    slot: Option<Box<dyn Actor>>,
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
        event!(Level::DEBUG, name, "creating actor");

        // Create and insert the actor itself
        let entry = ActorEntry {
            name,
            slot: Some(Box::new(actor)),
        };
        let index = self.actors.insert(entry);

        // Track it in the receiver
        self.receiver.register(index);

        // Call the `start` callback to let the actor bind its `Signal`
        self.process_actor(index, Actor::register)
            .context("failed to start")?;

        Ok(Id { index })
    }

    /// Schedule an actor to be stopped and removed.
    pub fn stop(&mut self, id: Id) {
        self.pending_stop.push_back(id.index);
    }

    /// Get a `Signal` for an actor.
    pub fn signal(&self, id: Id) -> Signal {
        self.receiver.signal(id.index)
    }

    /// Process all pending actors, until none are left pending.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.receiver.next()? {
            // Run process step
            self.process_actor(index, Actor::process)
                .context("failed to process")?;

            // Handle pending stop actions
            while let Some(stop) = self.pending_stop.pop_front() {
                self.process_stop(stop)?;
            }
        }

        Ok(())
    }

    fn process_actor<F>(&mut self, index: Index, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut dyn Actor, &mut World, Id) -> Result<(), Error>,
    {
        // Borrow the actor
        let node = self.actors.get_mut(index).context("failed to find actor")?;
        let mut actor = node.slot.take().context("actor unavailable")?;

        // TODO: Re-think our usage of tracing, we maybe should use an actor-native logging system.
        let span = span!(Level::INFO, "actor", name = node.name);
        let _entered = span.enter();

        // Process the actor
        event!(Level::TRACE, "calling into actor");

        // Let the actor's implementation process
        let id = Id { index };
        let result = f(actor.as_mut(), self, id);

        // Check if processing failed
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "error while calling actor");

            // If a processing error happens, the actor should be stopped.
            // It's better to stop than to potentially retain inconsistent state.
            self.stop(id);
        };

        // Return the actor
        let node = self
            .actors
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.slot = Some(actor);

        Ok(())
    }

    fn process_stop(&mut self, index: Index) -> Result<(), Error> {
        event!(Level::DEBUG, "stopping actor");

        self.receiver.unregister(index)?;

        self.actors.remove(index).context("failed to find actor")?;

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

/// Identifier of an actor in a world.
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
