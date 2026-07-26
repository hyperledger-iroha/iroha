//! Native Iroha testnet instantiation of ZK-AMS anonymous provisioning.
//!
//! The protocol workflow follows ZK-AMS v2, arXiv:2602.16130, Algorithms
//! 1--4 and Appendices A/C.  The paper intentionally leaves the concrete
//! linkable ring-signature group, hash, transcript, and wire unspecified.
//! This module closes Phase V to an LSAG instance over prime-order
//! Ristretto255 with SHA3-512 and supplies the holder-possession Schnorr
//! component composed with the admission relation. It is an Iroha
//! experimental profile and is not wire-compatible with the paper prototype.
//!
//! Batch admission remains fail-closed until the sibling setup-free,
//! zero-knowledge relaxed-R1CS finalizer is complete.  Keeping the complete
//! provisioning primitive here allows its algebra and wire to be tested
//! independently without making the protocol activatable.

use curve25519_dalek::{
    RistrettoPoint, constants::RISTRETTO_BASEPOINT_POINT, ristretto::CompressedRistretto,
    scalar::Scalar, traits::Identity,
};
use iroha_data_model::account::AccountId;
use iroha_data_model::privacy::{
    IrohaZkAmsStatementV1, PrivacyIssuerIdV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1,
    PrivacyRootV1, PrivacyStatementV1, PrivacyZkAmsActionV1, PrivacyZkAmsAdmissionAnchorV1,
    PrivacyZkAmsBatchAdmissionV1, PrivacyZkAmsIssuerPolicyRecordDigestV1, PrivacyZkAmsKeyImageV1,
    PrivacyZkAmsPersonhoodCredentialV1, PrivacyZkAmsProvisionAccountV1, PrivacyZkAmsRegistryIdV1,
    PrivacyZkAmsRegistryRecordDigestV1, PrivacyZkAmsSeedPublicKeyV1, ZK_AMS_PHC_VERSION_V1,
};
use iroha_zkp_halo2::vega::{
    MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1, MaskedRelaxedRandomErrorV1,
    MaskedRelaxedRandomSourceV1, ZkAmsAdmissionPublicInputV1, ZkAmsAdmissionRelationWitnessV1,
    ZkAmsMaskedProverConfigV1, ZkAmsProofContextV1, prove_zk_ams_admission_relation_v1,
    verify_zk_ams_admission_relation_v1,
};
use p256::{
    AffinePoint as P256AffinePoint, EncodedPoint as P256EncodedPoint, FieldBytes as P256FieldBytes,
    ProjectivePoint as P256ProjectivePoint, Scalar as P256Scalar,
    ecdsa::{
        Signature as P256Signature, VerifyingKey as P256VerifyingKey,
        signature::hazmat::PrehashVerifier as _,
    },
    elliptic_curve::{
        Field as _, PrimeField as _,
        bigint::U256,
        group::Group as _,
        ops::Reduce,
        sec1::{FromEncodedPoint as _, ToEncodedPoint as _},
    },
};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest as _, Sha256};
use sha3::{Digest, Sha3_256, Sha3_512};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::p256::{P256EngineError, TranscriptBindingV1};

/// Pinned source used for the Iroha ZK-AMS workflow and relation.
pub const ZK_AMS_SOURCE_PROFILE_V1: &[u8] = b"arxiv:2602.16130v2:algorithms-1-4:appendices-a-c";
/// Exact Iroha Phase-V suite label.
pub const ZK_AMS_LSAG_SUITE_V1: &[u8] = b"iroha-zk-ams-v1:phase-v:lsag-ristretto255-sha3-512";
/// Exact holder-possession suite composed with batch admission.
pub const ZK_AMS_ADMISSION_POSSESSION_SUITE_V1: &[u8] =
    b"iroha-zk-ams-v1:batch-admission:seed-possession-schnorr-ristretto255-sha3-512";
/// Hash-to-Ristretto domain for admitted seed public keys.
pub const ZK_AMS_HASH_TO_POINT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.lsag.hash-to-ristretto";
/// Canonical proof wire version.
pub const ZK_AMS_LSAG_PROOF_VERSION_V1: u8 = 1;
/// Canonical holder-possession proof wire version.
pub const ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1: u8 = 1;
/// Smallest closed Phase-V ring.
pub const ZK_AMS_MIN_RING_SIZE_V1: usize = 16;
/// Largest closed Phase-V ring.
pub const ZK_AMS_MAX_RING_SIZE_V1: usize = 64;
/// Exact admitted ring sizes.
pub const ZK_AMS_RING_SIZES_V1: [usize; 3] = [16, 32, 64];
/// Hard cap checked before Norito proof decoding.
pub const MAX_ZK_AMS_LSAG_PROOF_BYTES_V1: usize = 4 * 1024;
/// Hard cap checked before holder-possession proof decoding.
pub const MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1: usize = 256;
/// Hard cap checked before the composed batch proof is decoded.
pub const MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1: usize =
    MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1
        + 8 * MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1
        + 4 * 1024;

const RANDOM_REJECTION_ATTEMPTS: u32 = 1 << 16;
const TRANSCRIPT_VERSION_V1: u8 = 1;
const GENERATOR_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.generator-digest";
const REGISTRY_TRANSITION_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ams:registry-transition:v1";
const RELATION_PROOF_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ams:relation-proof:v1";

/// Failure while constructing, decoding, signing, or verifying ZK-AMS Phase V.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ZkAmsErrorV1 {
    /// A shared consensus transcript field is invalid.
    #[error("invalid ZK-AMS consensus transcript binding")]
    InvalidBinding,
    /// A transcript label or value cannot be represented canonically.
    #[error("ZK-AMS transcript field is too large")]
    TranscriptFieldTooLarge,
    /// A seed public key or key image is malformed, non-canonical, or identity.
    #[error("invalid canonical nonidentity Ristretto255 point")]
    InvalidPoint,
    /// A secret or proof scalar is not canonical.
    #[error("invalid canonical Ristretto255 scalar")]
    InvalidScalar,
    /// A secret seed scalar is zero.
    #[error("ZK-AMS seed secret must be nonzero")]
    ZeroSecret,
    /// The ring is not one of the closed first-release sizes.
    #[error("ZK-AMS ring size {actual} is not one of 16, 32, or 64")]
    InvalidRingSize {
        /// Supplied number of ring members.
        actual: usize,
    },
    /// The ring is not strictly increasing in canonical byte order.
    #[error("ZK-AMS ring must be strictly increasing and duplicate-free")]
    NonCanonicalRing,
    /// The signer index is outside the supplied ring.
    #[error("ZK-AMS signer index {index} is outside ring size {ring_size}")]
    SignerIndexOutOfBounds {
        /// Supplied signer index.
        index: usize,
        /// Supplied ring size.
        ring_size: usize,
    },
    /// The secret key does not derive the selected public ring member.
    #[error("ZK-AMS seed secret does not match the selected ring member")]
    SignerPublicKeyMismatch,
    /// The supplied key image does not derive from the selected seed secret.
    #[error("ZK-AMS key image does not match the selected seed secret")]
    KeyImageMismatch,
    /// The random source failed to yield a canonical nonzero scalar.
    #[error("ZK-AMS random scalar rejection sampling exhausted its work bound")]
    RandomnessExhausted,
    /// Proof bytes exceed the dedicated decoder cap.
    #[error("ZK-AMS LSAG proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Supplied proof bytes.
        actual: usize,
        /// Hard maximum.
        max: usize,
    },
    /// Holder-possession proof bytes exceed the dedicated decoder cap.
    #[error("ZK-AMS admission possession proof length {actual} exceeds hard maximum {max}")]
    PossessionProofTooLarge {
        /// Supplied proof bytes.
        actual: usize,
        /// Hard maximum.
        max: usize,
    },
    /// Exact Norito decode, shape validation, or canonical re-encoding failed.
    #[error("invalid canonical ZK-AMS LSAG proof encoding")]
    InvalidProofEncoding,
    /// The closed LSAG verification equation failed.
    #[error("ZK-AMS LSAG verification failed")]
    VerificationFailed,
    /// The typed statement is not a valid batch-admission statement.
    #[error("invalid typed ZK-AMS batch-admission statement")]
    InvalidStatement,
    /// The transcript binding differs from the complete typed statement.
    #[error("ZK-AMS statement/transcript binding mismatch")]
    BindingMismatch,
    /// Credential witness count or order differs from public anchors.
    #[error("ZK-AMS admission credential witnesses do not match ordered anchors")]
    CredentialMismatch,
    /// A PHC version, hidden field, or identifier is outside the fixed profile.
    #[error("invalid canonical ZK-AMS Personhood Credential")]
    InvalidCredential,
    /// The issuer key is malformed, non-canonical, or identity.
    #[error("invalid canonical ZK-AMS P-256 issuer key")]
    InvalidIssuerKey,
    /// An issuer signature is malformed or does not verify.
    #[error("invalid ZK-AMS issuer ES256 signature")]
    InvalidIssuerSignature,
    /// High-s issuer signatures are forbidden instead of normalized.
    #[error("non-canonical high-s ZK-AMS issuer signature")]
    HighSIssuerSignature,
    /// The declared final registry root is not the exact ordered transition.
    #[error("ZK-AMS ordered registry transition root mismatch")]
    RegistryTransitionMismatch,
    /// Composed batch proof exceeds its pre-decode cap.
    #[error("ZK-AMS batch proof length {actual} exceeds hard maximum {max}")]
    BatchProofTooLarge {
        /// Supplied proof length.
        actual: usize,
        /// Exact hard maximum.
        max: usize,
    },
    /// The native masked relation engine rejected witness, context, or proof.
    #[error("ZK-AMS masked admission relation failed")]
    AdmissionRelation,
}

impl From<P256EngineError> for ZkAmsErrorV1 {
    fn from(_: P256EngineError) -> Self {
        Self::InvalidBinding
    }
}

/// Zeroizing canonical little-endian Ristretto scalar used as a seed secret.
pub struct ZkAmsSeedSecretV1 {
    bytes: Zeroizing<[u8; 32]>,
}

impl ZkAmsSeedSecretV1 {
    /// Parse one canonical nonzero seed secret.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-canonical or zero scalar.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ZkAmsErrorV1> {
        let scalar = scalar_from_canonical(bytes)?;
        if scalar == Scalar::ZERO {
            return Err(ZkAmsErrorV1::ZeroSecret);
        }
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    /// Sample one unbiased canonical nonzero scalar.
    ///
    /// # Errors
    ///
    /// Returns an error when the random source does not produce an admitted
    /// canonical scalar within the fixed work bound.
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Result<Self, ZkAmsErrorV1> {
        let scalar = random_nonzero_scalar(rng)?;
        let mut bytes = scalar.to_bytes();
        let secret = Self::from_bytes(bytes);
        bytes.zeroize();
        secret
    }

    fn expose_scalar(&self) -> Scalar {
        scalar_from_canonical(*self.bytes)
            .expect("ZK-AMS seed secret was validated at construction")
    }
}

impl core::fmt::Debug for ZkAmsSeedSecretV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsSeedSecretV1([REDACTED])")
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct ZkAmsLsagProofWireV1 {
    version: u8,
    initial_challenge: [u8; 32],
    responses: Vec<[u8; 32]>,
}

impl Zeroize for ZkAmsLsagProofWireV1 {
    fn zeroize(&mut self) {
        self.version.zeroize();
        self.initial_challenge.zeroize();
        self.responses.zeroize();
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct ZkAmsAdmissionPossessionProofWireV1 {
    version: u8,
    commitment: [u8; 32],
    response: [u8; 32],
}

impl Zeroize for ZkAmsAdmissionPossessionProofWireV1 {
    fn zeroize(&mut self) {
        self.version.zeroize();
        self.commitment.zeroize();
        self.response.zeroize();
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct ZkAmsBatchAdmissionProofWireV1 {
    version: u8,
    relation_proof: Vec<u8>,
    possession_proofs: Vec<Vec<u8>>,
}

impl Zeroize for ZkAmsBatchAdmissionProofWireV1 {
    fn zeroize(&mut self) {
        self.version.zeroize();
        self.relation_proof.zeroize();
        self.possession_proofs.zeroize();
    }
}

/// Borrowed secret material for one ordered admission anchor.
pub struct ZkAmsBatchCredentialWitnessV1<'a> {
    credential: &'a PrivacyZkAmsPersonhoodCredentialV1,
    issuer_signature: &'a [u8; 64],
    seed_secret: &'a ZkAmsSeedSecretV1,
}

impl<'a> ZkAmsBatchCredentialWitnessV1<'a> {
    /// Construct a borrowed admission witness. Exact credential, signature,
    /// anchor, and seed-key consistency is checked by the prover.
    #[must_use]
    pub const fn new(
        credential: &'a PrivacyZkAmsPersonhoodCredentialV1,
        issuer_signature: &'a [u8; 64],
        seed_secret: &'a ZkAmsSeedSecretV1,
    ) -> Self {
        Self {
            credential,
            issuer_signature,
            seed_secret,
        }
    }
}

impl core::fmt::Debug for ZkAmsBatchCredentialWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsBatchCredentialWitnessV1([REDACTED])")
    }
}

struct ZkAmsIssuerSignatureWitnessV1 {
    r: Zeroizing<[u8; 32]>,
    s: Zeroizing<[u8; 32]>,
    recovery_x: Zeroizing<[u8; 32]>,
    recovery_y: Zeroizing<[u8; 32]>,
}

/// Atomic state effect certified by a complete batch-admission proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedZkAmsBatchAdmissionV1 {
    /// Credential issuer namespace.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Admission policy namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Governed policy digest that runtime must match authoritatively.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Authoritative issuer/policy record digest to match.
    pub issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    /// Admitted-identity registry namespace.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Authoritative prior registry-record digest to match.
    pub registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    /// Exact current registry root.
    pub current_root: PrivacyRootV1,
    /// Epoch of `current_root`.
    pub current_epoch: u64,
    /// Exact resulting registry root.
    pub next_root: PrivacyRootV1,
    /// Successor epoch of `next_root`.
    pub next_epoch: u64,
    /// Ordered anchors to insert atomically after duplicate-state checks.
    pub anchors: Vec<PrivacyZkAmsAdmissionAnchorV1>,
}

/// Atomic state effect certified by one anonymous account-provisioning proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedZkAmsProvisionAccountV1 {
    /// Credential issuer namespace.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Admission policy namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Governed policy digest runtime must match authoritatively.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Authoritative issuer/key/policy record digest.
    pub issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    /// Admitted-identity registry namespace.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Authoritative current registry-snapshot record digest.
    pub registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    /// Exact current admitted-identity root.
    pub current_root: PrivacyRootV1,
    /// Exact current admitted-identity epoch.
    pub current_epoch: u64,
    /// Strictly ordered seed-key anonymity ring.
    pub ring: Vec<PrivacyZkAmsSeedPublicKeyV1>,
    /// Fresh target account to create and bind atomically.
    pub account_id: AccountId,
    /// Deterministic one-time provisioning replay marker.
    pub key_image: PrivacyZkAmsKeyImageV1,
}

/// Derive the canonical admitted seed public key.
#[must_use]
pub fn zk_ams_seed_public_key_v1(secret: &ZkAmsSeedSecretV1) -> [u8; 32] {
    (secret.expose_scalar() * RISTRETTO_BASEPOINT_POINT)
        .compress()
        .to_bytes()
}

/// Derive the deterministic Phase-V key image used as a replay nullifier.
///
/// # Errors
///
/// Returns an error only if the derived point is the identity, a
/// cryptographically negligible event that is nevertheless rejected.
pub fn zk_ams_key_image_v1(secret: &ZkAmsSeedSecretV1) -> Result<[u8; 32], ZkAmsErrorV1> {
    let public = zk_ams_seed_public_key_v1(secret);
    let hash_point = hash_public_key_to_point(&public)?;
    let key_image = secret.expose_scalar() * hash_point;
    if key_image == RistrettoPoint::identity() {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(key_image.compress().to_bytes())
}

/// Return the digest of the exact Ristretto generator and hash-to-point suite.
#[must_use]
pub fn zk_ams_generator_digest_v1() -> [u8; 32] {
    let mut hash = Sha3_256::new();
    hash.update(GENERATOR_DIGEST_DOMAIN_V1);
    hash.update(ZK_AMS_LSAG_SUITE_V1);
    hash.update(RISTRETTO_BASEPOINT_POINT.compress().as_bytes());
    hash.update(ZK_AMS_HASH_TO_POINT_DOMAIN_V1);
    hash.update(iroha_zkp_halo2::vega::zk_ams_t256_generator_digest_v1());
    hash.update(iroha_zkp_halo2::vega::zk_ams_compiled_profile_digest_v1());
    hash.finalize().into()
}

/// Compute the exact ordered registry root after one public anchor.
#[must_use]
pub fn zk_ams_registry_transition_root_v1(
    registry_id: PrivacyZkAmsRegistryIdV1,
    prior_root: PrivacyRootV1,
    current_epoch: u64,
    next_epoch: u64,
    batch_size: u32,
    anchor_index: u32,
    anchor: PrivacyZkAmsAdmissionAnchorV1,
) -> PrivacyRootV1 {
    let mut hash = Sha256::new();
    hash.update(REGISTRY_TRANSITION_DOMAIN_V1);
    hash.update(registry_id.as_bytes());
    hash.update(prior_root.as_bytes());
    hash.update(current_epoch.to_be_bytes());
    hash.update(next_epoch.to_be_bytes());
    hash.update(batch_size.to_be_bytes());
    hash.update(anchor_index.to_be_bytes());
    hash.update(anchor.phc_hash.as_bytes());
    hash.update(anchor.seed_public_key.as_bytes());
    PrivacyRootV1::new(hash.finalize().into())
}

/// Prove one complete ordered ZK-AMS credential-admission batch.
///
/// The returned envelope contains the masked relaxed-R1CS proof plus one
/// transcript-bound Ristretto Schnorr possession proof per anchor. The prover
/// runs the public verifier before releasing bytes.
///
/// # Errors
///
/// Fails closed for statement/binding drift, malformed credentials or low-s
/// signatures, seed mismatch, root-transition mismatch, random failure, or
/// native relation failure.
pub fn prove_zk_ams_batch_admission_v1<R: CryptoRng + RngCore>(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
    witnesses: &[ZkAmsBatchCredentialWitnessV1<'_>],
    config: ZkAmsMaskedProverConfigV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAmsErrorV1> {
    let (public_inputs, issuer_key) = build_admission_public_inputs(statement, binding)?;
    if witnesses.len() != public_inputs.len() {
        return Err(ZkAmsErrorV1::CredentialMismatch);
    }
    let batch = batch_action(statement)?;
    let mut signatures = Vec::with_capacity(witnesses.len());
    for ((witness, public), anchor) in witnesses
        .iter()
        .zip(public_inputs.iter())
        .zip(batch.anchors.iter())
    {
        validate_credential_witness(statement, anchor, witness)?;
        signatures.push(validate_issuer_signature(
            witness.issuer_signature,
            public.phc_hash,
            &issuer_key,
        )?);
    }
    let relation_witnesses = witnesses
        .iter()
        .zip(&signatures)
        .map(|(witness, signature)| {
            ZkAmsAdmissionRelationWitnessV1::new(
                witness.credential.subject_commitment.as_bytes(),
                witness.credential.credential_nonce.as_bytes(),
                &signature.r,
                &signature.s,
                &signature.recovery_x,
                &signature.recovery_y,
            )
            .map_err(|_| ZkAmsErrorV1::InvalidCredential)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let relation_context = relation_context(binding);
    let relation_proof = {
        let mut adapter = MaskedRandomAdapter(rng);
        prove_zk_ams_admission_relation_v1(
            &relation_context,
            &public_inputs,
            &relation_witnesses,
            config,
            &mut adapter,
        )
        .map_err(|_| ZkAmsErrorV1::AdmissionRelation)?
    };
    let relation_digest = relation_proof_digest(&relation_proof);
    let possession_proofs = witnesses
        .iter()
        .zip(batch.anchors.iter())
        .enumerate()
        .map(|(index, (witness, anchor))| {
            prove_zk_ams_admission_possession_v1(
                binding,
                u32::try_from(index).expect("batch is bounded to eight"),
                *anchor.phc_hash.as_bytes(),
                *anchor.seed_public_key.as_bytes(),
                relation_digest,
                witness.seed_secret,
                rng,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let proof = Zeroizing::new(ZkAmsBatchAdmissionProofWireV1 {
        version: ZK_AMS_LSAG_PROOF_VERSION_V1,
        relation_proof,
        possession_proofs,
    });
    let encoded = Zeroizing::new(norito::codec::encode_adaptive(&*proof));
    if encoded.len() > MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::BatchProofTooLarge {
            actual: encoded.len(),
            max: MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,
        });
    }
    verify_zk_ams_batch_admission_v1(statement, binding, encoded.as_slice())?;
    Ok(encoded.to_vec())
}

/// Verify the complete batch composition and return one atomic state effect.
///
/// Runtime must still match the returned issuer/policy and registry record
/// digests against authoritative state and reject any already-admitted PHC or
/// seed key before applying all anchors atomically.
///
/// # Errors
///
/// Oversized input is rejected before Norito. Exact decoding, relation
/// verification, every possession proof, and the final transition root must
/// all succeed before an effect is returned.
pub fn verify_zk_ams_batch_admission_v1(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
    proof_bytes: &[u8],
) -> Result<VerifiedZkAmsBatchAdmissionV1, ZkAmsErrorV1> {
    if proof_bytes.len() > MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::BatchProofTooLarge {
            actual: proof_bytes.len(),
            max: MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,
        });
    }
    let (public_inputs, _) = build_admission_public_inputs(statement, binding)?;
    let batch = batch_action(statement)?;
    let proof =
        norito::codec::decode_exact_from_slice::<ZkAmsBatchAdmissionProofWireV1>(proof_bytes)
            .map_err(|_| ZkAmsErrorV1::InvalidProofEncoding)?;
    if proof.version != ZK_AMS_LSAG_PROOF_VERSION_V1
        || proof.possession_proofs.len() != batch.anchors.len()
        || norito::codec::encode_adaptive(&proof) != proof_bytes
    {
        return Err(ZkAmsErrorV1::InvalidProofEncoding);
    }
    let relation_context = relation_context(binding);
    verify_zk_ams_admission_relation_v1(&relation_context, &public_inputs, &proof.relation_proof)
        .map_err(|_| ZkAmsErrorV1::AdmissionRelation)?;
    let relation_digest = relation_proof_digest(&proof.relation_proof);
    for (index, (anchor, possession)) in batch
        .anchors
        .iter()
        .zip(&proof.possession_proofs)
        .enumerate()
    {
        verify_zk_ams_admission_possession_v1(
            binding,
            u32::try_from(index).expect("batch is bounded to eight"),
            *anchor.phc_hash.as_bytes(),
            *anchor.seed_public_key.as_bytes(),
            relation_digest,
            possession,
        )?;
    }
    Ok(VerifiedZkAmsBatchAdmissionV1 {
        issuer_id: statement.issuer_id,
        policy_id: statement.policy_id,
        policy_digest: statement.policy_digest,
        issuer_policy_record_digest: statement.issuer_policy_record_digest,
        registry_id: statement.registry_id,
        registry_record_digest: statement.registry_record_digest,
        current_root: batch.account_registry_root,
        current_epoch: batch.account_registry_root_epoch,
        next_root: batch.next_account_registry_root,
        next_epoch: batch.next_account_registry_root_epoch,
        anchors: batch.anchors.clone(),
    })
}

/// Sign one account-provisioning statement with the selected seed secret.
///
/// `binding.statement_digest` is the digest of the complete typed ZK-AMS
/// statement, including account id, ordered ring, root/epoch, and key image.
///
/// # Errors
///
/// Fails closed for a malformed ring or key image, a mismatched secret, an
/// invalid consensus binding, or random-source exhaustion.
pub fn sign_zk_ams_provision_v1<R: CryptoRng + RngCore>(
    binding: &TranscriptBindingV1<'_>,
    ring: &[[u8; 32]],
    key_image_bytes: [u8; 32],
    signer_index: usize,
    secret: &ZkAmsSeedSecretV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAmsErrorV1> {
    binding.validate()?;
    let ring_points = validate_ring(ring)?;
    let ring_size = ring.len();
    if signer_index >= ring_size {
        return Err(ZkAmsErrorV1::SignerIndexOutOfBounds {
            index: signer_index,
            ring_size,
        });
    }
    let secret_scalar = Zeroizing::new(secret.expose_scalar());
    if *secret_scalar * RISTRETTO_BASEPOINT_POINT != ring_points[signer_index] {
        return Err(ZkAmsErrorV1::SignerPublicKeyMismatch);
    }
    let key_image = decode_nonidentity_point(key_image_bytes)?;
    let expected_image = *secret_scalar * hash_public_key_to_point(&ring[signer_index])?;
    if key_image != expected_image {
        return Err(ZkAmsErrorV1::KeyImageMismatch);
    }

    let transcript = LsagTranscriptV1::new(binding, ring, key_image_bytes)?;
    let mut alpha = Zeroizing::new(random_nonzero_scalar(rng)?);
    let mut responses = Zeroizing::new(vec![Scalar::ZERO; ring_size]);
    for (index, response) in responses.iter_mut().enumerate() {
        if index != signer_index {
            *response = random_nonzero_scalar(rng)?;
        }
    }
    let mut challenges = Zeroizing::new(vec![Scalar::ZERO; ring_size]);
    let next = (signer_index + 1) % ring_size;
    let signer_hash_point = hash_public_key_to_point(&ring[signer_index])?;
    challenges[next] = transcript.challenge(
        signer_index,
        *alpha * RISTRETTO_BASEPOINT_POINT,
        *alpha * signer_hash_point,
    )?;
    let mut index = next;
    while index != signer_index {
        let hash_point = hash_public_key_to_point(&ring[index])?;
        let left =
            responses[index] * RISTRETTO_BASEPOINT_POINT + challenges[index] * ring_points[index];
        let right = responses[index] * hash_point + challenges[index] * key_image;
        challenges[(index + 1) % ring_size] = transcript.challenge(index, left, right)?;
        index = (index + 1) % ring_size;
    }
    responses[signer_index] = *alpha - challenges[signer_index] * *secret_scalar;
    alpha.zeroize();

    let proof = Zeroizing::new(ZkAmsLsagProofWireV1 {
        version: ZK_AMS_LSAG_PROOF_VERSION_V1,
        initial_challenge: challenges[0].to_bytes(),
        responses: responses.iter().map(Scalar::to_bytes).collect(),
    });
    let encoded = Zeroizing::new(norito::codec::encode_adaptive(&*proof));
    if encoded.len() > MAX_ZK_AMS_LSAG_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::ProofTooLarge {
            actual: encoded.len(),
            max: MAX_ZK_AMS_LSAG_PROOF_BYTES_V1,
        });
    }
    verify_zk_ams_provision_v1(binding, ring, key_image_bytes, encoded.as_slice())?;
    // The returned proof is public. The guarded construction copy is erased
    // on every early return, including cap, encoding, and self-check failures.
    Ok(encoded.to_vec())
}

/// Sign one complete typed ZK-AMS account-provisioning statement.
///
/// The wrapper validates and transcript-binds every authoritative record,
/// current root/epoch, ring key, target account, and key image before invoking
/// the LSAG prover, then runs the complete typed verifier before release.
pub fn sign_zk_ams_provision_statement_v1<R: CryptoRng + RngCore>(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
    signer_index: usize,
    secret: &ZkAmsSeedSecretV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAmsErrorV1> {
    let provision = validate_provision_statement(statement, binding)?;
    let ring = provision
        .admitted_seed_key_ring
        .iter()
        .map(|key| *key.as_bytes())
        .collect::<Vec<_>>();
    let encoded = sign_zk_ams_provision_v1(
        binding,
        &ring,
        *provision.key_image.as_bytes(),
        signer_index,
        secret,
        rng,
    )?;
    verify_zk_ams_provision_statement_v1(statement, binding, &encoded)?;
    Ok(encoded)
}

/// Prove possession of the seed scalar for one ordered admission anchor.
///
/// This proof is intentionally a separate composed Schnorr component, not an
/// R1CS claim. Its Fiat--Shamir challenge binds the complete consensus
/// transcript, exact anchor, and digest of the masked relaxed-R1CS proof.
///
/// # Errors
///
/// Fails closed for an invalid binding or point, a mismatched seed secret,
/// random-source exhaustion, or an internal verifier self-check failure.
pub fn prove_zk_ams_admission_possession_v1<R: CryptoRng + RngCore>(
    binding: &TranscriptBindingV1<'_>,
    anchor_index: u32,
    phc_hash: [u8; 32],
    seed_public_key: [u8; 32],
    relation_proof_digest: [u8; 32],
    secret: &ZkAmsSeedSecretV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAmsErrorV1> {
    binding.validate()?;
    if phc_hash == [0; 32] || relation_proof_digest == [0; 32] {
        return Err(ZkAmsErrorV1::InvalidBinding);
    }
    let public = decode_nonidentity_point(seed_public_key)?;
    let secret_scalar = Zeroizing::new(secret.expose_scalar());
    if *secret_scalar * RISTRETTO_BASEPOINT_POINT != public {
        return Err(ZkAmsErrorV1::SignerPublicKeyMismatch);
    }
    let nonce = Zeroizing::new(random_nonzero_scalar(rng)?);
    let commitment = *nonce * RISTRETTO_BASEPOINT_POINT;
    let challenge = admission_possession_challenge(
        binding,
        anchor_index,
        phc_hash,
        seed_public_key,
        relation_proof_digest,
        commitment,
    )?;
    let response = Zeroizing::new(*nonce + challenge * *secret_scalar);
    let proof = Zeroizing::new(ZkAmsAdmissionPossessionProofWireV1 {
        version: ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1,
        commitment: commitment.compress().to_bytes(),
        response: response.to_bytes(),
    });
    let encoded = Zeroizing::new(norito::codec::encode_adaptive(&*proof));
    if encoded.len() > MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::PossessionProofTooLarge {
            actual: encoded.len(),
            max: MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1,
        });
    }
    verify_zk_ams_admission_possession_v1(
        binding,
        anchor_index,
        phc_hash,
        seed_public_key,
        relation_proof_digest,
        encoded.as_slice(),
    )?;
    Ok(encoded.to_vec())
}

/// Verify the transcript-composed seed-possession proof for one anchor.
///
/// # Errors
///
/// Rejects oversized or non-canonical Norito, malformed points/scalars,
/// wrong-suite material, mutated transcript fields, and failed equations.
pub fn verify_zk_ams_admission_possession_v1(
    binding: &TranscriptBindingV1<'_>,
    anchor_index: u32,
    phc_hash: [u8; 32],
    seed_public_key: [u8; 32],
    relation_proof_digest: [u8; 32],
    proof_bytes: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    binding.validate()?;
    if phc_hash == [0; 32] || relation_proof_digest == [0; 32] {
        return Err(ZkAmsErrorV1::InvalidBinding);
    }
    if proof_bytes.len() > MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::PossessionProofTooLarge {
            actual: proof_bytes.len(),
            max: MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1,
        });
    }
    let public = decode_nonidentity_point(seed_public_key)?;
    let proof =
        norito::codec::decode_exact_from_slice::<ZkAmsAdmissionPossessionProofWireV1>(proof_bytes)
            .map_err(|_| ZkAmsErrorV1::InvalidProofEncoding)?;
    if proof.version != ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1
        || norito::codec::encode_adaptive(&proof) != proof_bytes
    {
        return Err(ZkAmsErrorV1::InvalidProofEncoding);
    }
    let commitment = decode_nonidentity_point(proof.commitment)?;
    let response = scalar_from_canonical(proof.response)?;
    let challenge = admission_possession_challenge(
        binding,
        anchor_index,
        phc_hash,
        seed_public_key,
        relation_proof_digest,
        commitment,
    )?;
    if response * RISTRETTO_BASEPOINT_POINT != commitment + challenge * public {
        return Err(ZkAmsErrorV1::VerificationFailed);
    }
    Ok(())
}

/// Verify one canonical Phase-V LSAG proof.
///
/// # Errors
///
/// Fails closed before allocation for oversized proof bytes, then rejects
/// non-canonical Norito, scalars, points, ring order, or verification
/// equations.
pub fn verify_zk_ams_provision_v1(
    binding: &TranscriptBindingV1<'_>,
    ring: &[[u8; 32]],
    key_image_bytes: [u8; 32],
    proof_bytes: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    binding.validate()?;
    if proof_bytes.len() > MAX_ZK_AMS_LSAG_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::ProofTooLarge {
            actual: proof_bytes.len(),
            max: MAX_ZK_AMS_LSAG_PROOF_BYTES_V1,
        });
    }
    let ring_points = validate_ring(ring)?;
    let key_image = decode_nonidentity_point(key_image_bytes)?;
    let proof = norito::codec::decode_exact_from_slice::<ZkAmsLsagProofWireV1>(proof_bytes)
        .map_err(|_| ZkAmsErrorV1::InvalidProofEncoding)?;
    if proof.version != ZK_AMS_LSAG_PROOF_VERSION_V1
        || proof.responses.len() != ring.len()
        || norito::codec::encode_adaptive(&proof) != proof_bytes
    {
        return Err(ZkAmsErrorV1::InvalidProofEncoding);
    }
    let mut challenge = scalar_from_canonical(proof.initial_challenge)?;
    let responses = proof
        .responses
        .into_iter()
        .map(scalar_from_canonical)
        .collect::<Result<Vec<_>, _>>()?;
    let transcript = LsagTranscriptV1::new(binding, ring, key_image_bytes)?;
    for (index, ((public_key, response), public_bytes)) in ring_points
        .iter()
        .copied()
        .zip(responses.iter().copied())
        .zip(ring.iter())
        .enumerate()
    {
        let hash_point = hash_public_key_to_point(public_bytes)?;
        let left = response * RISTRETTO_BASEPOINT_POINT + challenge * public_key;
        let right = response * hash_point + challenge * key_image;
        challenge = transcript.challenge(index, left, right)?;
    }
    if challenge.to_bytes() != proof.initial_challenge {
        return Err(ZkAmsErrorV1::VerificationFailed);
    }
    Ok(())
}

/// Verify one complete typed provisioning statement and derive its atomic
/// ledger effect.
pub fn verify_zk_ams_provision_statement_v1(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
    proof_bytes: &[u8],
) -> Result<VerifiedZkAmsProvisionAccountV1, ZkAmsErrorV1> {
    let provision = validate_provision_statement(statement, binding)?;
    let ring = provision
        .admitted_seed_key_ring
        .iter()
        .map(|key| *key.as_bytes())
        .collect::<Vec<_>>();
    verify_zk_ams_provision_v1(binding, &ring, *provision.key_image.as_bytes(), proof_bytes)?;
    Ok(VerifiedZkAmsProvisionAccountV1 {
        issuer_id: statement.issuer_id,
        policy_id: statement.policy_id,
        policy_digest: statement.policy_digest,
        issuer_policy_record_digest: statement.issuer_policy_record_digest,
        registry_id: statement.registry_id,
        registry_record_digest: statement.registry_record_digest,
        current_root: provision.account_registry_root,
        current_epoch: provision.account_registry_root_epoch,
        ring: provision.admitted_seed_key_ring.clone(),
        account_id: provision.account_id.clone(),
        key_image: provision.key_image,
    })
}

fn validate_ring(ring: &[[u8; 32]]) -> Result<Vec<RistrettoPoint>, ZkAmsErrorV1> {
    if !ZK_AMS_RING_SIZES_V1.contains(&ring.len()) {
        return Err(ZkAmsErrorV1::InvalidRingSize { actual: ring.len() });
    }
    if ring.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ZkAmsErrorV1::NonCanonicalRing);
    }
    ring.iter().copied().map(decode_nonidentity_point).collect()
}

fn decode_nonidentity_point(bytes: [u8; 32]) -> Result<RistrettoPoint, ZkAmsErrorV1> {
    let point = CompressedRistretto(bytes)
        .decompress()
        .ok_or(ZkAmsErrorV1::InvalidPoint)?;
    if point == RistrettoPoint::identity() || point.compress().to_bytes() != bytes {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn hash_public_key_to_point(bytes: &[u8; 32]) -> Result<RistrettoPoint, ZkAmsErrorV1> {
    let mut hash = Sha3_512::new();
    hash.update(ZK_AMS_HASH_TO_POINT_DOMAIN_V1);
    hash.update(
        u16::try_from(ZK_AMS_LSAG_SUITE_V1.len())
            .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
            .to_be_bytes(),
    );
    hash.update(ZK_AMS_LSAG_SUITE_V1);
    hash.update(bytes);
    let uniform: [u8; 64] = hash.finalize().into();
    let point = RistrettoPoint::from_uniform_bytes(&uniform);
    if point == RistrettoPoint::identity() {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn admission_possession_challenge(
    binding: &TranscriptBindingV1<'_>,
    anchor_index: u32,
    phc_hash: [u8; 32],
    seed_public_key: [u8; 32],
    relation_proof_digest: [u8; 32],
    commitment: RistrettoPoint,
) -> Result<Scalar, ZkAmsErrorV1> {
    if commitment == RistrettoPoint::identity() {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    let mut hash = Sha3_512::new();
    append_field(&mut hash, b"domain", ZK_AMS_ADMISSION_POSSESSION_SUITE_V1)?;
    append_field(&mut hash, b"transcript_version", &[TRANSCRIPT_VERSION_V1])?;
    append_field(&mut hash, b"chain_id", binding.chain_id)?;
    append_field(&mut hash, b"genesis_hash", &binding.genesis_hash)?;
    append_field(
        &mut hash,
        b"action_index",
        &binding.action_index.to_be_bytes(),
    )?;
    append_field(&mut hash, b"statement_digest", &binding.statement_digest)?;
    append_field(&mut hash, b"parameter_id", &binding.parameter_id)?;
    append_field(&mut hash, b"parameter_digest", &binding.parameter_digest)?;
    append_field(&mut hash, b"verifier_digest", &binding.verifier_digest)?;
    append_field(
        &mut hash,
        b"statement_schema_digest",
        &binding.statement_schema_digest,
    )?;
    append_field(
        &mut hash,
        b"engine_manifest_digest",
        &binding.engine_manifest_digest,
    )?;
    append_field(&mut hash, b"generator_digest", &binding.generator_digest)?;
    append_field(&mut hash, b"anchor_index", &anchor_index.to_be_bytes())?;
    append_field(&mut hash, b"phc_hash", &phc_hash)?;
    append_field(&mut hash, b"seed_public_key", &seed_public_key)?;
    append_field(&mut hash, b"relation_proof_digest", &relation_proof_digest)?;
    append_field(&mut hash, b"commitment", commitment.compress().as_bytes())?;
    let wide: [u8; 64] = hash.finalize().into();
    Ok(Scalar::from_bytes_mod_order_wide(&wide))
}

fn batch_action(
    statement: &IrohaZkAmsStatementV1,
) -> Result<&PrivacyZkAmsBatchAdmissionV1, ZkAmsErrorV1> {
    match &statement.action {
        PrivacyZkAmsActionV1::BatchAdmission(batch) => Ok(batch),
        PrivacyZkAmsActionV1::ProvisionAccount(_) => Err(ZkAmsErrorV1::InvalidStatement),
    }
}

fn provision_action(
    statement: &IrohaZkAmsStatementV1,
) -> Result<&PrivacyZkAmsProvisionAccountV1, ZkAmsErrorV1> {
    match &statement.action {
        PrivacyZkAmsActionV1::ProvisionAccount(provision) => Ok(provision),
        PrivacyZkAmsActionV1::BatchAdmission(_) => Err(ZkAmsErrorV1::InvalidStatement),
    }
}

fn validate_provision_statement<'a>(
    statement: &'a IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<&'a PrivacyZkAmsProvisionAccountV1, ZkAmsErrorV1> {
    validate_statement_binding(statement, binding)?;
    let provision = provision_action(statement)?;
    if statement.issuer_id.is_zero()
        || statement.policy_id.is_zero()
        || statement.registry_id.is_zero()
        || statement.issuer_policy_record_digest.is_zero()
        || statement.registry_record_digest.is_zero()
        || statement.policy_digest.is_zero()
        || provision.account_registry_root.is_zero()
        || provision.account_registry_root_epoch == 0
        || provision.key_image.is_zero()
    {
        return Err(ZkAmsErrorV1::InvalidStatement);
    }
    let issuer_key = P256VerifyingKey::from_sec1_bytes(statement.issuer_public_key.as_bytes())
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerKey)?;
    if issuer_key.to_encoded_point(true).as_bytes() != statement.issuer_public_key.as_bytes() {
        return Err(ZkAmsErrorV1::InvalidIssuerKey);
    }
    let ring = provision
        .admitted_seed_key_ring
        .iter()
        .map(|key| *key.as_bytes())
        .collect::<Vec<_>>();
    validate_ring(&ring)?;
    decode_nonidentity_point(*provision.key_image.as_bytes())?;
    Ok(provision)
}

fn validate_statement_binding(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<(), ZkAmsErrorV1> {
    binding.validate()?;
    let context = &statement.context;
    if binding.chain_id != context.chain_id.as_str().as_bytes()
        || binding.action_index != context.action_index
        || binding.parameter_id != *context.parameter_id.as_bytes()
        || binding.parameter_digest != *context.parameter_digest.as_bytes()
        || binding.verifier_digest != *context.verifier_digest.as_bytes()
        || binding.statement_schema_digest != *context.statement_schema_digest.as_bytes()
        || binding.engine_manifest_digest != *context.engine_manifest_digest.as_bytes()
        || binding.generator_digest != zk_ams_generator_digest_v1()
        || context.transaction_intent_digest.is_zero()
    {
        return Err(ZkAmsErrorV1::BindingMismatch);
    }
    let statement_digest = PrivacyStatementV1::IrohaZkAmsV1(statement.clone())
        .digest()
        .map_err(|_| ZkAmsErrorV1::InvalidStatement)?;
    if binding.statement_digest != *statement_digest.as_bytes() {
        return Err(ZkAmsErrorV1::BindingMismatch);
    }
    Ok(())
}

fn build_admission_public_inputs(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<(Vec<ZkAmsAdmissionPublicInputV1>, P256VerifyingKey), ZkAmsErrorV1> {
    validate_statement_binding(statement, binding)?;
    let batch = batch_action(statement)?;
    if statement.issuer_id.is_zero()
        || statement.policy_id.is_zero()
        || statement.registry_id.is_zero()
        || statement.issuer_policy_record_digest.is_zero()
        || statement.registry_record_digest.is_zero()
        || statement.policy_digest.is_zero()
        || batch.account_registry_root.is_zero()
        || batch.next_account_registry_root.is_zero()
        || batch.account_registry_root_epoch == 0
        || batch
            .account_registry_root_epoch
            .checked_add(1)
            .is_none_or(|epoch| epoch != batch.next_account_registry_root_epoch)
        || batch.anchors.is_empty()
        || batch.anchors.len() > 8
    {
        return Err(ZkAmsErrorV1::InvalidStatement);
    }
    for (index, anchor) in batch.anchors.iter().enumerate() {
        if anchor.phc_hash.is_zero()
            || anchor.seed_public_key.is_zero()
            || batch.anchors[..index]
                .iter()
                .any(|prior| prior.phc_hash == anchor.phc_hash)
            || batch.anchors[..index]
                .iter()
                .any(|prior| prior.seed_public_key == anchor.seed_public_key)
        {
            return Err(ZkAmsErrorV1::InvalidStatement);
        }
        decode_nonidentity_point(*anchor.seed_public_key.as_bytes())?;
    }
    let issuer_key = P256VerifyingKey::from_sec1_bytes(statement.issuer_public_key.as_bytes())
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerKey)?;
    let canonical = issuer_key.to_encoded_point(true);
    if canonical.as_bytes() != statement.issuer_public_key.as_bytes() {
        return Err(ZkAmsErrorV1::InvalidIssuerKey);
    }
    let uncompressed = issuer_key.to_encoded_point(false);
    let issuer_key_x: [u8; 32] = uncompressed
        .x()
        .ok_or(ZkAmsErrorV1::InvalidIssuerKey)?
        .as_slice()
        .try_into()
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerKey)?;
    let issuer_key_y: [u8; 32] = uncompressed
        .y()
        .ok_or(ZkAmsErrorV1::InvalidIssuerKey)?
        .as_slice()
        .try_into()
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerKey)?;
    let issuer_key_prefix = statement.issuer_public_key.as_bytes()[0];
    let batch_size =
        u32::try_from(batch.anchors.len()).map_err(|_| ZkAmsErrorV1::InvalidStatement)?;
    let mut prior_root = batch.account_registry_root;
    let mut public_inputs = Vec::with_capacity(batch.anchors.len());
    for (index, anchor) in batch.anchors.iter().copied().enumerate() {
        let anchor_index = u32::try_from(index).expect("batch is bounded to eight");
        let next_root = zk_ams_registry_transition_root_v1(
            statement.registry_id,
            prior_root,
            batch.account_registry_root_epoch,
            batch.next_account_registry_root_epoch,
            batch_size,
            anchor_index,
            anchor,
        );
        public_inputs.push(ZkAmsAdmissionPublicInputV1 {
            issuer_key_x,
            issuer_key_y,
            issuer_key_prefix,
            issuer_id: *statement.issuer_id.as_bytes(),
            policy_id: *statement.policy_id.as_bytes(),
            issuer_policy_record_digest: *statement.issuer_policy_record_digest.as_bytes(),
            registry_id: *statement.registry_id.as_bytes(),
            registry_record_digest: *statement.registry_record_digest.as_bytes(),
            policy_digest: *statement.policy_digest.as_bytes(),
            phc_hash: *anchor.phc_hash.as_bytes(),
            seed_public_key: *anchor.seed_public_key.as_bytes(),
            prior_registry_root: *prior_root.as_bytes(),
            next_registry_root: *next_root.as_bytes(),
            current_registry_epoch: batch.account_registry_root_epoch,
            next_registry_epoch: batch.next_account_registry_root_epoch,
            batch_size,
            anchor_index,
        });
        prior_root = next_root;
    }
    if prior_root != batch.next_account_registry_root {
        return Err(ZkAmsErrorV1::RegistryTransitionMismatch);
    }
    Ok((public_inputs, issuer_key))
}

fn validate_credential_witness(
    statement: &IrohaZkAmsStatementV1,
    anchor: &PrivacyZkAmsAdmissionAnchorV1,
    witness: &ZkAmsBatchCredentialWitnessV1<'_>,
) -> Result<(), ZkAmsErrorV1> {
    let credential = witness.credential;
    if credential.version != ZK_AMS_PHC_VERSION_V1
        || credential.issuer_id != statement.issuer_id
        || credential.policy_id != statement.policy_id
        || credential.subject_commitment.is_zero()
        || credential.credential_nonce.is_zero()
        || credential.seed_public_key != anchor.seed_public_key
        || credential.digest() != anchor.phc_hash
        || zk_ams_seed_public_key_v1(witness.seed_secret) != *anchor.seed_public_key.as_bytes()
    {
        return Err(ZkAmsErrorV1::InvalidCredential);
    }
    Ok(())
}

fn validate_issuer_signature(
    signature_bytes: &[u8; 64],
    message_digest: [u8; 32],
    issuer_key: &P256VerifyingKey,
) -> Result<ZkAmsIssuerSignatureWitnessV1, ZkAmsErrorV1> {
    let signature = P256Signature::from_slice(signature_bytes)
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerSignature)?;
    if signature.normalize_s().is_some() {
        return Err(ZkAmsErrorV1::HighSIssuerSignature);
    }
    issuer_key
        .verify_prehash(&message_digest, &signature)
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerSignature)?;
    let (r, s) = signature.split_scalars();
    let s_inverse = Option::<P256Scalar>::from(s.as_ref().invert())
        .ok_or(ZkAmsErrorV1::InvalidIssuerSignature)?;
    let digest_scalar =
        <P256Scalar as Reduce<U256>>::reduce_bytes(&P256FieldBytes::from(message_digest));
    let issuer_point = P256ProjectivePoint::from(*issuer_key.as_affine());
    let recovery =
        (P256ProjectivePoint::GENERATOR * digest_scalar + issuer_point * *r.as_ref()) * s_inverse;
    if bool::from(recovery.is_identity()) {
        return Err(ZkAmsErrorV1::InvalidIssuerSignature);
    }
    let recovery = P256AffinePoint::from(recovery).to_encoded_point(false);
    let recovery_x: [u8; 32] = recovery
        .x()
        .ok_or(ZkAmsErrorV1::InvalidIssuerSignature)?
        .as_slice()
        .try_into()
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerSignature)?;
    let recovery_y: [u8; 32] = recovery
        .y()
        .ok_or(ZkAmsErrorV1::InvalidIssuerSignature)?
        .as_slice()
        .try_into()
        .map_err(|_| ZkAmsErrorV1::InvalidIssuerSignature)?;
    Ok(ZkAmsIssuerSignatureWitnessV1 {
        r: Zeroizing::new(r.as_ref().to_repr().into()),
        s: Zeroizing::new(s.as_ref().to_repr().into()),
        recovery_x: Zeroizing::new(recovery_x),
        recovery_y: Zeroizing::new(recovery_y),
    })
}

fn relation_context<'a>(binding: &'a TranscriptBindingV1<'a>) -> ZkAmsProofContextV1<'a> {
    ZkAmsProofContextV1 {
        chain_id: binding.chain_id,
        genesis_hash: binding.genesis_hash,
        action_index: binding.action_index,
        statement_digest: binding.statement_digest,
        parameter_id: binding.parameter_id,
        parameter_digest: binding.parameter_digest,
        verifier_digest: binding.verifier_digest,
        statement_schema_digest: binding.statement_schema_digest,
        engine_manifest_digest: binding.engine_manifest_digest,
        generator_digest: binding.generator_digest,
    }
}

fn relation_proof_digest(proof_bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(RELATION_PROOF_DIGEST_DOMAIN_V1);
    hash.update(
        u64::try_from(proof_bytes.len())
            .expect("bounded proof length fits u64")
            .to_le_bytes(),
    );
    hash.update(proof_bytes);
    hash.finalize().into()
}

struct MaskedRandomAdapter<'a, R>(&'a mut R);

impl<R: CryptoRng + RngCore> MaskedRelaxedRandomSourceV1 for MaskedRandomAdapter<'_, R> {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        self.0
            .try_fill_bytes(destination)
            .map_err(|_| MaskedRelaxedRandomErrorV1::Unavailable)
    }
}

fn scalar_from_canonical(bytes: [u8; 32]) -> Result<Scalar, ZkAmsErrorV1> {
    Option::<Scalar>::from(Scalar::from_canonical_bytes(bytes)).ok_or(ZkAmsErrorV1::InvalidScalar)
}

fn random_nonzero_scalar<R: CryptoRng + RngCore>(rng: &mut R) -> Result<Scalar, ZkAmsErrorV1> {
    for _ in 0..RANDOM_REJECTION_ATTEMPTS {
        let mut candidate = [0_u8; 32];
        rng.fill_bytes(&mut candidate);
        let parsed = scalar_from_canonical(candidate);
        candidate.zeroize();
        if let Ok(scalar) = parsed {
            if scalar != Scalar::ZERO {
                return Ok(scalar);
            }
        }
    }
    Err(ZkAmsErrorV1::RandomnessExhausted)
}

#[derive(Clone)]
struct LsagTranscriptV1 {
    prefix: Sha3_512,
}

impl LsagTranscriptV1 {
    fn new(
        binding: &TranscriptBindingV1<'_>,
        ring: &[[u8; 32]],
        key_image: [u8; 32],
    ) -> Result<Self, ZkAmsErrorV1> {
        binding.validate()?;
        let mut prefix = Sha3_512::new();
        append_field(&mut prefix, b"domain", ZK_AMS_LSAG_SUITE_V1)?;
        append_field(&mut prefix, b"transcript_version", &[TRANSCRIPT_VERSION_V1])?;
        append_field(&mut prefix, b"chain_id", binding.chain_id)?;
        append_field(&mut prefix, b"genesis_hash", &binding.genesis_hash)?;
        append_field(
            &mut prefix,
            b"action_index",
            &binding.action_index.to_be_bytes(),
        )?;
        append_field(&mut prefix, b"statement_digest", &binding.statement_digest)?;
        append_field(&mut prefix, b"parameter_id", &binding.parameter_id)?;
        append_field(&mut prefix, b"parameter_digest", &binding.parameter_digest)?;
        append_field(&mut prefix, b"verifier_digest", &binding.verifier_digest)?;
        append_field(
            &mut prefix,
            b"statement_schema_digest",
            &binding.statement_schema_digest,
        )?;
        append_field(
            &mut prefix,
            b"engine_manifest_digest",
            &binding.engine_manifest_digest,
        )?;
        append_field(&mut prefix, b"generator_digest", &binding.generator_digest)?;
        append_field(
            &mut prefix,
            b"ring_count",
            &u32::try_from(ring.len())
                .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
                .to_be_bytes(),
        )?;
        for (index, public_key) in ring.iter().enumerate() {
            append_indexed_field(&mut prefix, b"ring_public_key", index, public_key)?;
        }
        append_field(&mut prefix, b"key_image", &key_image)?;
        Ok(Self { prefix })
    }

    fn challenge(
        &self,
        index: usize,
        left: RistrettoPoint,
        right: RistrettoPoint,
    ) -> Result<Scalar, ZkAmsErrorV1> {
        if left == RistrettoPoint::identity() || right == RistrettoPoint::identity() {
            return Err(ZkAmsErrorV1::VerificationFailed);
        }
        let mut hash = self.prefix.clone();
        append_field(
            &mut hash,
            b"ring_index",
            &u32::try_from(index)
                .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
                .to_be_bytes(),
        )?;
        append_field(&mut hash, b"left", left.compress().as_bytes())?;
        append_field(&mut hash, b"right", right.compress().as_bytes())?;
        let wide: [u8; 64] = hash.finalize().into();
        Ok(Scalar::from_bytes_mod_order_wide(&wide))
    }
}

fn append_indexed_field(
    hash: &mut Sha3_512,
    label: &[u8],
    index: usize,
    value: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    let index = u32::try_from(index).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    let mut indexed_label = Vec::with_capacity(label.len() + 4);
    indexed_label.extend_from_slice(label);
    indexed_label.extend_from_slice(&index.to_be_bytes());
    append_field(hash, &indexed_label, value)
}

fn append_field(hash: &mut Sha3_512, label: &[u8], value: &[u8]) -> Result<(), ZkAmsErrorV1> {
    let label_len =
        u16::try_from(label.len()).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    let value_len =
        u32::try_from(value.len()).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    hash.update(label_len.to_be_bytes());
    hash.update(label);
    hash.update(value_len.to_be_bytes());
    hash.update(value);
    Ok(())
}
