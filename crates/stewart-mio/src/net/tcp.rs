use std::{net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::Interest;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox};
use tracing::{event, instrument, Level};

use crate::{net::check_io, Ready, Registry};

pub struct Listener {
    events: Mailbox<Stream>,
    local_addr: SocketAddr,
}

impl Listener {
    pub fn events(&self) -> &Mailbox<Stream> {
        &self.events
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

pub struct Stream {}

/// Open a TCP stream listener on the given address.
///
/// TCP, unlike UDP, works with ongoing connections.
/// Before a connection is established, you first need to 'listen' for those on a port.
#[instrument("tcp::listen", skip_all)]
pub fn listen(
    world: &mut World,
    registry: Rc<Registry>,
    addr: SocketAddr,
) -> Result<Listener, Error> {
    let (actor, listener) = Service::new(registry, addr)?;
    world.insert("tcp-listener", actor)?;

    Ok(listener)
}

struct Service {
    ready: Mailbox<Ready>,
    listener: mio::net::TcpListener,
}

impl Service {
    fn new(registry: Rc<Registry>, addr: SocketAddr) -> Result<(Self, Listener), Error> {
        let (events, _events_sender) = mailbox();
        let (ready, ready_sender) = mailbox();

        // Create the socket
        let mut listener = mio::net::TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;
        let token = registry.token();

        // Register the socket for ready events
        registry.register(&mut listener, token, Interest::READABLE, ready_sender)?;

        let value = Self { ready, listener };
        let listener = Listener { local_addr, events };
        Ok((value, listener))
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.ready.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        // Check if we've got new streams ready to be accepted
        let mut readable = false;
        while let Some(ready) = self.ready.recv() {
            readable |= ready.readable;
        }

        // If we've got streams ready, accept them
        if readable {
            while let Some((_stream, remote_addr)) = check_io(self.listener.accept())? {
                event!(Level::WARN, ?remote_addr, "stream accepted, unimplemented");
            }
        }

        Ok(())
    }
}
