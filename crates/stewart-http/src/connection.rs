use std::collections::VecDeque;

use anyhow::Error;
use bytes::{BufMut, Bytes, BytesMut};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Metadata, World,
};
use stewart_mio::net::tcp;
use tracing::{event, Level};

use crate::{HttpEvent, RequestAction, RequestEvent};

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
    tcp_events: Mailbox<tcp::ConnectionEvent>,
    tcp_actions: Sender<tcp::ConnectionAction>,
    actions: Mailbox<ConnectionAction>,
    events: Sender<ConnectionEvent>,
    http_events: Sender<HttpEvent>,

    tcp_closed: bool,
    receive_buffer: BytesMut,
    pending_requests: VecDeque<Mailbox<RequestAction>>,
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
            tcp_events,
            tcp_actions,
            actions: Mailbox::default(),
            events,
            http_events,

            tcp_closed: false,
            receive_buffer: BytesMut::new(),
            pending_requests: VecDeque::new(),
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
        self.process_received(world, meta)?;

        // Check requests we can resolve
        // HTTP 1.1 sends back responses in the same order as requests, so we only check the first
        while let Some(request) = self.pending_requests.front() {
            // Check if we got a response to send
            let Some(action) = request.recv() else { break };
            let RequestAction::SendResponse(body) = action;

            self.send_response(body)?;

            // Remove this resolved request
            self.pending_requests.pop_front();
        }

        Ok(())
    }
}

impl Service {
    fn process_tcp(&mut self, meta: &mut Metadata) -> Result<(), Error> {
        while let Some(event) = self.tcp_events.recv() {
            match event {
                tcp::ConnectionEvent::Recv(event) => {
                    event!(Level::TRACE, bytes = event.data.len(), "received data");
                    self.receive_buffer.extend(&event.data);
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

    /// Process pending data previously received, but not yet processed.
    fn process_received(&mut self, world: &mut World, meta: &mut Metadata) -> Result<(), Error> {
        // TODO: Instead of checking every time if we have a full header's worth, do something
        // smarter.
        // TODO: Handle requests with body content

        loop {
            // Check if we have a full request worth of data left in the buffer
            let location = self
                .receive_buffer
                .windows(4)
                .enumerate()
                .find(|(_, window)| window == b"\r\n\r\n")
                .map(|(i, _)| i);
            let Some(location) = location else { break; };

            event!(Level::DEBUG, "received request");

            // Split off the request we have to process
            let request = self.receive_buffer.split_to(location + 4);
            let data = std::str::from_utf8(&request)?;
            println!("REQUEST: {:?}", data);

            // Create the mailbox to send a response back through
            let mailbox = Mailbox::default();
            let signal = world.signal(meta.id());
            mailbox.set_signal(signal);

            // Send the request event
            let event = RequestEvent {
                actions: mailbox.sender(),
            };
            self.http_events.send(HttpEvent::Request(event))?;

            // Track the request
            self.pending_requests.push_back(mailbox);
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
