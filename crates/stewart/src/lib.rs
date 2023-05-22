#![deny(missing_docs)]

//! Stewart is a minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod context;
mod tree;
mod unique_queue;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, Options, State},
    context::Context,
    world::{Addr, World},
};

/// Error on actor creation.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CreateError {
    /// Parent not found.
    #[error("parent not found")]
    ParentNotFound,
}

/// Error on actor starting.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum StartError {
    /// Can't start root.
    #[error("cant start root")]
    CantStartRoot,
    /// The actor has already been started.
    #[error("actor already started")]
    ActorAlreadyStarted,
    /// The actor couldn't be found.
    #[error("actor not found")]
    ActorNotFound,
}

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
