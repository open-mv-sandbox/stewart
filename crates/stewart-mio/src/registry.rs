use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use anyhow::{Context as _, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart::World;
use stewart_message::Sender;
use tracing::{event, Level};

/// Shared mio context registry.
///
/// Actors can use an instance of this registry to register ready events.
/// The registry is created by the event loop.
pub struct Registry {
    poll: RefCell<Poll>,
    next_token: AtomicUsize,
    ready_senders: RefCell<HashMap<Token, Sender<Ready>>>,
}

impl Registry {
    pub(crate) fn new(poll: Poll) -> Self {
        Self {
            poll: RefCell::new(poll),
            next_token: AtomicUsize::new(0),
            ready_senders: Default::default(),
        }
    }

    pub(crate) fn poll(&self, events: &mut Events) -> Result<(), Error> {
        let mut poll = self.poll.borrow_mut();
        poll.poll(events, Some(Duration::from_millis(1)))?;
        Ok(())
    }

    pub(crate) fn send_ready(
        &self,
        world: &mut World,
        token: Token,
        readable: bool,
        writable: bool,
    ) -> Result<(), Error> {
        event!(Level::TRACE, "sending ready");

        // Get the ready handler for this token
        let ready_senders = self.ready_senders.borrow();
        let sender = ready_senders
            .get(&token)
            .context("failed to get ready sender")?;

        // Send out the message
        sender.send(world, Ready { readable, writable })?;

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
        sender: Sender<Ready>,
    ) -> Result<(), Error>
    where
        S: Source,
    {
        // Store the ready callback
        self.ready_senders.borrow_mut().insert(token, sender);

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

pub struct Ready {
    pub readable: bool,
    pub writable: bool,
}
