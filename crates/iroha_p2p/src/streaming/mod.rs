//! Norito Streaming transport helpers.
//!
//! This module exposes the QUIC transport primitives used by the Norito
//! Streaming Codec (NSC): a dedicated control stream for `ControlFrame`
//! exchange and unreliable DATAGRAM delivery for encrypted media chunks.
//! Implementations can use these helpers to negotiate capabilities,
//! announce manifests, and move ciphertext payloads while delegating
//! cryptographic verification to `iroha_crypto::streaming`.

#[cfg(feature = "quic")]
/// QUIC transport runtime for Norito Streaming.
pub mod quic;

#[cfg(feature = "quic")]
pub use quic::{
    CapabilityNegotiation, ControlStreamDirection, EndpointRole, StreamingClient,
    StreamingConnection, StreamingServer,
};
