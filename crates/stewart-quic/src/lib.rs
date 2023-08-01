use std::{net::SocketAddr, rc::Rc, sync::Arc};

use anyhow::Error;
use bytes::BytesMut;
use quinn_proto::{DatagramEvent, Endpoint, EndpointConfig, ServerConfig};
use rustls::{Certificate, PrivateKey};
use stewart::{Actor, Context, World};
use stewart_mio::{net::udp, Registry};
use tracing::{event, Level};

pub fn endpoint(
    world: &mut World,
    registry: Rc<Registry>,
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
    socket: udp::Socket,
}

impl Service {
    fn new(
        world: &mut World,
        registry: Rc<Registry>,
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
        let socket = udp::bind(world, registry, addr)?;

        let value = Service { endpoint, socket };
        Ok(value)
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.socket.events().set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        while let Some(packet) = self.socket.events().recv() {
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
