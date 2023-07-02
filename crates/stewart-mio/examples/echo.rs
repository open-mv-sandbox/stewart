mod utils;

use anyhow::Error;
use stewart::{utils::Sender, Actor, Context, State};
use stewart_mio::net::udp::{self, Packet};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    stewart_mio::run_event_loop(init)?;

    Ok(())
}

fn init(cx: &mut Context) -> Result<(), Error> {
    let hnd = cx.create("echo-server")?;
    let mut cx = cx.with(hnd);
    let sender = hnd.sender();

    // Start the listen port
    let info = udp::bind(
        &mut cx,
        "0.0.0.0:1234".parse()?,
        sender.clone().map(Message::Server),
    )?;
    event!(Level::INFO, addr = ?info.local_addr(), "listening");
    let server_addr = info.local_addr();
    let server_sender = info.sender().clone();

    // Start the client port
    let info = udp::bind(&mut cx, "0.0.0.0:0".parse()?, sender.map(Message::Client))?;
    event!(Level::INFO, addr = ?info.local_addr(), "sending");

    let actor = EchoExample { server_sender };
    cx.start(hnd, actor)?;

    // Send a message to be echo'd
    let packet = Packet {
        peer: server_addr,
        data: b"Client Packet".to_vec(),
    };
    info.sender().send(&mut cx, packet);

    Ok(())
}

struct EchoExample {
    server_sender: Sender<Packet>,
}

impl Actor for EchoExample {
    type Message = Message;

    fn process(&mut self, cx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        if let Some(message) = state.next() {
            match message {
                Message::Server(mut packet) => {
                    let message = std::str::from_utf8(&packet.data)?;
                    event!(Level::INFO, data = message, "server received packet");

                    // Echo back
                    packet.data = format!("Hello, {}!", message).into_bytes();
                    self.server_sender.send(cx, packet);
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
