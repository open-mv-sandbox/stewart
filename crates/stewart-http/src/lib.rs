use std::{io::Write, net::SocketAddr, rc::Rc};

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{net::tcp, Registry};
use tracing::{event, Level};

pub fn listen(
    world: &mut World,
    registry: Rc<Registry>,
    addr: SocketAddr,
    body: String,
) -> Result<(), Error> {
    let actor = Service::new(world, registry, addr, body)?;
    world.insert("http-server", actor)?;

    Ok(())
}

struct Service {
    server_mailbox: Mailbox<tcp::ListenerEvent>,
    _server_sender: Sender<tcp::ListenerAction>,

    body: String,
    connections: Vec<Connection>,
}

struct Connection {
    event: tcp::ConnectedEvent,
    pending: String,
    closed: bool,
}

impl Service {
    fn new(
        world: &mut World,
        registry: Rc<Registry>,
        addr: SocketAddr,
        body: String,
    ) -> Result<Self, Error> {
        let (server_mailbox, on_event) = mailbox();

        // Start the listen port
        let (server_sender, server_info) = tcp::listen(world, registry, addr, on_event)?;
        event!(Level::INFO, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            server_mailbox,
            _server_sender: server_sender,

            body,
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
                    let body = self.body.as_bytes();
                    let mut data = Vec::new();

                    data.write_all(b"HTTP/1.1 200 OK")?;
                    data.write_all(b"\r\nContent-Type: text/html\r\nContent-Length: ")?;
                    let length = body.len().to_string();
                    data.write_all(length.as_bytes())?;
                    data.write_all(b"\r\n\r\n")?;
                    data.write_all(body)?;

                    let action = tcp::SendAction { data };
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
