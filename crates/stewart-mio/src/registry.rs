use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::Duration,
};

use anyhow::{Context as _, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart_message::Sender;
use tracing::{event, Level};

/// Mio context registry.
pub struct Registry {
    inner: Rc<RefCell<RegistryInner>>,
}

impl Registry {
    pub fn new() -> Result<Self, Error> {
        let poll = Poll::new()?;

        let inner = RegistryInner {
            poll,
            next_token: 0,
            ready_senders: HashMap::new(),
        };

        let value = Self {
            inner: Rc::new(RefCell::new(inner)),
        };
        Ok(value)
    }

    pub fn handle(&self) -> RegistryHandle {
        RegistryHandle {
            inner: Rc::downgrade(&self.inner),
        }
    }
}

impl Registry {
    pub(crate) fn poll(&self, events: &mut Events) -> Result<(), Error> {
        let mut inner = self.inner.borrow_mut();

        inner.poll.poll(events, Some(Duration::from_millis(1)))?;

        Ok(())
    }

    pub(crate) fn send(&self, token: Token, ready: ReadyEvent) -> Result<(), Error> {
        let inner = self.inner.borrow();

        let sender = inner
            .ready_senders
            .get(&token)
            .context("failed to get ready sender")?;

        sender.send(ready)?;

        Ok(())
    }
}

/// Shared handle to access a registry.
///
/// Actors can use an instance of this to register for receiving mio events.
#[derive(Clone)]
pub struct RegistryHandle {
    inner: Weak<RefCell<RegistryInner>>,
}

impl RegistryHandle {
    /// Add a source to the registry, registering it with mio.
    ///
    /// You **must** manually deregister too, see mio docs for more information.
    pub fn register<S>(
        &self,
        source: &mut S,
        interest: Interest,
        sender: Sender<ReadyEvent>,
    ) -> Result<Token, Error>
    where
        S: Source,
    {
        let inner = self.try_inner()?;
        let mut inner = inner.borrow_mut();

        // Store the ready callback
        let token = inner.token();
        inner.ready_senders.insert(token, sender);

        // Register with the generated token
        inner.poll.registry().register(source, token, interest)?;

        Ok(token)
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
        let inner = self.try_inner()?;
        let inner = inner.borrow();

        inner.poll.registry().reregister(source, token, interest)?;

        Ok(())
    }

    pub fn deregister<S>(&self, source: &mut S, token: Token)
    where
        S: Source,
    {
        let Ok(inner) = self.try_inner() else {
            event!(Level::ERROR, "failed to deregister, registry dropped");
            return;
        };
        let mut inner = inner.borrow_mut();

        // Remove from mio registry
        let result = inner.poll.registry().deregister(source);

        // Remove the ready callback
        inner.ready_senders.remove(&token);

        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to deregister");
        }
    }

    fn try_inner(&self) -> Result<Rc<RefCell<RegistryInner>>, Error> {
        self.inner.upgrade().context("registry dropped")
    }
}

pub struct RegistryInner {
    poll: Poll,
    next_token: usize,
    /// TODO: Since ready events always get 'squashed', we can manually track that in an
    /// Rc<RefCel<_>>, instead of sending around messages.
    ready_senders: HashMap<Token, Sender<ReadyEvent>>,
}

impl RegistryInner {
    /// Create a new registry-unique token.
    fn token(&mut self) -> Token {
        let token = self.next_token;
        self.next_token += 1;
        Token(token)
    }
}

#[derive(Debug)]
pub struct ReadyEvent {
    pub token: Token,
    pub readable: bool,
    pub writable: bool,
}
