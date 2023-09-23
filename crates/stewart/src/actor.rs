use anyhow::Error;
use std::ops::ControlFlow;
use thiserror::Error;

use crate::Runtime;

/// Actor identity and processing implementation trait.
pub trait Actor: 'static {
    /// The message type this actor processes.
    ///
    /// This does not necessarily need to be the public exposed message type.
    /// Most actors will have a different internal message type that's not exposed.
    type Message;

    /// Process a message.
    fn handle(
        &mut self,
        rt: &mut Runtime,
        message: Self::Message,
    ) -> Result<ControlFlow<()>, ActorError>;
}

/// Internal error in actor.
#[derive(Error, Debug)]
#[error("internal error in actor")]
pub struct ActorError {
    #[from]
    source: Error,
}
