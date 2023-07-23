mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_mio::{
    net::udp::{self, SocketInfo},
    Registry,
};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    stewart_mio::run_event_loop(init)?;

    Ok(())
}

fn init(world: &mut World, registry: &Rc<Registry>) -> Result<(), Error> {
    // Start the actor
    let actor = EchoExample::new(world, registry)?;
    let server_addr = actor.server.local_addr();
    let client_send = actor.client.send().clone();
    world.create("echo-example", actor)?;

    // Send a message to be echo'd
    let packet = udp::Packet {
        peer: server_addr,
        data: b"Client Packet".to_vec(),
    };
    let message = udp::Message::Send(packet);
    client_send.send(message)?;

    let packet = udp::Packet {
        peer: server_addr,
        data: b"Somewhat Longer Packet".to_vec(),
    };
    let message = udp::Message::Send(packet);
    client_send.send(message)?;

    Ok(())
}

struct EchoExample {
    server: SocketInfo,
    client: SocketInfo,
}

impl EchoExample {
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        // Start the listen port
        let server = udp::bind(world, registry.clone(), "0.0.0.0:1234".parse()?)?;
        event!(Level::INFO, addr = ?server.local_addr(), "listening");

        // Start the client port
        let client = udp::bind(world, registry.clone(), "0.0.0.0:0".parse()?)?;
        event!(Level::INFO, addr = ?client.local_addr(), "sending");

        let actor = EchoExample { server, client };

        Ok(actor)
    }
}

impl Actor for EchoExample {
    fn start(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server.recv().signal(ctx.signal());
        self.client.recv().signal(ctx.signal());

        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(mut packet) = self.server.recv().recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "server received packet");

            // Echo back with a hello message
            packet.data = format!("Hello, \"{}\"!", data).into_bytes();
            let message = udp::Message::Send(packet);
            self.server.send().send(message)?;
        }

        while let Some(packet) = self.client.recv().recv() {
            let data = std::str::from_utf8(&packet.data)?;
            event!(Level::INFO, data, "client received packet");
        }

        Ok(())
    }
}
