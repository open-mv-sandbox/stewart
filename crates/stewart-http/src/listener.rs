use std::net::SocketAddr;

use anyhow::Error;
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::{net::tcp, RegistryHandle};
use tracing::{event, Level};

use crate::stream;

pub fn listen(world: &mut World, registry: RegistryHandle, addr: SocketAddr) -> Result<(), Error> {
    let actor = Service::new(world, registry, addr)?;
    world.insert("http-server", actor)?;

    Ok(())
}

struct Service {
    listener_events: Mailbox<tcp::ListenerEvent>,
    listener_actions: Sender<tcp::ListenerAction>,
    streams: Vec<StreamEntry>,
}

struct StreamEntry {
    events: Mailbox<stream::StreamEvent>,
    actions: Sender<stream::StreamAction>,
    closed: bool,
}

impl Service {
    fn new(world: &mut World, registry: RegistryHandle, addr: SocketAddr) -> Result<Self, Error> {
        // Start the listen port
        let listener_events = Mailbox::default();
        let (listener_actions, server_info) =
            tcp::bind(world, registry, addr, listener_events.sender())?;

        event!(Level::DEBUG, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            listener_events,
            listener_actions,
            streams: Vec::new(),
        };
        Ok(actor)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.listener_actions.send(tcp::ListenerAction::Close);

        // Close all not yet closed streams
        for stream in &self.streams {
            if stream.closed {
                continue;
            }

            let _ = stream.actions.send(stream::StreamAction::Close);
        }
    }
}

impl Actor for Service {
    fn register(&mut self, _world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        self.listener_events.set_signal(meta.signal());
        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        // Handle incoming TCP connections
        while let Some(event) = self.listener_events.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    let events = Mailbox::default();
                    events.set_signal(meta.signal());

                    let actions =
                        stream::open(world, event.events, event.actions, events.sender())?;

                    // Track the connection
                    let connection = StreamEntry {
                        events,
                        actions,
                        closed: false,
                    };
                    self.streams.push(connection);
                }
                tcp::ListenerEvent::Closed => meta.set_stop(),
            }
        }

        // Process open streams
        for stream in &mut self.streams {
            while let Some(event) = stream.events.recv() {
                match event {
                    stream::StreamEvent::Request(_) => {
                        let body = "testing response".into();
                        let action = stream::ResponseAction { body };
                        stream
                            .actions
                            .send(stream::StreamAction::Response(action))?;
                    }
                    stream::StreamEvent::Closed => {
                        stream.closed = true;
                    }
                }
            }
        }
        self.streams.retain(|s| !s.closed);

        Ok(())
    }
}
