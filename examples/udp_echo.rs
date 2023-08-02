mod utils;

use std::{net::SocketAddr, rc::Rc};

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{net::udp, Registry};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Rc::new(Registry::new()?);

    // Start the actor
    let actor = Service::new(&mut world, &registry)?;
    let server_addr = actor.server_addr;
    let client_send = actor.client_sender.clone();
    world.insert("echo-example", actor)?;

    // Send a message to be echo'd
    let packet = udp::SendAction {
        remote: server_addr,
        data: b"Client Packet".to_vec(),
    };
    let message = udp::Action::Send(packet);
    client_send.send(message)?;

    let packet = udp::SendAction {
        remote: server_addr,
        data: b"Somewhat Longer Packet".to_vec(),
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
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        // Start the listen port
        let (server_mailbox, server_sender) = mailbox();
        let (server_sender, info) = udp::bind(
            world,
            registry.clone(),
            "0.0.0.0:1234".parse()?,
            server_sender,
        )?;
        event!(Level::INFO, addr = ?info.local_addr, "listening");
        let server_addr = info.local_addr;

        // Start the client port
        let (client_mailbox, client_sender) = mailbox();
        let (client_sender, info) =
            udp::bind(world, registry.clone(), "0.0.0.0:0".parse()?, client_sender)?;
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
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server_mailbox.set_signal(ctx.signal());
        self.client_mailbox.set_signal(ctx.signal());

        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(packet) = self.server_mailbox.recv()? {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "server received packet");

            // Echo back with a hello message
            let packet = udp::SendAction {
                remote: packet.remote,
                data: format!("Hello, \"{}\"!", data).into_bytes(),
            };
            let message = udp::Action::Send(packet);
            self.server_sender.send(message)?;
        }

        while let Some(packet) = self.client_mailbox.recv()? {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "client received packet");
        }

        Ok(())
    }
}
