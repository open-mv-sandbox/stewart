#![deny(missing_docs)]

//! Actors, done well.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod context;
pub mod message;
mod signal;
mod world;

pub use self::{
    actor::Actor,
    context::Context,
    signal::Signal,
    world::{ProcessError, World},
};
