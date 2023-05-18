#![deny(missing_docs)]

//! Stewart is a minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod system;
mod tree;
mod unique_queue;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    system::{State, System, SystemOptions},
    tree::ActorId,
    world::{Addr, SystemId, World},
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
    /// The actor has already been started.
    #[error("actor already started")]
    ActorAlreadyStarted,
    /// The actor couldn't be found.
    #[error("actor not found")]
    ActorNotFound,
    /// The system couldn't be found.
    #[error("system not found")]
    SystemNotFound,
    /// The system is unavailable.
    #[error("system unavailable")]
    SystemUnavailable,
    /// Actor instance's type is wrong.
    #[error("instance wrong type")]
    InstanceWrongType,
}

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);

// TODO: Consider if all API functions should either return at least `Result<?, InternalError>`,
// or if internal errors shouldn't ever be 'leaked'. They probably should be bubbled up, because
// an internal error ignored could lead to security bugs.
