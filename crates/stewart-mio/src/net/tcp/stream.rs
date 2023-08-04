use std::{
    io::{ErrorKind, Read},
    rc::Rc,
};

use anyhow::Error;
use mio::Interest;
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

    stream: mio::net::TcpStream,
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

            stream,
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
        // Handle ready
        let mut readable = false;
        while let Some(ready) = self.ready_mailbox.recv()? {
            readable |= ready.readable;
        }

        if readable {
            self.on_ready_readable(ctx)?;
        }

        // Handle actions
        while let Some(action) = self.action_mailbox.recv()? {
            match action {
                StreamAction::Send(action) => self.on_action_send(action),
                StreamAction::Close => ctx.set_stop(),
            }
        }

        Ok(())
    }
}

impl Service {
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
            event!(Level::TRACE, count = bytes_read, "received data");
            let data = buffer[..bytes_read].to_vec();
            let event = RecvEvent { data };
            self.on_event.send(event)?;
        }

        // If the stream got closed, stop the actor
        if closed {
            // TODO: Event
            event!(Level::DEBUG, "closing stream");
            ctx.set_stop();
        }

        Ok(())
    }

    fn on_action_send(&mut self, _action: SendAction) {
        event!(Level::WARN, "attempted to send, not implemented");
        // TODO
    }
}
