//! Deal engine records for SoraFS providers and clients (SF-8).
//!
//! These types provide a Norito-friendly interface for the storage &
//! retrieval marketplace, covering deal proposals, active contracts,
//! probabilistic micropayment tickets, and settlement ledgers.

use std::cmp::Ordering;

use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity, RoundingMode};
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
/// Ledger precision used by XOR-denominated deal charges.
pub const XOR_QUANTITY_SCALE: u32 = 9;

/// Commercial terms negotiated for a deal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DealTerms {
    /// Nominal storage price per GiB-month.
    pub storage_price_per_gib_month: Quantity,
    /// Nominal egress price per gibibyte.
    pub egress_price_per_gib: Quantity,
    /// Number of epochs in a settlement window.
    pub settlement_window_epochs: u64,
    /// Probability (basis points) that an individual ticket pays out.
    pub micropayment_probability_bps: u16,
    /// Nominal payout when a ticket wins.
    pub micropayment_payout: Quantity,
}

impl DealTerms {
    /// Compute the bond requirement (3× monthly storage earnings).
    ///
    /// # Errors
    /// Returns a bounded-domain error if the exact product is unrepresentable.
    pub fn bond_requirement(&self, capacity_gib: u64) -> Result<Quantity, NumericOperationError> {
        let factor = Numeric::new(u128::from(capacity_gib) * 3, 0);
        self.storage_price_per_gib_month.try_mul_decimal(&factor)
    }

    /// Compute the deterministic storage charge for `gib_hours`.
    ///
    /// Fractional asset units are rounded toward zero at the rate's canonical
    /// scale; the rounding policy is explicit and consensus-visible.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn storage_charge(&self, gib_hours: u128) -> Result<Quantity, NumericOperationError> {
        self.storage_price_per_gib_month
            .try_mul_decimal(&Numeric::new(gib_hours, 0))?
            .try_div_decimal_round(
                &Numeric::new(GIB_HOURS_PER_MONTH, 0),
                XOR_QUANTITY_SCALE,
                RoundingMode::TowardZero,
            )
    }

    /// Compute the deterministic egress charge for the supplied bytes.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn egress_charge(&self, bytes: u128) -> Result<Quantity, NumericOperationError> {
        self.egress_price_per_gib
            .try_mul_decimal(&Numeric::new(bytes, 0))?
            .try_div_decimal_round(
                &Numeric::new(BYTES_PER_GIB, 0),
                XOR_QUANTITY_SCALE,
                RoundingMode::TowardZero,
            )
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
    pub expected_charge: Quantity,
    /// Credit obtained through micropayments.
    pub micropayment_credit: Quantity,
    /// Client credit debited during settlement.
    pub client_credit_debit: Quantity,
    /// Bond amount slashed to cover arrears.
    pub bond_slash: Quantity,
    /// Outstanding balance carried forward.
    pub outstanding: Quantity,
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProviderBondLedgerEntry {
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Total nominal quantity currently bonded.
    pub bonded: Quantity,
    /// Portion of the bond locked against active deals.
    pub locked: Quantity,
    /// Total nominal quantity slashed to date.
    pub slashed: Quantity,
    /// Total nominal quantity released back to the provider.
    pub released: Quantity,
    /// Epoch when the ledger was last updated.
    pub last_updated_epoch: u64,
}

impl ProviderBondLedgerEntry {
    /// Lock an additional nominal `amount` against active deals.
    ///
    /// # Errors
    /// Returns a bounded-domain error without mutating the ledger.
    pub fn lock(&mut self, amount: &Quantity, epoch: u64) -> Result<(), NumericOperationError> {
        let locked = self.locked.checked_add(amount)?;
        let bonded = self.bonded.checked_add(amount)?;
        self.locked = locked;
        self.bonded = bonded;
        self.last_updated_epoch = epoch;
        Ok(())
    }

    /// Slash a portion of the locked bond.
    ///
    /// # Errors
    /// Returns a bounded-domain error without mutating the ledger.
    pub fn slash(&mut self, amount: &Quantity, epoch: u64) -> Result<(), NumericOperationError> {
        let slash = if amount > &self.locked {
            self.locked.clone()
        } else {
            amount.clone()
        };
        let locked = self.locked.checked_sub(&slash)?;
        let bonded = self.bonded.checked_sub(&slash)?;
        let slashed = self.slashed.checked_add(&slash)?;
        self.locked = locked;
        self.bonded = bonded;
        self.slashed = slashed;
        self.last_updated_epoch = epoch;
        Ok(())
    }

    /// Release a portion of the locked bond back to the provider.
    ///
    /// # Errors
    /// Returns a bounded-domain error without mutating the ledger.
    pub fn release(&mut self, amount: &Quantity, epoch: u64) -> Result<(), NumericOperationError> {
        let release = if amount > &self.locked {
            self.locked.clone()
        } else {
            amount.clone()
        };
        let locked = self.locked.checked_sub(&release)?;
        let bonded = self.bonded.checked_sub(&release)?;
        let released = self.released.checked_add(&release)?;
        self.locked = locked;
        self.bonded = bonded;
        self.released = released;
        self.last_updated_epoch = epoch;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantity_nanos(value: u128) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 9))
            .expect("u128 nano-XOR fixture fits Quantity")
    }

    #[derive(Encode)]
    struct ForgedDealTerms {
        storage_price_per_gib_month: Numeric,
        egress_price_per_gib: Quantity,
        settlement_window_epochs: u64,
        micropayment_probability_bps: u16,
        micropayment_payout: Quantity,
    }

    #[test]
    fn bond_requirement_scales_with_capacity() {
        let terms = DealTerms {
            storage_price_per_gib_month: quantity_nanos(500_000_000),
            egress_price_per_gib: quantity_nanos(50_000_000),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout: quantity_nanos(10_000_000),
        };

        let requirement = terms.bond_requirement(4).expect("bounded bond");
        assert_eq!(requirement, quantity_nanos(6_000_000_000));
    }

    #[test]
    fn storage_charge_uses_gib_hours() {
        let terms = DealTerms {
            storage_price_per_gib_month: quantity_nanos(720_000_000),
            egress_price_per_gib: quantity_nanos(50_000_000),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout: quantity_nanos(10_000_000),
        };

        let charge = terms.storage_charge(720).expect("bounded charge");
        assert_eq!(charge, quantity_nanos(720_000_000));
    }

    #[test]
    fn egress_charge_scales_with_bytes() {
        let terms = DealTerms {
            storage_price_per_gib_month: quantity_nanos(720_000_000),
            egress_price_per_gib: quantity_nanos(90_000_000),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout: quantity_nanos(10_000_000),
        };

        let charge = terms.egress_charge(BYTES_PER_GIB).expect("bounded charge");
        assert_eq!(charge, quantity_nanos(90_000_000));
    }

    #[test]
    fn deal_terms_reject_forged_negative_price() {
        let forged = ForgedDealTerms {
            storage_price_per_gib_month: Numeric::new(-1_i32, 0),
            egress_price_per_gib: quantity_nanos(1),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout: quantity_nanos(1),
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            <DealTerms as Decode>::decode(&mut input).is_err(),
            "deal terms must reject a forged negative storage price"
        );
    }

    #[test]
    fn bond_ledger_clamps_explicitly_and_preserves_exact_totals() {
        let mut ledger = ProviderBondLedgerEntry::default();
        ledger.lock(&quantity_nanos(100), 1).expect("lock");
        ledger.slash(&quantity_nanos(150), 2).expect("slash");
        assert_eq!(ledger.locked, Quantity::zero());
        assert_eq!(ledger.bonded, Quantity::zero());
        assert_eq!(ledger.slashed, quantity_nanos(100));
        assert_eq!(ledger.last_updated_epoch, 2);
    }
}
