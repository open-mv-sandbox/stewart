use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use anyhow::Error;
use bytes::{Bytes, BytesMut};
use mio::{Interest, Token};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Id, World,
};
use tracing::{event, instrument, Level};

use crate::{
    net::check_io,
    registry::{ReadyEvent, RegistryHandle},
};

pub enum Action {
    /// Send a packet to a peer.
    Send(SendAction),
    /// Close and stop the socket.
    Close,
}

pub struct SendAction {
    pub remote: SocketAddr,
    pub data: Bytes,
}

pub struct RecvEvent {
    pub remote: SocketAddr,
    pub arrived: Instant,
    pub data: Bytes,
}

pub struct SocketInfo {
    pub local_addr: SocketAddr,
}

#[instrument("udp::bind", skip_all)]
pub fn bind(
    world: &mut World,
    registry: RegistryHandle,
    addr: SocketAddr,
    event_sender: Sender<RecvEvent>,
) -> Result<(Sender<Action>, SocketInfo), Error> {
    let (actor, sender, socket) = Service::new(registry, addr, event_sender)?;
    world.insert("udp-socket", actor)?;

    Ok((sender, socket))
}

struct Service {
    action_mailbox: Mailbox<Action>,
    ready_mailbox: Mailbox<ReadyEvent>,
    event_sender: Sender<RecvEvent>,

    registry: RegistryHandle,
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: BytesMut,
    queue: VecDeque<SendAction>,
}

impl Service {
    fn new(
        registry: RegistryHandle,
        addr: SocketAddr,
        event_sender: Sender<RecvEvent>,
    ) -> Result<(Self, Sender<Action>, SocketInfo), Error> {
        event!(Level::DEBUG, "binding");

        let action_mailbox = Mailbox::default();
        let ready_mailbox = Mailbox::default();

        let action_sender = action_mailbox.sender();
        let ready_sender = ready_mailbox.sender();

        // Create the socket
        let mut socket = mio::net::UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;

        // Register the socket for ready events
        let token = registry.register(&mut socket, Interest::READABLE, ready_sender)?;

        let value = Self {
            action_mailbox,
            ready_mailbox,
            event_sender,

            registry,
            socket,
            token,

            buffer: BytesMut::new(),
            queue: VecDeque::new(),
        };
        let socket = SocketInfo { local_addr };
        Ok((value, action_sender, socket))
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        self.action_mailbox.set_signal(world.signal(id));
        self.ready_mailbox.set_signal(world.signal(id));

        Ok(())
    }

    fn process(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        self.poll_actions(world, id)?;
        self.poll_ready()?;

        Ok(())
    }
}

impl Service {
    fn poll_actions(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        while let Some(message) = self.action_mailbox.recv() {
            match message {
                Action::Send(packet) => self.on_action_send(packet)?,
                Action::Close => world.stop(id),
            }
        }

        Ok(())
    }

    fn on_action_send(&mut self, packet: SendAction) -> Result<(), Error> {
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

        while let Some(ready) = self.ready_mailbox.recv() {
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
        // Max size of a UDP packet
        self.buffer.resize(65536, 0);

        // Attempt to receive packet
        let result = self.socket.recv_from(&mut self.buffer);
        let Some((size, remote)) = check_io(result)? else {
            return Ok(false)
        };

        event!(Level::TRACE, ?remote, "received incoming");

        // Track time of arrival
        let arrived = Instant::now();

        // Split off the read data
        let data = self.buffer.split_to(size).freeze();

        // Send the packet to the listener
        let packet = RecvEvent {
            remote,
            arrived,
            data,
        };
        self.event_sender.send(packet)?;

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
        event!(Level::TRACE, peer = ?packet.remote, "sent outgoing");
        self.queue.pop_front();

        Ok(true)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");
        self.registry.deregister(&mut self.socket, self.token);
    }
}
