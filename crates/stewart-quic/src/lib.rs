#![deny(unsafe_code)]

use std::ops::ControlFlow;
use std::{net::SocketAddr, sync::Arc};

use anyhow::Error;
use bytes::BytesMut;
use quinn_proto::{DatagramEvent, Endpoint, EndpointConfig, ServerConfig};
use rustls::{Certificate, PrivateKey};
use stewart::{
    sender::{Mailbox, Sender, Signal},
    Actor, Runtime,
};
use stewart_mio::{net::udp, RegistryRef};
use tracing::{event, Level};

pub fn endpoint(
    world: &mut Runtime,
    registry: RegistryRef,
    addr: SocketAddr,
    certificate: Certificate,
    private_key: PrivateKey,
) -> Result<(), Error> {
    let (actor, signal) = Service::new(world, registry, addr, certificate, private_key)?;

    let id = world.insert("quic-endpoint", actor);
    signal.set_id(id);

    Ok(())
}

struct Service {
    endpoint: Endpoint,
    event_mailbox: Mailbox<udp::RecvEvent>,
    #[allow(dead_code)]
    action_sender: Sender<udp::Action>,
}

impl Service {
    fn new(
        world: &mut Runtime,
        registry: RegistryRef,
        addr: SocketAddr,
        certificate: Certificate,
        private_key: PrivateKey,
    ) -> Result<(Self, Signal), Error> {
        // TODO: This is currently always a server, make sure it can be a client
        event!(Level::DEBUG, ?addr, "starting endpoint");

        // Create crypto backend
        let certs = vec![certificate];
        let crypto = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)?;

        // Configure and create protocol endpoint
        let config = EndpointConfig::default();
        let server_config = ServerConfig::with_crypto(Arc::new(crypto));
        let endpoint = Endpoint::new(Arc::new(config), Some(Arc::new(server_config)), false);

        // Bind the UDP socket to listen on
        let signal = Signal::default();
        let event_mailbox = Mailbox::new(signal.clone());
        let (action_sender, _) = udp::bind(world, registry, addr, event_mailbox.sender())?;

        let this = Service {
            endpoint,
            event_mailbox,
            action_sender,
        };
        Ok((this, signal))
    }
}

impl Actor for Service {
    fn handle(&mut self, _world: &mut Runtime) -> ControlFlow<()> {
        while let Some(packet) = self.event_mailbox.recv() {
            event!(Level::TRACE, "received packet");

            // TODO: Make this part of the socket API
            let mut data = BytesMut::with_capacity(packet.data.len());
            data.extend_from_slice(&packet.data);

            // Pass to the QUIC protocol implementation
            let result = self
                .endpoint
                .handle(packet.arrived, packet.remote, None, None, data);

            if let Some((_connection_handle, event)) = result {
                match event {
                    DatagramEvent::ConnectionEvent(event) => {
                        event!(Level::DEBUG, ?event, "connection event");
                    }
                    DatagramEvent::NewConnection(connection) => {
                        event!(Level::DEBUG, ?connection, "new connection");
                    }
                }
            }
        }

        // TODO: Poll transmit

        ControlFlow::Continue(())
    }
}
