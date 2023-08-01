mod utils;

use std::rc::Rc;

use anyhow::Error;
use stewart::{Actor, Context, World};
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
    server: tcp::Listener,
}

impl Service {
    pub fn new(world: &mut World, registry: &Rc<Registry>) -> Result<Self, Error> {
        // Start the listen port
        let server = tcp::listen(world, registry.clone(), "127.0.0.1:1234".parse()?)?;
        event!(Level::INFO, addr = ?server.local_addr(), "listening");

        let actor = Service { server };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.server.events().set_signal(ctx.signal());

        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(_stream) = self.server.events().recv() {
            event!(Level::INFO, "stream accepted");
        }

        Ok(())
    }
}
