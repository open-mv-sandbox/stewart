use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
};

use anyhow::Error;
use bytes::{Buf, Bytes, BytesMut};
use mio::{Interest, Token};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use tracing::{event, Level};

use crate::{ReadyEvent, RegistryHandle};

pub enum ConnectionAction {
    /// Send a data to the stream.
    Send(SendAction),
    /// Close the stream.
    Close,
}

pub struct SendAction {
    pub data: Bytes,
}

pub enum ConnectionEvent {
    /// Data received on the stream.
    Recv(RecvEvent),
    /// Stream has been closed.
    Closed,
}

pub struct RecvEvent {
    pub data: Bytes,
}

pub(crate) fn open(
    world: &mut World,
    registry: RegistryHandle,
    stream: mio::net::TcpStream,
    event_sender: Sender<ConnectionEvent>,
) -> Result<Sender<ConnectionAction>, Error> {
    let (actor, sender) = Service::new(registry, stream, event_sender)?;
    world.insert("tcp-stream", actor)?;

    Ok(sender)
}

struct Service {
    registry: RegistryHandle,
    action_mailbox: Mailbox<ConnectionAction>,
    ready_mailbox: Mailbox<ReadyEvent>,
    event_sender: Sender<ConnectionEvent>,

    stream: mio::net::TcpStream,
    token: Token,

    queue: VecDeque<Bytes>,
    buffer: BytesMut,
}

impl Service {
    fn new(
        registry: RegistryHandle,
        mut stream: mio::net::TcpStream,
        event_sender: Sender<ConnectionEvent>,
    ) -> Result<(Self, Sender<ConnectionAction>), Error> {
        event!(Level::DEBUG, "opening stream");

        let action_mailbox = Mailbox::default();
        let ready_mailbox = Mailbox::default();

        let action_sender = action_mailbox.sender();
        let ready_sender = ready_mailbox.sender();

        // Register for mio events
        let token = registry.register(&mut stream, Interest::READABLE, ready_sender)?;

        let value = Service {
            registry,
            action_mailbox,
            ready_mailbox,
            event_sender,

            stream,
            token,

            queue: VecDeque::new(),
            buffer: BytesMut::new(),
        };
        Ok((value, action_sender))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.event_sender.send(ConnectionEvent::Closed);

        self.registry.deregister(&mut self.stream, self.token);
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());

        self.action_mailbox.set_signal(signal.clone());
        self.ready_mailbox.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, _world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        self.poll_actions(meta)?;
        self.poll_ready(meta)?;

        Ok(())
    }
}

impl Service {
    fn poll_actions(&mut self, meta: &mut Metadata) -> Result<(), Error> {
        // Handle actions
        while let Some(action) = self.action_mailbox.recv() {
            match action {
                ConnectionAction::Send(action) => self.on_action_send(action)?,
                ConnectionAction::Close => meta.set_stop(),
            }
        }

        Ok(())
    }

    fn poll_ready(&mut self, meta: &mut Metadata) -> Result<(), Error> {
        // Handle ready
        let mut readable = false;
        let mut writable = false;
        while let Some(ready) = self.ready_mailbox.recv() {
            readable |= ready.readable;
            writable |= ready.writable;
        }

        if readable {
            self.on_ready_readable(meta)?;
        }

        if writable {
            self.on_ready_writable()?;
        }

        Ok(())
    }

    fn on_ready_readable(&mut self, meta: &mut Metadata) -> Result<(), Error> {
        // Make sure we have at least a minimum amount of buffer space left
        if self.buffer.len() < 1024 {
            self.buffer.resize(2048, 0);
        }

        let mut closed = false;
        let mut bytes_read = 0;

        loop {
            // Attempt to receive data
            let result = self.stream.read(&mut self.buffer[bytes_read..]);

            match result {
                Ok(len) => {
                    // Read of zero means the stream has been closed
                    if len == 0 {
                        closed = true;
                        break;
                    }

                    // Add additional read data to buffer
                    bytes_read += len;
                    if bytes_read == self.buffer.len() {
                        self.buffer.resize(self.buffer.len() + 1024, 0);
                    }
                }
                Err(error) => match error.kind() {
                    ErrorKind::WouldBlock => break,
                    ErrorKind::Interrupted => break,
                    _ => return Err(error.into()),
                },
            }
        }

        // Send read data to listener
        if bytes_read != 0 {
            event!(Level::TRACE, count = bytes_read, "received incoming");
            let data = self.buffer.split_to(bytes_read).freeze();
            let event = RecvEvent { data };
            self.event_sender.send(ConnectionEvent::Recv(event))?;
        }

        // If the stream got closed, stop the actor
        if closed {
            meta.set_stop();
        }

        Ok(())
    }

    fn on_ready_writable(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling write");

        while self.try_send()? {}

        // If we have nothing left, remove the writable registry
        if self.queue.is_empty() {
            self.registry
                .reregister(&mut self.stream, self.token, Interest::READABLE)?;
        }

        Ok(())
    }

    fn try_send(&mut self) -> Result<bool, Error> {
        // Check if we have anything to send
        let Some(data) = self.queue.front_mut() else {
            return Ok(false)
        };

        // Attempt to send as much as we can
        match self.stream.write(data) {
            Ok(count) => {
                // We wrote data correctly, check if we wrote all of it, if not we need to truncate
                if count < data.len() {
                    data.advance(count);
                } else {
                    // We wrote all, no need to retain
                    self.queue.pop_front();
                }

                Ok(true)
            }
            Err(error) => match error.kind() {
                ErrorKind::WouldBlock => {
                    // We got interrupted, and we can't retry
                    Ok(false)
                }
                ErrorKind::Interrupted => {
                    // We got interrupted, but we can retry
                    Ok(true)
                }
                _ => {
                    // Fatal error
                    Err(error.into())
                }
            },
        }
    }

    fn on_action_send(&mut self, action: SendAction) -> Result<(), Error> {
        event!(Level::TRACE, "received outgoing");

        // Queue outgoing packet
        let should_register = self.queue.is_empty();
        self.queue.push_back(action.data);

        // Reregister so we can receive write events
        if should_register {
            self.registry.reregister(
                &mut self.stream,
                self.token,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        }

        Ok(())
    }
}
