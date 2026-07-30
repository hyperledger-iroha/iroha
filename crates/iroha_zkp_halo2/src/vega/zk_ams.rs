//! Exact setup-free masked R1CS relation for Iroha ZK-AMS admission.
//!
//! Each strict instance proves one canonical fixed PHC preimage, its SHA-256
//! digest, one canonical low-s ES256 issuer signature, and one deterministic
//! registry-root transition. A full random relaxed assignment masks the
//! sequential Nova fold before the terminal Spartan proof.

#![allow(unexpected_cfgs)]

use core::fmt;

use once_cell::sync::Lazy;
use thiserror::Error;

use super::{
    VEGA_T256_BASE_MODULUS_BE_V1, VegaT256ScalarV1 as Scalar,
    circuit::{CircuitAssignment, CircuitBuilder, CircuitError, LinearCombination},
    derive_t256_generators_v1,
    masked_relaxed::{
        MAX_MASKED_RELAXED_STRICT_INSTANCES_V1, MaskedRelaxedDimensionsV1, MaskedRelaxedErrorV1,
        MaskedRelaxedProofWireV1, MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1,
        prove_masked_relaxed_v1, verify_masked_relaxed_v1,
    },
    p256::{public_compressed_point, verify_es256_low_s},
    r1cs::Shape,
    sha256::{
        ByteVar, WordVar, allocate_byte, allocate_bytes, enforce_byte_constant, public_word, sha256,
    },
    sponge::keccak256,
};

/// Exact number of public T256 scalars in one admission relation instance.
pub const ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1: usize = 89;
/// Hard cap checked before Norito decoding of a batch relation proof.
pub const MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1: usize = 2 * 1024 * 1024;
/// Exact canonical fixed PHC payload width.
pub const ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1: usize = 161;

const PROOF_VERSION_V1: u8 = 1;
const MAX_CHAIN_ID_BYTES_V1: usize = 255;
const COMPOSITION_DOMAIN_V1: &[u8] = b"iroha-zk-ams-v1:batch-admission:masked-relaxed-spartan-t256";
const COMMITMENT_KEY_LABEL_V1: &[u8] = b"iroha.zk-ams.v1.batch-admission.hyrax-t256";
const PROFILE_DESCRIPTOR_V1: &[u8] =
    b"iroha.zk-ams.v1.batch-admission.canonical-phc-es256-root-transition";
const SOURCE_PROFILE_V1: &[u8] = b"arxiv:2602.16130v2:algorithms-1-4:appendices-a-c";
const PHC_HASH_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ams:phc:v1";
const REGISTRY_TRANSITION_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ams:registry-transition:v1";

const ISSUER_X_INDEX: usize = 0;
const ISSUER_Y_INDEX: usize = 1;
const ISSUER_PREFIX_INDEX: usize = 2;
const ISSUER_ID_WORD_START: usize = 3;
const POLICY_ID_WORD_START: usize = 11;
const ISSUER_POLICY_RECORD_WORD_START: usize = 19;
const REGISTRY_ID_WORD_START: usize = 27;
const REGISTRY_RECORD_WORD_START: usize = 35;
const POLICY_DIGEST_WORD_START: usize = 43;
const PHC_HASH_WORD_START: usize = 51;
const SEED_KEY_WORD_START: usize = 59;
const PRIOR_ROOT_WORD_START: usize = 67;
const NEXT_ROOT_WORD_START: usize = 75;
const CURRENT_EPOCH_HIGH_INDEX: usize = 83;
const CURRENT_EPOCH_LOW_INDEX: usize = 84;
const NEXT_EPOCH_HIGH_INDEX: usize = 85;
const NEXT_EPOCH_LOW_INDEX: usize = 86;
const BATCH_SIZE_INDEX: usize = 87;
const ANCHOR_INDEX: usize = 88;

static CANONICAL_SHAPE: Lazy<Result<Shape, CircuitError>> = Lazy::new(build_canonical_shape);
static T256_GENERATOR_DIGEST: Lazy<[u8; 32]> = Lazy::new(|| {
    let points = derive_t256_generators_v1(
        COMMITMENT_KEY_LABEL_V1,
        super::masked_relaxed::MASKED_RELAXED_COMMITMENT_COLUMNS_V1 + 1,
    )
    .expect("released ZK-AMS T256 generator basis must derive");
    let mut frame = Vec::with_capacity(64 + points.len() * 64);
    frame.extend_from_slice(b"iroha.zk-ams.v1.t256-generator-basis");
    frame.extend_from_slice(
        &u32::try_from(points.len())
            .expect("released generator count fits u32")
            .to_be_bytes(),
    );
    for point in points {
        frame.extend_from_slice(
            &point
                .to_transcript_bytes()
                .expect("derived generator is nonidentity"),
        );
    }
    keccak256(&frame)
});

/// Full consensus binding absorbed before all Nova and Spartan material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsProofContextV1<'a> {
    /// Exact chain identifier.
    pub chain_id: &'a [u8],
    /// Independently trusted genesis digest.
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index.
    pub action_index: u32,
    /// Digest of the complete typed public statement.
    pub statement_digest: [u8; 32],
    /// Governed parameter identifier.
    pub parameter_id: [u8; 32],
    /// Governed parameter digest.
    pub parameter_digest: [u8; 32],
    /// Exact verifier-artifact digest.
    pub verifier_digest: [u8; 32],
    /// Typed statement-schema digest.
    pub statement_schema_digest: [u8; 32],
    /// Native engine-manifest digest.
    pub engine_manifest_digest: [u8; 32],
    /// Combined Ristretto/T256 generator-basis digest.
    pub generator_digest: [u8; 32],
}

/// Public values for one ordered credential and registry transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsAdmissionPublicInputV1 {
    /// Canonical issuer P-256 x-coordinate.
    pub issuer_key_x: [u8; 32],
    /// Canonical issuer P-256 y-coordinate.
    pub issuer_key_y: [u8; 32],
    /// Canonical compressed SEC1 prefix (`0x02` or `0x03`).
    pub issuer_key_prefix: u8,
    /// Exact issuer identifier embedded in the PHC.
    pub issuer_id: [u8; 32],
    /// Exact policy identifier embedded in the PHC.
    pub policy_id: [u8; 32],
    /// Authoritative issuer-policy-record digest.
    pub issuer_policy_record_digest: [u8; 32],
    /// Registry namespace.
    pub registry_id: [u8; 32],
    /// Authoritative registry-snapshot-record digest.
    pub registry_record_digest: [u8; 32],
    /// Exact governed admission-policy digest.
    pub policy_digest: [u8; 32],
    /// Public hash of the canonical PHC.
    pub phc_hash: [u8; 32],
    /// Exact compressed Ristretto seed key embedded in the PHC.
    pub seed_public_key: [u8; 32],
    /// Root before this ordered anchor.
    pub prior_registry_root: [u8; 32],
    /// Root after this ordered anchor.
    pub next_registry_root: [u8; 32],
    /// Authoritative current registry epoch.
    pub current_registry_epoch: u64,
    /// Exact successor registry epoch.
    pub next_registry_epoch: u64,
    /// Total anchors in this atomic batch.
    pub batch_size: u32,
    /// Zero-based position of this anchor.
    pub anchor_index: u32,
}

impl ZkAmsAdmissionPublicInputV1 {
    fn to_scalars(self) -> Result<Vec<Scalar>, ZkAmsAdmissionRelationErrorV1> {
        self.validate()?;
        let mut values = Vec::with_capacity(ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1);
        values.push(
            Scalar::from_be_bytes_exact(self.issuer_key_x)
                .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidPublicInput)?,
        );
        values.push(
            Scalar::from_be_bytes_exact(self.issuer_key_y)
                .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidPublicInput)?,
        );
        values.push(Scalar::from_u64(u64::from(self.issuer_key_prefix)));
        for bytes in [
            self.issuer_id,
            self.policy_id,
            self.issuer_policy_record_digest,
            self.registry_id,
            self.registry_record_digest,
            self.policy_digest,
            self.phc_hash,
            self.seed_public_key,
            self.prior_registry_root,
            self.next_registry_root,
        ] {
            push_digest_words(&mut values, bytes);
        }
        values.push(Scalar::from_u64(self.current_registry_epoch >> 32));
        values.push(Scalar::from_u64(self.current_registry_epoch & 0xffff_ffff));
        values.push(Scalar::from_u64(self.next_registry_epoch >> 32));
        values.push(Scalar::from_u64(self.next_registry_epoch & 0xffff_ffff));
        values.push(Scalar::from_u64(u64::from(self.batch_size)));
        values.push(Scalar::from_u64(u64::from(self.anchor_index)));
        if values.len() != ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1 {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile);
        }
        Ok(values)
    }

    fn validate(self) -> Result<(), ZkAmsAdmissionRelationErrorV1> {
        if !matches!(self.issuer_key_prefix, 0x02 | 0x03)
            || self.issuer_key_x >= VEGA_T256_BASE_MODULUS_BE_V1
            || self.issuer_key_y >= VEGA_T256_BASE_MODULUS_BE_V1
            || self.batch_size == 0
            || self.batch_size as usize > MAX_MASKED_RELAXED_STRICT_INSTANCES_V1
            || self.anchor_index >= self.batch_size
            || self
                .current_registry_epoch
                .checked_add(1)
                .is_none_or(|next| next != self.next_registry_epoch)
            || [
                self.issuer_id,
                self.policy_id,
                self.issuer_policy_record_digest,
                self.registry_id,
                self.registry_record_digest,
                self.policy_digest,
                self.phc_hash,
                self.seed_public_key,
                self.prior_registry_root,
                self.next_registry_root,
            ]
            .into_iter()
            .any(|bytes| bytes == [0; 32])
        {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidPublicInput);
        }
        Ok(())
    }
}

/// Private witness for one exact canonical PHC and issuer signature.
#[derive(Clone, Copy)]
pub struct ZkAmsAdmissionRelationWitnessV1<'a> {
    subject_commitment: &'a [u8; 32],
    credential_nonce: &'a [u8; 32],
    issuer_signature_r: &'a [u8; 32],
    issuer_signature_s: &'a [u8; 32],
    signature_recovery_x: &'a [u8; 32],
    signature_recovery_y: &'a [u8; 32],
}

impl<'a> ZkAmsAdmissionRelationWitnessV1<'a> {
    /// Construct a borrowed witness. Algebraic and canonical checks occur in
    /// the exact circuit; this constructor rejects zero hidden PHC fields.
    pub fn new(
        subject_commitment: &'a [u8; 32],
        credential_nonce: &'a [u8; 32],
        issuer_signature_r: &'a [u8; 32],
        issuer_signature_s: &'a [u8; 32],
        signature_recovery_x: &'a [u8; 32],
        signature_recovery_y: &'a [u8; 32],
    ) -> Result<Self, ZkAmsAdmissionRelationErrorV1> {
        if *subject_commitment == [0; 32] || *credential_nonce == [0; 32] {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidWitness);
        }
        Ok(Self {
            subject_commitment,
            credential_nonce,
            issuer_signature_r,
            issuer_signature_s,
            signature_recovery_x,
            signature_recovery_y,
        })
    }
}

impl fmt::Debug for ZkAmsAdmissionRelationWitnessV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZkAmsAdmissionRelationWitnessV1([REDACTED])")
    }
}

/// Explicit bounded native prover configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMaskedProverConfigV1 {
    worker_count: usize,
}

impl ZkAmsMaskedProverConfigV1 {
    /// Select a deterministic commitment worker count in `1..=20`.
    pub const fn new(worker_count: usize) -> Result<Self, ZkAmsAdmissionRelationErrorV1> {
        if worker_count == 0 || worker_count > 20 {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidWorkerCount {
                actual: worker_count,
            });
        }
        Ok(Self { worker_count })
    }

    /// Return the exact selected worker count.
    #[must_use]
    pub const fn worker_count(self) -> usize {
        self.worker_count
    }
}

/// Frozen compiled proof dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsAdmissionRelationDimensionsV1 {
    /// Padded private variable count.
    pub variable_count: usize,
    /// Padded constraint count.
    pub constraint_count: usize,
    /// Exact public input count.
    pub public_input_count: usize,
    /// T256 points in each strict witness commitment.
    pub witness_commitment_points: usize,
    /// T256 points in each mask/cross-term error commitment.
    pub error_commitment_points: usize,
    /// Outer Spartan rounds.
    pub outer_sumcheck_rounds: usize,
    /// Inner Spartan rounds.
    pub inner_sumcheck_rounds: usize,
}

/// Failure at the exact ZK-AMS admission relation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ZkAmsAdmissionRelationErrorV1 {
    /// A public relation value is malformed or outside the closed profile.
    #[error("invalid ZK-AMS admission public input")]
    InvalidPublicInput,
    /// Private PHC/signature material is malformed or unsatisfied.
    #[error("invalid or unsatisfied ZK-AMS admission witness")]
    InvalidWitness,
    /// Consensus transcript context is empty, oversized, or zero.
    #[error("invalid ZK-AMS admission consensus context")]
    InvalidContext,
    /// Batch and witness vector lengths differ or are outside `1..=8`.
    #[error("invalid ZK-AMS admission batch size {actual}")]
    InvalidBatchSize {
        /// Supplied number of strict instances.
        actual: usize,
    },
    /// Commitment worker count is outside `1..=20`.
    #[error("invalid ZK-AMS admission worker count {actual}")]
    InvalidWorkerCount {
        /// Supplied worker count.
        actual: usize,
    },
    /// Deterministic circuit synthesis drifted from the released shape.
    #[error("invalid ZK-AMS compiled admission profile")]
    InvalidCompiledProfile,
    /// Proof bytes exceed the pre-decode cap.
    #[error("ZK-AMS admission proof length {actual} exceeds {max}")]
    ProofTooLarge {
        /// Actual supplied byte length.
        actual: usize,
        /// Exact hard maximum.
        max: usize,
    },
    /// Norito or algebraic proof encoding is not exact and canonical.
    #[error("invalid canonical ZK-AMS admission proof encoding")]
    InvalidProofEncoding,
    /// Nova/Spartan verification or prover self-check failed.
    #[error("ZK-AMS admission relation verification failed")]
    VerificationFailed,
    /// Cryptographic randomness was unavailable.
    #[error("ZK-AMS admission cryptographic random source unavailable")]
    RandomUnavailable,
    /// Cryptographic randomness was degenerate.
    #[error("ZK-AMS admission cryptographic randomness is degenerate")]
    DegenerateRandomness,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
#[norito(decode_from_slice)]
struct ZkAmsAdmissionProofWireV1 {
    version: u8,
    relation: MaskedRelaxedProofWireV1,
}

/// Return the exact compiled relation dimensions.
pub fn zk_ams_admission_relation_dimensions_v1()
-> Result<ZkAmsAdmissionRelationDimensionsV1, ZkAmsAdmissionRelationErrorV1> {
    let shape = canonical_shape()?;
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(&shape)
        .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile)?;
    Ok(ZkAmsAdmissionRelationDimensionsV1 {
        variable_count: dimensions.variable_count,
        constraint_count: dimensions.constraint_count,
        public_input_count: dimensions.public_input_count,
        witness_commitment_points: dimensions.witness_commitment_points,
        error_commitment_points: dimensions.error_commitment_points,
        outer_sumcheck_rounds: dimensions.outer_sumcheck_rounds,
        inner_sumcheck_rounds: dimensions.inner_sumcheck_rounds,
    })
}

/// Return the digest of the exact circuit/composer profile admitted by
/// governance.
#[must_use]
pub fn zk_ams_compiled_profile_digest_v1() -> [u8; 32] {
    let shape = canonical_shape().expect("released ZK-AMS shape must synthesize");
    let dimensions =
        MaskedRelaxedDimensionsV1::from_shape(&shape).expect("released dimensions must be valid");
    keccak256(
        &profile_frame(&shape, dimensions).expect("released profile frame is fixed and bounded"),
    )
}

/// Return the digest of every independently derived T256 commitment generator,
/// including the hiding generator.
#[must_use]
pub fn zk_ams_t256_generator_digest_v1() -> [u8; 32] {
    *T256_GENERATOR_DIGEST
}

/// Prove an ordered batch and self-verify the exact canonical output.
pub fn prove_zk_ams_admission_relation_v1<R: MaskedRelaxedRandomSourceV1>(
    context: &ZkAmsProofContextV1<'_>,
    public_inputs: &[ZkAmsAdmissionPublicInputV1],
    witnesses: &[ZkAmsAdmissionRelationWitnessV1<'_>],
    config: ZkAmsMaskedProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, ZkAmsAdmissionRelationErrorV1> {
    validate_context(context)?;
    validate_batch(public_inputs, witnesses.len())?;
    let shape = canonical_shape()?;
    let mut assignments = Vec::with_capacity(public_inputs.len());
    for (public, witness) in public_inputs.iter().copied().zip(witnesses) {
        let assignment = synthesize_admission(public, witness)
            .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidWitness)?;
        if assignment.shape != shape {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile);
        }
        assignments.push(assignment);
    }
    let context_frame = context_frame(context)?;
    let relation = prove_masked_relaxed_v1(
        COMPOSITION_DOMAIN_V1,
        &context_frame,
        COMMITMENT_KEY_LABEL_V1,
        assignments,
        config.worker_count,
        random,
    )
    .map_err(map_composition_error)?;
    let proof = ZkAmsAdmissionProofWireV1 {
        version: PROOF_VERSION_V1,
        relation,
    };
    let encoded = norito::codec::encode_adaptive(&proof);
    if encoded.len() > MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1 {
        return Err(ZkAmsAdmissionRelationErrorV1::ProofTooLarge {
            actual: encoded.len(),
            max: MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
        });
    }
    verify_zk_ams_admission_relation_v1(context, public_inputs, &encoded)?;
    Ok(encoded)
}

/// Verify one bounded, exact-canonical batch relation proof.
pub fn verify_zk_ams_admission_relation_v1(
    context: &ZkAmsProofContextV1<'_>,
    public_inputs: &[ZkAmsAdmissionPublicInputV1],
    proof_bytes: &[u8],
) -> Result<(), ZkAmsAdmissionRelationErrorV1> {
    validate_context(context)?;
    validate_batch(public_inputs, public_inputs.len())?;
    if proof_bytes.len() > MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1 {
        return Err(ZkAmsAdmissionRelationErrorV1::ProofTooLarge {
            actual: proof_bytes.len(),
            max: MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
        });
    }
    let shape = canonical_shape()?;
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(&shape)
        .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile)?;
    let decode_limits = dimensions
        .proof_decode_limits(
            public_inputs.len(),
            proof_bytes.len(),
            MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
        )
        .map_err(map_composition_error)?;
    let proof = norito::codec::decode_exact_from_slice_with_limits::<ZkAmsAdmissionProofWireV1>(
        proof_bytes,
        decode_limits,
    )
    .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidProofEncoding)?;
    if proof.version != PROOF_VERSION_V1 || norito::codec::encode_adaptive(&proof) != proof_bytes {
        return Err(ZkAmsAdmissionRelationErrorV1::InvalidProofEncoding);
    }
    let strict_public_inputs = public_inputs
        .iter()
        .copied()
        .map(ZkAmsAdmissionPublicInputV1::to_scalars)
        .collect::<Result<Vec<_>, _>>()?;
    let context_frame = context_frame(context)?;
    verify_masked_relaxed_v1(
        COMPOSITION_DOMAIN_V1,
        &context_frame,
        COMMITMENT_KEY_LABEL_V1,
        &shape,
        &strict_public_inputs,
        &proof.relation,
    )
    .map_err(map_composition_error)
}

fn synthesize_admission(
    public: ZkAmsAdmissionPublicInputV1,
    witness: &ZkAmsAdmissionRelationWitnessV1<'_>,
) -> Result<CircuitAssignment, CircuitError> {
    let public_scalars = public
        .to_scalars()
        .map_err(|_| CircuitError::InvalidAssignment)?;
    let mut builder = CircuitBuilder::new(public_scalars)?;
    let issuer_key = public_compressed_point(
        &mut builder,
        ISSUER_X_INDEX,
        ISSUER_Y_INDEX,
        ISSUER_PREFIX_INDEX,
    )?;
    let issuer_id = public_digest_bytes(&mut builder, ISSUER_ID_WORD_START)?;
    let policy_id = public_digest_bytes(&mut builder, POLICY_ID_WORD_START)?;
    let seed_key = public_digest_bytes(&mut builder, SEED_KEY_WORD_START)?;
    let phc_hash = public_digest_words(&mut builder, PHC_HASH_WORD_START)?;

    let mut phc_message = constant_bytes(&mut builder, PHC_HASH_DOMAIN_V1)?;
    phc_message.extend(constant_bytes(
        &mut builder,
        &(ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1 as u64).to_le_bytes(),
    )?);
    phc_message.push(constant_byte(&mut builder, 1)?);
    phc_message.extend_from_slice(&issuer_id);
    phc_message.extend_from_slice(&policy_id);
    let subject = allocate_bytes(&mut builder, witness.subject_commitment)?;
    enforce_nonzero_bytes(&mut builder, &subject)?;
    phc_message.extend_from_slice(&subject);
    phc_message.extend_from_slice(&seed_key);
    let nonce = allocate_bytes(&mut builder, witness.credential_nonce)?;
    enforce_nonzero_bytes(&mut builder, &nonce)?;
    phc_message.extend_from_slice(&nonce);
    let computed_phc_hash = sha256(&mut builder, &phc_message)?;
    bind_digest_words(&mut builder, computed_phc_hash, phc_hash)?;
    verify_es256_low_s(
        &mut builder,
        computed_phc_hash,
        &issuer_key,
        *witness.issuer_signature_r,
        *witness.issuer_signature_s,
        *witness.signature_recovery_x,
        *witness.signature_recovery_y,
    )?;

    // Record/policy digests are deliberately public inputs even though the
    // circuit does not derive state. Runtime matches them to authoritative
    // records; Nova and the transcript still bind their exact values.
    for start in [
        ISSUER_POLICY_RECORD_WORD_START,
        REGISTRY_RECORD_WORD_START,
        POLICY_DIGEST_WORD_START,
    ] {
        let _ = public_digest_words(&mut builder, start)?;
    }
    let registry_id = public_digest_bytes(&mut builder, REGISTRY_ID_WORD_START)?;
    let prior_root = public_digest_bytes(&mut builder, PRIOR_ROOT_WORD_START)?;
    let next_root = public_digest_words(&mut builder, NEXT_ROOT_WORD_START)?;
    let current_epoch = public_u64_bytes(
        &mut builder,
        CURRENT_EPOCH_HIGH_INDEX,
        CURRENT_EPOCH_LOW_INDEX,
    )?;
    let next_epoch = public_u64_bytes(&mut builder, NEXT_EPOCH_HIGH_INDEX, NEXT_EPOCH_LOW_INDEX)?;
    enforce_successor_epoch(
        &mut builder,
        CURRENT_EPOCH_HIGH_INDEX,
        CURRENT_EPOCH_LOW_INDEX,
        NEXT_EPOCH_HIGH_INDEX,
        NEXT_EPOCH_LOW_INDEX,
    )?;
    let batch_size = public_word(&mut builder, BATCH_SIZE_INDEX)?;
    let anchor_index = public_word(&mut builder, ANCHOR_INDEX)?;
    let mut transition = constant_bytes(&mut builder, REGISTRY_TRANSITION_DOMAIN_V1)?;
    transition.extend_from_slice(&registry_id);
    transition.extend_from_slice(&prior_root);
    transition.extend_from_slice(&current_epoch);
    transition.extend_from_slice(&next_epoch);
    transition.extend_from_slice(&batch_size.to_be_bytes());
    transition.extend_from_slice(&anchor_index.to_be_bytes());
    transition.extend_from_slice(
        &phc_hash
            .into_iter()
            .flat_map(WordVar::to_be_bytes)
            .collect::<Vec<_>>(),
    );
    transition.extend_from_slice(&seed_key);
    let computed_next = sha256(&mut builder, &transition)?;
    bind_digest_words(&mut builder, computed_next, next_root)?;
    builder.finalize()
}

fn build_canonical_shape() -> Result<Shape, CircuitError> {
    let mut public = ZkAmsAdmissionPublicInputV1 {
        issuer_key_x: [1; 32],
        issuer_key_y: [1; 32],
        issuer_key_prefix: 3,
        issuer_id: [2; 32],
        policy_id: [3; 32],
        issuer_policy_record_digest: [4; 32],
        registry_id: [5; 32],
        registry_record_digest: [6; 32],
        policy_digest: [7; 32],
        phc_hash: [8; 32],
        seed_public_key: [9; 32],
        prior_registry_root: [10; 32],
        next_registry_root: [11; 32],
        current_registry_epoch: 1,
        next_registry_epoch: 2,
        batch_size: 1,
        anchor_index: 0,
    };
    // Coordinates need only be canonical for deterministic synthesis; curve
    // membership is a constraint and does not alter shape.
    public.issuer_key_x[0] = 0;
    public.issuer_key_y[0] = 0;
    let one = [1_u8; 32];
    let witness = ZkAmsAdmissionRelationWitnessV1::new(&one, &one, &one, &one, &one, &one)
        .map_err(|_| CircuitError::InvalidAssignment)?;
    synthesize_admission(public, &witness).map(|assignment| assignment.shape)
}

fn canonical_shape() -> Result<Shape, ZkAmsAdmissionRelationErrorV1> {
    CANONICAL_SHAPE
        .as_ref()
        .map(Clone::clone)
        .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile)
}

fn validate_batch(
    public_inputs: &[ZkAmsAdmissionPublicInputV1],
    witness_count: usize,
) -> Result<(), ZkAmsAdmissionRelationErrorV1> {
    if public_inputs.is_empty()
        || public_inputs.len() > MAX_MASKED_RELAXED_STRICT_INSTANCES_V1
        || public_inputs.len() != witness_count
    {
        return Err(ZkAmsAdmissionRelationErrorV1::InvalidBatchSize {
            actual: public_inputs.len(),
        });
    }
    let count = u32::try_from(public_inputs.len()).map_err(|_| {
        ZkAmsAdmissionRelationErrorV1::InvalidBatchSize {
            actual: public_inputs.len(),
        }
    })?;
    for (index, public) in public_inputs.iter().copied().enumerate() {
        public.validate()?;
        if public.batch_size != count
            || public.anchor_index
                != u32::try_from(index)
                    .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidPublicInput)?
        {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidPublicInput);
        }
        if index > 0 && public.prior_registry_root != public_inputs[index - 1].next_registry_root {
            return Err(ZkAmsAdmissionRelationErrorV1::InvalidPublicInput);
        }
    }
    Ok(())
}

fn validate_context(
    context: &ZkAmsProofContextV1<'_>,
) -> Result<(), ZkAmsAdmissionRelationErrorV1> {
    if context.chain_id.is_empty()
        || context.chain_id.len() > MAX_CHAIN_ID_BYTES_V1
        || [
            context.genesis_hash,
            context.statement_digest,
            context.parameter_id,
            context.parameter_digest,
            context.verifier_digest,
            context.statement_schema_digest,
            context.engine_manifest_digest,
            context.generator_digest,
        ]
        .into_iter()
        .any(|digest| digest == [0; 32])
    {
        return Err(ZkAmsAdmissionRelationErrorV1::InvalidContext);
    }
    Ok(())
}

fn context_frame(
    context: &ZkAmsProofContextV1<'_>,
) -> Result<Vec<u8>, ZkAmsAdmissionRelationErrorV1> {
    let mut frame = Vec::with_capacity(512 + context.chain_id.len());
    push_frame(&mut frame, 0, COMPOSITION_DOMAIN_V1)?;
    push_frame(&mut frame, 1, SOURCE_PROFILE_V1)?;
    push_frame(&mut frame, 2, context.chain_id)?;
    push_frame(&mut frame, 3, &context.genesis_hash)?;
    push_frame(&mut frame, 4, &context.action_index.to_be_bytes())?;
    push_frame(&mut frame, 5, &context.statement_digest)?;
    push_frame(&mut frame, 6, &context.parameter_id)?;
    push_frame(&mut frame, 7, &context.parameter_digest)?;
    push_frame(&mut frame, 8, &context.verifier_digest)?;
    push_frame(&mut frame, 9, &context.statement_schema_digest)?;
    push_frame(&mut frame, 10, &context.engine_manifest_digest)?;
    push_frame(&mut frame, 11, &context.generator_digest)?;
    push_frame(&mut frame, 12, &zk_ams_compiled_profile_digest_v1())?;
    Ok(frame)
}

fn profile_frame(
    shape: &Shape,
    dimensions: MaskedRelaxedDimensionsV1,
) -> Result<Vec<u8>, ZkAmsAdmissionRelationErrorV1> {
    let mut frame = Vec::with_capacity(512);
    for (tag, value) in [
        (0, PROFILE_DESCRIPTOR_V1),
        (1, SOURCE_PROFILE_V1),
        (2, COMPOSITION_DOMAIN_V1),
        (3, COMMITMENT_KEY_LABEL_V1),
        (4, PHC_HASH_DOMAIN_V1),
        (5, REGISTRY_TRANSITION_DOMAIN_V1),
    ] {
        push_frame(&mut frame, tag, value)?;
    }
    for (tag, value) in [
        (6, PROOF_VERSION_V1 as u64),
        (7, ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1 as u64),
        (8, shape.variable_count() as u64),
        (9, shape.constraint_count() as u64),
        (10, shape.public_input_count() as u64),
        (11, dimensions.witness_commitment_points as u64),
        (12, dimensions.error_commitment_points as u64),
        (13, dimensions.outer_sumcheck_rounds as u64),
        (14, dimensions.inner_sumcheck_rounds as u64),
        (15, MAX_MASKED_RELAXED_STRICT_INSTANCES_V1 as u64),
        (16, MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1 as u64),
    ] {
        push_frame(&mut frame, tag, &value.to_be_bytes())?;
    }
    Ok(frame)
}

fn push_frame(
    output: &mut Vec<u8>,
    tag: u8,
    value: &[u8],
) -> Result<(), ZkAmsAdmissionRelationErrorV1> {
    output.push(tag);
    output.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile)?
            .to_be_bytes(),
    );
    output.extend_from_slice(value);
    Ok(())
}

fn public_digest_words(
    builder: &mut CircuitBuilder,
    start: usize,
) -> Result<[WordVar; 8], CircuitError> {
    (0..8)
        .map(|offset| public_word(builder, start + offset))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| CircuitError::InvalidDimension)
}

fn public_digest_bytes(
    builder: &mut CircuitBuilder,
    start: usize,
) -> Result<[ByteVar; 32], CircuitError> {
    public_digest_words(builder, start)?
        .into_iter()
        .flat_map(WordVar::to_be_bytes)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| CircuitError::InvalidDimension)
}

fn public_u64_bytes(
    builder: &mut CircuitBuilder,
    high_index: usize,
    low_index: usize,
) -> Result<[ByteVar; 8], CircuitError> {
    let high = public_word(builder, high_index)?;
    let low = public_word(builder, low_index)?;
    high.to_be_bytes()
        .into_iter()
        .chain(low.to_be_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| CircuitError::InvalidDimension)
}

fn bind_digest_words(
    builder: &mut CircuitBuilder,
    actual: [WordVar; 8],
    expected: [WordVar; 8],
) -> Result<(), CircuitError> {
    for (actual, expected) in actual.into_iter().zip(expected) {
        builder.enforce_equal(actual.lc(), expected.lc())?;
    }
    Ok(())
}

fn constant_bytes(
    builder: &mut CircuitBuilder,
    bytes: &[u8],
) -> Result<Vec<ByteVar>, CircuitError> {
    bytes
        .iter()
        .copied()
        .map(|byte| constant_byte(builder, byte))
        .collect()
}

fn constant_byte(builder: &mut CircuitBuilder, value: u8) -> Result<ByteVar, CircuitError> {
    let byte = allocate_byte(builder, value)?;
    enforce_byte_constant(builder, byte, value)?;
    Ok(byte)
}

fn enforce_nonzero_bytes(
    builder: &mut CircuitBuilder,
    bytes: &[ByteVar],
) -> Result<(), CircuitError> {
    let mut all_zero = builder.is_zero(bytes[0].lc())?;
    for byte in &bytes[1..] {
        let zero = builder.is_zero(byte.lc())?;
        all_zero = builder.and(all_zero, zero)?;
    }
    builder.enforce_zero(all_zero.lc())
}

fn enforce_successor_epoch(
    builder: &mut CircuitBuilder,
    current_high: usize,
    current_low: usize,
    next_high: usize,
    next_low: usize,
) -> Result<(), CircuitError> {
    let two_32 = Scalar::from_u64(1_u64 << 32);
    let current = LinearCombination::from(builder.public(current_high)?)
        .scaled(two_32)
        .plus(&builder.public(current_low)?.into());
    let next = LinearCombination::from(builder.public(next_high)?)
        .scaled(two_32)
        .plus(&builder.public(next_low)?.into());
    builder.enforce_equal(
        next,
        current.plus(&LinearCombination::constant(Scalar::one())),
    )
}

fn push_digest_words(values: &mut Vec<Scalar>, bytes: [u8; 32]) {
    values.extend(bytes.chunks_exact(4).map(|word| {
        Scalar::from_u64(u64::from(u32::from_be_bytes(
            word.try_into().expect("exact four-byte word"),
        )))
    }));
}

fn map_composition_error(error: MaskedRelaxedErrorV1) -> ZkAmsAdmissionRelationErrorV1 {
    match error {
        MaskedRelaxedErrorV1::InvalidInstanceCount { actual, .. } => {
            ZkAmsAdmissionRelationErrorV1::InvalidBatchSize { actual }
        }
        MaskedRelaxedErrorV1::InvalidWorkerCount { actual, .. } => {
            ZkAmsAdmissionRelationErrorV1::InvalidWorkerCount { actual }
        }
        MaskedRelaxedErrorV1::InvalidProfile | MaskedRelaxedErrorV1::UnsatisfiedWitness => {
            ZkAmsAdmissionRelationErrorV1::InvalidCompiledProfile
        }
        MaskedRelaxedErrorV1::InvalidProofEncoding => {
            ZkAmsAdmissionRelationErrorV1::InvalidProofEncoding
        }
        MaskedRelaxedErrorV1::VerificationFailed => {
            ZkAmsAdmissionRelationErrorV1::VerificationFailed
        }
        MaskedRelaxedErrorV1::DegenerateRandomness => {
            ZkAmsAdmissionRelationErrorV1::DegenerateRandomness
        }
        MaskedRelaxedErrorV1::Random(MaskedRelaxedRandomErrorV1::Unavailable) => {
            ZkAmsAdmissionRelationErrorV1::RandomUnavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        VegaPointWireV1, VegaScalarWireV1,
        masked_relaxed::{MASKED_RELAXED_COMMITMENT_COLUMNS_V1, MaskedRelaxedCommitmentWireV1},
    };
    use hex_literal::hex;
    use norito::NoritoSerialize;

    #[derive(Clone)]
    struct AdmissionAssignmentFixture {
        public: ZkAmsAdmissionPublicInputV1,
        subject_commitment: [u8; 32],
        credential_nonce: [u8; 32],
        signature_r: [u8; 32],
        signature_s: [u8; 32],
        recovery_x: [u8; 32],
        recovery_y: [u8; 32],
    }

    impl AdmissionAssignmentFixture {
        fn witness(&self) -> ZkAmsAdmissionRelationWitnessV1<'_> {
            ZkAmsAdmissionRelationWitnessV1::new(
                &self.subject_commitment,
                &self.credential_nonce,
                &self.signature_r,
                &self.signature_s,
                &self.recovery_x,
                &self.recovery_y,
            )
            .expect("nonzero fixed witness")
        }
    }

    fn admission_assignment_fixture() -> AdmissionAssignmentFixture {
        AdmissionAssignmentFixture {
            public: ZkAmsAdmissionPublicInputV1 {
                issuer_key_x: hex!(
                    "8e533b6fa0bf7b4625bb30667c01fb607ef9f8b8a80fef5b300628703187b2a3"
                ),
                issuer_key_y: hex!(
                    "73eb1dbde03318366d069f83a6f5900053c73633cb041b21c55e1a86c1f400b4"
                ),
                issuer_key_prefix: 0x02,
                issuer_id: [0x31; 32],
                policy_id: [0x35; 32],
                issuer_policy_record_digest: [0x32; 32],
                registry_id: [0x33; 32],
                registry_record_digest: [0x34; 32],
                policy_digest: [0x36; 32],
                phc_hash: hex!("9383ba61dc82dee66ba0210e99a86d9bc45c6ed62c717a111239991e347a3edd"),
                seed_public_key: [0x51; 32],
                prior_registry_root: [0x37; 32],
                next_registry_root: hex!(
                    "84e0c6b4ab07ab28b71ad3828e3896e68aa821816c413bba257082df1238a586"
                ),
                current_registry_epoch: 9,
                next_registry_epoch: 10,
                batch_size: 1,
                anchor_index: 0,
            },
            subject_commitment: [0x41; 32],
            credential_nonce: [0x61; 32],
            signature_r: hex!("3ed113b7883b4c590638379db0c21cda16742ed0255048bf433391d374bc21d1"),
            signature_s: hex!("06d6d7ac6abd44d90dbdf7da0a16796a7228576114ad79a8e8d5ba374fb6a016"),
            recovery_x: hex!("3ed113b7883b4c590638379db0c21cda16742ed0255048bf433391d374bc21d1"),
            recovery_y: hex!("9099209accc4c8a224c843afa4f4c68a090d04da5e9889dae2f8eefce82a3740"),
        }
    }

    fn synthesize_fixture(fixture: &AdmissionAssignmentFixture) -> CircuitAssignment {
        synthesize_admission(fixture.public, &fixture.witness())
            .expect("fixed-shape admission synthesis")
    }

    fn assignment_is_satisfied(assignment: &CircuitAssignment) -> bool {
        assignment
            .shape
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); assignment.shape.constraint_count()],
            )
            .is_ok()
    }

    fn synthetic_dimensions() -> MaskedRelaxedDimensionsV1 {
        MaskedRelaxedDimensionsV1 {
            variable_count: 524_288,
            constraint_count: 1_048_576,
            public_input_count: ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1,
            witness_commitment_points: 512,
            error_commitment_points: 1_024,
            outer_sumcheck_rounds: 20,
            inner_sumcheck_rounds: 20,
        }
    }

    fn empty_relation() -> MaskedRelaxedProofWireV1 {
        let scalar = VegaScalarWireV1::from_raw_bytes_for_test([0; 32]);
        MaskedRelaxedProofWireV1 {
            version: 1,
            strict_instance_count: 1,
            mask_witness_commitment: MaskedRelaxedCommitmentWireV1 { points: Vec::new() },
            mask_error_commitment: MaskedRelaxedCommitmentWireV1 { points: Vec::new() },
            mask_relaxation: scalar,
            mask_public_inputs: Vec::new(),
            strict_witness_commitments: Vec::new(),
            cross_term_commitments: Vec::new(),
            outer_sumcheck_rounds: Vec::new(),
            outer_claims: [scalar; 3],
            inner_sumcheck_rounds: Vec::new(),
            witness_opening: Vec::new(),
            witness_opening_blinding: scalar,
            error_opening: Vec::new(),
            error_opening_blinding: scalar,
        }
    }

    fn empty_proof() -> ZkAmsAdmissionProofWireV1 {
        ZkAmsAdmissionProofWireV1 {
            version: PROOF_VERSION_V1,
            relation: empty_relation(),
        }
    }

    #[test]
    fn admission_decoder_preflights_oversized_and_forged_nested_counts() {
        let dimensions = synthetic_dimensions();
        let point = VegaPointWireV1::from_raw_bytes_for_test([1; 33]);
        let mut proof = empty_proof();
        proof.relation.mask_witness_commitment.points =
            vec![point; MASKED_RELAXED_COMMITMENT_COLUMNS_V1 + 1];
        let encoded = norito::codec::encode_adaptive(&proof);
        let limits = dimensions
            .proof_decode_limits(
                1,
                encoded.len(),
                MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
            )
            .expect("released relation limits");
        assert!(matches!(
            norito::codec::decode_exact_from_slice_with_limits::<ZkAmsAdmissionProofWireV1>(
                &encoded, limits
            ),
            Err(norito::Error::SequenceLengthExceeded {
                length: 1_025,
                limit: 1_024
            })
        ));

        let encoded_count = 1_025_u32.to_le_bytes();
        let count_offset = encoded
            .windows(encoded_count.len())
            .rposition(|window| window == encoded_count)
            .expect("oversized nested count is present in canonical wire");
        let mut forged = encoded;
        forged[count_offset..count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let limits = dimensions
            .proof_decode_limits(
                1,
                forged.len(),
                MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
            )
            .expect("released relation limits");
        let forged_error = norito::codec::decode_exact_from_slice_with_limits::<
            ZkAmsAdmissionProofWireV1,
        >(&forged, limits)
        .expect_err("forged nested count must fail before allocation");
        assert!(
            matches!(forged_error, norito::Error::SequenceLengthExceeded { length, limit }
                if length == u64::from(u32::MAX)
                    && limit <= MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1 as u64
                    && limit < length),
            "unexpected forged-count rejection: {forged_error:?}"
        );
    }

    #[test]
    fn admission_decoder_rejects_truncation_trailing_and_alternate_layout() {
        let proof = empty_proof();
        let canonical = norito::codec::encode_adaptive(&proof);
        let decode = |bytes: &[u8]| {
            let limits = synthetic_dimensions()
                .proof_decode_limits(1, bytes.len(), MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1)
                .expect("released relation limits");
            norito::codec::decode_exact_from_slice_with_limits::<ZkAmsAdmissionProofWireV1>(
                bytes, limits,
            )
        };
        assert_eq!(decode(&canonical).expect("canonical wire"), proof);

        assert!(decode(&canonical[..canonical.len() - 1]).is_err());
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());

        let mut alternate = Vec::new();
        {
            let flags =
                norito::core::default_encode_flags() & !norito::core::header_flags::COMPACT_LEN;
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            proof
                .serialize(&mut alternate)
                .expect("encode alternate length layout");
        }
        assert_ne!(alternate, canonical);
        assert!(decode(&alternate).is_err());
    }

    #[test]
    fn admission_assignment_accepts_low_s_and_rejects_malleability_and_rebinding() {
        // Independent P-256 arithmetic fixture: private key 7, nonce 11. The
        // high-s signature below is the valid `n-s` counterpart and uses `-R`,
        // so only the canonical low-s constraint distinguishes the two.
        let fixture = admission_assignment_fixture();
        let low_s = synthesize_fixture(&fixture);
        assert!(
            assignment_is_satisfied(&low_s),
            "canonical low-s admission assignment must satisfy every constraint"
        );

        for public_index in [PHC_HASH_WORD_START, NEXT_ROOT_WORD_START] {
            let mut rebound_public = low_s.public_inputs.clone();
            rebound_public[public_index] += Scalar::one();
            assert!(
                low_s
                    .shape
                    .validate_relaxed_assignment(
                        &low_s.witness,
                        Scalar::one(),
                        &rebound_public,
                        &vec![Scalar::zero(); low_s.shape.constraint_count()],
                    )
                    .is_err(),
                "mutating a statement-bound public value must fail"
            );
        }
        drop(low_s);

        let mut high_s_fixture = fixture.clone();
        high_s_fixture.signature_s =
            hex!("f92928529542bb27f2420825f5e986954abea34c926a24dc0ae4108bacac853b");
        high_s_fixture.recovery_y =
            hex!("6f66df64333b375edb37bc505b0b3975f6f2fb26a16776251d07110317d5c8bf");
        let high_s = synthesize_fixture(&high_s_fixture);
        assert!(
            !assignment_is_satisfied(&high_s),
            "the algebraically valid high-s counterpart must violate the circuit"
        );
        drop(high_s);

        let mut changed_witness = fixture;
        changed_witness.subject_commitment[0] ^= 1;
        let changed_witness = synthesize_fixture(&changed_witness);
        assert!(
            !assignment_is_satisfied(&changed_witness),
            "a hidden PHC-field mutation must fail its public hash binding"
        );
    }
}
