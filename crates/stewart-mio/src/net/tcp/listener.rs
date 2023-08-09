use std::net::SocketAddr;

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Meta, World,
};
use tracing::{event, instrument, Level};

use crate::{
    net::{self, check_io},
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
    pub event_mailbox: Mailbox<net::tcp::StreamEvent>,
    pub actions_sender: Sender<net::tcp::StreamAction>,
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
    let (actor, actions_sender, info) = Service::new(registry, addr, event_sender)?;
    world.insert("tcp-listener", actor)?;

    Ok((actions_sender, info))
}

struct Service {
    registry: RegistryHandle,
    action_mailbox: Mailbox<ListenerAction>,
    ready_mailbox: Mailbox<ReadyEvent>,
    event_sender: Sender<ListenerEvent>,

    listener: mio::net::TcpListener,
    token: Token,
}

impl Service {
    fn new(
        registry: RegistryHandle,
        addr: SocketAddr,
        event_sender: Sender<ListenerEvent>,
    ) -> Result<(Self, Sender<ListenerAction>, ListenerInfo), Error> {
        event!(Level::DEBUG, "binding");

        let action_mailbox = Mailbox::default();
        let ready_mailbox = Mailbox::default();

        let action_sender = action_mailbox.sender();
        let ready_sender = ready_mailbox.sender();

        // Create the socket
        let mut listener = mio::net::TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;

        // Register the socket for ready events
        let token = registry.register(&mut listener, Interest::READABLE, ready_sender.clone())?;

        let value = Self {
            registry,
            action_mailbox,
            ready_mailbox,
            event_sender,

            listener,
            token,
        };
        let listener = ListenerInfo { local_addr };
        Ok((value, action_sender, listener))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.event_sender.send(ListenerEvent::Closed);

        self.registry.deregister(&mut self.listener, self.token);
    }
}

impl Actor for Service {
    fn register(&mut self, _world: &mut World, meta: &mut Meta) -> Result<(), Error> {
        self.action_mailbox.set_signal(meta.signal());
        self.ready_mailbox.set_signal(meta.signal());
        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Meta) -> Result<(), Error> {
        let mut readable = false;
        while let Some(ready) = self.ready_mailbox.recv() {
            readable |= ready.readable;
        }

        if readable {
            self.on_listener_ready(world)?;
        }

        while let Some(_action) = self.action_mailbox.recv() {
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
            let actions_sender = net::tcp::stream::open(
                world,
                self.registry.clone(),
                stream,
                event_mailbox.sender(),
            )?;

            // Notify
            // TODO: Temporarily store the stream, until we get a reply truly accepting the stream.
            //  This allows the caller to screen IPs and related data.
            let event = ConnectedEvent {
                actions_sender,
                event_mailbox,
            };
            self.event_sender.send(ListenerEvent::Connected(event))?;
        }

        Ok(())
    }
}
