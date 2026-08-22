//! Sole-`z` pre/post commitment rendezvous for the 40-limb global lookup.
//!
//! This private stage consumes statement 4, authenticates the exact
//! challenge-independent commitment roles (including distinct global-lookup
//! and inverse-product mask commitments), and derives the only lookup
//! challenge before it touches any inverse commitment.  It then exact-decodes
//! the 11,696 shared existing-radix inverse commitments and aliases the 20,712
//! added inverse commitments already owned by the cross-field inventory.
//!
//! No inverse product, range, lookup, cross-field, readiness, release, or
//! authorization claim is made here.  In particular, all predecessor proof,
//! residual, binding, codec, and full-inventory digests are excluded from the
//! pre-`z` transcript. The sole admitted terminal chronology input is an
//! opaque domain-separated commitment to the exact post-cross binding and
//! global challenge seed. The statement-4 residual and binding enter only
//! after the complete post-`z` frame and codec have been checked.

use super::{
    rns_native_centering_subtraction_relation::{
        RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1,
        RnsNativeCenteringSubtractionPrerequisiteV1,
    },
    rns_native_profile::{ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1},
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_transcript::ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
};
use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZeroizingT256ScalarCopyV1},
    sponge::Keccak256,
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZGZ1";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const GROUPS_V1: usize = 344;
const LOW_DIGITS_V1: usize = 17;
const BORROWS_V1: usize = 18;
const REPETITIONS_V1: usize = 5;
const BLOCKS_PER_Q_MASK_CORE_V1: usize = 8;
const SMALL_BLOCKS_V1: usize = 1_032;
const Q_MASK_BLOCKS_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1 * BLOCKS_PER_Q_MASK_CORE_V1;
const Q_MASK_DIGITS_V1: usize = 4;
const SOURCE_COMMITMENTS_V1: usize = 344;
const PRE_Z_SCALAR_COMMITMENTS_V1: usize = 3;
const PRE_Z_POINT_ROLE_COUNT_V1: usize = 14;
const EXISTING_LOW_PER_ROLE_V1: usize = GROUPS_V1 * LOW_DIGITS_V1;
const Q_MASK_PER_ROLE_V1: usize = Q_MASK_BLOCKS_V1 * Q_MASK_DIGITS_V1;
const PRE_Z_PHYSICAL_COMMITMENTS_V1: usize = 3 * EXISTING_LOW_PER_ROLE_V1
    + 3 * GROUPS_V1
    + GROUPS_V1 * BORROWS_V1
    + 2 * SMALL_BLOCKS_V1
    + 2 * Q_MASK_PER_ROLE_V1
    + PRE_Z_SCALAR_COMMITMENTS_V1;
const EXISTING_INVERSE_POINTS_V1: usize = 2 * EXISTING_LOW_PER_ROLE_V1;
const COMPARATOR_INVERSE_POINTS_V1: usize = EXISTING_LOW_PER_ROLE_V1;
const SMALL_INVERSE_POINTS_V1: usize = 2 * SMALL_BLOCKS_V1;
const Q_MASK_INVERSE_POINTS_V1: usize = 2 * Q_MASK_PER_ROLE_V1;
const ADDED_INVERSE_POINTS_V1: usize =
    COMPARATOR_INVERSE_POINTS_V1 + SMALL_INVERSE_POINTS_V1 + Q_MASK_INVERSE_POINTS_V1;
const GLOBAL_INVERSE_POINTS_V1: usize = EXISTING_INVERSE_POINTS_V1 + ADDED_INVERSE_POINTS_V1;
const LOOKUP_TABLE_VALUES_V1: u32 = 1 << 15;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const Z_CHALLENGE_ORDINAL_V1: u8 = 0;

// Header fields are all fixed geometry or public lengths and are safe before
// z.  No digest of the current frame occurs in this prefix.
const HEADER_BYTES_V1: usize = 56;
const PRE_Z_POINT_BYTES_V1: usize = PRE_Z_SCALAR_COMMITMENTS_V1 * POINT_BYTES_V1;
const EXISTING_INVERSE_BYTES_V1: usize = EXISTING_INVERSE_POINTS_V1 * POINT_BYTES_V1;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize =
    HEADER_BYTES_V1 + PRE_Z_POINT_BYTES_V1 + EXISTING_INVERSE_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - PRE_Z_POINT_BYTES_V1
        - EXISTING_INVERSE_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

// The accepted T256 generalized-Bulletproof kernel has a fixed 65,536-gate
// basis.  One direct inverse relation uses one multiplication gate at each of
// 16,384 coordinates, so a core can cover exactly four physical planes.  The
// smallest complete physical role is either of the two 1,032-plane small-sign
// roles.  Its proof records alone exceed the remaining transport cap.  A
// fitting prefix would complete no role and would strand the global proof, so
// this stage deliberately emits no partial product token.
const INVERSE_PRODUCT_COORDINATES_V1: usize = 16_384;
const INVERSE_PRODUCT_PLANES_PER_CORE_V1: usize = 4;
const INVERSE_PRODUCT_GATES_PER_CORE_V1: usize =
    INVERSE_PRODUCT_COORDINATES_V1 * INVERSE_PRODUCT_PLANES_PER_CORE_V1;
const INVERSE_PRODUCT_LOG_PADDED_GATES_V1: usize = 16;
const INVERSE_PRODUCT_COMMITMENTS_PER_PLANE_V1: usize = 2;
const INVERSE_PRODUCT_COMMITMENTS_PER_CORE_V1: usize =
    INVERSE_PRODUCT_PLANES_PER_CORE_V1 * INVERSE_PRODUCT_COMMITMENTS_PER_PLANE_V1;
const INVERSE_PRODUCT_FIXED_CORE_POINTS_V1: usize = 2 * INVERSE_PRODUCT_COMMITMENTS_PER_CORE_V1 + 9;
const INVERSE_PRODUCT_IPA_POINTS_V1: usize = 2 * INVERSE_PRODUCT_LOG_PADDED_GATES_V1;
const INVERSE_PRODUCT_CORE_POINTS_V1: usize =
    INVERSE_PRODUCT_FIXED_CORE_POINTS_V1 + INVERSE_PRODUCT_IPA_POINTS_V1;
const INVERSE_PRODUCT_CORE_SCALARS_V1: usize = 5;
const INVERSE_PRODUCT_CORE_BYTES_V1: usize =
    INVERSE_PRODUCT_CORE_POINTS_V1 * POINT_BYTES_V1 + INVERSE_PRODUCT_CORE_SCALARS_V1 * 32;
const INVERSE_PRODUCT_RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const INVERSE_PRODUCT_RECORD_BYTES_V1: usize =
    INVERSE_PRODUCT_RECORD_HEADER_BYTES_V1 + INVERSE_PRODUCT_CORE_BYTES_V1;
const SMALLEST_COMPLETE_ROLE_PLANES_V1: usize = SMALL_BLOCKS_V1;
const SMALLEST_COMPLETE_ROLE_CORES_V1: usize =
    SMALLEST_COMPLETE_ROLE_PLANES_V1 / INVERSE_PRODUCT_PLANES_PER_CORE_V1;
const SMALLEST_COMPLETE_ROLE_RECORD_BYTES_V1: usize =
    SMALLEST_COMPLETE_ROLE_CORES_V1 * INVERSE_PRODUCT_RECORD_BYTES_V1;
const MIN_NONEMPTY_RESIDUAL_BYTES_V1: usize = 1;
const SMALLEST_COMPLETE_ROLE_MIN_BYTES_V1: usize =
    SMALLEST_COMPLETE_ROLE_RECORD_BYTES_V1 + CODEC_DIGEST_BYTES_V1 + MIN_NONEMPTY_RESIDUAL_BYTES_V1;
const SMALLEST_COMPLETE_ROLE_CAP_EXCESS_V1: usize =
    SMALLEST_COMPLETE_ROLE_MIN_BYTES_V1 - RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1;
const MAX_FITTING_PARTIAL_CORES_V1: usize =
    RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1 / INVERSE_PRODUCT_RECORD_BYTES_V1;
const MAX_FITTING_PARTIAL_PLANES_V1: usize =
    MAX_FITTING_PARTIAL_CORES_V1 * INVERSE_PRODUCT_PLANES_PER_CORE_V1;
const BOTH_SMALL_ROLES_MIN_BYTES_V1: usize = 2 * SMALLEST_COMPLETE_ROLE_RECORD_BYTES_V1
    + CODEC_DIGEST_BYTES_V1
    + MIN_NONEMPTY_RESIDUAL_BYTES_V1;
const ALL_INVERSE_PRODUCT_CORES_V1: usize =
    GLOBAL_INVERSE_POINTS_V1 / INVERSE_PRODUCT_PLANES_PER_CORE_V1;
const ALL_INVERSE_PRODUCT_RECORD_BYTES_V1: usize =
    ALL_INVERSE_PRODUCT_CORES_V1 * INVERSE_PRODUCT_RECORD_BYTES_V1;
const ALL_INVERSE_PRODUCT_MIN_BYTES_V1: usize =
    ALL_INVERSE_PRODUCT_RECORD_BYTES_V1 + CODEC_DIGEST_BYTES_V1 + MIN_NONEMPTY_RESIDUAL_BYTES_V1;
const ALL_INVERSE_PRODUCT_CAP_EXCESS_V1: usize =
    ALL_INVERSE_PRODUCT_MIN_BYTES_V1 - RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1;

const FIXED_AXES_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.fixed-axes";
const SOURCE_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.safe-source";
const QPCS_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.safe-qpcs";
const ROLE_ROOT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.pre-z-role";
const PRE_Z_INVENTORY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.pre-z-inventory";
const PRE_Z_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.pre-z-transcript";
const CLAIMED_PRE_GLOBAL_LABEL_V1: &[u8] = b"claimed-pre-global";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.challenge";
const POST_Z_ROLE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.post-z-role";
const ALIAS_ROOT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.alias-root";
const GLOBAL_INVERSE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.global-inverse-root";
const POST_Z_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.post-z-transcript";
const RESIDUAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.post-z-residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.codec";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-lookup.post-z-prerequisite";
const PRE_Z_ORDER_V1: &[u8] = b"physical-points-only:D-low[5848],S-low[5848],D-top[344],S-top[344],Delta[5848],beta[6192],m[344],small-signed[1032],small-negative[1032],q-digit[column-major,6400],q-complement[column-major,6400],multiplicity[1],retired-global-lookup-sumcheck-mask[1;n=1024;702-scalars;retained-not-consumed-by-direct-membership],inverse-product-mask[1;n=16384;87-scalars]";
const POST_Z_ORDER_V1: &[u8] = b"D-inverse[5848],S-inverse[5848],Delta-inverse[5848],small-positive-inverse[1032],small-negative-inverse[1032],q-digit-inverse[column-major,6400],q-complement-inverse[column-major,6400]";
const SAFE_AXIS_LANGUAGE_V1: &[u8] = b"pre-z-only=opaque-domain-separated-exact-post-cross-binding-plus-global-seed-commitment,fixed-manifest,source-layout-and-receipt,source-formula-and-mapping,source-opening-bundle,qpcs-fixed-parameters,qpcs-canonical-evaluations,and-role-separated-challenge-independent-point-roots;claimed-pre-global-commitment-precedes-all-local-safe-axes;excluded=prior-context,full-added-inventory-root,all-other-S3/S5/S8/S10-11/S2/S4-proof-and-transcript-roots,residuals,bindings,codec-digests,and-all-inverse-points";
const SOURCE_POINT_ACCOUNTING_LANGUAGE_V1: &[u8] = b"source-snapshot-commitments=344;bound-by-direct-snapshot/source-binding-digest;not-reencoded-as-pre-z-physical-point-roles;pre-z-physical-role-total=39635";
const DIRECT_MEMBERSHIP_MASK_STATUS_V1: &[u8] = b"legacy-702-scalar-global-lookup-sumcheck-mask=is-pre-z-authenticated-and-retained-but-retired;direct-membership-consumes-no-legacy-sumcheck-mask;inverse-product-mask-remains-distinct-and-consumed-only-by-compact-inverse";
const REMAINING_BOUNDARY_V1: &[u8] = b"not-yet-verified:(z-A)*U=1,all-digit-membership,integer-no-wrap,canonical-q-mask,source-and-packing-same-opening,cross-field-q-relation,global-sumcheck,global-lookup,readiness,release";
const PRODUCT_CAP_BLOCKER_LANGUAGE_V1: &[u8] = b"direct-inverse-product=(z-A[p,v])*U[p,v]=1;accepted-T256-GBP=four-planes-per-65536-gate-core,2045-byte-canonical-record;smallest-role=1032-planes,258-records,527643-byte-envelope-free-minimum;available=114484;forbidden=partial-role-token-or-unaudited-rho-kappa-sumcheck-aggregation;required=separately-audited-succinct-streaming-product-kernel-or-larger-canonical-transport";

const SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1: bool = true;
const POST_Z_INVERSE_COMMITMENT_VIEW_AUTHENTICATED_V1: bool = true;
const LEGACY_GLOBAL_LOOKUP_SUMCHECK_MASK_RETIRED_V1: bool = true;
const INVERSE_PRODUCT_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(GROUPS_V1 == 43 * 8);
    assert!(EXISTING_LOW_PER_ROLE_V1 == 5_848);
    assert!(Q_MASK_BLOCKS_V1 == 1_600);
    assert!(Q_MASK_PER_ROLE_V1 == 6_400);
    assert!(PRE_Z_PHYSICAL_COMMITMENTS_V1 == 39_635);
    assert!(EXISTING_INVERSE_POINTS_V1 == 11_696);
    assert!(COMPARATOR_INVERSE_POINTS_V1 == 5_848);
    assert!(SMALL_INVERSE_POINTS_V1 == 2_064);
    assert!(Q_MASK_INVERSE_POINTS_V1 == 12_800);
    assert!(ADDED_INVERSE_POINTS_V1 == 20_712);
    assert!(GLOBAL_INVERSE_POINTS_V1 == 32_408);
    assert!(EXISTING_INVERSE_BYTES_V1 == 385_968);
    assert!(HEADER_BYTES_V1 == 56);
    assert!(MIN_WIRE_BYTES_V1 == 386_156);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1 == 114_484);
    assert!(INVERSE_PRODUCT_GATES_PER_CORE_V1 == 65_536);
    assert!(INVERSE_PRODUCT_COMMITMENTS_PER_CORE_V1 == 8);
    assert!(INVERSE_PRODUCT_FIXED_CORE_POINTS_V1 == 25);
    assert!(INVERSE_PRODUCT_IPA_POINTS_V1 == 32);
    assert!(INVERSE_PRODUCT_CORE_POINTS_V1 == 57);
    assert!(INVERSE_PRODUCT_CORE_BYTES_V1 == 2_041);
    assert!(INVERSE_PRODUCT_RECORD_BYTES_V1 == 2_045);
    assert!(SMALLEST_COMPLETE_ROLE_PLANES_V1 == 1_032);
    assert!(SMALLEST_COMPLETE_ROLE_PLANES_V1.is_multiple_of(INVERSE_PRODUCT_PLANES_PER_CORE_V1));
    assert!(SMALLEST_COMPLETE_ROLE_CORES_V1 == 258);
    assert!(SMALLEST_COMPLETE_ROLE_RECORD_BYTES_V1 == 527_610);
    assert!(SMALLEST_COMPLETE_ROLE_MIN_BYTES_V1 == 527_643);
    assert!(SMALLEST_COMPLETE_ROLE_CAP_EXCESS_V1 == 413_159);
    assert!(MAX_FITTING_PARTIAL_CORES_V1 == 55);
    assert!(MAX_FITTING_PARTIAL_PLANES_V1 == 220);
    assert!(MAX_FITTING_PARTIAL_PLANES_V1 < SMALLEST_COMPLETE_ROLE_PLANES_V1);
    assert!(BOTH_SMALL_ROLES_MIN_BYTES_V1 == 1_055_253);
    assert!(GLOBAL_INVERSE_POINTS_V1.is_multiple_of(INVERSE_PRODUCT_PLANES_PER_CORE_V1));
    assert!(ALL_INVERSE_PRODUCT_CORES_V1 == 8_102);
    assert!(ALL_INVERSE_PRODUCT_RECORD_BYTES_V1 == 16_568_590);
    assert!(ALL_INVERSE_PRODUCT_MIN_BYTES_V1 == 16_568_623);
    assert!(ALL_INVERSE_PRODUCT_CAP_EXCESS_V1 == 16_454_139);
    assert!(SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1);
    assert!(POST_Z_INVERSE_COMMITMENT_VIEW_AUTHENTICATED_V1);
    assert!(LEGACY_GLOBAL_LOOKUP_SUMCHECK_MASK_RETIRED_V1);
    assert!(!INVERSE_PRODUCT_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeGlobalLookupZCommitmentViewErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidPoint,
    InvalidContext,
    InvalidIntegrity,
    ChallengeExhausted,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeGlobalLookupZCommitmentViewErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeGlobalLookupZCommitmentViewErrorV1 {}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalPurposeV1 {
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
    GlobalLookupSumcheckMask = 14,
    SharedDifferenceInverse = 15,
    SharedSumInverse = 16,
    ComparatorDifferenceInverse = 17,
    SmallPositiveInverse = 18,
    SmallNegativeInverse = 19,
    QMaskDigitInverse = 20,
    QMaskComplementInverse = 21,
    InverseProductMask = 22,
}

impl PhysicalPurposeV1 {
    const fn count_v1(self) -> usize {
        match self {
            Self::ExistingDifferenceLow
            | Self::ExistingSumLow
            | Self::ComparatorDifferenceDigit
            | Self::SharedDifferenceInverse
            | Self::SharedSumInverse
            | Self::ComparatorDifferenceInverse => EXISTING_LOW_PER_ROLE_V1,
            Self::ComparatorDifferenceTop | Self::ComparatorSumTop | Self::ComparatorMixedTop => {
                GROUPS_V1
            }
            Self::ComparatorBorrow => GROUPS_V1 * BORROWS_V1,
            Self::SmallSigned
            | Self::SmallNegativeMagnitude
            | Self::SmallPositiveInverse
            | Self::SmallNegativeInverse => SMALL_BLOCKS_V1,
            Self::QMaskDigit
            | Self::QMaskComplementDigit
            | Self::QMaskDigitInverse
            | Self::QMaskComplementInverse => Q_MASK_PER_ROLE_V1,
            Self::Multiplicity | Self::GlobalLookupSumcheckMask | Self::InverseProductMask => 1,
        }
    }
}

const PRE_Z_POINT_PURPOSES_V1: [PhysicalPurposeV1; PRE_Z_POINT_ROLE_COUNT_V1] = [
    PhysicalPurposeV1::ExistingDifferenceLow,
    PhysicalPurposeV1::ExistingSumLow,
    PhysicalPurposeV1::ComparatorDifferenceTop,
    PhysicalPurposeV1::ComparatorSumTop,
    PhysicalPurposeV1::ComparatorDifferenceDigit,
    PhysicalPurposeV1::ComparatorBorrow,
    PhysicalPurposeV1::ComparatorMixedTop,
    PhysicalPurposeV1::SmallSigned,
    PhysicalPurposeV1::SmallNegativeMagnitude,
    PhysicalPurposeV1::QMaskDigit,
    PhysicalPurposeV1::QMaskComplementDigit,
    PhysicalPurposeV1::Multiplicity,
    PhysicalPurposeV1::GlobalLookupSumcheckMask,
    PhysicalPurposeV1::InverseProductMask,
];

const POST_Z_POINT_PURPOSES_V1: [PhysicalPurposeV1; 7] = [
    PhysicalPurposeV1::SharedDifferenceInverse,
    PhysicalPurposeV1::SharedSumInverse,
    PhysicalPurposeV1::ComparatorDifferenceInverse,
    PhysicalPurposeV1::SmallPositiveInverse,
    PhysicalPurposeV1::SmallNegativeInverse,
    PhysicalPurposeV1::QMaskDigitInverse,
    PhysicalPurposeV1::QMaskComplementInverse,
];

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)?;
    Ok(encoded)
}

struct RoleRootBuilderV1 {
    expected: usize,
    absorbed: usize,
    hash: Keccak256,
}

impl RoleRootBuilderV1 {
    fn new_v1(purpose: PhysicalPurposeV1) -> Self {
        let expected = purpose.count_v1();
        let mut hash = Keccak256::new();
        hash.update(ROLE_ROOT_DOMAIN_V1);
        hash.update(&[VERSION_V1, purpose as u8]);
        hash.update(&(expected as u32).to_be_bytes());
        Self {
            expected,
            absorbed: 0,
            hash,
        }
    }

    fn absorb_v1(
        &mut self,
        point: Point,
    ) -> Result<(), RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        if self.absorbed >= self.expected {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry);
        }
        self.hash.update(&(self.absorbed as u32).to_be_bytes());
        self.hash.update(&encode_point_v1(point)?);
        self.absorbed += 1;
        Ok(())
    }

    fn finish_v1(
        self,
    ) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        if self.absorbed != self.expected {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry);
        }
        let digest = self.hash.finalize();
        if digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
        }
        Ok(digest)
    }
}

#[derive(Clone, Copy)]
struct PreZRoleRootsV1 {
    roots: [[u8; DIGEST_BYTES_V1]; PRE_Z_POINT_PURPOSES_V1.len()],
}

impl PreZRoleRootsV1 {
    fn validate_v1(self) -> Result<(), RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        unique_nonzero_digests_v1(&self.roots)
    }
}

#[derive(Clone, Copy)]
struct PreZSafeContextV1 {
    fixed_axes_digest: [u8; DIGEST_BYTES_V1],
    source_binding_digest: [u8; DIGEST_BYTES_V1],
    qpcs_binding_digest: [u8; DIGEST_BYTES_V1],
}

impl PreZSafeContextV1 {
    fn validate_v1(self) -> Result<(), RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        unique_nonzero_digests_v1(&[
            self.fixed_axes_digest,
            self.source_binding_digest,
            self.qpcs_binding_digest,
        ])
    }
}

fn unique_nonzero_digests_v1(
    digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<(), RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    for (ordinal, digest) in digests.iter().enumerate() {
        if *digest == [0; DIGEST_BYTES_V1] || digests[..ordinal].contains(digest) {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext);
        }
    }
    Ok(())
}

fn fixed_axes_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(FIXED_AXES_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for value in [
        GROUPS_V1,
        LOW_DIGITS_V1,
        BORROWS_V1,
        SMALL_BLOCKS_V1,
        Q_MASK_BLOCKS_V1,
        Q_MASK_DIGITS_V1,
        SOURCE_COMMITMENTS_V1,
        PRE_Z_PHYSICAL_COMMITMENTS_V1,
        EXISTING_INVERSE_POINTS_V1,
        ADDED_INVERSE_POINTS_V1,
        GLOBAL_INVERSE_POINTS_V1,
        LOOKUP_TABLE_VALUES_V1 as usize,
    ] {
        hash.update(&(value as u32).to_be_bytes());
    }
    for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
        hash.update(&modulus.to_be_bytes());
    }
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [
        PRE_Z_ORDER_V1,
        POST_Z_ORDER_V1,
        SAFE_AXIS_LANGUAGE_V1,
        SOURCE_POINT_ACCOUNTING_LANGUAGE_V1,
        DIRECT_MEMBERSHIP_MASK_STATUS_V1,
        REMAINING_BOUNDARY_V1,
        PRODUCT_CAP_BLOCKER_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.finalize()
}

fn source_binding_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeCenteringSubtractionPrerequisiteV1<'_, '_, S>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let existing = previous.previous().previous();
    let inventory = existing
        .previous()
        .previous()
        .previous()
        .previous()
        .inventory();
    let linked = inventory.linked();
    let source = linked.source();
    let layout = source.snapshot().layout();
    layout
        .validate()
        .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
    let receipt = source
        .snapshot()
        .structural_receipt()
        .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
    let mut hash = Keccak256::new();
    hash.update(SOURCE_BINDING_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for digest in [
        layout.profile_digest(),
        layout.topology_digest(),
        layout.release_candidate_digest(),
        layout.statement_digest(),
        layout.operational_context_digest(),
        layout.source_binding_digest(),
        receipt.main_snapshot_digest,
        receipt.nonce_snapshot_digest,
        receipt.receipt_digest,
        source.public_key_digest(),
        source.public_bundle_digest(),
        source.formula_digest(),
        source.mapping_digest(),
        linked.opening_bundle_digest(),
    ] {
        if digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext);
        }
        hash.update(&digest);
    }
    hash.update(&(SOURCE_COMMITMENTS_V1 as u32).to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn qpcs_binding_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeCenteringSubtractionPrerequisiteV1<'_, '_, S>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let existing = previous.previous().previous();
    let inventory = existing
        .previous()
        .previous()
        .previous()
        .previous()
        .inventory();
    let parameter_digest = inventory.linked().source().qpcs().parameter_digest();
    if parameter_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(QPCS_BINDING_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&parameter_digest);
    for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
        for repetition in 0..REPETITIONS_V1 {
            let (product, quotient) = inventory
                .qpcs_evaluation(limb, repetition)
                .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
            if product >= modulus || quotient >= modulus {
                return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext);
            }
            hash.update(&[limb as u8, repetition as u8]);
            hash.update(&modulus.to_be_bytes());
            hash.update(&product.to_be_bytes());
            hash.update(&quotient.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn pre_z_safe_context_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeCenteringSubtractionPrerequisiteV1<'_, '_, S>,
) -> Result<PreZSafeContextV1, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let context = PreZSafeContextV1 {
        fixed_axes_digest: fixed_axes_digest_v1(),
        source_binding_digest: source_binding_digest_v1(previous)?,
        qpcs_binding_digest: qpcs_binding_digest_v1(previous)?,
    };
    context.validate_v1()?;
    Ok(context)
}

fn pre_z_role_roots_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeCenteringSubtractionPrerequisiteV1<'_, '_, S>,
    multiplicity: Point,
    sumcheck_mask: Point,
    inverse_product_mask: Point,
) -> Result<PreZRoleRootsV1, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let existing = previous.previous().previous();
    let q_mask = existing.previous();
    let inventory = q_mask.previous().previous().previous().inventory();
    let mut builders: [RoleRootBuilderV1; PRE_Z_POINT_PURPOSES_V1.len()] =
        core::array::from_fn(|index| RoleRootBuilderV1::new_v1(PRE_Z_POINT_PURPOSES_V1[index]));

    for group in 0..GROUPS_V1 {
        let radix = existing
            .existing_radix_commitments(group)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
        for point in radix.difference_low {
            builders[0].absorb_v1(point)?;
        }
        for point in radix.slack_low {
            builders[1].absorb_v1(point)?;
        }
        builders[2].absorb_v1(radix.difference_top)?;
        builders[3].absorb_v1(radix.slack_top)?;

        let subtraction = inventory
            .comparator_subtraction_commitments(group)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
        let range = inventory
            .comparator_range_carry_commitments(group)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
        for column in 0..LOW_DIGITS_V1 {
            if subtraction.borrows[column] != range.borrows[column] {
                return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext);
            }
            builders[4].absorb_v1(subtraction.difference_digits[column])?;
        }
        for point in range.borrows {
            builders[5].absorb_v1(point)?;
        }
        builders[6].absorb_v1(range.mixed_top)?;
    }

    for block in 0..SMALL_BLOCKS_V1 {
        let commitments = inventory
            .small_source_product_commitments(block)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
        builders[7].absorb_v1(commitments.signed)?;
    }
    for block in 0..SMALL_BLOCKS_V1 {
        let commitments = inventory
            .small_source_product_commitments(block)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
        builders[8].absorb_v1(commitments.negative_magnitude)?;
    }
    for column in 0..Q_MASK_DIGITS_V1 {
        for owner in 0..Q_MASK_BLOCKS_V1 {
            let commitments = inventory
                .q_mask_linear_commitments(owner)
                .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
            builders[9].absorb_v1(commitments.digits[column])?;
        }
    }
    for column in 0..Q_MASK_DIGITS_V1 {
        for owner in 0..Q_MASK_BLOCKS_V1 {
            let commitments = inventory
                .q_mask_linear_commitments(owner)
                .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
            builders[10].absorb_v1(commitments.complement_digits[column])?;
        }
    }
    builders[11].absorb_v1(multiplicity)?;
    builders[12].absorb_v1(sumcheck_mask)?;
    builders[13].absorb_v1(inverse_product_mask)?;

    let roots: [Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1>;
        PRE_Z_POINT_ROLE_COUNT_V1] = core::array::from_fn(|index| {
        // `RoleRootBuilderV1` is not Copy; replace each completed builder with
        // a fresh empty owner and consume the original.
        let purpose = PRE_Z_POINT_PURPOSES_V1[index];
        let builder = core::mem::replace(&mut builders[index], RoleRootBuilderV1::new_v1(purpose));
        builder.finish_v1()
    });
    let mut output = [[0_u8; DIGEST_BYTES_V1]; PRE_Z_POINT_PURPOSES_V1.len()];
    for (index, result) in roots.into_iter().enumerate() {
        output[index] = result?;
    }
    let roots = PreZRoleRootsV1 { roots: output };
    roots.validate_v1()?;
    Ok(roots)
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct PreZEnvelopeViewV1<'a> {
    bytes: &'a [u8],
    multiplicity: Point,
    sumcheck_mask: Point,
    inverse_product_mask: Point,
    existing_inverse_bytes: &'a [u8],
    residual: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
    codec_offset: usize,
}

impl<'a> PreZEnvelopeViewV1<'a> {
    fn from_canonical_prefix_v1(
        bytes: &'a [u8],
    ) -> Result<Self, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
        if bytes.len() > RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::from(decoder.u16()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || usize::from(decoder.u8()?) != LOW_DIGITS_V1
            || usize::from(decoder.u8()?) != BORROWS_V1
            || usize::from(decoder.u8()?) != REPETITIONS_V1
            || usize::from(decoder.u8()?) != PRE_Z_SCALAR_COMMITMENTS_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || decoder.u8()? != Z_CHALLENGE_ORDINAL_V1
            || decoder.u8()? != MAX_CHALLENGE_ATTEMPTS_V1
            || usize::from(decoder.u8()?) != Q_MASK_DIGITS_V1
            || decoder.u32()? != LOOKUP_TABLE_VALUES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != SOURCE_COMMITMENTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != PRE_Z_PHYSICAL_COMMITMENTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != EXISTING_INVERSE_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != ADDED_INVERSE_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != GLOBAL_INVERSE_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
                != Q_MASK_BLOCKS_V1
        {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry);
        }
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || residual_len > RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1
        {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader);
        }
        let expected = HEADER_BYTES_V1
            .checked_add(PRE_Z_POINT_BYTES_V1)
            .and_then(|value| value.checked_add(EXISTING_INVERSE_BYTES_V1))
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?;
        if expected != bytes.len() {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader);
        }
        let multiplicity = Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
            .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)?;
        let sumcheck_mask =
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)?;
        let inverse_product_mask =
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)?;
        let existing_inverse_bytes = decoder.take(EXISTING_INVERSE_BYTES_V1)?;
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidHeader);
        }
        Ok(Self {
            bytes,
            multiplicity,
            sumcheck_mask,
            inverse_product_mask,
            existing_inverse_bytes,
            residual,
            codec_digest,
            codec_offset,
        })
    }
}

fn append_frame_v1(
    state: &mut Keccak256,
    label: &[u8],
    value: &[u8],
) -> Result<(), RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    state.update(
        &u16::try_from(label.len())
            .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.update(label);
    state.update(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.update(value);
    Ok(())
}

fn pre_z_inventory_digest_v1(
    source_binding_digest: [u8; DIGEST_BYTES_V1],
    roots: PreZRoleRootsV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    roots.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(PRE_Z_INVENTORY_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&(SOURCE_COMMITMENTS_V1 as u32).to_be_bytes());
    hash.update(&source_binding_digest);
    for (purpose, root) in PRE_Z_POINT_PURPOSES_V1.into_iter().zip(roots.roots) {
        hash.update(&[purpose as u8]);
        hash.update(&(purpose.count_v1() as u32).to_be_bytes());
        hash.update(&root);
    }
    hash.update(&(PRE_Z_PHYSICAL_COMMITMENTS_V1 as u32).to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

struct PreZChallengeLiveV1 {
    state: Keccak256,
    z: ZeroizingT256ScalarCopyV1,
    pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
}

fn challenge_outside_table_v1(challenge: Scalar) -> bool {
    let bytes = challenge.to_le_bytes();
    bytes[2..].iter().any(|byte| *byte != 0)
        || u16::from_le_bytes([bytes[0], bytes[1]]) >= LOOKUP_TABLE_VALUES_V1 as u16
}

fn derive_global_z_v1(
    pre_global_capability: &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
    context: PreZSafeContextV1,
    roots: PreZRoleRootsV1,
) -> Result<PreZChallengeLiveV1, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    context.validate_v1()?;
    roots.validate_v1()?;
    let inventory_digest = pre_z_inventory_digest_v1(context.source_binding_digest, roots)?;
    let claimed_pre_global_digest = pre_global_capability
        .sole_z_binding_digest_v1()
        .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?;
    let mut state = Keccak256::new();
    state.update(PRE_Z_TRANSCRIPT_DOMAIN_V1);
    state.update(&[VERSION_V1]);
    append_frame_v1(
        &mut state,
        CLAIMED_PRE_GLOBAL_LABEL_V1,
        &claimed_pre_global_digest,
    )?;
    for (label, digest) in [
        (b"fixed-axes".as_slice(), context.fixed_axes_digest),
        (b"source".as_slice(), context.source_binding_digest),
        (b"qpcs".as_slice(), context.qpcs_binding_digest),
        (b"pre-z-inventory".as_slice(), inventory_digest),
    ] {
        append_frame_v1(&mut state, label, &digest)?;
    }
    for (purpose, root) in PRE_Z_POINT_PURPOSES_V1.into_iter().zip(roots.roots) {
        append_frame_v1(&mut state, &[purpose as u8], &root)?;
    }
    let pre_z_binding_digest = state.fork_v1().finalize();
    if pre_z_binding_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut fork = state.fork_v1();
            fork.update(CHALLENGE_DOMAIN_V1);
            fork.update(&[Z_CHALLENGE_ORDINAL_V1, attempt, branch]);
            let start = usize::from(branch) * DIGEST_BYTES_V1;
            wide[start..start + DIGEST_BYTES_V1].copy_from_slice(&fork.finalize());
        }
        let mut challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if challenge_outside_table_v1(challenge) {
            append_frame_v1(&mut state, b"z-ordinal", &[Z_CHALLENGE_ORDINAL_V1])?;
            append_frame_v1(&mut state, b"z-attempt", &[attempt])?;
            append_frame_v1(&mut state, b"z", &challenge.to_le_bytes())?;
            return Ok(PreZChallengeLiveV1 {
                state,
                z: ZeroizingT256ScalarCopyV1::take(&mut challenge),
                pre_z_binding_digest,
            });
        }
        challenge.clear_secret();
    }
    Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::ChallengeExhausted)
}

#[derive(Clone, Copy)]
struct PostZRootsV1 {
    roots: [[u8; DIGEST_BYTES_V1]; POST_Z_POINT_PURPOSES_V1.len()],
    existing_root: [u8; DIGEST_BYTES_V1],
    added_root: [u8; DIGEST_BYTES_V1],
    alias_root: [u8; DIGEST_BYTES_V1],
    global_root: [u8; DIGEST_BYTES_V1],
}

fn point_from_existing_inverse_v1(
    bytes: &[u8],
    ordinal: usize,
) -> Result<Point, RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    if bytes.len() != EXISTING_INVERSE_BYTES_V1 || ordinal >= EXISTING_INVERSE_POINTS_V1 {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry);
    }
    let offset = ordinal
        .checked_mul(POINT_BYTES_V1)
        .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::ArithmeticOverflow)?;
    Point::from_non_identity_wire_bytes_exact(
        bytes
            .get(offset..offset + POINT_BYTES_V1)
            .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry)?,
    )
    .map_err(|_| RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)
}

fn finish_combined_root_v1(
    domain: &[u8],
    roles: &[(PhysicalPurposeV1, [u8; DIGEST_BYTES_V1])],
    total: usize,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[VERSION_V1]);
    for (purpose, root) in roles {
        hash.update(&[*purpose as u8]);
        hash.update(&(purpose.count_v1() as u32).to_be_bytes());
        hash.update(root);
    }
    hash.update(&(total as u32).to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn post_z_roots_v1<F>(
    existing_inverse_bytes: &[u8],
    mut added_at: F,
) -> Result<PostZRootsV1, RnsNativeGlobalLookupZCommitmentViewErrorV1>
where
    F: FnMut(PhysicalPurposeV1, usize) -> Option<Point>,
{
    let mut builders: [RoleRootBuilderV1; POST_Z_POINT_PURPOSES_V1.len()] =
        core::array::from_fn(|index| RoleRootBuilderV1 {
            expected: POST_Z_POINT_PURPOSES_V1[index].count_v1(),
            absorbed: 0,
            hash: {
                let purpose = POST_Z_POINT_PURPOSES_V1[index];
                let mut hash = Keccak256::new();
                hash.update(POST_Z_ROLE_ROOT_DOMAIN_V1);
                hash.update(&[VERSION_V1, purpose as u8]);
                hash.update(&(purpose.count_v1() as u32).to_be_bytes());
                hash
            },
        });
    for ordinal in 0..EXISTING_LOW_PER_ROLE_V1 {
        builders[0].absorb_v1(point_from_existing_inverse_v1(
            existing_inverse_bytes,
            ordinal,
        )?)?;
        builders[1].absorb_v1(point_from_existing_inverse_v1(
            existing_inverse_bytes,
            EXISTING_LOW_PER_ROLE_V1 + ordinal,
        )?)?;
    }
    for (builder, purpose) in builders
        .iter_mut()
        .skip(2)
        .zip(POST_Z_POINT_PURPOSES_V1[2..].iter())
    {
        for ordinal in 0..purpose.count_v1() {
            builder.absorb_v1(
                added_at(*purpose, ordinal)
                    .ok_or(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)?,
            )?;
        }
    }
    let mut roots = [[0_u8; DIGEST_BYTES_V1]; POST_Z_POINT_PURPOSES_V1.len()];
    for index in 0..roots.len() {
        let purpose = POST_Z_POINT_PURPOSES_V1[index];
        let builder = core::mem::replace(&mut builders[index], RoleRootBuilderV1::new_v1(purpose));
        roots[index] = builder.finish_v1()?;
    }
    unique_nonzero_digests_v1(&roots)?;
    let existing_roles = [
        (POST_Z_POINT_PURPOSES_V1[0], roots[0]),
        (POST_Z_POINT_PURPOSES_V1[1], roots[1]),
    ];
    let added_roles = [
        (POST_Z_POINT_PURPOSES_V1[2], roots[2]),
        (POST_Z_POINT_PURPOSES_V1[3], roots[3]),
        (POST_Z_POINT_PURPOSES_V1[4], roots[4]),
        (POST_Z_POINT_PURPOSES_V1[5], roots[5]),
        (POST_Z_POINT_PURPOSES_V1[6], roots[6]),
    ];
    let existing_root = finish_combined_root_v1(
        POST_Z_ROLE_ROOT_DOMAIN_V1,
        &existing_roles,
        EXISTING_INVERSE_POINTS_V1,
    )?;
    let added_root = finish_combined_root_v1(
        POST_Z_ROLE_ROOT_DOMAIN_V1,
        &added_roles,
        ADDED_INVERSE_POINTS_V1,
    )?;
    let mut alias = Keccak256::new();
    alias.update(ALIAS_ROOT_DOMAIN_V1);
    alias.update(&[VERSION_V1]);
    alias.update(&existing_root);
    alias.update(&(EXISTING_INVERSE_POINTS_V1 as u32).to_be_bytes());
    for ordinal in 0..EXISTING_INVERSE_POINTS_V1 {
        alias.update(&(ordinal as u32).to_be_bytes());
        alias.update(&(ordinal as u32).to_be_bytes());
    }
    let alias_root = alias.finalize();
    let all_roles = [
        existing_roles[0],
        existing_roles[1],
        added_roles[0],
        added_roles[1],
        added_roles[2],
        added_roles[3],
        added_roles[4],
    ];
    let global_root = finish_combined_root_v1(
        GLOBAL_INVERSE_ROOT_DOMAIN_V1,
        &all_roles,
        GLOBAL_INVERSE_POINTS_V1,
    )?;
    unique_nonzero_digests_v1(&[existing_root, added_root, alias_root, global_root])?;
    Ok(PostZRootsV1 {
        roots,
        existing_root,
        added_root,
        alias_root,
        global_root,
    })
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

struct PostZChallengeLiveV1 {
    // The inverse-product verifier must consume this transcript and the sole
    // z together.  Underscore names keep this intentionally opaque owner
    // warning-clean until that verifier lands; there is no scalar getter.
    _state: Keccak256,
    _z: ZeroizingT256ScalarCopyV1,
}

fn bind_post_z_v1<F>(
    mut live: PreZChallengeLiveV1,
    view: PreZEnvelopeViewV1<'_>,
    added_at: F,
) -> Result<
    ([u8; DIGEST_BYTES_V1], PostZRootsV1, PostZChallengeLiveV1),
    RnsNativeGlobalLookupZCommitmentViewErrorV1,
>
where
    F: FnMut(PhysicalPurposeV1, usize) -> Option<Point>,
{
    let roots = post_z_roots_v1(view.existing_inverse_bytes, added_at)?;
    if codec_digest_v1(&view.bytes[..view.codec_offset]) != view.codec_digest {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    live.state.update(POST_Z_TRANSCRIPT_DOMAIN_V1);
    for (label, root) in [
        (b"existing-inverses".as_slice(), roots.existing_root),
        (b"added-inverses".as_slice(), roots.added_root),
        (b"alias-map".as_slice(), roots.alias_root),
        (b"global-inverses".as_slice(), roots.global_root),
    ] {
        append_frame_v1(&mut live.state, label, &root)?;
    }
    for (purpose, root) in POST_Z_POINT_PURPOSES_V1.into_iter().zip(roots.roots) {
        append_frame_v1(&mut live.state, &[purpose as u8], &root)?;
    }
    let transcript_digest = live.state.fork_v1().finalize();
    if transcript_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok((
        transcript_digest,
        roots,
        PostZChallengeLiveV1 {
            _state: live.state,
            _z: live.z,
        },
    ))
}

fn residual_digest_v1(
    pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
    post_z_transcript_digest: [u8; DIGEST_BYTES_V1],
    roots: PostZRootsV1,
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalLookupZCommitmentViewErrorV1> {
    if residual.is_empty() {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for digest in [
        pre_z_binding_digest,
        post_z_transcript_digest,
        roots.existing_root,
        roots.added_root,
        roots.alias_root,
        roots.global_root,
    ] {
        hash.update(&digest);
    }
    hash.update(&(residual.len() as u32).to_be_bytes());
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only private owner after the unique lookup `z` has been derived but
/// before any inverse point has been decoded or absorbed.
#[allow(
    missing_copy_implementations,
    reason = "the sole-z owner and statement-4 predecessor must advance exactly once"
)]
pub(super) struct RnsNativeGlobalLookupPreZPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>,
    view: PreZEnvelopeViewV1<'proof>,
    live: PreZChallengeLiveV1,
}

/// Move-only private owner after every post-`z` inverse commitment has been
/// exact-decoded and bound.  It is not evidence that any inverse equation is
/// true.
#[allow(
    missing_copy_implementations,
    reason = "the post-z commitment owner and downstream proof must advance exactly once"
)]
pub(super) struct RnsNativeGlobalLookupPostZPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>,
    residual: &'proof [u8],
    existing_inverse_bytes: &'proof [u8],
    multiplicity: Point,
    // Authenticated before z and retained for chronology compatibility, but
    // direct membership retires this legacy 702-scalar mask and exposes no
    // consumer accessor.
    _retired_sumcheck_mask: Point,
    inverse_product_mask: Point,
    pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
    post_z_transcript_digest: [u8; DIGEST_BYTES_V1],
    existing_inverse_root: [u8; DIGEST_BYTES_V1],
    added_inverse_root: [u8; DIGEST_BYTES_V1],
    alias_root: [u8; DIGEST_BYTES_V1],
    global_inverse_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
    _live: PostZChallengeLiveV1,
}

#[allow(
    dead_code,
    reason = "private post-z bindings await the inverse-product and global-lookup verifier"
)]
impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S> {
        &self.previous
    }

    pub(super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    pub(super) const fn existing_inverse_bytes(&self) -> &'proof [u8] {
        self.existing_inverse_bytes
    }

    pub(super) const fn multiplicity(&self) -> Point {
        self.multiplicity
    }

    pub(super) const fn inverse_product_mask(&self) -> Point {
        self.inverse_product_mask
    }

    pub(super) fn z_challenge(&self) -> Scalar {
        self._live._z.get()
    }

    pub(super) const fn pre_z_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.pre_z_binding_digest
    }

    pub(super) const fn post_z_transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.post_z_transcript_digest
    }

    pub(super) const fn existing_inverse_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.existing_inverse_root
    }

    pub(super) const fn added_inverse_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.added_inverse_root
    }

    pub(super) const fn alias_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.alias_root
    }

    pub(super) const fn global_inverse_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.global_inverse_root
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }
}

#[allow(
    dead_code,
    reason = "the private sole-z entry awaits its immediate post-z inverse consumer"
)]
pub(super) fn derive_rns_native_global_lookup_pre_z_v1<'source, 'proof, S>(
    previous: RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeGlobalLookupPreZPrerequisiteV1<'source, 'proof, S>,
    RnsNativeGlobalLookupZCommitmentViewErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let view = PreZEnvelopeViewV1::from_canonical_prefix_v1(previous.residual())?;
    let context = pre_z_safe_context_v1(&previous)?;
    let roots = pre_z_role_roots_v1(
        &previous,
        view.multiplicity,
        view.sumcheck_mask,
        view.inverse_product_mask,
    )?;
    let live = {
        let pre_global_capability = previous.pre_global_lookup_capability_v1();
        derive_global_z_v1(pre_global_capability, context, roots)?
    };
    Ok(RnsNativeGlobalLookupPreZPrerequisiteV1 {
        previous,
        view,
        live,
    })
}

#[allow(
    dead_code,
    reason = "the private post-z entry awaits inverse-product and global-lookup proof consumers"
)]
pub(super) fn authenticate_rns_native_global_lookup_post_z_v1<'source, 'proof, S>(
    pre_z: RnsNativeGlobalLookupPreZPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S>,
    RnsNativeGlobalLookupZCommitmentViewErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let existing = pre_z.previous.previous().previous();
    let inventory = existing
        .previous()
        .previous()
        .previous()
        .previous()
        .inventory();
    let added_at = |purpose: PhysicalPurposeV1, ordinal: usize| match purpose {
        PhysicalPurposeV1::ComparatorDifferenceInverse => inventory
            .comparator_difference_inverse(ordinal / LOW_DIGITS_V1, ordinal % LOW_DIGITS_V1),
        PhysicalPurposeV1::SmallPositiveInverse => inventory
            .small_source_lookup_inverses(ordinal)
            .map(|values| values.0),
        PhysicalPurposeV1::SmallNegativeInverse => inventory
            .small_source_lookup_inverses(ordinal)
            .map(|values| values.1),
        PhysicalPurposeV1::QMaskDigitInverse => inventory
            .q_mask_lookup_inverses(ordinal % Q_MASK_BLOCKS_V1)
            .map(|values| values.digit_inverses[ordinal / Q_MASK_BLOCKS_V1]),
        PhysicalPurposeV1::QMaskComplementInverse => inventory
            .q_mask_lookup_inverses(ordinal % Q_MASK_BLOCKS_V1)
            .map(|values| values.complement_inverses[ordinal / Q_MASK_BLOCKS_V1]),
        _ => None,
    };
    let pre_z_binding_digest = pre_z.live.pre_z_binding_digest;
    let (post_z_transcript_digest, roots, live) = bind_post_z_v1(pre_z.live, pre_z.view, added_at)?;
    let residual_digest = residual_digest_v1(
        pre_z_binding_digest,
        post_z_transcript_digest,
        roots,
        pre_z.view.residual,
    )?;
    let mut binding = Keccak256::new();
    binding.update(PREREQUISITE_DOMAIN_V1);
    binding.update(&[VERSION_V1]);
    for digest in [
        pre_z_binding_digest,
        post_z_transcript_digest,
        roots.existing_root,
        roots.added_root,
        roots.alias_root,
        roots.global_root,
        residual_digest,
        pre_z.view.codec_digest,
        // These statement-4 values hash the complete current frame.  They are
        // deliberately admitted only after z derivation, inverse decoding,
        // and codec verification.
        pre_z.previous.residual_digest(),
        pre_z.previous.binding_digest(),
    ] {
        binding.update(&digest);
    }
    let binding_digest = binding.finalize();
    if binding_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(RnsNativeGlobalLookupPostZPrerequisiteV1 {
        previous: pre_z.previous,
        residual: pre_z.view.residual,
        existing_inverse_bytes: pre_z.view.existing_inverse_bytes,
        multiplicity: pre_z.view.multiplicity,
        _retired_sumcheck_mask: pre_z.view.sumcheck_mask,
        inverse_product_mask: pre_z.view.inverse_product_mask,
        pre_z_binding_digest,
        post_z_transcript_digest,
        existing_inverse_root: roots.existing_root,
        added_inverse_root: roots.added_root,
        alias_root: roots.alias_root,
        global_inverse_root: roots.global_root,
        residual_digest,
        binding_digest,
        _live: live,
    })
}

#[path = "rns_native_global_inverse_product_sumcheck.rs"]
pub(super) mod rns_native_global_inverse_product_sumcheck;

#[cfg(test)]
#[path = "rns_native_global_lookup_z_commitment_view_tests.rs"]
mod tests;
