//! Canonical persistent-opening inventory for the native40 global lookup.
//!
//! The verifier and source session share these exact roles. Source commitments
//! remain separately authenticated; native pre-z roles exclude them. The sole
//! post-z inverse set is shared, and no residual-vector or second-z inventory
//! exists. This descriptor cannot construct entropy, a proof, or source authority.

use super::{ZkAmsMkheErrorV1, rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1};
use crate::vega::{
    VEGA_T256_SCALAR_MODULUS_BE_V1, bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    sponge::Keccak256,
};

#[path = "global_lookup_statement_v1/vector_arithmetic_plane_openings_v1.rs"]
mod vector_arithmetic_plane_openings_v1;
pub(in crate::vega::zk_ams::mkhe) use vector_arithmetic_plane_openings_v1::{
    OrderedPlaneSpoolSnapshotV1, OrderedPlaneSpoolWriterV1, OrderedSnapshotErrorV1,
    OrderedStorageSessionBudgetV1, QMaskSFileMemoryV1, QMaskSFilePlanV1, QMaskSFileV1,
    SealedQMaskSFileV1, WrittenQMaskSBlockFileV1, materialized_plane_context_digest_v1,
};

const VERSION_V1: u8 = 1;
const GROUPS_V1: u32 = 43 * 8;
const LOW_DIGITS_V1: u32 = 17;
const BORROWS_V1: u32 = 18;
const SMALL_BLOCKS_V1: u32 = 43 * 3 * 8;
const Q_MASK_PER_ROLE_V1: u32 = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u32 * 5 * 8 * 4;
const TOPOLOGY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.topology\0";
const INVENTORY_LANGUAGE_V1: &[u8] = b"native40-persistent-opening-inventory;source344-is-separately-bound;one-pre-z-set;sole-z-before-all-inverses;shared-D/S-inverses;no-residual-q3/q5/q8;no-retired702-mask;distinct-multiplicity32768-and-inverse-product-mask16384-with87-prefix-scalars;storage-ordinals-are-not-wire-ordinals";

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GlobalLookupCommitmentPhaseV1 {
    ChallengeIndependent = 1,
    PostZ = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GlobalLookupCommitmentPurposeV1 {
    Source = 1,
    ExistingDifferenceLow = 2,
    ExistingSumLow = 3,
    ComparatorDifferenceTop = 4,
    ComparatorSumTop = 5,
    ComparatorDifferenceDigit = 6,
    ComparatorBorrow = 7,
    ComparatorMixedTop = 8,
    SmallSigned = 9,
    SmallNegativeMagnitude = 10,
    QMaskDigit = 11,
    QMaskComplementDigit = 12,
    Multiplicity = 13,
    InverseProductMask = 14,
    SharedDifferenceInverse = 15,
    SharedSumInverse = 16,
    ComparatorDifferenceInverse = 17,
    SmallPositiveInverse = 18,
    SmallNegativeInverse = 19,
    QMaskDigitInverse = 20,
    QMaskComplementInverse = 21,
}

impl GlobalLookupCommitmentPurposeV1 {
    pub(super) const fn count_v1(self) -> usize {
        let count = match self {
            Self::Source
            | Self::ComparatorDifferenceTop
            | Self::ComparatorSumTop
            | Self::ComparatorMixedTop => GROUPS_V1,
            Self::ExistingDifferenceLow
            | Self::ExistingSumLow
            | Self::ComparatorDifferenceDigit
            | Self::SharedDifferenceInverse
            | Self::SharedSumInverse
            | Self::ComparatorDifferenceInverse => GROUPS_V1 * LOW_DIGITS_V1,
            Self::ComparatorBorrow => GROUPS_V1 * BORROWS_V1,
            Self::SmallSigned
            | Self::SmallNegativeMagnitude
            | Self::SmallPositiveInverse
            | Self::SmallNegativeInverse => SMALL_BLOCKS_V1,
            Self::QMaskDigit
            | Self::QMaskComplementDigit
            | Self::QMaskDigitInverse
            | Self::QMaskComplementInverse => Q_MASK_PER_ROLE_V1,
            Self::Multiplicity | Self::InverseProductMask => 1,
        };
        count as usize
    }

    pub(super) const fn phase_v1(self) -> GlobalLookupCommitmentPhaseV1 {
        match self {
            Self::SharedDifferenceInverse
            | Self::SharedSumInverse
            | Self::ComparatorDifferenceInverse
            | Self::SmallPositiveInverse
            | Self::SmallNegativeInverse
            | Self::QMaskDigitInverse
            | Self::QMaskComplementInverse => GlobalLookupCommitmentPhaseV1::PostZ,
            _ => GlobalLookupCommitmentPhaseV1::ChallengeIndependent,
        }
    }
}

pub(super) const PRE_Z_POINT_PURPOSES_V1: [GlobalLookupCommitmentPurposeV1; 13] = [
    GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow,
    GlobalLookupCommitmentPurposeV1::ExistingSumLow,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop,
    GlobalLookupCommitmentPurposeV1::ComparatorSumTop,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit,
    GlobalLookupCommitmentPurposeV1::ComparatorBorrow,
    GlobalLookupCommitmentPurposeV1::ComparatorMixedTop,
    GlobalLookupCommitmentPurposeV1::SmallSigned,
    GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude,
    GlobalLookupCommitmentPurposeV1::QMaskDigit,
    GlobalLookupCommitmentPurposeV1::QMaskComplementDigit,
    GlobalLookupCommitmentPurposeV1::Multiplicity,
    GlobalLookupCommitmentPurposeV1::InverseProductMask,
];

pub(super) const POST_Z_POINT_PURPOSES_V1: [GlobalLookupCommitmentPurposeV1; 7] = [
    GlobalLookupCommitmentPurposeV1::SharedDifferenceInverse,
    GlobalLookupCommitmentPurposeV1::SharedSumInverse,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceInverse,
    GlobalLookupCommitmentPurposeV1::SmallPositiveInverse,
    GlobalLookupCommitmentPurposeV1::SmallNegativeInverse,
    GlobalLookupCommitmentPurposeV1::QMaskDigitInverse,
    GlobalLookupCommitmentPurposeV1::QMaskComplementInverse,
];

pub(super) const ALL_POINT_PURPOSES_V1: [GlobalLookupCommitmentPurposeV1; 21] = [
    GlobalLookupCommitmentPurposeV1::Source,
    GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow,
    GlobalLookupCommitmentPurposeV1::ExistingSumLow,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop,
    GlobalLookupCommitmentPurposeV1::ComparatorSumTop,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit,
    GlobalLookupCommitmentPurposeV1::ComparatorBorrow,
    GlobalLookupCommitmentPurposeV1::ComparatorMixedTop,
    GlobalLookupCommitmentPurposeV1::SmallSigned,
    GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude,
    GlobalLookupCommitmentPurposeV1::QMaskDigit,
    GlobalLookupCommitmentPurposeV1::QMaskComplementDigit,
    GlobalLookupCommitmentPurposeV1::Multiplicity,
    GlobalLookupCommitmentPurposeV1::InverseProductMask,
    GlobalLookupCommitmentPurposeV1::SharedDifferenceInverse,
    GlobalLookupCommitmentPurposeV1::SharedSumInverse,
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceInverse,
    GlobalLookupCommitmentPurposeV1::SmallPositiveInverse,
    GlobalLookupCommitmentPurposeV1::SmallNegativeInverse,
    GlobalLookupCommitmentPurposeV1::QMaskDigitInverse,
    GlobalLookupCommitmentPurposeV1::QMaskComplementInverse,
];

pub(super) const COMPARATOR_SIGNED_POINT_PURPOSES_V1: [GlobalLookupCommitmentPurposeV1; 6] = [
    GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop,
    GlobalLookupCommitmentPurposeV1::ComparatorSumTop,
    GlobalLookupCommitmentPurposeV1::ComparatorBorrow,
    GlobalLookupCommitmentPurposeV1::ComparatorMixedTop,
    GlobalLookupCommitmentPurposeV1::SmallSigned,
    GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude,
];

pub(super) fn comparator_signed_coordinate_v1(
    logical_ordinal: u32,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let mut first = 0_u32;
    for purpose in COMPARATOR_SIGNED_POINT_PURPOSES_V1 {
        let count = purpose.count_v1() as u32;
        let end = first
            .checked_add(count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if (first..end).contains(&logical_ordinal) {
            return commitment_coordinate_v1(
                first_commitment_ordinal_v1(purpose) + logical_ordinal - first,
            );
        }
        first = end;
    }
    Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GlobalLookupCommitmentCoordinateV1 {
    pub(super) global_ordinal: u32,
    pub(super) phase: GlobalLookupCommitmentPhaseV1,
    pub(super) purpose: GlobalLookupCommitmentPurposeV1,
    pub(super) purpose_ordinal: u32,
}

const fn inventory_count_v1() -> u32 {
    let mut total = 0;
    let mut index = 0;
    while index < ALL_POINT_PURPOSES_V1.len() {
        total += ALL_POINT_PURPOSES_V1[index].count_v1() as u32;
        index += 1;
    }
    total
}

pub(super) const GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1: u32 = inventory_count_v1();

pub(super) fn commitment_coordinate_v1(
    global_ordinal: u32,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let mut first = 0_u32;
    for purpose in ALL_POINT_PURPOSES_V1 {
        let count = purpose.count_v1() as u32;
        let end = first
            .checked_add(count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if (first..end).contains(&global_ordinal) {
            return Ok(GlobalLookupCommitmentCoordinateV1 {
                global_ordinal,
                phase: purpose.phase_v1(),
                purpose,
                purpose_ordinal: global_ordinal - first,
            });
        }
        first = end;
    }
    Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

pub(super) fn first_commitment_ordinal_v1(purpose: GlobalLookupCommitmentPurposeV1) -> u32 {
    let mut first = 0;
    for current in ALL_POINT_PURPOSES_V1 {
        if current == purpose {
            return first;
        }
        first += current.count_v1() as u32;
    }
    unreachable!("closed commitment purpose inventory")
}

/// Source-coupled identity of the exact current inventory, without a second transcript.
pub(super) fn global_lookup_topology_digest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TOPOLOGY_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for value in [
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u32,
        GROUPS_V1,
        LOW_DIGITS_V1,
        BORROWS_V1,
        SMALL_BLOCKS_V1,
        Q_MASK_PER_ROLE_V1,
        GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1,
    ] {
        hash.update(&value.to_be_bytes());
    }
    let mut first = 0_u32;
    for purpose in ALL_POINT_PURPOSES_V1 {
        hash.update(&[purpose.phase_v1() as u8, purpose as u8]);
        hash.update(&first.to_be_bytes());
        hash.update(&(purpose.count_v1() as u32).to_be_bytes());
        first += purpose.count_v1() as u32;
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.update(&(INVENTORY_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(INVENTORY_LANGUAGE_V1);
    hash.finalize()
}

#[cfg(test)]
#[path = "global_lookup_statement_v1_tests.rs"]
mod tests;
