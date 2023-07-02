mod event_loop;
pub mod net;

use std::{cell::RefCell, collections::HashMap};

use mio::{Poll, Token};
use stewart::Handler;

pub use self::event_loop::run_event_loop;

struct ThreadContext {
    poll: Poll,
    next_token: usize,
    wake_senders: HashMap<Token, Handler<WakeEvent>>,
}

struct WakeEvent {
    read: bool,
    write: bool,
}

type ThreadContextEntry = RefCell<ThreadContext>;
