use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context as _, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart::{Handler, World};
use tracing::{event, Level};

/// Shared mio context registry.
///
/// Actors can use an instance of this registry to register wake events.
/// The registry is created by the event loop.
pub struct Registry {
    poll: RefCell<Poll>,
    next_token: AtomicUsize,
    wake_handlers: RefCell<HashMap<Token, Handler<WakeEvent>>>,
}

impl Registry {
    pub(crate) fn new(poll: Poll) -> Self {
        Self {
            poll: RefCell::new(poll),
            next_token: AtomicUsize::new(0),
            wake_handlers: Default::default(),
        }
    }

    pub(crate) fn poll(&self, events: &mut Events) -> Result<(), Error> {
        self.poll.borrow_mut().poll(events, None)?;
        Ok(())
    }

    pub(crate) fn wake(
        &self,
        world: &mut World,
        token: Token,
        readable: bool,
        writable: bool,
    ) -> Result<(), Error> {
        event!(Level::TRACE, "sending wake");

        // Get the wake handler for this token
        let wake_handlers = self.wake_handlers.borrow();
        let handler = wake_handlers
            .get(&token)
            .context("failed to get wake handler")?;

        // Send out the message
        handler.handle(world, WakeEvent { readable, writable })?;

        Ok(())
    }

    /// Create a new unique token for this registry.
    pub fn token(&self) -> Token {
        let index = self.next_token.fetch_add(1, Ordering::SeqCst);
        Token(index)
    }

    pub fn register<S>(
        &self,
        source: &mut S,
        token: Token,
        interest: Interest,
        wake: Handler<WakeEvent>,
    ) -> Result<(), Error>
    where
        S: Source,
    {
        // Store the waker callback
        self.wake_handlers.borrow_mut().insert(token, wake);

        // Register with the generated token
        self.poll
            .borrow()
            .registry()
            .register(source, token, interest)?;

        Ok(())
    }

    pub fn reregister<S>(
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

    pub fn deregister<S>(&self, source: &mut S) -> Result<(), Error>
    where
        S: Source,
    {
        self.poll.borrow().registry().deregister(source)?;
        Ok(())
    }
}

pub struct WakeEvent {
    pub readable: bool,
    pub writable: bool,
}
