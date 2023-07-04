#![deny(missing_docs)]

//! A minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod handler;
mod schedule;
mod tree;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, Context},
    handler::Handler,
    world::{Id, World},
};

/// Error on actor starting.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum StartError {
    /// Invalid actor ID, this can be one of:
    ///
    /// - The actor has already been started.
    /// - The actor cannot be found.
    /// - The ID you have given isn't associated with an actor.
    #[error("invalid actor id")]
    InvalidId,
    /// Internal error.
    #[error("internal error")]
    InternalError(#[from] InternalError),
}

/// Error on sending.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SendError {
    /// Invalid actor ID, this can be one of:
    ///
    /// - The actor cannot be found.
    /// - The ID you have given isn't associated with an actor.
    /// - The found actor doesn't accept messages of this type.
    #[error("invalid actor id")]
    InvalidId,
    /// Actor is currently processing, you cannot send a message to a processing actor.
    #[error("can't send to processing actor")]
    Processing,
    /// Internal error.
    #[error("internal error")]
    InternalError(#[from] InternalError),
}

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
