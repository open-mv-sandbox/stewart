mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, Handler, State, World};
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

fn init(world: &mut World, cx: &Context, registry: &Rc<Registry>) -> Result<(), Error> {
    let (cx, id) = world.create(cx, "echo-example")?;
    let sender = Handler::to(id);

    // Start the listen port
    let info = udp::bind(
        world,
        &cx,
        registry.clone(),
        "0.0.0.0:1234".parse()?,
        sender.clone().map(Message::Server),
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "listening");
    let server_addr = info.local_addr();
    let server_sender = info.sender().clone();

    // Start the client port
    let info = udp::bind(
        world,
        &cx,
        registry.clone(),
        "0.0.0.0:0".parse()?,
        sender.map(Message::Client),
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "sending");

    let actor = EchoExample { server_sender };
    world.start(id, actor)?;

    // Send a message to be echo'd
    let packet = Packet {
        peer: server_addr,
        data: b"Client Packet".to_vec(),
    };
    info.sender().handle(world, packet);

    Ok(())
}

struct EchoExample {
    server_sender: Handler<Packet>,
}

impl Actor for EchoExample {
    type Message = Message;

    fn process(
        &mut self,
        world: &mut World,
        _cx: &Context,
        state: &mut State<Self>,
    ) -> Result<(), Error> {
        if let Some(message) = state.next() {
            match message {
                Message::Server(mut packet) => {
                    let message = std::str::from_utf8(&packet.data)?;
                    event!(Level::INFO, data = message, "server received packet");

                    // Echo back
                    packet.data = format!("Hello, {}!", message).into_bytes();
                    self.server_sender.handle(world, packet);
                }
                Message::Client(packet) => {
                    let message = std::str::from_utf8(&packet.data)?;
                    event!(Level::INFO, data = message, "client received packet");
                }
            }
        }

        Ok(())
    }
}

enum Message {
    Server(Packet),
    Client(Packet),
}
