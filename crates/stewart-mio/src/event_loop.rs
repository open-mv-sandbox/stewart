use anyhow::Error;
use mio::{event::Event, Events};
use stewart::World;
use tracing::{event, instrument, Level};

use crate::{registry::ReadyEvent, Registry};

#[instrument("mio-event-loop", skip_all)]
pub fn run_event_loop(world: &mut World, registry: &Registry) -> Result<(), Error> {
    // Process pending messages raised from initialization
    event!(Level::TRACE, "processing init messages");
    world.process()?;

    // Run the inner mio loop
    run_poll_loop(world, registry)?;

    Ok(())
}

fn run_poll_loop(world: &mut World, registry: &Registry) -> Result<(), Error> {
    let mut events = Events::with_capacity(256);

    loop {
        // Wait for pending events
        registry.poll(&mut events)?;

        // Send out ready events
        for event in events.iter() {
            handle(registry, event)?;
        }

        // Process all pending actor messages
        // This will likely start with the ready messages
        event!(Level::TRACE, "processing poll step messages");
        world.process()?;
    }
}

fn handle(registry: &Registry, event: &Event) -> Result<(), Error> {
    event!(Level::TRACE, "handling mio event");

    let ready = ReadyEvent {
        token: event.token(),
        readable: event.is_readable(),
        writable: event.is_writable(),
    };

    // Don't send events with nothing ready at all
    if !ready.readable && !ready.writable {
        return Ok(());
    }

    // Send out the message
    registry.send(event.token(), ready)?;

    Ok(())
}
