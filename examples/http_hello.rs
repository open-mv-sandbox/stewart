mod utils;

use std::ops::ControlFlow;
use anyhow::Error;
use stewart::{
    message::{Mailbox, Signal},
    Actor, Runtime,
};
use stewart_http::{HttpEvent, RequestAction};
use stewart_mio::{Registry, RegistryRef};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = Runtime::default();
    let registry = Registry::new()?;

    // Start the actor
    let signal = Signal::default();
    let actor = Service::new(&mut world, signal.clone(), registry.handle())?;
    let id = world.insert("http-hello", actor);
    signal.set_id(id);

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {
    http_events: Mailbox<HttpEvent>,
}

impl Service {
    pub fn new(world: &mut Runtime, signal: Signal, registry: RegistryRef) -> Result<Self, Error> {
        let http_events = Mailbox::new(signal);

        let addr = "127.0.0.1:1234".parse()?;
        stewart_http::bind(world, registry, addr, http_events.sender())?;

        let this = Service { http_events };
        Ok(this)
    }
}

impl Actor for Service {
    fn process(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        while let Some(event) = self.http_events.recv() {
            let HttpEvent::Request(request) = event;

            event!(Level::INFO, "received request");
            println!("HEADER: {:?}", request.header);

            let body = RESPONSE.into();
            request
                .actions
                .send(world, RequestAction::SendResponse(body)).unwrap();
        }

        ControlFlow::Continue(())
    }
}

const RESPONSE: &str = "<!DOCTYPE html><html><body><h1>Hello, World!</h1></body></html>";
