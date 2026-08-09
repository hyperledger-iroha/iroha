//! Fixed BLNS credential relation compiled into application-ring linear form.
//!
//! The native Lantern prover consumes exactly the 8-by-48 relation
//!
//! `A_r r + A_tau tau - I s1 - B s2 + A_m,hidden m_hidden
//!      + A_m,public m_public = 0`.
//!
//! Publicly disclosed attributes are removed from the witness matrix and
//! accumulated in the public offset. This fixed-width zero-column technique
//! preserves one canonical witness layout for every disclosure bitmap.

use iroha_data_model::privacy::{
    BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternIssuerPolicyV1,
    IrohaBootleLanternAnoncredStatementV1,
};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::{
    params::{
        APPLICATION_ROWS_V1, APPLICATION_WITNESS_POLYNOMIALS_V1, RANDOMNESS_NORM_SQUARED_BOUND_V1,
        SIGNATURE_NORM_SQUARED_BOUND_V1,
    },
    ring::ApplicationPolynomialV1,
    transcript::{MatrixRoleV1, MatrixSeedV1, expand_application_matrix_v1},
};

const RANDOMNESS_POLYNOMIALS_V1: usize = 16;
const TAG_POLYNOMIALS_V1: usize = 8;
const SIGNATURE_HALF_POLYNOMIALS_V1: usize = 8;
const ATTRIBUTE_POLYNOMIALS_V1: usize = 8;

const RANDOMNESS_START_V1: usize = 0;
const TAG_START_V1: usize = RANDOMNESS_START_V1 + RANDOMNESS_POLYNOMIALS_V1;
const SIGNATURE_ONE_START_V1: usize = TAG_START_V1 + TAG_POLYNOMIALS_V1;
const SIGNATURE_TWO_START_V1: usize = SIGNATURE_ONE_START_V1 + SIGNATURE_HALF_POLYNOMIALS_V1;
const ATTRIBUTE_START_V1: usize = SIGNATURE_TWO_START_V1 + SIGNATURE_HALF_POLYNOMIALS_V1;

/// Secret opening of one issued credential.
///
/// Attributes retain their direct 64-bit form; conversion to binary ring
/// polynomials occurs only inside the canonical relation compiler.
#[derive(Clone, PartialEq, Eq)]
pub struct BootleLanternPresentationWitnessV1 {
    /// Credential commitment randomness `r`.
    pub randomness: [ApplicationPolynomialV1; RANDOMNESS_POLYNOMIALS_V1],
    /// Binary issuer nonce/tag `tau`.
    pub tag: [ApplicationPolynomialV1; TAG_POLYNOMIALS_V1],
    /// First signature-preimage half `s1`.
    pub signature_one: [ApplicationPolynomialV1; SIGNATURE_HALF_POLYNOMIALS_V1],
    /// Second signature-preimage half `s2`.
    pub signature_two: [ApplicationPolynomialV1; SIGNATURE_HALF_POLYNOMIALS_V1],
    /// All eight direct credential attributes.
    pub attributes: [[u8; 8]; ATTRIBUTE_POLYNOMIALS_V1],
}

impl core::fmt::Debug for BootleLanternPresentationWitnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BootleLanternPresentationWitnessV1")
            .field("secret", &"<redacted>")
            .finish()
    }
}

impl Zeroize for BootleLanternPresentationWitnessV1 {
    fn zeroize(&mut self) {
        self.randomness.zeroize();
        self.tag.zeroize();
        self.signature_one.zeroize();
        self.signature_two.zeroize();
        self.attributes.zeroize();
    }
}

impl Drop for BootleLanternPresentationWitnessV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Canonical secret application witness, zeroized before heap release.
pub(crate) struct CanonicalSecretWitnessVectorV1 {
    polynomials: Box<[ApplicationPolynomialV1; APPLICATION_WITNESS_POLYNOMIALS_V1]>,
}

impl CanonicalSecretWitnessVectorV1 {
    fn zero() -> Self {
        Self {
            polynomials: Box::new(
                [ApplicationPolynomialV1::ZERO; APPLICATION_WITNESS_POLYNOMIALS_V1],
            ),
        }
    }

    /// Borrow the fixed canonical secret ordering.
    #[must_use]
    pub(crate) fn polynomials(
        &self,
    ) -> &[ApplicationPolynomialV1; APPLICATION_WITNESS_POLYNOMIALS_V1] {
        &self.polynomials
    }
}

impl core::fmt::Debug for CanonicalSecretWitnessVectorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("CanonicalSecretWitnessVectorV1(<redacted>)")
    }
}

impl Zeroize for CanonicalSecretWitnessVectorV1 {
    fn zeroize(&mut self) {
        self.polynomials.as_mut().zeroize();
    }
}

impl Drop for CanonicalSecretWitnessVectorV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Canonical public 8-by-48 application relation for one presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootleLanternApplicationRelationV1 {
    matrix: Box<[ApplicationPolynomialV1]>,
    public_offset: [ApplicationPolynomialV1; APPLICATION_ROWS_V1],
    disclosure_bitmap: u8,
    disclosed_attributes: [Option<[u8; 8]>; ATTRIBUTE_POLYNOMIALS_V1],
}

impl BootleLanternApplicationRelationV1 {
    /// Fixed row count.
    #[must_use]
    pub const fn rows(&self) -> usize {
        APPLICATION_ROWS_V1
    }

    /// Fixed witness-column count.
    #[must_use]
    pub const fn columns(&self) -> usize {
        APPLICATION_WITNESS_POLYNOMIALS_V1
    }

    /// Borrow one matrix polynomial.
    #[must_use]
    pub fn get(&self, row: usize, column: usize) -> Option<&ApplicationPolynomialV1> {
        if row >= self.rows() || column >= self.columns() {
            return None;
        }
        self.matrix
            .get(row * APPLICATION_WITNESS_POLYNOMIALS_V1 + column)
    }

    /// Borrow the eight-polynomial public offset.
    #[must_use]
    pub const fn public_offset(&self) -> &[ApplicationPolynomialV1; APPLICATION_ROWS_V1] {
        &self.public_offset
    }

    /// Exact disclosure bitmap compiled into this relation.
    #[must_use]
    pub const fn disclosure_bitmap(&self) -> u8 {
        self.disclosure_bitmap
    }

    /// Public value at one disclosed index.
    #[must_use]
    pub fn disclosed_attribute(&self, index: usize) -> Option<[u8; 8]> {
        self.disclosed_attributes.get(index).copied().flatten()
    }
}

/// Compile trusted policy and typed statement into the unique public linear
/// relation consumed by the Lantern proof system.
///
/// # Errors
///
/// Rejects an intrinsically invalid policy, any statement-to-record mismatch,
/// a missing required disclosure, a disallowed public value, a global
/// parameter-seed mismatch, or transparent-matrix expansion failure.
pub fn compile_application_relation_v1(
    statement: &IrohaBootleLanternAnoncredStatementV1,
    policy: &BootleLanternIssuerPolicyV1,
    matrix_seed: MatrixSeedV1,
    canonical_genesis_hash: [u8; 32],
) -> Result<BootleLanternApplicationRelationV1, RelationErrorV1> {
    policy
        .validate()
        .map_err(|_| RelationErrorV1::InvalidIssuerPolicy)?;
    if statement.issuer_id != policy.issuer_id {
        return Err(RelationErrorV1::IssuerMismatch);
    }
    if statement.policy_id != policy.policy_id {
        return Err(RelationErrorV1::PolicyMismatch);
    }
    if statement.issuer_policy_epoch != policy.epoch {
        return Err(RelationErrorV1::IssuerPolicyEpochMismatch);
    }
    if statement.issuer_policy_record_digest != policy.record_digest {
        return Err(RelationErrorV1::IssuerPolicyRecordDigestMismatch);
    }
    if statement.issuer_parameter_id != policy.issuer_parameter_id {
        return Err(RelationErrorV1::IssuerParameterIdMismatch);
    }
    if statement.issuer_parameter_digest != policy.issuer_parameter_digest {
        return Err(RelationErrorV1::IssuerParameterDigestMismatch);
    }
    if statement.context.parameter_digest.as_bytes() != matrix_seed.parameter_digest() {
        return Err(RelationErrorV1::MatrixParameterDigestMismatch);
    }
    let credential_scope = super::scope::BootleLanternCredentialScopeV1::new(
        &statement.context,
        canonical_genesis_hash,
        policy,
    )
    .map_err(|_| RelationErrorV1::InvalidCredentialScope)?;
    let credential_scope_term = credential_scope
        .application_term()
        .map_err(|_| RelationErrorV1::InvalidCredentialScope)?;

    let mut disclosed_attributes = [None; ATTRIBUTE_POLYNOMIALS_V1];
    let mut disclosure_bitmap = 0_u8;
    let mut previous = None;
    for disclosure in &statement.disclosures {
        let index = usize::from(disclosure.index);
        if index >= BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1 {
            return Err(RelationErrorV1::DisclosureIndexOutOfRange);
        }
        if previous.is_some_and(|previous| disclosure.index <= previous) {
            return Err(RelationErrorV1::DisclosuresNotStrictlyIncreasing);
        }
        previous = Some(disclosure.index);
        let value = *disclosure.value.as_bytes();
        disclosed_attributes[index] = Some(value);
        disclosure_bitmap |= 1_u8 << index;

        let allowed = &policy.allowed_values[index].values;
        if !allowed.is_empty()
            && allowed
                .binary_search_by_key(&value, |candidate| *candidate.as_bytes())
                .is_err()
        {
            return Err(RelationErrorV1::DisclosedValueNotAllowed {
                index: disclosure.index,
            });
        }
    }
    let missing_required = policy.required_disclosure_bitmap & !disclosure_bitmap;
    if missing_required != 0 {
        return Err(RelationErrorV1::MissingRequiredDisclosure {
            index: u8::try_from(missing_required.trailing_zeros())
                .expect("u8 bitmap position fits u8"),
        });
    }

    let ar = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationRandomness)
        .map_err(|_| RelationErrorV1::MatrixExpansion)?;
    let am = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationAttributes)
        .map_err(|_| RelationErrorV1::MatrixExpansion)?;
    let a_tau = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationTag)
        .map_err(|_| RelationErrorV1::MatrixExpansion)?;
    let issuer_matrix = decode_issuer_matrix(policy)?;

    let mut matrix = vec![
        ApplicationPolynomialV1::ZERO;
        APPLICATION_ROWS_V1 * APPLICATION_WITNESS_POLYNOMIALS_V1
    ];
    let mut public_offset = credential_scope_term;
    let minus_one = ApplicationPolynomialV1::constant(super::params::APPLICATION_MODULUS_V1 - 1)
        .map_err(|_| RelationErrorV1::InternalInvariant)?;

    for row in 0..APPLICATION_ROWS_V1 {
        for column in 0..RANDOMNESS_POLYNOMIALS_V1 {
            matrix[matrix_index(row, RANDOMNESS_START_V1 + column)] = *ar
                .get(
                    u16::try_from(row).expect("row fits u16"),
                    u16::try_from(column).expect("column fits u16"),
                )
                .ok_or(RelationErrorV1::InternalInvariant)?;
        }
        for column in 0..TAG_POLYNOMIALS_V1 {
            matrix[matrix_index(row, TAG_START_V1 + column)] = *a_tau
                .get(
                    u16::try_from(row).expect("row fits u16"),
                    u16::try_from(column).expect("column fits u16"),
                )
                .ok_or(RelationErrorV1::InternalInvariant)?;
        }
        matrix[matrix_index(row, SIGNATURE_ONE_START_V1 + row)] = minus_one;
        for column in 0..SIGNATURE_HALF_POLYNOMIALS_V1 {
            matrix[matrix_index(row, SIGNATURE_TWO_START_V1 + column)] =
                issuer_matrix[row][column].negate();
        }
        for column in 0..ATTRIBUTE_POLYNOMIALS_V1 {
            let coefficient = *am
                .get(
                    u16::try_from(row).expect("row fits u16"),
                    u16::try_from(column).expect("column fits u16"),
                )
                .ok_or(RelationErrorV1::InternalInvariant)?;
            if let Some(attribute) = disclosed_attributes[column] {
                public_offset[row] = public_offset[row].add(
                    coefficient.multiply(ApplicationPolynomialV1::from_direct_attribute(attribute)),
                );
            } else {
                matrix[matrix_index(row, ATTRIBUTE_START_V1 + column)] = coefficient;
            }
        }
    }

    Ok(BootleLanternApplicationRelationV1 {
        matrix: matrix.into_boxed_slice(),
        public_offset,
        disclosure_bitmap,
        disclosed_attributes,
    })
}

/// Compile the holder's blind-issuance request relation
/// `A_r*r + A_m*m - t = 0` into the fixed 8-by-48 proof shape.
pub(crate) fn compile_blind_issuance_request_relation_v1(
    matrix_seed: MatrixSeedV1,
    target: &[ApplicationPolynomialV1; APPLICATION_ROWS_V1],
) -> Result<BootleLanternApplicationRelationV1, RelationErrorV1> {
    let ar = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationRandomness)
        .map_err(|_| RelationErrorV1::MatrixExpansion)?;
    let am = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationAttributes)
        .map_err(|_| RelationErrorV1::MatrixExpansion)?;
    let mut matrix = vec![
        ApplicationPolynomialV1::ZERO;
        APPLICATION_ROWS_V1 * APPLICATION_WITNESS_POLYNOMIALS_V1
    ];
    for row in 0..APPLICATION_ROWS_V1 {
        for column in 0..RANDOMNESS_POLYNOMIALS_V1 {
            matrix[matrix_index(row, RANDOMNESS_START_V1 + column)] = *ar
                .get(
                    u16::try_from(row).expect("row fits u16"),
                    u16::try_from(column).expect("column fits u16"),
                )
                .ok_or(RelationErrorV1::InternalInvariant)?;
        }
        for column in 0..ATTRIBUTE_POLYNOMIALS_V1 {
            matrix[matrix_index(row, ATTRIBUTE_START_V1 + column)] = *am
                .get(
                    u16::try_from(row).expect("row fits u16"),
                    u16::try_from(column).expect("column fits u16"),
                )
                .ok_or(RelationErrorV1::InternalInvariant)?;
        }
    }
    Ok(BootleLanternApplicationRelationV1 {
        matrix: matrix.into_boxed_slice(),
        public_offset: target.map(ApplicationPolynomialV1::negate),
        disclosure_bitmap: 0,
        disclosed_attributes: [None; ATTRIBUTE_POLYNOMIALS_V1],
    })
}

/// Validate a complete credential witness against its compiled public
/// relation and both exact squared-norm bounds.
///
/// # Errors
///
/// Rejects non-binary `tau`, a public-attribute mismatch, either norm bound,
/// or the first non-zero application-equation row.
pub fn validate_presentation_witness_v1(
    relation: &BootleLanternApplicationRelationV1,
    witness: &BootleLanternPresentationWitnessV1,
) -> Result<(), RelationErrorV1> {
    for (index, tag) in witness.tag.iter().enumerate() {
        let _decoded_tag = Zeroizing::new(tag.to_direct_attribute().map_err(|_| {
            RelationErrorV1::NonBinaryTag {
                index: u8::try_from(index).expect("tag index fits u8"),
            }
        })?);
    }
    for (index, attribute) in witness.attributes.iter().enumerate() {
        if let Some(disclosed) = relation.disclosed_attribute(index)
            && disclosed != *attribute
        {
            return Err(RelationErrorV1::DisclosedAttributeMismatch {
                index: u8::try_from(index).expect("attribute index fits u8"),
            });
        }
    }

    let randomness_norm = Zeroizing::new(
        witness
            .randomness
            .iter()
            .map(ApplicationPolynomialV1::centered_squared_norm)
            .sum::<u64>(),
    );
    if *randomness_norm > RANDOMNESS_NORM_SQUARED_BOUND_V1 {
        return Err(RelationErrorV1::RandomnessNormExceeded);
    }
    let signature_norm = Zeroizing::new(
        witness
            .signature_one
            .iter()
            .chain(&witness.signature_two)
            .map(ApplicationPolynomialV1::centered_squared_norm)
            .sum::<u64>(),
    );
    if *signature_norm > SIGNATURE_NORM_SQUARED_BOUND_V1 {
        return Err(RelationErrorV1::SignatureNormExceeded);
    }

    let witness_vector = canonical_witness_vector_v1(witness, relation.disclosure_bitmap);
    for row in 0..APPLICATION_ROWS_V1 {
        let mut equation = Zeroizing::new(relation.public_offset[row]);
        for (column, witness_polynomial) in witness_vector.polynomials().iter().enumerate() {
            let matrix_polynomial = relation
                .get(row, column)
                .ok_or(RelationErrorV1::InternalInvariant)?;
            let product = Zeroizing::new(matrix_polynomial.multiply(*witness_polynomial));
            *equation = (*equation).add(*product);
        }
        if *equation != ApplicationPolynomialV1::ZERO {
            return Err(RelationErrorV1::ApplicationEquationFailed {
                row: u8::try_from(row).expect("row fits u8"),
            });
        }
    }
    Ok(())
}

/// Lift the canonical presentation witness into its fixed 48-polynomial
/// application-relation order.
///
/// Publicly disclosed attribute columns are represented by zero because their
/// values are already accumulated into the relation's public offset.
#[must_use]
pub(crate) fn canonical_witness_vector_v1(
    witness: &BootleLanternPresentationWitnessV1,
    disclosure_bitmap: u8,
) -> CanonicalSecretWitnessVectorV1 {
    let mut vector = CanonicalSecretWitnessVectorV1::zero();
    vector.polynomials[RANDOMNESS_START_V1..TAG_START_V1].copy_from_slice(&witness.randomness);
    vector.polynomials[TAG_START_V1..SIGNATURE_ONE_START_V1].copy_from_slice(&witness.tag);
    vector.polynomials[SIGNATURE_ONE_START_V1..SIGNATURE_TWO_START_V1]
        .copy_from_slice(&witness.signature_one);
    vector.polynomials[SIGNATURE_TWO_START_V1..ATTRIBUTE_START_V1]
        .copy_from_slice(&witness.signature_two);
    for (index, attribute) in witness.attributes.iter().enumerate() {
        if disclosure_bitmap & (1_u8 << index) == 0 {
            vector.polynomials[ATTRIBUTE_START_V1 + index] =
                ApplicationPolynomialV1::from_direct_attribute(*attribute);
        }
    }
    vector
}

fn decode_issuer_matrix(
    policy: &BootleLanternIssuerPolicyV1,
) -> Result<
    [[ApplicationPolynomialV1; SIGNATURE_HALF_POLYNOMIALS_V1]; APPLICATION_ROWS_V1],
    RelationErrorV1,
> {
    let mut matrix =
        [[ApplicationPolynomialV1::ZERO; SIGNATURE_HALF_POLYNOMIALS_V1]; APPLICATION_ROWS_V1];
    for (output, encoded) in matrix
        .iter_mut()
        .flatten()
        .zip(&policy.issuer_public_matrix.entries)
    {
        let coefficients: [u16; super::params::APPLICATION_RING_DEGREE_V1] = encoded
            .coefficients
            .as_slice()
            .try_into()
            .map_err(|_| RelationErrorV1::InternalInvariant)?;
        *output = ApplicationPolynomialV1::new(coefficients)
            .map_err(|_| RelationErrorV1::InternalInvariant)?;
    }
    Ok(matrix)
}

fn matrix_index(row: usize, column: usize) -> usize {
    row * APPLICATION_WITNESS_POLYNOMIALS_V1 + column
}

/// Fixed relation compilation or witness-validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RelationErrorV1 {
    /// The trusted record failed intrinsic canonical validation.
    #[error("Bootle/Lantern issuer policy is invalid")]
    InvalidIssuerPolicy,
    /// Statement issuer did not select the trusted record.
    #[error("Bootle/Lantern statement issuer does not match trusted policy")]
    IssuerMismatch,
    /// Statement policy id did not select the trusted record.
    #[error("Bootle/Lantern statement policy id does not match trusted policy")]
    PolicyMismatch,
    /// Statement epoch was stale or future.
    #[error("Bootle/Lantern statement issuer-policy epoch mismatch")]
    IssuerPolicyEpochMismatch,
    /// Statement record digest did not match committed state.
    #[error("Bootle/Lantern statement issuer-policy record digest mismatch")]
    IssuerPolicyRecordDigestMismatch,
    /// Statement issuer-parameter id did not match committed state.
    #[error("Bootle/Lantern statement issuer-parameter id mismatch")]
    IssuerParameterIdMismatch,
    /// Statement issuer-parameter digest did not match committed state.
    #[error("Bootle/Lantern statement issuer-parameter digest mismatch")]
    IssuerParameterDigestMismatch,
    /// Matrix seed was not for the statement's global parameter manifest.
    #[error("Bootle/Lantern matrix seed parameter digest mismatch")]
    MatrixParameterDigestMismatch,
    /// Reusable chain/governance scope could not be bound into the relation.
    #[error("Bootle/Lantern credential scope is invalid")]
    InvalidCredentialScope,
    /// Disclosure index was outside the fixed eight attributes.
    #[error("Bootle/Lantern disclosure index is outside 0..8")]
    DisclosureIndexOutOfRange,
    /// Disclosures were not strictly increasing.
    #[error("Bootle/Lantern disclosures must be strictly increasing")]
    DisclosuresNotStrictlyIncreasing,
    /// A required policy attribute was not public.
    #[error("Bootle/Lantern required attribute {index} was not disclosed")]
    MissingRequiredDisclosure {
        /// Missing index.
        index: u8,
    },
    /// A disclosed value was outside the governed allow-list.
    #[error("Bootle/Lantern disclosed attribute {index} is not allowed")]
    DisclosedValueNotAllowed {
        /// Rejected index.
        index: u8,
    },
    /// Transparent expansion failed.
    #[error("Bootle/Lantern matrix expansion failed")]
    MatrixExpansion,
    /// A `tau` polynomial was not binary.
    #[error("Bootle/Lantern tag polynomial {index} is not binary")]
    NonBinaryTag {
        /// Rejected polynomial index.
        index: u8,
    },
    /// Witness and public attribute differed.
    #[error("Bootle/Lantern witness attribute {index} differs from disclosure")]
    DisclosedAttributeMismatch {
        /// Mismatched index.
        index: u8,
    },
    /// `r` exceeded its exact squared-norm bound.
    #[error("Bootle/Lantern credential randomness exceeds its squared-norm bound")]
    RandomnessNormExceeded,
    /// `(s1,s2)` exceeded its exact squared-norm bound.
    #[error("Bootle/Lantern signature preimage exceeds its squared-norm bound")]
    SignatureNormExceeded,
    /// One application row did not equal zero.
    #[error("Bootle/Lantern application relation row {row} is non-zero")]
    ApplicationEquationFailed {
        /// First failing row.
        row: u8,
    },
    /// A fixed internal shape or canonicality invariant failed.
    #[error("Bootle/Lantern internal relation invariant failed")]
    InternalInvariant,
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use iroha_data_model::privacy::{
        BootleLanternAllowedAttributeValuesV1, BootleLanternAttributeValueV1,
        BootleLanternDisclosedAttributeV1, PrivacyBootleLanternIssuerPolicyDigestV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    };
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::privacy_engines::bootle_lantern::{
        issuer::{
            BootleLanternInMemoryIssuanceStoreV1, BootleLanternIssuerKeyPairV1,
            BootleLanternIssuerPolicyMetadataV1, holder_finalize_blind_issuance_v1,
            holder_prepare_blind_issuance_with_rng_v1, issuer_authorize_blind_issuance_with_rng_v1,
            issuer_blind_issue_once_with_rng_v1,
        },
        transcript::matrix_seed_v1,
    };

    struct TestRng {
        seed: [u8; 32],
        counter: u64,
    }

    impl TestRng {
        const fn new(seed: [u8; 32]) -> Self {
            Self { seed, counter: 0 }
        }
    }

    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0; 4];
            self.fill_bytes(&mut bytes);
            u32::from_be_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0; 8];
            self.fill_bytes(&mut bytes);
            u64::from_be_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            self.try_fill_bytes(destination)
                .expect("relation test RNG is infallible");
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            for chunk in destination.chunks_mut(32) {
                let mut hash = Sha256::new();
                hash.update(b"iroha.privacy.bootle-lantern.relation-test-rng.v1");
                hash.update(self.seed);
                hash.update(self.counter.to_be_bytes());
                self.counter = self.counter.wrapping_add(1);
                let block: [u8; 32] = hash.finalize().into();
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
            Ok(())
        }
    }

    impl CryptoRng for TestRng {}

    fn raw(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn matrix_seed() -> MatrixSeedV1 {
        matrix_seed_v1([0x31; 32]).expect("seed")
    }

    const fn genesis_hash() -> [u8; 32] {
        [0x32; 32]
    }

    fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: "bootle-lantern-test".parse().expect("chain id"),
            action_index: 0,
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
            context: context(),
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
        witness: BootleLanternPresentationWitnessV1,
    }

    fn issued_fixture() -> &'static IssuedFixture {
        static FIXTURE: OnceLock<IssuedFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let mut keygen_rng = TestRng::new([0x11; 32]);
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
                    allowed_values: (0..ATTRIBUTE_POLYNOMIALS_V1)
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
            let context = context();
            let issuance_store = BootleLanternInMemoryIssuanceStoreV1::new();
            let mut authorization_rng = TestRng::new([0x16; 32]);
            let authorization = issuer_authorize_blind_issuance_with_rng_v1(
                &issuer_key_pair,
                &context,
                genesis_hash(),
                &policy,
                raw(16),
                10,
                20,
                &issuance_store,
                &mut authorization_rng,
            )
            .expect("issuer one-shot authorization");
            let mut attributes = [[0_u8; 8]; ATTRIBUTE_POLYNOMIALS_V1];
            attributes[1] = [1; 8];
            let mut holder_issuance_rng = TestRng::new([0x12; 32]);
            let (request, state) = holder_prepare_blind_issuance_with_rng_v1(
                &context,
                genesis_hash(),
                &policy,
                &authorization,
                attributes,
                &mut holder_issuance_rng,
            )
            .expect("holder blind-issuance request");
            let mut issuer_issuance_rng = TestRng::new([0x14; 32]);
            let response = issuer_blind_issue_once_with_rng_v1(
                &issuer_key_pair,
                &context,
                genesis_hash(),
                &policy,
                &authorization,
                &request,
                10,
                &issuance_store,
                &mut issuer_issuance_rng,
            )
            .expect("native blind issuance");
            let credential = holder_finalize_blind_issuance_v1(
                state,
                &context,
                genesis_hash(),
                &policy,
                response,
            )
            .expect("holder issuance finalization");
            let witness = credential
                .presentation_witness_v1(&statement(&policy), &policy, genesis_hash())
                .expect("issued presentation witness");
            IssuedFixture { policy, witness }
        })
    }

    fn policy() -> BootleLanternIssuerPolicyV1 {
        issued_fixture().policy.clone()
    }

    fn valid_witness() -> BootleLanternPresentationWitnessV1 {
        issued_fixture().witness.clone()
    }

    fn redigest(policy: &mut BootleLanternIssuerPolicyV1) {
        policy.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        policy.record_digest = policy.computed_record_digest().expect("policy digest");
    }

    #[test]
    fn canonical_relation_has_exact_shape_zeroed_public_column_and_offset() {
        let policy = policy();
        let relation = compile_application_relation_v1(
            &statement(&policy),
            &policy,
            matrix_seed(),
            genesis_hash(),
        )
        .expect("relation");
        assert_eq!(relation.rows(), 8);
        assert_eq!(relation.columns(), 48);
        assert_eq!(relation.disclosure_bitmap(), 0b10);
        assert_eq!(relation.disclosed_attribute(1), Some([1; 8]));
        assert_eq!(relation.disclosed_attribute(0), None);
        for row in 0..APPLICATION_ROWS_V1 {
            assert_eq!(
                relation.get(row, ATTRIBUTE_START_V1 + 1),
                Some(&ApplicationPolynomialV1::ZERO)
            );
        }
        assert!(
            relation
                .public_offset()
                .iter()
                .any(|polynomial| *polynomial != ApplicationPolynomialV1::ZERO)
        );
        validate_presentation_witness_v1(&relation, &valid_witness())
            .expect("valid relation witness");
    }

    #[test]
    fn canonical_secret_vector_is_heap_backed_and_debug_redacted() {
        assert_eq!(
            core::mem::size_of::<CanonicalSecretWitnessVectorV1>(),
            core::mem::size_of::<Box<[ApplicationPolynomialV1; APPLICATION_WITNESS_POLYNOMIALS_V1]>>(
            )
        );
        let mut witness = valid_witness();
        witness.attributes[0] = [0xA5; 8];
        let vector = canonical_witness_vector_v1(&witness, 0b10);

        assert_ne!(
            vector.polynomials()[ATTRIBUTE_START_V1],
            ApplicationPolynomialV1::ZERO
        );
        assert_eq!(
            vector.polynomials()[ATTRIBUTE_START_V1 + 1],
            ApplicationPolynomialV1::ZERO
        );
        assert_eq!(
            format!("{vector:?}"),
            "CanonicalSecretWitnessVectorV1(<redacted>)"
        );
    }

    #[test]
    fn every_trusted_record_binding_fails_independently() {
        let policy = policy();
        let base = statement(&policy);
        let mut variants = Vec::new();
        let mut changed = base.clone();
        changed.issuer_id = PrivacyIssuerIdV1::new(raw(21));
        variants.push((changed, RelationErrorV1::IssuerMismatch));
        changed = base.clone();
        changed.policy_id = PrivacyPolicyIdV1::new(raw(22));
        variants.push((changed, RelationErrorV1::PolicyMismatch));
        changed = base.clone();
        changed.issuer_policy_epoch += 1;
        variants.push((changed, RelationErrorV1::IssuerPolicyEpochMismatch));
        changed = base.clone();
        changed.issuer_policy_record_digest =
            PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(23));
        variants.push((changed, RelationErrorV1::IssuerPolicyRecordDigestMismatch));
        changed = base.clone();
        changed.issuer_parameter_id = PrivacyParameterIdV1::new(raw(24));
        variants.push((changed, RelationErrorV1::IssuerParameterIdMismatch));
        changed = base;
        changed.issuer_parameter_digest = PrivacyParameterDigestV1::new(raw(25));
        variants.push((changed, RelationErrorV1::IssuerParameterDigestMismatch));

        for (changed, expected) in variants {
            assert_eq!(
                compile_application_relation_v1(&changed, &policy, matrix_seed(), genesis_hash()),
                Err(expected)
            );
        }
    }

    #[test]
    fn policy_disclosure_and_seed_attacks_fail_closed() {
        let policy = policy();
        let mut changed = statement(&policy);
        changed.disclosures.clear();
        assert_eq!(
            compile_application_relation_v1(&changed, &policy, matrix_seed(), genesis_hash()),
            Err(RelationErrorV1::MissingRequiredDisclosure { index: 1 })
        );

        changed = statement(&policy);
        changed.disclosures[0].value = BootleLanternAttributeValueV1::new([2; 8]);
        assert_eq!(
            compile_application_relation_v1(&changed, &policy, matrix_seed(), genesis_hash()),
            Err(RelationErrorV1::DisclosedValueNotAllowed { index: 1 })
        );

        assert_eq!(
            compile_application_relation_v1(
                &statement(&policy),
                &policy,
                MatrixSeedV1::new([0x32; 32], [0x72; 32]).expect("seed"),
                genesis_hash(),
            ),
            Err(RelationErrorV1::MatrixParameterDigestMismatch)
        );

        let mut invalid_policy = policy.clone();
        invalid_policy.issuer_public_matrix.entries.clear();
        redigest(&mut invalid_policy);
        assert_eq!(
            compile_application_relation_v1(
                &statement(&invalid_policy),
                &invalid_policy,
                matrix_seed(),
                genesis_hash(),
            ),
            Err(RelationErrorV1::InvalidIssuerPolicy)
        );
    }

    #[test]
    fn witness_disclosure_binary_and_norm_attacks_fail_before_equations() {
        let policy = policy();
        let relation = compile_application_relation_v1(
            &statement(&policy),
            &policy,
            matrix_seed(),
            genesis_hash(),
        )
        .expect("relation");

        let mut changed = valid_witness();
        changed.attributes[1][0] ^= 1;
        assert_eq!(
            validate_presentation_witness_v1(&relation, &changed),
            Err(RelationErrorV1::DisclosedAttributeMismatch { index: 1 })
        );

        changed = valid_witness();
        changed.tag[3] = ApplicationPolynomialV1::constant(2).expect("canonical");
        assert_eq!(
            validate_presentation_witness_v1(&relation, &changed),
            Err(RelationErrorV1::NonBinaryTag { index: 3 })
        );

        changed = valid_witness();
        changed.randomness[0] = ApplicationPolynomialV1::constant(110).expect("canonical");
        assert_eq!(
            validate_presentation_witness_v1(&relation, &changed),
            Err(RelationErrorV1::RandomnessNormExceeded)
        );

        changed = valid_witness();
        changed.signature_one[0] = ApplicationPolynomialV1::constant(5_834).expect("canonical");
        assert_eq!(
            validate_presentation_witness_v1(&relation, &changed),
            Err(RelationErrorV1::SignatureNormExceeded)
        );
    }

    #[test]
    fn mutating_each_witness_family_breaks_the_application_equation() {
        let policy = policy();
        let relation = compile_application_relation_v1(
            &statement(&policy),
            &policy,
            matrix_seed(),
            genesis_hash(),
        )
        .expect("relation");
        let one = ApplicationPolynomialV1::constant(1).expect("one");

        let mut variants = Vec::new();
        let mut changed = valid_witness();
        changed.randomness[0] = one;
        variants.push(changed);
        changed = valid_witness();
        changed.tag[0] = one;
        variants.push(changed);
        changed = valid_witness();
        changed.signature_one[0] = one;
        variants.push(changed);
        changed = valid_witness();
        changed.signature_two[0] = changed.signature_two[0].add(one);
        variants.push(changed);
        changed = valid_witness();
        changed.attributes[0][0] ^= 1;
        variants.push(changed);

        for changed in variants {
            assert!(matches!(
                validate_presentation_witness_v1(&relation, &changed),
                Err(RelationErrorV1::ApplicationEquationFailed { .. })
            ));
        }
    }
}
