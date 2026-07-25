use super::*;
use crate::sorafs::{
    capacity::{
        CapacityDeclarationRecord, CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
    },
    moderation_ledger::{
        ModerationAppealIntakeV1, ModerationChallengeDecisionV1, ModerationChallengeKindV1,
        ModerationLedgerPolicyV1,
    },
    orderbook::OrderbookAdmissionPolicyV1,
    pin_registry::{ManifestAliasBinding, ManifestDigest, ReplicationOrderId},
    pop_registry::PopIssuerPolicyV1,
    pricing::{PricingScheduleRecord, ProviderCreditRecord},
    proof_ledger::ProofOutcomeSignerPolicyV1,
    reserve::{
        ReserveAuthorityPolicyV1, ReserveLifecycleStage, ReserveMovementKindV1,
        ReserveProviderTermsV1,
    },
};
use sorafs_manifest::deal::XorQuantity;

isi! {
    /// Register a canonical `SoraFS` manifest with the paid pin registry.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterPinManifest {
        /// Canonical Norito-encoded `sorafs_manifest::ManifestV1` payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub manifest_payload: Vec<u8>,
        /// Epoch (inclusive) recorded for the submission event.
        pub submitted_epoch: u64,
        /// Optional alias binding approved with the manifest.
        pub alias: Option<ManifestAliasBinding>,
        /// Optional predecessor manifest digest forming a succession chain.
        pub successor_of: Option<ManifestDigest>,
    }
}

impl crate::seal::Instruction for RegisterPinManifest {}

isi! {
    /// Approve a previously registered manifest digest.
pub struct ApprovePinManifest {
    /// Manifest digest previously registered with the pin registry.
    pub digest: ManifestDigest,
        /// Epoch (inclusive) when the manifest becomes part of the active replication set.
        pub approved_epoch: u64,
        /// Optional governance envelope (`manifest_signatures.json`) attached to the approval.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::base64_vec::option")
        )]
        pub council_envelope: Option<Vec<u8>>,
        /// Optional digest of the council envelope (`manifest_signatures.json`).
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub council_envelope_digest: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for ApprovePinManifest {}

isi! {
    /// Retire a manifest digest from the pin registry.
pub struct RetirePinManifest {
    /// Manifest digest to retire.
    pub digest: ManifestDigest,
        /// Epoch (inclusive) after which the manifest is no longer required.
        pub retired_epoch: u64,
        /// Optional human-readable reason recorded alongside the retirement.
        pub reason: Option<String>,
    }
}

impl crate::seal::Instruction for RetirePinManifest {}

isi! {
    /// Bind an approved alias to a manifest digest.
pub struct BindManifestAlias {
    /// Manifest digest that will be associated with the alias.
    pub digest: ManifestDigest,
        /// Alias binding payload approved by governance.
        pub binding: ManifestAliasBinding,
        /// Epoch (inclusive) when the alias becomes active.
        pub bound_epoch: u64,
        /// Epoch (inclusive) when the alias expires unless renewed.
        pub expiry_epoch: u64,
    }
}

impl crate::seal::Instruction for BindManifestAlias {}

isi! {
    /// Register or update a provider capacity declaration.
pub struct RegisterCapacityDeclaration {
    /// Declaration record persisted by the capacity registry.
    pub record: CapacityDeclarationRecord,
    }
}

impl crate::seal::Instruction for RegisterCapacityDeclaration {}

isi! {
    /// Record a capacity telemetry snapshot for a provider.
pub struct RecordCapacityTelemetry {
    /// Telemetry record used to update the fee ledger.
    pub record: CapacityTelemetryRecord,
    }
}

impl crate::seal::Instruction for RecordCapacityTelemetry {}

isi! {
    /// Register a governance-authored dispute targeting a storage provider.
pub struct RegisterCapacityDispute {
    /// Canonical dispute record that will be persisted in the registry.
    pub record: CapacityDisputeRecord,
    }
}

impl crate::seal::Instruction for RegisterCapacityDispute {}

isi! {
    /// Issue a replication order covering one or more storage providers.
pub struct IssueReplicationOrder {
    /// Deterministic identifier assigned to the replication order.
    pub order_id: ReplicationOrderId,
        /// Canonical Norito-encoded replication order payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub order_payload: Vec<u8>,
        /// Epoch (inclusive) when the order is issued.
        pub issued_epoch: u64,
        /// Epoch (inclusive) when the order expires.
        pub deadline_epoch: u64,
    }
}

impl crate::seal::Instruction for IssueReplicationOrder {}

isi! {
    /// Mark a replication order as completed.
pub struct CompleteReplicationOrder {
    /// Identifier of the replication order.
    pub order_id: ReplicationOrderId,
        /// Epoch (inclusive) when replication completed.
        pub completion_epoch: u64,
    }
}

impl crate::seal::Instruction for CompleteReplicationOrder {}

isi! {
    /// Mark a pending replication order as expired after its deadline.
pub struct ExpireReplicationOrder {
    /// Identifier of the replication order.
    pub order_id: ReplicationOrderId,
        /// Epoch at which the order is expired; must be later than its deadline.
        pub expiration_epoch: u64,
    }
}

impl crate::seal::Instruction for ExpireReplicationOrder {}

isi! {
    /// Register or update the owner binding for a `SoraFS` provider.
pub struct RegisterProviderOwner {
    /// Provider identifier that will be bound.
    pub provider_id: ProviderId,
        /// Account identifier that owns the provider.
        pub owner: AccountId,
    }
}

impl crate::seal::Instruction for RegisterProviderOwner {}

isi! {
    /// Remove the owner binding for a `SoraFS` provider.
pub struct UnregisterProviderOwner {
    /// Provider identifier whose binding will be removed.
    pub provider_id: ProviderId,
    }
}

impl crate::seal::Instruction for UnregisterProviderOwner {}

isi! {
    /// Update the governance-controlled pricing schedule for `SoraFS`.
    pub struct SetPricingSchedule {
        /// Pricing schedule record that replaces the previous schedule.
        pub schedule: PricingScheduleRecord,
    }
}

impl crate::seal::Instruction for SetPricingSchedule {}

isi! {
    /// Upsert the credit ledger entry for a storage provider.
    pub struct UpsertProviderCredit {
        /// Credit record snapshot used to seed or update governance accounting.
        pub record: ProviderCreditRecord,
    }
}

impl crate::seal::Instruction for UpsertProviderCredit {}

isi! {
    /// Activate the next governance-controlled `PoP` issuer policy revision.
    pub struct SetSorafsPopIssuerPolicy {
        /// Policy revision to validate and activate.
        pub policy: PopIssuerPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsPopIssuerPolicy {}

isi! {
    /// Commit a bounded batch of private `PoP` credentials and public roots atomically.
    pub struct CommitSorafsPopCredentialBatch {
        /// Exact canonical Norito `PopCredentialCommitmentBatchV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub batch_payload: Vec<u8>,
    }
}

impl crate::seal::Instruction for CommitSorafsPopCredentialBatch {}

isi! {
    /// Publish a strict signed extension of the active `PoP` revocation list.
    pub struct PublishSorafsPopRevocationList {
        /// Exact canonical Norito `sorafs_manifest::PopRevocationListV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub revocation_list_payload: Vec<u8>,
        /// Exact active issuer-policy digest expected by the publisher.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub issuer_policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for PublishSorafsPopRevocationList {}

isi! {
    /// Activate the next governance-controlled `SoraFS` orderbook policy revision.
    pub struct SetSorafsOrderbookPolicy {
        /// Policy revision to validate and activate.
        pub policy: OrderbookAdmissionPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsOrderbookPolicy {}

isi! {
    /// Submit a signed canonical order to the authoritative `SoraFS` orderbook ledger.
    pub struct SubmitSorafsOrderbookOrder {
        /// Exact canonical Norito `sorafs_manifest::orderbook::OrderRequestV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub order_payload: Vec<u8>,
        /// Active governance policy digest expected by the submitter.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for SubmitSorafsOrderbookOrder {}

isi! {
    /// Commit a signed owner cancellation to the authoritative `SoraFS` orderbook ledger.
    pub struct CancelSorafsOrderbookOrder {
        /// Exact canonical Norito `sorafs_manifest::orderbook::OrderCancelV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub cancel_payload: Vec<u8>,
        /// Active governance policy digest expected by the submitter.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for CancelSorafsOrderbookOrder {}

isi! {
    /// Execute one bounded deterministic price-time matching transition.
    pub struct MatchSorafsOrderbook {
        /// Exact active governance policy digest expected by the matcher.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Exact authoritative book revision on which the match was computed.
        pub expected_book_revision: u64,
        /// Maximum fills to commit in this transition.
        pub max_fills: u32,
    }
}

impl crate::seal::Instruction for MatchSorafsOrderbook {}

isi! {
    /// Retire expired orders and channels in one bounded authoritative transition.
    pub struct MaintainSorafsOrderbook {
        /// Exact active governance policy digest expected by the caller.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Exact authoritative book revision expected by the caller.
        pub expected_book_revision: u64,
        /// Maximum order/channel records to retire.
        pub max_items: u32,
    }
}

impl crate::seal::Instruction for MaintainSorafsOrderbook {}

isi! {
    /// Settle a funded channel lock and record its signed receipt in the authoritative ledger.
    pub struct RecordSorafsOrderbookSettlementReceipt {
        /// Exact canonical Norito `sorafs_manifest::orderbook::SettlementReceiptV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub receipt_payload: Vec<u8>,
        /// Active governance policy digest expected by the recorder.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for RecordSorafsOrderbookSettlementReceipt {}

isi! {
    /// Activate the next chain-authoritative reserve/rent policy revision.
    pub struct SetSorafsReservePolicy {
        /// Governance policy to validate and activate.
        pub policy: ReserveAuthorityPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsReservePolicy {}

isi! {
    /// Register one provider reserve partition and immutable underwriting terms.
    pub struct RegisterSorafsReserveAccount {
        /// Provider underwriting terms.
        pub terms: ReserveProviderTermsV1,
        /// Exact active reserve policy digest expected by governance.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for RegisterSorafsReserveAccount {}

isi! {
    /// Submit a provider-authenticated reserve top-up or withdrawal request.
    pub struct RequestSorafsReserveMovement {
        /// Globally unique request identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub movement_id: [u8; 32],
        /// Provider reserve partition.
        pub provider_id: ProviderId,
        /// Top-up or withdrawal direction.
        pub kind: ReserveMovementKindV1,
        /// Exact non-zero movement amount.
        pub amount: XorQuantity,
        /// Provider account revision expected by the request.
        pub expected_provider_revision: u64,
        /// Exact active reserve policy digest expected by the request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for RequestSorafsReserveMovement {}

isi! {
    /// Decide and atomically apply or reject a pending reserve movement.
    pub struct DecideSorafsReserveMovement {
        /// Pending movement identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub movement_id: [u8; 32],
        /// Provider account revision expected by the decision.
        pub expected_provider_revision: u64,
        /// Exact active reserve policy digest expected by the decision service.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Whether governance approves the movement.
        pub approve: bool,
        /// Bounded governance rationale.
        pub rationale: String,
    }
}

impl crate::seal::Instruction for DecideSorafsReserveMovement {}

isi! {
    /// Charge one or more deterministic rent periods to a provider.
    pub struct ChargeSorafsReserveRent {
        /// Provider reserve partition.
        pub provider_id: ProviderId,
        /// Provider account revision expected by the charge.
        pub expected_provider_revision: u64,
        /// Number of billing periods, bounded by native execution.
        pub billing_periods: u16,
        /// Exact active reserve policy digest expected by governance.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for ChargeSorafsReserveRent {}

isi! {
    /// Advance a provider's deterministic reserve lifecycle projection.
    pub struct AdvanceSorafsReserveLifecycle {
        /// Provider reserve partition.
        pub provider_id: ProviderId,
        /// Provider account revision expected by the transition.
        pub expected_provider_revision: u64,
        /// Deterministic days past due.
        pub days_past_due: u16,
        /// Exact active reserve policy digest expected by governance.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for AdvanceSorafsReserveLifecycle {}

isi! {
    /// Draw reserve credit under the provider's tier and global debt caps.
    pub struct DrawSorafsReserveCredit {
        /// Provider reserve partition.
        pub provider_id: ProviderId,
        /// Provider account revision expected by the draw.
        pub expected_provider_revision: u64,
        /// Exact non-zero draw amount.
        pub amount: XorQuantity,
        /// Exact active reserve policy digest expected by governance.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for DrawSorafsReserveCredit {}

isi! {
    /// Repay accrued reserve interest and then credit principal.
    pub struct RepaySorafsReserveCredit {
        /// Provider reserve partition.
        pub provider_id: ProviderId,
        /// Provider account revision expected by the repayment.
        pub expected_provider_revision: u64,
        /// Exact non-zero repayment amount.
        pub amount: XorQuantity,
        /// Exact active reserve policy digest expected by the provider.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for RepaySorafsReserveCredit {}

isi! {
    /// Submit a bounded provider-authenticated reserve lifecycle appeal.
    pub struct SubmitSorafsReserveAppeal {
        /// Globally unique appeal identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub appeal_id: [u8; 32],
        /// Appealing provider.
        pub provider_id: ProviderId,
        /// Provider revision expected by the appeal.
        pub expected_provider_revision: u64,
        /// Requested lifecycle stage.
        pub requested_stage: ReserveLifecycleStage,
        /// Bounded provider reason.
        pub reason: String,
        /// Optional external evidence digest.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub evidence_digest: Option<[u8; 32]>,
        /// Exact active reserve policy digest expected by the provider.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for SubmitSorafsReserveAppeal {}

isi! {
    /// Attach a terminal governance decision to a pending reserve appeal.
    pub struct DecideSorafsReserveAppeal {
        /// Pending appeal identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub appeal_id: [u8; 32],
        /// Provider account revision expected by the decision.
        pub expected_provider_revision: u64,
        /// Exact active reserve policy digest expected by the decision service.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Whether governance accepts and applies the requested stage.
        pub accept: bool,
        /// Bounded governance rationale.
        pub rationale: String,
    }
}

impl crate::seal::Instruction for DecideSorafsReserveAppeal {}

/// Repair lease-claim action.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsRepairClaimV1 {
    /// Requested lease duration measured from the committing block time.
    pub lease_duration_ms: u64,
    /// Bounded caller key used for exact replay handling.
    pub idempotency_key: String,
}

/// Repair lease-renewal action.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsRepairRenewV1 {
    /// Exact current lease generation.
    pub lease_generation: u64,
    /// Requested lease duration measured from the committing block time.
    pub lease_duration_ms: u64,
    /// Bounded caller key used for exact replay handling.
    pub idempotency_key: String,
}

/// Successful repair terminal action.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsRepairCompleteV1 {
    /// Exact current lease generation.
    pub lease_generation: u64,
    /// Digest of external completion verification evidence.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_digest: [u8; 32],
    /// Bounded caller key used for exact replay handling.
    pub idempotency_key: String,
}

/// Failed repair terminal action without slashing.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsRepairFailV1 {
    /// Exact current lease generation.
    pub lease_generation: u64,
    /// Digest of the failure reason or external evidence.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub failure_digest: [u8; 32],
    /// Bounded caller key used for exact replay handling.
    pub idempotency_key: String,
}

/// Escalated repair terminal action with an atomic slash proposal.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsRepairEscalateV1 {
    /// Exact current lease generation.
    pub lease_generation: u64,
    /// Exact canonical `sorafs_manifest::repair::RepairSlashProposalV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub slash_proposal_payload: Vec<u8>,
    /// Bounded caller key used for exact replay handling.
    pub idempotency_key: String,
}

/// Chain-authoritative mutation of one SoraFS repair task.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "action", content = "value", rename_all = "snake_case")
)]
pub enum SorafsRepairTaskActionV1 {
    /// Acquire an absent or expired exclusive worker lease.
    Claim(SorafsRepairClaimV1),
    /// Extend the caller's unexpired lease.
    Renew(SorafsRepairRenewV1),
    /// Commit the task's single successful terminal outcome.
    Complete(SorafsRepairCompleteV1),
    /// Commit the task's single unsuccessful terminal outcome without slashing.
    Fail(SorafsRepairFailV1),
    /// Commit an escalated terminal outcome and slash proposal atomically.
    Escalate(SorafsRepairEscalateV1),
}

impl SorafsRepairTaskActionV1 {
    /// Return the caller-supplied idempotency key.
    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        match self {
            Self::Claim(action) => &action.idempotency_key,
            Self::Renew(action) => &action.idempotency_key,
            Self::Complete(action) => &action.idempotency_key,
            Self::Fail(action) => &action.idempotency_key,
            Self::Escalate(action) => &action.idempotency_key,
        }
    }
}

isi! {
    /// Admit one exact canonical repair report under a subsystem exactly-once identity.
    pub struct SubmitSorafsRepairTask {
        /// Non-zero source identity. `PoTR` uses the signed receipt digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_identity: [u8; 32],
        /// Exact canonical `sorafs_manifest::repair::RepairReportV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub report_payload: Vec<u8>,
    }
}

impl crate::seal::Instruction for SubmitSorafsRepairTask {}

isi! {
    /// Apply one compare-and-set repair lease or terminal transition.
    pub struct ApplySorafsRepairTaskAction {
        /// Canonical repair ticket identifier.
        pub ticket_id: String,
        /// Exact task revision observed by the submitter.
        pub expected_revision: u64,
        /// Typed mutation to commit.
        pub action: SorafsRepairTaskActionV1,
    }
}

impl crate::seal::Instruction for ApplySorafsRepairTaskAction {}

isi! {
    /// Commit the provider owner's single appeal against an escalated repair slash.
    pub struct SubmitSorafsRepairAppeal {
        /// Canonical repair ticket identifier.
        pub ticket_id: String,
        /// Exact task revision observed by the submitter.
        pub expected_revision: u64,
        /// Digest of external appeal evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub evidence_digest: [u8; 32],
        /// Bounded payload-free appeal reason.
        pub reason: String,
        /// Bounded caller key used for exact replay handling.
        pub idempotency_key: String,
    }
}

impl crate::seal::Instruction for SubmitSorafsRepairAppeal {}

/// Canonical PDP proof material accepted by the chain-authoritative outcome journal.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SorafsPdpProofOutcomeSubmissionV1 {
    /// Exact canonical `sorafs_manifest::PdpGovernanceArchiveV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub archive_payload: Vec<u8>,
}

/// Canonical PoTR proof material accepted by the chain-authoritative outcome journal.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SorafsPotrProofOutcomeSubmissionV1 {
    /// Exact canonical dual-signed `sorafs_manifest::PotrReceiptV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub receipt_payload: Vec<u8>,
    /// Council-verified admission envelope captured during receipt validation.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub admission_envelope_digest: [u8; 32],
}

/// Existing canonical proof material accepted by the chain-authoritative outcome journal.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "proof_kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum SorafsProofOutcomeSubmissionV1 {
    /// Exact canonical PDP terminal archive and authentication material.
    #[codec(index = 0)]
    Pdp(SorafsPdpProofOutcomeSubmissionV1),
    /// Exact canonical dual-signed PoTR receipt and admission binding.
    #[codec(index = 1)]
    Potr(SorafsPotrProofOutcomeSubmissionV1),
}

isi! {
    /// Activate or rotate provider-scoped governed keys for PDP and PoTR outcome validation.
    pub struct SetSorafsProofOutcomeSignerPolicy {
        /// Monotonic provider-scoped signer policy.
        pub policy: ProofOutcomeSignerPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsProofOutcomeSignerPolicy {}

isi! {
    /// Commit one validated PDP or PoTR terminal outcome.
    pub struct SubmitSorafsProofOutcome {
        /// Existing canonical proof/archive material; no competing receipt schema is accepted.
        pub submission: SorafsProofOutcomeSubmissionV1,
    }
}

impl crate::seal::Instruction for SubmitSorafsProofOutcome {}

isi! {
    /// Activate the next authoritative `SoraFS` moderation-ledger policy revision.
    pub struct SetSorafsModerationPolicy {
        /// Policy revision to validate and activate.
        pub policy: ModerationLedgerPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsModerationPolicy {}

isi! {
    /// Admit one appellant-authenticated moderation appeal and pin active `PoP` anchors.
    pub struct SubmitSorafsModerationAppeal {
        /// Immutable bounded appeal intake.
        pub intake: ModerationAppealIntakeV1,
    }
}

impl crate::seal::Instruction for SubmitSorafsModerationAppeal {}

isi! {
    /// Register one authority-bound private `PoP` membership proof for panel eligibility.
    pub struct RegisterSorafsModerationJurorEligibility {
        /// Appeal case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Exact canonical Norito `PopMembershipProofV1` bytes; never persisted.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub membership_proof_payload: Vec<u8>,
    }
}

impl crate::seal::Instruction for RegisterSorafsModerationJurorEligibility {}

isi! {
    /// Close eligibility registration and persist the uniquely deterministic panel draw.
    pub struct FinalizeSorafsModerationSortition {
        /// Appeal case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Exact pinned `PoP` snapshot digest expected by the operator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub pop_snapshot_digest: [u8; 32],
        /// Exact latest committed parent hash expected to seed the draw.
        ///
        /// Native execution requires this anchor to match consensus state after
        /// registration closes, preventing applicants or candidates from
        /// precomputing and selectively entering a favorable draw.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub randomness_anchor: [u8; 32],
        /// Proposed primary roster; execution recomputes and rejects biased input.
        pub proposed_jurors: Vec<AccountId>,
        /// Proposed failover order; execution recomputes and rejects biased input.
        pub proposed_waitlist: Vec<AccountId>,
    }
}

impl crate::seal::Instruction for FinalizeSorafsModerationSortition {}

isi! {
    /// Accept one authority-bound primary moderation-juror assignment.
    pub struct AcceptSorafsModerationJurorAssignment {
        /// Appeal case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Exact sortition digest expected by the juror.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sortition_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for AcceptSorafsModerationJurorAssignment {}

isi! {
    /// Apply deterministic no-show replacements and activate commit/reveal atomically.
    pub struct ActivateSorafsModerationCase {
        /// Appeal case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Exact sortition digest expected by the operator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sortition_digest: [u8; 32],
    }
}

impl crate::seal::Instruction for ActivateSorafsModerationCase {}

isi! {
    /// Submit one canonical juror commitment to an authoritative moderation case.
    pub struct SubmitSorafsModerationCommit {
        /// Exact canonical Norito `SoraFsModerationBallotCommitV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub commit_payload: Vec<u8>,
    }
}

impl crate::seal::Instruction for SubmitSorafsModerationCommit {}

isi! {
    /// Raise one bounded payload-free challenge during a moderation challenge window.
    pub struct RaiseSorafsModerationChallenge {
        /// Moderation case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Challenge id unique within the case and round.
        pub challenge_id: String,
        /// Fixed challenge category.
        pub kind: ModerationChallengeKindV1,
        /// Optional canonical juror target.
        pub target_juror: Option<AccountId>,
        /// Digest of external evidence; raw evidence is not placed on-chain.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub evidence_digest: [u8; 32],
        /// Bounded payload-free reason label.
        pub reason: String,
    }
}

impl crate::seal::Instruction for RaiseSorafsModerationChallenge {}

isi! {
    /// Resolve one pending authoritative moderation challenge.
    pub struct ResolveSorafsModerationChallenge {
        /// Moderation case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
        /// Existing challenge identifier.
        pub challenge_id: String,
        /// Governance decision.
        pub decision: ModerationChallengeDecisionV1,
    }
}

impl crate::seal::Instruction for ResolveSorafsModerationChallenge {}

isi! {
    /// Submit one canonical juror reveal to an authoritative moderation case.
    pub struct SubmitSorafsModerationReveal {
        /// Exact canonical Norito `SoraFsModerationBallotRevealV1` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub reveal_payload: Vec<u8>,
    }
}

impl crate::seal::Instruction for SubmitSorafsModerationReveal {}

isi! {
    /// Finalize a closed moderation case and atomically record outcome and no-shows.
    pub struct FinalizeSorafsModerationCase {
        /// Moderation case identifier.
        pub case_id: String,
        /// Ballot round identifier.
        pub round_id: String,
    }
}

impl crate::seal::Instruction for FinalizeSorafsModerationCase {}

impl RegisterPinManifest {
    /// Create a new `RegisterPinManifest` instruction.
    #[must_use]
    pub fn new(
        manifest_payload: Vec<u8>,
        submitted_epoch: u64,
        alias: Option<ManifestAliasBinding>,
        successor_of: Option<ManifestDigest>,
    ) -> Self {
        Self {
            manifest_payload,
            submitted_epoch,
            alias,
            successor_of,
        }
    }
}

impl ApprovePinManifest {
    /// Create a new `ApprovePinManifest` instruction.
    #[must_use]
    pub fn new(
        digest: ManifestDigest,
        approved_epoch: u64,
        council_envelope: Option<Vec<u8>>,
        council_envelope_digest: Option<[u8; 32]>,
    ) -> Self {
        Self {
            digest,
            approved_epoch,
            council_envelope,
            council_envelope_digest,
        }
    }
}

impl RetirePinManifest {
    /// Create a new `RetirePinManifest` instruction.
    #[must_use]
    pub fn new(digest: ManifestDigest, retired_epoch: u64, reason: Option<String>) -> Self {
        Self {
            digest,
            retired_epoch,
            reason,
        }
    }
}

impl BindManifestAlias {
    /// Create a new `BindManifestAlias` instruction.
    #[must_use]
    pub fn new(
        digest: ManifestDigest,
        binding: ManifestAliasBinding,
        bound_epoch: u64,
        expiry_epoch: u64,
    ) -> Self {
        Self {
            digest,
            binding,
            bound_epoch,
            expiry_epoch,
        }
    }
}

impl RegisterCapacityDeclaration {
    /// Create a new `RegisterCapacityDeclaration` instruction.
    #[must_use]
    pub fn new(record: CapacityDeclarationRecord) -> Self {
        Self { record }
    }
}

impl RecordCapacityTelemetry {
    /// Create a new `RecordCapacityTelemetry` instruction.
    #[must_use]
    pub fn new(record: CapacityTelemetryRecord) -> Self {
        Self { record }
    }
}

impl RegisterCapacityDispute {
    /// Create a new `RegisterCapacityDispute` instruction.
    #[must_use]
    pub fn new(record: CapacityDisputeRecord) -> Self {
        Self { record }
    }
}

impl IssueReplicationOrder {
    /// Create a new `IssueReplicationOrder` instruction.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        order_id: ReplicationOrderId,
        order_payload: Vec<u8>,
        issued_epoch: u64,
        deadline_epoch: u64,
    ) -> Self {
        Self {
            order_id,
            order_payload,
            issued_epoch,
            deadline_epoch,
        }
    }
}

impl CompleteReplicationOrder {
    /// Create a new `CompleteReplicationOrder` instruction.
    #[must_use]
    pub fn new(order_id: ReplicationOrderId, completion_epoch: u64) -> Self {
        Self {
            order_id,
            completion_epoch,
        }
    }
}

impl ExpireReplicationOrder {
    /// Create a new `ExpireReplicationOrder` instruction.
    #[must_use]
    pub fn new(order_id: ReplicationOrderId, expiration_epoch: u64) -> Self {
        Self {
            order_id,
            expiration_epoch,
        }
    }
}

impl SetPricingSchedule {
    /// Create a new `SetPricingSchedule` instruction.
    #[must_use]
    pub fn new(schedule: PricingScheduleRecord) -> Self {
        Self { schedule }
    }
}

impl UpsertProviderCredit {
    /// Create a new `UpsertProviderCredit` instruction.
    #[must_use]
    pub fn new(record: ProviderCreditRecord) -> Self {
        Self { record }
    }
}

impl SetSorafsPopIssuerPolicy {
    /// Construct an issuer-policy activation instruction.
    #[must_use]
    pub fn new(policy: PopIssuerPolicyV1) -> Self {
        Self { policy }
    }
}

impl CommitSorafsPopCredentialBatch {
    /// Construct an atomic credential commitment batch instruction.
    #[must_use]
    pub fn new(batch_payload: Vec<u8>) -> Self {
        Self { batch_payload }
    }
}

impl PublishSorafsPopRevocationList {
    /// Construct a signed revocation publication instruction.
    #[must_use]
    pub fn new(revocation_list_payload: Vec<u8>, issuer_policy_digest: [u8; 32]) -> Self {
        Self {
            revocation_list_payload,
            issuer_policy_digest,
        }
    }
}

impl SetSorafsOrderbookPolicy {
    /// Construct a policy-activation instruction.
    #[must_use]
    pub fn new(policy: OrderbookAdmissionPolicyV1) -> Self {
        Self { policy }
    }
}

impl SubmitSorafsOrderbookOrder {
    /// Construct a signed order-submission instruction.
    #[must_use]
    pub fn new(order_payload: Vec<u8>, policy_digest: [u8; 32]) -> Self {
        Self {
            order_payload,
            policy_digest,
        }
    }
}

impl CancelSorafsOrderbookOrder {
    /// Construct a signed owner-cancellation instruction.
    #[must_use]
    pub fn new(cancel_payload: Vec<u8>, policy_digest: [u8; 32]) -> Self {
        Self {
            cancel_payload,
            policy_digest,
        }
    }
}

impl MatchSorafsOrderbook {
    /// Construct a bounded deterministic matching instruction.
    #[must_use]
    pub const fn new(policy_digest: [u8; 32], expected_book_revision: u64, max_fills: u32) -> Self {
        Self {
            policy_digest,
            expected_book_revision,
            max_fills,
        }
    }
}

impl MaintainSorafsOrderbook {
    /// Construct a bounded order/channel expiry instruction.
    #[must_use]
    pub const fn new(policy_digest: [u8; 32], expected_book_revision: u64, max_items: u32) -> Self {
        Self {
            policy_digest,
            expected_book_revision,
            max_items,
        }
    }
}

impl RecordSorafsOrderbookSettlementReceipt {
    /// Construct a signed funded-lock settlement and receipt instruction.
    #[must_use]
    pub fn new(receipt_payload: Vec<u8>, policy_digest: [u8; 32]) -> Self {
        Self {
            receipt_payload,
            policy_digest,
        }
    }
}

impl SetSorafsReservePolicy {
    /// Construct an authoritative reserve policy activation.
    #[must_use]
    pub fn new(policy: ReserveAuthorityPolicyV1) -> Self {
        Self { policy }
    }
}

impl RegisterSorafsReserveAccount {
    /// Construct a provider reserve-account registration.
    #[must_use]
    pub fn new(terms: ReserveProviderTermsV1, policy_digest: [u8; 32]) -> Self {
        Self {
            terms,
            policy_digest,
        }
    }
}

impl RequestSorafsReserveMovement {
    /// Construct a provider reserve movement request.
    #[must_use]
    pub fn new(
        movement_id: [u8; 32],
        provider_id: ProviderId,
        kind: ReserveMovementKindV1,
        amount: XorQuantity,
        expected_provider_revision: u64,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            movement_id,
            provider_id,
            kind,
            amount,
            expected_provider_revision,
            policy_digest,
        }
    }
}

impl DecideSorafsReserveMovement {
    /// Construct a terminal reserve movement decision.
    #[must_use]
    pub fn new(
        movement_id: [u8; 32],
        expected_provider_revision: u64,
        policy_digest: [u8; 32],
        approve: bool,
        rationale: String,
    ) -> Self {
        Self {
            movement_id,
            expected_provider_revision,
            policy_digest,
            approve,
            rationale,
        }
    }
}

impl ChargeSorafsReserveRent {
    /// Construct a deterministic reserve rent charge.
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        expected_provider_revision: u64,
        billing_periods: u16,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            provider_id,
            expected_provider_revision,
            billing_periods,
            policy_digest,
        }
    }
}

impl AdvanceSorafsReserveLifecycle {
    /// Construct a reserve lifecycle transition.
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        expected_provider_revision: u64,
        days_past_due: u16,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            provider_id,
            expected_provider_revision,
            days_past_due,
            policy_digest,
        }
    }
}

impl DrawSorafsReserveCredit {
    /// Construct a capped reserve-credit draw.
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        expected_provider_revision: u64,
        amount: XorQuantity,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            provider_id,
            expected_provider_revision,
            amount,
            policy_digest,
        }
    }
}

impl RepaySorafsReserveCredit {
    /// Construct a reserve-credit repayment.
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        expected_provider_revision: u64,
        amount: XorQuantity,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            provider_id,
            expected_provider_revision,
            amount,
            policy_digest,
        }
    }
}

impl SubmitSorafsReserveAppeal {
    /// Construct a provider reserve lifecycle appeal.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        appeal_id: [u8; 32],
        provider_id: ProviderId,
        expected_provider_revision: u64,
        requested_stage: ReserveLifecycleStage,
        reason: String,
        evidence_digest: Option<[u8; 32]>,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            appeal_id,
            provider_id,
            expected_provider_revision,
            requested_stage,
            reason,
            evidence_digest,
            policy_digest,
        }
    }
}

impl DecideSorafsReserveAppeal {
    /// Construct a terminal reserve appeal decision.
    #[must_use]
    pub fn new(
        appeal_id: [u8; 32],
        expected_provider_revision: u64,
        policy_digest: [u8; 32],
        accept: bool,
        rationale: String,
    ) -> Self {
        Self {
            appeal_id,
            expected_provider_revision,
            policy_digest,
            accept,
            rationale,
        }
    }
}

impl SubmitSorafsRepairTask {
    /// Construct an exactly-once repair-task admission instruction.
    #[must_use]
    pub fn new(source_identity: [u8; 32], report_payload: Vec<u8>) -> Self {
        Self {
            source_identity,
            report_payload,
        }
    }
}

impl ApplySorafsRepairTaskAction {
    /// Construct a compare-and-set repair-task mutation.
    #[must_use]
    pub fn new(
        ticket_id: String,
        expected_revision: u64,
        action: SorafsRepairTaskActionV1,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            action,
        }
    }
}

impl SubmitSorafsRepairAppeal {
    /// Construct a provider-owner repair slash appeal.
    #[must_use]
    pub fn new(
        ticket_id: String,
        expected_revision: u64,
        evidence_digest: [u8; 32],
        reason: String,
        idempotency_key: String,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            evidence_digest,
            reason,
            idempotency_key,
        }
    }
}

impl SetSorafsProofOutcomeSignerPolicy {
    /// Construct a governed proof-signer policy activation.
    #[must_use]
    pub fn new(policy: ProofOutcomeSignerPolicyV1) -> Self {
        Self { policy }
    }
}

impl SubmitSorafsProofOutcome {
    /// Construct a canonical proof-outcome submission.
    #[must_use]
    pub fn new(submission: SorafsProofOutcomeSubmissionV1) -> Self {
        Self { submission }
    }
}

impl SetSorafsModerationPolicy {
    /// Construct a moderation policy activation instruction.
    #[must_use]
    pub fn new(policy: ModerationLedgerPolicyV1) -> Self {
        Self { policy }
    }
}

impl SubmitSorafsModerationAppeal {
    /// Construct an appellant-authenticated appeal-intake instruction.
    #[must_use]
    pub fn new(intake: ModerationAppealIntakeV1) -> Self {
        Self { intake }
    }
}

impl RegisterSorafsModerationJurorEligibility {
    /// Construct a private `PoP` eligibility-proof submission.
    #[must_use]
    pub fn new(case_id: String, round_id: String, membership_proof_payload: Vec<u8>) -> Self {
        Self {
            case_id,
            round_id,
            membership_proof_payload,
        }
    }
}

impl FinalizeSorafsModerationSortition {
    /// Construct a deterministic sortition-finalization instruction.
    #[must_use]
    pub fn new(
        case_id: String,
        round_id: String,
        pop_snapshot_digest: [u8; 32],
        randomness_anchor: [u8; 32],
        proposed_jurors: Vec<AccountId>,
        proposed_waitlist: Vec<AccountId>,
    ) -> Self {
        Self {
            case_id,
            round_id,
            pop_snapshot_digest,
            randomness_anchor,
            proposed_jurors,
            proposed_waitlist,
        }
    }
}

impl AcceptSorafsModerationJurorAssignment {
    /// Construct an authority-bound assignment acceptance.
    #[must_use]
    pub fn new(case_id: String, round_id: String, sortition_digest: [u8; 32]) -> Self {
        Self {
            case_id,
            round_id,
            sortition_digest,
        }
    }
}

impl ActivateSorafsModerationCase {
    /// Construct a deterministic failover and ballot-activation instruction.
    #[must_use]
    pub fn new(case_id: String, round_id: String, sortition_digest: [u8; 32]) -> Self {
        Self {
            case_id,
            round_id,
            sortition_digest,
        }
    }
}

impl SubmitSorafsModerationCommit {
    /// Construct a canonical commitment submission.
    #[must_use]
    pub fn new(commit_payload: Vec<u8>) -> Self {
        Self { commit_payload }
    }
}

impl RaiseSorafsModerationChallenge {
    /// Construct a payload-free challenge submission.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        case_id: String,
        round_id: String,
        challenge_id: String,
        kind: ModerationChallengeKindV1,
        target_juror: Option<AccountId>,
        evidence_digest: [u8; 32],
        reason: String,
    ) -> Self {
        Self {
            case_id,
            round_id,
            challenge_id,
            kind,
            target_juror,
            evidence_digest,
            reason,
        }
    }
}

impl ResolveSorafsModerationChallenge {
    /// Construct a challenge-resolution instruction.
    #[must_use]
    pub fn new(
        case_id: String,
        round_id: String,
        challenge_id: String,
        decision: ModerationChallengeDecisionV1,
    ) -> Self {
        Self {
            case_id,
            round_id,
            challenge_id,
            decision,
        }
    }
}

impl SubmitSorafsModerationReveal {
    /// Construct a canonical reveal submission.
    #[must_use]
    pub fn new(reveal_payload: Vec<u8>) -> Self {
        Self { reveal_payload }
    }
}

impl FinalizeSorafsModerationCase {
    /// Construct a terminal case-finalization instruction.
    #[must_use]
    pub fn new(case_id: String, round_id: String) -> Self {
        Self { case_id, round_id }
    }
}

impl RegisterProviderOwner {
    /// Create a new `RegisterProviderOwner` instruction.
    #[must_use]
    pub fn new(provider_id: ProviderId, owner: AccountId) -> Self {
        Self { provider_id, owner }
    }
}

impl UnregisterProviderOwner {
    /// Create a new `UnregisterProviderOwner` instruction.
    #[must_use]
    pub fn new(provider_id: ProviderId) -> Self {
        Self { provider_id }
    }
}

fn sorafs_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_sorafs_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = sorafs_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_sorafs_decode_from_slice!(RegisterPinManifest {
    manifest_payload: Vec<u8>,
    submitted_epoch: u64,
    alias: Option<ManifestAliasBinding>,
    successor_of: Option<ManifestDigest>,
});

impl_sorafs_decode_from_slice!(ApprovePinManifest {
    digest: ManifestDigest,
    approved_epoch: u64,
    council_envelope: Option<Vec<u8>>,
    council_envelope_digest: Option<[u8; 32]>,
});

impl_sorafs_decode_from_slice!(RetirePinManifest {
    digest: ManifestDigest,
    retired_epoch: u64,
    reason: Option<String>,
});

impl_sorafs_decode_from_slice!(BindManifestAlias {
    digest: ManifestDigest,
    binding: ManifestAliasBinding,
    bound_epoch: u64,
    expiry_epoch: u64,
});

impl_sorafs_decode_from_slice!(RegisterCapacityDeclaration {
    record: CapacityDeclarationRecord,
});

impl_sorafs_decode_from_slice!(RecordCapacityTelemetry {
    record: CapacityTelemetryRecord,
});

impl_sorafs_decode_from_slice!(RegisterCapacityDispute {
    record: CapacityDisputeRecord,
});

impl_sorafs_decode_from_slice!(IssueReplicationOrder {
    order_id: ReplicationOrderId,
    order_payload: Vec<u8>,
    issued_epoch: u64,
    deadline_epoch: u64,
});

impl_sorafs_decode_from_slice!(CompleteReplicationOrder {
    order_id: ReplicationOrderId,
    completion_epoch: u64,
});

impl_sorafs_decode_from_slice!(ExpireReplicationOrder {
    order_id: ReplicationOrderId,
    expiration_epoch: u64,
});

impl_sorafs_decode_from_slice!(RegisterProviderOwner {
    provider_id: ProviderId,
    owner: AccountId,
});

impl_sorafs_decode_from_slice!(UnregisterProviderOwner {
    provider_id: ProviderId,
});

impl_sorafs_decode_from_slice!(SetPricingSchedule {
    schedule: PricingScheduleRecord,
});

impl_sorafs_decode_from_slice!(UpsertProviderCredit {
    record: ProviderCreditRecord,
});

impl_sorafs_decode_from_slice!(SetSorafsPopIssuerPolicy {
    policy: PopIssuerPolicyV1,
});

impl_sorafs_decode_from_slice!(CommitSorafsPopCredentialBatch {
    batch_payload: Vec<u8>,
});

impl_sorafs_decode_from_slice!(PublishSorafsPopRevocationList {
    revocation_list_payload: Vec<u8>,
    issuer_policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(SetSorafsOrderbookPolicy {
    policy: OrderbookAdmissionPolicyV1,
});

impl_sorafs_decode_from_slice!(SubmitSorafsOrderbookOrder {
    order_payload: Vec<u8>,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(CancelSorafsOrderbookOrder {
    cancel_payload: Vec<u8>,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(MatchSorafsOrderbook {
    policy_digest: [u8; 32],
    expected_book_revision: u64,
    max_fills: u32,
});

impl_sorafs_decode_from_slice!(MaintainSorafsOrderbook {
    policy_digest: [u8; 32],
    expected_book_revision: u64,
    max_items: u32,
});

impl_sorafs_decode_from_slice!(RecordSorafsOrderbookSettlementReceipt {
    receipt_payload: Vec<u8>,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(SetSorafsReservePolicy {
    policy: ReserveAuthorityPolicyV1,
});

impl_sorafs_decode_from_slice!(RegisterSorafsReserveAccount {
    terms: ReserveProviderTermsV1,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(RequestSorafsReserveMovement {
    movement_id: [u8; 32],
    provider_id: ProviderId,
    kind: ReserveMovementKindV1,
    amount: XorQuantity,
    expected_provider_revision: u64,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(DecideSorafsReserveMovement {
    movement_id: [u8; 32],
    expected_provider_revision: u64,
    policy_digest: [u8; 32],
    approve: bool,
    rationale: String,
});

impl_sorafs_decode_from_slice!(ChargeSorafsReserveRent {
    provider_id: ProviderId,
    expected_provider_revision: u64,
    billing_periods: u16,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(AdvanceSorafsReserveLifecycle {
    provider_id: ProviderId,
    expected_provider_revision: u64,
    days_past_due: u16,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(DrawSorafsReserveCredit {
    provider_id: ProviderId,
    expected_provider_revision: u64,
    amount: XorQuantity,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(RepaySorafsReserveCredit {
    provider_id: ProviderId,
    expected_provider_revision: u64,
    amount: XorQuantity,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(SubmitSorafsReserveAppeal {
    appeal_id: [u8; 32],
    provider_id: ProviderId,
    expected_provider_revision: u64,
    requested_stage: ReserveLifecycleStage,
    reason: String,
    evidence_digest: Option<[u8; 32]>,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(DecideSorafsReserveAppeal {
    appeal_id: [u8; 32],
    expected_provider_revision: u64,
    policy_digest: [u8; 32],
    accept: bool,
    rationale: String,
});

impl_sorafs_decode_from_slice!(SubmitSorafsRepairTask {
    source_identity: [u8; 32],
    report_payload: Vec<u8>,
});

impl_sorafs_decode_from_slice!(ApplySorafsRepairTaskAction {
    ticket_id: String,
    expected_revision: u64,
    action: SorafsRepairTaskActionV1,
});

impl_sorafs_decode_from_slice!(SubmitSorafsRepairAppeal {
    ticket_id: String,
    expected_revision: u64,
    evidence_digest: [u8; 32],
    reason: String,
    idempotency_key: String,
});

impl_sorafs_decode_from_slice!(SetSorafsProofOutcomeSignerPolicy {
    policy: ProofOutcomeSignerPolicyV1,
});

impl_sorafs_decode_from_slice!(SubmitSorafsProofOutcome {
    submission: SorafsProofOutcomeSubmissionV1,
});

impl_sorafs_decode_from_slice!(SetSorafsModerationPolicy {
    policy: ModerationLedgerPolicyV1,
});

impl_sorafs_decode_from_slice!(SubmitSorafsModerationAppeal {
    intake: ModerationAppealIntakeV1,
});

impl_sorafs_decode_from_slice!(RegisterSorafsModerationJurorEligibility {
    case_id: String,
    round_id: String,
    membership_proof_payload: Vec<u8>,
});

impl_sorafs_decode_from_slice!(FinalizeSorafsModerationSortition {
    case_id: String,
    round_id: String,
    pop_snapshot_digest: [u8; 32],
    randomness_anchor: [u8; 32],
    proposed_jurors: Vec<AccountId>,
    proposed_waitlist: Vec<AccountId>,
});

impl_sorafs_decode_from_slice!(AcceptSorafsModerationJurorAssignment {
    case_id: String,
    round_id: String,
    sortition_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(ActivateSorafsModerationCase {
    case_id: String,
    round_id: String,
    sortition_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(SubmitSorafsModerationCommit {
    commit_payload: Vec<u8>,
});

impl_sorafs_decode_from_slice!(RaiseSorafsModerationChallenge {
    case_id: String,
    round_id: String,
    challenge_id: String,
    kind: ModerationChallengeKindV1,
    target_juror: Option<AccountId>,
    evidence_digest: [u8; 32],
    reason: String,
});

impl_sorafs_decode_from_slice!(ResolveSorafsModerationChallenge {
    case_id: String,
    round_id: String,
    challenge_id: String,
    decision: ModerationChallengeDecisionV1,
});

impl_sorafs_decode_from_slice!(SubmitSorafsModerationReveal {
    reveal_payload: Vec<u8>,
});

impl_sorafs_decode_from_slice!(FinalizeSorafsModerationCase {
    case_id: String,
    round_id: String,
});

#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::{Numeric, Quantity};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::sorafs::capacity::{CapacityDisputeEvidence, CapacityDisputeId};

    fn owner() -> AccountId {
        AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key"),
        )
    }

    fn digest(byte: u8) -> ManifestDigest {
        ManifestDigest::new([byte; 32])
    }

    fn provider(byte: u8) -> ProviderId {
        ProviderId::new([byte; 32])
    }

    fn xor_quantity_nanos(value: u128) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(
            value,
            crate::sorafs::pricing::XOR_QUANTITY_SCALE,
        ))
        .expect("u128 nano-XOR SoraFS fixture fits Quantity")
    }

    fn order_id() -> ReplicationOrderId {
        ReplicationOrderId::new([0x44; 32])
    }

    fn alias() -> ManifestAliasBinding {
        ManifestAliasBinding {
            namespace: "sora".to_owned(),
            name: "docs".to_owned(),
            proof: vec![0xAA, 0xBB],
        }
    }

    fn capacity_declaration() -> CapacityDeclarationRecord {
        CapacityDeclarationRecord::new(
            provider(0x31),
            vec![0x01, 0x02, 0x03],
            512,
            10,
            12,
            120,
            Metadata::default(),
        )
    }

    fn capacity_telemetry() -> CapacityTelemetryRecord {
        CapacityTelemetryRecord::new(
            provider(0x32),
            20,
            21,
            512,
            500,
            480,
            4,
            3,
            9_999,
            9_998,
            1_024,
            5,
            1,
            2,
            0,
        )
        .with_nonce(7)
    }

    fn capacity_dispute() -> CapacityDisputeRecord {
        CapacityDisputeRecord::new_pending(
            CapacityDisputeId::new([0xD1; 32]),
            provider(0x33),
            [0xC1; 32],
            Some([0x44; 32]),
            1,
            30,
            "provider missed replication target".to_owned(),
            Some("replicate to replacement provider".to_owned()),
            CapacityDisputeEvidence {
                digest: [0xE1; 32],
                media_type: Some("application/norito".to_owned()),
                uri: Some("sorafs://evidence".to_owned()),
                size_bytes: Some(4096),
            },
            vec![0x99, 0x88],
        )
    }

    fn provider_credit() -> ProviderCreditRecord {
        ProviderCreditRecord::new(
            provider(0x34),
            xor_quantity_nanos(10_000),
            xor_quantity_nanos(20_000),
            xor_quantity_nanos(15_000),
            xor_quantity_nanos(1_000),
            100,
            200,
            Metadata::default(),
        )
    }

    fn orderbook_policy() -> OrderbookAdmissionPolicyV1 {
        OrderbookAdmissionPolicyV1 {
            version: crate::sorafs::orderbook::ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
            matcher_authority: owner(),
            settlement_authority: owner(),
            paused: false,
            min_order_gib: 1,
            max_order_gib: 1_024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 86_400,
            max_receipt_age_secs: 3_600,
            max_clock_skew_secs: 30,
            max_receipt_bytes: 1 << 30,
            max_receipts_per_channel: 1_024,
        }
    }

    fn reserve_policy() -> ReserveAuthorityPolicyV1 {
        let domain = crate::domain::DomainId::try_new("sora", "universal").expect("reserve domain");
        ReserveAuthorityPolicyV1 {
            version: crate::sorafs::reserve::RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: crate::sorafs::reserve::ReservePolicyV1::default(),
            asset_definition: crate::asset::AssetDefinitionId::new(
                domain,
                "xor".parse().expect("reserve asset name"),
            ),
            custody_account: owner(),
            treasury_account: AccountId::new(
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                    .parse()
                    .expect("treasury public key"),
            ),
            operations_authority: owner(),
            decision_authority: owner(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
                .expect("reserve debt cap"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        }
    }

    #[test]
    fn reserve_policy_digest_commits_service_authorities() {
        let policy = reserve_policy();
        let baseline = policy.digest().expect("baseline reserve policy digest");

        let mut operations_rotated = policy.clone();
        operations_rotated.operations_authority = policy.treasury_account.clone();
        assert_ne!(
            operations_rotated
                .digest()
                .expect("operations-authority digest"),
            baseline
        );

        let mut decision_rotated = policy;
        decision_rotated.decision_authority = decision_rotated.treasury_account.clone();
        assert_ne!(
            decision_rotated
                .digest()
                .expect("decision-authority digest"),
            baseline
        );
    }

    fn reserve_terms() -> ReserveProviderTermsV1 {
        ReserveProviderTermsV1 {
            provider_id: provider(0x36),
            provider_account: owner(),
            tier: crate::sorafs::reserve::ReserveTier::TierA,
            storage_class: crate::sorafs::pin_registry::StorageClass::Hot,
            duration: crate::sorafs::reserve::ReserveDuration::Monthly,
            capacity_gib: 16,
        }
    }

    fn pop_issuer_policy() -> PopIssuerPolicyV1 {
        PopIssuerPolicyV1 {
            version: crate::sorafs::pop_registry::POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_account: owner(),
            issuer_public_key: [1; 32],
            max_credentials_per_batch: 16,
            max_revocations_per_publication: 16,
            max_credential_lifetime_secs: 86_400,
            max_future_clock_skew_secs: 30,
            paused: false,
        }
    }

    fn moderation_policy() -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: crate::sorafs::moderation_ledger::MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            max_panel_size: 5,
            max_candidate_pool_size: 32,
            max_waitlist_size: 5,
            max_exclusions_per_case: 16,
            max_total_window_ms: 60_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        }
    }

    fn proof_outcome_signer_policy() -> ProofOutcomeSignerPolicyV1 {
        ProofOutcomeSignerPolicyV1 {
            version: crate::sorafs::proof_ledger::PROOF_OUTCOME_SIGNER_POLICY_VERSION_V1,
            provider_id: provider(0x74),
            revision: 1,
            predecessor_digest: None,
            admission_envelope_digest: [0x75; 32],
            pdp_public_key: [0x76; 32],
            potr_mldsa_public_key: vec![0x77, 0x78],
            gateway_public_key: [0x79; 32],
            valid_from_unix: 1_000,
            valid_until_unix: 2_000,
        }
    }

    fn moderation_appeal_intake() -> ModerationAppealIntakeV1 {
        let appellant = owner();
        ModerationAppealIntakeV1 {
            version: crate::sorafs::moderation_ledger::MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: "appeal-1".to_owned(),
            round_id: "round-1".to_owned(),
            appellant: appellant.clone(),
            appealed_decision_digest: [0x61; 32],
            proof_token_digest: [0x62; 32],
            evidence_bundle_digest: [0x63; 32],
            appeal_deposit_lock_digest: [0x64; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "policy-v1".to_owned(),
            evidence_uri: None,
            panel_size: 1,
            waitlist_size: 1,
            quorum: 1,
            exclusions: vec![appellant],
            registration_deadline_unix_ms: 1_000,
            acceptance_deadline_unix_ms: 2_000,
            commit_deadline_unix_ms: 3_000,
            challenge_deadline_unix_ms: 4_000,
            reveal_deadline_unix_ms: 5_000,
            policy_digest: moderation_policy().digest().expect("policy digest"),
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[cfg(feature = "json")]
    #[test]
    fn register_pin_manifest_json_roundtrip() {
        let manifest = RegisterPinManifest::new(vec![1, 2, 3], 42, None, None);

        let value = norito::json::to_value(&manifest).expect("register pin manifest json");
        let decoded: RegisterPinManifest =
            norito::json::from_value(value).expect("register pin manifest decode");

        assert_eq!(decoded, manifest);
    }

    #[cfg(feature = "json")]
    #[test]
    fn proof_outcome_submission_json_is_tagged_exact_and_fail_closed() {
        let pdp = SorafsProofOutcomeSubmissionV1::Pdp(SorafsPdpProofOutcomeSubmissionV1 {
            archive_payload: vec![0, 1, 2],
        });
        let expected_pdp: norito::json::Value =
            norito::json::from_str(r#"{"proof_kind":"pdp","value":{"archive_payload":"AAEC"}}"#)
                .expect("valid expected PDP JSON");
        assert_eq!(
            norito::json::to_value(&pdp).expect("encode PDP submission"),
            expected_pdp
        );
        assert_eq!(
            norito::json::from_value::<SorafsProofOutcomeSubmissionV1>(expected_pdp)
                .expect("decode PDP submission"),
            pdp
        );

        let potr = SorafsProofOutcomeSubmissionV1::Potr(SorafsPotrProofOutcomeSubmissionV1 {
            receipt_payload: vec![4, 5],
            admission_envelope_digest: [7; 32],
        });
        let expected_potr: norito::json::Value = norito::json::from_str(&format!(
            r#"{{"proof_kind":"potr","value":{{"receipt_payload":"BAU=","admission_envelope_digest":[{}]}}}}"#,
            std::iter::repeat_n("7", 32).collect::<Vec<_>>().join(",")
        ))
        .expect("valid expected PoTR JSON");
        assert_eq!(
            norito::json::to_value(&potr).expect("encode PoTR submission"),
            expected_potr
        );
        assert_eq!(
            norito::json::from_value::<SorafsProofOutcomeSubmissionV1>(expected_potr)
                .expect("decode PoTR submission"),
            potr
        );

        for malformed in [
            r#"{"proof_kind":"unknown","value":{"archive_payload":"AAEC"}}"#,
            r#"{"proof_kind":"pdp","value":{"archive_payload":"***"}}"#,
            r#"{"proof_kind":"potr","value":{"receipt_payload":"BAU="}}"#,
            r#"{"proof_kind":"potr","value":{"receipt_payload":"BAU=","admission_envelope_digest":[7]}}"#,
            r#"{"proof_kind":"pdp","value":{"archive_payload":"AAEC","receipt_payload":"BAU="}}"#,
            r#"{"proof_kind":"pdp","proof_kind":"potr","value":{"archive_payload":"AAEC"}}"#,
            r#"{"proof_kind":"pdp","value":{"archive_payload":"AAEC","archive_payload":"AAEC"}}"#,
        ] {
            assert!(
                norito::json::from_str::<SorafsProofOutcomeSubmissionV1>(malformed).is_err(),
                "malformed proof-outcome JSON must fail closed: {malformed}"
            );
        }
    }

    #[test]
    fn proof_outcome_submission_schema_references_explicit_payloads() {
        use core::any::TypeId;

        use iroha_schema::{IntoSchema as _, Metadata};

        let schema = SorafsProofOutcomeSubmissionV1::schema();
        let Metadata::Enum(metadata) = schema
            .get::<SorafsProofOutcomeSubmissionV1>()
            .expect("proof-outcome enum schema")
        else {
            panic!("proof-outcome schema must be an enum");
        };
        assert_eq!(metadata.variants.len(), 2);
        assert_eq!(metadata.variants[0].tag, "pdp");
        assert_eq!(metadata.variants[0].discriminant, 0);
        assert_eq!(
            metadata.variants[0].ty,
            Some(TypeId::of::<SorafsPdpProofOutcomeSubmissionV1>())
        );
        assert_eq!(metadata.variants[1].tag, "potr");
        assert_eq!(metadata.variants[1].discriminant, 1);
        assert_eq!(
            metadata.variants[1].ty,
            Some(TypeId::of::<SorafsPotrProofOutcomeSubmissionV1>())
        );
        assert!(schema.contains_key::<SorafsPdpProofOutcomeSubmissionV1>());
        assert!(schema.contains_key::<SorafsPotrProofOutcomeSubmissionV1>());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn sorafs_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterPinManifest::new(
            vec![0x01, 0x02, 0x03],
            42,
            Some(alias()),
            Some(digest(0x10)),
        ));
        assert_slice_roundtrip(ApprovePinManifest::new(
            digest(0x11),
            64,
            Some(vec![0xCA, 0xFE]),
            Some([0x23; 32]),
        ));
        assert_slice_roundtrip(RetirePinManifest::new(
            digest(0x11),
            128,
            Some("superseded".to_owned()),
        ));
        assert_slice_roundtrip(BindManifestAlias::new(digest(0x11), alias(), 65, 365));
        assert_slice_roundtrip(RegisterCapacityDeclaration::new(capacity_declaration()));
        assert_slice_roundtrip(RecordCapacityTelemetry::new(capacity_telemetry()));
        assert_slice_roundtrip(RegisterCapacityDispute::new(capacity_dispute()));
        assert_slice_roundtrip(IssueReplicationOrder::new(
            order_id(),
            vec![0x01, 0x02, 0x03],
            70,
            90,
        ));
        assert_slice_roundtrip(CompleteReplicationOrder::new(order_id(), 88));
        assert_slice_roundtrip(ExpireReplicationOrder::new(order_id(), 91));
        assert_slice_roundtrip(RegisterProviderOwner::new(provider(0x35), owner()));
        assert_slice_roundtrip(UnregisterProviderOwner::new(provider(0x35)));
        assert_slice_roundtrip(SetPricingSchedule::new(
            PricingScheduleRecord::launch_default(),
        ));
        assert_slice_roundtrip(UpsertProviderCredit::new(provider_credit()));
        assert_slice_roundtrip(SetSorafsPopIssuerPolicy::new(pop_issuer_policy()));
        assert_slice_roundtrip(CommitSorafsPopCredentialBatch::new(vec![0x01, 0x02]));
        assert_slice_roundtrip(PublishSorafsPopRevocationList::new(
            vec![0x03, 0x04],
            [0x50; 32],
        ));
        assert_slice_roundtrip(SetSorafsOrderbookPolicy::new(orderbook_policy()));
        assert_slice_roundtrip(SubmitSorafsOrderbookOrder::new(
            vec![0x01, 0x02],
            [0x51; 32],
        ));
        assert_slice_roundtrip(CancelSorafsOrderbookOrder::new(
            vec![0x03, 0x04],
            [0x52; 32],
        ));
        assert_slice_roundtrip(MatchSorafsOrderbook::new([0x53; 32], 7, 16));
        assert_slice_roundtrip(MaintainSorafsOrderbook::new([0x54; 32], 8, 32));
        assert_slice_roundtrip(RecordSorafsOrderbookSettlementReceipt::new(
            vec![0x05, 0x06],
            [0x55; 32],
        ));
        assert_slice_roundtrip(SetSorafsReservePolicy::new(reserve_policy()));
        assert_slice_roundtrip(RegisterSorafsReserveAccount::new(
            reserve_terms(),
            [0x56; 32],
        ));
        assert_slice_roundtrip(RequestSorafsReserveMovement::new(
            [0x57; 32],
            provider(0x36),
            ReserveMovementKindV1::TopUp,
            XorQuantity::try_from_micro(10_000_000).expect("movement amount"),
            1,
            [0x56; 32],
        ));
        assert_slice_roundtrip(DecideSorafsReserveMovement::new(
            [0x57; 32],
            2,
            [0x56; 32],
            true,
            "approved".to_owned(),
        ));
        assert_slice_roundtrip(ChargeSorafsReserveRent::new(
            provider(0x36),
            2,
            1,
            [0x56; 32],
        ));
        assert_slice_roundtrip(AdvanceSorafsReserveLifecycle::new(
            provider(0x36),
            3,
            8,
            [0x56; 32],
        ));
        assert_slice_roundtrip(DrawSorafsReserveCredit::new(
            provider(0x36),
            4,
            XorQuantity::try_from_micro(5_000_000).expect("credit amount"),
            [0x56; 32],
        ));
        assert_slice_roundtrip(RepaySorafsReserveCredit::new(
            provider(0x36),
            5,
            XorQuantity::try_from_micro(1_000_000).expect("repayment amount"),
            [0x56; 32],
        ));
        assert_slice_roundtrip(SubmitSorafsReserveAppeal::new(
            [0x58; 32],
            provider(0x36),
            6,
            ReserveLifecycleStage::Warning,
            "review delinquency evidence".to_owned(),
            Some([0x59; 32]),
            [0x56; 32],
        ));
        assert_slice_roundtrip(DecideSorafsReserveAppeal::new(
            [0x58; 32],
            7,
            [0x56; 32],
            false,
            "evidence insufficient".to_owned(),
        ));
        assert_slice_roundtrip(SubmitSorafsRepairTask::new([0x70; 32], vec![0x01, 0x02]));
        for action in [
            SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                lease_duration_ms: 1_000,
                idempotency_key: "claim-1".to_owned(),
            }),
            SorafsRepairTaskActionV1::Renew(SorafsRepairRenewV1 {
                lease_generation: 1,
                lease_duration_ms: 2_000,
                idempotency_key: "renew-1".to_owned(),
            }),
            SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                lease_generation: 1,
                evidence_digest: [0x71; 32],
                idempotency_key: "complete-1".to_owned(),
            }),
            SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
                lease_generation: 1,
                failure_digest: [0x72; 32],
                idempotency_key: "fail-1".to_owned(),
            }),
            SorafsRepairTaskActionV1::Escalate(SorafsRepairEscalateV1 {
                lease_generation: 1,
                slash_proposal_payload: vec![0x03, 0x04],
                idempotency_key: "escalate-1".to_owned(),
            }),
        ] {
            assert_slice_roundtrip(ApplySorafsRepairTaskAction::new(
                "REP-1".to_owned(),
                1,
                action,
            ));
        }
        assert_slice_roundtrip(SubmitSorafsRepairAppeal::new(
            "REP-1".to_owned(),
            3,
            [0x73; 32],
            "provider evidence".to_owned(),
            "appeal-1".to_owned(),
        ));
        assert_slice_roundtrip(SetSorafsProofOutcomeSignerPolicy::new(
            proof_outcome_signer_policy(),
        ));
        assert_slice_roundtrip(SubmitSorafsProofOutcome::new(
            SorafsProofOutcomeSubmissionV1::Pdp(SorafsPdpProofOutcomeSubmissionV1 {
                archive_payload: vec![0x74, 0x75],
            }),
        ));
        assert_slice_roundtrip(SubmitSorafsProofOutcome::new(
            SorafsProofOutcomeSubmissionV1::Potr(SorafsPotrProofOutcomeSubmissionV1 {
                receipt_payload: vec![0x76, 0x77],
                admission_envelope_digest: [0x78; 32],
            }),
        ));
        assert_slice_roundtrip(SetSorafsModerationPolicy::new(moderation_policy()));
        assert_slice_roundtrip(SubmitSorafsModerationAppeal::new(moderation_appeal_intake()));
        assert_slice_roundtrip(RegisterSorafsModerationJurorEligibility::new(
            "appeal-1".to_owned(),
            "round-1".to_owned(),
            vec![0x01, 0x02],
        ));
        let finalize_sortition = FinalizeSorafsModerationSortition::new(
            "appeal-1".to_owned(),
            "round-1".to_owned(),
            [0x65; 32],
            [0x64; 32],
            vec![owner()],
            Vec::new(),
        );
        assert_eq!(finalize_sortition.randomness_anchor, [0x64; 32]);
        assert_slice_roundtrip(finalize_sortition);
        assert_slice_roundtrip(AcceptSorafsModerationJurorAssignment::new(
            "appeal-1".to_owned(),
            "round-1".to_owned(),
            [0x66; 32],
        ));
        assert_slice_roundtrip(ActivateSorafsModerationCase::new(
            "appeal-1".to_owned(),
            "round-1".to_owned(),
            [0x66; 32],
        ));
        assert_slice_roundtrip(SubmitSorafsModerationCommit::new(vec![0x07, 0x08]));
        assert_slice_roundtrip(RaiseSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeKindV1::EvidenceMismatch,
            None,
            [0x61; 32],
            "evidence mismatch".to_owned(),
        ));
        assert_slice_roundtrip(ResolveSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeDecisionV1::Rejected,
        ));
        assert_slice_roundtrip(SubmitSorafsModerationReveal::new(vec![0x09, 0x0A]));
        assert_slice_roundtrip(FinalizeSorafsModerationCase::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
        ));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one linear registry scenario verifies every SoraFS instruction type name"
    )]
    fn sorafs_registry_decodes_type_names() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            RegisterPinManifest::new(
                vec![0x01, 0x02, 0x03],
                42,
                Some(alias()),
                Some(digest(0x10)),
            ),
        );
        assert_registry_decodes(
            &registry,
            ApprovePinManifest::new(digest(0x11), 64, Some(vec![0xCA, 0xFE]), Some([0x23; 32])),
        );
        assert_registry_decodes(
            &registry,
            RetirePinManifest::new(digest(0x11), 128, Some("superseded".to_owned())),
        );
        assert_registry_decodes(
            &registry,
            BindManifestAlias::new(digest(0x11), alias(), 65, 365),
        );
        assert_registry_decodes(
            &registry,
            RegisterCapacityDeclaration::new(capacity_declaration()),
        );
        assert_registry_decodes(
            &registry,
            RecordCapacityTelemetry::new(capacity_telemetry()),
        );
        assert_registry_decodes(&registry, RegisterCapacityDispute::new(capacity_dispute()));
        assert_registry_decodes(
            &registry,
            IssueReplicationOrder::new(order_id(), vec![0x01, 0x02, 0x03], 70, 90),
        );
        assert_registry_decodes(&registry, CompleteReplicationOrder::new(order_id(), 88));
        assert_registry_decodes(&registry, ExpireReplicationOrder::new(order_id(), 91));
        assert_registry_decodes(
            &registry,
            RegisterProviderOwner::new(provider(0x35), owner()),
        );
        assert_registry_decodes(&registry, UnregisterProviderOwner::new(provider(0x35)));
        assert_registry_decodes(
            &registry,
            SetSorafsPopIssuerPolicy::new(pop_issuer_policy()),
        );
        assert_registry_decodes(
            &registry,
            CommitSorafsPopCredentialBatch::new(vec![0x01, 0x02]),
        );
        assert_registry_decodes(
            &registry,
            PublishSorafsPopRevocationList::new(vec![0x03, 0x04], [0x50; 32]),
        );
        assert_registry_decodes(&registry, SetSorafsOrderbookPolicy::new(orderbook_policy()));
        assert_registry_decodes(
            &registry,
            SubmitSorafsOrderbookOrder::new(vec![0x01, 0x02], [0x51; 32]),
        );
        assert_registry_decodes(
            &registry,
            CancelSorafsOrderbookOrder::new(vec![0x03, 0x04], [0x52; 32]),
        );
        assert_registry_decodes(&registry, MatchSorafsOrderbook::new([0x53; 32], 7, 16));
        assert_registry_decodes(&registry, MaintainSorafsOrderbook::new([0x54; 32], 8, 32));
        assert_registry_decodes(
            &registry,
            RecordSorafsOrderbookSettlementReceipt::new(vec![0x05, 0x06], [0x55; 32]),
        );
        assert_registry_decodes(&registry, SetSorafsReservePolicy::new(reserve_policy()));
        assert_registry_decodes(
            &registry,
            RegisterSorafsReserveAccount::new(reserve_terms(), [0x56; 32]),
        );
        assert_registry_decodes(
            &registry,
            RequestSorafsReserveMovement::new(
                [0x57; 32],
                provider(0x36),
                ReserveMovementKindV1::TopUp,
                XorQuantity::try_from_micro(10_000_000).expect("movement amount"),
                1,
                [0x56; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            DecideSorafsReserveMovement::new(
                [0x57; 32],
                2,
                [0x56; 32],
                true,
                "approved".to_owned(),
            ),
        );
        assert_registry_decodes(
            &registry,
            ChargeSorafsReserveRent::new(provider(0x36), 2, 1, [0x56; 32]),
        );
        assert_registry_decodes(
            &registry,
            AdvanceSorafsReserveLifecycle::new(provider(0x36), 3, 8, [0x56; 32]),
        );
        assert_registry_decodes(
            &registry,
            DrawSorafsReserveCredit::new(
                provider(0x36),
                4,
                XorQuantity::try_from_micro(5_000_000).expect("credit amount"),
                [0x56; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            RepaySorafsReserveCredit::new(
                provider(0x36),
                5,
                XorQuantity::try_from_micro(1_000_000).expect("repayment amount"),
                [0x56; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsReserveAppeal::new(
                [0x58; 32],
                provider(0x36),
                6,
                ReserveLifecycleStage::Warning,
                "review delinquency evidence".to_owned(),
                Some([0x59; 32]),
                [0x56; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            DecideSorafsReserveAppeal::new(
                [0x58; 32],
                7,
                [0x56; 32],
                false,
                "evidence insufficient".to_owned(),
            ),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsRepairTask::new([0x70; 32], vec![0x01, 0x02]),
        );
        assert_registry_decodes(
            &registry,
            ApplySorafsRepairTaskAction::new(
                "REP-1".to_owned(),
                1,
                SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                    lease_duration_ms: 1_000,
                    idempotency_key: "claim-1".to_owned(),
                }),
            ),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsRepairAppeal::new(
                "REP-1".to_owned(),
                3,
                [0x73; 32],
                "provider evidence".to_owned(),
                "appeal-1".to_owned(),
            ),
        );
        assert_registry_decodes(
            &registry,
            SetSorafsProofOutcomeSignerPolicy::new(proof_outcome_signer_policy()),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsProofOutcome::new(SorafsProofOutcomeSubmissionV1::Pdp(
                SorafsPdpProofOutcomeSubmissionV1 {
                    archive_payload: vec![0x74, 0x75],
                },
            )),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsProofOutcome::new(SorafsProofOutcomeSubmissionV1::Potr(
                SorafsPotrProofOutcomeSubmissionV1 {
                    receipt_payload: vec![0x76, 0x77],
                    admission_envelope_digest: [0x78; 32],
                },
            )),
        );
        assert_registry_decodes(
            &registry,
            SetSorafsModerationPolicy::new(moderation_policy()),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsModerationAppeal::new(moderation_appeal_intake()),
        );
        assert_registry_decodes(
            &registry,
            RegisterSorafsModerationJurorEligibility::new(
                "appeal-1".to_owned(),
                "round-1".to_owned(),
                vec![0x01, 0x02],
            ),
        );
        assert_registry_decodes(
            &registry,
            FinalizeSorafsModerationSortition::new(
                "appeal-1".to_owned(),
                "round-1".to_owned(),
                [0x65; 32],
                [0x64; 32],
                vec![owner()],
                Vec::new(),
            ),
        );
        assert_registry_decodes(
            &registry,
            AcceptSorafsModerationJurorAssignment::new(
                "appeal-1".to_owned(),
                "round-1".to_owned(),
                [0x66; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            ActivateSorafsModerationCase::new(
                "appeal-1".to_owned(),
                "round-1".to_owned(),
                [0x66; 32],
            ),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsModerationCommit::new(vec![0x07, 0x08]),
        );
        assert_registry_decodes(
            &registry,
            RaiseSorafsModerationChallenge::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
                ModerationChallengeKindV1::EvidenceMismatch,
                None,
                [0x61; 32],
                "evidence mismatch".to_owned(),
            ),
        );
        assert_registry_decodes(
            &registry,
            ResolveSorafsModerationChallenge::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
                ModerationChallengeDecisionV1::Rejected,
            ),
        );
        assert_registry_decodes(
            &registry,
            SubmitSorafsModerationReveal::new(vec![0x09, 0x0A]),
        );
        assert_registry_decodes(
            &registry,
            FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned()),
        );
    }
}
