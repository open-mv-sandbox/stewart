use std::rc::Rc;

use thunderdome::Index;
use tracing::{event, instrument, Level};

use crate::Context;

/// Typed message sender, optionally bundling message mapping.
pub struct Sender<M> {
    apply: Apply<M>,
}

enum Apply<M> {
    Noop,
    Direct(Index),
    Callback(Rc<dyn Fn(&mut Context, M)>),
}

impl<M> Sender<M>
where
    M: 'static,
{
    /// Create a no-op sender, that does nothing.
    pub fn noop() -> Self {
        Self { apply: Apply::Noop }
    }

    pub(crate) fn direct(index: Index) -> Self {
        Self {
            apply: Apply::Direct(index),
        }
    }

    /// Create a new mapping sender, wrapping the original sender.
    pub fn map<F, I>(self, callback: F) -> Sender<I>
    where
        F: Fn(I) -> M + 'static,
    {
        let callback = move |ctx: &mut Context, message: I| {
            let message = callback(message);
            self.send(ctx, message)
        };
        let callback = Rc::new(callback);

        Sender {
            apply: Apply::Callback(callback),
        }
    }

    /// Apply the sender, potentially sending a message to a receiving actor.
    #[instrument("Sender::send", skip_all)]
    pub fn send(&self, ctx: &mut Context, message: M) {
        match &self.apply {
            Apply::Noop => {}
            Apply::Direct(index) => {
                let result = ctx.send(*index, message);

                // TODO: What to do with this error?
                if let Err(error) = result {
                    event!(Level::ERROR, ?error, "failed to send message");
                }
            }
            Apply::Callback(callback) => callback(ctx, message),
        }
    }
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        let apply = match &self.apply {
            Apply::Noop => Apply::Noop,
            Apply::Direct(index) => Apply::Direct(*index),
            Apply::Callback(callback) => Apply::Callback(callback.clone()),
        };

        Self { apply }
    }
}
