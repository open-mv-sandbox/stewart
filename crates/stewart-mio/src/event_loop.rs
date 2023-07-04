use std::rc::Rc;

use anyhow::{Context as _, Error};
use mio::{Events, Poll};
use stewart::World;
use tracing::{event, instrument, Level};

use crate::{registry::WakeEvent, Registry};

// TODO: This needs cleanup, the functions are hard to read, and this file contains too much

#[instrument("mio-event-loop", skip_all)]
pub fn run_event_loop<I>(init: I) -> Result<(), Error>
where
    I: FnOnce(&mut World, &Rc<Registry>) -> Result<(), Error>,
{
    // Set up the local world
    let mut world = World::default();

    // Initialize mio context
    let poll = Poll::new()?;
    let registry = Rc::new(Registry::new(poll));

    // User init
    init(&mut world, &registry)?;

    // Process pending messages raised from initialization
    event!(Level::TRACE, "processing init messages");
    world.run_until_idle()?;

    // Run the inner mio loop
    let result = run_poll_loop(&mut world, &registry);
    if let Err(error) = result {
        // TODO: Shut down or restart the system?
        event!(Level::ERROR, "error in event pipeline: {}", error);
    }

    Ok(())
}

fn run_poll_loop(world: &mut World, registry: &Rc<Registry>) -> Result<(), Error> {
    let mut events = Events::with_capacity(128);
    loop {
        registry.poll.borrow_mut().poll(&mut events, None)?;

        // Send out wake events
        for event in events.iter() {
            event!(Level::TRACE, "sending wake event");

            // Route event to correct destination
            let wake_handlers = registry.wake_handlers.borrow();
            let handler = wake_handlers
                .get(&event.token())
                .context("failed to get wake handler")?;

            handler.handle(
                world,
                WakeEvent {
                    read: event.is_readable(),
                    write: event.is_writable(),
                },
            );
        }

        // Process all pending actor messages, including wake events
        event!(Level::TRACE, "processing poll step messages");
        world.run_until_idle()?;
    }
}
