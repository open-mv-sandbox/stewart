mod utils;

use anyhow::Error;
use stewart::World;
use tracing::{event, Level};

use crate::hello_service::{start_hello_service, HelloMesage};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::new();

    // Start the hello service
    let service = start_hello_service(&mut world, "Example".to_string())?;

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");
    world.send(service, HelloMesage::Greet("World".to_string()));
    world.send(service, HelloMesage::Greet("Actors".to_string()));

    // Stop the actor, automatically cleaning up associated resources
    world.send(service, HelloMesage::Stop);

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
    pub fn start_hello_service(
        world: &mut World,
        name: String,
    ) -> Result<Addr<HelloMesage>, Error> {
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

    pub enum HelloMesage {
        Greet(String),
        Stop,
    }

    // The actor implementation below remains entirely private to the module.

    struct HelloSystem;

    impl System for HelloSystem {
        type Instance = HelloService;
        type Message = HelloMesage;

        fn process(&mut self, world: &mut World, state: &mut State<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some((id, message)) = state.next() {
                let instance = state.get_mut(id).context("failed to get instance")?;

                let span = span!(Level::INFO, "hello_service", name = instance.name);
                let _enter = span.enter();

                // Process the message
                match message {
                    HelloMesage::Greet(to) => {
                        event!(Level::INFO, "Hello, {} from {}!", to, instance.name)
                    }
                    HelloMesage::Stop => {
                        event!(Level::INFO, "stopping service");
                        world.stop(id)?;
                    }
                }
            }

            Ok(())
        }
    }

    struct HelloService {
        name: String,
    }
}
