//! Exact 40-limb commitment-inventory prerequisite for the cross-field proof.
//!
//! The preceding source/terminal stage authenticates the confidential source,
//! the terminal Hyrax openings, the zero-padding proof, and the opaque outer
//! cross-field section as one context.  It cannot hand commitment points to a
//! relation verifier, however, because the nested proof body previously had no
//! canonical schema.  This module closes exactly that transport gap.
//!
//! The decoder below admits one fixed 40-limb inventory: 688 comparator-top
//! points, 18,232 comparator auxiliary points, 4,128 signed-small-source
//! points, and 25,600 q-mask points in their sole implicit role/owner/column
//! order.  The top prefix is the exact missing input to the first streaming
//! product argument: all 344 difference-top commitments followed by all 344
//! sum-top commitments.  The decoder checks the section cap before
//! parsing, exact length and geometry, every non-identity point encoding, a
//! context-bound streaming inventory root, a non-empty bounded continuation,
//! and the final codec digest.  It also retains the already-authenticated 200
//! qPCS product/opening-quotient pairs as canonical residues.
//!
//! A separate move-only pre-qPCS preflight may validate that same inner proof
//! body before the final transcript exists and lend only its 6,400 q-mask
//! digit commitments to the early root.  That state is provisional and
//! non-authorizing: the header supplies its own prior-context digest, the
//! production proof-slice lease is uninhabited, and no raw bytes, digest,
//! root, continuation, or point array can escape.  Later authentication must
//! consume the preflight against the exact same typed proof allocation and
//! the linked final context; that final transition performs no second header,
//! full-point-validation, inventory-root, continuation, or codec pass.  The
//! required early root still decodes its 6,400 selected points individually.
//!
//! This is deliberately not the cross-field/global-lookup verifier.  The
//! retired implementation covers only 38 limbs and assumes uninhabited source,
//! range, q-mask, and qPCS seals.  Its global lookup additionally lacks the
//! three enormous monolithic vector-arithmetic product arguments.  The first
//! argument is handled downstream as 344 bounded statement-3 proofs, and the
//! comparator-carry product is split into five bounded statement-5 cores per
//! group.  Radix-digit membership and subtraction, signed-small, q-mask,
//! inverse, and lookup relations still remain.  The output here therefore
//! stays a move-only private prerequisite and the composite verifier remains
//! fail-closed.

use super::{
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_RADIX_LOG2_V1,
    },
    rns_native_section_codec::{
        CROSS_LOOKUP_FIXED_BYTES_V1, ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_source_terminal_cross_field::RnsNativeSourceTerminalCrossFieldPrerequisiteV1,
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
    rns_native_wire::ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_LOOKUP_SECTION_MAX_BYTES_V1,
};
use crate::vega::{VegaT256PointV1 as Point, sponge::Keccak256};
use core::convert::Infallible;

#[cfg(test)]
use std::cell::Cell;

const INVENTORY_VERSION_V1: u8 = 1;
const INVENTORY_FLAGS_V1: u8 = 0;
const INVENTORY_MAGIC_V1: [u8; 4] = *b"ZC40";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const REPETITIONS_V1: usize = 5;
const RECORDS_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize;
const BLOCKS_PER_RECORD_V1: usize = 8;
const RADIX_DIGITS_V1: usize = 18;
const COMPARATOR_SUBTRACTION_DIGITS_V1: usize = RADIX_DIGITS_V1 - 1;
const Q_MASK_DIGITS_V1: usize = 4;
const COMPARATOR_GROUPS_V1: usize = RECORDS_V1 * BLOCKS_PER_RECORD_V1;
const COMPARATOR_TOP_POINTS_V1: usize = 2 * COMPARATOR_GROUPS_V1;
const COMPARATOR_POINTS_PER_GROUP_V1: usize = 17 + 1 + 18 + 17;
const COMPARATOR_AUXILIARY_POINTS_V1: usize = COMPARATOR_GROUPS_V1 * COMPARATOR_POINTS_PER_GROUP_V1;
const COMPARATOR_POINTS_V1: usize = COMPARATOR_TOP_POINTS_V1 + COMPARATOR_AUXILIARY_POINTS_V1;
const SMALL_SOURCE_ROLES_V1: usize = 3;
const SMALL_SOURCE_POINTS_PER_BLOCK_V1: usize = 4;
const SMALL_SOURCE_BLOCKS_V1: usize = RECORDS_V1 * SMALL_SOURCE_ROLES_V1 * BLOCKS_PER_RECORD_V1;
const SMALL_SOURCE_POINTS_V1: usize = SMALL_SOURCE_BLOCKS_V1 * SMALL_SOURCE_POINTS_PER_BLOCK_V1;
const Q_MASK_POINTS_PER_BLOCK_V1: usize = 4 * Q_MASK_DIGITS_V1;
const Q_MASK_BLOCKS_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1 * BLOCKS_PER_RECORD_V1;
const Q_MASK_POINTS_V1: usize = Q_MASK_BLOCKS_V1 * Q_MASK_POINTS_PER_BLOCK_V1;
const INVENTORY_POINTS_V1: usize = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1 + Q_MASK_POINTS_V1;
const INVENTORY_BYTES_V1: usize = INVENTORY_POINTS_V1 * POINT_BYTES_V1;
const PRE_QPCS_Q_MASK_S_POINTS_V1: usize = Q_MASK_BLOCKS_V1 * Q_MASK_DIGITS_V1;
const PRE_QPCS_Q_MASK_S_CANONICAL_BYTES_V1: usize = PRE_QPCS_Q_MASK_S_POINTS_V1 * POINT_BYTES_V1;
const Q_MASK_INVENTORY_FIRST_ORDINAL_V1: usize = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1;
const PRE_QPCS_Q_MASK_FIRST_PROOF_OFFSET_V1: usize =
    HEADER_BYTES_V1 + Q_MASK_INVENTORY_FIRST_ORDINAL_V1 * POINT_BYTES_V1;
const PRE_QPCS_Q_MASK_LAST_INVENTORY_ORDINAL_V1: usize = Q_MASK_INVENTORY_FIRST_ORDINAL_V1
    + (Q_MASK_BLOCKS_V1 - 1) * Q_MASK_POINTS_PER_BLOCK_V1
    + (Q_MASK_DIGITS_V1 - 1);
const PRE_QPCS_Q_MASK_LAST_PROOF_OFFSET_V1: usize =
    HEADER_BYTES_V1 + PRE_QPCS_Q_MASK_LAST_INVENTORY_ORDINAL_V1 * POINT_BYTES_V1;
const QPCS_ROWS_PER_REPETITION_V1: usize = 2;
const QPCS_EVALUATION_BYTES_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1 * QPCS_ROWS_PER_REPETITION_V1 * 8;

// Header: magic/version/flags/header/total, eight one-byte geometry fields,
// four u32 point counts, three digests, and one u32 continuation length.
const HEADER_BYTES_V1: usize = 4 + 1 + 1 + 2 + 4 + 8 + 4 * 4 + 3 * DIGEST_BYTES_V1 + 4;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const MIN_PROOF_BYTES_V1: usize = HEADER_BYTES_V1 + INVENTORY_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
const PROOF_MAX_BYTES_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_LOOKUP_SECTION_MAX_BYTES_V1
    as usize
    - CROSS_LOOKUP_FIXED_BYTES_V1;
pub(super) const RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1: usize =
    PROOF_MAX_BYTES_V1 - HEADER_BYTES_V1 - INVENTORY_BYTES_V1 - CODEC_DIGEST_BYTES_V1;

const PRIOR_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.prior-context";
const INVENTORY_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.root";
const CONTINUATION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.continuation";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.codec";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.prerequisite";

// These are acceptance capabilities, not progress counters.  They must remain
// false until the named proof obligations have actual first-party verifiers.
const COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_ARGUMENT_AVAILABLE_V1: bool = true;
const RANGE_AND_CARRY_RELATIONS_VERIFIED_V1: bool = false;
const CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const PRE_QPCS_Q_MASK_PRODUCTION_LEASE_AVAILABLE_V1: bool = false;
const PRE_QPCS_Q_MASK_PREFLIGHT_LIVE_V1: bool = false;
const PRE_QPCS_Q_MASK_SOURCE_INTEGRATED_V1: bool = false;
const PRE_QPCS_Q_MASK_DIRECT_INTEGRATED_V1: bool = false;
const PRE_QPCS_Q_MASK_COMPOSITE_INTEGRATED_V1: bool = false;
const PRE_QPCS_Q_MASK_RESOURCE_EVIDENCE_QUALIFIED_V1: bool = false;
const PRE_QPCS_Q_MASK_READINESS_V1: bool = false;
const PRE_QPCS_Q_MASK_RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(RECORDS_V1 == 43);
    assert!(COMPARATOR_GROUPS_V1 == 344);
    assert!(COMPARATOR_TOP_POINTS_V1 == 688);
    assert!(COMPARATOR_POINTS_PER_GROUP_V1 == 53);
    assert!(COMPARATOR_AUXILIARY_POINTS_V1 == 18_232);
    assert!(COMPARATOR_POINTS_V1 == 18_920);
    assert!(SMALL_SOURCE_BLOCKS_V1 == 1_032);
    assert!(SMALL_SOURCE_POINTS_V1 == 4_128);
    assert!(Q_MASK_BLOCKS_V1 == 1_600);
    assert!(Q_MASK_POINTS_V1 == 25_600);
    assert!(PRE_QPCS_Q_MASK_S_POINTS_V1 == 6_400);
    assert!(PRE_QPCS_Q_MASK_S_CANONICAL_BYTES_V1 == 211_200);
    assert!(Q_MASK_INVENTORY_FIRST_ORDINAL_V1 == 23_048);
    assert!(PRE_QPCS_Q_MASK_FIRST_PROOF_OFFSET_V1 == 760_720);
    assert!(PRE_QPCS_Q_MASK_LAST_INVENTORY_ORDINAL_V1 == 48_635);
    assert!(PRE_QPCS_Q_MASK_LAST_PROOF_OFFSET_V1 == 1_605_091);
    assert!(PRE_QPCS_Q_MASK_LAST_PROOF_OFFSET_V1 + POINT_BYTES_V1 == 1_605_124);
    assert!(INVENTORY_POINTS_V1 == 48_648);
    assert!(INVENTORY_BYTES_V1 == 1_605_384);
    assert!(QPCS_EVALUATION_BYTES_V1 == 3_200);
    assert!(HEADER_BYTES_V1 == 136);
    assert!(MIN_PROOF_BYTES_V1 == 1_605_553);
    assert!(PROOF_MAX_BYTES_V1 == 8_385_797);
    assert!(RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 == 6_780_245);
    assert!(MIN_PROOF_BYTES_V1 < PROOF_MAX_BYTES_V1);
    assert!(COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_ARGUMENT_AVAILABLE_V1);
    assert!(!RANGE_AND_CARRY_RELATIONS_VERIFIED_V1);
    assert!(!CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!PRE_QPCS_Q_MASK_PRODUCTION_LEASE_AVAILABLE_V1);
    assert!(!PRE_QPCS_Q_MASK_PREFLIGHT_LIVE_V1);
    assert!(!PRE_QPCS_Q_MASK_SOURCE_INTEGRATED_V1);
    assert!(!PRE_QPCS_Q_MASK_DIRECT_INTEGRATED_V1);
    assert!(!PRE_QPCS_Q_MASK_COMPOSITE_INTEGRATED_V1);
    assert!(!PRE_QPCS_Q_MASK_RESOURCE_EVIDENCE_QUALIFIED_V1);
    assert!(!PRE_QPCS_Q_MASK_READINESS_V1);
    assert!(!PRE_QPCS_Q_MASK_RELEASE_READY_V1);
};

/// Failure while authenticating the exact 40-limb commitment inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeCrossFieldInventoryErrorV1 {
    InvalidContext,
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidPoint,
    InvalidQpcsEvaluation,
    InvalidIntegrity,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeCrossFieldInventoryErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeCrossFieldInventoryErrorV1 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum InventoryPointRoleV1 {
    ComparatorDifferenceTop = 1,
    ComparatorSumTop = 2,
    ComparatorDifferenceDigit = 3,
    ComparatorMixedTop = 4,
    ComparatorBorrow = 5,
    ComparatorDifferenceInverse = 6,
    SmallSigned = 7,
    SmallNegativeMagnitude = 8,
    SmallPositiveInverse = 9,
    SmallNegativeInverse = 10,
    QMaskDigit = 11,
    QMaskDigitInverse = 12,
    QMaskComplementDigit = 13,
    QMaskComplementInverse = 14,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InventoryCoordinateV1 {
    role: InventoryPointRoleV1,
    owner: usize,
    column: usize,
}

/// Exact commitment tuple consumed by comparator product statement 5.
///
/// The ordering is semantic rather than wire-dependent: `difference_top` is
/// the already-verified `bD` vector, `mixed_top` is `m = bD * beta_16`, and
/// `borrows[h]` is the vector `beta_h` for `h = 0..17`.
#[derive(Clone, Copy)]
pub(super) struct ComparatorRangeCarryCommitmentsV1 {
    pub(super) difference_top: Point,
    pub(super) mixed_top: Point,
    pub(super) borrows: [Point; RADIX_DIGITS_V1],
}

/// Exact low-digit tuple consumed by centering-subtraction statement 4.
///
/// `difference_digits[h]` aliases the inventory `Delta_h` commitment and
/// `borrows[h]` aliases `beta_h`, both for `h = 0..16`.  The mixed-top,
/// final-borrow, and inverse points remain under their original owners.
#[derive(Clone, Copy)]
pub(super) struct ComparatorSubtractionCommitmentsV1 {
    pub(super) difference_digits: [Point; COMPARATOR_SUBTRACTION_DIGITS_V1],
    pub(super) borrows: [Point; COMPARATOR_SUBTRACTION_DIGITS_V1],
}

/// Exact commitment tuple consumed by small-sign product statement 8.
///
/// `positive` is derived from the two authenticated inventory points and is
/// rejected if the sum is the identity.  It is never accepted as an
/// independently supplied commitment.
#[derive(Clone, Copy)]
pub(super) struct SmallSourceProductCommitmentsV1 {
    pub(super) signed: Point,
    pub(super) negative_magnitude: Point,
    pub(super) positive: Point,
}

/// Exact q-mask digit tuple consumed by linear statements 10 and 11.
///
/// Lookup inverses deliberately remain owned by the inventory for the later
/// committed-global-lookup verifier.
#[derive(Clone, Copy)]
pub(super) struct QMaskLinearCommitmentsV1 {
    pub(super) digits: [Point; Q_MASK_DIGITS_V1],
    pub(super) complement_digits: [Point; Q_MASK_DIGITS_V1],
}

/// Exact post-`z` q-mask inverse tuple aliased by the sole global lookup.
///
/// The raw digit commitments remain under `QMaskLinearCommitmentsV1`; this
/// tuple exposes only their already-authenticated inverse owners and never
/// duplicates point bytes.
#[derive(Clone, Copy)]
pub(super) struct QMaskLookupInverseCommitmentsV1 {
    pub(super) digit_inverses: [Point; Q_MASK_DIGITS_V1],
    pub(super) complement_inverses: [Point; Q_MASK_DIGITS_V1],
}

fn comparator_subtraction_commitments_v1(
    inventory: &[u8],
    group: usize,
) -> Option<ComparatorSubtractionCommitmentsV1> {
    if inventory.len() != INVENTORY_BYTES_V1 || group >= COMPARATOR_GROUPS_V1 {
        return None;
    }
    let point_at = |ordinal: usize| {
        let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
        let end = offset.checked_add(POINT_BYTES_V1)?;
        Point::from_non_identity_wire_bytes_exact(inventory.get(offset..end)?).ok()
    };
    let auxiliary =
        COMPARATOR_TOP_POINTS_V1.checked_add(group.checked_mul(COMPARATOR_POINTS_PER_GROUP_V1)?)?;
    let first_borrow = auxiliary.checked_add(18)?;
    let mut difference_digits = [point_at(auxiliary)?; COMPARATOR_SUBTRACTION_DIGITS_V1];
    let mut borrows = [point_at(first_borrow)?; COMPARATOR_SUBTRACTION_DIGITS_V1];
    for column in 1..COMPARATOR_SUBTRACTION_DIGITS_V1 {
        difference_digits[column] = point_at(auxiliary.checked_add(column)?)?;
        borrows[column] = point_at(first_borrow.checked_add(column)?)?;
    }
    Some(ComparatorSubtractionCommitmentsV1 {
        difference_digits,
        borrows,
    })
}

fn small_source_product_commitments_v1(
    inventory: &[u8],
    block: usize,
) -> Option<SmallSourceProductCommitmentsV1> {
    if inventory.len() != INVENTORY_BYTES_V1 || block >= SMALL_SOURCE_BLOCKS_V1 {
        return None;
    }
    let point_at = |ordinal: usize| {
        let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
        let end = offset.checked_add(POINT_BYTES_V1)?;
        Point::from_non_identity_wire_bytes_exact(inventory.get(offset..end)?).ok()
    };
    let first =
        COMPARATOR_POINTS_V1.checked_add(block.checked_mul(SMALL_SOURCE_POINTS_PER_BLOCK_V1)?)?;
    let signed = point_at(first)?;
    let negative_magnitude = point_at(first.checked_add(1)?)?;
    let positive = signed + negative_magnitude;
    if positive.is_identity() {
        return None;
    }
    Some(SmallSourceProductCommitmentsV1 {
        signed,
        negative_magnitude,
        positive,
    })
}

fn q_mask_linear_commitments_v1(
    inventory: &[u8],
    owner: usize,
) -> Option<QMaskLinearCommitmentsV1> {
    if inventory.len() != INVENTORY_BYTES_V1 || owner >= Q_MASK_BLOCKS_V1 {
        return None;
    }
    let point_at = |ordinal: usize| {
        let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
        let end = offset.checked_add(POINT_BYTES_V1)?;
        Point::from_non_identity_wire_bytes_exact(inventory.get(offset..end)?).ok()
    };
    let first = COMPARATOR_POINTS_V1
        .checked_add(SMALL_SOURCE_POINTS_V1)?
        .checked_add(owner.checked_mul(Q_MASK_POINTS_PER_BLOCK_V1)?)?;
    let mut digits = [point_at(first)?; Q_MASK_DIGITS_V1];
    let complement_first = first.checked_add(2 * Q_MASK_DIGITS_V1)?;
    let mut complement_digits = [point_at(complement_first)?; Q_MASK_DIGITS_V1];
    for digit in 1..Q_MASK_DIGITS_V1 {
        digits[digit] = point_at(first.checked_add(digit)?)?;
        complement_digits[digit] = point_at(complement_first.checked_add(digit)?)?;
    }
    Some(QMaskLinearCommitmentsV1 {
        digits,
        complement_digits,
    })
}

/// Decode exactly one pre-qPCS q-mask `S` digit from its canonical inventory
/// owner.  This deliberately cannot project complements, inverses, or an
/// array of points.
fn pre_qpcs_q_mask_s_digit_v1(inventory: &[u8], owner: usize, digit: usize) -> Option<Point> {
    if inventory.len() != INVENTORY_BYTES_V1
        || owner >= Q_MASK_BLOCKS_V1
        || digit >= Q_MASK_DIGITS_V1
    {
        return None;
    }
    let ordinal = COMPARATOR_POINTS_V1
        .checked_add(SMALL_SOURCE_POINTS_V1)?
        .checked_add(owner.checked_mul(Q_MASK_POINTS_PER_BLOCK_V1)?)?
        .checked_add(digit)?;
    let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
    #[cfg(test)]
    update_preflight_audit_counters_v1(|counters| counters.q_mask_digit_projections += 1);
    Point::from_non_identity_wire_bytes_exact(inventory.get(offset..offset + POINT_BYTES_V1)?).ok()
}

fn comparator_difference_inverse_v1(
    inventory: &[u8],
    group: usize,
    column: usize,
) -> Option<Point> {
    if inventory.len() != INVENTORY_BYTES_V1
        || group >= COMPARATOR_GROUPS_V1
        || column >= COMPARATOR_SUBTRACTION_DIGITS_V1
    {
        return None;
    }
    let ordinal = COMPARATOR_TOP_POINTS_V1
        .checked_add(group.checked_mul(COMPARATOR_POINTS_PER_GROUP_V1)?)?
        .checked_add(36)?
        .checked_add(column)?;
    let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
    Point::from_non_identity_wire_bytes_exact(inventory.get(offset..offset + POINT_BYTES_V1)?).ok()
}

fn small_source_lookup_inverses_v1(inventory: &[u8], block: usize) -> Option<(Point, Point)> {
    if inventory.len() != INVENTORY_BYTES_V1 || block >= SMALL_SOURCE_BLOCKS_V1 {
        return None;
    }
    let first = COMPARATOR_POINTS_V1
        .checked_add(block.checked_mul(SMALL_SOURCE_POINTS_PER_BLOCK_V1)?)?
        .checked_add(2)?;
    let point_at = |ordinal: usize| {
        let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
        Point::from_non_identity_wire_bytes_exact(inventory.get(offset..offset + POINT_BYTES_V1)?)
            .ok()
    };
    Some((point_at(first)?, point_at(first.checked_add(1)?)?))
}

fn q_mask_lookup_inverses_v1(
    inventory: &[u8],
    owner: usize,
) -> Option<QMaskLookupInverseCommitmentsV1> {
    if inventory.len() != INVENTORY_BYTES_V1 || owner >= Q_MASK_BLOCKS_V1 {
        return None;
    }
    let first = COMPARATOR_POINTS_V1
        .checked_add(SMALL_SOURCE_POINTS_V1)?
        .checked_add(owner.checked_mul(Q_MASK_POINTS_PER_BLOCK_V1)?)?;
    let point_at = |local: usize| {
        let ordinal = first.checked_add(local)?;
        let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
        Point::from_non_identity_wire_bytes_exact(inventory.get(offset..offset + POINT_BYTES_V1)?)
            .ok()
    };
    let mut digit_inverses = [point_at(Q_MASK_DIGITS_V1)?; Q_MASK_DIGITS_V1];
    let complement_first = 3 * Q_MASK_DIGITS_V1;
    let mut complement_inverses = [point_at(complement_first)?; Q_MASK_DIGITS_V1];
    for column in 1..Q_MASK_DIGITS_V1 {
        digit_inverses[column] = point_at(Q_MASK_DIGITS_V1.checked_add(column)?)?;
        complement_inverses[column] = point_at(complement_first.checked_add(column)?)?;
    }
    Some(QMaskLookupInverseCommitmentsV1 {
        digit_inverses,
        complement_inverses,
    })
}

fn inventory_coordinate_v1(
    ordinal: usize,
) -> Result<InventoryCoordinateV1, RnsNativeCrossFieldInventoryErrorV1> {
    if ordinal < COMPARATOR_GROUPS_V1 {
        return Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceTop,
            owner: ordinal,
            column: 0,
        });
    }
    if ordinal < COMPARATOR_TOP_POINTS_V1 {
        return Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorSumTop,
            owner: ordinal - COMPARATOR_GROUPS_V1,
            column: 0,
        });
    }
    if ordinal < COMPARATOR_POINTS_V1 {
        let auxiliary = ordinal - COMPARATOR_TOP_POINTS_V1;
        let owner = auxiliary / COMPARATOR_POINTS_PER_GROUP_V1;
        let local = auxiliary % COMPARATOR_POINTS_PER_GROUP_V1;
        let (role, column) = match local {
            0..=16 => (InventoryPointRoleV1::ComparatorDifferenceDigit, local),
            17 => (InventoryPointRoleV1::ComparatorMixedTop, 0),
            18..=35 => (InventoryPointRoleV1::ComparatorBorrow, local - 18),
            36..=52 => (
                InventoryPointRoleV1::ComparatorDifferenceInverse,
                local - 36,
            ),
            _ => return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry),
        };
        return Ok(InventoryCoordinateV1 {
            role,
            owner,
            column,
        });
    }

    let small = ordinal - COMPARATOR_POINTS_V1;
    if small < SMALL_SOURCE_POINTS_V1 {
        let owner = small / SMALL_SOURCE_POINTS_PER_BLOCK_V1;
        let local = small % SMALL_SOURCE_POINTS_PER_BLOCK_V1;
        let role = [
            InventoryPointRoleV1::SmallSigned,
            InventoryPointRoleV1::SmallNegativeMagnitude,
            InventoryPointRoleV1::SmallPositiveInverse,
            InventoryPointRoleV1::SmallNegativeInverse,
        ][local];
        return Ok(InventoryCoordinateV1 {
            role,
            owner,
            column: 0,
        });
    }

    let mask = small - SMALL_SOURCE_POINTS_V1;
    if mask >= Q_MASK_POINTS_V1 {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry);
    }
    let owner = mask / Q_MASK_POINTS_PER_BLOCK_V1;
    let local = mask % Q_MASK_POINTS_PER_BLOCK_V1;
    let (role, column) = match local {
        0..=3 => (InventoryPointRoleV1::QMaskDigit, local),
        4..=7 => (InventoryPointRoleV1::QMaskDigitInverse, local - 4),
        8..=11 => (InventoryPointRoleV1::QMaskComplementDigit, local - 8),
        12..=15 => (InventoryPointRoleV1::QMaskComplementInverse, local - 12),
        _ => return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry),
    };
    Ok(InventoryCoordinateV1 {
        role,
        owner,
        column,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalQpcsEvaluationV1 {
    product: u64,
    opening_quotient: u64,
}

#[derive(Clone, Copy)]
struct CanonicalQpcsEvaluationGridV1<'a> {
    bytes: &'a [u8],
}

impl<'a> CanonicalQpcsEvaluationGridV1<'a> {
    fn from_authenticated_bytes_v1(
        bytes: &'a [u8],
    ) -> Result<Self, RnsNativeCrossFieldInventoryErrorV1> {
        if bytes.len() != QPCS_EVALUATION_BYTES_V1 {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation);
        }
        let grid = Self { bytes };
        for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            for repetition in 0..REPETITIONS_V1 {
                let value = grid
                    .get_v1(limb, repetition)
                    .ok_or(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation)?;
                let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
                if value.product >= modulus || value.opening_quotient >= modulus {
                    return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation);
                }
            }
        }
        Ok(grid)
    }

    fn get_v1(&self, limb: usize, repetition: usize) -> Option<CanonicalQpcsEvaluationV1> {
        if limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 || repetition >= REPETITIONS_V1 {
            return None;
        }
        let relation = limb * REPETITIONS_V1 + repetition;
        let offset = relation * QPCS_ROWS_PER_REPETITION_V1 * 8;
        let product = u64::from_be_bytes(self.bytes.get(offset..offset + 8)?.try_into().ok()?);
        let opening_quotient =
            u64::from_be_bytes(self.bytes.get(offset + 8..offset + 16)?.try_into().ok()?);
        Some(CanonicalQpcsEvaluationV1 {
            product,
            opening_quotient,
        })
    }
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeCrossFieldInventoryErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeCrossFieldInventoryErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeCrossFieldInventoryErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeCrossFieldInventoryErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeCrossFieldInventoryErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PreflightAuditCountersV1 {
    header_passes: usize,
    inventory_root_passes: usize,
    point_validation_decodes: usize,
    continuation_hash_passes: usize,
    codec_hash_passes: usize,
    q_mask_digit_projections: usize,
}

#[cfg(test)]
std::thread_local! {
    static PREFLIGHT_AUDIT_COUNTERS_V1: Cell<PreflightAuditCountersV1> =
        const { Cell::new(PreflightAuditCountersV1 {
            header_passes: 0,
            inventory_root_passes: 0,
            point_validation_decodes: 0,
            continuation_hash_passes: 0,
            codec_hash_passes: 0,
            q_mask_digit_projections: 0,
        }) };
}

#[cfg(test)]
fn update_preflight_audit_counters_v1(update: impl FnOnce(&mut PreflightAuditCountersV1)) {
    PREFLIGHT_AUDIT_COUNTERS_V1.with(|cell| {
        let mut counters = cell.get();
        update(&mut counters);
        cell.set(counters);
    });
}

#[cfg(test)]
fn preflight_audit_counters_v1() -> PreflightAuditCountersV1 {
    PREFLIGHT_AUDIT_COUNTERS_V1.with(Cell::get)
}

struct CrossFieldInventoryProofViewV1<'a> {
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    continuation_digest: [u8; DIGEST_BYTES_V1],
    inventory: &'a [u8],
    continuation: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> CrossFieldInventoryProofViewV1<'a> {
    /// Parse and authenticate the internally declared inventory body without
    /// granting authority to its header-supplied prior context.
    fn from_self_consistent_canonical_bytes_exact_v1(
        bytes: &'a [u8],
    ) -> Result<Self, RnsNativeCrossFieldInventoryErrorV1> {
        if bytes.len() > PROOF_MAX_BYTES_V1 {
            return Err(RnsNativeCrossFieldInventoryErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_PROOF_BYTES_V1 {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader);
        }
        #[cfg(test)]
        update_preflight_audit_counters_v1(|counters| counters.header_passes += 1);
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != INVENTORY_MAGIC_V1
            || decoder.u8()? != INVENTORY_VERSION_V1
            || decoder.u8()? != INVENTORY_FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || usize::from(decoder.u8()?) != REPETITIONS_V1
            || usize::from(decoder.u8()?) != RECORDS_V1
            || usize::from(decoder.u8()?) != BLOCKS_PER_RECORD_V1
            || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_RADIX_LOG2_V1
            || usize::from(decoder.u8()?) != RADIX_DIGITS_V1
            || usize::from(decoder.u8()?) != Q_MASK_DIGITS_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
                != COMPARATOR_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
                != SMALL_SOURCE_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
                != Q_MASK_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
                != INVENTORY_POINTS_V1
        {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry);
        }
        let prior_context_digest = decoder.array()?;
        let inventory_root = decoder.array()?;
        let continuation_digest = decoder.array()?;
        let continuation_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(INVENTORY_BYTES_V1)
            .and_then(|value| value.checked_add(continuation_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || continuation_len == 0
            || expected_total != bytes.len()
            || [prior_context_digest, inventory_root, continuation_digest].contains(&[0; 32])
            || prior_context_digest == inventory_root
            || prior_context_digest == continuation_digest
            || inventory_root == continuation_digest
        {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader);
        }
        let inventory = decoder.take(INVENTORY_BYTES_V1)?;
        let continuation = decoder.take(continuation_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        if decoder.cursor != bytes.len()
            || canonical_inventory_root_v1(prior_context_digest, inventory)? != inventory_root
            || canonical_continuation_digest_v1(prior_context_digest, inventory_root, continuation)?
                != continuation_digest
            || codec_digest == [0; DIGEST_BYTES_V1]
            || codec_digest == prior_context_digest
            || codec_digest == inventory_root
            || codec_digest == continuation_digest
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity);
        }
        Ok(Self {
            prior_context_digest,
            inventory_root,
            continuation_digest,
            inventory,
            continuation,
            codec_digest,
        })
    }

    fn validate_expected_prior_context_v1(
        &self,
        expected_prior_context: [u8; DIGEST_BYTES_V1],
    ) -> Result<(), RnsNativeCrossFieldInventoryErrorV1> {
        if self.prior_context_digest != expected_prior_context {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader);
        }
        Ok(())
    }

    fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
        expected_prior_context: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativeCrossFieldInventoryErrorV1> {
        let view = Self::from_self_consistent_canonical_bytes_exact_v1(bytes)?;
        view.validate_expected_prior_context_v1(expected_prior_context)?;
        Ok(view)
    }
}

/// Uninhabited production authority for the early inner-proof slice lease.
///
/// The outer proof/envelope layer does not yet issue this authority.  Keeping
/// the unavailable state in the type prevents a detached proof slice from
/// becoming a production preflight through this module.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the production proof-slice lease issuer is deliberately uninhabited"
)]
pub(super) struct RnsNativePreQpcsCrossProofLeaseIssuerV1 {
    unavailable: Infallible,
}

/// Move-only lease over the exact inner cross-proof allocation.
///
/// It has no raw-byte accessor and is useful only by consumption into the
/// provisional preflight below.  Production construction remains impossible;
/// the raw fixture is compiled only for tests.
#[allow(
    missing_copy_implementations,
    reason = "an exact proof allocation must have one consuming preflight owner"
)]
#[must_use = "the exact proof-slice lease must be consumed by its provisional preflight"]
pub(super) struct RnsNativePreQpcsCrossProofLeaseV1<'proof> {
    proof: &'proof [u8],
}

impl<'proof> RnsNativePreQpcsCrossProofLeaseV1<'proof> {
    #[allow(
        dead_code,
        reason = "the outer proof owner cannot issue this lease until its production adapter exists"
    )]
    pub(super) fn from_production_issuer_v1(
        issuer: RnsNativePreQpcsCrossProofLeaseIssuerV1,
    ) -> Self {
        match issuer.unavailable {}
    }

    #[cfg(test)]
    pub(super) const fn from_raw_fixture_v1(proof: &'proof [u8]) -> Self {
        Self { proof }
    }
}

/// Provisional, move-only, non-authorizing q-mask inventory preflight.
///
/// This owner proves only that one leased inner proof body is internally
/// canonical and self-consistent.  In particular, its header-selected prior
/// context is not trusted.  It exposes one q-mask `S` digit at a time and no
/// raw bytes, digest, root, continuation, inverse, complement, or point array.
/// Final authority can arise only when this exact owner is consumed against
/// the identical typed cross-section allocation and linked final context.
#[allow(
    missing_copy_implementations,
    reason = "the provisional proof lease and parsed view must be consumed exactly once"
)]
#[must_use = "the provisional q-mask owner must be consumed by final inventory authentication"]
pub(super) struct RnsNativePreQpcsQMaskInventoryPreflightV1<'proof> {
    lease: RnsNativePreQpcsCrossProofLeaseV1<'proof>,
    view: CrossFieldInventoryProofViewV1<'proof>,
}

impl<'proof> RnsNativePreQpcsQMaskInventoryPreflightV1<'proof> {
    pub(super) fn preflight_v1(
        lease: RnsNativePreQpcsCrossProofLeaseV1<'proof>,
    ) -> Result<Self, RnsNativeCrossFieldInventoryErrorV1> {
        let view = CrossFieldInventoryProofViewV1::from_self_consistent_canonical_bytes_exact_v1(
            lease.proof,
        )?;
        Ok(Self { lease, view })
    }

    /// Lend one exact q-mask `S` digit.  The direct module supplies the
    /// limb/repetition/block-to-owner mapping and converts failures into its
    /// own closed error vocabulary.
    pub(super) fn project_q_mask_s_digit_v1(&self, owner: usize, digit: usize) -> Option<Point> {
        pre_qpcs_q_mask_s_digit_v1(self.view.inventory, owner, digit)
    }

    fn into_exact_proof_view_v1(
        self,
        exact_proof: &'proof [u8],
    ) -> Result<CrossFieldInventoryProofViewV1<'proof>, RnsNativeCrossFieldInventoryErrorV1> {
        let Self { lease, view } = self;
        let same_pointer = core::ptr::eq(lease.proof.as_ptr(), exact_proof.as_ptr());
        let same_length = lease.proof.len() == exact_proof.len();
        let same_bytes = lease.proof == exact_proof;
        if !(same_pointer && same_length && same_bytes) {
            return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidContext);
        }
        Ok(view)
    }
}

fn canonical_inventory_root_v1(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldInventoryErrorV1> {
    if prior_context_digest == [0; DIGEST_BYTES_V1] || inventory.len() != INVENTORY_BYTES_V1 {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry);
    }
    #[cfg(test)]
    update_preflight_audit_counters_v1(|counters| counters.inventory_root_passes += 1);
    let mut hash = Keccak256::new();
    hash.update(INVENTORY_ROOT_DOMAIN_V1);
    hash.update(&[INVENTORY_VERSION_V1]);
    hash.update(&prior_context_digest);
    hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
    hash.update(&(REPETITIONS_V1 as u16).to_be_bytes());
    hash.update(&(INVENTORY_POINTS_V1 as u32).to_be_bytes());
    for (ordinal, encoded) in inventory.chunks_exact(POINT_BYTES_V1).enumerate() {
        #[cfg(test)]
        update_preflight_audit_counters_v1(|counters| {
            counters.point_validation_decodes += 1;
        });
        Point::from_non_identity_wire_bytes_exact(encoded)
            .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::InvalidPoint)?;
        let coordinate = inventory_coordinate_v1(ordinal)?;
        hash.update(&(ordinal as u32).to_be_bytes());
        hash.update(&[coordinate.role as u8]);
        hash.update(&(coordinate.owner as u32).to_be_bytes());
        hash.update(&(coordinate.column as u16).to_be_bytes());
        hash.update(encoded);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn canonical_continuation_digest_v1(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    continuation: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldInventoryErrorV1> {
    if prior_context_digest == [0; DIGEST_BYTES_V1]
        || inventory_root == [0; DIGEST_BYTES_V1]
        || continuation.is_empty()
    {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity);
    }
    #[cfg(test)]
    update_preflight_audit_counters_v1(|counters| counters.continuation_hash_passes += 1);
    let mut hash = Keccak256::new();
    hash.update(CONTINUATION_DOMAIN_V1);
    hash.update(&[INVENTORY_VERSION_V1]);
    hash.update(&prior_context_digest);
    hash.update(&inventory_root);
    hash.update(
        &u32::try_from(continuation.len())
            .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(continuation);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    #[cfg(test)]
    update_preflight_audit_counters_v1(|counters| counters.codec_hash_passes += 1);
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[INVENTORY_VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

fn absorb_digest_v1(hash: &mut Keccak256, digest: [u8; DIGEST_BYTES_V1]) {
    hash.update(&digest);
}

fn prior_context_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    linked: &RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'_, S>,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'_>,
    qpcs_evaluations: CanonicalQpcsEvaluationGridV1<'_>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldInventoryErrorV1> {
    linked
        .validate_cross_section_v1(cross)
        .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::InvalidContext)?;
    linked
        .terminal()
        .validate_context_v1(transcript)
        .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::InvalidContext)?;
    linked
        .zero_padding()
        .validate_context_v1(transcript)
        .map_err(|_| RnsNativeCrossFieldInventoryErrorV1::InvalidContext)?;
    let source = linked.source();
    let layout = source.snapshot().layout();
    let qpcs = source.qpcs();
    if layout.profile_digest() != transcript.profile_digest()
        || layout.topology_digest() != transcript.topology_digest()
        || layout.release_candidate_digest() != transcript.release_candidate_digest()
        || layout.statement_digest() != transcript.statement_digest()
        || layout.operational_context_digest() != transcript.operational_context_digest()
        || layout.source_binding_digest() != transcript.source_binding_digest()
        || source.public_bundle_digest() != transcript.public_ciphertext_digest()
        || qpcs.transcript_digest() != transcript.transcript_digest()
        || qpcs.query_seed() != transcript.qpcs_query_challenge_seed()
    {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidContext);
    }

    let mut hash = Keccak256::new();
    hash.update(PRIOR_CONTEXT_DOMAIN_V1);
    hash.update(&[INVENTORY_VERSION_V1]);
    for digest in [
        transcript.profile_manifest_digest(),
        transcript.profile_digest(),
        transcript.topology_digest(),
        transcript.release_candidate_digest(),
        transcript.statement_digest(),
        transcript.operational_context_digest(),
        transcript.source_binding_digest(),
        transcript.main_snapshot_digest(),
        transcript.nonce_snapshot_digest(),
        transcript.source_receipt_digest(),
        transcript.governed_roster_digest(),
        transcript.public_ciphertext_digest(),
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.qpcs_initial_root(),
        transcript.qpcs_quotient_root(),
        transcript.cross_field_root(),
        transcript.global_lookup_root(),
        transcript.zero_padding_root(),
        transcript.transcript_digest(),
        source.public_key_digest(),
        source.public_bundle_digest(),
        source.formula_digest(),
        source.mapping_digest(),
        source.aggregation_schedule_digest(),
        source.preflight_statement_digest(),
        qpcs.parameter_digest(),
        qpcs.transcript_digest(),
        qpcs.query_seed(),
        qpcs.section_binding_digest(),
        qpcs.schedule_digest(),
        qpcs.evaluation_binding_digest(),
        linked.terminal().binding_digest(),
        linked.terminal().hyrax_digest(),
        linked.terminal().bp_digest(),
        linked.terminal().bridge_root(),
        linked.zero_padding().binding_digest(),
        linked.zero_padding().point_set_digest(),
        linked.zero_padding().root(),
        linked.zero_padding().proof_digest(),
        linked.formula_digest(),
        linked.opening_bundle_digest(),
        linked.aggregate_point_digest(),
        linked.point_bundle_digest(),
        linked.limb_bundle_digest(),
        linked.round_bundle_digest(),
        linked.zero_limb_bundle_digest(),
    ] {
        absorb_digest_v1(&mut hash, digest);
    }
    for opening in transcript.opening_commitments() {
        hash.update(&[opening.family() as u8, opening.family_index()]);
        absorb_digest_v1(&mut hash, opening.source_commitment_digest());
        absorb_digest_v1(&mut hash, opening.hyrax_commitment_digest());
    }
    for root in transcript.qpcs_fri_roots() {
        hash.update(&[root.layer()]);
        absorb_digest_v1(&mut hash, root.root());
    }
    for seed in transcript.ordered_challenge_seeds() {
        absorb_digest_v1(&mut hash, seed);
    }
    for digest in linked.zero_padding().limb_padding_digests() {
        absorb_digest_v1(&mut hash, *digest);
    }
    for digest in cross
        .point_evaluation_digests()
        .iter()
        .chain(cross.limb_relation_digests())
        .chain(cross.sumcheck_round_digests())
    {
        absorb_digest_v1(&mut hash, *digest);
    }
    hash.update(&(qpcs_evaluations.bytes.len() as u32).to_be_bytes());
    hash.update(qpcs_evaluations.bytes);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn prerequisite_binding_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    linked: &RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'_, S>,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'_>,
    view: &CrossFieldInventoryProofViewV1<'_>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldInventoryErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[INVENTORY_VERSION_V1]);
    for digest in [
        view.prior_context_digest,
        linked.source().statement_anchor_digest(),
        linked.source().qpcs().residual_digest(),
        linked.cross_proof_digest(),
        linked.cross_link_digest(),
        linked.anchor_digest(),
        view.inventory_root,
        view.continuation_digest,
        view.codec_digest,
    ] {
        absorb_digest_v1(&mut hash, digest);
    }
    for digest in cross
        .point_evaluation_digests()
        .iter()
        .chain(cross.limb_relation_digests())
        .chain(cross.sumcheck_round_digests())
    {
        absorb_digest_v1(&mut hash, *digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn finish_rns_native_cross_field_inventory_authentication_v1<'source, 'proof, S>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    linked: RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'source, S>,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'proof>,
    qpcs_evaluations: CanonicalQpcsEvaluationGridV1<'source>,
    view: CrossFieldInventoryProofViewV1<'proof>,
) -> Result<
    RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
    RnsNativeCrossFieldInventoryErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let binding_digest = prerequisite_binding_digest_v1(&linked, cross, &view)?;
    Ok(RnsNativeCrossFieldInventoryPrerequisiteV1 {
        linked,
        qpcs_evaluations,
        inventory: view.inventory,
        continuation: view.continuation,
        terminal_transcript_digest: transcript.transcript_digest(),
        prior_context_digest: view.prior_context_digest,
        inventory_root: view.inventory_root,
        continuation_digest: view.continuation_digest,
        binding_digest,
    })
}

/// Move-only, private, non-authorizing 40-limb commitment inventory.
///
/// The continuation is intentionally opaque.  It must later be consumed by a
/// streaming verifier that proves the cross-field algebra and the complete
/// committed lookup before any verified receipt can exist.
#[allow(
    missing_copy_implementations,
    reason = "the exact inventory and source owner must be consumed once by the future streaming verifier"
)]
pub(super) struct RnsNativeCrossFieldInventoryPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    linked: RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'source, S>,
    qpcs_evaluations: CanonicalQpcsEvaluationGridV1<'source>,
    inventory: &'proof [u8],
    continuation: &'proof [u8],
    terminal_transcript_digest: [u8; DIGEST_BYTES_V1],
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    continuation_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn linked(
        &self,
    ) -> &RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'source, S> {
        &self.linked
    }

    pub(super) const fn continuation(&self) -> &'proof [u8] {
        self.continuation
    }

    /// Exact final terminal transcript that authenticated this inventory.
    /// The direct claimed-frame adapter uses this private identity to reject
    /// cross-session inventory/claim pairing before exposing any successor.
    pub(super) const fn terminal_transcript_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.terminal_transcript_digest
    }

    pub(super) const fn prior_context_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.prior_context_digest
    }

    pub(super) const fn inventory_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.inventory_root
    }

    pub(super) const fn continuation_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.continuation_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    /// Narrow post-equation alias for the enclosing packing identity retained
    /// by the exact linked source/terminal owner. It must never be projected
    /// into a pre-challenge safe core.
    pub(super) const fn enclosing_packing_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.linked.anchor_digest()
    }

    pub(super) fn comparator_top_commitments(&self, group: usize) -> Option<(Point, Point)> {
        if group >= COMPARATOR_GROUPS_V1 {
            return None;
        }
        let point_at = |ordinal: usize| {
            let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
            let end = offset.checked_add(POINT_BYTES_V1)?;
            Point::from_non_identity_wire_bytes_exact(self.inventory.get(offset..end)?).ok()
        };
        Some((point_at(group)?, point_at(COMPARATOR_GROUPS_V1 + group)?))
    }

    pub(super) fn comparator_range_carry_commitments(
        &self,
        group: usize,
    ) -> Option<ComparatorRangeCarryCommitmentsV1> {
        if group >= COMPARATOR_GROUPS_V1 {
            return None;
        }
        let point_at = |ordinal: usize| {
            let offset = ordinal.checked_mul(POINT_BYTES_V1)?;
            let end = offset.checked_add(POINT_BYTES_V1)?;
            Point::from_non_identity_wire_bytes_exact(self.inventory.get(offset..end)?).ok()
        };
        let auxiliary = COMPARATOR_TOP_POINTS_V1
            .checked_add(group.checked_mul(COMPARATOR_POINTS_PER_GROUP_V1)?)?;
        let first_borrow = auxiliary.checked_add(18)?;
        let mut borrows = [point_at(first_borrow)?; RADIX_DIGITS_V1];
        for (column, borrow) in borrows.iter_mut().enumerate().skip(1) {
            *borrow = point_at(first_borrow.checked_add(column)?)?;
        }
        Some(ComparatorRangeCarryCommitmentsV1 {
            difference_top: point_at(group)?,
            mixed_top: point_at(auxiliary.checked_add(17)?)?,
            borrows,
        })
    }

    pub(super) fn comparator_subtraction_commitments(
        &self,
        group: usize,
    ) -> Option<ComparatorSubtractionCommitmentsV1> {
        comparator_subtraction_commitments_v1(self.inventory, group)
    }

    pub(super) fn small_source_product_commitments(
        &self,
        block: usize,
    ) -> Option<SmallSourceProductCommitmentsV1> {
        small_source_product_commitments_v1(self.inventory, block)
    }

    pub(super) fn q_mask_linear_commitments(
        &self,
        owner: usize,
    ) -> Option<QMaskLinearCommitmentsV1> {
        q_mask_linear_commitments_v1(self.inventory, owner)
    }

    pub(super) fn comparator_difference_inverse(
        &self,
        group: usize,
        column: usize,
    ) -> Option<Point> {
        comparator_difference_inverse_v1(self.inventory, group, column)
    }

    pub(super) fn small_source_lookup_inverses(&self, block: usize) -> Option<(Point, Point)> {
        small_source_lookup_inverses_v1(self.inventory, block)
    }

    pub(super) fn q_mask_lookup_inverses(
        &self,
        owner: usize,
    ) -> Option<QMaskLookupInverseCommitmentsV1> {
        q_mask_lookup_inverses_v1(self.inventory, owner)
    }

    pub(super) fn qpcs_evaluation(&self, limb: usize, repetition: usize) -> Option<(u64, u64)> {
        self.qpcs_evaluations
            .get_v1(limb, repetition)
            .map(|value| (value.product, value.opening_quotient))
    }
}

/// Consume the authenticated source/terminal token into the exact inventory
/// prerequisite.  This function performs transport and identity
/// authentication only; it does not verify the opaque continuation.
#[allow(
    dead_code,
    reason = "the private source-to-inventory entry is retained until the composite can supply its confidential snapshot owner"
)]
pub(super) fn authenticate_rns_native_cross_field_inventory_v1<'source, 'proof, S>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    linked: RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'source, S>,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'proof>,
) -> Result<
    RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
    RnsNativeCrossFieldInventoryErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let qpcs_evaluations = CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(
        linked.source().qpcs().evaluations(),
    )?;
    let prior_context_digest =
        prior_context_digest_v1(transcript, &linked, cross, qpcs_evaluations)?;
    let view = CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(
        cross.proof(),
        prior_context_digest,
    )?;
    finish_rns_native_cross_field_inventory_authentication_v1(
        transcript,
        linked,
        cross,
        qpcs_evaluations,
        view,
    )
}

/// Consume the provisional pre-qPCS owner into the ordinary authenticated
/// inventory prerequisite without a second header, full point-validation,
/// inventory-root, continuation, or codec pass.  The ordinary prerequisite
/// binding hash is still computed exactly once after final context binding.
///
/// The exact typed `cross.proof()` allocation must be pointer-, length-, and
/// byte-identical to the leased allocation.  Equal bytes copied into a second
/// allocation are rejected.  This is a source-only transition: its production
/// lease issuer and every live/integration/readiness/release gate remain
/// unavailable.
#[allow(
    dead_code,
    reason = "the exact outer production proof-slice lease issuer is deliberately unavailable"
)]
pub(super) fn authenticate_rns_native_cross_field_inventory_from_pre_qpcs_preflight_v1<
    'source,
    'proof,
    S,
>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    linked: RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'source, S>,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'proof>,
    preflight: RnsNativePreQpcsQMaskInventoryPreflightV1<'proof>,
) -> Result<
    RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
    RnsNativeCrossFieldInventoryErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let view = preflight.into_exact_proof_view_v1(cross.proof())?;
    let qpcs_evaluations = CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(
        linked.source().qpcs().evaluations(),
    )?;
    let prior_context_digest =
        prior_context_digest_v1(transcript, &linked, cross, qpcs_evaluations)?;
    view.validate_expected_prior_context_v1(prior_context_digest)?;
    finish_rns_native_cross_field_inventory_authentication_v1(
        transcript,
        linked,
        cross,
        qpcs_evaluations,
        view,
    )
}

#[cfg(test)]
#[path = "rns_native_cross_field_inventory_tests.rs"]
mod tests;
