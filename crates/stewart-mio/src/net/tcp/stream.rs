use std::{
    io::{ErrorKind, Read},
    rc::Rc,
};

use anyhow::Error;
use mio::Interest;
use stewart::{Actor, World};
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

pub(crate) fn open(
    world: &mut World,
    registry: Rc<Registry>,
    stream: mio::net::TcpStream,
) -> Result<Sender<StreamAction>, Error> {
    let (actor, sender) = Service::new(registry, stream)?;
    world.insert("tcp-stream", actor)?;

    Ok(sender)
}

struct Service {
    action_mailbox: Mailbox<StreamAction>,
    ready_mailbox: Mailbox<Ready>,
    stream: mio::net::TcpStream,
}

impl Service {
    fn new(
        registry: Rc<Registry>,
        mut stream: mio::net::TcpStream,
    ) -> Result<(Self, Sender<StreamAction>), Error> {
        let (action_mailbox, sender) = mailbox();
        let (ready_mailbox, ready_sender) = mailbox();

        // Register for mio events
        let token = registry.token();
        registry.register(&mut stream, token, Interest::READABLE, ready_sender)?;

        let value = Service {
            action_mailbox,
            ready_mailbox,
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

    fn process(&mut self, _ctx: &mut stewart::Context) -> Result<(), Error> {
        let mut readable = false;
        while let Some(ready) = self.ready_mailbox.recv() {
            readable |= ready.readable;
        }

        if readable {
            self.on_ready_readable()?;
        }

        // TODO: actions

        Ok(())
    }
}

impl Service {
    fn on_ready_readable(&mut self) -> Result<(), Error> {
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

        // TODO: Send read data to listener
        event!(
            Level::WARN,
            "received {} bytes, closed = {}",
            bytes_read,
            closed
        );

        // TODO: Do something with `closed`

        Ok(())
    }
}
