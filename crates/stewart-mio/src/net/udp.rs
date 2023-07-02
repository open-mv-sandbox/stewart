use std::{collections::VecDeque, io::ErrorKind, net::SocketAddr};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{utils::Sender, Actor, Context, State};
use tracing::{event, Level};

use crate::{with_thread_context, WakeEvent};

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

pub fn bind(
    cx: &mut Context,
    addr: SocketAddr,
    on_packet: Sender<Packet>,
) -> Result<SocketInfo, Error> {
    let hnd = cx.create("udp-socket")?;

    // Create the socket
    let mut socket = mio::net::UdpSocket::bind(addr)?;
    let local_addr = socket.local_addr()?;

    // Register the socket with mio
    let wake = hnd.sender().map(ImplMessage::Wake);
    let token = with_thread_context(|tcx| {
        // Get the next poll token
        let index = tcx.next_token;
        tcx.next_token += 1;
        let token = Token(index);

        // Register the socket
        tcx.poll
            .registry()
            .register(&mut socket, token, Interest::READABLE)?;

        // Store routing for receiving wakeup events
        tcx.wake_senders.insert(token, wake);

        Ok(token)
    })?;
    // TODO: Registry cleanup when the socket is stopped

    let actor = UdpSocket {
        socket,
        token,

        // Max size of a UDP packet
        buffer: vec![0; 1 << 16],
        on_packet,
        queue: VecDeque::new(),
    };
    cx.start(hnd, actor)?;

    let info = SocketInfo {
        sender: hnd.sender().map(ImplMessage::Send),
        local_addr,
    };
    Ok(info)
}

struct UdpSocket {
    socket: mio::net::UdpSocket,
    token: Token,

    buffer: Vec<u8>,
    queue: VecDeque<Packet>,
    on_packet: Sender<Packet>,
}

impl Actor for UdpSocket {
    type Message = ImplMessage;

    fn process(&mut self, cx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        let mut wake = None;

        while let Some(message) = state.next() {
            match message {
                ImplMessage::Send(packet) => {
                    event!(Level::DEBUG, peer = ?packet.peer, "received outgoing packet");

                    // Queue outgoing packet
                    let should_register = self.queue.is_empty();
                    self.queue.push_back(packet);

                    // Reregister so we can receive write events
                    if should_register {
                        with_thread_context(|tcx| {
                            tcx.poll.registry().reregister(
                                &mut self.socket,
                                self.token,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;
                            Ok(())
                        })?;
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
                self.poll_read(cx)?
            }
            if wake.write {
                self.poll_write(cx)?
            }
        }

        Ok(())
    }
}

impl UdpSocket {
    fn poll_read(&mut self, cx: &mut Context) -> Result<(), Error> {
        event!(Level::TRACE, "polling read");

        while self.try_recv(cx)? {}

        Ok(())
    }

    fn try_recv(&mut self, cx: &mut Context) -> Result<bool, Error> {
        // Attempt to receive packet
        let result = self.socket.recv_from(&mut self.buffer);

        // Check result
        let (packet_size, peer) = match result {
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
        let data = self.buffer[..packet_size].to_vec();
        let packet = Packet { peer, data };
        self.on_packet.send(cx, packet);

        Ok(true)
    }

    fn poll_write(&mut self, cx: &mut Context) -> Result<(), Error> {
        event!(Level::TRACE, "polling write");

        while self.try_send(cx)? {}

        // If we have nothing left, remove the writable registry
        if self.queue.is_empty() {
            with_thread_context(|tcx| {
                tcx.poll
                    .registry()
                    .reregister(&mut self.socket, self.token, Interest::READABLE)?;
                Ok(())
            })?;
        }

        Ok(())
    }

    fn try_send(&mut self, _cx: &mut Context) -> Result<bool, Error> {
        // Check if we have anything to send
        let packet = if let Some(packet) = self.queue.front() {
            packet
        } else {
            return Ok(false);
        };

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
