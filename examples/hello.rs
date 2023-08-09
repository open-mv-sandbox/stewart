mod utils;

use anyhow::Error;
use stewart::{message::Mailbox, World};
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

    // Mailboxes don't need to be associated with an actor.
    let mailbox = Mailbox::default();
    mailbox.set_floating();

    let action = hello::Action::Greet {
        name: "World".to_string(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
        result_sender: mailbox.sender(),
    };
    service.send(message)?;

    let action = hello::Action::Greet {
        name: "Actors".to_string(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
        result_sender: mailbox.sender(),
    };
    service.send(message)?;

    // Stop the actor
    let message = hello::Request {
        id: Uuid::new_v4(),
        action: hello::Action::Stop,
        result_sender: mailbox.sender(),
    };
    service.send(message)?;

    // Process messages
    world.run_until_idle()?;

    // We can receive messages outside actors by just checking
    while let Some(uuid) = mailbox.recv() {
        event!(Level::INFO, ?uuid, "received response");
    }

    Ok(())
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::Error;
    use stewart::{
        message::{Mailbox, Sender},
        Actor, Id, World,
    };
    use tracing::{event, instrument, Level};

    /// You can define your public interfaces as a "protocol", which contains just the types
    /// necessary to talk to your service.
    /// This is equivalent to an "interface" or "trait".
    pub mod protocol {
        use stewart::message::Sender;
        use uuid::Uuid;

        /// It's good practice to wrap your service's actions in a `Request` type, for adding
        /// additional message metadata.
        pub struct Request {
            /// It's generally a good idea to add an ID to your messages, so it can be tracked at
            /// various stages of the process, and when sent over the network.
            /// This ID should be globally unique, such as by using UUIDs.
            pub id: Uuid,

            /// You can make different actions available through the same protocol message.
            /// Though, here we only need one.
            pub action: Action,

            /// As part of your protocol, you can include handlers to respond.
            /// Of course when bridging between worlds and across the network, these can't be
            /// directly serialized, but they can be translated by 'envoy' actors.
            pub result_sender: Sender<Uuid>,
        }

        pub enum Action {
            Greet {
                name: String,
            },
            /// It's important to send back when stop has completed, as dropping all senders
            /// will raise errors in mailboxes.
            /// By sending back when it's done, other actors can wait with cleaning them up
            /// until the stop is completed.
            Stop,
        }
    }

    /// Start a hello service on the current actor world.
    #[instrument("hello::start", skip_all)]
    pub fn start(world: &mut World, name: String) -> Result<Sender<protocol::Request>, Error> {
        event!(Level::INFO, "starting");

        let (actor, sender) = Service::new(name);
        world.insert("hello", actor)?;

        Ok(sender)
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    /// Since it is private, you are recommended to avoid `namespace::namespace`ing your types.
    struct Service {
        name: String,
        mailbox: Mailbox<protocol::Request>,
    }

    impl Service {
        fn new(name: String) -> (Self, Sender<protocol::Request>) {
            // Mailboxes let you send message around
            let mailbox = Mailbox::default();
            let sender = mailbox.sender();

            // Create the actor in the world
            let actor = Service { name, mailbox };

            (actor, sender)
        }
    }

    impl Actor for Service {
        fn register(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
            // To wake up our actor when a message gets sent, register it with the mailbox for
            // notification
            self.mailbox.set_signal(world.signal(id));

            Ok(())
        }

        fn process(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
            event!(Level::INFO, "processing messages");

            // Process messages on the mailbox
            while let Some(request) = self.mailbox.recv() {
                match request.action {
                    protocol::Action::Greet { name } => {
                        event!(Level::INFO, "Hello \"{}\", from {}!", name, self.name);
                    }
                    protocol::Action::Stop => {
                        world.stop(id);
                    }
                }

                // Reply back to the sender
                request.result_sender.send(request.id)?;
            }

            Ok(())
        }
    }
}
