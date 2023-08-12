use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::Duration,
};

use anyhow::{Context as _, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart::Signal;
use tracing::{event, Level};

/// Mio context registry.
pub struct Registry {
    shared: Rc<RefCell<RegistryShared>>,
}

impl Registry {
    pub fn new() -> Result<Self, Error> {
        let poll = Poll::new()?;

        let shared = RegistryShared {
            poll,
            next_token: 0,
            tokens: HashMap::new(),
        };

        let value = Self {
            shared: Rc::new(RefCell::new(shared)),
        };
        Ok(value)
    }

    pub fn handle(&self) -> RegistryRef {
        RegistryRef {
            shared: Rc::downgrade(&self.shared),
        }
    }
}

impl Registry {
    pub(crate) fn poll(&self, events: &mut Events) -> Result<(), Error> {
        let mut inner = self.shared.borrow_mut();

        inner.poll.poll(events, Some(Duration::from_millis(1)))?;

        Ok(())
    }

    pub(crate) fn update_state(&self, token: Token, ready: ReadyState) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();

        let entry = shared
            .tokens
            .get_mut(&token)
            .context("failed to get token entry")?;

        let signal = entry
            .signal
            .as_ref()
            .context("readyness received for token, but no signal set")?;
        signal.send()?;

        // Just in case, unhandled read/write events should be combined together if they have not
        // yet been handled. Mio doesn't garantuee that we'll get another event, and actors should
        // make sure they handle WouldBlock anyways.
        entry.state.readable |= ready.readable;
        entry.state.writable |= ready.writable;

        Ok(())
    }
}

/// Shared weak reference to a registry.
///
/// Actors can use an instance of this to register for receiving mio events.
#[derive(Clone)]
pub struct RegistryRef {
    shared: Weak<RefCell<RegistryShared>>,
}

impl RegistryRef {
    /// Add a source to the registry, registering it with mio.
    ///
    /// You **must** manually deregister too, see mio docs for more information.
    pub fn register<S>(&self, source: &mut S, interest: Interest) -> Result<ReadyRef, Error>
    where
        S: Source,
    {
        let shared = try_shared(&self.shared)?;
        let mut shared = shared.borrow_mut();

        // Store the ready callback
        let token = shared.token();
        let state = ReadyState {
            readable: false,
            writable: false,
        };
        let entry = TokenEntry {
            signal: None,
            state,
        };
        shared.tokens.insert(token, entry);

        // Register with the generated token
        shared.poll.registry().register(source, token, interest)?;

        let ready = ReadyRef {
            shared: self.shared.clone(),
            token,
        };
        Ok(ready)
    }
}

/// Reference to a tracked ready state.
pub struct ReadyRef {
    shared: Weak<RefCell<RegistryShared>>,
    token: Token,
}

impl ReadyRef {
    pub fn reregister<S>(&self, source: &mut S, interest: Interest) -> Result<(), Error>
    where
        S: Source,
    {
        let shared = try_shared(&self.shared)?;
        let shared = shared.borrow();

        shared
            .poll
            .registry()
            .reregister(source, self.token, interest)?;

        Ok(())
    }

    pub fn deregister<S>(&self, source: &mut S)
    where
        S: Source,
    {
        let Ok(shared) = try_shared(&self.shared) else {
            event!(Level::ERROR, "failed to deregister, registry dropped");
            return;
        };
        let mut shared = shared.borrow_mut();

        // Remove from mio registry
        let result = shared.poll.registry().deregister(source);

        // Remove the token entry
        shared.tokens.remove(&self.token);

        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to deregister");
        }
    }

    pub fn set_signal(&self, signal: Signal) -> Result<(), Error> {
        let shared = try_shared(&self.shared)?;
        let mut shared = shared.borrow_mut();

        let entry = shared
            .tokens
            .get_mut(&self.token)
            .context("failed to get token entry")?;
        entry.signal = Some(signal);

        Ok(())
    }

    /// Get the ready state, and reset it for future events.
    pub fn take(&self) -> Result<ReadyState, Error> {
        let shared = try_shared(&self.shared)?;
        let mut shared = shared.borrow_mut();

        let entry = shared
            .tokens
            .get_mut(&self.token)
            .context("failed to get token entry")?;

        let empty = ReadyState {
            readable: false,
            writable: false,
        };
        let state = std::mem::replace(&mut entry.state, empty);

        Ok(state)
    }
}

struct RegistryShared {
    poll: Poll,
    /// TODO: We can start using an `Arena`.
    next_token: usize,
    /// TODO: Since ready events always get 'squashed', we can manually track that in an
    /// Rc<RefCel<_>>, instead of sending around messages.
    tokens: HashMap<Token, TokenEntry>,
}

pub struct TokenEntry {
    signal: Option<Signal>,
    state: ReadyState,
}

#[derive(Debug, Clone)]
pub struct ReadyState {
    pub readable: bool,
    pub writable: bool,
}

impl RegistryShared {
    /// Create a new registry-unique token.
    fn token(&mut self) -> Token {
        let token = self.next_token;
        self.next_token += 1;
        Token(token)
    }
}

fn try_shared(
    shared: &Weak<RefCell<RegistryShared>>,
) -> Result<Rc<RefCell<RegistryShared>>, Error> {
    shared.upgrade().context("registry dropped")
}
