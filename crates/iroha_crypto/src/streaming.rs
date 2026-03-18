//! Helpers for the Norito Streaming control-plane handshake and key management.
//!
//! This module wraps the lower-level primitives provided by `norito::streaming::crypto`
//! so that runtime components can verify `KeyUpdate`/`ContentKeyUpdate` frames,
//! derive transport keys, and unwrap Group Content Keys (GCKs) while enforcing
//! the monotonic counter and suite invariants mandated by the spec.

use std::{cmp, convert::TryInto, fmt};

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::DecodeFromSlice,
    streaming::{
        CapabilityFlags, CapabilityRole, ContentKeyUpdate, EncryptionSuite, FeedbackHintFrame,
        Hash, HpkeSuite, KeyUpdate, PrivacyBucketGranularity, ReceiverReport,
        TransportCapabilityResolution,
        crypto::{
            self as streaming_crypto, ContentKeyState, CryptoError as StreamingCryptoError,
            KeyUpdateState, TransportKeys,
        },
    },
};
use rand::RngCore;
use sha3::{Digest, Sha3_256};
use soranet_pq::{MlKemSuite, decapsulate_mlkem, encapsulate_mlkem};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::{
    Algorithm, KeyPair, PrivateKey, PublicKey, SessionKey, Signature, rng,
    signature::ed25519::Ed25519Sha512,
};

const SNAPSHOT_KEY_DOMAIN: &[u8] = b"iroha.streaming.snapshot-key";
const FEEDBACK_FP_SHIFT: u32 = 16;
const FEEDBACK_ALPHA_FP: u32 = 13_107;
const FEEDBACK_OFFSET_FP: u32 = 327;
const FEEDBACK_CEIL_BIAS: u32 = 0xFFFF;
const FEC_WINDOW_CHUNKS: u32 = 12;
const MAX_PARITY_CHUNKS: u8 = 6;
/// Default ML-KEM suite used for streaming key material when no explicit override is configured.
pub const STREAMING_DEFAULT_KEM_SUITE: MlKemSuite = MlKemSuite::MlKem768;

fn mlkem_suite_to_id(suite: MlKemSuite) -> u8 {
    match suite {
        MlKemSuite::MlKem512 => 0,
        MlKemSuite::MlKem768 => 1,
        MlKemSuite::MlKem1024 => 2,
    }
}

fn mlkem_suite_from_id(id: u8) -> Option<MlKemSuite> {
    match id {
        0 => Some(MlKemSuite::MlKem512),
        1 => Some(MlKemSuite::MlKem768),
        2 => Some(MlKemSuite::MlKem1024),
        _ => None,
    }
}

/// Errors that can occur while processing streaming handshake frames.
#[derive(Debug, Error)]
pub enum HandshakeError {
    /// Underlying streaming crypto failure while deriving or verifying handshake state.
    #[error(transparent)]
    Crypto(#[from] StreamingCryptoError),
    /// Norito codec failure when encoding or decoding control-plane frames.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Signature attached to a `KeyUpdate` frame failed verification.
    #[error("signature verification failed")]
    BadSignature,
    /// The remote advertised a key algorithm that this session cannot validate.
    #[error("unsupported key algorithm {0:?}")]
    UnsupportedAlgorithm(Algorithm),
    /// Transport keys have not yet been negotiated via a `KeyUpdate` exchange.
    #[error("transport keys not negotiated yet")]
    MissingTransportKeys,
    /// A content-encryption suite has not yet been negotiated for this session.
    #[error("content-encryption suite not negotiated yet")]
    SuiteNotNegotiated,
    /// The wrapped key payload was shorter than the nonce length implied by the suite.
    #[error("wrapped key payload too short (expected nonce {expected}, found {found})")]
    MalformedWrappedKey {
        /// Required nonce length for the negotiated suite.
        expected: usize,
        /// Actual byte length of the wrapped GCK payload received from the peer.
        found: usize,
    },
    /// The remote advertised an ephemeral public key of unexpected length.
    #[error("invalid ephemeral public key length (expected {expected}, found {found})")]
    InvalidEphemeralPublicKey {
        /// Expected byte length for the negotiated suite.
        expected: usize,
        /// Length observed in the frame.
        found: usize,
    },
    /// The remote Kyber public key had the wrong length.
    #[error("kyber public key length mismatch (expected {expected}, found {found})")]
    InvalidKyberPublicKeyLength {
        /// Expected byte length for a Kyber public key.
        expected: usize,
        /// Actual byte length provided by the caller.
        found: usize,
    },
    /// The remote Kyber public key bytes could not be decoded.
    #[error("kyber public key bytes rejected")]
    InvalidKyberPublicKey,
    /// The configured Kyber secret key had the wrong length.
    #[error("kyber secret key length mismatch (expected {expected}, found {found})")]
    InvalidKyberSecretKeyLength {
        /// Expected byte length for a Kyber secret key.
        expected: usize,
        /// Actual byte length provided by the caller.
        found: usize,
    },
    /// The configured Kyber secret key bytes could not be decoded.
    #[error("kyber secret key bytes rejected")]
    InvalidKyberSecretKey,
    /// Kyber remote public key has not been configured for this session.
    #[error("kyber remote public key not configured")]
    MissingKyberRemotePublic,
    /// Kyber local secret key has not been configured for this session.
    #[error("kyber local secret key not configured")]
    MissingKyberLocalSecret,
    /// The Kyber fingerprint advertised in the suite mismatched the configured key.
    #[error("kyber suite fingerprint mismatch: expected {expected:?}, found {found:?}")]
    KyberFingerprintMismatch {
        /// Fingerprint derived from the configured Kyber public key.
        expected: Hash,
        /// Fingerprint claimed by the remote frame.
        found: Hash,
    },
    /// Failed to decode the Kyber ciphertext transmitted in `KeyUpdate`.
    #[error("kyber ciphertext rejected")]
    InvalidKyberCiphertext,
    /// Encountered an unsupported ML-KEM suite identifier while restoring state.
    #[error("unsupported ML-KEM suite id {0}")]
    UnsupportedKemSuite(u8),
    /// Attempted to restore session state into a mismatched endpoint role.
    #[error("session role mismatch: expected {expected:?}, found {found:?}")]
    RoleMismatch {
        /// Role carried by the existing session.
        expected: CapabilityRole,
        /// Role stored inside the snapshot being restored.
        found: CapabilityRole,
    },
}

const KYBER_FINGERPRINT_DOMAIN: &[u8] = b"nsc_kyber_pk";

/// Errors that can occur while preparing local streaming key material.
#[derive(Debug, Error)]
pub enum KeyMaterialError {
    /// Identity keys must use Ed25519 so `KeyUpdate` signatures are interoperable.
    #[error("identity key must use Ed25519, found {0:?}")]
    UnsupportedIdentityAlgorithm(Algorithm),
    /// The supplied Kyber public key had an unexpected length.
    #[error("kyber public key length mismatch (expected {expected}, found {found})")]
    InvalidKyberPublicKeyLength {
        /// Expected byte length for a Kyber public key.
        expected: usize,
        /// Actual byte length provided by the caller.
        found: usize,
    },
    /// Kyber public key bytes failed validation.
    #[error("kyber public key bytes rejected")]
    InvalidKyberPublicKey,
    /// The supplied Kyber secret key had an unexpected length.
    #[error("kyber secret key length mismatch (expected {expected}, found {found})")]
    InvalidKyberSecretKeyLength {
        /// Expected byte length for a Kyber secret key.
        expected: usize,
        /// Actual byte length provided by the caller.
        found: usize,
    },
    /// Kyber secret key bytes failed validation.
    #[error("kyber secret key bytes rejected")]
    InvalidKyberSecretKey,
    /// Wrapper for lower-level handshake errors surfaced while installing material.
    #[error(transparent)]
    Handshake(HandshakeError),
}

impl From<HandshakeError> for KeyMaterialError {
    fn from(err: HandshakeError) -> Self {
        match err {
            HandshakeError::InvalidKyberPublicKeyLength { expected, found } => {
                Self::InvalidKyberPublicKeyLength { expected, found }
            }
            HandshakeError::InvalidKyberPublicKey => Self::InvalidKyberPublicKey,
            HandshakeError::InvalidKyberSecretKeyLength { expected, found } => {
                Self::InvalidKyberSecretKeyLength { expected, found }
            }
            HandshakeError::InvalidKyberSecretKey => Self::InvalidKyberSecretKey,
            other => Self::Handshake(other),
        }
    }
}

/// Runtime-owned key material used to drive the control-plane handshake.
#[derive(Clone, Debug)]
pub struct StreamingKeyMaterial {
    identity: KeyPair,
    kyber_public: Option<Vec<u8>>,
    kyber_secret: Option<Zeroizing<Vec<u8>>>,
    kyber_fingerprint: Option<Hash>,
    kem_suite: MlKemSuite,
}

impl StreamingKeyMaterial {
    /// Construct a new key-material bundle from an identity key pair.
    ///
    /// # Errors
    ///
    /// Returns [`KeyMaterialError::UnsupportedIdentityAlgorithm`] when the provided keys are not
    /// Ed25519, which is required for signing `KeyUpdate` frames.
    pub fn new(identity: KeyPair) -> Result<Self, KeyMaterialError> {
        if identity.algorithm() != Algorithm::Ed25519 {
            return Err(KeyMaterialError::UnsupportedIdentityAlgorithm(
                identity.algorithm(),
            ));
        }
        Ok(Self {
            identity,
            kyber_public: None,
            kyber_secret: None,
            kyber_fingerprint: None,
            kem_suite: STREAMING_DEFAULT_KEM_SUITE,
        })
    }

    /// Return the configured Ed25519 identity key pair.
    pub fn identity(&self) -> &KeyPair {
        &self.identity
    }

    /// Record the Kyber key pair used for HPKE interactions.
    ///
    /// # Errors
    ///
    /// Returns length/validation-related [`KeyMaterialError`] variants when the byte slices do not
    /// describe valid Kyber keys.
    pub fn set_kyber_keys(
        &mut self,
        public_key: &[u8],
        secret_key: &[u8],
    ) -> Result<(), KeyMaterialError> {
        let expected_public = self.kem_suite.public_key_len();
        if public_key.len() != expected_public {
            return Err(KeyMaterialError::InvalidKyberPublicKeyLength {
                expected: expected_public,
                found: public_key.len(),
            });
        }
        self.kem_suite
            .validate_public_key(public_key)
            .map_err(|_| KeyMaterialError::InvalidKyberPublicKey)?;

        let expected_secret = self.kem_suite.secret_key_len();
        if secret_key.len() != expected_secret {
            return Err(KeyMaterialError::InvalidKyberSecretKeyLength {
                expected: expected_secret,
                found: secret_key.len(),
            });
        }
        self.kem_suite
            .validate_secret_key(secret_key)
            .map_err(|_| KeyMaterialError::InvalidKyberSecretKey)?;

        let fingerprint = fingerprint_kyber_public(public_key, self.kem_suite);
        self.kyber_fingerprint = Some(fingerprint);
        self.kyber_public = Some(public_key.to_vec());
        self.kyber_secret = Some(Zeroizing::new(secret_key.to_vec()));
        Ok(())
    }

    /// Return the configured Kyber public key bytes, if present.
    pub fn kyber_public(&self) -> Option<&[u8]> {
        self.kyber_public.as_deref()
    }

    /// Return the configured Kyber secret key bytes, if present.
    pub fn kyber_secret(&self) -> Option<&[u8]> {
        self.kyber_secret.as_ref().map(|secret| secret.as_slice())
    }

    /// Return the fingerprint associated with the configured Kyber public key.
    pub fn kyber_fingerprint(&self) -> Option<Hash> {
        self.kyber_fingerprint
    }

    /// Return the ML-KEM suite configured for streaming HPKE.
    #[must_use]
    pub fn kem_suite(&self) -> MlKemSuite {
        self.kem_suite
    }

    /// Override the ML-KEM suite used for streaming HPKE. Existing key material is cleared when
    /// the suite changes so callers must provide fresh public/secret bytes for the new profile.
    pub fn set_kem_suite(&mut self, suite: MlKemSuite) {
        if self.kem_suite != suite {
            self.kyber_public = None;
            self.kyber_secret = None;
            self.kyber_fingerprint = None;
        }
        self.kem_suite = suite;
    }

    /// Install local key material onto a [`StreamingSession`], enabling HPKE decapsulation.
    ///
    /// Sessions that do not require Kyber support can call this without previously configuring
    /// Kyber keys; the method becomes a no-op in that case.
    ///
    /// # Errors
    ///
    /// Returns [`KeyMaterialError::InvalidKyberSecretKeyLength`] or
    /// [`KeyMaterialError::InvalidKyberSecretKey`] if the stored Kyber secret key bytes are
    /// rejected by [`StreamingSession::set_kyber_local_secret`], and propagates other
    /// [`KeyMaterialError`] variants surfaced while installing the key.
    pub fn install_into_session(
        &self,
        session: &mut StreamingSession,
    ) -> Result<(), KeyMaterialError> {
        session.set_kem_suite(self.kem_suite);
        if let Some(secret) = &self.kyber_secret {
            session
                .set_kyber_local_secret(secret.as_slice())
                .map_err(KeyMaterialError::from)?;
        }
        Ok(())
    }

    /// Convenience wrapper around [`StreamingSession::build_key_update`] using the stored identity.
    ///
    /// # Errors
    ///
    /// Returns the same [`HandshakeError`] variants as
    /// [`StreamingSession::build_key_update`], including
    /// [`HandshakeError::UnsupportedAlgorithm`] if the identity key is not Ed25519.
    pub fn build_key_update(
        &self,
        session: &mut StreamingSession,
        session_id: Hash,
        suite: &EncryptionSuite,
        protocol_version: u16,
        key_counter: u64,
    ) -> Result<KeyUpdate, HandshakeError> {
        session.build_key_update(
            session_id,
            suite,
            protocol_version,
            key_counter,
            self.identity.private_key(),
        )
    }

    /// Derive the symmetric session key used to encrypt streaming snapshots.
    #[must_use]
    pub fn snapshot_session_key(&self) -> SessionKey {
        let (_, private_bytes) = self.identity.private_key().to_bytes();
        let private = Zeroizing::new(private_bytes);
        let mut hasher = Sha3_256::new();
        hasher.update(SNAPSHOT_KEY_DOMAIN);
        hasher.update(&*private);
        SessionKey::new(hasher.finalize().to_vec())
    }
}

/// Compute the Kyber fingerprint advertised inside `EncryptionSuite::Kyber768XChaCha20Poly1305`.
///
/// # Errors
///
/// Returns length/validation [`KeyMaterialError`] variants if the provided bytes are not a valid
/// Kyber public key.
pub fn kyber_public_fingerprint(public_key: &[u8]) -> Result<Hash, KeyMaterialError> {
    kyber_public_fingerprint_with_suite(public_key, STREAMING_DEFAULT_KEM_SUITE)
}

/// Compute the Kyber fingerprint for the provided suite.
///
/// # Errors
///
/// Returns length/validation [`KeyMaterialError`] variants if the provided bytes are not a valid
/// Kyber public key for the requested suite.
pub fn kyber_public_fingerprint_with_suite(
    public_key: &[u8],
    suite: MlKemSuite,
) -> Result<Hash, KeyMaterialError> {
    let expected = suite.public_key_len();
    if public_key.len() != expected {
        return Err(KeyMaterialError::InvalidKyberPublicKeyLength {
            expected,
            found: public_key.len(),
        });
    }
    suite
        .validate_public_key(public_key)
        .map_err(|_| KeyMaterialError::InvalidKyberPublicKey)?;
    Ok(fingerprint_kyber_public(public_key, suite))
}

fn fingerprint_kyber_public(bytes: &[u8], _suite: MlKemSuite) -> Hash {
    let mut hasher = Sha3_256::new();
    hasher.update(KYBER_FINGERPRINT_DOMAIN);
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[derive(Clone)]
enum EphemeralState {
    X25519(X25519Ephemeral),
}

#[derive(Clone, Copy)]
enum EphemeralMechanism<'suite> {
    X25519,
    Kyber768 { fingerprint: &'suite Hash },
}

fn suite_ephemeral_mechanism(
    suite: &EncryptionSuite,
) -> Result<EphemeralMechanism<'_>, StreamingCryptoError> {
    if let EncryptionSuite::X25519ChaCha20Poly1305(_) = suite {
        Ok(EphemeralMechanism::X25519)
    } else if let EncryptionSuite::Kyber768XChaCha20Poly1305(fingerprint) = suite {
        Ok(EphemeralMechanism::Kyber768 { fingerprint })
    } else {
        Err(StreamingCryptoError::UnsupportedSuite)
    }
}

#[derive(Clone)]
struct X25519Ephemeral {
    secret: StaticSecret,
    public: [u8; 32],
}

impl X25519Ephemeral {
    fn new_random() -> Self {
        let secret = StaticSecret::random_from_rng(rng::os_rng());
        Self::from_secret(secret)
    }

    fn from_secret(secret: StaticSecret) -> Self {
        let public = X25519PublicKey::from(&secret).to_bytes();
        Self { secret, public }
    }

    fn shared_secret(&self, peer: &X25519PublicKey) -> [u8; 32] {
        let shared = self.secret.diffie_hellman(peer);
        let mut out = [0u8; 32];
        out.copy_from_slice(shared.as_bytes());
        out
    }
}

/// In-memory state machine for a single NSC control-stream pairing.
#[derive(Clone)]
pub struct StreamingSession {
    role: CapabilityRole,
    session_id: Option<Hash>,
    key_state: KeyUpdateState,
    #[allow(dead_code)]
    /// Tracks the latest content-key rotation accepted by this session.
    content_state: ContentKeyState,
    suite: Option<EncryptionSuite>,
    sts_root: Option<[u8; 32]>,
    transport_keys: Option<TransportKeys>,
    latest_gck: Option<Vec<u8>>,
    cadence: Option<SessionCadence>,
    transport_resolution: Option<TransportCapabilityResolution>,
    transport_capabilities_hash: Option<Hash>,
    negotiated_capabilities: Option<CapabilityFlags>,
    feedback: FeedbackState,
    local_ephemeral: Option<EphemeralState>,
    kyber_remote_public: Option<Vec<u8>>,
    kyber_remote_fingerprint: Option<Hash>,
    kyber_local_secret: Option<Zeroizing<Vec<u8>>>,
    kem_suite: MlKemSuite,
}

/// Persistable view of the resolved transport capabilities.
#[derive(Copy, Clone, Debug, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct TransportCapabilityResolutionSnapshot {
    /// Selected HPKE suite.
    pub hpke_suite: HpkeSuite,
    /// Whether QUIC DATAGRAM delivery is enabled.
    pub use_datagram: bool,
    /// Maximum DATAGRAM payload in bytes.
    pub max_segment_datagram_size: u16,
    /// Negotiated feedback-hint cadence in milliseconds.
    pub fec_feedback_interval_ms: u16,
    /// Privacy bucket granularity advertised for telemetry.
    pub privacy_bucket_granularity: PrivacyBucketGranularity,
}

impl From<&TransportCapabilityResolution> for TransportCapabilityResolutionSnapshot {
    fn from(resolution: &TransportCapabilityResolution) -> Self {
        Self {
            hpke_suite: resolution.hpke_suite,
            use_datagram: resolution.use_datagram,
            max_segment_datagram_size: resolution.max_segment_datagram_size,
            fec_feedback_interval_ms: resolution.fec_feedback_interval_ms,
            privacy_bucket_granularity: resolution.privacy_bucket_granularity,
        }
    }
}

impl From<TransportCapabilityResolutionSnapshot> for TransportCapabilityResolution {
    fn from(snapshot: TransportCapabilityResolutionSnapshot) -> Self {
        Self {
            hpke_suite: snapshot.hpke_suite,
            use_datagram: snapshot.use_datagram,
            max_segment_datagram_size: snapshot.max_segment_datagram_size,
            fec_feedback_interval_ms: snapshot.fec_feedback_interval_ms,
            privacy_bucket_granularity: snapshot.privacy_bucket_granularity,
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for TransportCapabilityResolutionSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<TransportCapabilityResolutionSnapshot>(bytes)
    }
}

/// Minimal persistence snapshot for resuming streaming sessions after restarts.
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StreamingSessionSnapshot {
    /// Endpoint role associated with the session.
    pub role: CapabilityRole,
    /// Unique session identifier derived from the establishing handshake.
    pub session_id: Hash,
    /// Number of key updates that have been processed in this session.
    pub key_counter: u64,
    /// Encryption suite negotiated for the session.
    pub suite: EncryptionSuite,
    /// Identifier for the ML-KEM suite associated with the control-plane HPKE keys.
    pub kem_suite_id: u8,
    /// Root of the STS transcript for resumable authentication.
    pub sts_root: [u8; 32],
    /// Latest group content key blob staged for distribution.
    pub latest_gck: Option<Vec<u8>>,
    /// Identifier of the most recent content key issued to the peer.
    pub last_content_key_id: Option<u64>,
    /// Logical start instant for the latest content key distributed.
    pub last_content_key_valid_from: Option<u64>,
    /// Persisted cadence state for deterministic rekey scheduling.
    pub cadence: Option<SessionCadenceSnapshot>,
    /// Resolved transport capabilities negotiated with the peer.
    pub transport_capabilities: Option<TransportCapabilityResolutionSnapshot>,
    /// Capability feature flags advertised by the peer.
    pub negotiated_capabilities: Option<CapabilityFlags>,
    /// Remote Kyber public key bytes expected for the session (if negotiated).
    pub kyber_remote_public: Option<Vec<u8>>,
    /// Remote Kyber fingerprint used to validate suite claims.
    pub kyber_remote_fingerprint: Option<Hash>,
}

impl<'a> DecodeFromSlice<'a> for StreamingSessionSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<StreamingSessionSnapshot>(bytes)
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    #[test]
    fn streaming_session_snapshot_decode_roundtrip() {
        let snapshot = StreamingSessionSnapshot {
            role: CapabilityRole::Viewer,
            session_id: [0x11; 32],
            key_counter: 1,
            suite: EncryptionSuite::X25519ChaCha20Poly1305([0x22; 32]),
            kem_suite_id: mlkem_suite_to_id(STREAMING_DEFAULT_KEM_SUITE),
            sts_root: [0x33; 32],
            latest_gck: Some(vec![0x44, 0x45]),
            last_content_key_id: Some(7),
            last_content_key_valid_from: Some(8),
            cadence: None,
            transport_capabilities: None,
            negotiated_capabilities: None,
            kyber_remote_public: None,
            kyber_remote_fingerprint: None,
        };
        let bytes = norito::codec::encode_adaptive(&snapshot);
        let (decoded, used) =
            StreamingSessionSnapshot::decode_from_slice(&bytes).expect("decode snapshot");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn streaming_session_snapshot_rejects_invalid_payload() {
        let err = StreamingSessionSnapshot::decode_from_slice(b"invalid payload")
            .expect_err("invalid snapshot should fail");
        assert!(matches!(
            err,
            norito::Error::LengthMismatch | norito::Error::DecodePanic { .. }
        ));
    }
}

/// Snapshot of feedback-processing state captured for telemetry/reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedbackStateSnapshot {
    /// Latest parity decision derived from feedback frames.
    pub parity_chunks: u8,
    /// Total feedback hints processed.
    pub hints_received: u32,
    /// Total receiver reports processed.
    pub reports_received: u32,
    /// Current EWMA of the observed loss (Q16 fixed point).
    pub loss_ewma_q16: Option<u32>,
    /// Latest delivered sequence reported by the viewer.
    pub last_delivered_sequence: Option<u64>,
    /// Last parity value reported by the viewer.
    pub latest_parity_applied: Option<u8>,
    /// Last advertised FEC budget from the viewer.
    pub latest_fec_budget: Option<u8>,
    /// Most recent report interval suggested by the viewer.
    pub report_interval_ms: Option<u16>,
    /// Latest latency gradient observed (Q16 fixed point, signed).
    pub latency_gradient_q16: i32,
    /// Latest RTT observation reported by the viewer (milliseconds).
    pub observed_rtt_ms: u16,
}

#[derive(Clone, Debug, Default)]
struct FeedbackState {
    stream_id: Option<Hash>,
    hints_received: u32,
    reports_received: u32,
    loss_ewma_q16: Option<u32>,
    parity_chunks: u8,
    latest_parity: Option<u8>,
    latest_parity_applied: Option<u8>,
    latest_fec_budget: Option<u8>,
    last_delivered_sequence: Option<u64>,
    report_interval_ms: Option<u16>,
    latency_gradient_q16: i32,
    observed_rtt_ms: u16,
}

/// Persisted view of the session rekey cadence.
#[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct SessionCadenceSnapshot {
    /// Wall-clock start of the session in milliseconds since Unix epoch.
    pub started_at_ms: u64,
    /// Total payload bytes transmitted since session start.
    pub total_payload_bytes: u64,
}

impl<'a> DecodeFromSlice<'a> for SessionCadenceSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<SessionCadenceSnapshot>(bytes)
    }
}

/// Deterministic cadence tracker used to decide when rekeys must be emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionCadence {
    started_at_ms: u64,
    total_payload_bytes: u64,
}

impl SessionCadence {
    /// Maximum payload budget per HPKE key (64 MiB).
    pub const PAYLOAD_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
    /// Maximum wall-clock budget per HPKE key (5 minutes).
    pub const DURATION_BUDGET_MS: u64 = 300_000;

    /// Construct a new cadence tracker starting at the given millisecond timestamp.
    #[must_use]
    pub const fn new(started_at_ms: u64) -> Self {
        Self {
            started_at_ms,
            total_payload_bytes: 0,
        }
    }

    /// Return the session start timestamp the cadence is anchored to.
    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    /// Return the cumulative payload bytes accounted for by this cadence.
    #[must_use]
    pub const fn total_payload_bytes(&self) -> u64 {
        self.total_payload_bytes
    }

    /// Record additional payload bytes emitted while using the current transport keys.
    pub fn record_payload_bytes(&mut self, bytes: u64) {
        self.total_payload_bytes = self.total_payload_bytes.saturating_add(bytes);
    }

    /// Overwrite the cumulative payload counter (e.g., after snapshot restoration).
    pub fn set_total_payload_bytes(&mut self, total: u64) {
        self.total_payload_bytes = total;
    }

    /// Compute the minimum `KeyUpdate.key_counter` required at `now_ms`.
    ///
    /// The counter grows whenever the cumulative payload crosses a 64 MiB boundary or the
    /// elapsed time since session start exceeds the 5-minute budget.
    #[must_use]
    pub fn required_key_counter(&self, now_ms: u64) -> u64 {
        let elapsed_ms = now_ms.saturating_sub(self.started_at_ms);
        let payload_rekeys = Self::div_ceil(self.total_payload_bytes, Self::PAYLOAD_BUDGET_BYTES);
        let time_rekeys = Self::div_ceil(elapsed_ms, Self::DURATION_BUDGET_MS);
        cmp::max(1, payload_rekeys.max(time_rekeys))
    }

    /// Produce a snapshot suitable for persistence.
    #[must_use]
    pub const fn snapshot(&self) -> SessionCadenceSnapshot {
        SessionCadenceSnapshot {
            started_at_ms: self.started_at_ms,
            total_payload_bytes: self.total_payload_bytes,
        }
    }

    /// Reconstruct a cadence tracker from a previously recorded snapshot.
    #[must_use]
    pub const fn from_snapshot(snapshot: SessionCadenceSnapshot) -> Self {
        Self {
            started_at_ms: snapshot.started_at_ms,
            total_payload_bytes: snapshot.total_payload_bytes,
        }
    }

    const fn div_ceil(value: u64, divisor: u64) -> u64 {
        debug_assert!(divisor > 0, "cadence divisor must be non-zero");
        if value == 0 {
            0
        } else {
            1 + (value - 1) / divisor
        }
    }
}

impl fmt::Debug for StreamingSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamingSession")
            .field("role", &self.role)
            .field("session_id", &self.session_id)
            .field("suite", &self.suite)
            .field("key_state", &self.key_state)
            .field("content_state", &self.content_state)
            .field("transport_keys", &self.transport_keys.is_some())
            .field("sts_root_set", &self.sts_root.is_some())
            .field("latest_gck_len", &self.latest_gck.as_ref().map(Vec::len))
            .field("cadence_set", &self.cadence.is_some())
            .field("transport_resolution", &self.transport_resolution)
            .field(
                "transport_capabilities_hash",
                &self.transport_capabilities_hash,
            )
            .field("negotiated_capabilities", &self.negotiated_capabilities)
            .field("feedback", &self.feedback)
            .field("local_ephemeral_set", &self.local_ephemeral.is_some())
            .field(
                "kyber_remote_public_set",
                &self.kyber_remote_public.is_some(),
            )
            .field("kyber_remote_fingerprint", &self.kyber_remote_fingerprint)
            .field("kyber_local_secret_set", &self.kyber_local_secret.is_some())
            .field("kem_suite", &self.kem_suite)
            .finish()
    }
}

impl StreamingSession {
    /// Create a new session state machine for the given endpoint role.
    pub fn new(role: CapabilityRole) -> Self {
        Self {
            role,
            session_id: None,
            key_state: KeyUpdateState::default(),
            content_state: ContentKeyState::default(),
            suite: None,
            sts_root: None,
            transport_keys: None,
            latest_gck: None,
            cadence: None,
            transport_resolution: None,
            transport_capabilities_hash: None,
            negotiated_capabilities: None,
            feedback: FeedbackState::default(),
            local_ephemeral: None,
            kyber_remote_public: None,
            kyber_remote_fingerprint: None,
            kyber_local_secret: None,
            kem_suite: STREAMING_DEFAULT_KEM_SUITE,
        }
    }

    /// Override the ML-KEM suite used when negotiating HPKE material for this session.
    pub fn set_kem_suite(&mut self, suite: MlKemSuite) {
        self.kem_suite = suite;
    }

    fn reset_for_new_session(&mut self) {
        self.session_id = None;
        self.key_state = KeyUpdateState::default();
        self.content_state = ContentKeyState::default();
        self.suite = None;
        self.sts_root = None;
        self.transport_keys = None;
        self.latest_gck = None;
        self.cadence = None;
        self.transport_resolution = None;
        self.transport_capabilities_hash = None;
        self.negotiated_capabilities = None;
        self.feedback = FeedbackState::default();
    }

    /// Return the endpoint role associated with this session.
    pub fn role(&self) -> CapabilityRole {
        self.role
    }

    /// Return the negotiated encryption suite, if one has been recorded.
    pub fn negotiated_suite(&self) -> Option<&EncryptionSuite> {
        self.suite.as_ref()
    }

    /// Return the derived transport keys, if the handshake has completed.
    pub fn transport_keys(&self) -> Option<&TransportKeys> {
        self.transport_keys.as_ref()
    }

    /// Return the negotiated transport capability resolution, if recorded.
    pub fn transport_capabilities(&self) -> Option<&TransportCapabilityResolution> {
        self.transport_resolution.as_ref()
    }

    /// Return the hash of the negotiated transport capabilities.
    pub fn transport_capabilities_hash(&self) -> Option<Hash> {
        self.transport_capabilities_hash
    }

    /// Return the negotiated feature flags for this session, if any.
    pub fn capabilities(&self) -> Option<CapabilityFlags> {
        self.negotiated_capabilities
    }

    /// Record the resolved transport capability negotiation.
    pub fn record_transport_capabilities(&mut self, resolution: TransportCapabilityResolution) {
        self.transport_capabilities_hash = Some(resolution.capabilities_hash());
        self.transport_resolution = Some(resolution);
    }

    /// Record the accepted capability feature flags.
    pub fn record_capabilities(&mut self, capabilities: CapabilityFlags) {
        self.negotiated_capabilities = Some(capabilities);
    }

    /// Return the Session Transport Secret (STS) root derived from the last key update.
    pub fn sts_root(&self) -> Option<&[u8; 32]> {
        self.sts_root.as_ref()
    }

    /// Return the most recently unwrapped Group Content Key (GCK), if available.
    pub fn latest_gck(&self) -> Option<&[u8]> {
        self.latest_gck.as_deref()
    }

    /// Record a feedback hint and update EWMA/loss state.
    pub fn process_feedback_hint(&mut self, hint: &FeedbackHintFrame) {
        let feedback = &mut self.feedback;
        feedback.stream_id = Some(hint.stream_id);
        feedback.hints_received = feedback.hints_received.saturating_add(1);
        feedback.report_interval_ms = Some(hint.report_interval_ms);
        feedback.parity_chunks = feedback.parity_chunks.max(hint.parity_chunks);
        feedback.latency_gradient_q16 = hint.latency_gradient_q16;
        feedback.observed_rtt_ms = hint.observed_rtt_ms;
        feedback.loss_ewma_q16 = Some(
            feedback
                .loss_ewma_q16
                .map_or(hint.loss_ewma_q16, |current| {
                    ewma_update(current, hint.loss_ewma_q16)
                }),
        );
    }

    /// Record a receiver report and return the parity decision for the current window.
    pub fn process_receiver_report(&mut self, report: &ReceiverReport) -> u8 {
        let feedback = &mut self.feedback;
        feedback.reports_received = feedback.reports_received.saturating_add(1);
        feedback.last_delivered_sequence = Some(report.delivered_sequence);
        feedback.latest_parity_applied = Some(report.parity_applied);
        feedback.latest_fec_budget = Some(report.fec_budget);

        let loss_fp = feedback
            .loss_ewma_q16
            .or_else(|| loss_percent_to_q16(report.loss_percent_x100))
            .unwrap_or(0);
        let mut parity = parity_from_loss_fp(loss_fp);
        if let Some(existing) = feedback.latest_parity {
            parity = parity.max(existing);
        }
        parity = parity.min(MAX_PARITY_CHUNKS);
        feedback.latest_parity = Some(parity);
        feedback.parity_chunks = feedback.parity_chunks.max(parity);
        parity
    }

    /// Return the latest parity derived from feedback state.
    pub fn latest_feedback_parity(&self) -> Option<u8> {
        self.feedback.latest_parity
    }

    /// Produce a snapshot of the feedback state for persistence/telemetry.
    pub fn feedback_snapshot(&self) -> Option<FeedbackStateSnapshot> {
        if self.feedback.hints_received == 0
            && self.feedback.reports_received == 0
            && self.feedback.latest_parity.is_none()
        {
            None
        } else {
            Some(self.feedback.snapshot())
        }
    }

    /// Return the cadence tracker associated with this session, if one is installed.
    pub fn cadence(&self) -> Option<&SessionCadence> {
        self.cadence.as_ref()
    }

    /// Return a mutable reference to the cadence tracker, if one is installed.
    pub fn cadence_mut(&mut self) -> Option<&mut SessionCadence> {
        self.cadence.as_mut()
    }

    /// Replace the cadence tracker for this session.
    pub fn set_cadence(&mut self, cadence: SessionCadence) {
        self.cadence = Some(cadence);
    }

    /// Capture the current handshake state so it can be persisted and restored later.
    pub fn snapshot_state(&self) -> Option<StreamingSessionSnapshot> {
        let session_id = self.session_id?;
        let key_counter = self.key_state.last_counter()?;
        let suite = self.suite.or_else(|| self.key_state.suite().copied())?;
        let sts_root = self.sts_root?;
        Some(StreamingSessionSnapshot {
            role: self.role,
            session_id,
            key_counter,
            suite,
            kem_suite_id: mlkem_suite_to_id(self.kem_suite),
            sts_root,
            latest_gck: self.latest_gck.clone(),
            last_content_key_id: self.content_state.last_id(),
            last_content_key_valid_from: self.content_state.last_valid_from(),
            cadence: self.cadence.as_ref().map(SessionCadence::snapshot),
            transport_capabilities: self
                .transport_resolution
                .as_ref()
                .map(TransportCapabilityResolutionSnapshot::from),
            negotiated_capabilities: self.negotiated_capabilities,
            kyber_remote_public: self.kyber_remote_public.clone(),
            kyber_remote_fingerprint: self.kyber_remote_fingerprint,
        })
    }

    /// Restore a previously snapshotted handshake state, replacing the current context.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::RoleMismatch`] when the snapshot was captured for the opposite
    /// endpoint role, or [`HandshakeError::Crypto`] if the stored transport root cannot be used
    /// to re-derive the symmetric keys.
    pub fn restore_from_snapshot(
        &mut self,
        snapshot: StreamingSessionSnapshot,
    ) -> Result<(), HandshakeError> {
        let StreamingSessionSnapshot {
            role,
            session_id,
            key_counter,
            suite,
            kem_suite_id,
            sts_root,
            latest_gck,
            last_content_key_id,
            last_content_key_valid_from,
            cadence,
            transport_capabilities: transport_capabilities_snapshot,
            negotiated_capabilities: negotiated_capabilities_snapshot,
            kyber_remote_public,
            kyber_remote_fingerprint,
        } = snapshot;

        if role != self.role {
            return Err(HandshakeError::RoleMismatch {
                expected: self.role,
                found: role,
            });
        }

        self.reset_for_new_session();
        self.session_id = Some(session_id);
        self.key_state.restore(Some(key_counter), Some(suite));
        self.suite = Some(suite);
        self.kem_suite = mlkem_suite_from_id(kem_suite_id)
            .ok_or(HandshakeError::UnsupportedKemSuite(kem_suite_id))?;
        self.sts_root = Some(sts_root);
        let transport =
            streaming_crypto::derive_transport_keys_from_sts_root(&sts_root, self.role)?;
        self.transport_keys = Some(transport);
        self.latest_gck = latest_gck;
        self.content_state
            .restore(last_content_key_id, last_content_key_valid_from);
        self.cadence = cadence.map(SessionCadence::from_snapshot);
        let transport_resolution =
            transport_capabilities_snapshot.map(TransportCapabilityResolution::from);
        self.transport_capabilities_hash = transport_resolution
            .as_ref()
            .map(TransportCapabilityResolution::capabilities_hash);
        self.transport_resolution = transport_resolution;
        self.negotiated_capabilities = negotiated_capabilities_snapshot;
        self.kyber_remote_public = kyber_remote_public;
        self.kyber_remote_fingerprint = kyber_remote_fingerprint;
        Ok(())
    }

    /// Return the local ephemeral public key for the negotiated suite, generating it if needed.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested encryption suite does not define an ephemeral key type
    /// supported by this implementation.
    pub fn local_ephemeral_public(
        &mut self,
        suite: &EncryptionSuite,
    ) -> Result<Vec<u8>, HandshakeError> {
        match suite_ephemeral_mechanism(suite)? {
            EphemeralMechanism::X25519 => {
                let eph = self.ensure_x25519_ephemeral();
                Ok(eph.public.to_vec())
            }
            EphemeralMechanism::Kyber768 { fingerprint } => {
                self.kyber_encapsulate(suite, fingerprint)
            }
        }
    }

    /// Override the locally generated X25519 ephemeral key with a deterministic value.
    ///
    /// Returns the derived public key so callers can advertise it in outbound `KeyUpdate`
    /// frames.
    pub fn set_local_ephemeral_x25519(&mut self, secret_bytes: [u8; 32]) -> Vec<u8> {
        let secret = StaticSecret::from(secret_bytes);
        let eph = X25519Ephemeral::from_secret(secret);
        let public = eph.public.to_vec();
        self.local_ephemeral = Some(EphemeralState::X25519(eph));
        public
    }

    /// Configure the remote Kyber public key expected for HPKE handshakes.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::InvalidKyberPublicKeyLength`] if the key length is unexpected,
    /// [`HandshakeError::InvalidKyberPublicKey`] when the bytes fail validation, and
    /// [`HandshakeError::KyberFingerprintMismatch`] if the fingerprint does not match the expected
    /// value.
    pub fn set_kyber_remote_public(
        &mut self,
        expected_fingerprint: Hash,
        public_key: &[u8],
    ) -> Result<(), HandshakeError> {
        let expected_len = self.kem_suite.public_key_len();
        if public_key.len() != expected_len {
            return Err(HandshakeError::InvalidKyberPublicKeyLength {
                expected: expected_len,
                found: public_key.len(),
            });
        }
        self.kem_suite
            .validate_public_key(public_key)
            .map_err(|_| HandshakeError::InvalidKyberPublicKey)?;
        let fingerprint = fingerprint_kyber_public(public_key, self.kem_suite);
        if fingerprint != expected_fingerprint {
            return Err(HandshakeError::KyberFingerprintMismatch {
                expected: expected_fingerprint,
                found: fingerprint,
            });
        }
        self.kyber_remote_public = Some(public_key.to_vec());
        self.kyber_remote_fingerprint = Some(fingerprint);
        Ok(())
    }

    /// Configure the local Kyber secret key used to decapsulate remote HPKE payloads.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::InvalidKyberSecretKeyLength`] if the key length is unexpected and
    /// [`HandshakeError::InvalidKyberSecretKey`] when the bytes fail validation.
    pub fn set_kyber_local_secret(&mut self, secret_key: &[u8]) -> Result<(), HandshakeError> {
        let expected_len = self.kem_suite.secret_key_len();
        if secret_key.len() != expected_len {
            return Err(HandshakeError::InvalidKyberSecretKeyLength {
                expected: expected_len,
                found: secret_key.len(),
            });
        }
        self.kem_suite
            .validate_secret_key(secret_key)
            .map_err(|_| HandshakeError::InvalidKyberSecretKey)?;
        self.kyber_local_secret = Some(Zeroizing::new(secret_key.to_vec()));
        Ok(())
    }

    /// Construct a signed `KeyUpdate` frame for the provided session parameters.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::UnsupportedAlgorithm`] if the signer is not Ed25519 and propagates
    /// failures from [`StreamingSession::local_ephemeral_public`].
    pub fn build_key_update(
        &mut self,
        session_id: Hash,
        suite: &EncryptionSuite,
        protocol_version: u16,
        key_counter: u64,
        signer: &PrivateKey,
    ) -> Result<KeyUpdate, HandshakeError> {
        if signer.algorithm() != Algorithm::Ed25519 {
            return Err(HandshakeError::UnsupportedAlgorithm(signer.algorithm()));
        }
        self.record_outbound_key_update(session_id, suite, key_counter);
        let pub_ephemeral = self.local_ephemeral_public(suite)?;
        let mut frame = KeyUpdate {
            session_id,
            suite: *suite,
            protocol_version,
            pub_ephemeral,
            key_counter,
            signature: [0u8; 64],
        };
        let transcript = key_update_transcript_bytes(&frame)?;
        let signature = Signature::new(signer, &transcript);
        frame.signature.copy_from_slice(signature.payload());
        Ok(frame)
    }

    fn record_outbound_key_update(
        &mut self,
        session_id: Hash,
        suite: &EncryptionSuite,
        key_counter: u64,
    ) {
        if self.session_id != Some(session_id) {
            self.reset_for_new_session();
            self.session_id = Some(session_id);
        }
        let negotiated_suite = self.key_state.suite().copied();
        self.key_state.restore(Some(key_counter), negotiated_suite);
        self.suite = Some(*suite);
    }

    /// Process a remote `KeyUpdate` frame, verifying its signature and updating transport keys.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::UnsupportedAlgorithm`] when the remote identity uses a non-Ed25519
    /// algorithm, [`HandshakeError::BadSignature`] if signature verification fails, and propagates
    /// other [`HandshakeError`] variants emitted while recording the frame or deriving keys.
    pub fn process_remote_key_update(
        &mut self,
        frame: &KeyUpdate,
        remote_identity: &PublicKey,
    ) -> Result<&TransportKeys, HandshakeError> {
        let message = key_update_transcript_bytes(frame)?;
        let (algorithm, pk_bytes) = remote_identity.to_bytes();
        if algorithm != Algorithm::Ed25519 {
            return Err(HandshakeError::UnsupportedAlgorithm(algorithm));
        }
        let verifying_key =
            Ed25519Sha512::parse_public_key(pk_bytes).map_err(|_| HandshakeError::BadSignature)?;
        Ed25519Sha512::verify(&message, frame.signature.as_slice(), &verifying_key)
            .map_err(|_| HandshakeError::BadSignature)?;

        let session_changed = self.session_id != Some(frame.session_id);
        if session_changed {
            self.reset_for_new_session();
            self.session_id = Some(frame.session_id);
        }

        let suite = frame.suite;

        let shared_secret_bytes = match suite_ephemeral_mechanism(&suite)? {
            EphemeralMechanism::X25519 => {
                const X25519_PUBLIC_LEN: usize = 32;
                let state = self.ensure_x25519_ephemeral();
                let remote_bytes: [u8; X25519_PUBLIC_LEN] =
                    frame.pub_ephemeral.as_slice().try_into().map_err(|_| {
                        StreamingCryptoError::InvalidEphemeralPublicKey {
                            expected: X25519_PUBLIC_LEN,
                            found: frame.pub_ephemeral.len(),
                        }
                    })?;
                let remote = X25519PublicKey::from(remote_bytes);
                state.shared_secret(&remote)
            }
            EphemeralMechanism::Kyber768 { fingerprint } => self
                .kyber_shared_secret_from_ciphertext(fingerprint, frame.pub_ephemeral.as_slice())?,
        };

        self.key_state.record(frame)?;
        self.apply_shared_secret_for_suite(&suite, &shared_secret_bytes)?;

        Ok(self.transport_keys.as_ref().expect("transport keys set"))
    }

    /// Process a remote `ContentKeyUpdate` frame, enforcing counter monotonicity and returning the
    /// decrypted GCK bytes.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::SuiteNotNegotiated`] if no content-encryption suite has been
    /// recorded, [`HandshakeError::MissingTransportKeys`] when the key update handshake has not yet
    /// derived transport keys, [`HandshakeError::MalformedWrappedKey`] when the wrapped payload is
    /// shorter than the suite nonce, or [`HandshakeError::Crypto`] if the GCK cannot be unwrapped.
    pub fn process_content_key_update(
        &mut self,
        frame: &ContentKeyUpdate,
    ) -> Result<Vec<u8>, HandshakeError> {
        let suite = self.suite.ok_or(HandshakeError::SuiteNotNegotiated)?;
        let transport = self
            .transport_keys
            .as_ref()
            .ok_or(HandshakeError::MissingTransportKeys)?;
        let nonce_len = streaming_crypto::nonce_len_for_suite(&suite);
        if frame.gck_wrapped.len() < nonce_len {
            return Err(HandshakeError::MalformedWrappedKey {
                expected: nonce_len,
                found: frame.gck_wrapped.len(),
            });
        }

        self.content_state.record(frame)?;

        let (nonce, ciphertext) = frame.gck_wrapped.split_at(nonce_len);
        let gck = streaming_crypto::unwrap_gck(
            &suite,
            &transport.recv,
            nonce,
            ciphertext,
            frame.content_key_id,
            frame.valid_from_segment,
        )?;
        self.latest_gck = Some(gck.clone());
        Ok(gck)
    }

    /// Construct a `ContentKeyUpdate` for the provided plaintext GCK and rotation metadata.
    ///
    /// # Errors
    ///
    /// Returns [`HandshakeError::SuiteNotNegotiated`] if no suite has been negotiated,
    /// [`HandshakeError::MissingTransportKeys`] when transport keys are unavailable, and
    /// propagates lower-level crypto/material validation errors.
    pub fn build_content_key_update(
        &mut self,
        gck_plaintext: &[u8],
        content_key_id: u64,
        valid_from_segment: u64,
    ) -> Result<ContentKeyUpdate, HandshakeError> {
        let suite = self.suite.ok_or(HandshakeError::SuiteNotNegotiated)?;
        let transport = self
            .transport_keys
            .as_ref()
            .ok_or(HandshakeError::MissingTransportKeys)?;
        let nonce_len = streaming_crypto::nonce_len_for_suite(&suite);
        let mut nonce = vec![0u8; nonce_len];
        let mut rng = rng::os_rng();
        rng.fill_bytes(&mut nonce);
        let gck_wrapped = streaming_crypto::wrap_gck(
            &suite,
            &transport.send,
            &nonce,
            gck_plaintext,
            content_key_id,
            valid_from_segment,
        )?;
        let update = ContentKeyUpdate {
            content_key_id,
            gck_wrapped,
            valid_from_segment,
        };
        self.content_state.record(&update)?;
        self.latest_gck = Some(gck_plaintext.to_vec());
        Ok(update)
    }

    fn ensure_x25519_ephemeral(&mut self) -> &mut X25519Ephemeral {
        let state = self
            .local_ephemeral
            .get_or_insert_with(|| EphemeralState::X25519(X25519Ephemeral::new_random()));
        match state {
            EphemeralState::X25519(inner) => inner,
        }
    }

    fn kyber_encapsulate(
        &mut self,
        suite: &EncryptionSuite,
        _fingerprint: &Hash,
    ) -> Result<Vec<u8>, HandshakeError> {
        let public_bytes = self
            .kyber_remote_public
            .as_ref()
            .ok_or(HandshakeError::MissingKyberRemotePublic)?;
        self.kem_suite
            .validate_public_key(public_bytes.as_slice())
            .map_err(|_| HandshakeError::InvalidKyberPublicKey)?;
        let (shared_secret, ciphertext) =
            encapsulate_mlkem(self.kem_suite, public_bytes.as_slice())
                .map_err(|_| HandshakeError::InvalidKyberPublicKey)?;
        debug_assert_eq!(
            shared_secret.as_bytes().len(),
            self.kem_suite.shared_secret_len()
        );
        debug_assert_eq!(ciphertext.as_bytes().len(), self.kem_suite.ciphertext_len());
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(shared_secret.as_bytes());
        self.apply_shared_secret_for_suite(suite, &secret_bytes)?;
        Ok(ciphertext.as_bytes().to_vec())
    }

    fn kyber_shared_secret_from_ciphertext(
        &self,
        fingerprint: &Hash,
        ciphertext: &[u8],
    ) -> Result<[u8; 32], HandshakeError> {
        let stored_fingerprint = self
            .kyber_remote_fingerprint
            .ok_or(HandshakeError::MissingKyberRemotePublic)?;
        if stored_fingerprint != *fingerprint {
            return Err(HandshakeError::KyberFingerprintMismatch {
                expected: stored_fingerprint,
                found: *fingerprint,
            });
        }
        let secret_bytes = self
            .kyber_local_secret
            .as_ref()
            .ok_or(HandshakeError::MissingKyberLocalSecret)?;
        let expected_len = self.kem_suite.ciphertext_len();
        if ciphertext.len() != expected_len {
            return Err(HandshakeError::InvalidEphemeralPublicKey {
                expected: expected_len,
                found: ciphertext.len(),
            });
        }
        self.kem_suite
            .validate_secret_key(secret_bytes.as_ref())
            .map_err(|_| HandshakeError::InvalidKyberSecretKey)?;
        self.kem_suite
            .validate_ciphertext(ciphertext)
            .map_err(|_| HandshakeError::InvalidKyberCiphertext)?;
        let shared = decapsulate_mlkem(self.kem_suite, secret_bytes.as_ref(), ciphertext)
            .map_err(|_| HandshakeError::InvalidKyberCiphertext)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(shared.as_bytes());
        Ok(out)
    }

    fn apply_shared_secret_for_suite(
        &mut self,
        suite: &EncryptionSuite,
        shared: &[u8; 32],
    ) -> Result<(), HandshakeError> {
        let sts_root = streaming_crypto::derive_sts_root(shared)?;
        let transport =
            streaming_crypto::derive_transport_keys_from_sts_root(&sts_root, self.role)?;
        self.suite = Some(*suite);
        self.sts_root = Some(sts_root);
        self.transport_keys = Some(transport);
        Ok(())
    }
}

impl FeedbackState {
    fn snapshot(&self) -> FeedbackStateSnapshot {
        FeedbackStateSnapshot {
            parity_chunks: self.parity_chunks,
            hints_received: self.hints_received,
            reports_received: self.reports_received,
            loss_ewma_q16: self.loss_ewma_q16,
            last_delivered_sequence: self.last_delivered_sequence,
            latest_parity_applied: self.latest_parity_applied,
            latest_fec_budget: self.latest_fec_budget,
            report_interval_ms: self.report_interval_ms,
            latency_gradient_q16: self.latency_gradient_q16,
            observed_rtt_ms: self.observed_rtt_ms,
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ewma_update(current: u32, sample: u32) -> u32 {
    if current == sample {
        return current;
    }
    let diff = i64::from(sample) - i64::from(current);
    let update = ((diff * i64::from(FEEDBACK_ALPHA_FP)) + (1 << (FEEDBACK_FP_SHIFT - 1)))
        >> FEEDBACK_FP_SHIFT;
    let mut next = i64::from(current) + update;
    if next < 0 {
        next = 0;
    }
    next.clamp(0, i64::from(u32::MAX)) as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn loss_percent_to_q16(loss_percent_x100: u16) -> Option<u32> {
    let ratio = f64::from(loss_percent_x100) / 10_000.0;
    if !ratio.is_finite() || ratio.is_sign_negative() {
        return None;
    }
    let scaled = (ratio * f64::from(1u32 << FEEDBACK_FP_SHIFT)).round();
    if scaled < 0.0 {
        Some(0)
    } else if scaled > f64::from(u32::MAX) {
        Some(u32::MAX)
    } else {
        Some(scaled as u32)
    }
}

#[allow(clippy::cast_possible_truncation)]
fn parity_from_loss_fp(loss_fp: u32) -> u8 {
    let scaled = (u64::from(loss_fp) * 5) / 4;
    let adjusted = (scaled + u64::from(FEEDBACK_OFFSET_FP)) * u64::from(FEC_WINDOW_CHUNKS);
    let parity = ((adjusted + u64::from(FEEDBACK_CEIL_BIAS)) >> FEEDBACK_FP_SHIFT)
        .min(u64::from(MAX_PARITY_CHUNKS));
    parity as u8
}

/// Canonical Norito transcript used for signing and verifying `KeyUpdate` frames.
#[derive(Clone, norito::derive::NoritoSerialize)]
struct KeyUpdateTranscript {
    session_id: Hash,
    suite: EncryptionSuite,
    protocol_version: u16,
    pub_ephemeral: Vec<u8>,
    key_counter: u64,
}

/// Produce the canonical byte representation of a `KeyUpdate` frame used for signing.
///
/// # Errors
///
/// Returns [`norito::Error`] if the transcript cannot be serialized with the Norito codec.
pub fn key_update_transcript_bytes(frame: &KeyUpdate) -> Result<Vec<u8>, norito::Error> {
    let payload = KeyUpdateTranscript {
        session_id: frame.session_id,
        suite: frame.suite,
        protocol_version: frame.protocol_version,
        pub_ephemeral: frame.pub_ephemeral.clone(),
        key_counter: frame.key_counter,
    };
    norito::to_bytes(&payload)
}
