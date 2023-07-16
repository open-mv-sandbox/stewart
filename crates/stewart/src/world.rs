use anyhow::{anyhow, Context as _, Error};
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::{
    any::{ActorEntry, AnyActorEntry},
    schedule::Schedule,
    Actor, CreateError, ProcessError, SendError, StartError,
};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct World {
    actors: Arena<Entry>,
    schedule: Schedule,
    pending_start: Vec<Index>,
}

pub struct Entry {
    pub name: &'static str,
    pub slot: Option<Box<dyn AnyActorEntry>>,
}

impl World {
    /// Create a new actor.
    ///
    /// The given `name` will be used in logging.
    #[instrument("World::create", level = "debug", skip_all)]
    pub fn create(&mut self, name: &'static str) -> Result<Id, CreateError> {
        event!(Level::DEBUG, name, "creating actor");

        let node = Entry { name, slot: None };
        let index = self.actors.insert(node);

        // Track that the actor has to be started
        self.pending_start.push(index);

        let id = Id { index };
        Ok(id)
    }

    /// Start an actor instance at an `Id`.
    #[instrument("World::start", level = "debug", skip_all)]
    pub fn start<A>(&mut self, id: Id, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, "starting actor");

        // Find the node for the actor
        let node = self.actors.get_mut(id.index).context("can't find actor")?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == id.index);
        let Some(pending_index) = maybe_index else {
            return Err(anyhow!("actor already started").into());
        };

        // Give the actor to the node
        let entry = ActorEntry::new(id, actor);
        node.slot = Some(Box::new(entry));

        // Finalize remove pending
        self.pending_start.remove(pending_index);

        Ok(())
    }

    /// Remove any actors that weren't started in time.
    fn timeout_starting(&mut self) -> Result<(), Error> {
        while let Some(actor) = self.pending_start.pop() {
            event!(Level::DEBUG, "actor start timed out");
            self.remove(actor)?;
        }

        Ok(())
    }

    /// Send a message to the actor at the ID.
    #[instrument("World::send", level = "debug", skip_all)]
    pub fn send<M>(&mut self, id: Id, message: M) -> Result<(), SendError>
    where
        M: 'static,
    {
        // Get the actor in tree
        let node = self.actors.get_mut(id.index).context("can't find actor")?;

        // Hand the message to the actor
        let entry = node
            .slot
            .as_mut()
            .context("can't send to processing or starting")?;
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        // Queue for processing
        self.schedule.queue_process(id.index);

        Ok(())
    }

    /// Process all pending messages, until none are left.
    #[instrument("World::run_until_idle", level = "debug", skip_all)]
    pub fn run_until_idle(&mut self) -> Result<(), ProcessError> {
        self.timeout_starting()?;

        while let Some(index) = self.schedule.next() {
            self.process(index).context("failed to process")?;

            self.timeout_starting()?;
        }

        Ok(())
    }

    fn process(&mut self, index: Index) -> Result<(), Error> {
        // Borrow the actor
        let node = self.actors.get_mut(index).context("failed to find actor")?;
        let mut actor = node.slot.take().context("actor unavailable")?;

        let span = span!(Level::INFO, "actor", name = node.name);
        let _entered = span.enter();

        // Process the actor
        event!(Level::DEBUG, "processing actor");
        let stop = actor.process(self);

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

/// ID of an actor slot in a `World`.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Id {
    index: Index,
}
