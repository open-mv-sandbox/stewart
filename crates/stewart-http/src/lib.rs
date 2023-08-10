mod connection;
mod listener;

use bytes::Bytes;
use stewart::message::Sender;

pub use self::listener::listen;

pub enum HttpEvent {
    Request(RequestEvent),
}

pub struct RequestEvent {
    /// Sender for actions specific to this request.
    ///
    /// Multiplexing behavior is up to specific connection types.
    /// For example, HTTP/1.1 can't be multiplexed, and will store your response if previous
    /// responses haven't been sent yet.
    pub actions: Sender<RequestAction>,
}

pub enum RequestAction {
    SendResponse(Bytes),
}
