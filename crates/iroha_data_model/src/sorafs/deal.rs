//! Deal engine records for SoraFS providers and clients (SF-8).
//!
//! These types provide a Norito-friendly interface for the storage &
//! retrieval marketplace, covering deal proposals, active contracts,
//! probabilistic micropayment tickets, and settlement ledgers.

use std::cmp::Ordering;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    metadata::Metadata,
    sorafs::{capacity::ProviderId, pin_registry::StorageClass},
};

/// Client identifier (BLAKE3-256 digest allocated during admission).
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
pub struct ClientId(pub [u8; 32]);

impl ClientId {
    /// Construct a new client identifier from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the underlying digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Deal identifier (BLAKE3-256 digest of the canonical proposal).
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
pub struct DealId(pub [u8; 32]);

impl DealId {
    /// Construct a new deal identifier.
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

/// Ticket identifier referenced by probabilistic micropayment receipts.
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
pub struct TicketId(pub [u8; 32]);

impl TicketId {
    /// Construct a new ticket identifier.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the underlying digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Settlement constants.
pub const GIB_HOURS_PER_MONTH: u128 = 720;
/// Bytes per gibibyte (2³⁰).
pub const BYTES_PER_GIB: u128 = 1 << 30;

/// Commercial terms negotiated for a deal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealTerms {
    /// Storage price in nano-XOR per GiB-month.
    pub storage_price_nano_per_gib_month: u64,
    /// Egress price in nano-XOR per gibibyte.
    pub egress_price_nano_per_gib: u64,
    /// Number of epochs in a settlement window.
    pub settlement_window_epochs: u64,
    /// Probability (basis points) that an individual ticket pays out.
    pub micropayment_probability_bps: u16,
    /// Payout when a ticket wins, denominated in nano-XOR.
    pub micropayment_payout_nano: u64,
}

impl DealTerms {
    /// Compute the bond requirement (3× monthly storage earnings).
    #[must_use]
    pub fn bond_requirement_nano(&self, capacity_gib: u64) -> u128 {
        let monthly_storage = u128::from(self.storage_price_nano_per_gib_month)
            .saturating_mul(u128::from(capacity_gib));
        monthly_storage.saturating_mul(3)
    }

    /// Compute the deterministic storage charge for `gib_hours`.
    #[must_use]
    pub fn storage_charge_nano(&self, gib_hours: u128) -> u128 {
        let price = u128::from(self.storage_price_nano_per_gib_month);
        price
            .saturating_mul(gib_hours)
            .saturating_div(GIB_HOURS_PER_MONTH.max(1))
    }

    /// Compute the deterministic egress charge for the supplied bytes.
    #[must_use]
    pub fn egress_charge_nano(&self, bytes: u128) -> u128 {
        let price = u128::from(self.egress_price_nano_per_gib);
        price
            .saturating_mul(bytes)
            .saturating_div(BYTES_PER_GIB.max(1))
    }
}

/// Canonical proposal for a deal prior to activation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealProposal {
    /// Provider offering the storage capacity.
    pub provider_id: ProviderId,
    /// Client requesting storage services.
    pub client_id: ClientId,
    /// Storage class targeted by the deal.
    pub storage_class: StorageClass,
    /// GiB committed to the client.
    pub capacity_gib: u64,
    /// Epoch (inclusive) when the deal becomes active.
    pub start_epoch: u64,
    /// Epoch (inclusive) when the deal expires.
    pub end_epoch: u64,
    /// Commercial terms associated with the deal.
    pub terms: DealTerms,
    /// Optional metadata (notes, jurisdiction codes, etc.).
    pub metadata: Metadata,
}

/// Lifecycle status recorded for a deal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "status", content = "value")
)]
pub enum DealStatus {
    /// Deal has been proposed but not yet activated.
    Proposed,
    /// Deal is active. Contains the epoch when it was activated.
    Active(u64),
    /// Deal completed successfully at the supplied epoch.
    Settled(u64),
    /// Deal was cancelled before activation.
    Cancelled(u64),
    /// Deal defaulted; remaining outstanding amount escalates.
    Defaulted(u64),
}

impl DealStatus {
    /// Returns `true` when the deal is currently active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }
}

/// Registry record describing a deal and its lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealRecord {
    /// Canonical identifier derived from the proposal.
    pub deal_id: DealId,
    /// Provider offering the storage services.
    pub provider_id: ProviderId,
    /// Client consuming the storage services.
    pub client_id: ClientId,
    /// Storage class covered by the deal.
    pub storage_class: StorageClass,
    /// GiB committed to the client.
    pub capacity_gib: u64,
    /// Epoch (inclusive) when the deal became active.
    pub start_epoch: u64,
    /// Epoch (inclusive) when the deal expires.
    pub end_epoch: u64,
    /// Commercial terms associated with the deal.
    pub terms: DealTerms,
    /// Optional metadata associated with the deal.
    pub metadata: Metadata,
    /// Current lifecycle status.
    pub status: DealStatus,
}

impl DealRecord {
    /// Returns `true` when the deal has reached its expiry epoch.
    #[must_use]
    pub fn has_expired(&self, epoch: u64) -> bool {
        epoch > self.end_epoch
    }
}

impl PartialOrd for DealRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DealRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.deal_id,
            self.provider_id,
            self.client_id,
            self.start_epoch,
        )
            .cmp(&(
                other.deal_id,
                other.provider_id,
                other.client_id,
                other.start_epoch,
            ))
    }
}

/// Usage sample submitted for a billing window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealUsageReport {
    /// Deal identifier.
    pub deal_id: DealId,
    /// Epoch associated with the usage sample.
    pub epoch: u64,
    /// Total GiB-hours delivered during the window.
    pub storage_gib_hours: u64,
    /// Total egress bytes delivered during the window.
    pub egress_bytes: u64,
    /// Micropayment tickets consumed during the window.
    pub tickets: Vec<MicropaymentTicket>,
}

/// Probabilistic micropayment ticket associated with a deal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MicropaymentTicket {
    /// Identifier derived from the ticket seed.
    pub ticket_id: TicketId,
    /// Epoch when the ticket was issued.
    pub issued_epoch: u64,
    /// Storage GiB-hours covered by the ticket.
    pub storage_gib_hours: u64,
    /// Egress bytes covered by the ticket.
    pub egress_bytes: u64,
}

impl MicropaymentTicket {
    /// Returns `true` when the ticket carries no accounting weight.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.storage_gib_hours == 0 && self.egress_bytes == 0
    }
}

/// Settlement ledger entry recorded after a billing cycle completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealSettlementRecord {
    /// Provider offering the storage services.
    pub provider_id: ProviderId,
    /// Client consuming the storage services.
    pub client_id: ClientId,
    /// Deal identifier.
    pub deal_id: DealId,
    /// Incremental settlement counter.
    pub settlement_index: u64,
    /// Epoch when the settlement occurred.
    pub settled_epoch: u64,
    /// Inclusive window start epoch.
    pub window_start_epoch: u64,
    /// Inclusive window end epoch.
    pub window_end_epoch: u64,
    /// GiB-hours billed within the window.
    pub billed_storage_gib_hours: u128,
    /// Total egress bytes billed within the window.
    pub billed_egress_bytes: u128,
    /// Expected deterministic charge.
    pub expected_charge_nano: u128,
    /// Credit obtained through micropayments.
    pub micropayment_credit_nano: u128,
    /// Client credit debited during settlement.
    pub client_credit_debit_nano: u128,
    /// Bond amount slashed to cover arrears.
    pub bond_slash_nano: u128,
    /// Outstanding balance carried forward.
    pub outstanding_nano: u128,
}

impl PartialOrd for DealSettlementRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DealSettlementRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.deal_id,
            self.settlement_index,
            self.window_start_epoch,
            self.window_end_epoch,
        )
            .cmp(&(
                other.deal_id,
                other.settlement_index,
                other.window_start_epoch,
                other.window_end_epoch,
            ))
    }
}

/// Provider bond ledger entry tracked by governance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProviderBondLedgerEntry {
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Total nano-XOR currently bonded.
    pub bonded_nano: u128,
    /// Portion of the bond locked against active deals.
    pub locked_nano: u128,
    /// Total nano-XOR slashed to date.
    pub slashed_nano: u128,
    /// Total nano-XOR released back to the provider.
    pub released_nano: u128,
    /// Epoch when the ledger was last updated.
    pub last_updated_epoch: u64,
}

impl ProviderBondLedgerEntry {
    /// Lock an additional `amount` of nano-XOR against active deals.
    pub fn lock(&mut self, amount: u128, epoch: u64) {
        self.locked_nano = self.locked_nano.saturating_add(amount);
        self.bonded_nano = self.bonded_nano.saturating_add(amount);
        self.last_updated_epoch = epoch;
    }

    /// Slash a portion of the locked bond.
    pub fn slash(&mut self, amount: u128, epoch: u64) {
        let slash = amount.min(self.locked_nano);
        self.locked_nano = self.locked_nano.saturating_sub(slash);
        self.bonded_nano = self.bonded_nano.saturating_sub(slash);
        self.slashed_nano = self.slashed_nano.saturating_add(slash);
        self.last_updated_epoch = epoch;
    }

    /// Release a portion of the locked bond back to the provider.
    pub fn release(&mut self, amount: u128, epoch: u64) {
        let release = amount.min(self.locked_nano);
        self.locked_nano = self.locked_nano.saturating_sub(release);
        self.bonded_nano = self.bonded_nano.saturating_sub(release);
        self.released_nano = self.released_nano.saturating_add(release);
        self.last_updated_epoch = epoch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bond_requirement_scales_with_capacity() {
        let terms = DealTerms {
            storage_price_nano_per_gib_month: 500_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout_nano: 10_000_000,
        };

        let requirement = terms.bond_requirement_nano(4);
        assert_eq!(requirement, 6_000_000_000);
    }

    #[test]
    fn storage_charge_uses_gib_hours() {
        let terms = DealTerms {
            storage_price_nano_per_gib_month: 720_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout_nano: 10_000_000,
        };

        let charge = terms.storage_charge_nano(720);
        assert_eq!(charge, 720_000_000);
    }

    #[test]
    fn egress_charge_scales_with_bytes() {
        let terms = DealTerms {
            storage_price_nano_per_gib_month: 720_000_000,
            egress_price_nano_per_gib: 90_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout_nano: 10_000_000,
        };

        let charge = terms.egress_charge_nano(BYTES_PER_GIB);
        assert_eq!(charge, 90_000_000);
    }
}
