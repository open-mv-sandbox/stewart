use std::{net::SocketAddr, rc::Rc, sync::Arc};

use anyhow::Error;
use quinn_proto::{Endpoint, EndpointConfig, ServerConfig};
use rustls::{Certificate, PrivateKey};
use stewart::{Actor, Context, World};
use stewart_mio::{net::udp::SocketInfo, Registry};
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
    socket: SocketInfo,
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
        let socket = stewart_mio::net::udp::bind(world, registry, addr)?;

        let value = Service { endpoint, socket };
        Ok(value)
    }
}

impl Actor for Service {
    fn register(&mut self, ctx: &mut Context) -> Result<(), Error> {
        self.socket.recv().set_signal(ctx.signal());
        Ok(())
    }

    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        while let Some(packet) = self.socket.recv().recv() {
            event!(Level::DEBUG, "received packet");
            println!("receiving etc");
        }

        // TODO: Poll transmit

        Ok(())
    }
}
