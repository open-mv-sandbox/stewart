#![deny(missing_docs)]

//! Stewart is a minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod any;
mod context;
mod sender;
mod tree;
mod unique_queue;
mod world;

use anyhow::Error;
use thiserror::Error;

pub use self::{
    actor::{Actor, State},
    context::{Context, StartError},
    sender::Sender,
    world::World,
};

/// Internal error, this is always a bug.
#[derive(Error, Debug)]
#[error("internal error, this is a bug")]
pub struct InternalError(#[from] Error);
