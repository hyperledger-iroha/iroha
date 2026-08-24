//! Fixed-suite public primitives for Iroha threshold-BLS sessions.
//!
//! This module deliberately does not place threshold secret shares in
//! [`crate::PrivateKey`] or [`crate::KeyPair`]. It validates the immutable
//! public/session surface shared by the randomness beacon and Parliament
//! timelock release protocol, while keeping their keys distinct at the Rust
//! type level.
//!
//! [`AdaptiveThresholdBlsPublicTranscript`] implements Iroha's fixed profile of
//! the three-polynomial Das--Ren construction: coefficient commitments are
//! `g^s h^r v^u` with unblinded constants, partial signatures are
//! `H0(m)^s H1(m)^r`, and each partial carries the paper's two-equation
//! representation proof. The Fiat--Shamir transcript contains every paper
//! statement element `(x, y, pk_i, sigma_i, m)` plus the typed Iroha session,
//! qualified-transcript, participant-seat, and parameter bindings.
//!
//! # Security model and limits
//!
//! “Adaptive” in public type names identifies this protocol profile; it is not
//! a claim of adaptive security under generic or standard assumptions. The
//! original [Das--Ren analysis](https://eprint.iacr.org/2023/1553) is in the
//! random-oracle model and relies on the structure of its three correlated
//! polynomials and independent generators. Subsequent
//! [Ciampi--Crites--Komlo--Maller work](https://eprint.iacr.org/2025/943)
//! rules out broad classes of full-adaptive reductions for *key-unique*
//! threshold signatures. This profile deliberately has non-key-unique public
//! shares `g^s h^r v^u`, so accepting the representation proof on every
//! partial is mandatory. Its reconstructed standard BLS signature remains
//! unique and is tested to be independent of the threshold subset.
//!
//! V1 fixes `n = 3f + 1`, signing threshold `f + 1`, and a lifetime budget of
//! at most `f` distinct exposed signing shares for one unrefreshed key session.
//! It implements no proactive/mobile-adversary refresh: exceeding that
//! cumulative budget requires a fresh DKG and a purpose-distinct new session.
//! Secret buffers are zeroizing defense in depth, not a compiler/OS/hardware
//! erasure guarantee. The surrounding protocol remains responsible for
//! authenticated private-share transport, complaint deadlines, qualified-set
//! agreement, retirement of dealer contributions and proof nonces, and key
//! rotation. [`AdaptiveThresholdBlsPublicTranscript::ensure_adaptive_protocol_ready`]
//! attests only that these public cryptographic checks ran; it is not a theorem
//! certificate or an operational release approval. The older
//! [`ThresholdBlsPublicTranscript`] lacks even that evidence and remains fail
//! closed for this readiness check.

use core::{fmt, marker::PhantomData};
use std::{collections::HashSet, vec::Vec};

use blstrs::{G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Scalar};
use group::{Curve as _, Group as _, ff::Field as _, prime::PrimeCurveAffine as _};
use hkdf::Hkdf;
use pairing::{MillerLoopResult as _, MultiMillerLoop as _};
use rand_core::{OsRng, TryCryptoRng};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

/// Version of the fixed threshold-BLS transcript profile.
pub const THRESHOLD_BLS_PROTOCOL_VERSION_V1: u16 = 1;
/// Minimum exact `3f + 1` committee size accepted by the v1 profile.
pub const THRESHOLD_BLS_MIN_COMMITTEE_SIZE_V1: u16 = 4;
/// Maximum exact `3f + 1` committee size accepted by the v1 profile.
pub const THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1: u16 = 31;
/// Maximum caller payload incorporated into one signing transcript.
pub const THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1: usize = 16 * 1024;
/// V1 has no proactive share-refresh protocol; rotate through a fresh DKG instead.
pub const THRESHOLD_BLS_PROACTIVE_REFRESH_SUPPORTED_V1: bool = false;
/// Canonical compressed width of a public key in G2.
pub const THRESHOLD_BLS_PUBLIC_KEY_BYTES: usize = 96;
/// Canonical compressed width of a signature in G1.
pub const THRESHOLD_BLS_SIGNATURE_BYTES: usize = 48;
/// Canonical `(s, r, u)` coefficient bytes accepted only inside zeroizing builders.
pub type DasRenSecretCoefficientV1 = [[u8; 32]; 3];
/// RFC 9380 hash-to-G1 DST for the global randomness beacon.
pub const BEACON_SIGNATURE_DST_V1: &[u8] =
    b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL:IROHA-BEACON-V1_";
/// RFC 9380 hash-to-G1 DST for Parliament timelock release identities.
pub const TLE_RELEASE_SIGNATURE_DST_V1: &[u8] =
    b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL:IROHA-TLE-RELEASE-V1_";

const SESSION_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.session.v1\0";
const MESSAGE_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.message.v1\0";
const PUBLIC_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.public-transcript.v1\0";
const BEACON_SEED_SALT_V1: &[u8] = b"iroha.threshold-bls.beacon-seed.salt.v1\0";
const BEACON_SEED_INFO_V1: &[u8] = b"iroha.threshold-bls.beacon-seed.hkdf-sha256.v1\0";
const ADAPTIVE_PARAMETERS_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.adaptive-parameters.v1\0";
const ADAPTIVE_H_DST_V1: &[u8] = b"IROHA-THRESHOLD-BLS-DKG-H_BLS12381G2_XMD:SHA-256_SSWU_RO_V1_";
const ADAPTIVE_V_DST_V1: &[u8] = b"IROHA-THRESHOLD-BLS-DKG-V_BLS12381G2_XMD:SHA-256_SSWU_RO_V1_";
const DEALER_POK_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.das-ren.dealer-pok.v1\0";
const ADAPTIVE_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.das-ren.public-transcript.v1\0";
const ADAPTIVE_PARTICIPANT_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.das-ren.participant-seat.v1\0";
const PARTIAL_PROOF_DOMAIN_V1: &[u8] = b"iroha.threshold-bls.das-ren.partial-proof.v1\0";
const PARTIAL_H1_DST_V1: &[u8] =
    b"IROHA-THRESHOLD-BLS-PARTIAL-H1_BLS12381G1_XMD:SHA-256_SSWU_RO_V1_";
const SCALAR_REJECTION_LIMIT: u32 = u16::MAX as u32;

mod sealed {
    pub trait Sealed {}
}

/// Type-level role for a fixed threshold-BLS ciphersuite.
pub trait ThresholdBlsPurpose: sealed::Sealed + Copy + fmt::Debug + Eq + 'static {
    /// Canonical one-byte role tag included in every session transcript.
    const ROLE_TAG: u8;
    /// Exact RFC 9380 hash-to-G1 domain separator for this role.
    const SIGNATURE_DST: &'static [u8];
}

/// Type-level marker for the global randomness-beacon key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BeaconPurpose;

impl sealed::Sealed for BeaconPurpose {}

impl ThresholdBlsPurpose for BeaconPurpose {
    const ROLE_TAG: u8 = 1;
    const SIGNATURE_DST: &'static [u8] = BEACON_SIGNATURE_DST_V1;
}

/// Type-level marker for the Parliament timelock-release key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TleReleasePurpose;

impl sealed::Sealed for TleReleasePurpose {}

impl ThresholdBlsPurpose for TleReleasePurpose {
    const ROLE_TAG: u8 = 2;
    const SIGNATURE_DST: &'static [u8] = TLE_RELEASE_SIGNATURE_DST_V1;
}

/// Public validation errors for the fixed threshold-BLS profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ThresholdBlsError {
    /// A protocol binding that must be a digest was the all-zero placeholder.
    #[error("threshold-BLS binding digest must be non-zero")]
    ZeroBinding,
    /// The committee must contain exactly `3f + 1` validators in the supported range.
    #[error("threshold-BLS committee must be an exact supported 3f + 1 committee")]
    InvalidCommitteeSize,
    /// The v1 threshold must be exactly `f + 1`.
    #[error("threshold-BLS threshold must equal f + 1")]
    InvalidThreshold,
    /// A signing payload exceeded the fixed v1 admission bound.
    #[error("threshold-BLS signing payload exceeds the v1 bound")]
    MessageTooLarge,
    /// Public-key bytes were not one canonical, subgroup-checked, non-identity G2 point.
    #[error("threshold-BLS public key must be a canonical non-identity G2 point")]
    InvalidPublicKey,
    /// Signature bytes were not one canonical, subgroup-checked, non-identity G1 point.
    #[error("threshold-BLS signature must be a canonical non-identity G1 point")]
    InvalidSignature,
    /// A participant index was zero or outside the frozen committee.
    #[error("threshold-BLS participant index is outside the frozen committee")]
    InvalidParticipantIndex,
    /// Public shares were not the complete canonical index sequence `1..=n`.
    #[error("threshold-BLS public shares must contain each canonical index exactly once")]
    NonCanonicalShareSet,
    /// Two public shares named the same frozen participant identity.
    #[error("threshold-BLS public transcript contains a duplicate participant")]
    DuplicateParticipant,
    /// A key, share, or signature was created for another session.
    #[error("threshold-BLS object is bound to another session")]
    SessionMismatch,
    /// A BLS signature failed pairing verification.
    #[error("threshold-BLS signature verification failed")]
    SignatureMismatch,
    /// The caller requested a public share which is not in the frozen transcript.
    #[error("threshold-BLS public share is not in the frozen transcript")]
    UnknownParticipant,
    /// The transcript lacks the complete triple-generator DKG/signing evidence.
    #[error("legacy threshold-BLS transcript lacks verified adaptive DKG evidence")]
    AdaptiveProtocolNotReady,
    /// HKDF expansion failed for a fixed-size output.
    #[error("threshold-BLS HKDF expansion failed")]
    HkdfExpand,
    /// A DKG or proof scalar was not canonically encoded.
    #[error("adaptive threshold-BLS scalar is not canonical")]
    InvalidScalar,
    /// Deterministic hash-to-scalar rejection sampling exhausted its bound.
    #[error("adaptive threshold-BLS hash-to-scalar derivation failed")]
    ScalarDerivation,
    /// The independent Pedersen generators were malformed or degenerate.
    #[error("adaptive threshold-BLS generators are invalid")]
    InvalidAdaptiveGenerator,
    /// A dealer coefficient commitment was malformed or used the wrong session.
    #[error("adaptive threshold-BLS coefficient commitment is invalid")]
    InvalidCoefficientCommitment,
    /// A dealer's constant-coefficient Schnorr proof failed.
    #[error("adaptive threshold-BLS dealer constant proof failed")]
    InvalidDealerProof,
    /// A revealed complaint response did not match the dealer polynomial.
    #[error("adaptive threshold-BLS complaint response failed verification")]
    InvalidComplaintResponse,
    /// Imported aggregate secret components did not match the public share.
    #[error("adaptive threshold-BLS secret share does not match its public commitment")]
    SecretShareMismatch,
    /// Qualified dealers were missing, duplicated, reordered, or outside the committee.
    #[error("adaptive threshold-BLS qualified dealer set is not canonical")]
    NonCanonicalQualifiedSet,
    /// A partial signature proof was malformed or failed its equations.
    #[error("adaptive threshold-BLS partial signature proof failed")]
    InvalidPartialSignatureProof,
    /// Partial signatures were insufficient, duplicated, reordered, or out of range.
    #[error("adaptive threshold-BLS partial signature set is not canonical")]
    NonCanonicalPartialSignatureSet,
    /// Fallible cryptographic randomness failed.
    #[error("adaptive threshold-BLS CSPRNG failed")]
    RandomnessUnavailable,
    /// Repeated random bytes did not produce a nonzero field scalar.
    #[error("adaptive threshold-BLS CSPRNG returned inert scalar material")]
    InertRandomness,
}

/// Immutable bindings for one beacon or TLE threshold-BLS session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThresholdBlsSession<P: ThresholdBlsPurpose> {
    network_id: [u8; 32],
    session_id: [u8; 32],
    roster_hash: [u8; 32],
    committee_size: u16,
    threshold: u16,
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> ThresholdBlsSession<P> {
    /// Construct and validate one immutable v1 session.
    ///
    /// The committee must be in `4..=31`, have exact shape `3f + 1`, and use
    /// threshold `f + 1`. All digest bindings reject the all-zero placeholder.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] when a binding or committee parameter is invalid.
    pub fn new(
        network_id: [u8; 32],
        session_id: [u8; 32],
        roster_hash: [u8; 32],
        committee_size: u16,
        threshold: u16,
    ) -> Result<Self, ThresholdBlsError> {
        if [network_id, session_id, roster_hash]
            .iter()
            .any(|binding| is_zero(binding))
        {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        if !(THRESHOLD_BLS_MIN_COMMITTEE_SIZE_V1..=THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1)
            .contains(&committee_size)
            || committee_size % 3 != 1
        {
            return Err(ThresholdBlsError::InvalidCommitteeSize);
        }
        let fault_tolerance = (committee_size - 1) / 3;
        if threshold != fault_tolerance + 1 {
            return Err(ThresholdBlsError::InvalidThreshold);
        }
        Ok(Self {
            network_id,
            session_id,
            roster_hash,
            committee_size,
            threshold,
            marker: PhantomData,
        })
    }

    /// Return the canonical network/genesis binding.
    #[must_use]
    pub const fn network_id(&self) -> &[u8; 32] {
        &self.network_id
    }

    /// Return the unique session identifier.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }

    /// Return the hash of the frozen ordered validator roster.
    #[must_use]
    pub const fn roster_hash(&self) -> &[u8; 32] {
        &self.roster_hash
    }

    /// Return the exact frozen committee size.
    #[must_use]
    pub const fn committee_size(&self) -> u16 {
        self.committee_size
    }

    /// Return the exact `f + 1` signature threshold.
    #[must_use]
    pub const fn threshold(&self) -> u16 {
        self.threshold
    }

    /// Return the lifetime bound on distinct exposed shares without key rotation.
    ///
    /// V1 has no proactive refresh. This `f = threshold - 1` budget is
    /// cumulative for the complete key-session lifetime, not reset per block,
    /// epoch, payload, or signing round.
    #[must_use]
    pub const fn maximum_distinct_share_exposures_without_rotation(&self) -> u16 {
        self.threshold - 1
    }

    /// Build the exact byte string hashed to G1 for a caller payload.
    ///
    /// Fixed-width fields are big-endian. The payload is prefixed by a
    /// big-endian `u32` length, making the transcript fully consuming and
    /// independent of serializer settings.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError::MessageTooLarge`] when `payload` exceeds
    /// the fixed v1 bound.
    pub fn signing_message(&self, payload: &[u8]) -> Result<Vec<u8>, ThresholdBlsError> {
        let payload_len = u32::try_from(payload.len())
            .ok()
            .filter(|_| payload.len() <= THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1)
            .ok_or(ThresholdBlsError::MessageTooLarge)?;
        let mut message = Vec::with_capacity(
            MESSAGE_DOMAIN_V1.len()
                + SESSION_DOMAIN_V1.len()
                + 2
                + 1
                + 32 * 3
                + 2 * 2
                + 4
                + payload.len(),
        );
        message.extend_from_slice(MESSAGE_DOMAIN_V1);
        self.write_canonical(&mut message);
        message.extend_from_slice(&payload_len.to_be_bytes());
        message.extend_from_slice(payload);
        Ok(message)
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(SESSION_DOMAIN_V1);
        out.extend_from_slice(&THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
        out.push(P::ROLE_TAG);
        out.extend_from_slice(&self.network_id);
        out.extend_from_slice(&self.session_id);
        out.extend_from_slice(&self.roster_hash);
        out.extend_from_slice(&self.committee_size.to_be_bytes());
        out.extend_from_slice(&self.threshold.to_be_bytes());
    }
}

/// A canonical G2 group public key bound to one typed session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThresholdBlsPublicKey<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    bytes: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for ThresholdBlsPublicKey<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThresholdBlsPublicKey")
            .field("purpose", &P::ROLE_TAG)
            .field("session_id", &hex::encode(self.session_id))
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> ThresholdBlsPublicKey<P> {
    /// Parse a canonical, subgroup-checked, non-identity G2 public key.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError::InvalidPublicKey`] for a wrong length or
    /// invalid point and [`ThresholdBlsError::ZeroBinding`] for a zero session ID.
    pub fn from_bytes(session_id: [u8; 32], bytes: &[u8]) -> Result<Self, ThresholdBlsError> {
        if is_zero(&session_id) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        let encoded: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| ThresholdBlsError::InvalidPublicKey)?;
        decode_g2(&encoded)?;
        Ok(Self {
            session_id,
            bytes: encoded,
            marker: PhantomData,
        })
    }

    /// Return the unique session binding carried by this key.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }

    /// Return the canonical compressed G2 encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.bytes
    }

    /// Verify one typed final signature over a session-framed payload.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid payload, session mismatch,
    /// malformed signature, or failed pairing.
    pub fn verify_payload(
        &self,
        session: &ThresholdBlsSession<P>,
        payload: &[u8],
        signature: &ThresholdBlsSignature<P>,
    ) -> Result<(), ThresholdBlsError> {
        if self.session_id != *session.session_id() || signature.session_id != *session.session_id()
        {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        let message = session.signing_message(payload)?;
        verify_signature::<P>(&self.bytes, &message, &signature.bytes)
    }

    pub(crate) fn decode(&self) -> Result<G2Affine, ThresholdBlsError> {
        decode_g2(&self.bytes)
    }
}

/// Public key share for one frozen participant and typed session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThresholdBlsPublicShare<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    index: u16,
    participant_hash: [u8; 32],
    bytes: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for ThresholdBlsPublicShare<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThresholdBlsPublicShare")
            .field("purpose", &P::ROLE_TAG)
            .field("session_id", &hex::encode(self.session_id))
            .field("index", &self.index)
            .field("participant_hash", &hex::encode(self.participant_hash))
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> ThresholdBlsPublicShare<P> {
    /// Parse and bind one public verification share.
    ///
    /// Committee-range validation happens when the share enters a full public
    /// transcript, because the share itself does not carry `n`.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for index zero, zero bindings, or an invalid G2 point.
    pub fn from_bytes(
        session_id: [u8; 32],
        index: u16,
        participant_hash: [u8; 32],
        bytes: &[u8],
    ) -> Result<Self, ThresholdBlsError> {
        if index == 0 {
            return Err(ThresholdBlsError::InvalidParticipantIndex);
        }
        if is_zero(&session_id) || is_zero(&participant_hash) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        let encoded: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| ThresholdBlsError::InvalidPublicKey)?;
        decode_g2(&encoded)?;
        Ok(Self {
            session_id,
            index,
            participant_hash,
            bytes: encoded,
            marker: PhantomData,
        })
    }

    /// Return the fixed one-based DKG participant index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the frozen participant-identity hash.
    #[must_use]
    pub const fn participant_hash(&self) -> &[u8; 32] {
        &self.participant_hash
    }

    /// Return the canonical compressed G2 verification-share encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.bytes
    }
}

/// A canonical G1 final signature bound to one typed session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThresholdBlsSignature<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    bytes: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for ThresholdBlsSignature<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThresholdBlsSignature")
            .field("purpose", &P::ROLE_TAG)
            .field("session_id", &hex::encode(self.session_id))
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> ThresholdBlsSignature<P> {
    /// Parse a canonical, subgroup-checked, non-identity G1 signature.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a zero session binding or invalid point.
    pub fn from_bytes(session_id: [u8; 32], bytes: &[u8]) -> Result<Self, ThresholdBlsError> {
        if is_zero(&session_id) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        let encoded: [u8; THRESHOLD_BLS_SIGNATURE_BYTES] = bytes
            .try_into()
            .map_err(|_| ThresholdBlsError::InvalidSignature)?;
        decode_g1(&encoded)?;
        Ok(Self {
            session_id,
            bytes: encoded,
            marker: PhantomData,
        })
    }

    /// Return the canonical compressed G1 encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_SIGNATURE_BYTES] {
        &self.bytes
    }

    /// Return the unique session binding carried by this signature.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }
}

/// One canonical G1 signature share bound to a fixed participant index.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThresholdBlsSignatureShare<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    index: u16,
    bytes: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for ThresholdBlsSignatureShare<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThresholdBlsSignatureShare")
            .field("purpose", &P::ROLE_TAG)
            .field("session_id", &hex::encode(self.session_id))
            .field("index", &self.index)
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> ThresholdBlsSignatureShare<P> {
    /// Parse one canonical signature share.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for index zero, a zero session binding,
    /// or an invalid G1 point.
    pub fn from_bytes(
        session_id: [u8; 32],
        index: u16,
        bytes: &[u8],
    ) -> Result<Self, ThresholdBlsError> {
        if index == 0 {
            return Err(ThresholdBlsError::InvalidParticipantIndex);
        }
        if is_zero(&session_id) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        let encoded: [u8; THRESHOLD_BLS_SIGNATURE_BYTES] = bytes
            .try_into()
            .map_err(|_| ThresholdBlsError::InvalidSignature)?;
        decode_g1(&encoded)?;
        Ok(Self {
            session_id,
            index,
            bytes: encoded,
            marker: PhantomData,
        })
    }

    /// Return the immutable one-based participant index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the canonical compressed G1 signature-share encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_SIGNATURE_BYTES] {
        &self.bytes
    }
}

/// Deterministically derived independent generators for the adaptive DKG.
///
/// `h` and `v` are hash-to-G2 outputs under distinct domain separators. No
/// party is given a discrete-log relation between either point and the standard
/// G2 generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdaptiveThresholdBlsParameters<P: ThresholdBlsPurpose> {
    session: ThresholdBlsSession<P>,
    h: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    v: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
}

impl<P: ThresholdBlsPurpose> AdaptiveThresholdBlsParameters<P> {
    /// Derive the fixed triple-generator parameters for one typed session.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError::InvalidAdaptiveGenerator`] if hash-to-curve
    /// unexpectedly yields an identity, duplicate, or standard generator.
    pub fn derive(session: &ThresholdBlsSession<P>) -> Result<Self, ThresholdBlsError> {
        let mut message = Vec::new();
        message.extend_from_slice(ADAPTIVE_PARAMETERS_DOMAIN_V1);
        session.write_canonical(&mut message);
        let h = G2Projective::hash_to_curve(&message, ADAPTIVE_H_DST_V1, &[]).to_affine();
        let v = G2Projective::hash_to_curve(&message, ADAPTIVE_V_DST_V1, &[]).to_affine();
        if bool::from(h.is_identity())
            || bool::from(v.is_identity())
            || h == v
            || h == G2Affine::generator()
            || v == G2Affine::generator()
        {
            return Err(ThresholdBlsError::InvalidAdaptiveGenerator);
        }
        Ok(Self {
            session: *session,
            h: h.to_compressed(),
            v: v.to_compressed(),
        })
    }

    /// Return the immutable typed threshold session.
    #[must_use]
    pub const fn session(&self) -> &ThresholdBlsSession<P> {
        &self.session
    }

    /// Return the canonical compressed independent `h` generator.
    #[must_use]
    pub const fn h_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.h
    }

    /// Return the canonical compressed independent `v` generator.
    #[must_use]
    pub const fn v_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.v
    }

    fn h_point(&self) -> Result<G2Affine, ThresholdBlsError> {
        decode_g2(&self.h).map_err(|_| ThresholdBlsError::InvalidAdaptiveGenerator)
    }

    fn v_point(&self) -> Result<G2Affine, ThresholdBlsError> {
        decode_g2(&self.v).map_err(|_| ThresholdBlsError::InvalidAdaptiveGenerator)
    }

    fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(ADAPTIVE_PARAMETERS_DOMAIN_V1);
        let mut session = Vec::new();
        self.session.write_canonical(&mut session);
        hasher.update(session);
        hasher.update(self.h);
        hasher.update(self.v);
        hasher.finalize().into()
    }
}

/// One canonical triple-generator polynomial coefficient commitment.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DasRenCoefficientCommitment<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    bytes: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for DasRenCoefficientCommitment<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DasRenCoefficientCommitment")
            .field("purpose", &P::ROLE_TAG)
            .field("parameters_digest", &hex::encode(self.parameters_digest))
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> DasRenCoefficientCommitment<P> {
    /// Parse a canonical non-identity G2 coefficient commitment.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError::InvalidCoefficientCommitment`] for a
    /// malformed, noncanonical, non-subgroup, or identity point.
    pub fn from_bytes(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        bytes: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    ) -> Result<Self, ThresholdBlsError> {
        decode_g2(&bytes).map_err(|_| ThresholdBlsError::InvalidCoefficientCommitment)?;
        Ok(Self {
            parameters_digest: parameters.digest(),
            bytes,
            marker: PhantomData,
        })
    }

    /// Return the canonical compressed commitment encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.bytes
    }

    fn point(&self) -> Result<G2Affine, ThresholdBlsError> {
        decode_g2(&self.bytes).map_err(|_| ThresholdBlsError::InvalidCoefficientCommitment)
    }
}

/// Canonical Schnorr proof of knowledge for a dealer's constant coefficient.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DasRenSchnorrPok<P: ThresholdBlsPurpose> {
    commitment: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    response: [u8; 32],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for DasRenSchnorrPok<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DasRenSchnorrPok")
            .field("purpose", &P::ROLE_TAG)
            .field("commitment", &hex::encode(self.commitment))
            .field("response", &hex::encode(self.response))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> DasRenSchnorrPok<P> {
    /// Parse a canonical G2 commitment and big-endian scalar response.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a malformed point or scalar.
    pub fn from_bytes(
        commitment: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
        response: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        decode_g2(&commitment).map_err(|_| ThresholdBlsError::InvalidDealerProof)?;
        decode_scalar(&response)?;
        Ok(Self {
            commitment,
            response,
            marker: PhantomData,
        })
    }

    /// Return the canonical proof commitment.
    #[must_use]
    pub const fn commitment_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.commitment
    }

    /// Return the canonical big-endian response scalar.
    #[must_use]
    pub const fn response_bytes(&self) -> &[u8; 32] {
        &self.response
    }
}

/// Namespace for verifying one dealer's complete adaptive DKG commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DasRenDealerCommitment<P: ThresholdBlsPurpose>(PhantomData<P>);

impl<P: ThresholdBlsPurpose> DasRenDealerCommitment<P> {
    /// Verify a dealer polynomial and constant-coefficient knowledge proof.
    ///
    /// The coefficient vector must have exact length `threshold`, so the
    /// sharing polynomial has degree `f`. The proof establishes knowledge of
    /// the standard-generator discrete log of the constant commitment; this
    /// prevents hidden `h`/`v` blinding from entering the group public key.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid dealer index, coefficient
    /// vector, scalar, session binding, or Schnorr equation.
    pub fn verify(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer_index: u16,
        coefficients: &[[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES]],
        constant_pok_commitment: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
        constant_pok_response: [u8; 32],
    ) -> Result<ValidatedDealerCommitment<P>, ThresholdBlsError> {
        validate_participant_index(parameters.session(), dealer_index)?;
        if coefficients.len() != usize::from(parameters.session().threshold()) {
            return Err(ThresholdBlsError::InvalidCoefficientCommitment);
        }
        let coefficients = coefficients
            .iter()
            .map(|bytes| DasRenCoefficientCommitment::from_bytes(parameters, *bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let proof =
            DasRenSchnorrPok::<P>::from_bytes(constant_pok_commitment, constant_pok_response)?;
        let challenge = dealer_pok_challenge(parameters, dealer_index, &coefficients, &proof)?;
        let response = decode_scalar(&proof.response)?;
        let lhs = (G2Projective::generator() * response).to_affine();
        let rhs = (G2Projective::from(decode_g2(&proof.commitment)?)
            + G2Projective::from(coefficients[0].point()?) * challenge)
            .to_affine();
        if lhs != rhs {
            return Err(ThresholdBlsError::InvalidDealerProof);
        }
        Ok(ValidatedDealerCommitment {
            parameters_digest: parameters.digest(),
            dealer_index,
            coefficients,
            proof,
        })
    }
}

/// Proof-validated commitment vector for one canonical DKG dealer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDealerCommitment<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    dealer_index: u16,
    coefficients: Vec<DasRenCoefficientCommitment<P>>,
    proof: DasRenSchnorrPok<P>,
}

impl<P: ThresholdBlsPurpose> ValidatedDealerCommitment<P> {
    /// Return the fixed one-based dealer index.
    #[must_use]
    pub const fn dealer_index(&self) -> u16 {
        self.dealer_index
    }

    /// Return the exact degree-`f` coefficient commitment vector.
    #[must_use]
    pub fn coefficients(&self) -> &[DasRenCoefficientCommitment<P>] {
        &self.coefficients
    }

    /// Return the verified constant-coefficient knowledge proof.
    #[must_use]
    pub const fn constant_proof(&self) -> &DasRenSchnorrPok<P> {
        &self.proof
    }
}

/// Non-cloneable, zeroizing owner of one dealer's `(s, r, u)` polynomials.
///
/// The type has no serialization or `Debug` implementation. Its constant
/// blinding coefficients are forced to zero, while the standard-generator
/// constant is nonzero and proven by Schnorr knowledge.
pub struct DasRenDealerSecret<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    session_id: [u8; 32],
    dealer_index: u16,
    coefficients: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> DasRenDealerSecret<P> {
    /// Generate one complete dealer polynomial using operating-system randomness.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid index, randomness failure,
    /// or an unexpected degenerate commitment.
    pub fn generate(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer_index: u16,
    ) -> Result<(Self, ValidatedDealerCommitment<P>), ThresholdBlsError> {
        Self::generate_with_rng(parameters, dealer_index, &mut OsRng)
    }

    /// Generate one complete dealer polynomial using an explicit CSPRNG.
    ///
    /// This entry point supports deterministic interoperability vectors. Live
    /// DKG dealers must use fresh, non-repeating CSPRNG state per session.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid index, randomness failure,
    /// or an unexpected degenerate commitment.
    pub fn generate_with_rng<R: TryCryptoRng + ?Sized>(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer_index: u16,
        rng: &mut R,
    ) -> Result<(Self, ValidatedDealerCommitment<P>), ThresholdBlsError> {
        validate_participant_index(parameters.session(), dealer_index)?;
        let mut coefficients = Zeroizing::new(Vec::with_capacity(usize::from(
            parameters.session().threshold(),
        )));
        for coefficient_index in 0..parameters.session().threshold() {
            coefficients.push([
                random_nonzero_scalar_bytes(rng)?,
                if coefficient_index == 0 {
                    Scalar::from(0_u64).to_bytes_be()
                } else {
                    random_nonzero_scalar_bytes(rng)?
                },
                if coefficient_index == 0 {
                    Scalar::from(0_u64).to_bytes_be()
                } else {
                    random_nonzero_scalar_bytes(rng)?
                },
            ]);
        }
        Self::from_coefficients_with_rng(parameters, dealer_index, coefficients, rng)
    }

    /// Import canonical zeroizing coefficients and build the public commitment.
    ///
    /// The first coefficient must have `r = u = 0`; all other coefficients may
    /// contain any canonical scalar. The builder consumes the zeroizing vector,
    /// retaining it only in this secret owner.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for wrong degree/index, noncanonical or
    /// inert constants, randomness failure, or a degenerate public point.
    pub fn from_coefficients(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer_index: u16,
        coefficients: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
    ) -> Result<(Self, ValidatedDealerCommitment<P>), ThresholdBlsError> {
        Self::from_coefficients_with_rng(parameters, dealer_index, coefficients, &mut OsRng)
    }

    /// Import canonical zeroizing coefficients with an explicit proof CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for wrong degree/index, noncanonical or
    /// inert constants, randomness failure, or a degenerate public point.
    pub fn from_coefficients_with_rng<R: TryCryptoRng + ?Sized>(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer_index: u16,
        coefficients: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
        rng: &mut R,
    ) -> Result<(Self, ValidatedDealerCommitment<P>), ThresholdBlsError> {
        validate_participant_index(parameters.session(), dealer_index)?;
        if coefficients.len() != usize::from(parameters.session().threshold()) {
            return Err(ThresholdBlsError::InvalidCoefficientCommitment);
        }
        let zero = Scalar::from(0_u64).to_bytes_be();
        if coefficients[0][1] != zero || coefficients[0][2] != zero {
            return Err(ThresholdBlsError::InvalidCoefficientCommitment);
        }
        let h = parameters.h_point()?;
        let v = parameters.v_point()?;
        let mut commitment_bytes = Vec::with_capacity(coefficients.len());
        for coefficient in coefficients.iter() {
            let s = decode_scalar(&coefficient[0])?;
            let r = decode_scalar(&coefficient[1])?;
            let u = decode_scalar(&coefficient[2])?;
            if commitment_bytes.is_empty() && s == Scalar::from(0_u64) {
                return Err(ThresholdBlsError::InvalidCoefficientCommitment);
            }
            let point = G2Projective::generator() * s
                + G2Projective::from(h) * r
                + G2Projective::from(v) * u;
            if bool::from(point.is_identity()) {
                return Err(ThresholdBlsError::InvalidCoefficientCommitment);
            }
            commitment_bytes.push(point.to_affine().to_compressed());
        }
        let nonce_bytes = Zeroizing::new(random_nonzero_scalar_bytes(rng)?);
        let nonce = decode_scalar(&nonce_bytes)?;
        let proof_commitment = (G2Projective::generator() * nonce)
            .to_affine()
            .to_compressed();
        let parsed_coefficients = commitment_bytes
            .iter()
            .map(|bytes| DasRenCoefficientCommitment::from_bytes(parameters, *bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let provisional =
            DasRenSchnorrPok::from_bytes(proof_commitment, Scalar::from(0_u64).to_bytes_be())?;
        let challenge =
            dealer_pok_challenge(parameters, dealer_index, &parsed_coefficients, &provisional)?;
        let constant = decode_scalar(&coefficients[0][0])?;
        let proof_response = (nonce + challenge * constant).to_bytes_be();
        let validated = DasRenDealerCommitment::verify(
            parameters,
            dealer_index,
            &commitment_bytes,
            proof_commitment,
            proof_response,
        )?;
        Ok((
            Self {
                parameters_digest: parameters.digest(),
                session_id: *parameters.session().session_id(),
                dealer_index,
                coefficients,
                marker: PhantomData,
            },
            validated,
        ))
    }

    /// Derive and self-verify one recipient's private share contribution.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a wrong session/dealer binding,
    /// invalid recipient, or unexpected commitment mismatch.
    pub fn private_share(
        &self,
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer: &ValidatedDealerCommitment<P>,
        recipient_index: u16,
    ) -> Result<DasRenPrivateShare<P>, ThresholdBlsError> {
        if self.parameters_digest != parameters.digest()
            || self.session_id != *parameters.session().session_id()
            || self.dealer_index != dealer.dealer_index
            || dealer.parameters_digest != self.parameters_digest
        {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        validate_participant_index(parameters.session(), recipient_index)?;
        let mut components = [Scalar::from(0_u64); 3];
        let x = Scalar::from(u64::from(recipient_index));
        let mut power = Scalar::from(1_u64);
        for coefficient in self.coefficients.iter() {
            for component in 0..3 {
                components[component] += decode_scalar(&coefficient[component])? * power;
            }
            power *= x;
        }
        DasRenPrivateShare::from_components(
            parameters,
            dealer,
            recipient_index,
            components[0].to_bytes_be(),
            components[1].to_bytes_be(),
            components[2].to_bytes_be(),
        )
    }
}

/// One verified, non-cloneable, zeroizing private dealer-to-recipient contribution.
///
/// It has no generic serialization or `Debug` implementation. The only byte
/// export returns a fresh [`Zeroizing`] value explicitly intended for an
/// authenticated encrypted transport.
pub struct DasRenPrivateShare<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    dealer_index: u16,
    recipient_index: u16,
    scalar_bytes: Zeroizing<DasRenSecretCoefficientV1>,
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> DasRenPrivateShare<P> {
    /// Import and verify one private contribution against a dealer commitment.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for invalid bindings/scalars or a failed
    /// coefficient equation.
    pub fn from_components(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer: &ValidatedDealerCommitment<P>,
        recipient_index: u16,
        s: [u8; 32],
        r: [u8; 32],
        u: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        verify_share_equation(parameters, dealer, recipient_index, &s, &r, &u)?;
        Ok(Self {
            parameters_digest: parameters.digest(),
            dealer_index: dealer.dealer_index,
            recipient_index,
            scalar_bytes: Zeroizing::new([s, r, u]),
            marker: PhantomData,
        })
    }

    /// Return the dealer index without exposing share material.
    #[must_use]
    pub const fn dealer_index(&self) -> u16 {
        self.dealer_index
    }

    /// Return the recipient index without exposing share material.
    #[must_use]
    pub const fn recipient_index(&self) -> u16 {
        self.recipient_index
    }

    /// Copy the contribution into an independently zeroizing transport buffer.
    ///
    /// The caller must immediately authenticate and encrypt this buffer for the
    /// bound recipient; it must never enter a public DTO or log.
    #[must_use]
    pub fn components_for_authenticated_encryption(&self) -> Zeroizing<DasRenSecretCoefficientV1> {
        Zeroizing::new(*self.scalar_bytes)
    }
}

/// Publicly revealed and verified response to one DKG complaint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DasRenRevealedShare<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    dealer_index: u16,
    recipient_index: u16,
    s: [u8; 32],
    r: [u8; 32],
    u: [u8; 32],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> DasRenRevealedShare<P> {
    /// Verify `g^s h^r v^u = product(C_k^(recipient^k))`.
    ///
    /// These values are public only because this type represents an explicit
    /// complaint response. Normal private DKG shares must not use this type.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for wrong bindings, noncanonical scalars,
    /// an invalid recipient index, or a failed commitment equation.
    pub fn verify(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer: &ValidatedDealerCommitment<P>,
        recipient_index: u16,
        s: [u8; 32],
        r: [u8; 32],
        u: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        verify_share_equation(parameters, dealer, recipient_index, &s, &r, &u)?;
        Ok(Self {
            parameters_digest: parameters.digest(),
            dealer_index: dealer.dealer_index,
            recipient_index,
            s,
            r,
            u,
            marker: PhantomData,
        })
    }

    /// Return the dealer index whose complaint was answered.
    #[must_use]
    pub const fn dealer_index(&self) -> u16 {
        self.dealer_index
    }

    /// Return the recipient index whose share was revealed.
    #[must_use]
    pub const fn recipient_index(&self) -> u16 {
        self.recipient_index
    }

    /// Return the three canonical public response scalars `(s, r, u)`.
    #[must_use]
    pub const fn scalar_bytes(&self) -> (&[u8; 32], &[u8; 32], &[u8; 32]) {
        (&self.s, &self.r, &self.u)
    }
}

/// One composite triple-generator verification share in an adaptive transcript.
///
/// Unlike [`ThresholdBlsPublicShare`], these bytes encode `g^s h^r v^u`, not
/// just `g^s`; they must only be used with the adaptive partial proof verifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdaptiveThresholdBlsPublicShare<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    index: u16,
    participant_hash: [u8; 32],
    bytes: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for AdaptiveThresholdBlsPublicShare<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdaptiveThresholdBlsPublicShare")
            .field("purpose", &P::ROLE_TAG)
            .field("index", &self.index)
            .field("participant_hash", &hex::encode(self.participant_hash))
            .field("bytes", &hex::encode(self.bytes))
            .finish()
    }
}

impl<P: ThresholdBlsPurpose> AdaptiveThresholdBlsPublicShare<P> {
    /// Return the one-based committee index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the deterministic frozen-seat binding.
    #[must_use]
    pub const fn participant_hash(&self) -> &[u8; 32] {
        &self.participant_hash
    }

    /// Return the canonical compressed composite commitment.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.bytes
    }

    fn point(&self) -> Result<G2Affine, ThresholdBlsError> {
        decode_g2(&self.bytes).map_err(|_| ThresholdBlsError::InvalidCoefficientCommitment)
    }
}

/// One adaptive BLS partial signature and its representation proof.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DasRenPartialSignature<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    index: u16,
    sigma: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
    proof_x: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    proof_y: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
    z_s: [u8; 32],
    z_r: [u8; 32],
    z_u: [u8; 32],
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> fmt::Debug for DasRenPartialSignature<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DasRenPartialSignature")
            .field("purpose", &P::ROLE_TAG)
            .field("session_id", &hex::encode(self.session_id))
            .field("index", &self.index)
            .field("sigma", &hex::encode(self.sigma))
            .field("proof_x", &hex::encode(self.proof_x))
            .field("proof_y", &hex::encode(self.proof_y))
            .finish_non_exhaustive()
    }
}

impl<P: ThresholdBlsPurpose> DasRenPartialSignature<P> {
    /// Parse canonical subgroup-checked proof points and response scalars.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid index, session, point, or scalar.
    #[allow(clippy::too_many_arguments)]
    pub fn from_bytes(
        session_id: [u8; 32],
        index: u16,
        sigma: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
        proof_x: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
        proof_y: [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
        z_s: [u8; 32],
        z_r: [u8; 32],
        z_u: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        if is_zero(&session_id) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        if index == 0 {
            return Err(ThresholdBlsError::InvalidParticipantIndex);
        }
        decode_g1(&sigma)?;
        decode_g2(&proof_x).map_err(|_| ThresholdBlsError::InvalidPartialSignatureProof)?;
        decode_g1(&proof_y).map_err(|_| ThresholdBlsError::InvalidPartialSignatureProof)?;
        decode_scalar(&z_s)?;
        decode_scalar(&z_r)?;
        decode_scalar(&z_u)?;
        Ok(Self {
            session_id,
            index,
            sigma,
            proof_x,
            proof_y,
            z_s,
            z_r,
            z_u,
            marker: PhantomData,
        })
    }

    /// Return the unique threshold session binding.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }

    /// Return the one-based signer index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the canonical `H0(message)^s * H1(message)^r` share bytes.
    #[must_use]
    pub const fn sigma_bytes(&self) -> &[u8; THRESHOLD_BLS_SIGNATURE_BYTES] {
        &self.sigma
    }

    /// Return the canonical G2 representation-proof commitment.
    #[must_use]
    pub const fn proof_x_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.proof_x
    }

    /// Return the canonical G1 signature-proof commitment.
    #[must_use]
    pub const fn proof_y_bytes(&self) -> &[u8; THRESHOLD_BLS_SIGNATURE_BYTES] {
        &self.proof_y
    }

    /// Return the three canonical proof response scalars.
    #[must_use]
    pub const fn response_bytes(&self) -> (&[u8; 32], &[u8; 32], &[u8; 32]) {
        (&self.z_s, &self.z_r, &self.z_u)
    }
}

/// Fully verified qualified-dealer transcript for adaptive threshold BLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveThresholdBlsPublicTranscript<P: ThresholdBlsPurpose> {
    parameters: AdaptiveThresholdBlsParameters<P>,
    qualified_indices: Vec<u16>,
    group_public_key: ThresholdBlsPublicKey<P>,
    public_shares: Vec<AdaptiveThresholdBlsPublicShare<P>>,
    dkg_event_hash: [u8; 32],
    transcript_hash: [u8; 32],
}

impl<P: ThresholdBlsPurpose> AdaptiveThresholdBlsPublicTranscript<P> {
    /// Finalize an exact canonical qualified dealer set.
    ///
    /// `validated_dealers` and `qualified_indices` must have identical,
    /// strictly increasing indices. At least `n - f` validated dealers are
    /// required. Composite public shares are evaluated for every committee
    /// seat, while the group key uses only the proven unblinded constants.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a zero event hash, wrong session,
    /// noncanonical/insufficient qualified set, or degenerate aggregate point.
    pub fn from_qualified_dealers(
        parameters: AdaptiveThresholdBlsParameters<P>,
        validated_dealers: &[ValidatedDealerCommitment<P>],
        qualified_indices: &[u16],
        dkg_event_hash: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        if is_zero(&dkg_event_hash) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        let session = parameters.session();
        let fault_tolerance = (session.committee_size() - 1) / 3;
        let minimum_qualified = usize::from(session.committee_size() - fault_tolerance);
        if validated_dealers.len() != qualified_indices.len()
            || validated_dealers.len() < minimum_qualified
            || validated_dealers.len() > usize::from(session.committee_size())
        {
            return Err(ThresholdBlsError::NonCanonicalQualifiedSet);
        }
        let parameters_digest = parameters.digest();
        let mut previous = 0_u16;
        for (dealer, index) in validated_dealers.iter().zip(qualified_indices) {
            validate_participant_index(session, *index)
                .map_err(|_| ThresholdBlsError::NonCanonicalQualifiedSet)?;
            if *index <= previous
                || dealer.dealer_index != *index
                || dealer.parameters_digest != parameters_digest
            {
                return Err(ThresholdBlsError::NonCanonicalQualifiedSet);
            }
            previous = *index;
        }

        let mut group_point = G2Projective::identity();
        for dealer in validated_dealers {
            group_point += G2Projective::from(dealer.coefficients[0].point()?);
        }
        if bool::from(group_point.is_identity()) {
            return Err(ThresholdBlsError::InvalidPublicKey);
        }
        let group_public_key = ThresholdBlsPublicKey::from_bytes(
            *session.session_id(),
            &group_point.to_affine().to_compressed(),
        )?;

        let mut public_shares = Vec::with_capacity(usize::from(session.committee_size()));
        for index in 1_u16..=session.committee_size() {
            let mut composite = G2Projective::identity();
            for dealer in validated_dealers {
                composite += G2Projective::from(evaluate_commitments(&dealer.coefficients, index)?);
            }
            if bool::from(composite.is_identity()) {
                return Err(ThresholdBlsError::InvalidCoefficientCommitment);
            }
            public_shares.push(AdaptiveThresholdBlsPublicShare {
                parameters_digest,
                index,
                participant_hash: adaptive_participant_hash(session, index),
                bytes: composite.to_affine().to_compressed(),
                marker: PhantomData,
            });
        }
        let transcript_hash = compute_adaptive_transcript_hash(
            &parameters,
            validated_dealers,
            qualified_indices,
            &group_public_key,
            &public_shares,
            &dkg_event_hash,
        );
        Ok(Self {
            parameters,
            qualified_indices: qualified_indices.to_vec(),
            group_public_key,
            public_shares,
            dkg_event_hash,
            transcript_hash,
        })
    }

    /// Return the immutable typed session.
    #[must_use]
    pub const fn session(&self) -> &ThresholdBlsSession<P> {
        self.parameters.session()
    }

    /// Return the fixed independent-generator parameters.
    #[must_use]
    pub const fn parameters(&self) -> &AdaptiveThresholdBlsParameters<P> {
        &self.parameters
    }

    /// Return the exact canonical qualified-dealer indices.
    #[must_use]
    pub fn qualified_indices(&self) -> &[u16] {
        &self.qualified_indices
    }

    /// Return the standard-generator group public key.
    #[must_use]
    pub const fn group_public_key(&self) -> &ThresholdBlsPublicKey<P> {
        &self.group_public_key
    }

    /// Return all canonically indexed composite verification shares.
    #[must_use]
    pub fn public_shares(&self) -> &[AdaptiveThresholdBlsPublicShare<P>] {
        &self.public_shares
    }

    /// Return the consensus event hash binding complaints and qualification.
    #[must_use]
    pub const fn dkg_event_hash(&self) -> &[u8; 32] {
        &self.dkg_event_hash
    }

    /// Return the deterministic complete adaptive transcript hash.
    #[must_use]
    pub const fn transcript_hash(&self) -> &[u8; 32] {
        &self.transcript_hash
    }

    /// Confirm that this value came from the complete verified three-scalar path.
    ///
    /// This is a structural cryptographic-readiness check only. It does not
    /// certify authenticated transport, corruption accounting, secure erasure,
    /// side-channel resistance, or a generic adaptive-security theorem.
    ///
    /// # Errors
    ///
    /// This constructor-authenticated type returns `Ok(())`; malformed remote
    /// input cannot instantiate it without passing all DKG checks.
    pub const fn ensure_adaptive_protocol_ready(&self) -> Result<(), ThresholdBlsError> {
        Ok(())
    }

    /// Verify one adaptive signature share and both representation equations.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a wrong session/index, invalid payload,
    /// malformed proof, or failed G1/G2 proof equation.
    pub fn verify_partial_signature(
        &self,
        payload: &[u8],
        partial: &DasRenPartialSignature<P>,
    ) -> Result<(), ThresholdBlsError> {
        if partial.session_id != *self.session().session_id() {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        let share = self
            .public_shares
            .get(usize::from(partial.index.saturating_sub(1)))
            .filter(|share| share.index == partial.index)
            .ok_or(ThresholdBlsError::UnknownParticipant)?;
        if share.parameters_digest != self.parameters.digest() {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        let message = self.session().signing_message(payload)?;
        let message_h0 = hash_message_to_g1::<P>(&message);
        let message_h1 = hash_message_to_h1::<P>(&message);
        let sigma = decode_g1(&partial.sigma)?;
        let proof_x = decode_g2(&partial.proof_x)
            .map_err(|_| ThresholdBlsError::InvalidPartialSignatureProof)?;
        let proof_y = decode_g1(&partial.proof_y)
            .map_err(|_| ThresholdBlsError::InvalidPartialSignatureProof)?;
        let z_s = decode_scalar(&partial.z_s)?;
        let z_r = decode_scalar(&partial.z_r)?;
        let z_u = decode_scalar(&partial.z_u)?;
        let challenge = partial_proof_challenge(self, &message, share, partial)?;
        let lhs_g2 = (G2Projective::generator() * z_s
            + G2Projective::from(self.parameters.h_point()?) * z_r
            + G2Projective::from(self.parameters.v_point()?) * z_u)
            .to_affine();
        let rhs_g2 = (G2Projective::from(proof_x) + G2Projective::from(share.point()?) * challenge)
            .to_affine();
        let lhs_g1 = (G1Projective::from(message_h0) * z_s + G1Projective::from(message_h1) * z_r)
            .to_affine();
        let rhs_g1 =
            (G1Projective::from(proof_y) + G1Projective::from(sigma) * challenge).to_affine();
        if lhs_g2 != rhs_g2 || lhs_g1 != rhs_g1 {
            return Err(ThresholdBlsError::InvalidPartialSignatureProof);
        }
        Ok(())
    }

    /// Verify, interpolate, and final-verify a canonical threshold subset.
    ///
    /// The returned standard BLS signature carries no reconstruction bitmap;
    /// all valid threshold subsets interpolate to the same group signature.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an insufficient/reordered/duplicate
    /// subset, any invalid partial proof, interpolation failure, or final pairing.
    pub fn combine_partial_signatures(
        &self,
        payload: &[u8],
        partials: &[DasRenPartialSignature<P>],
    ) -> Result<ThresholdBlsSignature<P>, ThresholdBlsError> {
        if partials.len() < usize::from(self.session().threshold())
            || partials.len() > usize::from(self.session().committee_size())
        {
            return Err(ThresholdBlsError::NonCanonicalPartialSignatureSet);
        }
        let mut previous = 0_u16;
        for partial in partials {
            if partial.index <= previous
                || partial.index > self.session().committee_size()
                || partial.session_id != *self.session().session_id()
            {
                return Err(ThresholdBlsError::NonCanonicalPartialSignatureSet);
            }
            self.verify_partial_signature(payload, partial)?;
            previous = partial.index;
        }
        let indices = partials
            .iter()
            .map(|partial| partial.index)
            .collect::<Vec<_>>();
        let mut combined = G1Projective::identity();
        for partial in partials {
            let coefficient = lagrange_at_zero(partial.index, &indices)?;
            combined += G1Projective::from(decode_g1(&partial.sigma)?) * coefficient;
        }
        if bool::from(combined.is_identity()) {
            return Err(ThresholdBlsError::InvalidSignature);
        }
        let signature = ThresholdBlsSignature::from_bytes(
            *self.session().session_id(),
            &combined.to_affine().to_compressed(),
        )?;
        self.verify_final_signature(payload, &signature)?;
        Ok(signature)
    }

    /// Verify one final signature against the qualified group key.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a wrong session, payload, or pairing.
    pub fn verify_final_signature(
        &self,
        payload: &[u8],
        signature: &ThresholdBlsSignature<P>,
    ) -> Result<(), ThresholdBlsError> {
        self.group_public_key
            .verify_payload(self.session(), payload, signature)
    }
}

impl AdaptiveThresholdBlsPublicTranscript<BeaconPurpose> {
    /// Verify a final adaptive beacon signature and derive its unique seed.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] when signature verification or HKDF fails.
    pub fn finalized_seed(
        &self,
        payload: &[u8],
        signature: &ThresholdBlsSignature<BeaconPurpose>,
    ) -> Result<[u8; 32], ThresholdBlsError> {
        self.verify_final_signature(payload, signature)?;
        derive_beacon_seed(self.session(), &self.transcript_hash, payload, signature)
    }
}

/// Non-cloneable, zeroizing adaptive threshold signing share `(s, r, u)`.
///
/// The type has no byte export, serialization, or `Debug` implementation and
/// is deliberately separate from generic account/consensus key containers.
pub struct AdaptiveThresholdBlsSecretShare<P: ThresholdBlsPurpose> {
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    index: u16,
    scalar_bytes: Zeroizing<[[u8; 32]; 3]>,
    marker: PhantomData<P>,
}

impl<P: ThresholdBlsPurpose> AdaptiveThresholdBlsSecretShare<P> {
    /// Combine the exact qualified private contributions for one recipient.
    ///
    /// Contributions must be strictly aligned with the transcript's complete
    /// qualified-dealer list. Missing, extra, duplicate, reordered, or
    /// cross-recipient shares fail closed.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a noncanonical contribution set,
    /// malformed secret scalar, or aggregate public-share mismatch.
    pub fn from_dealer_shares(
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        shares: &[DasRenPrivateShare<P>],
    ) -> Result<Self, ThresholdBlsError> {
        if shares.len() != transcript.qualified_indices.len() || shares.is_empty() {
            return Err(ThresholdBlsError::NonCanonicalQualifiedSet);
        }
        let recipient_index = shares[0].recipient_index;
        validate_participant_index(transcript.session(), recipient_index)?;
        let parameters_digest = transcript.parameters.digest();
        let mut aggregate = [Scalar::from(0_u64); 3];
        for (share, qualified_index) in shares.iter().zip(&transcript.qualified_indices) {
            if share.parameters_digest != parameters_digest
                || share.dealer_index != *qualified_index
                || share.recipient_index != recipient_index
            {
                return Err(ThresholdBlsError::NonCanonicalQualifiedSet);
            }
            for (component, sum) in aggregate.iter_mut().enumerate() {
                *sum += decode_scalar(&share.scalar_bytes[component])?;
            }
        }
        Self::from_components(
            transcript,
            recipient_index,
            aggregate[0].to_bytes_be(),
            aggregate[1].to_bytes_be(),
            aggregate[2].to_bytes_be(),
        )
    }

    /// Import three canonical secret-share scalars and validate their public commitment.
    ///
    /// Callers should construct these bytes only by summing authenticated
    /// private contributions from every qualified dealer, then discard the
    /// individual contributions according to the DKG erasure policy.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid index/scalar or a public
    /// commitment mismatch.
    pub fn from_components(
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        index: u16,
        s: [u8; 32],
        r: [u8; 32],
        u: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        validate_participant_index(transcript.session(), index)?;
        let share = transcript
            .public_shares
            .get(usize::from(index - 1))
            .filter(|share| share.index == index)
            .ok_or(ThresholdBlsError::UnknownParticipant)?;
        let s_scalar = decode_scalar(&s)?;
        let r_scalar = decode_scalar(&r)?;
        let u_scalar = decode_scalar(&u)?;
        let commitment = (G2Projective::generator() * s_scalar
            + G2Projective::from(transcript.parameters.h_point()?) * r_scalar
            + G2Projective::from(transcript.parameters.v_point()?) * u_scalar)
            .to_affine();
        if commitment != share.point()? {
            return Err(ThresholdBlsError::SecretShareMismatch);
        }
        Ok(Self {
            session_id: *transcript.session().session_id(),
            transcript_hash: transcript.transcript_hash,
            index,
            scalar_bytes: Zeroizing::new([s, r, u]),
            marker: PhantomData,
        })
    }

    /// Return the one-based signer index without exposing share material.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Create an adaptive partial signature using operating-system randomness.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a transcript mismatch, payload bound,
    /// randomness failure, or unexpected proof validation failure.
    pub fn sign_payload(
        &self,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        payload: &[u8],
    ) -> Result<DasRenPartialSignature<P>, ThresholdBlsError> {
        self.sign_payload_with_rng(transcript, payload, &mut OsRng)
    }

    /// Create an adaptive partial signature using an explicit CSPRNG.
    ///
    /// This entry point supports deterministic interoperability vectors. Live
    /// signers must never reuse proof randomness across messages or sessions.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a transcript mismatch, payload bound,
    /// randomness failure, or unexpected proof validation failure.
    pub fn sign_payload_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        payload: &[u8],
        rng: &mut R,
    ) -> Result<DasRenPartialSignature<P>, ThresholdBlsError> {
        if self.session_id != *transcript.session().session_id()
            || self.transcript_hash != transcript.transcript_hash
        {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        let s = decode_scalar(&self.scalar_bytes[0])?;
        let r = decode_scalar(&self.scalar_bytes[1])?;
        let u = decode_scalar(&self.scalar_bytes[2])?;
        let message = transcript.session().signing_message(payload)?;
        let message_h0 = hash_message_to_g1::<P>(&message);
        let message_h1 = hash_message_to_h1::<P>(&message);
        let sigma = (G1Projective::from(message_h0) * s + G1Projective::from(message_h1) * r)
            .to_affine()
            .to_compressed();
        let nonce_bytes = Zeroizing::new([
            random_nonzero_scalar_bytes(rng)?,
            random_nonzero_scalar_bytes(rng)?,
            random_nonzero_scalar_bytes(rng)?,
        ]);
        let nonce_s = decode_scalar(&nonce_bytes[0])?;
        let nonce_r = decode_scalar(&nonce_bytes[1])?;
        let nonce_u = decode_scalar(&nonce_bytes[2])?;
        let proof_x = (G2Projective::generator() * nonce_s
            + G2Projective::from(transcript.parameters.h_point()?) * nonce_r
            + G2Projective::from(transcript.parameters.v_point()?) * nonce_u)
            .to_affine()
            .to_compressed();
        let proof_y = (G1Projective::from(message_h0) * nonce_s
            + G1Projective::from(message_h1) * nonce_r)
            .to_affine()
            .to_compressed();
        let mut partial = DasRenPartialSignature::from_bytes(
            self.session_id,
            self.index,
            sigma,
            proof_x,
            proof_y,
            Scalar::from(0_u64).to_bytes_be(),
            Scalar::from(0_u64).to_bytes_be(),
            Scalar::from(0_u64).to_bytes_be(),
        )?;
        let share = transcript
            .public_shares
            .get(usize::from(self.index - 1))
            .filter(|share| share.index == self.index)
            .ok_or(ThresholdBlsError::UnknownParticipant)?;
        let challenge = partial_proof_challenge(transcript, &message, share, &partial)?;
        partial.z_s = (nonce_s + challenge * s).to_bytes_be();
        partial.z_r = (nonce_r + challenge * r).to_bytes_be();
        partial.z_u = (nonce_u + challenge * u).to_bytes_be();
        transcript.verify_partial_signature(payload, &partial)?;
        Ok(partial)
    }
}

/// Fully validated public transcript for one typed threshold-BLS session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThresholdBlsPublicTranscript<P: ThresholdBlsPurpose> {
    session: ThresholdBlsSession<P>,
    group_public_key: ThresholdBlsPublicKey<P>,
    public_shares: Vec<ThresholdBlsPublicShare<P>>,
    dkg_contribution_hash: [u8; 32],
    transcript_hash: [u8; 32],
}

impl<P: ThresholdBlsPurpose> ThresholdBlsPublicTranscript<P> {
    /// Validate and construct a canonical complete public transcript.
    ///
    /// Shares must be ordered exactly by index and cover `1..=n`. This
    /// prohibits duplicate or remapped indices instead of normalizing an
    /// ambiguous remote representation.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for session mismatches, malformed share
    /// sets, duplicate participants, or a zero DKG contribution hash.
    pub fn new(
        session: ThresholdBlsSession<P>,
        group_public_key: ThresholdBlsPublicKey<P>,
        public_shares: Vec<ThresholdBlsPublicShare<P>>,
        dkg_contribution_hash: [u8; 32],
    ) -> Result<Self, ThresholdBlsError> {
        if is_zero(&dkg_contribution_hash) {
            return Err(ThresholdBlsError::ZeroBinding);
        }
        if group_public_key.session_id != *session.session_id() {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        if public_shares.len() != usize::from(session.committee_size()) {
            return Err(ThresholdBlsError::NonCanonicalShareSet);
        }
        let mut participants = HashSet::with_capacity(public_shares.len());
        for (offset, share) in public_shares.iter().enumerate() {
            let expected_index = u16::try_from(offset + 1)
                .map_err(|_| ThresholdBlsError::InvalidParticipantIndex)?;
            if share.index != expected_index {
                return Err(ThresholdBlsError::NonCanonicalShareSet);
            }
            if share.session_id != *session.session_id() {
                return Err(ThresholdBlsError::SessionMismatch);
            }
            if !participants.insert(share.participant_hash) {
                return Err(ThresholdBlsError::DuplicateParticipant);
            }
        }
        let transcript_hash = compute_public_transcript_hash(
            &session,
            &group_public_key,
            &public_shares,
            &dkg_contribution_hash,
        );
        Ok(Self {
            session,
            group_public_key,
            public_shares,
            dkg_contribution_hash,
            transcript_hash,
        })
    }

    /// Return the immutable typed session.
    #[must_use]
    pub const fn session(&self) -> &ThresholdBlsSession<P> {
        &self.session
    }

    /// Return the canonical group public key.
    #[must_use]
    pub const fn group_public_key(&self) -> &ThresholdBlsPublicKey<P> {
        &self.group_public_key
    }

    /// Return the complete canonically indexed verification-share list.
    #[must_use]
    pub fn public_shares(&self) -> &[ThresholdBlsPublicShare<P>] {
        &self.public_shares
    }

    /// Return the caller-supplied hash of all canonical DKG contributions.
    #[must_use]
    pub const fn dkg_contribution_hash(&self) -> &[u8; 32] {
        &self.dkg_contribution_hash
    }

    /// Return the deterministic hash of this public transcript.
    #[must_use]
    pub const fn transcript_hash(&self) -> &[u8; 32] {
        &self.transcript_hash
    }

    /// Verify one signature share against the frozen participant public share.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for a session/index mismatch, invalid
    /// payload, or failed BLS pairing.
    pub fn verify_signature_share(
        &self,
        payload: &[u8],
        signature_share: &ThresholdBlsSignatureShare<P>,
    ) -> Result<(), ThresholdBlsError> {
        if signature_share.session_id != *self.session.session_id() {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        let share = self
            .public_shares
            .get(usize::from(signature_share.index.saturating_sub(1)))
            .filter(|share| share.index == signature_share.index)
            .ok_or(ThresholdBlsError::UnknownParticipant)?;
        let message = self.session.signing_message(payload)?;
        verify_signature::<P>(&share.bytes, &message, &signature_share.bytes)
    }

    /// Verify one final signature against the session group public key.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] for an invalid transcript binding or pairing.
    pub fn verify_final_signature(
        &self,
        payload: &[u8],
        signature: &ThresholdBlsSignature<P>,
    ) -> Result<(), ThresholdBlsError> {
        self.group_public_key
            .verify_payload(&self.session, payload, signature)
    }

    /// Fail closed because this legacy transcript carries no adaptive DKG evidence.
    ///
    /// Public point validation is not an activation certificate. In particular,
    /// it cannot establish qualified-set agreement, complaint correctness,
    /// bias resistance, erasure, or the Das--Ren correlated-share invariants.
    /// Use [`AdaptiveThresholdBlsPublicTranscript`] for the verified path.
    ///
    /// # Errors
    ///
    /// Always returns [`ThresholdBlsError::AdaptiveProtocolNotReady`] for this legacy type.
    pub const fn ensure_adaptive_protocol_ready(&self) -> Result<(), ThresholdBlsError> {
        Err(ThresholdBlsError::AdaptiveProtocolNotReady)
    }
}

impl ThresholdBlsPublicTranscript<BeaconPurpose> {
    /// Verify a finalized beacon signature and derive its unique public seed.
    ///
    /// The seed depends on the final group signature, session transcript, and
    /// exact framed message, never on a signer bitmap or reconstruction subset.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdBlsError`] when signature verification or HKDF fails.
    pub fn finalized_seed(
        &self,
        payload: &[u8],
        signature: &ThresholdBlsSignature<BeaconPurpose>,
    ) -> Result<[u8; 32], ThresholdBlsError> {
        self.verify_final_signature(payload, signature)?;
        derive_beacon_seed(&self.session, &self.transcript_hash, payload, signature)
    }
}

fn validate_participant_index<P: ThresholdBlsPurpose>(
    session: &ThresholdBlsSession<P>,
    index: u16,
) -> Result<(), ThresholdBlsError> {
    if index == 0 || index > session.committee_size() {
        Err(ThresholdBlsError::InvalidParticipantIndex)
    } else {
        Ok(())
    }
}

fn decode_scalar(bytes: &[u8; 32]) -> Result<Scalar, ThresholdBlsError> {
    Scalar::from_bytes_be(bytes)
        .into_option()
        .ok_or(ThresholdBlsError::InvalidScalar)
}

fn scalar_from_transcript(hasher: Sha256) -> Result<Scalar, ThresholdBlsError> {
    for counter in 0_u32..=SCALAR_REJECTION_LIMIT {
        let mut attempt = hasher.clone();
        attempt.update(counter.to_be_bytes());
        let candidate: [u8; 32] = attempt.finalize().into();
        if let Some(scalar) = Scalar::from_bytes_be(&candidate).into_option() {
            return Ok(scalar);
        }
    }
    Err(ThresholdBlsError::ScalarDerivation)
}

fn random_nonzero_scalar_bytes<R: TryCryptoRng + ?Sized>(
    rng: &mut R,
) -> Result<[u8; 32], ThresholdBlsError> {
    let mut candidate = Zeroizing::new([0_u8; 32]);
    for _ in 0_u32..=SCALAR_REJECTION_LIMIT {
        rng.try_fill_bytes(candidate.as_mut())
            .map_err(|_| ThresholdBlsError::RandomnessUnavailable)?;
        if let Some(scalar) = Scalar::from_bytes_be(&candidate).into_option()
            && scalar != Scalar::from(0_u64)
        {
            return Ok(*candidate);
        }
    }
    Err(ThresholdBlsError::InertRandomness)
}

fn dealer_pok_challenge<P: ThresholdBlsPurpose>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    dealer_index: u16,
    coefficients: &[DasRenCoefficientCommitment<P>],
    proof: &DasRenSchnorrPok<P>,
) -> Result<Scalar, ThresholdBlsError> {
    let mut hasher = Sha256::new();
    hasher.update(DEALER_POK_DOMAIN_V1);
    hasher.update(THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    hasher.update(parameters.digest());
    let mut session = Vec::new();
    parameters.session.write_canonical(&mut session);
    hasher.update(session);
    hasher.update(dealer_index.to_be_bytes());
    hasher.update((coefficients.len() as u32).to_be_bytes());
    for coefficient in coefficients {
        if coefficient.parameters_digest != parameters.digest() {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        hasher.update(coefficient.bytes);
    }
    hasher.update(proof.commitment);
    scalar_from_transcript(hasher)
}

fn evaluate_commitments<P: ThresholdBlsPurpose>(
    coefficients: &[DasRenCoefficientCommitment<P>],
    recipient_index: u16,
) -> Result<G2Affine, ThresholdBlsError> {
    let x = Scalar::from(u64::from(recipient_index));
    let mut power = Scalar::from(1_u64);
    let mut result = G2Projective::identity();
    for coefficient in coefficients {
        result += G2Projective::from(coefficient.point()?) * power;
        power *= x;
    }
    Ok(result.to_affine())
}

fn verify_share_equation<P: ThresholdBlsPurpose>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    dealer: &ValidatedDealerCommitment<P>,
    recipient_index: u16,
    s: &[u8; 32],
    r: &[u8; 32],
    u: &[u8; 32],
) -> Result<(), ThresholdBlsError> {
    validate_participant_index(parameters.session(), recipient_index)?;
    if dealer.parameters_digest != parameters.digest() {
        return Err(ThresholdBlsError::SessionMismatch);
    }
    let s_scalar = decode_scalar(s)?;
    let r_scalar = decode_scalar(r)?;
    let u_scalar = decode_scalar(u)?;
    let lhs = (G2Projective::generator() * s_scalar
        + G2Projective::from(parameters.h_point()?) * r_scalar
        + G2Projective::from(parameters.v_point()?) * u_scalar)
        .to_affine();
    let rhs = evaluate_commitments(&dealer.coefficients, recipient_index)?;
    if lhs != rhs {
        return Err(ThresholdBlsError::InvalidComplaintResponse);
    }
    Ok(())
}

fn adaptive_participant_hash<P: ThresholdBlsPurpose>(
    session: &ThresholdBlsSession<P>,
    index: u16,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ADAPTIVE_PARTICIPANT_DOMAIN_V1);
    let mut session_bytes = Vec::new();
    session.write_canonical(&mut session_bytes);
    hasher.update(session_bytes);
    hasher.update(index.to_be_bytes());
    hasher.finalize().into()
}

fn compute_adaptive_transcript_hash<P: ThresholdBlsPurpose>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    dealers: &[ValidatedDealerCommitment<P>],
    qualified_indices: &[u16],
    group_public_key: &ThresholdBlsPublicKey<P>,
    public_shares: &[AdaptiveThresholdBlsPublicShare<P>],
    dkg_event_hash: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ADAPTIVE_TRANSCRIPT_DOMAIN_V1);
    hasher.update(THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    hasher.update(parameters.digest());
    hasher.update(dkg_event_hash);
    hasher.update((dealers.len() as u32).to_be_bytes());
    for (dealer, index) in dealers.iter().zip(qualified_indices) {
        hasher.update(index.to_be_bytes());
        for coefficient in &dealer.coefficients {
            hasher.update(coefficient.bytes);
        }
        hasher.update(dealer.proof.commitment);
        hasher.update(dealer.proof.response);
    }
    hasher.update(group_public_key.bytes);
    for share in public_shares {
        hasher.update(share.index.to_be_bytes());
        hasher.update(share.participant_hash);
        hasher.update(share.bytes);
    }
    hasher.finalize().into()
}

fn partial_proof_challenge<P: ThresholdBlsPurpose>(
    transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
    message: &[u8],
    share: &AdaptiveThresholdBlsPublicShare<P>,
    partial: &DasRenPartialSignature<P>,
) -> Result<Scalar, ThresholdBlsError> {
    if partial.session_id != *transcript.session().session_id()
        || share.index != partial.index
        || share.parameters_digest != transcript.parameters.digest()
    {
        return Err(ThresholdBlsError::SessionMismatch);
    }
    let mut hasher = Sha256::new();
    hasher.update(PARTIAL_PROOF_DOMAIN_V1);
    hasher.update(THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    hasher.update(transcript.transcript_hash);
    hasher.update(transcript.parameters.digest());
    hasher.update((message.len() as u32).to_be_bytes());
    hasher.update(message);
    hasher.update(partial.index.to_be_bytes());
    hasher.update(share.participant_hash);
    hasher.update(share.bytes);
    hasher.update(partial.sigma);
    hasher.update(partial.proof_x);
    hasher.update(partial.proof_y);
    scalar_from_transcript(hasher)
}

fn lagrange_at_zero(index: u16, indices: &[u16]) -> Result<Scalar, ThresholdBlsError> {
    let x_i = Scalar::from(u64::from(index));
    let mut numerator = Scalar::from(1_u64);
    let mut denominator = Scalar::from(1_u64);
    for other in indices {
        if *other == index {
            continue;
        }
        let x_j = Scalar::from(u64::from(*other));
        numerator *= -x_j;
        denominator *= x_i - x_j;
    }
    let inverse = denominator
        .invert()
        .into_option()
        .ok_or(ThresholdBlsError::NonCanonicalPartialSignatureSet)?;
    Ok(numerator * inverse)
}

fn derive_beacon_seed(
    session: &ThresholdBlsSession<BeaconPurpose>,
    transcript_hash: &[u8; 32],
    payload: &[u8],
    signature: &ThresholdBlsSignature<BeaconPurpose>,
) -> Result<[u8; 32], ThresholdBlsError> {
    let message = session.signing_message(payload)?;
    let message_hash: [u8; 32] = Sha256::digest(&message).into();
    let mut salt_hasher = Sha256::new();
    salt_hasher.update(BEACON_SEED_SALT_V1);
    salt_hasher.update(transcript_hash);
    salt_hasher.update(message_hash);
    let salt: [u8; 32] = salt_hasher.finalize().into();
    let hkdf = Hkdf::<Sha256>::new(Some(&salt), signature.as_bytes());
    let mut seed = [0_u8; 32];
    hkdf.expand(BEACON_SEED_INFO_V1, &mut seed)
        .map_err(|_| ThresholdBlsError::HkdfExpand)?;
    Ok(seed)
}

fn compute_public_transcript_hash<P: ThresholdBlsPurpose>(
    session: &ThresholdBlsSession<P>,
    group_public_key: &ThresholdBlsPublicKey<P>,
    public_shares: &[ThresholdBlsPublicShare<P>],
    dkg_contribution_hash: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PUBLIC_TRANSCRIPT_DOMAIN_V1);
    let mut session_bytes = Vec::new();
    session.write_canonical(&mut session_bytes);
    hasher.update(session_bytes);
    hasher.update(group_public_key.bytes);
    hasher.update(dkg_contribution_hash);
    for share in public_shares {
        hasher.update(share.index.to_be_bytes());
        hasher.update(share.participant_hash);
        hasher.update(share.bytes);
    }
    hasher.finalize().into()
}

pub(crate) fn hash_message_to_g1<P: ThresholdBlsPurpose>(message: &[u8]) -> G1Affine {
    G1Projective::hash_to_curve(message, P::SIGNATURE_DST, &[]).to_affine()
}

fn hash_message_to_h1<P: ThresholdBlsPurpose>(message: &[u8]) -> G1Affine {
    let mut purpose_bound_message = Vec::with_capacity(message.len() + 1);
    purpose_bound_message.push(P::ROLE_TAG);
    purpose_bound_message.extend_from_slice(message);
    G1Projective::hash_to_curve(&purpose_bound_message, PARTIAL_H1_DST_V1, &[]).to_affine()
}

fn verify_signature<P: ThresholdBlsPurpose>(
    public_key: &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    message: &[u8],
    signature: &[u8; THRESHOLD_BLS_SIGNATURE_BYTES],
) -> Result<(), ThresholdBlsError> {
    let public_key = decode_g2(public_key)?;
    let signature = decode_g1(signature)?;
    let message_point = hash_message_to_g1::<P>(message);
    let terms: [(&G1Affine, &G2Prepared); 2] = [
        (&signature, &G2Prepared::from(G2Affine::generator())),
        (
            &(-G1Projective::from(message_point)).to_affine(),
            &G2Prepared::from(public_key),
        ),
    ];
    let pairing = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
    if bool::from(pairing.is_identity()) {
        Ok(())
    } else {
        Err(ThresholdBlsError::SignatureMismatch)
    }
}

fn decode_g1(encoded: &[u8; THRESHOLD_BLS_SIGNATURE_BYTES]) -> Result<G1Affine, ThresholdBlsError> {
    let point = G1Affine::from_compressed(encoded)
        .into_option()
        .ok_or(ThresholdBlsError::InvalidSignature)?;
    if bool::from(point.is_identity()) || point.to_compressed() != *encoded {
        return Err(ThresholdBlsError::InvalidSignature);
    }
    Ok(point)
}

fn decode_g2(
    encoded: &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
) -> Result<G2Affine, ThresholdBlsError> {
    let point = G2Affine::from_compressed(encoded)
        .into_option()
        .ok_or(ThresholdBlsError::InvalidPublicKey)?;
    if bool::from(point.is_identity()) || point.to_compressed() != *encoded {
        return Err(ThresholdBlsError::InvalidPublicKey);
    }
    Ok(point)
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use blstrs::{G2Affine, Scalar};
    use group::{Curve as _, prime::PrimeCurveAffine as _};
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;

    fn binding(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn session<P: ThresholdBlsPurpose>() -> ThresholdBlsSession<P> {
        ThresholdBlsSession::new(binding(1), binding(2), binding(3), 4, 2).expect("valid session")
    }

    fn key<P: ThresholdBlsPurpose>(
        session: &ThresholdBlsSession<P>,
        scalar: u64,
    ) -> ThresholdBlsPublicKey<P> {
        let bytes = (G2Affine::generator() * Scalar::from(scalar))
            .to_affine()
            .to_compressed();
        ThresholdBlsPublicKey::from_bytes(*session.session_id(), &bytes).expect("valid key")
    }

    fn sign<P: ThresholdBlsPurpose>(
        session: &ThresholdBlsSession<P>,
        scalar: u64,
        payload: &[u8],
    ) -> ThresholdBlsSignature<P> {
        let message = session.signing_message(payload).expect("message");
        let signature = (hash_message_to_g1::<P>(&message) * Scalar::from(scalar))
            .to_affine()
            .to_compressed();
        ThresholdBlsSignature::from_bytes(*session.session_id(), &signature).expect("signature")
    }

    type DealerScalars = [[Scalar; 3]; 2];

    struct AdaptiveFixture {
        parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
        dealers: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
        dealer_scalars: Vec<DealerScalars>,
        transcript: AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
    }

    fn adaptive_dealer(
        parameters: &AdaptiveThresholdBlsParameters<BeaconPurpose>,
        dealer_index: u16,
    ) -> (ValidatedDealerCommitment<BeaconPurpose>, DealerScalars) {
        let offset = u64::from(dealer_index);
        let scalars = [
            [
                Scalar::from(10 + offset),
                Scalar::from(0_u64),
                Scalar::from(0_u64),
            ],
            [
                Scalar::from(20 + offset),
                Scalar::from(30 + offset),
                Scalar::from(40 + offset),
            ],
        ];
        let h = parameters.h_point().expect("h");
        let v = parameters.v_point().expect("v");
        let coefficient_bytes = scalars
            .map(|coefficient| {
                (G2Projective::generator() * coefficient[0]
                    + G2Projective::from(h) * coefficient[1]
                    + G2Projective::from(v) * coefficient[2])
                    .to_affine()
                    .to_compressed()
            })
            .to_vec();
        let nonce = Scalar::from(100 + offset);
        let proof_commitment = (G2Projective::generator() * nonce)
            .to_affine()
            .to_compressed();
        let parsed_coefficients = coefficient_bytes
            .iter()
            .map(|bytes| DasRenCoefficientCommitment::from_bytes(parameters, *bytes))
            .collect::<Result<Vec<_>, _>>()
            .expect("coefficients");
        let provisional =
            DasRenSchnorrPok::from_bytes(proof_commitment, Scalar::from(0_u64).to_bytes_be())
                .expect("provisional proof");
        let challenge =
            dealer_pok_challenge(parameters, dealer_index, &parsed_coefficients, &provisional)
                .expect("challenge");
        let response = (nonce + challenge * scalars[0][0]).to_bytes_be();
        let validated = DasRenDealerCommitment::verify(
            parameters,
            dealer_index,
            &coefficient_bytes,
            proof_commitment,
            response,
        )
        .expect("dealer commitment");
        (validated, scalars)
    }

    fn adaptive_fixture() -> AdaptiveFixture {
        let session = session::<BeaconPurpose>();
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut dealers = Vec::new();
        let mut dealer_scalars = Vec::new();
        for index in 1_u16..=3 {
            let (dealer, scalars) = adaptive_dealer(&parameters, index);
            dealers.push(dealer);
            dealer_scalars.push(scalars);
        }
        let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            parameters,
            &dealers,
            &[1, 2, 3],
            binding(90),
        )
        .expect("adaptive transcript");
        AdaptiveFixture {
            parameters,
            dealers,
            dealer_scalars,
            transcript,
        }
    }

    fn evaluate_dealer_scalar(
        coefficients: &DealerScalars,
        recipient: u16,
        component: usize,
    ) -> Scalar {
        coefficients[0][component] + coefficients[1][component] * Scalar::from(u64::from(recipient))
    }

    fn adaptive_secret(
        fixture: &AdaptiveFixture,
        recipient: u16,
    ) -> AdaptiveThresholdBlsSecretShare<BeaconPurpose> {
        let mut components = [Scalar::from(0_u64); 3];
        for dealer in &fixture.dealer_scalars {
            for (component, aggregate) in components.iter_mut().enumerate() {
                *aggregate += evaluate_dealer_scalar(dealer, recipient, component);
            }
        }
        AdaptiveThresholdBlsSecretShare::from_components(
            &fixture.transcript,
            recipient,
            components[0].to_bytes_be(),
            components[1].to_bytes_be(),
            components[2].to_bytes_be(),
        )
        .expect("adaptive secret")
    }

    #[test]
    fn sessions_enforce_exact_committee_and_threshold() {
        assert_eq!(
            ThresholdBlsSession::<BeaconPurpose>::new(binding(1), binding(2), binding(3), 5, 2),
            Err(ThresholdBlsError::InvalidCommitteeSize)
        );
        assert_eq!(
            ThresholdBlsSession::<BeaconPurpose>::new(binding(1), binding(2), binding(3), 4, 3),
            Err(ThresholdBlsError::InvalidThreshold)
        );
        assert_eq!(
            ThresholdBlsSession::<BeaconPurpose>::new([0; 32], binding(2), binding(3), 4, 2),
            Err(ThresholdBlsError::ZeroBinding)
        );
        assert!(
            ThresholdBlsSession::<BeaconPurpose>::new(binding(1), binding(2), binding(3), 31, 11)
                .is_ok()
        );
    }

    #[test]
    fn public_points_reject_identity_and_noncanonical_encodings() {
        let session = session::<BeaconPurpose>();
        let mut g2_identity = [0_u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES];
        g2_identity[0] = 0xc0;
        assert_eq!(
            ThresholdBlsPublicKey::<BeaconPurpose>::from_bytes(*session.session_id(), &g2_identity),
            Err(ThresholdBlsError::InvalidPublicKey)
        );
        let mut g1_identity = [0_u8; THRESHOLD_BLS_SIGNATURE_BYTES];
        g1_identity[0] = 0xc0;
        assert_eq!(
            ThresholdBlsSignature::<BeaconPurpose>::from_bytes(*session.session_id(), &g1_identity),
            Err(ThresholdBlsError::InvalidSignature)
        );
        assert_eq!(
            ThresholdBlsPublicKey::<BeaconPurpose>::from_bytes(*session.session_id(), &[0; 95]),
            Err(ThresholdBlsError::InvalidPublicKey)
        );
    }

    #[test]
    fn role_and_session_bindings_change_messages_and_reject_replay() {
        let beacon = session::<BeaconPurpose>();
        let tle = session::<TleReleasePurpose>();
        assert_ne!(
            beacon.signing_message(b"pulse").expect("beacon message"),
            tle.signing_message(b"pulse").expect("tle message")
        );
        let key = key(&beacon, 7);
        let signature = sign(&beacon, 7, b"pulse");
        assert_eq!(key.verify_payload(&beacon, b"pulse", &signature), Ok(()));
        assert_eq!(
            key.verify_payload(&beacon, b"other", &signature),
            Err(ThresholdBlsError::SignatureMismatch)
        );
        let other_session =
            ThresholdBlsSession::<BeaconPurpose>::new(binding(1), binding(9), binding(3), 4, 2)
                .expect("other session");
        assert_eq!(
            key.verify_payload(&other_session, b"pulse", &signature),
            Err(ThresholdBlsError::SessionMismatch)
        );
    }

    #[test]
    fn transcript_rejects_duplicate_indices_participants_and_session_mix() {
        let session = session::<BeaconPurpose>();
        let group_key = key(&session, 7);
        let shares = (1_u16..=4)
            .map(|index| {
                let share_key = key(&session, u64::from(index) + 10);
                ThresholdBlsPublicShare::from_bytes(
                    *session.session_id(),
                    index,
                    binding(index as u8 + 20),
                    share_key.as_bytes(),
                )
                .expect("share")
            })
            .collect::<Vec<_>>();
        assert!(
            ThresholdBlsPublicTranscript::new(session, group_key, shares.clone(), binding(50))
                .is_ok()
        );

        let mut duplicate_index = shares.clone();
        duplicate_index[1].index = 1;
        assert_eq!(
            ThresholdBlsPublicTranscript::new(session, group_key, duplicate_index, binding(50)),
            Err(ThresholdBlsError::NonCanonicalShareSet)
        );

        let mut duplicate_participant = shares.clone();
        duplicate_participant[1].participant_hash = duplicate_participant[0].participant_hash;
        assert_eq!(
            ThresholdBlsPublicTranscript::new(
                session,
                group_key,
                duplicate_participant,
                binding(50)
            ),
            Err(ThresholdBlsError::DuplicateParticipant)
        );

        let mut wrong_session = shares;
        wrong_session[0].session_id = binding(99);
        assert_eq!(
            ThresholdBlsPublicTranscript::new(session, group_key, wrong_session, binding(50)),
            Err(ThresholdBlsError::SessionMismatch)
        );
    }

    #[test]
    fn public_transcript_verifies_shares_but_adaptive_gate_stays_closed() {
        let session = session::<BeaconPurpose>();
        let group_key = key(&session, 7);
        let share_scalars = [11_u64, 12, 13, 14];
        let shares = share_scalars
            .iter()
            .enumerate()
            .map(|(offset, scalar)| {
                let index = u16::try_from(offset + 1).expect("index");
                let share_key = key(&session, *scalar);
                ThresholdBlsPublicShare::from_bytes(
                    *session.session_id(),
                    index,
                    binding(index as u8 + 20),
                    share_key.as_bytes(),
                )
                .expect("share")
            })
            .collect::<Vec<_>>();
        let transcript = ThresholdBlsPublicTranscript::new(session, group_key, shares, binding(50))
            .expect("transcript");
        let message = transcript
            .session()
            .signing_message(b"pulse")
            .expect("message");
        let share_signature = (hash_message_to_g1::<BeaconPurpose>(&message)
            * Scalar::from(11_u64))
        .to_affine()
        .to_compressed();
        let share_signature = ThresholdBlsSignatureShare::from_bytes(
            *transcript.session().session_id(),
            1,
            &share_signature,
        )
        .expect("signature share");
        assert_eq!(
            transcript.verify_signature_share(b"pulse", &share_signature),
            Ok(())
        );
        assert_eq!(
            transcript.ensure_adaptive_protocol_ready(),
            Err(ThresholdBlsError::AdaptiveProtocolNotReady)
        );
    }

    #[test]
    fn finalized_beacon_seed_requires_a_valid_signature() {
        let session = session::<BeaconPurpose>();
        let group_key = key(&session, 7);
        let shares = (1_u16..=4)
            .map(|index| {
                let share_key = key(&session, u64::from(index) + 10);
                ThresholdBlsPublicShare::from_bytes(
                    *session.session_id(),
                    index,
                    binding(index as u8 + 20),
                    share_key.as_bytes(),
                )
                .expect("share")
            })
            .collect();
        let transcript = ThresholdBlsPublicTranscript::new(session, group_key, shares, binding(50))
            .expect("transcript");
        let signature = sign(transcript.session(), 7, b"pulse-42");
        let seed = transcript
            .finalized_seed(b"pulse-42", &signature)
            .expect("seed");
        assert_ne!(seed, [0; 32]);
        assert_eq!(
            transcript.finalized_seed(b"pulse-43", &signature),
            Err(ThresholdBlsError::SignatureMismatch)
        );
    }

    #[test]
    fn adaptive_parameters_are_deterministic_distinct_and_session_bound() {
        let session = session::<BeaconPurpose>();
        let first = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let second = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        assert_eq!(first, second);
        assert_ne!(first.h_bytes(), first.v_bytes());
        assert_ne!(*first.h_bytes(), G2Affine::generator().to_compressed());
        assert_ne!(*first.v_bytes(), G2Affine::generator().to_compressed());

        let other =
            ThresholdBlsSession::<BeaconPurpose>::new(binding(1), binding(8), binding(3), 4, 2)
                .expect("other session");
        let other = AdaptiveThresholdBlsParameters::derive(&other).expect("other parameters");
        assert_ne!(first.h_bytes(), other.h_bytes());
        assert_ne!(first.v_bytes(), other.v_bytes());
    }

    #[test]
    fn dealer_constant_proof_and_complaint_response_are_fully_bound() {
        let fixture = adaptive_fixture();
        let dealer = &fixture.dealers[0];
        let scalars = &fixture.dealer_scalars[0];
        let recipient = 4_u16;
        let s = evaluate_dealer_scalar(scalars, recipient, 0).to_bytes_be();
        let r = evaluate_dealer_scalar(scalars, recipient, 1).to_bytes_be();
        let u = evaluate_dealer_scalar(scalars, recipient, 2).to_bytes_be();
        let revealed = DasRenRevealedShare::verify(&fixture.parameters, dealer, recipient, s, r, u)
            .expect("complaint response");
        assert_eq!(revealed.dealer_index(), 1);
        assert_eq!(revealed.recipient_index(), recipient);

        let bad_s = (decode_scalar(&s).expect("s") + Scalar::from(1_u64)).to_bytes_be();
        assert_eq!(
            DasRenRevealedShare::verify(&fixture.parameters, dealer, recipient, bad_s, r, u,),
            Err(ThresholdBlsError::InvalidComplaintResponse)
        );

        let coefficient_bytes = dealer
            .coefficients()
            .iter()
            .map(|coefficient| *coefficient.as_bytes())
            .collect::<Vec<_>>();
        let mut bad_response = *dealer.constant_proof().response_bytes();
        bad_response[31] ^= 1;
        assert!(matches!(
            DasRenDealerCommitment::verify(
                &fixture.parameters,
                dealer.dealer_index(),
                &coefficient_bytes,
                *dealer.constant_proof().commitment_bytes(),
                bad_response,
            ),
            Err(ThresholdBlsError::InvalidDealerProof | ThresholdBlsError::InvalidScalar)
        ));
        assert_eq!(
            DasRenDealerCommitment::verify(
                &fixture.parameters,
                dealer.dealer_index(),
                &coefficient_bytes[..1],
                *dealer.constant_proof().commitment_bytes(),
                *dealer.constant_proof().response_bytes(),
            ),
            Err(ThresholdBlsError::InvalidCoefficientCommitment)
        );
    }

    #[test]
    fn qualified_set_finalization_is_exact_sorted_and_fail_closed() {
        let fixture = adaptive_fixture();
        assert_eq!(fixture.transcript.ensure_adaptive_protocol_ready(), Ok(()));
        assert_eq!(fixture.transcript.qualified_indices(), &[1, 2, 3]);
        assert_eq!(fixture.transcript.public_shares().len(), 4);
        assert_ne!(fixture.transcript.transcript_hash(), &[0; 32]);

        assert_eq!(
            AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                fixture.parameters,
                &fixture.dealers[..2],
                &[1, 2],
                binding(91),
            ),
            Err(ThresholdBlsError::NonCanonicalQualifiedSet)
        );
        assert_eq!(
            AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                fixture.parameters,
                &fixture.dealers,
                &[1, 3, 2],
                binding(91),
            ),
            Err(ThresholdBlsError::NonCanonicalQualifiedSet)
        );
        assert_eq!(
            AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                fixture.parameters,
                &fixture.dealers,
                &[1, 2, 3],
                [0; 32],
            ),
            Err(ThresholdBlsError::ZeroBinding)
        );
    }

    #[test]
    fn adaptive_partial_proofs_combine_to_one_subset_independent_signature() {
        let fixture = adaptive_fixture();
        let mut rng = ChaCha20Rng::from_seed([77; 32]);
        let partials = (1_u16..=4)
            .map(|index| {
                adaptive_secret(&fixture, index)
                    .sign_payload_with_rng(&fixture.transcript, b"pulse-adaptive", &mut rng)
                    .expect("partial")
            })
            .collect::<Vec<_>>();
        for partial in &partials {
            assert_eq!(
                fixture
                    .transcript
                    .verify_partial_signature(b"pulse-adaptive", partial),
                Ok(())
            );
            assert_eq!(
                fixture
                    .transcript
                    .verify_partial_signature(b"wrong-pulse", partial),
                Err(ThresholdBlsError::InvalidPartialSignatureProof)
            );
        }

        let first = fixture
            .transcript
            .combine_partial_signatures(b"pulse-adaptive", &partials[..2])
            .expect("first subset");
        let second_subset = [partials[1], partials[3]];
        let second = fixture
            .transcript
            .combine_partial_signatures(b"pulse-adaptive", &second_subset)
            .expect("second subset");
        assert_eq!(first, second);
        assert_eq!(
            fixture
                .transcript
                .verify_final_signature(b"pulse-adaptive", &first),
            Ok(())
        );
        let seed = fixture
            .transcript
            .finalized_seed(b"pulse-adaptive", &first)
            .expect("seed");
        assert_ne!(seed, [0; 32]);

        let reordered = [partials[1], partials[0]];
        assert_eq!(
            fixture
                .transcript
                .combine_partial_signatures(b"pulse-adaptive", &reordered),
            Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
        );
        let mut forged = partials[0];
        forged.z_r = (decode_scalar(&forged.z_r).expect("z_r") + Scalar::from(1_u64)).to_bytes_be();
        assert_eq!(
            fixture
                .transcript
                .verify_partial_signature(b"pulse-adaptive", &forged),
            Err(ThresholdBlsError::InvalidPartialSignatureProof)
        );
    }

    #[test]
    fn adaptive_secret_import_and_rng_failures_do_not_fall_back() {
        #[derive(Debug)]
        struct ZeroRng;
        impl rand_core::RngCore for ZeroRng {
            fn next_u32(&mut self) -> u32 {
                0
            }

            fn next_u64(&mut self) -> u64 {
                0
            }

            fn fill_bytes(&mut self, destination: &mut [u8]) {
                destination.fill(0);
            }
        }
        impl rand_core::CryptoRng for ZeroRng {}

        let fixture = adaptive_fixture();
        let secret = adaptive_secret(&fixture, 1);
        assert!(matches!(
            secret.sign_payload_with_rng(&fixture.transcript, b"pulse", &mut ZeroRng),
            Err(ThresholdBlsError::InertRandomness)
        ));
        assert_eq!(
            AdaptiveThresholdBlsSecretShare::from_components(
                &fixture.transcript,
                1,
                Scalar::from(1_u64).to_bytes_be(),
                Scalar::from(2_u64).to_bytes_be(),
                Scalar::from(3_u64).to_bytes_be(),
            )
            .err(),
            Some(ThresholdBlsError::SecretShareMismatch)
        );

        let identity_g1 = {
            let mut bytes = [0_u8; THRESHOLD_BLS_SIGNATURE_BYTES];
            bytes[0] = 0xc0;
            bytes
        };
        assert!(matches!(
            DasRenPartialSignature::<BeaconPurpose>::from_bytes(
                *fixture.transcript.session().session_id(),
                1,
                identity_g1,
                *fixture.parameters.h_bytes(),
                identity_g1,
                [0; 32],
                [0; 32],
                [0; 32],
            ),
            Err(ThresholdBlsError::InvalidSignature)
        ));
    }

    #[test]
    fn public_dealer_builder_produces_verified_private_contributions() {
        let session = session::<BeaconPurpose>();
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut rng = ChaCha20Rng::from_seed([88; 32]);
        let mut dealer_secrets = Vec::new();
        let mut dealers = Vec::new();
        for dealer_index in 1_u16..=3 {
            let (secret, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                    .expect("dealer generation");
            dealer_secrets.push(secret);
            dealers.push(dealer);
        }
        let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            parameters,
            &dealers,
            &[1, 2, 3],
            binding(92),
        )
        .expect("transcript");
        let recipient = 2_u16;
        let private_shares = dealer_secrets
            .iter()
            .zip(&dealers)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&parameters, dealer, recipient)
                    .expect("private share")
            })
            .collect::<Vec<_>>();
        let transport = private_shares[0].components_for_authenticated_encryption();
        assert_eq!(transport.len(), 3);
        let imported = DasRenPrivateShare::from_components(
            &parameters,
            &dealers[0],
            recipient,
            transport[0],
            transport[1],
            transport[2],
        )
        .expect("imported private share");
        assert_eq!(imported.dealer_index(), 1);
        assert_eq!(imported.recipient_index(), recipient);

        let signer =
            AdaptiveThresholdBlsSecretShare::from_dealer_shares(&transcript, &private_shares)
                .expect("combined signer");
        let partial = signer
            .sign_payload_with_rng(&transcript, b"builder-pulse", &mut rng)
            .expect("partial");
        assert_eq!(
            transcript.verify_partial_signature(b"builder-pulse", &partial),
            Ok(())
        );
    }

    #[test]
    fn message_bound_is_enforced_before_allocation() {
        let session = session::<BeaconPurpose>();
        assert_eq!(
            session.signing_message(&vec![0; THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1 + 1]),
            Err(ThresholdBlsError::MessageTooLarge)
        );
    }
}
