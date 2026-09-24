//! Sole topology custody/operation reducer over explicit, untrusted execution claims.
//!
//! Core must supply actual execution coordinates, enforce permissions and authenticate parent
//! state/floor hashes before atomically publishing the returned canonical history and deltas.
//! Neither this model nor successful replay grants signing, execution or finality authority.
//! The typed V1 ISI is registered but Core admission is closed. TODO: integrate this reducer into
//! persisted indices, snapshot reader and exact ordered input/result/output Check proof consumer
//! before production admission.

use crate::account::AccountId;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::{
    custody::SignerCustodyAnchorV1,
    protocol::{
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1, SignerOperationIntentV1,
        SignerOperationReservationV1,
    },
    topology::{SignerTopologyRequestV1, subject::TopologyApprovalSubjectV1},
};

pub mod reducer;
/// Normal control revision limit, retaining two final emergency revocations.
pub const TOPOLOGY_CONTROL_NORMAL_LIMIT_V1: u64 = 8192;
/// Total control capacity, including separate signer and attester revocations.
pub const TOPOLOGY_CONTROL_LIMIT_V1: u64 = TOPOLOGY_CONTROL_NORMAL_LIMIT_V1 + 2;
/// Permanent operation IDs; an admitted ID is never deleted or reused.
pub const TOPOLOGY_OPERATION_LIMIT_V1: u64 = 65_536;
/// Reservation lifetime, additionally capped by custody, trust and reviewed-subject expiry.
pub const TOPOLOGY_RESERVATION_MS_V1: u64 = 60_000;
/// Maximum canonical action/control/operation frame, including Norito layout header.
pub const TOPOLOGY_RECORD_MAX_BYTES_V1: usize = 48 * 1024;
/// Maximum replay entry; each action appears once, without duplicate record payloads.
pub const TOPOLOGY_HISTORY_MAX_BYTES_V1: usize = 64 * 1024;
/// Finite retained history capacity, reserving a terminal transition for every admitted ID.
pub const TOPOLOGY_HISTORY_LIMIT_V1: u64 =
    TOPOLOGY_CONTROL_LIMIT_V1 + 2 * TOPOLOGY_OPERATION_LIMIT_V1;
/// Dedicated durable namespace; neither role14 nor its permissions may be reused.
pub const TOPOLOGY_AUTHORITY_NAMESPACE_V1: &str = "sorafs_topology_authority_v1";

/// Public commitment to a complete prefix; zero revision and zero digest occur together.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyHeadV1")]
pub struct TopologyHeadV1 {
    /// Retained one-based revision, or zero before the first entry.
    pub revision: u64,
    /// Exact terminal record digest, or zero before the first entry.
    pub digest: [u8; 32],
}
impl TopologyHeadV1 {
    /// Empty prefix; Core may select it only after proving that no durable rows exist.
    pub const EMPTY: Self = Self {
        revision: 0,
        digest: [0; 32],
    };
}

/// Compact derived state retained beside native indexed rows; decoding it proves no authority.
///
/// Cold restoration must replay the complete authenticated prefix through the sole reducer and
/// compare this entire summary plus every durable row/tombstone. A live reader borrows this exact
/// State-owned value and performs only indexed reads; it must not reset or reconstruct history.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyRetainedStateV1")]
pub struct TopologyRetainedStateV1 {
    /// Exact current custody prefix.
    pub control_head: TopologyHeadV1,
    /// Exact current operation prefix.
    pub operation_head: TopologyHeadV1,
    /// Complete canonical replay prefix.
    pub history_head: TopologyHeadV1,
    /// Sole reserved operation, if any.
    pub active: Option<[u8; 32]>,
    /// Last issued fencing token; permanently equal to admitted operation count.
    pub fence: u64,
    /// Last immutable completion audit.
    pub audit: SignerOperationAuditHeadV1,
    /// Permanent operation-ID cardinality, never decremented by terminalization.
    pub operation_count: u64,
    /// Permanent signer-key tombstone cardinality.
    pub signer_key_count: u64,
    /// Permanent independent-attester-key tombstone cardinality.
    pub attester_key_count: u64,
    /// Last mutating topology execution, independent of no-write Checks.
    pub last_execution: Option<TopologyExecutionClaimV1>,
}
impl TopologyRetainedStateV1 {
    /// Empty derived summary, usable natively only after proving absence of all rows.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            control_head: TopologyHeadV1::EMPTY,
            operation_head: TopologyHeadV1::EMPTY,
            history_head: TopologyHeadV1::EMPTY,
            active: None,
            fence: 0,
            audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
            operation_count: 0,
            signer_key_count: 0,
            attester_key_count: 0,
            last_execution: None,
        }
    }
}

/// Wire provenance claims. Decoding these fields never proves native execution.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyExecutionClaimV1")]
pub struct TopologyExecutionClaimV1 {
    /// Claimed executing block height; Core must derive it from the actual StateTransaction.
    pub height: u64,
    /// Zero-based topology mutation ordinal in that block, excluding no-write Checks.
    pub ordinal: u32,
    /// Claimed block timestamp; Core must supply actual execution time.
    pub recorded_at_unix_ms: u64,
    /// Claimed registered transaction authority; Core must check scoped permissions.
    pub authority: AccountId,
}

/// Claimed native cut required by pure validation, never independently authenticated here.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyContextClaimV1")]
pub struct TopologyContextClaimV1 {
    /// Actual execution coordinates must be supplied by the future Core caller.
    pub execution: TopologyExecutionClaimV1,
    /// Committed predecessor anchor for current custody; mandatory on enrollment and key use.
    pub custody_anchor: Option<SignerCustodyAnchorV1>,
    /// Independently requested finalized floor selected from native block history for Check.
    pub floor: Option<TopologyFloorClaimV1>,
}
/// A requested finalized prefix claim; a matching digest is not a finality proof.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyFloorClaimV1")]
pub struct TopologyFloorClaimV1 {
    /// Positive finalized floor height preceding Check execution.
    pub height: u64,
    /// Exact independently retained hash at that height.
    pub block_hash: [u8; 32],
}

/// Exact separately reviewed topology candidate and original signer intent.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyReserveV1")]
pub struct TopologyReserveV1 {
    /// Full candidate-bound configuration subject; cannot change after admission.
    pub subject: TopologyApprovalSubjectV1,
    /// Exact whole binding, original custody and operation id.
    pub request: SignerTopologyRequestV1,
    /// Sign action, request digest and exact current audit predecessor.
    pub intent: SignerOperationIntentV1,
}
/// Immutable completion input; signatures remain private until genuinely finalized release.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyCompleteV1")]
pub struct TopologyCompleteV1 {
    /// Original exact request; renewal or candidate substitution is forbidden.
    pub request: SignerTopologyRequestV1,
    /// Original exact Sign intent.
    pub intent: SignerOperationIntentV1,
    /// Original reservation, never a newly issued replacement.
    pub reservation: SignerOperationReservationV1,
    /// Exactly one next audit plus response commitment.
    pub commitment: SignerOperationCommitmentV1,
    /// Nonzero commitment to the four ordered privately staged signatures.
    pub signatures_digest: [u8; 32],
}
/// Exact terminalization request; no erased identity or replacement fence.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyExpireV1")]
pub struct TopologyExpireV1 {
    /// Original admitted operation identity.
    pub operation_id: [u8; 32],
    /// Exact original reservation to expire.
    pub reservation: SignerOperationReservationV1,
}

/// Closed operation phases; every phase requires a separately retained fresh native challenge.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyCheckPhaseV1")]
pub enum TopologyCheckPhaseV1 {
    /// Current custody plus exact audit; grants no reservation.
    #[codec(index = 0)]
    Current(Box<SignerOperationAuditHeadV1>),
    /// Exact unexpired original operation before provider I/O.
    #[codec(index = 1)]
    BeforeProvider(Box<TopologyOperationRecordV1>),
    /// Exact unexpired original operation after provider I/O.
    #[codec(index = 2)]
    AfterProvider(Box<TopologyOperationRecordV1>),
    /// Exact unexpired original operation before completion CAS.
    #[codec(index = 3)]
    BeforeCommit(Box<TopologyOperationRecordV1>),
    /// Exact immutable timely completion at commit/recovery.
    #[codec(index = 4)]
    AfterCommit(Box<TopologyOperationRecordV1>),
    /// Exact immutable timely completion before releasing signatures.
    #[codec(index = 5)]
    BeforeRelease(Box<TopologyOperationRecordV1>),
}
/// Challenged no-write input. The native consumer must bind the exact executed transaction/result.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyCheckV1")]
pub struct TopologyCheckV1 {
    /// Nonzero consumer challenge; replay resistance requires the private Core round owner.
    pub challenge: [u8; 32],
    /// Exact genesis-derived network pinned independently from this input.
    pub network_id: [u8; 32],
    /// Independent retained finalized floor.
    pub floor: TopologyFloorClaimV1,
    /// Registered operator expected to retain the scoped permission at execution and use.
    pub expected_operator: AccountId,
    /// Entire reviewed subject/request/intent, checked in every phase.
    pub reviewed: TopologyReserveV1,
    /// Exact current or original operation phase.
    pub phase: TopologyCheckPhaseV1,
}

/// Exact emergency revocation selection; each generation may be revoked once.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyRevocationV1")]
pub struct TopologyRevocationV1 {
    /// Revoke current signer generation.
    pub signer: bool,
    /// Revoke current attester generation.
    pub attester: bool,
}

/// Topology-only operations; native permissions and actual execution are deliberately external.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyActionV1")]
pub enum TopologyActionV1 {
    /// Canonical role16 custody policy; configuration invalidates any active operation.
    #[codec(index = 0)]
    Configure(Vec<u8>),
    /// Canonical independent enrollment for the exact committed control predecessor.
    #[codec(index = 1)]
    Enroll(Vec<u8>),
    /// Monotonic emergency signer/attester revocation, invalidating any active operation.
    #[codec(index = 2)]
    Revoke(TopologyRevocationV1),
    /// Reserve exactly one candidate/request/intent before key I/O.
    #[codec(index = 3)]
    Reserve(Box<TopologyReserveV1>),
    /// Commit the exact original reservation before exclusive expiry.
    #[codec(index = 4)]
    Complete(Box<TopologyCompleteV1>),
    /// Terminalize without advancing audit or deleting the operation id.
    #[codec(index = 5)]
    Expire(TopologyExpireV1),
    /// Check claimed current state without consuming an id or changing any head.
    #[codec(index = 6)]
    Check(Box<TopologyCheckV1>),
}
/// CAS input wrapped by the registered, explicitly closed V1 topology instruction.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyTransitionV1")]
pub struct TopologyTransitionV1 {
    /// Exact deployment scope.
    pub deployment_id: String,
    /// Expected custody revision and digest, independent of operation progress.
    pub control: TopologyHeadV1,
    /// Expected operation revision and digest, independent of custody progress.
    pub operations: TopologyHeadV1,
    /// Sole intended action.
    pub action: TopologyActionV1,
}

/// Canonical immutable custody row; its digest excludes operation/audit progress.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyControlRecordV1")]
pub struct TopologyControlRecordV1 {
    /// Exact deployment identity.
    pub deployment_id: String,
    /// Next one-based custody revision.
    pub revision: u64,
    /// Exact previous control digest, zero only at revision one.
    pub predecessor_digest: [u8; 32],
    /// Exact action plus asserted executing authority commitment.
    pub request_digest: [u8; 32],
    /// Execution claims; no finality implied.
    pub execution: TopologyExecutionClaimV1,
    /// Canonical Manifest custody control frame.
    pub control_state: Vec<u8>,
    /// Exact current enrolled statement; absent after configuration.
    pub enrollment: Option<Vec<u8>>,
}

/// Timely immutable completion commitments, before signature release.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyCompletionV1")]
pub struct TopologyCompletionV1 {
    /// Exactly one next audit and response commitment.
    pub commitment: SignerOperationCommitmentV1,
    /// Exact ordered staged signatures commitment.
    pub signatures_digest: [u8; 32],
}
/// Terminal state never resets to Reserved.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyOutcomeV1")]
pub enum TopologyOutcomeV1 {
    /// Active exclusive reservation.
    #[codec(index = 0)]
    Reserved,
    /// Timely immutable commitments; no signatures published by this action.
    #[codec(index = 1)]
    Completed(TopologyCompletionV1),
    /// Explicit expiration, retaining its original operation identity.
    #[codec(index = 2)]
    Expired,
    /// Current custody changed, retaining its original operation identity.
    #[codec(index = 3)]
    Invalidated,
}
/// Immutable operation row preserving original candidate, owner, custody, intent and reservation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyOperationRecordV1")]
pub struct TopologyOperationRecordV1 {
    /// Exact deployment identity.
    pub deployment_id: String,
    /// Global one-based operation history revision.
    pub revision: u64,
    /// Exact preceding operation history digest.
    pub predecessor_digest: [u8; 32],
    /// Exact transition plus asserted authority commitment.
    pub transition_digest: [u8; 32],
    /// Current transition execution claims.
    pub execution: TopologyExecutionClaimV1,
    /// Original reservation execution and owner, never relabelled.
    pub reserved: TopologyExecutionClaimV1,
    /// Original complete candidate, request and intent.
    pub reviewed: TopologyReserveV1,
    /// Original exclusive reservation.
    pub reservation: SignerOperationReservationV1,
    /// Current permanent outcome.
    pub outcome: TopologyOutcomeV1,
}
/// Complete canonical replay entry; its expected terminal head must come from native storage.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::topology_authority::TopologyHistoryEntryV1")]
pub struct TopologyHistoryEntryV1 {
    /// Consecutive global mutation revision, covering control and operation transitions.
    pub revision: u64,
    /// Exact previous history digest, zero only at revision one.
    pub predecessor_digest: [u8; 32],
    /// Full canonical action, retained to replay the same sole reducer.
    pub transition: TopologyTransitionV1,
    /// Original claimed execution/parent inputs; Core authenticates their provenance.
    pub context: TopologyContextClaimV1,
    /// Resulting exact control head.
    pub control: TopologyHeadV1,
    /// Resulting exact operation head.
    pub operations: TopologyHeadV1,
}
