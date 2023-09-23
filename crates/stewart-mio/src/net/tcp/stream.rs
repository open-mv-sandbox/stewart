use std::ops::ControlFlow;
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
};

use anyhow::Error;
use bytes::{Buf, Bytes, BytesMut};
use mio::Interest;
use stewart::{
    sender::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use tracing::{event, Level};

use crate::{ReadyRef, RegistryRef};

pub enum StreamAction {
    /// Send a data to the stream.
    Send(SendAction),
    /// Close the stream.
    Close,
}

pub struct SendAction {
    pub data: Bytes,
}

pub enum StreamEvent {
    /// Data received on the stream.
    Recv(RecvEvent),
    /// Stream has been closed.
    Closed,
}

pub struct RecvEvent {
    pub data: Bytes,
}

pub(crate) fn open(
    world: &mut Runtime,
    registry: RegistryRef,
    stream: mio::net::TcpStream,
    event_sender: Sender<StreamEvent>,
) -> Result<Sender<StreamAction>, Error> {
    let signal = Signal::default();
    let actor = Service::new(signal.clone(), registry, stream, event_sender)?;
    let actions = actor.actions.sender();

    let id = world.insert("tcp-stream", actor);
    signal.set_id(id);

    Ok(actions)
}

struct Service {
    actions: Mailbox<StreamAction>,
    events: Sender<StreamEvent>,

    stream: mio::net::TcpStream,
    ready: ReadyRef,
    closed: bool,

    queue: VecDeque<Bytes>,
    buffer: BytesMut,
}

impl Service {
    fn new(
        signal: Signal,
        registry: RegistryRef,
        mut stream: mio::net::TcpStream,
        events: Sender<StreamEvent>,
    ) -> Result<Self, Error> {
        event!(Level::DEBUG, "opening stream");

        let actions = Mailbox::new(signal.clone());

        // Register for mio events
        let ready = registry.register(&mut stream, Interest::READABLE, signal)?;

        let this = Service {
            actions,
            events,

            stream,
            ready,
            closed: false,

            queue: VecDeque::new(),
            buffer: BytesMut::new(),
        };
        Ok(this)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.ready.deregister(&mut self.stream);
    }
}

impl Actor for Service {
    fn handle(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        self.poll_actions()?;
        self.poll_ready(world).unwrap();

        if self.closed {
            event!(Level::DEBUG, "stopping");
            let _ = self.events.send(world, StreamEvent::Closed);
        }

        ControlFlow::Continue(())
    }
}

impl Service {
    fn poll_actions(&mut self) -> ControlFlow<()> {
        // Handle actions
        while let Some(action) = self.actions.recv() {
            match action {
                StreamAction::Send(action) => self.on_action_send(action).unwrap(),
                StreamAction::Close => return ControlFlow::Break(()),
            }
        }

        ControlFlow::Continue(())
    }

    fn poll_ready(&mut self, world: &mut Runtime) -> Result<(), Error> {
        let state = self.ready.take()?;

        if state.readable {
            self.on_ready_readable(world)?;
        }

        if state.writable {
            self.on_ready_writable()?;
        }

        Ok(())
    }

    fn on_ready_readable(&mut self, world: &mut Runtime) -> Result<(), Error> {
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
            self.events.send(world, StreamEvent::Recv(event))?;
        }

        // If the stream got closed, remember
        if closed {
            self.closed = true;
        }

        Ok(())
    }

    fn on_ready_writable(&mut self) -> Result<(), Error> {
        event!(Level::TRACE, "polling write");

        while self.try_send()? {}

        // If we have nothing left, remove the writable registry
        if self.queue.is_empty() {
            self.ready
                .reregister(&mut self.stream, Interest::READABLE)?;
        }

        Ok(())
    }

    fn try_send(&mut self) -> Result<bool, Error> {
        // Check if we have anything to send
        let Some(data) = self.queue.front_mut() else {
            return Ok(false);
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

        // Re-register so we can receive write events
        if should_register {
            self.ready
                .reregister(&mut self.stream, Interest::READABLE | Interest::WRITABLE)?;
        }

        Ok(())
    }
}
