mod utils;

use anyhow::Error;
use stewart::World;
use tracing::{event, Level};

use crate::hello_service::{start_hello_service, HelloMesage};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::new();
    let mut ctx = world.root();

    // Start the hello service
    let service = start_hello_service(&mut ctx, "Example".to_string())?;

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
    use anyhow::Error;
    use stewart::{Actor, Addr, Context, Options, State};
    use tracing::{event, instrument, Level};

    /// Start a hello service on the current actor world.
    #[instrument(skip_all, fields(name = name))]
    pub fn start_hello_service(
        ctx: &mut Context,
        name: String,
    ) -> Result<Addr<HelloMesage>, Error> {
        event!(Level::INFO, "starting");

        // Create the actor in the world
        let mut ctx = ctx.create(Options::default())?;

        // Start the actor
        let actor = HelloService { name };
        ctx.start(actor)?;

        Ok(ctx.addr()?)
    }

    pub enum HelloMesage {
        Greet(String),
        Stop,
    }

    // The actor implementation below remains entirely private to the module.

    struct HelloService {
        name: String,
    }

    impl Actor for HelloService {
        type Message = HelloMesage;

        #[instrument("hello_service", skip_all, fields(name = self.name))]
        fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some(message) = state.next() {
                // Process the message
                match message {
                    HelloMesage::Greet(to) => {
                        event!(Level::INFO, "Hello, {} from {}!", to, self.name)
                    }
                    HelloMesage::Stop => {
                        event!(Level::INFO, "stopping service");
                        ctx.stop()?;
                    }
                }
            }

            Ok(())
        }
    }
}
