use std::{collections::VecDeque, net::SocketAddr, rc::Rc, time::Instant};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, instrument, Level};

use crate::{net::check_io, registry::Ready, Registry};

pub enum Action {
    /// Send a packet to a peer.
    Send(SendAction),
    /// Close and stop the socket.
    Close,
}

pub struct SendAction {
    pub remote: SocketAddr,
    pub data: Vec<u8>,
}

pub struct RecvEvent {
    pub remote: SocketAddr,
    pub arrived: Instant,
    pub data: Vec<u8>,
}

pub struct Socket {
    sender: Sender<Action>,
    events: Mailbox<RecvEvent>,
    local_addr: SocketAddr,
}

impl Socket {
    pub fn sender(&self) -> &Sender<Action> {
        &self.sender
    }

    pub fn events(&self) -> &Mailbox<RecvEvent> {
        &self.events
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[instrument("udp::bind", skip_all)]
pub fn bind(world: &mut World, registry: Rc<Registry>, addr: SocketAddr) -> Result<Socket, Error> {
    let (actor, socket) = Service::new(registry, addr)?;
    world.insert("udp-socket", actor)?;

    Ok(socket)
}

struct Service {
    send: Mailbox<Action>,
    events: Sender<RecvEvent>,
    ready: Mailbox<Ready>,

    registry: Rc<Registry>,
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: Vec<u8>,
    queue: VecDeque<SendAction>,
}

impl Service {
    fn new(registry: Rc<Registry>, addr: SocketAddr) -> Result<(Self, Socket), Error> {
        let (recv_mailbox, recv_sender) = mailbox();
        let (send_mailbox, send_sender) = mailbox();
        let (ready, ready_sender) = mailbox();

        // Create the socket
        let mut socket = mio::net::UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;

        // Register the socket for ready events
        let token = registry.token();
        registry.register(&mut socket, token, Interest::READABLE, ready_sender)?;

        let value = Self {
            send: send_mailbox.clone(),
            events: recv_sender,
            ready: ready.clone(),

            registry,
            socket,
            token,

            // Max size of a UDP packet
            buffer: vec![0; 65536],
            queue: VecDeque::new(),
        };
        let socket = Socket {
            sender: send_sender,
            events: recv_mailbox,
            local_addr,
        };
        Ok((value, socket))
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.send.set_signal(ctx.signal());
        self.ready.set_signal(ctx.signal());

        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.poll_mailbox(ctx)?;
        self.poll_ready()?;

        Ok(())
    }
}

impl Service {
    fn poll_mailbox(&mut self, ctx: &mut Context) -> Result<(), Error> {
        while let Some(message) = self.send.recv() {
            match message {
                Action::Send(packet) => self.on_message_send(packet)?,
                Action::Close => ctx.set_stop(),
            }
        }

        Ok(())
    }

    fn on_message_send(&mut self, packet: SendAction) -> Result<(), Error> {
        event!(Level::TRACE, peer = ?packet.remote, "received outgoing packet");

        // Queue outgoing packet
        let should_register = self.queue.is_empty();
        self.queue.push_back(packet);

        // Reregister so we can receive write events
        if should_register {
            self.registry.reregister(
                &mut self.socket,
                self.token,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        }

        Ok(())
    }

    fn poll_ready(&mut self) -> Result<(), Error> {
        let mut readable = false;
        let mut writable = false;

        while let Some(ready) = self.ready.recv() {
            readable |= ready.readable;
            writable |= ready.writable;
        }

        // Handle current state if the socket is ready
        if readable {
            self.poll_read()?
        }
        if writable {
            self.poll_write()?
        }

        Ok(())
    }

    fn poll_read(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling read");

        while self.try_recv()? {}

        Ok(())
    }

    fn try_recv(&mut self) -> Result<bool, Error> {
        // Attempt to receive packet
        let result = self.socket.recv_from(&mut self.buffer);
        let Some((size, remote)) = check_io(result)? else {
            return Ok(false)
        };

        event!(Level::TRACE, ?remote, "received incoming packet");

        // Track time of arrival
        let arrived = Instant::now();

        // Send the packet to the listener
        let data = self.buffer[..size].to_vec();
        let packet = RecvEvent {
            remote,
            arrived,
            data,
        };
        self.events.send(packet)?;

        Ok(true)
    }

    fn poll_write(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling write");

        while self.try_send()? {}

        // If we have nothing left, remove the writable registry
        if self.queue.is_empty() {
            self.registry
                .reregister(&mut self.socket, self.token, Interest::READABLE)?;
        }

        Ok(())
    }

    fn try_send(&mut self) -> Result<bool, Error> {
        // Check if we have anything to send
        let Some(packet) = self.queue.front() else {
            return Ok(false)
        };

        // Attempt to send it
        let result = self.socket.send_to(&packet.data, packet.remote);
        let Some(_) = check_io(result)? else {
            return Ok(false)
        };

        // Remove the packet we've sent
        event!(Level::TRACE, peer = ?packet.remote, "sent outgoing packet");
        self.queue.pop_front();

        Ok(true)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "dropping socket");

        // Cleanup the current socket from the registry
        let result = self.registry.deregister(&mut self.socket);
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to deregister");
        }
    }
}
