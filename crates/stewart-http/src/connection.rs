use std::collections::VecDeque;

use anyhow::Error;
use bytes::{BufMut, Bytes, BytesMut};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::net::tcp;
use tracing::{event, Level};

use crate::{
    parser::{HttpParser, ParserEvent},
    HttpEvent, RequestAction, RequestEvent,
};

pub enum ConnectionEvent {
    Closed,
}

pub enum ConnectionAction {
    Close,
}

/// Open a TCP based HTTP connection.
pub fn open(
    world: &mut World,
    tcp_events: Mailbox<tcp::ConnectionEvent>,
    tcp_actions: Sender<tcp::ConnectionAction>,
    events: Sender<ConnectionEvent>,
    http_events: Sender<HttpEvent>,
) -> Result<Sender<ConnectionAction>, Error> {
    let actor = Service::new(tcp_events, tcp_actions, events, http_events);
    let actions = actor.actions.sender();
    world.insert("http-connection", actor)?;

    Ok(actions)
}

struct Service {
    actions: Mailbox<ConnectionAction>,
    events: Sender<ConnectionEvent>,
    tcp_events: Mailbox<tcp::ConnectionEvent>,
    tcp_actions: Sender<tcp::ConnectionAction>,
    http_events: Sender<HttpEvent>,

    tcp_closed: bool,
    parser: HttpParser,
    requests: VecDeque<RequestState>,
}

enum RequestState {
    New,
    Pending { actions: Mailbox<RequestAction> },
}

impl Service {
    pub fn new(
        tcp_events: Mailbox<tcp::ConnectionEvent>,
        tcp_actions: Sender<tcp::ConnectionAction>,
        events: Sender<ConnectionEvent>,
        http_events: Sender<HttpEvent>,
    ) -> Self {
        event!(Level::DEBUG, "connection opened");

        Self {
            actions: Mailbox::default(),
            events,
            tcp_events,
            tcp_actions,
            http_events,

            tcp_closed: false,
            parser: HttpParser::default(),
            requests: VecDeque::new(),
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.events.send(ConnectionEvent::Closed);
        if !self.tcp_closed {
            let _ = self.tcp_actions.send(tcp::ConnectionAction::Close);
        }
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        let signal = world.signal(meta.id());

        self.tcp_events.set_signal(signal.clone());
        self.actions.set_signal(signal);

        Ok(())
    }

    fn process(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        self.process_tcp(meta)?;

        // Can't do anything further if we don't have an open TCP connection
        if self.tcp_closed {
            return Ok(());
        }

        self.process_actions()?;
        self.process_requests(world, meta)?;

        Ok(())
    }
}

impl Service {
    fn process_tcp(&mut self, meta: &mut Metadata) -> Result<(), Error> {
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::ConnectionEvent::Recv(event) => {
                    self.handle_recv(event);
                }
                tcp::ConnectionEvent::Closed => {
                    event!(Level::DEBUG, "connection closed");

                    self.tcp_closed = true;
                    meta.set_stop();
                }
            }
        }

        Ok(())
    }

    fn handle_recv(&mut self, mut event: tcp::RecvEvent) {
        event!(Level::TRACE, bytes = event.data.len(), "received data");

        // Consume data into the parser
        while !event.data.is_empty() {
            let event = self.parser.consume(&mut event.data);

            if let Some(event) = event {
                match event {
                    ParserEvent::Header(header) => {
                        // Temporary debug print
                        println!("H: {:?}", header);

                        // Track the request
                        self.requests.push_back(RequestState::New);
                    }
                }
            }
        }
    }

    fn process_actions(&mut self) -> Result<(), Error> {
        while let Some(action) = self.actions.recv() {
            match action {
                ConnectionAction::Close => {
                    self.tcp_actions.send(tcp::ConnectionAction::Close)?;
                }
            }
        }

        Ok(())
    }

    fn process_requests(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        // Check new requests we have to send out
        for request in &mut self.requests {
            let RequestState::New = request else { continue };

            // Create the mailbox to send a response back through
            let actions = Mailbox::default();
            let signal = world.signal(meta.id());
            actions.set_signal(signal);

            // Send the request event
            let event = RequestEvent {
                actions: actions.sender(),
            };
            self.http_events.send(HttpEvent::Request(event))?;

            // Continue tracking the request
            *request = RequestState::Pending { actions };
        }

        // Check requests we can resolve
        // HTTP 1.1 sends back responses in the same order as requests, so we only check the first
        while let Some(RequestState::Pending { actions }) = self.requests.front() {
            // Check if we got a response to send
            let Some(action) = actions.recv() else { break };
            let RequestAction::SendResponse(body) = action;

            self.send_response(body)?;

            // Remove this resolved request
            self.requests.pop_front();
        }

        Ok(())
    }

    fn send_response(&mut self, body: Bytes) -> Result<(), Error> {
        // Send the response
        let mut data = BytesMut::new();

        data.put(&b"HTTP/1.1 200 OK\r\n"[..]);
        data.put(&b"Content-Type: text/html\r\nContent-Length: "[..]);
        let length = body.len().to_string();
        data.put(length.as_bytes());
        data.put(&b"\r\n\r\n"[..]);
        data.put(body);

        let action = tcp::SendAction {
            data: data.freeze(),
        };
        self.tcp_actions.send(tcp::ConnectionAction::Send(action))?;

        Ok(())
    }
}
