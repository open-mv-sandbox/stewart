use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Error};
use thiserror::Error;

use crate::{Id, World};

/// Shared *single-threaded* multi-sender multi-receiver message queue.
///
/// An instance of `Mailbox` is considered 'authoritative'.
/// You can use it to create senders, and register actors that should be notified.
///
/// You are not actually required to a mailbox, if your actor has a different mechanism for being
/// notified.
pub struct Mailbox<M> {
    inner: Rc<RefCell<MailboxInner<M>>>,
}

impl<M> Default for Mailbox<M> {
    fn default() -> Self {
        let inner = MailboxInner {
            queue: VecDeque::new(),
            notify: None,
        };
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }
}

impl<M> Clone for Mailbox<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct MailboxInner<M> {
    queue: VecDeque<M>,
    notify: Option<Id>,
}

impl<M> Mailbox<M> {
    /// Register an actor to be notified when this mailbox receives a message.
    ///
    /// Only one actor can be registered at a time, setting this will remove the previous
    /// registration.
    pub fn register(&self, id: Id) {
        self.inner.borrow_mut().notify = Some(id);
    }

    /// Get the next message, if any is available.
    pub fn next(&self) -> Option<M> {
        self.inner.borrow_mut().queue.pop_front()
    }

    /// Create a sender for sending messages to this mailbox.
    pub fn sender(&self) -> Sender<M> {
        Sender {
            inner: Some(Rc::downgrade(&self.inner)),
        }
    }
}

/// Sending utility, for sending messages to a mailbox.
pub struct Sender<M> {
    inner: Option<Weak<RefCell<MailboxInner<M>>>>,
}

impl<M> Sender<M> {
    /// Create no-op sender, that always succeeds but throws away the message.
    ///
    /// TODO: Consider if this is behavior we want at all? Creating mailboxes is now a lot easier
    /// than handlers were earlier, and no longer need an actor.
    pub fn none() -> Self {
        Self { inner: None }
    }

    /// Send a message to the target mailbox of this sender.
    pub fn send(&self, world: &mut World, message: M) -> Result<(), SendError> {
        // Check if we have a mailbox or if we're no-op
        let Some(inner) = self.inner.as_ref() else {
            return Ok(())
        };

        // Check if the mailbox is still available
        let Some(inner) = inner.upgrade() else {
            return Err(anyhow!("mailbox closed").into())
        };
        let mut inner = inner.borrow_mut();

        // Check if there's an actor listening
        let Some(notify) = inner.notify else {
            return Err(anyhow!("no actor listening").into())
        };

        // Finally, apply the message to the queue and notify
        inner.queue.push_back(message);
        world.notify(notify);

        Ok(())
    }
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Error while sending message.
///
/// This happens if the receiving mailbox no longer exists, or isn't registered to an actor.
#[derive(Error, Debug)]
#[error("sending message failed")]
pub struct SendError {
    #[from]
    source: Error,
}
