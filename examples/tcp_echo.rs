mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox, Sender};
use stewart_mio::{
    net::tcp::{self},
    Registry,
};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Rc::new(Registry::new()?);

    // Start the actor
    let actor = Service::new(&mut world, &registry)?;
    world.insert("echo-example", actor)?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {
    server_mailbox: Mailbox<tcp::ConnectedEvent>,
    _server_sender: Sender<tcp::ListenerAction>,
    streams: Vec<tcp::ConnectedEvent>,
}

impl Service {
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        let (server_mailbox, on_event) = mailbox();

        // Start the listen port
        let (server_sender, server_info) =
            tcp::listen(world, registry.clone(), "127.0.0.1:1234".parse()?, on_event)?;
        event!(Level::INFO, addr = ?server_info.local_addr, "listening");

        let actor = Service {
            server_mailbox,
            _server_sender: server_sender,
            streams: Vec::new(),
        };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server_mailbox.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        while let Some(stream) = self.server_mailbox.recv() {
            event!(Level::INFO, "stream accepted");

            // Send a greeting message
            let data = b"HELLO WORLD\n".to_vec();
            let action = tcp::SendAction { data };
            stream
                .actions_sender
                .send(tcp::StreamAction::Send(action))?;

            // Keep track of the stream
            stream.event_mailbox.set_signal(ctx.signal());
            self.streams.push(stream);
        }

        for stream in &self.streams {
            while let Some(event) = stream.event_mailbox.recv() {
                event!(Level::INFO, bytes = event.data.len(), "received data");

                // Reply with an echo
                let data = std::str::from_utf8(&event.data)?;
                let data = data.trim();
                let packet = tcp::SendAction {
                    data: format!("HELLO, \"{}\"!\n", data).into_bytes(),
                };
                let message = tcp::StreamAction::Send(packet);
                stream.actions_sender.send(message)?;
            }
        }

        Ok(())
    }
}
