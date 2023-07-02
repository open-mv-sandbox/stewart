use std::{marker::PhantomData, sync::atomic::AtomicPtr};

use thunderdome::Index;

use crate::Actor;

/// Typed actor handle, for performing operations on an actor.
///
/// TODO: This is only used for creation, and maybe should be replaced with a create init function.
pub struct Handle<A> {
    pub(crate) index: Index,
    _a: PhantomData<AtomicPtr<A>>,
}

impl<A> Handle<A>
where
    A: Actor,
{
    pub(crate) fn new(index: Index) -> Self {
        Self {
            index,
            _a: PhantomData,
        }
    }
}

impl<A> Copy for Handle<A> {}

impl<A> Clone for Handle<A> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _a: PhantomData,
        }
    }
}
