use std::{fs, sync::Arc};

use anyhow::{Context, Error};
use quinn_proto::{Endpoint, EndpointConfig, ServerConfig};
use tracing::{event, Level};

pub const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

pub fn endpoint() -> Result<(), Error> {
    // Load key
    let (cert, key) = generate_cert()?;
    let cert = rustls::Certificate(cert);
    let key = rustls::PrivateKey(key);

    // Create crypto backend
    let certs = vec![cert];
    let crypto = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    // Configure and create protocol endpoint
    let config = EndpointConfig::default();
    let server_config = ServerConfig::with_crypto(Arc::new(crypto));
    let _endpoint = Endpoint::new(Arc::new(config), Some(Arc::new(server_config)), false);

    Ok(())
}

fn generate_cert() -> Result<(Vec<u8>, Vec<u8>), Error> {
    // TODO: Hacky temporary function

    event!(Level::WARN, "generating self-signed certificate");

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = cert.serialize_private_key_der();
    let cert = cert.serialize_der().unwrap();

    // Dump key where the client can find it
    fs::write("./target/example-cert", &cert).context("failed to dump cert")?;
    fs::write("./target/example-key", &key).context("failed to dump key")?;

    Ok((cert, key))
}
