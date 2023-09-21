use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

use anyhow::{Context, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart::{message::Signal, Runtime};
use thunderdome::{Arena, Index};
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
            tokens: Arena::new(),
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

    pub(crate) fn update_state(
        &self,
        world: &mut Runtime,
        token: Token,
        ready: ReadyState,
    ) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();

        let index = Index::from_bits(token.0 as u64).context("invalid token")?;
        let entry = shared
            .tokens
            .get_mut(index)
            .context("failed to get token entry")?;

        entry.signal.send(world)?;

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
    pub fn register<S>(
        &self,
        source: &mut S,
        interest: Interest,
        signal: Signal,
    ) -> Result<ReadyRef, Error>
    where
        S: Source,
    {
        let shared = try_shared(&self.shared)?;
        let mut shared = shared.borrow_mut();

        // Store the ready callback
        let state = ReadyState {
            readable: false,
            writable: false,
        };
        let entry = TokenEntry { signal, state };
        let index = shared.tokens.insert(entry);

        // Register with the generated token
        let token = Token(index.to_bits() as usize);
        shared.poll.registry().register(source, token, interest)?;

        let ready = ReadyRef {
            shared: self.shared.clone(),
            index,
        };
        Ok(ready)
    }
}

/// Reference to a tracked ready state.
#[derive(Clone)]
pub struct ReadyRef {
    shared: Weak<RefCell<RegistryShared>>,
    index: Index,
}

impl ReadyRef {
    pub fn reregister<S>(&self, source: &mut S, interest: Interest) -> Result<(), Error>
    where
        S: Source,
    {
        let shared = try_shared(&self.shared)?;
        let shared = shared.borrow();

        let token = Token(self.index.to_bits() as usize);
        shared.poll.registry().reregister(source, token, interest)?;

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
        shared.tokens.remove(self.index);

        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to deregister");
        }
    }

    /// Get the ready state, and reset it for future events.
    pub fn take(&self) -> Result<ReadyState, Error> {
        let shared = try_shared(&self.shared)?;
        let mut shared = shared.borrow_mut();

        let entry = shared
            .tokens
            .get_mut(self.index)
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
    tokens: Arena<TokenEntry>,
}

struct TokenEntry {
    signal: Signal,
    state: ReadyState,
}

#[derive(Debug, Clone)]
pub struct ReadyState {
    pub readable: bool,
    pub writable: bool,
}

fn try_shared(
    shared: &Weak<RefCell<RegistryShared>>,
) -> Result<Rc<RefCell<RegistryShared>>, Error> {
    shared.upgrade().context("registry dropped")
}
