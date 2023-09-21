use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

use anyhow::{anyhow, Context, Error};
use thiserror::Error;

use crate::{message::Signal, Runtime};

/// Shared *single-threaded* multi-sender multi-receiver message queue.
///
/// An instance of `Mailbox` is considered 'authoritative'.
/// You can use it to create senders, and set the actors that should be notified.
pub struct Mailbox<M> {
    shared: Rc<RefCell<MailboxShared<M>>>,
}

struct MailboxShared<M> {
    queue: VecDeque<M>,
    notify: Notify,
}

enum Notify {
    Signal(Signal),
    Floating,
}

impl<M> Mailbox<M> {
    /// Create a new mailbox.
    pub fn new(signal: Signal) -> Self {
        let shared = MailboxShared {
            queue: VecDeque::new(),
            notify: Notify::Signal(signal),
        };

        Self {
            shared: Rc::new(RefCell::new(shared)),
        }
    }

    /// Create a new floating mailbox.
    ///
    /// Floating mailboxes do not signal an actor, but can still receive messages like normal.
    pub fn floating() -> Self {
        let shared = MailboxShared {
            queue: VecDeque::new(),
            notify: Notify::Floating,
        };

        Self {
            shared: Rc::new(RefCell::new(shared)),
        }
    }

    /// Create a new `Sender` that senders to this mailbox.
    pub fn sender(&self) -> Sender<M> {
        Sender {
            shared: Rc::downgrade(&self.shared),
        }
    }

    /// Set the `Signal` to be sent when this mailbox receives a message.
    ///
    /// Only one signal can be set at a time, setting this will remove the previous value.
    pub fn set_signal(&self, signal: Signal) {
        self.shared.borrow_mut().notify = Notify::Signal(signal);
    }

    /// Set this mailbox to be managed externally from a `World`, not sending a signal.
    pub fn set_floating(&self) {
        self.shared.borrow_mut().notify = Notify::Floating;
    }

    /// Get the next message, if any is available.
    pub fn recv(&self) -> Option<M> {
        self.shared.borrow_mut().queue.pop_front()
    }
}

/// Handle for sending messages to a mailbox.
pub struct Sender<M> {
    shared: Weak<RefCell<MailboxShared<M>>>,
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<M> Sender<M> {
    /// Send a message to the target mailbox of this sender.
    pub fn send(&self, world: &mut Runtime, message: M) -> Result<(), SendError> {
        // Check if the mailbox is still available
        let Some(shared) = self.shared.upgrade() else {
            return Err(anyhow!("mailbox closed").into())
        };
        let mut shared = shared.borrow_mut();

        // Notify a listening actor, do this first, so we know there's no inner error first
        match &shared.notify {
            Notify::Signal(signal) => {
                signal
                    .send(world)
                    .context("failed to send signal for message")?;
            }
            Notify::Floating => {}
        }

        // Apply the message to the queue
        shared.queue.push_back(message);

        Ok(())
    }
}

/// Error while sending a message.
#[derive(Error, Debug)]
#[error("failed to send message")]
pub struct SendError {
    #[from]
    source: Error,
}
