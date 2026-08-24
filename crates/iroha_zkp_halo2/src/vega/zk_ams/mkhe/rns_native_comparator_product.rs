//! First streaming sparse vector-product proof for the 40-limb replacement.
//!
//! This stage consumes the authenticated commitment inventory and verifies
//! global-lookup statement 3 at the release vector width.  The wire contains
//! exactly 344 generalized-Bulletproof cores.  Core `g` opens the committed
//! comparator top vectors `bD_g` and `bS_g` and proves, at every one of 16,384
//! coordinates,
//!
//! `bD * (bD - 1) = 0`, `bS * (bS - 1) = 0`, and `bD * bS = 0`.
//!
//! One 49,152-gate circuit is built and verified at a time against the fixed
//! 65,536-generator T256 prefix.  Thus neither all 344 circuits nor their
//! sparse rows coexist in memory.  Every core is exact-decoded before the
//! first algebra check, and its Fiat-Shamir state binds the complete upstream
//! source/terminal/qPCS/zero/transcript context through the inventory context,
//! both commitment identities, its group ordinal, and the fixed circuit and
//! generator manifests.
//!
//! The output is private, move-only, and non-authorizing.  Comparator carry,
//! signed-small-source, q-mask, and global lookup obligations remain absent,
//! so the residual continuation remains opaque and composite readiness stays
//! fail-closed.

use core::marker::PhantomData;

use super::{
    rns_native_claimed_successor::RnsNativeClaimedSuccessorV1,
    rns_native_cross_field_inventory::RnsNativeCrossFieldInventoryPrerequisiteV1,
    rns_native_cross_field_rlwe_direct::{
        RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1,
    },
    rns_native_existing_radix_commitment_view::RnsNativeExistingRadixValidationPermitV1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_transcript::ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
};
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, GeneralizedBulletproofErrorV1, LinComb, ProofSuite, Variable,
        VerifierTranscript,
    },
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZSP3";
const STATEMENT_V1: u8 = 3;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const REPETITIONS_V1: usize = 5;
const BLOCKS_PER_RECORD_V1: usize = 8;
const GROUPS_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize * BLOCKS_PER_RECORD_V1;
const COORDINATES_V1: usize = 16_384;
const PRODUCTS_PER_COORDINATE_V1: usize = 3;
const GATES_V1: usize = COORDINATES_V1 * PRODUCTS_PER_COORDINATE_V1;
const PADDED_GATES_V1: usize = 65_536;
const LOG_PADDED_GATES_V1: usize = 16;
const CONSTRAINTS_PER_COORDINATE_V1: usize = 9;
const CONSTRAINTS_V1: usize = COORDINATES_V1 * CONSTRAINTS_PER_COORDINATE_V1;
const COMMITMENTS_V1: usize = 2;
const FIXED_CORE_POINTS_V1: usize = 13;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;

// Fixed prefix through residual length: frame header, eleven geometry fields,
// four binding digests, and the residual length.
const HEADER_BYTES_V1: usize = 171;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = GROUPS_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statement=3;group-count=344;coordinate-count=16384;commitments=(bD,bS);gate-order=coordinate-major:(bD*(bD-1),bS*(bS-1),bD*bS);rows-per-coordinate=9;each-input-linked-to-its-exact-commitment-coordinate;each-right-input-linked;each-product-output-zero;padded-gates=65536;no-aggregate-residual";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZSP3/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-comparator-product.prerequisite";

const COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true;
const COMPARATOR_RANGE_AND_CARRY_PRODUCT_VERIFIED_V1: bool = false;
const SMALL_SIGNED_PRODUCT_VERIFIED_V1: bool = false;
const CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;

const _: () = {
    assert!(GROUPS_V1 == 344);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(REPETITIONS_V1 == 5);
    assert!(GATES_V1 == 49_152);
    assert!(PADDED_GATES_V1 == 65_536);
    assert!(CONSTRAINTS_V1 == 147_456);
    assert!(CORE_POINTS_V1 == 45);
    assert!(CORE_BYTES_V1 == 1_645);
    assert!(RECORD_BYTES_V1 == 1_649);
    assert!(RECORD_SET_BYTES_V1 == 567_256);
    assert!(HEADER_BYTES_V1 == 171);
    assert!(MIN_WIRE_BYTES_V1 == 567_460);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1);
    assert!(RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1 == 6_180_515);
    assert!(COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_VERIFIER_IMPLEMENTED_V1);
    assert!(!COMPARATOR_RANGE_AND_CARRY_PRODUCT_VERIFIED_V1);
    assert!(!SMALL_SIGNED_PRODUCT_VERIFIED_V1);
    assert!(!CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
};

/// Failure while decoding or verifying the first 40-limb product argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeComparatorProductErrorV1 {
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

impl core::fmt::Display for RnsNativeComparatorProductErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeComparatorProductErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeComparatorProductErrorV1 {
    fn from(_: GeneralizedBulletproofErrorV1) -> Self {
        Self::Algebra
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeComparatorProductErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeComparatorProductErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeComparatorProductErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeComparatorProductErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeComparatorProductErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeComparatorProductErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeComparatorProductErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeComparatorProductErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
        }
        let mut decoder = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidScalar)?;
        }
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct ComparatorProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> ComparatorProofSetViewV1<'a> {
    #[cfg(test)]
    #[allow(dead_code, reason = "legacy raw-inventory parser is test-only")]
    fn from_inventory_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        inventory: &RnsNativeCrossFieldInventoryPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeComparatorProductErrorV1> {
        Self::from_components_v1(
            inventory.continuation(),
            inventory.prior_context_digest(),
            inventory.inventory_root(),
            |group| inventory.comparator_top_commitments(group),
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected_prior_context_digest: [u8; DIGEST_BYTES_V1],
        expected_inventory_root: [u8; DIGEST_BYTES_V1],
        commitment_at: F,
    ) -> Result<Self, RnsNativeComparatorProductErrorV1>
    where
        F: FnMut(usize) -> Option<(Point, Point)>,
    {
        if bytes.len() > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 {
            return Err(RnsNativeComparatorProductErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeComparatorProductErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != STATEMENT_V1
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != CONSTRAINTS_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
        }
        let prior_context_digest = decoder.array()?;
        let inventory_root = decoder.array()?;
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || expected_total != bytes.len()
            || prior_context_digest != expected_prior_context_digest
            || inventory_root != expected_inventory_root
            || [
                prior_context_digest,
                inventory_root,
                proof_set_root,
                residual_digest,
            ]
            .contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeComparatorProductErrorV1::InvalidHeader);
        }
        let records = decoder.take(RECORD_SET_BYTES_V1)?;
        for group in 0..GROUPS_V1 {
            let record = record_at_v1(records, group)?;
            ExactCoreViewV1::parse_v1(record)?;
        }
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        if decoder.cursor != bytes.len()
            || canonical_proof_set_root_v1(
                prior_context_digest,
                inventory_root,
                records,
                commitment_at,
            )? != proof_set_root
            || canonical_residual_digest_v1(
                prior_context_digest,
                inventory_root,
                proof_set_root,
                residual,
            )? != residual_digest
            || codec_digest == [0; DIGEST_BYTES_V1]
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity);
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
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeComparatorProductErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, group)?)
    }
}

fn record_at_v1(records: &[u8], group: usize) -> Result<&[u8], RnsNativeComparatorProductErrorV1> {
    if group >= GROUPS_V1 || records.len() != RECORD_SET_BYTES_V1 {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    let offset = group
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let end = offset
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(offset..end)
        .ok_or(RnsNativeComparatorProductErrorV1::InvalidGeometry)?;
    if u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidGeometry)?,
    ) as usize
        != group
        || usize::from(u16::from_be_bytes(
            record[2..4]
                .try_into()
                .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeComparatorProductErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeComparatorProductErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn circuit_manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CIRCUIT_MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    hash.update(&(GROUPS_V1 as u16).to_be_bytes());
    hash.update(&(COORDINATES_V1 as u32).to_be_bytes());
    hash.update(&(GATES_V1 as u32).to_be_bytes());
    hash.update(&(PADDED_GATES_V1 as u32).to_be_bytes());
    hash.update(&(CONSTRAINTS_V1 as u32).to_be_bytes());
    hash.update(&(CIRCUIT_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(CIRCUIT_LANGUAGE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.finalize()
}

fn canonical_proof_set_root_v1<F>(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    records: &[u8],
    mut commitment_at: F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorProductErrorV1>
where
    F: FnMut(usize) -> Option<(Point, Point)>,
{
    if records.len() != RECORD_SET_BYTES_V1 {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    hash.update(&prior_context_digest);
    hash.update(&inventory_root);
    hash.update(&circuit_manifest_digest_v1());
    for group in 0..GROUPS_V1 {
        let core = record_at_v1(records, group)?;
        let (difference, sum) =
            commitment_at(group).ok_or(RnsNativeComparatorProductErrorV1::InvalidContext)?;
        hash.update(&(group as u16).to_be_bytes());
        hash.update(&encode_point_v1(difference)?);
        hash.update(&encode_point_v1(sum)?);
        hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
        hash.update(core);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

pub(super) fn canonical_residual_digest_v1(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorProductErrorV1> {
    if residual.is_empty() {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&prior_context_digest);
    hash.update(&inventory_root);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity);
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

fn comparator_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeComparatorProductErrorV1> {
    if coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates
            .checked_mul(PRODUCTS_PER_COORDINATE_V1)
            .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
            > padded_gates
    {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    let constraint_count = coordinates
        .checked_mul(CONSTRAINTS_PER_COORDINATE_V1)
        .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let one = Scalar::one();
    for coordinate in 0..coordinates {
        let difference_gate = coordinate * PRODUCTS_PER_COORDINATE_V1;
        let sum_gate = difference_gate + 1;
        let exclusion_gate = difference_gate + 2;
        constraints.extend([
            LinComb::empty()
                .term(one, Variable::aL(difference_gate))
                .term(
                    -one,
                    Variable::CG {
                        commitment: 0,
                        index: coordinate,
                    },
                ),
            LinComb::empty()
                .term(one, Variable::aR(difference_gate))
                .term(-one, Variable::aL(difference_gate))
                .constant(one),
            LinComb::empty().term(one, Variable::aO(difference_gate)),
            LinComb::empty().term(one, Variable::aL(sum_gate)).term(
                -one,
                Variable::CG {
                    commitment: 1,
                    index: coordinate,
                },
            ),
            LinComb::empty()
                .term(one, Variable::aR(sum_gate))
                .term(-one, Variable::aL(sum_gate))
                .constant(one),
            LinComb::empty().term(one, Variable::aO(sum_gate)),
            LinComb::empty()
                .term(one, Variable::aL(exclusion_gate))
                .term(
                    -one,
                    Variable::CG {
                        commitment: 0,
                        index: coordinate,
                    },
                ),
            LinComb::empty()
                .term(one, Variable::aR(exclusion_gate))
                .term(
                    -one,
                    Variable::CG {
                        commitment: 1,
                        index: coordinate,
                    },
                ),
            LinComb::empty().term(one, Variable::aO(exclusion_gate)),
        ]);
    }
    if constraints.len() != constraint_count {
        return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_comparator_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    difference: Point,
    sum: Point,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeComparatorProductErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        comparator_constraints_v1(coordinates, padded_gates)?,
        vec![difference, sum],
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeComparatorProductErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

#[derive(Clone, Copy)]
struct ComparatorTranscriptContextV1 {
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    group: usize,
    difference: Point,
    sum: Point,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
}

fn initial_transcript_state_v1(
    context: ComparatorTranscriptContextV1,
) -> Result<Vec<u8>, RnsNativeComparatorProductErrorV1> {
    let ComparatorTranscriptContextV1 {
        prior_context_digest,
        inventory_root,
        group,
        difference,
        sum,
        coordinates,
        padded_gates,
        generator_basis_digest,
    } = context;
    if prior_context_digest == [0; DIGEST_BYTES_V1]
        || inventory_root == [0; DIGEST_BYTES_V1]
        || group >= GROUPS_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
    {
        return Err(RnsNativeComparatorProductErrorV1::InvalidContext);
    }
    let gates = coordinates
        .checked_mul(PRODUCTS_PER_COORDINATE_V1)
        .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let constraints = coordinates
        .checked_mul(CONSTRAINTS_PER_COORDINATE_V1)
        .ok_or(RnsNativeComparatorProductErrorV1::ArithmeticOverflow)?;
    let mut state = Vec::with_capacity(512);
    for frame in [
        TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1, STATEMENT_V1],
        TRANSCRIPT_SCHEMA_V1,
        prior_context_digest.as_slice(),
        inventory_root.as_slice(),
        (group as u16).to_be_bytes().as_slice(),
        (coordinates as u32).to_be_bytes().as_slice(),
        (gates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (constraints as u32).to_be_bytes().as_slice(),
        encode_point_v1(difference)?.as_slice(),
        encode_point_v1(sum)?.as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
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

struct ComparatorVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> ComparatorVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn new_v1(
        context: ComparatorTranscriptContextV1,
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeComparatorProductErrorV1> {
        Ok(Self {
            state: initial_transcript_state_v1(context)?,
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorProductErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeComparatorProductErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for ComparatorVerifierTranscriptV1<'_, S>
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
    inventory: &RnsNativeCrossFieldInventoryPrerequisiteV1<'_, '_, S>,
    view: ComparatorProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorProductErrorV1> {
    let linked = inventory.linked();
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    for digest in [
        inventory.prior_context_digest(),
        inventory.inventory_root(),
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
                .ok_or(RnsNativeComparatorProductErrorV1::InvalidContext)?;
            hash.update(&[limb as u8, repetition as u8]);
            hash.update(&product.to_be_bytes());
            hash.update(&opening_quotient.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only, private evidence that statement 3 alone has been verified.
///
/// The retained residual is not authenticated as any later proof schema by
/// this token and confers no release, receipt, or authorization capability.
#[allow(
    missing_copy_implementations,
    reason = "the consumed inventory and unverified residual must advance exactly once"
)]
pub(super) struct RnsNativeComparatorProductPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    _parent: RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    >,
    _residual: &'proof [u8],
    _proof_set_root: [u8; DIGEST_BYTES_V1],
    _verified_transcript_root: [u8; DIGEST_BYTES_V1],
    _residual_digest: [u8; DIGEST_BYTES_V1],
    _binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeComparatorProductPrerequisiteV1<'source, 'proof, S>
{
    /// Purpose-forward the opaque snapshot from the exact claimed direct
    /// parent; no caller may substitute a free session capability.
    pub(super) const fn pre_global_lookup_capability_v1(
        &self,
    ) -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1 {
        self._parent.parent().pre_global_lookup_capability_v1()
    }

    pub(super) const fn inventory(
        &self,
    ) -> &RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S> {
        self._parent.parent().inventory()
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

    /// Consume statement-3 evidence and recover the exact claimed direct
    /// successor carrier. This is the only ownership path back to the direct
    /// frame retained by the comparator chain.
    pub(super) fn into_previous_v1(
        self,
    ) -> RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    > {
        self._parent
    }

    pub(super) fn take_existing_radix_validation_permit_v1(
        &mut self,
    ) -> Option<RnsNativeExistingRadixValidationPermitV1> {
        self._parent.take_existing_radix_validation_permit_v1()
    }
}

struct VerifiedComparatorProductPartsV1<'proof> {
    residual: &'proof [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

fn verify_comparator_product_parts_v1<'source, 'proof, S>(
    inventory: &RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
    successor: &'proof [u8],
) -> Result<VerifiedComparatorProductPartsV1<'proof>, RnsNativeComparatorProductErrorV1>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let view = ComparatorProofSetViewV1::from_components_v1(
        successor,
        inventory.prior_context_digest(),
        inventory.inventory_root(),
        |group| inventory.comparator_top_commitments(group),
    )?;
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, STATEMENT_V1]);
    verified.update(&inventory.prior_context_digest());
    verified.update(&inventory.inventory_root());
    verified.update(&view.proof_set_root);
    for group in 0..GROUPS_V1 {
        let (difference, sum) = inventory
            .comparator_top_commitments(group)
            .ok_or(RnsNativeComparatorProductErrorV1::InvalidContext)?;
        let core = view.core_v1(group)?;
        let transcript_context = ComparatorTranscriptContextV1 {
            prior_context_digest: inventory.prior_context_digest(),
            inventory_root: inventory.inventory_root(),
            group,
            difference,
            sum,
            coordinates: COORDINATES_V1,
            padded_gates: PADDED_GATES_V1,
            generator_basis_digest: ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        };
        let mut transcript = ComparatorVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
            transcript_context,
            core,
        )?;
        build_comparator_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
            COORDINATES_V1,
            PADDED_GATES_V1,
            difference,
            sum,
        )?
        .verify(&mut transcript)?;
        let transcript_digest = transcript.finish_v1()?;
        verified.update(&(group as u16).to_be_bytes());
        verified.update(&transcript_digest);
    }
    let verified_transcript_root = verified.finalize();
    if verified_transcript_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(inventory, view, verified_transcript_root)?;
    Ok(VerifiedComparatorProductPartsV1 {
        residual: view.residual,
        proof_set_root: view.proof_set_root,
        verified_transcript_root,
        residual_digest: view.residual_digest,
        binding_digest,
    })
}

/// Consume the exact-preflighted direct successor claim and verify all 344
/// comparator boolean/disjoint product proofs sequentially. This does not
/// assert the retained direct algebra. A production caller cannot enter this
/// stage from raw `inventory.continuation()` bytes.
#[allow(
    dead_code,
    reason = "the sound private statement-3 entry awaits the remaining statement-5, statement-8, and lookup consumers"
)]
pub(super) fn verify_rns_native_comparator_product_v1<'source, 'proof, S>(
    parent: RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    >,
) -> Result<
    RnsNativeComparatorProductPrerequisiteV1<'source, 'proof, S>,
    RnsNativeComparatorProductErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let verified =
        verify_comparator_product_parts_v1(parent.parent().inventory(), parent.successor())?;
    Ok(RnsNativeComparatorProductPrerequisiteV1 {
        _parent: parent,
        _residual: verified.residual,
        _proof_set_root: verified.proof_set_root,
        _verified_transcript_root: verified.verified_transcript_root,
        _residual_digest: verified.residual_digest,
        _binding_digest: verified.binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_comparator_product_tests.rs"]
mod tests;
