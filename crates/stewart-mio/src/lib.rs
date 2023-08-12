#![deny(unsafe_code)]

mod event_loop;
pub mod net;
mod registry;

pub use self::{
    event_loop::run_event_loop,
    registry::{ReadyEvent, Registry, RegistryHandle},
};
