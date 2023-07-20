#![deny(missing_docs)]

//! A minimalist, high-performance, modular, and non-exclusive actor system.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
mod schedule;
mod signal;
mod world;

pub use self::{
    actor::{Actor, Context},
    signal::Signal,
    world::{ProcessError, World},
};
