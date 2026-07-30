//! Fixed-profile Jindo commitment, batched opening, and verification.
//!
//! This is a clean-room implementation of Figures 1--5 of ePrint 2026/044,
//! including the paper's generalized sixteen-slot CELPC encoding.  The
//! transcript and wire are Iroha-specific, closed, and versioned.

use iroha_data_model::privacy::{
    IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1, IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1,
    IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1, IrohaJindoPolynomialCommitmentStatementV1,
    PrivacyJindoFieldElementV1, PrivacyJindoLatticeCommitmentV1, PrivacyStatementV1,
};
use rand_core_06::{CryptoRng, RngCore};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::privacy_engines::p256::TranscriptBindingV1;

use super::{
    JINDO_ENCODING_SLOTS_V1, JINDO_MAX_COEFFICIENTS_V1, JINDO_RING_DEGREE_V1,
    JindoCanonicalPolynomialErrorV1,
    codec::{JINDO_PROOF_BYTES_V1, JindoEvaluationProofV1, JindoProofCodecErrorV1},
    crs::{commit_key_v1, crs_digest_v1},
    encoding::{decode_coefficient_slots_v1, encode_coefficient_slots_v1},
    field::JindoFieldElementV1,
    norm::{
        JINDO_DECOMPOSED_NORM_SQUARED_BOUND_V1, JINDO_RESPONSE_NORM_SQUARED_BOUND_V1,
        two_norm_squared_is_below_v1,
    },
    parameters::{JINDO_PARAMETER_MANIFEST_V1, JINDO_PARAMETERS_V1, JindoGaussianWidthV1},
    ring::{
        JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoPrimeModulusV1, JindoRnsPolynomialV1,
    },
    sampling::{
        JindoSamplingErrorV1, health_checked_jindo_rng_v1, randomized_encode_coefficient_slots_v1,
        sample_gaussian_polynomial_v1, sample_uniform_field_element_v1,
    },
    transcript::{JindoTranscriptErrorV1, JindoTranscriptV1},
    validate_canonical_polynomial_v1,
};

/// Exact native proof byte width for this Jindo profile.
pub const JINDO_NATIVE_PROOF_BYTES_V1: usize = JINDO_PROOF_BYTES_V1;
/// Exact clean-room source profile implemented by this engine.
pub const JINDO_SOURCE_PROFILE_V1: &[u8] = b"eprint-2026-044-v1-figures-1-5";
/// Exact native proof suite.
pub const JINDO_SUITE_V1: &[u8] = b"iroha-jindo-batched-univariate-opening-v1";

/// Opaque opening produced together with one public commitment.
#[derive(Clone)]
pub struct JindoOpeningV1 {
    polynomial: Zeroizing<Vec<JindoFieldElementV1>>,
    encoded_columns: Zeroizing<Vec<Vec<JindoRnsPolynomialV1>>>,
    mlwe_columns: Zeroizing<Vec<Vec<JindoRnsPolynomialV1>>>,
    rounded_inner_commitments: Zeroizing<Vec<JindoRnsPolynomialV1>>,
    commitment_encoding: Vec<u8>,
}

impl core::fmt::Debug for JindoOpeningV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("JindoOpeningV1([REDACTED])")
    }
}

impl Drop for JindoOpeningV1 {
    fn drop(&mut self) {
        self.commitment_encoding.zeroize();
    }
}

/// Consensus binding field selected by a Jindo diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JindoBindingFieldV1 {
    /// Exact chain identifier.
    ChainId,
    /// Zero-based privacy action index.
    ActionIndex,
    /// Digest of the complete typed public statement.
    StatementDigest,
    /// Governed parameter-set identifier.
    ParameterId,
    /// Digest of the governed parameter set.
    ParameterDigest,
    /// Digest of the native verifier artifact.
    VerifierDigest,
    /// Digest of the typed statement schema.
    StatementSchemaDigest,
    /// Digest of the admitted native engine manifest.
    EngineManifestDigest,
    /// Digest of the transparent commitment matrices.
    CrsDigest,
}

/// Native Jindo failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JindoErrorV1 {
    /// The statement or witness contains no polynomial or exceeds the fixed batch.
    #[error("Jindo polynomial count {count} is outside 1..={max}")]
    InvalidPolynomialCount {
        /// Observed polynomial count.
        count: usize,
        /// Compiled first-release maximum.
        max: usize,
    },
    /// A polynomial used the empty vector instead of the unique `[0]` zero
    /// polynomial encoding.
    #[error("Jindo polynomial {index} is empty")]
    EmptyPolynomial {
        /// Zero-based polynomial index.
        index: usize,
    },
    /// A witness polynomial exceeds the fixed degree bound.
    #[error("Jindo polynomial {index} has {count} coefficients; maximum is {max}")]
    PolynomialTooLarge {
        /// Zero-based polynomial index.
        index: usize,
        /// Observed coefficient count.
        count: usize,
        /// Compiled first-release maximum.
        max: usize,
    },
    /// A witness coefficient is not the unique little-endian field encoding.
    #[error("Jindo polynomial {polynomial_index} coefficient {coefficient_index} is non-canonical")]
    NonCanonicalCoefficient {
        /// Zero-based polynomial index.
        polynomial_index: usize,
        /// Zero-based coefficient index.
        coefficient_index: usize,
    },
    /// A polynomial used a redundant high zero coefficient.
    #[error("Jindo polynomial {index} has a trailing zero coefficient")]
    TrailingZeroCoefficient {
        /// Zero-based polynomial index.
        index: usize,
    },
    /// The evaluation point is not a canonical coefficient-field element.
    #[error("Jindo statement evaluation point is non-canonical")]
    NonCanonicalEvaluationPoint,
    /// A claimed evaluation is not a canonical coefficient-field element.
    #[error("Jindo statement claim {index} is non-canonical")]
    NonCanonicalClaim {
        /// Zero-based claim index.
        index: usize,
    },
    /// The public claim and commitment vectors have different lengths.
    #[error("Jindo claimed-evaluation count differs from polynomial count")]
    ClaimCountMismatch,
    /// The supplied secret-opening and commitment vectors have different lengths.
    #[error("Jindo opening count differs from polynomial count")]
    OpeningCountMismatch,
    /// A secret opening does not belong to its witness and public commitment.
    #[error("Jindo opening {index} belongs to another polynomial or commitment")]
    OpeningMismatch {
        /// Zero-based opening index.
        index: usize,
    },
    /// A supplied witness does not evaluate to its public claim.
    #[error("Jindo witness polynomial {index} does not evaluate to its public claim")]
    ClaimMismatch {
        /// Zero-based witness index.
        index: usize,
    },
    /// A public commitment has the wrong width or an out-of-range coefficient.
    #[error("Jindo commitment {index} has a malformed fixed encoding")]
    InvalidCommitmentEncoding {
        /// Zero-based commitment index.
        index: usize,
    },
    /// The all-zero public commitment sentinel was supplied.
    #[error("Jindo commitment {index} is the all-zero sentinel")]
    ZeroCommitment {
        /// Zero-based commitment index.
        index: usize,
    },
    /// A public commitment repeats an earlier batch member.
    #[error("Jindo commitment {index} duplicates an earlier commitment")]
    DuplicateCommitment {
        /// Zero-based duplicate commitment index.
        index: usize,
    },
    /// The statement omitted the mandatory transaction-intent binding.
    #[error("Jindo transaction-intent digest must be non-zero")]
    ZeroTransactionIntentDigest,
    /// The first-release one-action transaction profile was violated.
    #[error("Jindo first-release action index must be zero, got {index}")]
    InvalidActionIndex {
        /// Observed action index.
        index: u32,
    },
    /// Commitment rounding produced a coefficient outside the canonical wire.
    #[error("Jindo rounded commitment coefficient cannot fit the canonical wire")]
    RoundedCommitmentOutOfRange,
    /// A runtime transcript binding differs from the typed statement.
    #[error("Jindo consensus binding mismatches {field:?}")]
    BindingMismatch {
        /// Exact mismatching binding field.
        field: JindoBindingFieldV1,
    },
    /// Canonical statement hashing failed.
    #[error("Jindo statement digest could not be canonically encoded")]
    StatementEncoding,
    /// Prover-only bounded randomness generation failed.
    #[error("Jindo prover randomness failed: {0}")]
    Sampling(JindoSamplingErrorV1),
    /// Canonical Fiat--Shamir transcript construction failed.
    #[error("Jindo transcript failed: {0}")]
    Transcript(JindoTranscriptErrorV1),
    /// Strict fixed-width proof decoding failed.
    #[error("Jindo proof encoding failed: {0}")]
    ProofCodec(JindoProofCodecErrorV1),
    /// The outer rounded lattice commitment or its strict norm check failed.
    #[error("Jindo outer commitment relation failed")]
    OuterCommitmentRelation,
    /// The inner rounded lattice commitment or its strict norm check failed.
    #[error("Jindo inner commitment relation failed")]
    InnerCommitmentRelation,
    /// The encoded evaluation response is inconsistent with its partials.
    #[error("Jindo evaluation-response consistency relation failed")]
    EvaluationConsistency,
    /// The decoded partials do not equal the claimed batched evaluation.
    #[error("Jindo claimed polynomial evaluation relation failed")]
    EvaluationRelation,
    /// A locally produced proof failed the same public verifier.
    #[error("Jindo prover produced a proof rejected by its own verifier")]
    ProverSelfCheck,
}

impl From<JindoSamplingErrorV1> for JindoErrorV1 {
    fn from(value: JindoSamplingErrorV1) -> Self {
        Self::Sampling(value)
    }
}

impl From<JindoCanonicalPolynomialErrorV1> for JindoErrorV1 {
    fn from(error: JindoCanonicalPolynomialErrorV1) -> Self {
        match error {
            JindoCanonicalPolynomialErrorV1::Empty { polynomial_index } => Self::EmptyPolynomial {
                index: polynomial_index,
            },
            JindoCanonicalPolynomialErrorV1::TooLarge {
                polynomial_index,
                count,
            } => Self::PolynomialTooLarge {
                index: polynomial_index,
                count,
                max: JINDO_MAX_COEFFICIENTS_V1,
            },
            JindoCanonicalPolynomialErrorV1::NonCanonicalCoefficient {
                polynomial_index,
                coefficient_index,
            } => Self::NonCanonicalCoefficient {
                polynomial_index,
                coefficient_index,
            },
            JindoCanonicalPolynomialErrorV1::TrailingZeroCoefficient { polynomial_index } => {
                Self::TrailingZeroCoefficient {
                    index: polynomial_index,
                }
            }
        }
    }
}

impl From<JindoTranscriptErrorV1> for JindoErrorV1 {
    fn from(value: JindoTranscriptErrorV1) -> Self {
        Self::Transcript(value)
    }
}

impl From<JindoProofCodecErrorV1> for JindoErrorV1 {
    fn from(value: JindoProofCodecErrorV1) -> Self {
        Self::ProofCodec(value)
    }
}

/// Return the digest of the exact transparent CRS matrices.
#[must_use]
pub fn jindo_crs_digest_v1() -> [u8; 32] {
    crs_digest_v1()
}

/// Evaluate one degree-bounded polynomial at a canonical field point.
///
/// # Errors
///
/// Returns an error for an empty, oversized, trailing-zero, or non-canonical
/// polynomial, or for a non-canonical evaluation point.
pub fn evaluate_polynomial_v1(
    coefficients: &[PrivacyJindoFieldElementV1],
    evaluation_point: PrivacyJindoFieldElementV1,
) -> Result<PrivacyJindoFieldElementV1, JindoErrorV1> {
    let polynomial = parse_polynomial(coefficients, 0)?;
    let point = JindoFieldElementV1::from_canonical_bytes(evaluation_point.encoding)
        .ok_or(JindoErrorV1::NonCanonicalEvaluationPoint)?;
    Ok(PrivacyJindoFieldElementV1::new(
        evaluate_polynomial(&polynomial, point).to_canonical_bytes(),
    ))
}

/// Commit one canonically encoded degree-bounded polynomial with fresh
/// evaluation-hiding randomness.
pub fn commit_polynomial_v1<R>(
    coefficients: &[PrivacyJindoFieldElementV1],
    rng: &mut R,
) -> Result<(PrivacyJindoLatticeCommitmentV1, JindoOpeningV1), JindoErrorV1>
where
    R: CryptoRng + RngCore,
{
    let polynomial = parse_polynomial(coefficients, 0)?;
    let mut checked_rng = health_checked_jindo_rng_v1(rng)?;
    commit_parsed_polynomial_v1(polynomial, &mut checked_rng)
}

pub(super) fn commit_polynomial_with_checked_rng_v1<R>(
    coefficients: &[PrivacyJindoFieldElementV1],
    rng: &mut R,
) -> Result<(PrivacyJindoLatticeCommitmentV1, JindoOpeningV1), JindoErrorV1>
where
    R: CryptoRng + RngCore,
{
    let polynomial = parse_polynomial(coefficients, 0)?;
    commit_parsed_polynomial_v1(polynomial, rng)
}

fn commit_parsed_polynomial_v1<R>(
    polynomial: Zeroizing<Vec<JindoFieldElementV1>>,
    rng: &mut R,
) -> Result<(PrivacyJindoLatticeCommitmentV1, JindoOpeningV1), JindoErrorV1>
where
    R: CryptoRng + RngCore,
{
    let (first_row, last_row) = split_boundary_rows(&polynomial, rng)?;

    let mut encoded_columns = Zeroizing::new(vec![
        Vec::with_capacity(JINDO_PARAMETERS_V1.rows),
        Vec::with_capacity(JINDO_PARAMETERS_V1.rows),
    ]);
    encoded_columns[0].push(randomized_encode_coefficient_slots_v1(
        first_row.as_ref(),
        JindoGaussianWidthV1::EcdBlind,
        rng,
    )?);
    for row in 1..(JINDO_PARAMETERS_V1.rows - 1) {
        let start = row * JINDO_ENCODING_SLOTS_V1;
        let end = start + JINDO_ENCODING_SLOTS_V1;
        encoded_columns[0].push(randomized_encode_coefficient_slots_v1(
            &polynomial[start..end],
            JindoGaussianWidthV1::Ecd,
            rng,
        )?);
    }
    encoded_columns[0].push(randomized_encode_coefficient_slots_v1(
        last_row.as_ref(),
        JindoGaussianWidthV1::Ecd,
        rng,
    )?);

    for row in 0..JINDO_PARAMETERS_V1.rows {
        let mut mask = Zeroizing::new([JindoFieldElementV1::ZERO; JINDO_ENCODING_SLOTS_V1]);
        for value in mask.iter_mut() {
            *value = sample_uniform_field_element_v1(rng)?;
        }
        let width = if row == 0 {
            JindoGaussianWidthV1::MaskBlind
        } else {
            JindoGaussianWidthV1::Mask
        };
        encoded_columns[1].push(randomized_encode_coefficient_slots_v1(
            mask.as_ref(),
            width,
            rng,
        )?);
    }

    let mut mlwe_columns = Zeroizing::new(Vec::with_capacity(JINDO_PARAMETERS_V1.columns + 1));
    for column in 0..=JINDO_PARAMETERS_V1.columns {
        let width = if column == JINDO_PARAMETERS_V1.columns {
            JindoGaussianWidthV1::MaskMlwe
        } else {
            JindoGaussianWidthV1::Mlwe
        };
        let value_count = JINDO_PARAMETERS_V1.mlwe_rank + JINDO_PARAMETERS_V1.inner_msis_rank;
        mlwe_columns.push(Vec::with_capacity(value_count));
        for _ in 0..value_count {
            mlwe_columns[column].push(sample_gaussian_polynomial_v1(
                width,
                JINDO_INNER_MODULI_V1,
                rng,
            )?);
        }
    }

    let rounded_inner_commitments = Zeroizing::new(compute_rounded_inner_commitments(
        &encoded_columns,
        &mlwe_columns,
    ));
    let public_coefficients = compute_public_commitment(&rounded_inner_commitments)?;
    let commitment_encoding = encode_public_commitment(&public_coefficients);
    if commitment_encoding.iter().all(|byte| *byte == 0) {
        return Err(JindoErrorV1::ZeroCommitment { index: 0 });
    }
    let commitment = PrivacyJindoLatticeCommitmentV1::new(commitment_encoding.clone());
    Ok((
        commitment,
        JindoOpeningV1 {
            polynomial,
            encoded_columns,
            mlwe_columns,
            rounded_inner_commitments,
            commitment_encoding,
        },
    ))
}

/// Produce one canonical batched proof for the supplied public statement.
pub fn prove_batched_evaluation_v1(
    statement: &IrohaJindoPolynomialCommitmentStatementV1,
    witness_polynomials: &[Vec<PrivacyJindoFieldElementV1>],
    openings: &[JindoOpeningV1],
    binding: &TranscriptBindingV1<'_>,
) -> Result<Vec<u8>, JindoErrorV1> {
    let public = validate_statement_and_binding(statement, binding)?;
    if witness_polynomials.len() != public.batch_count {
        return Err(JindoErrorV1::InvalidPolynomialCount {
            count: witness_polynomials.len(),
            max: JINDO_PARAMETERS_V1.max_batch_size,
        });
    }
    if openings.len() != public.batch_count {
        return Err(JindoErrorV1::OpeningCountMismatch);
    }

    for (index, coefficients) in witness_polynomials.iter().enumerate() {
        let polynomial = parse_polynomial(coefficients, index)?;
        if evaluate_polynomial(&polynomial, public.evaluation_point) != public.claims[index] {
            return Err(JindoErrorV1::ClaimMismatch { index });
        }
        let opening = &openings[index];
        if opening.polynomial.as_slice() != polynomial.as_slice()
            || opening.commitment_encoding != statement.polynomial_commitments[index].encoding
            || !opening_has_fixed_shape(opening)
        {
            return Err(JindoErrorV1::OpeningMismatch { index });
        }
    }

    let mut transcript = statement_transcript(statement, binding)?;
    let batch_challenges = batch_challenges(&mut transcript, public.batch_count)?;
    let rounded_inner_commitments = combine_rounded_inner(openings, &batch_challenges.outer);
    let encoded_columns =
        Zeroizing::new(combine_encoded_columns(openings, &batch_challenges.inner));
    let mlwe_columns = Zeroizing::new(combine_mlwe_columns(openings, &batch_challenges.inner));

    let left = evaluation_left_vector(public.evaluation_point);
    let mut partials = Vec::with_capacity(JINDO_PARAMETERS_V1.columns);
    for column in 0..JINDO_PARAMETERS_V1.columns {
        partials.push(inner_product_encoded(&left, &encoded_columns[column]));
    }
    let partial_mask = inner_product_encoded(&left, &encoded_columns[JINDO_PARAMETERS_V1.columns]);
    absorb_partial_commitments(&mut transcript, &partials, &partial_mask)?;
    let evaluation_challenge = transcript.challenge(b"evaluation-column", 0)?;
    let evaluation_challenge_inner = evaluation_challenge.polynomial(JINDO_INNER_MODULI_V1);

    let encode_responses = encoded_columns[JINDO_PARAMETERS_V1.columns]
        .iter()
        .zip(&encoded_columns[0])
        .map(|(mask, value)| {
            let mut response = mask.clone();
            response.add_assign(
                &value.mul(&evaluation_challenge_inner, JINDO_INNER_MODULI_V1),
                JINDO_INNER_MODULI_V1,
            );
            response
        })
        .collect();
    let mlwe_responses = mlwe_columns[JINDO_PARAMETERS_V1.columns]
        .iter()
        .zip(&mlwe_columns[0])
        .map(|(mask, value)| {
            let mut response = mask.clone();
            response.add_assign(
                &value.mul(&evaluation_challenge_inner, JINDO_INNER_MODULI_V1),
                JINDO_INNER_MODULI_V1,
            );
            response
        })
        .collect();

    let proof = JindoEvaluationProofV1::new(
        u8::try_from(public.batch_count).expect("Jindo batch count fits u8"),
        rounded_inner_commitments,
        partials,
        partial_mask,
        encode_responses,
        mlwe_responses,
    )?;
    let encoded = proof.encode();
    if verify_batched_evaluation_v1(
        statement,
        &encoded,
        binding,
        u32::try_from(encoded.len()).expect("fixed Jindo proof length fits u32"),
    )
    .is_err()
    {
        return Err(JindoErrorV1::ProverSelfCheck);
    }
    Ok(encoded)
}

/// Verify one exact canonical Jindo batched opening.
pub fn verify_batched_evaluation_v1(
    statement: &IrohaJindoPolynomialCommitmentStatementV1,
    proof_bytes: &[u8],
    binding: &TranscriptBindingV1<'_>,
    max_proof_bytes: u32,
) -> Result<(), JindoErrorV1> {
    let public = validate_statement_and_binding(statement, binding)?;
    let proof =
        JindoEvaluationProofV1::decode_exact(proof_bytes, public.batch_count, max_proof_bytes)?;
    let commitments: Vec<_> = statement
        .polynomial_commitments
        .iter()
        .enumerate()
        .map(|(index, commitment)| parse_public_commitment(commitment, index))
        .collect::<Result<_, _>>()?;

    let mut transcript = statement_transcript(statement, binding)?;
    let batch_challenges = batch_challenges(&mut transcript, public.batch_count)?;
    absorb_partial_commitments(&mut transcript, &proof.partials, &proof.partial_mask)?;
    let evaluation_challenge = transcript.challenge(b"evaluation-column", 0)?;
    let evaluation_challenge_inner = evaluation_challenge.polynomial(JINDO_INNER_MODULI_V1);
    let evaluation_challenge_outer = evaluation_challenge.polynomial(JINDO_OUTER_MODULI_V1);

    verify_outer_relation(&proof, &commitments, &batch_challenges.outer)?;
    verify_inner_relation(&proof, &evaluation_challenge_outer)?;
    verify_consistency_relation(&proof, public.evaluation_point, &evaluation_challenge_inner)?;
    verify_evaluation_relation(
        &proof,
        public.evaluation_point,
        &public.claims,
        &batch_challenges.scalars,
    )
}

struct ParsedPublicStatementV1 {
    batch_count: usize,
    evaluation_point: JindoFieldElementV1,
    claims: Vec<JindoFieldElementV1>,
}

struct BatchChallengesV1 {
    inner: Vec<JindoRnsPolynomialV1>,
    outer: Vec<JindoRnsPolynomialV1>,
    scalars: Vec<JindoFieldElementV1>,
}

fn validate_statement_and_binding(
    statement: &IrohaJindoPolynomialCommitmentStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<ParsedPublicStatementV1, JindoErrorV1> {
    binding
        .validate()
        .map_err(JindoTranscriptErrorV1::Binding)?;
    let batch_count = statement.polynomial_commitments.len();
    if batch_count == 0 || batch_count > JINDO_PARAMETERS_V1.max_batch_size {
        return Err(JindoErrorV1::InvalidPolynomialCount {
            count: batch_count,
            max: JINDO_PARAMETERS_V1.max_batch_size,
        });
    }
    if statement.claimed_evaluations.len() != batch_count {
        return Err(JindoErrorV1::ClaimCountMismatch);
    }
    for (index, commitment) in statement.polynomial_commitments.iter().enumerate() {
        let _ = parse_public_commitment(commitment, index)?;
        if statement.polynomial_commitments[..index]
            .iter()
            .any(|earlier| earlier.encoding == commitment.encoding)
        {
            return Err(JindoErrorV1::DuplicateCommitment { index });
        }
    }
    let evaluation_point =
        JindoFieldElementV1::from_canonical_bytes(statement.evaluation_point.encoding)
            .ok_or(JindoErrorV1::NonCanonicalEvaluationPoint)?;
    let claims = statement
        .claimed_evaluations
        .iter()
        .enumerate()
        .map(|(index, value)| {
            JindoFieldElementV1::from_canonical_bytes(value.encoding)
                .ok_or(JindoErrorV1::NonCanonicalClaim { index })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let context = &statement.context;
    if context.transaction_intent_digest.is_zero() {
        return Err(JindoErrorV1::ZeroTransactionIntentDigest);
    }
    if context.action_index != 0 {
        return Err(JindoErrorV1::InvalidActionIndex {
            index: context.action_index,
        });
    }
    for (field, supplied, expected) in [
        (
            JindoBindingFieldV1::ParameterId,
            binding.parameter_id,
            *context.parameter_id.as_bytes(),
        ),
        (
            JindoBindingFieldV1::ParameterDigest,
            binding.parameter_digest,
            *context.parameter_digest.as_bytes(),
        ),
        (
            JindoBindingFieldV1::VerifierDigest,
            binding.verifier_digest,
            *context.verifier_digest.as_bytes(),
        ),
        (
            JindoBindingFieldV1::StatementSchemaDigest,
            binding.statement_schema_digest,
            *context.statement_schema_digest.as_bytes(),
        ),
        (
            JindoBindingFieldV1::EngineManifestDigest,
            binding.engine_manifest_digest,
            *context.engine_manifest_digest.as_bytes(),
        ),
    ] {
        if supplied != expected {
            return Err(JindoErrorV1::BindingMismatch { field });
        }
    }
    if binding.chain_id != context.chain_id.as_str().as_bytes() {
        return Err(JindoErrorV1::BindingMismatch {
            field: JindoBindingFieldV1::ChainId,
        });
    }
    if binding.action_index != context.action_index {
        return Err(JindoErrorV1::BindingMismatch {
            field: JindoBindingFieldV1::ActionIndex,
        });
    }
    if binding.generator_digest != crs_digest_v1() {
        return Err(JindoErrorV1::BindingMismatch {
            field: JindoBindingFieldV1::CrsDigest,
        });
    }
    let statement_digest = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement.clone())
        .digest()
        .map_err(|_| JindoErrorV1::StatementEncoding)?;
    if binding.statement_digest != *statement_digest.as_bytes() {
        return Err(JindoErrorV1::BindingMismatch {
            field: JindoBindingFieldV1::StatementDigest,
        });
    }
    Ok(ParsedPublicStatementV1 {
        batch_count,
        evaluation_point,
        claims,
    })
}

fn statement_transcript(
    statement: &IrohaJindoPolynomialCommitmentStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<JindoTranscriptV1, JindoErrorV1> {
    let mut transcript = JindoTranscriptV1::new(binding, crs_digest_v1())?;
    transcript.append_message(b"suite", JINDO_SUITE_V1)?;
    transcript.append_message(b"source_profile", JINDO_SOURCE_PROFILE_V1)?;
    transcript.append_message(b"parameter_manifest", JINDO_PARAMETER_MANIFEST_V1)?;
    transcript.append_message(
        b"batch_count",
        &u32::try_from(statement.polynomial_commitments.len())
            .expect("Jindo batch count fits u32")
            .to_be_bytes(),
    )?;
    for (index, commitment) in statement.polynomial_commitments.iter().enumerate() {
        transcript.append_message(&indexed_label(b"commitment", index), &commitment.encoding)?;
    }
    transcript.append_message(b"evaluation_point", &statement.evaluation_point.encoding)?;
    for (index, claim) in statement.claimed_evaluations.iter().enumerate() {
        transcript.append_message(&indexed_label(b"claim", index), &claim.encoding)?;
    }
    Ok(transcript)
}

fn batch_challenges(
    transcript: &mut JindoTranscriptV1,
    batch_count: usize,
) -> Result<BatchChallengesV1, JindoErrorV1> {
    let mut inner = Vec::with_capacity(batch_count);
    let mut outer = Vec::with_capacity(batch_count);
    let mut scalars = Vec::with_capacity(batch_count);
    if batch_count == 1 {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        coefficients[0] = 1;
        inner.push(JindoRnsPolynomialV1::from_balanced_coefficients(
            coefficients,
            JINDO_INNER_MODULI_V1,
        ));
        outer.push(JindoRnsPolynomialV1::from_balanced_coefficients(
            coefficients,
            JINDO_OUTER_MODULI_V1,
        ));
        scalars.push(JindoFieldElementV1::ONE);
    } else {
        for index in 0..batch_count {
            let challenge = transcript.challenge(
                b"batch-polynomial",
                u32::try_from(index).expect("Jindo batch index fits u32"),
            )?;
            let inner_challenge = challenge.polynomial(JINDO_INNER_MODULI_V1);
            scalars.push(decode_coefficient_slots_v1(&inner_challenge)[0]);
            inner.push(inner_challenge);
            outer.push(challenge.polynomial(JINDO_OUTER_MODULI_V1));
        }
    }
    Ok(BatchChallengesV1 {
        inner,
        outer,
        scalars,
    })
}

fn absorb_partial_commitments(
    transcript: &mut JindoTranscriptV1,
    partials: &[JindoRnsPolynomialV1],
    partial_mask: &JindoRnsPolynomialV1,
) -> Result<(), JindoErrorV1> {
    for (index, partial) in partials.iter().enumerate() {
        let encoded = encode_rns_polynomial(partial);
        transcript.append_message(&indexed_label(b"partial", index), &encoded)?;
    }
    let encoded = encode_rns_polynomial(partial_mask);
    transcript.append_message(b"partial_mask", &encoded)?;
    Ok(())
}

fn indexed_label(prefix: &[u8], index: usize) -> Vec<u8> {
    let mut label = Vec::with_capacity(prefix.len() + 4);
    label.extend_from_slice(prefix);
    label.extend_from_slice(
        &u32::try_from(index)
            .expect("fixed Jindo index fits u32")
            .to_be_bytes(),
    );
    label
}

fn parse_polynomial(
    coefficients: &[PrivacyJindoFieldElementV1],
    polynomial_index: usize,
) -> Result<Zeroizing<Vec<JindoFieldElementV1>>, JindoErrorV1> {
    validate_canonical_polynomial_v1(coefficients, polynomial_index).map_err(JindoErrorV1::from)?;
    let mut polynomial = Zeroizing::new(vec![JindoFieldElementV1::ZERO; JINDO_MAX_COEFFICIENTS_V1]);
    for (coefficient_index, coefficient) in coefficients.iter().enumerate() {
        polynomial[coefficient_index] = JindoFieldElementV1::from_canonical_bytes(
            coefficient.encoding,
        )
        .ok_or(JindoErrorV1::NonCanonicalCoefficient {
            polynomial_index,
            coefficient_index,
        })?;
    }
    Ok(polynomial)
}

fn split_boundary_rows<R>(
    polynomial: &[JindoFieldElementV1],
    rng: &mut R,
) -> Result<
    (
        Zeroizing<[JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1]>,
        Zeroizing<[JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1]>,
    ),
    JindoErrorV1,
>
where
    R: CryptoRng + RngCore,
{
    let mut last = Zeroizing::new([JindoFieldElementV1::ZERO; JINDO_ENCODING_SLOTS_V1]);
    for value in &mut last[..JINDO_ENCODING_SLOTS_V1 - 1] {
        *value = sample_uniform_field_element_v1(rng)?;
    }
    let mut first = Zeroizing::new([JindoFieldElementV1::ZERO; JINDO_ENCODING_SLOTS_V1]);
    first[0] = polynomial[0];
    for index in 1..JINDO_ENCODING_SLOTS_V1 {
        first[index] = polynomial[index] - last[index - 1];
    }
    Ok((first, last))
}

fn compute_rounded_inner_commitments(
    encoded_columns: &[Vec<JindoRnsPolynomialV1>],
    mlwe_columns: &[Vec<JindoRnsPolynomialV1>],
) -> Vec<JindoRnsPolynomialV1> {
    let key = commit_key_v1();
    let mut rounded =
        Vec::with_capacity(JINDO_PARAMETERS_V1.inner_msis_rank * (JINDO_PARAMETERS_V1.columns + 1));
    for column in 0..=JINDO_PARAMETERS_V1.columns {
        for row in 0..JINDO_PARAMETERS_V1.inner_msis_rank {
            let mut commitment = JindoRnsPolynomialV1::zero();
            for (matrix, value) in key.inner[row].iter().zip(&encoded_columns[column]) {
                commitment.add_assign(
                    &matrix.mul(value, JINDO_INNER_MODULI_V1),
                    JINDO_INNER_MODULI_V1,
                );
            }
            for (matrix, value) in key.mlwe[row]
                .iter()
                .zip(&mlwe_columns[column][..JINDO_PARAMETERS_V1.mlwe_rank])
            {
                commitment.add_assign(
                    &matrix.mul(value, JINDO_INNER_MODULI_V1),
                    JINDO_INNER_MODULI_V1,
                );
            }
            commitment.add_assign(
                &mlwe_columns[column][JINDO_PARAMETERS_V1.mlwe_rank + row],
                JINDO_INNER_MODULI_V1,
            );
            rounded.push(round_and_change_basis(
                &commitment,
                JINDO_INNER_MODULI_V1,
                JINDO_OUTER_MODULI_V1,
                JINDO_PARAMETERS_V1.log_inner_cutoff,
            ));
        }
    }
    rounded
}

fn compute_public_commitment(
    rounded_inner_commitments: &[JindoRnsPolynomialV1],
) -> Result<Vec<i32>, JindoErrorV1> {
    let key = commit_key_v1();
    let mut coefficients =
        Vec::with_capacity(JINDO_PARAMETERS_V1.outer_msis_rank * JINDO_RING_DEGREE_V1);
    for row in 0..JINDO_PARAMETERS_V1.outer_msis_rank {
        let mut commitment = JindoRnsPolynomialV1::zero();
        for (matrix, value) in key.outer[row].iter().zip(rounded_inner_commitments) {
            commitment.add_assign(
                &matrix.mul(value, JINDO_OUTER_MODULI_V1),
                JINDO_OUTER_MODULI_V1,
            );
        }
        for coefficient_index in 0..JINDO_RING_DEGREE_V1 {
            let value = floor_div_power_of_two(
                commitment.balanced_coefficient(coefficient_index, JINDO_OUTER_MODULI_V1),
                JINDO_PARAMETERS_V1.log_outer_cutoff,
            );
            let value =
                i32::try_from(value).map_err(|_| JindoErrorV1::RoundedCommitmentOutOfRange)?;
            if !(IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1
                ..=IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1)
                .contains(&value)
            {
                return Err(JindoErrorV1::RoundedCommitmentOutOfRange);
            }
            coefficients.push(value);
        }
    }
    Ok(coefficients)
}

fn floor_div_power_of_two(value: i128, exponent: u32) -> i128 {
    value.div_euclid(1_i128 << exponent)
}

fn round_and_change_basis(
    polynomial: &JindoRnsPolynomialV1,
    source_moduli: [JindoPrimeModulusV1; 2],
    destination_moduli: [JindoPrimeModulusV1; 2],
    exponent: u32,
) -> JindoRnsPolynomialV1 {
    let coefficients = core::array::from_fn(|index| {
        floor_div_power_of_two(
            polynomial.balanced_coefficient(index, source_moduli),
            exponent,
        )
    });
    JindoRnsPolynomialV1::from_balanced_coefficients(coefficients, destination_moduli)
}

fn change_basis(
    polynomial: &JindoRnsPolynomialV1,
    source_moduli: [JindoPrimeModulusV1; 2],
    destination_moduli: [JindoPrimeModulusV1; 2],
) -> JindoRnsPolynomialV1 {
    let coefficients =
        core::array::from_fn(|index| polynomial.balanced_coefficient(index, source_moduli));
    JindoRnsPolynomialV1::from_balanced_coefficients(coefficients, destination_moduli)
}

fn encode_public_commitment(coefficients: &[i32]) -> Vec<u8> {
    let mut encoding = Vec::with_capacity(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1);
    for coefficient in coefficients {
        encoding.extend_from_slice(&coefficient.to_le_bytes());
    }
    debug_assert_eq!(encoding.len(), IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1);
    encoding
}

fn parse_public_commitment(
    commitment: &PrivacyJindoLatticeCommitmentV1,
    index: usize,
) -> Result<Vec<JindoRnsPolynomialV1>, JindoErrorV1> {
    if commitment.encoding.len() != IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1 {
        return Err(JindoErrorV1::InvalidCommitmentEncoding { index });
    }
    let mut polynomials = Vec::with_capacity(JINDO_PARAMETERS_V1.outer_msis_rank);
    let mut any_nonzero = false;
    for row in 0..JINDO_PARAMETERS_V1.outer_msis_rank {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        for (coefficient_index, coefficient) in coefficients.iter_mut().enumerate() {
            let offset = (row * JINDO_RING_DEGREE_V1 + coefficient_index) * 4;
            let value = i32::from_le_bytes(
                commitment.encoding[offset..offset + 4]
                    .try_into()
                    .expect("commitment width prevalidated"),
            );
            if !(IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1
                ..=IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1)
                .contains(&value)
            {
                return Err(JindoErrorV1::InvalidCommitmentEncoding { index });
            }
            any_nonzero |= value != 0;
            *coefficient = i128::from(value);
        }
        polynomials.push(JindoRnsPolynomialV1::from_balanced_coefficients(
            coefficients,
            JINDO_OUTER_MODULI_V1,
        ));
    }
    if !any_nonzero {
        return Err(JindoErrorV1::ZeroCommitment { index });
    }
    Ok(polynomials)
}

fn combine_rounded_inner(
    openings: &[JindoOpeningV1],
    challenges: &[JindoRnsPolynomialV1],
) -> Vec<JindoRnsPolynomialV1> {
    let mut combined = vec![
        JindoRnsPolynomialV1::zero();
        JINDO_PARAMETERS_V1.inner_msis_rank * (JINDO_PARAMETERS_V1.columns + 1)
    ];
    for (opening, challenge) in openings.iter().zip(challenges) {
        for (output, value) in combined
            .iter_mut()
            .zip(opening.rounded_inner_commitments.iter())
        {
            output.add_assign(
                &value.mul(challenge, JINDO_OUTER_MODULI_V1),
                JINDO_OUTER_MODULI_V1,
            );
        }
    }
    combined
}

fn combine_encoded_columns(
    openings: &[JindoOpeningV1],
    challenges: &[JindoRnsPolynomialV1],
) -> Vec<Vec<JindoRnsPolynomialV1>> {
    let mut combined = vec![
        vec![JindoRnsPolynomialV1::zero(); JINDO_PARAMETERS_V1.rows];
        JINDO_PARAMETERS_V1.columns + 1
    ];
    for (opening, challenge) in openings.iter().zip(challenges) {
        for (output_column, input_column) in combined.iter_mut().zip(opening.encoded_columns.iter())
        {
            for (output, value) in output_column.iter_mut().zip(input_column) {
                output.add_assign(
                    &value.mul(challenge, JINDO_INNER_MODULI_V1),
                    JINDO_INNER_MODULI_V1,
                );
            }
        }
    }
    combined
}

fn combine_mlwe_columns(
    openings: &[JindoOpeningV1],
    challenges: &[JindoRnsPolynomialV1],
) -> Vec<Vec<JindoRnsPolynomialV1>> {
    let mut combined = vec![
        vec![
            JindoRnsPolynomialV1::zero();
            JINDO_PARAMETERS_V1.mlwe_rank + JINDO_PARAMETERS_V1.inner_msis_rank
        ];
        JINDO_PARAMETERS_V1.columns + 1
    ];
    for (opening, challenge) in openings.iter().zip(challenges) {
        for (output_column, input_column) in combined.iter_mut().zip(opening.mlwe_columns.iter()) {
            for (output, value) in output_column.iter_mut().zip(input_column) {
                output.add_assign(
                    &value.mul(challenge, JINDO_INNER_MODULI_V1),
                    JINDO_INNER_MODULI_V1,
                );
            }
        }
    }
    combined
}

fn evaluation_left_vector(evaluation_point: JindoFieldElementV1) -> Vec<JindoRnsPolynomialV1> {
    let mut field_values = vec![JindoFieldElementV1::ONE; JINDO_PARAMETERS_V1.rows];
    let stride = field_pow(evaluation_point, JINDO_ENCODING_SLOTS_V1);
    for row in 1..JINDO_PARAMETERS_V1.rows {
        field_values[row] = field_values[row - 1] * stride;
    }
    field_values[JINDO_PARAMETERS_V1.rows - 1] = evaluation_point;
    field_values
        .into_iter()
        .map(|value| {
            encode_coefficient_slots_v1(&[value])
                .expect("one field element fits the fixed CELPC slot vector")
        })
        .collect()
}

fn inner_product_encoded(
    left: &[JindoRnsPolynomialV1],
    right: &[JindoRnsPolynomialV1],
) -> JindoRnsPolynomialV1 {
    let mut result = JindoRnsPolynomialV1::zero();
    for (left, right) in left.iter().zip(right) {
        result.add_assign(
            &left.mul(right, JINDO_INNER_MODULI_V1),
            JINDO_INNER_MODULI_V1,
        );
    }
    result
}

fn verify_outer_relation(
    proof: &JindoEvaluationProofV1,
    commitments: &[Vec<JindoRnsPolynomialV1>],
    batch_challenges: &[JindoRnsPolynomialV1],
) -> Result<(), JindoErrorV1> {
    let key = commit_key_v1();
    let mut relation = proof.rounded_inner_commitments.clone();
    for row in 0..JINDO_PARAMETERS_V1.outer_msis_rank {
        let mut batched_commitment = JindoRnsPolynomialV1::zero();
        for (commitment, challenge) in commitments.iter().zip(batch_challenges) {
            batched_commitment.add_assign(
                &commitment[row].mul(challenge, JINDO_OUTER_MODULI_V1),
                JINDO_OUTER_MODULI_V1,
            );
        }
        let mut residual = batched_commitment
            .scale_power_of_two(JINDO_PARAMETERS_V1.log_outer_cutoff, JINDO_OUTER_MODULI_V1);
        for (matrix, value) in key.outer[row].iter().zip(&proof.rounded_inner_commitments) {
            residual.sub_assign(
                &matrix.mul(value, JINDO_OUTER_MODULI_V1),
                JINDO_OUTER_MODULI_V1,
            );
        }
        relation.push(residual);
    }
    if !two_norm_squared_is_below_v1(
        &relation,
        JINDO_OUTER_MODULI_V1,
        JINDO_DECOMPOSED_NORM_SQUARED_BOUND_V1,
    ) {
        return Err(JindoErrorV1::OuterCommitmentRelation);
    }
    Ok(())
}

fn verify_inner_relation(
    proof: &JindoEvaluationProofV1,
    evaluation_challenge_outer: &JindoRnsPolynomialV1,
) -> Result<(), JindoErrorV1> {
    let key = commit_key_v1();
    let mut relation = Vec::with_capacity(
        JINDO_PARAMETERS_V1.rows
            + JINDO_PARAMETERS_V1.mlwe_rank
            + 2 * JINDO_PARAMETERS_V1.inner_msis_rank,
    );
    relation.extend(proof.encode_responses.iter().cloned());
    relation.extend(proof.mlwe_responses.iter().cloned());
    for row in 0..JINDO_PARAMETERS_V1.inner_msis_rank {
        let real = &proof.rounded_inner_commitments[row];
        let mask = &proof.rounded_inner_commitments[JINDO_PARAMETERS_V1.inner_msis_rank + row];
        let mut combined = mask.clone();
        combined.add_assign(
            &real.mul(evaluation_challenge_outer, JINDO_OUTER_MODULI_V1),
            JINDO_OUTER_MODULI_V1,
        );
        let mut residual = change_basis(&combined, JINDO_OUTER_MODULI_V1, JINDO_INNER_MODULI_V1)
            .scale_power_of_two(JINDO_PARAMETERS_V1.log_inner_cutoff, JINDO_INNER_MODULI_V1);
        for (matrix, value) in key.inner[row].iter().zip(&proof.encode_responses) {
            residual.sub_assign(
                &matrix.mul(value, JINDO_INNER_MODULI_V1),
                JINDO_INNER_MODULI_V1,
            );
        }
        for (matrix, value) in key.mlwe[row]
            .iter()
            .zip(&proof.mlwe_responses[..JINDO_PARAMETERS_V1.mlwe_rank])
        {
            residual.sub_assign(
                &matrix.mul(value, JINDO_INNER_MODULI_V1),
                JINDO_INNER_MODULI_V1,
            );
        }
        residual.sub_assign(
            &proof.mlwe_responses[JINDO_PARAMETERS_V1.mlwe_rank + row],
            JINDO_INNER_MODULI_V1,
        );
        relation.push(residual);
    }
    if !two_norm_squared_is_below_v1(
        &relation,
        JINDO_INNER_MODULI_V1,
        JINDO_RESPONSE_NORM_SQUARED_BOUND_V1,
    ) {
        return Err(JindoErrorV1::InnerCommitmentRelation);
    }
    Ok(())
}

fn verify_consistency_relation(
    proof: &JindoEvaluationProofV1,
    evaluation_point: JindoFieldElementV1,
    evaluation_challenge: &JindoRnsPolynomialV1,
) -> Result<(), JindoErrorV1> {
    let left = evaluation_left_vector(evaluation_point);
    let mut relation = inner_product_encoded(&left, &proof.encode_responses);
    relation.sub_assign(
        &proof.partials[0].mul(evaluation_challenge, JINDO_INNER_MODULI_V1),
        JINDO_INNER_MODULI_V1,
    );
    relation.sub_assign(&proof.partial_mask, JINDO_INNER_MODULI_V1);
    if !relation.is_zero() {
        return Err(JindoErrorV1::EvaluationConsistency);
    }
    Ok(())
}

fn verify_evaluation_relation(
    proof: &JindoEvaluationProofV1,
    evaluation_point: JindoFieldElementV1,
    claims: &[JindoFieldElementV1],
    batch_scalars: &[JindoFieldElementV1],
) -> Result<(), JindoErrorV1> {
    let decoded = decode_coefficient_slots_v1(&proof.partials[0]);
    let mut actual = JindoFieldElementV1::ZERO;
    let mut power = JindoFieldElementV1::ONE;
    for value in decoded {
        actual = actual + power * value;
        power = power * evaluation_point;
    }
    let expected = claims
        .iter()
        .zip(batch_scalars)
        .fold(JindoFieldElementV1::ZERO, |sum, (claim, scalar)| {
            sum + *claim * *scalar
        });
    if actual != expected {
        return Err(JindoErrorV1::EvaluationRelation);
    }
    Ok(())
}

fn evaluate_polynomial(
    polynomial: &[JindoFieldElementV1],
    point: JindoFieldElementV1,
) -> JindoFieldElementV1 {
    polynomial
        .iter()
        .rev()
        .fold(JindoFieldElementV1::ZERO, |value, coefficient| {
            value * point + *coefficient
        })
}

fn field_pow(mut base: JindoFieldElementV1, mut exponent: usize) -> JindoFieldElementV1 {
    let mut result = JindoFieldElementV1::ONE;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result * base;
        }
        base = base * base;
        exponent >>= 1;
    }
    result
}

fn opening_has_fixed_shape(opening: &JindoOpeningV1) -> bool {
    opening.polynomial.len() == JINDO_MAX_COEFFICIENTS_V1
        && opening.encoded_columns.len() == JINDO_PARAMETERS_V1.columns + 1
        && opening
            .encoded_columns
            .iter()
            .all(|column| column.len() == JINDO_PARAMETERS_V1.rows)
        && opening.mlwe_columns.len() == JINDO_PARAMETERS_V1.columns + 1
        && opening.mlwe_columns.iter().all(|column| {
            column.len() == JINDO_PARAMETERS_V1.mlwe_rank + JINDO_PARAMETERS_V1.inner_msis_rank
        })
        && opening.rounded_inner_commitments.len()
            == JINDO_PARAMETERS_V1.inner_msis_rank * (JINDO_PARAMETERS_V1.columns + 1)
        && opening.commitment_encoding.len() == IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1
}

fn encode_rns_polynomial(polynomial: &JindoRnsPolynomialV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 * JINDO_RING_DEGREE_V1 * 8);
    for residue in polynomial.residues().iter().flatten() {
        bytes.extend_from_slice(&residue.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
        PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
        PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    };
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};

    use super::*;
    use crate::privacy_engines::jindo::{
        JINDO_MAX_BATCH_SIZE_V1,
        codec::{
            JINDO_PROOF_HEADER_BYTES_V1, JINDO_PROOF_INNER_POLYNOMIALS_V1,
            JINDO_PROOF_OUTER_POLYNOMIALS_V1, JINDO_PROOF_RNS_POLYNOMIAL_BYTES_V1,
        },
        ring::{JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1},
    };

    #[derive(Clone)]
    struct TestRng(u64);

    impl TestRng {
        const fn new(seed: u64) -> Self {
            Self(seed)
        }
    }

    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            let mut value = self.0;
            value ^= value >> 12;
            value ^= value << 25;
            value ^= value >> 27;
            self.0 = value;
            value.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for chunk in destination.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for TestRng {}

    struct NonCanonicalFieldRng;

    impl RngCore for NonCanonicalFieldRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }

        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(u8::MAX);
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for NonCanonicalFieldRng {}

    struct PanicRng;

    impl RngCore for PanicRng {
        fn next_u32(&mut self) -> u32 {
            panic!("non-canonical polynomial reached Jindo randomness")
        }

        fn next_u64(&mut self) -> u64 {
            panic!("non-canonical polynomial reached Jindo randomness")
        }

        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("non-canonical polynomial reached Jindo randomness")
        }

        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), RngError> {
            panic!("non-canonical polynomial reached Jindo randomness")
        }
    }

    impl CryptoRng for PanicRng {}

    fn field(value: u64) -> PrivacyJindoFieldElementV1 {
        let mut encoding = [0_u8; 32];
        encoding[..8].copy_from_slice(&value.to_le_bytes());
        PrivacyJindoFieldElementV1::new(encoding)
    }

    fn field_modulus_encoding() -> [u8; 32] {
        let mut encoding = [0_u8; 32];
        for (chunk, limb) in encoding
            .chunks_exact_mut(core::mem::size_of::<u64>())
            .zip(JindoFieldElementV1::MODULUS)
        {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }
        encoding
    }

    fn field_modulus_plus_one_encoding() -> [u8; 32] {
        let mut encoding = field_modulus_encoding();
        encoding[0] = encoding[0]
            .checked_add(1)
            .expect("Jindo modulus low byte leaves room for one");
        encoding
    }

    fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: "jindo-native-test".parse().expect("chain id"),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([1; 32]),
            parameter_id: PrivacyParameterIdV1::new([2; 32]),
            parameter_digest: PrivacyParameterDigestV1::new([3; 32]),
            verifier_digest: PrivacyVerifierDigestV1::new([4; 32]),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new([5; 32]),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new([6; 32]),
        }
    }

    fn binding(statement: &IrohaJindoPolynomialCommitmentStatementV1) -> TranscriptBindingV1<'_> {
        let digest = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement.clone())
            .digest()
            .expect("statement digest");
        TranscriptBindingV1 {
            chain_id: statement.context.chain_id.as_str().as_bytes(),
            genesis_hash: [7; 32],
            action_index: statement.context.action_index,
            statement_digest: *digest.as_bytes(),
            parameter_id: *statement.context.parameter_id.as_bytes(),
            parameter_digest: *statement.context.parameter_digest.as_bytes(),
            verifier_digest: *statement.context.verifier_digest.as_bytes(),
            statement_schema_digest: *statement.context.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *statement.context.engine_manifest_digest.as_bytes(),
            generator_digest: crs_digest_v1(),
        }
    }

    fn commit_statement(
        polynomials: &[Vec<PrivacyJindoFieldElementV1>],
        evaluation_point: PrivacyJindoFieldElementV1,
        seed: u64,
    ) -> (
        IrohaJindoPolynomialCommitmentStatementV1,
        Vec<JindoOpeningV1>,
    ) {
        let mut rng = TestRng::new(seed);
        let mut commitments = Vec::with_capacity(polynomials.len());
        let mut claims = Vec::with_capacity(polynomials.len());
        let mut openings = Vec::with_capacity(polynomials.len());
        for polynomial in polynomials {
            let (commitment, opening) =
                commit_polynomial_v1(polynomial, &mut rng).expect("commitment");
            commitments.push(commitment);
            openings.push(opening);
            claims.push(
                evaluate_polynomial_v1(polynomial, evaluation_point).expect("canonical evaluation"),
            );
        }
        (
            IrohaJindoPolynomialCommitmentStatementV1 {
                context: context(),
                polynomial_commitments: commitments,
                evaluation_point,
                claimed_evaluations: claims,
            },
            openings,
        )
    }

    fn prove_fixture(
        polynomials: &[Vec<PrivacyJindoFieldElementV1>],
        evaluation_point: PrivacyJindoFieldElementV1,
        seed: u64,
    ) -> (IrohaJindoPolynomialCommitmentStatementV1, Vec<u8>) {
        let (statement, openings) = commit_statement(polynomials, evaluation_point, seed);
        let proof =
            prove_batched_evaluation_v1(&statement, polynomials, &openings, &binding(&statement))
                .expect("proof");
        (statement, proof)
    }

    fn assert_verification_rejected(
        label: &str,
        statement: &IrohaJindoPolynomialCommitmentStatementV1,
        proof: &[u8],
        binding: &TranscriptBindingV1<'_>,
    ) {
        let result = verify_batched_evaluation_v1(
            statement,
            proof,
            binding,
            u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
        );
        assert!(result.is_err(), "{label} unexpectedly verified");
    }

    fn perturb_polynomial(
        polynomial: &mut JindoRnsPolynomialV1,
        moduli: [JindoPrimeModulusV1; 2],
        magnitude: i128,
    ) {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        coefficients[0] = magnitude;
        polynomial.add_assign(
            &JindoRnsPolynomialV1::from_balanced_coefficients(coefficients, moduli),
            moduli,
        );
    }

    #[test]
    fn single_polynomial_commit_open_verify_roundtrip() {
        let polynomial = vec![field(3), field(5), field(7), field(11)];
        let evaluation_point = field(13);
        let claim =
            evaluate_polynomial_v1(&polynomial, evaluation_point).expect("canonical evaluation");
        let (commitment, opening) =
            commit_polynomial_v1(&polynomial, &mut TestRng::new(0x1234_5678_9abc_def0))
                .expect("commitment");
        assert_eq!(format!("{opening:?}"), "JindoOpeningV1([REDACTED])");

        let statement = IrohaJindoPolynomialCommitmentStatementV1 {
            context: context(),
            polynomial_commitments: vec![commitment],
            evaluation_point,
            claimed_evaluations: vec![claim],
        };
        let binding = binding(&statement);
        let proof = prove_batched_evaluation_v1(&statement, &[polynomial], &[opening], &binding)
            .expect("proof");
        assert_eq!(proof.len(), JINDO_PROOF_BYTES_V1);
        verify_batched_evaluation_v1(
            &statement,
            &proof,
            &binding,
            u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
        )
        .expect("valid proof");
    }

    #[test]
    fn two_polynomial_batch_and_zero_evaluation_point_roundtrip() {
        let polynomials = vec![
            vec![field(0)],
            vec![field(5), field(7), field(11), field(13), field(17)],
        ];
        let (statement, proof) = prove_fixture(&polynomials, field(0), 0x2234_5678_9abc_def0);
        assert_eq!(statement.claimed_evaluations, vec![field(0), field(5)]);
        verify_batched_evaluation_v1(
            &statement,
            &proof,
            &binding(&statement),
            u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
        )
        .expect("valid two-polynomial proof at zero");

        let mut duplicate = statement.clone();
        duplicate.polynomial_commitments[1] = duplicate.polynomial_commitments[0].clone();
        assert_eq!(
            verify_batched_evaluation_v1(
                &duplicate,
                &proof,
                &binding(&duplicate),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::DuplicateCommitment { index: 1 })
        );
    }

    #[test]
    fn canonical_polynomial_rule_is_shared_by_evaluate_commit_and_prove() {
        let zero = vec![field(0)];
        assert_eq!(
            evaluate_polynomial_v1(&zero, field(29)),
            Ok(field(0)),
            "[0] is the unique zero-polynomial encoding"
        );

        let exact_cap = vec![field(1); JINDO_MAX_COEFFICIENTS_V1];
        assert_eq!(
            evaluate_polynomial_v1(&exact_cap, field(0)),
            Ok(field(1)),
            "a non-zero leading coefficient at the exact cap is canonical"
        );

        let mut trailing_zero_at_cap = exact_cap.clone();
        trailing_zero_at_cap[JINDO_MAX_COEFFICIENTS_V1 - 1] = field(0);
        let oversized = vec![field(1); JINDO_MAX_COEFFICIENTS_V1 + 1];
        let noncanonical_modulus = vec![PrivacyJindoFieldElementV1::new(field_modulus_encoding())];
        let noncanonical_modulus_plus_one = vec![PrivacyJindoFieldElementV1::new(
            field_modulus_plus_one_encoding(),
        )];

        for (label, polynomial, expected) in [
            (
                "empty",
                Vec::new(),
                JindoErrorV1::EmptyPolynomial { index: 0 },
            ),
            (
                "redundant zero polynomial",
                vec![field(0), field(0)],
                JindoErrorV1::TrailingZeroCoefficient { index: 0 },
            ),
            (
                "trailing zero at cap",
                trailing_zero_at_cap,
                JindoErrorV1::TrailingZeroCoefficient { index: 0 },
            ),
            (
                "over coefficient cap",
                oversized,
                JindoErrorV1::PolynomialTooLarge {
                    index: 0,
                    count: JINDO_MAX_COEFFICIENTS_V1 + 1,
                    max: JINDO_MAX_COEFFICIENTS_V1,
                },
            ),
            (
                "field modulus",
                noncanonical_modulus,
                JindoErrorV1::NonCanonicalCoefficient {
                    polynomial_index: 0,
                    coefficient_index: 0,
                },
            ),
            (
                "field modulus plus one",
                noncanonical_modulus_plus_one,
                JindoErrorV1::NonCanonicalCoefficient {
                    polynomial_index: 0,
                    coefficient_index: 0,
                },
            ),
        ] {
            assert_eq!(
                evaluate_polynomial_v1(&polynomial, field(31)),
                Err(expected),
                "{label} reached polynomial evaluation"
            );
            let commit_error = commit_polynomial_v1(&polynomial, &mut PanicRng)
                .expect_err("non-canonical polynomial reached prover randomness");
            assert_eq!(commit_error, expected, "{label} returned the wrong error");
        }

        let canonical = vec![field(37), field(41), field(43)];
        let (statement, openings) = commit_statement(
            core::slice::from_ref(&canonical),
            field(47),
            0x2a34_5678_9abc_def0,
        );
        for (label, polynomial, expected) in [
            (
                "empty",
                Vec::new(),
                JindoErrorV1::EmptyPolynomial { index: 0 },
            ),
            (
                "trailing zero",
                {
                    let mut value = canonical.clone();
                    value.push(field(0));
                    value
                },
                JindoErrorV1::TrailingZeroCoefficient { index: 0 },
            ),
            (
                "noncanonical coefficient",
                {
                    let mut value = canonical.clone();
                    value[1] = PrivacyJindoFieldElementV1::new(field_modulus_plus_one_encoding());
                    value
                },
                JindoErrorV1::NonCanonicalCoefficient {
                    polynomial_index: 0,
                    coefficient_index: 1,
                },
            ),
        ] {
            assert_eq!(
                prove_batched_evaluation_v1(
                    &statement,
                    &[polynomial],
                    &openings,
                    &binding(&statement),
                ),
                Err(expected),
                "{label} reached the Jindo proving transcript"
            );
        }
    }

    #[test]
    fn canonical_adversarial_mutations_reach_and_fail_every_proof_relation() {
        let polynomials = vec![vec![field(3), field(5), field(7), field(11)]];
        let (statement, proof_bytes) =
            prove_fixture(&polynomials, field(13), 0x3234_5678_9abc_def0);
        let decoded = JindoEvaluationProofV1::decode_exact(
            &proof_bytes,
            1,
            u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
        )
        .expect("canonical proof");

        let mut mutations = Vec::new();
        for index in [0, JINDO_PROOF_OUTER_POLYNOMIALS_V1 - 1] {
            let mut proof = decoded.clone();
            perturb_polynomial(
                &mut proof.rounded_inner_commitments[index],
                JINDO_OUTER_MODULI_V1,
                1,
            );
            mutations.push(("rounded-inner-commitment", proof.encode()));
        }
        {
            let mut proof = decoded.clone();
            perturb_polynomial(&mut proof.partials[0], JINDO_INNER_MODULI_V1, 1);
            mutations.push(("evaluation-partial", proof.encode()));
        }
        {
            let mut proof = decoded.clone();
            perturb_polynomial(&mut proof.partial_mask, JINDO_INNER_MODULI_V1, 1);
            mutations.push(("evaluation-partial-mask", proof.encode()));
        }
        for index in [0, decoded.encode_responses.len() - 1] {
            let mut proof = decoded.clone();
            perturb_polynomial(&mut proof.encode_responses[index], JINDO_INNER_MODULI_V1, 1);
            mutations.push(("encoded-column-response", proof.encode()));
        }
        for index in [0, 31] {
            let mut proof = decoded.clone();
            perturb_polynomial(&mut proof.mlwe_responses[index], JINDO_INNER_MODULI_V1, 1);
            mutations.push(("mlwe-response", proof.encode()));
        }
        // The final inner-MSIS-rank responses are explicit short-error
        // witnesses in an approximate lattice relation. A unit perturbation
        // can legitimately remain within the norm ball, so cross the strict
        // norm ceiling while retaining a canonical RNS encoding.
        for index in [32, decoded.mlwe_responses.len() - 1] {
            let mut proof = decoded.clone();
            perturb_polynomial(
                &mut proof.mlwe_responses[index],
                JINDO_INNER_MODULI_V1,
                1_i128 << 80,
            );
            mutations.push(("mlwe-short-error-norm", proof.encode()));
        }

        assert_eq!(mutations.len(), 10);
        for (label, mutation) in mutations {
            assert_eq!(mutation.len(), JINDO_PROOF_BYTES_V1);
            JindoEvaluationProofV1::decode_exact(
                &mutation,
                1,
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            )
            .expect("mutation remains canonically encoded");
            assert_verification_rejected(label, &statement, &mutation, &binding(&statement));
        }
    }

    #[test]
    fn proof_caps_lengths_headers_and_noncanonical_residues_fail_closed() {
        let polynomials = vec![vec![field(19), field(23), field(29)]];
        let (statement, proof) = prove_fixture(&polynomials, field(31), 0x4234_5678_9abc_def0);
        let exact_cap = u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length");
        verify_batched_evaluation_v1(&statement, &proof, &binding(&statement), exact_cap)
            .expect("exact cap");
        assert!(matches!(
            verify_batched_evaluation_v1(&statement, &proof, &binding(&statement), exact_cap - 1),
            Err(JindoErrorV1::ProofCodec(
                JindoProofCodecErrorV1::TooLarge { .. }
            ))
        ));

        for (label, malformed) in [
            ("truncated", proof[..proof.len() - 1].to_vec()),
            ("trailing", {
                let mut bytes = proof.clone();
                bytes.push(0);
                bytes
            }),
            ("magic", {
                let mut bytes = proof.clone();
                bytes[0] ^= 1;
                bytes
            }),
            ("version", {
                let mut bytes = proof.clone();
                bytes[4] = 2;
                bytes
            }),
            ("batch-count", {
                let mut bytes = proof.clone();
                bytes[5] = 2;
                bytes
            }),
            ("flags", {
                let mut bytes = proof.clone();
                bytes[6] = 1;
                bytes
            }),
            ("reserved", {
                let mut bytes = proof.clone();
                bytes[7] = 1;
                bytes
            }),
        ] {
            assert_verification_rejected(label, &statement, &malformed, &binding(&statement));
        }

        let mut noncanonical_outer = proof.clone();
        noncanonical_outer[JINDO_PROOF_HEADER_BYTES_V1..JINDO_PROOF_HEADER_BYTES_V1 + 8]
            .copy_from_slice(&JINDO_OUTER_MODULI_V1[0].modulus().to_le_bytes());
        assert_verification_rejected(
            "noncanonical outer residue",
            &statement,
            &noncanonical_outer,
            &binding(&statement),
        );

        let inner_start = JINDO_PROOF_HEADER_BYTES_V1
            + JINDO_PROOF_OUTER_POLYNOMIALS_V1 * JINDO_PROOF_RNS_POLYNOMIAL_BYTES_V1;
        let mut noncanonical_inner = proof.clone();
        noncanonical_inner[inner_start..inner_start + 8]
            .copy_from_slice(&JINDO_INNER_MODULI_V1[0].modulus().to_le_bytes());
        assert_verification_rejected(
            "noncanonical inner residue",
            &statement,
            &noncanonical_inner,
            &binding(&statement),
        );
        assert_eq!(
            JINDO_PROOF_OUTER_POLYNOMIALS_V1 + JINDO_PROOF_INNER_POLYNOMIALS_V1,
            96
        );
    }

    #[test]
    fn every_consensus_binding_and_public_statement_axis_rejects_replay() {
        let polynomials = vec![vec![field(37), field(41), field(43), field(47)]];
        let (statement, proof) = prove_fixture(&polynomials, field(53), 0x5234_5678_9abc_def0);
        let valid = binding(&statement);

        macro_rules! reject_binding_mutation {
            ($label:literal, $field:ident, $value:expr) => {{
                let mut mutated = valid;
                mutated.$field = $value;
                assert_verification_rejected($label, &statement, &proof, &mutated);
            }};
        }
        reject_binding_mutation!("genesis", genesis_hash, [8; 32]);
        reject_binding_mutation!("action", action_index, 1);
        reject_binding_mutation!("statement digest", statement_digest, [8; 32]);
        reject_binding_mutation!("parameter id", parameter_id, [8; 32]);
        reject_binding_mutation!("parameter digest", parameter_digest, [8; 32]);
        reject_binding_mutation!("verifier digest", verifier_digest, [8; 32]);
        reject_binding_mutation!("schema digest", statement_schema_digest, [8; 32]);
        reject_binding_mutation!("manifest digest", engine_manifest_digest, [8; 32]);
        reject_binding_mutation!("CRS digest", generator_digest, [8; 32]);
        let mut wrong_chain = valid;
        wrong_chain.chain_id = b"jindo-replay-chain";
        assert_verification_rejected("chain", &statement, &proof, &wrong_chain);

        macro_rules! reject_statement_mutation {
            ($label:literal, $mutation:expr) => {{
                let mut mutated = statement.clone();
                $mutation(&mut mutated);
                assert_verification_rejected($label, &mutated, &proof, &binding(&mutated));
            }};
        }
        reject_statement_mutation!(
            "claim",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.claimed_evaluations[0] = field(59);
            }
        );
        reject_statement_mutation!(
            "evaluation point",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.evaluation_point = field(61);
            }
        );
        reject_statement_mutation!(
            "transaction intent",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.transaction_intent_digest =
                    PrivacyTransactionIntentDigestV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "chain context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.chain_id = "jindo-other-chain".parse().expect("chain id");
            }
        );
        reject_statement_mutation!(
            "parameter id context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.parameter_id = PrivacyParameterIdV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "parameter digest context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.parameter_digest = PrivacyParameterDigestV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "verifier digest context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.verifier_digest = PrivacyVerifierDigestV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "schema digest context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.statement_schema_digest =
                    PrivacyStatementSchemaDigestV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "manifest digest context",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                value.context.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([9; 32]);
            }
        );
        reject_statement_mutation!(
            "commitment",
            |value: &mut IrohaJindoPolynomialCommitmentStatementV1| {
                let first: [u8; 4] = value.polynomial_commitments[0].encoding[..4]
                    .try_into()
                    .expect("coefficient");
                let replacement = if i32::from_le_bytes(first) == 0 {
                    1_i32
                } else {
                    0_i32
                };
                value.polynomial_commitments[0].encoding[..4]
                    .copy_from_slice(&replacement.to_le_bytes());
            }
        );
    }

    #[test]
    fn malformed_statements_witnesses_openings_and_rng_are_rejected() {
        let polynomial = vec![field(67), field(71), field(73)];
        let (statement, openings) = commit_statement(
            core::slice::from_ref(&polynomial),
            field(79),
            0x6234_5678_9abc_def0,
        );

        let mut no_commitments = statement.clone();
        no_commitments.polynomial_commitments.clear();
        no_commitments.claimed_evaluations.clear();
        assert_eq!(
            verify_batched_evaluation_v1(
                &no_commitments,
                &[],
                &binding(&no_commitments),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::InvalidPolynomialCount {
                count: 0,
                max: JINDO_MAX_BATCH_SIZE_V1,
            })
        );

        let mut wrong_claim_count = statement.clone();
        wrong_claim_count.claimed_evaluations.clear();
        assert_eq!(
            verify_batched_evaluation_v1(
                &wrong_claim_count,
                &[],
                &binding(&wrong_claim_count),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::ClaimCountMismatch)
        );

        let mut short_commitment = statement.clone();
        short_commitment.polynomial_commitments[0].encoding.pop();
        assert_eq!(
            verify_batched_evaluation_v1(
                &short_commitment,
                &[],
                &binding(&short_commitment),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::InvalidCommitmentEncoding { index: 0 })
        );

        let mut zero_commitment = statement.clone();
        zero_commitment.polynomial_commitments[0].encoding.fill(0);
        assert_eq!(
            verify_batched_evaluation_v1(
                &zero_commitment,
                &[],
                &binding(&zero_commitment),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::ZeroCommitment { index: 0 })
        );

        let mut out_of_range_commitment = statement.clone();
        out_of_range_commitment.polynomial_commitments[0].encoding[..4]
            .copy_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(
            verify_batched_evaluation_v1(
                &out_of_range_commitment,
                &[],
                &binding(&out_of_range_commitment),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::InvalidCommitmentEncoding { index: 0 })
        );

        let mut noncanonical_point = statement.clone();
        noncanonical_point.evaluation_point =
            PrivacyJindoFieldElementV1::new(field_modulus_encoding());
        assert_eq!(
            verify_batched_evaluation_v1(
                &noncanonical_point,
                &[],
                &binding(&noncanonical_point),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::NonCanonicalEvaluationPoint)
        );

        let mut noncanonical_claim = statement.clone();
        noncanonical_claim.claimed_evaluations[0] =
            PrivacyJindoFieldElementV1::new(field_modulus_encoding());
        assert_eq!(
            verify_batched_evaluation_v1(
                &noncanonical_claim,
                &[],
                &binding(&noncanonical_claim),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::NonCanonicalClaim { index: 0 })
        );

        let mut zero_intent = statement.clone();
        zero_intent.context.transaction_intent_digest =
            PrivacyTransactionIntentDigestV1::new([0; 32]);
        assert_eq!(
            verify_batched_evaluation_v1(
                &zero_intent,
                &[],
                &binding(&zero_intent),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::ZeroTransactionIntentDigest)
        );

        let mut wrong_action = statement.clone();
        wrong_action.context.action_index = 1;
        assert_eq!(
            verify_batched_evaluation_v1(
                &wrong_action,
                &[],
                &binding(&wrong_action),
                u32::try_from(JINDO_PROOF_BYTES_V1).expect("proof length"),
            ),
            Err(JindoErrorV1::InvalidActionIndex { index: 1 })
        );

        let oversized = vec![field(1); JINDO_MAX_COEFFICIENTS_V1 + 1];
        assert!(matches!(
            evaluate_polynomial_v1(&oversized, field(1)),
            Err(JindoErrorV1::PolynomialTooLarge { .. })
        ));
        let mut noncanonical_witness = polynomial.clone();
        noncanonical_witness[1] = PrivacyJindoFieldElementV1::new(field_modulus_encoding());
        assert_eq!(
            prove_batched_evaluation_v1(
                &statement,
                &[noncanonical_witness],
                &openings,
                &binding(&statement),
            ),
            Err(JindoErrorV1::NonCanonicalCoefficient {
                polynomial_index: 0,
                coefficient_index: 1,
            })
        );
        let mut wrong_witness = polynomial.clone();
        wrong_witness[0] = field(83);
        assert_eq!(
            prove_batched_evaluation_v1(
                &statement,
                &[wrong_witness],
                &openings,
                &binding(&statement),
            ),
            Err(JindoErrorV1::ClaimMismatch { index: 0 })
        );

        let (_, wrong_openings) = commit_statement(
            &[vec![field(89), field(97)]],
            field(79),
            0x7234_5678_9abc_def0,
        );
        assert_eq!(
            prove_batched_evaluation_v1(
                &statement,
                &[polynomial],
                &wrong_openings,
                &binding(&statement),
            ),
            Err(JindoErrorV1::OpeningMismatch { index: 0 })
        );

        assert!(matches!(
            commit_polynomial_v1(&[field(1)], &mut NonCanonicalFieldRng),
            Err(JindoErrorV1::Sampling(
                JindoSamplingErrorV1::RandomnessHealthCheckFailed
            ))
        ));
    }
}
