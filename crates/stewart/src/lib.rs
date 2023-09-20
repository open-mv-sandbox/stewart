#![deny(missing_docs, unsafe_code)]

//! Actors, done well.
//!
//! This is an API reference for the stewart rust library. For a detailed user guide, read the
//! stewart book.

mod actor;
pub mod message;
mod world;

pub use self::{
    actor::{Actor, Metadata},
    world::{Id, ProcessError, World},
};
