//! Private authenticated numeric/opening rendezvous after the complete public read.
//!
//! This child consumes the whole 3,520-object public-read owner and moves its
//! retained relation schedule through complete qPCS authentication. It retains
//! the resulting FRI-complete stage, rather than prematurely extracting a
//! completed-qPCS lineage, and serves exactly 200 direct numeric destinations
//! in limb-major, repetition-major order.  Construction consumes the exact
//! move-only transcript owner and retains the authenticated equation/limb
//! commitment digests; callers never supply `a/A/B/C0/C1/P~/H~` arrays.
//!
//! This is still source-only and non-authorizing. It owns no direct commitment
//! or quotient opening, implements only the schedule-free numeric cursor split,
//! and exposes no raw-parts, lineage, or schedule-taking transition.

#![allow(
    dead_code,
    reason = "the private source-only rendezvous awaits source-preflight and direct opening owners"
)]

use core::{convert::Infallible, fmt, mem::size_of};

use super::super::super::super::{
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldNumericCursorV1, RnsNativeCrossFieldNumericEvaluationV1,
        RnsNativeCrossFieldQuotientOpeningCursorV1, RnsNativeCrossFieldQuotientOpeningSignV1,
        RnsNativeCrossFieldRlweDirectErrorV1,
    },
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1,
    },
    rns_native_public_polynomial_reader::{
        RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReadReceiptV1,
    },
    rns_native_qpcs_fri_complete::{
        RnsNativeQpcsFriCompleteStageV1, authenticate_rns_native_qpcs_fri_complete_with_schedule_v1,
    },
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
};
use super::{RnsNativeCompletedQpcsSourceReadV2, RnsNativeWholePublicationOwnersV2};
use crate::vega::{VegaT256ScalarV1 as Scalar, sponge::Keccak256};

const VERSION_V2: u8 = 2;
const RECORDS_V2: usize = 43;
const EQUATIONS_V2: usize = 2;
const REPETITIONS_V2: usize = 5;
const RELATIONS_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V2;
const QUERY_OPENINGS_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize;
const QUOTIENT_OPENING_SIGNS_V2: usize = 2;
const QUOTIENT_OPENING_OWNERS_V2: usize = RELATIONS_V2 * QUOTIENT_OPENING_SIGNS_V2;
const QUOTIENT_OPENING_COORDINATES_V2: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / RECORDS_PER_RING_BLOCK_V2;
const RECORDS_PER_RING_BLOCK_V2: usize = 8;
const QUOTIENT_OPENING_BITS_V2: usize = 103;
const QUOTIENT_OPENING_SCALARS_PER_OWNER_V2: usize =
    QUOTIENT_OPENING_COORDINATES_V2 + 1 + QUOTIENT_OPENING_BITS_V2;
const QUOTIENT_OPENING_BYTES_PER_OWNER_V2: usize =
    QUOTIENT_OPENING_SCALARS_PER_OWNER_V2 * size_of::<Scalar>();
const QUOTIENT_OPENING_STREAM_SCALARS_V2: usize =
    QUOTIENT_OPENING_OWNERS_V2 * QUOTIENT_OPENING_SCALARS_PER_OWNER_V2;
const QUOTIENT_OPENING_STREAM_BYTES_V2: usize =
    QUOTIENT_OPENING_STREAM_SCALARS_V2 * size_of::<Scalar>();
const QPCS_PAIR_BYTES_V2: usize = 2 * size_of::<u64>();
const QPCS_EVALUATION_BYTES_V2: usize = RELATIONS_V2 * QPCS_PAIR_BYTES_V2;
const PUBLIC_EVALUATION_BYTES_V2: usize = size_of::<RnsNativePublicPolynomialEvaluationV1>();
const RETAINED_PUBLIC_EVALUATION_BYTES_V2: usize = RELATIONS_V2 * PUBLIC_EVALUATION_BYTES_V2;
const RETAINED_TRANSCRIPT_OWNER_BYTES_V2: usize = size_of::<ZkAmsMkheRnsNativeChallengeSeedsV1>();
const RETAINED_COMMITMENT_DIGEST_BYTES_V2: usize =
    (EQUATIONS_V2 + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1) * 32;
const POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2: usize = RETAINED_PUBLIC_EVALUATION_BYTES_V2
    + RETAINED_TRANSCRIPT_OWNER_BYTES_V2
    + RETAINED_COMMITMENT_DIGEST_BYTES_V2;
const NUMERIC_VALUES_PER_RELATION_V2: usize = 3 + 2 * RECORDS_V2 + 2;
const NUMERIC_DESTINATION_BYTES_V2: usize = size_of::<RnsNativeCrossFieldNumericEvaluationV1>();
const CANONICAL_CHECKS_V2: u64 = (RELATIONS_V2 * NUMERIC_VALUES_PER_RELATION_V2) as u64;
const RING_POWER_SQUARINGS_PER_RELATION_V2: u64 = 17;
const RING_POWER_SQUARINGS_V2: u64 = RELATIONS_V2 as u64 * RING_POWER_SQUARINGS_PER_RELATION_V2;
const FACTOR_PRODUCT_MULTIPLICATIONS_V2: u64 = RELATIONS_V2 as u64;
const MODULAR_MULTIPLICATIONS_V2: u64 = RING_POWER_SQUARINGS_V2 + FACTOR_PRODUCT_MULTIPLICATIONS_V2;
const MODULAR_ADDITIONS_V2: u64 = RELATIONS_V2 as u64;
const POST_AUTHENTICATION_NUMERIC_VALIDATION_WORK_UNITS_V2: u64 =
    CANONICAL_CHECKS_V2 + MODULAR_MULTIPLICATIONS_V2 + MODULAR_ADDITIONS_V2;
const PUBLIC_OBJECTS_V2: u16 = 3_520;
const PUBLIC_CANONICAL_BYTES_V2: u64 = 3_691_001_600;
const PUBLIC_COEFFICIENTS_V2: u64 = 461_373_440;
const PUBLIC_MODULAR_MULTIPLICATIONS_V2: u64 = 2_311_357_600;
const PUBLIC_MODULAR_ADDITIONS_V2: u64 = 2_309_120_000;

const JOINT_BINDING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native.numeric-opening-handoff.joint-binding";
// Domain, version/four u16 geometry fields, lifecycle digest, five receipt
// digests, receipt counters, four relation-schedule axes, seven completed-qPCS
// digests, and the bounded proof length.
const JOINT_BINDING_FIXED_BYTES_V2: usize =
    1 + 4 * 2 + 32 + 5 * 32 + 2 + 4 * 8 + 4 * 32 + 7 * 32 + 8;
const POST_AUTHENTICATION_JOINT_BINDING_HASH_BYTES_V2: usize =
    JOINT_BINDING_DOMAIN_V2.len() + JOINT_BINDING_FIXED_BYTES_V2;
const POST_AUTHENTICATION_LOCAL_WORK_UNITS_V2: u64 =
    POST_AUTHENTICATION_NUMERIC_VALIDATION_WORK_UNITS_V2
        + POST_AUTHENTICATION_JOINT_BINDING_HASH_BYTES_V2 as u64
        + RETAINED_COMMITMENT_DIGEST_BYTES_V2 as u64;
const POST_AUTHENTICATION_LOCAL_RESOURCE_SCOPE_V2: &[u8] = b"post-authentication-local-numeric-rendezvous-only;excludes-existing-qpcs-prefix-and-fri-authentication-work;not-end-to-end-resource-accounting";

/// This tranche settles only the private source contract and numeric checks.
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SOURCE_SETTLED_V2: bool = true;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_QPCS_JOIN_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_QUOTIENT_OPENING_CURSOR_CONTRACT_IMPLEMENTED_V2: bool = true;

/// Every live, downstream, evidence, readiness, and release gate stays closed.
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_LIVE_OWNER_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SOURCE_PREFLIGHT_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_COMPLETED_LINEAGE_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_DIRECT_NUMERIC_SOURCE_INTEGRATED_V2: bool =
    false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_DIRECT_OPENINGS_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SINGLE_OWNER_DIRECT_CHRONOLOGY_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_READINESS_V2: bool = false;
pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_RELEASE_AUTHORIZED_V2: bool = false;

/// Exact additive accounting only for the local numeric rendezvous after the
/// existing qPCS prefix/FRI authenticator returns successfully.
///
/// This ledger excludes all proof decoding, authenticated proof bytes,
/// Merkle/FRI hashing, and field work performed by the existing qPCS verifier.
/// Full qPCS prefix/FRI authentication is existing verifier work excluded from
/// this additive local ledger.
/// Retained payload bytes cover exactly the public-evaluation allocation, moved
/// transcript record, and copied equation/limb digest arrays named below; they
/// are not a whole-process resident-memory claim.  Local work charges one unit
/// for each of the 1,344 commitment-digest bytes copied after authentication.
/// It is not end-to-end qPCS accounting and cannot qualify resource evidence.
pub(super) struct RnsNativeNumericOpeningHandoffPostAuthenticationLocalResourceLedgerV2 {
    pub(super) post_authentication_relations: u16,
    pub(super) post_authentication_retained_public_evaluation_bytes: u32,
    pub(super) post_authentication_retained_transcript_owner_bytes: u32,
    pub(super) post_authentication_retained_commitment_digest_bytes: u16,
    pub(super) post_authentication_retained_payload_bytes: u32,
    pub(super) post_authentication_borrowed_qpcs_evaluation_bytes: u16,
    pub(super) post_authentication_numeric_destination_bytes: u16,
    pub(super) post_authentication_canonical_checks: u32,
    pub(super) post_authentication_ring_power_squarings: u32,
    pub(super) post_authentication_modular_multiplications: u32,
    pub(super) post_authentication_modular_additions: u16,
    pub(super) post_authentication_joint_binding_hash_bytes: u16,
    pub(super) post_authentication_commitment_digest_copy_bytes: u16,
    pub(super) post_authentication_local_work_units: u32,
    pub(super) post_authentication_new_heap_bytes: u8,
    pub(super) post_authentication_new_spool_bytes: u8,
    pub(super) post_authentication_new_wire_bytes: u8,
    pub(super) post_authentication_new_authenticated_io_bytes: u8,
}

pub(super) const RNS_NATIVE_NUMERIC_OPENING_HANDOFF_POST_AUTHENTICATION_LOCAL_RESOURCE_LEDGER_V2:
    RnsNativeNumericOpeningHandoffPostAuthenticationLocalResourceLedgerV2 =
    RnsNativeNumericOpeningHandoffPostAuthenticationLocalResourceLedgerV2 {
        post_authentication_relations: RELATIONS_V2 as u16,
        post_authentication_retained_public_evaluation_bytes: RETAINED_PUBLIC_EVALUATION_BYTES_V2
            as u32,
        post_authentication_retained_transcript_owner_bytes: RETAINED_TRANSCRIPT_OWNER_BYTES_V2
            as u32,
        post_authentication_retained_commitment_digest_bytes: RETAINED_COMMITMENT_DIGEST_BYTES_V2
            as u16,
        post_authentication_retained_payload_bytes: POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2
            as u32,
        post_authentication_borrowed_qpcs_evaluation_bytes: QPCS_EVALUATION_BYTES_V2 as u16,
        post_authentication_numeric_destination_bytes: NUMERIC_DESTINATION_BYTES_V2 as u16,
        post_authentication_canonical_checks: CANONICAL_CHECKS_V2 as u32,
        post_authentication_ring_power_squarings: RING_POWER_SQUARINGS_V2 as u32,
        post_authentication_modular_multiplications: MODULAR_MULTIPLICATIONS_V2 as u32,
        post_authentication_modular_additions: MODULAR_ADDITIONS_V2 as u16,
        post_authentication_joint_binding_hash_bytes:
            POST_AUTHENTICATION_JOINT_BINDING_HASH_BYTES_V2 as u16,
        post_authentication_commitment_digest_copy_bytes: RETAINED_COMMITMENT_DIGEST_BYTES_V2
            as u16,
        post_authentication_local_work_units: POST_AUTHENTICATION_LOCAL_WORK_UNITS_V2 as u32,
        post_authentication_new_heap_bytes: 0,
        post_authentication_new_spool_bytes: 0,
        post_authentication_new_wire_bytes: 0,
        post_authentication_new_authenticated_io_bytes: 0,
    };

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(RECORDS_V2 == 43);
    assert!(EQUATIONS_V2 == 2);
    assert!(REPETITIONS_V2 == 5);
    assert!(RELATIONS_V2 == 200);
    assert!(QUERY_OPENINGS_V2 == 160);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 1 << 17);
    assert!(PUBLIC_EVALUATION_BYTES_V2 == 704);
    assert!(RETAINED_PUBLIC_EVALUATION_BYTES_V2 == 140_800);
    assert!(RETAINED_TRANSCRIPT_OWNER_BYTES_V2 == 5_096);
    assert!(RETAINED_COMMITMENT_DIGEST_BYTES_V2 == 1_344);
    assert!(
        POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2
            == RETAINED_PUBLIC_EVALUATION_BYTES_V2 + RETAINED_TRANSCRIPT_OWNER_BYTES_V2 + 1_344
    );
    assert!(POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2 == 147_240);
    assert!(RETAINED_TRANSCRIPT_OWNER_BYTES_V2 <= u32::MAX as usize);
    assert!(POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2 <= u32::MAX as usize);
    assert!(QPCS_PAIR_BYTES_V2 == 16);
    assert!(QPCS_EVALUATION_BYTES_V2 == 3_200);
    assert!(size_of::<Scalar>() == 32);
    assert!(QUOTIENT_OPENING_SIGNS_V2 == 2);
    assert!(QUOTIENT_OPENING_OWNERS_V2 == 400);
    assert!(QUOTIENT_OPENING_COORDINATES_V2 == 16_384);
    assert!(QUOTIENT_OPENING_SCALARS_PER_OWNER_V2 == 16_488);
    assert!(QUOTIENT_OPENING_BYTES_PER_OWNER_V2 == 527_616);
    assert!(QUOTIENT_OPENING_STREAM_SCALARS_V2 == 6_595_200);
    assert!(QUOTIENT_OPENING_STREAM_BYTES_V2 == 211_046_400);
    assert!(NUMERIC_VALUES_PER_RELATION_V2 == 91);
    assert!(NUMERIC_DESTINATION_BYTES_V2 == 728);
    assert!(CANONICAL_CHECKS_V2 == 18_200);
    assert!(RING_POWER_SQUARINGS_V2 == 3_400);
    assert!(MODULAR_MULTIPLICATIONS_V2 == 3_600);
    assert!(MODULAR_ADDITIONS_V2 == 200);
    assert!(POST_AUTHENTICATION_NUMERIC_VALIDATION_WORK_UNITS_V2 == 22_000);
    assert!(JOINT_BINDING_FIXED_BYTES_V2 == 595);
    assert!(POST_AUTHENTICATION_JOINT_BINDING_HASH_BYTES_V2 == 664);
    assert!(POST_AUTHENTICATION_LOCAL_WORK_UNITS_V2 == 24_008);
    assert!(POST_AUTHENTICATION_LOCAL_RESOURCE_SCOPE_V2.len() == 142);
    assert!(
        (POST_AUTHENTICATION_RETAINED_PAYLOAD_BYTES_V2 as u64
            + QPCS_EVALUATION_BYTES_V2 as u64
            + NUMERIC_DESTINATION_BYTES_V2 as u64)
            < ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1
    );
    assert!(
        (RNS_NATIVE_NUMERIC_OPENING_HANDOFF_POST_AUTHENTICATION_LOCAL_RESOURCE_LEDGER_V2
            .post_authentication_new_spool_bytes as u64)
            < ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1
    );
    assert!(RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SOURCE_SETTLED_V2);
    assert!(RNS_NATIVE_NUMERIC_OPENING_HANDOFF_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_NUMERIC_OPENING_HANDOFF_QPCS_JOIN_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_QUOTIENT_OPENING_CURSOR_CONTRACT_IMPLEMENTED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_LIVE_OWNER_INTEGRATED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SOURCE_PREFLIGHT_INTEGRATED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_COMPLETED_LINEAGE_INTEGRATED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_DIRECT_NUMERIC_SOURCE_INTEGRATED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_DIRECT_OPENINGS_AVAILABLE_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_SINGLE_OWNER_DIRECT_CHRONOLOGY_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_RESOURCE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_READINESS_V2);
    assert!(!RNS_NATIVE_NUMERIC_OPENING_HANDOFF_RELEASE_AUTHORIZED_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeNumericOpeningHandoffErrorV2 {
    InvalidContext,
    InvalidCount,
    InvalidOrder,
    InvalidPoint,
    NonCanonicalResidue,
    ZeroFactor,
    InvalidRelation,
    Authentication,
    ArithmeticOverflow,
    Incomplete,
    Poisoned,
}

impl fmt::Display for RnsNativeNumericOpeningHandoffErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeNumericOpeningHandoffErrorV2 {}

/// Fixed-shape verifier input consuming the sole transcript owner.  Commitment
/// arrays and proof bytes are borrowed only for construction; no numeric
/// evaluation is accepted through this boundary.
pub(super) struct RnsNativeQpcsNumericVerificationInputV2<'digests, 'proof> {
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    equation_commitment_digests: &'digests [[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: &'digests [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    query_opening_digests: &'digests [[u8; 32]; QUERY_OPENINGS_V2],
    proof: &'proof [u8],
}

impl<'digests, 'proof> RnsNativeQpcsNumericVerificationInputV2<'digests, 'proof> {
    pub(super) const fn new_v2(
        transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
        equation_commitment_digests: &'digests [[u8; 32]; EQUATIONS_V2],
        limb_commitment_digests: &'digests [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
        query_opening_digests: &'digests [[u8; 32]; QUERY_OPENINGS_V2],
        proof: &'proof [u8],
    ) -> Self {
        Self {
            transcript,
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
            proof,
        }
    }
}

/// The full direct source cannot become inhabited until every named opening
/// owner and the pre-direct inventory axes have a real production source. In
/// particular, each positive/negative owner must move all 16,384 coordinates,
/// its commitment mask, and its 103-bit quotient owner; no prover opening spool
/// or replay/mask receipt exists in this tranche.
pub(super) enum RnsNativeDirectOpeningOwnersUnavailableV2 {
    Production {
        pre_direct_inventory_axes: Infallible,
        q_mask_s_commitment_owner: Infallible,
        message_radix_commitment_owner: Infallible,
        small_signed_commitment_owner: Infallible,
        small_negative_magnitude_commitment_owner: Infallible,
        comparator_final_borrow_commitment_owner: Infallible,
        positive_quotient_opening_owner: Infallible,
        negative_quotient_opening_owner: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RnsNativeQpcsAuthenticatedPairV2 {
    product: u64,
    opening_quotient: u64,
}

struct RnsNativeRelationCursorV2 {
    next_relation: usize,
    poisoned: bool,
}

impl RnsNativeRelationCursorV2 {
    const fn new_v2() -> Self {
        Self {
            next_relation: 0,
            poisoned: false,
        }
    }

    fn begin_v2(
        &mut self,
        limb: usize,
        repetition: usize,
    ) -> Result<usize, RnsNativeNumericOpeningHandoffErrorV2> {
        if self.poisoned {
            return Err(RnsNativeNumericOpeningHandoffErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.next_relation >= RELATIONS_V2
            || limb != self.next_relation / REPETITIONS_V2
            || repetition != self.next_relation % REPETITIONS_V2
        {
            return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidOrder);
        }
        Ok(self.next_relation)
    }

    fn commit_v2(&mut self) {
        self.next_relation += 1;
        self.poisoned = false;
    }

    const fn is_complete_v2(&self) -> bool {
        !self.poisoned && self.next_relation == RELATIONS_V2
    }
}

/// Private authority boundary for the still-absent 400 secret quotient
/// openings. No production type implements this trait in this tranche.
trait RnsNativeQuotientOpeningAuthorityV2 {
    fn fill_next_quotient_opening_v2(
        &mut self,
        relation_ordinal: usize,
        sign: RnsNativeCrossFieldQuotientOpeningSignV1,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1>;

    /// Irrevocably clear every opening not yet transferred to the caller.
    fn clear_retained_quotient_openings_v2(&mut self);
}

/// Armed borrowed-destination owner. Construction clears all caller slots;
/// an error or unwind clears them again before the borrow is released.
struct RnsNativeQuotientOpeningDestinationGuardV2<'a> {
    values: &'a mut [Scalar],
    commitment_mask: &'a mut Scalar,
    quotient_bits: &'a mut [Scalar],
    armed: bool,
}

impl<'a> RnsNativeQuotientOpeningDestinationGuardV2<'a> {
    fn new_v2(
        values: &'a mut [Scalar],
        commitment_mask: &'a mut Scalar,
        quotient_bits: &'a mut [Scalar],
    ) -> Self {
        let mut guard = Self {
            values,
            commitment_mask,
            quotient_bits,
            armed: true,
        };
        guard.clear_v2();
        guard
    }

    fn clear_v2(&mut self) {
        for value in &mut *self.values {
            value.clear_secret();
        }
        self.commitment_mask.clear_secret();
        for bit in &mut *self.quotient_bits {
            bit.clear_secret();
        }
    }

    fn lengths_v2(&self) -> (usize, usize) {
        (self.values.len(), self.quotient_bits.len())
    }

    fn parts_mut_v2(&mut self) -> (&mut [Scalar], &mut Scalar, &mut [Scalar]) {
        (
            &mut *self.values,
            &mut *self.commitment_mask,
            &mut *self.quotient_bits,
        )
    }

    fn quotient_bits_v2(&self) -> &[Scalar] {
        &*self.quotient_bits
    }

    fn disarm_v2(&mut self) {
        self.armed = false;
    }
}

impl Drop for RnsNativeQuotientOpeningDestinationGuardV2<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.clear_v2();
        }
    }
}

/// Armed authority guard. A provider error or unwind clears every retained
/// opening before control returns to the poisoned cursor.
struct RnsNativeQuotientOpeningAuthorityGuardV2<'a, A: RnsNativeQuotientOpeningAuthorityV2> {
    authority: &'a mut A,
    armed: bool,
}

impl<'a, A: RnsNativeQuotientOpeningAuthorityV2> RnsNativeQuotientOpeningAuthorityGuardV2<'a, A> {
    fn new_v2(authority: &'a mut A) -> Self {
        Self {
            authority,
            armed: true,
        }
    }

    fn fill_v2(
        &mut self,
        relation_ordinal: usize,
        sign: RnsNativeCrossFieldQuotientOpeningSignV1,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.authority.fill_next_quotient_opening_v2(
            relation_ordinal,
            sign,
            values,
            commitment_mask,
            quotient_bits,
        )
    }

    fn disarm_v2(&mut self) {
        self.armed = false;
    }
}

impl<A: RnsNativeQuotientOpeningAuthorityV2> Drop
    for RnsNativeQuotientOpeningAuthorityGuardV2<'_, A>
{
    fn drop(&mut self) {
        if self.armed {
            self.authority.clear_retained_quotient_openings_v2();
        }
    }
}

/// Hardened, schedule-free cursor over a future secret-opening authority.
///
/// Its fields are private, its only constructor is test-only, and its
/// authority trait has no production implementation. The type therefore
/// settles ordering and zeroization without making direct openings available.
#[must_use = "dropping the cursor clears every retained quotient opening"]
struct RnsNativeQuotientOpeningCursorV2<A: RnsNativeQuotientOpeningAuthorityV2> {
    authority: A,
    next_owner: usize,
    poisoned: bool,
}

impl<A: RnsNativeQuotientOpeningAuthorityV2> RnsNativeQuotientOpeningCursorV2<A> {
    #[cfg(test)]
    fn test_fixture_v2(authority: A) -> Self {
        Self {
            authority,
            next_owner: 0,
            poisoned: false,
        }
    }
}

impl<A: RnsNativeQuotientOpeningAuthorityV2> RnsNativeCrossFieldQuotientOpeningCursorV1
    for RnsNativeQuotientOpeningCursorV2<A>
{
    fn take_next_quotient_opening_v1(
        &mut self,
        relation_ordinal: usize,
        sign: RnsNativeCrossFieldQuotientOpeningSignV1,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let mut destination = RnsNativeQuotientOpeningDestinationGuardV2::new_v2(
            values,
            commitment_mask,
            quotient_bits,
        );
        if self.poisoned {
            self.authority.clear_retained_quotient_openings_v2();
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable);
        }
        self.poisoned = true;

        let expected_relation = self.next_owner / QUOTIENT_OPENING_SIGNS_V2;
        let expected_sign = if self.next_owner.is_multiple_of(QUOTIENT_OPENING_SIGNS_V2) {
            RnsNativeCrossFieldQuotientOpeningSignV1::Positive
        } else {
            RnsNativeCrossFieldQuotientOpeningSignV1::Negative
        };
        if self.next_owner >= QUOTIENT_OPENING_OWNERS_V2
            || relation_ordinal != expected_relation
            || sign != expected_sign
            || destination.lengths_v2()
                != (QUOTIENT_OPENING_COORDINATES_V2, QUOTIENT_OPENING_BITS_V2)
        {
            self.authority.clear_retained_quotient_openings_v2();
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }

        let mut authority = RnsNativeQuotientOpeningAuthorityGuardV2::new_v2(&mut self.authority);
        let (values, commitment_mask, quotient_bits) = destination.parts_mut_v2();
        authority.fill_v2(
            relation_ordinal,
            sign,
            values,
            commitment_mask,
            quotient_bits,
        )?;
        if destination
            .quotient_bits_v2()
            .iter()
            .any(|bit| !bit.is_zero() && *bit != Scalar::one())
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidScalar);
        }

        authority.disarm_v2();
        drop(authority);
        destination.disarm_v2();
        self.next_owner += 1;
        self.poisoned = false;
        Ok(())
    }
}

impl<A: RnsNativeQuotientOpeningAuthorityV2> Drop for RnsNativeQuotientOpeningCursorV2<A> {
    fn drop(&mut self) {
        self.authority.clear_retained_quotient_openings_v2();
    }
}

/// Move-only live cursor retaining every public owner and its FRI-complete
/// qPCS stage. The schedule remains private and cannot be used as evidence of
/// a shared sole lineage with the separately constructed claimed relation.
pub(super) struct RnsNativeQpcsNumericOpeningHandoffV2<'proof> {
    owners: RnsNativeWholePublicationOwnersV2,
    evaluations: Box<[RnsNativePublicPolynomialEvaluationV1]>,
    read_receipt: RnsNativePublicPolynomialReadReceiptV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    equation_commitment_digests: [[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    qpcs: RnsNativeQpcsFriCompleteStageV1<'proof>,
    joint_binding_digest: [u8; 32],
    cursor: RnsNativeRelationCursorV2,
}

/// Completed numeric traversal. The qPCS schedule remains quarantined inside
/// the retained FRI stage: the current ownership cycle provides no production
/// source-preflight/direct transition from this owner.
pub(super) struct RnsNativeCompletedQpcsNumericOpeningHandoffV2<'proof> {
    owners: RnsNativeWholePublicationOwnersV2,
    evaluations: Box<[RnsNativePublicPolynomialEvaluationV1]>,
    read_receipt: RnsNativePublicPolynomialReadReceiptV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    equation_commitment_digests: [[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    qpcs: RnsNativeQpcsFriCompleteStageV1<'proof>,
    joint_binding_digest: [u8; 32],
}

impl RnsNativeCompletedQpcsNumericOpeningHandoffV2<'_> {
    pub(super) const fn joint_binding_digest_v2(&self) -> [u8; 32] {
        self.joint_binding_digest
    }

    pub(super) const fn read_receipt_v2(&self) -> &RnsNativePublicPolynomialReadReceiptV1 {
        &self.read_receipt
    }

    pub(super) fn retained_evaluation_count_v2(&self) -> usize {
        self.evaluations.len()
    }

    pub(super) const fn has_relation_schedule_v2(&self) -> bool {
        self.qpcs.has_relation_schedule_v1()
    }
}

fn map_direct_numeric_error_v2(
    error: RnsNativeNumericOpeningHandoffErrorV2,
) -> RnsNativeCrossFieldRlweDirectErrorV1 {
    match error {
        RnsNativeNumericOpeningHandoffErrorV2::ArithmeticOverflow => {
            RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow
        }
        RnsNativeNumericOpeningHandoffErrorV2::InvalidPoint
        | RnsNativeNumericOpeningHandoffErrorV2::NonCanonicalResidue
        | RnsNativeNumericOpeningHandoffErrorV2::ZeroFactor
        | RnsNativeNumericOpeningHandoffErrorV2::InvalidRelation => {
            RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation
        }
        RnsNativeNumericOpeningHandoffErrorV2::InvalidContext
        | RnsNativeNumericOpeningHandoffErrorV2::Authentication => {
            RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext
        }
        RnsNativeNumericOpeningHandoffErrorV2::InvalidCount
        | RnsNativeNumericOpeningHandoffErrorV2::InvalidOrder
        | RnsNativeNumericOpeningHandoffErrorV2::Incomplete
        | RnsNativeNumericOpeningHandoffErrorV2::Poisoned => {
            RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable
        }
    }
}

/// Numeric-only direct cursor implementation for the live handoff. It exposes
/// neither completed-owner parts nor any public-point/quotient-opening source,
/// qPCS schedule, or lineage capability.
/// There is deliberately no numeric/membership join caller: this owner and the
/// claimed relation currently retain separate move-only qPCS schedule and
/// final-transcript owners. A future top-level carrier must resolve that
/// chronology before it may borrow this cursor and call `finish_v2` after
/// direct verification. The cursor itself is not production authority.
impl RnsNativeCrossFieldNumericCursorV1 for RnsNativeQpcsNumericOpeningHandoffV2<'_> {
    fn authoritative_binding_digest_v1(&self) -> [u8; 32] {
        self.transcript.source_binding_digest()
    }

    fn take_numeric_evaluation_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.take_numeric_evaluation_v2(limb, repetition, destination)
            .map_err(map_direct_numeric_error_v2)
    }
}

impl<'proof> RnsNativeQpcsNumericOpeningHandoffV2<'proof> {
    /// Fill one complete direct numeric destination.  The destination is reset
    /// before any fallible work and is assigned only after every value and the
    /// `P~=(a^N+1)H~` equation validate.
    pub(super) fn take_numeric_evaluation_v2(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeNumericOpeningHandoffErrorV2> {
        *destination = RnsNativeCrossFieldNumericEvaluationV1::default();
        let relation = self.cursor.begin_v2(limb, repetition)?;
        let public = *self
            .evaluations
            .get(relation)
            .ok_or(RnsNativeNumericOpeningHandoffErrorV2::Incomplete)?;
        let point = self
            .qpcs
            .relation_schedule_v1()
            .map_err(|_| RnsNativeNumericOpeningHandoffErrorV2::Authentication)?
            .point(limb, repetition)
            .ok_or(RnsNativeNumericOpeningHandoffErrorV2::InvalidPoint)?;
        let pair = decode_qpcs_pair_v2(self.qpcs.evaluations(), limb, repetition)?;
        let numeric = materialize_numeric_evaluation_v2(limb, repetition, point, public, pair)?;
        *destination = numeric;
        self.cursor.commit_v2();
        Ok(())
    }

    pub(super) fn finish_v2(
        self,
    ) -> Result<
        RnsNativeCompletedQpcsNumericOpeningHandoffV2<'proof>,
        RnsNativeNumericOpeningHandoffErrorV2,
    > {
        if !self.cursor.is_complete_v2()
            || self.evaluations.len() != RELATIONS_V2
            || !self.qpcs.has_relation_schedule_v1()
        {
            return Err(RnsNativeNumericOpeningHandoffErrorV2::Incomplete);
        }
        Ok(RnsNativeCompletedQpcsNumericOpeningHandoffV2 {
            owners: self.owners,
            evaluations: self.evaluations,
            read_receipt: self.read_receipt,
            transcript: self.transcript,
            equation_commitment_digests: self.equation_commitment_digests,
            limb_commitment_digests: self.limb_commitment_digests,
            qpcs: self.qpcs,
            joint_binding_digest: self.joint_binding_digest,
        })
    }
}

/// Consume the exact completed public read and authenticate qPCS with its sole
/// schedule.  Any failure destroys all owners together.
pub(super) fn authenticate_rns_native_qpcs_numeric_opening_handoff_v2<'digests, 'proof>(
    source_read: RnsNativeCompletedQpcsSourceReadV2,
    input: RnsNativeQpcsNumericVerificationInputV2<'digests, 'proof>,
) -> Result<RnsNativeQpcsNumericOpeningHandoffV2<'proof>, RnsNativeNumericOpeningHandoffErrorV2> {
    let RnsNativeCompletedQpcsSourceReadV2 {
        owners,
        schedule,
        evaluations,
        read_receipt,
    } = source_read;
    let RnsNativeQpcsNumericVerificationInputV2 {
        transcript,
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
        proof,
    } = input;
    validate_completed_read_shape_v2(&owners, &evaluations, &read_receipt)?;
    let proof_len = proof.len();
    let qpcs = authenticate_rns_native_qpcs_fri_complete_with_schedule_v1(
        &transcript,
        schedule,
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
        proof,
    )
    .map_err(|_| RnsNativeNumericOpeningHandoffErrorV2::Authentication)?;
    let joint_binding_digest =
        joint_binding_digest_v2(&owners, &read_receipt, &qpcs, &transcript, proof_len)?;
    // Copy only the exact arrays accepted by the qPCS verifier.  Query-opening
    // digests remain construction-only and are deliberately not retained.
    let equation_commitment_digests = *equation_commitment_digests;
    let limb_commitment_digests = *limb_commitment_digests;
    Ok(RnsNativeQpcsNumericOpeningHandoffV2 {
        owners,
        evaluations,
        read_receipt,
        transcript,
        equation_commitment_digests,
        limb_commitment_digests,
        qpcs,
        joint_binding_digest,
        cursor: RnsNativeRelationCursorV2::new_v2(),
    })
}

fn validate_completed_read_shape_v2(
    owners: &RnsNativeWholePublicationOwnersV2,
    evaluations: &[RnsNativePublicPolynomialEvaluationV1],
    receipt: &RnsNativePublicPolynomialReadReceiptV1,
) -> Result<(), RnsNativeNumericOpeningHandoffErrorV2> {
    if owners.lifecycle_digest == [0; 32]
        || evaluations.len() != RELATIONS_V2
        || receipt.object_count_v1() != PUBLIC_OBJECTS_V2
        || receipt.canonical_bytes_v1() != PUBLIC_CANONICAL_BYTES_V2
        || receipt.coefficient_count_v1() != PUBLIC_COEFFICIENTS_V2
        || receipt.modular_multiplications_v1() != PUBLIC_MODULAR_MULTIPLICATIONS_V2
        || receipt.modular_additions_v1() != PUBLIC_MODULAR_ADDITIONS_V2
    {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidCount);
    }
    if [
        receipt.manifest_digest_v1(),
        receipt.qpcs_schedule_digest_v1(),
        receipt.provider_identity_v1(),
        receipt.snapshot_identity_v1(),
        receipt.read_set_digest_v1(),
    ]
    .contains(&[0; 32])
    {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::Authentication);
    }
    Ok(())
}

fn joint_binding_digest_v2(
    owners: &RnsNativeWholePublicationOwnersV2,
    receipt: &RnsNativePublicPolynomialReadReceiptV1,
    qpcs: &RnsNativeQpcsFriCompleteStageV1<'_>,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    proof_len: usize,
) -> Result<[u8; 32], RnsNativeNumericOpeningHandoffErrorV2> {
    let schedule = qpcs
        .relation_schedule_v1()
        .map_err(|_| RnsNativeNumericOpeningHandoffErrorV2::Authentication)?;
    if qpcs.evaluations().len() != QPCS_EVALUATION_BYTES_V2
        || qpcs.parameter_digest() != schedule.parameter_digest()
        || qpcs.transcript_digest() != transcript.transcript_digest()
        || schedule.q_mask_s_root() != transcript.q_mask_s_root()
        || schedule.qpcs_pre_relation_transcript_digest()
            != transcript.qpcs_pre_relation_transcript_digest()
        || schedule.relation_seed() != transcript.qpcs_relation_challenge_seed()
    {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidContext);
    }
    let proof_len = u64::try_from(proof_len)
        .map_err(|_| RnsNativeNumericOpeningHandoffErrorV2::ArithmeticOverflow)?;
    let mut hash = Keccak256::new();
    hash.update(JOINT_BINDING_DOMAIN_V2);
    hash.update(&[VERSION_V2]);
    hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
    hash.update(&(REPETITIONS_V2 as u16).to_be_bytes());
    hash.update(&(RECORDS_V2 as u16).to_be_bytes());
    hash.update(&(RELATIONS_V2 as u16).to_be_bytes());
    hash.update(&owners.lifecycle_digest);
    for digest in [
        receipt.manifest_digest_v1(),
        receipt.qpcs_schedule_digest_v1(),
        receipt.provider_identity_v1(),
        receipt.snapshot_identity_v1(),
        receipt.read_set_digest_v1(),
    ] {
        hash.update(&digest);
    }
    hash.update(&receipt.object_count_v1().to_be_bytes());
    hash.update(&receipt.canonical_bytes_v1().to_be_bytes());
    hash.update(&receipt.coefficient_count_v1().to_be_bytes());
    hash.update(&receipt.modular_multiplications_v1().to_be_bytes());
    hash.update(&receipt.modular_additions_v1().to_be_bytes());
    for digest in [
        schedule.parameter_digest(),
        schedule.q_mask_s_root(),
        schedule.qpcs_pre_relation_transcript_digest(),
        schedule.relation_seed(),
        qpcs.parameter_digest(),
        qpcs.transcript_digest(),
        qpcs.query_seed(),
        qpcs.section_binding_digest(),
        qpcs.schedule_digest(),
        qpcs.evaluation_binding_digest(),
        qpcs.residual_digest(),
    ] {
        hash.update(&digest);
    }
    hash.update(&proof_len.to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::Authentication);
    }
    Ok(digest)
}

fn decode_qpcs_pair_v2(
    bytes: &[u8],
    limb: usize,
    repetition: usize,
) -> Result<RnsNativeQpcsAuthenticatedPairV2, RnsNativeNumericOpeningHandoffErrorV2> {
    if bytes.len() != QPCS_EVALUATION_BYTES_V2
        || limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || repetition >= REPETITIONS_V2
    {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidCount);
    }
    let relation = limb
        .checked_mul(REPETITIONS_V2)
        .and_then(|value| value.checked_add(repetition))
        .ok_or(RnsNativeNumericOpeningHandoffErrorV2::ArithmeticOverflow)?;
    let offset = relation
        .checked_mul(QPCS_PAIR_BYTES_V2)
        .ok_or(RnsNativeNumericOpeningHandoffErrorV2::ArithmeticOverflow)?;
    let product = u64::from_be_bytes(
        bytes
            .get(offset..offset + 8)
            .and_then(|value| value.try_into().ok())
            .ok_or(RnsNativeNumericOpeningHandoffErrorV2::InvalidCount)?,
    );
    let opening_quotient = u64::from_be_bytes(
        bytes
            .get(offset + 8..offset + 16)
            .and_then(|value| value.try_into().ok())
            .ok_or(RnsNativeNumericOpeningHandoffErrorV2::InvalidCount)?,
    );
    Ok(RnsNativeQpcsAuthenticatedPairV2 {
        product,
        opening_quotient,
    })
}

fn materialize_numeric_evaluation_v2(
    limb: usize,
    repetition: usize,
    point: u64,
    public: RnsNativePublicPolynomialEvaluationV1,
    pair: RnsNativeQpcsAuthenticatedPairV2,
) -> Result<RnsNativeCrossFieldNumericEvaluationV1, RnsNativeNumericOpeningHandoffErrorV2> {
    let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .get(limb)
        .ok_or(RnsNativeNumericOpeningHandoffErrorV2::InvalidOrder)?;
    if repetition >= REPETITIONS_V2 || point == 0 || point >= modulus {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidPoint);
    }
    if [
        point,
        public.public_a,
        public.public_b,
        pair.product,
        pair.opening_quotient,
    ]
    .iter()
    .chain(public.ciphertext_c0.iter())
    .chain(public.ciphertext_c1.iter())
    .any(|value| *value >= modulus)
    {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::NonCanonicalResidue);
    }
    let point_to_n = pow_ring_degree_v2(point, modulus);
    let factor = mod_add_v2(point_to_n, 1, modulus);
    if factor == 0 {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::ZeroFactor);
    }
    if pair.product != mod_mul_v2(factor, pair.opening_quotient, modulus) {
        return Err(RnsNativeNumericOpeningHandoffErrorV2::InvalidRelation);
    }
    Ok(RnsNativeCrossFieldNumericEvaluationV1 {
        a: point,
        public_a: public.public_a,
        public_b: public.public_b,
        ciphertext_c0: public.ciphertext_c0,
        ciphertext_c1: public.ciphertext_c1,
        qpcs_product: pair.product,
        qpcs_opening_quotient: pair.opening_quotient,
    })
}

fn mod_add_v2(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_mul_v2(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) * u128::from(right)) % u128::from(modulus)) as u64
}

fn pow_ring_degree_v2(mut value: u64, modulus: u64) -> u64 {
    for _ in 0..RING_POWER_SQUARINGS_PER_RELATION_V2 {
        value = mod_mul_v2(value, value, modulus);
    }
    value
}

#[cfg(test)]
#[path = "numeric_opening_handoff_v2_tests.rs"]
mod tests;
