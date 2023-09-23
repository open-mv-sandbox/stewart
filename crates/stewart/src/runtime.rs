use std::collections::VecDeque;
use std::ops::ControlFlow;

use anyhow::{Context, Error};
use thiserror::Error;
use thunderdome::{Arena, Index};
use tracing::{event, instrument, span, Level};

use crate::container::ActorContainer;
use crate::{container::AnyActorContainer, Actor, InternalError};

/// Thread-local actor tracking and execution system.
#[derive(Default)]
pub struct Runtime {
    actors: Arena<ActorEntry>,
    queue: VecDeque<Index>,
}

struct ActorEntry {
    name: &'static str,
    container: Option<Box<dyn AnyActorContainer>>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let mut names = Vec::new();

        for (_, node) in &self.actors {
            names.push(node.name);
        }

        if !names.is_empty() {
            event!(
                Level::WARN,
                ?names,
                "actors not cleaned up before runtime drop",
            );
        }
    }
}

impl Runtime {
    /// Insert an actor into the runtime.
    ///
    /// The given `name` will be used in logging.
    #[instrument("Runtime::insert", level = "debug", skip_all)]
    pub fn insert<A>(&mut self, name: &'static str, actor: A) -> Result<Id, InternalError>
    where
        A: Actor,
    {
        event!(Level::DEBUG, name, "inserting actor");

        // Create and insert the actor itself
        let container = ActorContainer::new(actor);
        let entry = ActorEntry {
            name,
            container: Some(Box::new(container)),
        };
        let index = self.actors.insert(entry);

        let id = Id { index };
        Ok(id)
    }

    /// Remove an actor from the runtime.
    #[instrument("Runtime::remove", level = "debug", skip_all)]
    pub fn remove(&mut self, id: Id) -> Result<Result<(), RemoveError>, InternalError> {
        event!(Level::DEBUG, "removing actor");

        // Remove from the queue if it's there
        self.queue.retain(|i| *i != id.index);

        // Remove the actor itself
        let Some(entry) = self.actors.remove(id.index) else {
            return Ok(Err(RemoveError::NotFound));
        };

        event!(Level::DEBUG, name = entry.name, "removed actor");

        Ok(Ok(()))
    }

    /// Send a message to an actor.
    #[instrument("Runtime::send", level = "debug", skip_all)]
    pub fn send<M>(&mut self, id: Id, message: M) -> Result<Result<(), SendError>, InternalError>
    where
        M: 'static,
    {
        event!(Level::TRACE, "sending message to actor");

        // Validate actor exists
        let Some(entry) = self.actors.get_mut(id.index) else {
            return Ok(Err(SendError::NotFound));
        };
        let container = entry
            .container
            .as_mut()
            .context("expected container not available")?;

        // Try applying the message to the actor
        let mut message = Some(message);
        container.push_message(&mut message)?;

        // Check it was actually consumed
        if message.is_some() {
            return Ok(Err(SendError::WrongType));
        }

        self.enqueue(id);

        Ok(Ok(()))
    }

    fn enqueue(&mut self, id: Id) {
        // Don't double-enqueue
        if self.queue.iter().any(|i| *i == id.index) {
            return;
        }

        // Add to the end of queue
        self.queue.push_back(id.index);
    }

    /// Process all pending signalled actors, until none are left pending.
    #[instrument("Runtime::process", level = "debug", skip_all)]
    pub fn process(&mut self) -> Result<(), ProcessError> {
        while let Some(index) = self.queue.pop_front() {
            self.process_actor(index).context("failed to process")?;
        }

        Ok(())
    }

    fn process_actor(&mut self, index: Index) -> Result<(), Error> {
        let (name, mut container) = self.borrow(index)?;

        // TODO: Re-think our usage of tracing, we maybe should use an actor-native logging system.
        let span = span!(Level::INFO, "actor", name);
        let _entered = span.enter();

        // Let the actor's implementation process
        event!(Level::TRACE, "calling actor");
        let id = Id { index };
        let result = container.process(self);

        // Return the actor now that we're done with it
        self.unborrow(index, container)?;

        // Check if an error happened
        let flow = match result {
            Ok(flow) => flow,
            Err(error) => {
                event!(Level::ERROR, "internal error in actor:\n{}", error);
                ControlFlow::Break(())
            }
        };

        // Stop if necessary
        if flow.is_break() {
            span!(Level::DEBUG, "actor control flow break");
            self.remove(id)??;
        }

        Ok(())
    }

    fn borrow(
        &mut self,
        index: Index,
    ) -> Result<(&'static str, Box<dyn AnyActorContainer>), InternalError> {
        let entry = self.actors.get_mut(index).context("failed to find actor")?;
        let container = entry
            .container
            .take()
            .context("expected container not available")?;

        Ok((entry.name, container))
    }

    fn unborrow(
        &mut self,
        index: Index,
        container: Box<dyn AnyActorContainer>,
    ) -> Result<(), InternalError> {
        let entry = self.actors.get_mut(index).context("failed to find actor")?;
        entry.container = Some(container);

        Ok(())
    }
}

/// Identifier of an actor inserted into a runtime.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Id {
    index: Index,
}

/// Failed to remove actor.
#[derive(Error, Debug)]
pub enum RemoveError {
    /// No actor found for id.
    #[error("no actor found for id")]
    NotFound,
}

/// Failed to send message to actor.
#[derive(Error, Debug)]
pub enum SendError {
    /// No actor found for id.
    #[error("no actor found for id")]
    NotFound,

    /// Message wrong type for actor.
    #[error("message wrong type for actor")]
    WrongType,
}

/// Failed to process actors.
#[derive(Error, Debug)]
#[error("failed to process actors")]
pub struct ProcessError {
    #[from]
    source: Error,
}
