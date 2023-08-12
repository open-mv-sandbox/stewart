use std::net::SocketAddr;

use anyhow::Error;
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::{net::tcp, RegistryRef};
use tracing::{event, Level};

use crate::{connection, HttpEvent};

pub fn listen(
    world: &mut World,
    registry: RegistryRef,
    addr: SocketAddr,
    http_events: Sender<HttpEvent>,
) -> Result<(), Error> {
    let actor = Service::new(world, registry, addr, http_events)?;
    world.insert("http-server", actor)?;

    Ok(())
}

struct Service {
    tcp_events: Mailbox<tcp::ListenerEvent>,
    tcp_actions: Sender<tcp::ListenerAction>,
    http_events: Sender<HttpEvent>,

    connections: Vec<StreamEntry>,
}

struct StreamEntry {
    events: Mailbox<connection::ConnectionEvent>,
    actions: Sender<connection::ConnectionAction>,
    closed: bool,
}

impl Service {
    fn new(
        world: &mut World,
        registry: RegistryRef,
        addr: SocketAddr,
        http_events: Sender<HttpEvent>,
    ) -> Result<Self, Error> {
        // Start the listen port
        let tcp_events = Mailbox::default();
        let (tcp_actions, server_info) = tcp::bind(world, registry, addr, tcp_events.sender())?;

        event!(Level::DEBUG, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            tcp_events,
            tcp_actions,
            http_events,

            connections: Vec::new(),
        };
        Ok(actor)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.tcp_actions.send(tcp::ListenerAction::Close);

        // Close all not yet closed streams
        for stream in &self.connections {
            if stream.closed {
                continue;
            }

            let _ = stream.actions.send(connection::ConnectionAction::Close);
        }
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());
        self.tcp_events.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        // Handle incoming TCP connections
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    let events = Mailbox::default();
                    let signal = world.signal(meta.id());
                    events.set_signal(signal);

                    let actions = connection::open(
                        world,
                        event.events,
                        event.actions,
                        events.sender(),
                        self.http_events.clone(),
                    )?;

                    // Track the connection
                    let connection = StreamEntry {
                        events,
                        actions,
                        closed: false,
                    };
                    self.connections.push(connection);
                }
                tcp::ListenerEvent::Closed => meta.set_stop(),
            }
        }

        // Process open streams
        for connection in &mut self.connections {
            // Handle stream events
            while let Some(event) = connection.events.recv() {
                match event {
                    connection::ConnectionEvent::Closed => {
                        connection.closed = true;
                    }
                }
            }
        }
        self.connections.retain(|s| !s.closed);

        Ok(())
    }
}
