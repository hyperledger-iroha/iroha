//! Native fixed-profile Bootle/Lantern P1/P2 prover and verifier.
//!
//! This module implements the blind-issuance-request (P1) and presentation
//! (P2) paths over their distinct transcript purposes and nominal wire types:
//! transparent commitments, projected norm witnesses, Schwartz compression,
//! the generic quadratic linearization, ABDLOP response compression, strict
//! proof construction, verifier-side challenge reconstruction, and prover
//! self-verification.

use rand_core_06::{CryptoRng, RngCore};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::{
    bounds::{ResponseBoundErrorV1, validate_public_response_bounds_v1},
    codec::{
        BootleLanternBlindIssuanceRequestProofV1, BootleLanternPresentationProofV1,
        H_POLYNOMIALS_V1, HINT_POLYNOMIALS_V1, PROOF_COEFFICIENTS_V1, ProofCodecErrorV1,
        T_A1_POLYNOMIALS_V1, T_B_POLYNOMIALS_V1, Z1_POLYNOMIALS_V1, Z3_POLYNOMIALS_V1,
        Z4_POLYNOMIALS_V1, Z21_POLYNOMIALS_V1,
    },
    compression::{
        CompressionErrorV1, gamma_decompose_v1, make_gamma_hint_v1, power2round_v1,
        use_gamma_hint_v1,
    },
    params::{
        APPLICATION_RELATION_QUOTIENT_BOUND_V1, APPLICATION_RING_DEGREE_V1, COMPRESSION_GAMMA_V1,
        DECOMPOSITION_BITS_V1, MAX_PROJECTION_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1,
        MAX_PROOF_SAMPLING_ATTEMPTS_V1, MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1,
        PROOF_INVERSE_TWO_V1, RESPONSE_NORM_SQUARED_BOUND_V1, TBOX_KMSIS_V1, TBOX_LEXT_V1,
        TBOX_M1_V1, TBOX_M2_V1, Z3_NORM_SQUARED_BOUND_V1, Z4_INFINITY_NORM_BOUND_V1,
    },
    relation::{BootleLanternApplicationRelationV1, BootleLanternPresentationWitnessV1},
    ring::ProofPolynomialV1,
    sampling::{BootleSamplingProfileV1, ProofRandomnessV1, SamplingErrorV1},
    toolbox::{
        COMBINED_QUADRATIC_EQUATIONS_V1, EVALUATION_CONSTRAINTS_V1, InternalMatricesV1,
        PROJECTION_COORDINATES_V1, PROJECTION_POLYNOMIALS_V1, QUADRATIC_MESSAGE_POLYNOMIALS_V1,
        QuadraticEquationV1, QuadraticVariablesV1, S21_POLYNOMIALS_V1, SCHWARTZ_ACCUMULATORS_V1,
        ToolboxErrorV1, application_quotient_v1, application_relation_digest_v1,
        boxed_polynomial_array_from_fn_v1, boxed_zero_polynomial_array_v1,
        commit_extended_messages_v1, encode_polynomials_v1, expand_projection_matrix_v1,
        flatten_polynomials, lift_short_witness_v1, matrix_vector_product_v1,
        projected_norm_witness_v1,
    },
    transcript::{
        BlindIssuanceRequestTranscriptV1, PresentationTranscriptV1, ProofTranscriptCoreV1,
    },
};

const PROJECTION_R_STAGE_V1: &[u8] = b"projection-r-v1";
const PROJECTION_R_PRIME_STAGE_V1: &[u8] = b"projection-r-prime-v1";
const SCHWARTZ_WEIGHT_STAGE_V1: &[u8] = b"schwartz-weights-v1";
const EQUATION_MULTIPLIER_STAGE_V1: &[u8] = b"quadratic-equation-multipliers-v1";

const Y3_MESSAGE_START_V1: usize = 0;
const Y4_MESSAGE_START_V1: usize = 4;
const BETA_MESSAGE_INDEX_V1: usize = 8;
const G_MESSAGE_START_V1: usize = 9;
const LINEARIZATION_MESSAGE_INDEX_V1: usize = 11;
const PROVER_PRECOMPUTED_QUADRATIC_EVALUATIONS_V1: usize = 2;
const PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1: usize = 3;
const VERIFIER_QUADRATIC_EVALUATIONS_V1: usize = 3;

#[cfg(test)]
const MAX_QUADRATIC_EVALUATIONS_PER_PROVE_ATTEMPT_V1: usize =
    PROVER_PRECOMPUTED_QUADRATIC_EVALUATIONS_V1
        + PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1
            * MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 as usize;
#[cfg(test)]
const MIN_RESPONSE_CONTEXTS_AT_GLOBAL_BUDGET_V1: usize = (MAX_PROOF_SAMPLING_ATTEMPTS_V1 as usize
    + MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 as usize)
    / (MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 as usize + 1);
// Spending all B shared draws on the evaluation-heavy path needs at least
// ceil(B / (R + 1)) contexts: one projection draw plus at most R response
// draws apiece.  Treating every draw as three evaluations and replacing each
// required projection draw by its two-evaluation context prelude gives the
// tight prover bound 3B-contexts; a successful proof adds three verifier
// evaluations.
#[cfg(test)]
const MAX_TOP_LEVEL_QUADRATIC_EVALUATIONS_V1: usize = 3 * MAX_PROOF_SAMPLING_ATTEMPTS_V1 as usize
    - MIN_RESPONSE_CONTEXTS_AT_GLOBAL_BUDGET_V1
    + VERIFIER_QUADRATIC_EVALUATIONS_V1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofRejectionStageV1 {
    Projection,
    ResponseMask,
}

#[derive(Default, PartialEq, Eq)]
struct ProofRejectionStatsV1 {
    projection: u32,
    response_sampling: u32,
    response_norm: u32,
}

impl core::fmt::Debug for ProofRejectionStatsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ProofRejectionStatsV1(<redacted>)")
    }
}

impl Zeroize for ProofRejectionStatsV1 {
    fn zeroize(&mut self) {
        self.projection.zeroize();
        self.response_sampling.zeroize();
        self.response_norm.zeroize();
    }
}

impl Drop for ProofRejectionStatsV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ProofRejectionStatsV1 {
    fn increment(counter: &mut u32) {
        *counter = counter
            .checked_add(1)
            .expect("a rejection count cannot exceed the shared u32 draw budget");
    }
}

struct ProofRejectionBudgetV1 {
    remaining: u32,
    projection_draws: u32,
    response_mask_draws: u32,
    rejections: ProofRejectionStatsV1,
}

impl core::fmt::Debug for ProofRejectionBudgetV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ProofRejectionBudgetV1(<redacted>)")
    }
}

impl Zeroize for ProofRejectionBudgetV1 {
    fn zeroize(&mut self) {
        self.remaining.zeroize();
        self.projection_draws.zeroize();
        self.response_mask_draws.zeroize();
        self.rejections.zeroize();
    }
}

impl Drop for ProofRejectionBudgetV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ProofRejectionBudgetV1 {
    const fn new(limit: u32) -> Self {
        Self {
            remaining: limit,
            projection_draws: 0,
            response_mask_draws: 0,
            rejections: ProofRejectionStatsV1 {
                projection: 0,
                response_sampling: 0,
                response_norm: 0,
            },
        }
    }

    fn reserve(&mut self, stage: ProofRejectionStageV1) -> bool {
        let Some(remaining) = self.remaining.checked_sub(1) else {
            return false;
        };
        self.remaining = remaining;
        let draws = match stage {
            ProofRejectionStageV1::Projection => &mut self.projection_draws,
            ProofRejectionStageV1::ResponseMask => &mut self.response_mask_draws,
        };
        *draws = draws
            .checked_add(1)
            .expect("draw count cannot exceed its u32 budget");
        true
    }

    const fn remaining(&self) -> u32 {
        self.remaining
    }

    const fn is_exhausted(&self) -> bool {
        self.remaining == 0
    }

    fn total_draws(&self) -> u32 {
        self.projection_draws
            .checked_add(self.response_mask_draws)
            .expect("stage draws cannot exceed their shared u32 budget")
    }

    fn record_projection_rejection(&mut self) {
        ProofRejectionStatsV1::increment(&mut self.rejections.projection);
    }

    fn record_response_sampling_rejection(&mut self) {
        ProofRejectionStatsV1::increment(&mut self.rejections.response_sampling);
    }

    fn record_response_norm_rejection(&mut self) {
        ProofRejectionStatsV1::increment(&mut self.rejections.response_norm);
    }

    fn exhaustion_error(&self) -> PresentationProofErrorV1 {
        PresentationProofErrorV1::SamplingBudgetExhausted
    }
}

struct SecretPolynomialVectorV1<const N: usize> {
    polynomials: Box<[ProofPolynomialV1; N]>,
}

impl<const N: usize> SecretPolynomialVectorV1<N> {
    fn zero() -> Self {
        Self {
            polynomials: boxed_zero_polynomial_array_v1(),
        }
    }

    fn from_polynomials(polynomials: Box<[ProofPolynomialV1; N]>) -> Self {
        Self { polynomials }
    }
}

impl<const N: usize> core::fmt::Debug for SecretPolynomialVectorV1<N> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("SecretPolynomialVectorV1(<redacted>)")
    }
}

impl<const N: usize> Drop for SecretPolynomialVectorV1<N> {
    fn drop(&mut self) {
        self.polynomials.as_mut().zeroize();
    }
}

struct ProjectionProofV1 {
    projection_r: Box<[i8]>,
    projection_r_prime: Box<[i8]>,
    z3: SecretPolynomialVectorV1<PROJECTION_POLYNOMIALS_V1>,
    z4: SecretPolynomialVectorV1<PROJECTION_POLYNOMIALS_V1>,
}

/// Prove one validated anonymous-credential presentation.
///
/// The transcript must carry `application_relation_digest_v1(relation)`.
/// Randomness is obtained once from the caller's fallible cryptographic RNG,
/// then every internal stream is independently domain-separated.
///
/// # Errors
///
/// Fails closed on a relation/transcript mismatch, invalid witness, random
/// source failure, bounded rejection exhaustion, arithmetic/compression
/// failure, non-canonical proof construction, or failed prover self-check.
///
/// # Timing boundary
///
/// This is a local prover API. Its bounded Gaussian and response rejection
/// samplers are intentionally variable-work. Deployments must not expose proof
/// completion timing to an untrusted remote observer or a hostile co-tenant;
/// use process isolation and a local authenticated invocation boundary.
pub fn prove_presentation_v1<R: CryptoRng + RngCore>(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
    transcript: PresentationTranscriptV1,
    rng: &mut R,
) -> Result<BootleLanternPresentationProofV1, PresentationProofErrorV1> {
    prove_with_transcript_core_v1(
        relation,
        witness,
        transcript.proof_core(),
        rng,
        MAX_PROOF_SAMPLING_ATTEMPTS_V1,
    )
}

pub(crate) fn prove_blind_issuance_request_v1<R: CryptoRng + RngCore>(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
    transcript: BlindIssuanceRequestTranscriptV1,
    rng: &mut R,
) -> Result<BootleLanternBlindIssuanceRequestProofV1, PresentationProofErrorV1> {
    let body = prove_with_transcript_core_v1(
        relation,
        witness,
        transcript.proof_core(),
        rng,
        MAX_PROOF_SAMPLING_ATTEMPTS_V1,
    )?;
    Ok(BootleLanternBlindIssuanceRequestProofV1::from_validated_body_v1(body))
}

fn prove_presentation_with_rejection_limit_v1<R: CryptoRng + RngCore>(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
    transcript: PresentationTranscriptV1,
    rng: &mut R,
    rejection_draw_limit: u32,
) -> Result<BootleLanternPresentationProofV1, PresentationProofErrorV1> {
    prove_with_transcript_core_v1(
        relation,
        witness,
        transcript.proof_core(),
        rng,
        rejection_draw_limit,
    )
}

fn prove_with_transcript_core_v1<R: CryptoRng + RngCore>(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
    transcript: ProofTranscriptCoreV1,
    rng: &mut R,
    rejection_draw_limit: u32,
) -> Result<BootleLanternPresentationProofV1, PresentationProofErrorV1> {
    if rejection_draw_limit == 0 || rejection_draw_limit > MAX_PROOF_SAMPLING_ATTEMPTS_V1 {
        return Err(PresentationProofErrorV1::InternalInvariant);
    }
    require_relation_digest(relation, transcript)?;
    let short =
        lift_short_witness_v1(relation, witness).map_err(PresentationProofErrorV1::Toolbox)?;
    let mut randomness =
        ProofRandomnessV1::from_rng(rng).map_err(PresentationProofErrorV1::Sampling)?;
    let matrices =
        InternalMatricesV1::expand(&transcript).map_err(PresentationProofErrorV1::Toolbox)?;

    let mut rejection_budget = ProofRejectionBudgetV1::new(rejection_draw_limit);
    while !rejection_budget.is_exhausted() {
        let draws_before = rejection_budget.total_draws();
        if let Some(proof) = prove_attempt(
            relation,
            &short,
            transcript,
            &matrices,
            &mut randomness,
            &mut rejection_budget,
        )? {
            if let Err(_error) = verify_with_transcript_core_v1(relation, transcript, &proof) {
                #[cfg(test)]
                eprintln!("Bootle/Lantern prover self-check detail: {_error:?}");
                return Err(PresentationProofErrorV1::ProverSelfCheckFailed);
            }
            return Ok(proof);
        }
        if rejection_budget.total_draws() <= draws_before {
            return Err(PresentationProofErrorV1::InternalInvariant);
        }
    }
    Err(rejection_budget.exhaustion_error())
}

/// Verify one strictly decoded presentation proof.
///
/// # Errors
///
/// Fails on any transcript/relation mismatch, public response bound,
/// reconciliation equation, Schwartz commitment shape, or Fiat--Shamir
/// challenge mismatch.
pub fn verify_presentation_v1(
    relation: &BootleLanternApplicationRelationV1,
    transcript: PresentationTranscriptV1,
    proof: &BootleLanternPresentationProofV1,
) -> Result<(), PresentationProofErrorV1> {
    verify_with_transcript_core_v1(relation, transcript.proof_core(), proof)
}

pub(crate) fn verify_blind_issuance_request_v1(
    relation: &BootleLanternApplicationRelationV1,
    transcript: BlindIssuanceRequestTranscriptV1,
    proof: &BootleLanternBlindIssuanceRequestProofV1,
) -> Result<(), PresentationProofErrorV1> {
    verify_with_transcript_core_v1(relation, transcript.proof_core(), proof.validated_body_v1())
}

fn verify_with_transcript_core_v1(
    relation: &BootleLanternApplicationRelationV1,
    transcript: ProofTranscriptCoreV1,
    proof: &BootleLanternPresentationProofV1,
) -> Result<(), PresentationProofErrorV1> {
    require_relation_digest(relation, transcript)?;
    validate_public_response_bounds_v1(proof).map_err(PresentationProofErrorV1::ResponseBound)?;

    let t_b = proof_polynomial_array::<T_B_POLYNOMIALS_V1>(|index| proof.t_b_polynomial(index))?;
    let h = proof_polynomial_array::<H_POLYNOMIALS_V1>(|index| proof.h_polynomial(index))?;
    let t_a1 = proof_polynomial_array::<T_A1_POLYNOMIALS_V1>(|index| proof.t_a1_polynomial(index))?;
    let hint = proof_polynomial_array::<HINT_POLYNOMIALS_V1>(|index| proof.hint_polynomial(index))?;
    let z1 = proof_polynomial_array::<Z1_POLYNOMIALS_V1>(|index| proof.z1_polynomial(index))?;
    let z21 = proof_polynomial_array::<Z21_POLYNOMIALS_V1>(|index| proof.z21_polynomial(index))?;
    let z3 = proof_polynomial_array::<Z3_POLYNOMIALS_V1>(|index| proof.z3_polynomial(index))?;
    let z4 = proof_polynomial_array::<Z4_POLYNOMIALS_V1>(|index| proof.z4_polynomial(index))?;
    let challenge = proof.challenge_polynomial();

    require_schwartz_commitment_shape(&h)?;
    let matrices =
        InternalMatricesV1::expand(&transcript).map_err(PresentationProofErrorV1::Toolbox)?;
    let (projection_r, projection_r_prime) = derive_projection_matrices(transcript, &t_b)?;
    let weights = derive_schwartz_weights(transcript, &t_b)?;
    let multipliers = derive_equation_multipliers(transcript, &t_b, &h, &z3, &z4)?;
    let equation = QuadraticEquationV1::new(
        relation,
        projection_r,
        projection_r_prime,
        z3.clone(),
        z4.clone(),
        h.clone(),
        weights,
        multipliers,
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;

    let recovered_w1 = recover_gamma_high(&matrices, &z1, &z21, &t_a1, challenge, &hint)?;
    validate_compressed_response_bound(&matrices, &z1, &z21, &t_a1, challenge, &recovered_w1)?;

    let b_z21 = matrix_vector_product_v1(&matrices.b_prime, z21.as_ref())
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let variables = QuadraticVariablesV1 {
        short: z1,
        message: boxed_polynomial_array_from_fn_v1(|index| {
            challenge.multiply(t_b[index]).sub(b_z21[index])
        }),
    };
    let f = challenge
        .multiply(t_b[LINEARIZATION_MESSAGE_INDEX_V1])
        .sub(b_z21[LINEARIZATION_MESSAGE_INDEX_V1]);
    // A quadratic map is completely split into its constant, linear, and
    // homogeneous-quadratic parts by Q(0), Q(z), and Q(-z).  Keep those
    // three expensive black-box evaluations shared by both reconstructions.
    let [q0, q_z, q_negative_z]: [ProofPolynomialV1; VERIFIER_QUADRATIC_EVALUATIONS_V1] = [
        equation
            .evaluate(&QuadraticVariablesV1::zero())
            .map_err(PresentationProofErrorV1::Toolbox)?,
        equation
            .evaluate(&variables)
            .map_err(PresentationProofErrorV1::Toolbox)?,
        equation
            .evaluate(&variables.negate())
            .map_err(PresentationProofErrorV1::Toolbox)?,
    ];
    let (linear_z, q2_z) = decompose_signed_quadratic_evaluations(q0, q_z, q_negative_z)?;
    let recovered_v = q2_z
        .add(challenge.multiply(linear_z))
        .add(challenge.multiply(challenge).multiply(q0))
        .sub(f);

    let pre_challenge = pre_challenge_wire(&t_b, &h, &t_a1, &z3, &z4, &recovered_w1, recovered_v)?;
    let expected = transcript
        .derive_final_challenge(&pre_challenge)
        .map_err(|error| PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error)))?;
    if expected != challenge {
        return Err(PresentationProofErrorV1::ChallengeMismatch);
    }
    Ok(())
}

fn prove_attempt(
    relation: &BootleLanternApplicationRelationV1,
    short: &super::toolbox::ShortWitnessV1,
    transcript: ProofTranscriptCoreV1,
    matrices: &InternalMatricesV1,
    randomness: &mut ProofRandomnessV1,
    rejection_budget: &mut ProofRejectionBudgetV1,
) -> Result<Option<BootleLanternPresentationProofV1>, PresentationProofErrorV1> {
    let s2 = sample_ternary_vector::<TBOX_M2_V1>(randomness, b"s2")?;
    let (t_a1, t_a2) = commit_short_witness(matrices, short.polynomials(), &s2.polynomials)?;
    let mut messages = SecretPolynomialVectorV1::<TBOX_LEXT_V1>::zero();

    let projection = match prove_projected_responses(
        relation,
        short.polynomials(),
        transcript,
        matrices,
        &s2.polynomials,
        &mut messages.polynomials,
        randomness,
        rejection_budget,
    )? {
        Some(projection) => projection,
        None => return Ok(None),
    };

    messages.polynomials[G_MESSAGE_START_V1] = sample_uniform_g(randomness, b"schwartz-g0")?;
    messages.polynomials[G_MESSAGE_START_V1 + 1] = sample_uniform_g(randomness, b"schwartz-g1")?;
    let mut t_b = Zeroizing::new(
        commit_extended_messages_v1(
            &matrices.b_prime,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&s2.polynomials),
            &messages.polynomials,
        )
        .map_err(PresentationProofErrorV1::Toolbox)?,
    );

    let weights = derive_schwartz_weights(transcript, &*t_b)?;
    let variables = QuadraticVariablesV1 {
        short: short
            .polynomials()
            .to_vec()
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("short witness shape is fixed")),
        message: boxed_polynomial_array_from_fn_v1(|index| messages.polynomials[index]),
    };
    let z3 = projection.z3;
    let z4 = projection.z4;
    let mut equation = QuadraticEquationV1::new(
        relation,
        projection.projection_r,
        projection.projection_r_prime,
        z3.polynomials.clone(),
        z4.polynomials.clone(),
        boxed_zero_polynomial_array_v1(),
        weights,
        boxed_zero_polynomial_array_v1(),
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    let schwartz = Zeroizing::new(
        equation
            .schwartz_polynomials(&variables)
            .map_err(PresentationProofErrorV1::Toolbox)?,
    );
    let h = boxed_polynomial_array_from_fn_v1(|index| {
        messages.polynomials[G_MESSAGE_START_V1 + index].add(schwartz[index])
    });
    require_schwartz_commitment_shape(&h)?;
    let multipliers =
        derive_equation_multipliers(transcript, &*t_b, &h, &z3.polynomials, &z4.polynomials)?;
    equation.bind_final_equations(h.clone(), multipliers);
    // These two values are invariant across every response-mask retry.  In
    // particular, checking Q(secret) here preserves the witness soundness
    // check while avoiding its former repetition inside the retry loop.
    let q0 = equation
        .evaluate(&QuadraticVariablesV1::zero())
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let q_secret = Zeroizing::new(
        equation
            .evaluate(&variables)
            .map_err(PresentationProofErrorV1::Toolbox)?,
    );
    if !q_secret.is_zero() {
        return Err(PresentationProofErrorV1::ConstraintSystemRejectedWitness);
    }
    // B' * s21 is independent of the response mask as well.  Retain only
    // the coordinate used by the linearization and zeroize the full product.
    let b_s21 = matrix_vector_product_v1(
        &matrices.b_prime,
        array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&s2.polynomials),
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    let b_s21_linearization = Zeroizing::new(b_s21[LINEARIZATION_MESSAGE_INDEX_V1]);

    for _ in 0..MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 {
        if !rejection_budget.reserve(ProofRejectionStageV1::ResponseMask) {
            return Ok(None);
        }
        let y1 =
            sample_gaussian_vector::<TBOX_M1_V1>(randomness, BootleSamplingProfileV1::ResponseZ1)?;
        let y2 =
            sample_gaussian_vector::<TBOX_M2_V1>(randomness, BootleSamplingProfileV1::ResponseZ2)?;
        let (t_candidate, v) = quadratic_linearization(
            &equation,
            &variables,
            q0,
            *q_secret,
            matrices,
            *b_s21_linearization,
            &y1.polynomials,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&y2.polynomials),
        )?;
        let t_candidate = Zeroizing::new(t_candidate);
        t_b[LINEARIZATION_MESSAGE_INDEX_V1] = *t_candidate;

        let (w1, w0) = decompose_mask_commitment(
            matrices,
            &y1.polynomials,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&y2.polynomials),
            array_suffix::<TBOX_KMSIS_V1, TBOX_M2_V1>(&y2.polynomials),
        )?;
        let pre_challenge =
            pre_challenge_wire(&*t_b, &h, &t_a1, &z3.polynomials, &z4.polynomials, &w1, v)?;
        let challenge = transcript
            .derive_final_challenge(&pre_challenge)
            .map_err(|error| {
                PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error))
            })?;

        let c_short = SecretPolynomialVectorV1::from_polynomials(multiply_vector_by_polynomial(
            short.polynomials(),
            challenge,
        ));
        let c_s2 = SecretPolynomialVectorV1::from_polynomials(multiply_vector_by_polynomial(
            &s2.polynomials,
            challenge,
        ));
        let z1 = SecretPolynomialVectorV1::from_polynomials(add_arrays(
            &y1.polynomials,
            &c_short.polynomials,
        ));
        let mut z2 = SecretPolynomialVectorV1::from_polynomials(add_arrays(
            &y2.polynomials,
            &c_s2.polynomials,
        ));
        let z1_centered = Zeroizing::new(centered_vector(z1.polynomials.as_ref()));
        let c_short_centered = Zeroizing::new(centered_vector(c_short.polynomials.as_ref()));
        let z2_centered = Zeroizing::new(centered_vector(z2.polynomials.as_ref()));
        let c_s2_centered = Zeroizing::new(centered_vector(c_s2.polynomials.as_ref()));
        let accept_z1 = randomness
            .accept_rejection(
                z1_centered.as_ref(),
                c_short_centered.as_ref(),
                BootleSamplingProfileV1::ResponseZ1,
            )
            .map_err(PresentationProofErrorV1::Sampling)?;
        let accept_z2 = randomness
            .accept_rejection(
                z2_centered.as_ref(),
                c_s2_centered.as_ref(),
                BootleSamplingProfileV1::ResponseZ2,
            )
            .map_err(PresentationProofErrorV1::Sampling)?;
        if !accept_z1 || !accept_z2 {
            rejection_budget.record_response_sampling_rejection();
            continue;
        }

        let c_t_a2 = SecretPolynomialVectorV1::from_polynomials(multiply_vector_by_polynomial(
            &t_a2.polynomials,
            challenge,
        ));
        for index in 0..TBOX_KMSIS_V1 {
            let response_index = S21_POLYNOMIALS_V1 + index;
            z2.polynomials[response_index] = z2.polynomials[response_index]
                .sub(c_t_a2.polynomials[index])
                .sub(w0.polynomials[index]);
        }
        if centered_squared_norm(z2.polynomials.as_ref())?
            > u128::from(RESPONSE_NORM_SQUARED_BOUND_V1)
        {
            rejection_budget.record_response_norm_rejection();
            continue;
        }
        let hint = make_hint(
            &w1,
            array_suffix::<TBOX_KMSIS_V1, TBOX_M2_V1>(&z2.polynomials),
        )?;

        let proof = construct_proof(
            &*t_b,
            &h,
            &t_a1,
            challenge,
            &hint,
            &z1.polynomials,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&z2.polynomials),
            &z3.polynomials,
            &z4.polynomials,
        )?;
        validate_public_response_bounds_v1(&proof)
            .map_err(PresentationProofErrorV1::ResponseBound)?;
        return Ok(Some(proof));
    }
    Ok(None)
}

fn prove_projected_responses(
    relation: &BootleLanternApplicationRelationV1,
    short: &[ProofPolynomialV1; TBOX_M1_V1],
    transcript: ProofTranscriptCoreV1,
    matrices: &InternalMatricesV1,
    s2: &[ProofPolynomialV1; TBOX_M2_V1],
    messages: &mut [ProofPolynomialV1; TBOX_LEXT_V1],
    randomness: &mut ProofRandomnessV1,
    rejection_budget: &mut ProofRejectionBudgetV1,
) -> Result<Option<ProjectionProofV1>, PresentationProofErrorV1> {
    let s3 = Zeroizing::new(projected_norm_witness_v1(short));
    let s4 = Zeroizing::new(
        application_quotient_v1(relation, short).map_err(PresentationProofErrorV1::Toolbox)?,
    );
    if s4.iter().any(|polynomial| {
        polynomial
            .coefficients()
            .iter()
            .enumerate()
            .any(|(index, _)| {
                polynomial.centered_coefficient(index).unsigned_abs()
                    > APPLICATION_RELATION_QUOTIENT_BOUND_V1
            })
    }) {
        return Err(PresentationProofErrorV1::ApplicationQuotientBoundExceeded);
    }
    let s3_coefficients = Zeroizing::new(centered_vector(s3.as_ref()));
    let s4_coefficients = Zeroizing::new(centered_vector(s4.as_ref()));

    for _ in 0..MAX_PROJECTION_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 {
        if !rejection_budget.reserve(ProofRejectionStageV1::Projection) {
            return Ok(None);
        }
        let y3 = sample_gaussian_vector::<PROJECTION_POLYNOMIALS_V1>(
            randomness,
            BootleSamplingProfileV1::ProjectionZ3,
        )?;
        let y4 = sample_gaussian_vector::<PROJECTION_POLYNOMIALS_V1>(
            randomness,
            BootleSamplingProfileV1::ProjectionZ4,
        )?;
        let beta3 = randomness.sign(b"z3-sign");
        let beta4 = randomness.sign(b"z4-sign");
        messages[Y3_MESSAGE_START_V1..Y3_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1]
            .copy_from_slice(y3.polynomials.as_ref());
        messages[Y4_MESSAGE_START_V1..Y4_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1]
            .copy_from_slice(y4.polynomials.as_ref());
        let mut beta_coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        beta_coefficients[0] = beta3;
        beta_coefficients[APPLICATION_RING_DEGREE_V1 / 2] = beta4;
        messages[BETA_MESSAGE_INDEX_V1] =
            ProofPolynomialV1::from_centered_coefficients(beta_coefficients);

        let t_b = commit_extended_messages_v1(
            &matrices.b_prime,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(s2),
            messages,
        )
        .map_err(PresentationProofErrorV1::Toolbox)?;
        let (projection_r, projection_r_prime) = derive_projection_matrices(transcript, &t_b)?;
        let projected_s3 = project_centered(
            &projection_r,
            s3_coefficients.len(),
            s3_coefficients.as_ref(),
        )?;
        let projected_s4 = project_centered(
            &projection_r_prime,
            s4_coefficients.len(),
            s4_coefficients.as_ref(),
        )?;
        let mut z3_centered = Zeroizing::new(centered_vector(y3.polynomials.as_ref()));
        let mut z4_centered = Zeroizing::new(centered_vector(y4.polynomials.as_ref()));
        for index in 0..PROJECTION_COORDINATES_V1 {
            z3_centered[index] = z3_centered[index]
                .checked_add(
                    beta3
                        .checked_mul(projected_s3[index])
                        .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?;
            z4_centered[index] = z4_centered[index]
                .checked_add(
                    beta4
                        .checked_mul(projected_s4[index])
                        .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?;
        }
        let z3 = SecretPolynomialVectorV1::from_polynomials(polynomials_from_centered_projection(
            z3_centered.as_ref(),
        ));
        let z4 = SecretPolynomialVectorV1::from_polynomials(polynomials_from_centered_projection(
            z4_centered.as_ref(),
        ));
        let accept_z3 = randomness
            .accept_rejection(
                z3_centered.as_ref(),
                projected_s3.as_ref(),
                BootleSamplingProfileV1::ProjectionZ3,
            )
            .map_err(PresentationProofErrorV1::Sampling)?;
        let accept_z4 = randomness
            .accept_rejection(
                z4_centered.as_ref(),
                projected_s4.as_ref(),
                BootleSamplingProfileV1::ProjectionZ4,
            )
            .map_err(PresentationProofErrorV1::Sampling)?;
        let z3_norm = centered_squared_norm(z3.polynomials.as_ref())?;
        let z4_infinity = z4
            .polynomials
            .iter()
            .flat_map(ProofPolynomialV1::coefficients)
            .enumerate()
            .map(|(index, residue)| {
                let polynomial = index / APPLICATION_RING_DEGREE_V1;
                let coefficient = index % APPLICATION_RING_DEGREE_V1;
                let _ = residue;
                z4.polynomials[polynomial]
                    .centered_coefficient(coefficient)
                    .unsigned_abs()
            })
            .max()
            .unwrap_or(0);
        if accept_z3
            && accept_z4
            && z3_norm <= u128::from(Z3_NORM_SQUARED_BOUND_V1)
            && z4_infinity <= Z4_INFINITY_NORM_BOUND_V1
        {
            return Ok(Some(ProjectionProofV1 {
                projection_r,
                projection_r_prime,
                z3,
                z4,
            }));
        }
        rejection_budget.record_projection_rejection();
    }
    Ok(None)
}

fn derive_projection_matrices(
    transcript: ProofTranscriptCoreV1,
    t_b: &[ProofPolynomialV1; TBOX_LEXT_V1],
) -> Result<(Box<[i8]>, Box<[i8]>), PresentationProofErrorV1> {
    let projection_commitments = encode_polynomials_v1(&t_b[..BETA_MESSAGE_INDEX_V1 + 1]);
    let components = [projection_commitments.as_slice()];
    let projection_r = expand_projection_matrix_v1(
        &transcript,
        PROJECTION_R_STAGE_V1,
        &components,
        50 * APPLICATION_RING_DEGREE_V1,
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    let projection_r_prime = expand_projection_matrix_v1(
        &transcript,
        PROJECTION_R_PRIME_STAGE_V1,
        &components,
        8 * APPLICATION_RING_DEGREE_V1,
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    Ok((projection_r, projection_r_prime))
}

fn derive_schwartz_weights(
    transcript: ProofTranscriptCoreV1,
    t_b: &[ProofPolynomialV1; TBOX_LEXT_V1],
) -> Result<Box<[u64]>, PresentationProofErrorV1> {
    let commitment = encode_polynomials_v1(&t_b[..QUADRATIC_MESSAGE_POLYNOMIALS_V1]);
    transcript
        .derive_uniform_scalars(
            SCHWARTZ_WEIGHT_STAGE_V1,
            &[commitment.as_slice()],
            SCHWARTZ_ACCUMULATORS_V1 * EVALUATION_CONSTRAINTS_V1,
        )
        .map(Vec::into_boxed_slice)
        .map_err(|error| PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error)))
}

fn derive_equation_multipliers(
    transcript: ProofTranscriptCoreV1,
    t_b: &[ProofPolynomialV1; TBOX_LEXT_V1],
    h: &[ProofPolynomialV1; H_POLYNOMIALS_V1],
    z3: &[ProofPolynomialV1; Z3_POLYNOMIALS_V1],
    z4: &[ProofPolynomialV1; Z4_POLYNOMIALS_V1],
) -> Result<Box<[ProofPolynomialV1; COMBINED_QUADRATIC_EQUATIONS_V1]>, PresentationProofErrorV1> {
    let t_b_wire = encode_polynomials_v1(&t_b[..QUADRATIC_MESSAGE_POLYNOMIALS_V1]);
    let h_wire = encode_polynomials_v1(h);
    let z3_wire = encode_polynomials_v1(z3);
    let z4_wire = encode_polynomials_v1(z4);
    let multipliers = transcript
        .derive_uniform_polynomials(
            EQUATION_MULTIPLIER_STAGE_V1,
            &[
                t_b_wire.as_slice(),
                h_wire.as_slice(),
                z3_wire.as_slice(),
                z4_wire.as_slice(),
            ],
            COMBINED_QUADRATIC_EQUATIONS_V1,
        )
        .map_err(|error| PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error)))?;
    multipliers
        .into_boxed_slice()
        .try_into()
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn commit_short_witness(
    matrices: &InternalMatricesV1,
    short: &[ProofPolynomialV1; TBOX_M1_V1],
    s2: &[ProofPolynomialV1; TBOX_M2_V1],
) -> Result<
    (
        Box<[ProofPolynomialV1; T_A1_POLYNOMIALS_V1]>,
        SecretPolynomialVectorV1<T_A1_POLYNOMIALS_V1>,
    ),
    PresentationProofErrorV1,
> {
    let a1_short =
        matrix_vector_product_v1(&matrices.a1, short).map_err(PresentationProofErrorV1::Toolbox)?;
    let a2_s21 = matrix_vector_product_v1(
        &matrices.a2_prime,
        array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(s2),
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    let s22 = array_suffix::<TBOX_KMSIS_V1, TBOX_M2_V1>(s2);
    let mut high = boxed_zero_polynomial_array_v1();
    let mut low = SecretPolynomialVectorV1::zero();
    for row in 0..T_A1_POLYNOMIALS_V1 {
        let commitment = Zeroizing::new(a1_short[row].add(a2_s21[row]).add(s22[row]));
        let mut high_coefficients = Zeroizing::new([0_u64; APPLICATION_RING_DEGREE_V1]);
        let mut low_coefficients = Zeroizing::new([0_i64; APPLICATION_RING_DEGREE_V1]);
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let rounded = power2round_v1(commitment.coefficients()[index])
                .map_err(PresentationProofErrorV1::Compression)?;
            high_coefficients[index] = rounded.high;
            low_coefficients[index] = rounded.low;
        }
        high[row] = ProofPolynomialV1::new(*high_coefficients)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        low.polynomials[row] = ProofPolynomialV1::from_centered_coefficients(*low_coefficients);
    }
    Ok((high, low))
}

fn quadratic_linearization(
    equation: &QuadraticEquationV1<'_>,
    secret: &QuadraticVariablesV1,
    q0: ProofPolynomialV1,
    q_secret: ProofPolynomialV1,
    matrices: &InternalMatricesV1,
    b_s21_linearization: ProofPolynomialV1,
    y1: &[ProofPolynomialV1; TBOX_M1_V1],
    y21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
) -> Result<(ProofPolynomialV1, ProofPolynomialV1), PresentationProofErrorV1> {
    let b_y21 = matrix_vector_product_v1(&matrices.b_prime, y21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let mask = QuadraticVariablesV1 {
        short: y1
            .to_vec()
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("response mask shape is fixed")),
        message: boxed_polynomial_array_from_fn_v1(|index| b_y21[index].negate()),
    };
    let (bilinear, linear_mask, quadratic_mask) =
        decompose_mask_quadratic_evaluations(secret, &mask, q0, q_secret, |variables| {
            equation
                .evaluate(variables)
                .map_err(PresentationProofErrorV1::Toolbox)
        })?;
    let g1 = bilinear.add(linear_mask);
    let t = b_s21_linearization.add(g1);
    let v = quadratic_mask.add(b_y21[LINEARIZATION_MESSAGE_INDEX_V1]);
    Ok((t, v))
}

fn decompose_signed_quadratic_evaluations(
    q0: ProofPolynomialV1,
    q_positive: ProofPolynomialV1,
    q_negative: ProofPolynomialV1,
) -> Result<(ProofPolynomialV1, ProofPolynomialV1), PresentationProofErrorV1> {
    let linear = q_positive
        .sub(q_negative)
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
    let quadratic = q_positive
        .add(q_negative)
        .sub(q0.scale_centered(2))
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
    Ok((linear, quadratic))
}

fn decompose_mask_quadratic_evaluations(
    secret: &QuadraticVariablesV1,
    mask: &QuadraticVariablesV1,
    q0: ProofPolynomialV1,
    q_secret: ProofPolynomialV1,
    mut evaluate: impl FnMut(
        &QuadraticVariablesV1,
    ) -> Result<ProofPolynomialV1, PresentationProofErrorV1>,
) -> Result<(ProofPolynomialV1, ProofPolynomialV1, ProofPolynomialV1), PresentationProofErrorV1> {
    // Polarization needs exactly Q(mask), Q(-mask), and Q(secret + mask).
    // Q(0) and Q(secret) are supplied by the invariant prelude.
    let q_mask = Zeroizing::new(evaluate(mask)?);
    let q_negative_mask = Zeroizing::new(evaluate(&mask.negate())?);
    let q_sum = Zeroizing::new(evaluate(&secret.add(mask))?);
    let bilinear = (*q_sum).sub(q_secret).sub(*q_mask).add(q0);
    let (linear, quadratic) =
        decompose_signed_quadratic_evaluations(q0, *q_mask, *q_negative_mask)?;
    Ok((bilinear, linear, quadratic))
}

fn decompose_mask_commitment(
    matrices: &InternalMatricesV1,
    y1: &[ProofPolynomialV1; TBOX_M1_V1],
    y21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
    y22: &[ProofPolynomialV1; TBOX_KMSIS_V1],
) -> Result<
    (
        [ProofPolynomialV1; TBOX_KMSIS_V1],
        SecretPolynomialVectorV1<TBOX_KMSIS_V1>,
    ),
    PresentationProofErrorV1,
> {
    let a1_y1 =
        matrix_vector_product_v1(&matrices.a1, y1).map_err(PresentationProofErrorV1::Toolbox)?;
    let a2_y21 = matrix_vector_product_v1(&matrices.a2_prime, y21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let mut high = [ProofPolynomialV1::ZERO; TBOX_KMSIS_V1];
    let mut low = SecretPolynomialVectorV1::zero();
    for row in 0..TBOX_KMSIS_V1 {
        let commitment = Zeroizing::new(a1_y1[row].add(a2_y21[row]).add(y22[row]));
        let mut high_coefficients = Zeroizing::new([0_u64; APPLICATION_RING_DEGREE_V1]);
        let mut low_coefficients = Zeroizing::new([0_i64; APPLICATION_RING_DEGREE_V1]);
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let decomposition = gamma_decompose_v1(commitment.coefficients()[index])
                .map_err(PresentationProofErrorV1::Compression)?;
            high_coefficients[index] = decomposition.high;
            low_coefficients[index] = decomposition.low;
        }
        high[row] = ProofPolynomialV1::new(*high_coefficients)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        low.polynomials[row] = ProofPolynomialV1::from_centered_coefficients(*low_coefficients);
    }
    Ok((high, low))
}

fn make_hint(
    w1: &[ProofPolynomialV1; TBOX_KMSIS_V1],
    adjusted_z22: &[ProofPolynomialV1; TBOX_KMSIS_V1],
) -> Result<[ProofPolynomialV1; HINT_POLYNOMIALS_V1], PresentationProofErrorV1> {
    // LNP22 Figure 18 applies MakeGHint to the complete centered z2,2
    // response.  Its low part is not a gamma-decomposition remainder and
    // therefore must not be truncated or rejected at +/-gamma/2; only the
    // resulting centered hint is bounded modulo m.
    let mut hints = Zeroizing::new([ProofPolynomialV1::ZERO; HINT_POLYNOMIALS_V1]);
    for row in 0..TBOX_KMSIS_V1 {
        let gamma_high = w1[row]
            .scale_canonical(COMPRESSION_GAMMA_V1)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        let base = Zeroizing::new(gamma_high.sub(adjusted_z22[row]));
        let mut coefficients = Zeroizing::new([0_i64; APPLICATION_RING_DEGREE_V1]);
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let correction = Zeroizing::new(adjusted_z22[row].centered_coefficient(index));
            let hint = make_gamma_hint_v1(base.coefficients()[index], *correction)
                .map_err(PresentationProofErrorV1::Compression)?;
            let recovered = use_gamma_hint_v1(base.coefficients()[index], hint)
                .map_err(PresentationProofErrorV1::Compression)?;
            if recovered != w1[row].coefficients()[index] {
                return Err(PresentationProofErrorV1::InternalInvariant);
            }
            coefficients[index] = hint;
        }
        hints[row] = ProofPolynomialV1::from_centered_coefficients(*coefficients);
    }
    Ok(*hints)
}

fn recover_gamma_high(
    matrices: &InternalMatricesV1,
    z1: &[ProofPolynomialV1; Z1_POLYNOMIALS_V1],
    z21: &[ProofPolynomialV1; Z21_POLYNOMIALS_V1],
    t_a1: &[ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
    challenge: ProofPolynomialV1,
    hint: &[ProofPolynomialV1; HINT_POLYNOMIALS_V1],
) -> Result<[ProofPolynomialV1; TBOX_KMSIS_V1], PresentationProofErrorV1> {
    let tmp1 = compressed_response_residue(matrices, z1, z21, t_a1, challenge)?;
    let mut recovered = [ProofPolynomialV1::ZERO; TBOX_KMSIS_V1];
    for row in 0..TBOX_KMSIS_V1 {
        let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let centered_hint = hint[row].centered_coefficient(index);
            coefficients[index] = use_gamma_hint_v1(tmp1[row].coefficients()[index], centered_hint)
                .map_err(PresentationProofErrorV1::Compression)?;
        }
        recovered[row] = ProofPolynomialV1::new(coefficients)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
    }
    Ok(recovered)
}

fn compressed_response_residue(
    matrices: &InternalMatricesV1,
    z1: &[ProofPolynomialV1; Z1_POLYNOMIALS_V1],
    z21: &[ProofPolynomialV1; Z21_POLYNOMIALS_V1],
    t_a1: &[ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
    challenge: ProofPolynomialV1,
) -> Result<[ProofPolynomialV1; TBOX_KMSIS_V1], PresentationProofErrorV1> {
    let a1_z1 =
        matrix_vector_product_v1(&matrices.a1, z1).map_err(PresentationProofErrorV1::Toolbox)?;
    let a2_z21 = matrix_vector_product_v1(&matrices.a2_prime, z21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let shifted_challenge = challenge.scale_centered(1_i64 << DECOMPOSITION_BITS_V1);
    Ok(core::array::from_fn(|index| {
        a1_z1[index]
            .add(a2_z21[index])
            .sub(shifted_challenge.multiply(t_a1[index]))
    }))
}

fn validate_compressed_response_bound(
    matrices: &InternalMatricesV1,
    z1: &[ProofPolynomialV1; Z1_POLYNOMIALS_V1],
    z21: &[ProofPolynomialV1; Z21_POLYNOMIALS_V1],
    t_a1: &[ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
    challenge: ProofPolynomialV1,
    recovered_w1: &[ProofPolynomialV1; TBOX_KMSIS_V1],
) -> Result<(), PresentationProofErrorV1> {
    let tmp1 = compressed_response_residue(matrices, z1, z21, t_a1, challenge)?;
    let mut norm = centered_squared_norm(z21)?;
    for row in 0..TBOX_KMSIS_V1 {
        let residual = tmp1[row].sub(
            recovered_w1[row]
                .scale_canonical(COMPRESSION_GAMMA_V1)
                .map_err(|_| PresentationProofErrorV1::InternalInvariant)?,
        );
        norm = norm
            .checked_add(residual.centered_squared_norm())
            .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?;
    }
    if norm > u128::from(RESPONSE_NORM_SQUARED_BOUND_V1) {
        return Err(PresentationProofErrorV1::CompressedResponseBoundExceeded);
    }
    Ok(())
}

fn construct_proof(
    t_b: &[ProofPolynomialV1; T_B_POLYNOMIALS_V1],
    h: &[ProofPolynomialV1; H_POLYNOMIALS_V1],
    t_a1: &[ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
    challenge: ProofPolynomialV1,
    hint: &[ProofPolynomialV1; HINT_POLYNOMIALS_V1],
    z1: &[ProofPolynomialV1; Z1_POLYNOMIALS_V1],
    z21: &[ProofPolynomialV1; Z21_POLYNOMIALS_V1],
    z3: &[ProofPolynomialV1; Z3_POLYNOMIALS_V1],
    z4: &[ProofPolynomialV1; Z4_POLYNOMIALS_V1],
) -> Result<BootleLanternPresentationProofV1, PresentationProofErrorV1> {
    let mut coefficients = Vec::with_capacity(PROOF_COEFFICIENTS_V1);
    for section in [
        t_b.as_slice(),
        h.as_slice(),
        t_a1.as_slice(),
        core::slice::from_ref(&challenge),
        hint.as_slice(),
        z1.as_slice(),
        z21.as_slice(),
        z3.as_slice(),
        z4.as_slice(),
    ] {
        coefficients.extend(flatten_polynomials(section));
    }
    if coefficients.len() != PROOF_COEFFICIENTS_V1 {
        return Err(PresentationProofErrorV1::InternalInvariant);
    }
    BootleLanternPresentationProofV1::from_coefficients(coefficients.into_boxed_slice())
        .map_err(PresentationProofErrorV1::Codec)
}

fn pre_challenge_wire(
    t_b: &[ProofPolynomialV1; T_B_POLYNOMIALS_V1],
    h: &[ProofPolynomialV1; H_POLYNOMIALS_V1],
    t_a1: &[ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
    z3: &[ProofPolynomialV1; Z3_POLYNOMIALS_V1],
    z4: &[ProofPolynomialV1; Z4_POLYNOMIALS_V1],
    w1: &[ProofPolynomialV1; TBOX_KMSIS_V1],
    v: ProofPolynomialV1,
) -> Result<Vec<u8>, PresentationProofErrorV1> {
    let mut output = Vec::new();
    for (tag, section) in [
        (1_u8, t_b.as_slice()),
        (2, h.as_slice()),
        (3, t_a1.as_slice()),
        (4, z3.as_slice()),
        (5, z4.as_slice()),
        (6, w1.as_slice()),
        (7, core::slice::from_ref(&v)),
    ] {
        let encoded = encode_polynomials_v1(section);
        output.push(tag);
        output.extend_from_slice(
            &u32::try_from(encoded.len())
                .map_err(|_| PresentationProofErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        output.extend_from_slice(&encoded);
    }
    Ok(output)
}

fn require_relation_digest(
    relation: &BootleLanternApplicationRelationV1,
    transcript: ProofTranscriptCoreV1,
) -> Result<(), PresentationProofErrorV1> {
    if transcript.relation_digest() != application_relation_digest_v1(relation) {
        return Err(PresentationProofErrorV1::RelationDigestMismatch);
    }
    Ok(())
}

fn require_schwartz_commitment_shape(
    h: &[ProofPolynomialV1; H_POLYNOMIALS_V1],
) -> Result<(), PresentationProofErrorV1> {
    if h.iter().any(|polynomial| {
        polynomial.coefficients()[0] != 0
            || polynomial.coefficients()[APPLICATION_RING_DEGREE_V1 / 2] != 0
    }) {
        return Err(PresentationProofErrorV1::InvalidSchwartzCommitment);
    }
    Ok(())
}

fn sample_ternary_vector<const N: usize>(
    randomness: &mut ProofRandomnessV1,
    domain: &[u8],
) -> Result<SecretPolynomialVectorV1<N>, PresentationProofErrorV1> {
    let mut output = SecretPolynomialVectorV1::zero();
    for polynomial in output.polynomials.iter_mut() {
        *polynomial = randomness
            .ternary_polynomial(domain)
            .map_err(PresentationProofErrorV1::Sampling)?;
    }
    Ok(output)
}

fn sample_gaussian_vector<const N: usize>(
    randomness: &mut ProofRandomnessV1,
    profile: BootleSamplingProfileV1,
) -> Result<SecretPolynomialVectorV1<N>, PresentationProofErrorV1> {
    if N != profile.expected_polynomials() {
        return Err(PresentationProofErrorV1::Sampling(
            SamplingErrorV1::InvalidGaussianShape,
        ));
    }
    let mut output = SecretPolynomialVectorV1::zero();
    for polynomial in output.polynomials.iter_mut() {
        *polynomial = randomness
            .gaussian_polynomial(profile)
            .map_err(PresentationProofErrorV1::Sampling)?;
    }
    Ok(output)
}

fn sample_uniform_g(
    randomness: &mut ProofRandomnessV1,
    domain: &[u8],
) -> Result<ProofPolynomialV1, PresentationProofErrorV1> {
    let uniform = randomness
        .uniform_polynomial(domain)
        .map_err(PresentationProofErrorV1::Sampling)?;
    let mut coefficients = *uniform.coefficients();
    coefficients[0] = 0;
    coefficients[APPLICATION_RING_DEGREE_V1 / 2] = 0;
    ProofPolynomialV1::new(coefficients).map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn project_centered(
    matrix: &[i8],
    columns: usize,
    vector: &[i64],
) -> Result<Zeroizing<Vec<i64>>, PresentationProofErrorV1> {
    if columns == 0
        || vector.len() != columns
        || matrix.len()
            != PROJECTION_COORDINATES_V1
                .checked_mul(columns)
                .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?
    {
        return Err(PresentationProofErrorV1::InternalInvariant);
    }
    let mut output = Zeroizing::new(vec![0_i64; PROJECTION_COORDINATES_V1]);
    for (row, value) in output.iter_mut().enumerate() {
        let start = row * columns;
        let mut accumulator = 0_i128;
        for (coefficient, secret) in matrix[start..start + columns]
            .iter()
            .copied()
            .zip(vector.iter().copied())
        {
            accumulator = accumulator
                .checked_add(i128::from(coefficient) * i128::from(secret))
                .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?;
        }
        *value =
            i64::try_from(accumulator).map_err(|_| PresentationProofErrorV1::ArithmeticOverflow)?;
    }
    Ok(output)
}

fn polynomials_from_centered_projection(
    coefficients: &[i64],
) -> Box<[ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1]> {
    debug_assert_eq!(coefficients.len(), PROJECTION_COORDINATES_V1);
    boxed_polynomial_array_from_fn_v1(|polynomial| {
        let start = polynomial * APPLICATION_RING_DEGREE_V1;
        let mut array = [0_i64; APPLICATION_RING_DEGREE_V1];
        array.copy_from_slice(&coefficients[start..start + APPLICATION_RING_DEGREE_V1]);
        ProofPolynomialV1::from_centered_coefficients(array)
    })
}

fn multiply_vector_by_polynomial<const N: usize>(
    vector: &[ProofPolynomialV1; N],
    scalar: ProofPolynomialV1,
) -> Box<[ProofPolynomialV1; N]> {
    boxed_polynomial_array_from_fn_v1(|index| scalar.multiply(vector[index]))
}

fn add_arrays<const N: usize>(
    lhs: &[ProofPolynomialV1; N],
    rhs: &[ProofPolynomialV1; N],
) -> Box<[ProofPolynomialV1; N]> {
    boxed_polynomial_array_from_fn_v1(|index| lhs[index].add(rhs[index]))
}

fn centered_vector(polynomials: &[ProofPolynomialV1]) -> Vec<i64> {
    let mut output = Vec::with_capacity(polynomials.len() * APPLICATION_RING_DEGREE_V1);
    for polynomial in polynomials {
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            output.push(polynomial.centered_coefficient(index));
        }
    }
    output
}

fn centered_squared_norm(
    polynomials: &[ProofPolynomialV1],
) -> Result<u128, PresentationProofErrorV1> {
    let mut norm = 0_u128;
    for polynomial in polynomials {
        norm = norm
            .checked_add(polynomial.centered_squared_norm())
            .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?;
    }
    Ok(norm)
}

fn proof_polynomial_array<const N: usize>(
    mut polynomial: impl FnMut(usize) -> Option<ProofPolynomialV1>,
) -> Result<Box<[ProofPolynomialV1; N]>, PresentationProofErrorV1> {
    let mut output = Vec::with_capacity(N);
    for index in 0..N {
        output.push(polynomial(index).ok_or(PresentationProofErrorV1::MalformedProof)?);
    }
    output
        .into_boxed_slice()
        .try_into()
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn array_prefix<const N: usize, const M: usize>(
    input: &[ProofPolynomialV1; M],
) -> &[ProofPolynomialV1; N] {
    input[..N]
        .try_into()
        .expect("fixed profile prefix shape is valid")
}

fn array_suffix<const N: usize, const M: usize>(
    input: &[ProofPolynomialV1; M],
) -> &[ProofPolynomialV1; N] {
    input[M - N..]
        .try_into()
        .expect("fixed profile suffix shape is valid")
}

/// Complete native presentation proof failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PresentationProofErrorV1 {
    /// The transcript named another compiled application relation.
    #[error("Bootle/Lantern transcript relation digest mismatch")]
    RelationDigestMismatch,
    /// Native sampling failed.
    #[error("Bootle/Lantern presentation sampling failed: {0}")]
    Sampling(SamplingErrorV1),
    /// Constraint compilation or transparent expansion failed.
    #[error("Bootle/Lantern presentation toolbox failed: {0}")]
    Toolbox(ToolboxErrorV1),
    /// Canonical compression or reconciliation failed.
    #[error("Bootle/Lantern presentation compression failed: {0}")]
    Compression(CompressionErrorV1),
    /// Strict proof construction failed.
    #[error("Bootle/Lantern presentation proof codec failed: {0}")]
    Codec(ProofCodecErrorV1),
    /// A public response exceeded its fixed theorem-derived bound.
    #[error("Bootle/Lantern presentation public response bound failed: {0}")]
    ResponseBound(ResponseBoundErrorV1),
    /// The lifted application quotient exceeded the fixed ARP bound.
    #[error("Bootle/Lantern lifted application quotient exceeds its fixed bound")]
    ApplicationQuotientBoundExceeded,
    /// The compressed, partially hidden `z2` response exceeded its bound.
    #[error("Bootle/Lantern compressed response exceeds its fixed squared-norm bound")]
    CompressedResponseBoundExceeded,
    /// A public Schwartz commitment exposed a non-zero checked coefficient.
    #[error("Bootle/Lantern Schwartz commitment has a non-zero checked coefficient")]
    InvalidSchwartzCommitment,
    /// The exact constraint system rejected a purported valid witness.
    #[error("Bootle/Lantern constraint system rejected the presentation witness")]
    ConstraintSystemRejectedWitness,
    /// The stored challenge did not match verifier reconstruction.
    #[error("Bootle/Lantern Fiat--Shamir challenge mismatch")]
    ChallengeMismatch,
    /// A typed section was unexpectedly absent from a validated proof.
    #[error("Bootle/Lantern validated proof has a malformed typed section")]
    MalformedProof,
    /// The shared bounded rejection-sampling budget was exhausted.
    ///
    /// The public error deliberately omits the retry stage and counters because
    /// those diagnostics are derived from secret prover work.
    #[error("Bootle/Lantern sampling exhausted the shared proof work budget")]
    SamplingBudgetExhausted,
    /// Prover output failed the independent verifier.
    #[error("Bootle/Lantern prover self-check failed")]
    ProverSelfCheckFailed,
    /// Checked arithmetic overflowed.
    #[error("Bootle/Lantern presentation arithmetic overflowed")]
    ArithmeticOverflow,
    /// A fixed implementation invariant failed.
    #[error("Bootle/Lantern presentation internal invariant failed")]
    InternalInvariant,
}

// INTEGER_ONLY_PROOF_PRODUCTION_END

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use iroha_data_model::privacy::{
        BootleLanternAllowedAttributeValuesV1, BootleLanternAttributeValueV1,
        BootleLanternDisclosedAttributeV1, BootleLanternIssuerPolicyV1,
        IrohaBootleLanternAnoncredStatementV1, PrivacyBootleLanternIssuerPolicyDigestV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyStatementV1, PrivacyTransactionIntentDigestV1,
        PrivacyVerifierDigestV1,
    };
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};
    use sha3::{Digest, Sha3_256};

    use super::*;
    use crate::privacy_engines::bootle_lantern::{
        BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1, BoundPresentationEncodedErrorV1,
        BoundPresentationErrorV1,
        codec::{PROOF_BYTES_V1, PROOF_HEADER_BYTES_V1},
        compression::proof_residue_from_centered_v1,
        issuer::{
            BootleLanternIssuerKeyPairV1, BootleLanternIssuerPolicyMetadataV1,
            holder_finalize_blind_issuance_v1, holder_prepare_blind_issuance_with_rng_v1,
            issuer_blind_issue_with_rng_v1,
        },
        params::{
            APPLICATION_MODULUS_V1, CHALLENGE_OMEGA_V1, COMPRESSION_MODULUS_V1, PROOF_MODULUS_V1,
            Z4_INFINITY_NORM_BOUND_V1,
        },
        prove_bound_presentation_v1,
        relation::{compile_application_relation_v1, validate_presentation_witness_v1},
        ring::ApplicationPolynomialV1,
        transcript::{
            BlindIssuanceRequestChallengeBindingV1, MatrixSeedV1, PresentationChallengeBindingV1,
            matrix_seed_v1,
        },
        verify_bound_presentation_encoded_v1, verify_bound_presentation_v1,
    };

    const H_START_TEST: usize = T_B_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const T_A1_START_TEST: usize = H_START_TEST + H_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const CHALLENGE_START_TEST: usize =
        T_A1_START_TEST + T_A1_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const HINT_START_TEST: usize = CHALLENGE_START_TEST + APPLICATION_RING_DEGREE_V1;
    const Z1_START_TEST: usize = HINT_START_TEST + HINT_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z21_START_TEST: usize = Z1_START_TEST + Z1_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z3_START_TEST: usize = Z21_START_TEST + Z21_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z4_START_TEST: usize = Z3_START_TEST + Z3_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;

    struct TestRng {
        state: u64,
        fail: bool,
        stuck: Option<u8>,
        period: Option<u8>,
    }

    impl TestRng {
        const fn healthy(seed: u64) -> Self {
            Self {
                state: seed,
                fail: false,
                stuck: None,
                period: None,
            }
        }

        const fn failed() -> Self {
            Self {
                state: 1,
                fail: true,
                stuck: None,
                period: None,
            }
        }

        const fn stuck(byte: u8) -> Self {
            Self {
                state: 1,
                fail: false,
                stuck: Some(byte),
                period: None,
            }
        }

        const fn periodic(period: u8) -> Self {
            Self {
                state: 1,
                fail: false,
                stuck: None,
                period: Some(period),
            }
        }
    }

    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0_u8; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0_u8; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            self.try_fill_bytes(destination)
                .expect("infallible test invocation");
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            if self.fail {
                return Err(RngError::new("injected Bootle/Lantern RNG failure"));
            }
            if let Some(byte) = self.stuck {
                destination.fill(byte);
                return Ok(());
            }
            if let Some(period) = self.period {
                for (index, byte) in destination.iter_mut().enumerate() {
                    *byte = ((index % usize::from(period)) as u8)
                        .wrapping_mul(31)
                        .wrapping_add(5);
                }
                return Ok(());
            }
            for byte in destination {
                self.state ^= self.state << 13;
                self.state ^= self.state >> 7;
                self.state ^= self.state << 17;
                *byte = self.state as u8;
            }
            Ok(())
        }
    }

    impl CryptoRng for TestRng {}

    struct Fixture {
        policy: BootleLanternIssuerPolicyV1,
        statement: IrohaBootleLanternAnoncredStatementV1,
        genesis_hash: [u8; 32],
        relation: BootleLanternApplicationRelationV1,
        witness: BootleLanternPresentationWitnessV1,
        transcript: PresentationTranscriptV1,
    }

    fn raw(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn matrix_seed() -> MatrixSeedV1 {
        matrix_seed_v1([0x31; 32]).expect("valid governed matrix seed")
    }

    fn statement_context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: "bootle-lantern-proof-test".parse().expect("valid chain id"),
            action_index: 3,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(raw(1)),
            parameter_id: PrivacyParameterIdV1::new(raw(2)),
            parameter_digest: PrivacyParameterDigestV1::new([0x31; 32]),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(4)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(5)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(6)),
        }
    }

    fn statement(policy: &BootleLanternIssuerPolicyV1) -> IrohaBootleLanternAnoncredStatementV1 {
        IrohaBootleLanternAnoncredStatementV1 {
            context: statement_context(),
            issuer_id: policy.issuer_id,
            policy_id: policy.policy_id,
            issuer_policy_epoch: policy.epoch,
            issuer_policy_record_digest: policy.record_digest,
            issuer_parameter_id: policy.issuer_parameter_id,
            issuer_parameter_digest: policy.issuer_parameter_digest,
            disclosures: vec![BootleLanternDisclosedAttributeV1 {
                index: 1,
                value: BootleLanternAttributeValueV1::new([1; 8]),
            }],
        }
    }

    struct IssuedFixture {
        policy: BootleLanternIssuerPolicyV1,
        statement: IrohaBootleLanternAnoncredStatementV1,
        witness: BootleLanternPresentationWitnessV1,
    }

    fn issued_fixture() -> &'static IssuedFixture {
        static FIXTURE: OnceLock<IssuedFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let mut keygen_rng = TestRng::healthy(0x6a09_e667_f3bc_c908);
            let issuer_key_pair = BootleLanternIssuerKeyPairV1::generate_with_rng_v1(
                PrivacyParameterIdV1::new(raw(13)),
                &mut keygen_rng,
            )
            .expect("native issuer key generation");
            let policy = issuer_key_pair
                .active_policy_v1(BootleLanternIssuerPolicyMetadataV1 {
                    issuer_id: PrivacyIssuerIdV1::new(raw(11)),
                    policy_id: PrivacyPolicyIdV1::new(raw(12)),
                    epoch: 1,
                    required_disclosure_bitmap: 0b0000_0010,
                    allowed_values: (0..8)
                        .map(|index| BootleLanternAllowedAttributeValuesV1 {
                            values: if index == 1 {
                                vec![BootleLanternAttributeValueV1::new([1; 8])]
                            } else {
                                Vec::new()
                            },
                        })
                        .collect(),
                })
                .expect("active native issuer policy");
            let context = statement_context();
            let genesis_hash = [0x32; 32];
            let mut attributes = [[0_u8; 8]; 8];
            attributes[1] = [1; 8];
            let mut holder_mask_rng = TestRng::healthy(0xbb67_ae85_84ca_a73b);
            let mut holder_proof_rng = TestRng::healthy(0x3c6e_f372_fe94_f82b);
            let (request, state) = holder_prepare_blind_issuance_with_rng_v1(
                &context,
                genesis_hash,
                &policy,
                attributes,
                &mut holder_mask_rng,
                &mut holder_proof_rng,
            )
            .expect("holder blind-issuance request");
            let mut tag_rng = TestRng::healthy(0xa54f_f53a_5f1d_36f1);
            let mut preimage_rng = TestRng::healthy(0x510e_527f_ade6_82d1);
            let response = issuer_blind_issue_with_rng_v1(
                &issuer_key_pair,
                &context,
                genesis_hash,
                &policy,
                &request,
                &mut tag_rng,
                &mut preimage_rng,
            )
            .expect("native blind issuance");
            let credential =
                holder_finalize_blind_issuance_v1(state, &context, genesis_hash, &policy, response)
                    .expect("holder issuance finalization");
            let statement = statement(&policy);
            let witness = credential
                .presentation_witness_v1(&statement, &policy, genesis_hash)
                .expect("issued presentation witness");
            IssuedFixture {
                policy,
                statement,
                witness,
            }
        })
    }

    fn valid_witness() -> BootleLanternPresentationWitnessV1 {
        issued_fixture().witness.clone()
    }

    fn fixture() -> Fixture {
        let issued = issued_fixture();
        let policy = issued.policy.clone();
        let statement = issued.statement.clone();
        let genesis_hash = [0x32; 32];
        let relation =
            compile_application_relation_v1(&statement, &policy, matrix_seed(), genesis_hash)
                .expect("compiled application relation");
        let witness = issued.witness.clone();
        validate_presentation_witness_v1(&relation, &witness).expect("valid presentation witness");
        let statement_digest = PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement.clone())
            .digest()
            .expect("canonical typed statement digest");
        let transcript = PresentationTranscriptV1::new(
            PresentationChallengeBindingV1 {
                parameter_digest: *statement.context.parameter_digest.as_bytes(),
                genesis_hash,
                statement_digest: *statement_digest.as_bytes(),
                issuer_policy_record_digest: *statement.issuer_policy_record_digest.as_bytes(),
                transaction_intent_digest: *statement.context.transaction_intent_digest.as_bytes(),
            },
            matrix_seed(),
            application_relation_digest_v1(&relation),
        )
        .expect("fully bound presentation transcript");
        Fixture {
            policy,
            statement,
            genesis_hash,
            relation,
            witness,
            transcript,
        }
    }

    fn proof_from_mutation(
        proof: &BootleLanternPresentationProofV1,
        mutate: impl FnOnce(&mut [u64]),
    ) -> BootleLanternPresentationProofV1 {
        let mut coefficients = proof.coefficients().to_vec();
        mutate(&mut coefficients);
        BootleLanternPresentationProofV1::from_coefficients(coefficients.into_boxed_slice())
            .expect("mutation remains a canonical proof encoding")
    }

    fn alternate_residue(residue: u64) -> u64 {
        if residue == 0 { 1 } else { 0 }
    }

    #[test]
    fn blind_issuance_and_presentation_purposes_derive_distinct_challenges() {
        let seed = matrix_seed();
        let relation_digest = [0x95; 32];
        let presentation = PresentationTranscriptV1::new(
            PresentationChallengeBindingV1 {
                parameter_digest: [0x31; 32],
                genesis_hash: [0x32; 32],
                statement_digest: [0x33; 32],
                issuer_policy_record_digest: [0x34; 32],
                transaction_intent_digest: [0x35; 32],
            },
            seed,
            relation_digest,
        )
        .expect("canonical P2 transcript");
        let blind_issuance = BlindIssuanceRequestTranscriptV1::new(
            BlindIssuanceRequestChallengeBindingV1 {
                parameter_digest: [0x31; 32],
                genesis_hash: [0x32; 32],
                issuer_profile_digest: [0x33; 32],
                credential_scope_digest: [0x36; 32],
                issuer_policy_record_digest: [0x34; 32],
                masked_target_digest: [0x37; 32],
                request_nonce: [0x35; 32],
            },
            seed,
            relation_digest,
        )
        .expect("canonical P1 transcript");
        let pre_challenge = b"same canonical fixed-profile proof body";
        assert_ne!(
            presentation
                .derive_final_challenge(pre_challenge)
                .expect("P2 challenge"),
            blind_issuance
                .derive_final_challenge(pre_challenge)
                .expect("P1 challenge"),
            "P1 and P2 must not share a Fiat--Shamir challenge namespace"
        );
    }

    fn quadratic_test_variables(
        x: ProofPolynomialV1,
        y: ProofPolynomialV1,
    ) -> QuadraticVariablesV1 {
        let mut variables = QuadraticVariablesV1::zero();
        variables.short[0] = x;
        variables.message[0] = y;
        variables
    }

    fn quadratic_test_map(variables: &QuadraticVariablesV1) -> ProofPolynomialV1 {
        // Q(x, y) = 3x² - 5xy + 2y² + 7x - 11y - 13.
        let x = variables.short[0];
        let y = variables.message[0];
        x.multiply(x)
            .scale_centered(3)
            .add(x.multiply(y).scale_centered(-5))
            .add(y.multiply(y).scale_centered(2))
            .add(x.scale_centered(7))
            .add(y.scale_centered(-11))
            .add(ProofPolynomialV1::constant_centered(-13))
    }

    fn actual_quadratic_equation<'a>(fixture: &'a Fixture) -> QuadraticEquationV1<'a> {
        let t_b = boxed_polynomial_array_from_fn_v1::<TBOX_LEXT_V1>(|polynomial| {
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|coefficient| {
                let polynomial = i64::try_from(polynomial).expect("polynomial index fits i64");
                let coefficient = i64::try_from(coefficient).expect("coefficient index fits i64");
                (polynomial * 37 + coefficient * 19).rem_euclid(257) - 128
            }))
        });
        let z3 = boxed_polynomial_array_from_fn_v1::<PROJECTION_POLYNOMIALS_V1>(|polynomial| {
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|coefficient| {
                let polynomial = i64::try_from(polynomial).expect("polynomial index fits i64");
                let coefficient = i64::try_from(coefficient).expect("coefficient index fits i64");
                (polynomial * 43 + coefficient * 23).rem_euclid(193) - 96
            }))
        });
        let z4 = boxed_polynomial_array_from_fn_v1::<PROJECTION_POLYNOMIALS_V1>(|polynomial| {
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|coefficient| {
                let polynomial = i64::try_from(polynomial).expect("polynomial index fits i64");
                let coefficient = i64::try_from(coefficient).expect("coefficient index fits i64");
                87 - (polynomial * 31 + coefficient * 17).rem_euclid(175)
            }))
        });
        let h = boxed_polynomial_array_from_fn_v1::<H_POLYNOMIALS_V1>(|polynomial| {
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|coefficient| {
                if coefficient == 0 || coefficient == APPLICATION_RING_DEGREE_V1 / 2 {
                    0
                } else {
                    let polynomial = i64::try_from(polynomial).expect("polynomial index fits i64");
                    let coefficient =
                        i64::try_from(coefficient).expect("coefficient index fits i64");
                    (polynomial * 29 + coefficient * 11).rem_euclid(127) - 63
                }
            }))
        });
        let (projection_r, projection_r_prime) =
            derive_projection_matrices(fixture.transcript, &t_b)
                .expect("transcript-bound projection matrices");
        let weights = derive_schwartz_weights(fixture.transcript, &t_b).expect("Schwartz weights");
        let multipliers = derive_equation_multipliers(fixture.transcript, &t_b, &h, &z3, &z4)
            .expect("ring equation multipliers");
        QuadraticEquationV1::new(
            &fixture.relation,
            projection_r,
            projection_r_prime,
            z3,
            z4,
            h,
            weights,
            multipliers,
        )
        .expect("fully compiled actual quadratic equation")
    }

    fn actual_quadratic_variables() -> QuadraticVariablesV1 {
        let short = boxed_polynomial_array_from_fn_v1::<TBOX_M1_V1>(|polynomial| {
            ProofPolynomialV1::new(core::array::from_fn(|coefficient| {
                match (polynomial + coefficient) % 7 {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    3 => PROOF_MODULUS_V1 - 1,
                    4 => PROOF_MODULUS_V1 - 2,
                    5 => u64::try_from((polynomial * 65_537 + coefficient * 131_071) % 1_000_003)
                        .expect("small patterned residue fits u64"),
                    _ => {
                        PROOF_MODULUS_V1
                            - 1
                            - u64::try_from((polynomial + 3 * coefficient) % 19)
                                .expect("small residue fits u64")
                    }
                }
            }))
            .expect("all adversarial residues are canonical")
        });
        let mut message =
            boxed_polynomial_array_from_fn_v1::<QUADRATIC_MESSAGE_POLYNOMIALS_V1>(|polynomial| {
                ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|coefficient| {
                    let polynomial = i64::try_from(polynomial).expect("polynomial index fits i64");
                    let coefficient =
                        i64::try_from(coefficient).expect("coefficient index fits i64");
                    (polynomial * 53 + coefficient * 97).rem_euclid(509) - 254
                }))
            });
        let mut beta = [0_i64; APPLICATION_RING_DEGREE_V1];
        beta[0] = -1;
        beta[APPLICATION_RING_DEGREE_V1 / 2] = 1;
        message[BETA_MESSAGE_INDEX_V1] = ProofPolynomialV1::from_centered_coefficients(beta);
        QuadraticVariablesV1 { short, message }
    }

    fn scale_quadratic_variables(
        variables: &QuadraticVariablesV1,
        scalar: ProofPolynomialV1,
    ) -> QuadraticVariablesV1 {
        QuadraticVariablesV1 {
            short: boxed_polynomial_array_from_fn_v1(|index| {
                scalar.multiply(variables.short[index])
            }),
            message: boxed_polynomial_array_from_fn_v1(|index| {
                scalar.multiply(variables.message[index])
            }),
        }
    }

    fn actual_quadratic_parts(
        equation: &QuadraticEquationV1<'_>,
        variables: &QuadraticVariablesV1,
    ) -> (ProofPolynomialV1, ProofPolynomialV1) {
        let q0 = equation
            .evaluate(&QuadraticVariablesV1::zero())
            .expect("actual Q at zero");
        let positive = equation.evaluate(variables).expect("actual Q");
        let negative = equation
            .evaluate(&variables.negate())
            .expect("actual Q at negated variables");
        decompose_signed_quadratic_evaluations(q0, positive, negative)
            .expect("fixed inverse of two")
    }

    fn autostable_challenges() -> [ProofPolynomialV1; 3] {
        let sparse = {
            let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
            coefficients[0] = 1;
            coefficients[1] = 1;
            coefficients[APPLICATION_RING_DEGREE_V1 - 1] = -1;
            ProofPolynomialV1::from_centered_coefficients(coefficients)
        };
        let boundary = {
            let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
            coefficients[0] = -CHALLENGE_OMEGA_V1;
            coefficients[31] = CHALLENGE_OMEGA_V1;
            coefficients[33] = -CHALLENGE_OMEGA_V1;
            ProofPolynomialV1::from_centered_coefficients(coefficients)
        };
        let patterned = {
            let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
            for (index, coefficient) in coefficients[..32].iter_mut().enumerate() {
                *coefficient = i64::try_from((index * 7) % 17).expect("small residue") - 8;
            }
            for index in 33..APPLICATION_RING_DEGREE_V1 {
                coefficients[index] = -coefficients[APPLICATION_RING_DEGREE_V1 - index];
            }
            ProofPolynomialV1::from_centered_coefficients(coefficients)
        };
        for challenge in [sparse, boundary, patterned] {
            assert_eq!(challenge, challenge.automorphism());
            assert!(!challenge.is_zero());
        }
        [sparse, boundary, patterned]
    }

    fn adversarial_quadratic_inputs() -> [(
        ProofPolynomialV1,
        ProofPolynomialV1,
        ProofPolynomialV1,
        ProofPolynomialV1,
    ); 4] {
        let patterned_positive =
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|index| {
                i64::try_from(index).expect("ring index fits i64") * 65_537 - 2_000_000
            }));
        let patterned_negative =
            ProofPolynomialV1::from_centered_coefficients(core::array::from_fn(|index| {
                1_700_000 - i64::try_from(index).expect("ring index fits i64") * 131_071
            }));
        let canonical_minus_one =
            ProofPolynomialV1::new([PROOF_MODULUS_V1 - 1; APPLICATION_RING_DEGREE_V1])
                .expect("q - 1 is canonical");
        let canonical_boundary_pattern = ProofPolynomialV1::new(core::array::from_fn(|index| {
            PROOF_MODULUS_V1 - 1 - u64::try_from(index % 3).expect("small residue")
        }))
        .expect("boundary residues are canonical");
        [
            (
                ProofPolynomialV1::ZERO,
                ProofPolynomialV1::ZERO,
                ProofPolynomialV1::ZERO,
                ProofPolynomialV1::ZERO,
            ),
            (
                ProofPolynomialV1::constant_centered(19),
                ProofPolynomialV1::constant_centered(-23),
                ProofPolynomialV1::constant_centered(-29),
                ProofPolynomialV1::constant_centered(31),
            ),
            (
                patterned_positive,
                patterned_negative,
                patterned_negative,
                patterned_positive,
            ),
            (
                canonical_minus_one,
                canonical_boundary_pattern,
                canonical_boundary_pattern,
                canonical_minus_one,
            ),
        ]
    }

    #[test]
    fn verifier_quadratic_reuse_matches_legacy_formulas_at_adversarial_residues() {
        for (x, y, _, _) in adversarial_quadratic_inputs() {
            let variables = quadratic_test_variables(x, y);
            let zero = QuadraticVariablesV1::zero();
            let q0 = quadratic_test_map(&zero);
            let q_positive = quadratic_test_map(&variables);
            let q_negative = quadratic_test_map(&variables.negate());

            let (linear, quadratic) =
                decompose_signed_quadratic_evaluations(q0, q_positive, q_negative)
                    .expect("fixed inverse of two");
            let legacy_linear = q_positive
                .sub(q_negative)
                .scale_canonical(PROOF_INVERSE_TWO_V1)
                .expect("fixed inverse of two");
            let legacy_quadratic = q_positive
                .add(q_negative)
                .sub(q0.scale_centered(2))
                .scale_canonical(PROOF_INVERSE_TWO_V1)
                .expect("fixed inverse of two");

            assert_eq!(linear, legacy_linear);
            assert_eq!(quadratic, legacy_quadratic);
            assert_eq!(linear, x.scale_centered(7).add(y.scale_centered(-11)));
            assert_eq!(
                quadratic,
                x.multiply(x)
                    .scale_centered(3)
                    .add(x.multiply(y).scale_centered(-5))
                    .add(y.multiply(y).scale_centered(2))
            );
            assert_eq!(q_positive, quadratic.add(linear).add(q0));
        }
    }

    #[test]
    fn prover_mask_reuse_is_exactly_three_evaluations_and_matches_legacy_formulas() {
        for (secret_x, secret_y, mask_x, mask_y) in adversarial_quadratic_inputs() {
            let secret = quadratic_test_variables(secret_x, secret_y);
            let mask = quadratic_test_variables(mask_x, mask_y);
            let zero = QuadraticVariablesV1::zero();
            let q0 = quadratic_test_map(&zero);
            let q_secret = quadratic_test_map(&secret);
            let expected_inputs = [mask.clone(), mask.negate(), secret.add(&mask)];
            let evaluations = [
                quadratic_test_map(&expected_inputs[0]),
                quadratic_test_map(&expected_inputs[1]),
                quadratic_test_map(&expected_inputs[2]),
            ];
            let mut calls = 0_usize;

            let (bilinear, linear, quadratic) =
                decompose_mask_quadratic_evaluations(&secret, &mask, q0, q_secret, |variables| {
                    assert!(
                        calls < PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1,
                        "the retry kernel must not make a fourth evaluation"
                    );
                    assert_eq!(variables, &expected_inputs[calls]);
                    let evaluation = evaluations[calls];
                    calls += 1;
                    Ok(evaluation)
                })
                .expect("fixed inverse of two");
            assert_eq!(calls, PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1);

            let [q_mask, q_negative_mask, q_sum] = evaluations;
            let legacy_bilinear = q_sum.sub(q_secret).sub(q_mask).add(q0);
            let legacy_linear = q_mask
                .sub(q_negative_mask)
                .scale_canonical(PROOF_INVERSE_TWO_V1)
                .expect("fixed inverse of two");
            let legacy_quadratic = q_mask
                .add(q_negative_mask)
                .sub(q0.scale_centered(2))
                .scale_canonical(PROOF_INVERSE_TWO_V1)
                .expect("fixed inverse of two");
            assert_eq!(bilinear, legacy_bilinear);
            assert_eq!(linear, legacy_linear);
            assert_eq!(quadratic, legacy_quadratic);

            let expected_bilinear = secret_x
                .multiply(mask_x)
                .scale_centered(6)
                .add(
                    secret_x
                        .multiply(mask_y)
                        .add(mask_x.multiply(secret_y))
                        .scale_centered(-5),
                )
                .add(secret_y.multiply(mask_y).scale_centered(4));
            assert_eq!(bilinear, expected_bilinear);
            assert_eq!(
                linear,
                mask_x.scale_centered(7).add(mask_y.scale_centered(-11))
            );
            assert_eq!(
                quadratic,
                mask_x
                    .multiply(mask_x)
                    .scale_centered(3)
                    .add(mask_x.multiply(mask_y).scale_centered(-5))
                    .add(mask_y.multiply(mask_y).scale_centered(2))
            );
        }
    }

    #[test]
    fn prover_mask_reuse_fails_closed_at_each_black_box_evaluation() {
        let secret = QuadraticVariablesV1::zero();
        let mask = QuadraticVariablesV1::zero();
        for failure_index in 0..PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1 {
            let mut calls = 0_usize;
            let result = decompose_mask_quadratic_evaluations(
                &secret,
                &mask,
                ProofPolynomialV1::ZERO,
                ProofPolynomialV1::ZERO,
                |_| {
                    let index = calls;
                    calls += 1;
                    if index == failure_index {
                        Err(PresentationProofErrorV1::InternalInvariant)
                    } else {
                        Ok(ProofPolynomialV1::ZERO)
                    }
                },
            );
            assert_eq!(result, Err(PresentationProofErrorV1::InternalInvariant));
            assert_eq!(
                calls,
                failure_index + 1,
                "evaluation must stop immediately after the injected failure"
            );
        }
    }

    #[test]
    fn actual_zq_compiler_checked_coefficients_match_scalar_oracle() {
        let fixture = fixture();
        let equation = actual_quadratic_equation(&fixture);
        let variables = actual_quadratic_variables();
        let canonical_constraints = equation
            .constraints(&variables)
            .expect("canonical coefficient constraints");
        let lifted_constraints = equation
            .lifted_constraints(&variables)
            .expect("homogeneous coefficient-field lift");
        assert_eq!(
            lifted_constraints, canonical_constraints,
            "a correctly shaped beta must make the homogeneous lift exactly canonical"
        );
        let scalar = equation
            .scalar_schwartz_accumulators(&variables)
            .expect("cfg(test) scalar oracle");
        let lifted = equation
            .lifted_schwartz_accumulators(&variables)
            .expect("cfg(test) homogeneous-lift oracle");
        assert_eq!(lifted, scalar);
        let masked = equation
            .schwartz_polynomials(&variables)
            .expect("masked Zq-to-Rq compiler");

        assert_eq!(masked[0].coefficients()[0], scalar[0]);
        assert_eq!(
            masked[0].coefficients()[APPLICATION_RING_DEGREE_V1 / 2],
            scalar[1]
        );
        assert_eq!(masked[1].coefficients()[0], scalar[2]);
        assert_eq!(
            masked[1].coefficients()[APPLICATION_RING_DEGREE_V1 / 2],
            scalar[3]
        );
    }

    #[test]
    fn actual_zq_compiler_matches_augmented_oracle_for_adversarial_beta_shape_noise() {
        let fixture = fixture();
        let equation = actual_quadratic_equation(&fixture);

        for (index, noise) in [(1, 7_i64), (17, -11), (31, 19), (47, -23), (63, 29)] {
            let mut variables = actual_quadratic_variables();
            let mut beta = core::array::from_fn(|coefficient| {
                variables.message[BETA_MESSAGE_INDEX_V1].centered_coefficient(coefficient)
            });
            beta[index] = noise;
            variables.message[BETA_MESSAGE_INDEX_V1] =
                ProofPolynomialV1::from_centered_coefficients(beta);

            let canonical_constraints = equation
                .constraints(&variables)
                .expect("canonical coefficient constraints");
            let lifted_constraints = equation
                .lifted_constraints(&variables)
                .expect("homogeneous coefficient-field lift");
            assert!(
                canonical_constraints[..2 * (APPLICATION_RING_DEGREE_V1 - 1)]
                    .iter()
                    .any(|constraint| *constraint != 0),
                "each injected beta coefficient must trip an independent shape equation"
            );
            let projection_start = EVALUATION_CONSTRAINTS_V1 - 2 * PROJECTION_COORDINATES_V1;
            assert!(
                canonical_constraints[projection_start..]
                    .iter()
                    .zip(&lifted_constraints[projection_start..])
                    .any(|(canonical, lifted)| canonical != lifted),
                "shape noise must exercise the augmented projection lift"
            );

            let lifted = equation
                .lifted_schwartz_accumulators(&variables)
                .expect("cfg(test) homogeneous-lift oracle");
            let masked = equation
                .schwartz_polynomials(&variables)
                .expect("masked Zq-to-Rq compiler");
            assert_eq!(masked[0].coefficients()[0], lifted[0]);
            assert_eq!(
                masked[0].coefficients()[APPLICATION_RING_DEGREE_V1 / 2],
                lifted[1]
            );
            assert_eq!(masked[1].coefficients()[0], lifted[2]);
            assert_eq!(
                masked[1].coefficients()[APPLICATION_RING_DEGREE_V1 / 2],
                lifted[3]
            );
        }
    }

    #[test]
    fn actual_quadratic_equation_is_equivariant_for_adversarial_autostable_challenges() {
        let fixture = fixture();
        let equation = actual_quadratic_equation(&fixture);
        let variables = actual_quadratic_variables();
        let (linear, quadratic) = actual_quadratic_parts(&equation, &variables);

        for challenge in autostable_challenges() {
            let scaled = scale_quadratic_variables(&variables, challenge);
            let (scaled_linear, scaled_quadratic) = actual_quadratic_parts(&equation, &scaled);
            assert_eq!(
                scaled_linear,
                challenge.multiply(linear),
                "actual linear part must commute with the full ring challenge"
            );
            assert_eq!(
                scaled_quadratic,
                challenge.multiply(challenge).multiply(quadratic),
                "actual homogeneous-quadratic part must commute with challenge squared"
            );
        }
    }

    #[test]
    fn quadratic_evaluation_budgets_are_fixed_and_bounded() {
        assert_eq!(PROVER_PRECOMPUTED_QUADRATIC_EVALUATIONS_V1, 2);
        assert_eq!(PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1, 3);
        assert_eq!(VERIFIER_QUADRATIC_EVALUATIONS_V1, 3);
        assert_eq!(MAX_QUADRATIC_EVALUATIONS_PER_PROVE_ATTEMPT_V1, 3_074);
        assert_eq!(
            MAX_QUADRATIC_EVALUATIONS_PER_PROVE_ATTEMPT_V1,
            PROVER_PRECOMPUTED_QUADRATIC_EVALUATIONS_V1
                + PROVER_QUADRATIC_EVALUATIONS_PER_MASK_RETRY_V1
                    * MAX_RESPONSE_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1 as usize
        );
        assert_eq!(MIN_RESPONSE_CONTEXTS_AT_GLOBAL_BUDGET_V1, 4);
        assert_eq!(MAX_TOP_LEVEL_QUADRATIC_EVALUATIONS_V1, 12_287);
    }

    #[test]
    fn native_secret_workspaces_are_heap_backed_for_validator_worker_stacks() {
        assert_eq!(
            core::mem::size_of::<SecretPolynomialVectorV1<TBOX_M2_V1>>(),
            core::mem::size_of::<Box<[ProofPolynomialV1; TBOX_M2_V1]>>()
        );
        assert_eq!(
            core::mem::size_of::<QuadraticVariablesV1>(),
            core::mem::size_of::<Box<[ProofPolynomialV1; TBOX_M1_V1]>>()
                + core::mem::size_of::<Box<[ProofPolynomialV1; QUADRATIC_MESSAGE_POLYNOMIALS_V1]>>(
                )
        );
        assert_eq!(
            core::mem::size_of::<super::super::toolbox::ShortWitnessV1>(),
            core::mem::size_of::<Box<[ProofPolynomialV1; TBOX_M1_V1]>>()
        );
    }

    #[test]
    fn shared_rejection_budget_exhausts_projection_stage_exactly() {
        let mut budget = ProofRejectionBudgetV1::new(7);
        for draw in 0..7 {
            assert!(budget.reserve(ProofRejectionStageV1::Projection));
            assert_eq!(budget.projection_draws, draw + 1);
            assert_eq!(budget.response_mask_draws, 0);
            assert_eq!(budget.remaining(), 6 - draw);
        }
        assert!(!budget.reserve(ProofRejectionStageV1::Projection));
        assert_eq!(budget.projection_draws, 7);
        assert_eq!(budget.response_mask_draws, 0);
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            budget.exhaustion_error(),
            PresentationProofErrorV1::SamplingBudgetExhausted
        );
    }

    #[test]
    fn shared_rejection_budget_exhausts_response_stage_exactly() {
        let mut budget = ProofRejectionBudgetV1::new(7);
        for draw in 0..7 {
            assert!(budget.reserve(ProofRejectionStageV1::ResponseMask));
            assert_eq!(budget.projection_draws, 0);
            assert_eq!(budget.response_mask_draws, draw + 1);
            assert_eq!(budget.remaining(), 6 - draw);
        }
        assert!(!budget.reserve(ProofRejectionStageV1::ResponseMask));
        assert_eq!(budget.projection_draws, 0);
        assert_eq!(budget.response_mask_draws, 7);
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            budget.exhaustion_error(),
            PresentationProofErrorV1::SamplingBudgetExhausted
        );
    }

    #[test]
    fn shared_rejection_budget_cannot_multiply_across_stages() {
        let mut budget = ProofRejectionBudgetV1::new(MAX_PROOF_SAMPLING_ATTEMPTS_V1);
        let projection_draws = MAX_PROJECTION_SAMPLING_ATTEMPTS_PER_PROVE_ATTEMPT_V1;
        for _ in 0..projection_draws {
            assert!(budget.reserve(ProofRejectionStageV1::Projection));
        }
        let response_draws = MAX_PROOF_SAMPLING_ATTEMPTS_V1 - projection_draws;
        for _ in 0..response_draws {
            assert!(budget.reserve(ProofRejectionStageV1::ResponseMask));
        }
        assert!(!budget.reserve(ProofRejectionStageV1::Projection));
        assert_eq!(budget.projection_draws, projection_draws);
        assert_eq!(budget.response_mask_draws, response_draws);
        assert_eq!(
            budget
                .projection_draws
                .checked_add(budget.response_mask_draws),
            Some(MAX_PROOF_SAMPLING_ATTEMPTS_V1)
        );
    }

    #[test]
    fn rejection_accounting_preserves_each_actual_failure_boundary() {
        let mut budget = ProofRejectionBudgetV1::new(3);
        assert!(budget.reserve(ProofRejectionStageV1::Projection));
        budget.record_projection_rejection();
        assert!(budget.reserve(ProofRejectionStageV1::ResponseMask));
        budget.record_response_sampling_rejection();
        assert!(budget.reserve(ProofRejectionStageV1::ResponseMask));
        budget.record_response_norm_rejection();
        assert!(!budget.reserve(ProofRejectionStageV1::ResponseMask));
        assert_eq!(budget.rejections.projection, 1);
        assert_eq!(budget.rejections.response_sampling, 1);
        assert_eq!(budget.rejections.response_norm, 1);
        assert_eq!(
            budget.exhaustion_error(),
            PresentationProofErrorV1::SamplingBudgetExhausted
        );
    }

    #[test]
    fn secret_rejection_diagnostics_are_invariant_and_fully_redacted() {
        let untouched = ProofRejectionBudgetV1::new(4_095);
        let mut exercised = ProofRejectionBudgetV1::new(17);
        assert!(exercised.reserve(ProofRejectionStageV1::Projection));
        exercised.record_projection_rejection();
        assert!(exercised.reserve(ProofRejectionStageV1::ResponseMask));
        exercised.record_response_sampling_rejection();
        exercised.record_response_norm_rejection();

        assert_eq!(
            format!("{untouched:?}"),
            "ProofRejectionBudgetV1(<redacted>)"
        );
        assert_eq!(format!("{exercised:?}"), format!("{untouched:?}"));
        assert_eq!(
            format!("{:?}", &exercised.rejections),
            "ProofRejectionStatsV1(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", ProofRejectionStatsV1::default()),
            format!("{:?}", &exercised.rejections)
        );
        assert_eq!(
            format!("{}", exercised.exhaustion_error()),
            "Bootle/Lantern sampling exhausted the shared proof work budget"
        );
    }

    #[test]
    #[ignore = "explicit short-budget diagnostic for the expensive native prover"]
    fn short_budget_reports_the_first_native_rejection_boundary() {
        let fixture = fixture();
        let mut rng = TestRng::healthy(0x9e37_79b9_7f4a_7c15);
        match prove_presentation_with_rejection_limit_v1(
            &fixture.relation,
            &fixture.witness,
            fixture.transcript,
            &mut rng,
            32,
        ) {
            Ok(proof) => {
                verify_presentation_v1(&fixture.relation, fixture.transcript, &proof)
                    .expect("short-budget proof self-verifies");
                eprintln!("short-budget diagnostic: proof accepted");
            }
            Err(error) => eprintln!("short-budget diagnostic: {error:?}"),
        }
    }

    #[test]
    fn complete_native_proof_round_trip_and_adversarial_matrix() {
        assert!(
            BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1,
            "the first-release public Bootle/Lantern engine must be active"
        );
        let fixture = fixture();
        let mut rng = TestRng::healthy(0x9e37_79b9_7f4a_7c15);
        let proof = prove_bound_presentation_v1(
            &fixture.statement,
            &fixture.policy,
            fixture.genesis_hash,
            &fixture.witness,
            &mut rng,
        )
        .expect("fully governed native proof");
        verify_presentation_v1(&fixture.relation, fixture.transcript, &proof)
            .expect("native proof verifies");
        verify_bound_presentation_v1(
            &fixture.statement,
            &fixture.policy,
            fixture.genesis_hash,
            &proof,
        )
        .expect("governed native proof verifies");

        let encoded = proof.encode();
        assert_eq!(encoded.len(), PROOF_BYTES_V1);
        assert_eq!(
            hex::encode(Sha3_256::digest(&encoded)),
            "fcca08f5077d94520395e3e6ba49c716e919561d4fb7b9a4b8302988409b0ec8"
        );
        let decoded = BootleLanternPresentationProofV1::decode_exact(
            &encoded,
            u32::try_from(encoded.len()).expect("proof length fits u32"),
        )
        .expect("strict wire round trip");
        assert_eq!(decoded, proof);
        verify_presentation_v1(&fixture.relation, fixture.transcript, &decoded)
            .expect("decoded native proof verifies");
        verify_bound_presentation_encoded_v1(
            &fixture.statement,
            &fixture.policy,
            fixture.genesis_hash,
            &encoded,
            u32::try_from(encoded.len()).expect("proof length fits u32"),
        )
        .expect("strictly decoded governed proof verifies");

        let proof_ceiling =
            u32::try_from(encoded.len() - 1).expect("fixed proof length minus one fits u32");
        assert!(matches!(
            verify_bound_presentation_encoded_v1(
                &fixture.statement,
                &fixture.policy,
                fixture.genesis_hash,
                &encoded,
                proof_ceiling,
            ),
            Err(BoundPresentationEncodedErrorV1::Codec(
                ProofCodecErrorV1::TooLarge { .. }
            ))
        ));
        assert!(matches!(
            verify_bound_presentation_encoded_v1(
                &fixture.statement,
                &fixture.policy,
                fixture.genesis_hash,
                &encoded[..encoded.len() - 1],
                u32::try_from(encoded.len()).expect("proof length fits u32"),
            ),
            Err(BoundPresentationEncodedErrorV1::Codec(
                ProofCodecErrorV1::WrongLength { .. }
            ))
        ));
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(matches!(
            verify_bound_presentation_encoded_v1(
                &fixture.statement,
                &fixture.policy,
                fixture.genesis_hash,
                &trailing,
                u32::try_from(trailing.len()).expect("proof length fits u32"),
            ),
            Err(BoundPresentationEncodedErrorV1::Codec(
                ProofCodecErrorV1::WrongLength { .. }
            ))
        ));
        let mut noncanonical = encoded.clone();
        noncanonical[PROOF_HEADER_BYTES_V1
            ..PROOF_HEADER_BYTES_V1 + super::super::params::PROOF_RESIDUE_BYTES_V1]
            .copy_from_slice(
                &PROOF_MODULUS_V1.to_le_bytes()[..super::super::params::PROOF_RESIDUE_BYTES_V1],
            );
        assert!(matches!(
            verify_bound_presentation_encoded_v1(
                &fixture.statement,
                &fixture.policy,
                fixture.genesis_hash,
                &noncanonical,
                u32::try_from(noncanonical.len()).expect("proof length fits u32"),
            ),
            Err(BoundPresentationEncodedErrorV1::Codec(
                ProofCodecErrorV1::NonCanonicalResidue { index: 0, .. }
            ))
        ));

        let mut bad_magic = encoded.clone();
        bad_magic[0] ^= 1;
        assert!(matches!(
            verify_bound_presentation_encoded_v1(
                &fixture.statement,
                &fixture.policy,
                fixture.genesis_hash,
                &bad_magic,
                u32::try_from(bad_magic.len()).expect("proof length fits u32"),
            ),
            Err(BoundPresentationEncodedErrorV1::Codec(
                ProofCodecErrorV1::InvalidMagic
            ))
        ));

        assert!(matches!(
            verify_bound_presentation_v1(&fixture.statement, &fixture.policy, [0x33; 32], &proof,),
            Err(BoundPresentationErrorV1::Proof(_))
        ));
        let mut changed_intent = fixture.statement.clone();
        changed_intent.context.transaction_intent_digest =
            PrivacyTransactionIntentDigestV1::new(raw(0x61));
        assert!(
            verify_bound_presentation_v1(
                &changed_intent,
                &fixture.policy,
                fixture.genesis_hash,
                &proof,
            )
            .is_err()
        );
        let mut changed_action = fixture.statement.clone();
        changed_action.context.action_index += 1;
        assert!(
            verify_bound_presentation_v1(
                &changed_action,
                &fixture.policy,
                fixture.genesis_hash,
                &proof,
            )
            .is_err()
        );
        let mut changed_parameter = fixture.statement.clone();
        changed_parameter.context.parameter_digest = PrivacyParameterDigestV1::new(raw(0x62));
        assert!(
            verify_bound_presentation_v1(
                &changed_parameter,
                &fixture.policy,
                fixture.genesis_hash,
                &proof,
            )
            .is_err()
        );
        let mut changed_disclosure = fixture.statement.clone();
        changed_disclosure.disclosures[0].value = BootleLanternAttributeValueV1::new([2; 8]);
        assert!(matches!(
            verify_bound_presentation_v1(
                &changed_disclosure,
                &fixture.policy,
                fixture.genesis_hash,
                &proof,
            ),
            Err(BoundPresentationErrorV1::Relation(_))
        ));
        let mut changed_policy_digest = fixture.statement.clone();
        changed_policy_digest.issuer_policy_record_digest =
            PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(0x63));
        assert!(matches!(
            verify_bound_presentation_v1(
                &changed_policy_digest,
                &fixture.policy,
                fixture.genesis_hash,
                &proof,
            ),
            Err(BoundPresentationErrorV1::Relation(_))
        ));
        let mut substituted_policy = fixture.policy.clone();
        substituted_policy.policy_id = PrivacyPolicyIdV1::new(raw(0x64));
        substituted_policy.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        substituted_policy.record_digest = substituted_policy
            .computed_record_digest()
            .expect("substituted policy digest");
        substituted_policy
            .validate()
            .expect("independently valid substituted policy");
        assert!(matches!(
            verify_bound_presentation_v1(
                &fixture.statement,
                &substituted_policy,
                fixture.genesis_hash,
                &proof,
            ),
            Err(BoundPresentationErrorV1::Relation(_))
        ));

        // Every canonical proof section is transcript- or equation-bound.
        for index in [
            0,
            H_START_TEST + 1,
            T_A1_START_TEST,
            HINT_START_TEST,
            Z1_START_TEST,
            Z21_START_TEST,
            Z3_START_TEST,
            Z4_START_TEST,
        ] {
            let changed = proof_from_mutation(&proof, |coefficients| {
                coefficients[index] = alternate_residue(coefficients[index]);
            });
            assert!(
                verify_presentation_v1(&fixture.relation, fixture.transcript, &changed).is_err(),
                "mutation at flat coefficient {index} must fail"
            );
        }

        // Keep the challenge encoding canonical while changing its value.
        let challenge_changed = proof_from_mutation(&proof, |coefficients| {
            let current = proof.challenge_polynomial().centered_coefficient(0);
            let replacement = if current == CHALLENGE_OMEGA_V1 {
                current - 1
            } else {
                current + 1
            };
            coefficients[CHALLENGE_START_TEST] = proof_residue_from_centered_v1(replacement);
        });
        assert!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &challenge_changed)
                .is_err()
        );

        let invalid_h = proof_from_mutation(&proof, |coefficients| {
            coefficients[H_START_TEST] = 1;
        });
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &invalid_h),
            Err(PresentationProofErrorV1::InvalidSchwartzCommitment)
        ));

        let excessive_z1 = proof_from_mutation(&proof, |coefficients| {
            coefficients[Z1_START_TEST] = proof_residue_from_centered_v1(1_040_728_452);
        });
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &excessive_z1),
            Err(PresentationProofErrorV1::ResponseBound(
                ResponseBoundErrorV1::Z1NormExceeded
            ))
        ));

        let invalid_hint = proof_from_mutation(&proof, |coefficients| {
            coefficients[HINT_START_TEST] = proof_residue_from_centered_v1(
                i64::try_from(COMPRESSION_MODULUS_V1 / 2 + 1).expect("hint bound fits i64"),
            );
        });
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &invalid_hint),
            Err(PresentationProofErrorV1::ResponseBound(
                ResponseBoundErrorV1::HintOutOfRange
            ))
        ));

        let excessive_z3 = proof_from_mutation(&proof, |coefficients| {
            coefficients[Z3_START_TEST] = proof_residue_from_centered_v1(10_661_921);
        });
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &excessive_z3),
            Err(PresentationProofErrorV1::ResponseBound(
                ResponseBoundErrorV1::Z3NormExceeded
            ))
        ));

        let excessive_z4 = proof_from_mutation(&proof, |coefficients| {
            coefficients[Z4_START_TEST] = proof_residue_from_centered_v1(
                i64::try_from(Z4_INFINITY_NORM_BOUND_V1 + 1).expect("z4 bound fits i64"),
            );
        });
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &excessive_z4),
            Err(PresentationProofErrorV1::ResponseBound(
                ResponseBoundErrorV1::Z4InfinityNormExceeded
            ))
        ));

        // A replay under another public statement or transaction intent fails.
        for changed_binding in [
            PresentationChallengeBindingV1 {
                statement_digest: [0x51; 32],
                ..fixture.transcript.binding()
            },
            PresentationChallengeBindingV1 {
                genesis_hash: [0x54; 32],
                ..fixture.transcript.binding()
            },
            PresentationChallengeBindingV1 {
                transaction_intent_digest: [0x52; 32],
                ..fixture.transcript.binding()
            },
            PresentationChallengeBindingV1 {
                issuer_policy_record_digest: [0x53; 32],
                ..fixture.transcript.binding()
            },
        ] {
            let changed_transcript = PresentationTranscriptV1::new(
                changed_binding,
                fixture.transcript.matrix_seed(),
                fixture.transcript.relation_digest(),
            )
            .expect("changed transcript remains structurally valid");
            assert!(verify_presentation_v1(&fixture.relation, changed_transcript, &proof).is_err());
        }

        let wrong_relation_transcript = PresentationTranscriptV1::new(
            fixture.transcript.binding(),
            fixture.transcript.matrix_seed(),
            [0x61; 32],
        )
        .expect("non-zero wrong relation digest");
        assert!(matches!(
            verify_presentation_v1(&fixture.relation, wrong_relation_transcript, &proof),
            Err(PresentationProofErrorV1::RelationDigestMismatch)
        ));

        // The all-zero algebraic forgery is canonical on the wire but invalid.
        let zero_forgery = BootleLanternPresentationProofV1::from_coefficients(
            vec![0_u64; PROOF_COEFFICIENTS_V1].into_boxed_slice(),
        )
        .expect("zero proof has a canonical challenge representation");
        assert!(
            verify_presentation_v1(&fixture.relation, fixture.transcript, &zero_forgery).is_err()
        );
    }

    #[test]
    fn prover_rejects_rng_failure_health_sentinels_and_invalid_witnesses() {
        let fixture = fixture();
        for mut rng in [
            TestRng::failed(),
            TestRng::stuck(0),
            TestRng::stuck(0x7f),
            TestRng::periodic(8),
        ] {
            assert!(matches!(
                prove_presentation_v1(
                    &fixture.relation,
                    &fixture.witness,
                    fixture.transcript,
                    &mut rng
                ),
                Err(PresentationProofErrorV1::Sampling(
                    SamplingErrorV1::RandomnessUnavailable
                        | SamplingErrorV1::RandomnessHealthCheckFailed
                ))
            ));
        }

        let mut invalid = valid_witness();
        invalid.signature_one[0] = ApplicationPolynomialV1::constant(APPLICATION_MODULUS_V1 / 2)
            .expect("canonical application coefficient");
        let mut rng = TestRng::healthy(0x243f_6a88_85a3_08d3);
        assert!(matches!(
            prove_presentation_v1(&fixture.relation, &invalid, fixture.transcript, &mut rng),
            Err(PresentationProofErrorV1::Toolbox(_))
        ));
    }

    #[test]
    fn proof_constructor_rejects_noncanonical_residues_before_verification() {
        let mut coefficients = vec![0_u64; PROOF_COEFFICIENTS_V1];
        coefficients[0] = PROOF_MODULUS_V1;
        assert!(matches!(
            BootleLanternPresentationProofV1::from_coefficients(coefficients.into_boxed_slice()),
            Err(ProofCodecErrorV1::NonCanonicalResidue { index: 0, .. })
        ));
    }
}
