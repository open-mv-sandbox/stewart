use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    rc::Rc,
};

use anyhow::Error;
use mio::{Interest, Token};
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use tracing::{event, Level};

use crate::{Ready, Registry};

pub enum StreamAction {
    /// Send a data to a stream.
    Send(SendAction),
    /// Close and stop the stream.
    Close,
}

pub struct SendAction {
    pub data: Vec<u8>,
}

pub struct RecvEvent {
    pub data: Vec<u8>,
}

pub(crate) fn open(
    world: &mut World,
    registry: Rc<Registry>,
    stream: mio::net::TcpStream,
    on_event: Sender<RecvEvent>,
) -> Result<Sender<StreamAction>, Error> {
    let (actor, sender) = Service::new(registry, stream, on_event)?;
    world.insert("tcp-stream", actor)?;

    Ok(sender)
}

struct Service {
    action_mailbox: Mailbox<StreamAction>,
    ready_mailbox: Mailbox<Ready>,
    on_event: Sender<RecvEvent>,

    registry: Rc<Registry>,
    stream: mio::net::TcpStream,
    token: Token,

    queue: VecDeque<Vec<u8>>,
}

impl Service {
    fn new(
        registry: Rc<Registry>,
        mut stream: mio::net::TcpStream,
        on_event: Sender<RecvEvent>,
    ) -> Result<(Self, Sender<StreamAction>), Error> {
        event!(Level::DEBUG, "opening stream");

        let (action_mailbox, sender) = mailbox();
        let (ready_mailbox, ready_sender) = mailbox();

        // Register for mio events
        let token = registry.token();
        registry.register(&mut stream, token, Interest::READABLE, ready_sender)?;

        let value = Service {
            action_mailbox,
            ready_mailbox,
            on_event,

            registry,
            stream,
            token,

            queue: VecDeque::new(),
        };
        Ok((value, sender))
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut stewart::Context) -> Result<(), Error> {
        self.action_mailbox.set_signal(ctx.signal());
        self.ready_mailbox.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.poll_actions(ctx)?;
        self.poll_ready(ctx)?;

        Ok(())
    }
}

impl Service {
    fn poll_actions(&mut self, ctx: &mut Context) -> Result<(), Error> {
        // Handle actions
        while let Some(action) = self.action_mailbox.recv() {
            match action {
                StreamAction::Send(action) => self.on_action_send(action)?,
                StreamAction::Close => ctx.set_stop(),
            }
        }

        Ok(())
    }

    fn poll_ready(&mut self, ctx: &mut Context) -> Result<(), Error> {
        // Handle ready
        let mut readable = false;
        let mut writable = false;
        while let Some(ready) = self.ready_mailbox.recv() {
            readable |= ready.readable;
            writable |= ready.writable;
        }

        if readable {
            self.on_ready_readable(ctx)?;
        }

        if writable {
            self.on_ready_writable()?;
        }

        Ok(())
    }

    fn on_ready_readable(&mut self, ctx: &mut Context) -> Result<(), Error> {
        // TODO: Re-use buffer where possible
        let mut closed = false;
        let mut buffer = vec![0; 1024];
        let mut bytes_read = 0;

        loop {
            // Attempt to receive data
            let result = self.stream.read(&mut buffer[bytes_read..]);

            match result {
                Ok(len) => {
                    // Read of zero means the stream has been closed
                    if len == 0 {
                        closed = true;
                        break;
                    }

                    // Add additional read data to buffer
                    bytes_read += len;
                    if bytes_read == buffer.len() {
                        buffer.resize(buffer.len() + 1024, 0);
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
            let data = buffer[..bytes_read].to_vec();
            let event = RecvEvent { data };
            self.on_event.send(event)?;
        }

        // If the stream got closed, stop the actor
        if closed {
            // TODO: Emit an event that the stream was closed
            event!(Level::DEBUG, "closing stream");
            ctx.set_stop();
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
                    *data = data[count..].to_vec();
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
