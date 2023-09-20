//! Messaging utilities.

mod mailbox;
mod signal;

pub use self::{
    mailbox::{Mailbox, SendError, Sender},
    signal::Signal,
};
