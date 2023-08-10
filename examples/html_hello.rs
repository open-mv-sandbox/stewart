mod utils;

use anyhow::Error;
use stewart::{Actor, Metadata, World};
use stewart_mio::{Registry, RegistryHandle};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Registry::new()?;

    // Start the actor
    let actor = Service::new(&mut world, registry.handle())?;
    world.insert("html-example", actor)?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {}

impl Service {
    pub fn new(world: &mut World, registry: RegistryHandle) -> Result<Self, Error> {
        let addr = "127.0.0.1:1234".parse()?;
        stewart_http::listen(world, registry, addr)?;

        let actor = Service {};
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, _world: &mut World, _meta: &mut Metadata) -> Result<(), Error> {
        Ok(())
    }

    fn process(&mut self, _world: &mut World, _meta: &mut Metadata) -> Result<(), Error> {
        Ok(())
    }
}

//const RESPONSE: &str = "<!DOCTYPE html><html><body><h1>Hello, World!</h1></body></html>";
