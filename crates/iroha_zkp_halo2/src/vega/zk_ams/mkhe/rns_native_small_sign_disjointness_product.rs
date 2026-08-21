//! Exact streaming small-sign product proof for the 40-limb replacement.
//!
//! This stage consumes the move-only statement-5 prerequisite and verifies
//! global-lookup coefficient statement 8 directly.  For every small-source
//! owner `u` and every one of its 16,384 coordinates it proves
//!
//! `(x_u + n_u) * n_u = 0`,
//!
//! where `x_u` is the committed signed value and `n_u` is its committed
//! negative magnitude.  The positive commitment is derived, never supplied:
//! `C_plus = C_signed + C_negative`.  Four owners fit exactly in one bounded
//! 65,536-gate T256 circuit, so the release proof contains 258 independently
//! bound generalized-Bulletproof cores.  All cores are canonical-decoded
//! before algebra starts; verification then retains only one circuit at a
//! time.
//!
//! Directly constraining each product to zero is stronger than the retired
//! kappa aggregate and adds no polynomial-batching error.  This stage does not
//! establish that either factor belongs to the 15-bit lookup table, does not
//! verify the inverse commitments, and does not verify comparator digit,
//! q-mask, or global-lookup relations.  Those obligations remain fail-closed.
//! The output is private, move-only, and non-authorizing.
//!
//! These bytes are nested inside the statement-5 residual.  Consequently no
//! statement-5 residual/binding digest, statement-3 residual/binding digest,
//! or inventory continuation/binding digest is admitted to this wire or to a
//! proof transcript: each hashes a container that includes these bytes.  The
//! complete identities are retained only in the post-verification output
//! binding, avoiding a Fiat-Shamir fixed-point cycle.

use core::marker::PhantomData;

use super::{
    rns_native_comparator_range_carry_product::{
        RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1,
        RnsNativeComparatorRangeCarryPrerequisiteV1,
    },
    rns_native_cross_field_inventory::SmallSourceProductCommitmentsV1,
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
const MAGIC_V1: [u8; 4] = *b"ZSP8";
const STATEMENT_V1: u8 = 8;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const REPETITIONS_V1: usize = 5;
const SOURCE_ROLES_V1: usize = 3;
const BLOCKS_PER_RECORD_V1: usize = 8;
const BLOCKS_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize * SOURCE_ROLES_V1 * BLOCKS_PER_RECORD_V1;
const BLOCKS_PER_CORE_V1: usize = 4;
const CORES_V1: usize = BLOCKS_V1 / BLOCKS_PER_CORE_V1;
const COORDINATES_V1: usize = 16_384;
const GATES_PER_COORDINATE_V1: usize = BLOCKS_PER_CORE_V1;
const GATES_V1: usize = COORDINATES_V1 * GATES_PER_COORDINATE_V1;
const PADDED_GATES_V1: usize = 65_536;
const LOG_PADDED_GATES_V1: usize = 16;
const CONSTRAINTS_PER_GATE_V1: usize = 3;
const CONSTRAINTS_V1: usize = GATES_V1 * CONSTRAINTS_PER_GATE_V1;
const COMMITMENTS_PER_BLOCK_V1: usize = 2;
const COMMITMENTS_PER_CORE_V1: usize = BLOCKS_PER_CORE_V1 * COMMITMENTS_PER_BLOCK_V1;
const FIXED_CORE_POINTS_V1: usize = 2 * COMMITMENTS_PER_CORE_V1 + 9;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;

// Fixed prefix through residual length: frame/geometry, eight digests (six
// acyclic predecessor axes plus proof-set and residual), and residual length.
const HEADER_BYTES_V1: usize = 302;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = CORES_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statement=8;owners=1032;coordinates=16384;four-owner-core;owner-major-coordinate-fast-gate-order;commitments-per-owner=(C_plus=C_signed+C_negative,C_negative);constraints-per-gate=(aL=plus-owner-coordinate,aR=negative-owner-coordinate,aO=0);gates=4*16384=65536;constraints=3*65536;no-kappa-aggregate;no-residual-q8";
const FIELD_SOUNDNESS_LANGUAGE_V1: &[u8] = b"T256-is-a-field;direct-product-zero-implies-plus=0-or-negative=0;integer-sign-interpretation-is-deliberately-deferred-until-both-factors-have-global-lookup-membership-in-[0,32768);no-unverified-range-is-promoted";
const REMAINING_BOUNDARY_V1: &[u8] = b"not-yet-verified:comparator-difference-digit-membership-and-linear-radix-relations,small-positive-and-negative-15-bit-membership,small-inverse-lookup,q-mask-radix-and-complement,source-and-packing-same-opening,global-lookup";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZSP8/direct-four-owner/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-small-sign-disjointness.prerequisite";

const SMALL_SIGN_DISJOINTNESS_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true;
const COMPARATOR_RADIX_RELATIONS_VERIFIED_V1: bool = false;
const SMALL_SIGNED_RANGE_AND_INVERSES_VERIFIED_V1: bool = false;
const CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;

const _: () = {
    assert!(BLOCKS_V1 == 1_032);
    assert!(BLOCKS_V1.is_multiple_of(BLOCKS_PER_CORE_V1));
    assert!(CORES_V1 == 258);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(REPETITIONS_V1 == 5);
    assert!(GATES_V1 == 65_536);
    assert!(PADDED_GATES_V1 == 65_536);
    assert!(CONSTRAINTS_V1 == 196_608);
    assert!(COMMITMENTS_PER_CORE_V1 == 8);
    assert!(FIXED_CORE_POINTS_V1 == 25);
    assert!(CORE_POINTS_V1 == 57);
    assert!(CORE_BYTES_V1 == 2_041);
    assert!(RECORD_BYTES_V1 == 2_045);
    assert!(RECORD_SET_BYTES_V1 == 527_610);
    assert!(HEADER_BYTES_V1 == 302);
    assert!(MIN_WIRE_BYTES_V1 == 527_945);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1 == 2_619_526);
    assert!(SMALL_SIGN_DISJOINTNESS_PRODUCT_VERIFIER_IMPLEMENTED_V1);
    assert!(!COMPARATOR_RADIX_RELATIONS_VERIFIED_V1);
    assert!(!SMALL_SIGNED_RANGE_AND_INVERSES_VERIFIED_V1);
    assert!(!CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
};

/// Failure while decoding or verifying small-sign product statement 8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeSmallSignDisjointnessErrorV1 {
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

impl core::fmt::Display for RnsNativeSmallSignDisjointnessErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeSmallSignDisjointnessErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeSmallSignDisjointnessErrorV1 {
    fn from(_: GeneralizedBulletproofErrorV1) -> Self {
        Self::Algebra
    }
}

#[derive(Clone, Copy)]
struct UpstreamBindingV1 {
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    statement3_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement3_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    statement5_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement5_verified_transcript_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeComparatorRangeCarryPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        let statement3 = previous.previous();
        let inventory = statement3.inventory();
        Self {
            prior_context_digest: inventory.prior_context_digest(),
            inventory_root: inventory.inventory_root(),
            statement3_proof_set_root: statement3.proof_set_root(),
            statement3_verified_transcript_root: statement3.verified_transcript_root(),
            statement5_proof_set_root: previous.proof_set_root(),
            statement5_verified_transcript_root: previous.verified_transcript_root(),
        }
    }

    fn is_valid_v1(self) -> bool {
        ![
            self.prior_context_digest,
            self.inventory_root,
            self.statement3_proof_set_root,
            self.statement3_verified_transcript_root,
            self.statement5_proof_set_root,
            self.statement5_verified_transcript_root,
        ]
        .contains(&[0; DIGEST_BYTES_V1])
    }
}

#[derive(Clone, Copy)]
struct SmallSignCoreCommitmentsV1 {
    owners: [SmallSourceProductCommitmentsV1; BLOCKS_PER_CORE_V1],
    points: [Point; COMMITMENTS_PER_CORE_V1],
}

impl SmallSignCoreCommitmentsV1 {
    fn new_v1(
        owners: [SmallSourceProductCommitmentsV1; BLOCKS_PER_CORE_V1],
    ) -> Result<Self, RnsNativeSmallSignDisjointnessErrorV1> {
        for owner in owners {
            if owner.signed.is_identity()
                || owner.negative_magnitude.is_identity()
                || owner.positive.is_identity()
                || owner.signed + owner.negative_magnitude != owner.positive
            {
                return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext);
            }
        }
        let points = core::array::from_fn(|index| {
            let owner = owners[index / COMMITMENTS_PER_BLOCK_V1];
            if index.is_multiple_of(COMMITMENTS_PER_BLOCK_V1) {
                owner.positive
            } else {
                owner.negative_magnitude
            }
        });
        Ok(Self { owners, points })
    }
}

fn core_commitments_v1<F>(
    core: usize,
    commitment_at: &mut F,
) -> Result<SmallSignCoreCommitmentsV1, RnsNativeSmallSignDisjointnessErrorV1>
where
    F: FnMut(usize) -> Option<SmallSourceProductCommitmentsV1>,
{
    if core >= CORES_V1 {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    let first_owner = core
        .checked_mul(BLOCKS_PER_CORE_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let first =
        commitment_at(first_owner).ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext)?;
    let mut owners = [first; BLOCKS_PER_CORE_V1];
    for (local, owner) in owners.iter_mut().enumerate().skip(1) {
        *owner = commitment_at(
            first_owner
                .checked_add(local)
                .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?,
        )
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext)?;
    }
    SmallSignCoreCommitmentsV1::new_v1(owners)
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeSmallSignDisjointnessErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeSmallSignDisjointnessErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeSmallSignDisjointnessErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeSmallSignDisjointnessErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeSmallSignDisjointnessErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeSmallSignDisjointnessErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
        }
        let mut decoder = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidScalar)?;
        }
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct SmallSignProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> SmallSignProofSetViewV1<'a> {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeComparatorRangeCarryPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeSmallSignDisjointnessErrorV1> {
        let inventory = previous.previous().inventory();
        Self::from_components_v1(
            previous.residual(),
            UpstreamBindingV1::from_prerequisite_v1(previous),
            |block| inventory.small_source_product_commitments(block),
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
        commitment_at: F,
    ) -> Result<Self, RnsNativeSmallSignDisjointnessErrorV1>
    where
        F: FnMut(usize) -> Option<SmallSourceProductCommitmentsV1>,
    {
        if bytes.len() > RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != STATEMENT_V1
            || usize::from(decoder.u16()?) != BLOCKS_V1
            || usize::from(decoder.u8()?) != BLOCKS_PER_CORE_V1
            || usize::from(decoder.u16()?) != CORES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != CONSTRAINTS_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
        }
        let upstream = UpstreamBindingV1 {
            prior_context_digest: decoder.array()?,
            inventory_root: decoder.array()?,
            statement3_proof_set_root: decoder.array()?,
            statement3_verified_transcript_root: decoder.array()?,
            statement5_proof_set_root: decoder.array()?,
            statement5_verified_transcript_root: decoder.array()?,
        };
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || upstream.prior_context_digest != expected.prior_context_digest
            || upstream.inventory_root != expected.inventory_root
            || upstream.statement3_proof_set_root != expected.statement3_proof_set_root
            || upstream.statement3_verified_transcript_root
                != expected.statement3_verified_transcript_root
            || upstream.statement5_proof_set_root != expected.statement5_proof_set_root
            || upstream.statement5_verified_transcript_root
                != expected.statement5_verified_transcript_root
            || [proof_set_root, residual_digest].contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader);
        }
        let records = decoder.take(RECORD_SET_BYTES_V1)?;
        for core in 0..CORES_V1 {
            ExactCoreViewV1::parse_v1(record_at_v1(records, core)?)?;
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
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity);
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
        core: usize,
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeSmallSignDisjointnessErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, core)?)
    }
}

fn record_at_v1(
    records: &[u8],
    core: usize,
) -> Result<&[u8], RnsNativeSmallSignDisjointnessErrorV1> {
    if core >= CORES_V1 || records.len() != RECORD_SET_BYTES_V1 {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    let offset = core
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let end = offset
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(offset..end)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry)?;
    if usize::from(u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry)?,
    )) != core
        || usize::from(u16::from_be_bytes(
            record[2..4]
                .try_into()
                .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn absorb_upstream_v1(hash: &mut Keccak256, upstream: UpstreamBindingV1) {
    for digest in [
        upstream.prior_context_digest,
        upstream.inventory_root,
        upstream.statement3_proof_set_root,
        upstream.statement3_verified_transcript_root,
        upstream.statement5_proof_set_root,
        upstream.statement5_verified_transcript_root,
    ] {
        hash.update(&digest);
    }
}

fn circuit_manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CIRCUIT_MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    for value in [
        BLOCKS_V1 as u32,
        BLOCKS_PER_CORE_V1 as u32,
        CORES_V1 as u32,
        COORDINATES_V1 as u32,
        GATES_V1 as u32,
        PADDED_GATES_V1 as u32,
        CONSTRAINTS_V1 as u32,
        COMMITMENTS_PER_CORE_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for language in [
        CIRCUIT_LANGUAGE_V1,
        FIELD_SOUNDNESS_LANGUAGE_V1,
        REMAINING_BOUNDARY_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.finalize()
}

fn absorb_owner_commitments_v1(
    hash: &mut Keccak256,
    owner: usize,
    commitments: SmallSourceProductCommitmentsV1,
) -> Result<(), RnsNativeSmallSignDisjointnessErrorV1> {
    hash.update(
        &u16::try_from(owner)
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for point in [
        commitments.signed,
        commitments.negative_magnitude,
        commitments.positive,
    ] {
        hash.update(&encode_point_v1(point)?);
    }
    Ok(())
}

fn canonical_proof_set_root_v1<F>(
    upstream: UpstreamBindingV1,
    records: &[u8],
    mut commitment_at: F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1>
where
    F: FnMut(usize) -> Option<SmallSourceProductCommitmentsV1>,
{
    if records.len() != RECORD_SET_BYTES_V1 || !upstream.is_valid_v1() {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&circuit_manifest_digest_v1());
    for core in 0..CORES_V1 {
        let commitments = core_commitments_v1(core, &mut commitment_at)?;
        let proof = record_at_v1(records, core)?;
        hash.update(&(core as u16).to_be_bytes());
        let first_owner = core * BLOCKS_PER_CORE_V1;
        for (local, owner) in commitments.owners.into_iter().enumerate() {
            absorb_owner_commitments_v1(&mut hash, first_owner + local, owner)?;
        }
        hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
        hash.update(proof);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1> {
    if residual.is_empty() || !upstream.is_valid_v1() {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity);
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

fn small_sign_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeSmallSignDisjointnessErrorV1> {
    let gates = coordinates
        .checked_mul(BLOCKS_PER_CORE_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let constraint_count = gates
        .checked_mul(CONSTRAINTS_PER_GATE_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    if coordinates == 0 || !padded_gates.is_power_of_two() || gates > padded_gates {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let one = Scalar::one();
    for local in 0..BLOCKS_PER_CORE_V1 {
        let positive_commitment = local * COMMITMENTS_PER_BLOCK_V1;
        let negative_commitment = positive_commitment + 1;
        for coordinate in 0..coordinates {
            let gate = local * coordinates + coordinate;
            constraints.extend([
                LinComb::empty().term(one, Variable::aL(gate)).term(
                    -one,
                    Variable::CG {
                        commitment: positive_commitment,
                        index: coordinate,
                    },
                ),
                LinComb::empty().term(one, Variable::aR(gate)).term(
                    -one,
                    Variable::CG {
                        commitment: negative_commitment,
                        index: coordinate,
                    },
                ),
                LinComb::empty().term(one, Variable::aO(gate)),
            ]);
        }
    }
    if constraints.len() != constraint_count {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_small_sign_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    commitments: SmallSignCoreCommitmentsV1,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeSmallSignDisjointnessErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        small_sign_constraints_v1(coordinates, padded_gates)?,
        commitments.points.to_vec(),
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeSmallSignDisjointnessErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_transcript_state_v1(
    upstream: UpstreamBindingV1,
    core: usize,
    commitments: SmallSignCoreCommitmentsV1,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Vec<u8>, RnsNativeSmallSignDisjointnessErrorV1> {
    let gates = coordinates
        .checked_mul(BLOCKS_PER_CORE_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    let constraints = gates
        .checked_mul(CONSTRAINTS_PER_GATE_V1)
        .ok_or(RnsNativeSmallSignDisjointnessErrorV1::ArithmeticOverflow)?;
    if !upstream.is_valid_v1()
        || core >= CORES_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
        || gates > padded_gates
    {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext);
    }
    let mut state = Vec::with_capacity(2_048);
    for frame in [
        TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1, STATEMENT_V1],
        TRANSCRIPT_SCHEMA_V1,
        upstream.prior_context_digest.as_slice(),
        upstream.inventory_root.as_slice(),
        upstream.statement3_proof_set_root.as_slice(),
        upstream.statement3_verified_transcript_root.as_slice(),
        upstream.statement5_proof_set_root.as_slice(),
        upstream.statement5_verified_transcript_root.as_slice(),
        (core as u16).to_be_bytes().as_slice(),
        (coordinates as u32).to_be_bytes().as_slice(),
        (gates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (constraints as u32).to_be_bytes().as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
    }
    let first_owner = core * BLOCKS_PER_CORE_V1;
    for (local, owner) in commitments.owners.into_iter().enumerate() {
        append_frame_v1(&mut state, &((first_owner + local) as u16).to_be_bytes())?;
        for point in [owner.signed, owner.negative_magnitude, owner.positive] {
            append_frame_v1(&mut state, &encode_point_v1(point)?)?;
        }
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

struct SmallSignVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> SmallSignVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    #[allow(clippy::too_many_arguments)]
    fn new_v1(
        upstream: UpstreamBindingV1,
        core_ordinal: usize,
        commitments: SmallSignCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        generator_basis_digest: [u8; DIGEST_BYTES_V1],
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeSmallSignDisjointnessErrorV1> {
        Ok(Self {
            state: initial_transcript_state_v1(
                upstream,
                core_ordinal,
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for SmallSignVerifierTranscriptV1<'_, S>
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
    previous: &RnsNativeComparatorRangeCarryPrerequisiteV1<'_, '_, S>,
    view: SmallSignProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1> {
    let statement3 = previous.previous();
    let inventory = statement3.inventory();
    let linked = inventory.linked();
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    for digest in [
        previous.residual_digest(),
        previous.binding_digest(),
        statement3.residual_digest(),
        statement3.binding_digest(),
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
                .ok_or(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext)?;
            hash.update(&[limb as u8, repetition as u8]);
            hash.update(&product.to_be_bytes());
            hash.update(&opening_quotient.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only, private evidence that small-sign product statement 8 has been
/// verified after statements 3 and 5.
///
/// The residual is not authenticated as a later proof schema by this token and
/// confers no receipt, release, or authorization capability.
#[allow(
    missing_copy_implementations,
    reason = "the statement-5 owner and unverified residual must advance exactly once"
)]
pub(super) struct RnsNativeSmallSignDisjointnessPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    _previous: RnsNativeComparatorRangeCarryPrerequisiteV1<'source, 'proof, S>,
    _residual: &'proof [u8],
    _proof_set_root: [u8; DIGEST_BYTES_V1],
    _verified_transcript_root: [u8; DIGEST_BYTES_V1],
    _residual_digest: [u8; DIGEST_BYTES_V1],
    _binding_digest: [u8; DIGEST_BYTES_V1],
}

/// Consume statement 5 and verify all 258 bounded statement-8 cores
/// sequentially.
#[allow(
    dead_code,
    reason = "the sound private statement-8 entry awaits comparator linear/range, q-mask, and global-lookup consumers"
)]
pub(super) fn verify_rns_native_small_sign_disjointness_v1<'source, 'proof, S>(
    previous: RnsNativeComparatorRangeCarryPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeSmallSignDisjointnessPrerequisiteV1<'source, 'proof, S>,
    RnsNativeSmallSignDisjointnessErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let upstream = UpstreamBindingV1::from_prerequisite_v1(&previous);
    let view = SmallSignProofSetViewV1::from_prerequisite_v1(&previous)?;
    let inventory = previous.previous().inventory();
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut verified, upstream);
    verified.update(&view.proof_set_root);
    for core in 0..CORES_V1 {
        let mut commitment_at = |block| inventory.small_source_product_commitments(block);
        let commitments = core_commitments_v1(core, &mut commitment_at)?;
        let proof = view.core_v1(core)?;
        let mut transcript = SmallSignVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
            upstream,
            core,
            commitments,
            COORDINATES_V1,
            PADDED_GATES_V1,
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
            proof,
        )?;
        build_small_sign_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
            COORDINATES_V1,
            PADDED_GATES_V1,
            commitments,
        )?
        .verify(&mut transcript)?;
        let transcript_digest = transcript.finish_v1()?;
        verified.update(&(core as u16).to_be_bytes());
        verified.update(&transcript_digest);
    }
    let verified_transcript_root = verified.finalize();
    if verified_transcript_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(&previous, view, verified_transcript_root)?;
    Ok(RnsNativeSmallSignDisjointnessPrerequisiteV1 {
        _previous: previous,
        _residual: view.residual,
        _proof_set_root: view.proof_set_root,
        _verified_transcript_root: verified_transcript_root,
        _residual_digest: view.residual_digest,
        _binding_digest: binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_small_sign_disjointness_product_tests.rs"]
mod tests;
