mod utils;

use anyhow::Error;
use stewart::{Mailbox, World};
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
    service.send(&mut world, message)?;

    let action = hello::Action::Greet("Actors".to_string());
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
    };
    service.send(&mut world, message)?;

    // Stop the actor
    let mailbox = Mailbox::default();
    let action = hello::Action::Stop {
        // Mailboxes don't need to be associated with an actor.
        on_result: mailbox.sender(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
    };
    service.send(&mut world, message)?;

    // Process messages
    world.run_until_idle()?;

    // We can receive messages outside actors by just checking
    while let Some(uuid) = mailbox.next() {
        event!(Level::INFO, ?uuid, "received stop response");
    }

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::Error;
    use stewart::{Actor, Context, Mailbox, Sender, World};
    use tracing::{event, instrument, Level};

    /// You can define your public interfaces as a "protocol", which contains just the types
    /// necessary to talk to your service.
    /// This is equivalent to an "interface" or "trait".
    pub mod protocol {
        use stewart::Sender;
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
                on_result: Sender<Uuid>,
            },
        }
    }

    /// Start a hello service on the current actor world.
    #[instrument("hello::start", skip_all)]
    pub fn start(world: &mut World, name: String) -> Result<Sender<protocol::Request>, Error> {
        event!(Level::INFO, "starting");

        // Mailboxes let you send message around
        let mailbox = Mailbox::default();

        // Create the actor in the world
        let actor = Service {
            name,
            mailbox: mailbox.clone(),
        };
        let id = world.create("hello", actor);

        // To wake up our actor when a message gets sent, register it with the mailbox for
        // notification
        mailbox.register(id);

        Ok(mailbox.sender())
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    /// Since it is private, you are recommended to avoid `namespace::namespace`ing your types.
    struct Service {
        name: String,
        mailbox: Mailbox<protocol::Request>,
    }

    impl Actor for Service {
        fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            // Process messages on the mailbox
            while let Some(request) = self.mailbox.next() {
                match request.action {
                    protocol::Action::Greet(to) => {
                        event!(Level::INFO, "Hello \"{}\", from {}!", to, self.name)
                    }
                    protocol::Action::Stop { on_result } => {
                        event!(Level::INFO, "stopping service");

                        ctx.stop();
                        on_result.send(ctx, request.id)?;
                    }
                }
            }

            Ok(())
        }

        fn stop(&mut self, _ctx: &mut Context) {
            event!(Level::INFO, "service stopping");
        }
    }
}
