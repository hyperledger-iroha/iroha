//! Exact streamed verifier for existing-radix coefficient statement 2.
//!
//! The preceding transport authenticates the 17 low `D` and `S` commitments
//! for each of 344 groups and aliases their two top commitments from the
//! original inventory.  This stage derives one commitment per group,
//!
//! `R = sum(h=0..16, 2^(15h) * (C_D[h] + C_S[h]))
//!      + 2^255 * (C_bD + C_bS)`,
//!
//! and verifies with one generalized-Bulletproof core that every one of its
//! 16,384 coordinates is `p_T - 1` (equivalently `R[v] + 1 = 0` in T256).
//! All 344 cores are borrowed and verified sequentially.
//!
//! This is only the field-linear statement.  Until the one global lookup
//! proves every radix digit is in `[0, 2^15)`, it does not establish canonical
//! integer decompositions or a canonical complement.  The sole lookup
//! challenge must still be derived only from the candidate commitments in
//! their fixed pre-z order; no proof root, residual, binding, codec digest, or
//! predecessor axis from this stage is eligible for that preimage.

use core::marker::PhantomData;

use super::{
    rns_native_existing_radix_commitment_view::{
        ExistingRadixCommitmentsV1, RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1,
        RnsNativeExistingRadixCommitmentPrerequisiteV1,
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
const MAGIC_V1: [u8; 4] = *b"ZRC2";
const STATEMENT_V1: u8 = 2;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const GROUPS_V1: usize = 344;
const RADIX_LOG2_V1: u8 = 15;
const RADIX_BASE_V1: u64 = 1 << RADIX_LOG2_V1;
const RADIX_LOW_DIGITS_V1: usize = 17;
const RADIX_DIGITS_V1: usize = RADIX_LOW_DIGITS_V1 + 1;
const CORES_V1: usize = GROUPS_V1;
const COORDINATES_V1: usize = 16_384;
const GATES_V1: usize = COORDINATES_V1;
const PADDED_GATES_V1: usize = COORDINATES_V1;
const LOG_PADDED_GATES_V1: usize = 14;
const CONSTRAINTS_PER_CORE_V1: usize = COORDINATES_V1;
const COMMITMENTS_PER_CORE_V1: usize = 1;
const FIXED_CORE_POINTS_V1: usize = 2 * COMMITMENTS_PER_CORE_V1 + 7;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;
const UPSTREAM_DIGESTS_V1: usize = 11;

// Forty-four geometry bytes, eleven acyclic predecessor/candidate axes, the
// current proof-set and residual digests, and the residual length.
const HEADER_BYTES_V1: usize = 44 + (UPSTREAM_DIGESTS_V1 + 2) * DIGEST_BYTES_V1 + 4;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = CORES_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statement=2;groups=344;coordinates=16384;B=2^15;raw-owner=(D_0..D_16,bD,S_0..S_16,bS);derived-R=sum_h=0..16(B^h*(C_D_h+C_S_h))+B^17*(C_bD+C_bS);one-derived-commitment;constraints=for-v=0..16383:R[v]+1=0-in-T256;padded-gates=16384;no-random-aggregate";
const FIELD_BOUNDARY_LANGUAGE_V1: &[u8] = b"T256-field-equality-only;digit-membership-in-[0,32768)-not-yet-verified;therefore-D-and-S-canonical-integer-reconstruction-and-canonical-complement-are-not-yet-claimed";
const SOLE_Z_ORDER_LANGUAGE_V1: &[u8] = b"existing-radix-transport-order-is-group-major:D[g,0..16],S[g,0..16];future-global-A-slot-order-is-role-major:D[17g+h],S[5848+17g+h];consumer-must-authenticate-this-permutation-before-deriving-the-sole-z;exclude-added-inventory-root,S3/S5/S8/S10-11-roots,this-proof-root,all-transcript-roots,residuals,bindings,codec-digests,and-all-inverse-commitments";
const REMAINING_BOUNDARY_V1: &[u8] = b"not-yet-verified:radix-digit-membership-and-inverses,D-minus-K-subtraction,difference-digit-membership,small-source-membership-and-inverses,q-mask-digit-membership-and-inverses,qPCS-S-same-opening,source-and-packing-same-opening,sole-z,global-lookup";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZRC2/direct-coefficient/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-radix-complement-linear.prerequisite";

const RADIX_COMPLEMENT_FIELD_RELATION_VERIFIED_V1: bool = true;
const EXISTING_RADIX_TRANSPORT_ORDER_AUTHENTICATED_V1: bool = true;
const SOLE_Z_GLOBAL_SLOT_PERMUTATION_VERIFIED_V1: bool = false;
const SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1: bool = false;
const RADIX_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false;
const CANONICAL_RADIX_RECONSTRUCTION_VERIFIED_V1: bool = false;
const CANONICAL_RADIX_COMPLEMENT_VERIFIED_V1: bool = false;
const CENTERING_SUBTRACTION_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(GROUPS_V1 == 43 * 8);
    assert!(RADIX_BASE_V1 == 32_768);
    assert!(RADIX_LOW_DIGITS_V1 == 17);
    assert!(RADIX_DIGITS_V1 == 18);
    assert!(CORES_V1 == 344);
    assert!(GATES_V1 == 16_384);
    assert!(PADDED_GATES_V1 == 16_384);
    assert!(CONSTRAINTS_PER_CORE_V1 == 16_384);
    assert!(COMMITMENTS_PER_CORE_V1 == 1);
    assert!(FIXED_CORE_POINTS_V1 == 9);
    assert!(IPA_POINTS_V1 == 28);
    assert!(CORE_POINTS_V1 == 37);
    assert!(CORE_BYTES_V1 == 1_381);
    assert!(RECORD_BYTES_V1 == 1_385);
    assert!(RECORD_SET_BYTES_V1 == 476_440);
    assert!(HEADER_BYTES_V1 == 464);
    assert!(MIN_WIRE_BYTES_V1 == 476_937);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1 == 1_340_903);
    assert!(RADIX_COMPLEMENT_FIELD_RELATION_VERIFIED_V1);
    assert!(EXISTING_RADIX_TRANSPORT_ORDER_AUTHENTICATED_V1);
    assert!(!SOLE_Z_GLOBAL_SLOT_PERMUTATION_VERIFIED_V1);
    assert!(!SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1);
    assert!(!RADIX_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1);
    assert!(!CANONICAL_RADIX_RECONSTRUCTION_VERIFIED_V1);
    assert!(!CANONICAL_RADIX_COMPLEMENT_VERIFIED_V1);
    assert!(!CENTERING_SUBTRACTION_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeRadixComplementLinearErrorV1 {
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

impl core::fmt::Display for RnsNativeRadixComplementLinearErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeRadixComplementLinearErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeRadixComplementLinearErrorV1 {
    fn from(_: GeneralizedBulletproofErrorV1) -> Self {
        Self::Algebra
    }
}

#[derive(Clone, Copy)]
struct UpstreamBindingV1 {
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    added_inventory_root: [u8; DIGEST_BYTES_V1],
    statement3_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement3_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    statement5_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement5_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    statement8_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement8_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    q_mask_proof_set_root: [u8; DIGEST_BYTES_V1],
    q_mask_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeExistingRadixCommitmentPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        let q_mask = previous.previous();
        let statement8 = q_mask.previous();
        let statement5 = statement8.previous();
        let statement3 = statement5.previous();
        let inventory = statement3.inventory();
        Self {
            prior_context_digest: inventory.prior_context_digest(),
            added_inventory_root: inventory.inventory_root(),
            statement3_proof_set_root: statement3.proof_set_root(),
            statement3_verified_transcript_root: statement3.verified_transcript_root(),
            statement5_proof_set_root: statement5.proof_set_root(),
            statement5_verified_transcript_root: statement5.verified_transcript_root(),
            statement8_proof_set_root: statement8.proof_set_root(),
            statement8_verified_transcript_root: statement8.verified_transcript_root(),
            q_mask_proof_set_root: q_mask.proof_set_root(),
            q_mask_verified_transcript_root: q_mask.verified_transcript_root(),
            pre_z_candidate_root: previous.pre_z_candidate_root(),
        }
    }

    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; UPSTREAM_DIGESTS_V1] {
        [
            self.prior_context_digest,
            self.added_inventory_root,
            self.statement3_proof_set_root,
            self.statement3_verified_transcript_root,
            self.statement5_proof_set_root,
            self.statement5_verified_transcript_root,
            self.statement8_proof_set_root,
            self.statement8_verified_transcript_root,
            self.q_mask_proof_set_root,
            self.q_mask_verified_transcript_root,
            self.pre_z_candidate_root,
        ]
    }

    fn is_valid_v1(self) -> bool {
        unique_nonzero_digests_v1(&self.digests_v1())
    }
}

fn unique_nonzero_digests_v1(digests: &[[u8; DIGEST_BYTES_V1]]) -> bool {
    for (ordinal, digest) in digests.iter().enumerate() {
        if *digest == [0; DIGEST_BYTES_V1] || digests[..ordinal].contains(digest) {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
struct RadixComplementCoreCommitmentsV1 {
    raw: ExistingRadixCommitmentsV1,
    derived: [Point; COMMITMENTS_PER_CORE_V1],
}

fn weighted_radix_commitment_v1(low: [Point; RADIX_LOW_DIGITS_V1], top: Point) -> Point {
    let mut result = Point::identity();
    let mut weight = Scalar::one();
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    for point in low {
        result += point.mul_scalar(weight);
        weight *= radix;
    }
    result + top.mul_scalar(weight)
}

impl RadixComplementCoreCommitmentsV1 {
    fn new_v1(
        raw: ExistingRadixCommitmentsV1,
    ) -> Result<Self, RnsNativeRadixComplementLinearErrorV1> {
        if raw
            .difference_low
            .into_iter()
            .chain(raw.slack_low)
            .chain([raw.difference_top, raw.slack_top])
            .any(Point::is_identity)
        {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidPoint);
        }
        let reconstructed_difference =
            weighted_radix_commitment_v1(raw.difference_low, raw.difference_top);
        let reconstructed_slack = weighted_radix_commitment_v1(raw.slack_low, raw.slack_top);
        let derived = [reconstructed_difference + reconstructed_slack];
        if derived[0].is_identity() {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidPoint);
        }
        Ok(Self { raw, derived })
    }
}

fn core_commitments_v1<F>(
    group: usize,
    commitment_at: &mut F,
) -> Result<RadixComplementCoreCommitmentsV1, RnsNativeRadixComplementLinearErrorV1>
where
    F: FnMut(usize) -> Option<ExistingRadixCommitmentsV1>,
{
    if group >= GROUPS_V1 {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    RadixComplementCoreCommitmentsV1::new_v1(
        commitment_at(group).ok_or(RnsNativeRadixComplementLinearErrorV1::InvalidContext)?,
    )
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeRadixComplementLinearErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeRadixComplementLinearErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeRadixComplementLinearErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeRadixComplementLinearErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeRadixComplementLinearErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeRadixComplementLinearErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeRadixComplementLinearErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeRadixComplementLinearErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
        }
        let mut cursor = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(cursor.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            let encoded = cursor.array::<SCALAR_BYTES_V1>()?;
            Scalar::from_le_bytes_exact(encoded)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(cursor.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            let encoded = cursor.array::<SCALAR_BYTES_V1>()?;
            Scalar::from_le_bytes_exact(encoded)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidScalar)?;
        }
        if cursor.cursor != bytes.len() {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct RadixComplementProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> RadixComplementProofSetViewV1<'a> {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeExistingRadixCommitmentPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeRadixComplementLinearErrorV1> {
        Self::from_components_v1(
            previous.residual(),
            UpstreamBindingV1::from_prerequisite_v1(previous),
            |group| previous.existing_radix_commitments(group),
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
        mut commitment_at: F,
    ) -> Result<Self, RnsNativeRadixComplementLinearErrorV1>
    where
        F: FnMut(usize) -> Option<ExistingRadixCommitmentsV1>,
    {
        if bytes.len() > RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeRadixComplementLinearErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != STATEMENT_V1
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::from(decoder.u8()?) != RADIX_LOW_DIGITS_V1
            || usize::from(decoder.u8()?) != RADIX_DIGITS_V1
            || decoder.u8()? != RADIX_LOG2_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::from(decoder.u16()?) != CORES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != CONSTRAINTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
        }
        let upstream = UpstreamBindingV1 {
            prior_context_digest: decoder.array()?,
            added_inventory_root: decoder.array()?,
            statement3_proof_set_root: decoder.array()?,
            statement3_verified_transcript_root: decoder.array()?,
            statement5_proof_set_root: decoder.array()?,
            statement5_verified_transcript_root: decoder.array()?,
            statement8_proof_set_root: decoder.array()?,
            statement8_verified_transcript_root: decoder.array()?,
            q_mask_proof_set_root: decoder.array()?,
            q_mask_verified_transcript_root: decoder.array()?,
            pre_z_candidate_root: decoder.array()?,
        };
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
        let mut bound_digests = upstream.digests_v1().to_vec();
        bound_digests.extend([proof_set_root, residual_digest]);
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || residual_len > RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || upstream.digests_v1() != expected.digests_v1()
            || !unique_nonzero_digests_v1(&bound_digests)
        {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidHeader);
        }
        let records = decoder.take(RECORD_SET_BYTES_V1)?;
        for group in 0..GROUPS_V1 {
            ExactCoreViewV1::parse_v1(record_at_v1(records, group)?)?;
        }
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        bound_digests.push(codec_digest);
        if decoder.cursor != bytes.len()
            || canonical_proof_set_root_v1(upstream, records, &mut commitment_at)? != proof_set_root
            || canonical_residual_digest_v1(upstream, proof_set_root, residual)? != residual_digest
            || !unique_nonzero_digests_v1(&bound_digests)
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity);
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
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeRadixComplementLinearErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, group)?)
    }
}

fn record_at_v1(
    records: &[u8],
    group: usize,
) -> Result<&[u8], RnsNativeRadixComplementLinearErrorV1> {
    if records.len() != RECORD_SET_BYTES_V1 || group >= GROUPS_V1 {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    let start = group
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
    let end = start
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(start..end)
        .ok_or(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry)?;
    if usize::from(u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidGeometry)?,
    )) != group
        || usize::from(u16::from_be_bytes(
            record[2..4]
                .try_into()
                .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeRadixComplementLinearErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeRadixComplementLinearErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn absorb_upstream_v1(hash: &mut Keccak256, upstream: UpstreamBindingV1) {
    for digest in upstream.digests_v1() {
        hash.update(&digest);
    }
}

fn circuit_manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CIRCUIT_MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    for value in [
        GROUPS_V1 as u32,
        RADIX_LOW_DIGITS_V1 as u32,
        RADIX_DIGITS_V1 as u32,
        RADIX_LOG2_V1 as u32,
        COORDINATES_V1 as u32,
        GATES_V1 as u32,
        PADDED_GATES_V1 as u32,
        CONSTRAINTS_PER_CORE_V1 as u32,
        COMMITMENTS_PER_CORE_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for language in [
        CIRCUIT_LANGUAGE_V1,
        FIELD_BOUNDARY_LANGUAGE_V1,
        SOLE_Z_ORDER_LANGUAGE_V1,
        REMAINING_BOUNDARY_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.finalize()
}

fn absorb_raw_owner_v1(
    hash: &mut Keccak256,
    group: usize,
    commitments: ExistingRadixCommitmentsV1,
) -> Result<(), RnsNativeRadixComplementLinearErrorV1> {
    hash.update(
        &u16::try_from(group)
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for (role, low, top) in [
        (0_u8, commitments.difference_low, commitments.difference_top),
        (1_u8, commitments.slack_low, commitments.slack_top),
    ] {
        hash.update(&[role]);
        for (column, point) in low.into_iter().enumerate() {
            hash.update(&[column as u8]);
            hash.update(&encode_point_v1(point)?);
        }
        hash.update(&[RADIX_LOW_DIGITS_V1 as u8]);
        hash.update(&encode_point_v1(top)?);
    }
    Ok(())
}

fn canonical_proof_set_root_v1<F>(
    upstream: UpstreamBindingV1,
    records: &[u8],
    commitment_at: &mut F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRadixComplementLinearErrorV1>
where
    F: FnMut(usize) -> Option<ExistingRadixCommitmentsV1>,
{
    if records.len() != RECORD_SET_BYTES_V1 || !upstream.is_valid_v1() {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&circuit_manifest_digest_v1());
    for group in 0..GROUPS_V1 {
        let commitments = core_commitments_v1(group, commitment_at)?;
        let proof = record_at_v1(records, group)?;
        absorb_raw_owner_v1(&mut hash, group, commitments.raw)?;
        hash.update(&encode_point_v1(commitments.derived[0])?);
        hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
        hash.update(proof);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRadixComplementLinearErrorV1> {
    if residual.is_empty() || !upstream.is_valid_v1() || proof_set_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity);
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

fn radix_complement_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeRadixComplementLinearErrorV1> {
    if coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || padded_gates > COORDINATES_V1
    {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(coordinates)
        .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?;
    for coordinate in 0..coordinates {
        constraints.push(
            LinComb::empty()
                .term(
                    Scalar::one(),
                    Variable::CG {
                        commitment: 0,
                        index: coordinate,
                    },
                )
                .constant(Scalar::one()),
        );
    }
    if constraints.len() != coordinates {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_radix_complement_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    commitments: RadixComplementCoreCommitmentsV1,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeRadixComplementLinearErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        radix_complement_constraints_v1(coordinates, padded_gates)?,
        commitments.derived.to_vec(),
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeRadixComplementLinearErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeRadixComplementLinearErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_transcript_state_v1(
    upstream: UpstreamBindingV1,
    group: usize,
    commitments: RadixComplementCoreCommitmentsV1,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Vec<u8>, RnsNativeRadixComplementLinearErrorV1> {
    if !upstream.is_valid_v1()
        || group >= GROUPS_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || padded_gates > COORDINATES_V1
        || generator_basis_digest == [0; DIGEST_BYTES_V1]
    {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidContext);
    }
    let mut state = Vec::with_capacity(3_072);
    for frame in [
        TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1, STATEMENT_V1],
        TRANSCRIPT_SCHEMA_V1,
        upstream.prior_context_digest.as_slice(),
        upstream.added_inventory_root.as_slice(),
        upstream.statement3_proof_set_root.as_slice(),
        upstream.statement3_verified_transcript_root.as_slice(),
        upstream.statement5_proof_set_root.as_slice(),
        upstream.statement5_verified_transcript_root.as_slice(),
        upstream.statement8_proof_set_root.as_slice(),
        upstream.statement8_verified_transcript_root.as_slice(),
        upstream.q_mask_proof_set_root.as_slice(),
        upstream.q_mask_verified_transcript_root.as_slice(),
        upstream.pre_z_candidate_root.as_slice(),
        (group as u16).to_be_bytes().as_slice(),
        (coordinates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (CONSTRAINTS_PER_CORE_V1 as u32).to_be_bytes().as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
    }
    for (role, low, top) in [
        (
            0_u8,
            commitments.raw.difference_low,
            commitments.raw.difference_top,
        ),
        (1_u8, commitments.raw.slack_low, commitments.raw.slack_top),
    ] {
        append_frame_v1(&mut state, &[role])?;
        for (column, point) in low.into_iter().enumerate() {
            append_frame_v1(&mut state, &[column as u8])?;
            append_frame_v1(&mut state, &encode_point_v1(point)?)?;
        }
        append_frame_v1(&mut state, &[RADIX_LOW_DIGITS_V1 as u8])?;
        append_frame_v1(&mut state, &encode_point_v1(top)?)?;
    }
    append_frame_v1(&mut state, &encode_point_v1(commitments.derived[0])?)?;
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

struct RadixComplementVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> RadixComplementVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    #[allow(clippy::too_many_arguments)]
    fn new_v1(
        upstream: UpstreamBindingV1,
        group: usize,
        commitments: RadixComplementCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        generator_basis_digest: [u8; DIGEST_BYTES_V1],
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeRadixComplementLinearErrorV1> {
        Ok(Self {
            state: initial_transcript_state_v1(
                upstream,
                group,
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRadixComplementLinearErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for RadixComplementVerifierTranscriptV1<'_, S>
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
    previous: &RnsNativeExistingRadixCommitmentPrerequisiteV1<'_, '_, S>,
    view: RadixComplementProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRadixComplementLinearErrorV1> {
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    // The existing-radix residual and binding digest contain this complete
    // statement-2 wire, so they are admitted only after exact decoding and
    // verification.  They never enter the header, proof root, or transcript.
    for digest in [
        previous.residual_digest(),
        previous.binding_digest(),
        view.proof_set_root,
        verified_transcript_root,
        view.residual_digest,
        view.codec_digest,
        circuit_manifest_digest_v1(),
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    ] {
        hash.update(&digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only private evidence that the radix-complement field relation in
/// statement 2 has been verified for every coefficient group.
///
/// This is not digit membership, canonical reconstruction, subtraction,
/// global lookup, readiness, release, or authorization evidence.
#[allow(
    missing_copy_implementations,
    reason = "the existing-radix owner and unverified downstream residual must advance exactly once"
)]
pub(super) struct RnsNativeRadixComplementLinearPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: RnsNativeExistingRadixCommitmentPrerequisiteV1<'source, 'proof, S>,
    residual: &'proof [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeExistingRadixCommitmentPrerequisiteV1<'source, 'proof, S> {
        &self.previous
    }

    pub(super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    pub(super) const fn proof_set_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.proof_set_root
    }

    pub(super) const fn verified_transcript_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.verified_transcript_root
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }
}

/// Consume the existing-radix view and verify all 344 statement-2 cores
/// sequentially.
#[allow(
    dead_code,
    reason = "the private statement-2 entry awaits subtraction, lookup, and sole-z consumers"
)]
pub(super) fn verify_rns_native_radix_complement_linear_relation_v1<'source, 'proof, S>(
    previous: RnsNativeExistingRadixCommitmentPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S>,
    RnsNativeRadixComplementLinearErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let upstream = UpstreamBindingV1::from_prerequisite_v1(&previous);
    let view = RadixComplementProofSetViewV1::from_prerequisite_v1(&previous)?;
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut verified, upstream);
    verified.update(&view.proof_set_root);
    for group in 0..GROUPS_V1 {
        let commitments = RadixComplementCoreCommitmentsV1::new_v1(
            previous
                .existing_radix_commitments(group)
                .ok_or(RnsNativeRadixComplementLinearErrorV1::InvalidContext)?,
        )?;
        let proof = view.core_v1(group)?;
        let mut transcript =
            RadixComplementVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
                upstream,
                group,
                commitments,
                COORDINATES_V1,
                PADDED_GATES_V1,
                ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
                proof,
            )?;
        build_radix_complement_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
            COORDINATES_V1,
            PADDED_GATES_V1,
            commitments,
        )?
        .verify(&mut transcript)?;
        let transcript_digest = transcript.finish_v1()?;
        verified.update(&(group as u16).to_be_bytes());
        verified.update(&transcript_digest);
    }
    let verified_transcript_root = verified.finalize();
    if verified_transcript_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(&previous, view, verified_transcript_root)?;
    Ok(RnsNativeRadixComplementLinearPrerequisiteV1 {
        previous,
        residual: view.residual,
        proof_set_root: view.proof_set_root,
        verified_transcript_root,
        residual_digest: view.residual_digest,
        binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_radix_complement_linear_relation_tests.rs"]
mod tests;
