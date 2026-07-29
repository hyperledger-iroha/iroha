//! Canonical proof envelope for the closed Vega Figure 9 relation.
//!
//! Privacy is obtained by the same composition as the pinned Microsoft Vega
//! implementation. For every proof the prover samples a full relaxed
//! assignment
//! `Z1 = (W1, u1, X1)` uniformly from the T256 scalar field, computes
//! `E1 = A Z1 ∘ B Z1 - u1 C Z1`, and commits to `W1` and `E1` with fresh
//! row blindings. Nova then folds this satisfying random pair with the real
//! strict pair using a challenge sampled only after `U1`, `U2`, and `comm_T`
//! are committed:
//!
//! `W* = W1 + r W2`, `E* = E1 + r T`.
//!
//! The crate-private Relaxed Spartan direct openings are not zero-knowledge on
//! their own. They are released only for this freshly masked fold. Pedersen
//! row commitments are perfectly hiding with uniform blindings, and the full
//! random relaxed assignment masks every folded witness coordinate. A random
//! tape and every commitment blinding must be fresh for each invocation.

#![allow(unexpected_cfgs)]

use once_cell::sync::Lazy;
use thiserror::Error;

use super::{
    MAX_VEGA_PROOF_BYTES_V1, VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1,
    VEGA_MDL_BIRTH_RANDOM_BYTES_V1, VEGA_MDL_FULL_DATE_TEXT_BYTES_V1,
    VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1, VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1,
    VEGA_MDL_MAX_PRESENTATION_YEAR_V1, VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1,
    VEGA_MDL_MIN_PRESENTATION_YEAR_V1, VEGA_MDL_MSO_PAYLOAD_BYTES_V1,
    VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1, VegaPointWireV1, VegaScalarWireV1,
    VegaT256ScalarV1 as Scalar,
    circuit::{CircuitError, MAX_CIRCUIT_ROWS},
    commitment::{
        COMMITMENT_WORKER_STACK_BYTES, Commitment, CommitmentError, CommitmentKey,
        MAX_COMMITMENT_WORKERS,
    },
    date::{RFC3339_SECONDS_PER_DAY_V1, RFC3339_TIMESTAMP_ORDER_SLACK_BITS_V1},
    figure9::{VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaMdlFigure9WitnessV1, synthesize_figure9},
    figure9_layout::FIGURE9_LAYOUT,
    nifs::NovaNifs,
    r1cs::{Instance, RelaxedInstance, RelaxedWitness, Shape, SparseMatrix, Witness},
    spartan::RelaxedSpartanProof,
    sponge::{Keccak256, keccak256},
    sumcheck::{CompressedUnivariate, SumcheckProof},
    transcript::VegaTranscriptV1,
    validate_proof_byte_cap_v1,
};

/// Exact external privacy protocol label absorbed before all proof material.
pub const VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1: &[u8] = b"vega-existing-credential-zk-v0";
/// Exact internal Microsoft transcript persona.
pub const VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1: &[u8] = b"neutronnova_prove";
/// Keccak-256 digest of the complete canonical Figure 9 relation.
///
/// The framed preimage contains every sparse A/B/C entry with its canonical
/// T256 coefficient, all exact dimensions, the fixed issuer and birth
/// templates and masks, layout ranges, and the closed semantic constants.
pub const VEGA_MDL_CANONICAL_RELATION_DIGEST_V1: [u8; 32] = [
    0xf3, 0x27, 0xbb, 0x5b, 0x0a, 0xa3, 0xa0, 0x94, 0x18, 0xa7, 0xc0, 0x62, 0xca, 0xc8, 0x11, 0x96,
    0xcd, 0xfb, 0x65, 0xc5, 0x62, 0x59, 0xe9, 0x01, 0x95, 0xf8, 0x09, 0x83, 0xda, 0x53, 0x07, 0xe3,
];
/// Keccak-256 digest of the exact first-release Figure 9 compiled profile.
///
/// Keeping this value in the executable manifest lets capability discovery and
/// governance remain constant-time: they do not synthesize the million-row
/// circuit merely to identify the verifier. Proof and verification paths
/// independently recompute the digest once the canonical shape is already
/// required and fail closed if the compiled relation has drifted.
pub const VEGA_MDL_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0xfd, 0x97, 0xbb, 0x0f, 0x9d, 0x67, 0x36, 0x77, 0x18, 0x2f, 0x0b, 0x87, 0x34, 0x60, 0x9c, 0x7c,
    0x03, 0x47, 0x80, 0x63, 0x24, 0xee, 0x64, 0xc3, 0x74, 0xcc, 0xc6, 0xa7, 0xa1, 0x5e, 0xe4, 0x73,
];

const PROOF_VERSION: u8 = 1;
const COMMITMENT_KEY_COLUMNS: usize = 1024;
const COMMITMENT_KEY_LABEL: &[u8] = b"iroha.vega.figure9.hyrax-t256.v1";
const PROFILE_DESCRIPTOR: &[u8] = b"iroha.vega.figure9.mdl-age.v1";
const PINNED_SOURCE_COMMIT: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
const CANONICAL_RELATION_DIGEST_DOMAIN_V1: &[u8] = b"iroha.vega.figure9.canonical-r1cs-relation.v1";
const CANONICAL_RELATION_LAYOUT_SCHEMA_V1: &[u8] = b"issuer-birth-digest|issuer-device-x|issuer-device-y|issuer-signed-rfc3339|issuer-valid-from-rfc3339|issuer-valid-until-rfc3339|birth-random|birth-full-date";
const CANONICAL_RELATION_SEMANTIC_SCHEMA_V1: &[u8] = b"issuer-sig-structure-bytes|mso-payload-bytes|birth-item-bytes|birth-random-bytes|full-date-text-bytes|rfc3339-utc-seconds-text-bytes|min-presentation-year|max-presentation-year|min-age-threshold|max-age-threshold|figure9-public-inputs|rfc3339-seconds-per-day|rfc3339-timestamp-order-slack-bits";
const MAX_CHAIN_ID_BYTES: usize = 255;
const RANDOM_HEALTH_RETRIES: usize = 16;
const COMMITMENT_WORKER_HEAP_BOUND_BYTES: usize = 256 * 1024;

/// Hard release cap for caller-selected Vega commitment workers.
pub const MAX_VEGA_PROVER_WORKERS_V1: usize = MAX_COMMITMENT_WORKERS;
/// Conservative shared resident-memory admission budget for one Vega proof.
///
/// This covers the fixed compiled shape, sparse matrices, assignments, mask,
/// transcript, generator tables, and proof buffers. Worker-local stack and
/// MSM scratch are added separately by
/// [`VegaMdlProverConfigV1::release_memory_ceiling_bytes`].
pub const VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1: usize = 2 * 1024 * 1024 * 1024;
/// Largest released per-proof memory ceiling, at twenty workers.
pub const MAX_VEGA_PROVER_RELEASE_MEMORY_CEILING_BYTES_V1: usize =
    VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1
        + MAX_COMMITMENT_WORKERS
            * (COMMITMENT_WORKER_STACK_BYTES + COMMITMENT_WORKER_HEAP_BOUND_BYTES);

static CANONICAL_SHAPE: Lazy<Result<Shape, CircuitError>> = Lazy::new(build_canonical_shape);
static COMMITMENT_KEY: Lazy<Result<CommitmentKey, CommitmentError>> =
    Lazy::new(|| CommitmentKey::derive(COMMITMENT_KEY_LABEL, COMMITMENT_KEY_COLUMNS));

/// Explicit failure returned by an injected cryptographic random source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaRandomSourceErrorV1 {
    /// The operating-system or hardware random source was unavailable.
    #[error("Vega cryptographic random source is unavailable")]
    Unavailable,
}

/// Random-byte source used by the Vega prover.
///
/// Production implementations must be cryptographically secure, must not
/// repeat a stream across proofs, and must not be deterministically seeded.
/// The trait is injected so conformance tests can supply an exact random tape.
pub trait VegaRandomSourceV1 {
    /// Fill the entire destination or return an error.
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1>;
}

/// Explicit, bounded native prover execution configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlProverConfigV1 {
    worker_count: usize,
}

impl VegaMdlProverConfigV1 {
    /// Select an exact deterministic row partition.
    ///
    /// # Errors
    ///
    /// Worker counts outside `1..=20` are rejected.
    pub const fn new(worker_count: usize) -> Result<Self, VegaMdlProofErrorV1> {
        if worker_count == 0 || worker_count > MAX_VEGA_PROVER_WORKERS_V1 {
            return Err(VegaMdlProofErrorV1::InvalidWorkerCount {
                actual: worker_count,
                min: 1,
                max: MAX_VEGA_PROVER_WORKERS_V1,
            });
        }
        Ok(Self { worker_count })
    }

    /// Exact number of commitment workers.
    #[must_use]
    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    /// Conservative release bound for concurrently owned worker scratch.
    ///
    /// Every worker has a fixed 512 KiB stack and at most 256 KiB of
    /// row-MSM heap scratch at the fixed 1,024-column width. Shared
    /// assignments, matrices, generator tables, and fixed-size proof outputs
    /// are not counted in this worker-only value.
    #[must_use]
    pub const fn commitment_worker_scratch_bound_bytes(self) -> usize {
        self.worker_count * (COMMITMENT_WORKER_STACK_BYTES + COMMITMENT_WORKER_HEAP_BOUND_BYTES)
    }

    /// Conservative release-mode resident-memory admission ceiling.
    ///
    /// The fixed 2 GiB shared budget is combined with the exact number of
    /// configured worker stacks and bounded row-MSM scratch areas. Deployments
    /// must reserve this amount before starting a proof; concurrency admission
    /// is intentionally owned by the caller rather than a hidden global pool.
    #[must_use]
    pub const fn release_memory_ceiling_bytes(self) -> usize {
        VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1 + self.commitment_worker_scratch_bound_bytes()
    }
}

/// Consensus context bound into every Figure 9 Fiat--Shamir challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlProofContextV1<'a> {
    /// Exact chain identifier bytes.
    pub chain_id: &'a [u8],
    /// Independently trusted genesis hash.
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index.
    pub action_index: u32,
    /// Governed parameter identifier.
    pub parameter_id: [u8; 32],
    /// Governed parameter digest.
    pub parameter_digest: [u8; 32],
    /// Digest of the exact verifier artifact.
    pub verifier_digest: [u8; 32],
    /// Digest of the typed statement schema.
    pub statement_schema_digest: [u8; 32],
    /// Digest of the native engine manifest.
    pub engine_manifest_digest: [u8; 32],
}

/// Failure while proving, decoding, or verifying the released Vega proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlProofErrorV1 {
    /// The requested native row-worker count is outside the released bound.
    #[error("Vega prover worker count {actual} is outside {min}..={max}")]
    InvalidWorkerCount {
        /// Requested workers.
        actual: usize,
        /// Inclusive minimum.
        min: usize,
        /// Inclusive maximum.
        max: usize,
    },
    /// A consensus context field is empty, oversized, or a zero digest.
    #[error("invalid Vega proof consensus context")]
    InvalidContext,
    /// The private assignment failed the complete Figure 9 relation.
    #[error("Vega Figure 9 witness is unsatisfied")]
    UnsatisfiedWitness,
    /// The random source reported a failure.
    #[error(transparent)]
    RandomSource(#[from] VegaRandomSourceErrorV1),
    /// The source repeatedly returned degenerate zero material or a
    /// commitment collapsed to the identity.
    #[error("Vega prover randomness is degenerate")]
    DegenerateRandomness,
    /// The local fixed relation or commitment profile could not be built.
    #[error("Vega compiled proof profile is invalid")]
    InvalidCompiledProfile,
    /// The proof exceeded the Vega-specific 512 KiB cap.
    #[error("Vega proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Actual encoded byte length.
        actual: usize,
        /// Released maximum.
        max: usize,
    },
    /// Norito decoding, exact-shape validation, algebraic decoding, or
    /// byte-identical canonical re-encoding failed.
    #[error("invalid canonical Vega proof encoding")]
    InvalidProofEncoding,
    /// The proof equations or Fiat--Shamir replay failed.
    #[error("Vega proof verification failed")]
    VerificationFailed,
}

/// Exact released work and proof-shape bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlProofDimensionsV1 {
    /// Padded private R1CS variables.
    pub variable_count: usize,
    /// Padded R1CS constraints.
    pub constraint_count: usize,
    /// Hyrax scalars per committed row and direct opening.
    pub commitment_columns: usize,
    /// Points in each witness commitment.
    pub witness_commitment_points: usize,
    /// Points in each error or cross-term commitment.
    pub error_commitment_points: usize,
    /// Cubic outer sum-check rounds.
    pub outer_sumcheck_rounds: usize,
    /// Quadratic inner sum-check rounds.
    pub inner_sumcheck_rounds: usize,
}

impl VegaMdlProofDimensionsV1 {
    fn from_shape(shape: &Shape) -> Result<Self, VegaMdlProofErrorV1> {
        if shape.public_input_count() != VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1
            || shape.variable_count() > MAX_CIRCUIT_ROWS
            || shape.constraint_count() > MAX_CIRCUIT_ROWS
            || !shape.variable_count().is_power_of_two()
            || !shape.constraint_count().is_power_of_two()
            || COMMITMENT_KEY_COLUMNS > shape.variable_count()
            || COMMITMENT_KEY_COLUMNS > shape.constraint_count()
        {
            return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
        }
        let outer_sumcheck_rounds = usize::try_from(shape.constraint_count().ilog2())
            .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
        let inner_sumcheck_rounds = usize::try_from(shape.variable_count().ilog2())
            .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
            .checked_add(1)
            .ok_or(VegaMdlProofErrorV1::InvalidCompiledProfile)?;
        let witness_commitment_points = shape.variable_count().div_ceil(COMMITMENT_KEY_COLUMNS);
        let error_commitment_points = shape.constraint_count().div_ceil(COMMITMENT_KEY_COLUMNS);
        if !witness_commitment_points.is_power_of_two()
            || !error_commitment_points.is_power_of_two()
        {
            return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
        }
        Ok(Self {
            variable_count: shape.variable_count(),
            constraint_count: shape.constraint_count(),
            commitment_columns: COMMITMENT_KEY_COLUMNS,
            witness_commitment_points,
            error_commitment_points,
            outer_sumcheck_rounds,
            inner_sumcheck_rounds,
        })
    }

    fn proof_decode_limits(
        self,
        payload_len: usize,
    ) -> Result<norito::DecodeLimits, VegaMdlProofErrorV1> {
        let max_sequence_elements = [
            self.witness_commitment_points,
            self.error_commitment_points,
            self.outer_sumcheck_rounds,
            self.inner_sumcheck_rounds,
            self.commitment_columns,
        ]
        .into_iter()
        .max()
        .ok_or(VegaMdlProofErrorV1::InvalidCompiledProfile)?;
        let max_total_elements = self
            .witness_commitment_points
            .checked_mul(2)
            .and_then(|total| {
                self.error_commitment_points
                    .checked_mul(2)
                    .and_then(|points| total.checked_add(points))
            })
            .and_then(|total| total.checked_add(self.outer_sumcheck_rounds))
            .and_then(|total| total.checked_add(self.inner_sumcheck_rounds))
            .and_then(|total| {
                self.commitment_columns
                    .checked_mul(2)
                    .and_then(|openings| total.checked_add(openings))
            })
            .ok_or(VegaMdlProofErrorV1::InvalidCompiledProfile)?;
        Ok(norito::DecodeLimits::new(
            max_sequence_elements,
            payload_len,
            max_total_elements,
            MAX_VEGA_PROOF_BYTES_V1.saturating_mul(8),
            16,
        ))
    }
}

/// Return the exact compiled Figure 9 dimensions.
///
/// # Errors
///
/// Returns [`VegaMdlProofErrorV1::InvalidCompiledProfile`] if deterministic
/// synthesis does not match the released work limits.
pub fn vega_mdl_proof_dimensions_v1() -> Result<VegaMdlProofDimensionsV1, VegaMdlProofErrorV1> {
    let shape = canonical_shape()?;
    VegaMdlProofDimensionsV1::from_shape(&shape)
}

/// Return the pinned digest of the complete canonical Figure 9 R1CS relation.
#[must_use]
pub const fn vega_mdl_canonical_relation_digest_v1() -> [u8; 32] {
    VEGA_MDL_CANONICAL_RELATION_DIGEST_V1
}

/// Return the Keccak-256 digest of the exact compiled Figure 9 profile frame.
///
/// The frame binds the pinned source revision, proof version, complete
/// canonical relation digest, Hyrax commitment shape, sumcheck schedule,
/// commitment-key derivation label, and native proof-byte cap. Governance uses
/// this digest as an input to its parameter and verifier manifests so a binary
/// cannot activate a profile whose compiled proof relation has drifted.
#[must_use]
pub const fn vega_mdl_compiled_profile_digest_v1() -> [u8; 32] {
    VEGA_MDL_COMPILED_PROFILE_DIGEST_V1
}

/// Prove the complete Figure 9 relation and return exact canonical Norito.
///
/// The supplied random source is consumed directly; callers cannot supply or
/// reuse an already-built relaxed mask object. Every field element is sampled
/// by reducing an independent 64-byte wide string. Two distinct nonzero health
/// samples and nonzero commitment blindings reject an all-zero or constant
/// source before any witness-dependent proof material is committed.
///
/// # Errors
///
/// Fails closed on invalid context, unsatisfied witness, random-source failure,
/// degenerate randomness, invalid compiled profile, or a proof above 512 KiB.
pub fn prove_vega_mdl_figure9_v1<R: VegaRandomSourceV1>(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
    config: VegaMdlProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    validate_context(context)?;
    let assignment = synthesize_figure9(public_inputs, witness)
        .map_err(|_| VegaMdlProofErrorV1::UnsatisfiedWitness)?;
    assignment
        .shape
        .validate_relaxed_assignment(
            &assignment.witness,
            Scalar::one(),
            &assignment.public_inputs,
            &vec![Scalar::zero(); assignment.shape.constraint_count()],
        )
        .map_err(|_| VegaMdlProofErrorV1::UnsatisfiedWitness)?;
    let shape = canonical_shape()?;
    if assignment.shape != shape {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    let dimensions = VegaMdlProofDimensionsV1::from_shape(&shape)?;
    let key = commitment_key(config.worker_count())?;

    // Consumed distinct nonzero draws detect the most common catastrophic RNG
    // failures before allocating a full masking assignment.
    validate_random_health(random)?;

    let regular_blindings = sample_nonzero_scalars(random, dimensions.witness_commitment_points)?;
    let regular_witness = Witness {
        values: assignment.witness,
        blindings: regular_blindings,
    };
    let regular_instance = Instance {
        witness_commitment: key
            .commit(&regular_witness.values, &regular_witness.blindings)
            .map_err(|_| VegaMdlProofErrorV1::DegenerateRandomness)?,
        public_inputs: public_inputs.to_vec(),
    };

    let (mask_instance, mask_witness) = sample_relaxed_mask(random, &shape, &key, dimensions)?;
    let cross_term_blindings = sample_nonzero_scalars(random, dimensions.error_commitment_points)?;
    let mut transcript = release_transcript(context, public_inputs, &shape, dimensions)?;
    let (nifs, folded_instance, folded_witness) = NovaNifs::prove(
        &key,
        &shape,
        &mask_instance,
        &mask_witness,
        &regular_instance,
        &regular_witness,
        &cross_term_blindings,
        &mut transcript,
    )
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let spartan = RelaxedSpartanProof::prove(
        &shape,
        &key,
        &folded_instance,
        &folded_witness,
        &mut transcript,
    )
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;

    let proof =
        VegaMdlProofWireV1::from_protocol(&mask_instance, &regular_instance, &nifs, &spartan)?;
    let encoded = norito::codec::encode_adaptive(&proof);
    if encoded.len() > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaMdlProofErrorV1::ProofTooLarge {
            actual: encoded.len(),
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    Ok(encoded)
}

/// Decode and verify one canonical Figure 9 proof.
///
/// The 512 KiB cap is checked before Norito is invoked. Decoding must consume
/// the complete input, every vector must match the fixed compiled shape, every
/// scalar and point must be canonical, and re-encoding must reproduce the
/// exact input bytes.
///
/// # Errors
///
/// Returns a precise cap error, an encoding error, or a proof-verification
/// error.
pub fn verify_vega_mdl_figure9_v1(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    proof_bytes: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    validate_context(context)?;
    if let Err(error) = validate_proof_byte_cap_v1(proof_bytes) {
        return match error {
            super::VegaWireError::ProofTooLarge { actual, max } => {
                Err(VegaMdlProofErrorV1::ProofTooLarge { actual, max })
            }
            _ => Err(VegaMdlProofErrorV1::InvalidProofEncoding),
        };
    }
    let shape = canonical_shape()?;
    let dimensions = VegaMdlProofDimensionsV1::from_shape(&shape)?;
    let key = commitment_key(1)?;
    let proof = norito::codec::decode_exact_from_slice_with_limits::<VegaMdlProofWireV1>(
        proof_bytes,
        dimensions.proof_decode_limits(proof_bytes.len())?,
    )
    .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?;
    proof.validate_shape(dimensions)?;
    if norito::codec::encode_adaptive(&proof) != proof_bytes {
        return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
    }
    let (mask_instance, regular_instance, nifs, spartan) = proof.to_protocol(public_inputs)?;

    let mut transcript = release_transcript(context, public_inputs, &shape, dimensions)?;
    let folded = nifs
        .verify(
            &key,
            &shape,
            &mut transcript,
            &mask_instance,
            &regular_instance,
        )
        .map_err(|_| VegaMdlProofErrorV1::VerificationFailed)?;
    spartan
        .verify(&shape, &key, &folded, &mut transcript)
        .map_err(|_| VegaMdlProofErrorV1::VerificationFailed)
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct CommitmentWireV1 {
    points: Vec<VegaPointWireV1>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct VegaMdlProofWireV1 {
    version: u8,
    mask_witness_commitment: CommitmentWireV1,
    mask_error_commitment: CommitmentWireV1,
    mask_relaxation: VegaScalarWireV1,
    mask_public_inputs: [VegaScalarWireV1; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    regular_witness_commitment: CommitmentWireV1,
    cross_term_commitment: CommitmentWireV1,
    outer_sumcheck_rounds: Vec<[VegaScalarWireV1; 3]>,
    outer_claims: [VegaScalarWireV1; 3],
    inner_sumcheck_rounds: Vec<[VegaScalarWireV1; 2]>,
    witness_opening: Vec<VegaScalarWireV1>,
    witness_opening_blinding: VegaScalarWireV1,
    error_opening: Vec<VegaScalarWireV1>,
    error_opening_blinding: VegaScalarWireV1,
}

impl VegaMdlProofWireV1 {
    fn from_protocol(
        mask: &RelaxedInstance,
        regular: &Instance,
        nifs: &NovaNifs,
        spartan: &RelaxedSpartanProof,
    ) -> Result<Self, VegaMdlProofErrorV1> {
        Ok(Self {
            version: PROOF_VERSION,
            mask_witness_commitment: CommitmentWireV1::from_commitment(&mask.witness_commitment)?,
            mask_error_commitment: CommitmentWireV1::from_commitment(&mask.error_commitment)?,
            mask_relaxation: VegaScalarWireV1::from_scalar(mask.relaxation),
            mask_public_inputs: scalars_to_wire_array(&mask.public_inputs)?,
            regular_witness_commitment: CommitmentWireV1::from_commitment(
                &regular.witness_commitment,
            )?,
            cross_term_commitment: CommitmentWireV1::from_commitment(&nifs.cross_term_commitment)?,
            outer_sumcheck_rounds: spartan
                .outer_sumcheck
                .rounds
                .iter()
                .map(|round| scalars_to_wire_array(round.coefficients()))
                .collect::<Result<_, _>>()?,
            outer_claims: spartan.outer_claims.map(VegaScalarWireV1::from_scalar),
            inner_sumcheck_rounds: spartan
                .inner_sumcheck
                .rounds
                .iter()
                .map(|round| scalars_to_wire_array(round.coefficients()))
                .collect::<Result<_, _>>()?,
            witness_opening: spartan
                .witness_opening
                .iter()
                .copied()
                .map(VegaScalarWireV1::from_scalar)
                .collect(),
            witness_opening_blinding: VegaScalarWireV1::from_scalar(
                spartan.witness_opening_blinding,
            ),
            error_opening: spartan
                .error_opening
                .iter()
                .copied()
                .map(VegaScalarWireV1::from_scalar)
                .collect(),
            error_opening_blinding: VegaScalarWireV1::from_scalar(spartan.error_opening_blinding),
        })
    }

    fn validate_shape(
        &self,
        dimensions: VegaMdlProofDimensionsV1,
    ) -> Result<(), VegaMdlProofErrorV1> {
        if self.version != PROOF_VERSION
            || self.mask_witness_commitment.points.len() != dimensions.witness_commitment_points
            || self.regular_witness_commitment.points.len() != dimensions.witness_commitment_points
            || self.mask_error_commitment.points.len() != dimensions.error_commitment_points
            || self.cross_term_commitment.points.len() != dimensions.error_commitment_points
            || self.outer_sumcheck_rounds.len() != dimensions.outer_sumcheck_rounds
            || self.inner_sumcheck_rounds.len() != dimensions.inner_sumcheck_rounds
            || self.witness_opening.len() != dimensions.commitment_columns
            || self.error_opening.len() != dimensions.commitment_columns
        {
            return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
        }
        Ok(())
    }

    fn to_protocol(
        &self,
        regular_public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    ) -> Result<(RelaxedInstance, Instance, NovaNifs, RelaxedSpartanProof), VegaMdlProofErrorV1>
    {
        let mask = RelaxedInstance {
            witness_commitment: self.mask_witness_commitment.to_commitment()?,
            error_commitment: self.mask_error_commitment.to_commitment()?,
            public_inputs: wire_to_scalars(&self.mask_public_inputs)?,
            relaxation: self
                .mask_relaxation
                .to_scalar()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?,
        };
        let regular = Instance {
            witness_commitment: self.regular_witness_commitment.to_commitment()?,
            public_inputs: regular_public_inputs.to_vec(),
        };
        let nifs = NovaNifs {
            cross_term_commitment: self.cross_term_commitment.to_commitment()?,
        };
        let spartan = RelaxedSpartanProof {
            outer_sumcheck: SumcheckProof::new(
                self.outer_sumcheck_rounds
                    .iter()
                    .map(|round| {
                        CompressedUnivariate::new(wire_to_scalars(round)?, 3)
                            .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)
                    })
                    .collect::<Result<_, _>>()?,
            ),
            outer_claims: wire_to_scalar_array(&self.outer_claims)?,
            inner_sumcheck: SumcheckProof::new(
                self.inner_sumcheck_rounds
                    .iter()
                    .map(|round| {
                        CompressedUnivariate::new(wire_to_scalars(round)?, 2)
                            .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)
                    })
                    .collect::<Result<_, _>>()?,
            ),
            witness_opening: wire_to_scalars(&self.witness_opening)?,
            witness_opening_blinding: self
                .witness_opening_blinding
                .to_scalar()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?,
            error_opening: wire_to_scalars(&self.error_opening)?,
            error_opening_blinding: self
                .error_opening_blinding
                .to_scalar()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?,
        };
        Ok((mask, regular, nifs, spartan))
    }
}

impl CommitmentWireV1 {
    fn from_commitment(commitment: &Commitment) -> Result<Self, VegaMdlProofErrorV1> {
        Ok(Self {
            points: commitment
                .points()
                .iter()
                .copied()
                .map(VegaPointWireV1::from_point)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?,
        })
    }

    fn to_commitment(&self) -> Result<Commitment, VegaMdlProofErrorV1> {
        Commitment::from_points(
            self.points
                .iter()
                .copied()
                .map(VegaPointWireV1::to_point)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?,
        )
        .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)
    }
}

fn canonical_shape() -> Result<Shape, VegaMdlProofErrorV1> {
    CANONICAL_SHAPE
        .as_ref()
        .map(Clone::clone)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

fn commitment_key(worker_count: usize) -> Result<CommitmentKey, VegaMdlProofErrorV1> {
    COMMITMENT_KEY
        .as_ref()
        .map(Clone::clone)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
        .and_then(|key| {
            key.with_worker_count(worker_count).map_err(|_| {
                VegaMdlProofErrorV1::InvalidWorkerCount {
                    actual: worker_count,
                    min: 1,
                    max: MAX_VEGA_PROVER_WORKERS_V1,
                }
            })
        })
}

fn build_canonical_shape() -> Result<Shape, CircuitError> {
    let public_inputs = [
        Scalar::from_u64(1),
        Scalar::from_u64(1),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::zero(),
        Scalar::from_u64(2026),
        Scalar::from_u64(7),
        Scalar::from_u64(26),
        Scalar::from_u64(18),
    ];
    let one = [1_u8; 32];
    let witness = VegaMdlFigure9WitnessV1::new(
        &FIGURE9_LAYOUT.issuer_template,
        &FIGURE9_LAYOUT.birth_template,
        &one,
        &one,
        &one,
        &one,
    )
    .map_err(|_| CircuitError::InvalidDimension)?;
    synthesize_figure9(&public_inputs, &witness).map(|assignment| assignment.shape)
}

fn sample_relaxed_mask<R: VegaRandomSourceV1>(
    random: &mut R,
    shape: &Shape,
    key: &CommitmentKey,
    dimensions: VegaMdlProofDimensionsV1,
) -> Result<(RelaxedInstance, RelaxedWitness), VegaMdlProofErrorV1> {
    let values = sample_scalars(random, shape.variable_count())?;
    let relaxation = sample_scalar(random)?;
    let public_inputs = sample_scalars(random, shape.public_input_count())?;
    if relaxation.is_zero()
        && values.iter().all(|value| value.is_zero())
        && public_inputs.iter().all(|value| value.is_zero())
    {
        return Err(VegaMdlProofErrorV1::DegenerateRandomness);
    }
    let mut assignment = Vec::with_capacity(shape.columns());
    assignment.extend_from_slice(&values);
    assignment.push(relaxation);
    assignment.extend_from_slice(&public_inputs);
    let (a, b, c) = shape
        .multiply(&assignment)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let error = a
        .into_iter()
        .zip(b)
        .zip(c)
        .map(|((a, b), c)| a * b - relaxation * c)
        .collect::<Vec<_>>();
    let witness_blindings = sample_nonzero_scalars(random, dimensions.witness_commitment_points)?;
    let error_blindings = sample_nonzero_scalars(random, dimensions.error_commitment_points)?;
    let witness_commitment = key
        .commit(&values, &witness_blindings)
        .map_err(|_| VegaMdlProofErrorV1::DegenerateRandomness)?;
    let error_commitment = key
        .commit(&error, &error_blindings)
        .map_err(|_| VegaMdlProofErrorV1::DegenerateRandomness)?;
    Ok((
        RelaxedInstance {
            witness_commitment,
            error_commitment,
            public_inputs,
            relaxation,
        },
        RelaxedWitness {
            values,
            witness_blindings,
            error,
            error_blindings,
        },
    ))
}

fn sample_scalar<R: VegaRandomSourceV1>(random: &mut R) -> Result<Scalar, VegaMdlProofErrorV1> {
    let mut wide = [0_u8; 64];
    random.fill_bytes(&mut wide)?;
    Ok(Scalar::from_uniform_le_bytes(wide))
}

fn sample_nonzero_scalar<R: VegaRandomSourceV1>(
    random: &mut R,
) -> Result<Scalar, VegaMdlProofErrorV1> {
    for _ in 0..RANDOM_HEALTH_RETRIES {
        let value = sample_scalar(random)?;
        if !value.is_zero() {
            return Ok(value);
        }
    }
    Err(VegaMdlProofErrorV1::DegenerateRandomness)
}

fn validate_random_health<R: VegaRandomSourceV1>(
    random: &mut R,
) -> Result<(), VegaMdlProofErrorV1> {
    let first = sample_nonzero_scalar(random)?;
    for _ in 0..RANDOM_HEALTH_RETRIES {
        if sample_nonzero_scalar(random)? != first {
            return Ok(());
        }
    }
    Err(VegaMdlProofErrorV1::DegenerateRandomness)
}

fn sample_scalars<R: VegaRandomSourceV1>(
    random: &mut R,
    count: usize,
) -> Result<Vec<Scalar>, VegaMdlProofErrorV1> {
    (0..count).map(|_| sample_scalar(random)).collect()
}

fn sample_nonzero_scalars<R: VegaRandomSourceV1>(
    random: &mut R,
    count: usize,
) -> Result<Vec<Scalar>, VegaMdlProofErrorV1> {
    (0..count).map(|_| sample_nonzero_scalar(random)).collect()
}

fn release_transcript(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    shape: &Shape,
    dimensions: VegaMdlProofDimensionsV1,
) -> Result<VegaTranscriptV1, VegaMdlProofErrorV1> {
    let mut transcript = VegaTranscriptV1::new_neutron_nova();
    transcript
        .domain_separator(VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let frame = context_frame(context, public_inputs, shape, dimensions)?;
    transcript
        .absorb_raw(b"figure9_release_context_v1", &frame)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    Ok(transcript)
}

fn context_frame(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    shape: &Shape,
    dimensions: VegaMdlProofDimensionsV1,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    let profile = profile_frame(shape, dimensions)?;
    let profile_digest = keccak256(&profile);
    if profile_digest != VEGA_MDL_COMPILED_PROFILE_DIGEST_V1 {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    let mut frame = Vec::with_capacity(2048);
    push_frame_field(&mut frame, 0, VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1)?;
    push_frame_field(&mut frame, 1, VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1)?;
    push_frame_field(&mut frame, 2, context.chain_id)?;
    push_frame_field(&mut frame, 3, &context.genesis_hash)?;
    push_frame_field(&mut frame, 4, &context.action_index.to_be_bytes())?;
    push_frame_field(&mut frame, 5, &context.parameter_id)?;
    push_frame_field(&mut frame, 6, &context.parameter_digest)?;
    push_frame_field(&mut frame, 7, &context.verifier_digest)?;
    push_frame_field(&mut frame, 8, &context.statement_schema_digest)?;
    push_frame_field(&mut frame, 9, &context.engine_manifest_digest)?;
    push_frame_field(&mut frame, 10, &profile_digest)?;

    let mut statement = Vec::with_capacity(4 + 32 * public_inputs.len());
    statement.extend_from_slice(
        &u32::try_from(public_inputs.len())
            .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
            .to_be_bytes(),
    );
    for input in public_inputs {
        statement.extend_from_slice(&input.to_be_bytes());
    }
    push_frame_field(&mut frame, 11, &statement)?;
    push_frame_field(&mut frame, 12, &profile)?;
    Ok(frame)
}

struct CanonicalRelationMaterialV1<'a> {
    dimensions: [usize; 4],
    matrices: [&'a SparseMatrix; 3],
    issuer_template: &'a [u8],
    issuer_fixed: &'a [bool],
    birth_template: &'a [u8],
    birth_fixed: &'a [bool],
    layout_ranges: [core::ops::Range<usize>; 8],
    layout_schema: &'a [u8],
    semantic_schema: &'a [u8],
    semantics: [u64; 13],
}

fn canonical_relation_digest(shape: &Shape) -> Result<[u8; 32], VegaMdlProofErrorV1> {
    let material = CanonicalRelationMaterialV1 {
        dimensions: [
            shape.constraint_count(),
            shape.variable_count(),
            shape.public_input_count(),
            shape.columns(),
        ],
        matrices: [&shape.a, &shape.b, &shape.c],
        issuer_template: &FIGURE9_LAYOUT.issuer_template,
        issuer_fixed: &FIGURE9_LAYOUT.issuer_fixed,
        birth_template: &FIGURE9_LAYOUT.birth_template,
        birth_fixed: &FIGURE9_LAYOUT.birth_fixed,
        layout_ranges: [
            FIGURE9_LAYOUT.issuer_birth_digest.clone(),
            FIGURE9_LAYOUT.issuer_device_x.clone(),
            FIGURE9_LAYOUT.issuer_device_y.clone(),
            FIGURE9_LAYOUT.issuer_signed_datetime.clone(),
            FIGURE9_LAYOUT.issuer_valid_from_datetime.clone(),
            FIGURE9_LAYOUT.issuer_valid_until_datetime.clone(),
            FIGURE9_LAYOUT.birth_random.clone(),
            FIGURE9_LAYOUT.birth_date.clone(),
        ],
        layout_schema: CANONICAL_RELATION_LAYOUT_SCHEMA_V1,
        semantic_schema: CANONICAL_RELATION_SEMANTIC_SCHEMA_V1,
        semantics: [
            relation_usize_to_u64(VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1)?,
            relation_usize_to_u64(VEGA_MDL_MSO_PAYLOAD_BYTES_V1)?,
            relation_usize_to_u64(VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1)?,
            relation_usize_to_u64(VEGA_MDL_BIRTH_RANDOM_BYTES_V1)?,
            relation_usize_to_u64(VEGA_MDL_FULL_DATE_TEXT_BYTES_V1)?,
            relation_usize_to_u64(VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1)?,
            u64::from(VEGA_MDL_MIN_PRESENTATION_YEAR_V1),
            u64::from(VEGA_MDL_MAX_PRESENTATION_YEAR_V1),
            u64::from(VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1),
            u64::from(VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1),
            relation_usize_to_u64(VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1)?,
            RFC3339_SECONDS_PER_DAY_V1,
            relation_usize_to_u64(RFC3339_TIMESTAMP_ORDER_SLACK_BITS_V1)?,
        ],
    };
    canonical_relation_digest_from_material(&material)
}

fn canonical_relation_digest_from_material(
    material: &CanonicalRelationMaterialV1<'_>,
) -> Result<[u8; 32], VegaMdlProofErrorV1> {
    let mut hash = Keccak256::new();
    hash_relation_field(&mut hash, 0, CANONICAL_RELATION_DIGEST_DOMAIN_V1)?;

    let mut dimensions = Vec::with_capacity(4 * 8);
    for value in material.dimensions {
        dimensions.extend_from_slice(&relation_usize_to_u64(value)?.to_be_bytes());
    }
    hash_relation_field(&mut hash, 1, &dimensions)?;
    hash_relation_matrix(&mut hash, 2, material.matrices[0])?;
    hash_relation_matrix(&mut hash, 3, material.matrices[1])?;
    hash_relation_matrix(&mut hash, 4, material.matrices[2])?;
    hash_relation_field(&mut hash, 5, material.issuer_template)?;
    hash_relation_mask(&mut hash, 6, material.issuer_fixed)?;
    hash_relation_field(&mut hash, 7, material.birth_template)?;
    hash_relation_mask(&mut hash, 8, material.birth_fixed)?;
    hash_relation_field(&mut hash, 9, material.layout_schema)?;

    let mut ranges = Vec::with_capacity(8 * 2 * 8);
    for range in &material.layout_ranges {
        ranges.extend_from_slice(&relation_usize_to_u64(range.start)?.to_be_bytes());
        ranges.extend_from_slice(&relation_usize_to_u64(range.end)?.to_be_bytes());
    }
    hash_relation_field(&mut hash, 10, &ranges)?;
    hash_relation_field(&mut hash, 11, material.semantic_schema)?;

    let mut semantics = Vec::with_capacity(material.semantics.len() * 8);
    for value in material.semantics {
        semantics.extend_from_slice(&value.to_be_bytes());
    }
    hash_relation_field(&mut hash, 12, &semantics)?;
    Ok(hash.finalize())
}

fn hash_relation_matrix(
    hash: &mut Keccak256,
    tag: u8,
    matrix: &SparseMatrix,
) -> Result<(), VegaMdlProofErrorV1> {
    const MATRIX_HEADER_BYTES: usize = 3 * 8;
    const MATRIX_ENTRY_BYTES: usize = 8 + 8 + 32;
    let payload_len = matrix
        .entry_count()
        .checked_mul(MATRIX_ENTRY_BYTES)
        .and_then(|entries| entries.checked_add(MATRIX_HEADER_BYTES))
        .ok_or(VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    hash_relation_field_header(hash, tag, payload_len)?;
    for value in [matrix.rows(), matrix.columns(), matrix.entry_count()] {
        hash.update(&relation_usize_to_u64(value)?.to_be_bytes());
    }
    for (row, column, coefficient) in matrix.canonical_entries() {
        let mut entry = [0_u8; MATRIX_ENTRY_BYTES];
        entry[..8].copy_from_slice(&relation_usize_to_u64(row)?.to_be_bytes());
        entry[8..16].copy_from_slice(&relation_usize_to_u64(column)?.to_be_bytes());
        entry[16..].copy_from_slice(&coefficient.to_be_bytes());
        hash.update(&entry);
    }
    Ok(())
}

fn hash_relation_mask(
    hash: &mut Keccak256,
    tag: u8,
    mask: &[bool],
) -> Result<(), VegaMdlProofErrorV1> {
    hash_relation_field_header(hash, tag, mask.len())?;
    for value in mask {
        hash.update(&[u8::from(*value)]);
    }
    Ok(())
}

fn hash_relation_field(
    hash: &mut Keccak256,
    tag: u8,
    value: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    hash_relation_field_header(hash, tag, value.len())?;
    hash.update(value);
    Ok(())
}

fn hash_relation_field_header(
    hash: &mut Keccak256,
    tag: u8,
    value_len: usize,
) -> Result<(), VegaMdlProofErrorV1> {
    hash.update(&[tag]);
    hash.update(&relation_usize_to_u64(value_len)?.to_be_bytes());
    Ok(())
}

fn relation_usize_to_u64(value: usize) -> Result<u64, VegaMdlProofErrorV1> {
    u64::try_from(value).map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

fn profile_frame(
    shape: &Shape,
    dimensions: VegaMdlProofDimensionsV1,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    let relation_digest = canonical_relation_digest(shape)?;
    if relation_digest != VEGA_MDL_CANONICAL_RELATION_DIGEST_V1 {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    profile_frame_with_relation_digest(shape, dimensions, relation_digest)
}

fn profile_frame_with_relation_digest(
    shape: &Shape,
    dimensions: VegaMdlProofDimensionsV1,
    relation_digest: [u8; 32],
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    let mut profile = Vec::with_capacity(512);
    push_frame_field(&mut profile, 0, PROFILE_DESCRIPTOR)?;
    push_frame_field(&mut profile, 1, PINNED_SOURCE_COMMIT)?;
    push_frame_field(&mut profile, 2, COMMITMENT_KEY_LABEL)?;
    for (tag, value) in [
        (3, PROOF_VERSION as u64),
        (4, FIGURE9_LAYOUT.issuer_template.len() as u64),
        (5, FIGURE9_LAYOUT.birth_template.len() as u64),
        (6, shape.variable_count() as u64),
        (7, shape.constraint_count() as u64),
        (8, shape.public_input_count() as u64),
        (9, dimensions.commitment_columns as u64),
        (10, dimensions.witness_commitment_points as u64),
        (11, dimensions.error_commitment_points as u64),
        (12, dimensions.outer_sumcheck_rounds as u64),
        (13, 3),
        (14, dimensions.inner_sumcheck_rounds as u64),
        (15, 2),
        (16, dimensions.commitment_columns as u64),
        (17, dimensions.commitment_columns as u64),
        (18, MAX_VEGA_PROOF_BYTES_V1 as u64),
    ] {
        push_frame_field(&mut profile, tag, &value.to_be_bytes())?;
    }
    let outer_indices = (0..dimensions.outer_sumcheck_rounds)
        .flat_map(|index| (index as u16).to_be_bytes())
        .collect::<Vec<_>>();
    let inner_indices = (0..dimensions.inner_sumcheck_rounds)
        .flat_map(|index| (index as u16).to_be_bytes())
        .collect::<Vec<_>>();
    push_frame_field(&mut profile, 19, &outer_indices)?;
    push_frame_field(&mut profile, 20, &inner_indices)?;
    push_frame_field(&mut profile, 21, &relation_digest)?;
    Ok(profile)
}

fn push_frame_field(
    destination: &mut Vec<u8>,
    tag: u8,
    value: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    let length =
        u32::try_from(value.len()).map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    destination.push(tag);
    destination.extend_from_slice(&length.to_be_bytes());
    destination.extend_from_slice(value);
    Ok(())
}

fn validate_context(context: &VegaMdlProofContextV1<'_>) -> Result<(), VegaMdlProofErrorV1> {
    if context.chain_id.is_empty()
        || context.chain_id.len() > MAX_CHAIN_ID_BYTES
        || [
            context.genesis_hash,
            context.parameter_id,
            context.parameter_digest,
            context.verifier_digest,
            context.statement_schema_digest,
            context.engine_manifest_digest,
        ]
        .into_iter()
        .any(|digest| digest == [0; 32])
    {
        return Err(VegaMdlProofErrorV1::InvalidContext);
    }
    Ok(())
}

fn scalars_to_wire_array<const N: usize>(
    scalars: &[Scalar],
) -> Result<[VegaScalarWireV1; N], VegaMdlProofErrorV1> {
    scalars
        .iter()
        .copied()
        .map(VegaScalarWireV1::from_scalar)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

fn wire_to_scalar_array<const N: usize>(
    scalars: &[VegaScalarWireV1; N],
) -> Result<[Scalar; N], VegaMdlProofErrorV1> {
    wire_to_scalars(scalars)?
        .try_into()
        .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)
}

fn wire_to_scalars(scalars: &[VegaScalarWireV1]) -> Result<Vec<Scalar>, VegaMdlProofErrorV1> {
    scalars
        .iter()
        .copied()
        .map(|scalar| {
            scalar
                .to_scalar()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::figure9::tests::baseline_signed_fixture;
    use crate::vega::r1cs::R1csError;

    struct ZeroRandom;

    impl VegaRandomSourceV1 for ZeroRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            destination.fill(0);
            Ok(())
        }
    }

    struct FailureRandom;

    impl VegaRandomSourceV1 for FailureRandom {
        fn fill_bytes(&mut self, _: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            Err(VegaRandomSourceErrorV1::Unavailable)
        }
    }

    struct ConstantRandom(u8);

    impl VegaRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    /// Deterministic conformance tape only; never a production CSPRNG.
    struct TapeRandom(u64);

    impl VegaRandomSourceV1 for TapeRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            for chunk in destination.chunks_mut(8) {
                let mut value = self.0;
                value ^= value << 13;
                value ^= value >> 7;
                value ^= value << 17;
                self.0 = value;
                chunk.copy_from_slice(&value.to_le_bytes()[..chunk.len()]);
            }
            Ok(())
        }
    }

    fn relation_digest_test_shape(
        a_entries: &[(usize, usize, Scalar)],
    ) -> Result<Shape, R1csError> {
        let a = SparseMatrix::new(2, 3, a_entries)?;
        let b = SparseMatrix::new(2, 3, &[(0, 1, Scalar::one())])?;
        let c = SparseMatrix::new(2, 3, &[(0, 2, Scalar::one())])?;
        Shape::new(2, 1, 1, a, b, c)
    }

    #[allow(clippy::too_many_arguments)]
    fn relation_digest_test_material<'a>(
        shape: &'a Shape,
        dimensions: [usize; 4],
        issuer_template: &'a [u8],
        issuer_fixed: &'a [bool],
        birth_template: &'a [u8],
        birth_fixed: &'a [bool],
        layout_ranges: [core::ops::Range<usize>; 8],
        semantics: [u64; 13],
    ) -> CanonicalRelationMaterialV1<'a> {
        CanonicalRelationMaterialV1 {
            dimensions,
            matrices: [&shape.a, &shape.b, &shape.c],
            issuer_template,
            issuer_fixed,
            birth_template,
            birth_fixed,
            layout_ranges,
            layout_schema: b"test-layout-schema-v1",
            semantic_schema: b"test-semantic-schema-v1",
            semantics,
        }
    }

    fn context() -> VegaMdlProofContextV1<'static> {
        VegaMdlProofContextV1 {
            chain_id: b"taira-vega-test",
            genesis_hash: [0x11; 32],
            action_index: 3,
            parameter_id: [0x21; 32],
            parameter_digest: [0x22; 32],
            verifier_digest: [0x23; 32],
            statement_schema_digest: [0x24; 32],
            engine_manifest_digest: [0x25; 32],
        }
    }

    fn decode_proof(proof: &[u8]) -> VegaMdlProofWireV1 {
        norito::codec::decode_exact_from_slice(proof).expect("canonical proof wire")
    }

    fn encode_proof(proof: &VegaMdlProofWireV1) -> Vec<u8> {
        norito::codec::encode_adaptive(proof)
    }

    fn assert_invalid_shape(
        public: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
        proof: &VegaMdlProofWireV1,
    ) {
        assert_eq!(
            verify_vega_mdl_figure9_v1(&context(), public, &encode_proof(proof)),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );
    }

    fn assert_verification_failure(
        public: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
        proof: &VegaMdlProofWireV1,
    ) {
        assert_eq!(
            verify_vega_mdl_figure9_v1(&context(), public, &encode_proof(proof)),
            Err(VegaMdlProofErrorV1::VerificationFailed)
        );
    }

    fn increment_scalar(value: VegaScalarWireV1) -> VegaScalarWireV1 {
        VegaScalarWireV1::from_scalar(value.to_scalar().expect("canonical scalar") + Scalar::one())
    }

    #[test]
    fn compiled_dimensions_are_exact_and_within_work_cap() {
        let dimensions = vega_mdl_proof_dimensions_v1().expect("compiled profile");
        assert_eq!(
            dimensions,
            VegaMdlProofDimensionsV1 {
                variable_count: 524_288,
                constraint_count: 1_048_576,
                commitment_columns: 1_024,
                witness_commitment_points: 512,
                error_commitment_points: 1_024,
                outer_sumcheck_rounds: 20,
                inner_sumcheck_rounds: 20,
            }
        );
        assert_eq!(dimensions.constraint_count, MAX_CIRCUIT_ROWS);
    }

    #[test]
    fn proof_decoder_preflights_oversized_and_forged_vector_counts() {
        let dimensions = VegaMdlProofDimensionsV1 {
            variable_count: 524_288,
            constraint_count: 1_048_576,
            commitment_columns: 1_024,
            witness_commitment_points: 512,
            error_commitment_points: 1_024,
            outer_sumcheck_rounds: 20,
            inner_sumcheck_rounds: 20,
        };
        let point = VegaPointWireV1::from_raw_bytes_for_test([1; 33]);
        let scalar = VegaScalarWireV1::from_raw_bytes_for_test([0; 32]);
        let oversized = VegaMdlProofWireV1 {
            version: PROOF_VERSION,
            mask_witness_commitment: CommitmentWireV1 {
                points: vec![point; dimensions.commitment_columns + 1],
            },
            mask_error_commitment: CommitmentWireV1 { points: Vec::new() },
            mask_relaxation: scalar,
            mask_public_inputs: [scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
            regular_witness_commitment: CommitmentWireV1 { points: Vec::new() },
            cross_term_commitment: CommitmentWireV1 { points: Vec::new() },
            outer_sumcheck_rounds: Vec::new(),
            outer_claims: [scalar; 3],
            inner_sumcheck_rounds: Vec::new(),
            witness_opening: Vec::new(),
            witness_opening_blinding: scalar,
            error_opening: Vec::new(),
            error_opening_blinding: scalar,
        };
        let encoded = encode_proof(&oversized);
        let limits = dimensions
            .proof_decode_limits(encoded.len())
            .expect("released decoder limits");
        assert!(matches!(
            norito::codec::decode_exact_from_slice_with_limits::<VegaMdlProofWireV1>(
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
            .expect("oversized vector count is present in canonical wire");
        let mut forged = encoded;
        forged[count_offset..count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let limits = dimensions
            .proof_decode_limits(forged.len())
            .expect("released decoder limits");
        let forged_error =
            norito::codec::decode_exact_from_slice_with_limits::<VegaMdlProofWireV1>(
                &forged, limits,
            )
            .expect_err("forged vector count must fail before allocation");
        assert!(
            matches!(forged_error, norito::Error::SequenceLengthExceeded { length, limit }
                if length == u64::from(u32::MAX)
                    && limit <= MAX_VEGA_PROOF_BYTES_V1 as u64
                    && limit < length),
            "unexpected forged-count rejection: {forged_error:?}"
        );
    }

    #[test]
    fn context_frame_binds_every_field_profile_round_index_and_vector_length() {
        let shape = canonical_shape().expect("shape");
        let dimensions = VegaMdlProofDimensionsV1::from_shape(&shape).expect("dimensions");
        let public = [Scalar::from_u64(7); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1];
        let baseline = context_frame(&context(), &public, &shape, dimensions).expect("frame");

        let mut candidates = Vec::new();
        let mut changed = context();
        changed.chain_id = b"taira-vega-other";
        candidates.push(changed);
        let mut changed = context();
        changed.genesis_hash[0] ^= 1;
        candidates.push(changed);
        let mut changed = context();
        changed.action_index += 1;
        candidates.push(changed);
        let mut changed = context();
        changed.parameter_id[0] ^= 1;
        candidates.push(changed);
        let mut changed = context();
        changed.parameter_digest[0] ^= 1;
        candidates.push(changed);
        let mut changed = context();
        changed.verifier_digest[0] ^= 1;
        candidates.push(changed);
        let mut changed = context();
        changed.statement_schema_digest[0] ^= 1;
        candidates.push(changed);
        let mut changed = context();
        changed.engine_manifest_digest[0] ^= 1;
        candidates.push(changed);
        for changed in candidates {
            assert_ne!(
                context_frame(&changed, &public, &shape, dimensions).expect("frame"),
                baseline
            );
        }
        for index in 0..public.len() {
            let mut changed = public;
            changed[index] += Scalar::one();
            assert_ne!(
                context_frame(&context(), &changed, &shape, dimensions).expect("statement frame"),
                baseline,
                "public input {index}"
            );
        }
        let profile = profile_frame(&shape, dimensions).expect("profile");
        for index in 0..dimensions.outer_sumcheck_rounds {
            assert!(
                profile
                    .windows(2)
                    .any(|window| window == (index as u16).to_be_bytes())
            );
        }
    }

    #[test]
    fn random_source_failure_zero_and_constant_tapes_fail_closed() {
        assert_eq!(
            sample_scalar(&mut FailureRandom),
            Err(VegaMdlProofErrorV1::RandomSource(
                VegaRandomSourceErrorV1::Unavailable
            ))
        );
        assert_eq!(
            sample_nonzero_scalar(&mut ZeroRandom),
            Err(VegaMdlProofErrorV1::DegenerateRandomness)
        );
        assert_eq!(
            validate_random_health(&mut ConstantRandom(1)),
            Err(VegaMdlProofErrorV1::DegenerateRandomness)
        );
        let mut healthy = TapeRandom(0x9e37_79b9_7f4a_7c15);
        validate_random_health(&mut healthy).expect("distinct deterministic conformance draws");
    }

    #[test]
    fn canonical_relation_digest_binds_full_sparse_entries_and_fixed_layout() {
        let shape = canonical_shape().expect("canonical shape");
        let digest = canonical_relation_digest(&shape).expect("canonical relation digest");
        assert_eq!(digest, VEGA_MDL_CANONICAL_RELATION_DIGEST_V1);
        assert_eq!(
            hex::encode(VEGA_MDL_CANONICAL_RELATION_DIGEST_V1),
            "f327bb5b0aa3a09418a7c062cac81196cdfb65c56259e90195f80983da5307e3"
        );

        let base_shape =
            relation_digest_test_shape(&[(0, 0, Scalar::one())]).expect("base test shape");
        let dimensions = [2, 1, 1, 3];
        let issuer_template = vec![0x10, 0x11, 0x12];
        let issuer_fixed = vec![true, false, true];
        let birth_template = vec![0x20, 0x21, 0x22];
        let birth_fixed = vec![true, true, false];
        let layout_ranges = [0..1, 1..2, 2..3, 0..1, 1..2, 2..3, 0..2, 2..3];
        let semantics = core::array::from_fn(|index| (index + 1) as u64);
        let base_digest = canonical_relation_digest_from_material(&relation_digest_test_material(
            &base_shape,
            dimensions,
            &issuer_template,
            &issuer_fixed,
            &birth_template,
            &birth_fixed,
            layout_ranges.clone(),
            semantics,
        ))
        .expect("base test material");

        for (mutation, changed_shape) in [
            (
                "coefficient",
                relation_digest_test_shape(&[(0, 0, Scalar::from_u64(2))])
                    .expect("coefficient mutation"),
            ),
            (
                "row",
                relation_digest_test_shape(&[(1, 0, Scalar::one())]).expect("row mutation"),
            ),
            (
                "column",
                relation_digest_test_shape(&[(0, 1, Scalar::one())]).expect("column mutation"),
            ),
        ] {
            assert_ne!(
                canonical_relation_digest_from_material(&relation_digest_test_material(
                    &changed_shape,
                    dimensions,
                    &issuer_template,
                    &issuer_fixed,
                    &birth_template,
                    &birth_fixed,
                    layout_ranges.clone(),
                    semantics,
                ))
                .expect("mutated sparse material"),
                base_digest,
                "{mutation} mutation must change the canonical relation digest"
            );
        }

        let mut changed_issuer_template = issuer_template.clone();
        changed_issuer_template[1] ^= 1;
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &changed_issuer_template,
                &issuer_fixed,
                &birth_template,
                &birth_fixed,
                layout_ranges.clone(),
                semantics,
            ))
            .expect("issuer template mutation"),
            base_digest
        );
        let mut changed_issuer_mask = issuer_fixed.clone();
        changed_issuer_mask[1] = !changed_issuer_mask[1];
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &issuer_template,
                &changed_issuer_mask,
                &birth_template,
                &birth_fixed,
                layout_ranges.clone(),
                semantics,
            ))
            .expect("issuer mask mutation"),
            base_digest
        );
        let mut changed_birth_template = birth_template.clone();
        changed_birth_template[2] ^= 1;
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &issuer_template,
                &issuer_fixed,
                &changed_birth_template,
                &birth_fixed,
                layout_ranges.clone(),
                semantics,
            ))
            .expect("birth template mutation"),
            base_digest
        );
        let mut changed_birth_mask = birth_fixed.clone();
        changed_birth_mask[2] = !changed_birth_mask[2];
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &issuer_template,
                &issuer_fixed,
                &birth_template,
                &changed_birth_mask,
                layout_ranges.clone(),
                semantics,
            ))
            .expect("birth mask mutation"),
            base_digest
        );
        let mut changed_ranges = layout_ranges.clone();
        changed_ranges[5].end += 1;
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &issuer_template,
                &issuer_fixed,
                &birth_template,
                &birth_fixed,
                changed_ranges,
                semantics,
            ))
            .expect("layout endpoint mutation"),
            base_digest
        );
        let mut changed_semantics = semantics;
        changed_semantics[11] += 1;
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                dimensions,
                &issuer_template,
                &issuer_fixed,
                &birth_template,
                &birth_fixed,
                layout_ranges.clone(),
                changed_semantics,
            ))
            .expect("semantic mutation"),
            base_digest
        );
        let mut changed_dimensions = dimensions;
        changed_dimensions[2] += 1;
        assert_ne!(
            canonical_relation_digest_from_material(&relation_digest_test_material(
                &base_shape,
                changed_dimensions,
                &issuer_template,
                &issuer_fixed,
                &birth_template,
                &birth_fixed,
                layout_ranges,
                semantics,
            ))
            .expect("dimension mutation"),
            base_digest
        );

        assert_eq!(
            SparseMatrix::new(2, 3, &[(0, 0, Scalar::one()), (0, 0, Scalar::from_u64(2)),],),
            Err(R1csError::NonCanonicalMatrix)
        );
        assert_eq!(
            SparseMatrix::new(2, 3, &[(1, 0, Scalar::one()), (0, 1, Scalar::one())],),
            Err(R1csError::NonCanonicalMatrix)
        );
    }

    #[test]
    #[ignore = "operator-only KAT regeneration after an intentional relation change"]
    fn print_canonical_relation_and_compiled_profile_digests() {
        let shape = canonical_shape().expect("canonical shape");
        let dimensions = VegaMdlProofDimensionsV1::from_shape(&shape).expect("dimensions");
        let relation = canonical_relation_digest(&shape).expect("relation");
        let profile =
            profile_frame_with_relation_digest(&shape, dimensions, relation).expect("profile");
        eprintln!(
            "VEGA_MDL_CANONICAL_RELATION_DIGEST_V1={}",
            hex::encode(relation)
        );
        eprintln!(
            "VEGA_MDL_COMPILED_PROFILE_DIGEST_V1={}",
            hex::encode(keccak256(&profile))
        );
    }

    #[test]
    fn compiled_profile_digest_matches_the_exact_circuit_shape() {
        let shape = canonical_shape().expect("canonical shape");
        let dimensions = VegaMdlProofDimensionsV1::from_shape(&shape).expect("dimensions");
        let recomputed = keccak256(&profile_frame(&shape, dimensions).expect("profile frame"));
        assert_eq!(recomputed, VEGA_MDL_COMPILED_PROFILE_DIGEST_V1);
        assert_eq!(
            hex::encode(VEGA_MDL_COMPILED_PROFILE_DIGEST_V1),
            "fd97bb0f9d673677182f0b8734609c7c0347806324ee64c374ccc6a7a15ee473"
        );
        assert_eq!(
            vega_mdl_compiled_profile_digest_v1(),
            VEGA_MDL_COMPILED_PROFILE_DIGEST_V1
        );
    }

    #[test]
    fn invalid_contexts_fail_closed() {
        let mut invalid = context();
        invalid.chain_id = b"";
        assert_eq!(
            validate_context(&invalid),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
        let mut invalid = context();
        invalid.verifier_digest = [0; 32];
        assert_eq!(
            validate_context(&invalid),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
    }

    #[test]
    fn prover_worker_counts_and_concurrent_scratch_are_hard_bounded() {
        assert_eq!(
            VegaMdlProverConfigV1::new(0),
            Err(VegaMdlProofErrorV1::InvalidWorkerCount {
                actual: 0,
                min: 1,
                max: 20,
            })
        );
        assert_eq!(
            VegaMdlProverConfigV1::new(21),
            Err(VegaMdlProofErrorV1::InvalidWorkerCount {
                actual: 21,
                min: 1,
                max: 20,
            })
        );
        let maximum = VegaMdlProverConfigV1::new(20).expect("maximum workers");
        assert_eq!(
            maximum.commitment_worker_scratch_bound_bytes(),
            15 * 1024 * 1024
        );
        assert_eq!(
            maximum.release_memory_ceiling_bytes(),
            MAX_VEGA_PROVER_RELEASE_MEMORY_CEILING_BYTES_V1
        );
        assert_eq!(
            VegaMdlProverConfigV1::new(1)
                .expect("one worker")
                .release_memory_ceiling_bytes(),
            VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1 + 768 * 1024
        );
    }

    #[test]
    fn signed_figure9_proof_roundtrips_under_the_exact_byte_cap() {
        let fixture = baseline_signed_fixture();
        let mut random = TapeRandom(0x9e37_79b9_7f4a_7c15);
        let proof = prove_vega_mdl_figure9_v1(
            &context(),
            &fixture.public,
            &fixture.witness(),
            VegaMdlProverConfigV1::new(20).expect("bounded workers"),
            &mut random,
        )
        .expect("canonical proof");
        assert_eq!(proof.len(), 181_375);
        verify_vega_mdl_figure9_v1(&context(), &fixture.public, &proof)
            .expect("canonical verification");
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "single-worker full Figure 9 proof is a release resource gate"
    )]
    fn single_worker_signed_figure9_proof_roundtrips_at_frozen_size() {
        let fixture = baseline_signed_fixture();
        let mut random = TapeRandom(0x9e37_79b9_7f4a_7c15);
        let proof = prove_vega_mdl_figure9_v1(
            &context(),
            &fixture.public,
            &fixture.witness(),
            VegaMdlProverConfigV1::new(1).expect("one worker"),
            &mut random,
        )
        .expect("one-worker canonical proof");
        assert_eq!(proof.len(), 181_375);
        verify_vega_mdl_figure9_v1(&context(), &fixture.public, &proof)
            .expect("one-worker canonical verification");
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "three full Figure 9 proofs are a release-mode determinism and adversarial gate"
    )]
    fn one_and_twenty_workers_are_byte_identical_and_full_proof_adversarial_gate() {
        let fixture = baseline_signed_fixture();
        let seed = 0x9e37_79b9_7f4a_7c15;
        let mut one_worker_random = TapeRandom(seed);
        let one_worker = prove_vega_mdl_figure9_v1(
            &context(),
            &fixture.public,
            &fixture.witness(),
            VegaMdlProverConfigV1::new(1).expect("one worker"),
            &mut one_worker_random,
        )
        .expect("one-worker proof");
        let mut twenty_worker_random = TapeRandom(seed);
        let twenty_workers = prove_vega_mdl_figure9_v1(
            &context(),
            &fixture.public,
            &fixture.witness(),
            VegaMdlProverConfigV1::new(20).expect("twenty workers"),
            &mut twenty_worker_random,
        )
        .expect("twenty-worker proof");
        assert_eq!(one_worker, twenty_workers);
        assert_eq!(one_worker.len(), 181_375);
        verify_vega_mdl_figure9_v1(&context(), &fixture.public, &one_worker)
            .expect("one-worker canonical verification");

        let mut fresh_random = TapeRandom(seed ^ 0xa5a5_a5a5_a5a5_a5a5);
        let fresh = prove_vega_mdl_figure9_v1(
            &context(),
            &fixture.public,
            &fixture.witness(),
            VegaMdlProverConfigV1::new(20).expect("twenty workers"),
            &mut fresh_random,
        )
        .expect("fresh proof");
        assert_ne!(
            one_worker, fresh,
            "fresh full masks and row blindings must unlink repeated witnesses"
        );
        verify_vega_mdl_figure9_v1(&context(), &fixture.public, &fresh)
            .expect("fresh canonical verification");

        let baseline = decode_proof(&one_worker);
        let alternate = decode_proof(&fresh);

        // Every variable-length proof vector is exact: both deletion and
        // insertion are rejected before algebraic verification.
        let mut changed = baseline.clone();
        changed.mask_witness_commitment.points.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .mask_witness_commitment
            .points
            .push(baseline.mask_witness_commitment.points[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.mask_error_commitment.points.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .mask_error_commitment
            .points
            .push(baseline.mask_error_commitment.points[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.regular_witness_commitment.points.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .regular_witness_commitment
            .points
            .push(baseline.regular_witness_commitment.points[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.cross_term_commitment.points.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .cross_term_commitment
            .points
            .push(baseline.cross_term_commitment.points[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.outer_sumcheck_rounds.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .outer_sumcheck_rounds
            .push(baseline.outer_sumcheck_rounds[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.inner_sumcheck_rounds.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed
            .inner_sumcheck_rounds
            .push(baseline.inner_sumcheck_rounds[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.witness_opening.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.witness_opening.push(baseline.witness_opening[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut changed = baseline.clone();
        changed.error_opening.pop();
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.error_opening.push(baseline.error_opening[0]);
        assert_invalid_shape(&fixture.public, &changed);

        let mut trailing = one_worker.clone();
        trailing.push(0);
        assert_eq!(
            verify_vega_mdl_figure9_v1(&context(), &fixture.public, &trailing),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );
        assert_eq!(
            verify_vega_mdl_figure9_v1(
                &context(),
                &fixture.public,
                &vec![0; MAX_VEGA_PROOF_BYTES_V1 + 1]
            ),
            Err(VegaMdlProofErrorV1::ProofTooLarge {
                actual: MAX_VEGA_PROOF_BYTES_V1 + 1,
                max: MAX_VEGA_PROOF_BYTES_V1,
            })
        );

        let mut changed = baseline.clone();
        changed.version = PROOF_VERSION + 1;
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.mask_relaxation = VegaScalarWireV1::from_raw_bytes_for_test([0xff; 32]);
        assert_invalid_shape(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.mask_witness_commitment.points[0] =
            VegaPointWireV1::from_raw_bytes_for_test([0; 33]);
        assert_invalid_shape(&fixture.public, &changed);

        // Splicing independently masked top-level proof components must never
        // yield a valid proof for the same witness and statement.
        let mut changed = baseline.clone();
        changed.mask_witness_commitment = alternate.mask_witness_commitment.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.mask_error_commitment = alternate.mask_error_commitment.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.mask_relaxation = alternate.mask_relaxation;
        assert_verification_failure(&fixture.public, &changed);
        for index in 0..VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1 {
            let mut changed = baseline.clone();
            changed.mask_public_inputs[index] = alternate.mask_public_inputs[index];
            assert_verification_failure(&fixture.public, &changed);
        }
        let mut changed = baseline.clone();
        changed.regular_witness_commitment = alternate.regular_witness_commitment.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.cross_term_commitment = alternate.cross_term_commitment.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.outer_sumcheck_rounds = alternate.outer_sumcheck_rounds.clone();
        assert_verification_failure(&fixture.public, &changed);
        for index in 0..3 {
            let mut changed = baseline.clone();
            changed.outer_claims[index] = alternate.outer_claims[index];
            assert_verification_failure(&fixture.public, &changed);
        }
        let mut changed = baseline.clone();
        changed.inner_sumcheck_rounds = alternate.inner_sumcheck_rounds.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.witness_opening = alternate.witness_opening.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.witness_opening_blinding = alternate.witness_opening_blinding;
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.error_opening = alternate.error_opening.clone();
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.error_opening_blinding = alternate.error_opening_blinding;
        assert_verification_failure(&fixture.public, &changed);

        // Independently mutate one canonical scalar in every response class.
        let mut changed = baseline.clone();
        changed.outer_sumcheck_rounds[0][0] = increment_scalar(changed.outer_sumcheck_rounds[0][0]);
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.inner_sumcheck_rounds[0][0] = increment_scalar(changed.inner_sumcheck_rounds[0][0]);
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.witness_opening[0] = increment_scalar(changed.witness_opening[0]);
        assert_verification_failure(&fixture.public, &changed);
        let mut changed = baseline.clone();
        changed.error_opening[0] = increment_scalar(changed.error_opening[0]);
        assert_verification_failure(&fixture.public, &changed);

        // All consensus context fields and all fourteen statement scalars are
        // Fiat--Shamir bound and reject replay under mutation.
        let mut contexts = Vec::new();
        let mut changed_context = context();
        changed_context.chain_id = b"taira-vega-replay";
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.genesis_hash[0] ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.action_index ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.parameter_id[0] ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.parameter_digest[0] ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.verifier_digest[0] ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.statement_schema_digest[0] ^= 1;
        contexts.push(changed_context);
        let mut changed_context = context();
        changed_context.engine_manifest_digest[0] ^= 1;
        contexts.push(changed_context);
        for replay_context in contexts {
            assert_eq!(
                verify_vega_mdl_figure9_v1(&replay_context, &fixture.public, &one_worker),
                Err(VegaMdlProofErrorV1::VerificationFailed)
            );
        }
        for index in 0..VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1 {
            let mut changed_public = fixture.public;
            changed_public[index] += Scalar::one();
            assert_eq!(
                verify_vega_mdl_figure9_v1(&context(), &changed_public, &one_worker),
                Err(VegaMdlProofErrorV1::VerificationFailed),
                "public input {index}"
            );
        }
    }
}
