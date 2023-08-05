#![deny(missing_docs)]

//! Messaging utilities for stewart.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod mailbox;

pub use self::mailbox::{mailbox, Mailbox, SendError, Sender};
