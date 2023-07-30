use anyhow::{Context as _, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{signal::SignalReceiver, Actor, Context, Signal};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<ActorEntry>,
    receiver: SignalReceiver,
}

struct ActorEntry {
    name: &'static str,
    slot: Option<Box<dyn Actor>>,
}

impl World {
    pub(crate) fn signal(&self, index: Index) -> Signal {
        self.receiver.signal(index)
    }

    /// Insert an actor into the world.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::create", level = "debug", skip_all)]
    pub fn insert<A>(&mut self, name: &'static str, actor: A) -> Result<(), Error>
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
        self.actor_do(index, Actor::register)
            .context("failed to start")?;

        Ok(())
    }

    /// Process all pending actors, until none are left pending.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.receiver.next()? {
            self.actor_do(index, Actor::process)
                .context("failed to process")?;
        }

        Ok(())
    }

    fn actor_do<F>(&mut self, index: Index, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut dyn Actor, &mut Context) -> Result<(), Error>,
    {
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
        let result = f(actor.as_mut(), &mut ctx);

        // Check if processing failed
        let stop = match result {
            Ok(()) => ctx.stop(),
            Err(error) => {
                event!(Level::ERROR, ?error, "error while processing");

                // If a processing error happens, the actor should be stopped.
                // It's better to stop than to potentially retain inconsistent state.
                true
            }
        };

        // Return the actor
        let node = self
            .actors
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.slot = Some(actor);

        // If the actor requested to stop, stop it
        if stop {
            self.stop(index)
                .context("failed to stop actor after error")?;
        }

        Ok(())
    }

    fn stop(&mut self, index: Index) -> Result<(), Error> {
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

/// Error while processing actors.
#[derive(Error, Debug)]
#[error("processing world failed")]
pub struct ProcessError {
    #[from]
    source: Error,
}
