use std::rc::Rc;

use tracing::instrument;

use crate::{Id, SendError, World};

/// Typed encapsulated callback handler.
///
/// May send a message to one or more actors, after potentially transforming it.
pub struct Handler<M> {
    apply: Kind<M>,
}

enum Kind<M> {
    None,
    To(Id),
    Map(Rc<dyn Fn(&mut World, M) -> Result<(), SendError>>),
}

impl<M> Handler<M>
where
    M: 'static,
{
    /// Create a 'none' handler, that does nothing.
    pub fn none() -> Self {
        Self { apply: Kind::None }
    }

    /// Create a handler to a specific actor.
    pub fn to(id: Id) -> Self {
        Self {
            apply: Kind::To(id),
        }
    }

    /// Create a new mapping handler, wrapping the original handler.
    pub fn map<F, I>(self, callback: F) -> Handler<I>
    where
        F: Fn(I) -> M + 'static,
    {
        let callback = move |world: &mut World, message: I| {
            let message = callback(message);
            self.handle(world, message)
        };
        let callback = Rc::new(callback);

        Handler {
            apply: Kind::Map(callback),
        }
    }

    /// Apply the handler, potentially sending a message to a receiving actor.
    #[instrument("Handler::handle", level = "debug", skip_all)]
    pub fn handle(&self, world: &mut World, message: M) -> Result<(), SendError> {
        match &self.apply {
            Kind::None => Ok(()),
            Kind::To(id) => world.send(*id, message),
            Kind::Map(callback) => callback(world, message),
        }
    }
}

impl<M> Clone for Handler<M> {
    fn clone(&self) -> Self {
        let apply = match &self.apply {
            Kind::None => Kind::None,
            Kind::To(index) => Kind::To(*index),
            Kind::Map(callback) => Kind::Map(callback.clone()),
        };

        Self { apply }
    }
}
