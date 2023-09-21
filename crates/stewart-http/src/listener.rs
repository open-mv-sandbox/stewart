use std::net::SocketAddr;
use std::ops::ControlFlow;

use anyhow::Error;
use stewart::{
    message::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use stewart_mio::{net::tcp, RegistryRef};
use tracing::{event, Level};

use crate::{connection, HttpEvent};

/// Open a HTTP listener on the given address.
pub fn bind(
    world: &mut Runtime,
    registry: RegistryRef,
    addr: SocketAddr,
    http_events: Sender<HttpEvent>,
) -> Result<(), Error> {
    let (actor, signal) = Service::new(world, registry, addr, http_events)?;

    let id = world.insert("http-listener", actor);
    signal.set_id(id);

    Ok(())
}

struct Service {
    signal: Signal,
    tcp_events: Mailbox<tcp::ListenerEvent>,
    tcp_actions: Sender<tcp::ListenerAction>,
    http_events: Sender<HttpEvent>,

    connections: Vec<StreamEntry>,
    closed: bool,
}

struct StreamEntry {
    events: Mailbox<connection::ConnectionEvent>,
    actions: Sender<connection::ConnectionAction>,
    closed: bool,
}

impl Service {
    fn new(
        world: &mut Runtime,
        registry: RegistryRef,
        addr: SocketAddr,
        http_events: Sender<HttpEvent>,
    ) -> Result<(Self, Signal), Error> {
        let signal = Signal::default();

        // Start the listen port
        let tcp_events = Mailbox::new(signal.clone());
        let (tcp_actions, server_info) = tcp::bind(world, registry, addr, tcp_events.sender())?;

        event!(Level::DEBUG, addr = ?server_info.local_addr, "listening");

        let this = Service {
            signal: signal.clone(),
            tcp_events,
            tcp_actions,
            http_events,

            connections: Vec::new(),
            closed: false,
        };
        Ok((this, signal))
    }
}

impl Actor for Service {
    fn process(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        // Handle incoming TCP connections
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    let events = Mailbox::new(self.signal.clone());

                    let actions = connection::open(
                        world,
                        event.events,
                        event.actions,
                        events.sender(),
                        self.http_events.clone(),
                    ).unwrap();

                    // Track the connection
                    let connection = StreamEntry {
                        events,
                        actions,
                        closed: false,
                    };
                    self.connections.push(connection);
                }
                tcp::ListenerEvent::Closed => self.closed = true,
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

        if self.closed {
            event!(Level::DEBUG, "stopping");

            let _ = self.tcp_actions.send(world, tcp::ListenerAction::Close);

            // Close all not yet closed streams
            for stream in &self.connections {
                if stream.closed {
                    continue;
                }

                let _ = stream
                    .actions
                    .send(world, connection::ConnectionAction::Close);
            }
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }
}
