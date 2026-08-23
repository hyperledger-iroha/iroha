//! Exact private q-mask linear relations for the 40-limb replacement.
//!
//! This stage consumes the move-only statement-8 prerequisite and verifies the
//! two q-mask relations whose complete commitment inventory is already
//! available:
//!
//! * statement 10: `S_q + S_q_bar = q - 1`, and
//! * statement 11: `S_q[N - 1] = 0`.
//!
//! Statement 9, `S_q = sum_h 2^(15h) s_qh`, is used only as the exact
//! verifier-side definition of the derived vector commitments.  Binding that
//! definition to the qPCS mask owner remains a later same-opening obligation.
//! Likewise, this proof does not establish 15-bit digit membership or any
//! inverse/global-lookup relation.  Consequently field equalities proved here
//! are not promoted to canonical integer claims.
//!
//! There is one generalized-Bulletproof core per `(limb, repetition)`.  Each
//! core derives eight blockwise complement commitments and one final-block
//! radix commitment from the authenticated inventory, then proves 131,073
//! direct linear constraints over 16,384 coordinates.  Only one core and its
//! constraints are live at a time.
//!
//! These bytes are nested inside the statement-8 residual.  Statement-8 (and
//! earlier) residual/binding digests therefore cannot occur in this wire or
//! its proof transcripts without a Keccak fixed-point cycle.  They are bound
//! only after verification in the private output token.

use core::marker::PhantomData;

use super::{
    rns_native_cross_field_inventory::QMaskLinearCommitmentsV1,
    rns_native_existing_radix_commitment_view::RnsNativeExistingRadixValidationPermitV1,
    rns_native_profile::{ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1},
    rns_native_small_sign_disjointness_product::{
        RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1,
        RnsNativeSmallSignDisjointnessPrerequisiteV1,
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
const MAGIC_V1: [u8; 4] = *b"ZQ11";
const FIRST_STATEMENT_V1: u8 = 10;
const LAST_STATEMENT_V1: u8 = 11;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const REPETITIONS_V1: usize = 5;
const BLOCKS_PER_RELATION_V1: usize = 8;
const RADIX_DIGITS_V1: usize = 4;
const RADIX_LOG2_V1: u8 = 15;
const RADIX_BASE_V1: u64 = 1 << RADIX_LOG2_V1;
const RELATIONS_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1;
const CORES_V1: usize = RELATIONS_V1;
const COORDINATES_V1: usize = 16_384;
const GATES_V1: usize = COORDINATES_V1;
const PADDED_GATES_V1: usize = COORDINATES_V1;
const LOG_PADDED_GATES_V1: usize = 14;
const COMPLEMENT_CONSTRAINTS_PER_CORE_V1: usize = BLOCKS_PER_RELATION_V1 * COORDINATES_V1;
const TOP_ZERO_CONSTRAINTS_PER_CORE_V1: usize = 1;
const CONSTRAINTS_PER_CORE_V1: usize =
    COMPLEMENT_CONSTRAINTS_PER_CORE_V1 + TOP_ZERO_CONSTRAINTS_PER_CORE_V1;
const COMMITMENTS_PER_CORE_V1: usize = BLOCKS_PER_RELATION_V1 + 1;
const TOP_COMMITMENT_V1: usize = BLOCKS_PER_RELATION_V1;
const FIXED_CORE_POINTS_V1: usize = 2 * COMMITMENTS_PER_CORE_V1 + 7;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;

// Fixed prefix through residual length: exact geometry, eight acyclic
// predecessor axes, current proof-set/residual roots, and residual length.
const HEADER_BYTES_V1: usize = 370;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = CORES_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statements=10,11;relations=40*5;owner=((limb*5+repetition)*8+block);coordinates=16384;derived-R_b=sum_h(2^(15h)*(C_s_bh+C_sbar_bh));derived-T=sum_h(2^(15h)*C_s_block7_h);commitments=(R_0..R_7,T);constraints=(for-b=0..7,for-v=0..16383:R_b[v]-(q_l-1)=0;T[16383]=0);one-relation-core;linear-only;no-random-aggregate";
const FIELD_BOUNDARY_LANGUAGE_V1: &[u8] = b"T256-field-equalities-only;statement9-is-verifier-local-radix-definition;qPCS-S-same-opening-not-yet-verified;15-bit-digit-membership-not-yet-verified;therefore-no-integer-complement-or-canonical-q-mask-claim";
const REMAINING_BOUNDARY_V1: &[u8] = b"not-yet-verified:existing-D-and-slack-commitment-owners,digit-membership-and-inverses,D-minus-K-subtraction,radix-reconstruction,canonical-complement,small-source-membership-and-inverses,q-mask-digit-membership-and-inverses,qPCS-S-same-opening,source-and-packing-same-opening,global-lookup";
const TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZQ11/direct-relation/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-q-mask-linear.prerequisite";

const Q_MASK_LINEAR_RELATIONS_VERIFIER_IMPLEMENTED_V1: bool = true;
const Q_MASK_RADIX_SAME_OPENING_VERIFIED_V1: bool = false;
const Q_MASK_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false;
const CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(REPETITIONS_V1 == 5);
    assert!(BLOCKS_PER_RELATION_V1 == 8);
    assert!(RADIX_DIGITS_V1 == 4);
    assert!(RELATIONS_V1 == 200);
    assert!(CORES_V1 == 200);
    assert!(GATES_V1 == 16_384);
    assert!(PADDED_GATES_V1 == 16_384);
    assert!(CONSTRAINTS_PER_CORE_V1 == 131_073);
    assert!(COMMITMENTS_PER_CORE_V1 == 9);
    assert!(FIXED_CORE_POINTS_V1 == 25);
    assert!(IPA_POINTS_V1 == 28);
    assert!(CORE_POINTS_V1 == 53);
    assert!(CORE_BYTES_V1 == 1_909);
    assert!(RECORD_BYTES_V1 == 1_913);
    assert!(RECORD_SET_BYTES_V1 == 382_600);
    assert!(HEADER_BYTES_V1 == 370);
    assert!(MIN_WIRE_BYTES_V1 == 383_003);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1 == 2_204_253);
    assert!(Q_MASK_LINEAR_RELATIONS_VERIFIER_IMPLEMENTED_V1);
    assert!(!Q_MASK_RADIX_SAME_OPENING_VERIFIED_V1);
    assert!(!Q_MASK_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1);
    assert!(!CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

/// Failure while decoding or verifying q-mask linear statements 10 and 11.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeQMaskLinearRelationsErrorV1 {
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

impl core::fmt::Display for RnsNativeQMaskLinearRelationsErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeQMaskLinearRelationsErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeQMaskLinearRelationsErrorV1 {
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
    statement8_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement8_verified_transcript_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeSmallSignDisjointnessPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        let statement5 = previous.previous();
        let statement3 = statement5.previous();
        let inventory = statement3.inventory();
        Self {
            prior_context_digest: inventory.prior_context_digest(),
            inventory_root: inventory.inventory_root(),
            statement3_proof_set_root: statement3.proof_set_root(),
            statement3_verified_transcript_root: statement3.verified_transcript_root(),
            statement5_proof_set_root: statement5.proof_set_root(),
            statement5_verified_transcript_root: statement5.verified_transcript_root(),
            statement8_proof_set_root: previous.proof_set_root(),
            statement8_verified_transcript_root: previous.verified_transcript_root(),
        }
    }

    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 8] {
        [
            self.prior_context_digest,
            self.inventory_root,
            self.statement3_proof_set_root,
            self.statement3_verified_transcript_root,
            self.statement5_proof_set_root,
            self.statement5_verified_transcript_root,
            self.statement8_proof_set_root,
            self.statement8_verified_transcript_root,
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
struct QMaskCoreCommitmentsV1 {
    raw: [QMaskLinearCommitmentsV1; BLOCKS_PER_RELATION_V1],
    derived: [Point; COMMITMENTS_PER_CORE_V1],
    modulus: u64,
}

fn weighted_digits_v1(points: [Point; RADIX_DIGITS_V1]) -> Point {
    let mut result = Point::identity();
    let mut weight = Scalar::one();
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    for point in points {
        result += point.mul_scalar(weight);
        weight *= radix;
    }
    result
}

impl QMaskCoreCommitmentsV1 {
    fn new_v1(
        raw: [QMaskLinearCommitmentsV1; BLOCKS_PER_RELATION_V1],
        modulus: u64,
    ) -> Result<Self, RnsNativeQMaskLinearRelationsErrorV1> {
        if modulus == 0 || modulus >= RADIX_BASE_V1.pow(RADIX_DIGITS_V1 as u32) {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidContext);
        }
        for owner in raw {
            if owner
                .digits
                .into_iter()
                .chain(owner.complement_digits)
                .any(Point::is_identity)
            {
                return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint);
            }
        }
        let first =
            weighted_digits_v1(raw[0].digits) + weighted_digits_v1(raw[0].complement_digits);
        let mut derived = [first; COMMITMENTS_PER_CORE_V1];
        for block in 1..BLOCKS_PER_RELATION_V1 {
            derived[block] = weighted_digits_v1(raw[block].digits)
                + weighted_digits_v1(raw[block].complement_digits);
        }
        derived[TOP_COMMITMENT_V1] = weighted_digits_v1(raw[BLOCKS_PER_RELATION_V1 - 1].digits);
        if derived.into_iter().any(Point::is_identity) {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint);
        }
        Ok(Self {
            raw,
            derived,
            modulus,
        })
    }
}

fn core_commitments_v1<F>(
    relation: usize,
    commitment_at: &mut F,
) -> Result<QMaskCoreCommitmentsV1, RnsNativeQMaskLinearRelationsErrorV1>
where
    F: FnMut(usize) -> Option<QMaskLinearCommitmentsV1>,
{
    if relation >= RELATIONS_V1 {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    let first_owner = relation
        .checked_mul(BLOCKS_PER_RELATION_V1)
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
    let first =
        commitment_at(first_owner).ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidContext)?;
    let mut raw = [first; BLOCKS_PER_RELATION_V1];
    for (block, owner) in raw.iter_mut().enumerate().skip(1) {
        *owner = commitment_at(
            first_owner
                .checked_add(block)
                .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?,
        )
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidContext)?;
    }
    QMaskCoreCommitmentsV1::new_v1(
        raw,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[relation / REPETITIONS_V1],
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeQMaskLinearRelationsErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeQMaskLinearRelationsErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeQMaskLinearRelationsErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeQMaskLinearRelationsErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeQMaskLinearRelationsErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeQMaskLinearRelationsErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
        }
        let mut decoder = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(decoder.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            Scalar::from_le_bytes_exact(decoder.array()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidScalar)?;
        }
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct QMaskLinearProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> QMaskLinearProofSetViewV1<'a> {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeSmallSignDisjointnessPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeQMaskLinearRelationsErrorV1> {
        let inventory = previous.previous().previous().inventory();
        Self::from_components_v1(
            previous.residual(),
            UpstreamBindingV1::from_prerequisite_v1(previous),
            |owner| inventory.q_mask_linear_commitments(owner),
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
        commitment_at: F,
    ) -> Result<Self, RnsNativeQMaskLinearRelationsErrorV1>
    where
        F: FnMut(usize) -> Option<QMaskLinearCommitmentsV1>,
    {
        if bytes.len() > RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != FIRST_STATEMENT_V1
            || decoder.u8()? != LAST_STATEMENT_V1
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || usize::from(decoder.u8()?) != REPETITIONS_V1
            || usize::from(decoder.u8()?) != BLOCKS_PER_RELATION_V1
            || usize::from(decoder.u8()?) != RADIX_DIGITS_V1
            || usize::from(decoder.u16()?) != RELATIONS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::from(decoder.u16()?) != CORES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != CONSTRAINTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
        }
        let upstream = UpstreamBindingV1 {
            prior_context_digest: decoder.array()?,
            inventory_root: decoder.array()?,
            statement3_proof_set_root: decoder.array()?,
            statement3_verified_transcript_root: decoder.array()?,
            statement5_proof_set_root: decoder.array()?,
            statement5_verified_transcript_root: decoder.array()?,
            statement8_proof_set_root: decoder.array()?,
            statement8_verified_transcript_root: decoder.array()?,
        };
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
        let mut bound_digests = upstream.digests_v1().to_vec();
        bound_digests.extend([proof_set_root, residual_digest]);
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || residual_len > RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || upstream.digests_v1() != expected.digests_v1()
            || !unique_nonzero_digests_v1(&bound_digests)
        {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader);
        }
        let records = decoder.take(RECORD_SET_BYTES_V1)?;
        for relation in 0..RELATIONS_V1 {
            ExactCoreViewV1::parse_v1(record_at_v1(records, relation)?)?;
        }
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        bound_digests.push(codec_digest);
        if decoder.cursor != bytes.len()
            || canonical_proof_set_root_v1(upstream, records, commitment_at)? != proof_set_root
            || canonical_residual_digest_v1(upstream, proof_set_root, residual)? != residual_digest
            || !unique_nonzero_digests_v1(&bound_digests)
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity);
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
        relation: usize,
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeQMaskLinearRelationsErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, relation)?)
    }
}

fn record_at_v1(
    records: &[u8],
    relation: usize,
) -> Result<&[u8], RnsNativeQMaskLinearRelationsErrorV1> {
    if relation >= RELATIONS_V1 || records.len() != RECORD_SET_BYTES_V1 {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    let offset = relation
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
    let end = offset
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(offset..end)
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry)?;
    if usize::from(u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry)?,
    )) != relation
        || usize::from(u16::from_be_bytes(
            record[2..4]
                .try_into()
                .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint)?;
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
    hash.update(&[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1]);
    for value in [
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u32,
        REPETITIONS_V1 as u32,
        BLOCKS_PER_RELATION_V1 as u32,
        RADIX_DIGITS_V1 as u32,
        RADIX_LOG2_V1 as u32,
        RELATIONS_V1 as u32,
        COORDINATES_V1 as u32,
        GATES_V1 as u32,
        PADDED_GATES_V1 as u32,
        CONSTRAINTS_PER_CORE_V1 as u32,
        COMMITMENTS_PER_CORE_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
        hash.update(&modulus.to_be_bytes());
    }
    for language in [
        CIRCUIT_LANGUAGE_V1,
        FIELD_BOUNDARY_LANGUAGE_V1,
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
    owner: usize,
    commitments: QMaskLinearCommitmentsV1,
) -> Result<(), RnsNativeQMaskLinearRelationsErrorV1> {
    hash.update(
        &u16::try_from(owner)
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for (role, points) in [
        (0_u8, commitments.digits),
        (1_u8, commitments.complement_digits),
    ] {
        hash.update(&[role]);
        for (digit, point) in points.into_iter().enumerate() {
            hash.update(&[digit as u8]);
            hash.update(&encode_point_v1(point)?);
        }
    }
    Ok(())
}

fn canonical_proof_set_root_v1<F>(
    upstream: UpstreamBindingV1,
    records: &[u8],
    mut commitment_at: F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1>
where
    F: FnMut(usize) -> Option<QMaskLinearCommitmentsV1>,
{
    if records.len() != RECORD_SET_BYTES_V1 || !upstream.is_valid_v1() {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&circuit_manifest_digest_v1());
    for relation in 0..RELATIONS_V1 {
        let commitments = core_commitments_v1(relation, &mut commitment_at)?;
        let proof = record_at_v1(records, relation)?;
        hash.update(&(relation as u16).to_be_bytes());
        hash.update(&[
            (relation / REPETITIONS_V1) as u8,
            (relation % REPETITIONS_V1) as u8,
        ]);
        hash.update(&commitments.modulus.to_be_bytes());
        let first_owner = relation * BLOCKS_PER_RELATION_V1;
        for (block, owner) in commitments.raw.into_iter().enumerate() {
            absorb_raw_owner_v1(&mut hash, first_owner + block, owner)?;
        }
        for (ordinal, point) in commitments.derived.into_iter().enumerate() {
            hash.update(&[ordinal as u8]);
            hash.update(&encode_point_v1(point)?);
        }
        hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
        hash.update(proof);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1> {
    if residual.is_empty() || !upstream.is_valid_v1() {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity);
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

fn q_mask_linear_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
    modulus: u64,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeQMaskLinearRelationsErrorV1> {
    if coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || modulus == 0
        || modulus >= RADIX_BASE_V1.pow(RADIX_DIGITS_V1 as u32)
    {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    let count = BLOCKS_PER_RELATION_V1
        .checked_mul(coordinates)
        .and_then(|value| value.checked_add(1))
        .ok_or(RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(count)
        .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?;
    let one = Scalar::one();
    let expected = Scalar::from_u64(modulus - 1);
    for block in 0..BLOCKS_PER_RELATION_V1 {
        for coordinate in 0..coordinates {
            constraints.push(
                LinComb::empty()
                    .term(
                        one,
                        Variable::CG {
                            commitment: block,
                            index: coordinate,
                        },
                    )
                    .constant(-expected),
            );
        }
    }
    constraints.push(LinComb::empty().term(
        one,
        Variable::CG {
            commitment: TOP_COMMITMENT_V1,
            index: coordinates - 1,
        },
    ));
    if constraints.len() != count {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_q_mask_linear_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    commitments: QMaskCoreCommitmentsV1,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeQMaskLinearRelationsErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        q_mask_linear_constraints_v1(coordinates, padded_gates, commitments.modulus)?,
        commitments.derived.to_vec(),
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeQMaskLinearRelationsErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeQMaskLinearRelationsErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_transcript_state_v1(
    upstream: UpstreamBindingV1,
    relation: usize,
    commitments: QMaskCoreCommitmentsV1,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Vec<u8>, RnsNativeQMaskLinearRelationsErrorV1> {
    if !upstream.is_valid_v1()
        || relation >= RELATIONS_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || commitments.modulus != ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[relation / REPETITIONS_V1]
    {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidContext);
    }
    let mut state = Vec::with_capacity(4_096);
    for frame in [
        TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1],
        TRANSCRIPT_SCHEMA_V1,
        upstream.prior_context_digest.as_slice(),
        upstream.inventory_root.as_slice(),
        upstream.statement3_proof_set_root.as_slice(),
        upstream.statement3_verified_transcript_root.as_slice(),
        upstream.statement5_proof_set_root.as_slice(),
        upstream.statement5_verified_transcript_root.as_slice(),
        upstream.statement8_proof_set_root.as_slice(),
        upstream.statement8_verified_transcript_root.as_slice(),
        (relation as u16).to_be_bytes().as_slice(),
        &[
            (relation / REPETITIONS_V1) as u8,
            (relation % REPETITIONS_V1) as u8,
        ],
        commitments.modulus.to_be_bytes().as_slice(),
        (coordinates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (CONSTRAINTS_PER_CORE_V1 as u32).to_be_bytes().as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
    }
    let first_owner = relation * BLOCKS_PER_RELATION_V1;
    for (block, owner) in commitments.raw.into_iter().enumerate() {
        append_frame_v1(&mut state, &((first_owner + block) as u16).to_be_bytes())?;
        for points in [owner.digits, owner.complement_digits] {
            for point in points {
                append_frame_v1(&mut state, &encode_point_v1(point)?)?;
            }
        }
    }
    for point in commitments.derived {
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

struct QMaskLinearVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> QMaskLinearVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    #[allow(clippy::too_many_arguments)]
    fn new_v1(
        upstream: UpstreamBindingV1,
        relation: usize,
        commitments: QMaskCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        generator_basis_digest: [u8; DIGEST_BYTES_V1],
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeQMaskLinearRelationsErrorV1> {
        Ok(Self {
            state: initial_transcript_state_v1(
                upstream,
                relation,
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for QMaskLinearVerifierTranscriptV1<'_, S>
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
    previous: &RnsNativeSmallSignDisjointnessPrerequisiteV1<'_, '_, S>,
    view: QMaskLinearProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1> {
    let statement5 = previous.previous();
    let statement3 = statement5.previous();
    let inventory = statement3.inventory();
    let linked = inventory.linked();
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    for digest in [
        previous.residual_digest(),
        previous.binding_digest(),
        statement5.residual_digest(),
        statement5.binding_digest(),
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
                .ok_or(RnsNativeQMaskLinearRelationsErrorV1::InvalidContext)?;
            hash.update(&[limb as u8, repetition as u8]);
            hash.update(&product.to_be_bytes());
            hash.update(&opening_quotient.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only, private evidence that q-mask field statements 10 and 11 have
/// been verified after statements 3, 5, and 8.
///
/// This is not evidence of q-mask digit membership, qPCS same-opening, global
/// lookup validity, readiness, release, or authorization.
#[allow(
    missing_copy_implementations,
    reason = "the statement-8 owner and unverified residual must advance exactly once"
)]
pub(super) struct RnsNativeQMaskLinearRelationsPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    _previous: RnsNativeSmallSignDisjointnessPrerequisiteV1<'source, 'proof, S>,
    _residual: &'proof [u8],
    _proof_set_root: [u8; DIGEST_BYTES_V1],
    _verified_transcript_root: [u8; DIGEST_BYTES_V1],
    _residual_digest: [u8; DIGEST_BYTES_V1],
    _binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeSmallSignDisjointnessPrerequisiteV1<'source, 'proof, S> {
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

    /// Consume q-mask linear evidence and recover its exact statement-8
    /// predecessor.
    pub(super) fn into_previous_v1(
        self,
    ) -> RnsNativeSmallSignDisjointnessPrerequisiteV1<'source, 'proof, S> {
        self._previous
    }

    pub(super) fn take_existing_radix_validation_permit_v1(
        &mut self,
    ) -> Option<RnsNativeExistingRadixValidationPermitV1> {
        self._previous.take_existing_radix_validation_permit_v1()
    }
}

/// Consume statement 8 and verify all 200 bounded q-mask linear cores
/// sequentially.
#[allow(
    dead_code,
    reason = "the private q-mask entry awaits digit membership, qPCS same-opening, and global-lookup consumers"
)]
pub(super) fn verify_rns_native_q_mask_linear_relations_v1<'source, 'proof, S>(
    previous: RnsNativeSmallSignDisjointnessPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S>,
    RnsNativeQMaskLinearRelationsErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let upstream = UpstreamBindingV1::from_prerequisite_v1(&previous);
    let view = QMaskLinearProofSetViewV1::from_prerequisite_v1(&previous)?;
    let inventory = previous.previous().previous().inventory();
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, FIRST_STATEMENT_V1, LAST_STATEMENT_V1]);
    absorb_upstream_v1(&mut verified, upstream);
    verified.update(&view.proof_set_root);
    for relation in 0..RELATIONS_V1 {
        let mut commitment_at = |owner| inventory.q_mask_linear_commitments(owner);
        let commitments = core_commitments_v1(relation, &mut commitment_at)?;
        let proof = view.core_v1(relation)?;
        let mut transcript =
            QMaskLinearVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
                upstream,
                relation,
                commitments,
                COORDINATES_V1,
                PADDED_GATES_V1,
                ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
                proof,
            )?;
        build_q_mask_linear_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
            COORDINATES_V1,
            PADDED_GATES_V1,
            commitments,
        )?
        .verify(&mut transcript)?;
        let transcript_digest = transcript.finish_v1()?;
        verified.update(&(relation as u16).to_be_bytes());
        verified.update(&transcript_digest);
    }
    let verified_transcript_root = verified.finalize();
    if verified_transcript_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(&previous, view, verified_transcript_root)?;
    Ok(RnsNativeQMaskLinearRelationsPrerequisiteV1 {
        _previous: previous,
        _residual: view.residual,
        _proof_set_root: view.proof_set_root,
        _verified_transcript_root: verified_transcript_root,
        _residual_digest: view.residual_digest,
        _binding_digest: binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_q_mask_linear_relations_tests.rs"]
mod tests;
