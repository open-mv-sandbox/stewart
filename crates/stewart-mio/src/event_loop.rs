use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context as _, Error};
use mio::{event::Source, Events, Interest, Poll, Token};
use stewart::{Blackboard, Context, Handler, World};
use tracing::{event, instrument, Level};

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
    let thread_context = MioContext::new(poll);

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
            .get::<MioContext>()
            .context("failed to get context")?;
        tcx.poll.borrow_mut().poll(&mut events, None)?;

        // Send out wake events
        for event in events.iter() {
            event!(Level::TRACE, "sending wake event");

            // Route event to correct destination
            let wake_senders = tcx.wake_senders.borrow();
            let sender = wake_senders
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

pub struct MioContext {
    poll: RefCell<Poll>,
    next_token: AtomicUsize,
    wake_senders: RefCell<HashMap<Token, Handler<WakeEvent>>>,
}

impl MioContext {
    fn new(poll: Poll) -> Self {
        Self {
            poll: RefCell::new(poll),
            next_token: AtomicUsize::new(0),
            wake_senders: Default::default(),
        }
    }

    pub fn register<S>(
        &self,
        wake: Handler<WakeEvent>,
        source: &mut S,
        interest: Interest,
    ) -> Result<Token, Error>
    where
        S: Source,
    {
        // Get the next poll token
        let index = self.next_token.fetch_add(1, Ordering::SeqCst);
        let token = Token(index);

        // Store the waker callback
        self.wake_senders.borrow_mut().insert(token, wake);

        // Register with the generated token
        self.poll
            .borrow()
            .registry()
            .register(source, token, interest)?;

        Ok(token)
    }

    pub fn reregister<S>(
        &self,
        source: &mut S,
        token: Token,
        interest: Interest,
    ) -> Result<(), Error>
    where
        S: Source,
    {
        self.poll
            .borrow()
            .registry()
            .reregister(source, token, interest)?;

        Ok(())
    }
}

pub struct WakeEvent {
    pub read: bool,
    pub write: bool,
}
