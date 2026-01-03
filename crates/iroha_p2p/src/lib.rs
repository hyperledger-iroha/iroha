//! This module provides a network layer for holding of persistent
//! connections between blockchain nodes. Sane defaults for secure
//! Cryptography are chosen in this module, and encapsulated.
#![allow(unexpected_cfgs)]
#![allow(clippy::all)]
use std::{io, net::AddrParseError};

use iroha_crypto::{
    blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    },
    encryption::ChaCha20Poly1305,
    kex::X25519Sha256,
};
pub use iroha_data_model::confidential::ConfidentialFeatureDigest;
pub use network::message::{UpdateTrustedPeers, *};
use norito::codec::{Decode, Encode};
use thiserror::Error;

pub mod network;
pub mod peer;
pub mod streaming;
pub mod transport;

pub(crate) mod sampler {
    //! Simple per-event log sampler: emits once per period and accumulates a suppressed count.
    #[derive(Debug, Clone)]
    pub struct LogSampler {
        last: tokio::time::Instant,
        suppressed: u64,
    }

    impl LogSampler {
        pub fn new() -> Self {
            Self {
                last: tokio::time::Instant::now() - tokio::time::Duration::from_secs(3600),
                suppressed: 0,
            }
        }
        /// Returns `Some(suppressed_count)` if it is time to log; otherwise increments internal counter and returns None.
        pub fn should_log(&mut self, period: tokio::time::Duration) -> Option<u64> {
            let now = tokio::time::Instant::now();
            if now.duration_since(self.last) >= period {
                self.last = now;
                let s = self.suppressed;
                self.suppressed = 0;
                Some(s)
            } else {
                self.suppressed = self.suppressed.saturating_add(1);
                None
            }
        }
    }
}

/// The main type to use for secure communication.
pub type NetworkHandle<T> = network::NetworkBaseHandle<T, X25519Sha256, ChaCha20Poly1305>;

pub mod boilerplate {
    //! Module containing trait shorthands. Remove when trait aliases
    //! are stable <https://github.com/rust-lang/rust/issues/41517>

    use aead::{Aead, KeyInit};
    use iroha_crypto::kex::KeyExchangeScheme;

    use super::*;

    /// Shorthand for traits required for payload
    pub trait Pload: Encode + Decode + Send + Clone + 'static {}
    impl<T> Pload for T where T: Encode + Decode + Send + Clone + 'static {}

    /// Shorthand for traits required for key exchange
    pub trait Kex: KeyExchangeScheme + Send + 'static {}
    impl<T> Kex for T where T: KeyExchangeScheme + Send + 'static {}

    /// Shorthand for traits required for encryptor type marker.
    pub trait Enc: Aead + KeyInit + Clone + Send + 'static {}
    impl<T> Enc for T where T: Aead + KeyInit + Clone + Send + 'static {}
}

/// Errors used in [`crate`].
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed IO operation
    Io(#[source] std::sync::Arc<io::Error>),
    /// Failed to bind TCP listener for configured addresses `{listen_addr}` / `{public_address}`
    BindListener {
        /// Listen address (with origin) that failed to bind.
        listen_addr: String,
        /// Public address (with origin) associated with the listener.
        public_address: String,
        /// Underlying IO error.
        #[source]
        error: std::sync::Arc<io::Error>,
    },
    /// Message improperly formatted
    Format,
    /// Field is not defined for a peer at this stage
    Field,
    /// Norito codec error
    NoritoCodec(#[from] norito::codec::Error),
    /// Failed to create keys
    Keys(#[from] iroha_crypto::error::Error),
    /// Symmetric encryption has failed
    SymmetricEncryption(#[from] iroha_crypto::encryption::Error),
    /// Failed to parse socket address
    Addr(#[from] AddrParseError),
    /// Connection reset by peer in the middle of message transfer
    ConnectionResetByPeer,
    /// Incoming frame exceeds configured maximum size
    FrameTooLarge,
    /// Handshake preface header invalid
    HandshakeBadPreface,
    /// Peer consensus handshake mismatch ({reason})
    HandshakeConsensusMismatch {
        /// Human-readable mismatch reason (mode/proto/fingerprint/config)
        reason: String,
    },
    /// Peer confidential handshake mismatch (`enabled/assume_valid/backend`)
    HandshakeConfidentialMismatch,
    /// Peer crypto handshake mismatch (`sm_enabled/sm_openssl_preview`)
    HandshakeCryptoMismatch,
    /// Handshake metadata exceeds the maximum supported length (`u16::MAX` bytes)
    HandshakeMessageTooLarge,
    /// `SoraNet` handshake negotiation failed.
    HandshakeSoranet(String),
    /// Noise handshake negotiation failed.
    HandshakeNoise(String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(std::sync::Arc::new(e))
    }
}

/// Result shorthand.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Optional consensus handshake capabilities exchanged during p2p handshake.
///
/// These fields allow peers to gate connections by consensus mode/protocol
/// and a deterministic fingerprint derived from genesis and parameters.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ConsensusConfigCaps {
    /// Number of collectors (K).
    pub collectors_k: u16,
    /// Redundant send fanout (r).
    pub redundant_send_r: u8,
    /// Data availability enabled (RBC + availability QC gating).
    pub da_enabled: bool,
    /// Execution QC gating enabled.
    pub require_execution_qc: bool,
    /// Execution QC persistence in WSV required before commit.
    pub require_wsv_exec_qc: bool,
    /// Maximum RBC chunk size in bytes.
    pub rbc_chunk_max_bytes: u64,
    /// RBC session TTL in milliseconds.
    pub rbc_session_ttl_ms: u64,
    /// Hard cap on persisted RBC sessions.
    pub rbc_store_max_sessions: u32,
    /// Soft cap on persisted RBC sessions.
    pub rbc_store_soft_sessions: u32,
    /// Hard cap on persisted RBC payload bytes.
    pub rbc_store_max_bytes: u64,
    /// Soft cap on persisted RBC payload bytes.
    pub rbc_store_soft_bytes: u64,
}

/// Optional consensus handshake capabilities exchanged during p2p handshake.
///
/// These fields allow peers to gate connections by consensus mode/protocol
/// and a deterministic fingerprint derived from genesis and parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusHandshakeCaps {
    /// Compile-time mode tag (e.g., "`iroha2-consensus::permissioned-sumeragi@v1`").
    pub mode_tag: String,
    /// Protocol wire version for consensus messages.
    pub proto_version: u32,
    /// Deterministic consensus fingerprint (blake2b-32 bytes).
    pub consensus_fingerprint: [u8; 32],
    /// Deterministic runtime config summary (collectors/DA/ExecQC/RBC caps).
    pub config: ConsensusConfigCaps,
}

/// Optional confidential-handshake capabilities exchanged during p2p handshake.
///
/// Nodes that advertise confidential capabilities signal whether they enforce
/// confidential verification locally (`enabled`), whether they accept blocks
/// without verifying (`assume_valid`), which verifier backend they expect, and
/// which confidential feature digest they currently enforce.
#[derive(Clone, Debug)]
pub struct ConfidentialHandshakeCaps {
    /// Whether the node enforces confidential verification locally.
    pub enabled: bool,
    /// Whether the node treats confidential verification as best-effort (observers).
    pub assume_valid: bool,
    /// Identifier of the verifier backend (e.g., `halo2-ipa-pallas`).
    pub verifier_backend: String,
    /// Optional digest of confidential registry/parameter expectations.
    pub features: Option<ConfidentialFeatureDigest>,
}

/// Optional crypto handshake capabilities exchanged during p2p handshake.
///
/// These values communicate whether SM helpers are enabled locally and whether
/// the OpenSSL preview path is active so peers can refuse mismatched
/// configurations before accepting transactions.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)] // handshake capability flags intentionally stored as booleans
pub struct CryptoHandshakeCaps {
    /// Whether SM helpers (SM2/SM3/SM4) are enabled locally.
    pub sm_enabled: bool,
    /// Whether the OpenSSL/Tongsuo preview provider is enabled.
    pub sm_openssl_preview: bool,
    /// Require peers to advertise matching SM helper availability during handshake.
    pub require_sm_handshake_match: bool,
    /// Require peers to match the OpenSSL preview toggle during handshake.
    pub require_sm_openssl_preview_match: bool,
}

/// Relay role advertised during the p2p handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RelayRole {
    /// Relay disabled; peers communicate directly.
    Disabled,
    /// Relay hub; accepts spokes and forwards traffic.
    Hub,
    /// Relay spoke; relies on a hub for fan-out.
    Spoke,
}

/// Module for unbounded channel with attached length of the channel.
/// Create Blake2b hash as u64 value
pub fn blake2b_hash(slice: impl AsRef<[u8]>) -> u64 {
    const U64_SIZE: usize = core::mem::size_of::<u64>();
    let hash = Blake2bVar::new(U64_SIZE)
        .expect("Failed to create hash with given length")
        .chain(&slice)
        .finalize_boxed();
    let mut bytes = [0; U64_SIZE];
    bytes.copy_from_slice(&hash);
    u64::from_be_bytes(bytes)
}
