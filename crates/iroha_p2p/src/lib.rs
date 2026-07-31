//! This module provides a network layer for holding of persistent
//! connections between blockchain nodes. Sane defaults for secure
//! Cryptography are chosen in this module, and encapsulated.
#![allow(unexpected_cfgs)]
#![allow(clippy::all)]
use std::{io, net::AddrParseError};

use aead::{Nonce, Tag};
use iroha_crypto::encryption::ChaCha20Poly1305;
pub use iroha_data_model::{
    block::consensus_v2::ConsensusMode, confidential::ConfidentialFeatureDigest,
};
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
pub type NetworkHandle<T> = network::NetworkBaseHandle<T, ChaCha20Poly1305>;

#[cfg(test)]
const P2P_ENCRYPTION_OVERHEAD_BYTES: usize =
    core::mem::size_of::<Nonce<ChaCha20Poly1305>>() + core::mem::size_of::<Tag<ChaCha20Poly1305>>();
const P2P_FRAME_LENGTH_PREFIX_BYTES: usize = core::mem::size_of::<u32>();

/// Largest encrypted P2P frame body representable by the on-wire length prefix.
pub const MAX_WIRE_ENCRYPTED_FRAME_BYTES: usize = u32::MAX as usize;

/// Largest encrypted P2P frame body accepted by the runtime configuration.
///
/// A stream frame is buffered contiguously as its four-byte length prefix plus
/// its encrypted body. This architecture-independent ceiling keeps that whole
/// allocation within `i32::MAX` bytes, so 32-bit and 64-bit validators accept
/// the same configuration and frame geometry.
pub const MAX_ENCRYPTED_FRAME_BYTES: usize = 2_147_483_643;

/// Return the maximum plaintext payload that fits the default ChaCha20-Poly1305 frame.
pub fn frame_plaintext_cap(max_frame_bytes: usize) -> usize {
    frame_plaintext_cap_for::<ChaCha20Poly1305>(max_frame_bytes)
}

/// Return the maximum plaintext payload that fits an encrypted frame for `E`.
pub fn frame_plaintext_cap_for<E: aead::AeadCore>(max_frame_bytes: usize) -> usize {
    let encryption_overhead = core::mem::size_of::<Nonce<E>>() + core::mem::size_of::<Tag<E>>();
    max_frame_bytes
        .min(MAX_ENCRYPTED_FRAME_BYTES)
        .saturating_sub(encryption_overhead)
}

/// Return the outbound byte-queue charge for one plaintext frame.
///
/// The default P2P transport queues a four-byte encrypted-frame length prefix,
/// a fixed nonce and authentication tag, and the encrypted plaintext bytes.
/// Returns `None` when the complete stream-frame length is not representable.
pub fn frame_queue_charge(plaintext_frame_bytes: usize) -> Option<usize> {
    frame_queue_charge_for::<ChaCha20Poly1305>(plaintext_frame_bytes)
}

/// Return the stream-queue charge for one plaintext frame encrypted by `E`.
pub fn frame_queue_charge_for<E: aead::AeadCore>(plaintext_frame_bytes: usize) -> Option<usize> {
    let encryption_overhead = core::mem::size_of::<Nonce<E>>() + core::mem::size_of::<Tag<E>>();
    plaintext_frame_bytes
        .checked_add(encryption_overhead)?
        .checked_add(P2P_FRAME_LENGTH_PREFIX_BYTES)
}

pub mod boilerplate {
    //! Module containing trait shorthands. Remove when trait aliases
    //! are stable <https://github.com/rust-lang/rust/issues/41517>

    use super::*;
    use aead::{Aead, AeadInOut, KeyInit};

    /// Shorthand for traits required for payload
    pub trait Pload:
        Encode + Decode + for<'a> norito::core::DecodeFromSlice<'a> + Send + Clone + 'static
    {
    }
    impl<T> Pload for T where
        T: Encode + Decode + for<'a> norito::core::DecodeFromSlice<'a> + Send + Clone + 'static
    {
    }

    /// Shorthand for traits required for encryptor type marker.
    pub trait Enc: Aead + AeadInOut + KeyInit + Clone + Send + 'static {}
    impl<T> Enc for T where T: Aead + AeadInOut + KeyInit + Clone + Send + 'static {}
}

/// Errors used in [`crate`].
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed IO operation
    Io(#[source] std::sync::Arc<io::Error>),
    /// Failed to bind TCP listener for configured addresses `{listen_addr}` / `{public_address}`: {error}
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
    /// Encrypted P2P frame exceeds the runtime cap or stream `u32` wire-body limit
    FrameTooLarge,
    /// Outbound {priority} frame queue full ({queued_frames}/{max_frames} frames, {queued_bytes}/{max_bytes} bytes)
    #[allow(clippy::doc_markdown)]
    OutboundFrameQueueFull {
        /// Queue priority label.
        priority: &'static str,
        /// Stream wire bytes already queued (four-byte prefix plus encrypted body).
        queued_bytes: usize,
        /// Maximum queued stream wire bytes allowed.
        max_bytes: usize,
        /// Encrypted frames already queued.
        queued_frames: usize,
        /// Maximum encrypted frames allowed.
        max_frames: usize,
    },
    /// Decrypted frame carried a malformed inner payload
    MalformedPayloadFrame,
    /// Decrypted frame exceeded the cap selected from its raw inbound topic
    InboundTopicCapExceeded,
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
    /// Unexpected peer identity during handshake (expected {expected}, found {found})
    HandshakePeerMismatch {
        /// Peer identifier the outbound dial expected to authenticate.
        expected: iroha_data_model::prelude::PeerId,
        /// Peer identifier actually authenticated by the signed handshake.
        found: iroha_data_model::prelude::PeerId,
    },
    /// Handshake metadata exceeds the maximum supported length (`u16::MAX` bytes)
    HandshakeMessageTooLarge,
    /// Local peer public key is malformed during handshake
    HandshakePublicKeyMalformed(#[source] iroha_crypto::error::ParseError),
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

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn frame_plaintext_cap_subtracts_overhead() {
        let cap = P2P_ENCRYPTION_OVERHEAD_BYTES + 64;
        assert_eq!(frame_plaintext_cap(cap), 64);
    }

    #[test]
    fn frame_plaintext_cap_saturates_when_too_small() {
        let cap = P2P_ENCRYPTION_OVERHEAD_BYTES.saturating_sub(1);
        assert_eq!(frame_plaintext_cap(cap), 0);
    }

    #[test]
    fn frame_plaintext_cap_clamps_to_cross_platform_runtime_limit() {
        assert_eq!(MAX_WIRE_ENCRYPTED_FRAME_BYTES, u32::MAX as usize);
        assert_eq!(
            MAX_ENCRYPTED_FRAME_BYTES + P2P_FRAME_LENGTH_PREFIX_BYTES,
            i32::MAX as usize
        );
        assert_eq!(
            frame_plaintext_cap(MAX_ENCRYPTED_FRAME_BYTES + 1),
            MAX_ENCRYPTED_FRAME_BYTES - P2P_ENCRYPTION_OVERHEAD_BYTES
        );
        assert_eq!(
            frame_plaintext_cap(usize::MAX),
            MAX_ENCRYPTED_FRAME_BYTES - P2P_ENCRYPTION_OVERHEAD_BYTES
        );
    }

    #[test]
    fn frame_queue_charge_includes_encryption_and_length_prefix() {
        assert_eq!(
            frame_queue_charge(64),
            Some(64 + P2P_ENCRYPTION_OVERHEAD_BYTES + P2P_FRAME_LENGTH_PREFIX_BYTES)
        );
        assert_eq!(frame_queue_charge(usize::MAX), None);
        assert_eq!(
            frame_queue_charge(MAX_ENCRYPTED_FRAME_BYTES - P2P_ENCRYPTION_OVERHEAD_BYTES),
            Some(MAX_ENCRYPTED_FRAME_BYTES + P2P_FRAME_LENGTH_PREFIX_BYTES)
        );
    }
}

/// Optional consensus handshake capabilities exchanged during p2p handshake.
///
/// These fields allow peers to gate connections by consensus mode/protocol
/// and a deterministic fingerprint derived from genesis and parameters.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ConsensusConfigCaps {
    /// Canonical V1 identity of every boot policy input which can affect execution.
    pub execution_policy_hash: [u8; 32],
    /// Canonical digest of deterministic, locally configured Nexus policy.
    pub nexus_policy_digest: [u8; 32],
    /// Canonical fixed-width Sumeragi v2 shared-runtime configuration hash.
    pub v2_config_fingerprint: [u8; 32],
    /// Canonical digest of the complete IVM gas schedule in this binary.
    pub ivm_gas_schedule_hash: [u8; 32],
}

/// Optional consensus handshake capabilities exchanged during p2p handshake.
///
/// These fields allow peers to gate connections by consensus mode/protocol
/// and a deterministic fingerprint derived from genesis and parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsensusHandshakeCaps {
    /// Canonical consensus mode.
    pub mode: ConsensusMode,
    /// Protocol wire version for consensus messages.
    pub proto_version: u32,
    /// Deterministic consensus fingerprint (blake2b-32 bytes).
    pub consensus_fingerprint: [u8; 32],
    /// Canonical v2 shared-config fingerprint.
    pub config: ConsensusConfigCaps,
}

/// Optional confidential-handshake capabilities exchanged during p2p handshake.
///
/// Nodes that advertise confidential capabilities signal whether they enforce
/// confidential verification locally (`enabled`), whether they accept blocks
/// without verifying (`assume_valid`), which verifier backend they expect, and
/// which static confidential policy digest they expect. Runtime registry roots
/// are validated through block headers instead of handshakes so catching-up peers
/// can reconnect at different committed heights.
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
