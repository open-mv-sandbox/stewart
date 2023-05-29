use anyhow::{Context as _, Error};
use thunderdome::{Arena, Index};
use tracing::{event, span, Level};

use crate::{
    any::{ActorEntry, AnyActorEntry},
    Actor, Context, Schedule, StartError,
};

/// Hierarchical actor collection.
#[derive(Default)]
pub struct World {
    nodes: Arena<Node>,
    pending_start: Vec<Index>,
}

struct Node {
    name: &'static str,
    parent: Option<Index>,
    entry: Option<Box<dyn AnyActorEntry>>,
}

impl World {
    pub(crate) fn create(
        &mut self,
        name: &'static str,
        parent: Option<Index>,
    ) -> Result<Index, Error> {
        // Link to the parent
        if let Some(parent) = parent {
            self.nodes.get_mut(parent).context("parent not found")?;
        }

        // Insert the node
        let node = Node {
            name,
            parent,
            entry: None,
        };
        let index = self.nodes.insert(node);

        // Track that the actor has to be started
        self.pending_start.push(index);

        Ok(index)
    }

    pub(crate) fn start<A>(&mut self, index: Index, actor: A) -> Result<(), StartError>
    where
        A: Actor,
    {
        // Find the node for the actor
        let node = self.nodes.get_mut(index).ok_or(StartError::ActorNotFound)?;

        // Validate if it's not started yet
        let maybe_index = self.pending_start.iter().position(|v| *v == index);
        let pending_index = if let Some(value) = maybe_index {
            value
        } else {
            return Err(StartError::ActorAlreadyStarted);
        };

        // Give the actor to the node
        let entry = ActorEntry::new(actor);
        node.entry = Some(Box::new(entry));

        // Finalize remove pending
        self.pending_start.remove(pending_index);

        Ok(())
    }

    /// Remove any actors that weren't started in time
    pub(crate) fn timeout_starting(&mut self) {
        while let Some(actor) = self.pending_start.pop() {
            self.remove(actor);
        }
    }

    pub(crate) fn queue_message<M>(&mut self, index: Index, message: M) -> Result<(), Error>
    where
        M: 'static,
    {
        // Get the actor in tree
        let node = self.nodes.get_mut(index).context("failed to find actor")?;

        // Hand the message to the actor
        let entry = node.entry.as_mut().context("actor unavailable")?;
        let mut message = Some(message);
        entry.enqueue(&mut message)?;

        Ok(())
    }

    pub(crate) fn process(&mut self, schedule: &mut Schedule, index: Index) -> Result<(), Error> {
        // Borrow the actor
        let node = self.nodes.get_mut(index).context("failed to find actor")?;
        let mut actor = node.entry.take().context("actor unavailable")?;

        let span = span!(Level::INFO, "actor", name = node.name);
        let _entered = span.enter();
        event!(Level::DEBUG, "processing actor");

        // Run the process handler
        let mut ctx = Context::new(self, schedule, Some(index));
        actor.process(&mut ctx);
        let stop = actor.is_stop_requested();

        // Return the actor
        let node = self
            .nodes
            .get_mut(index)
            .context("failed to find actor for return")?;
        node.entry = Some(actor);

        // If the actor requested to remove itself, remove it
        if stop {
            self.remove(index);
        }

        Ok(())
    }

    /// Remove actor and its hierarchy.
    fn remove(&mut self, index: Index) {
        let mut queue = vec![index];

        while let Some(index) = queue.last().cloned() {
            // Check if all dependents have already stopped
            if !self.check_remove_dependents(index, &mut queue) {
                continue;
            }

            // We verified this actor can be removed, so pop it from the queue and remove it
            event!(Level::DEBUG, "removing actor");
            queue.pop();
            self.nodes.remove(index);
        }
    }

    /// Returns true if all dependencies have been removed.
    fn check_remove_dependents(&mut self, index: Index, queue: &mut Vec<Index>) -> bool {
        let mut ready = true;

        for (child_index, node) in &self.nodes {
            if node.parent != Some(index) {
                continue;
            }

            queue.push(child_index);
            ready = false;
        }

        ready
    }

    fn query_debug_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        for (_, node) in &self.nodes {
            names.push(node.name);
        }

        names
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let debug_names = self.query_debug_names();

        if !debug_names.is_empty() {
            event!(
                Level::WARN,
                ?debug_names,
                "actors not cleaned up before world drop",
            );
        }
    }
}
