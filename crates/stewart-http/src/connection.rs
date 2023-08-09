use anyhow::Error;
use bytes::{BufMut, Bytes, BytesMut};
use stewart::{Actor, Meta, World};
use stewart_mio::net::tcp;
use tracing::{event, Level};

pub fn open(world: &mut World, event: tcp::ConnectedEvent, body: Bytes) -> Result<(), Error> {
    let actor = Service::new(event, body);
    world.insert("http-connection", actor)?;

    Ok(())
}

struct Service {
    event: tcp::ConnectedEvent,
    pending: String,
    closed: bool,
    body: Bytes,
}

impl Service {
    pub fn new(event: tcp::ConnectedEvent, body: Bytes) -> Self {
        event!(Level::DEBUG, "connection opened");

        Self {
            event,
            pending: String::new(),
            closed: false,
            body,
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        event!(Level::DEBUG, "closing");

        let _ = self.event.actions_sender.send(tcp::StreamAction::Close);
    }
}

impl Actor for Service {
    fn register(&mut self, _world: &mut World, meta: &mut Meta) -> Result<(), Error> {
        self.event.event_mailbox.set_signal(meta.signal());
        Ok(())
    }

    fn process(&mut self, _world: &mut World, _meta: &mut Meta) -> Result<(), Error> {
        while let Some(event) = self.event.event_mailbox.recv() {
            match event {
                tcp::StreamEvent::Recv(event) => {
                    event!(Level::TRACE, bytes = event.data.len(), "received data");

                    let data = std::str::from_utf8(&event.data)?;
                    self.pending.push_str(data);
                }
                tcp::StreamEvent::Closed => {
                    event!(Level::DEBUG, "connection closed");
                    self.closed = true;
                }
            }
        }

        if !self.closed {
            // Check if we have a full request worth of data
            let split = self.pending.split_once("\r\n\r\n");
            if let Some((_left, right)) = split {
                event!(Level::DEBUG, "responding to request");
                self.pending = right.to_string();

                // Send the response
                let body = self.body.clone();
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
                self.event
                    .actions_sender
                    .send(tcp::StreamAction::Send(action))?;
            }
        }

        Ok(())
    }
}
