mod utils;

use anyhow::Error;
use bytes::Bytes;
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::{
    net::tcp::{self},
    Registry, RegistryRef,
};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Registry::new()?;

    // Start the actor
    let actor = Service::new(&mut world, registry.handle())?;
    world.insert("echo-example", actor)?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {
    server_mailbox: Mailbox<tcp::ListenerEvent>,
    _server_sender: Sender<tcp::ListenerAction>,
    connections: Vec<Connection>,
}

impl Service {
    pub fn new(world: &mut World, registry: RegistryRef) -> Result<Self, Error> {
        let server_mailbox = Mailbox::default();

        // Start the listen port
        let (server_sender, server_info) = tcp::bind(
            world,
            registry,
            "127.0.0.1:1234".parse()?,
            server_mailbox.sender(),
        )?;
        event!(Level::INFO, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            server_mailbox,
            _server_sender: server_sender,
            connections: Vec::new(),
        };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());
        self.server_mailbox.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        self.poll_listener(world, meta)?;
        self.poll_connections()?;

        Ok(())
    }
}

impl Service {
    fn poll_listener(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        while let Some(event) = self.server_mailbox.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    event!(Level::INFO, "stream accepted");

                    // Send a greeting message
                    let data: Bytes = "HELLO WORLD\n".into();
                    let action = tcp::SendAction { data };
                    event.actions.send(tcp::ConnectionAction::Send(action))?;

                    // Keep track of the stream
                    let signal = world.signal(meta.id());
                    event.events.set_signal(signal);
                    let connection = Connection {
                        event,
                        pending: String::new(),
                        closed: false,
                    };
                    self.connections.push(connection);
                }
                tcp::ListenerEvent::Closed => meta.set_stop(),
            }
        }

        Ok(())
    }

    fn poll_connections(&mut self) -> Result<(), Error> {
        for connection in &mut self.connections {
            connection.poll()?;
        }

        self.connections.retain(|c| !c.closed);

        Ok(())
    }
}

struct Connection {
    event: tcp::ConnectedEvent,
    pending: String,
    closed: bool,
}

impl Connection {
    fn poll(&mut self) -> Result<(), Error> {
        // Handle any incoming TCP stream events
        while let Some(event) = self.event.events.recv() {
            match event {
                tcp::ConnectionEvent::Recv(event) => {
                    event!(Level::INFO, bytes = event.data.len(), "received data");

                    let data = std::str::from_utf8(&event.data)?;
                    self.pending.push_str(data);
                }
                tcp::ConnectionEvent::Closed => {
                    event!(Level::INFO, "stream closed");
                    self.closed = true;
                }
            }
        }

        // If the stream is now closed, we can't do anything else
        if self.closed {
            return Ok(());
        }

        // Check how many messages ended with a newline we have
        let lines: Vec<_> = self.pending.split('\n').collect();
        let len = lines.len();

        // More than one line means we have messages terminated by a newline
        if len > 1 {
            // Keep only the last line
            let remaining = lines.last().unwrap_or(&"").to_string();

            // Echo every line independently, except the last which is not yet done
            for line in lines.into_iter().take(len - 1) {
                let reply = format!("HELLO, \"{}\"!\n", line.trim());

                let packet = tcp::SendAction { data: reply.into() };
                let message = tcp::ConnectionAction::Send(packet);
                self.event.actions.send(message)?;
            }

            self.pending = remaining;
        }

        Ok(())
    }
}
