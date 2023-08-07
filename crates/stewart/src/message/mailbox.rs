use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Context as _, Error};
use thiserror::Error;

use crate::Signal;

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

struct MailboxInner<M> {
    queue: VecDeque<M>,
    notify: Notify,
}

enum Notify {
    Pending,
    Signal(Signal),
    Floating,
}

impl<M> Default for Mailbox<M> {
    fn default() -> Self {
        let inner = MailboxInner {
            queue: VecDeque::new(),
            notify: Notify::Pending,
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

impl<M> Mailbox<M> {
    /// Create a new `Sender` that senders to this mailbox.
    pub fn sender(&self) -> Sender<M> {
        Sender {
            inner: Rc::downgrade(&self.inner),
        }
    }

    /// Set the `Signal` to be sent when this mailbox receives a message.
    ///
    /// Only one signal can be set at a time, setting this will remove the previous value.
    pub fn set_signal(&self, signal: Signal) {
        self.inner.borrow_mut().notify = Notify::Signal(signal);
    }

    /// Set this mailbox to be managed externally from a `World`, not sending a signal.
    pub fn set_floating(&self) {
        self.inner.borrow_mut().notify = Notify::Floating;
    }

    /// Get the next message, if any is available.
    pub fn recv(&self) -> Option<M> {
        self.inner.borrow_mut().queue.pop_front()
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

        // Notify a listening actor, do this first, so we know there's no inner error first
        match &inner.notify {
            Notify::Pending => {
                return Err(anyhow!("mailbox has no signal and isn't marked floating").into());
            }
            Notify::Signal(signal) => {
                signal.send().context("failed to notify registered actor")?;
            }
            Notify::Floating => {}
        }

        // Apply the message to the queue
        inner.queue.push_back(message);

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

/// Error while sending a message.
#[derive(Error, Debug)]
#[error("sending message failed")]
pub struct SendError {
    #[from]
    source: Error,
}
