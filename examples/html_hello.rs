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
    server_mailbox: Mailbox<tcp::ConnectedEvent>,
    _server_sender: Sender<tcp::ListenerAction>,

    connections: Vec<Connection>,
}

struct Connection {
    stream: tcp::ConnectedEvent,
    pending: String,
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
        while let Some(stream) = self.server_mailbox.recv() {
            event!(Level::DEBUG, "stream accepted");

            // Keep track of the stream
            stream.event_mailbox.set_signal(ctx.signal());
            let connection = Connection {
                stream,
                pending: String::new(),
            };
            self.connections.push(connection);
        }

        for connection in &mut self.connections {
            while let Some(event) = connection.stream.event_mailbox.recv() {
                // TODO: Stream close event

                event!(Level::DEBUG, bytes = event.data.len(), "received data");

                // Try get the request data
                let result = std::str::from_utf8(&event.data);
                let Ok(data) = result else {
                    // Reject malformed connection silently
                    let action = tcp::StreamAction::Close;
                    connection.stream.actions_sender.send(action)?;
                    continue;
                };

                // Append the data to anything pending
                connection.pending.push_str(data);

                // Check if we have a full request worth of data
                let split = connection.pending.split_once("\r\n\r\n");
                if let Some((_left, right)) = split {
                    event!(Level::INFO, "responding to request");
                    connection.pending = right.to_string();

                    // Send the response
                    let data = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 63\r\n\r\n<!DOCTYPE html><html><body><h1>Hello, World!</h1></body></html>".to_vec();
                    let action = tcp::SendAction { data };
                    connection
                        .stream
                        .actions_sender
                        .send(tcp::StreamAction::Send(action))?;
                }
            }
        }

        Ok(())
    }
}
