use std::{io::ErrorKind, net::SocketAddr};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, Sender, State};
use tracing::{event, Level};

use crate::with_thread_context;

pub struct Message {}

pub struct SocketInfo {
    sender: Sender<Message>,
    local_addr: SocketAddr,
}

impl SocketInfo {
    pub fn sender(&self) -> &Sender<Message> {
        &self.sender
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

/// TODO: Auto-determine token ID
pub fn udp_bind(cx: &mut Context, addr: SocketAddr) -> Result<SocketInfo, Error> {
    let hnd = cx.create("udp-socket")?;

    // Create the socket
    let mut socket = mio::net::UdpSocket::bind(addr)?;
    let local_addr = socket.local_addr()?;

    // Register the socket with mio
    let wake = hnd.sender().map(|_| ImplMessage::Wake);
    with_thread_context(|tcx| {
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

        Ok(())
    })?;
    // TODO: Registry cleanup when the socket is stopped

    let actor = UdpSocket { socket };
    cx.start(hnd, actor)?;

    let info = SocketInfo {
        sender: hnd.sender().map(ImplMessage::Message),
        local_addr,
    };
    Ok(info)
}

struct UdpSocket {
    socket: mio::net::UdpSocket,
}

impl Actor for UdpSocket {
    type Message = ImplMessage;

    fn process(&mut self, _cx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        let mut wake = false;
        while let Some(_message) = state.next() {
            wake = true;
        }

        if wake {
            // Receive all packets queued for the socket
            loop {
                let mut buf = vec![0; 1 << 16];
                match self.socket.recv_from(&mut buf) {
                    Ok((packet_size, source_address)) => {
                        let data = &buf[..packet_size];
                        event!(Level::INFO, ?source_address, ?data, "received packet");

                        // TODO: Echo the data
                        //socket.send_to(&buf[..packet_size], source_address)?;
                    }
                    Err(error) => {
                        if error.kind() == ErrorKind::WouldBlock {
                            // This is fine, nothing more to do
                            break;
                        } else {
                            // Internal error of some kind
                            return Err(error.into());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

enum ImplMessage {
    Message(Message),
    Wake,
}
