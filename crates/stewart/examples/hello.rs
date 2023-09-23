use anyhow::Error;
use std::ops::ControlFlow;
use stewart::sender::Sender;
use stewart::{Actor, ActorError, Runtime};
use tracing::{event, Level};
use uuid::Uuid;

// Import the protocol as an alias to differentiate between multiple protocol modules.
use crate::hello_service::protocol as hello;

fn main() -> Result<(), Error> {
    devutils::init_logging();

    let mut rt = Runtime::default();

    // Start the hello service
    let service_sender = hello_service::start(&mut rt, "Example".to_string())?;

    // Start a result receiver, so we can actually receive the replies given
    let id = rt.insert("result-listener", ResultListener)?;
    let result_sender = Sender::new(id);

    // Now that we have an address, send it some data
    event!(Level::INFO, "sending messages");

    let action = hello::Action::Greet {
        name: "World".to_string(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
        result_sender: result_sender.clone(),
    };
    service_sender.send(&mut rt, message)??;

    let action = hello::Action::Greet {
        name: "Actors".to_string(),
    };
    let message = hello::Request {
        id: Uuid::new_v4(),
        action,
        result_sender: result_sender.clone(),
    };
    service_sender.send(&mut rt, message)??;

    // Stop the actor
    let message = hello::Request {
        id: Uuid::new_v4(),
        action: hello::Action::Stop,
        result_sender: result_sender.clone(),
    };
    service_sender.send(&mut rt, message)??;

    // Process messages
    rt.process()?;

    Ok(())
}

struct ResultListener;

impl Actor for ResultListener {
    type Message = Uuid;

    fn handle(
        &mut self,
        _rt: &mut Runtime,
        message: Self::Message,
    ) -> Result<ControlFlow<()>, ActorError> {
        event!(Level::INFO, uuid = ?message, "received response");
        Ok(ControlFlow::Continue(()))
    }
}

/// To demonstrate encapsulation, an inner module is used here.
mod hello_service {
    use anyhow::{Context, Error};
    use std::ops::ControlFlow;
    use stewart::{sender::Sender, Actor, ActorError, Runtime};
    use tracing::{event, instrument, Level};

    /// You can define your public interfaces as a "protocol", which contains just the types
    /// necessary to talk to your service.
    /// This is equivalent to an "interface" or "trait".
    pub mod protocol {
        use stewart::sender::Sender;
        use uuid::Uuid;

        /// It's good practice to wrap your service's actions in a `Request` type, for adding
        /// additional message metadata.
        pub struct Request {
            /// It's can be a good idea to add an ID to your messages, so it can be tracked at
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
            Greet { name: String },
            Stop,
        }
    }

    /// Start a hello service on the current actor world.
    #[instrument("hello::start", skip_all)]
    pub fn start(rt: &mut Runtime, name: String) -> Result<Sender<protocol::Request>, Error> {
        event!(Level::INFO, "starting");

        let service = Service::new(name);
        let id = rt.insert("hello", service)?;

        // Send messages to your actor using a `Sender` abstraction.
        // You can send directly using the runtime, but using a sender will let you add mapping to
        // separate public from private messages.
        let sender = Sender::new(id);

        Ok(sender)
    }

    /// The actor implementation remains entirely private to the module, only exposed through the
    /// `start` function above.
    /// Since it is private, you are recommended to avoid `namespace::namespace`ing your types.
    struct Service {
        name: String,
    }

    impl Service {
        fn new(name: String) -> Self {
            Self { name }
        }
    }

    impl Actor for Service {
        type Message = protocol::Request;

        fn handle(
            &mut self,
            rt: &mut Runtime,
            message: protocol::Request,
        ) -> Result<ControlFlow<()>, ActorError> {
            event!(Level::INFO, "processing messages");

            let mut flow = ControlFlow::Continue(());

            // Process messages on the mailbox
            match message.action {
                protocol::Action::Greet { name } => {
                    event!(Level::INFO, "Hello \"{}\", from {}!", name, self.name);
                }
                protocol::Action::Stop => {
                    event!(Level::INFO, "stopping service");
                    flow = ControlFlow::Break(());
                }
            }

            // Reply back to the sender.
            message
                .result_sender
                .send(rt, message.id)
                .context("failed to send")?
                .context("failed to send")?;

            Ok(flow)
        }
    }
}
