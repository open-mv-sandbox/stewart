use std::{collections::VecDeque, io::ErrorKind, net::SocketAddr, rc::Rc};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, instrument, Level};

use crate::{registry::Ready, Registry};

pub enum Message {
    /// Send a packet to a peer.
    Send(Packet),
    /// Close and stop the socket.
    Close,
}

#[derive(Debug)]
pub struct Packet {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
}

pub struct SocketInfo {
    send: Sender<Message>,
    recv: Mailbox<Packet>,
    local_addr: SocketAddr,
}

impl SocketInfo {
    pub fn send(&self) -> &Sender<Message> {
        &self.send
    }

    pub fn recv(&self) -> &Mailbox<Packet> {
        &self.recv
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
) -> Result<SocketInfo, Error> {
    let (actor, info) = Service::new(registry, addr)?;
    world.insert("udp-socket", actor)?;

    Ok(info)
}

struct Service {
    send: Mailbox<Message>,
    recv: Sender<Packet>,
    ready: Mailbox<Ready>,

    registry: Rc<Registry>,
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: Vec<u8>,
    queue: VecDeque<Packet>,
}

impl Service {
    fn new(registry: Rc<Registry>, addr: SocketAddr) -> Result<(Self, SocketInfo), Error> {
        let (recv_mailbox, recv_sender) = mailbox();
        let (send_mailbox, send_sender) = mailbox();
        let (ready, ready_sender) = mailbox();

        // Create the socket
        let mut socket = mio::net::UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;
        let token = registry.token();

        // Register the socket for ready events
        registry.register(&mut socket, token, Interest::READABLE, ready_sender)?;

        let actor = Service {
            send: send_mailbox.clone(),
            recv: recv_sender,
            ready: ready.clone(),

            registry,
            socket,
            token,

            // Max size of a UDP packet
            buffer: vec![0; 65536],
            queue: VecDeque::new(),
        };
        let info = SocketInfo {
            send: send_sender,
            recv: recv_mailbox,
            local_addr,
        };
        Ok((actor, info))
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
                Message::Send(packet) => self.on_message_packet(packet)?,
                Message::Close => ctx.set_stop(),
            }
        }

        Ok(())
    }

    fn on_message_packet(&mut self, packet: Packet) -> Result<(), Error> {
        event!(Level::TRACE, peer = ?packet.peer, "received outgoing packet");

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

        event!(Level::TRACE, ?peer, "received incoming packet");

        // Send the packet to the listener
        let data = self.buffer[..size].to_vec();
        let packet = Packet { peer, data };
        self.recv.send(packet)?;

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
        event!(Level::TRACE, peer = ?packet.peer, "sent outgoing packet");
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
