//! Parliament timelock-encryption KEM/DEM helpers.
//!
//! The v1 envelope is a fixed Boneh--Franklin FullIdent-style transform over
//! BLS12-381 followed by HKDF-SHA256 and XChaCha20-Poly1305. The independently
//! generated TLE master key is in G2; the future identity private key (the
//! threshold release signature) is in G1. Type-level key roles prevent a
//! randomness-beacon key from being passed to this module.
//!
//! This generic envelope module is not a distributed release protocol and does
//! not prove that an envelope contains the opening of a previously accepted
//! ballot commitment. Parliament's ballot path must use
//! [`crate::timed_ovn`], whose release term is intrinsic to its one-hot proof;
//! this standalone DEM envelope is not a plaintext or manual-opening fallback.

use std::vec::Vec;

use aead::{Aead as _, KeyInit as _, Payload};
use blstrs::{Compress as _, G1Affine, G2Affine, G2Projective, Scalar, pairing};
use chacha20poly1305::XChaCha20Poly1305;
use group::{Curve as _, Group as _, prime::PrimeCurveAffine as _};
use hkdf::Hkdf;
use rand_core::{OsRng, TryCryptoRng};
use sha2::{Digest as _, Sha256, Sha512};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::threshold_bls::{
    THRESHOLD_BLS_PROTOCOL_VERSION_V1, THRESHOLD_BLS_PUBLIC_KEY_BYTES,
    THRESHOLD_BLS_SIGNATURE_BYTES, ThresholdBlsError, ThresholdBlsPublicKey, ThresholdBlsSession,
    ThresholdBlsSignature, TleReleasePurpose, hash_message_to_g1,
};

/// Fixed protocol version for Parliament TLE envelopes.
pub const TLE_PROTOCOL_VERSION_V1: u16 = 1;
/// Maximum caller-supplied AAD bytes accepted by the v1 envelope.
pub const TLE_MAX_AAD_BYTES_V1: usize = 16 * 1024;
/// Maximum encrypted masked-opening bytes accepted by the v1 envelope.
pub const TLE_MAX_PLAINTEXT_BYTES_V1: usize = 64 * 1024;
/// Width of the random FullIdent sigma value.
pub const TLE_SIGMA_BYTES_V1: usize = 32;
/// Width of the random data-encryption key wrapped by FullIdent.
pub const TLE_DEK_BYTES_V1: usize = 32;
/// Width of the XChaCha20-Poly1305 nonce.
pub const TLE_NONCE_BYTES_V1: usize = 24;
/// Width of the XChaCha20-Poly1305 authentication tag.
pub const TLE_TAG_BYTES_V1: usize = 16;

const IDENTITY_PAYLOAD_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.identity-payload.v1\0";
const AAD_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.aad.v1\0";
const H2_PAIRING_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.fullident.h2-pairing.v1\0";
const H3_SCALAR_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.fullident.h3-scalar.v1\0";
const H4_MESSAGE_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.fullident.h4-message.v1\0";
const DEM_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.parliament.tle.dem-transcript.v1\0";
const DEM_HKDF_INFO_V1: &[u8] = b"iroha.parliament.tle.xchacha20poly1305.key.v1\0";
const WIRE_MAGIC_V1: &[u8; 8] = b"ITLEV1\0\0";
const FIXED_ENVELOPE_BYTES_V1: usize = WIRE_MAGIC_V1.len()
    + 32 * 3
    + THRESHOLD_BLS_PUBLIC_KEY_BYTES
    + TLE_SIGMA_BYTES_V1
    + TLE_DEK_BYTES_V1
    + TLE_NONCE_BYTES_V1
    + 4;

/// Errors returned by Parliament TLE validation, sealing, and opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TleError {
    /// A required digest binding was an all-zero placeholder.
    #[error("TLE binding digest must be non-zero")]
    ZeroBinding,
    /// The target finalized height must be non-zero.
    #[error("TLE target finalized height must be non-zero")]
    ZeroTargetHeight,
    /// A threshold-BLS key, session, or signature was invalid.
    #[error("invalid threshold-BLS material for TLE")]
    InvalidThresholdMaterial,
    /// A future identity private key was malformed or did not match the master key and identity.
    #[error("invalid TLE identity private key")]
    InvalidIdentitySecretKey,
    /// Caller AAD exceeded the fixed v1 bound.
    #[error("TLE AAD exceeds the v1 bound")]
    AadTooLarge,
    /// Plaintext exceeded the fixed v1 bound.
    #[error("TLE plaintext exceeds the v1 bound")]
    PlaintextTooLarge,
    /// The operating-system or caller CSPRNG failed.
    #[error("TLE CSPRNG failed")]
    RandomnessUnavailable,
    /// A CSPRNG returned an inert all-zero sigma, DEK, or nonce.
    #[error("TLE CSPRNG returned inert all-zero material")]
    InertRandomness,
    /// Envelope bytes were malformed, noncanonical, oversized, or had trailing data.
    #[error("invalid canonical TLE envelope")]
    InvalidEnvelope,
    /// The supplied identity, session, or AAD did not match the envelope binding.
    #[error("TLE envelope binding mismatch")]
    BindingMismatch,
    /// FullIdent consistency or AEAD authentication failed.
    #[error("TLE envelope opening failed")]
    OpenFailed,
    /// Hash-to-scalar rejection sampling exhausted its defensive bound.
    #[error("TLE hash-to-scalar derivation failed")]
    ScalarDerivation,
    /// A fixed HKDF expansion failed.
    #[error("TLE HKDF expansion failed")]
    HkdfExpand,
    /// Canonical pairing-target compression unexpectedly failed.
    #[error("TLE pairing-target encoding failed")]
    PairingEncoding,
}

impl From<ThresholdBlsError> for TleError {
    fn from(_: ThresholdBlsError) -> Self {
        Self::InvalidThresholdMaterial
    }
}

/// Independently generated Parliament TLE master public key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TleMasterPublicKey(ThresholdBlsPublicKey<TleReleasePurpose>);

impl TleMasterPublicKey {
    /// Parse a canonical non-identity G2 key bound to `session_id`.
    ///
    /// # Errors
    ///
    /// Returns [`TleError::InvalidThresholdMaterial`] for malformed key material.
    pub fn from_bytes(session_id: [u8; 32], bytes: &[u8]) -> Result<Self, TleError> {
        Ok(Self(ThresholdBlsPublicKey::from_bytes(session_id, bytes)?))
    }

    /// Wrap an already validated typed threshold-BLS TLE-release public key.
    #[must_use]
    pub const fn from_threshold_key(key: ThresholdBlsPublicKey<TleReleasePurpose>) -> Self {
        Self(key)
    }

    /// Return the unique TLE DKG session binding.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        self.0.session_id()
    }

    /// Return the canonical compressed G2 encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        self.0.as_bytes()
    }
}

/// Exact future identity signed by the Parliament TLE threshold committee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TleReleaseIdentityV1 {
    session: ThresholdBlsSession<TleReleasePurpose>,
    governance_attempt_id: [u8; 32],
    body_instance_id: [u8; 32],
    ballot_attempt_id: [u8; 32],
    survivor_corpus_root: [u8; 32],
    // Generic compatibility slot for an application-defined context root.
    // Timed OVN passes only its replay-derived no-recovery sentinel here.
    recovery_root: [u8; 32],
    target_finalized_height: u64,
    parameter_hash: [u8; 32],
}

impl TleReleaseIdentityV1 {
    /// Construct the complete future release identity.
    ///
    /// The embedded threshold session supplies the network, TLE session,
    /// frozen validator-roster hash, committee size, and threshold bindings.
    /// `protocol_context_root` is an opaque application binding: this module
    /// neither interprets it as a recovery corpus nor exposes a recovery path.
    /// Timed OVN supplies its locally replay-derived no-recovery sentinel.
    ///
    /// # Errors
    ///
    /// Returns [`TleError`] for an all-zero digest or zero target height.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: ThresholdBlsSession<TleReleasePurpose>,
        governance_attempt_id: [u8; 32],
        body_instance_id: [u8; 32],
        ballot_attempt_id: [u8; 32],
        survivor_corpus_root: [u8; 32],
        protocol_context_root: [u8; 32],
        target_finalized_height: u64,
        parameter_hash: [u8; 32],
    ) -> Result<Self, TleError> {
        if [
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            survivor_corpus_root,
            protocol_context_root,
            parameter_hash,
        ]
        .iter()
        .any(|binding| is_zero(binding))
        {
            return Err(TleError::ZeroBinding);
        }
        if target_finalized_height == 0 {
            return Err(TleError::ZeroTargetHeight);
        }
        Ok(Self {
            session,
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            survivor_corpus_root,
            recovery_root: protocol_context_root,
            target_finalized_height,
            parameter_hash,
        })
    }

    /// Return the typed threshold-BLS release session.
    #[must_use]
    pub const fn session(&self) -> &ThresholdBlsSession<TleReleasePurpose> {
        &self.session
    }

    /// Return the governance-attempt binding carried by this release identity.
    #[must_use]
    pub const fn governance_attempt_id(&self) -> &[u8; 32] {
        &self.governance_attempt_id
    }

    /// Return the governed-body instance binding carried by this release identity.
    #[must_use]
    pub const fn body_instance_id(&self) -> &[u8; 32] {
        &self.body_instance_id
    }

    /// Return the ballot-attempt binding carried by this release identity.
    #[must_use]
    pub const fn ballot_attempt_id(&self) -> &[u8; 32] {
        &self.ballot_attempt_id
    }

    /// Return the exact frozen survivor-corpus root.
    #[must_use]
    pub const fn survivor_corpus_root(&self) -> &[u8; 32] {
        &self.survivor_corpus_root
    }

    /// Return the protocol-parameter binding carried by this release identity.
    #[must_use]
    pub const fn parameter_hash(&self) -> &[u8; 32] {
        &self.parameter_hash
    }

    /// Return the finalized height before which honest validators must not sign.
    #[must_use]
    pub const fn target_finalized_height(&self) -> u64 {
        self.target_finalized_height
    }

    /// Return the canonical application payload framed by the threshold session.
    #[must_use]
    pub fn payload_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(IDENTITY_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 6 + 8);
        payload.extend_from_slice(IDENTITY_PAYLOAD_DOMAIN_V1);
        payload.extend_from_slice(&TLE_PROTOCOL_VERSION_V1.to_be_bytes());
        payload.extend_from_slice(&self.governance_attempt_id);
        payload.extend_from_slice(&self.body_instance_id);
        payload.extend_from_slice(&self.ballot_attempt_id);
        payload.extend_from_slice(&self.survivor_corpus_root);
        payload.extend_from_slice(&self.recovery_root);
        payload.extend_from_slice(&self.target_finalized_height.to_be_bytes());
        payload.extend_from_slice(&self.parameter_hash);
        payload
    }

    /// Return the exact bytes hashed to G1 by the threshold release signature and IBE KEM.
    ///
    /// # Errors
    ///
    /// Returns [`TleError`] only if the fixed payload unexpectedly exceeds the
    /// threshold transcript bound.
    pub fn release_message(&self) -> Result<Vec<u8>, TleError> {
        self.session
            .signing_message(&self.payload_bytes())
            .map_err(Into::into)
    }

    fn release_message_digest(&self) -> Result<[u8; 32], TleError> {
        Ok(Sha256::digest(self.release_message()?).into())
    }
}

/// Canonical FullIdent KEM plus XChaCha20-Poly1305 DEM envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TleEnvelopeV1 {
    session_id: [u8; 32],
    identity_digest: [u8; 32],
    aad_digest: [u8; 32],
    u: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    v: [u8; TLE_SIGMA_BYTES_V1],
    w: [u8; TLE_DEK_BYTES_V1],
    nonce: [u8; TLE_NONCE_BYTES_V1],
    ciphertext: Vec<u8>,
}

impl TleEnvelopeV1 {
    /// Parse a fully consuming canonical envelope.
    ///
    /// # Errors
    ///
    /// Returns [`TleError::InvalidEnvelope`] for malformed, oversized,
    /// noncanonical, identity-point, inert-nonce, or trailing input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TleError> {
        if bytes.len() < FIXED_ENVELOPE_BYTES_V1 + TLE_TAG_BYTES_V1
            || bytes.len() > FIXED_ENVELOPE_BYTES_V1 + TLE_MAX_PLAINTEXT_BYTES_V1 + TLE_TAG_BYTES_V1
        {
            return Err(TleError::InvalidEnvelope);
        }
        let mut cursor = 0_usize;
        if take::<8>(bytes, &mut cursor)? != *WIRE_MAGIC_V1 {
            return Err(TleError::InvalidEnvelope);
        }
        let session_id = take::<32>(bytes, &mut cursor)?;
        let identity_digest = take::<32>(bytes, &mut cursor)?;
        let aad_digest = take::<32>(bytes, &mut cursor)?;
        let u = take::<THRESHOLD_BLS_PUBLIC_KEY_BYTES>(bytes, &mut cursor)?;
        let v = take::<TLE_SIGMA_BYTES_V1>(bytes, &mut cursor)?;
        let w = take::<TLE_DEK_BYTES_V1>(bytes, &mut cursor)?;
        let nonce = take::<TLE_NONCE_BYTES_V1>(bytes, &mut cursor)?;
        let ciphertext_len = u32::from_be_bytes(take::<4>(bytes, &mut cursor)?) as usize;
        let remaining = bytes
            .len()
            .checked_sub(cursor)
            .ok_or(TleError::InvalidEnvelope)?;
        if ciphertext_len != remaining
            || ciphertext_len < TLE_TAG_BYTES_V1
            || ciphertext_len > TLE_MAX_PLAINTEXT_BYTES_V1 + TLE_TAG_BYTES_V1
        {
            return Err(TleError::InvalidEnvelope);
        }
        let envelope = Self {
            session_id,
            identity_digest,
            aad_digest,
            u,
            v,
            w,
            nonce,
            ciphertext: bytes[cursor..].to_vec(),
        };
        envelope.validate_public_shape()?;
        Ok(envelope)
    }

    /// Encode the envelope into its canonical fully consuming v1 layout.
    ///
    /// # Errors
    ///
    /// Returns [`TleError::InvalidEnvelope`] if an in-memory value no longer
    /// satisfies the canonical public envelope bounds.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TleError> {
        self.validate_public_shape()?;
        let ciphertext_len =
            u32::try_from(self.ciphertext.len()).map_err(|_| TleError::InvalidEnvelope)?;
        let mut bytes = Vec::with_capacity(FIXED_ENVELOPE_BYTES_V1 + self.ciphertext.len());
        bytes.extend_from_slice(WIRE_MAGIC_V1);
        bytes.extend_from_slice(&self.session_id);
        bytes.extend_from_slice(&self.identity_digest);
        bytes.extend_from_slice(&self.aad_digest);
        bytes.extend_from_slice(&self.u);
        bytes.extend_from_slice(&self.v);
        bytes.extend_from_slice(&self.w);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&ciphertext_len.to_be_bytes());
        bytes.extend_from_slice(&self.ciphertext);
        Ok(bytes)
    }

    /// Return the unique TLE DKG session binding.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }

    /// Return SHA-256 of the exact future threshold release message.
    #[must_use]
    pub const fn identity_digest(&self) -> &[u8; 32] {
        &self.identity_digest
    }

    /// Return SHA-256 of the complete typed AEAD associated data.
    #[must_use]
    pub const fn aad_digest(&self) -> &[u8; 32] {
        &self.aad_digest
    }

    /// Return the canonical compressed FullIdent `U` point in G2.
    #[must_use]
    pub const fn kem_u(&self) -> &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES] {
        &self.u
    }

    /// Return the authenticated ciphertext and tag without its nonce.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    fn validate_public_shape(&self) -> Result<(), TleError> {
        if is_zero(&self.session_id)
            || is_zero(&self.identity_digest)
            || is_zero(&self.aad_digest)
            || is_zero(&self.nonce)
        {
            return Err(TleError::InvalidEnvelope);
        }
        let point = G2Affine::from_compressed(&self.u)
            .into_option()
            .ok_or(TleError::InvalidEnvelope)?;
        if bool::from(point.is_identity()) || point.to_compressed() != self.u {
            return Err(TleError::InvalidEnvelope);
        }
        if self.ciphertext.len() < TLE_TAG_BYTES_V1
            || self.ciphertext.len() > TLE_MAX_PLAINTEXT_BYTES_V1 + TLE_TAG_BYTES_V1
        {
            return Err(TleError::InvalidEnvelope);
        }
        Ok(())
    }
}

/// Dedicated, non-cloneable, zeroizing future TLE identity private key.
///
/// This value is intentionally separate from generic account and consensus
/// signing keys and has no serialization, byte-export, or `Debug` implementation.
pub struct TleIdentitySecretKeyV1 {
    master_public_key: TleMasterPublicKey,
    identity_digest: [u8; 32],
    secret_bytes: Zeroizing<[u8; THRESHOLD_BLS_SIGNATURE_BYTES]>,
}

impl TleIdentitySecretKeyV1 {
    /// Validate and import the threshold release signature as an IBE identity key.
    ///
    /// The signature must verify under `master_public_key` for the exact typed
    /// future release identity. The stored compressed point is zeroized on drop.
    ///
    /// # Errors
    ///
    /// Returns [`TleError::InvalidIdentitySecretKey`] for malformed or mismatched material.
    pub fn from_threshold_signature(
        master_public_key: TleMasterPublicKey,
        identity: &TleReleaseIdentityV1,
        signature_bytes: &[u8],
    ) -> Result<Self, TleError> {
        if master_public_key.session_id() != identity.session().session_id() {
            return Err(TleError::InvalidIdentitySecretKey);
        }
        let signature = ThresholdBlsSignature::<TleReleasePurpose>::from_bytes(
            *identity.session().session_id(),
            signature_bytes,
        )
        .map_err(|_| TleError::InvalidIdentitySecretKey)?;
        master_public_key
            .0
            .verify_payload(identity.session(), &identity.payload_bytes(), &signature)
            .map_err(|_| TleError::InvalidIdentitySecretKey)?;
        let identity_digest = identity
            .release_message_digest()
            .map_err(|_| TleError::InvalidIdentitySecretKey)?;
        Ok(Self {
            master_public_key,
            identity_digest,
            secret_bytes: Zeroizing::new(*signature.as_bytes()),
        })
    }

    /// Open and authenticate one envelope for the exact release identity and AAD.
    ///
    /// FullIdent consistency and AEAD failures deliberately collapse to
    /// [`TleError::OpenFailed`] to avoid exposing a useful validity oracle.
    ///
    /// # Errors
    ///
    /// Returns [`TleError`] for bound violations/mismatches or failed authenticated opening.
    pub fn open(
        &self,
        identity: &TleReleaseIdentityV1,
        caller_aad: &[u8],
        envelope: &TleEnvelopeV1,
    ) -> Result<Vec<u8>, TleError> {
        validate_aad_len(caller_aad)?;
        envelope.validate_public_shape()?;
        let release_message = identity.release_message()?;
        let identity_digest: [u8; 32] = Sha256::digest(&release_message).into();
        let aad = canonical_aad(&release_message, caller_aad)?;
        let aad_digest: [u8; 32] = Sha256::digest(&aad).into();
        if envelope.session_id != *self.master_public_key.session_id()
            || envelope.session_id != *identity.session().session_id()
            || identity_digest != self.identity_digest
            || envelope.identity_digest != identity_digest
            || envelope.aad_digest != aad_digest
        {
            return Err(TleError::BindingMismatch);
        }

        let secret_point = decode_identity_secret(&self.secret_bytes)?;
        let u = G2Affine::from_compressed(&envelope.u)
            .into_option()
            .ok_or(TleError::OpenFailed)?;
        let shared = pairing(&secret_point, &u);
        let shared_bytes = pairing_bytes(&shared).map_err(|_| TleError::OpenFailed)?;
        let sigma_mask = h2_pairing_mask(&identity_digest, &aad_digest, &shared_bytes);
        let mut sigma = Zeroizing::new([0_u8; TLE_SIGMA_BYTES_V1]);
        xor_into(&mut sigma, &envelope.v, &sigma_mask);
        let message_mask = h4_message_mask(&identity_digest, &aad_digest, &sigma);
        let mut dek = Zeroizing::new([0_u8; TLE_DEK_BYTES_V1]);
        xor_into(&mut dek, &envelope.w, &message_mask);
        let scalar = h3_nonzero_scalar(&identity_digest, &aad_digest, &sigma, &dek)
            .map_err(|_| TleError::OpenFailed)?;
        let expected_u = (G2Projective::generator() * scalar)
            .to_affine()
            .to_compressed();
        if !bool::from(expected_u.ct_eq(&envelope.u)) {
            return Err(TleError::OpenFailed);
        }

        let transcript_hash = dem_transcript_hash(
            &identity_digest,
            &aad_digest,
            &envelope.u,
            &envelope.v,
            &envelope.w,
        );
        let key = derive_dem_key(&dek, &transcript_hash)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| TleError::OpenFailed)?;
        let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(envelope.nonce.as_slice())
            .map_err(|_| TleError::OpenFailed)?;
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &envelope.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| TleError::OpenFailed)
    }

    /// Return the validated release point to the folded timed-OVN aggregate opener.
    ///
    /// This crate-private bridge never exposes the stored secret encoding and
    /// rechecks the master-key and exact future-identity bindings before use.
    pub(crate) fn pairing_secret_point(
        &self,
        master_public_key: &TleMasterPublicKey,
        identity: &TleReleaseIdentityV1,
    ) -> Result<G1Affine, TleError> {
        let identity_digest = identity.release_message_digest()?;
        if self.master_public_key != *master_public_key
            || self.master_public_key.session_id() != identity.session().session_id()
            || self.identity_digest != identity_digest
        {
            return Err(TleError::BindingMismatch);
        }
        decode_identity_secret(&self.secret_bytes)
    }
}

/// Seal one masked opening using the operating-system CSPRNG.
///
/// # Errors
///
/// Returns [`TleError`] for invalid bindings, size limits, CSPRNG failure, or
/// an unexpected fixed primitive failure.
pub fn seal_tle_v1(
    master_public_key: &TleMasterPublicKey,
    identity: &TleReleaseIdentityV1,
    caller_aad: &[u8],
    plaintext: &[u8],
) -> Result<TleEnvelopeV1, TleError> {
    seal_tle_v1_with_rng(
        master_public_key,
        identity,
        caller_aad,
        plaintext,
        &mut OsRng,
    )
}

/// Seal one masked opening using an explicit CSPRNG.
///
/// This entry point supports deterministic interoperability vectors. Production
/// callers should normally use [`seal_tle_v1`] and must never reuse RNG state.
///
/// # Errors
///
/// Returns [`TleError`] for invalid bindings, size limits, CSPRNG failure, or
/// an unexpected fixed primitive failure.
pub fn seal_tle_v1_with_rng<R: TryCryptoRng + ?Sized>(
    master_public_key: &TleMasterPublicKey,
    identity: &TleReleaseIdentityV1,
    caller_aad: &[u8],
    plaintext: &[u8],
    rng: &mut R,
) -> Result<TleEnvelopeV1, TleError> {
    validate_aad_len(caller_aad)?;
    if plaintext.len() > TLE_MAX_PLAINTEXT_BYTES_V1 {
        return Err(TleError::PlaintextTooLarge);
    }
    if master_public_key.session_id() != identity.session().session_id() {
        return Err(TleError::BindingMismatch);
    }

    let release_message = identity.release_message()?;
    let identity_digest: [u8; 32] = Sha256::digest(&release_message).into();
    let aad = canonical_aad(&release_message, caller_aad)?;
    let aad_digest: [u8; 32] = Sha256::digest(&aad).into();
    let mut sigma = Zeroizing::new([0_u8; TLE_SIGMA_BYTES_V1]);
    let mut dek = Zeroizing::new([0_u8; TLE_DEK_BYTES_V1]);
    let mut nonce = [0_u8; TLE_NONCE_BYTES_V1];
    rng.try_fill_bytes(sigma.as_mut())
        .map_err(|_| TleError::RandomnessUnavailable)?;
    rng.try_fill_bytes(dek.as_mut())
        .map_err(|_| TleError::RandomnessUnavailable)?;
    rng.try_fill_bytes(&mut nonce)
        .map_err(|_| TleError::RandomnessUnavailable)?;
    if is_zero(sigma.as_ref()) || is_zero(dek.as_ref()) || is_zero(&nonce) {
        return Err(TleError::InertRandomness);
    }

    let scalar = h3_nonzero_scalar(&identity_digest, &aad_digest, &sigma, &dek)?;
    let u = (G2Projective::generator() * scalar)
        .to_affine()
        .to_compressed();
    let identity_point = hash_message_to_g1::<TleReleasePurpose>(&release_message);
    let master_point = master_public_key.0.decode()?;
    let shared = pairing(&identity_point, &master_point) * scalar;
    let shared_bytes = pairing_bytes(&shared)?;
    let sigma_mask = h2_pairing_mask(&identity_digest, &aad_digest, &shared_bytes);
    let mut v = [0_u8; TLE_SIGMA_BYTES_V1];
    xor_into(&mut v, &sigma, &sigma_mask);
    let message_mask = h4_message_mask(&identity_digest, &aad_digest, &sigma);
    let mut w = [0_u8; TLE_DEK_BYTES_V1];
    xor_into(&mut w, &dek, &message_mask);
    let transcript_hash = dem_transcript_hash(&identity_digest, &aad_digest, &u, &v, &w);
    let key = derive_dem_key(&dek, &transcript_hash)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| TleError::HkdfExpand)?;
    let aead_nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| TleError::InvalidEnvelope)?;
    let ciphertext = cipher
        .encrypt(
            &aead_nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| TleError::OpenFailed)?;
    let envelope = TleEnvelopeV1 {
        session_id: *identity.session().session_id(),
        identity_digest,
        aad_digest,
        u,
        v,
        w,
        nonce,
        ciphertext,
    };
    envelope.validate_public_shape()?;
    Ok(envelope)
}

fn validate_aad_len(aad: &[u8]) -> Result<(), TleError> {
    if aad.len() > TLE_MAX_AAD_BYTES_V1 {
        Err(TleError::AadTooLarge)
    } else {
        Ok(())
    }
}

fn canonical_aad(release_message: &[u8], caller_aad: &[u8]) -> Result<Vec<u8>, TleError> {
    validate_aad_len(caller_aad)?;
    let release_len = u32::try_from(release_message.len()).map_err(|_| TleError::AadTooLarge)?;
    let aad_len = u32::try_from(caller_aad.len()).map_err(|_| TleError::AadTooLarge)?;
    let mut aad =
        Vec::with_capacity(AAD_DOMAIN_V1.len() + 4 + release_message.len() + 4 + caller_aad.len());
    aad.extend_from_slice(AAD_DOMAIN_V1);
    aad.extend_from_slice(&release_len.to_be_bytes());
    aad.extend_from_slice(release_message);
    aad.extend_from_slice(&aad_len.to_be_bytes());
    aad.extend_from_slice(caller_aad);
    Ok(aad)
}

fn h2_pairing_mask(
    identity_digest: &[u8; 32],
    aad_digest: &[u8; 32],
    pairing_bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(H2_PAIRING_DOMAIN_V1);
    hasher.update(identity_digest);
    hasher.update(aad_digest);
    hasher.update(pairing_bytes);
    hasher.finalize().into()
}

fn h4_message_mask(
    identity_digest: &[u8; 32],
    aad_digest: &[u8; 32],
    sigma: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(H4_MESSAGE_DOMAIN_V1);
    hasher.update(identity_digest);
    hasher.update(aad_digest);
    hasher.update(sigma);
    hasher.finalize().into()
}

fn h3_nonzero_scalar(
    identity_digest: &[u8; 32],
    aad_digest: &[u8; 32],
    sigma: &[u8; 32],
    dek: &[u8; 32],
) -> Result<Scalar, TleError> {
    for counter in 0_u32..=u16::MAX.into() {
        let mut hasher = Sha512::new();
        hasher.update(H3_SCALAR_DOMAIN_V1);
        hasher.update(identity_digest);
        hasher.update(aad_digest);
        hasher.update(sigma);
        hasher.update(dek);
        hasher.update(counter.to_be_bytes());
        let digest = Zeroizing::new(<[u8; 64]>::from(hasher.finalize()));
        let mut candidate = Zeroizing::new([0_u8; 32]);
        candidate.copy_from_slice(&digest[..32]);
        if let Some(scalar) = Scalar::from_bytes_be(&candidate).into_option()
            && scalar != Scalar::from(0_u64)
        {
            return Ok(scalar);
        }
    }
    Err(TleError::ScalarDerivation)
}

fn dem_transcript_hash(
    identity_digest: &[u8; 32],
    aad_digest: &[u8; 32],
    u: &[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    v: &[u8; TLE_SIGMA_BYTES_V1],
    w: &[u8; TLE_DEK_BYTES_V1],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DEM_TRANSCRIPT_DOMAIN_V1);
    hasher.update(THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    hasher.update(TLE_PROTOCOL_VERSION_V1.to_be_bytes());
    hasher.update(identity_digest);
    hasher.update(aad_digest);
    hasher.update(u);
    hasher.update(v);
    hasher.update(w);
    hasher.finalize().into()
}

fn derive_dem_key(
    dek: &[u8; 32],
    transcript_hash: &[u8; 32],
) -> Result<Zeroizing<[u8; 32]>, TleError> {
    let hkdf = Hkdf::<Sha256>::new(Some(transcript_hash), dek);
    let mut key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(DEM_HKDF_INFO_V1, key.as_mut())
        .map_err(|_| TleError::HkdfExpand)?;
    Ok(key)
}

fn pairing_bytes(shared: &blstrs::Gt) -> Result<Vec<u8>, TleError> {
    let mut bytes = Vec::with_capacity(288);
    shared
        .write_compressed(&mut bytes)
        .map_err(|_| TleError::PairingEncoding)?;
    Ok(bytes)
}

fn decode_identity_secret(bytes: &[u8; 48]) -> Result<G1Affine, TleError> {
    let point = G1Affine::from_compressed(bytes)
        .into_option()
        .ok_or(TleError::InvalidIdentitySecretKey)?;
    if bool::from(point.is_identity()) || point.to_compressed() != *bytes {
        return Err(TleError::InvalidIdentitySecretKey);
    }
    Ok(point)
}

fn xor_into<const N: usize>(out: &mut [u8; N], left: &[u8; N], right: &[u8; N]) {
    for ((output, lhs), rhs) in out.iter_mut().zip(left).zip(right) {
        *output = *lhs ^ *rhs;
    }
}

fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], TleError> {
    let end = cursor.checked_add(N).ok_or(TleError::InvalidEnvelope)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(TleError::InvalidEnvelope)?
        .try_into()
        .map_err(|_| TleError::InvalidEnvelope)?;
    *cursor = end;
    Ok(value)
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

    fn fixture() -> (
        TleMasterPublicKey,
        TleReleaseIdentityV1,
        [u8; THRESHOLD_BLS_SIGNATURE_BYTES],
    ) {
        let session =
            ThresholdBlsSession::<TleReleasePurpose>::new(binding(1), binding(2), binding(3), 4, 2)
                .expect("session");
        let master_scalar = Scalar::from(7_u64);
        let master_bytes = (G2Affine::generator() * master_scalar)
            .to_affine()
            .to_compressed();
        let master = TleMasterPublicKey::from_bytes(*session.session_id(), &master_bytes)
            .expect("master key");
        let identity = TleReleaseIdentityV1::new(
            session,
            binding(4),
            binding(5),
            binding(6),
            binding(7),
            binding(8),
            1_000,
            binding(9),
        )
        .expect("identity");
        let release_message = identity.release_message().expect("release message");
        let secret = (hash_message_to_g1::<TleReleasePurpose>(&release_message) * master_scalar)
            .to_affine()
            .to_compressed();
        (master, identity, secret)
    }

    #[test]
    fn fullident_kem_dem_roundtrips_and_wire_is_canonical() {
        let (master, identity, secret) = fixture();
        let aad = b"proposal/attempt/seat/commitment-roots";
        let plaintext = b"masked ballot commitment opening";
        let mut rng = ChaCha20Rng::from_seed([42; 32]);
        let envelope =
            seal_tle_v1_with_rng(&master, &identity, aad, plaintext, &mut rng).expect("seal");
        let encoded = envelope.to_bytes().expect("encode envelope");
        let decoded = TleEnvelopeV1::from_bytes(&encoded).expect("canonical envelope");
        assert_eq!(decoded, envelope);
        let identity_key =
            TleIdentitySecretKeyV1::from_threshold_signature(master, &identity, &secret)
                .expect("identity key");
        assert_eq!(
            identity_key.open(&identity, aad, &decoded),
            Ok(plaintext.to_vec())
        );
    }

    #[test]
    fn envelope_rejects_wrong_aad_identity_and_session() {
        let (master, identity, secret) = fixture();
        let mut rng = ChaCha20Rng::from_seed([43; 32]);
        let envelope =
            seal_tle_v1_with_rng(&master, &identity, b"aad", b"opening", &mut rng).expect("seal");
        let identity_key =
            TleIdentitySecretKeyV1::from_threshold_signature(master, &identity, &secret)
                .expect("identity key");
        assert_eq!(
            identity_key.open(&identity, b"different", &envelope),
            Err(TleError::BindingMismatch)
        );

        let other_identity = TleReleaseIdentityV1::new(
            *identity.session(),
            binding(4),
            binding(5),
            binding(66),
            binding(7),
            binding(8),
            1_000,
            binding(9),
        )
        .expect("other identity");
        assert_eq!(
            identity_key.open(&other_identity, b"aad", &envelope),
            Err(TleError::BindingMismatch)
        );

        let other_session = ThresholdBlsSession::<TleReleasePurpose>::new(
            binding(1),
            binding(22),
            binding(3),
            4,
            2,
        )
        .expect("other session");
        let other_master_bytes = (G2Affine::generator() * Scalar::from(7_u64))
            .to_affine()
            .to_compressed();
        let other_master =
            TleMasterPublicKey::from_bytes(*other_session.session_id(), &other_master_bytes)
                .expect("other master");
        assert_eq!(
            seal_tle_v1_with_rng(
                &other_master,
                &identity,
                b"aad",
                b"opening",
                &mut ChaCha20Rng::from_seed([44; 32])
            ),
            Err(TleError::BindingMismatch)
        );
    }

    #[test]
    fn tampering_collapses_to_open_failed() {
        let (master, identity, secret) = fixture();
        let mut rng = ChaCha20Rng::from_seed([45; 32]);
        let envelope =
            seal_tle_v1_with_rng(&master, &identity, b"aad", b"opening", &mut rng).expect("seal");
        let identity_key =
            TleIdentitySecretKeyV1::from_threshold_signature(master, &identity, &secret)
                .expect("identity key");

        let mut kem_tampered = envelope.clone();
        kem_tampered.v[0] ^= 1;
        assert_eq!(
            identity_key.open(&identity, b"aad", &kem_tampered),
            Err(TleError::OpenFailed)
        );

        let mut dem_tampered = envelope;
        dem_tampered.ciphertext[0] ^= 1;
        assert_eq!(
            identity_key.open(&identity, b"aad", &dem_tampered),
            Err(TleError::OpenFailed)
        );
    }

    #[test]
    fn wrong_release_signature_is_rejected() {
        let (master, identity, _) = fixture();
        let wrong_secret = (hash_message_to_g1::<TleReleasePurpose>(
            &identity.release_message().expect("message"),
        ) * Scalar::from(8_u64))
        .to_affine()
        .to_compressed();
        assert!(matches!(
            TleIdentitySecretKeyV1::from_threshold_signature(master, &identity, &wrong_secret),
            Err(TleError::InvalidIdentitySecretKey)
        ));
    }

    #[test]
    fn parser_rejects_identity_points_trailing_bytes_and_inert_nonce() {
        let (master, identity, _) = fixture();
        let mut rng = ChaCha20Rng::from_seed([46; 32]);
        let envelope =
            seal_tle_v1_with_rng(&master, &identity, b"aad", b"opening", &mut rng).expect("seal");

        let mut identity_u = envelope.clone();
        identity_u.u = [0; THRESHOLD_BLS_PUBLIC_KEY_BYTES];
        identity_u.u[0] = 0xc0;
        assert_eq!(identity_u.to_bytes(), Err(TleError::InvalidEnvelope));

        let mut inert_nonce = envelope.clone();
        inert_nonce.nonce = [0; TLE_NONCE_BYTES_V1];
        assert_eq!(inert_nonce.to_bytes(), Err(TleError::InvalidEnvelope));

        let mut trailing = envelope.to_bytes().expect("encode envelope");
        trailing.push(0);
        assert_eq!(
            TleEnvelopeV1::from_bytes(&trailing),
            Err(TleError::InvalidEnvelope)
        );
    }

    #[test]
    fn size_and_inert_rng_bounds_fail_closed() {
        let (master, identity, _) = fixture();
        assert_eq!(
            seal_tle_v1_with_rng(
                &master,
                &identity,
                &vec![0; TLE_MAX_AAD_BYTES_V1 + 1],
                b"opening",
                &mut ChaCha20Rng::from_seed([47; 32])
            ),
            Err(TleError::AadTooLarge)
        );
        assert_eq!(
            seal_tle_v1_with_rng(
                &master,
                &identity,
                b"aad",
                &vec![0; TLE_MAX_PLAINTEXT_BYTES_V1 + 1],
                &mut ChaCha20Rng::from_seed([47; 32])
            ),
            Err(TleError::PlaintextTooLarge)
        );

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
        assert_eq!(
            seal_tle_v1_with_rng(&master, &identity, b"aad", b"opening", &mut ZeroRng),
            Err(TleError::InertRandomness)
        );
    }

    #[test]
    fn identity_rejects_zero_bindings_and_height() {
        let session =
            ThresholdBlsSession::<TleReleasePurpose>::new(binding(1), binding(2), binding(3), 4, 2)
                .expect("session");
        assert_eq!(
            TleReleaseIdentityV1::new(
                session,
                [0; 32],
                binding(5),
                binding(6),
                binding(7),
                binding(8),
                1_000,
                binding(9),
            ),
            Err(TleError::ZeroBinding)
        );
        assert_eq!(
            TleReleaseIdentityV1::new(
                session,
                binding(4),
                binding(5),
                binding(6),
                binding(7),
                binding(8),
                0,
                binding(9),
            ),
            Err(TleError::ZeroTargetHeight)
        );
    }
}
