use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Context, Error};
use thiserror::Error;

use stewart::Signal;

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
    notify: Option<Signal>,
}

impl<M> Mailbox<M> {
    /// Register an actor to be notified when this mailbox receives a message.
    ///
    /// Only one actor can be registered at a time, setting this will remove the previous
    /// registration.
    pub fn register(&self, signal: Signal) {
        self.inner.borrow_mut().notify = Some(signal);
    }

    /// Get the next message, if any is available.
    pub fn next(&self) -> Option<M> {
        self.inner.borrow_mut().queue.pop_front()
    }

    /// Create a sender for sending messages to this mailbox.
    pub fn sender(&self) -> Sender<M> {
        Sender {
            inner: Rc::downgrade(&self.inner),
        }
    }
}

/// Sending utility, for sending messages to a mailbox.
pub struct Sender<M> {
    inner: Weak<RefCell<MailboxInner<M>>>,
}

impl<M> Sender<M> {
    /// Send a message to the target mailbox of this sender.
    pub fn send(&self, message: M) -> Result<(), SendError> {
        // Check if the mailbox is still available
        let Some(inner) = self.inner.upgrade() else {
            return Err(anyhow!("mailbox closed").into())
        };
        let mut inner = inner.borrow_mut();

        // Apply the message to the queue and notify
        inner.queue.push_back(message);

        // Notify a listening actor
        if let Some(signal) = inner.notify.as_ref() {
            signal
                .notify()
                .context("failed to notify registered actor")?;
        }

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
/// This happens if the receiving mailbox no longer exists.
#[derive(Error, Debug)]
#[error("sending message failed")]
pub struct SendError {
    #[from]
    source: Error,
}
