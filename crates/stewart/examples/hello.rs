mod utils;

use anyhow::Error;
use stewart::World;
use tracing::{event, Level};

use crate::hello_service::{HelloAction, HelloMessage};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::new();
    let mut ctx = world.root();

    // Start the hello service
    let service = hello_service::start(&mut ctx, "Example".to_string())?;

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");

    let action = HelloAction::Greet("World".to_string());
    let message = HelloMessage { action };
    service.send(&mut ctx, message);

    let action = HelloAction::Greet("Actors".to_string());
    let message = HelloMessage { action };
    service.send(&mut ctx, message);

    // Stop the actor, automatically cleaning up associated resources
    let action = HelloAction::Stop;
    let message = HelloMessage { action };
    service.send(&mut ctx, message);

    // Process messages
    world.run_until_idle()?;

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::Error;
    use stewart::{Actor, Context, Options, Sender, State};
    use tracing::{event, instrument, Level};

    /// Start a hello service on the current actor world.
    #[instrument("hello_service", skip_all, fields(name))]
    pub fn start(ctx: &mut Context, name: String) -> Result<Sender<HelloMessage>, Error> {
        event!(Level::INFO, name, "starting");

        // Create the actor in the world
        let (mut ctx, sender) = ctx.create(Options::default())?;

        // Start the actor
        let actor = HelloService { name };
        ctx.start(actor)?;

        Ok(sender)
    }

    /// It's good practice to wrap your service's actions in a `Message` type, for adding
    /// additional message metadata. For example, a common pattern is to add a tracking UUID,
    /// so a caller can differentiate between replies.
    pub struct HelloMessage {
        pub action: HelloAction,
    }

    pub enum HelloAction {
        Greet(String),
        Stop,
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    struct HelloService {
        name: String,
    }

    impl Actor for HelloService {
        type Message = HelloMessage;

        #[instrument("hello_service", skip_all, fields(name = self.name))]
        fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some(message) = state.next() {
                // Process the message
                match message.action {
                    HelloAction::Greet(to) => {
                        event!(Level::INFO, "Hello, \"{}\"!", to)
                    }
                    HelloAction::Stop => {
                        event!(Level::INFO, "stopping service");
                        ctx.stop()?;
                    }
                }
            }

            Ok(())
        }
    }
}
