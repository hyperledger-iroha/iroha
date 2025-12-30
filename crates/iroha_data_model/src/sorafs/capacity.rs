//! Capacity registry records for SoraFS providers (SF-2c).
//!
//! These types provide a stable, schema-driven interface between
//! smart-contract ISI definitions and the runtime registry that
//! tracks provider capacity declarations, telemetry snapshots, and
//! fee accrual ledgers.

use core::fmt;
use std::cmp::Ordering;

use hex;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::metadata::Metadata;

/// Provider identifier (BLAKE3-256 digest allocated by governance).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(transparent),
    norito(with = "crate::json_helpers::fixed_bytes")
)]
pub struct ProviderId(pub [u8; 32]);

impl ProviderId {
    /// Construct a new provider identifier.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Stored capacity declaration along with metadata required for registry queries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityDeclarationRecord {
    /// Provider that authored the capacity declaration.
    pub provider_id: ProviderId,
    /// Canonical Norito encoding of `CapacityDeclarationV1`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub declaration: Vec<u8>,
    /// Total committed GiB advertised by the provider.
    pub committed_capacity_gib: u64,
    /// Epoch (inclusive) when the declaration was registered.
    pub registered_epoch: u64,
    /// Epoch (inclusive) when the declaration becomes active.
    pub valid_from_epoch: u64,
    /// Epoch (inclusive) when the declaration expires.
    pub valid_until_epoch: u64,
    /// Optional metadata annotations persisted alongside the declaration.
    pub metadata: Metadata,
}

impl CapacityDeclarationRecord {
    /// Construct a new record from raw components.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        declaration: Vec<u8>,
        committed_capacity_gib: u64,
        registered_epoch: u64,
        valid_from_epoch: u64,
        valid_until_epoch: u64,
        metadata: Metadata,
    ) -> Self {
        Self {
            provider_id,
            declaration,
            committed_capacity_gib,
            registered_epoch,
            valid_from_epoch,
            valid_until_epoch,
            metadata,
        }
    }
}

impl PartialOrd for CapacityDeclarationRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapacityDeclarationRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.provider_id,
            self.registered_epoch,
            self.valid_from_epoch,
            self.valid_until_epoch,
        )
            .cmp(&(
                other.provider_id,
                other.registered_epoch,
                other.valid_from_epoch,
                other.valid_until_epoch,
            ))
    }
}

/// Telemetry snapshot reported by a provider for a given epoch window.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityTelemetryRecord {
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Start epoch (inclusive) of the telemetry window.
    pub window_start_epoch: u64,
    /// End epoch (inclusive) of the telemetry window.
    pub window_end_epoch: u64,
    /// Declared GiB during the window.
    pub declared_gib: u64,
    /// Effective GiB (after deductions) during the window.
    pub effective_gib: u64,
    /// Utilised GiB (actual replication) during the window.
    pub utilised_gib: u64,
    /// Replication orders issued within the window.
    pub orders_issued: u64,
    /// Replication orders completed within the window.
    pub orders_completed: u64,
    /// Uptime success rate in basis points (0 – `10_000`).
    pub uptime_bps: u32,
    /// Proof-of-retrieval success rate in basis points (0 – `10_000`).
    pub por_success_bps: u32,
    /// Logical bytes served to clients during the window.
    #[norito(default)]
    pub egress_bytes: u64,
    /// PDP challenges issued during the window.
    #[norito(default)]
    pub pdp_challenges: u32,
    /// PDP failures observed during the window.
    #[norito(default)]
    pub pdp_failures: u32,
    /// `PoTR` windows evaluated during the window.
    #[norito(default)]
    pub potr_windows: u32,
    /// `PoTR` SLA breaches recorded during the window.
    #[norito(default)]
    pub potr_breaches: u32,
    /// Optional replay nonce carried with this window.
    #[norito(default)]
    pub nonce: u64,
}

impl CapacityTelemetryRecord {
    /// Construct a telemetry record.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        window_start_epoch: u64,
        window_end_epoch: u64,
        declared_gib: u64,
        effective_gib: u64,
        utilised_gib: u64,
        orders_issued: u64,
        orders_completed: u64,
        uptime_bps: u32,
        por_success_bps: u32,
        egress_bytes: u64,
        pdp_challenges: u32,
        pdp_failures: u32,
        potr_windows: u32,
        potr_breaches: u32,
    ) -> Self {
        Self {
            provider_id,
            window_start_epoch,
            window_end_epoch,
            declared_gib,
            effective_gib,
            utilised_gib,
            orders_issued,
            orders_completed,
            uptime_bps,
            por_success_bps,
            egress_bytes,
            pdp_challenges,
            pdp_failures,
            potr_windows,
            potr_breaches,
            nonce: 0,
        }
    }

    /// Return a copy of this record tagged with a nonce.
    #[must_use]
    pub fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }
}

impl PartialOrd for CapacityTelemetryRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapacityTelemetryRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.provider_id,
            self.window_start_epoch,
            self.window_end_epoch,
        )
            .cmp(&(
                other.provider_id,
                other.window_start_epoch,
                other.window_end_epoch,
            ))
    }
}

/// Aggregated fee ledger entry for a provider.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityFeeLedgerEntry {
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Total declared GiB across all active declarations.
    pub total_declared_gib: u128,
    /// Total utilised GiB recorded via telemetry.
    pub total_utilised_gib: u128,
    /// Accrued storage fees in nano-XOR.
    pub storage_fee_nano: u128,
    /// Accrued egress fees in nano-XOR.
    pub egress_fee_nano: u128,
    /// Total accrued fees (storage + egress) in nano-XOR.
    pub accrued_fee_nano: u128,
    /// Expected settlement charge for the upcoming window (nano-XOR).
    pub expected_settlement_nano: u128,
    /// Total penalties (nano-XOR) slashed because of under-delivery.
    #[cfg_attr(feature = "json", norito(default))]
    pub penalty_slashed_nano: u128,
    /// Number of penalties applied.
    #[cfg_attr(feature = "json", norito(default))]
    pub penalty_events: u32,
    /// Epoch when the ledger entry was last updated.
    pub last_updated_epoch: u64,
    /// Start epoch (inclusive) of the last accepted telemetry window.
    #[norito(default)]
    pub last_window_start_epoch: u64,
    /// End epoch (inclusive) of the last accepted telemetry window.
    #[norito(default)]
    pub last_window_end_epoch: u64,
    /// Last accepted telemetry nonce (0 when unused).
    #[norito(default)]
    pub last_nonce: u64,
}

/// Batch of accrual deltas used to update a [`CapacityFeeLedgerEntry`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CapacityAccrual {
    /// Newly declared GiB reported in the telemetry window.
    pub declared_delta_gib: u128,
    /// Newly utilised GiB reported in the telemetry window.
    pub utilised_delta_gib: u128,
    /// Additional storage fees accrued (nano-XOR).
    pub storage_fee_delta_nano: u128,
    /// Additional egress fees accrued (nano-XOR).
    pub egress_fee_delta_nano: u128,
    /// Expected settlement charge for the upcoming window (nano-XOR).
    pub expected_settlement_nano: u128,
    /// Start epoch (inclusive) of the accepted telemetry window.
    pub window_start_epoch: u64,
    /// End epoch (inclusive) of the accepted telemetry window.
    pub window_end_epoch: u64,
    /// Last accepted telemetry nonce (0 when unused).
    pub nonce: u64,
}

impl CapacityFeeLedgerEntry {
    /// Incrementally update utilisation and fee counters.
    pub fn accrue(&mut self, accrual: CapacityAccrual) {
        self.total_declared_gib = self
            .total_declared_gib
            .saturating_add(accrual.declared_delta_gib);
        self.total_utilised_gib = self
            .total_utilised_gib
            .saturating_add(accrual.utilised_delta_gib);
        self.storage_fee_nano = self
            .storage_fee_nano
            .saturating_add(accrual.storage_fee_delta_nano);
        self.egress_fee_nano = self
            .egress_fee_nano
            .saturating_add(accrual.egress_fee_delta_nano);
        self.accrued_fee_nano = self
            .accrued_fee_nano
            .saturating_add(accrual.storage_fee_delta_nano)
            .saturating_add(accrual.egress_fee_delta_nano);
        self.expected_settlement_nano = accrual.expected_settlement_nano;
        self.last_window_start_epoch = accrual.window_start_epoch;
        self.last_window_end_epoch = accrual.window_end_epoch;
        self.last_nonce = accrual.nonce;
        self.last_updated_epoch = accrual.window_end_epoch;
    }

    /// Accumulate a penalty amount.
    pub fn apply_penalty(&mut self, penalty_nano: u128, epoch: u64) {
        if penalty_nano == 0 {
            return;
        }
        self.penalty_slashed_nano = self.penalty_slashed_nano.saturating_add(penalty_nano);
        self.penalty_events = self.penalty_events.saturating_add(1);
        self.last_updated_epoch = epoch;
    }
}

/// Unique identifier for a capacity dispute (BLAKE3-256 digest of the payload).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(transparent),
    norito(with = "crate::json_helpers::fixed_bytes")
)]
pub struct CapacityDisputeId(pub [u8; 32]);

impl CapacityDisputeId {
    /// Construct a dispute identifier from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the underlying byte array.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Evidence metadata recorded alongside a dispute.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityDisputeEvidence {
    /// Deterministic digest (BLAKE3-256) of the evidence bundle.
    pub digest: [u8; 32],
    /// Optional media type describing the evidence payload.
    pub media_type: Option<String>,
    /// Optional URI pointing to the evidence bundle.
    pub uri: Option<String>,
    /// Optional size of the evidence bundle in bytes.
    pub size_bytes: Option<u64>,
}

/// Dispute outcome recorded once governance issues a ruling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "outcome", content = "value")
)]
pub enum CapacityDisputeOutcome {
    /// Dispute was upheld and remediation is required.
    Upheld,
    /// Dispute was dismissed (insufficient evidence or invalid claim).
    Dismissed,
    /// Dispute was withdrawn before a ruling was issued.
    Withdrawn,
}

/// Resolution metadata captured when a dispute leaves the pending queue.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityDisputeResolution {
    /// Epoch (inclusive) when the dispute was resolved.
    pub resolved_epoch: u64,
    /// Governance outcome applied to the dispute.
    pub outcome: CapacityDisputeOutcome,
    /// Optional human-readable notes describing the resolution.
    pub notes: Option<String>,
}

/// Lifecycle state of a capacity dispute.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "status", content = "payload")
)]
pub enum CapacityDisputeStatus {
    /// Dispute awaiting governance review.
    Pending,
    /// Dispute has been resolved according to the recorded outcome.
    Resolved(CapacityDisputeResolution),
}

impl CapacityDisputeStatus {
    /// Returns `true` when the dispute is still awaiting a decision.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Registry record for disputes raised against a capacity provider.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CapacityDisputeRecord {
    /// Unique identifier derived from the canonical payload.
    pub dispute_id: CapacityDisputeId,
    /// Provider targeted by the dispute.
    pub provider_id: ProviderId,
    /// Identifier for the complainant (32-byte digest).
    pub complainant_id: [u8; 32],
    /// Optional replication order identifier associated with the dispute.
    pub replication_order_id: Option<[u8; 32]>,
    /// Dispute category.
    pub kind: u8,
    /// Epoch when the dispute was submitted.
    pub submitted_epoch: u64,
    /// Human-readable description summarising the dispute.
    pub description: String,
    /// Optional requested remedy proposed by the complainant.
    pub requested_remedy: Option<String>,
    /// Evidence metadata accompanying the dispute.
    pub evidence: CapacityDisputeEvidence,
    /// Canonical Norito encoding of the dispute payload.
    pub dispute_payload: Vec<u8>,
    /// Current lifecycle status recorded by governance.
    pub status: CapacityDisputeStatus,
}

impl CapacityDisputeRecord {
    /// Construct a pending dispute record from raw components.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new_pending(
        dispute_id: CapacityDisputeId,
        provider_id: ProviderId,
        complainant_id: [u8; 32],
        replication_order_id: Option<[u8; 32]>,
        kind: u8,
        submitted_epoch: u64,
        description: String,
        requested_remedy: Option<String>,
        evidence: CapacityDisputeEvidence,
        dispute_payload: Vec<u8>,
    ) -> Self {
        Self {
            dispute_id,
            provider_id,
            complainant_id,
            replication_order_id,
            kind,
            submitted_epoch,
            description,
            requested_remedy,
            evidence,
            dispute_payload,
            status: CapacityDisputeStatus::Pending,
        }
    }
}

impl PartialOrd for CapacityDisputeRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapacityDisputeRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.provider_id,
            self.dispute_id,
            self.submitted_epoch,
            &self.status,
        )
            .cmp(&(
                other.provider_id,
                other.dispute_id,
                other.submitted_epoch,
                &other.status,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accrual_updates_fee_ledger_entry() {
        let mut entry = CapacityFeeLedgerEntry::default();
        let accrual = CapacityAccrual {
            declared_delta_gib: 10,
            utilised_delta_gib: 7,
            storage_fee_delta_nano: 100,
            egress_fee_delta_nano: 50,
            expected_settlement_nano: 200,
            window_start_epoch: 5,
            window_end_epoch: 6,
            nonce: 2,
        };

        entry.accrue(accrual);

        assert_eq!(entry.total_declared_gib, 10);
        assert_eq!(entry.total_utilised_gib, 7);
        assert_eq!(entry.storage_fee_nano, 100);
        assert_eq!(entry.egress_fee_nano, 50);
        assert_eq!(entry.accrued_fee_nano, 150);
        assert_eq!(entry.expected_settlement_nano, 200);
        assert_eq!(entry.last_window_start_epoch, 5);
        assert_eq!(entry.last_window_end_epoch, 6);
        assert_eq!(entry.last_nonce, 2);
        assert_eq!(entry.last_updated_epoch, 6);
    }
}
