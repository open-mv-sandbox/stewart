#![deny(missing_docs)]

//! Stewart is a minimalist, hndigh-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod context;
mod handle;
mod schedule;
mod sender;
mod tree;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, State},
    context::Context,
    handle::Handle,
    sender::Sender,
    world::World,
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
}

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
