mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{
    net::udp::{self, Packet},
    Registry,
};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    stewart_mio::run_event_loop(init)?;

    Ok(())
}

fn init(world: &mut World, registry: &Rc<Registry>) -> Result<(), Error> {
    let (server_packet, server_sender) = mailbox();
    let (client_packet, client_sender) = mailbox();

    // Start the listen port
    let info = udp::bind(
        world,
        registry.clone(),
        "0.0.0.0:1234".parse()?,
        server_sender,
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "listening");
    let server_addr = info.local_addr();
    let server_sender = info.sender().clone();

    // Start the client port
    let info = udp::bind(world, registry.clone(), "0.0.0.0:0".parse()?, client_sender)?;
    event!(Level::INFO, addr = ?info.local_addr(), "sending");

    let actor = EchoExample {
        server_packet: server_packet.clone(),
        client_packet: client_packet.clone(),
        server_sender,
    };
    let signal = world.create("echo-example", actor);
    server_packet.register(signal.clone());
    client_packet.register(signal);

    // Send a message to be echo'd
    let packet = Packet {
        peer: server_addr,
        data: b"Client Packet".to_vec(),
    };
    info.sender().send(packet)?;

    let packet = Packet {
        peer: server_addr,
        data: b"Somewhat Longer Packet".to_vec(),
    };
    info.sender().send(packet)?;

    Ok(())
}

struct EchoExample {
    server_packet: Mailbox<Packet>,
    client_packet: Mailbox<Packet>,
    server_sender: Sender<Packet>,
}

impl Actor for EchoExample {
    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(mut packet) = self.server_packet.recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "server received packet");

            // Echo back with a hello message
            packet.data = format!("Hello, \"{}\"!", data).into_bytes();
            self.server_sender.send(packet)?;
        }

        while let Some(packet) = self.client_packet.recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "client received packet");
        }

        Ok(())
    }
}