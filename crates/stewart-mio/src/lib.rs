mod event_loop;
pub mod net;

pub use self::event_loop::run_event_loop;

struct WakeEvent {
    read: bool,
    write: bool,
}
