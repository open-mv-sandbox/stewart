use std::rc::Rc;

use anyhow::{Context as _, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{schedule::Schedule, Actor, After, Context, Signal};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    schedule: Rc<Schedule>,
}

struct ActorEntry {
    name: &'static str,
    slot: Option<Box<dyn Actor>>,
}

impl World {
    /// Create a new actor.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::create", level = "debug", skip_all)]
    pub fn create<A>(&mut self, name: &'static str, actor: A) -> Signal
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

        // Track it in the schedule
        self.schedule.register(index);

        self.create_signal(index)
    }

    pub(crate) fn create_signal(&self, index: Index) -> Signal {
        Signal::new(&self.schedule, index)
    }

    /// Process all pending actors, until none are left pending.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.schedule.next()? {
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

        let mut ctx = Context::actor(self, index);

        // Let the actor's implementation process
        let result = actor.process(&mut ctx);

        // Check if processing failed
        let after = match result {
            Ok(after) => after,
            Err(error) => {
                event!(Level::ERROR, ?error, "error while processing");

                // If a processing error happens, the actor should be stopped.
                // It's better to stop than to potentially retain inconsistent state.
                After::Stop
            }
        };

        // Return the actor
        let node = self
            .actors
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.slot = Some(actor);

        // If the actor requested to stop, stop it
        if after == After::Stop {
            self.stop(index).context("failed to stop actor")?;
        }

        Ok(())
    }

    fn stop(&mut self, index: Index) -> Result<(), Error> {
        event!(Level::DEBUG, "stopping actor");

        self.schedule.unregister(index)?;

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

/// Error while processing actors.
#[derive(Error, Debug)]
#[error("processing world failed")]
pub struct ProcessError {
    #[from]
    source: Error,
}
