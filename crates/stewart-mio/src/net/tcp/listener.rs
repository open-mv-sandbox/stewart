use std::{net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::Interest;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, instrument, Level};

use crate::{
    net::{self, check_io},
    Ready, Registry,
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
    registry: Rc<Registry>,
    addr: SocketAddr,
    event_sender: Sender<ListenerEvent>,
) -> Result<(Sender<ListenerAction>, ListenerInfo), Error> {
    let (actor, actions_sender, info) = Service::new(registry, addr, event_sender)?;
    world.insert("tcp-listener", actor)?;

    Ok((actions_sender, info))
}

struct Service {
    registry: Rc<Registry>,
    actions_mailbox: Mailbox<ListenerAction>,
    ready_mailbox: Mailbox<Ready>,
    event_sender: Sender<ListenerEvent>,

    listener: mio::net::TcpListener,
}

impl Service {
    fn new(
        registry: Rc<Registry>,
        addr: SocketAddr,
        event_sender: Sender<ListenerEvent>,
    ) -> Result<(Self, Sender<ListenerAction>, ListenerInfo), Error> {
        event!(Level::DEBUG, "binding");

        let (actions_mailbox, actions_sender) = mailbox();
        let (ready_mailbox, ready_sender) = mailbox();

        // Create the socket
        let mut listener = mio::net::TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;

        // Register the socket for ready events
        let listener_token = registry.token();
        registry.register(
            &mut listener,
            listener_token,
            Interest::READABLE,
            ready_sender.clone(),
        )?;

        let value = Self {
            registry,
            actions_mailbox,
            ready_mailbox,
            event_sender,

            listener,
        };
        let listener = ListenerInfo { local_addr };
        Ok((value, actions_sender, listener))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");
        let _ = self.event_sender.send(ListenerEvent::Closed);
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.actions_mailbox.set_signal(ctx.signal());
        self.ready_mailbox.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        let mut readable = false;
        while let Some(ready) = self.ready_mailbox.recv() {
            readable |= ready.readable;
        }

        if readable {
            self.on_listener_ready(ctx)?;
        }

        while let Some(_action) = self.actions_mailbox.recv() {
            ctx.set_stop();
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
            let (event_mailbox, sender) = mailbox();
            let actions_sender =
                net::tcp::stream::open(world, self.registry.clone(), stream, sender)?;

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
