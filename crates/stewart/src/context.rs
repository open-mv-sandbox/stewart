use std::ops::{Deref, DerefMut};

use thunderdome::Index;

use crate::{Signal, World};

/// The context of an actor.
///
/// Bundles information and state of the actor for processing.
pub struct Context<'a> {
    world: &'a mut World,
    index: Index,
}

impl<'a> Context<'a> {
    pub(crate) fn actor(world: &'a mut World, index: Index) -> Self {
        Self { world, index }
    }

    /// Get a `Signal` instance for the current actor.
    pub fn signal(&self) -> Signal {
        self.world.signal(self.index)
    }
}

impl<'a> Deref for Context<'a> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl<'a> DerefMut for Context<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
