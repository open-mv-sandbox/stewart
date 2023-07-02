use std::{marker::PhantomData, sync::atomic::AtomicPtr};

use thunderdome::Index;

use crate::{utils::Sender, Actor};

/// Typed actor handle, for performing operations on an actor.
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

    /// Create a sender that sends messages to this actor.
    ///
    /// TODO: Remove dependencies on utils.
    pub fn sender(&self) -> Sender<A::Message> {
        Sender::new_send(self.index)
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
