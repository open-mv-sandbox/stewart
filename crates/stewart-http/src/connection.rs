use std::collections::VecDeque;
use std::ops::ControlFlow;

use anyhow::Error;
use bytes::{BufMut, Bytes, BytesMut};
use stewart::{
    sender::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use stewart_mio::net::tcp;
use tracing::{event, Level};

use crate::{
    parser::{HttpParser, ParserEvent},
    HttpEvent, HttpHeader, RequestAction, RequestEvent,
};

pub enum ConnectionEvent {
    Closed,
}

pub enum ConnectionAction {
    Close,
}

/// Open a TCP based HTTP connection.
pub fn open(
    world: &mut Runtime,
    tcp_events: Mailbox<tcp::StreamEvent>,
    tcp_actions: Sender<tcp::StreamAction>,
    events: Sender<ConnectionEvent>,
    http_events: Sender<HttpEvent>,
) -> Result<Sender<ConnectionAction>, Error> {
    let signal = Signal::default();
    tcp_events.set_signal(signal.clone());

    let actor = Service::new(signal.clone(), tcp_events, tcp_actions, events, http_events);
    let actions = actor.actions.sender();

    let id = world.insert("http-connection", actor);
    signal.set_id(id);

    Ok(actions)
}

struct Service {
    signal: Signal,
    actions: Mailbox<ConnectionAction>,
    events: Sender<ConnectionEvent>,
    tcp_events: Mailbox<tcp::StreamEvent>,
    tcp_actions: Sender<tcp::StreamAction>,
    http_events: Sender<HttpEvent>,

    closed: bool,
    parser: HttpParser,
    requests: VecDeque<RequestState>,
}

enum RequestState {
    New { header: HttpHeader },
    Pending { actions: Mailbox<RequestAction> },
}

impl Service {
    pub fn new(
        signal: Signal,
        tcp_events: Mailbox<tcp::StreamEvent>,
        tcp_actions: Sender<tcp::StreamAction>,
        events: Sender<ConnectionEvent>,
        http_events: Sender<HttpEvent>,
    ) -> Self {
        event!(Level::DEBUG, "connection opened");

        let actions = Mailbox::new(signal.clone());

        Self {
            signal,
            actions,
            events,
            tcp_events,
            tcp_actions,
            http_events,

            closed: false,
            parser: HttpParser::default(),
            requests: VecDeque::new(),
        }
    }
}

impl Actor for Service {
    fn handle(&mut self, world: &mut Runtime) -> ControlFlow<()> {
        self.process_tcp().unwrap();

        // Can't do anything further if we don't have an open TCP connection
        if self.closed {
            event!(Level::DEBUG, "stopping");

            let _ = self.events.send(world, ConnectionEvent::Closed);
            let _ = self.tcp_actions.send(world, tcp::StreamAction::Close);

            return ControlFlow::Break(());
        }

        self.process_actions(world).unwrap();
        self.process_requests(world).unwrap();

        ControlFlow::Continue(())
    }
}

impl Service {
    fn process_tcp(&mut self) -> Result<(), Error> {
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::StreamEvent::Recv(event) => {
                    self.handle_recv(event);
                }
                tcp::StreamEvent::Closed => {
                    event!(Level::DEBUG, "connection closed");
                    self.closed = true;
                }
            }
        }

        Ok(())
    }

    fn handle_recv(&mut self, mut event: tcp::RecvEvent) {
        println!("RECEIVING {:?}", event.data);
        event!(Level::TRACE, bytes = event.data.len(), "received data");

        // Consume data into the parser
        while !event.data.is_empty() {
            let event = self.parser.consume(&mut event.data);

            if let Some(event) = event {
                match event {
                    ParserEvent::Header(header) => {
                        // Track the request
                        let state = RequestState::New { header };
                        self.requests.push_back(state);
                    }
                }
            }
        }
    }

    fn process_actions(&mut self, world: &mut Runtime) -> Result<(), Error> {
        while let Some(action) = self.actions.recv() {
            match action {
                ConnectionAction::Close => {
                    self.tcp_actions.send(world, tcp::StreamAction::Close)?;
                }
            }
        }

        Ok(())
    }

    fn process_requests(&mut self, world: &mut Runtime) -> Result<(), Error> {
        // Check new requests we have to send out
        for request in &mut self.requests {
            let RequestState::New { header } = request else {
                continue;
            };

            // Create the mailbox to send a response back through
            let actions = Mailbox::new(self.signal.clone());

            // Send the request event
            let event = RequestEvent {
                header: header.clone(),
                actions: actions.sender(),
            };
            self.http_events.send(world, HttpEvent::Request(event))?;

            // Continue tracking the request
            *request = RequestState::Pending { actions };
        }

        // Check requests we can resolve
        // HTTP 1.1 sends back responses in the same order as requests, so we only check the first
        while let Some(RequestState::Pending { actions }) = self.requests.front() {
            // Check if we got a response to send
            let Some(action) = actions.recv() else {
                break;
            };
            let RequestAction::SendResponse(body) = action;

            self.send_response(world, body)?;

            // Remove this resolved request
            self.requests.pop_front();
        }

        Ok(())
    }

    fn send_response(&mut self, world: &mut Runtime, body: Bytes) -> Result<(), Error> {
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
        self.tcp_actions
            .send(world, tcp::StreamAction::Send(action))?;

        Ok(())
    }
}
