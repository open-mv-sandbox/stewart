use std::net::SocketAddr;

use anyhow::Error;
use bytes::Bytes;
use stewart::{
    message::{Mailbox, Sender},
    Actor, Id, World,
};
use stewart_mio::{net::tcp, RegistryHandle};
use tracing::{event, Level};

use crate::connection;

pub fn listen(
    world: &mut World,
    registry: RegistryHandle,
    addr: SocketAddr,
    body: Bytes,
) -> Result<(), Error> {
    let actor = Service::new(world, registry, addr, body)?;
    world.insert("http-server", actor)?;

    Ok(())
}

struct Service {
    listener_mailbox: Mailbox<tcp::ListenerEvent>,
    listener_sender: Sender<tcp::ListenerAction>,
    body: Bytes,
}

impl Service {
    fn new(
        world: &mut World,
        registry: RegistryHandle,
        addr: SocketAddr,
        body: Bytes,
    ) -> Result<Self, Error> {
        let listener_mailbox = Mailbox::default();

        // Start the listen port
        let (listener_sender, server_info) =
            tcp::bind(world, registry, addr, listener_mailbox.sender())?;
        event!(Level::INFO, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            listener_mailbox,
            listener_sender,
            body,
        };
        Ok(actor)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.listener_sender.send(tcp::ListenerAction::Close);
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        self.listener_mailbox.set_signal(world.signal(id));
        Ok(())
    }

    fn process(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        while let Some(event) = self.listener_mailbox.recv() {
            match event {
                tcp::ListenerEvent::Connected(event) => {
                    connection::open(world, event, self.body.clone())?
                }
                tcp::ListenerEvent::Closed => world.stop(id),
            }
        }

        Ok(())
    }
}
