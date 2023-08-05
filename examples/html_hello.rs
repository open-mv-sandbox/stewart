mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{net::tcp, Registry};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Rc::new(Registry::new()?);

    // Start the actor
    let actor = Service::new(&mut world, &registry)?;
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

struct Connection {
    event: tcp::ConnectedEvent,
    pending: String,
    closed: bool,
}

impl Service {
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        let (server_mailbox, on_event) = mailbox();

        // Start the listen port
        let (server_sender, server_info) =
            tcp::listen(world, registry.clone(), "127.0.0.1:1234".parse()?, on_event)?;
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
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server_mailbox.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.poll_listener(ctx)?;
        self.poll_connections()?;

        Ok(())
    }
}

impl Service {
    fn poll_listener(&mut self, ctx: &mut Context) -> Result<(), Error> {
        while let Some(event) = self.server_mailbox.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    event!(Level::INFO, "stream accepted");

                    // Keep track of the stream
                    event.event_mailbox.set_signal(ctx.signal());
                    let connection = Connection {
                        event,
                        pending: String::new(),
                        closed: false,
                    };
                    self.connections.push(connection);
                }
                tcp::ListenerEvent::Closed => ctx.set_stop(),
            }
        }

        Ok(())
    }

    fn poll_connections(&mut self) -> Result<(), Error> {
        for connection in &mut self.connections {
            while let Some(event) = connection.event.event_mailbox.recv() {
                match event {
                    tcp::StreamEvent::Recv(event) => {
                        event!(Level::DEBUG, bytes = event.data.len(), "received data");

                        let data = std::str::from_utf8(&event.data)?;
                        connection.pending.push_str(data);
                    }
                    tcp::StreamEvent::Closed => {
                        event!(Level::INFO, "stream closed");
                        connection.closed = true;
                    }
                }
            }

            if !connection.closed {
                // Check if we have a full request worth of data
                let split = connection.pending.split_once("\r\n\r\n");
                if let Some((_left, right)) = split {
                    event!(Level::INFO, "responding to request");
                    connection.pending = right.to_string();

                    // Send the response
                    let action = tcp::SendAction {
                        data: RESPONSE.to_vec(),
                    };
                    connection
                        .event
                        .actions_sender
                        .send(tcp::StreamAction::Send(action))?;
                }
            }
        }

        self.connections.retain(|c| !c.closed);

        Ok(())
    }
}

const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 63\r\n\r\n<!DOCTYPE html><html><body><h1>Hello, World!</h1></body></html>";
