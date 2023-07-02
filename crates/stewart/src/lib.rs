#![deny(missing_docs)]

//! A minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod context;
mod handler;
mod schedule;
mod tree;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, State},
    context::{Blackboard, Context},
    handler::Handler,
    world::{Id, World},
};

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
    /// Internal error.
    #[error("internal error")]
    InternalError(InternalError),
}

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
