use std::collections::HashMap;

use anyhow::{Context as _, Error};
use mio::{Events, Poll};
use stewart::{Context, World};
use tracing::{event, instrument, Level};

use crate::{with_thread_context, ThreadContext, WakeEvent, THREAD_CONTEXT};

// TODO: This entirely needs cleanup, it's way too messy

#[instrument("mio-event-loop", skip_all)]
pub fn run_event_loop<I>(init: I) -> Result<(), Error>
where
    I: FnOnce(&mut Context) -> Result<(), Error>,
{
    // Set up the local world
    let mut world = World::default();

    // Initialize mio context
    let poll = Poll::new()?;
    let thread_context = ThreadContext {
        poll,
        next_token: 0,
        wake_senders: HashMap::new(),
    };
    THREAD_CONTEXT.with(|tcx| *tcx.borrow_mut() = Some(thread_context));

    // User init
    let mut cx = Context::root(&mut world);
    init(&mut cx)?;

    // Process pending messages raised from initialization
    event!(Level::TRACE, "processing init messages");
    world.run_until_idle()?;

    // Run the inner mio loop
    let result = run_poll_loop(&mut world);
    if let Err(error) = result {
        // TODO: Shut down or restart the system?
        event!(Level::ERROR, "error in event pipeline: {}", error);
    }

    // TODO: Cleanup doesn't always run for common normal errors, such as user init, fix that
    THREAD_CONTEXT.with(|tcx| *tcx.borrow_mut() = None);

    Ok(())
}

fn run_poll_loop(world: &mut World) -> Result<(), Error> {
    let mut events = Events::with_capacity(128);
    loop {
        with_thread_context(|tcx| {
            tcx.poll.poll(&mut events, None)?;

            // Send out wake events
            let mut cx = Context::root(world);
            for event in events.iter() {
                event!(Level::TRACE, "sending wake event");

                // Route event to correct destination
                let sender = tcx
                    .wake_senders
                    .get(&event.token())
                    .context("failed to get wake sender")?;

                sender.send(
                    &mut cx,
                    WakeEvent {
                        read: event.is_readable(),
                        write: event.is_writable(),
                    },
                );
            }

            Ok(())
        })?;

        // Process all pending actor messages, including wake events
        event!(Level::TRACE, "processing poll step messages");
        world.run_until_idle()?;
    }
}
