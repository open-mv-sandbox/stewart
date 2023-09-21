use std::net::SocketAddr;
use std::ops::ControlFlow;

use anyhow::Error;
use mio::Interest;
use stewart::{
    message::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use tracing::{event, instrument, Level};

use crate::{
    net::{check_io, tcp},
    ReadyRef, RegistryRef,
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
    pub events: Mailbox<tcp::StreamEvent>,
    pub actions: Sender<tcp::StreamAction>,
}

/// Open a TCP stream listener on the given address.
///
/// TCP, unlike UDP, works with ongoing connections.
/// Before a connection is established, you first need to 'listen' for those on a port.
#[instrument("tcp::bind", skip_all)]
pub fn bind(
    world: &mut Runtime,
    registry: RegistryRef,
    addr: SocketAddr,
    event_sender: Sender<ListenerEvent>,
) -> Result<(Sender<ListenerAction>, ListenerInfo), Error> {
    let (actor, signal, info) = Service::new(registry, addr, event_sender)?;
    let actions = actor.actions.sender();

    let id = world.insert("tcp-listener", actor);
    signal.set_id(id);

    Ok((actions, info))
}

struct Service {
    registry: RegistryRef,
    actions: Mailbox<ListenerAction>,
    events: Sender<ListenerEvent>,

    listener: mio::net::TcpListener,
    ready: ReadyRef,
}

impl Service {
    fn new(
        registry: RegistryRef,
        addr: SocketAddr,
        events: Sender<ListenerEvent>,
    ) -> Result<(Self, Signal, ListenerInfo), Error> {
        event!(Level::DEBUG, "binding");

        let signal = Signal::default();
        let actions = Mailbox::new(signal.clone());

        // Create the socket
        let mut listener = mio::net::TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;

        // Register the socket for ready events
        let ready = registry.register(&mut listener, Interest::READABLE, signal.clone())?;

        let this = Self {
            registry,
            actions,
            events,

            listener,
            ready,
        };
        let listener = ListenerInfo { local_addr };
        Ok((this, signal, listener))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.ready.deregister(&mut self.listener);
    }
}

impl Actor for Service {
    fn process(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        let state = self.ready.take().unwrap();

        if state.readable {
            self.on_listener_ready(world).unwrap();
        }

        while let Some(_action) = self.actions.recv() {
            event!(Level::DEBUG, "stopping");
            self.events.send(world, ListenerEvent::Closed).unwrap();
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }
}

impl Service {
    fn on_listener_ready(&mut self, world: &mut Runtime) -> Result<(), Error> {
        // Accept any pending streams
        while let Some((stream, remote_addr)) = check_io(self.listener.accept())? {
            event!(Level::DEBUG, ?remote_addr, "stream accepted");

            // Start actor
            let event_mailbox = Mailbox::floating();
            let actions_sender =
                tcp::stream::open(world, self.registry.clone(), stream, event_mailbox.sender())?;

            // Notify
            // TODO: Temporarily store the stream, until we get a reply truly accepting the stream.
            //  This allows the caller to screen IPs and related data.
            let event = ConnectedEvent {
                actions: actions_sender,
                events: event_mailbox,
            };
            self.events.send(world, ListenerEvent::Connected(event))?;
        }

        Ok(())
    }
}
