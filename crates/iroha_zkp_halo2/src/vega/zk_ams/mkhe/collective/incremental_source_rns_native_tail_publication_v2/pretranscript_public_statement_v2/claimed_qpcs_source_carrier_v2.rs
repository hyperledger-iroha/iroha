//! Sealed claimed-qPCS/source/numeric carrier after transcript start.
//!
//! This source-only child is reachable only from the already-sealed started
//! pre-transcript owner. It moves that owner's reader bridge into one complete
//! 200-row schedule batch while retaining the exact confidential snapshot and
//! private facts, moves the returned sole schedule and exact qPCS-bound
//! transcript into the pre-auth claimed owner, authenticates with that owner's
//! provisional final seeds, source-preflights before schedule extraction, and
//! retains an exact 24-byte authenticated numeric tail for every row.
//!
//! The result implements only the schedule-free numeric cursor. It exposes no
//! source, facts, bridge, schedule, transcript, roots, chronology, raw parts,
//! direct relation, inventory, membership, readiness, or release authority.

#![allow(
    dead_code,
    reason = "the sealed source-only carrier awaits live correspondence and every direct successor"
)]

use core::{fmt, mem::size_of};

use super::super::super::super::super::{
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldNumericCursorV1, RnsNativeCrossFieldNumericEvaluationV1,
        RnsNativeCrossFieldRlweDirectErrorV1,
    },
    rns_native_profile::{ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1},
    rns_native_public_polynomial_reader::{
        RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReadReceiptV1,
    },
    rns_native_qpcs_fri_complete::{
        RNS_NATIVE_QPCS_CLAIMED_NUMERIC_TAIL_BYTES_V1,
        RNS_NATIVE_QPCS_CLAIMED_SOURCE_BINDING_HASH_BYTES_V1,
        RNS_NATIVE_QPCS_CLAIMED_TERMINAL_CHRONOLOGY_BYTES_V1, RnsNativeQpcsFriCompleteErrorV1,
        RnsNativeQpcsSchedulelessClaimedSourceV1, authenticate_rns_native_qpcs_pre_auth_claimed_v1,
        preflight_rns_native_qpcs_authenticated_claimed_source_v1,
        prepare_rns_native_qpcs_pre_auth_claimed_v1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
    rns_native_rlwe_source_statement::RnsNativePublicArtifactViewV1,
    rns_native_source::ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
    rns_native_transcript::{
        ZkAmsMkheRnsNativeQpcsBoundTranscriptV1, ZkAmsMkheRnsNativeTerminalRootsV1,
    },
};
use super::super::{
    RnsNativeCompletedQpcsSourceReadV2, RnsNativeSingleQpcsScheduleBatchV2,
    RnsNativeTailPublicationErrorV2, RnsNativeWholePublicationOwnersV2,
};
use super::{
    RnsNativePreTranscriptPublicStatementFactsV2, RnsNativeStartedPreTranscriptPublicStatementV2,
};
use crate::vega::sponge::Keccak256;

const VERSION_V2: u8 = 2;
const RECORDS_V2: usize = 43;
const EQUATIONS_V2: usize = 2;
const REPETITIONS_V2: usize = 5;
const RELATIONS_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V2;
const QUERY_OPENINGS_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize;
const PUBLIC_EVALUATION_BYTES_V2: usize = size_of::<RnsNativePublicPolynomialEvaluationV1>();
const RETAINED_PUBLIC_EVALUATION_BYTES_V2: usize = RELATIONS_V2 * PUBLIC_EVALUATION_BYTES_V2;
const RETAINED_NUMERIC_CACHE_BYTES_V2: usize =
    RETAINED_PUBLIC_EVALUATION_BYTES_V2 + RNS_NATIVE_QPCS_CLAIMED_NUMERIC_TAIL_BYTES_V1;
const RETAINED_COMMITMENT_DIGEST_BYTES_V2: usize =
    (EQUATIONS_V2 + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1) * 32;
const RETAINED_PAYLOAD_BYTES_V2: usize = RETAINED_NUMERIC_CACHE_BYTES_V2
    + RNS_NATIVE_QPCS_CLAIMED_TERMINAL_CHRONOLOGY_BYTES_V1
    + RETAINED_COMMITMENT_DIGEST_BYTES_V2;
const CANONICAL_CHECKS_V2: u64 = (RELATIONS_V2 * (3 + 2 * RECORDS_V2 + 2)) as u64;
const RING_POWER_SQUARINGS_V2: u64 = RELATIONS_V2 as u64 * 17;
const MODULAR_MULTIPLICATIONS_V2: u64 = RING_POWER_SQUARINGS_V2 + RELATIONS_V2 as u64;
const MODULAR_ADDITIONS_V2: u64 = RELATIONS_V2 as u64;
const NUMERIC_VALIDATION_WORK_UNITS_V2: u64 =
    CANONICAL_CHECKS_V2 + MODULAR_MULTIPLICATIONS_V2 + MODULAR_ADDITIONS_V2;
const NUMERIC_TAIL_RETENTION_WORK_UNITS_V2: u64 =
    RNS_NATIVE_QPCS_CLAIMED_NUMERIC_TAIL_BYTES_V1 as u64;

const CARRIER_BINDING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native.claimed-qpcs-source-carrier.binding";
// Domain, version, four u16 geometry values, lifecycle digest, five public-read
// receipt digests, receipt object count/four u64 counters, the opaque claimed-
// source digest, and the exact two-equation/40-limb digest arrays.
const CARRIER_BINDING_HASH_BYTES_V2: usize = CARRIER_BINDING_DOMAIN_V2.len()
    + 1
    + 4 * 2
    + 32
    + 5 * 32
    + 2
    + 4 * 8
    + 32
    + RETAINED_COMMITMENT_DIGEST_BYTES_V2;
const PRE_BINDING_LOCAL_WORK_UNITS_V2: u64 = NUMERIC_VALIDATION_WORK_UNITS_V2
    + NUMERIC_TAIL_RETENTION_WORK_UNITS_V2
    + RETAINED_COMMITMENT_DIGEST_BYTES_V2 as u64;
const COMBINED_BINDING_HASH_BYTES_V2: usize =
    RNS_NATIVE_QPCS_CLAIMED_SOURCE_BINDING_HASH_BYTES_V1 + CARRIER_BINDING_HASH_BYTES_V2;
const LOCAL_WORK_UNITS_V2: u64 =
    PRE_BINDING_LOCAL_WORK_UNITS_V2 + COMBINED_BINDING_HASH_BYTES_V2 as u64;

/// Source-only declaration and chronology implementation are settled.
pub(super) const RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_SOURCE_SETTLED_V2: bool = true;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_SOURCE_PREFLIGHT_ORDER_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_NUMERIC_TAIL_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_CARRIER_LEDGER_IS_ADDITIVE_V2: bool = true;

/// No live, direct, evidence, readiness, or release gate follows.
pub(super) const RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_LIVE_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_PRE_QPCS_Q_MASK_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_PRE_DIRECT_AXES_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_DIRECT_RELATION_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_INVENTORY_MEMBERSHIP_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_GLOBAL_ROOT_DISCHARGED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_ZERO_ROOT_DISCHARGED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_DIRECT_OPENINGS_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_READINESS_V2: bool = false;
pub(super) const RNS_NATIVE_CLAIMED_QPCS_RELEASE_AUTHORIZED_V2: bool = false;

/// Exact additive accounting for the carrier-local work after the existing
/// reader, qPCS, and source-preflight sub-transitions invoked by the consuming
/// entry. This is not end-to-end accounting for that entry. It excludes the
/// existing 3,520-object/200-evaluation public read, complete qPCS/FRI work,
/// both confidential-source passes, proof backing, publication owners, private
/// pre-transcript facts, allocator metadata, later numeric-cursor destination
/// writes, and all later direct work. Zero
/// new heap/authenticated-I/O bytes therefore means zero additive carrier-local
/// bytes after those separately accounted sub-transitions; the 4,800-byte
/// numeric tail is retained inline. The retained payload intentionally excludes
/// two non-authorizing 32-byte binding digests and ordinary owner headers,
/// matching the prior numeric ledger scope. Exact local work is 28,144 before
/// binding absorption plus 6,962 binding bytes, for 35,106 total units.
/// One work unit is charged per canonical check, modular operation, retained
/// or copied byte, and absorbed binding byte named in the ledger; this is not
/// an instruction count and excludes control-flow comparisons.
pub(super) struct RnsNativeClaimedQpcsSourceCarrierLocalResourceLedgerV2 {
    pub(super) relations: u16,
    pub(super) retained_public_evaluation_bytes: u32,
    pub(super) retained_numeric_tail_bytes: u16,
    pub(super) retained_numeric_cache_bytes: u32,
    pub(super) retained_terminal_chronology_bytes: u16,
    pub(super) retained_commitment_digest_bytes: u16,
    pub(super) retained_payload_bytes: u32,
    pub(super) canonical_checks: u32,
    pub(super) ring_power_squarings: u32,
    pub(super) modular_multiplications: u32,
    pub(super) modular_additions: u16,
    pub(super) numeric_tail_retention_work_units: u16,
    pub(super) pre_binding_local_work_units: u32,
    pub(super) claimed_source_binding_hash_bytes: u16,
    pub(super) carrier_binding_hash_bytes: u16,
    pub(super) combined_binding_hash_bytes: u16,
    pub(super) local_work_units: u32,
    pub(super) new_heap_bytes: u8,
    pub(super) new_spool_bytes: u8,
    pub(super) new_wire_bytes: u8,
    pub(super) new_authenticated_io_bytes: u8,
}

pub(super) const RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_LOCAL_RESOURCE_LEDGER_V2:
    RnsNativeClaimedQpcsSourceCarrierLocalResourceLedgerV2 =
    RnsNativeClaimedQpcsSourceCarrierLocalResourceLedgerV2 {
        relations: RELATIONS_V2 as u16,
        retained_public_evaluation_bytes: RETAINED_PUBLIC_EVALUATION_BYTES_V2 as u32,
        retained_numeric_tail_bytes: RNS_NATIVE_QPCS_CLAIMED_NUMERIC_TAIL_BYTES_V1 as u16,
        retained_numeric_cache_bytes: RETAINED_NUMERIC_CACHE_BYTES_V2 as u32,
        retained_terminal_chronology_bytes: RNS_NATIVE_QPCS_CLAIMED_TERMINAL_CHRONOLOGY_BYTES_V1
            as u16,
        retained_commitment_digest_bytes: RETAINED_COMMITMENT_DIGEST_BYTES_V2 as u16,
        retained_payload_bytes: RETAINED_PAYLOAD_BYTES_V2 as u32,
        canonical_checks: CANONICAL_CHECKS_V2 as u32,
        ring_power_squarings: RING_POWER_SQUARINGS_V2 as u32,
        modular_multiplications: MODULAR_MULTIPLICATIONS_V2 as u32,
        modular_additions: MODULAR_ADDITIONS_V2 as u16,
        numeric_tail_retention_work_units: NUMERIC_TAIL_RETENTION_WORK_UNITS_V2 as u16,
        pre_binding_local_work_units: PRE_BINDING_LOCAL_WORK_UNITS_V2 as u32,
        claimed_source_binding_hash_bytes: RNS_NATIVE_QPCS_CLAIMED_SOURCE_BINDING_HASH_BYTES_V1
            as u16,
        carrier_binding_hash_bytes: CARRIER_BINDING_HASH_BYTES_V2 as u16,
        combined_binding_hash_bytes: COMBINED_BINDING_HASH_BYTES_V2 as u16,
        local_work_units: LOCAL_WORK_UNITS_V2 as u32,
        new_heap_bytes: 0,
        new_spool_bytes: 0,
        new_wire_bytes: 0,
        new_authenticated_io_bytes: 0,
    };

const _: () = {
    assert!(RECORDS_V2 == 43);
    assert!(EQUATIONS_V2 == 2);
    assert!(REPETITIONS_V2 == 5);
    assert!(RELATIONS_V2 == 200);
    assert!(QUERY_OPENINGS_V2 == 160);
    assert!(PUBLIC_EVALUATION_BYTES_V2 == 704);
    assert!(RETAINED_PUBLIC_EVALUATION_BYTES_V2 == 140_800);
    assert!(RNS_NATIVE_QPCS_CLAIMED_NUMERIC_TAIL_BYTES_V1 == 4_800);
    assert!(RETAINED_NUMERIC_CACHE_BYTES_V2 == 145_600);
    assert!(RNS_NATIVE_QPCS_CLAIMED_TERMINAL_CHRONOLOGY_BYTES_V1 == 5_352);
    assert!(RETAINED_COMMITMENT_DIGEST_BYTES_V2 == 1_344);
    assert!(RETAINED_PAYLOAD_BYTES_V2 == 152_296);
    assert!(CANONICAL_CHECKS_V2 == 18_200);
    assert!(RING_POWER_SQUARINGS_V2 == 3_400);
    assert!(MODULAR_MULTIPLICATIONS_V2 == 3_600);
    assert!(MODULAR_ADDITIONS_V2 == 200);
    assert!(NUMERIC_VALIDATION_WORK_UNITS_V2 == 22_000);
    assert!(NUMERIC_TAIL_RETENTION_WORK_UNITS_V2 == 4_800);
    assert!(PRE_BINDING_LOCAL_WORK_UNITS_V2 == 28_144);
    assert!(RNS_NATIVE_QPCS_CLAIMED_SOURCE_BINDING_HASH_BYTES_V1 == 5_284);
    assert!(CARRIER_BINDING_HASH_BYTES_V2 == 1_678);
    assert!(COMBINED_BINDING_HASH_BYTES_V2 == 6_962);
    assert!(LOCAL_WORK_UNITS_V2 == 35_106);
    // Historical old-local-ledger-plus-tail floor only; not a pre-binding
    // claim. The exact pre-binding amount is 28,144 above.
    assert!(LOCAL_WORK_UNITS_V2 >= 28_808);
    assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_SOURCE_SETTLED_V2);
    assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_PREFLIGHT_ORDER_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_CLAIMED_QPCS_NUMERIC_TAIL_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_CLAIMED_QPCS_CARRIER_LEDGER_IS_ADDITIVE_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_LIVE_INTEGRATED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_PRE_QPCS_Q_MASK_INTEGRATED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_PRE_DIRECT_AXES_INTEGRATED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_DIRECT_RELATION_INTEGRATED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_INVENTORY_MEMBERSHIP_INTEGRATED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_GLOBAL_ROOT_DISCHARGED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_ZERO_ROOT_DISCHARGED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_DIRECT_OPENINGS_AVAILABLE_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_RESOURCE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_READINESS_V2);
    assert!(!RNS_NATIVE_CLAIMED_QPCS_RELEASE_AUTHORIZED_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeClaimedQpcsSourceCarrierErrorV2 {
    PublicRead,
    Qpcs,
    SourcePreflight,
    InvalidCount,
    InvalidOrder,
    InvalidBinding,
    Poisoned,
}

impl fmt::Display for RnsNativeClaimedQpcsSourceCarrierErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeClaimedQpcsSourceCarrierErrorV2 {}

/// Fixed-shape qPCS inputs accepted only by the consuming claimed transition.
/// There is deliberately no final transcript field: qPCS borrows final seeds
/// from the provisional chronology already owned by its pre-auth typestate.
pub(super) struct RnsNativeClaimedQpcsAuthenticationInputV2<'digests, 'proof> {
    equation_commitment_digests: &'digests [[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: &'digests [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    query_opening_digests: &'digests [[u8; 32]; QUERY_OPENINGS_V2],
    proof: &'proof [u8],
}

impl<'digests, 'proof> RnsNativeClaimedQpcsAuthenticationInputV2<'digests, 'proof> {
    pub(super) const fn new_v2(
        equation_commitment_digests: &'digests [[u8; 32]; EQUATIONS_V2],
        limb_commitment_digests: &'digests [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
        query_opening_digests: &'digests [[u8; 32]; QUERY_OPENINGS_V2],
        proof: &'proof [u8],
    ) -> Self {
        Self {
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
            proof,
        }
    }
}

struct RnsNativeClaimedSourceCursorV2 {
    next_relation: usize,
    poisoned: bool,
}

impl RnsNativeClaimedSourceCursorV2 {
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
    ) -> Result<usize, RnsNativeClaimedQpcsSourceCarrierErrorV2> {
        if self.poisoned {
            return Err(RnsNativeClaimedQpcsSourceCarrierErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.next_relation >= RELATIONS_V2
            || limb != self.next_relation / REPETITIONS_V2
            || repetition != self.next_relation % REPETITIONS_V2
        {
            return Err(RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidOrder);
        }
        Ok(self.next_relation)
    }

    fn commit_v2(&mut self) {
        self.next_relation += 1;
        self.poisoned = false;
    }
}

fn begin_numeric_destination_v2(
    cursor: &mut RnsNativeClaimedSourceCursorV2,
    limb: usize,
    repetition: usize,
    destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
) -> Result<usize, RnsNativeClaimedQpcsSourceCarrierErrorV2> {
    *destination = RnsNativeCrossFieldNumericEvaluationV1::default();
    cursor.begin_v2(limb, repetition)
}

/// Top-level move-only owner of the exact source/public/qPCS chronology and a
/// schedule-free numeric cursor. The nested claimed source privately retains
/// the extracted sole schedule, scheduleless source stage, all three terminal
/// obligations, pre-global capability, and final seeds.
#[must_use = "claimed qPCS/source carrier must remain intact until every successor verifies"]
pub(super) struct RnsNativeClaimedQpcsSourceCarrierV2<
    'proof,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
> {
    owners: RnsNativeWholePublicationOwnersV2,
    public_evaluations: Box<[RnsNativePublicPolynomialEvaluationV1]>,
    read_receipt: RnsNativePublicPolynomialReadReceiptV1,
    facts: RnsNativePreTranscriptPublicStatementFactsV2,
    equation_commitment_digests: [[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    claimed_source: RnsNativeQpcsSchedulelessClaimedSourceV1<'proof, S>,
    carrier_binding_digest: [u8; 32],
    cursor: RnsNativeClaimedSourceCursorV2,
}

impl<K, P, S> RnsNativeStartedPreTranscriptPublicStatementV2<K, P, S>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    /// Consume the sealed started owner through the complete claimed-qPCS,
    /// source-preflight, numeric-materialization, and schedule-extraction
    /// chronology. No intermediate owner or schedule escapes this call.
    pub(super) fn authenticate_claimed_qpcs_source_carrier_v2<'digests, 'proof>(
        self,
        relation_schedule: RnsNativeQpcsRelationScheduleV1,
        qpcs_transcript: ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
        terminal_roots: ZkAmsMkheRnsNativeTerminalRootsV1,
        input: RnsNativeClaimedQpcsAuthenticationInputV2<'digests, 'proof>,
    ) -> Result<
        RnsNativeClaimedQpcsSourceCarrierV2<'proof, S>,
        RnsNativeClaimedQpcsSourceCarrierErrorV2,
    > {
        let Self {
            bridge,
            source,
            facts,
        } = self;
        let mut batch = RnsNativeSingleQpcsScheduleBatchV2::begin_v2(bridge, relation_schedule)
            .map_err(map_public_read_error_v2)?;
        for _ in 0..RELATIONS_V2 {
            batch
                .take_next_evaluation_v2()
                .map_err(map_public_read_error_v2)?;
        }
        let RnsNativeCompletedQpcsSourceReadV2 {
            owners,
            schedule,
            evaluations: public_evaluations,
            read_receipt,
        } = batch.finish_v2().map_err(map_public_read_error_v2)?;
        if public_evaluations.len() != RELATIONS_V2 {
            return Err(RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidCount);
        }

        let RnsNativeClaimedQpcsAuthenticationInputV2 {
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
            proof,
        } = input;
        let pre_auth =
            prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs_transcript, terminal_roots)
                .map_err(map_qpcs_error_v2)?;
        let authenticated = authenticate_rns_native_qpcs_pre_auth_claimed_v1(
            pre_auth,
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
            proof,
        )
        .map_err(map_qpcs_error_v2)?;

        // Copy only the exact arrays accepted by the successful qPCS call.
        // Query-opening digests remain construction-only.
        let equation_commitment_digests = *equation_commitment_digests;
        let limb_commitment_digests = *limb_commitment_digests;
        let public = RnsNativePublicArtifactViewV1::new(
            facts.epoch,
            facts.governed_roster_digest,
            &facts.public_a_limb_digests,
            &facts.public_b_limb_digests,
            &facts.ciphertext_c0_limb_digests,
            &facts.ciphertext_c1_limb_digests,
            &facts.records,
            facts.public_bundle_digest,
        );
        let preflighted = preflight_rns_native_qpcs_authenticated_claimed_source_v1(
            authenticated,
            facts.layout,
            facts.receipt,
            public,
            &equation_commitment_digests,
            &limb_commitment_digests,
            source,
        )
        .map_err(|_| RnsNativeClaimedQpcsSourceCarrierErrorV2::SourcePreflight)?;
        let claimed_source = preflighted
            .materialize_numeric_and_take_schedule_v1(&public_evaluations)
            .map_err(|_| RnsNativeClaimedQpcsSourceCarrierErrorV2::SourcePreflight)?;
        let carrier_binding_digest = carrier_binding_digest_v2(
            &owners,
            &read_receipt,
            &claimed_source,
            &equation_commitment_digests,
            &limb_commitment_digests,
        )?;
        Ok(RnsNativeClaimedQpcsSourceCarrierV2 {
            owners,
            public_evaluations,
            read_receipt,
            facts,
            equation_commitment_digests,
            limb_commitment_digests,
            claimed_source,
            carrier_binding_digest,
            cursor: RnsNativeClaimedSourceCursorV2::new_v2(),
        })
    }
}

fn map_public_read_error_v2(
    _: RnsNativeTailPublicationErrorV2,
) -> RnsNativeClaimedQpcsSourceCarrierErrorV2 {
    RnsNativeClaimedQpcsSourceCarrierErrorV2::PublicRead
}

fn map_qpcs_error_v2(
    _: RnsNativeQpcsFriCompleteErrorV1,
) -> RnsNativeClaimedQpcsSourceCarrierErrorV2 {
    RnsNativeClaimedQpcsSourceCarrierErrorV2::Qpcs
}

fn carrier_binding_digest_v2<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1>(
    owners: &RnsNativeWholePublicationOwnersV2,
    receipt: &RnsNativePublicPolynomialReadReceiptV1,
    claimed_source: &RnsNativeQpcsSchedulelessClaimedSourceV1<'_, S>,
    equation_commitment_digests: &[[u8; 32]; EQUATIONS_V2],
    limb_commitment_digests: &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) -> Result<[u8; 32], RnsNativeClaimedQpcsSourceCarrierErrorV2> {
    if owners.lifecycle_digest == [0; 32]
        || [
            receipt.manifest_digest_v1(),
            receipt.qpcs_schedule_digest_v1(),
            receipt.provider_identity_v1(),
            receipt.snapshot_identity_v1(),
            receipt.read_set_digest_v1(),
            claimed_source.claimed_source_binding_digest_v1(),
        ]
        .contains(&[0; 32])
    {
        return Err(RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidBinding);
    }
    let mut hash = Keccak256::new();
    hash.update(CARRIER_BINDING_DOMAIN_V2);
    hash.update(&[VERSION_V2]);
    for geometry in [
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        REPETITIONS_V2,
        RECORDS_V2,
        RELATIONS_V2,
    ] {
        hash.update(&(geometry as u16).to_be_bytes());
    }
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
    hash.update(&claimed_source.claimed_source_binding_digest_v1());
    for digest in equation_commitment_digests
        .iter()
        .chain(limb_commitment_digests.iter())
    {
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidBinding);
    }
    Ok(digest)
}

fn map_numeric_cursor_error_v2(
    error: RnsNativeClaimedQpcsSourceCarrierErrorV2,
) -> RnsNativeCrossFieldRlweDirectErrorV1 {
    match error {
        RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidOrder
        | RnsNativeClaimedQpcsSourceCarrierErrorV2::Poisoned
        | RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidCount => {
            RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable
        }
        RnsNativeClaimedQpcsSourceCarrierErrorV2::PublicRead
        | RnsNativeClaimedQpcsSourceCarrierErrorV2::Qpcs
        | RnsNativeClaimedQpcsSourceCarrierErrorV2::SourcePreflight
        | RnsNativeClaimedQpcsSourceCarrierErrorV2::InvalidBinding => {
            RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext
        }
    }
}

impl<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1> RnsNativeCrossFieldNumericCursorV1
    for RnsNativeClaimedQpcsSourceCarrierV2<'_, S>
{
    fn authoritative_binding_digest_v1(&self) -> [u8; 32] {
        self.claimed_source.authoritative_source_binding_digest_v1()
    }

    fn take_numeric_evaluation_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let relation =
            begin_numeric_destination_v2(&mut self.cursor, limb, repetition, destination)
                .map_err(map_numeric_cursor_error_v2)?;
        let public = *self
            .public_evaluations
            .get(relation)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable)?;
        let tail = self
            .claimed_source
            .numeric_tail_v1(limb, repetition)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable)?;
        let (a, qpcs_product, qpcs_opening_quotient) = tail.values_v1();
        *destination = RnsNativeCrossFieldNumericEvaluationV1 {
            a,
            public_a: public.public_a,
            public_b: public.public_b,
            ciphertext_c0: public.ciphertext_c0,
            ciphertext_c1: public.ciphertext_c1,
            qpcs_product,
            qpcs_opening_quotient,
        };
        self.cursor.commit_v2();
        Ok(())
    }
}

#[cfg(test)]
#[path = "claimed_qpcs_source_carrier_v2_tests.rs"]
mod tests;
