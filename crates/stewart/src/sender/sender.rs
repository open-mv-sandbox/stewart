use std::rc::Rc;

use crate::{Id, InternalError, Runtime, SendError};

/// Utility for sending messages to an actor.
pub struct Sender<M> {
    kind: SenderKind<M>,
}

enum SenderKind<M> {
    Direct { target: Id },
    Map { apply: Rc<MapFn<M>> },
}

type MapFn<M> = dyn Fn(&mut Runtime, M) -> Result<Result<(), SendError>, InternalError>;

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        let kind = match &self.kind {
            SenderKind::Direct { target } => SenderKind::Direct { target: *target },
            SenderKind::Map { apply } => SenderKind::Map {
                apply: apply.clone(),
            },
        };

        Self { kind }
    }
}

impl<M> Sender<M>
where
    M: 'static,
{
    /// Create a new sender to a target actor.
    pub fn new(target: Id) -> Self {
        Self {
            kind: SenderKind::Direct { target },
        }
    }

    /// Wrap the sender in a mapping sender.
    ///
    /// This lets you translate between message types quick and cheap.
    pub fn map<I, F>(self, map: F) -> Sender<I>
    where
        F: Fn(I) -> M + 'static,
    {
        let apply = move |rt: &mut _, message| {
            let message = map(message);
            self.send(rt, message)
        };

        let kind = SenderKind::Map {
            apply: Rc::new(apply),
        };
        Sender { kind }
    }

    /// Send a message using the sender.
    pub fn send(
        &self,
        rt: &mut Runtime,
        message: M,
    ) -> Result<Result<(), SendError>, InternalError> {
        match &self.kind {
            SenderKind::Direct { target } => rt.send(*target, message),
            SenderKind::Map { apply } => apply(rt, message),
        }
    }
}
