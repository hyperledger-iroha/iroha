//! Deal engine records for SoraFS providers and clients (SF-8).
//!
//! These types provide a Norito-friendly interface for the storage &
//! retrieval marketplace, covering deal proposals, active contracts,
//! probabilistic micropayment tickets, and settlement ledgers.

use std::cmp::Ordering;

use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity, RoundingMode};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

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
/// Maximum micropayment tickets accepted in one usage report.
pub const MAX_DEAL_USAGE_TICKETS: usize = 4_096;

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
    /// Validate deal pricing and settlement invariants.
    ///
    /// # Errors
    ///
    /// Returns [`DealTermsValidationError`] when any monetary or timing term is
    /// zero, or the ticket probability is outside `1..=10_000` basis points.
    pub fn validate(&self) -> Result<(), DealTermsValidationError> {
        if self.storage_price_per_gib_month.is_zero() {
            return Err(DealTermsValidationError::ZeroStoragePrice);
        }
        if self.egress_price_per_gib.is_zero() {
            return Err(DealTermsValidationError::ZeroEgressPrice);
        }
        if self.settlement_window_epochs == 0 {
            return Err(DealTermsValidationError::ZeroSettlementWindow);
        }
        if self.micropayment_probability_bps == 0 || self.micropayment_probability_bps > 10_000 {
            return Err(DealTermsValidationError::InvalidMicropaymentProbability(
                self.micropayment_probability_bps,
            ));
        }
        if self.micropayment_payout.is_zero() {
            return Err(DealTermsValidationError::ZeroMicropaymentPayout);
        }
        Ok(())
    }

    /// Compute the bond requirement (3× monthly storage earnings).
    ///
    /// # Errors
    /// Returns [`DealComputationError`] if the exact requirement exceeds the
    /// bounded quantity domain or the deal terms are invalid.
    pub fn bond_requirement(&self, capacity_gib: u64) -> Result<Quantity, DealComputationError> {
        self.validate()?;
        if capacity_gib == 0 {
            return Err(DealComputationError::ZeroCapacity);
        }
        let factor = Numeric::new(u128::from(capacity_gib) * 3, 0);
        Ok(self.storage_price_per_gib_month.try_mul_decimal(&factor)?)
    }

    /// Compute the deterministic storage charge for `gib_hours`.
    ///
    /// Fractional asset units are rounded toward zero at the rate's canonical
    /// scale; the rounding policy is explicit and consensus-visible.
    ///
    /// # Errors
    /// Returns [`DealComputationError`] if arithmetic fails or the deal terms
    /// are invalid.
    pub fn storage_charge(&self, gib_hours: u128) -> Result<Quantity, DealComputationError> {
        self.validate()?;
        Ok(self.storage_price_per_gib_month.try_mul_div_decimal_round(
            &Numeric::new(gib_hours, 0),
            &Numeric::new(GIB_HOURS_PER_MONTH, 0),
            XOR_QUANTITY_SCALE,
            RoundingMode::TowardZero,
        )?)
    }

    /// Compute the deterministic egress charge for the supplied bytes.
    ///
    /// # Errors
    /// Returns [`DealComputationError`] if arithmetic fails or the deal terms
    /// are invalid.
    pub fn egress_charge(&self, bytes: u128) -> Result<Quantity, DealComputationError> {
        self.validate()?;
        Ok(self.egress_price_per_gib.try_mul_div_decimal_round(
            &Numeric::new(bytes, 0),
            &Numeric::new(BYTES_PER_GIB, 0),
            XOR_QUANTITY_SCALE,
            RoundingMode::TowardZero,
        )?)
    }
}

/// Errors raised while validating first-release deal terms.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DealTermsValidationError {
    /// Storage price must be positive.
    #[error("deal storage price must be positive")]
    ZeroStoragePrice,
    /// Egress price must be positive.
    #[error("deal egress price must be positive")]
    ZeroEgressPrice,
    /// Settlement windows must be positive.
    #[error("deal settlement window must be positive")]
    ZeroSettlementWindow,
    /// Ticket probability is outside the basis-point range.
    #[error("micropayment probability must be within 1..=10000 bps (found {0})")]
    InvalidMicropaymentProbability(u16),
    /// Ticket payout must be positive.
    #[error("micropayment payout must be positive")]
    ZeroMicropaymentPayout,
}

/// Errors raised while computing charges from validated deal terms.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DealComputationError {
    /// The deal terms are structurally invalid.
    #[error("invalid deal terms: {0}")]
    InvalidTerms(#[from] DealTermsValidationError),
    /// Exact deterministic arithmetic failed.
    #[error("deal arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
    /// Bond calculations require positive committed capacity.
    #[error("deal capacity must be positive")]
    ZeroCapacity,
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

impl DealProposal {
    /// Validate the identity, timing, capacity, and commercial terms of a proposal.
    ///
    /// # Errors
    ///
    /// Returns [`DealProposalValidationError`] when a proposal is inert or
    /// internally inconsistent.
    pub fn validate(&self) -> Result<(), DealProposalValidationError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealProposalValidationError::InvalidProviderId);
        }
        if self.client_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealProposalValidationError::InvalidClientId);
        }
        if self.capacity_gib == 0 {
            return Err(DealProposalValidationError::ZeroCapacity);
        }
        if self.start_epoch == 0 || self.end_epoch < self.start_epoch {
            return Err(DealProposalValidationError::InvalidEpochWindow {
                start: self.start_epoch,
                end: self.end_epoch,
            });
        }
        self.terms.validate()?;
        let duration = self
            .end_epoch
            .checked_sub(self.start_epoch)
            .and_then(|delta| delta.checked_add(1))
            .ok_or(DealProposalValidationError::InvalidEpochWindow {
                start: self.start_epoch,
                end: self.end_epoch,
            })?;
        if self.terms.settlement_window_epochs > duration {
            return Err(DealProposalValidationError::SettlementWindowExceedsDeal {
                window: self.terms.settlement_window_epochs,
                duration,
            });
        }
        Ok(())
    }
}

/// Errors raised while validating a canonical deal proposal.
#[allow(variant_size_differences)]
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DealProposalValidationError {
    /// Provider identifiers must be nonzero.
    #[error("deal provider identifier must be nonzero")]
    InvalidProviderId,
    /// Client identifiers must be nonzero.
    #[error("deal client identifier must be nonzero")]
    InvalidClientId,
    /// Committed capacity must be positive.
    #[error("deal capacity must be positive")]
    ZeroCapacity,
    /// Start must be positive and end must not precede the inclusive start.
    #[error("deal epoch interval {start}..={end} must start after zero and not be inverted")]
    InvalidEpochWindow {
        /// Start epoch.
        start: u64,
        /// End epoch.
        end: u64,
    },
    /// A settlement window cannot exceed the inclusive deal duration.
    #[error("deal settlement window {window} epochs exceeds deal duration {duration} epochs")]
    SettlementWindowExceedsDeal {
        /// Configured settlement-window length.
        window: u64,
        /// Inclusive deal duration.
        duration: u64,
    },
    /// Commercial terms are invalid.
    #[error("invalid deal terms: {0}")]
    InvalidTerms(#[from] DealTermsValidationError),
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
    /// Deal was cancelled before terminal completion at the supplied epoch.
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

impl DealUsageReport {
    /// Validate ticket ordering, bounds, epochs, and bounded usage coverage.
    ///
    /// # Errors
    ///
    /// Returns [`DealUsageValidationError`] for inert reports, resource floods,
    /// replay-shaped ticket order, or coverage mismatch/overflow.
    pub fn validate(&self) -> Result<(), DealUsageValidationError> {
        if self.deal_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealUsageValidationError::InvalidDealId);
        }
        if self.storage_gib_hours == 0 && self.egress_bytes == 0 && self.tickets.is_empty() {
            return Err(DealUsageValidationError::EmptyReport);
        }
        if self.tickets.len() > MAX_DEAL_USAGE_TICKETS {
            return Err(DealUsageValidationError::TooManyTickets {
                found: self.tickets.len(),
                maximum: MAX_DEAL_USAGE_TICKETS,
            });
        }

        let mut previous_ticket = None;
        let mut storage_total = 0u128;
        let mut egress_total = 0u128;
        for (index, ticket) in self.tickets.iter().enumerate() {
            ticket
                .validate()
                .map_err(|source| DealUsageValidationError::InvalidTicket { index, source })?;
            if ticket.issued_epoch != self.epoch {
                return Err(DealUsageValidationError::TicketEpochMismatch {
                    index,
                    issued_epoch: ticket.issued_epoch,
                    report_epoch: self.epoch,
                });
            }
            if previous_ticket.is_some_and(|previous| previous >= ticket.ticket_id) {
                return Err(DealUsageValidationError::NonCanonicalTicketOrder { index });
            }
            previous_ticket = Some(ticket.ticket_id);
            storage_total = storage_total
                .checked_add(u128::from(ticket.storage_gib_hours))
                .ok_or(DealUsageValidationError::CoverageOverflow("storage"))?;
            egress_total = egress_total
                .checked_add(u128::from(ticket.egress_bytes))
                .ok_or(DealUsageValidationError::CoverageOverflow("egress"))?;
        }
        if storage_total > u128::from(self.storage_gib_hours)
            || egress_total > u128::from(self.egress_bytes)
        {
            return Err(DealUsageValidationError::CoverageExceedsReport {
                reported_storage: self.storage_gib_hours,
                ticket_storage: storage_total,
                reported_egress: self.egress_bytes,
                ticket_egress: egress_total,
            });
        }
        Ok(())
    }
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

    /// Validate ticket identity and accounting weight.
    ///
    /// # Errors
    ///
    /// Returns [`MicropaymentTicketValidationError`] for an inert identifier or
    /// an empty ticket.
    pub fn validate(&self) -> Result<(), MicropaymentTicketValidationError> {
        if self.ticket_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(MicropaymentTicketValidationError::InvalidTicketId);
        }
        if self.is_empty() {
            return Err(MicropaymentTicketValidationError::EmptyTicket);
        }
        Ok(())
    }
}

/// Errors raised while validating a micropayment ticket.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MicropaymentTicketValidationError {
    /// Ticket identifiers must be nonzero.
    #[error("micropayment ticket identifier must be nonzero")]
    InvalidTicketId,
    /// Tickets must carry storage or egress weight.
    #[error("micropayment ticket must carry accounting weight")]
    EmptyTicket,
}

/// Errors raised while validating a deal usage report.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DealUsageValidationError {
    /// Deal identifiers must be nonzero.
    #[error("usage report deal identifier must be nonzero")]
    InvalidDealId,
    /// Reports must carry usage or ticket accounting.
    #[error("usage report must carry storage, egress, or ticket accounting")]
    EmptyReport,
    /// Ticket count exceeds the hard admission bound.
    #[error("usage report has {found} tickets; maximum is {maximum}")]
    TooManyTickets {
        /// Supplied ticket count.
        found: usize,
        /// Maximum permitted count.
        maximum: usize,
    },
    /// One ticket is invalid.
    #[error("usage report ticket {index} is invalid: {source}")]
    InvalidTicket {
        /// Ticket index.
        index: usize,
        /// Validation failure.
        source: MicropaymentTicketValidationError,
    },
    /// Ticket epochs must bind the exact usage report.
    #[error(
        "usage report ticket {index} epoch {issued_epoch} does not equal report epoch {report_epoch}"
    )]
    TicketEpochMismatch {
        /// Ticket index.
        index: usize,
        /// Ticket issue epoch.
        issued_epoch: u64,
        /// Report epoch.
        report_epoch: u64,
    },
    /// Ticket IDs must be strictly increasing and unique.
    #[error("usage report tickets are not in canonical order at index {index}")]
    NonCanonicalTicketOrder {
        /// Offending index.
        index: usize,
    },
    /// Ticket coverage sums overflowed.
    #[error("usage report {0} ticket coverage overflow")]
    CoverageOverflow(&'static str),
    /// Ticket sums cannot exceed report totals.
    #[error(
        "usage ticket coverage exceeds report: storage {ticket_storage}/{reported_storage}, egress {ticket_egress}/{reported_egress}"
    )]
    CoverageExceedsReport {
        /// Reported storage GiB-hours.
        reported_storage: u64,
        /// Ticket storage GiB-hours.
        ticket_storage: u128,
        /// Reported egress bytes.
        reported_egress: u64,
        /// Ticket egress bytes.
        ticket_egress: u128,
    },
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

impl DealSettlementRecord {
    /// Validate settlement identity, window ordering, and exact charge conservation.
    ///
    /// # Errors
    ///
    /// Returns [`DealSettlementValidationError`] when the record is inert,
    /// temporally inconsistent, overflows, or does not fully account for the
    /// expected charge.
    pub fn validate(&self) -> Result<(), DealSettlementValidationError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealSettlementValidationError::InvalidProviderId);
        }
        if self.client_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealSettlementValidationError::InvalidClientId);
        }
        if self.deal_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(DealSettlementValidationError::InvalidDealId);
        }
        if self.settlement_index == 0 {
            return Err(DealSettlementValidationError::ZeroSettlementIndex);
        }
        if self.window_start_epoch == 0 || self.window_end_epoch < self.window_start_epoch {
            return Err(DealSettlementValidationError::InvalidWindow {
                start: self.window_start_epoch,
                end: self.window_end_epoch,
            });
        }
        if self.settled_epoch < self.window_end_epoch {
            return Err(DealSettlementValidationError::SettlementBeforeWindowEnd {
                settled: self.settled_epoch,
                window_end: self.window_end_epoch,
            });
        }
        let accounted = self
            .micropayment_credit
            .checked_add(&self.client_credit_debit)
            .and_then(|total| total.checked_add(&self.bond_slash))
            .and_then(|total| total.checked_add(&self.outstanding))
            .map_err(|_| DealSettlementValidationError::AccountingOverflow)?;
        if accounted != self.expected_charge {
            return Err(DealSettlementValidationError::ChargeConservation {
                expected: self.expected_charge.clone(),
                accounted,
            });
        }
        Ok(())
    }
}

/// Errors raised while validating a settlement ledger record.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DealSettlementValidationError {
    /// Provider identifiers must be nonzero.
    #[error("settlement provider identifier must be nonzero")]
    InvalidProviderId,
    /// Client identifiers must be nonzero.
    #[error("settlement client identifier must be nonzero")]
    InvalidClientId,
    /// Deal identifiers must be nonzero.
    #[error("settlement deal identifier must be nonzero")]
    InvalidDealId,
    /// Settlement indices are one-based.
    #[error("settlement index must be positive")]
    ZeroSettlementIndex,
    /// Inclusive billing-window start must be positive and end must not precede it.
    #[error("settlement window {start}..={end} must start after zero and not be inverted")]
    InvalidWindow {
        /// Window start.
        start: u64,
        /// Window end.
        end: u64,
    },
    /// Settlement cannot occur before its billing window closes.
    #[error("settlement epoch {settled} predates window end {window_end}")]
    SettlementBeforeWindowEnd {
        /// Settlement epoch.
        settled: u64,
        /// Billing window end.
        window_end: u64,
    },
    /// Charge components overflowed while summing.
    #[error("settlement charge accounting overflow")]
    AccountingOverflow,
    /// All expected value must be allocated to payment, slash, or carry.
    #[error("settlement accounted charge {accounted} does not equal expected {expected}")]
    ChargeConservation {
        /// Expected deterministic charge.
        expected: Quantity,
        /// Sum of all settlement destinations.
        accounted: Quantity,
    },
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
    /// Validate the relationship between total and deal-locked collateral.
    ///
    /// # Errors
    ///
    /// Returns [`BondLedgerMutationError::LockedExceedsBonded`] when persisted
    /// state is internally inconsistent.
    pub fn validate(&self) -> Result<(), BondLedgerMutationError> {
        if self.locked > self.bonded {
            return Err(BondLedgerMutationError::LockedExceedsBonded {
                locked: self.locked.clone(),
                bonded: self.bonded.clone(),
            });
        }
        Ok(())
    }

    /// Lock an additional nominal `amount` against active deals.
    ///
    /// # Errors
    ///
    /// Returns [`BondLedgerMutationError`] without mutation for inconsistent
    /// state, a backdated epoch, or arithmetic overflow.
    pub fn lock(&mut self, amount: &Quantity, epoch: u64) -> Result<(), BondLedgerMutationError> {
        self.validate()?;
        self.ensure_epoch(epoch)?;
        if amount.is_zero() {
            return Ok(());
        }
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
    /// Returns [`BondLedgerMutationError`] without mutation when the requested
    /// slash exceeds locked collateral, the epoch is backdated, or accounting
    /// would overflow.
    pub fn slash(&mut self, amount: &Quantity, epoch: u64) -> Result<(), BondLedgerMutationError> {
        self.validate()?;
        self.ensure_epoch(epoch)?;
        if amount.is_zero() {
            return Ok(());
        }
        if amount > &self.locked {
            return Err(BondLedgerMutationError::AmountExceedsLocked {
                requested: amount.clone(),
                locked: self.locked.clone(),
            });
        }
        let locked = self.locked.checked_sub(amount)?;
        let bonded = self.bonded.checked_sub(amount)?;
        let slashed = self.slashed.checked_add(amount)?;
        self.locked = locked;
        self.bonded = bonded;
        self.slashed = slashed;
        self.last_updated_epoch = epoch;
        Ok(())
    }

    /// Release a portion of the locked bond back to the provider.
    ///
    /// # Errors
    /// Returns [`BondLedgerMutationError`] without mutation when the requested
    /// release exceeds locked collateral, the epoch is backdated, or accounting
    /// would overflow.
    pub fn release(
        &mut self,
        amount: &Quantity,
        epoch: u64,
    ) -> Result<(), BondLedgerMutationError> {
        self.validate()?;
        self.ensure_epoch(epoch)?;
        if amount.is_zero() {
            return Ok(());
        }
        if amount > &self.locked {
            return Err(BondLedgerMutationError::AmountExceedsLocked {
                requested: amount.clone(),
                locked: self.locked.clone(),
            });
        }
        let locked = self.locked.checked_sub(amount)?;
        let bonded = self.bonded.checked_sub(amount)?;
        let released = self.released.checked_add(amount)?;
        self.locked = locked;
        self.bonded = bonded;
        self.released = released;
        self.last_updated_epoch = epoch;
        Ok(())
    }

    fn ensure_epoch(&self, epoch: u64) -> Result<(), BondLedgerMutationError> {
        if epoch < self.last_updated_epoch {
            return Err(BondLedgerMutationError::BackdatedEpoch {
                previous: self.last_updated_epoch,
                proposed: epoch,
            });
        }
        Ok(())
    }
}

/// Errors raised while mutating provider bond accounting.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BondLedgerMutationError {
    /// Locked collateral cannot exceed the current bond.
    #[error("locked collateral {locked} exceeds bonded collateral {bonded}")]
    LockedExceedsBonded {
        /// Locked collateral.
        locked: Quantity,
        /// Current bonded collateral.
        bonded: Quantity,
    },
    /// Slash/release requests cannot exceed the locked amount.
    #[error("bond mutation amount {requested} exceeds locked collateral {locked}")]
    AmountExceedsLocked {
        /// Requested mutation.
        requested: Quantity,
        /// Available locked collateral.
        locked: Quantity,
    },
    /// A cumulative counter overflowed.
    #[error("bond ledger counter overflow while updating {0}")]
    CounterOverflow(&'static str),
    /// A cumulative counter underflowed.
    #[error("bond ledger counter underflow while updating {0}")]
    CounterUnderflow(&'static str),
    /// Ledger epochs cannot move backwards.
    #[error("bond ledger epoch {proposed} predates previous epoch {previous}")]
    BackdatedEpoch {
        /// Previous update epoch.
        previous: u64,
        /// Proposed update epoch.
        proposed: u64,
    },
    /// Exact nominal arithmetic exceeded the bounded quantity domain.
    #[error("bond ledger arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
}

#[cfg(test)]
mod tests {
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
    fn deal_charges_bound_only_the_final_ratio_result() {
        let maximum = maximum_quantity();
        let terms = DealTerms {
            storage_price_per_gib_month: maximum.clone(),
            egress_price_per_gib: maximum.clone(),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 500,
            micropayment_payout: Quantity::one(),
        };

        assert_eq!(
            terms
                .storage_charge(GIB_HOURS_PER_MONTH)
                .expect("equal storage ratio cancels a wide product"),
            maximum
        );
        assert_eq!(
            terms
                .egress_charge(BYTES_PER_GIB)
                .expect("equal egress ratio cancels a wide product"),
            terms.egress_price_per_gib
        );
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
    fn deal_terms_reject_zero_and_out_of_range_values() {
        let base = DealTerms {
            storage_price_per_gib_month: quantity_nanos(1),
            egress_price_per_gib: quantity_nanos(1),
            settlement_window_epochs: 1,
            micropayment_probability_bps: 1,
            micropayment_payout: quantity_nanos(1),
        };

        let mut candidate = base.clone();
        candidate.storage_price_per_gib_month = Quantity::zero();
        assert_eq!(
            candidate.validate(),
            Err(DealTermsValidationError::ZeroStoragePrice)
        );

        candidate = base.clone();
        candidate.egress_price_per_gib = Quantity::zero();
        assert_eq!(
            candidate.validate(),
            Err(DealTermsValidationError::ZeroEgressPrice)
        );

        candidate = base.clone();
        candidate.settlement_window_epochs = 0;
        assert_eq!(
            candidate.validate(),
            Err(DealTermsValidationError::ZeroSettlementWindow)
        );

        for probability in [0, 10_001] {
            candidate = base.clone();
            candidate.micropayment_probability_bps = probability;
            assert_eq!(
                candidate.validate(),
                Err(DealTermsValidationError::InvalidMicropaymentProbability(
                    probability
                ))
            );
        }

        candidate = base;
        candidate.micropayment_payout = Quantity::zero();
        assert_eq!(
            candidate.validate(),
            Err(DealTermsValidationError::ZeroMicropaymentPayout)
        );
    }

    #[test]
    fn deal_arithmetic_rejects_overflow_instead_of_saturating() {
        let terms = DealTerms {
            storage_price_per_gib_month: maximum_quantity(),
            egress_price_per_gib: maximum_quantity(),
            settlement_window_epochs: 1,
            micropayment_probability_bps: 1,
            micropayment_payout: Quantity::one(),
        };
        assert!(matches!(
            terms.bond_requirement(u64::MAX),
            Err(DealComputationError::Arithmetic(_))
        ));

        assert_eq!(
            terms.storage_charge(0),
            Ok(Quantity::zero()),
            "zero usage remains a valid zero charge"
        );
        assert_eq!(terms.egress_charge(0), Ok(Quantity::zero()));
        assert_eq!(
            terms.bond_requirement(0),
            Err(DealComputationError::ZeroCapacity)
        );
    }

    #[test]
    fn deal_proposal_rejects_inert_identity_capacity_and_window() {
        let terms = DealTerms {
            storage_price_per_gib_month: quantity_nanos(1),
            egress_price_per_gib: quantity_nanos(1),
            settlement_window_epochs: 1,
            micropayment_probability_bps: 1,
            micropayment_payout: quantity_nanos(1),
        };
        let base = DealProposal {
            provider_id: ProviderId::new([1; 32]),
            client_id: ClientId::new([2; 32]),
            storage_class: StorageClass::Hot,
            capacity_gib: 1,
            start_epoch: 10,
            end_epoch: 20,
            terms,
            metadata: Metadata::default(),
        };
        assert_eq!(base.validate(), Ok(()));

        let mut candidate = base.clone();
        candidate.provider_id = ProviderId::default();
        assert_eq!(
            candidate.validate(),
            Err(DealProposalValidationError::InvalidProviderId)
        );
        candidate = base.clone();
        candidate.client_id = ClientId::default();
        assert_eq!(
            candidate.validate(),
            Err(DealProposalValidationError::InvalidClientId)
        );
        candidate = base.clone();
        candidate.capacity_gib = 0;
        assert_eq!(
            candidate.validate(),
            Err(DealProposalValidationError::ZeroCapacity)
        );
        candidate = base;
        candidate.end_epoch = candidate.start_epoch;
        assert_eq!(candidate.validate(), Ok(()), "one-epoch deal is inclusive");
        candidate.terms.settlement_window_epochs = 2;
        assert!(matches!(
            candidate.validate(),
            Err(DealProposalValidationError::SettlementWindowExceedsDeal {
                window: 2,
                duration: 1,
            })
        ));
        candidate.terms.settlement_window_epochs = 1;
        candidate.end_epoch = candidate.start_epoch - 1;
        assert!(matches!(
            candidate.validate(),
            Err(DealProposalValidationError::InvalidEpochWindow { .. })
        ));
        candidate.start_epoch = 0;
        candidate.end_epoch = 1;
        assert!(matches!(
            candidate.validate(),
            Err(DealProposalValidationError::InvalidEpochWindow { .. })
        ));
    }

    #[test]
    fn usage_report_enforces_ticket_coverage_order_and_bounds() {
        let ticket_one = MicropaymentTicket {
            ticket_id: TicketId::new([1; 32]),
            issued_epoch: 10,
            storage_gib_hours: 2,
            egress_bytes: 3,
        };
        let ticket_two = MicropaymentTicket {
            ticket_id: TicketId::new([2; 32]),
            issued_epoch: 10,
            storage_gib_hours: 5,
            egress_bytes: 7,
        };
        let base = DealUsageReport {
            deal_id: DealId::new([3; 32]),
            epoch: 10,
            storage_gib_hours: 7,
            egress_bytes: 10,
            tickets: vec![ticket_one, ticket_two],
        };
        assert_eq!(base.validate(), Ok(()));

        let mut reversed = base.clone();
        reversed.tickets.reverse();
        assert!(matches!(
            reversed.validate(),
            Err(DealUsageValidationError::NonCanonicalTicketOrder { .. })
        ));

        let mut duplicate = base.clone();
        duplicate.tickets[1].ticket_id = duplicate.tickets[0].ticket_id;
        assert!(matches!(
            duplicate.validate(),
            Err(DealUsageValidationError::NonCanonicalTicketOrder { .. })
        ));

        let mut future = base.clone();
        future.tickets[0].issued_epoch = 11;
        assert!(matches!(
            future.validate(),
            Err(DealUsageValidationError::TicketEpochMismatch { .. })
        ));

        let mut mismatch = base.clone();
        mismatch.egress_bytes -= 1;
        assert!(matches!(
            mismatch.validate(),
            Err(DealUsageValidationError::CoverageExceedsReport { .. })
        ));

        let mut ticketless = base.clone();
        ticketless.tickets.clear();
        assert_eq!(ticketless.validate(), Ok(()));
        ticketless.storage_gib_hours = 0;
        ticketless.egress_bytes = 0;
        assert_eq!(
            ticketless.validate(),
            Err(DealUsageValidationError::EmptyReport)
        );

        let mut inert_ticket = base.clone();
        inert_ticket.tickets[0].ticket_id = TicketId::default();
        assert!(matches!(
            inert_ticket.validate(),
            Err(DealUsageValidationError::InvalidTicket {
                source: MicropaymentTicketValidationError::InvalidTicketId,
                ..
            })
        ));

        let mut flood = base;
        flood.tickets = vec![ticket_one; MAX_DEAL_USAGE_TICKETS + 1];
        assert!(matches!(
            flood.validate(),
            Err(DealUsageValidationError::TooManyTickets { .. })
        ));
    }

    #[test]
    fn settlement_record_enforces_exact_charge_conservation() {
        let base = DealSettlementRecord {
            provider_id: ProviderId::new([1; 32]),
            client_id: ClientId::new([2; 32]),
            deal_id: DealId::new([3; 32]),
            settlement_index: 1,
            settled_epoch: 20,
            window_start_epoch: 10,
            window_end_epoch: 20,
            billed_storage_gib_hours: 1,
            billed_egress_bytes: 1,
            expected_charge: quantity_nanos(100),
            micropayment_credit: quantity_nanos(10),
            client_credit_debit: quantity_nanos(20),
            bond_slash: quantity_nanos(30),
            outstanding: quantity_nanos(40),
        };
        assert_eq!(base.validate(), Ok(()));

        let mut one_epoch = base.clone();
        one_epoch.window_end_epoch = one_epoch.window_start_epoch;
        one_epoch.settled_epoch = one_epoch.window_end_epoch;
        assert_eq!(
            one_epoch.validate(),
            Ok(()),
            "inclusive one-epoch settlement window is valid"
        );
        one_epoch.window_end_epoch = one_epoch.window_start_epoch - 1;
        assert!(matches!(
            one_epoch.validate(),
            Err(DealSettlementValidationError::InvalidWindow { .. })
        ));
        one_epoch.window_start_epoch = 0;
        one_epoch.window_end_epoch = 0;
        assert!(matches!(
            one_epoch.validate(),
            Err(DealSettlementValidationError::InvalidWindow { .. })
        ));

        let mut mismatch = base.clone();
        mismatch.outstanding = quantity_nanos(39);
        assert!(matches!(
            mismatch.validate(),
            Err(DealSettlementValidationError::ChargeConservation { .. })
        ));

        let mut overflow = base.clone();
        overflow.micropayment_credit = maximum_quantity();
        assert_eq!(
            overflow.validate(),
            Err(DealSettlementValidationError::AccountingOverflow)
        );

        let mut premature = base;
        premature.settled_epoch = 19;
        assert!(matches!(
            premature.validate(),
            Err(DealSettlementValidationError::SettlementBeforeWindowEnd { .. })
        ));
    }

    #[test]
    fn bond_ledger_mutations_are_checked_and_conservative() {
        let mut ledger = ProviderBondLedgerEntry::default();
        ledger.lock(&quantity_nanos(100), 1).expect("lock bond");
        assert_eq!(ledger.bonded, quantity_nanos(100));
        assert_eq!(ledger.locked, quantity_nanos(100));

        ledger.slash(&quantity_nanos(30), 2).expect("slash bond");
        ledger
            .release(&quantity_nanos(20), 2)
            .expect("release bond");
        assert_eq!(ledger.bonded, quantity_nanos(50));
        assert_eq!(ledger.locked, quantity_nanos(50));
        assert_eq!(ledger.slashed, quantity_nanos(30));
        assert_eq!(ledger.released, quantity_nanos(20));
    }

    #[test]
    fn bond_ledger_rejects_overdraw_backdating_and_overflow_atomically() {
        let mut ledger = ProviderBondLedgerEntry::default();
        ledger.lock(&quantity_nanos(10), 5).expect("lock bond");
        let committed = ledger.clone();

        assert!(matches!(
            ledger.slash(&quantity_nanos(11), 6),
            Err(BondLedgerMutationError::AmountExceedsLocked { .. })
        ));
        assert_eq!(ledger, committed);
        assert!(matches!(
            ledger.release(&quantity_nanos(1), 4),
            Err(BondLedgerMutationError::BackdatedEpoch { .. })
        ));
        assert_eq!(ledger, committed);

        ledger.slashed = maximum_quantity();
        let slash_overflow = ledger.clone();
        assert!(matches!(
            ledger.slash(&quantity_nanos(1), 6),
            Err(BondLedgerMutationError::Arithmetic(_))
        ));
        assert_eq!(ledger, slash_overflow);

        let mut corrupt = ProviderBondLedgerEntry {
            bonded: quantity_nanos(1),
            locked: quantity_nanos(2),
            ..ProviderBondLedgerEntry::default()
        };
        let corrupt_before = corrupt.clone();
        assert!(matches!(
            corrupt.lock(&quantity_nanos(1), 1),
            Err(BondLedgerMutationError::LockedExceedsBonded { .. })
        ));
        assert_eq!(corrupt, corrupt_before);

        let mut lock_overflow = ProviderBondLedgerEntry {
            bonded: maximum_quantity(),
            locked: maximum_quantity(),
            ..ProviderBondLedgerEntry::default()
        };
        let lock_before = lock_overflow.clone();
        assert!(matches!(
            lock_overflow.lock(&quantity_nanos(1), 1),
            Err(BondLedgerMutationError::Arithmetic(_))
        ));
        assert_eq!(lock_overflow, lock_before);
    }
}
