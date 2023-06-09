mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, Handler, Id, World};
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
    let id = world.create(Id::none(), "echo-example")?;
    let handler = Handler::to(id);

    // Start the listen port
    let info = udp::bind(
        world,
        id,
        registry.clone(),
        "0.0.0.0:1234".parse()?,
        handler.clone().map(Message::Server),
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "listening");
    let server_addr = info.local_addr();
    let server_handler = info.handler().clone();

    // Start the client port
    let info = udp::bind(
        world,
        id,
        registry.clone(),
        "0.0.0.0:0".parse()?,
        handler.map(Message::Client),
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "sending");

    let actor = EchoExample { server_handler };
    world.start(id, actor)?;

    // Send a message to be echo'd
    let packet = Packet {
        peer: server_addr,
        data: b"Client Packet".to_vec(),
    };
    info.handler().handle(world, packet)?;

    let packet = Packet {
        peer: server_addr,
        data: b"Somewhat Longer Packet".to_vec(),
    };
    info.handler().handle(world, packet)?;

    Ok(())
}

struct EchoExample {
    server_handler: Handler<Packet>,
}

enum Message {
    Server(Packet),
    Client(Packet),
}

impl Actor for EchoExample {
    type Message = Message;

    fn process(&mut self, world: &mut World, mut cx: Context<Self>) -> Result<(), Error> {
        while let Some(message) = cx.next() {
            match message {
                Message::Server(mut packet) => {
                    let data = std::str::from_utf8(&packet.data)?;
                    event!(Level::INFO, data, "server received packet");

                    // Echo back with a hello message
                    packet.data = format!("Hello, \"{}\"!", data).into_bytes();
                    self.server_handler.handle(world, packet)?;
                }
                Message::Client(packet) => {
                    let data = std::str::from_utf8(&packet.data)?;
                    event!(Level::INFO, data, "client received packet");
                }
            }
        }

        Ok(())
    }
}
