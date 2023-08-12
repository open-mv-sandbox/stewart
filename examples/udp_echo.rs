mod utils;

use std::net::SocketAddr;

use anyhow::Error;
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::{net::udp, Registry, RegistryRef};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Registry::new()?;

    // Start the actor
    let actor = Service::new(&mut world, registry.handle())?;
    let server_addr = actor.server_addr;
    let client_send = actor.client_sender.clone();
    world.insert("echo-example", actor)?;

    // Send a message to be echo'd
    let packet = udp::SendAction {
        remote: server_addr,
        data: "Client Packet".into(),
    };
    let message = udp::Action::Send(packet);
    client_send.send(message)?;

    let packet = udp::SendAction {
        remote: server_addr,
        data: "Somewhat Longer Packet".into(),
    };
    let message = udp::Action::Send(packet);
    client_send.send(message)?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {
    server_mailbox: Mailbox<udp::RecvEvent>,
    server_sender: Sender<udp::Action>,
    server_addr: SocketAddr,

    client_mailbox: Mailbox<udp::RecvEvent>,
    client_sender: Sender<udp::Action>,
}

impl Service {
    pub fn new(world: &mut World, registry: RegistryRef) -> Result<Self, Error> {
        // Start the listen port
        let server_mailbox = Mailbox::default();
        let (server_sender, info) = udp::bind(
            world,
            registry.clone(),
            "0.0.0.0:1234".parse()?,
            server_mailbox.sender(),
        )?;
        event!(Level::INFO, addr = ?info.local_addr, "listening");
        let server_addr = info.local_addr;

        // Start the client port
        let client_mailbox = Mailbox::default();
        let (client_sender, info) = udp::bind(
            world,
            registry.clone(),
            "0.0.0.0:0".parse()?,
            client_mailbox.sender(),
        )?;
        event!(Level::INFO, addr = ?info.local_addr, "sending");

        let actor = Service {
            server_mailbox,
            server_sender,
            server_addr,

            client_mailbox,
            client_sender,
        };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());

        self.server_mailbox.set_signal(signal.clone());
        self.client_mailbox.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, _world: &mut World, _meta: &mut Metadata) -> Result<(), Error> {
        while let Some(packet) = self.server_mailbox.recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "server received packet");

            // Echo back with a hello message
            let data = data.trim();
            let packet = udp::SendAction {
                remote: packet.remote,
                data: format!("Hello, \"{}\"!\n", data).into(),
            };
            let message = udp::Action::Send(packet);
            self.server_sender.send(message)?;
        }

        while let Some(packet) = self.client_mailbox.recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "client received packet");
        }

        Ok(())
    }
}
