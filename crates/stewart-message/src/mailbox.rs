use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Context, Error};
use thiserror::Error;

use stewart::Signal;

pub fn mailbox<M>() -> (Mailbox<M>, Sender<M>) {
    let inner = MailboxInner {
        queue: VecDeque::new(),
        notify: None,
    };
    let inner = Rc::new(RefCell::new(inner));
    let weak = Rc::downgrade(&inner);

    let mailbox = Mailbox { inner };
    let sender = Sender { inner: weak };

    (mailbox, sender)
}

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
    /// Set the `Signal` to be sent when this mailbox receives a message.
    ///
    /// Only one signal can be set at a time, setting this will remove the previous value.
    pub fn set_signal(&self, signal: Signal) {
        self.inner.borrow_mut().notify = Some(signal);
    }

    /// Get the next message, if any is available.
    ///
    /// Recv will fail if all senders are dropped.
    /// This is because, all users of this mailbox being dropped probably means something has gone
    /// wrong.
    /// Making sure we fail if this happens prevents inconsistent behavior with dangling resources.
    pub fn recv(&self) -> Result<Option<M>, RecvError> {
        let count = Rc::weak_count(&self.inner);
        if count == 0 {
            return Err(anyhow!("all senders dropped").into());
        }

        let next = self.inner.borrow_mut().queue.pop_front();
        Ok(next)
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
            signal.send().context("failed to notify registered actor")?;
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
/// This happens if the mailbox senders no longer exists.
#[derive(Error, Debug)]
#[error("receiving messages failed")]
pub struct RecvError {
    #[from]
    source: Error,
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
