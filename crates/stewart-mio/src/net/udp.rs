use std::{collections::VecDeque, io::ErrorKind, net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, Handler, World};
use tracing::{event, instrument, Level};

use crate::{registry::WakeEvent, Registry};

#[derive(Debug)]
pub struct Packet {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
}

pub struct SocketInfo {
    handler: Handler<Packet>,
    local_addr: SocketAddr,
}

impl SocketInfo {
    pub fn handler(&self) -> &Handler<Packet> {
        &self.handler
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
    on_packet: Handler<Packet>,
) -> Result<SocketInfo, Error> {
    let id = world.create("udp-socket")?;

    // Create the socket
    let mut socket = mio::net::UdpSocket::bind(addr)?;
    let local_addr = socket.local_addr()?;

    // Register the socket
    let token = registry.token();
    let wake = Handler::to(id).map(Message::Wake);
    registry.register(&mut socket, token, Interest::READABLE, wake)?;

    let actor = UdpSocket {
        registry,
        socket,
        token,

        // Max size of a UDP packet
        buffer: vec![0; 65536],
        on_packet,
        queue: VecDeque::new(),
    };
    world.start(id, actor)?;

    let info = SocketInfo {
        handler: Handler::to(id).map(Message::Send),
        local_addr,
    };
    Ok(info)
}

struct UdpSocket {
    registry: Rc<Registry>,
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: Vec<u8>,
    queue: VecDeque<Packet>,
    on_packet: Handler<Packet>,
}

enum Message {
    Send(Packet),
    Wake(WakeEvent),
}

impl Actor for UdpSocket {
    type Message = Message;

    fn process(&mut self, world: &mut World, mut cx: Context<Self>) -> Result<(), Error> {
        let mut readable = false;
        let mut writable = false;

        while let Some(message) = cx.next_message() {
            match message {
                Message::Send(packet) => {
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
                Message::Wake(event) => {
                    readable |= event.readable;
                    writable |= event.writable;
                }
            }
        }

        if readable {
            self.poll_read(world)?
        }
        if writable {
            self.poll_write()?
        }

        Ok(())
    }
}

impl UdpSocket {
    fn poll_read(&mut self, world: &mut World) -> Result<(), Error> {
        event!(Level::TRACE, "polling read");

        while self.try_recv(world)? {}

        Ok(())
    }

    fn try_recv(&mut self, world: &mut World) -> Result<bool, Error> {
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
        self.on_packet.handle(world, packet)?;

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
