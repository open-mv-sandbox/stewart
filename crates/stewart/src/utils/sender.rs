use std::rc::Rc;

use anyhow::Error;
use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::{Handle, World};

// TODO: Convert all usages of internal-only APIs to use public only interfaces.
// TODO: Rename to `Handler` once `Handle` is removed.

/// Typed encapsulated message sender.
///
/// May send a message to one or more actors, after potentially transforming it.
pub struct Sender<M> {
    apply: Apply<M>,
}

enum Apply<M> {
    Noop,
    To(Index),
    Map(Rc<dyn Fn(&mut World, M)>),
}

impl<M> Sender<M>
where
    M: 'static,
{
    /// Create a no-op sender, that does nothing.
    pub fn noop() -> Self {
        Self { apply: Apply::Noop }
    }

    /// Create a sender to a specific actor.
    pub fn to<A>(hnd: Handle<A>) -> Self {
        Self {
            apply: Apply::To(hnd.index),
        }
    }

    /// Create a new mapping sender, wrapping the original sender.
    pub fn map<F, I>(self, callback: F) -> Sender<I>
    where
        F: Fn(I) -> M + 'static,
    {
        let callback = move |world: &mut World, message: I| {
            let message = callback(message);
            self.send(world, message)
        };
        let callback = Rc::new(callback);

        Sender {
            apply: Apply::Map(callback),
        }
    }

    /// Apply the sender, potentially sending a message to a receiving actor.
    #[instrument("Sender::send", level = "debug", skip_all)]
    pub fn send(&self, world: &mut World, message: M) {
        match &self.apply {
            Apply::Noop => {}
            Apply::To(index) => {
                let result = Self::try_send_direct(world, *index, message);

                // TODO: What to do with this error?
                if let Err(error) = result {
                    event!(Level::ERROR, ?error, "failed to send message");
                }
            }
            Apply::Map(callback) => callback(world, message),
        }
    }

    fn try_send_direct(world: &mut World, index: Index, message: M) -> Result<(), Error> {
        world.send(index, message)?;
        Ok(())
    }
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        let apply = match &self.apply {
            Apply::Noop => Apply::Noop,
            Apply::To(index) => Apply::To(*index),
            Apply::Map(callback) => Apply::Map(callback.clone()),
        };

        Self { apply }
    }
}
