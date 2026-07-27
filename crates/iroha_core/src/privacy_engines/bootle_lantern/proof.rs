//! Native fixed-profile Bootle/Lantern presentation prover and verifier.
//!
//! This module implements the complete presentation path: transparent
//! commitments, projected norm witnesses, Schwartz compression, the generic
//! quadratic linearization, ABDLOP response compression, strict proof
//! construction, verifier-side challenge reconstruction, and prover
//! self-verification.

use rand_core_06::{CryptoRng, RngCore};
use thiserror::Error;
use zeroize::Zeroize;

use super::{
    bounds::{ResponseBoundErrorV1, validate_public_response_bounds_v1},
    codec::{
        BootleLanternPresentationProofV1, H_POLYNOMIALS_V1, HINT_POLYNOMIALS_V1,
        PROOF_COEFFICIENTS_V1, ProofCodecErrorV1, T_A1_POLYNOMIALS_V1, T_B_POLYNOMIALS_V1,
        Z1_POLYNOMIALS_V1, Z3_POLYNOMIALS_V1, Z4_POLYNOMIALS_V1, Z21_POLYNOMIALS_V1,
    },
    compression::{
        CompressionErrorV1, gamma_decompose_v1, make_gamma_hint_v1, power2round_v1,
        use_gamma_hint_v1,
    },
    params::{
        APPLICATION_RELATION_QUOTIENT_BOUND_V1, APPLICATION_RING_DEGREE_V1, COMPRESSION_GAMMA_V1,
        DECOMPOSITION_BITS_V1, GAUSSIAN_1_VARIANCE_V1, GAUSSIAN_2_VARIANCE_V1,
        GAUSSIAN_3_VARIANCE_V1, GAUSSIAN_4_VARIANCE_V1, MAX_PROOF_SAMPLING_ATTEMPTS_V1,
        PROOF_INVERSE_TWO_V1, RESPONSE_NORM_SQUARED_BOUND_V1, TBOX_KMSIS_V1, TBOX_LEXT_V1,
        TBOX_M1_V1, TBOX_M2_V1, Z3_NORM_SQUARED_BOUND_V1, Z4_INFINITY_NORM_BOUND_V1,
    },
    relation::{BootleLanternApplicationRelationV1, BootleLanternPresentationWitnessV1},
    ring::ProofPolynomialV1,
    sampling::{ProofRandomnessV1, SamplingErrorV1},
    toolbox::{
        COMBINED_QUADRATIC_EQUATIONS_V1, EVALUATION_CONSTRAINTS_V1, InternalMatricesV1,
        PROJECTION_COORDINATES_V1, PROJECTION_POLYNOMIALS_V1, QUADRATIC_MESSAGE_POLYNOMIALS_V1,
        QuadraticEquationV1, QuadraticVariablesV1, S21_POLYNOMIALS_V1, SCHWARTZ_ACCUMULATORS_V1,
        ToolboxErrorV1, application_quotient_v1, application_relation_digest_v1,
        commit_extended_messages_v1, encode_polynomials_v1, expand_projection_matrix_v1,
        flatten_polynomials, lift_short_witness_v1, matrix_vector_product_v1,
        projected_norm_witness_v1,
    },
    transcript::PresentationTranscriptV1,
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
const GAMMA_HALF_V1: i64 = (COMPRESSION_GAMMA_V1 / 2) as i64;

struct SecretPolynomialVectorV1<const N: usize> {
    polynomials: [ProofPolynomialV1; N],
}

impl<const N: usize> SecretPolynomialVectorV1<N> {
    const fn zero() -> Self {
        Self {
            polynomials: [ProofPolynomialV1::ZERO; N],
        }
    }
}

impl<const N: usize> core::fmt::Debug for SecretPolynomialVectorV1<N> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("SecretPolynomialVectorV1(<redacted>)")
    }
}

impl<const N: usize> Drop for SecretPolynomialVectorV1<N> {
    fn drop(&mut self) {
        self.polynomials.zeroize();
    }
}

struct ProjectionProofV1 {
    projection_r: Box<[i8]>,
    projection_r_prime: Box<[i8]>,
    z3: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
    z4: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
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
pub fn prove_presentation_v1<R: CryptoRng + RngCore>(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
    transcript: PresentationTranscriptV1,
    rng: &mut R,
) -> Result<BootleLanternPresentationProofV1, PresentationProofErrorV1> {
    require_relation_digest(relation, transcript)?;
    let short =
        lift_short_witness_v1(relation, witness).map_err(PresentationProofErrorV1::Toolbox)?;
    let mut randomness =
        ProofRandomnessV1::from_rng(rng).map_err(PresentationProofErrorV1::Sampling)?;
    let matrices =
        InternalMatricesV1::expand(&transcript).map_err(PresentationProofErrorV1::Toolbox)?;

    for _ in 0..MAX_PROOF_SAMPLING_ATTEMPTS_V1 {
        if let Some(proof) =
            prove_attempt(relation, &short, transcript, &matrices, &mut randomness)?
        {
            verify_presentation_v1(relation, transcript, &proof)
                .map_err(|_| PresentationProofErrorV1::ProverSelfCheckFailed)?;
            return Ok(proof);
        }
    }
    Err(PresentationProofErrorV1::ProofSamplingExhausted)
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
        z3,
        z4,
        h,
        weights,
        multipliers,
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;

    let recovered_w1 = recover_gamma_high(&matrices, &z1, &z21, &t_a1, challenge, &hint)?;
    validate_compressed_response_bound(&matrices, &z1, &z21, &t_a1, challenge, &recovered_w1)?;

    let b_z21 = matrix_vector_product_v1(&matrices.b_prime, &z21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let variables = QuadraticVariablesV1 {
        short: z1,
        message: core::array::from_fn(|index| challenge.multiply(t_b[index]).sub(b_z21[index])),
    };
    let f = challenge
        .multiply(t_b[LINEARIZATION_MESSAGE_INDEX_V1])
        .sub(b_z21[LINEARIZATION_MESSAGE_INDEX_V1]);
    let q0 = equation
        .evaluate(&QuadraticVariablesV1::zero())
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let q2_z = quadratic_part(&equation, &variables, q0)?;
    let linear_z = linear_part(&equation, &variables)?;
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
    transcript: PresentationTranscriptV1,
    matrices: &InternalMatricesV1,
    randomness: &mut ProofRandomnessV1,
) -> Result<Option<BootleLanternPresentationProofV1>, PresentationProofErrorV1> {
    let mut s2 = sample_ternary_vector::<TBOX_M2_V1>(randomness, b"s2");
    let (t_a1, mut t_a2) = commit_short_witness(matrices, short.polynomials(), &s2.polynomials)?;
    let mut messages = SecretPolynomialVectorV1::<TBOX_LEXT_V1>::zero();

    let projection = match prove_projected_responses(
        relation,
        short.polynomials(),
        transcript,
        matrices,
        &s2.polynomials,
        &mut messages.polynomials,
        randomness,
    )? {
        Some(projection) => projection,
        None => {
            t_a2.zeroize();
            return Ok(None);
        }
    };

    messages.polynomials[G_MESSAGE_START_V1] = sample_uniform_g(randomness, b"schwartz-g0")?;
    messages.polynomials[G_MESSAGE_START_V1 + 1] = sample_uniform_g(randomness, b"schwartz-g1")?;
    let mut t_b = commit_extended_messages_v1(
        &matrices.b_prime,
        array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&s2.polynomials),
        &messages.polynomials,
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;

    let weights = derive_schwartz_weights(transcript, &t_b)?;
    let variables = QuadraticVariablesV1 {
        short: *short.polynomials(),
        message: core::array::from_fn(|index| messages.polynomials[index]),
    };
    let z3 = projection.z3;
    let z4 = projection.z4;
    let mut equation = QuadraticEquationV1::new(
        relation,
        projection.projection_r,
        projection.projection_r_prime,
        z3,
        z4,
        [ProofPolynomialV1::ZERO; 2],
        weights,
        [ProofPolynomialV1::ZERO; COMBINED_QUADRATIC_EQUATIONS_V1],
    )
    .map_err(PresentationProofErrorV1::Toolbox)?;
    let schwartz = equation
        .schwartz_polynomials(&variables)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let h = [
        messages.polynomials[G_MESSAGE_START_V1].add(schwartz[0]),
        messages.polynomials[G_MESSAGE_START_V1 + 1].add(schwartz[1]),
    ];
    require_schwartz_commitment_shape(&h)?;
    let multipliers = derive_equation_multipliers(transcript, &t_b, &h, &z3, &z4)?;
    equation.bind_final_equations(h, multipliers);
    if !equation
        .evaluate(&variables)
        .map_err(PresentationProofErrorV1::Toolbox)?
        .is_zero()
    {
        t_a2.zeroize();
        return Err(PresentationProofErrorV1::ConstraintSystemRejectedWitness);
    }

    for _ in 0..MAX_PROOF_SAMPLING_ATTEMPTS_V1 {
        let mut y1 = sample_gaussian_vector::<TBOX_M1_V1>(randomness, 23, 0, b"abdlop-y1")?;
        let mut y2 = sample_gaussian_vector::<TBOX_M2_V1>(randomness, 12, 1, b"abdlop-y2")?;
        let (mut t_candidate, v) = quadratic_linearization(
            &equation,
            &variables,
            matrices,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&s2.polynomials),
            &y1.polynomials,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&y2.polynomials),
        )?;
        t_b[LINEARIZATION_MESSAGE_INDEX_V1] = t_candidate;

        let (w1, w0) = decompose_mask_commitment(
            matrices,
            &y1.polynomials,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&y2.polynomials),
            array_suffix::<TBOX_KMSIS_V1, TBOX_M2_V1>(&y2.polynomials),
        )?;
        let pre_challenge = pre_challenge_wire(&t_b, &h, &t_a1, &z3, &z4, &w1, v)?;
        let challenge = transcript
            .derive_final_challenge(&pre_challenge)
            .map_err(|error| {
                PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error))
            })?;

        let c_short = multiply_vector_by_polynomial(short.polynomials(), challenge);
        let c_s2 = multiply_vector_by_polynomial(&s2.polynomials, challenge);
        let mut z1 = add_arrays(&y1.polynomials, &c_short);
        let mut z2 = add_arrays(&y2.polynomials, &c_s2);
        let z1_centered = centered_vector(&z1);
        let c_short_centered = centered_vector(&c_short);
        let z2_centered = centered_vector(&z2);
        let c_s2_centered = centered_vector(&c_s2);
        let accept_z1 = randomness
            .accept_standard(&z1_centered, &c_short_centered, 0, GAUSSIAN_1_VARIANCE_V1)
            .map_err(PresentationProofErrorV1::Sampling)?;
        let accept_z2 = randomness
            .accept_bimodal(&z2_centered, &c_s2_centered, 1, GAUSSIAN_2_VARIANCE_V1)
            .map_err(PresentationProofErrorV1::Sampling)?;
        if !accept_z1 || !accept_z2 {
            zeroize_response_attempt(&mut y1, &mut y2, &mut z1, &mut z2, t_candidate);
            continue;
        }

        let c_t_a2 = multiply_vector_by_polynomial(&t_a2, challenge);
        for index in 0..TBOX_KMSIS_V1 {
            let response_index = S21_POLYNOMIALS_V1 + index;
            z2[response_index] = z2[response_index].sub(c_t_a2[index]).sub(w0[index]);
        }
        if centered_squared_norm(&z2)? > u128::from(RESPONSE_NORM_SQUARED_BOUND_V1) {
            zeroize_response_attempt(&mut y1, &mut y2, &mut z1, &mut z2, t_candidate);
            continue;
        }
        let hint = match make_hint(&w1, array_suffix::<TBOX_KMSIS_V1, TBOX_M2_V1>(&z2))? {
            Some(hint) => hint,
            None => {
                zeroize_response_attempt(&mut y1, &mut y2, &mut z1, &mut z2, t_candidate);
                continue;
            }
        };

        let proof = construct_proof(
            &t_b,
            &h,
            &t_a1,
            challenge,
            &hint,
            &z1,
            array_prefix::<S21_POLYNOMIALS_V1, TBOX_M2_V1>(&z2),
            &z3,
            &z4,
        )?;
        validate_public_response_bounds_v1(&proof)
            .map_err(PresentationProofErrorV1::ResponseBound)?;
        y1.polynomials.zeroize();
        y2.polynomials.zeroize();
        z1.zeroize();
        z2.zeroize();
        t_candidate.zeroize();
        t_a2.zeroize();
        s2.polynomials.zeroize();
        messages.polynomials.zeroize();
        return Ok(Some(proof));
    }
    t_a2.zeroize();
    Ok(None)
}

fn prove_projected_responses(
    relation: &BootleLanternApplicationRelationV1,
    short: &[ProofPolynomialV1; TBOX_M1_V1],
    transcript: PresentationTranscriptV1,
    matrices: &InternalMatricesV1,
    s2: &[ProofPolynomialV1; TBOX_M2_V1],
    messages: &mut [ProofPolynomialV1; TBOX_LEXT_V1],
    randomness: &mut ProofRandomnessV1,
) -> Result<Option<ProjectionProofV1>, PresentationProofErrorV1> {
    let mut s3 = projected_norm_witness_v1(short);
    let mut s4 =
        application_quotient_v1(relation, short).map_err(PresentationProofErrorV1::Toolbox)?;
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
        s3.zeroize();
        s4.zeroize();
        return Err(PresentationProofErrorV1::ApplicationQuotientBoundExceeded);
    }
    let mut s3_coefficients = centered_vector(&s3);
    let mut s4_coefficients = centered_vector(&s4);

    for _ in 0..MAX_PROOF_SAMPLING_ATTEMPTS_V1 {
        let mut y3 =
            sample_gaussian_vector::<PROJECTION_POLYNOMIALS_V1>(randomness, 18, 2, b"z3-mask")?;
        let mut y4 =
            sample_gaussian_vector::<PROJECTION_POLYNOMIALS_V1>(randomness, 29, 3, b"z4-mask")?;
        let beta3 = randomness.sign(b"z3-sign");
        let beta4 = randomness.sign(b"z4-sign");
        messages[Y3_MESSAGE_START_V1..Y3_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1]
            .copy_from_slice(&y3.polynomials);
        messages[Y4_MESSAGE_START_V1..Y4_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1]
            .copy_from_slice(&y4.polynomials);
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
        let projected_s3 =
            project_centered(&projection_r, s3_coefficients.len(), &s3_coefficients)?;
        let projected_s4 =
            project_centered(&projection_r_prime, s4_coefficients.len(), &s4_coefficients)?;
        let mut z3_centered = centered_vector(&y3.polynomials);
        let mut z4_centered = centered_vector(&y4.polynomials);
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
        let z3 = polynomials_from_centered_projection(&z3_centered);
        let z4 = polynomials_from_centered_projection(&z4_centered);
        let accept_z3 = randomness
            .accept_bimodal(&z3_centered, &projected_s3, 2, GAUSSIAN_3_VARIANCE_V1)
            .map_err(PresentationProofErrorV1::Sampling)?;
        let accept_z4 = randomness
            .accept_bimodal(&z4_centered, &projected_s4, 3, GAUSSIAN_4_VARIANCE_V1)
            .map_err(PresentationProofErrorV1::Sampling)?;
        let z3_norm = centered_squared_norm(&z3)?;
        let z4_infinity = z4
            .iter()
            .flat_map(ProofPolynomialV1::coefficients)
            .enumerate()
            .map(|(index, residue)| {
                let polynomial = index / APPLICATION_RING_DEGREE_V1;
                let coefficient = index % APPLICATION_RING_DEGREE_V1;
                let _ = residue;
                z4[polynomial]
                    .centered_coefficient(coefficient)
                    .unsigned_abs()
            })
            .max()
            .unwrap_or(0);
        z3_centered.zeroize();
        z4_centered.zeroize();
        y3.polynomials.zeroize();
        y4.polynomials.zeroize();
        if accept_z3
            && accept_z4
            && z3_norm <= u128::from(Z3_NORM_SQUARED_BOUND_V1)
            && z4_infinity <= Z4_INFINITY_NORM_BOUND_V1
        {
            s3.zeroize();
            s4.zeroize();
            s3_coefficients.zeroize();
            s4_coefficients.zeroize();
            return Ok(Some(ProjectionProofV1 {
                projection_r,
                projection_r_prime,
                z3,
                z4,
            }));
        }
    }
    s3.zeroize();
    s4.zeroize();
    s3_coefficients.zeroize();
    s4_coefficients.zeroize();
    Ok(None)
}

fn derive_projection_matrices(
    transcript: PresentationTranscriptV1,
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
    transcript: PresentationTranscriptV1,
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
    transcript: PresentationTranscriptV1,
    t_b: &[ProofPolynomialV1; TBOX_LEXT_V1],
    h: &[ProofPolynomialV1; H_POLYNOMIALS_V1],
    z3: &[ProofPolynomialV1; Z3_POLYNOMIALS_V1],
    z4: &[ProofPolynomialV1; Z4_POLYNOMIALS_V1],
) -> Result<[ProofPolynomialV1; COMBINED_QUADRATIC_EQUATIONS_V1], PresentationProofErrorV1> {
    let t_b_wire = encode_polynomials_v1(&t_b[..QUADRATIC_MESSAGE_POLYNOMIALS_V1]);
    let h_wire = encode_polynomials_v1(h);
    let z3_wire = encode_polynomials_v1(z3);
    let z4_wire = encode_polynomials_v1(z4);
    transcript
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
        .map_err(|error| PresentationProofErrorV1::Toolbox(ToolboxErrorV1::Transcript(error)))?
        .try_into()
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn commit_short_witness(
    matrices: &InternalMatricesV1,
    short: &[ProofPolynomialV1; TBOX_M1_V1],
    s2: &[ProofPolynomialV1; TBOX_M2_V1],
) -> Result<
    (
        [ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
        [ProofPolynomialV1; T_A1_POLYNOMIALS_V1],
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
    let mut high = [ProofPolynomialV1::ZERO; T_A1_POLYNOMIALS_V1];
    let mut low = [ProofPolynomialV1::ZERO; T_A1_POLYNOMIALS_V1];
    for row in 0..T_A1_POLYNOMIALS_V1 {
        let commitment = a1_short[row].add(a2_s21[row]).add(s22[row]);
        let mut high_coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        let mut low_coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let rounded = power2round_v1(commitment.coefficients()[index])
                .map_err(PresentationProofErrorV1::Compression)?;
            high_coefficients[index] = rounded.high;
            low_coefficients[index] = rounded.low;
        }
        high[row] = ProofPolynomialV1::new(high_coefficients)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        low[row] = ProofPolynomialV1::from_centered_coefficients(low_coefficients);
    }
    Ok((high, low))
}

fn quadratic_linearization(
    equation: &QuadraticEquationV1<'_>,
    secret: &QuadraticVariablesV1,
    matrices: &InternalMatricesV1,
    s21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
    y1: &[ProofPolynomialV1; TBOX_M1_V1],
    y21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
) -> Result<(ProofPolynomialV1, ProofPolynomialV1), PresentationProofErrorV1> {
    let b_y21 = matrix_vector_product_v1(&matrices.b_prime, y21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let mask = QuadraticVariablesV1 {
        short: *y1,
        message: core::array::from_fn(|index| b_y21[index].negate()),
    };
    let q0 = equation
        .evaluate(&QuadraticVariablesV1::zero())
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let q_secret = equation
        .evaluate(secret)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    if !q_secret.is_zero() {
        return Err(PresentationProofErrorV1::ConstraintSystemRejectedWitness);
    }
    let q_mask = equation
        .evaluate(&mask)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let q_sum = equation
        .evaluate(&secret.add(&mask))
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let bilinear = q_sum.sub(q_secret).sub(q_mask).add(q0);
    let linear_mask = linear_part(equation, &mask)?;
    let g1 = bilinear.add(linear_mask);
    let b_s21 = matrix_vector_product_v1(&matrices.b_prime, s21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let t = b_s21[LINEARIZATION_MESSAGE_INDEX_V1].add(g1);
    let v = quadratic_part(equation, &mask, q0)?.add(b_y21[LINEARIZATION_MESSAGE_INDEX_V1]);
    Ok((t, v))
}

fn quadratic_part(
    equation: &QuadraticEquationV1<'_>,
    variables: &QuadraticVariablesV1,
    q0: ProofPolynomialV1,
) -> Result<ProofPolynomialV1, PresentationProofErrorV1> {
    equation
        .evaluate(variables)
        .map_err(PresentationProofErrorV1::Toolbox)?
        .add(
            equation
                .evaluate(&variables.negate())
                .map_err(PresentationProofErrorV1::Toolbox)?,
        )
        .sub(q0.scale_centered(2))
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn linear_part(
    equation: &QuadraticEquationV1<'_>,
    variables: &QuadraticVariablesV1,
) -> Result<ProofPolynomialV1, PresentationProofErrorV1> {
    equation
        .evaluate(variables)
        .map_err(PresentationProofErrorV1::Toolbox)?
        .sub(
            equation
                .evaluate(&variables.negate())
                .map_err(PresentationProofErrorV1::Toolbox)?,
        )
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| PresentationProofErrorV1::InternalInvariant)
}

fn decompose_mask_commitment(
    matrices: &InternalMatricesV1,
    y1: &[ProofPolynomialV1; TBOX_M1_V1],
    y21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
    y22: &[ProofPolynomialV1; TBOX_KMSIS_V1],
) -> Result<
    (
        [ProofPolynomialV1; TBOX_KMSIS_V1],
        [ProofPolynomialV1; TBOX_KMSIS_V1],
    ),
    PresentationProofErrorV1,
> {
    let a1_y1 =
        matrix_vector_product_v1(&matrices.a1, y1).map_err(PresentationProofErrorV1::Toolbox)?;
    let a2_y21 = matrix_vector_product_v1(&matrices.a2_prime, y21)
        .map_err(PresentationProofErrorV1::Toolbox)?;
    let mut high = [ProofPolynomialV1::ZERO; TBOX_KMSIS_V1];
    let mut low = [ProofPolynomialV1::ZERO; TBOX_KMSIS_V1];
    for row in 0..TBOX_KMSIS_V1 {
        let commitment = a1_y1[row].add(a2_y21[row]).add(y22[row]);
        let mut high_coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        let mut low_coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let decomposition = gamma_decompose_v1(commitment.coefficients()[index])
                .map_err(PresentationProofErrorV1::Compression)?;
            high_coefficients[index] = decomposition.high;
            low_coefficients[index] = decomposition.low;
        }
        high[row] = ProofPolynomialV1::new(high_coefficients)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        low[row] = ProofPolynomialV1::from_centered_coefficients(low_coefficients);
    }
    Ok((high, low))
}

fn make_hint(
    w1: &[ProofPolynomialV1; TBOX_KMSIS_V1],
    adjusted_z22: &[ProofPolynomialV1; TBOX_KMSIS_V1],
) -> Result<Option<[ProofPolynomialV1; HINT_POLYNOMIALS_V1]>, PresentationProofErrorV1> {
    let mut hints = [ProofPolynomialV1::ZERO; HINT_POLYNOMIALS_V1];
    for row in 0..TBOX_KMSIS_V1 {
        let gamma_high = w1[row]
            .scale_canonical(COMPRESSION_GAMMA_V1)
            .map_err(|_| PresentationProofErrorV1::InternalInvariant)?;
        let base = gamma_high.sub(adjusted_z22[row]);
        let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            let correction = adjusted_z22[row].centered_coefficient(index);
            if !(-GAMMA_HALF_V1..=GAMMA_HALF_V1).contains(&correction) {
                hints.zeroize();
                return Ok(None);
            }
            let hint = make_gamma_hint_v1(base.coefficients()[index], correction)
                .map_err(PresentationProofErrorV1::Compression)?;
            let recovered = use_gamma_hint_v1(base.coefficients()[index], hint)
                .map_err(PresentationProofErrorV1::Compression)?;
            if recovered != w1[row].coefficients()[index] {
                hints.zeroize();
                return Err(PresentationProofErrorV1::InternalInvariant);
            }
            coefficients[index] = hint;
        }
        hints[row] = ProofPolynomialV1::from_centered_coefficients(coefficients);
    }
    Ok(Some(hints))
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
    transcript: PresentationTranscriptV1,
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
) -> SecretPolynomialVectorV1<N> {
    let mut output = SecretPolynomialVectorV1::zero();
    for polynomial in &mut output.polynomials {
        *polynomial = randomness.ternary_polynomial(domain);
    }
    output
}

fn sample_gaussian_vector<const N: usize>(
    randomness: &mut ProofRandomnessV1,
    log2_sigma: u8,
    parameter_index: usize,
    domain: &[u8],
) -> Result<SecretPolynomialVectorV1<N>, PresentationProofErrorV1> {
    let mut output = SecretPolynomialVectorV1::zero();
    for polynomial in &mut output.polynomials {
        *polynomial = randomness
            .gaussian_polynomial(log2_sigma, parameter_index, domain)
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
) -> Result<Vec<i64>, PresentationProofErrorV1> {
    if columns == 0
        || vector.len() != columns
        || matrix.len()
            != PROJECTION_COORDINATES_V1
                .checked_mul(columns)
                .ok_or(PresentationProofErrorV1::ArithmeticOverflow)?
    {
        return Err(PresentationProofErrorV1::InternalInvariant);
    }
    let mut output = vec![0_i64; PROJECTION_COORDINATES_V1];
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
) -> [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1] {
    debug_assert_eq!(coefficients.len(), PROJECTION_COORDINATES_V1);
    core::array::from_fn(|polynomial| {
        let start = polynomial * APPLICATION_RING_DEGREE_V1;
        let mut array = [0_i64; APPLICATION_RING_DEGREE_V1];
        array.copy_from_slice(&coefficients[start..start + APPLICATION_RING_DEGREE_V1]);
        ProofPolynomialV1::from_centered_coefficients(array)
    })
}

fn multiply_vector_by_polynomial<const N: usize>(
    vector: &[ProofPolynomialV1; N],
    scalar: ProofPolynomialV1,
) -> [ProofPolynomialV1; N] {
    vector.map(|polynomial| scalar.multiply(polynomial))
}

fn add_arrays<const N: usize>(
    lhs: &[ProofPolynomialV1; N],
    rhs: &[ProofPolynomialV1; N],
) -> [ProofPolynomialV1; N] {
    core::array::from_fn(|index| lhs[index].add(rhs[index]))
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
) -> Result<[ProofPolynomialV1; N], PresentationProofErrorV1> {
    let mut output = Vec::with_capacity(N);
    for index in 0..N {
        output.push(polynomial(index).ok_or(PresentationProofErrorV1::MalformedProof)?);
    }
    output
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

fn zeroize_response_attempt<const N1: usize, const N2: usize>(
    y1: &mut SecretPolynomialVectorV1<N1>,
    y2: &mut SecretPolynomialVectorV1<N2>,
    z1: &mut [ProofPolynomialV1; N1],
    z2: &mut [ProofPolynomialV1; N2],
    mut t: ProofPolynomialV1,
) {
    y1.polynomials.zeroize();
    y2.polynomials.zeroize();
    z1.zeroize();
    z2.zeroize();
    t.zeroize();
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
    /// All bounded whole-proof attempts rejected.
    #[error("Bootle/Lantern presentation proof sampling exhausted its fixed work bound")]
    ProofSamplingExhausted,
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

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        BootleLanternAllowedAttributeValuesV1, BootleLanternAttributeValueV1,
        BootleLanternDisclosedAttributeV1, BootleLanternIssuerPolicyV1,
        BootleLanternIssuerPublicMatrixV1, BootleLanternPolynomialV1,
        IrohaBootleLanternAnoncredStatementV1, PrivacyBootleLanternIssuerPolicyDigestV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    };
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};

    use super::*;
    use crate::privacy_engines::bootle_lantern::{
        compression::proof_residue_from_centered_v1,
        params::{
            APPLICATION_MODULUS_V1, CHALLENGE_OMEGA_V1, COMPRESSION_MODULUS_V1, PROOF_MODULUS_V1,
            Z4_INFINITY_NORM_BOUND_V1,
        },
        relation::{compile_application_relation_v1, validate_presentation_witness_v1},
        ring::ApplicationPolynomialV1,
        transcript::{
            MatrixRoleV1, MatrixSeedV1, PresentationChallengeBindingV1,
            expand_application_matrix_v1,
        },
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
    }

    impl TestRng {
        const fn healthy(seed: u64) -> Self {
            Self {
                state: seed,
                fail: false,
                stuck: None,
            }
        }

        const fn failed() -> Self {
            Self {
                state: 1,
                fail: true,
                stuck: None,
            }
        }

        const fn stuck(byte: u8) -> Self {
            Self {
                state: 1,
                fail: false,
                stuck: Some(byte),
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
        relation: BootleLanternApplicationRelationV1,
        witness: BootleLanternPresentationWitnessV1,
        transcript: PresentationTranscriptV1,
    }

    fn raw(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn matrix_seed() -> MatrixSeedV1 {
        MatrixSeedV1::new([0x31; 32], [0x72; 32]).expect("valid non-zero matrix seed")
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

    fn issuer_policy() -> BootleLanternIssuerPolicyV1 {
        // This algebraic fixture sets B2=A_m, allowing the direct attributes
        // to serve as s2. It exercises the proof system only; production
        // issuer key generation and preimage sampling have separate tests.
        let attributes =
            expand_application_matrix_v1(matrix_seed(), MatrixRoleV1::ApplicationAttributes)
                .expect("application attribute matrix");
        let entries = attributes
            .entries()
            .iter()
            .map(|polynomial| BootleLanternPolynomialV1 {
                coefficients: polynomial.coefficients().to_vec(),
            })
            .collect();
        let mut policy = BootleLanternIssuerPolicyV1 {
            issuer_id: PrivacyIssuerIdV1::new(raw(11)),
            policy_id: PrivacyPolicyIdV1::new(raw(12)),
            epoch: 1,
            issuer_parameter_id: PrivacyParameterIdV1::new(raw(13)),
            issuer_parameter_digest: PrivacyParameterDigestV1::new(raw(14)),
            issuer_public_matrix: BootleLanternIssuerPublicMatrixV1 { entries },
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
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        policy.record_digest = policy.computed_record_digest().expect("policy digest");
        policy.validate().expect("valid issuer policy");
        policy
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

    fn valid_witness() -> BootleLanternPresentationWitnessV1 {
        let mut attributes = [[0_u8; 8]; 8];
        attributes[1] = [1; 8];
        let mut signature_two = [ApplicationPolynomialV1::ZERO; 8];
        for (output, attribute) in signature_two.iter_mut().zip(attributes) {
            *output = ApplicationPolynomialV1::from_direct_attribute(attribute);
        }
        BootleLanternPresentationWitnessV1 {
            randomness: [ApplicationPolynomialV1::ZERO; 16],
            tag: [ApplicationPolynomialV1::ZERO; 8],
            signature_one: [ApplicationPolynomialV1::ZERO; 8],
            signature_two,
            attributes,
        }
    }

    fn fixture() -> Fixture {
        let policy = issuer_policy();
        let relation = compile_application_relation_v1(&statement(&policy), &policy, matrix_seed())
            .expect("compiled application relation");
        let witness = valid_witness();
        validate_presentation_witness_v1(&relation, &witness).expect("valid presentation witness");
        let transcript = PresentationTranscriptV1::new(
            PresentationChallengeBindingV1 {
                parameter_digest: [0x31; 32],
                statement_digest: [0x41; 32],
                issuer_policy_record_digest: [0x42; 32],
                transaction_intent_digest: [0x43; 32],
            },
            matrix_seed(),
            application_relation_digest_v1(&relation),
        )
        .expect("fully bound presentation transcript");
        Fixture {
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
    fn complete_native_proof_round_trip_and_adversarial_matrix() {
        let fixture = fixture();
        let mut rng = TestRng::healthy(0x9e37_79b9_7f4a_7c15);
        let proof = prove_presentation_v1(
            &fixture.relation,
            &fixture.witness,
            fixture.transcript,
            &mut rng,
        )
        .expect("native proof");
        verify_presentation_v1(&fixture.relation, fixture.transcript, &proof)
            .expect("native proof verifies");

        let encoded = proof.encode();
        let decoded = BootleLanternPresentationProofV1::decode_exact(
            &encoded,
            u32::try_from(encoded.len()).expect("proof length fits u32"),
        )
        .expect("strict wire round trip");
        assert_eq!(decoded, proof);
        verify_presentation_v1(&fixture.relation, fixture.transcript, &decoded)
            .expect("decoded native proof verifies");

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
        for mut rng in [TestRng::failed(), TestRng::stuck(0), TestRng::stuck(0x7f)] {
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
