mod event_loop;
pub mod net;

use std::{cell::RefCell, collections::HashMap};

use anyhow::{Context, Error};
use mio::{Poll, Token};
use stewart::utils::Sender;

pub use self::event_loop::run_event_loop;

// TODO: Should there be a World or Context registry for global context values?
thread_local! {
    static THREAD_CONTEXT: RefCell<Option<ThreadContext>> = RefCell::new(None);
}

struct ThreadContext {
    poll: Poll,
    next_token: usize,
    wake_senders: HashMap<Token, Sender<WakeEvent>>,
}

fn with_thread_context<F, O>(f: F) -> Result<O, Error>
where
    F: FnOnce(&mut ThreadContext) -> Result<O, Error>,
{
    THREAD_CONTEXT.with::<_, Result<O, Error>>(|tcx| {
        let mut tcx = tcx.borrow_mut();
        let tcx = tcx.as_mut().context("failed to get thread context")?;

        f(tcx)
    })
}

struct WakeEvent {
    read: bool,
    write: bool,
}
