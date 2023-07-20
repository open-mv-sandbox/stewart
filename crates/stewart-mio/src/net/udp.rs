use std::{collections::VecDeque, io::ErrorKind, net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, instrument, Level};

use crate::{registry::Ready, Registry};

#[derive(Debug)]
pub struct Packet {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
}

pub struct SocketInfo {
    sender: Sender<Packet>,
    local_addr: SocketAddr,
}

impl SocketInfo {
    pub fn sender(&self) -> &Sender<Packet> {
        &self.sender
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[instrument("udp::bind", skip_all)]
pub fn bind(
    world: &mut World,
    registry: Rc<Registry>,
    addr: SocketAddr,
    on_packet: Sender<Packet>,
) -> Result<SocketInfo, Error> {
    let (outgoing, outgoing_sender) = mailbox();
    let (ready, ready_sender) = mailbox();

    // Create the socket
    let mut socket = mio::net::UdpSocket::bind(addr)?;
    let local_addr = socket.local_addr()?;
    let token = registry.token();

    // Register the socket for ready events
    registry.register(&mut socket, token, Interest::READABLE, ready_sender)?;

    let actor = UdpSocket {
        outgoing: outgoing.clone(),
        ready: ready.clone(),
        on_packet,

        registry,
        socket,
        token,

        // Max size of a UDP packet
        buffer: vec![0; 65536],
        queue: VecDeque::new(),
    };
    let signal = world.create("udp-socket", actor);

    outgoing.register(signal.clone());
    ready.register(signal);

    // Create the info wrapper the caller will use
    let info = SocketInfo {
        sender: outgoing_sender,
        local_addr,
    };
    Ok(info)
}

struct UdpSocket {
    outgoing: Mailbox<Packet>,
    ready: Mailbox<Ready>,
    on_packet: Sender<Packet>,

    registry: Rc<Registry>,
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: Vec<u8>,
    queue: VecDeque<Packet>,
}

impl Actor for UdpSocket {
    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        let mut readable = false;
        let mut writable = false;

        while let Some(packet) = self.outgoing.recv() {
            event!(Level::DEBUG, peer = ?packet.peer, "received outgoing packet");

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
        }

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
}

impl UdpSocket {
    fn poll_read(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling read");

        while self.try_recv()? {}

        Ok(())
    }

    fn try_recv(&mut self) -> Result<bool, Error> {
        // Attempt to receive packet
        let result = self.socket.recv_from(&mut self.buffer);

        // Check result
        let (size, peer) = match result {
            Ok(value) => value,
            Err(error) => {
                // WouldBlock just means we've run out of things to handle
                return if error.kind() == ErrorKind::WouldBlock {
                    Ok(false)
                } else {
                    Err(error.into())
                };
            }
        };

        event!(Level::DEBUG, ?peer, "received incoming packet");

        // Send the packet to the listener
        let data = self.buffer[..size].to_vec();
        let packet = Packet { peer, data };
        self.on_packet.send(packet)?;

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
        let Some(packet) = self.queue.front() else { return Ok(false) };

        // Attempt to send it
        let result = self.socket.send_to(&packet.data, packet.peer);

        // Check result
        if let Err(error) = result {
            // WouldBlock just means we've run out of things to handle
            return if error.kind() == ErrorKind::WouldBlock {
                Ok(false)
            } else {
                Err(error.into())
            };
        }

        // Remove the packet we've sent
        event!(Level::DEBUG, peer = ?packet.peer, "sent outgoing packet");
        self.queue.pop_front();

        Ok(true)
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        // Cleanup the current socket from the registry
        let result = self.registry.deregister(&mut self.socket);
        if let Err(error) = result {
            event!(Level::ERROR, ?error, "failed to deregister");
        }
    }
}
