//! Exact streaming comparator-carry product proof for the 40-limb replacement.
//!
//! This stage consumes the move-only statement-3 prerequisite and verifies the
//! three relation families that form global-lookup coefficient statement 5:
//!
//! `beta_h(beta_h-1)=0` for `h=0..17`,
//! `m=bD*beta_16`, and `beta_17=beta_16-m`.
//!
//! The retired design described one 107,085,824-gate product argument, but the
//! production T256 basis is intentionally capped at 65,536 gates.  This exact
//! replacement uses five independently bound cores per 16,384-coordinate
//! group: four cores cover four borrow columns each, and the fifth covers
//! `beta_16`, `beta_17`, `bD*beta_16=m`, and the terminal carry equation.  All
//! 1,720 cores are canonical-decoded before algebra starts; during verification
//! only one bounded circuit is live at a time.
//!
//! Because every product residual is constrained to zero directly, this is
//! stronger than the retired kappa/delta aggregate and has no polynomial
//! batching error.  Boolean roots are exactly `{0,1}` in the T256 scalar field,
//! and `beta_17-beta_16+m` has integer absolute value at most two, so the carry
//! equation cannot be satisfied by field wraparound.
//!
//! This product does not prove membership of the committed radix-difference
//! digits in `[0,2^15)`, the linear `D-K` subtraction recurrence, radix
//! reconstruction, inverse lookup relations, or the global lookup.  Those
//! obligations remain fail-closed.  The output is private, move-only, and
//! non-authorizing.
//!
//! The statement-3 residual and prerequisite-binding digests are intentionally
//! absent from this nested wire and its proof transcripts: both are computed
//! over a container that includes these statement-5 bytes.  They are retained
//! only in the post-verification prerequisite binding, avoiding a Keccak
//! fixed-point cycle while preserving the complete predecessor identity.

use core::marker::PhantomData;

use super::{
    rns_native_comparator_product::{
        RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1,
        RnsNativeComparatorProductPrerequisiteV1,
    },
    rns_native_cross_field_inventory::ComparatorRangeCarryCommitmentsV1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
};
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, GeneralizedBulletproofErrorV1, LinComb, ProofSuite, Variable,
        VerifierTranscript,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZSP5";
const STATEMENT_V1: u8 = 5;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const REPETITIONS_V1: usize = 5;
const BLOCKS_PER_RECORD_V1: usize = 8;
const GROUPS_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize * BLOCKS_PER_RECORD_V1;
const CHUNKS_PER_GROUP_V1: usize = 5;
const RECORDS_V1: usize = GROUPS_V1 * CHUNKS_PER_GROUP_V1;
const COORDINATES_V1: usize = 16_384;
const RADIX_LOG2_V1: u8 = 15;
const RADIX_BASE_V1: u64 = 1 << RADIX_LOG2_V1;
const RADIX_LOW_DIGITS_V1: usize = 17;
const BORROWS_V1: usize = 18;
const BORROWS_PER_BOOLEAN_CHUNK_V1: usize = 4;
const BOOLEAN_CHUNKS_V1: usize = 4;
const BOOLEAN_GATES_PER_COORDINATE_V1: usize = BORROWS_PER_BOOLEAN_CHUNK_V1;
const FINAL_GATES_PER_COORDINATE_V1: usize = 3;
const BOOLEAN_GATES_V1: usize = COORDINATES_V1 * BOOLEAN_GATES_PER_COORDINATE_V1;
const FINAL_GATES_V1: usize = COORDINATES_V1 * FINAL_GATES_PER_COORDINATE_V1;
const PADDED_GATES_V1: usize = 65_536;
const LOG_PADDED_GATES_V1: usize = 16;
const BOOLEAN_CONSTRAINTS_PER_COORDINATE_V1: usize = 3 * BORROWS_PER_BOOLEAN_CHUNK_V1;
const FINAL_CONSTRAINTS_PER_COORDINATE_V1: usize = 3 + 3 + 3 + 1;
const BOOLEAN_CONSTRAINTS_V1: usize = COORDINATES_V1 * BOOLEAN_CONSTRAINTS_PER_COORDINATE_V1;
const FINAL_CONSTRAINTS_V1: usize = COORDINATES_V1 * FINAL_CONSTRAINTS_PER_COORDINATE_V1;
const COMMITMENTS_PER_CORE_V1: usize = 4;
const DIFFERENCE_TOP_FINAL_COMMITMENT_V1: usize = 0;
const MIXED_TOP_FINAL_COMMITMENT_V1: usize = 1;
const BORROW_16_FINAL_COMMITMENT_V1: usize = 2;
const BORROW_17_FINAL_COMMITMENT_V1: usize = 3;
const CARRY_INTEGER_ABSOLUTE_BOUND_V1: u8 = 2;
const CONDITIONAL_RADIX_ROW_ABSOLUTE_BOUND_V1: u64 = 2 * RADIX_BASE_V1 - 1;
const FIXED_CORE_POINTS_V1: usize = 2 * COMMITMENTS_PER_CORE_V1 + 9;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 1 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;

// Fixed prefix through residual length: frame/geometry, six binding digests,
// and the residual length.
const HEADER_BYTES_V1: usize = 244;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = RECORDS_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statement=5;groups=344;coordinates=16384;group-chunks=(beta_0..3,beta_4..7,beta_8..11,beta_12..15,(beta_16,beta_17,bD,m));boolean-chunk-gates=4*16384;boolean-constraints-per-coordinate=(aL=beta,aR=aL-1,aO=0)*4;final-chunk-gates=3*16384;final-constraints=(beta16-boolean,beta17-boolean,aLtop=bD,aRtop=beta16,aOtop=m,beta17-beta16+m=0);padded-gates=65536;no-aggregate-residual";
const INTEGER_NO_WRAP_LANGUAGE_V1: &[u8] = b"statement3-fixes-bD-in-{0,1};statement5-fixes-beta_h-in-{0,1};m=bD*beta16-in-{0,1};beta17-beta16+m-has-integer-absolute-value<=2<pT;conditional-D-minus-K-radix-row-absolute-value<=65535<pT-after-digit-lookup;no-unverified-range-is-promoted";
const REMAINING_RANGE_BOUNDARY_V1: &[u8] = b"not-yet-verified:difference-digit-membership-in-[0,32768),D-minus-K-borrow-linear-equations,difference-inverse-lookup,radix-reconstruction,canonical-complement,small-source-product,q-mask,global-lookup";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZSP5/direct-five-chunk/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-range-carry.prerequisite";

const COMPARATOR_RANGE_CARRY_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true;
const RADIX_DIFFERENCE_DIGIT_RANGE_VERIFIED_V1: bool = false;
const RADIX_SUBTRACTION_AND_RECONSTRUCTION_VERIFIED_V1: bool = false;
const SMALL_SIGNED_PRODUCT_VERIFIED_V1: bool = false;
const CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;

const _: () = {
    assert!(GROUPS_V1 == 344);
    assert!(RECORDS_V1 == 1_720);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(REPETITIONS_V1 == 5);
    assert!(RADIX_LOW_DIGITS_V1 == 17);
    assert!(BORROWS_V1 == 18);
    assert!(BOOLEAN_CHUNKS_V1 * BORROWS_PER_BOOLEAN_CHUNK_V1 == 16);
    assert!(BOOLEAN_GATES_V1 == 65_536);
    assert!(FINAL_GATES_V1 == 49_152);
    assert!(PADDED_GATES_V1 == 65_536);
    assert!(BOOLEAN_CONSTRAINTS_V1 == 196_608);
    assert!(FINAL_CONSTRAINTS_V1 == 163_840);
    assert!(COMMITMENTS_PER_CORE_V1 == 4);
    assert!(FIXED_CORE_POINTS_V1 == 17);
    assert!(CORE_POINTS_V1 == 49);
    assert!(CORE_BYTES_V1 == 1_777);
    assert!(RECORD_BYTES_V1 == 1_782);
    assert!(RECORD_SET_BYTES_V1 == 3_065_040);
    assert!(HEADER_BYTES_V1 == 244);
    assert!(MIN_WIRE_BYTES_V1 == 3_065_317);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1 == 3_147_470);
    assert!(CARRY_INTEGER_ABSOLUTE_BOUND_V1 == 2);
    assert!(CONDITIONAL_RADIX_ROW_ABSOLUTE_BOUND_V1 == 65_535);
    assert!(VEGA_T256_SCALAR_MODULUS_BE_V1[0] != 0);
    assert!(COMPARATOR_RANGE_CARRY_PRODUCT_VERIFIER_IMPLEMENTED_V1);
    assert!(!RADIX_DIFFERENCE_DIGIT_RANGE_VERIFIED_V1);
    assert!(!RADIX_SUBTRACTION_AND_RECONSTRUCTION_VERIFIED_V1);
    assert!(!SMALL_SIGNED_PRODUCT_VERIFIED_V1);
    assert!(!CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
};

/// Failure while decoding or verifying comparator product statement 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeComparatorRangeCarryErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidPoint,
    InvalidScalar,
    InvalidIntegrity,
    InvalidContext,
    Algebra,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeComparatorRangeCarryErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeComparatorRangeCarryErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeComparatorRangeCarryErrorV1 {
    fn from(_: GeneralizedBulletproofErrorV1) -> Self {
        Self::Algebra
    }
}

#[derive(Clone, Copy)]
pub(super) struct UpstreamBindingV1 {
    pub(super) prior_context_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inventory_root: [u8; DIGEST_BYTES_V1],
    pub(super) statement3_proof_set_root: [u8; DIGEST_BYTES_V1],
    pub(super) statement3_verified_transcript_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeComparatorProductPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        Self {
            prior_context_digest: previous.inventory().prior_context_digest(),
            inventory_root: previous.inventory().inventory_root(),
            statement3_proof_set_root: previous.proof_set_root(),
            statement3_verified_transcript_root: previous.verified_transcript_root(),
        }
    }

    fn is_valid_v1(self) -> bool {
        ![
            self.prior_context_digest,
            self.inventory_root,
            self.statement3_proof_set_root,
            self.statement3_verified_transcript_root,
        ]
        .contains(&[0; DIGEST_BYTES_V1])
    }
}

#[derive(Clone, Copy)]
struct RangeCarryChunkCommitmentsV1 {
    points: [Point; COMMITMENTS_PER_CORE_V1],
}

fn chunk_commitments_v1(
    commitments: ComparatorRangeCarryCommitmentsV1,
    chunk: usize,
) -> Result<RangeCarryChunkCommitmentsV1, RnsNativeComparatorRangeCarryErrorV1> {
    let points = match chunk {
        0..=3 => {
            let first = chunk * BORROWS_PER_BOOLEAN_CHUNK_V1;
            [
                commitments.borrows[first],
                commitments.borrows[first + 1],
                commitments.borrows[first + 2],
                commitments.borrows[first + 3],
            ]
        }
        4 => [
            commitments.difference_top,
            commitments.mixed_top,
            commitments.borrows[16],
            commitments.borrows[17],
        ],
        _ => return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry),
    };
    Ok(RangeCarryChunkCommitmentsV1 { points })
}

fn chunk_geometry_v1(
    coordinates: usize,
    chunk: usize,
) -> Result<(usize, usize), RnsNativeComparatorRangeCarryErrorV1> {
    let (gates_per_coordinate, constraints_per_coordinate) = match chunk {
        0..=3 => (
            BOOLEAN_GATES_PER_COORDINATE_V1,
            BOOLEAN_CONSTRAINTS_PER_COORDINATE_V1,
        ),
        4 => (
            FINAL_GATES_PER_COORDINATE_V1,
            FINAL_CONSTRAINTS_PER_COORDINATE_V1,
        ),
        _ => return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry),
    };
    Ok((
        coordinates
            .checked_mul(gates_per_coordinate)
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?,
        coordinates
            .checked_mul(constraints_per_coordinate)
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?,
    ))
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeComparatorRangeCarryErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeComparatorRangeCarryErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeComparatorRangeCarryErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeComparatorRangeCarryErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeComparatorRangeCarryErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeComparatorRangeCarryErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
        }
        let mut decoder = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidScalar)?;
        }
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct ComparatorRangeCarryProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> ComparatorRangeCarryProofSetViewV1<'a> {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeComparatorProductPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeComparatorRangeCarryErrorV1> {
        Self::from_components_v1(
            previous.residual(),
            UpstreamBindingV1::from_prerequisite_v1(previous),
            |group| {
                previous
                    .inventory()
                    .comparator_range_carry_commitments(group)
            },
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
        commitment_at: F,
    ) -> Result<Self, RnsNativeComparatorRangeCarryErrorV1>
    where
        F: FnMut(usize) -> Option<ComparatorRangeCarryCommitmentsV1>,
    {
        if bytes.len() > RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeComparatorRangeCarryErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != STATEMENT_V1
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::from(decoder.u8()?) != CHUNKS_PER_GROUP_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != BOOLEAN_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != FINAL_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != BOOLEAN_CONSTRAINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != FINAL_CONSTRAINTS_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
        }
        let upstream = UpstreamBindingV1 {
            prior_context_digest: decoder.array()?,
            inventory_root: decoder.array()?,
            statement3_proof_set_root: decoder.array()?,
            statement3_verified_transcript_root: decoder.array()?,
        };
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || upstream.prior_context_digest != expected.prior_context_digest
            || upstream.inventory_root != expected.inventory_root
            || upstream.statement3_proof_set_root != expected.statement3_proof_set_root
            || upstream.statement3_verified_transcript_root
                != expected.statement3_verified_transcript_root
            || [proof_set_root, residual_digest].contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidHeader);
        }
        let records = decoder.take(RECORD_SET_BYTES_V1)?;
        for group in 0..GROUPS_V1 {
            for chunk in 0..CHUNKS_PER_GROUP_V1 {
                ExactCoreViewV1::parse_v1(record_at_v1(records, group, chunk)?)?;
            }
        }
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        if decoder.cursor != bytes.len()
            || canonical_proof_set_root_v1(upstream, records, commitment_at)? != proof_set_root
            || canonical_residual_digest_v1(upstream, proof_set_root, residual)? != residual_digest
            || codec_digest == [0; DIGEST_BYTES_V1]
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity);
        }
        Ok(Self {
            records,
            residual,
            proof_set_root,
            residual_digest,
            codec_digest,
        })
    }

    fn core_v1(
        &self,
        group: usize,
        chunk: usize,
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeComparatorRangeCarryErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, group, chunk)?)
    }
}

fn record_at_v1(
    records: &[u8],
    group: usize,
    chunk: usize,
) -> Result<&[u8], RnsNativeComparatorRangeCarryErrorV1> {
    if group >= GROUPS_V1 || chunk >= CHUNKS_PER_GROUP_V1 || records.len() != RECORD_SET_BYTES_V1 {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    let ordinal = group
        .checked_mul(CHUNKS_PER_GROUP_V1)
        .and_then(|value| value.checked_add(chunk))
        .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
    let offset = ordinal
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
    let end = offset
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(offset..end)
        .ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry)?;
    if usize::from(u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry)?,
    )) != group
        || usize::from(record[2]) != chunk
        || usize::from(u16::from_be_bytes(
            record[3..5]
                .try_into()
                .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn absorb_upstream_v1(hash: &mut Keccak256, upstream: UpstreamBindingV1) {
    for digest in [
        upstream.prior_context_digest,
        upstream.inventory_root,
        upstream.statement3_proof_set_root,
        upstream.statement3_verified_transcript_root,
    ] {
        hash.update(&digest);
    }
}

fn circuit_manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CIRCUIT_MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1, RADIX_LOG2_V1]);
    for value in [
        GROUPS_V1 as u32,
        CHUNKS_PER_GROUP_V1 as u32,
        COORDINATES_V1 as u32,
        BOOLEAN_GATES_V1 as u32,
        FINAL_GATES_V1 as u32,
        PADDED_GATES_V1 as u32,
        BOOLEAN_CONSTRAINTS_V1 as u32,
        FINAL_CONSTRAINTS_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for language in [
        CIRCUIT_LANGUAGE_V1,
        INTEGER_NO_WRAP_LANGUAGE_V1,
        REMAINING_RANGE_BOUNDARY_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.finalize()
}

fn canonical_proof_set_root_v1<F>(
    upstream: UpstreamBindingV1,
    records: &[u8],
    mut commitment_at: F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1>
where
    F: FnMut(usize) -> Option<ComparatorRangeCarryCommitmentsV1>,
{
    if records.len() != RECORD_SET_BYTES_V1 || !upstream.is_valid_v1() {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&circuit_manifest_digest_v1());
    for group in 0..GROUPS_V1 {
        let commitments =
            commitment_at(group).ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidContext)?;
        for chunk in 0..CHUNKS_PER_GROUP_V1 {
            let core = record_at_v1(records, group, chunk)?;
            let chunk_commitments = chunk_commitments_v1(commitments, chunk)?;
            hash.update(&(group as u16).to_be_bytes());
            hash.update(&[chunk as u8]);
            for point in chunk_commitments.points {
                hash.update(&encode_point_v1(point)?);
            }
            hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
            hash.update(core);
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

pub(super) fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1> {
    if residual.is_empty() || !upstream.is_valid_v1() {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

fn range_carry_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
    chunk: usize,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeComparatorRangeCarryErrorV1> {
    let (gates, constraint_count) = chunk_geometry_v1(coordinates, chunk)?;
    if coordinates == 0 || !padded_gates.is_power_of_two() || gates > padded_gates {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?;
    let one = Scalar::one();
    match chunk {
        0..=3 => {
            for coordinate in 0..coordinates {
                let first_gate = coordinate * BOOLEAN_GATES_PER_COORDINATE_V1;
                for local in 0..BORROWS_PER_BOOLEAN_CHUNK_V1 {
                    let gate = first_gate + local;
                    constraints.extend([
                        LinComb::empty().term(one, Variable::aL(gate)).term(
                            -one,
                            Variable::CG {
                                commitment: local,
                                index: coordinate,
                            },
                        ),
                        LinComb::empty()
                            .term(one, Variable::aR(gate))
                            .term(-one, Variable::aL(gate))
                            .constant(one),
                        LinComb::empty().term(one, Variable::aO(gate)),
                    ]);
                }
            }
        }
        4 => {
            for coordinate in 0..coordinates {
                let beta16_gate = coordinate * FINAL_GATES_PER_COORDINATE_V1;
                let beta17_gate = beta16_gate + 1;
                let top_gate = beta16_gate + 2;
                constraints.extend([
                    LinComb::empty().term(one, Variable::aL(beta16_gate)).term(
                        -one,
                        Variable::CG {
                            commitment: BORROW_16_FINAL_COMMITMENT_V1,
                            index: coordinate,
                        },
                    ),
                    LinComb::empty()
                        .term(one, Variable::aR(beta16_gate))
                        .term(-one, Variable::aL(beta16_gate))
                        .constant(one),
                    LinComb::empty().term(one, Variable::aO(beta16_gate)),
                    LinComb::empty().term(one, Variable::aL(beta17_gate)).term(
                        -one,
                        Variable::CG {
                            commitment: BORROW_17_FINAL_COMMITMENT_V1,
                            index: coordinate,
                        },
                    ),
                    LinComb::empty()
                        .term(one, Variable::aR(beta17_gate))
                        .term(-one, Variable::aL(beta17_gate))
                        .constant(one),
                    LinComb::empty().term(one, Variable::aO(beta17_gate)),
                    LinComb::empty().term(one, Variable::aL(top_gate)).term(
                        -one,
                        Variable::CG {
                            commitment: DIFFERENCE_TOP_FINAL_COMMITMENT_V1,
                            index: coordinate,
                        },
                    ),
                    LinComb::empty().term(one, Variable::aR(top_gate)).term(
                        -one,
                        Variable::CG {
                            commitment: BORROW_16_FINAL_COMMITMENT_V1,
                            index: coordinate,
                        },
                    ),
                    LinComb::empty().term(one, Variable::aO(top_gate)).term(
                        -one,
                        Variable::CG {
                            commitment: MIXED_TOP_FINAL_COMMITMENT_V1,
                            index: coordinate,
                        },
                    ),
                    LinComb::empty()
                        .term(
                            one,
                            Variable::CG {
                                commitment: BORROW_17_FINAL_COMMITMENT_V1,
                                index: coordinate,
                            },
                        )
                        .term(
                            -one,
                            Variable::CG {
                                commitment: BORROW_16_FINAL_COMMITMENT_V1,
                                index: coordinate,
                            },
                        )
                        .term(
                            one,
                            Variable::CG {
                                commitment: MIXED_TOP_FINAL_COMMITMENT_V1,
                                index: coordinate,
                            },
                        ),
                ]);
            }
        }
        _ => return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry),
    }
    if constraints.len() != constraint_count {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_range_carry_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    chunk: usize,
    commitments: RangeCarryChunkCommitmentsV1,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeComparatorRangeCarryErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        range_carry_constraints_v1(coordinates, padded_gates, chunk)?,
        commitments.points.to_vec(),
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeComparatorRangeCarryErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeComparatorRangeCarryErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_transcript_state_v1(
    upstream: UpstreamBindingV1,
    group: usize,
    chunk: usize,
    commitments: RangeCarryChunkCommitmentsV1,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Vec<u8>, RnsNativeComparatorRangeCarryErrorV1> {
    let (gates, constraints) = chunk_geometry_v1(coordinates, chunk)?;
    if !upstream.is_valid_v1()
        || group >= GROUPS_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
        || gates > padded_gates
    {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidContext);
    }
    let mut state = Vec::with_capacity(1_024);
    for frame in [
        TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1, STATEMENT_V1],
        TRANSCRIPT_SCHEMA_V1,
        upstream.prior_context_digest.as_slice(),
        upstream.inventory_root.as_slice(),
        upstream.statement3_proof_set_root.as_slice(),
        upstream.statement3_verified_transcript_root.as_slice(),
        (group as u16).to_be_bytes().as_slice(),
        &[chunk as u8],
        (coordinates as u32).to_be_bytes().as_slice(),
        (gates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (constraints as u32).to_be_bytes().as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
    }
    for point in commitments.points {
        append_frame_v1(&mut state, &encode_point_v1(point)?)?;
    }
    Ok(state)
}

fn hash_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(bytes);
    hash.finalize()
}

fn derive_challenge_v1(
    state: &mut Vec<u8>,
    ordinal: &mut u32,
) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
    for attempt in 0_u8..128 {
        let mut input = Vec::with_capacity(CHALLENGE_DOMAIN_V1.len() + state.len() + 6);
        input.extend_from_slice(CHALLENGE_DOMAIN_V1);
        input.extend_from_slice(state);
        input.extend_from_slice(&ordinal.to_be_bytes());
        input.push(attempt);
        let mut low = input.clone();
        low.push(0);
        input.push(1);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&hash_v1(&low));
        wide[32..].copy_from_slice(&hash_v1(&input));
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

struct ComparatorRangeCarryVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> ComparatorRangeCarryVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    #[allow(clippy::too_many_arguments)]
    fn new_v1(
        upstream: UpstreamBindingV1,
        group: usize,
        chunk: usize,
        commitments: RangeCarryChunkCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        generator_basis_digest: [u8; DIGEST_BYTES_V1],
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeComparatorRangeCarryErrorV1> {
        Ok(Self {
            state: initial_transcript_state_v1(
                upstream,
                group,
                chunk,
                commitments,
                coordinates,
                padded_gates,
                generator_basis_digest,
            )?,
            core,
            cursor: 0,
            challenge_ordinal: 0,
            suite: PhantomData,
        })
    }

    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let value = self.core.bytes.get(self.cursor..end).ok_or(
            GeneralizedBulletproofErrorV1::ProofLength {
                actual: self.core.bytes.len(),
                expected: end,
            },
        )?;
        self.cursor = end;
        Ok(value)
    }

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for ComparatorRangeCarryVerifierTranscriptV1<'_, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; SCALAR_BYTES_V1] = self
            .take_v1(SCALAR_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        Ok(scalar)
    }

    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; POINT_BYTES_V1] = self
            .take_v1(POINT_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        Ok(point)
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_challenge_v1(&mut self.state, &mut self.challenge_ordinal)
    }
}

fn prerequisite_binding_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeComparatorProductPrerequisiteV1<'_, '_, S>,
    view: ComparatorRangeCarryProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1> {
    let inventory = previous.inventory();
    let linked = inventory.linked();
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    for digest in [
        previous.residual_digest(),
        previous.binding_digest(),
        inventory.continuation_digest(),
        inventory.binding_digest(),
        linked.source().statement_anchor_digest(),
        linked.source().qpcs().transcript_digest(),
        linked.source().qpcs().residual_digest(),
        linked.terminal().binding_digest(),
        linked.zero_padding().binding_digest(),
        linked.cross_proof_digest(),
        linked.cross_link_digest(),
        linked.anchor_digest(),
        view.proof_set_root,
        verified_transcript_root,
        view.residual_digest,
        view.codec_digest,
        circuit_manifest_digest_v1(),
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    ] {
        hash.update(&digest);
    }
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        for repetition in 0..REPETITIONS_V1 {
            let (product, opening_quotient) = inventory
                .qpcs_evaluation(limb, repetition)
                .ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidContext)?;
            hash.update(&[limb as u8, repetition as u8]);
            hash.update(&product.to_be_bytes());
            hash.update(&opening_quotient.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only, private evidence that comparator product statement 5 has been
/// verified after statement 3.
///
/// The residual is not authenticated as a later proof schema by this token and
/// confers no receipt, release, or authorization capability.
#[allow(
    missing_copy_implementations,
    reason = "the statement-3 owner and unverified residual must advance exactly once"
)]
pub(super) struct RnsNativeComparatorRangeCarryPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    _previous: RnsNativeComparatorProductPrerequisiteV1<'source, 'proof, S>,
    _residual: &'proof [u8],
    _proof_set_root: [u8; DIGEST_BYTES_V1],
    _verified_transcript_root: [u8; DIGEST_BYTES_V1],
    _residual_digest: [u8; DIGEST_BYTES_V1],
    _binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeComparatorRangeCarryPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeComparatorProductPrerequisiteV1<'source, 'proof, S> {
        &self._previous
    }

    pub(super) const fn residual(&self) -> &'proof [u8] {
        self._residual
    }

    pub(super) const fn proof_set_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self._proof_set_root
    }

    pub(super) const fn verified_transcript_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self._verified_transcript_root
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self._residual_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self._binding_digest
    }
}

/// Consume statement 3 and verify every bounded statement-5 core
/// sequentially.
#[allow(
    dead_code,
    reason = "the sound private statement-5 entry awaits statement 8, q-mask, and global-lookup consumers"
)]
pub(super) fn verify_rns_native_comparator_range_carry_v1<'source, 'proof, S>(
    previous: RnsNativeComparatorProductPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeComparatorRangeCarryPrerequisiteV1<'source, 'proof, S>,
    RnsNativeComparatorRangeCarryErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let upstream = UpstreamBindingV1::from_prerequisite_v1(&previous);
    let view = ComparatorRangeCarryProofSetViewV1::from_prerequisite_v1(&previous)?;
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut verified, upstream);
    verified.update(&view.proof_set_root);
    for group in 0..GROUPS_V1 {
        let commitments = previous
            .inventory()
            .comparator_range_carry_commitments(group)
            .ok_or(RnsNativeComparatorRangeCarryErrorV1::InvalidContext)?;
        for chunk in 0..CHUNKS_PER_GROUP_V1 {
            let chunk_commitments = chunk_commitments_v1(commitments, chunk)?;
            let core = view.core_v1(group, chunk)?;
            let mut transcript =
                ComparatorRangeCarryVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
                    upstream,
                    group,
                    chunk,
                    chunk_commitments,
                    COORDINATES_V1,
                    PADDED_GATES_V1,
                    ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
                    core,
                )?;
            build_range_carry_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
                COORDINATES_V1,
                PADDED_GATES_V1,
                chunk,
                chunk_commitments,
            )?
            .verify(&mut transcript)?;
            let transcript_digest = transcript.finish_v1()?;
            verified.update(&(group as u16).to_be_bytes());
            verified.update(&[chunk as u8]);
            verified.update(&transcript_digest);
        }
    }
    let verified_transcript_root = verified.finalize();
    if verified_transcript_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(&previous, view, verified_transcript_root)?;
    Ok(RnsNativeComparatorRangeCarryPrerequisiteV1 {
        _previous: previous,
        _residual: view.residual,
        _proof_set_root: view.proof_set_root,
        _verified_transcript_root: verified_transcript_root,
        _residual_digest: view.residual_digest,
        _binding_digest: binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_comparator_range_carry_product_tests.rs"]
mod tests;
