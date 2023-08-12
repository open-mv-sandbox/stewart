mod utils;

use anyhow::Error;
use stewart::{message::Mailbox, Actor, Metadata, World};
use stewart_http::{HttpEvent, RequestAction};
use stewart_mio::{Registry, RegistryHandle};
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    utils::init_logging();

    let mut world = World::default();
    let registry = Registry::new()?;

    // Start the actor
    let actor = Service::new(&mut world, registry.handle())?;
    world.insert("html-example", actor)?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

struct Service {
    http_events: Mailbox<HttpEvent>,
}

impl Service {
    pub fn new(world: &mut World, registry: RegistryHandle) -> Result<Self, Error> {
        let http_events = Mailbox::default();

        let addr = "127.0.0.1:1234".parse()?;
        stewart_http::listen(world, registry, addr, http_events.sender())?;

        let actor = Service { http_events };
        Ok(actor)
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());
        self.http_events.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, _world: &mut World, _meta: &mut Metadata) -> Result<(), Error> {
        while let Some(event) = self.http_events.recv() {
            let HttpEvent::Request(request) = event;

            event!(Level::INFO, "received request");

            let body = RESPONSE.into();
            request.actions.send(RequestAction::SendResponse(body))?;
        }

        Ok(())
    }
}

const RESPONSE: &str = "<!DOCTYPE html><html><body><h1>Hello, World!</h1></body></html>";
