use anyhow::{Context, Error};
use thunderdome::{Arena, Index};

use crate::{any::AnyActorEntry, Options};

#[derive(Default)]
pub struct Tree {
    nodes: Arena<Node>,
}

impl Tree {
    pub fn insert(&mut self, node: Node) -> Result<Index, Error> {
        // Link to the parent
        if let Some(parent) = node.parent {
            self.nodes.get_mut(parent).context("parent not found")?;
        }

        // Insert the node
        let index = self.nodes.insert(node);

        Ok(index)
    }

    pub fn get_mut(&mut self, actor: Index) -> Option<&mut Node> {
        self.nodes.get_mut(actor)
    }

    /// Remove a node.
    ///
    /// Warning: This doesn't check the node doesn't have any children, leaving those orphaned if
    /// not removed first.
    pub fn remove(&mut self, actor: Index) -> Option<Node> {
        self.nodes.remove(actor)
    }

    /// Query the children of an actor.
    pub fn query_children<F>(&self, actor: Index, mut on_child: F) -> Result<(), Error>
    where
        F: FnMut(Index) -> Result<(), Error>,
    {
        // TODO: Optimize hierarchy walking
        let children = self.nodes.iter().filter(|(_, n)| n.parent() == Some(actor));
        for (index, _) in children {
            on_child(index)?;
        }

        Ok(())
    }

    pub fn query_debug_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        for (_, node) in &self.nodes {
            let name = node
                .entry
                .as_ref()
                .map(|e| e.debug_name())
                .unwrap_or("Unknown");
            names.push(name);
        }

        names
    }
}

pub struct Node {
    entry: Option<Box<dyn AnyActorEntry>>,
    parent: Option<Index>,
    options: Options,
}

impl Node {
    pub fn new(parent: Option<Index>, options: Options) -> Self {
        Self {
            entry: None,
            parent,
            options,
        }
    }

    pub fn entry_mut(&mut self) -> &mut Option<Box<dyn AnyActorEntry>> {
        // TODO: This function, can be replaced with some convenience functions
        //  for getting/setting/borrowing/etc actors.
        &mut self.entry
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn parent(&self) -> Option<Index> {
        self.parent
    }
}
