//! Exact streamed verifier for centering-subtraction statement 4.
//!
//! For every one of 344 groups, this stage consumes the authenticated low
//! radix commitments `D_h`, the comparator commitments `Delta_h`, and the
//! already-boolean borrow commitments `beta_h`, for `h = 0..16`.  It derives
//! 17 commitments and proves the coefficientwise T256 equations
//!
//! `D_0 - Delta_0 + B*beta_0 = K_0`,
//! `D_h - Delta_h - beta_(h-1) + B*beta_h = K_h` for `h = 1..16`,
//!
//! where `B=2^15` and `K=(p_T+1)/2`.  Each group is one C=17 generalized-
//! Bulletproof core, and all cores are borrowed and verified sequentially.
//!
//! Statement 5 already fixes each borrow to a bit.  This stage still proves
//! only field equalities: until the one global lookup proves `D_h` and
//! `Delta_h` are in `[0,B)`, it makes no integer no-wrap, canonical range, or
//! comparator claim.  The global-slot permutation and sole lookup challenge
//! also remain unverified and must exclude every proof/transcript/root axis
//! from this stage.

use core::marker::PhantomData;

use super::{
    rns_native_cross_field_inventory::ComparatorSubtractionCommitmentsV1,
    rns_native_radix_complement_linear_relation::{
        RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1,
        RnsNativeRadixComplementLinearPrerequisiteV1,
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
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZRS4";
const STATEMENT_V1: u8 = 4;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const GROUPS_V1: usize = 344;
const RADIX_LOG2_V1: u8 = 15;
const RADIX_BASE_V1: u64 = 1 << RADIX_LOG2_V1;
const RADIX_LOW_DIGITS_V1: usize = 17;
const BORROWS_V1: usize = RADIX_LOW_DIGITS_V1;
const CORES_V1: usize = GROUPS_V1;
const COORDINATES_V1: usize = 16_384;
const GATES_V1: usize = COORDINATES_V1;
const PADDED_GATES_V1: usize = COORDINATES_V1;
const LOG_PADDED_GATES_V1: usize = 14;
const CONSTRAINTS_PER_CORE_V1: usize = RADIX_LOW_DIGITS_V1 * COORDINATES_V1;
const COMMITMENTS_PER_CORE_V1: usize = RADIX_LOW_DIGITS_V1;
const FIXED_CORE_POINTS_V1: usize = 2 * COMMITMENTS_PER_CORE_V1 + 7;
const IPA_POINTS_V1: usize = 2 * LOG_PADDED_GATES_V1;
const CORE_POINTS_V1: usize = FIXED_CORE_POINTS_V1 + IPA_POINTS_V1;
const CORE_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = CORE_POINTS_V1 * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1;
const RECORD_HEADER_BYTES_V1: usize = 2 + 2;
const RECORD_BYTES_V1: usize = RECORD_HEADER_BYTES_V1 + CORE_BYTES_V1;
const UPSTREAM_DIGESTS_V1: usize = 13;

// Forty-four geometry bytes, thirteen acyclic predecessor/candidate axes, the
// current proof-set and residual digests, and the residual length.
const HEADER_BYTES_V1: usize = 44 + (UPSTREAM_DIGESTS_V1 + 2) * DIGEST_BYTES_V1 + 4;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const RECORD_SET_BYTES_V1: usize = CORES_V1 * RECORD_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + RECORD_SET_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - RECORD_SET_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const CIRCUIT_LANGUAGE_V1: &[u8] = b"statement=4;groups=344;coordinates=16384;B=2^15;K=(pT+1)/2;raw-owner=(D_0..D_16,Delta_0..Delta_16,beta_0..beta_16);derived-E_0=C_D0-C_Delta0+B*C_beta0;derived-E_h=C_Dh-C_Deltah-C_beta(h-1)+B*C_beta_h-for-h=1..16;commitments=E_0..E_16;constraints=for-h=0..16,for-v=0..16383:E_h[v]-K_h=0;padded-gates=16384;no-random-aggregate";
const FIELD_BOUNDARY_LANGUAGE_V1: &[u8] = b"statement5-already-fixes-beta_0..beta_16-in-{0,1};these-are-T256-field-equalities-only;D-and-Delta-digit-membership-in-[0,32768)-not-yet-verified;therefore-no-integer-no-wrap-centering-comparator-or-canonical-range-claim";
const SOLE_Z_ORDER_LANGUAGE_V1: &[u8] = b"future-global-A-slot-order-is-role-major-and-not-yet-authenticated;sole-z-must-exclude-added-inventory-root,S3/S5/S8/S10-11/S2/S4-proof-and-transcript-roots,residuals,bindings,codec-digests,and-all-inverse-commitments";
const REMAINING_BOUNDARY_V1: &[u8] = b"not-yet-verified:radix-digit-membership-and-inverses,difference-digit-membership-and-inverses,integer-no-wrap-centering-order,small-source-membership-and-inverses,q-mask-digit-membership-and-inverses,qPCS-S-same-opening,source-and-packing-same-opening,global-slot-permutation,sole-z,global-lookup";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.transcript";
const TRANSCRIPT_SCHEMA_V1: &[u8] = b"ZRS4/direct-coefficient/transcript/v1";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.challenge";
const CIRCUIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.circuit-manifest";
const PROOF_SET_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.proof-set-root";
const RESIDUAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.codec";
const VERIFIED_TRANSCRIPTS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.verified-transcripts";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-centering-subtraction.prerequisite";

const CENTERING_SUBTRACTION_FIELD_RECURRENCE_VERIFIED_V1: bool = true;
const COMPARATOR_BORROW_BOOLEANITY_PREVIOUSLY_VERIFIED_V1: bool = true;
const SOLE_Z_GLOBAL_SLOT_PERMUTATION_VERIFIED_V1: bool = false;
const SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1: bool = false;
const RADIX_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false;
const DIFFERENCE_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false;
const INTEGER_NO_WRAP_CENTERING_SUBTRACTION_VERIFIED_V1: bool = false;
const CANONICAL_CENTERING_COMPARATOR_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const fn centering_threshold_be_v1() -> [u8; 32] {
    let mut threshold = [0_u8; 32];
    let mut incoming = 0_u8;
    let mut index = 0_usize;
    while index < threshold.len() {
        let byte = VEGA_T256_SCALAR_MODULUS_BE_V1[index];
        threshold[index] = (incoming << 7) | (byte >> 1);
        incoming = byte & 1;
        index += 1;
    }
    let mut carry = 1_u16;
    let mut offset = 0_usize;
    while offset < threshold.len() {
        let index = threshold.len() - 1 - offset;
        let sum = threshold[index] as u16 + carry;
        threshold[index] = sum as u8;
        carry = sum >> 8;
        offset += 1;
    }
    threshold
}

const fn radix_low_digits_be_v1(encoded: [u8; 32]) -> [u16; RADIX_LOW_DIGITS_V1] {
    let mut digits = [0_u16; RADIX_LOW_DIGITS_V1];
    let mut digit = 0_usize;
    while digit < RADIX_LOW_DIGITS_V1 {
        let mut bit = 0_usize;
        while bit < RADIX_LOG2_V1 as usize {
            let absolute = digit * RADIX_LOG2_V1 as usize + bit;
            digits[digit] |= (((encoded[31 - absolute / 8] >> (absolute % 8)) & 1) as u16) << bit;
            bit += 1;
        }
        digit += 1;
    }
    digits
}

const CENTERING_THRESHOLD_BE_V1: [u8; 32] = centering_threshold_be_v1();
const CENTERING_THRESHOLD_DIGITS_V1: [u16; RADIX_LOW_DIGITS_V1] =
    radix_low_digits_be_v1(CENTERING_THRESHOLD_BE_V1);

const _: () = {
    assert!(GROUPS_V1 == 43 * 8);
    assert!(RADIX_BASE_V1 == 32_768);
    assert!(RADIX_LOW_DIGITS_V1 == 17);
    assert!(BORROWS_V1 == 17);
    assert!(CORES_V1 == 344);
    assert!(GATES_V1 == 16_384);
    assert!(PADDED_GATES_V1 == 16_384);
    assert!(CONSTRAINTS_PER_CORE_V1 == 278_528);
    assert!(COMMITMENTS_PER_CORE_V1 == 17);
    assert!(FIXED_CORE_POINTS_V1 == 41);
    assert!(IPA_POINTS_V1 == 28);
    assert!(CORE_POINTS_V1 == 69);
    assert!(CORE_BYTES_V1 == 2_437);
    assert!(RECORD_BYTES_V1 == 2_441);
    assert!(RECORD_SET_BYTES_V1 == 839_704);
    assert!(HEADER_BYTES_V1 == 528);
    assert!(MIN_WIRE_BYTES_V1 == 840_265);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1 == 500_639);
    assert!(CENTERING_SUBTRACTION_FIELD_RECURRENCE_VERIFIED_V1);
    assert!(COMPARATOR_BORROW_BOOLEANITY_PREVIOUSLY_VERIFIED_V1);
    assert!(!SOLE_Z_GLOBAL_SLOT_PERMUTATION_VERIFIED_V1);
    assert!(!SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1);
    assert!(!RADIX_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1);
    assert!(!DIFFERENCE_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1);
    assert!(!INTEGER_NO_WRAP_CENTERING_SUBTRACTION_VERIFIED_V1);
    assert!(!CANONICAL_CENTERING_COMPARATOR_VERIFIED_V1);
    assert!(CENTERING_THRESHOLD_BE_V1[0] >> 7 == 0);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeCenteringSubtractionErrorV1 {
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

impl core::fmt::Display for RnsNativeCenteringSubtractionErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeCenteringSubtractionErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeCenteringSubtractionErrorV1 {
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
    statement2_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement2_verified_transcript_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeRadixComplementLinearPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        let existing_radix = previous.previous();
        let q_mask = existing_radix.previous();
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
            pre_z_candidate_root: existing_radix.pre_z_candidate_root(),
            statement2_proof_set_root: previous.proof_set_root(),
            statement2_verified_transcript_root: previous.verified_transcript_root(),
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
            self.statement2_proof_set_root,
            self.statement2_verified_transcript_root,
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
struct CenteringSubtractionRawCommitmentsV1 {
    difference_digits: [Point; RADIX_LOW_DIGITS_V1],
    centered_difference_digits: [Point; RADIX_LOW_DIGITS_V1],
    borrows: [Point; BORROWS_V1],
}

#[derive(Clone, Copy)]
struct CenteringSubtractionCoreCommitmentsV1 {
    raw: CenteringSubtractionRawCommitmentsV1,
    derived: [Point; COMMITMENTS_PER_CORE_V1],
}

fn raw_commitments_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeRadixComplementLinearPrerequisiteV1<'_, '_, S>,
    group: usize,
) -> Option<CenteringSubtractionRawCommitmentsV1> {
    let existing_radix = previous.previous();
    let q_mask = existing_radix.previous();
    let inventory = q_mask.previous().previous().previous().inventory();
    let existing = existing_radix.existing_radix_commitments(group)?;
    let comparator: ComparatorSubtractionCommitmentsV1 =
        inventory.comparator_subtraction_commitments(group)?;
    Some(CenteringSubtractionRawCommitmentsV1 {
        difference_digits: existing.difference_low,
        centered_difference_digits: comparator.difference_digits,
        borrows: comparator.borrows,
    })
}

fn derived_subtraction_commitment_v1(
    raw: CenteringSubtractionRawCommitmentsV1,
    digit: usize,
) -> Result<Point, RnsNativeCenteringSubtractionErrorV1> {
    if digit >= RADIX_LOW_DIGITS_V1 {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    let minus_one = -Scalar::one();
    let mut result = raw.difference_digits[digit]
        + raw.centered_difference_digits[digit].mul_scalar(minus_one)
        + raw.borrows[digit].mul_scalar(radix);
    if digit != 0 {
        result += raw.borrows[digit - 1].mul_scalar(minus_one);
    }
    Ok(result)
}

impl CenteringSubtractionCoreCommitmentsV1 {
    fn new_v1(
        raw: CenteringSubtractionRawCommitmentsV1,
    ) -> Result<Self, RnsNativeCenteringSubtractionErrorV1> {
        if raw
            .difference_digits
            .into_iter()
            .chain(raw.centered_difference_digits)
            .chain(raw.borrows)
            .any(Point::is_identity)
        {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidPoint);
        }
        let mut derived = [derived_subtraction_commitment_v1(raw, 0)?; COMMITMENTS_PER_CORE_V1];
        for (digit, commitment) in derived.iter_mut().enumerate().skip(1) {
            *commitment = derived_subtraction_commitment_v1(raw, digit)?;
        }
        if derived.into_iter().any(Point::is_identity) {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidPoint);
        }
        Ok(Self { raw, derived })
    }
}

fn core_commitments_v1<F>(
    group: usize,
    commitment_at: &mut F,
) -> Result<CenteringSubtractionCoreCommitmentsV1, RnsNativeCenteringSubtractionErrorV1>
where
    F: FnMut(usize) -> Option<CenteringSubtractionRawCommitmentsV1>,
{
    if group >= GROUPS_V1 {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    CenteringSubtractionCoreCommitmentsV1::new_v1(
        commitment_at(group).ok_or(RnsNativeCenteringSubtractionErrorV1::InvalidContext)?,
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeCenteringSubtractionErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeCenteringSubtractionErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeCenteringSubtractionErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeCenteringSubtractionErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeCenteringSubtractionErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeCenteringSubtractionErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeCenteringSubtractionErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExactCoreViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> ExactCoreViewV1<'a> {
    fn parse_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeCenteringSubtractionErrorV1> {
        if bytes.len() != CORE_BYTES_V1 {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
        }
        let mut cursor = DecoderV1::new(bytes);
        for _ in 0..FIXED_CORE_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(cursor.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidPoint)?;
        }
        for _ in 0..3 {
            let encoded = cursor.array::<SCALAR_BYTES_V1>()?;
            Scalar::from_le_bytes_exact(encoded)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidScalar)?;
        }
        for _ in 0..IPA_POINTS_V1 {
            Point::from_non_identity_wire_bytes_exact(cursor.take(POINT_BYTES_V1)?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidPoint)?;
        }
        for _ in 0..2 {
            let encoded = cursor.array::<SCALAR_BYTES_V1>()?;
            Scalar::from_le_bytes_exact(encoded)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidScalar)?;
        }
        if cursor.cursor != bytes.len() {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
        }
        Ok(Self { bytes })
    }
}

#[derive(Clone, Copy)]
struct CenteringSubtractionProofSetViewV1<'a> {
    records: &'a [u8],
    residual: &'a [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> CenteringSubtractionProofSetViewV1<'a> {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeRadixComplementLinearPrerequisiteV1<'_, 'a, S>,
    ) -> Result<Self, RnsNativeCenteringSubtractionErrorV1> {
        Self::from_components_v1(
            previous.residual(),
            UpstreamBindingV1::from_prerequisite_v1(previous),
            |group| raw_commitments_v1(previous, group),
        )
    }

    fn from_components_v1<F>(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
        mut commitment_at: F,
    ) -> Result<Self, RnsNativeCenteringSubtractionErrorV1>
    where
        F: FnMut(usize) -> Option<CenteringSubtractionRawCommitmentsV1>,
    {
        if bytes.len() > RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeCenteringSubtractionErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.u8()? != STATEMENT_V1
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::from(decoder.u8()?) != RADIX_LOW_DIGITS_V1
            || usize::from(decoder.u8()?) != BORROWS_V1
            || decoder.u8()? != RADIX_LOG2_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != COORDINATES_V1
            || usize::from(decoder.u16()?) != CORES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != CONSTRAINTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
            || usize::from(decoder.u8()?) != LOG_PADDED_GATES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
                != CORE_BYTES_V1
        {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
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
            statement2_proof_set_root: decoder.array()?,
            statement2_verified_transcript_root: decoder.array()?,
        };
        let proof_set_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(RECORD_SET_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
        let mut bound_digests = upstream.digests_v1().to_vec();
        bound_digests.extend([proof_set_root, residual_digest]);
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || residual_len > RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || upstream.digests_v1() != expected.digests_v1()
            || !unique_nonzero_digests_v1(&bound_digests)
        {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidHeader);
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
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidIntegrity);
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
    ) -> Result<ExactCoreViewV1<'a>, RnsNativeCenteringSubtractionErrorV1> {
        ExactCoreViewV1::parse_v1(record_at_v1(self.records, group)?)
    }
}

fn record_at_v1(
    records: &[u8],
    group: usize,
) -> Result<&[u8], RnsNativeCenteringSubtractionErrorV1> {
    if records.len() != RECORD_SET_BYTES_V1 || group >= GROUPS_V1 {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    let start = group
        .checked_mul(RECORD_BYTES_V1)
        .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
    let end = start
        .checked_add(RECORD_BYTES_V1)
        .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
    let record = records
        .get(start..end)
        .ok_or(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry)?;
    if usize::from(u16::from_be_bytes(
        record[..2]
            .try_into()
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidGeometry)?,
    )) != group
        || usize::from(u16::from_be_bytes(
            record[2..4]
                .try_into()
                .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidGeometry)?,
        )) != CORE_BYTES_V1
    {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    Ok(&record[RECORD_HEADER_BYTES_V1..])
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeCenteringSubtractionErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeCenteringSubtractionErrorV1::InvalidPoint)?;
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
        BORROWS_V1 as u32,
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
    hash.update(&CENTERING_THRESHOLD_BE_V1);
    for digit in CENTERING_THRESHOLD_DIGITS_V1 {
        hash.update(&digit.to_be_bytes());
    }
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
    commitments: CenteringSubtractionRawCommitmentsV1,
) -> Result<(), RnsNativeCenteringSubtractionErrorV1> {
    hash.update(
        &u16::try_from(group)
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for (role, points) in [
        (0_u8, commitments.difference_digits),
        (1_u8, commitments.centered_difference_digits),
        (2_u8, commitments.borrows),
    ] {
        hash.update(&[role]);
        for (column, point) in points.into_iter().enumerate() {
            hash.update(&[column as u8]);
            hash.update(&encode_point_v1(point)?);
        }
    }
    Ok(())
}

fn canonical_proof_set_root_v1<F>(
    upstream: UpstreamBindingV1,
    records: &[u8],
    commitment_at: &mut F,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCenteringSubtractionErrorV1>
where
    F: FnMut(usize) -> Option<CenteringSubtractionRawCommitmentsV1>,
{
    if records.len() != RECORD_SET_BYTES_V1 || !upstream.is_valid_v1() {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
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
        for (digit, point) in commitments.derived.into_iter().enumerate() {
            hash.update(&[digit as u8]);
            hash.update(&encode_point_v1(point)?);
        }
        hash.update(&(CORE_BYTES_V1 as u16).to_be_bytes());
        hash.update(proof);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    proof_set_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCenteringSubtractionErrorV1> {
    if residual.is_empty() || !upstream.is_valid_v1() || proof_set_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&proof_set_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidIntegrity);
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

fn centering_subtraction_constraints_v1(
    coordinates: usize,
    padded_gates: usize,
) -> Result<Vec<LinComb<Scalar>>, RnsNativeCenteringSubtractionErrorV1> {
    if coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || padded_gates > COORDINATES_V1
    {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(
            RADIX_LOW_DIGITS_V1
                .checked_mul(coordinates)
                .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?,
        )
        .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?;
    for (digit, threshold) in CENTERING_THRESHOLD_DIGITS_V1.into_iter().enumerate() {
        let threshold = Scalar::from_u64(u64::from(threshold));
        for coordinate in 0..coordinates {
            constraints.push(
                LinComb::empty()
                    .term(
                        Scalar::one(),
                        Variable::CG {
                            commitment: digit,
                            index: coordinate,
                        },
                    )
                    .constant(-threshold),
            );
        }
    }
    if constraints.len()
        != RADIX_LOW_DIGITS_V1
            .checked_mul(coordinates)
            .ok_or(RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
    {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
    }
    Ok(constraints)
}

fn build_centering_subtraction_statement_v1<S>(
    coordinates: usize,
    padded_gates: usize,
    commitments: CenteringSubtractionCoreCommitmentsV1,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeCenteringSubtractionErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        centering_subtraction_constraints_v1(coordinates, padded_gates)?,
        commitments.derived.to_vec(),
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeCenteringSubtractionErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeCenteringSubtractionErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_transcript_state_v1(
    upstream: UpstreamBindingV1,
    group: usize,
    commitments: CenteringSubtractionCoreCommitmentsV1,
    coordinates: usize,
    padded_gates: usize,
    generator_basis_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Vec<u8>, RnsNativeCenteringSubtractionErrorV1> {
    if !upstream.is_valid_v1()
        || group >= GROUPS_V1
        || coordinates == 0
        || !padded_gates.is_power_of_two()
        || coordinates > padded_gates
        || padded_gates > COORDINATES_V1
        || generator_basis_digest == [0; DIGEST_BYTES_V1]
    {
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidContext);
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
        upstream.statement2_proof_set_root.as_slice(),
        upstream.statement2_verified_transcript_root.as_slice(),
        (group as u16).to_be_bytes().as_slice(),
        (coordinates as u32).to_be_bytes().as_slice(),
        (padded_gates as u32).to_be_bytes().as_slice(),
        (CONSTRAINTS_PER_CORE_V1 as u32).to_be_bytes().as_slice(),
        generator_basis_digest.as_slice(),
        circuit_manifest_digest_v1().as_slice(),
    ] {
        append_frame_v1(&mut state, frame)?;
    }
    for (role, points) in [
        (0_u8, commitments.raw.difference_digits),
        (1_u8, commitments.raw.centered_difference_digits),
        (2_u8, commitments.raw.borrows),
    ] {
        append_frame_v1(&mut state, &[role])?;
        for (column, point) in points.into_iter().enumerate() {
            append_frame_v1(&mut state, &[column as u8])?;
            append_frame_v1(&mut state, &encode_point_v1(point)?)?;
        }
    }
    for (digit, point) in commitments.derived.into_iter().enumerate() {
        append_frame_v1(&mut state, &[digit as u8])?;
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

struct CenteringSubtractionVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    core: ExactCoreViewV1<'a>,
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> CenteringSubtractionVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    #[allow(clippy::too_many_arguments)]
    fn new_v1(
        upstream: UpstreamBindingV1,
        group: usize,
        commitments: CenteringSubtractionCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        generator_basis_digest: [u8; DIGEST_BYTES_V1],
        core: ExactCoreViewV1<'a>,
    ) -> Result<Self, RnsNativeCenteringSubtractionErrorV1> {
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCenteringSubtractionErrorV1> {
        if self.cursor != self.core.bytes.len() {
            return Err(RnsNativeCenteringSubtractionErrorV1::InvalidGeometry);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S> VerifierTranscript<S> for CenteringSubtractionVerifierTranscriptV1<'_, S>
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
    previous: &RnsNativeRadixComplementLinearPrerequisiteV1<'_, '_, S>,
    view: CenteringSubtractionProofSetViewV1<'_>,
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCenteringSubtractionErrorV1> {
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    // The statement-2 residual and binding digest contain this complete
    // statement-4 wire, so they are admitted only after exact decoding and
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
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only private evidence that centering-subtraction statement 4 has been
/// verified as a T256 field relation for every coefficient group.
///
/// This is not digit membership, integer no-wrap, a canonical comparator,
/// global lookup, readiness, release, or authorization evidence.
#[allow(
    missing_copy_implementations,
    reason = "the statement-2 owner and unverified downstream residual must advance exactly once"
)]
pub(super) struct RnsNativeCenteringSubtractionPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S>,
    residual: &'proof [u8],
    proof_set_root: [u8; DIGEST_BYTES_V1],
    verified_transcript_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

#[allow(
    dead_code,
    reason = "private accessors await digit membership, sole-z, and global-lookup consumers"
)]
impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>
{
    /// Purpose-forward only the opaque snapshot owned by the exact claimed
    /// direct ancestor of this recursively owned proof chain.
    pub(super) const fn pre_global_lookup_capability_v1(
        &self,
    ) -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1 {
        self.previous()
            .previous()
            .previous()
            .previous()
            .previous()
            .previous()
            .pre_global_lookup_capability_v1()
    }

    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S> {
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

    /// Consume statement-4 evidence and recover its exact statement-2 owner.
    pub(super) fn into_previous_v1(
        self,
    ) -> RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S> {
        self.previous
    }
}

/// Consume statement 2 and verify all 344 statement-4 subtraction cores
/// sequentially.
#[allow(
    dead_code,
    reason = "the private statement-4 entry awaits digit membership, lookup, and sole-z consumers"
)]
pub(super) fn verify_rns_native_centering_subtraction_relation_v1<'source, 'proof, S>(
    previous: RnsNativeRadixComplementLinearPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>,
    RnsNativeCenteringSubtractionErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let upstream = UpstreamBindingV1::from_prerequisite_v1(&previous);
    let view = CenteringSubtractionProofSetViewV1::from_prerequisite_v1(&previous)?;
    let mut verified = Keccak256::new();
    verified.update(VERIFIED_TRANSCRIPTS_DOMAIN_V1);
    verified.update(&[VERSION_V1, STATEMENT_V1]);
    absorb_upstream_v1(&mut verified, upstream);
    verified.update(&view.proof_set_root);
    for group in 0..GROUPS_V1 {
        let commitments = CenteringSubtractionCoreCommitmentsV1::new_v1(
            raw_commitments_v1(&previous, group)
                .ok_or(RnsNativeCenteringSubtractionErrorV1::InvalidContext)?,
        )?;
        let proof = view.core_v1(group)?;
        let mut transcript =
            CenteringSubtractionVerifierTranscriptV1::<ZkAmsT256BulletproofSuiteV1>::new_v1(
                upstream,
                group,
                commitments,
                COORDINATES_V1,
                PADDED_GATES_V1,
                ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
                proof,
            )?;
        build_centering_subtraction_statement_v1::<ZkAmsT256BulletproofSuiteV1>(
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
        return Err(RnsNativeCenteringSubtractionErrorV1::InvalidIntegrity);
    }
    let binding_digest = prerequisite_binding_digest_v1(&previous, view, verified_transcript_root)?;
    Ok(RnsNativeCenteringSubtractionPrerequisiteV1 {
        previous,
        residual: view.residual,
        proof_set_root: view.proof_set_root,
        verified_transcript_root,
        residual_digest: view.residual_digest,
        binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_centering_subtraction_relation_tests.rs"]
mod tests;
