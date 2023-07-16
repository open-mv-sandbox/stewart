mod utils;

use anyhow::Error;
use stewart::{Handler, World};
use tracing::{event, Level};
use uuid::Uuid;

// Import the protocol as an alias to differentiate between multiple.
use crate::hello_service::protocol as hello;

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();

    // Start the hello service
    let service = hello_service::start(&mut world, "Example".to_string())?;

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");

    let action = hello::Action::Greet("World".to_string());
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
    };
    service.handle(&mut world, message)?;

    let action = hello::Action::Greet("Actors".to_string());
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
    };
    service.handle(&mut world, message)?;

    // Stop the actor, automatically cleaning up associated resources
    let action = hello::Action::Stop {
        // You don't necessarily need to actually do anything with a callback.
        on_result: Handler::none(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
    };
    service.handle(&mut world, message)?;

    // Process messages
    world.run_until_idle()?;

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::Error;
    use stewart::{Actor, Context, Handler, World};
    use tracing::{event, instrument, Level};

    /// You can define your public interfaces as a "protocol", which contains just the types
    /// necessary to talk to your service.
    /// This is equivalent to an "interface" or "trait".
    pub mod protocol {
        use stewart::Handler;
        use uuid::Uuid;

        /// It's good practice to wrap your service's actions in a `Request` type, for adding
        /// additional message metadata.
        pub struct Request {
            /// It's generally a good idea to add an ID to your messages, so it can be tracked at
            /// various stages of the process, and when sent over the network.
            /// This ID should be globally unique, such as by using UUIDs.
            pub id: Uuid,
            pub action: Action,
        }

        pub enum Action {
            Greet(String),
            Stop {
                /// As part of your protocol, you can include handlers to respond.
                /// Of course when bridging between worlds and across the network, these can't be
                /// directly serialized, but they can be stored by 'envoy' actors.
                on_result: Handler<Uuid>,
            },
        }
    }

    /// Start a hello service on the current actor world.
    #[instrument("hello::start", skip_all)]
    pub fn start(world: &mut World, name: String) -> Result<Handler<protocol::Request>, Error> {
        event!(Level::INFO, "starting");

        // Create the actor in the world
        let id = world.create("hello")?;

        // Start the actor
        let actor = Service { name };
        world.start(id, actor)?;

        // Handlers provide relatively cheap mapping functionality
        let handler = Handler::to(id).map(Message::Request);

        Ok(handler)
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    /// Since it is private, you are recommended to avoid `namespace::namespace`ing your types.
    struct Service {
        name: String,
    }

    /// If your service needs to communicate with other actors, wrapping the public API in an
    /// internal enum lets you multiplex those messages into your actor.
    enum Message {
        Request(protocol::Request),
    }

    impl Actor for Service {
        type Message = Message;

        fn process(&mut self, world: &mut World, mut cx: Context<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some(message) = cx.next_message() {
                let Message::Request(request) = message;

                // Process the message
                match request.action {
                    protocol::Action::Greet(to) => {
                        event!(Level::INFO, "Hello \"{}\", from {}!", to, self.name)
                    }
                    protocol::Action::Stop { on_result } => {
                        event!(Level::INFO, "stopping service");

                        cx.stop();
                        on_result.handle(world, request.id)?;
                    }
                }
            }

            Ok(())
        }
    }
}
