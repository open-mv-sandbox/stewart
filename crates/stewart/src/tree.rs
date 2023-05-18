use anyhow::Error;
use thunderdome::{Arena, Index};

use crate::{CreateError, SystemId};

#[derive(Default)]
pub struct Tree {
    nodes: Arena<Node>,
}

impl Tree {
    pub fn insert(&mut self, node: Node) -> Result<ActorId, CreateError> {
        // Link to the parent
        if let Some(parent) = node.parent {
            self.nodes
                .get_mut(parent.index)
                .ok_or(CreateError::ParentNotFound)?;
        }

        // Insert the node
        let index = self.nodes.insert(node);

        Ok(ActorId { index })
    }

    pub fn get(&self, actor: ActorId) -> Option<&Node> {
        self.nodes.get(actor.index)
    }

    pub fn get_mut(&mut self, actor: ActorId) -> Option<&mut Node> {
        self.nodes.get_mut(actor.index)
    }

    /// Remove a node.
    ///
    /// Warning: This doesn't check the node doesn't have any children, leaving those orphaned if
    /// not removed first.
    pub fn remove(&mut self, actor: ActorId) -> Option<Node> {
        self.nodes.remove(actor.index)
    }

    /// Query the children of an actor.
    pub fn query_children<F>(&self, actor: ActorId, mut on_child: F) -> Result<(), Error>
    where
        F: FnMut(ActorId) -> Result<(), Error>,
    {
        // TODO: Optimize hierarchy walking
        let children = self.nodes.iter().filter(|(_, n)| n.parent() == Some(actor));
        for (index, _) in children {
            on_child(ActorId { index })?;
        }

        Ok(())
    }
}

/// Handle referencing an actor in a `World`.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ActorId {
    index: Index,
}

pub struct Node {
    system: Option<SystemId>,
    parent: Option<ActorId>,
}

impl Node {
    pub fn new(parent: Option<ActorId>) -> Self {
        Self {
            system: None,
            parent,
        }
    }

    pub fn system(&self) -> Option<SystemId> {
        self.system
    }

    pub fn set_system(&mut self, value: Option<SystemId>) {
        self.system = value;
    }

    pub fn parent(&self) -> Option<ActorId> {
        self.parent
    }
}
