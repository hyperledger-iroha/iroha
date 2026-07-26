//! Typed fixed-profile Lantern/LNP22 constraint toolbox.
//!
//! The presentation relation is reduced to the exact 642 scalar evaluation
//! constraints used by the fixed anonymous-credential profile:
//!
//! - 63 coefficients force each of `beta3` and `beta4` to be constant;
//! - one positive integer equation proves the two norm-slack polynomials are
//!   binary;
//! - one positive integer equation proves the tag and attributes are binary;
//! - two equations bind the exact squared norms and their binary slack;
//! - 256 ternary projections bind `z4` to the lifted application relation;
//! - 256 ternary projections bind `z3` to the binary and norm witnesses.
//!
//! Four independently weighted scalar accumulators are embedded at
//! coefficients zero and 32 of two ring polynomials.  The generic quadratic
//! opening proof consumes those two equations plus the two sign equations.

use sha3::{Digest, Sha3_256};
use thiserror::Error;
use zeroize::Zeroize;

use super::{
    params::{
        APPLICATION_MODULUS_INVERSE_IN_PROOF_V1, APPLICATION_RING_DEGREE_V1, APPLICATION_ROWS_V1,
        APPLICATION_WITNESS_POLYNOMIALS_V1, BINARY_POLYNOMIALS_V1, PROOF_INVERSE_TWO_V1,
        PROOF_MODULUS_V1, RANDOMNESS_NORM_SQUARED_BOUND_V1, SIGNATURE_NORM_SQUARED_BOUND_V1,
        TBOX_KMSIS_V1, TBOX_LEXT_V1, TBOX_M1_V1, TBOX_M2_V1,
    },
    relation::{
        BootleLanternApplicationRelationV1, BootleLanternPresentationWitnessV1,
        canonical_witness_vector_v1, validate_presentation_witness_v1,
    },
    ring::ProofPolynomialV1,
    transcript::{
        MatrixRoleV1, PresentationTranscriptV1, ProofMatrixV1, TranscriptErrorV1,
        expand_proof_matrix_v1,
    },
};

/// Number of polynomials in the first, public part of `s2`.
pub const S21_POLYNOMIALS_V1: usize = TBOX_M2_V1 - TBOX_KMSIS_V1;
/// Number of projected field coordinates.
pub const PROJECTION_COORDINATES_V1: usize = 256;
/// Number of proof-ring polynomials carrying one projected vector.
pub const PROJECTION_POLYNOMIALS_V1: usize = PROJECTION_COORDINATES_V1 / APPLICATION_RING_DEGREE_V1;
/// Number of secret extended-message polynomials consumed by the quadratic
/// equation.  The twelfth `tB` row commits its linearization and is public.
pub const QUADRATIC_MESSAGE_POLYNOMIALS_V1: usize = TBOX_LEXT_V1 - 1;
/// Exact number of scalar evaluation constraints.
pub const EVALUATION_CONSTRAINTS_V1: usize =
    2 * (APPLICATION_RING_DEGREE_V1 - 1) + 2 + 2 + 2 * PROJECTION_COORDINATES_V1;
/// Number of independent Schwartz accumulators.
pub const SCHWARTZ_ACCUMULATORS_V1: usize = 4;
/// Number of full ring equations after Schwartz compression.
pub const COMBINED_QUADRATIC_EQUATIONS_V1: usize = 4;

const BETA3_SHAPE_START_V1: usize = 0;
const BETA4_SHAPE_START_V1: usize = BETA3_SHAPE_START_V1 + APPLICATION_RING_DEGREE_V1 - 1;
const SLACK_BINARY_INDEX_V1: usize = BETA4_SHAPE_START_V1 + APPLICATION_RING_DEGREE_V1 - 1;
const CREDENTIAL_BINARY_INDEX_V1: usize = SLACK_BINARY_INDEX_V1 + 1;
const RANDOMNESS_NORM_INDEX_V1: usize = CREDENTIAL_BINARY_INDEX_V1 + 1;
const SIGNATURE_NORM_INDEX_V1: usize = RANDOMNESS_NORM_INDEX_V1 + 1;
const Z4_PROJECTION_START_V1: usize = SIGNATURE_NORM_INDEX_V1 + 1;
const Z3_PROJECTION_START_V1: usize = Z4_PROJECTION_START_V1 + PROJECTION_COORDINATES_V1;

const BINARY_SHORT_INDICES_V1: [usize; BINARY_POLYNOMIALS_V1] = [
    16, 17, 18, 19, 20, 21, 22, 23, 40, 41, 42, 43, 44, 45, 46, 47,
];
const RANDOMNESS_SHORT_START_V1: usize = 0;
const RANDOMNESS_SHORT_END_V1: usize = 16;
const SIGNATURE_SHORT_START_V1: usize = 24;
const SIGNATURE_SHORT_END_V1: usize = 40;
const RANDOMNESS_SLACK_INDEX_V1: usize = 48;
const SIGNATURE_SLACK_INDEX_V1: usize = 49;
const Y3_MESSAGE_START_V1: usize = 0;
const Y4_MESSAGE_START_V1: usize = 4;
const BETA_MESSAGE_INDEX_V1: usize = 8;
const G_MESSAGE_START_V1: usize = 9;

const RELATION_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.bootle-lantern.application-relation.v1";

/// The 50 short witness polynomials, zeroized on drop.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ShortWitnessV1 {
    polynomials: [ProofPolynomialV1; TBOX_M1_V1],
}

impl core::fmt::Debug for ShortWitnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ShortWitnessV1")
            .field("polynomials", &"<redacted>")
            .finish()
    }
}

impl ShortWitnessV1 {
    #[must_use]
    pub(crate) const fn polynomials(&self) -> &[ProofPolynomialV1; TBOX_M1_V1] {
        &self.polynomials
    }
}

impl Zeroize for ShortWitnessV1 {
    fn zeroize(&mut self) {
        self.polynomials.zeroize();
    }
}

impl Drop for ShortWitnessV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Secret variables of the generic quadratic equation.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QuadraticVariablesV1 {
    pub(crate) short: [ProofPolynomialV1; TBOX_M1_V1],
    pub(crate) message: [ProofPolynomialV1; QUADRATIC_MESSAGE_POLYNOMIALS_V1],
}

impl core::fmt::Debug for QuadraticVariablesV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QuadraticVariablesV1")
            .field("variables", &"<redacted>")
            .finish()
    }
}

impl QuadraticVariablesV1 {
    /// Add component-wise in the proof ring.
    #[must_use]
    pub(crate) fn add(&self, rhs: &Self) -> Self {
        Self {
            short: core::array::from_fn(|index| self.short[index].add(rhs.short[index])),
            message: core::array::from_fn(|index| self.message[index].add(rhs.message[index])),
        }
    }

    /// Negate component-wise in the proof ring.
    #[must_use]
    pub(crate) fn negate(&self) -> Self {
        Self {
            short: self.short.map(ProofPolynomialV1::negate),
            message: self.message.map(ProofPolynomialV1::negate),
        }
    }

    /// All-zero variable vector.
    #[must_use]
    pub(crate) const fn zero() -> Self {
        Self {
            short: [ProofPolynomialV1::ZERO; TBOX_M1_V1],
            message: [ProofPolynomialV1::ZERO; QUADRATIC_MESSAGE_POLYNOMIALS_V1],
        }
    }
}

impl Zeroize for QuadraticVariablesV1 {
    fn zeroize(&mut self) {
        self.short.zeroize();
        self.message.zeroize();
    }
}

impl Drop for QuadraticVariablesV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Pre-expanded transparent matrices used by one proof or verification.
#[derive(Clone, Debug)]
pub(crate) struct InternalMatricesV1 {
    pub(crate) a1: ProofMatrixV1,
    pub(crate) a2_prime: ProofMatrixV1,
    pub(crate) b_prime: ProofMatrixV1,
}

impl InternalMatricesV1 {
    /// Expand all fixed internal matrices from the transcript-bound seed.
    pub(crate) fn expand(transcript: &PresentationTranscriptV1) -> Result<Self, ToolboxErrorV1> {
        let seed = transcript.matrix_seed();
        Ok(Self {
            a1: expand_proof_matrix_v1(seed, MatrixRoleV1::InternalA1)
                .map_err(ToolboxErrorV1::Transcript)?,
            a2_prime: expand_proof_matrix_v1(seed, MatrixRoleV1::InternalA2Prime)
                .map_err(ToolboxErrorV1::Transcript)?,
            b_prime: expand_proof_matrix_v1(seed, MatrixRoleV1::InternalBPrime)
                .map_err(ToolboxErrorV1::Transcript)?,
        })
    }
}

/// Public material required to evaluate the one combined quadratic equation.
pub(crate) struct QuadraticEquationV1<'a> {
    relation: &'a BootleLanternApplicationRelationV1,
    projection_r: Box<[i8]>,
    projection_r_prime: Box<[i8]>,
    z3: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
    z4: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
    h: [ProofPolynomialV1; 2],
    schwartz_weights: Box<[u64]>,
    equation_multipliers: [ProofPolynomialV1; COMBINED_QUADRATIC_EQUATIONS_V1],
}

impl core::fmt::Debug for QuadraticEquationV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QuadraticEquationV1")
            .field("constraints", &EVALUATION_CONSTRAINTS_V1)
            .finish_non_exhaustive()
    }
}

impl<'a> QuadraticEquationV1<'a> {
    /// Construct one fully shaped public equation.
    pub(crate) fn new(
        relation: &'a BootleLanternApplicationRelationV1,
        projection_r: Box<[i8]>,
        projection_r_prime: Box<[i8]>,
        z3: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
        z4: [ProofPolynomialV1; PROJECTION_POLYNOMIALS_V1],
        h: [ProofPolynomialV1; 2],
        schwartz_weights: Box<[u64]>,
        equation_multipliers: [ProofPolynomialV1; COMBINED_QUADRATIC_EQUATIONS_V1],
    ) -> Result<Self, ToolboxErrorV1> {
        let expected_r = PROJECTION_COORDINATES_V1
            .checked_mul(50 * APPLICATION_RING_DEGREE_V1)
            .ok_or(ToolboxErrorV1::ArithmeticOverflow)?;
        let expected_r_prime = PROJECTION_COORDINATES_V1
            .checked_mul(APPLICATION_ROWS_V1 * APPLICATION_RING_DEGREE_V1)
            .ok_or(ToolboxErrorV1::ArithmeticOverflow)?;
        let expected_weights = SCHWARTZ_ACCUMULATORS_V1
            .checked_mul(EVALUATION_CONSTRAINTS_V1)
            .ok_or(ToolboxErrorV1::ArithmeticOverflow)?;
        if projection_r.len() != expected_r
            || projection_r_prime.len() != expected_r_prime
            || schwartz_weights.len() != expected_weights
        {
            return Err(ToolboxErrorV1::InvalidShape);
        }
        if projection_r
            .iter()
            .chain(projection_r_prime.iter())
            .any(|coefficient| !(-1..=1).contains(coefficient))
            || schwartz_weights
                .iter()
                .any(|coefficient| *coefficient >= PROOF_MODULUS_V1)
        {
            return Err(ToolboxErrorV1::NonCanonicalPublicInput);
        }
        Ok(Self {
            relation,
            projection_r,
            projection_r_prime,
            z3,
            z4,
            h,
            schwartz_weights,
            equation_multipliers,
        })
    }

    /// Install the public Schwartz commitments and the final equation
    /// multipliers after both have been transcript-derived.
    pub(crate) fn bind_final_equations(
        &mut self,
        h: [ProofPolynomialV1; 2],
        equation_multipliers: [ProofPolynomialV1; COMBINED_QUADRATIC_EQUATIONS_V1],
    ) {
        self.h = h;
        self.equation_multipliers = equation_multipliers;
    }

    /// Evaluate the exact 642 scalar constraints.
    pub(crate) fn constraints(
        &self,
        variables: &QuadraticVariablesV1,
    ) -> Result<[u64; EVALUATION_CONSTRAINTS_V1], ToolboxErrorV1> {
        let mut constraints = [0_u64; EVALUATION_CONSTRAINTS_V1];
        let (beta3, beta4) = beta_polynomials_v1(variables.message[BETA_MESSAGE_INDEX_V1])?;

        constraints[BETA3_SHAPE_START_V1..BETA4_SHAPE_START_V1]
            .copy_from_slice(&beta3.coefficients()[1..]);
        constraints[BETA4_SHAPE_START_V1..SLACK_BINARY_INDEX_V1]
            .copy_from_slice(&beta4.coefficients()[1..]);
        constraints[SLACK_BINARY_INDEX_V1] = binary_inner_product(&[
            variables.short[RANDOMNESS_SLACK_INDEX_V1],
            variables.short[SIGNATURE_SLACK_INDEX_V1],
        ]);
        constraints[CREDENTIAL_BINARY_INDEX_V1] =
            binary_inner_product(&BINARY_SHORT_INDICES_V1.map(|index| variables.short[index]));
        constraints[RANDOMNESS_NORM_INDEX_V1] = norm_slack_equation(
            &variables.short[RANDOMNESS_SHORT_START_V1..RANDOMNESS_SHORT_END_V1],
            variables.short[RANDOMNESS_SLACK_INDEX_V1],
            RANDOMNESS_NORM_SQUARED_BOUND_V1,
        );
        constraints[SIGNATURE_NORM_INDEX_V1] = norm_slack_equation(
            &variables.short[SIGNATURE_SHORT_START_V1..SIGNATURE_SHORT_END_V1],
            variables.short[SIGNATURE_SLACK_INDEX_V1],
            SIGNATURE_NORM_SQUARED_BOUND_V1,
        );

        let mut s4 = application_quotient_v1(self.relation, &variables.short)?;
        let mut s3 = projected_norm_witness_v1(&variables.short);
        let mut s4_coefficients = flatten_polynomials(&s4);
        let mut s3_coefficients = flatten_polynomials(&s3);
        let mut y3 = flatten_polynomials(
            &variables.message
                [Y3_MESSAGE_START_V1..Y3_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1],
        );
        let mut y4 = flatten_polynomials(
            &variables.message
                [Y4_MESSAGE_START_V1..Y4_MESSAGE_START_V1 + PROJECTION_POLYNOMIALS_V1],
        );
        let z3 = flatten_polynomials(&self.z3);
        let z4 = flatten_polynomials(&self.z4);
        let beta3_scalar = beta3.coefficients()[0];
        let beta4_scalar = beta4.coefficients()[0];

        let r_columns = s3_coefficients.len();
        let r_prime_columns = s4_coefficients.len();
        for row in 0..PROJECTION_COORDINATES_V1 {
            let r_prime_start = row * r_prime_columns;
            let projected_s4 = ternary_dot(
                &self.projection_r_prime[r_prime_start..r_prime_start + r_prime_columns],
                &s4_coefficients,
            );
            constraints[Z4_PROJECTION_START_V1 + row] = sub_mod(
                add_mod(mul_mod(beta4_scalar, projected_s4), y4[row]),
                z4[row],
            );

            let r_start = row * r_columns;
            let projected_s3 = ternary_dot(
                &self.projection_r[r_start..r_start + r_columns],
                &s3_coefficients,
            );
            constraints[Z3_PROJECTION_START_V1 + row] = sub_mod(
                add_mod(mul_mod(beta3_scalar, projected_s3), y3[row]),
                z3[row],
            );
        }
        s4.zeroize();
        s3.zeroize();
        s4_coefficients.zeroize();
        s3_coefficients.zeroize();
        y3.zeroize();
        y4.zeroize();
        Ok(constraints)
    }

    /// Return the two Schwartz-compressed constraint polynomials.
    pub(crate) fn schwartz_polynomials(
        &self,
        variables: &QuadraticVariablesV1,
    ) -> Result<[ProofPolynomialV1; 2], ToolboxErrorV1> {
        let constraints = self.constraints(variables)?;
        let accumulators: [u64; SCHWARTZ_ACCUMULATORS_V1] = core::array::from_fn(|accumulator| {
            let weights_start = accumulator * EVALUATION_CONSTRAINTS_V1;
            constraints
                .iter()
                .copied()
                .zip(
                    &self.schwartz_weights
                        [weights_start..weights_start + EVALUATION_CONSTRAINTS_V1],
                )
                .fold(0_u64, |sum, (constraint, weight)| {
                    add_mod(sum, mul_mod(constraint, *weight))
                })
        });
        Ok([
            polynomial_at_zero_and_half(accumulators[0], accumulators[1])?,
            polynomial_at_zero_and_half(accumulators[2], accumulators[3])?,
        ])
    }

    /// Evaluate the combined quadratic ring equation.
    pub(crate) fn evaluate(
        &self,
        variables: &QuadraticVariablesV1,
    ) -> Result<ProofPolynomialV1, ToolboxErrorV1> {
        let schwartz = self.schwartz_polynomials(variables)?;
        let (beta3, beta4) = beta_polynomials_v1(variables.message[BETA_MESSAGE_INDEX_V1])?;
        let one = ProofPolynomialV1::constant(1).map_err(|_| ToolboxErrorV1::InternalInvariant)?;
        let equations = [
            schwartz[0]
                .add(variables.message[G_MESSAGE_START_V1])
                .sub(self.h[0]),
            schwartz[1]
                .add(variables.message[G_MESSAGE_START_V1 + 1])
                .sub(self.h[1]),
            beta3.multiply(beta3).sub(one),
            beta4.multiply(beta4).sub(one),
        ];
        Ok(equations
            .into_iter()
            .zip(self.equation_multipliers)
            .fold(ProofPolynomialV1::ZERO, |sum, (equation, multiplier)| {
                sum.add(multiplier.multiply(equation))
            }))
    }
}

/// Compute the canonical digest of the complete compiled application
/// relation.
#[must_use]
pub fn application_relation_digest_v1(relation: &BootleLanternApplicationRelationV1) -> [u8; 32] {
    let mut hash = Sha3_256::new();
    hash.update(
        u32::try_from(RELATION_DIGEST_DOMAIN_V1.len())
            .expect("fixed digest domain fits u32")
            .to_be_bytes(),
    );
    hash.update(RELATION_DIGEST_DOMAIN_V1);
    hash.update(
        u16::try_from(relation.rows())
            .expect("fixed rows fit u16")
            .to_be_bytes(),
    );
    hash.update(
        u16::try_from(relation.columns())
            .expect("fixed columns fit u16")
            .to_be_bytes(),
    );
    hash.update([relation.disclosure_bitmap()]);
    for row in 0..relation.rows() {
        for column in 0..relation.columns() {
            let polynomial = relation
                .get(row, column)
                .expect("fixed relation coordinates exist");
            for coefficient in polynomial.coefficients() {
                hash.update(coefficient.to_le_bytes());
            }
        }
    }
    for polynomial in relation.public_offset() {
        for coefficient in polynomial.coefficients() {
            hash.update(coefficient.to_le_bytes());
        }
    }
    hash.finalize().into()
}

/// Validate and lift an application witness, including the two binary norm
/// slack polynomials.
pub(crate) fn lift_short_witness_v1(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
) -> Result<ShortWitnessV1, ToolboxErrorV1> {
    validate_presentation_witness_v1(relation, witness)
        .map_err(|_| ToolboxErrorV1::InvalidApplicationWitness)?;
    let application = canonical_witness_vector_v1(witness, relation.disclosure_bitmap());
    let mut polynomials = [ProofPolynomialV1::ZERO; TBOX_M1_V1];
    for (output, input) in polynomials[..APPLICATION_WITNESS_POLYNOMIALS_V1]
        .iter_mut()
        .zip(application)
    {
        *output = ProofPolynomialV1::from_application_centered(input);
    }

    let randomness_norm = application[RANDOMNESS_SHORT_START_V1..RANDOMNESS_SHORT_END_V1]
        .iter()
        .map(super::ring::ApplicationPolynomialV1::centered_squared_norm)
        .sum::<u64>();
    let signature_norm = application[SIGNATURE_SHORT_START_V1..SIGNATURE_SHORT_END_V1]
        .iter()
        .map(super::ring::ApplicationPolynomialV1::centered_squared_norm)
        .sum::<u64>();
    let randomness_slack = RANDOMNESS_NORM_SQUARED_BOUND_V1
        .checked_sub(randomness_norm)
        .ok_or(ToolboxErrorV1::InvalidApplicationWitness)?;
    let signature_slack = SIGNATURE_NORM_SQUARED_BOUND_V1
        .checked_sub(signature_norm)
        .ok_or(ToolboxErrorV1::InvalidApplicationWitness)?;
    polynomials[RANDOMNESS_SLACK_INDEX_V1] = binary_expansion_polynomial(randomness_slack);
    polynomials[SIGNATURE_SLACK_INDEX_V1] = binary_expansion_polynomial(signature_slack);
    Ok(ShortWitnessV1 { polynomials })
}

/// Matrix-vector product in the proof ring.
pub(crate) fn matrix_vector_product_v1(
    matrix: &ProofMatrixV1,
    vector: &[ProofPolynomialV1],
) -> Result<Vec<ProofPolynomialV1>, ToolboxErrorV1> {
    if vector.len() != usize::from(matrix.columns()) {
        return Err(ToolboxErrorV1::InvalidShape);
    }
    let mut output = vec![ProofPolynomialV1::ZERO; usize::from(matrix.rows())];
    for row in 0..usize::from(matrix.rows()) {
        for (column, polynomial) in vector.iter().copied().enumerate() {
            let coefficient = *matrix
                .get(
                    u16::try_from(row).map_err(|_| ToolboxErrorV1::InvalidShape)?,
                    u16::try_from(column).map_err(|_| ToolboxErrorV1::InvalidShape)?,
                )
                .ok_or(ToolboxErrorV1::InvalidShape)?;
            output[row] = output[row].add(coefficient.multiply(polynomial));
        }
    }
    Ok(output)
}

/// Compute `B' * vector` and add the supplied message vector.
pub(crate) fn commit_extended_messages_v1(
    b_prime: &ProofMatrixV1,
    s21: &[ProofPolynomialV1; S21_POLYNOMIALS_V1],
    messages: &[ProofPolynomialV1; TBOX_LEXT_V1],
) -> Result<[ProofPolynomialV1; TBOX_LEXT_V1], ToolboxErrorV1> {
    let product = matrix_vector_product_v1(b_prime, s21)?;
    if product.len() != TBOX_LEXT_V1 {
        return Err(ToolboxErrorV1::InvalidShape);
    }
    Ok(core::array::from_fn(|index| {
        product[index].add(messages[index])
    }))
}

/// Flatten canonical polynomial coefficients in polynomial-major order.
#[must_use]
pub(crate) fn flatten_polynomials(polynomials: &[ProofPolynomialV1]) -> Vec<u64> {
    let mut output = Vec::with_capacity(polynomials.len() * APPLICATION_RING_DEGREE_V1);
    for polynomial in polynomials {
        output.extend_from_slice(polynomial.coefficients());
    }
    output
}

/// Encode canonical proof polynomials without a header for transcript stages.
#[must_use]
pub(crate) fn encode_polynomials_v1(polynomials: &[ProofPolynomialV1]) -> Vec<u8> {
    let mut output = Vec::with_capacity(polynomials.len() * APPLICATION_RING_DEGREE_V1 * 7);
    for coefficient in flatten_polynomials(polynomials) {
        output.extend_from_slice(&coefficient.to_le_bytes()[..7]);
    }
    output
}

/// Expand all projection rows from a transcript stage.
pub(crate) fn expand_projection_matrix_v1(
    transcript: &PresentationTranscriptV1,
    stage: &[u8],
    components: &[&[u8]],
    columns: usize,
) -> Result<Box<[i8]>, ToolboxErrorV1> {
    let capacity = PROJECTION_COORDINATES_V1
        .checked_mul(columns)
        .ok_or(ToolboxErrorV1::ArithmeticOverflow)?;
    let mut output = Vec::with_capacity(capacity);
    for row in 0..PROJECTION_COORDINATES_V1 {
        output.extend(
            transcript
                .derive_ternary_row(
                    stage,
                    components,
                    u16::try_from(row).map_err(|_| ToolboxErrorV1::InvalidShape)?,
                    columns,
                )
                .map_err(ToolboxErrorV1::Transcript)?,
        );
    }
    Ok(output.into_boxed_slice())
}

fn beta_polynomials_v1(
    beta: ProofPolynomialV1,
) -> Result<(ProofPolynomialV1, ProofPolynomialV1), ToolboxErrorV1> {
    let beta_auto = beta.automorphism();
    let beta3 = beta
        .add(beta_auto)
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| ToolboxErrorV1::InternalInvariant)?;
    let beta4 = beta_auto
        .sub(beta)
        .multiply_by_monomial(APPLICATION_RING_DEGREE_V1 / 2)
        .scale_canonical(PROOF_INVERSE_TWO_V1)
        .map_err(|_| ToolboxErrorV1::InternalInvariant)?;
    Ok((beta3, beta4))
}

pub(crate) fn projected_norm_witness_v1(
    short: &[ProofPolynomialV1; TBOX_M1_V1],
) -> [ProofPolynomialV1; 50] {
    let mut output = [ProofPolynomialV1::ZERO; 50];
    for (destination, source) in BINARY_SHORT_INDICES_V1.into_iter().enumerate() {
        output[destination] = short[source];
    }
    output[16..32].copy_from_slice(&short[RANDOMNESS_SHORT_START_V1..RANDOMNESS_SHORT_END_V1]);
    output[32..48].copy_from_slice(&short[SIGNATURE_SHORT_START_V1..SIGNATURE_SHORT_END_V1]);
    output[48] = short[RANDOMNESS_SLACK_INDEX_V1];
    output[49] = short[SIGNATURE_SLACK_INDEX_V1];
    output
}

pub(crate) fn application_quotient_v1(
    relation: &BootleLanternApplicationRelationV1,
    short: &[ProofPolynomialV1; TBOX_M1_V1],
) -> Result<[ProofPolynomialV1; APPLICATION_ROWS_V1], ToolboxErrorV1> {
    let mut output = [ProofPolynomialV1::ZERO; APPLICATION_ROWS_V1];
    for row in 0..APPLICATION_ROWS_V1 {
        let mut equation =
            ProofPolynomialV1::from_application_centered(relation.public_offset()[row]);
        for (column, witness) in short[..APPLICATION_WITNESS_POLYNOMIALS_V1]
            .iter()
            .copied()
            .enumerate()
        {
            let coefficient = ProofPolynomialV1::from_application_centered(
                *relation
                    .get(row, column)
                    .ok_or(ToolboxErrorV1::InvalidShape)?,
            );
            equation = equation.add(coefficient.multiply(witness));
        }
        output[row] = equation
            .scale_canonical(APPLICATION_MODULUS_INVERSE_IN_PROOF_V1)
            .map_err(|_| ToolboxErrorV1::InternalInvariant)?;
    }
    Ok(output)
}

fn binary_inner_product(polynomials: &[ProofPolynomialV1]) -> u64 {
    polynomials
        .iter()
        .flat_map(ProofPolynomialV1::coefficients)
        .copied()
        .fold(0_u64, |sum, coefficient| {
            add_mod(sum, mul_mod(coefficient, sub_mod(coefficient, 1)))
        })
}

fn norm_slack_equation(
    polynomials: &[ProofPolynomialV1],
    slack: ProofPolynomialV1,
    bound: u64,
) -> u64 {
    let norm = polynomials
        .iter()
        .flat_map(ProofPolynomialV1::coefficients)
        .copied()
        .fold(0_u64, |sum, coefficient| {
            add_mod(sum, mul_mod(coefficient, coefficient))
        });
    let mut power = 1_u64;
    let mut slack_value = 0_u64;
    for bit in slack.coefficients() {
        slack_value = add_mod(slack_value, mul_mod(*bit, power));
        power = add_mod(power, power);
    }
    sub_mod(add_mod(norm, slack_value), bound % PROOF_MODULUS_V1)
}

fn binary_expansion_polynomial(value: u64) -> ProofPolynomialV1 {
    let coefficients = core::array::from_fn(|index| (value >> index) & 1);
    ProofPolynomialV1::new(coefficients).expect("binary residues are canonical")
}

fn polynomial_at_zero_and_half(zero: u64, half: u64) -> Result<ProofPolynomialV1, ToolboxErrorV1> {
    let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
    coefficients[0] = zero;
    coefficients[APPLICATION_RING_DEGREE_V1 / 2] = half;
    ProofPolynomialV1::new(coefficients).map_err(|_| ToolboxErrorV1::InternalInvariant)
}

fn ternary_dot(row: &[i8], vector: &[u64]) -> u64 {
    row.iter()
        .copied()
        .zip(vector.iter().copied())
        .fold(0_u64, |sum, (coefficient, value)| match coefficient {
            -1 => sub_mod(sum, value),
            0 => sum,
            1 => add_mod(sum, value),
            _ => unreachable!("validated ternary matrix"),
        })
}

fn add_mod(lhs: u64, rhs: u64) -> u64 {
    u64::try_from((u128::from(lhs) + u128::from(rhs)) % u128::from(PROOF_MODULUS_V1))
        .expect("reduced residue fits u64")
}

fn sub_mod(lhs: u64, rhs: u64) -> u64 {
    if lhs >= rhs {
        lhs - rhs
    } else {
        PROOF_MODULUS_V1 - (rhs - lhs)
    }
}

fn mul_mod(lhs: u64, rhs: u64) -> u64 {
    u64::try_from(u128::from(lhs) * u128::from(rhs) % u128::from(PROOF_MODULUS_V1))
        .expect("reduced residue fits u64")
}

/// Fixed-profile constraint-toolbox failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ToolboxErrorV1 {
    /// The application witness failed its exact relation or bound.
    #[error("Bootle/Lantern application witness is invalid")]
    InvalidApplicationWitness,
    /// A fixed vector or matrix had the wrong shape.
    #[error("Bootle/Lantern proof toolbox shape mismatch")]
    InvalidShape,
    /// A public projection, weight, or residue was non-canonical.
    #[error("Bootle/Lantern proof toolbox public input is non-canonical")]
    NonCanonicalPublicInput,
    /// Transcript expansion failed.
    #[error("Bootle/Lantern proof toolbox transcript expansion failed: {0}")]
    Transcript(TranscriptErrorV1),
    /// Checked size arithmetic overflowed.
    #[error("Bootle/Lantern proof toolbox size arithmetic overflowed")]
    ArithmeticOverflow,
    /// A fixed internal invariant failed.
    #[error("Bootle/Lantern proof toolbox internal invariant failed")]
    InternalInvariant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_constraint_layout_is_exact() {
        assert_eq!(S21_POLYNOMIALS_V1, 44);
        assert_eq!(PROJECTION_POLYNOMIALS_V1, 4);
        assert_eq!(QUADRATIC_MESSAGE_POLYNOMIALS_V1, 11);
        assert_eq!(EVALUATION_CONSTRAINTS_V1, 642);
        assert_eq!(Z3_PROJECTION_START_V1 + PROJECTION_COORDINATES_V1, 642);
    }

    #[test]
    fn beta_extraction_is_exact_and_rejects_shape_noise() {
        for beta3_sign in [-1_i64, 1] {
            for beta4_sign in [-1_i64, 1] {
                let mut beta_coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
                beta_coefficients[0] = beta3_sign;
                beta_coefficients[APPLICATION_RING_DEGREE_V1 / 2] = beta4_sign;
                let beta = ProofPolynomialV1::from_centered_coefficients(beta_coefficients);
                let (beta3, beta4) = beta_polynomials_v1(beta).expect("fixed inverse");
                assert_eq!(beta3.centered_coefficient(0), beta3_sign);
                assert_eq!(beta4.centered_coefficient(0), beta4_sign);
                assert!(
                    beta3.coefficients()[1..]
                        .iter()
                        .all(|coefficient| *coefficient == 0)
                );
                assert!(
                    beta4.coefficients()[1..]
                        .iter()
                        .all(|coefficient| *coefficient == 0)
                );
            }
        }

        let mut noisy = [0_i64; APPLICATION_RING_DEGREE_V1];
        noisy[1] = 7;
        let (beta3, beta4) =
            beta_polynomials_v1(ProofPolynomialV1::from_centered_coefficients(noisy))
                .expect("fixed inverse");
        assert!(
            beta3.coefficients()[1..]
                .iter()
                .chain(&beta4.coefficients()[1..])
                .any(|coefficient| *coefficient != 0)
        );
    }

    #[test]
    fn binary_and_norm_equations_detect_adversarial_coefficients() {
        let binary = binary_expansion_polynomial(34_034_725);
        assert_eq!(binary_inner_product(&[binary]), 0);
        assert_eq!(
            norm_slack_equation(
                &[ProofPolynomialV1::constant(1).expect("one")],
                binary,
                34_034_726,
            ),
            0
        );

        let mut non_binary = [0_u64; APPLICATION_RING_DEGREE_V1];
        non_binary[17] = 2;
        let non_binary = ProofPolynomialV1::new(non_binary).expect("canonical");
        assert_ne!(binary_inner_product(&[non_binary]), 0);
    }

    #[test]
    fn quadratic_black_box_decomposition_identity_holds() {
        // Q(x) = x^2 + 7x + 11, embedded as constant polynomials.
        fn q(x: ProofPolynomialV1) -> ProofPolynomialV1 {
            x.multiply(x)
                .add(x.scale_centered(7))
                .add(ProofPolynomialV1::constant(11).expect("constant"))
        }
        let x = ProofPolynomialV1::constant(19).expect("x");
        let zero = ProofPolynomialV1::ZERO;
        let q0 = q(zero);
        let q2 = q(x)
            .add(q(x.negate()))
            .sub(q0.scale_centered(2))
            .scale_canonical(PROOF_INVERSE_TWO_V1)
            .expect("inverse");
        let linear = q(x)
            .sub(q(x.negate()))
            .scale_canonical(PROOF_INVERSE_TWO_V1)
            .expect("inverse");
        assert_eq!(q2, x.multiply(x));
        assert_eq!(linear, x.scale_centered(7));
        assert_eq!(q(x), q2.add(linear).add(q0));
    }
}
