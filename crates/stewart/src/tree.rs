use anyhow::{Context, Error};
use thunderdome::{Arena, Index};
use tracing::{event, Level};

use crate::any::AnyActorEntry;

#[derive(Default)]
pub struct Tree {
    nodes: Arena<Node>,
}

pub struct Node {
    pub name: &'static str,
    pub parent: Option<Index>,
    pub entry: Option<Box<dyn AnyActorEntry>>,
}

impl Tree {
    pub fn get_mut(&mut self, index: Index) -> Option<&mut Node> {
        self.nodes.get_mut(index)
    }

    pub fn insert(&mut self, node: Node) -> Result<Index, Error> {
        // Link to the parent
        if let Some(parent) = node.parent {
            self.nodes.get_mut(parent).context("parent not found")?;
        }

        // Insert the node
        let index = self.nodes.insert(node);

        Ok(index)
    }

    pub fn remove<F>(&mut self, index: Index, mut on_removed: F) -> Result<(), Error>
    where
        F: FnMut(Index),
    {
        let mut queue = vec![index];

        while let Some(index) = queue.last().cloned() {
            // Check if all dependents have already stopped
            if !self.check_remove_dependents(index, &mut queue) {
                continue;
            }

            // We verified this actor can be removed, so pop it from the queue and remove it
            queue.pop();
            let node = self
                .nodes
                .remove(index)
                .context("failed to get node to remove")?;
            event!(Level::DEBUG, name = node.name, "removing actor");

            // Notify removed
            on_removed(index);
        }

        Ok(())
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

    pub fn query_debug_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        for (_, node) in &self.nodes {
            names.push(node.name);
        }

        names
    }
}
