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
    MAX_VEGA_PROOF_BYTES_V1, VegaPointWireV1, VegaScalarWireV1, VegaT256ScalarV1 as Scalar,
    circuit::{CircuitError, MAX_CIRCUIT_ROWS},
    commitment::{
        COMMITMENT_WORKER_STACK_BYTES, Commitment, CommitmentError, CommitmentKey,
        MAX_COMMITMENT_WORKERS,
    },
    figure9::{VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaMdlFigure9WitnessV1, synthesize_figure9},
    figure9_layout::FIGURE9_LAYOUT,
    nifs::NovaNifs,
    r1cs::{Instance, RelaxedInstance, RelaxedWitness, Shape, Witness},
    spartan::RelaxedSpartanProof,
    sponge::keccak256,
    sumcheck::{CompressedUnivariate, SumcheckProof},
    transcript::VegaTranscriptV1,
    validate_proof_byte_cap_v1,
};

/// Exact external privacy protocol label absorbed before all proof material.
pub const VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1: &[u8] = b"vega-existing-credential-zk-v0";
/// Exact internal Microsoft transcript persona.
pub const VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1: &[u8] = b"neutronnova_prove";

const PROOF_VERSION: u8 = 1;
const COMMITMENT_KEY_COLUMNS: usize = 1024;
const COMMITMENT_KEY_LABEL: &[u8] = b"iroha.vega.figure9.hyrax-t256.v1";
const PROFILE_DESCRIPTOR: &[u8] = b"iroha.vega.figure9.mdl-age.v1";
const PINNED_SOURCE_COMMIT: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
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

/// Prove the complete Figure 9 relation and return exact canonical Norito.
///
/// The supplied random source is consumed directly; callers cannot supply or
/// reuse an already-built relaxed mask object. Every field element is sampled
/// by reducing an independent 64-byte wide string. A nonzero health sample and
/// nonzero commitment blindings reject an all-zero or stuck source.
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

    // A consumed nonzero health draw detects the most common catastrophic RNG
    // failure before allocating a full masking assignment.
    let _health = sample_nonzero_scalar(random)?;

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
    let proof = norito::codec::decode_exact_from_slice::<VegaMdlProofWireV1>(proof_bytes)
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

fn profile_frame(
    shape: &Shape,
    dimensions: VegaMdlProofDimensionsV1,
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
    fn random_source_failure_and_all_zero_tape_fail_closed() {
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
