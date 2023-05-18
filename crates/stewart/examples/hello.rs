mod utils;

use anyhow::Error;
use stewart::World;
use tracing::{event, Level};

use crate::hello_service::start_hello_service;

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::new();

    // Start the hello service
    let service = start_hello_service(&mut world, "Example".to_string())?;

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");
    world.send(service, "World");
    world.send(service, "Actors");

    // Process messages
    world.run_until_idle()?;

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::{Context, Error};
    use stewart::{Addr, State, System, SystemOptions, World};
    use tracing::{event, instrument, span, Level};

    /// Start a hello service on the current actor world.
    #[instrument(skip_all, fields(name = name))]
    pub fn start_hello_service(world: &mut World, name: String) -> Result<Addr<String>, Error> {
        event!(Level::DEBUG, "creating service");

        // Create the actor in the world
        let id = world.create(None)?;

        // Create a system, self-owned by this actor
        let system = world.register(HelloSystem, id, SystemOptions::default());

        // Start the instance on the actor
        let instance = HelloService { name };
        world.start(id, system, instance)?;

        Ok(Addr::new(id))
    }

    // The actor implementation below remains entirely private to the module.

    struct HelloSystem;

    impl System for HelloSystem {
        type Instance = HelloService;
        type Message = String;

        fn process(&mut self, _world: &mut World, state: &mut State<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some((id, message)) = state.next() {
                let instance = state.get_mut(id).context("failed to get instance")?;

                let span = span!(Level::INFO, "hello-service", name = instance.name);
                let _enter = span.enter();

                event!(Level::INFO, "Hello, {} from {}!", message, instance.name);
            }

            Ok(())
        }
    }

    struct HelloService {
        name: String,
    }
}
