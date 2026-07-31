//! Capacity registry records for SoraFS providers (SF-2c).
//!
//! These types provide a stable, schema-driven interface between
//! smart-contract ISI definitions and the runtime registry that
//! tracks provider capacity declarations, telemetry snapshots, and
//! fee accrual ledgers.

use core::fmt;
use std::cmp::Ordering;

use hex;
use iroha_primitives::numeric::{NumericOperationError, Quantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::metadata::Metadata;

/// Provider identifier (BLAKE3-256 digest allocated by governance).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
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
    /// Accrued nominal storage fees.
    pub storage_fee: Quantity,
    /// Accrued nominal egress fees.
    pub egress_fee: Quantity,
    /// Total nominal accrued fees (storage + egress).
    pub accrued_fee: Quantity,
    /// Expected nominal settlement charge for the upcoming window.
    pub expected_settlement: Quantity,
    /// Total nominal penalties slashed because of under-delivery.
    #[cfg_attr(feature = "json", norito(default))]
    pub penalty_slashed: Quantity,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapacityAccrual {
    /// Newly declared GiB reported in the telemetry window.
    pub declared_delta_gib: u128,
    /// Newly utilised GiB reported in the telemetry window.
    pub utilised_delta_gib: u128,
    /// Additional nominal storage fees accrued.
    pub storage_fee_delta: Quantity,
    /// Additional nominal egress fees accrued.
    pub egress_fee_delta: Quantity,
    /// Expected nominal settlement charge for the upcoming window.
    pub expected_settlement: Quantity,
    /// Start epoch (inclusive) of the accepted telemetry window.
    pub window_start_epoch: u64,
    /// End epoch (inclusive) of the accepted telemetry window.
    pub window_end_epoch: u64,
    /// Last accepted telemetry nonce (0 when unused).
    pub nonce: u64,
}

impl CapacityFeeLedgerEntry {
    /// Incrementally update utilisation and fee counters.
    ///
    /// # Errors
    /// Returns [`CapacityLedgerMutationError`] without mutation when the window
    /// is stale/overlapping, a nonce is replayed, a counter would overflow, or
    /// fee/utilisation conservation would be violated.
    pub fn accrue(&mut self, accrual: &CapacityAccrual) -> Result<(), CapacityLedgerMutationError> {
        if accrual.window_end_epoch <= accrual.window_start_epoch {
            return Err(CapacityLedgerMutationError::InvalidWindow {
                start: accrual.window_start_epoch,
                end: accrual.window_end_epoch,
            });
        }
        if accrual.nonce != 0 && accrual.nonce == self.last_nonce {
            return Err(CapacityLedgerMutationError::ReplayedNonce(accrual.nonce));
        }
        if self.last_window_end_epoch > 0 {
            if accrual.window_end_epoch <= self.last_window_end_epoch {
                return Err(CapacityLedgerMutationError::StaleWindow {
                    previous_end: self.last_window_end_epoch,
                    proposed_end: accrual.window_end_epoch,
                });
            }
            if accrual.window_start_epoch < self.last_window_end_epoch {
                return Err(CapacityLedgerMutationError::OverlappingWindow {
                    previous_end: self.last_window_end_epoch,
                    proposed_start: accrual.window_start_epoch,
                });
            }
        }

        let current_fee_total = self
            .storage_fee
            .checked_add(&self.egress_fee)
            .map_err(CapacityLedgerMutationError::Quantity)?;
        if current_fee_total != self.accrued_fee {
            return Err(CapacityLedgerMutationError::FeeConservationViolation {
                storage: self.storage_fee.clone(),
                egress: self.egress_fee.clone(),
                accrued: self.accrued_fee.clone(),
            });
        }

        let total_declared_gib = self
            .total_declared_gib
            .checked_add(accrual.declared_delta_gib)
            .ok_or(CapacityLedgerMutationError::CounterOverflow(
                "declared capacity",
            ))?;
        let total_utilised_gib = self
            .total_utilised_gib
            .checked_add(accrual.utilised_delta_gib)
            .ok_or(CapacityLedgerMutationError::CounterOverflow(
                "utilised capacity",
            ))?;
        if total_utilised_gib > total_declared_gib {
            return Err(CapacityLedgerMutationError::UtilisationExceedsDeclaration {
                utilised: total_utilised_gib,
                declared: total_declared_gib,
            });
        }
        let storage_fee = self.storage_fee.checked_add(&accrual.storage_fee_delta)?;
        let egress_fee = self.egress_fee.checked_add(&accrual.egress_fee_delta)?;
        let fee_delta = accrual
            .storage_fee_delta
            .checked_add(&accrual.egress_fee_delta)?;
        let accrued_fee = self.accrued_fee.checked_add(&fee_delta)?;
        let component_total = storage_fee.checked_add(&egress_fee)?;
        if component_total != accrued_fee {
            return Err(CapacityLedgerMutationError::FeeConservationViolation {
                storage: storage_fee,
                egress: egress_fee,
                accrued: accrued_fee,
            });
        }

        self.total_declared_gib = total_declared_gib;
        self.total_utilised_gib = total_utilised_gib;
        self.storage_fee = storage_fee;
        self.egress_fee = egress_fee;
        self.accrued_fee = accrued_fee;
        self.expected_settlement = accrual.expected_settlement.clone();
        self.last_window_start_epoch = accrual.window_start_epoch;
        self.last_window_end_epoch = accrual.window_end_epoch;
        self.last_nonce = accrual.nonce;
        self.last_updated_epoch = accrual.window_end_epoch;
        Ok(())
    }

    /// Accumulate a penalty amount.
    ///
    /// # Errors
    /// Returns [`CapacityLedgerMutationError`] without mutation when the epoch
    /// is backdated or penalty counters/quantities would overflow.
    pub fn apply_penalty(
        &mut self,
        penalty: &Quantity,
        epoch: u64,
    ) -> Result<(), CapacityLedgerMutationError> {
        if penalty.is_zero() {
            return Ok(());
        }
        if epoch < self.last_updated_epoch {
            return Err(CapacityLedgerMutationError::BackdatedPenalty {
                last_updated: self.last_updated_epoch,
                proposed: epoch,
            });
        }
        let penalty_slashed = self.penalty_slashed.checked_add(penalty)?;
        let penalty_events = self.penalty_events.checked_add(1).ok_or(
            CapacityLedgerMutationError::CounterOverflow("penalty event count"),
        )?;
        self.penalty_slashed = penalty_slashed;
        self.penalty_events = penalty_events;
        self.last_updated_epoch = epoch;
        Ok(())
    }
}

/// Errors raised while mutating a provider capacity-fee ledger.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CapacityLedgerMutationError {
    /// Telemetry windows must have positive duration.
    #[error("capacity ledger window end {end} must be greater than start {start}")]
    InvalidWindow {
        /// Proposed start epoch.
        start: u64,
        /// Proposed end epoch.
        end: u64,
    },
    /// A new window cannot overlap a committed one.
    #[error("capacity ledger window start {proposed_start} overlaps previous end {previous_end}")]
    OverlappingWindow {
        /// Previous committed end epoch.
        previous_end: u64,
        /// Proposed start epoch.
        proposed_start: u64,
    },
    /// A new window end must advance.
    #[error("capacity ledger window end {proposed_end} is not newer than {previous_end}")]
    StaleWindow {
        /// Previous committed end epoch.
        previous_end: u64,
        /// Proposed end epoch.
        proposed_end: u64,
    },
    /// Nonzero telemetry nonces may only be applied once.
    #[error("capacity ledger telemetry nonce {0} was replayed")]
    ReplayedNonce(u64),
    /// A cumulative counter exceeded its integer representation.
    #[error("capacity ledger counter overflow while updating {0}")]
    CounterOverflow(&'static str),
    /// Cumulative utilisation cannot exceed cumulative declarations.
    #[error("capacity ledger utilisation {utilised} exceeds declaration {declared}")]
    UtilisationExceedsDeclaration {
        /// Cumulative utilised GiB.
        utilised: u128,
        /// Cumulative declared GiB.
        declared: u128,
    },
    /// Fee components must exactly equal the accrued total.
    #[error(
        "capacity ledger fee conservation failed: storage {storage} + egress {egress} != accrued {accrued}"
    )]
    FeeConservationViolation {
        /// Storage-fee total.
        storage: Quantity,
        /// Egress-fee total.
        egress: Quantity,
        /// Aggregate accrued-fee total.
        accrued: Quantity,
    },
    /// Penalties cannot move the ledger clock backwards.
    #[error("capacity ledger penalty epoch {proposed} predates last update {last_updated}")]
    BackdatedPenalty {
        /// Last committed update epoch.
        last_updated: u64,
        /// Proposed penalty epoch.
        proposed: u64,
    },
    /// A nominal quantity operation exceeded its bounded exact domain.
    #[error("capacity accounting quantity error: {0}")]
    Quantity(#[from] NumericOperationError),
}

/// Unique identifier for a capacity dispute (BLAKE3-256 digest of the payload).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
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
    use iroha_primitives::numeric::Numeric;

    use super::*;

    fn quantity_nanos(value: u128) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 9))
            .expect("u128 nano-XOR fixture fits Quantity")
    }

    fn maximum_quantity() -> Quantity {
        "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("signed 512-bit maximum quantity")
    }

    #[derive(Encode)]
    struct ForgedCapacityFeeLedgerEntry {
        provider_id: ProviderId,
        total_declared_gib: u128,
        total_utilised_gib: u128,
        storage_fee: Numeric,
        egress_fee: Quantity,
        accrued_fee: Quantity,
        expected_settlement: Quantity,
        penalty_slashed: Quantity,
        penalty_events: u32,
        last_updated_epoch: u64,
        last_window_start_epoch: u64,
        last_window_end_epoch: u64,
        last_nonce: u64,
    }

    fn sample_accrual() -> CapacityAccrual {
        CapacityAccrual {
            declared_delta_gib: 10,
            utilised_delta_gib: 7,
            storage_fee_delta: quantity_nanos(100),
            egress_fee_delta: quantity_nanos(50),
            expected_settlement: quantity_nanos(200),
            window_start_epoch: 5,
            window_end_epoch: 6,
            nonce: 2,
        }
    }

    #[test]
    fn accrual_updates_fee_ledger_entry() {
        let mut entry = CapacityFeeLedgerEntry::default();
        let accrual = sample_accrual();

        entry.accrue(&accrual).expect("valid ledger accrual");

        assert_eq!(entry.total_declared_gib, 10);
        assert_eq!(entry.total_utilised_gib, 7);
        assert_eq!(entry.storage_fee, quantity_nanos(100));
        assert_eq!(entry.egress_fee, quantity_nanos(50));
        assert_eq!(entry.accrued_fee, quantity_nanos(150));
        assert_eq!(entry.expected_settlement, quantity_nanos(200));
        assert_eq!(entry.last_window_start_epoch, 5);
        assert_eq!(entry.last_window_end_epoch, 6);
        assert_eq!(entry.last_nonce, 2);
        assert_eq!(entry.last_updated_epoch, 6);
    }

    #[test]
    fn accrual_counter_overflow_is_atomic() {
        let mut entry = CapacityFeeLedgerEntry {
            total_declared_gib: u128::MAX,
            ..CapacityFeeLedgerEntry::default()
        };
        let before = entry.clone();
        let accrual = CapacityAccrual {
            declared_delta_gib: 1,
            utilised_delta_gib: 0,
            storage_fee_delta: quantity_nanos(1),
            egress_fee_delta: Quantity::zero(),
            expected_settlement: Quantity::zero(),
            window_start_epoch: 1,
            window_end_epoch: 2,
            nonce: 1,
        };
        assert_eq!(
            entry.accrue(&accrual),
            Err(CapacityLedgerMutationError::CounterOverflow(
                "declared capacity"
            ))
        );
        assert_eq!(entry, before);
    }

    #[test]
    fn capacity_fee_ledger_rejects_forged_negative_fee() {
        let forged = ForgedCapacityFeeLedgerEntry {
            provider_id: ProviderId::default(),
            total_declared_gib: 0,
            total_utilised_gib: 0,
            storage_fee: Numeric::new(-1_i32, 0),
            egress_fee: Quantity::zero(),
            accrued_fee: Quantity::zero(),
            expected_settlement: Quantity::zero(),
            penalty_slashed: Quantity::zero(),
            penalty_events: 0,
            last_updated_epoch: 0,
            last_window_start_epoch: 0,
            last_window_end_epoch: 0,
            last_nonce: 0,
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            <CapacityFeeLedgerEntry as Decode>::decode(&mut input).is_err(),
            "capacity ledger must reject a forged negative fee"
        );
    }

    #[test]
    fn accrual_rejects_window_and_replay_attacks_atomically() {
        let mut entry = CapacityFeeLedgerEntry::default();

        let mut invalid = sample_accrual();
        invalid.window_end_epoch = invalid.window_start_epoch;
        assert!(matches!(
            entry.accrue(&invalid),
            Err(CapacityLedgerMutationError::InvalidWindow { .. })
        ));
        assert_eq!(entry, CapacityFeeLedgerEntry::default());

        entry
            .accrue(&sample_accrual())
            .expect("commit initial accrual");
        let committed = entry.clone();
        assert!(matches!(
            entry.accrue(&sample_accrual()),
            Err(CapacityLedgerMutationError::ReplayedNonce(2))
        ));
        assert_eq!(entry, committed);

        let mut stale = sample_accrual();
        stale.nonce = 3;
        stale.window_start_epoch = 4;
        stale.window_end_epoch = 6;
        assert!(matches!(
            entry.accrue(&stale),
            Err(CapacityLedgerMutationError::StaleWindow { .. })
        ));
        assert_eq!(entry, committed);

        let mut overlap = sample_accrual();
        overlap.nonce = 3;
        overlap.window_start_epoch = 5;
        overlap.window_end_epoch = 7;
        assert!(matches!(
            entry.accrue(&overlap),
            Err(CapacityLedgerMutationError::OverlappingWindow { .. })
        ));
        assert_eq!(entry, committed);
    }

    #[test]
    fn accrual_rejects_overflow_and_conservation_attacks_atomically() {
        let mut overflow = CapacityFeeLedgerEntry {
            total_declared_gib: u128::MAX,
            ..CapacityFeeLedgerEntry::default()
        };
        let before = overflow.clone();
        assert!(matches!(
            overflow.accrue(&sample_accrual()),
            Err(CapacityLedgerMutationError::CounterOverflow(
                "declared capacity"
            ))
        ));
        assert_eq!(overflow, before);

        let mut overutilised = CapacityFeeLedgerEntry::default();
        let mut overutilisation = sample_accrual();
        overutilisation.declared_delta_gib = 1;
        overutilisation.utilised_delta_gib = 2;
        assert!(matches!(
            overutilised.accrue(&overutilisation),
            Err(CapacityLedgerMutationError::UtilisationExceedsDeclaration { .. })
        ));
        assert_eq!(overutilised, CapacityFeeLedgerEntry::default());

        let mut corrupt = CapacityFeeLedgerEntry {
            storage_fee: quantity_nanos(1),
            accrued_fee: Quantity::zero(),
            ..CapacityFeeLedgerEntry::default()
        };
        let corrupt_before = corrupt.clone();
        assert!(matches!(
            corrupt.accrue(&sample_accrual()),
            Err(CapacityLedgerMutationError::FeeConservationViolation { .. })
        ));
        assert_eq!(corrupt, corrupt_before);

        let mut fee_delta_overflow = sample_accrual();
        fee_delta_overflow.storage_fee_delta = maximum_quantity();
        fee_delta_overflow.egress_fee_delta = quantity_nanos(1);
        let mut clean = CapacityFeeLedgerEntry::default();
        assert!(matches!(
            clean.accrue(&fee_delta_overflow),
            Err(CapacityLedgerMutationError::Quantity(_))
        ));
        assert_eq!(clean, CapacityFeeLedgerEntry::default());
    }

    #[test]
    fn penalty_updates_are_checked_and_atomic() {
        let mut entry = CapacityFeeLedgerEntry::default();
        entry
            .accrue(&sample_accrual())
            .expect("commit initial accrual");
        entry
            .apply_penalty(&quantity_nanos(25), 6)
            .expect("same-epoch penalty");
        assert_eq!(entry.penalty_slashed, quantity_nanos(25));
        assert_eq!(entry.penalty_events, 1);

        let committed = entry.clone();
        assert!(matches!(
            entry.apply_penalty(&quantity_nanos(1), 5),
            Err(CapacityLedgerMutationError::BackdatedPenalty { .. })
        ));
        assert_eq!(entry, committed);

        entry.penalty_slashed = maximum_quantity();
        let overflow = entry.clone();
        assert!(matches!(
            entry.apply_penalty(&quantity_nanos(1), 7),
            Err(CapacityLedgerMutationError::Quantity(_))
        ));
        assert_eq!(entry, overflow);

        entry.penalty_slashed = Quantity::zero();
        entry.penalty_events = u32::MAX;
        let event_overflow = entry.clone();
        assert!(matches!(
            entry.apply_penalty(&quantity_nanos(1), 7),
            Err(CapacityLedgerMutationError::CounterOverflow(
                "penalty event count"
            ))
        ));
        assert_eq!(entry, event_overflow);
    }
}
