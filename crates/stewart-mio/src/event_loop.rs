use std::{cell::RefCell, collections::HashMap};

use anyhow::{Context as _, Error};
use mio::{Events, Poll};
use stewart::{Blackboard, Context, World};
use tracing::{event, instrument, Level};

use crate::{ThreadContext, ThreadContextEntry, WakeEvent};

// TODO: This entirely needs cleanup, it's way too messy

#[instrument("mio-event-loop", skip_all)]
pub fn run_event_loop<I>(init: I) -> Result<(), Error>
where
    I: FnOnce(&mut World, &Context) -> Result<(), Error>,
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
    let thread_context = RefCell::new(thread_context);

    let mut blackboard = Blackboard::default();
    blackboard.set(thread_context);

    // User init
    let cx = Context::new(blackboard);
    init(&mut world, &cx)?;

    // Process pending messages raised from initialization
    event!(Level::TRACE, "processing init messages");
    world.run_until_idle(&cx)?;

    // Run the inner mio loop
    let result = run_poll_loop(&mut world, &cx);
    if let Err(error) = result {
        // TODO: Shut down or restart the system?
        event!(Level::ERROR, "error in event pipeline: {}", error);
    }

    Ok(())
}

fn run_poll_loop(world: &mut World, cx: &Context) -> Result<(), Error> {
    let mut events = Events::with_capacity(128);
    loop {
        let tcx = cx
            .blackboard()
            .get::<ThreadContextEntry>()
            .context("failed to get context")?;
        let mut tcx = tcx.borrow_mut();
        tcx.poll.poll(&mut events, None)?;

        // Send out wake events
        for event in events.iter() {
            event!(Level::TRACE, "sending wake event");

            // Route event to correct destination
            let sender = tcx
                .wake_senders
                .get(&event.token())
                .context("failed to get wake sender")?;

            sender.handle(
                world,
                WakeEvent {
                    read: event.is_readable(),
                    write: event.is_writable(),
                },
            );
        }

        // Make sure tcx is no longer borrowed
        drop(tcx);

        // Process all pending actor messages, including wake events
        event!(Level::TRACE, "processing poll step messages");
        world.run_until_idle(&cx)?;
    }
}
