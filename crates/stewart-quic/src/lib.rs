use std::{net::SocketAddr, sync::Arc};

use anyhow::Error;
use bytes::BytesMut;
use quinn_proto::{DatagramEvent, Endpoint, EndpointConfig, ServerConfig};
use rustls::{Certificate, PrivateKey};
use stewart::{
    message::{Mailbox, Sender},
    Actor, Id, World,
};
use stewart_mio::{net::udp, RegistryHandle};
use tracing::{event, Level};

pub fn endpoint(
    world: &mut World,
    registry: RegistryHandle,
    addr: SocketAddr,
    certificate: Certificate,
    private_key: PrivateKey,
) -> Result<(), Error> {
    let actor = Service::new(world, registry, addr, certificate, private_key)?;
    world.insert("quic-endpoint", actor)?;

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
        world: &mut World,
        registry: RegistryHandle,
        addr: SocketAddr,
        certificate: Certificate,
        private_key: PrivateKey,
    ) -> Result<Self, Error> {
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
        let event_mailbox = Mailbox::default();
        let (action_sender, _) = udp::bind(world, registry, addr, event_mailbox.sender())?;

        let value = Service {
            endpoint,
            event_mailbox,
            action_sender,
        };
        Ok(value)
    }
}

impl Actor for Service {
    fn register(&mut self, world: &mut World, id: Id) -> Result<(), Error> {
        self.event_mailbox.set_signal(world.signal(id));
        Ok(())
    }

    fn process(&mut self, _world: &mut World, _id: Id) -> Result<(), Error> {
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

        Ok(())
    }
}
