#![deny(missing_docs, unsafe_code)]

//! Actors, done well.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod container;
mod runtime;
pub mod sender;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, ActorError},
    runtime::{Id, ProcessError, RemoveError, Runtime, SendError},
};

/// Internal error in stewart.
#[derive(Error, Debug)]
#[error("internal error in stewart")]
pub struct InternalError {
    #[from]
    source: Error,
}
