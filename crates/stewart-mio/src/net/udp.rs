use std::{collections::VecDeque, io::ErrorKind, net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, Handler, Id, World};
use tracing::{event, Level};

use crate::{registry::WakeEvent, Registry};

#[derive(Debug)]
pub struct Packet {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
}

pub struct SocketInfo {
    sender: Handler<Packet>,
    local_addr: SocketAddr,
}

impl SocketInfo {
    pub fn sender(&self) -> &Handler<Packet> {
        &self.sender
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

pub fn bind(
    world: &mut World,
    parent: Option<Id>,
    registry: Rc<Registry>,
    addr: SocketAddr,
    on_packet: Handler<Packet>,
) -> Result<SocketInfo, Error> {
    let id = world.create(parent, "udp-socket")?;

    // Create the socket
    let mut socket = mio::net::UdpSocket::bind(addr)?;
    let local_addr = socket.local_addr()?;

    // Register the socket
    let wake = Handler::to(id).map(ImplMessage::Wake);
    let token = registry.register(wake, &mut socket, Interest::READABLE)?;

    // TODO: Registry cleanup when the socket is stopped

    let actor = UdpSocket {
        registry,
        socket,
        token,

        // Max size of a UDP packet
        buffer: vec![0; 1 << 16],
        on_packet,
        queue: VecDeque::new(),
    };
    world.start(id, actor)?;

    let info = SocketInfo {
        sender: Handler::to(id).map(ImplMessage::Send),
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

impl Actor for UdpSocket {
    type Message = ImplMessage;

    fn process(&mut self, world: &mut World, mut cx: Context<Self>) -> Result<(), Error> {
        let mut wake = None;

        while let Some(message) = cx.next() {
            match message {
                ImplMessage::Send(packet) => {
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
                ImplMessage::Wake(event) => {
                    // Intentionally skips duplicates, a newer wake overwrites older ones
                    wake = Some(event);
                }
            }
        }

        if let Some(wake) = wake {
            if wake.read {
                self.poll_read(world)?
            }
            if wake.write {
                self.poll_write()?
            }
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
        self.on_packet.handle(world, packet);

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

enum ImplMessage {
    Send(Packet),
    Wake(WakeEvent),
}
