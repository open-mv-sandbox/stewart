use anyhow::Error;
use mio::{event::Event, Events};
use stewart::Runtime;
use tracing::{event, instrument, Level};

use crate::{registry::ReadyState, Registry};

#[instrument("mio-event-loop", skip_all)]
pub fn run_event_loop(world: &mut Runtime, registry: &Registry) -> Result<(), Error> {
    // Process pending messages raised from initialization
    event!(Level::TRACE, "processing init messages");
    world.process()?;

    // Run the inner mio loop
    run_poll_loop(world, registry)?;

    Ok(())
}

fn run_poll_loop(world: &mut Runtime, registry: &Registry) -> Result<(), Error> {
    let mut events = Events::with_capacity(256);

    loop {
        // Wait for pending events
        registry.poll(&mut events)?;

        // Send out ready events
        for event in events.iter() {
            handle(world, registry, event)?;
        }

        // Process all pending actor messages
        // This will likely start with the ready messages
        //event!(Level::TRACE, "processing poll step messages"); almost never useful, very spammy
        world.process()?;
    }
}

fn handle(world: &mut Runtime, registry: &Registry, event: &Event) -> Result<(), Error> {
    event!(Level::TRACE, "handling mio event");

    let ready = ReadyState {
        readable: event.is_readable(),
        writable: event.is_writable(),
    };

    // Don't send events with nothing ready at all
    if !ready.readable && !ready.writable {
        return Ok(());
    }

    // Send out the message
    registry.update_state(world, event.token(), ready)?;

    Ok(())
}
