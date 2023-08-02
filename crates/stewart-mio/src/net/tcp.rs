use std::{
    collections::HashMap,
    io::{ErrorKind, Read},
    net::SocketAddr,
    rc::Rc,
};

use anyhow::{Context as _, Error};
use mio::{Interest, Token};
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, instrument, Level};
use uuid::Uuid;

use crate::{net::check_io, Ready, Registry};

pub enum Action {
    /// Send a data to a stream.
    StreamSend(StreamSend),
    /// Close a stream.
    StreamClose(StreamClose),
    /// Close and stop the listener.
    Close,
}

pub struct StreamSend {
    pub stream: Uuid,
    pub data: Vec<u8>,
}

pub struct StreamClose {
    pub stream: Uuid,
}

pub struct Listener {
    events: Mailbox<Uuid>,
    local_addr: SocketAddr,
}

impl Listener {
    pub fn events(&self) -> &Mailbox<Uuid> {
        &self.events
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

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
    registry: Rc<Registry>,
    events_sender: Sender<Uuid>,
    ready: Mailbox<Ready>,
    ready_sender: Sender<Ready>,

    listener: mio::net::TcpListener,
    listener_token: Token,

    // TODO: It makes more sense to make them separate actors, this lets us handle clients and
    // servers in the same way too.
    streams: HashMap<Uuid, mio::net::TcpStream>,
    /// Mapping for incoming ready events.
    tokens: HashMap<Token, Uuid>,

    /// Scratch buffer for ready events.
    ready_events: Vec<Ready>,
}

impl Service {
    fn new(registry: Rc<Registry>, addr: SocketAddr) -> Result<(Self, Listener), Error> {
        let (events, events_sender) = mailbox();
        let (ready, ready_sender) = mailbox();

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
            events_sender,
            ready,
            ready_sender,

            listener,
            listener_token,

            streams: HashMap::new(),
            tokens: HashMap::new(),

            ready_events: Vec::new(),
        };
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
        self.accumulate_ready();

        while let Some(ready) = self.ready_events.pop() {
            if ready.token == self.listener_token {
                self.on_listener_ready()?;
            } else {
                // This means it's one of the streams
                self.on_stream_ready(ready)?;
            }
        }

        Ok(())
    }
}

impl Service {
    fn accumulate_ready(&mut self) {
        while let Some(ready) = self.ready.recv() {
            // Attempt to find an existing entry
            let result = self
                .ready_events
                .iter_mut()
                .find(|r| r.token == ready.token);

            if let Some(r) = result {
                r.readable |= ready.readable;
                r.writable |= ready.writable;
            } else {
                self.ready_events.push(ready);
            }
        }
    }

    fn on_listener_ready(&mut self) -> Result<(), Error> {
        // Accept any pending streams
        while let Some((mut stream, remote_addr)) = check_io(self.listener.accept())? {
            event!(Level::DEBUG, ?remote_addr, "stream accepted");
            let uuid = Uuid::new_v4();

            // Register for mio events
            let token = self.registry.token();
            self.registry.register(
                &mut stream,
                token,
                Interest::READABLE,
                self.ready_sender.clone(),
            )?;
            self.tokens.insert(token, uuid);

            // Remember the stream
            self.streams.insert(uuid, stream);

            // Notify
            self.events_sender.send(uuid)?;
        }

        Ok(())
    }

    fn on_stream_ready(&mut self, ready: Ready) -> Result<(), Error> {
        let uuid = *self
            .tokens
            .get(&ready.token)
            .context("stream token not registered")?;
        let stream = self
            .streams
            .get_mut(&uuid)
            .context("stream uuid not registered")?;

        if ready.readable {
            read_stream(stream)?;
        }

        if ready.writable {
            // TODO
        }

        Ok(())
    }
}

fn read_stream(stream: &mut mio::net::TcpStream) -> Result<(), Error> {
    // TODO: Re-use buffer where possible
    let mut closed = false;
    let mut buffer = vec![0; 1024];
    let mut bytes_read = 0;

    loop {
        // Attempt to receive data
        let result = stream.read(&mut buffer[bytes_read..]);

        match result {
            Ok(len) => {
                // Read of zero means the stream has been closed
                if len == 0 {
                    closed = true;
                    break;
                }

                // Add additional read data to buffer
                bytes_read += len;
                if bytes_read == buffer.len() {
                    buffer.resize(buffer.len() + 1024, 0);
                }
            }
            Err(error) => match error.kind() {
                ErrorKind::WouldBlock => break,
                ErrorKind::Interrupted => break,
                _ => return Err(error.into()),
            },
        }
    }

    // TODO: Send read data to listener
    event!(
        Level::WARN,
        "received {} bytes, closed = {}",
        bytes_read,
        closed
    );

    // TODO: Do something with `closed`

    Ok(())
}
