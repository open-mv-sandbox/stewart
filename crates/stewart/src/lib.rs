#![deny(missing_docs)]

//! Stewart is a minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod context;
mod schedule;
mod sender;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, State},
    context::{Context, StartError},
    schedule::Schedule,
    sender::Sender,
    world::World,
};

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
