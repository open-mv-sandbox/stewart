mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
use stewart_message::{mailbox, Mailbox};
use stewart_mio::{net::tcp, Registry};
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
    server_mailbox: Mailbox<tcp::StreamConnectedEvent>,
}

impl Service {
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        let (server_mailbox, on_event) = mailbox();

        // Start the listen port
        let (_server_sender, server_info) =
            tcp::listen(world, registry.clone(), "127.0.0.1:1234".parse()?, on_event)?;
        event!(Level::INFO, addr = ?server_info.local_addr, "listening");

        let actor = Service { server_mailbox };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server_mailbox.set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(_stream) = self.server_mailbox.recv() {
            event!(Level::INFO, "stream accepted");
        }

        Ok(())
    }
}
