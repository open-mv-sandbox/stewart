mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{
    net::tcp::{self},
    Registry,
};
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
    pending: Vec<String>,
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

                    // Send a greeting message
                    let data = b"HELLO WORLD\n".to_vec();
                    let action = tcp::SendAction { data };
                    event.actions_sender.send(tcp::StreamAction::Send(action))?;

                    // Keep track of the stream
                    event.event_mailbox.set_signal(ctx.signal());
                    let connection = Connection {
                        event,
                        pending: Vec::new(),
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
                        event!(Level::INFO, bytes = event.data.len(), "received data");

                        let data = String::from_utf8(event.data)?;
                        connection.pending.push(data);
                    }
                    tcp::StreamEvent::Closed => {
                        event!(Level::INFO, "stream closed");
                        connection.closed = true;
                    }
                }
            }

            if !connection.closed {
                for pending in connection.pending.drain(..) {
                    // Reply with an echo to all pending
                    let reply = format!("HELLO, \"{}\"!\n", pending.trim());
                    let packet = tcp::SendAction {
                        data: reply.into_bytes(),
                    };
                    let message = tcp::StreamAction::Send(packet);
                    connection.event.actions_sender.send(message)?;
                }
            }
        }

        self.connections.retain(|c| !c.closed);

        Ok(())
    }
}
