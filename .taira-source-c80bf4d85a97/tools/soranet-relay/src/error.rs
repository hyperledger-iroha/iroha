//! Error helpers for the relay runtime.

use thiserror::Error;

use crate::config::ConfigError;

/// Common error wrapper used across the relay runtime.
#[derive(Debug, Error)]
pub enum RelayError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS configuration error: {0}")]
    Tls(String),
    #[error("QUIC error: {0}")]
    Quic(String),
    #[error("logging configuration error: {0}")]
    Logging(String),
    #[error("cryptography error: {0}")]
    Crypto(String),
}
