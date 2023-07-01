mod utils;

use anyhow::Error;
use stewart::{Context, Sender, World};
use tracing::{event, Level};
use uuid::Uuid;

// Import the protocol as an alias to differentiate between multiple.
use crate::hello_service::protocol as hello;

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let mut cx = Context::root(&mut world);

    // Start the hello service
    let service = hello_service::start(&mut cx, "Example".to_string())?;

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");

    let action = hello::Action::Greet("World".to_string());
    let message = hello::Message {
        id: Uuid::new_v4(),
        action,
    };
    service.send(&mut cx, message);

    let action = hello::Action::Greet("Actors".to_string());
    let message = hello::Message {
        id: Uuid::new_v4(),
        action,
    };
    service.send(&mut cx, message);

    // Stop the actor, automatically cleaning up associated resources
    let action = hello::Action::Stop {
        // You don't necessarily need to actually do anything with a callback.
        on_result: Sender::noop(),
    };
    let message = hello::Message {
        id: Uuid::new_v4(),
        action,
    };
    service.send(&mut cx, message);

    // Process messages
    world.run_until_idle()?;

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::Error;
    use stewart::{Actor, Context, Sender, State};
    use tracing::{event, instrument, Level};

    /// Define your public interfaces as a "protocol", which contains just the types necessary to
    /// talk to your service. This is equivalent to an "interface" or "trait".
    pub mod protocol {
        use stewart::Sender;
        use uuid::Uuid;

        /// It's good practice to wrap your service's actions in a `Message` type, for adding
        /// additional message metadata.
        pub struct Message {
            /// It's generally a good idea to add an ID to your messages, so it can be tracked at
            /// various stages of the process, and when sent over the network.
            /// This ID should be globally unique, such as by using UUIDs.
            pub id: Uuid,
            pub action: Action,
        }

        pub enum Action {
            Greet(String),
            Stop {
                /// As part of your protocol, you can include senders to respond.
                /// Of course when bridging between worlds and across the network, these can't be
                /// directly serialized, but they can be stored by 'envoy' actors.
                on_result: Sender<Uuid>,
            },
        }
    }

    /// Start a hello service on the current actor world.
    #[instrument("hello::start", skip_all)]
    pub fn start(cx: &mut Context, name: String) -> Result<Sender<protocol::Message>, Error> {
        event!(Level::INFO, "starting");

        // Create the actor in the world
        let hnd = cx.create("hello")?;

        // Start the actor
        let actor = Service { name };
        cx.start(hnd, actor)?;

        Ok(hnd.sender())
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    /// Since it is private, you are recommended to avoid `namespace::namespace`ing your types.
    struct Service {
        name: String,
    }

    impl Actor for Service {
        type Message = protocol::Message;

        fn process(&mut self, cx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            while let Some(message) = state.next() {
                // Process the message
                match message.action {
                    protocol::Action::Greet(to) => {
                        event!(Level::INFO, "Hello \"{}\", from {}!", to, self.name)
                    }
                    protocol::Action::Stop { on_result } => {
                        event!(Level::INFO, "stopping service");

                        state.stop();
                        on_result.send(cx, message.id);
                    }
                }
            }

            Ok(())
        }
    }
}
