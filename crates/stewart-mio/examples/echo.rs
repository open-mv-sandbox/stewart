mod utils;

use anyhow::Error;
use stewart::{Actor, Context, State};
use stewart_mio::net::udp_bind;
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    stewart_mio::run_event_loop(init)?;

    Ok(())
}

fn init(cx: &mut Context) -> Result<(), Error> {
    let hnd = cx.create("echo-server")?;
    let mut cx = cx.with(hnd);

    // Start the listen port
    let info = udp_bind(&mut cx, "0.0.0.0:1234".parse()?)?;
    event!(Level::INFO, addr = ?info.local_addr(), "listening");

    let actor = EchoServer;
    cx.start(hnd, actor)?;

    Ok(())
}

struct EchoServer;

impl Actor for EchoServer {
    type Message = ();

    fn process(&mut self, _cx: &mut Context, _state: &mut State<Self>) -> Result<(), Error> {
        event!(Level::INFO, "received event");

        Ok(())
    }
}
