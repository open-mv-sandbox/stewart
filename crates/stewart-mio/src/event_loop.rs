use std::rc::Rc;

use anyhow::Error;
use mio::{Events, Poll};
use stewart::World;
use tracing::{event, instrument, Level};

use crate::Registry;

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
    run_poll_loop(&mut world, &registry)?;

    Ok(())
}

fn run_poll_loop(world: &mut World, registry: &Rc<Registry>) -> Result<(), Error> {
    let mut events = Events::with_capacity(256);

    loop {
        // Wait for pending events
        registry.poll(&mut events)?;

        // Send out ready events
        for event in events.iter() {
            registry.send_ready(
                world,
                event.token(),
                event.is_readable(),
                event.is_writable(),
            )?;
        }

        // Process all pending actor messages
        // This will likely start with the ready messages
        event!(Level::TRACE, "processing poll step messages");
        world.run_until_idle()?;
    }
}
