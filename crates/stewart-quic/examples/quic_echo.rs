use std::fs;

use anyhow::{Context, Error};
use rustls::{Certificate, PrivateKey};
use stewart::Runtime;
use stewart_mio::Registry;
use tracing::{event, Level};

fn main() -> Result<(), Error> {
    devutils::init_logging();

    let mut world = Runtime::default();
    let registry = Registry::new()?;

    // Set up the QUIC server
    let (certificate, private_key) = generate_certificate()?;
    let addr = "0.0.0.0:1234".parse()?;
    stewart_quic::endpoint(
        &mut world,
        registry.handle(),
        addr,
        certificate,
        private_key,
    )?;

    // Run the event loop
    stewart_mio::run_event_loop(&mut world, &registry)?;

    Ok(())
}

fn generate_certificate() -> Result<(Certificate, PrivateKey), Error> {
    event!(Level::WARN, "generating self-signed certificate");

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = cert.serialize_private_key_der();
    let cert = cert.serialize_der().unwrap();

    // Dump key where the client can find it
    fs::write("../../../target/example-cert", &cert).context("failed to dump cert")?;
    fs::write("../../../target/example-key", &key).context("failed to dump key")?;

    let certificate = rustls::Certificate(cert);
    let private_key = rustls::PrivateKey(key);
    Ok((certificate, private_key))
}
