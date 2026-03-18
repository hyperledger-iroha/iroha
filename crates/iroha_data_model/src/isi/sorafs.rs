use super::*;
use crate::sorafs::{
    capacity::{
        CapacityDeclarationRecord, CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
    },
    pin_registry::{
        ChunkerProfileHandle, ManifestAliasBinding, ManifestDigest, PinPolicy, ReplicationOrderId,
        ReplicationReceiptStatus,
    },
    pricing::{PricingScheduleRecord, ProviderCreditRecord},
};

isi! {
    /// Register a `SoraFS` manifest digest with the pin registry (pending state).
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
pub struct RegisterPinManifest {
    /// Canonical manifest digest (BLAKE3-256 of the Norito payload).
    pub digest: ManifestDigest,
        /// Chunker profile handle used to generate the CAR commitments.
        pub chunker: ChunkerProfileHandle,
        /// SHA3-256 digest emitted alongside the chunk metadata report.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub chunk_digest_sha3_256: [u8; 32],
    /// Requested replication policy.
    pub policy: PinPolicy,
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
    /// Record a provider replication receipt for an order.
pub struct RecordReplicationReceipt {
    /// Identifier of the replication order.
    pub order_id: ReplicationOrderId,
        /// Provider reporting replication progress.
        pub provider: ProviderId,
        /// Reported status outcome.
        pub status: ReplicationReceiptStatus,
        /// Unix timestamp (seconds) when the receipt was recorded.
        pub timestamp: u64,
        /// Optional digest of the `PoR` sample bundle.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub por_sample_digest: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for RecordReplicationReceipt {}

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

impl RegisterPinManifest {
    /// Create a new `RegisterPinManifest` instruction.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        digest: ManifestDigest,
        chunker: ChunkerProfileHandle,
        chunk_digest_sha3_256: [u8; 32],
        policy: PinPolicy,
        submitted_epoch: u64,
        alias: Option<ManifestAliasBinding>,
        successor_of: Option<ManifestDigest>,
    ) -> Self {
        Self {
            digest,
            chunker,
            chunk_digest_sha3_256,
            policy,
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

impl RecordReplicationReceipt {
    /// Create a new `RecordReplicationReceipt` instruction.
    #[must_use]
    pub fn new(
        order_id: ReplicationOrderId,
        provider: ProviderId,
        status: ReplicationReceiptStatus,
        timestamp: u64,
        por_sample_digest: Option<[u8; 32]>,
    ) -> Self {
        Self {
            order_id,
            provider,
            status,
            timestamp,
            por_sample_digest,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "json")]
    use crate::sorafs::pin_registry::StorageClass;

    #[cfg(feature = "json")]
    #[test]
    fn register_pin_manifest_json_roundtrip() {
        let manifest = RegisterPinManifest::new(
            ManifestDigest::new([9_u8; 32]),
            ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 31,
            },
            [7_u8; 32],
            PinPolicy {
                min_replicas: 2,
                storage_class: StorageClass::Hot,
                retention_epoch: 900,
            },
            42,
            None,
            None,
        );

        let value = norito::json::to_value(&manifest).expect("register pin manifest json");
        let decoded: RegisterPinManifest =
            norito::json::from_value(value).expect("register pin manifest decode");

        assert_eq!(decoded, manifest);
    }
}
