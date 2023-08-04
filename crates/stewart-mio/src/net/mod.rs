pub mod tcp;
pub mod udp;

use std::io::ErrorKind;

use anyhow::Error;

fn check_io<T>(value: Result<T, std::io::Error>) -> Result<Option<T>, Error> {
    match value {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            // WouldBlock just means we've run out of things to handle
            if error.kind() == ErrorKind::WouldBlock {
                Ok(None)
            } else {
                Err(error.into())
            }
        }
    }
}
