use std::rc::Rc;

use tracing::{event, instrument, Level};

use crate::{Id, World};

/// Typed encapsulated callback handler.
///
/// May send a message to one or more actors, after potentially transforming it.
pub struct Handler<M> {
    apply: Apply<M>,
}

enum Apply<M> {
    Noop,
    To(Id),
    Map(Rc<dyn Fn(&mut World, M)>),
}

impl<M> Handler<M>
where
    M: 'static,
{
    /// Create a no-op sender, that does nothing.
    pub fn noop() -> Self {
        Self { apply: Apply::Noop }
    }

    /// Create a sender to a specific actor.
    pub fn to(id: Id) -> Self {
        Self {
            apply: Apply::To(id),
        }
    }

    /// Create a new mapping sender, wrapping the original sender.
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
            apply: Apply::Map(callback),
        }
    }

    /// Apply the handler, potentially sending a message to a receiving actor.
    #[instrument("Handler::handle", level = "debug", skip_all)]
    pub fn handle(&self, world: &mut World, message: M) {
        match &self.apply {
            Apply::Noop => {}
            Apply::To(id) => {
                let result = world.send(*id, message);
                if let Err(error) = result {
                    event!(Level::ERROR, ?error, "failed to send message");
                }
            }
            Apply::Map(callback) => callback(world, message),
        }
    }
}

impl<M> Clone for Handler<M> {
    fn clone(&self) -> Self {
        let apply = match &self.apply {
            Apply::Noop => Apply::Noop,
            Apply::To(index) => Apply::To(*index),
            Apply::Map(callback) => Apply::Map(callback.clone()),
        };

        Self { apply }
    }
}
