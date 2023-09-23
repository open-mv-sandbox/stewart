#![deny(unsafe_code)]

mod connection;
mod listener;
mod parser;

use bytes::Bytes;
use stewart::sender::Sender;

pub use self::listener::bind;

pub enum HttpEvent {
    Request(RequestEvent),
}

pub struct RequestEvent {
    pub header: HttpHeader,

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

#[derive(Default, Debug, Clone)]
pub struct HttpHeader {
    pub fields: Vec<HttpField>,
}

#[derive(Default, Debug, Clone)]
pub struct HttpField {
    pub key: Bytes,
    pub value: Bytes,
}
