use std::sync::Arc;

use anyhow::Error;
use quinn_proto::{Endpoint, EndpointConfig, ServerConfig};
use rustls::{Certificate, PrivateKey};
use stewart::{Actor, Context, World};

pub fn endpoint(
    world: &mut World,
    certificate: Certificate,
    private_key: PrivateKey,
) -> Result<(), Error> {
    let actor = Service::new(certificate, private_key)?;
    world.insert("quic-endpoint", actor)?;

    Ok(())
}

struct Service {
    endpoint: Endpoint,
}

impl Service {
    fn new(certificate: Certificate, private_key: PrivateKey) -> Result<Self, Error> {
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

        let value = Service { endpoint };
        Ok(value)
    }
}

impl Actor for Service {
    fn process(&mut self, _ctx: &mut Context) -> Result<(), Error> {
        Ok(())
    }
}
