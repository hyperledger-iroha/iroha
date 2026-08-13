//! Static T256-to-q cross-field relation prerequisite.
//!
//! This private child freezes the exact added commitment inventory, ordered
//! transcript, two derived commitments, arithmetic-circuit shape, proof codec,
//! and conditional wire subtotal for the Phase-23 cross-field link.  It cannot be
//! entered in production: source, range, canonical q-mask, and authenticated
//! qPCS seals are all uninhabited.  In particular, this file does not replace
//! the missing global lookup proof or same-opening qPCS writer and grants no
//! proof, receipt, authority, or release capability.

use super::super::manifest::RELEASE_MODULI_V1;
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, GeneralizedBulletproofErrorV1, LinComb, ProofSuite,
        ProverTranscript, Variable, VerifierTranscript, multiexp,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::{Keccak256, keccak256},
    },
};
use core::{convert::Infallible, fmt, marker::PhantomData};
use std::sync::OnceLock;
const CROSS_FIELD_VERSION_V2: u8 = 2;
const LIMBS_V2: usize = 38;
const REPETITIONS_V2: usize = 5;
const RECORDS_V2: usize = 43;
const BLOCKS_PER_RECORD_V2: usize = 8;
const BLOCK_COEFFICIENTS_V2: usize = 16_384;
const RING_DEGREE_V2: usize = 131_072;
const RADIX_BASE_V2: u64 = 1 << 15;
const RADIX_LOW_DIGITS_V2: usize = 17;
const RADIX_DIGITS_V2: usize = 18;
const SMALL_SOURCE_ROLES_V2: usize = 3;
const MASK_DIGITS_V2: usize = 4;
const QUOTIENT_BITS_V2: usize = 103;
const MULTIPLICATION_GATES_V2: usize = 2 * QUOTIENT_BITS_V2;
const LINEAR_CONSTRAINTS_V2: usize = 2 * MULTIPLICATION_GATES_V2 + 1;
const GENERATOR_PREFIX_V2: usize = BLOCK_COEFFICIENTS_V2;
const RELATION_COUNT_V2: usize = LIMBS_V2 * REPETITIONS_V2;
const COMPARATOR_GROUPS_V2: usize = RECORDS_V2 * BLOCKS_PER_RECORD_V2;
const COMPARATOR_POINTS_PER_GROUP_V2: usize = 17 + 1 + 18 + 17;
const COMPARATOR_POINTS_V2: usize = COMPARATOR_GROUPS_V2 * COMPARATOR_POINTS_PER_GROUP_V2;
const SMALL_SOURCE_BLOCKS_V2: usize = RECORDS_V2 * SMALL_SOURCE_ROLES_V2 * BLOCKS_PER_RECORD_V2;
const SMALL_SOURCE_POINTS_PER_BLOCK_V2: usize = 4;
const SMALL_SOURCE_POINTS_V2: usize = SMALL_SOURCE_BLOCKS_V2 * SMALL_SOURCE_POINTS_PER_BLOCK_V2;
const Q_MASK_BLOCKS_V2: usize = RELATION_COUNT_V2 * BLOCKS_PER_RECORD_V2;
const Q_MASK_POINTS_PER_BLOCK_V2: usize = 4 * MASK_DIGITS_V2;
const Q_MASK_POINTS_V2: usize = Q_MASK_BLOCKS_V2 * Q_MASK_POINTS_PER_BLOCK_V2;
const ADDED_RAW_POINTS_V2: usize = COMPARATOR_POINTS_V2 + SMALL_SOURCE_POINTS_V2 + Q_MASK_POINTS_V2;
const POINT_BYTES_V2: usize = 33;
const SCALAR_BYTES_V2: usize = 32;
const BP_PROOF_POINTS_V2: usize = 41;
const BP_PROOF_SCALARS_V2: usize = 5;
const BP_PROOF_BYTES_V2: usize =
    BP_PROOF_POINTS_V2 * POINT_BYTES_V2 + BP_PROOF_SCALARS_V2 * SCALAR_BYTES_V2;
const ALL_BP_PROOFS_BYTES_V2: usize = RELATION_COUNT_V2 * BP_PROOF_BYTES_V2;
const OUTER_AUTH_FRAMING_BYTES_V2: usize = 512;
const COMPARATOR_WIRE_BYTES_V2: usize = COMPARATOR_POINTS_V2 * POINT_BYTES_V2;
const SMALL_SOURCE_WIRE_BYTES_V2: usize = SMALL_SOURCE_POINTS_V2 * POINT_BYTES_V2;
const Q_MASK_WIRE_BYTES_V2: usize = Q_MASK_POINTS_V2 * POINT_BYTES_V2;
const CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2: usize = COMPARATOR_WIRE_BYTES_V2
    + SMALL_SOURCE_WIRE_BYTES_V2
    + Q_MASK_WIRE_BYTES_V2
    + ALL_BP_PROOFS_BYTES_V2
    + OUTER_AUTH_FRAMING_BYTES_V2;
const PRIOR_CLAIMED_WIRE_MARGIN_BYTES_V2: usize = 2_158_923;
const CONDITIONAL_SUBTOTAL_RESERVE_BYTES_V2: usize =
    PRIOR_CLAIMED_WIRE_MARGIN_BYTES_V2 - CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2;
const EXISTING_LOOKUP_VALUES_V2: usize = 191_627_264;
const COMPARATOR_LOOKUP_VALUES_V2: usize = 95_813_632;
const SMALL_SOURCE_LOOKUP_VALUES_V2: usize = 33_816_576;
const Q_MASK_LOOKUP_VALUES_V2: usize = 199_229_440;
const EXPANDED_LOOKUP_VALUES_V2: usize = 520_486_912;
const EXISTING_LOOKUP_ROUNDS_V2: usize = 28;
const EXPANDED_LOOKUP_ROUNDS_V2: usize = 29;
const CONDITIONAL_MINIMUM_LOOKUP_DELTA_BYTES_V2: usize = 96;
const POSITIVE_TERMS_PER_COORDINATE_V2: usize = 7_256;
const NEGATIVE_TERMS_PER_COORDINATE_V2: usize = 1_376;
const POSITIVE_TERMS_TOTAL_V2: usize = POSITIVE_TERMS_PER_COORDINATE_V2 * BLOCK_COEFFICIENTS_V2;
const NEGATIVE_TERMS_TOTAL_V2: usize = NEGATIVE_TERMS_PER_COORDINATE_V2 * BLOCK_COEFFICIENTS_V2;
const Q_MIN_V2: u64 = 1_152_921_504_409_190_401;
const Q_MAX_V2: u64 = 1_152_921_504_606_584_833;
const AGGREGATE_DISCREPANCY_DEGREE_V2: u64 = 262_185;
const CROSS_SOUNDNESS_BITS_X100_FLOOR_V2: u32 = 20_475;
const V_PLUS_BITS_V2: u16 = 88;
const V_MINUS_BITS_V2: u16 = 86;
const U_PLUS_BITS_V2: u16 = 162;
const U_MINUS_BITS_V2: u16 = 160;
const INTEGER_EXPRESSION_BITS_V2: u16 = 165;
const INVENTORY_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.cross-field.inventory\0";
const EXISTING_D_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.cross-field.existing-d\0";
const BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.cross-field.binding\0";
const TRANSCRIPT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.cross-field.bp\0";
const CHALLENGE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.cross-field.bp.challenge\0";
const TRANSCRIPT_SCHEMA_V2: &[u8] = b"binding,initial-root,limb,repetition,q,gamma,beta,r,C+,C-,K(r),C(r),P~(r),H~(r),qpcs-entry,bp-basis";
const DERIVATION_FORMULA_V2: &[u8] = b"C+=[gamma^j*B^h*r^(bL)]q*CD+[gamma^j*K(r)*r^(bL)]q*Cr+ +[gamma^j*pq*r^(bL)]q*Ce0+ +[gamma^j*beta*pq*r^(bL)]q*Ce1+ +[(r^N+1)*B^h*r^(bL)]q*CS;C-=[gamma^j*K(r)*r^(bL)]q*Cr- +[gamma^j*pq*r^(bL)]q*Ce0- +[gamma^j*beta*pq*r^(bL)]q*Ce1- +[gamma^j*pq*r^(bL)]q*(C1-Cb18)";
const INTEGER_RELATION_V2: &[u8] = b"U+-U--y=q(z+-z-);0<=z+,z-<2^103;absolute-expression<2^165<pT";
const NO_WRAP_FORMULA_V2: &[u8] = b"V+<7256*(B-1)*(qmax-1)<2^88;V-<1376*(B-1)*(qmax-1)<2^86;U+<118882304*(B-1)*(qmax-1)^2<2^162;U-<22544384*(B-1)*(qmax-1)^2<2^160";
const SOUNDNESS_FORMULA_V2: &[u8] = b"degree=(2N-2)+42+1=262185;38*(262185/qmin)^5<2^-204.75";
const SOURCE_SET_BOUND_V2: bool = false;
const RANGE_SET_BOUND_V2: bool = false;
const CANONICAL_Q_MASK_SET_BOUND_V2: bool = false;
const AUTHENTICATED_QPCS_SET_BOUND_V2: bool = false;
const GLOBAL_LOOKUP_STATEMENT_INSTANTIATED_V2: bool = false;
const GLOBAL_LOOKUP_VERIFIED_V2: bool = false;
const COMPLETE_WIRE_ACCOUNTING_VERIFIED_V2: bool = false;
const CROSS_FIELD_RELATION_VERIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_THEOREM_ACCEPTED_V2: bool = false;
const AUTHORITY_MINTED_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const MEASURED_RSS_QUALIFIED_V2: bool = false;
const RELEASE_READY_V2: bool = false;
const _: () = {
    assert!(RELEASE_MODULI_V1.len() == LIMBS_V2);
    assert!(RING_DEGREE_V2 == BLOCKS_PER_RECORD_V2 * BLOCK_COEFFICIENTS_V2);
    assert!(COMPARATOR_POINTS_PER_GROUP_V2 == 53);
    assert!(COMPARATOR_POINTS_V2 == 18_232);
    assert!(SMALL_SOURCE_BLOCKS_V2 == 1_032);
    assert!(SMALL_SOURCE_POINTS_V2 == 4_128);
    assert!(Q_MASK_BLOCKS_V2 == 1_520);
    assert!(Q_MASK_POINTS_V2 == 24_320);
    assert!(ADDED_RAW_POINTS_V2 == 46_680);
    assert!(COMPARATOR_WIRE_BYTES_V2 == 601_656);
    assert!(SMALL_SOURCE_WIRE_BYTES_V2 == 136_224);
    assert!(Q_MASK_WIRE_BYTES_V2 == 802_560);
    assert!(BP_PROOF_BYTES_V2 == 1_513);
    assert!(ALL_BP_PROOFS_BYTES_V2 == 287_470);
    assert!(CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2 == 1_828_422);
    assert!(CONDITIONAL_SUBTOTAL_RESERVE_BYTES_V2 == 330_501);
    assert!(
        EXPANDED_LOOKUP_VALUES_V2
            == EXISTING_LOOKUP_VALUES_V2
                + COMPARATOR_LOOKUP_VALUES_V2
                + SMALL_SOURCE_LOOKUP_VALUES_V2
                + Q_MASK_LOOKUP_VALUES_V2
    );
    assert!(EXISTING_LOOKUP_VALUES_V2 < 1 << EXISTING_LOOKUP_ROUNDS_V2);
    assert!(EXPANDED_LOOKUP_VALUES_V2 > 1 << EXISTING_LOOKUP_ROUNDS_V2);
    assert!(EXPANDED_LOOKUP_VALUES_V2 < 1 << EXPANDED_LOOKUP_ROUNDS_V2);
    assert!(CONDITIONAL_MINIMUM_LOOKUP_DELTA_BYTES_V2 == 3 * 32);
    assert!(MULTIPLICATION_GATES_V2 == 206);
    assert!(LINEAR_CONSTRAINTS_V2 == 413);
    assert!(POSITIVE_TERMS_TOTAL_V2 == 118_882_304);
    assert!(NEGATIVE_TERMS_TOTAL_V2 == 22_544_384);
    assert!(AGGREGATE_DISCREPANCY_DEGREE_V2 == (2 * RING_DEGREE_V2 - 2 + 42 + 1) as u64);
    assert!(!SOURCE_SET_BOUND_V2);
    assert!(!RANGE_SET_BOUND_V2);
    assert!(!CANONICAL_Q_MASK_SET_BOUND_V2);
    assert!(!AUTHENTICATED_QPCS_SET_BOUND_V2);
    assert!(!GLOBAL_LOOKUP_STATEMENT_INSTANTIATED_V2);
    assert!(!GLOBAL_LOOKUP_VERIFIED_V2);
    assert!(!COMPLETE_WIRE_ACCOUNTING_VERIFIED_V2);
    assert!(!CROSS_FIELD_RELATION_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_THEOREM_ACCEPTED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!MEASURED_RSS_QUALIFIED_V2);
    assert!(!RELEASE_READY_V2);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrossFieldErrorV2 {
    Shape,
    Context,
    Point,
    Wire,
    Arithmetic,
    BindingUnavailable,
    Backend(GeneralizedBulletproofErrorV1),
}
impl fmt::Display for CrossFieldErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl From<GeneralizedBulletproofErrorV1> for CrossFieldErrorV2 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        Self::Backend(error)
    }
}
enum SourceCommitmentSealV2 {
    Production {
        authenticated_source_owner: Infallible,
        terminal_cross_schnorr: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum RadixRangeSealV2 {
    Production {
        canonical_radix: Infallible,
        comparator_and_lookup: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum CanonicalQMaskSealV2 {
    Production {
        one_owner_uniform_q_mask: Infallible,
        authenticated_opening_spool: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum AuthenticatedQpcsSealV2 {
    Production {
        complete_entry_verified: Infallible,
        same_mask_opening_consumed: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
#[derive(Clone, Copy)]
struct RadixGroupCommitmentsV2 {
    d_low: [Point; RADIX_LOW_DIGITS_V2],
    d_top: Point,
}
#[derive(Clone, Copy)]
struct ComparatorGroupCommitmentsV2 {
    difference_digits: [Point; RADIX_LOW_DIGITS_V2],
    mixed_top: Point,
    borrows: [Point; RADIX_DIGITS_V2],
    difference_inverses: [Point; RADIX_LOW_DIGITS_V2],
}
#[derive(Clone, Copy)]
struct SmallSourceBlockCommitmentsV2 {
    signed: Point,
    negative_magnitude: Point,
    positive_lookup_inverse: Point,
    negative_lookup_inverse: Point,
}
impl SmallSourceBlockCommitmentsV2 {
    fn positive_v2(&self) -> Result<Point, CrossFieldErrorV2> {
        let point = self.signed + self.negative_magnitude;
        (!point.is_identity())
            .then_some(point)
            .ok_or(CrossFieldErrorV2::Point)
    }
}
#[derive(Clone, Copy)]
struct QMaskBlockCommitmentsV2 {
    digits: [Point; MASK_DIGITS_V2],
    digit_inverses: [Point; MASK_DIGITS_V2],
    complement_digits: [Point; MASK_DIGITS_V2],
    complement_inverses: [Point; MASK_DIGITS_V2],
}
#[derive(Clone, Copy)]
struct CrossFieldAxesV2 {
    fixed_axes_digest: [u8; 32],
    source_manifest_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    source_formula_digest: [u8; 32],
    source_mapping_digest: [u8; 32],
    terminal_binding_digest: [u8; 32],
    radix_range_binding_digest: [u8; 32],
    qpcs_parameter_digest: [u8; 32],
}
impl CrossFieldAxesV2 {
    fn validate_v2(&self) -> Result<(), CrossFieldErrorV2> {
        let values = [
            self.fixed_axes_digest,
            self.source_manifest_digest,
            self.source_receipt_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.terminal_binding_digest,
            self.radix_range_binding_digest,
            self.qpcs_parameter_digest,
        ];
        if values.contains(&[0; 32]) {
            return Err(CrossFieldErrorV2::Context);
        }
        Ok(())
    }
}
/// Typed, sealed view of every commitment used by deterministic C+/C- derivation.
///
/// There is deliberately no constructor.  It contains no raw `Vec<Point>` and
/// cannot be produced from detached commitment points: four move-only upstream
/// seals must eventually be replaced by consuming verified owners.
struct BoundCommitmentViewV2<'a> {
    source_seal: SourceCommitmentSealV2,
    range_seal: RadixRangeSealV2,
    mask_seal: CanonicalQMaskSealV2,
    axes: CrossFieldAxesV2,
    radix_groups: &'a [RadixGroupCommitmentsV2],
    comparators: &'a [ComparatorGroupCommitmentsV2],
    small_source: &'a [SmallSourceBlockCommitmentsV2],
    q_masks: &'a [QMaskBlockCommitmentsV2],
}
#[repr(u8)]
#[derive(Clone, Copy)]
enum AddedPointRoleV2 {
    ComparatorDifferenceDigit = 1,
    ComparatorMixedTop = 2,
    ComparatorBorrow = 3,
    ComparatorDifferenceInverse = 4,
    SmallSigned = 5,
    SmallNegative = 6,
    SmallPositiveInverse = 7,
    SmallNegativeInverse = 8,
    QMaskDigit = 9,
    QMaskDigitInverse = 10,
    QMaskComplementDigit = 11,
    QMaskComplementInverse = 12,
}
fn absorb_u32_v2(hash: &mut Keccak256, value: usize) -> Result<(), CrossFieldErrorV2> {
    hash.update(
        &u32::try_from(value)
            .map_err(|_| CrossFieldErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    Ok(())
}
fn absorb_point_v2(
    hash: &mut Keccak256,
    ordinal: &mut usize,
    role: AddedPointRoleV2,
    owner: usize,
    column: usize,
    point: Point,
) -> Result<(), CrossFieldErrorV2> {
    hash.update(&[role as u8]);
    absorb_u32_v2(hash, *ordinal)?;
    absorb_u32_v2(hash, owner)?;
    absorb_u32_v2(hash, column)?;
    hash.update(
        &point
            .to_non_identity_wire_bytes()
            .map_err(|_| CrossFieldErrorV2::Point)?,
    );
    *ordinal = ordinal
        .checked_add(1)
        .ok_or(CrossFieldErrorV2::Arithmetic)?;
    Ok(())
}
impl BoundCommitmentViewV2<'_> {
    fn validate_shape_v2(&self) -> Result<(), CrossFieldErrorV2> {
        self.axes.validate_v2()?;
        if self.radix_groups.len() != COMPARATOR_GROUPS_V2
            || self.comparators.len() != COMPARATOR_GROUPS_V2
            || self.small_source.len() != SMALL_SOURCE_BLOCKS_V2
            || self.q_masks.len() != Q_MASK_BLOCKS_V2
        {
            return Err(CrossFieldErrorV2::Shape);
        }
        Ok(())
    }
    fn added_inventory_root_v2(&self) -> Result<[u8; 32], CrossFieldErrorV2> {
        self.validate_shape_v2()?;
        let mut hash = Keccak256::new();
        hash.update(INVENTORY_DOMAIN_V2);
        hash.update(&[CROSS_FIELD_VERSION_V2]);
        absorb_u32_v2(&mut hash, ADDED_RAW_POINTS_V2)?;
        let mut ordinal = 0;
        for (group, commitments) in self.comparators.iter().enumerate() {
            for (column, point) in commitments.difference_digits.iter().copied().enumerate() {
                absorb_point_v2(
                    &mut hash,
                    &mut ordinal,
                    AddedPointRoleV2::ComparatorDifferenceDigit,
                    group,
                    column,
                    point,
                )?;
            }
            absorb_point_v2(
                &mut hash,
                &mut ordinal,
                AddedPointRoleV2::ComparatorMixedTop,
                group,
                0,
                commitments.mixed_top,
            )?;
            for (column, point) in commitments.borrows.iter().copied().enumerate() {
                absorb_point_v2(
                    &mut hash,
                    &mut ordinal,
                    AddedPointRoleV2::ComparatorBorrow,
                    group,
                    column,
                    point,
                )?;
            }
            for (column, point) in commitments.difference_inverses.iter().copied().enumerate() {
                absorb_point_v2(
                    &mut hash,
                    &mut ordinal,
                    AddedPointRoleV2::ComparatorDifferenceInverse,
                    group,
                    column,
                    point,
                )?;
            }
        }
        for (block, commitments) in self.small_source.iter().enumerate() {
            for (role, point) in [
                (AddedPointRoleV2::SmallSigned, commitments.signed),
                (
                    AddedPointRoleV2::SmallNegative,
                    commitments.negative_magnitude,
                ),
                (
                    AddedPointRoleV2::SmallPositiveInverse,
                    commitments.positive_lookup_inverse,
                ),
                (
                    AddedPointRoleV2::SmallNegativeInverse,
                    commitments.negative_lookup_inverse,
                ),
            ] {
                absorb_point_v2(&mut hash, &mut ordinal, role, block, 0, point)?;
            }
        }
        for (block, commitments) in self.q_masks.iter().enumerate() {
            for (role, points) in [
                (AddedPointRoleV2::QMaskDigit, &commitments.digits),
                (
                    AddedPointRoleV2::QMaskDigitInverse,
                    &commitments.digit_inverses,
                ),
                (
                    AddedPointRoleV2::QMaskComplementDigit,
                    &commitments.complement_digits,
                ),
                (
                    AddedPointRoleV2::QMaskComplementInverse,
                    &commitments.complement_inverses,
                ),
            ] {
                for (column, point) in points.iter().copied().enumerate() {
                    absorb_point_v2(&mut hash, &mut ordinal, role, block, column, point)?;
                }
            }
        }
        if ordinal != ADDED_RAW_POINTS_V2 {
            return Err(CrossFieldErrorV2::Shape);
        }
        Ok(hash.finalize())
    }
    fn existing_d_root_v2(&self) -> Result<[u8; 32], CrossFieldErrorV2> {
        self.validate_shape_v2()?;
        let mut hash = Keccak256::new();
        hash.update(EXISTING_D_DOMAIN_V2);
        hash.update(&[CROSS_FIELD_VERSION_V2]);
        absorb_u32_v2(&mut hash, COMPARATOR_GROUPS_V2 * RADIX_DIGITS_V2)?;
        for (group, commitments) in self.radix_groups.iter().enumerate() {
            absorb_u32_v2(&mut hash, group)?;
            for point in commitments
                .d_low
                .iter()
                .copied()
                .chain(core::iter::once(commitments.d_top))
            {
                hash.update(
                    &point
                        .to_non_identity_wire_bytes()
                        .map_err(|_| CrossFieldErrorV2::Point)?,
                );
            }
        }
        Ok(hash.finalize())
    }
    fn source_binding_digest_v2(&self) -> Result<[u8; 32], CrossFieldErrorV2> {
        let mut hash = Keccak256::new();
        hash.update(BINDING_DOMAIN_V2);
        hash.update(&[CROSS_FIELD_VERSION_V2]);
        for digest in [
            self.axes.fixed_axes_digest,
            self.axes.source_manifest_digest,
            self.axes.source_receipt_digest,
            self.axes.source_formula_digest,
            self.axes.source_mapping_digest,
            self.axes.terminal_binding_digest,
            self.axes.radix_range_binding_digest,
            self.axes.qpcs_parameter_digest,
            self.existing_d_root_v2()?,
            self.added_inventory_root_v2()?,
        ] {
            hash.update(&digest);
        }
        hash.update(DERIVATION_FORMULA_V2);
        hash.update(INTEGER_RELATION_V2);
        Ok(hash.finalize())
    }
}
#[derive(Clone, Copy)]
struct AuthenticatedQpcsEvaluationV2 {
    seal: PhantomData<AuthenticatedQpcsSealV2>,
    initial_root: [u8; 32],
    complete_entry_digest: [u8; 32],
    limb: u8,
    repetition: u8,
    modulus: u64,
    gamma: u64,
    beta: u64,
    point: u64,
    key_evaluation: u64,
    ciphertext_evaluation: u64,
    masked_p_evaluation: u64,
    masked_h_evaluation: u64,
}
impl AuthenticatedQpcsEvaluationV2 {
    fn validate_v2(&self) -> Result<(), CrossFieldErrorV2> {
        let limb = usize::from(self.limb);
        if limb >= LIMBS_V2
            || usize::from(self.repetition) >= REPETITIONS_V2
            || self.modulus != RELEASE_MODULI_V1[limb]
            || self.initial_root == [0; 32]
            || self.complete_entry_digest == [0; 32]
        {
            return Err(CrossFieldErrorV2::Context);
        }
        for value in [
            self.gamma,
            self.beta,
            self.point,
            self.key_evaluation,
            self.ciphertext_evaluation,
            self.masked_p_evaluation,
            self.masked_h_evaluation,
        ] {
            if value >= self.modulus {
                return Err(CrossFieldErrorV2::Context);
            }
        }
        if self.gamma == 0 || self.beta == 0 || self.point == 0 || self.gamma == self.beta {
            return Err(CrossFieldErrorV2::Context);
        }
        let factor = mod_add_v2(
            mod_pow_v2(self.point, RING_DEGREE_V2 as u64, self.modulus),
            1,
            self.modulus,
        );
        if factor == 0
            || self.masked_p_evaluation
                != mod_mul_v2(factor, self.masked_h_evaluation, self.modulus)
        {
            return Err(CrossFieldErrorV2::Context);
        }
        Ok(())
    }
    fn public_y_v2(&self) -> u64 {
        mod_add_v2(
            self.masked_p_evaluation,
            self.ciphertext_evaluation,
            self.modulus,
        )
    }
}
struct DerivedCommitmentsV2 {
    positive: Point,
    negative: Point,
    source_binding_digest: [u8; 32],
}
fn mod_add_v2(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(modulus)) as u64
}
fn mod_mul_v2(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) * u128::from(right)) % u128::from(modulus)) as u64
}
fn mod_pow_v2(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mod_mul_v2(result, base, modulus);
        }
        base = mod_mul_v2(base, base, modulus);
        exponent >>= 1;
    }
    result
}
fn t256_mod_q_v2(modulus: u64) -> u64 {
    VEGA_T256_SCALAR_MODULUS_BE_V1
        .iter()
        .fold(0, |value, byte| {
            ((u128::from(value) << 8) + u128::from(*byte)).rem_euclid(u128::from(modulus)) as u64
        })
}
fn public_scalar_v2(value: u64, modulus: u64) -> Result<Scalar, CrossFieldErrorV2> {
    if value >= modulus {
        return Err(CrossFieldErrorV2::Arithmetic);
    }
    Ok(Scalar::from_u64(value))
}
fn one_vector_commitment_v2() -> Point {
    static COMMITMENT: OnceLock<Point> = OnceLock::new();
    *COMMITMENT.get_or_init(|| {
        let generators = ZkAmsT256BulletproofSuiteV1::generators();
        let terms: Vec<_> = generators.g_bold[..GENERATOR_PREFIX_V2]
            .iter()
            .copied()
            .map(|point| (Scalar::one(), point))
            .collect();
        multiexp::<ZkAmsT256BulletproofSuiteV1>(&terms)
    })
}
fn group_index_v2(record: usize, block: usize) -> usize {
    record * BLOCKS_PER_RECORD_V2 + block
}
fn small_source_index_v2(record: usize, role: usize, block: usize) -> usize {
    (record * SMALL_SOURCE_ROLES_V2 + role) * BLOCKS_PER_RECORD_V2 + block
}
fn mask_index_v2(limb: usize, repetition: usize, block: usize) -> usize {
    (limb * REPETITIONS_V2 + repetition) * BLOCKS_PER_RECORD_V2 + block
}
fn push_public_term_v2(
    terms: &mut Vec<(Scalar, Point)>,
    weight: u64,
    point: Point,
    modulus: u64,
) -> Result<(), CrossFieldErrorV2> {
    if point.is_identity() || terms.len() >= terms.capacity() {
        return Err(CrossFieldErrorV2::Point);
    }
    terms.push((public_scalar_v2(weight, modulus)?, point));
    Ok(())
}
impl BoundCommitmentViewV2<'_> {
    fn derive_v2(
        &self,
        evaluation: &AuthenticatedQpcsEvaluationV2,
    ) -> Result<DerivedCommitmentsV2, CrossFieldErrorV2> {
        self.validate_shape_v2()?;
        evaluation.validate_v2()?;
        let q = evaluation.modulus;
        let p_mod_q = t256_mod_q_v2(q);
        let r_block_step = mod_pow_v2(evaluation.point, BLOCK_COEFFICIENTS_V2 as u64, q);
        let mask_factor = mod_add_v2(mod_pow_v2(evaluation.point, RING_DEGREE_V2 as u64, q), 1, q);
        let one_commitment = one_vector_commitment_v2();
        if one_commitment.is_identity() {
            return Err(CrossFieldErrorV2::Point);
        }
        let mut positive_terms = Vec::new();
        positive_terms
            .try_reserve_exact(POSITIVE_TERMS_PER_COORDINATE_V2)
            .map_err(|_| CrossFieldErrorV2::Arithmetic)?;
        let mut negative_terms = Vec::new();
        negative_terms
            .try_reserve_exact(NEGATIVE_TERMS_PER_COORDINATE_V2)
            .map_err(|_| CrossFieldErrorV2::Arithmetic)?;
        let mut gamma_power = 1;
        for record in 0..RECORDS_V2 {
            let mut block_power = 1;
            for block in 0..BLOCKS_PER_RECORD_V2 {
                let group = group_index_v2(record, block);
                let radix = &self.radix_groups[group];
                let mut radix_power = 1;
                for point in radix
                    .d_low
                    .iter()
                    .copied()
                    .chain(core::iter::once(radix.d_top))
                {
                    let weight =
                        mod_mul_v2(mod_mul_v2(gamma_power, radix_power, q), block_power, q);
                    push_public_term_v2(&mut positive_terms, weight, point, q)?;
                    radix_power = mod_mul_v2(radix_power, RADIX_BASE_V2, q);
                }
                let r_weight = mod_mul_v2(
                    mod_mul_v2(gamma_power, evaluation.key_evaluation, q),
                    block_power,
                    q,
                );
                let e0_weight = mod_mul_v2(mod_mul_v2(gamma_power, p_mod_q, q), block_power, q);
                let e1_weight = mod_mul_v2(e0_weight, evaluation.beta, q);
                for (role, weight) in [(0, r_weight), (1, e0_weight), (2, e1_weight)] {
                    let source = &self.small_source[small_source_index_v2(record, role, block)];
                    push_public_term_v2(&mut positive_terms, weight, source.positive_v2()?, q)?;
                    push_public_term_v2(&mut negative_terms, weight, source.negative_magnitude, q)?;
                }
                let sigma = one_commitment - self.comparators[group].borrows[17];
                push_public_term_v2(&mut negative_terms, e0_weight, sigma, q)?;
                block_power = mod_mul_v2(block_power, r_block_step, q);
            }
            gamma_power = mod_mul_v2(gamma_power, evaluation.gamma, q);
        }
        let mut block_power = 1;
        for block in 0..BLOCKS_PER_RECORD_V2 {
            let mask = &self.q_masks[mask_index_v2(
                usize::from(evaluation.limb),
                usize::from(evaluation.repetition),
                block,
            )];
            let mut radix_power = 1;
            for point in mask.digits {
                let weight = mod_mul_v2(mod_mul_v2(mask_factor, radix_power, q), block_power, q);
                push_public_term_v2(&mut positive_terms, weight, point, q)?;
                radix_power = mod_mul_v2(radix_power, RADIX_BASE_V2, q);
            }
            block_power = mod_mul_v2(block_power, r_block_step, q);
        }
        if positive_terms.len() != POSITIVE_TERMS_PER_COORDINATE_V2
            || negative_terms.len() != NEGATIVE_TERMS_PER_COORDINATE_V2
        {
            return Err(CrossFieldErrorV2::Shape);
        }
        let positive = multiexp::<ZkAmsT256BulletproofSuiteV1>(&positive_terms);
        let negative = multiexp::<ZkAmsT256BulletproofSuiteV1>(&negative_terms);
        if positive.is_identity() || negative.is_identity() {
            return Err(CrossFieldErrorV2::Point);
        }
        Ok(DerivedCommitmentsV2 {
            positive,
            negative,
            source_binding_digest: self.source_binding_digest_v2()?,
        })
    }
}
fn boolean_constraints_v2(gate: usize) -> [LinComb<Scalar>; 2] {
    [
        LinComb::empty()
            .term(Scalar::one(), Variable::aL(gate))
            .term(-Scalar::one(), Variable::aR(gate)),
        LinComb::empty()
            .term(Scalar::one(), Variable::aO(gate))
            .term(-Scalar::one(), Variable::aL(gate)),
    ]
}
fn cross_field_constraints_v2(
    evaluation: &AuthenticatedQpcsEvaluationV2,
) -> Result<Vec<LinComb<Scalar>>, CrossFieldErrorV2> {
    evaluation.validate_v2()?;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(LINEAR_CONSTRAINTS_V2)
        .map_err(|_| CrossFieldErrorV2::Arithmetic)?;
    for gate in 0..MULTIPLICATION_GATES_V2 {
        constraints.extend(boolean_constraints_v2(gate));
    }
    let mut relation = LinComb::empty().constant(-Scalar::from_u64(evaluation.public_y_v2()));
    let mut point_power = 1;
    for index in 0..BLOCK_COEFFICIENTS_V2 {
        let weight = Scalar::from_u64(point_power);
        relation = relation
            .term(
                weight,
                Variable::CG {
                    commitment: 0,
                    index,
                },
            )
            .term(
                -weight,
                Variable::CG {
                    commitment: 1,
                    index,
                },
            );
        point_power = mod_mul_v2(point_power, evaluation.point, evaluation.modulus);
    }
    let mut quotient_weight = Scalar::from_u64(evaluation.modulus);
    for bit in 0..QUOTIENT_BITS_V2 {
        relation = relation
            .term(-quotient_weight, Variable::aL(bit))
            .term(quotient_weight, Variable::aL(QUOTIENT_BITS_V2 + bit));
        quotient_weight += quotient_weight;
    }
    constraints.push(relation);
    if constraints.len() != LINEAR_CONSTRAINTS_V2 {
        return Err(CrossFieldErrorV2::Shape);
    }
    Ok(constraints)
}
fn build_cross_field_statement_v2(
    derived: &DerivedCommitmentsV2,
    evaluation: &AuthenticatedQpcsEvaluationV2,
) -> Result<ArithmeticCircuitStatement<'static, ZkAmsT256BulletproofSuiteV1>, CrossFieldErrorV2> {
    if derived.source_binding_digest == [0; 32]
        || derived.positive.is_identity()
        || derived.negative.is_identity()
    {
        return Err(CrossFieldErrorV2::Context);
    }
    Ok(ArithmeticCircuitStatement::new(
        ZkAmsT256BulletproofSuiteV1::generators().reduce(GENERATOR_PREFIX_V2)?,
        cross_field_constraints_v2(evaluation)?,
        vec![derived.positive, derived.negative],
        Vec::new(),
    )?)
}
struct ExactProofViewV2<'a> {
    bytes: &'a [u8],
}
impl<'a> ExactProofViewV2<'a> {
    fn parse_v2(bytes: &'a [u8]) -> Result<Self, CrossFieldErrorV2> {
        if bytes.len() != BP_PROOF_BYTES_V2 {
            return Err(CrossFieldErrorV2::Wire);
        }
        let mut cursor = 0;
        for _ in 0..13 {
            parse_point_at_v2(bytes, &mut cursor)?;
        }
        for _ in 0..3 {
            parse_scalar_at_v2(bytes, &mut cursor)?;
        }
        for _ in 0..28 {
            parse_point_at_v2(bytes, &mut cursor)?;
        }
        for _ in 0..2 {
            parse_scalar_at_v2(bytes, &mut cursor)?;
        }
        if cursor != BP_PROOF_BYTES_V2 {
            return Err(CrossFieldErrorV2::Wire);
        }
        Ok(Self { bytes })
    }
}
fn take_at_v2<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    count: usize,
) -> Result<&'a [u8], CrossFieldErrorV2> {
    let end = cursor
        .checked_add(count)
        .ok_or(CrossFieldErrorV2::Arithmetic)?;
    let value = bytes.get(*cursor..end).ok_or(CrossFieldErrorV2::Wire)?;
    *cursor = end;
    Ok(value)
}
fn parse_point_at_v2(bytes: &[u8], cursor: &mut usize) -> Result<Point, CrossFieldErrorV2> {
    Point::from_non_identity_wire_bytes_exact(take_at_v2(bytes, cursor, POINT_BYTES_V2)?)
        .map_err(|_| CrossFieldErrorV2::Wire)
}
fn parse_scalar_at_v2(bytes: &[u8], cursor: &mut usize) -> Result<Scalar, CrossFieldErrorV2> {
    let encoded: [u8; SCALAR_BYTES_V2] = take_at_v2(bytes, cursor, SCALAR_BYTES_V2)?
        .try_into()
        .map_err(|_| CrossFieldErrorV2::Wire)?;
    Scalar::from_le_bytes_exact(encoded).map_err(|_| CrossFieldErrorV2::Wire)
}
fn append_frame_v2(state: &mut Vec<u8>, value: &[u8]) -> Result<(), CrossFieldErrorV2> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| CrossFieldErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}
fn initial_transcript_state_v2(
    derived: &DerivedCommitmentsV2,
    evaluation: &AuthenticatedQpcsEvaluationV2,
) -> Result<Vec<u8>, CrossFieldErrorV2> {
    evaluation.validate_v2()?;
    let mut state = Vec::with_capacity(512);
    append_frame_v2(&mut state, TRANSCRIPT_DOMAIN_V2)?;
    append_frame_v2(&mut state, &[CROSS_FIELD_VERSION_V2])?;
    append_frame_v2(&mut state, TRANSCRIPT_SCHEMA_V2)?;
    for value in [
        derived.source_binding_digest.as_slice(),
        evaluation.initial_root.as_slice(),
        &[evaluation.limb, evaluation.repetition],
        evaluation.modulus.to_be_bytes().as_slice(),
        evaluation.gamma.to_be_bytes().as_slice(),
        evaluation.beta.to_be_bytes().as_slice(),
        evaluation.point.to_be_bytes().as_slice(),
        derived
            .positive
            .to_non_identity_wire_bytes()
            .map_err(|_| CrossFieldErrorV2::Point)?
            .as_slice(),
        derived
            .negative
            .to_non_identity_wire_bytes()
            .map_err(|_| CrossFieldErrorV2::Point)?
            .as_slice(),
        evaluation.key_evaluation.to_be_bytes().as_slice(),
        evaluation.ciphertext_evaluation.to_be_bytes().as_slice(),
        evaluation.masked_p_evaluation.to_be_bytes().as_slice(),
        evaluation.masked_h_evaluation.to_be_bytes().as_slice(),
        evaluation.complete_entry_digest.as_slice(),
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1.as_slice(),
    ] {
        append_frame_v2(&mut state, value)?;
    }
    Ok(state)
}
fn derive_challenge_v2(
    state: &mut Vec<u8>,
    ordinal: &mut u32,
) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
    for attempt in 0_u8..128 {
        let mut input = Vec::with_capacity(CHALLENGE_DOMAIN_V2.len() + state.len() + 6);
        input.extend_from_slice(CHALLENGE_DOMAIN_V2);
        input.extend_from_slice(state);
        input.extend_from_slice(&ordinal.to_be_bytes());
        input.push(attempt);
        let mut low = input.clone();
        low.push(0);
        input.push(1);
        let mut wide = [0; 64];
        wide[..32].copy_from_slice(&keccak256(&low));
        wide[32..].copy_from_slice(&keccak256(&input));
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            state.push(2);
            state.extend_from_slice(&ordinal.to_be_bytes());
            state.push(attempt);
            state.extend_from_slice(&challenge.to_le_bytes());
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            return Ok(challenge);
        }
    }
    Err(GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)
}
struct CrossFieldProverTranscriptV2 {
    state: Vec<u8>,
    proof: [u8; BP_PROOF_BYTES_V2],
    cursor: usize,
    challenge_ordinal: u32,
}
impl CrossFieldProverTranscriptV2 {
    fn new_v2(
        derived: &DerivedCommitmentsV2,
        evaluation: &AuthenticatedQpcsEvaluationV2,
    ) -> Result<Self, CrossFieldErrorV2> {
        Ok(Self {
            state: initial_transcript_state_v2(derived, evaluation)?,
            proof: [0; BP_PROOF_BYTES_V2],
            cursor: 0,
            challenge_ordinal: 0,
        })
    }
    fn push_bytes_v2(&mut self, value: &[u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(value.len())
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let destination = self.proof.get_mut(self.cursor..end).ok_or(
            GeneralizedBulletproofErrorV1::ProofLength {
                actual: end,
                expected: BP_PROOF_BYTES_V2,
            },
        )?;
        destination.copy_from_slice(value);
        self.cursor = end;
        Ok(())
    }
    fn finish_v2(self) -> Result<([u8; BP_PROOF_BYTES_V2], [u8; 32]), CrossFieldErrorV2> {
        if self.cursor != BP_PROOF_BYTES_V2 {
            return Err(CrossFieldErrorV2::Wire);
        }
        Ok((self.proof, keccak256(&self.state)))
    }
}
impl ProverTranscript<ZkAmsT256BulletproofSuiteV1> for CrossFieldProverTranscriptV2 {
    fn push_scalar(&mut self, scalar: Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.state.push(0);
        self.state.extend_from_slice(&scalar.to_le_bytes());
        self.push_bytes_v2(&scalar.to_le_bytes())
    }
    fn push_point(&mut self, point: Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = point
            .to_non_identity_wire_bytes()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointIdentity)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        self.push_bytes_v2(&encoded)
    }
    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_challenge_v2(&mut self.state, &mut self.challenge_ordinal)
    }
}
struct CrossFieldVerifierTranscriptV2<'a> {
    state: Vec<u8>,
    proof: ExactProofViewV2<'a>,
    cursor: usize,
    challenge_ordinal: u32,
}
impl<'a> CrossFieldVerifierTranscriptV2<'a> {
    fn new_v2(
        derived: &DerivedCommitmentsV2,
        evaluation: &AuthenticatedQpcsEvaluationV2,
        proof: ExactProofViewV2<'a>,
    ) -> Result<Self, CrossFieldErrorV2> {
        Ok(Self {
            state: initial_transcript_state_v2(derived, evaluation)?,
            proof,
            cursor: 0,
            challenge_ordinal: 0,
        })
    }
    fn take_v2(&mut self, count: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let value = self.proof.bytes.get(self.cursor..end).ok_or(
            GeneralizedBulletproofErrorV1::ProofLength {
                actual: self.proof.bytes.len(),
                expected: end,
            },
        )?;
        self.cursor = end;
        Ok(value)
    }
    fn finish_v2(self) -> Result<[u8; 32], CrossFieldErrorV2> {
        if self.cursor != BP_PROOF_BYTES_V2 {
            return Err(CrossFieldErrorV2::Wire);
        }
        Ok(keccak256(&self.state))
    }
}
impl VerifierTranscript<ZkAmsT256BulletproofSuiteV1> for CrossFieldVerifierTranscriptV2<'_> {
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; SCALAR_BYTES_V2] = self
            .take_v2(SCALAR_BYTES_V2)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        Ok(scalar)
    }
    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; POINT_BYTES_V2] = self
            .take_v2(POINT_BYTES_V2)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        Ok(point)
    }
    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_challenge_v2(&mut self.state, &mut self.challenge_ordinal)
    }
}
fn verify_cross_field_proof_v2(
    view: BoundCommitmentViewV2<'_>,
    qpcs_seal: AuthenticatedQpcsSealV2,
    evaluation: AuthenticatedQpcsEvaluationV2,
    proof_bytes: &[u8],
) -> Result<[u8; 32], CrossFieldErrorV2> {
    let _qpcs_seal = qpcs_seal;
    let derived = view.derive_v2(&evaluation)?;
    let proof = ExactProofViewV2::parse_v2(proof_bytes)?;
    let mut transcript = CrossFieldVerifierTranscriptV2::new_v2(&derived, &evaluation, proof)?;
    build_cross_field_statement_v2(&derived, &evaluation)?.verify(&mut transcript)?;
    transcript.finish_v2()
}
/// Checks only the enumerated raw-point/BP/framing subtotal.  It is not a
/// complete-proof size certificate because the expanded global lookup
/// statement, endpoint openings, and their final framing are not instantiated.
struct CrossFieldConditionalSubtotalPreflightV2 {
    comparator_points: usize,
    small_source_points: usize,
    q_mask_points: usize,
    proof_count: usize,
    proof_bytes_each: usize,
    outer_auth_bytes: usize,
    conditional_subtotal_bytes: usize,
}
impl CrossFieldConditionalSubtotalPreflightV2 {
    fn validate_v2(&self) -> Result<(), CrossFieldErrorV2> {
        if self.comparator_points != COMPARATOR_POINTS_V2
            || self.small_source_points != SMALL_SOURCE_POINTS_V2
            || self.q_mask_points != Q_MASK_POINTS_V2
            || self.proof_count != RELATION_COUNT_V2
            || self.proof_bytes_each != BP_PROOF_BYTES_V2
            || self.outer_auth_bytes != OUTER_AUTH_FRAMING_BYTES_V2
            || self.conditional_subtotal_bytes != CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2
            || self.conditional_subtotal_bytes > PRIOR_CLAIMED_WIRE_MARGIN_BYTES_V2
        {
            return Err(CrossFieldErrorV2::Shape);
        }
        Ok(())
    }
}
#[path = "phase23_rns_link_cross_field_v2/joint_z_binding_v3.rs"]
mod joint_z_binding_v3;
#[cfg(test)]
#[path = "phase23_rns_link_cross_field_v2_tests.rs"]
mod tests;
