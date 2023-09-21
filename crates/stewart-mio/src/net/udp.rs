use std::{collections::VecDeque, net::SocketAddr, time::Instant};
use std::ops::ControlFlow;

use anyhow::Error;
use bytes::{Bytes, BytesMut};
use mio::Interest;
use stewart::{
    message::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use tracing::{event, instrument, Level};

use crate::{net::check_io, registry::RegistryRef, ReadyRef};

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
    world: &mut Runtime,
    registry: RegistryRef,
    addr: SocketAddr,
    event_sender: Sender<RecvEvent>,
) -> Result<(Sender<Action>, SocketInfo), Error> {
    let (actor, signal, info) = Service::new(registry, addr, event_sender)?;
    let actions = actor.actions.sender();

    let id = world.insert("udp-socket", actor);
    signal.set_id(id);

    Ok((actions, info))
}

struct Service {
    actions: Mailbox<Action>,
    events: Sender<RecvEvent>,

    socket: mio::net::UdpSocket,
    ready: ReadyRef,

    buffer: BytesMut,
    queue: VecDeque<SendAction>,
}

impl Service {
    fn new(
        registry: RegistryRef,
        addr: SocketAddr,
        events: Sender<RecvEvent>,
    ) -> Result<(Self, Signal, SocketInfo), Error> {
        event!(Level::DEBUG, "binding");

        let signal = Signal::default();
        let actions = Mailbox::new(signal.clone());

        // Create the socket
        let mut socket = mio::net::UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;

        // Register the socket for ready events
        let ready = registry.register(&mut socket, Interest::READABLE, signal.clone())?;

        let this = Self {
            actions,
            events,

            socket,
            ready,

            buffer: BytesMut::new(),
            queue: VecDeque::new(),
        };
        let info = SocketInfo { local_addr };
        Ok((this, signal, info))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");
        self.ready.deregister(&mut self.socket);
    }
}

impl Actor for Service {
    fn process(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        self.poll_actions()?;
        self.poll_ready(world).unwrap();

        ControlFlow::Continue(())
    }
}

impl Service {
    fn poll_actions(&mut self) -> ControlFlow<()> {
        while let Some(message) = self.actions.recv() {
            match message {
                Action::Send(packet) => self.on_action_send(packet).unwrap(),
                Action::Close => return ControlFlow::Break(()),
            }
        }

        ControlFlow::Continue(())
    }

    fn on_action_send(&mut self, packet: SendAction) -> Result<(), Error> {
        event!(Level::TRACE, peer = ?packet.remote, "received outgoing packet");

        // Queue outgoing packet
        let should_register = self.queue.is_empty();
        self.queue.push_back(packet);

        // Reregister so we can receive write events
        if should_register {
            self.ready
                .reregister(&mut self.socket, Interest::READABLE | Interest::WRITABLE)?;
        }

        Ok(())
    }

    fn poll_ready(&mut self, world: &mut Runtime) -> Result<(), Error> {
        let state = self.ready.take()?;

        // Handle current state if the socket is ready
        if state.readable {
            self.poll_read(world)?
        }
        if state.writable {
            self.poll_write()?
        }

        Ok(())
    }

    fn poll_read(&mut self, world: &mut Runtime) -> Result<(), Error> {
        event!(Level::TRACE, "polling read");

        while self.try_recv(world)? {}

        Ok(())
    }

    fn try_recv(&mut self, world: &mut Runtime) -> Result<bool, Error> {
        // Max size of a UDP packet
        self.buffer.resize(65536, 0);

        // Attempt to receive packet
        let result = self.socket.recv_from(&mut self.buffer);
        let Some((size, remote)) = check_io(result)? else {
            return Ok(false);
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
        self.events.send(world, packet)?;

        Ok(true)
    }

    fn poll_write(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling write");

        while self.try_send()? {}

        // If we have nothing left, remove the writable registry
        if self.queue.is_empty() {
            self.ready
                .reregister(&mut self.socket, Interest::READABLE)?;
        }

        Ok(())
    }

    fn try_send(&mut self) -> Result<bool, Error> {
        // Check if we have anything to send
        let Some(packet) = self.queue.front() else {
            return Ok(false);
        };

        // Attempt to send it
        let result = self.socket.send_to(&packet.data, packet.remote);
        let Some(_) = check_io(result)? else {
            return Ok(false);
        };

        // Remove the packet we've sent
        event!(Level::TRACE, peer = ?packet.remote, "sent outgoing");
        self.queue.pop_front();

        Ok(true)
    }
}
