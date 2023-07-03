use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Error;
use mio::{event::Source, Interest, Poll, Token};
use stewart::Handler;

/// Shared mio context registry.
///
/// Actors can use an instance of this registry to register wake events.
/// The registry is created by the event loop.
pub struct Registry {
    // TODO: Remove need for pub(crate) here
    pub(crate) poll: RefCell<Poll>,
    next_token: AtomicUsize,
    pub(crate) wake_senders: RefCell<HashMap<Token, Handler<WakeEvent>>>,
}

impl Registry {
    pub(crate) fn new(poll: Poll) -> Self {
        Self {
            poll: RefCell::new(poll),
            next_token: AtomicUsize::new(0),
            wake_senders: Default::default(),
        }
    }

    pub(crate) fn register<S>(
        &self,
        wake: Handler<WakeEvent>,
        source: &mut S,
        interest: Interest,
    ) -> Result<Token, Error>
    where
        S: Source,
    {
        // Get the next poll token
        let index = self.next_token.fetch_add(1, Ordering::SeqCst);
        let token = Token(index);

        // Store the waker callback
        self.wake_senders.borrow_mut().insert(token, wake);

        // Register with the generated token
        self.poll
            .borrow()
            .registry()
            .register(source, token, interest)?;

        Ok(token)
    }

    pub(crate) fn reregister<S>(
        &self,
        source: &mut S,
        token: Token,
        interest: Interest,
    ) -> Result<(), Error>
    where
        S: Source,
    {
        self.poll
            .borrow()
            .registry()
            .reregister(source, token, interest)?;

        Ok(())
    }
}

pub struct WakeEvent {
    pub read: bool,
    pub write: bool,
}
