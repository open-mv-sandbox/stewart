#![deny(missing_docs)]

//! A minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod handler;
mod schedule;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, Context},
    handler::Handler,
    world::{Id, World},
};

/// Error while creating actor.
#[derive(Error, Debug)]
#[error("creating actor failed")]
pub struct CreateError {
    #[from]
    source: Error,
}

/// Error while starting actor.
#[derive(Error, Debug)]
#[error("starting actor failed")]
pub struct StartError {
    #[from]
    source: Error,
}

/// Error while sending message.
#[derive(Error, Debug)]
#[error("sending message failed")]
pub struct SendError {
    #[from]
    source: Error,
}

/// Error while sending message.
#[derive(Error, Debug)]
#[error("process world failed")]
pub struct ProcessError {
    #[from]
    source: Error,
}
