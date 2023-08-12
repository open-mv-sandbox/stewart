use std::net::SocketAddr;

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use tracing::{event, instrument, Level};

use crate::{
    net::{check_io, tcp},
    ReadyEvent, RegistryHandle,
};

pub enum ListenerAction {
    /// Close the listener.
    Close,
}

pub struct ListenerInfo {
    pub local_addr: SocketAddr,
}

pub enum ListenerEvent {
    Connected(ConnectedEvent),
    Closed,
}

pub struct ConnectedEvent {
    pub events: Mailbox<tcp::ConnectionEvent>,
    pub actions: Sender<tcp::ConnectionAction>,
}

/// Open a TCP stream listener on the given address.
///
/// TCP, unlike UDP, works with ongoing connections.
/// Before a connection is established, you first need to 'listen' for those on a port.
#[instrument("tcp::listen", skip_all)]
pub fn bind(
    world: &mut World,
    registry: RegistryHandle,
    addr: SocketAddr,
    event_sender: Sender<ListenerEvent>,
) -> Result<(Sender<ListenerAction>, ListenerInfo), Error> {
    let (actor, info) = Service::new(registry, addr, event_sender)?;
    let actions = actor.actions.sender();
    world.insert("tcp-listener", actor)?;

    Ok((actions, info))
}

struct Service {
    registry: RegistryHandle,
    actions: Mailbox<ListenerAction>,
    events: Sender<ListenerEvent>,
    ready: Mailbox<ReadyEvent>,

    listener: mio::net::TcpListener,
    token: Token,
}

impl Service {
    fn new(
        registry: RegistryHandle,
        addr: SocketAddr,
        events: Sender<ListenerEvent>,
    ) -> Result<(Self, ListenerInfo), Error> {
        event!(Level::DEBUG, "binding");

        let actions = Mailbox::default();
        let ready = Mailbox::default();

        // Create the socket
        let mut listener = mio::net::TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;

        // Register the socket for ready events
        let token = registry.register(&mut listener, Interest::READABLE, ready.sender())?;

        let value = Self {
            registry,
            actions,
            events,
            ready,

            listener,
            token,
        };
        let listener = ListenerInfo { local_addr };
        Ok((value, listener))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.events.send(ListenerEvent::Closed);

        self.registry.deregister(&mut self.listener, self.token);
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());

        self.actions.set_signal(signal.clone());
        self.ready.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let mut readable = false;
        while let Some(ready) = self.ready.recv() {
            readable |= ready.readable;
        }

        if readable {
            self.on_listener_ready(world)?;
        }

        while let Some(_action) = self.actions.recv() {
            meta.set_stop();
        }

        Ok(())
    }
}

impl Service {
    fn on_listener_ready(&mut self, world: &mut World) -> Result<(), Error> {
        // Accept any pending streams
        while let Some((stream, remote_addr)) = check_io(self.listener.accept())? {
            event!(Level::DEBUG, ?remote_addr, "stream accepted");

            // Start actor
            let event_mailbox = Mailbox::default();
            let actions_sender =
                tcp::stream::open(world, self.registry.clone(), stream, event_mailbox.sender())?;

            // Notify
            // TODO: Temporarily store the stream, until we get a reply truly accepting the stream.
            //  This allows the caller to screen IPs and related data.
            let event = ConnectedEvent {
                actions: actions_sender,
                events: event_mailbox,
            };
            self.events.send(ListenerEvent::Connected(event))?;
        }

        Ok(())
    }
}
