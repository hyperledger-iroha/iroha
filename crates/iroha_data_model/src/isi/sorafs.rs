use super::*;
use crate::sorafs::{
    capacity::{
        CapacityDeclarationRecord, CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
    },
    moderation_ledger::{
        ModerationCaseSpecV1, ModerationChallengeDecisionV1, ModerationChallengeKindV1,
        ModerationLedgerPolicyV1,
    },
    orderbook::OrderbookAdmissionPolicyV1,
    pin_registry::{ManifestAliasBinding, ManifestDigest, ReplicationOrderId},
    pricing::{PricingScheduleRecord, ProviderCreditRecord},
};

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
    /// Activate the next governance-controlled SoraFS orderbook policy revision.
    pub struct SetSorafsOrderbookPolicy {
        /// Policy revision to validate and activate.
        pub policy: OrderbookAdmissionPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsOrderbookPolicy {}

isi! {
    /// Submit a signed canonical order to the authoritative SoraFS orderbook ledger.
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
    /// Commit a signed owner cancellation to the authoritative SoraFS orderbook ledger.
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
    /// Activate the next authoritative SoraFS moderation-ledger policy revision.
    pub struct SetSorafsModerationPolicy {
        /// Policy revision to validate and activate.
        pub policy: ModerationLedgerPolicyV1,
    }
}

impl crate::seal::Instruction for SetSorafsModerationPolicy {}

isi! {
    /// Open one authoritative SoraFS moderation case and commit/reveal round.
    pub struct OpenSorafsModerationCase {
        /// Immutable case, roster, quorum, deadline, and policy scope.
        pub spec: ModerationCaseSpecV1,
    }
}

impl crate::seal::Instruction for OpenSorafsModerationCase {}

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

impl SetSorafsModerationPolicy {
    /// Construct a moderation policy activation instruction.
    #[must_use]
    pub fn new(policy: ModerationLedgerPolicyV1) -> Self {
        Self { policy }
    }
}

impl OpenSorafsModerationCase {
    /// Construct an authoritative case-opening instruction.
    #[must_use]
    pub fn new(spec: ModerationCaseSpecV1) -> Self {
        Self { spec }
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

impl_sorafs_decode_from_slice!(RecordSorafsOrderbookSettlementReceipt {
    receipt_payload: Vec<u8>,
    policy_digest: [u8; 32],
});

impl_sorafs_decode_from_slice!(SetSorafsModerationPolicy {
    policy: ModerationLedgerPolicyV1,
});

impl_sorafs_decode_from_slice!(OpenSorafsModerationCase {
    spec: ModerationCaseSpecV1,
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
            10_000,
            20_000,
            15_000,
            1_000,
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

    fn moderation_policy() -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: crate::sorafs::moderation_ledger::MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            max_panel_size: 5,
            max_total_window_ms: 60_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        }
    }

    fn moderation_case_spec() -> ModerationCaseSpecV1 {
        let jurors = vec![owner()];
        ModerationCaseSpecV1 {
            version: crate::sorafs::moderation_ledger::MODERATION_LEDGER_CASE_VERSION_V1,
            context: crate::sorafs::moderation::SoraFsModerationBallotContextV1 {
                version: crate::sorafs::moderation::SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                case_id: "case-1".to_owned(),
                evidence_bundle_digest: [0x31; 32],
                appeal_finance_config_version: "finance-v1".to_owned(),
                panel_roster_hash:
                    crate::sorafs::moderation_ledger::sorafs_moderation_panel_roster_hash_v1(
                        &jurors, 1,
                    ),
                policy_reference: "policy-v1".to_owned(),
                evidence_uri: None,
            },
            round_id: "round-1".to_owned(),
            jurors,
            quorum: 1,
            commit_deadline_unix_ms: 2_000,
            challenge_deadline_unix_ms: 3_000,
            reveal_deadline_unix_ms: 4_000,
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
        assert_slice_roundtrip(SetSorafsOrderbookPolicy::new(orderbook_policy()));
        assert_slice_roundtrip(SubmitSorafsOrderbookOrder::new(
            vec![0x01, 0x02],
            [0x51; 32],
        ));
        assert_slice_roundtrip(CancelSorafsOrderbookOrder::new(
            vec![0x03, 0x04],
            [0x52; 32],
        ));
        assert_slice_roundtrip(RecordSorafsOrderbookSettlementReceipt::new(
            vec![0x05, 0x06],
            [0x53; 32],
        ));
        assert_slice_roundtrip(SetSorafsModerationPolicy::new(moderation_policy()));
        assert_slice_roundtrip(OpenSorafsModerationCase::new(moderation_case_spec()));
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
        assert_registry_decodes(&registry, SetSorafsOrderbookPolicy::new(orderbook_policy()));
        assert_registry_decodes(
            &registry,
            SubmitSorafsOrderbookOrder::new(vec![0x01, 0x02], [0x51; 32]),
        );
        assert_registry_decodes(
            &registry,
            CancelSorafsOrderbookOrder::new(vec![0x03, 0x04], [0x52; 32]),
        );
        assert_registry_decodes(
            &registry,
            RecordSorafsOrderbookSettlementReceipt::new(vec![0x05, 0x06], [0x53; 32]),
        );
        assert_registry_decodes(
            &registry,
            SetSorafsModerationPolicy::new(moderation_policy()),
        );
        assert_registry_decodes(
            &registry,
            OpenSorafsModerationCase::new(moderation_case_spec()),
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
