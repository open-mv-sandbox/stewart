use anyhow::Error;
use bytes::{BufMut, Bytes, BytesMut};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::net::tcp;
use tracing::{event, Level};

pub enum StreamEvent {
    Request(RequestEvent),
    Closed,
}

pub struct RequestEvent {}

pub enum StreamAction {
    Response(ResponseAction),
    Close,
}

pub struct ResponseAction {
    pub body: Bytes,
}

/// Open a TCP connection based "HTTP message stream".
pub fn open(
    world: &mut World,
    tcp_events: Mailbox<tcp::StreamEvent>,
    tcp_actions: Sender<tcp::StreamAction>,
    events: Sender<StreamEvent>,
) -> Result<Sender<StreamAction>, Error> {
    let actor = Service::new(tcp_events, tcp_actions, events);
    let actions = actor.actions.sender();
    world.insert("http-connection", actor)?;

    Ok(actions)
}

struct Service {
    tcp_events: Mailbox<tcp::StreamEvent>,
    tcp_actions: Sender<tcp::StreamAction>,
    actions: Mailbox<StreamAction>,
    events: Sender<StreamEvent>,

    buffer: String,
    tcp_closed: bool,
}

impl Service {
    pub fn new(
        tcp_events: Mailbox<tcp::StreamEvent>,
        tcp_actions: Sender<tcp::StreamAction>,
        events: Sender<StreamEvent>,
    ) -> Self {
        event!(Level::DEBUG, "connection opened");

        Self {
            tcp_events,
            tcp_actions,
            actions: Mailbox::default(),
            events,

            buffer: String::new(),
            tcp_closed: false,
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.events.send(StreamEvent::Closed);
        if !self.tcp_closed {
            let _ = self.tcp_actions.send(tcp::StreamAction::Close);
        }
    }
}

impl Actor for Service {
    fn register(&mut self, _world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        self.tcp_events.set_signal(meta.signal());
        self.actions.set_signal(meta.signal());
        Ok(())
    }

    fn process(&mut self, _world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        // Handle incoming data from the TCP stream
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::StreamEvent::Recv(event) => {
                    event!(Level::TRACE, bytes = event.data.len(), "received data");

                    let data = std::str::from_utf8(&event.data)?;
                    self.buffer.push_str(data);
                }
                tcp::StreamEvent::Closed => {
                    event!(Level::DEBUG, "connection closed");
                    self.tcp_closed = true;
                    meta.set_stop();
                }
            }
        }

        // Handle actions
        while let Some(action) = self.actions.recv() {
            match action {
                StreamAction::Response(response) => {
                    self.handle_action_response(response.body)?;
                }
                StreamAction::Close => {
                    meta.set_stop();
                }
            }
        }

        if !self.tcp_closed {
            // Check if we have a full request worth of data
            // TODO: This is very incorrect and really should be be redesigned entirely
            let split = self.buffer.split_once("\r\n\r\n");
            if let Some((_left, right)) = split {
                self.buffer = right.to_string();
                self.handle_request()?;
            }
        }

        Ok(())
    }
}

impl Service {
    fn handle_request(&mut self) -> Result<(), Error> {
        event!(Level::DEBUG, "received request");

        let event = RequestEvent {};
        self.events.send(StreamEvent::Request(event))?;

        Ok(())
    }

    fn handle_action_response(&mut self, response: Bytes) -> Result<(), Error> {
        if self.tcp_closed {
            return Ok(());
        }

        // Send the response
        let mut data = BytesMut::new();

        data.put(&b"HTTP/1.1 200 OK\r\n"[..]);
        data.put(&b"Content-Type: text/html\r\nContent-Length: "[..]);
        let length = response.len().to_string();
        data.put(length.as_bytes());
        data.put(&b"\r\n\r\n"[..]);
        data.put(response);

        let action = tcp::SendAction {
            data: data.freeze(),
        };
        self.tcp_actions.send(tcp::StreamAction::Send(action))?;

        Ok(())
    }
}
