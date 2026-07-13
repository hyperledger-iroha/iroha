#![allow(unexpected_cfgs)]

//! Deterministic SoraFS hedging and billing payload foundations.

use blake3::{Hash, Hasher};
use iroha_crypto::numeric::{Numeric, NumericOperationError, Quantity, RoundingMode};
use norito::{
    core::Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::deal::{BASIS_POINTS_PER_UNIT, DealAmountError, XorQuantity};

pub mod signed;

/// Schema version for [`HedgingPriceFeedV1`].
pub const HEDGING_PRICE_FEED_VERSION_V1: u8 = 1;
/// Schema version for [`HedgingReferencePriceDecisionV1`].
pub const HEDGING_REFERENCE_PRICE_DECISION_VERSION_V1: u8 = 1;
/// Schema version for [`BillingLineItemV1`].
pub const BILLING_LINE_ITEM_VERSION_V1: u8 = 1;
/// Schema version for [`BillingStatementV1`].
pub const BILLING_STATEMENT_VERSION_V1: u8 = 1;
/// Basis-point denominator for SoraFS hedging weights and divergence checks.
pub const HEDGING_BASIS_POINTS: u16 = BASIS_POINTS_PER_UNIT;
/// Maximum accepted feed count in one reference-price decision.
pub const MAX_HEDGING_PRICE_FEEDS: usize = 64;
/// Maximum canonical identifier byte length for feed and source identifiers.
pub const MAX_HEDGING_IDENTIFIER_BYTES: usize = 256;
/// Maximum human-readable note byte length.
pub const MAX_HEDGING_NOTE_BYTES: usize = 1_024;
/// Maximum deterministic degradation-reason count.
pub const MAX_HEDGING_DEGRADATION_REASONS: usize = MAX_HEDGING_PRICE_FEEDS * 2;
/// Maximum account identifier bytes in one billing statement.
pub const MAX_BILLING_ACCOUNT_ID_BYTES: usize = 512;
/// Maximum billing line count in one statement.
pub const MAX_BILLING_LINES: usize = 65_536;
/// Maximum exact canonical size of a feed sample or billing line archive.
pub const HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
/// Maximum exact canonical size of one reference-price decision archive.
pub const HEDGING_DECISION_MAX_CANONICAL_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum exact canonical size of one billing statement archive.
pub const BILLING_STATEMENT_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024 * 1024;
const HEDGING_DECODE_MAX_DEPTH_V1: usize = 64;

const REFERENCE_PRICE_DECISION_ID_DOMAIN_V1: &[u8] =
    b"sorafs.hedging.reference-price-decision-id.v1";
const BILLING_LINE_ITEM_ID_DOMAIN_V1: &[u8] = b"sorafs.billing.line-item-id.v1";
const BILLING_STATEMENT_ID_DOMAIN_V1: &[u8] = b"sorafs.billing.statement-id.v1";

/// Feed status observed by the hedging decision engine.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum HedgingFeedStatusV1 {
    /// Feed was accepted without local degradation.
    Ok,
    /// Feed was accepted but the collector observed a non-fatal degradation.
    Degraded,
    /// Feed was rejected and must not enter a reference-price decision.
    Rejected,
}

/// Direction of a billing line item.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "direction", content = "value", rename_all = "snake_case")]
pub enum BillingLineDirectionV1 {
    /// Debit increases the account amount due.
    Debit,
    /// Credit offsets debits for the account.
    Credit,
}

/// Billing line item category.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BillingLineItemKindV1 {
    /// Storage capacity charge.
    Storage,
    /// Egress transfer charge.
    Egress,
    /// Reserve/Rent lifecycle charge.
    ReserveRent,
    /// Orderbook or settlement fee.
    SettlementFee,
    /// Governance or SLA penalty.
    Penalty,
    /// Provider or buyer incentive credit.
    IncentiveCredit,
    /// Manual governance-approved adjustment.
    Adjustment,
}

/// Canonical XOR/USD feed sample normalized by a collector.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct HedgingPriceFeedV1 {
    /// Schema version (`HEDGING_PRICE_FEED_VERSION_V1`).
    pub version: u8,
    /// Governance-approved feed identifier.
    pub feed_id: String,
    /// Human-readable source label.
    pub source: String,
    /// Unix timestamp when the sample was observed.
    pub observed_at_unix: u64,
    /// Exact USD price of one XOR.
    pub xor_usd_price: Quantity,
    /// Decision weight, in basis points.
    pub weight_bps: u16,
    /// Digest of the signed feed envelope or collector attestation.
    pub evidence_digest: [u8; 32],
    /// Collector status for this sample.
    pub status: HedgingFeedStatusV1,
}

impl HedgingPriceFeedV1 {
    /// Validate feed structure before aggregation.
    pub fn validate(&self) -> Result<(), HedgingValidationError> {
        if self.version != HEDGING_PRICE_FEED_VERSION_V1 {
            return Err(HedgingValidationError::UnsupportedPriceFeedVersion {
                found: self.version,
            });
        }
        validate_identifier("feed_id", &self.feed_id)?;
        validate_identifier("source", &self.source)?;
        if self.observed_at_unix == 0 {
            return Err(HedgingValidationError::InvalidTimestamp {
                field: "observed_at_unix",
            });
        }
        if self.xor_usd_price.is_zero() {
            return Err(HedgingValidationError::ZeroReferencePrice);
        }
        if self.weight_bps == 0 || self.weight_bps > HEDGING_BASIS_POINTS {
            return Err(HedgingValidationError::InvalidBasisPoints {
                field: "weight_bps",
                value: self.weight_bps,
            });
        }
        validate_digest("evidence_digest", self.evidence_digest)?;
        Ok(())
    }
}

/// Deterministic reference-price decision used by SoraFS billing.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct HedgingReferencePriceDecisionV1 {
    /// Schema version (`HEDGING_REFERENCE_PRICE_DECISION_VERSION_V1`).
    pub version: u8,
    /// BLAKE3-256 digest over the canonical decision body.
    pub decision_id: [u8; 32],
    /// Unix timestamp when the decision becomes effective.
    pub effective_at_unix: u64,
    /// Weighted USD price of one XOR.
    pub xor_usd_price: Quantity,
    /// Maximum accepted feed age, in seconds.
    pub max_feed_age_secs: u64,
    /// Divergence threshold against the weighted price, in basis points.
    pub max_divergence_bps: u16,
    /// Normalized feed inputs included in the decision.
    pub feeds: Vec<HedgingPriceFeedV1>,
    /// Whether any accepted feed or divergence check degraded the decision.
    pub degraded: bool,
    /// Deterministic degradation reasons.
    #[norito(default)]
    pub degradation_reasons: Vec<String>,
}

impl HedgingReferencePriceDecisionV1 {
    /// Validate the decision and replay the deterministic weighted average.
    pub fn validate(&self) -> Result<(), HedgingValidationError> {
        if self.version != HEDGING_REFERENCE_PRICE_DECISION_VERSION_V1 {
            return Err(
                HedgingValidationError::UnsupportedReferencePriceDecisionVersion {
                    found: self.version,
                },
            );
        }
        validate_digest("decision_id", self.decision_id)?;
        if self.effective_at_unix == 0 {
            return Err(HedgingValidationError::InvalidTimestamp {
                field: "effective_at_unix",
            });
        }
        if self.xor_usd_price.is_zero() {
            return Err(HedgingValidationError::ZeroReferencePrice);
        }
        if self.max_feed_age_secs == 0 {
            return Err(HedgingValidationError::ZeroMaxFeedAge);
        }
        validate_bps("max_divergence_bps", self.max_divergence_bps)?;
        validate_degradation_reasons(&self.degradation_reasons)?;
        validate_canonical_feed_order(&self.feeds)?;

        let replay = derive_reference_price_decision_v1(
            self.effective_at_unix,
            try_clone_feeds(&self.feeds)?,
            self.max_feed_age_secs,
            self.max_divergence_bps,
        )?;
        if self.xor_usd_price != replay.xor_usd_price {
            return Err(HedgingValidationError::ReferencePriceMismatch {
                expected: replay.xor_usd_price,
                actual: self.xor_usd_price.clone(),
            });
        }
        if self.degraded != replay.degraded
            || self.degradation_reasons != replay.degradation_reasons
        {
            return Err(HedgingValidationError::DegradationMismatch);
        }
        if self.decision_id != replay.decision_id {
            return Err(HedgingValidationError::DigestMismatch {
                field: "decision_id",
            });
        }
        Ok(())
    }
}

/// One canonical billing line in a SoraFS statement.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct BillingLineItemV1 {
    /// Schema version (`BILLING_LINE_ITEM_VERSION_V1`).
    pub version: u8,
    /// BLAKE3-256 digest over the line item body.
    pub line_id: [u8; 32],
    /// Billing category.
    pub kind: BillingLineItemKindV1,
    /// Debit or credit direction.
    pub direction: BillingLineDirectionV1,
    /// Source event, settlement, deal, or governance adjustment id.
    pub source_id: String,
    /// Exact XOR amount for the line.
    pub xor_amount: XorQuantity,
    /// Exact USD equivalent at the statement reference price.
    pub usd_amount: Quantity,
    /// Source-specific quantity, such as GiB-seconds or transferred bytes.
    #[norito(default)]
    pub quantity_units: u128,
    /// Optional human-readable note.
    #[norito(default)]
    pub note: Option<String>,
}

impl BillingLineItemV1 {
    /// Validate line structure and deterministic id binding.
    pub fn validate(&self) -> Result<(), HedgingValidationError> {
        if self.version != BILLING_LINE_ITEM_VERSION_V1 {
            return Err(HedgingValidationError::UnsupportedBillingLineVersion {
                found: self.version,
            });
        }
        validate_digest("line_id", self.line_id)?;
        validate_identifier("source_id", &self.source_id)?;
        if self.xor_amount.is_zero() {
            return Err(HedgingValidationError::ZeroBillingAmount);
        }
        if self.usd_amount.is_zero() {
            return Err(HedgingValidationError::ZeroUsdAmount);
        }
        validate_line_semantics(self)?;
        if let Some(note) = &self.note {
            validate_human_text("note", note, MAX_HEDGING_NOTE_BYTES)?;
        }
        let expected = billing_line_item_id_v1(self)?;
        if self.line_id != expected {
            return Err(HedgingValidationError::DigestMismatch { field: "line_id" });
        }
        Ok(())
    }
}

/// Weekly or ad-hoc SoraFS billing statement.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct BillingStatementV1 {
    /// Schema version (`BILLING_STATEMENT_VERSION_V1`).
    pub version: u8,
    /// BLAKE3-256 digest over the statement body.
    pub statement_id: [u8; 32],
    /// Account receiving the statement, encoded as canonical account bytes.
    pub account_id: Vec<u8>,
    /// Inclusive period start timestamp.
    pub period_start_unix: u64,
    /// Exclusive period end timestamp.
    pub period_end_unix: u64,
    /// Payment due timestamp.
    pub due_at_unix: u64,
    /// Reference XOR/USD decision used for all USD equivalents.
    pub reference_price: HedgingReferencePriceDecisionV1,
    /// Statement line items.
    pub lines: Vec<BillingLineItemV1>,
    /// Exact total debits in XOR.
    pub total_debit_xor: XorQuantity,
    /// Exact total credits in XOR.
    pub total_credit_xor: XorQuantity,
    /// Exact net amount due in XOR after credits.
    pub net_due_xor: XorQuantity,
    /// Exact total debits in USD.
    pub total_debit_usd: Quantity,
    /// Exact total credits in USD.
    pub total_credit_usd: Quantity,
    /// Exact net amount due in USD after credits.
    pub net_due_usd: Quantity,
    /// Previous statement id when this statement rolls forward a series.
    #[norito(default)]
    pub previous_statement_id: Option<[u8; 32]>,
}

impl BillingStatementV1 {
    /// Validate totals, period bounds, reference price, and statement id.
    pub fn validate(&self) -> Result<(), HedgingValidationError> {
        if self.version != BILLING_STATEMENT_VERSION_V1 {
            return Err(HedgingValidationError::UnsupportedBillingStatementVersion {
                found: self.version,
            });
        }
        validate_digest("statement_id", self.statement_id)?;
        if self.account_id.is_empty() {
            return Err(HedgingValidationError::EmptyAccountId);
        }
        if self.account_id.len() > MAX_BILLING_ACCOUNT_ID_BYTES {
            return Err(HedgingValidationError::ResourceLimitExceeded {
                field: "account_id",
                count: self.account_id.len(),
                max: MAX_BILLING_ACCOUNT_ID_BYTES,
            });
        }
        if self.period_start_unix == 0 || self.period_end_unix <= self.period_start_unix {
            return Err(HedgingValidationError::InvalidPeriod);
        }
        if self.due_at_unix <= self.period_end_unix {
            return Err(HedgingValidationError::InvalidDueAt);
        }
        self.reference_price.validate()?;
        if self.reference_price.effective_at_unix != self.period_end_unix {
            return Err(HedgingValidationError::ReferencePriceOutsidePeriod {
                effective_at_unix: self.reference_price.effective_at_unix,
                period_end_unix: self.period_end_unix,
            });
        }
        if self.lines.is_empty() {
            return Err(HedgingValidationError::NoBillingLines);
        }
        if self.lines.len() > MAX_BILLING_LINES {
            return Err(HedgingValidationError::ResourceLimitExceeded {
                field: "lines",
                count: self.lines.len(),
                max: MAX_BILLING_LINES,
            });
        }
        validate_optional_digest("previous_statement_id", self.previous_statement_id)?;
        if self.previous_statement_id == Some(self.statement_id) {
            return Err(HedgingValidationError::SelfReferentialStatement);
        }

        let totals = BillingTotals::from_lines(&self.lines, &self.reference_price.xor_usd_price)?;
        if self.total_debit_xor != totals.total_debit_xor
            || self.total_credit_xor != totals.total_credit_xor
            || self.net_due_xor != totals.net_due_xor
            || self.total_debit_usd != totals.total_debit_usd
            || self.total_credit_usd != totals.total_credit_usd
            || self.net_due_usd != totals.net_due_usd
        {
            return Err(HedgingValidationError::BillingTotalsMismatch);
        }
        let expected = billing_statement_id_v1(self)?;
        if self.statement_id != expected {
            return Err(HedgingValidationError::DigestMismatch {
                field: "statement_id",
            });
        }
        Ok(())
    }
}

/// Decode an exact canonical price-feed archive under production resource limits.
pub fn decode_hedging_price_feed_v1(
    bytes: &[u8],
) -> Result<HedgingPriceFeedV1, HedgingPayloadDecodeError> {
    decode_hedging_payload_v1(
        bytes,
        HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1,
        MAX_HEDGING_DEGRADATION_REASONS,
    )
}

/// Decode an exact canonical reference-price decision under production resource limits.
pub fn decode_hedging_reference_price_decision_v1(
    bytes: &[u8],
) -> Result<HedgingReferencePriceDecisionV1, HedgingPayloadDecodeError> {
    decode_hedging_payload_v1(
        bytes,
        HEDGING_DECISION_MAX_CANONICAL_BYTES_V1,
        MAX_HEDGING_DEGRADATION_REASONS,
    )
}

/// Decode an exact canonical billing-line archive under production resource limits.
pub fn decode_billing_line_item_v1(
    bytes: &[u8],
) -> Result<BillingLineItemV1, HedgingPayloadDecodeError> {
    decode_hedging_payload_v1(
        bytes,
        HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1,
        MAX_HEDGING_DEGRADATION_REASONS,
    )
}

/// Decode an exact canonical billing statement under production resource limits.
pub fn decode_billing_statement_v1(
    bytes: &[u8],
) -> Result<BillingStatementV1, HedgingPayloadDecodeError> {
    decode_hedging_payload_v1(
        bytes,
        BILLING_STATEMENT_MAX_CANONICAL_BYTES_V1,
        MAX_BILLING_LINES,
    )
}

fn decode_hedging_payload_v1<T>(
    bytes: &[u8],
    maximum_bytes: usize,
    maximum_sequence_elements: usize,
) -> Result<T, HedgingPayloadDecodeError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.len() > maximum_bytes {
        return Err(HedgingPayloadDecodeError::PayloadTooLarge {
            length: bytes.len(),
            maximum: maximum_bytes,
        });
    }
    let limits = norito::DecodeLimits::new(
        maximum_sequence_elements,
        maximum_bytes,
        maximum_bytes.saturating_mul(2),
        maximum_bytes.saturating_mul(4),
        HEDGING_DECODE_MAX_DEPTH_V1,
    );
    let payload: T = norito::decode_from_bytes_with_limits(bytes, limits).map_err(|error| {
        HedgingPayloadDecodeError::Decode {
            reason: error.to_string(),
        }
    })?;
    let canonical = norito::to_bytes(&payload).map_err(|error| {
        HedgingPayloadDecodeError::CanonicalEncoding {
            reason: error.to_string(),
        }
    })?;
    if canonical != bytes {
        return Err(HedgingPayloadDecodeError::NonCanonicalEncoding);
    }
    Ok(payload)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BillingTotals {
    total_debit_xor: XorQuantity,
    total_credit_xor: XorQuantity,
    net_due_xor: XorQuantity,
    total_debit_usd: Quantity,
    total_credit_usd: Quantity,
    net_due_usd: Quantity,
}

impl BillingTotals {
    fn from_lines(
        lines: &[BillingLineItemV1],
        reference_price_xor_usd: &Quantity,
    ) -> Result<Self, HedgingValidationError> {
        if lines.len() > MAX_BILLING_LINES {
            return Err(HedgingValidationError::ResourceLimitExceeded {
                field: "lines",
                count: lines.len(),
                max: MAX_BILLING_LINES,
            });
        }
        let mut total_debit_xor = XorQuantity::zero();
        let mut total_credit_xor = XorQuantity::zero();
        let mut total_debit_usd = Quantity::zero();
        let mut total_credit_usd = Quantity::zero();
        let mut previous_line_id: Option<[u8; 32]> = None;
        for (index, line) in lines.iter().enumerate() {
            line.validate()?;
            if let Some(previous) = previous_line_id {
                if previous == line.line_id {
                    return Err(HedgingValidationError::DuplicateLineId);
                }
                if previous > line.line_id {
                    return Err(HedgingValidationError::NonCanonicalOrder { field: "lines" });
                }
            }
            if lines[..index]
                .iter()
                .any(|previous| previous.source_id == line.source_id)
            {
                return Err(HedgingValidationError::DuplicateBillingSource {
                    source_id: line.source_id.clone(),
                });
            }
            previous_line_id = Some(line.line_id);
            let expected_usd = xor_to_usd(&line.xor_amount, reference_price_xor_usd)?;
            if line.usd_amount != expected_usd {
                return Err(HedgingValidationError::BillingLineUsdMismatch {
                    source_id: line.source_id.clone(),
                    expected: expected_usd,
                    actual: line.usd_amount.clone(),
                });
            }
            match line.direction {
                BillingLineDirectionV1::Debit => {
                    total_debit_xor = total_debit_xor.checked_add(&line.xor_amount)?;
                    total_debit_usd = total_debit_usd.checked_add(&line.usd_amount)?;
                }
                BillingLineDirectionV1::Credit => {
                    total_credit_xor = total_credit_xor.checked_add(&line.xor_amount)?;
                    total_credit_usd = total_credit_usd.checked_add(&line.usd_amount)?;
                }
            }
        }
        let net_due_xor = total_debit_xor
            .checked_sub(&total_credit_xor)
            .map_err(|error| match error {
                DealAmountError::Underflow => HedgingValidationError::CreditsExceedDebits,
                other => HedgingValidationError::from(other),
            })?;
        let net_due_usd = total_debit_usd
            .checked_sub(&total_credit_usd)
            .map_err(|error| match error {
                NumericOperationError::QuantityUnderflow => {
                    HedgingValidationError::CreditsExceedDebits
                }
                other => HedgingValidationError::Numeric(other),
            })?;
        Ok(Self {
            total_debit_xor,
            total_credit_xor,
            net_due_xor,
            total_debit_usd,
            total_credit_usd,
            net_due_usd,
        })
    }
}

/// Build a deterministic reference-price decision from accepted feeds.
pub fn derive_reference_price_decision_v1(
    effective_at_unix: u64,
    mut feeds: Vec<HedgingPriceFeedV1>,
    max_feed_age_secs: u64,
    max_divergence_bps: u16,
) -> Result<HedgingReferencePriceDecisionV1, HedgingValidationError> {
    if effective_at_unix == 0 {
        return Err(HedgingValidationError::InvalidTimestamp {
            field: "effective_at_unix",
        });
    }
    if feeds.is_empty() {
        return Err(HedgingValidationError::NoPriceFeeds);
    }
    if feeds.len() > MAX_HEDGING_PRICE_FEEDS {
        return Err(HedgingValidationError::ResourceLimitExceeded {
            field: "feeds",
            count: feeds.len(),
            max: MAX_HEDGING_PRICE_FEEDS,
        });
    }
    if max_feed_age_secs == 0 {
        return Err(HedgingValidationError::ZeroMaxFeedAge);
    }
    validate_bps("max_divergence_bps", max_divergence_bps)?;
    feeds.sort_by(|left, right| left.feed_id.cmp(&right.feed_id));

    let mut weight_sum = 0_u128;
    let mut degraded = false;
    let mut degradation_reasons = Vec::new();
    let degradation_capacity = feeds
        .len()
        .checked_mul(2)
        .ok_or(HedgingValidationError::AmountOverflow)?;
    degradation_reasons
        .try_reserve_exact(degradation_capacity)
        .map_err(|_| HedgingValidationError::AllocationFailed {
            context: "degradation reasons",
        })?;
    for (index, feed) in feeds.iter().enumerate() {
        feed.validate()?;
        if index > 0 && feeds[index - 1].feed_id == feed.feed_id {
            return Err(HedgingValidationError::DuplicateFeedId {
                feed_id: feed.feed_id.clone(),
            });
        }
        if feeds[..index]
            .iter()
            .any(|previous| previous.source == feed.source)
        {
            return Err(HedgingValidationError::DuplicateFeedSource {
                feed_source: feed.source.clone(),
            });
        }
        if matches!(feed.status, HedgingFeedStatusV1::Rejected) {
            return Err(HedgingValidationError::RejectedFeed {
                feed_id: feed.feed_id.clone(),
            });
        }
        if feed.observed_at_unix > effective_at_unix {
            return Err(HedgingValidationError::FutureFeed {
                feed_id: feed.feed_id.clone(),
            });
        }
        let age = effective_at_unix - feed.observed_at_unix;
        if age > max_feed_age_secs {
            return Err(HedgingValidationError::StaleFeed {
                feed_id: feed.feed_id.clone(),
                age_secs: age,
                max_feed_age_secs,
            });
        }
        if matches!(feed.status, HedgingFeedStatusV1::Degraded) {
            degraded = true;
            degradation_reasons.push(format!("feed:{}:collector_degraded", feed.feed_id));
        }
        weight_sum = weight_sum
            .checked_add(u128::from(feed.weight_bps))
            .ok_or(HedgingValidationError::AmountOverflow)?;
    }
    if weight_sum != u128::from(HEDGING_BASIS_POINTS) {
        return Err(HedgingValidationError::InvalidFeedWeightSum {
            total_bps: weight_sum,
        });
    }
    // V1 publishes a price rounded toward zero at six USD fractional digits.
    // The rounding boundary is part of the decision policy, not its storage
    // type. Products and the aggregate stay conceptually unbounded until this
    // division, preventing false overflow near the signed-512-bit boundary.
    let xor_usd_price = Quantity::try_weighted_average_round(
        feeds
            .iter()
            .map(|feed| (&feed.xor_usd_price, u64::from(feed.weight_bps))),
        6,
        RoundingMode::TowardZero,
    )?;
    for feed in &feeds {
        let divergence = divergence_bps(&feed.xor_usd_price, &xor_usd_price)?;
        if divergence > max_divergence_bps {
            degraded = true;
            degradation_reasons.push(format!(
                "feed:{}:divergence_bps:{}",
                feed.feed_id, divergence
            ));
        }
    }
    degradation_reasons.sort();
    let mut decision = HedgingReferencePriceDecisionV1 {
        version: HEDGING_REFERENCE_PRICE_DECISION_VERSION_V1,
        decision_id: [0_u8; 32],
        effective_at_unix,
        xor_usd_price,
        max_feed_age_secs,
        max_divergence_bps,
        feeds,
        degraded,
        degradation_reasons,
    };
    decision.decision_id = reference_price_decision_id_v1(&decision)?;
    Ok(decision)
}

/// Build a billing line item and bind it to a deterministic id.
pub fn build_billing_line_item_v1(
    kind: BillingLineItemKindV1,
    direction: BillingLineDirectionV1,
    source_id: impl Into<String>,
    xor_amount: XorQuantity,
    reference_price_xor_usd: &Quantity,
    quantity_units: u128,
    note: Option<String>,
) -> Result<BillingLineItemV1, HedgingValidationError> {
    if xor_amount.is_zero() {
        return Err(HedgingValidationError::ZeroBillingAmount);
    }
    if reference_price_xor_usd.is_zero() {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    if let Some(note) = &note {
        validate_human_text("note", note, MAX_HEDGING_NOTE_BYTES)?;
    }
    let usd_amount = xor_to_usd(&xor_amount, reference_price_xor_usd)?;
    let mut line = BillingLineItemV1 {
        version: BILLING_LINE_ITEM_VERSION_V1,
        line_id: [0_u8; 32],
        kind,
        direction,
        source_id: source_id.into(),
        xor_amount,
        usd_amount,
        quantity_units,
        note,
    };
    validate_identifier("source_id", &line.source_id)?;
    line.line_id = billing_line_item_id_v1(&line)?;
    line.validate()?;
    Ok(line)
}

/// Build a deterministic billing statement from validated line items.
pub fn build_billing_statement_v1(
    account_id: Vec<u8>,
    period_start_unix: u64,
    period_end_unix: u64,
    due_at_unix: u64,
    reference_price: HedgingReferencePriceDecisionV1,
    mut lines: Vec<BillingLineItemV1>,
    previous_statement_id: Option<[u8; 32]>,
) -> Result<BillingStatementV1, HedgingValidationError> {
    if account_id.is_empty() {
        return Err(HedgingValidationError::EmptyAccountId);
    }
    if account_id.len() > MAX_BILLING_ACCOUNT_ID_BYTES {
        return Err(HedgingValidationError::ResourceLimitExceeded {
            field: "account_id",
            count: account_id.len(),
            max: MAX_BILLING_ACCOUNT_ID_BYTES,
        });
    }
    if period_start_unix == 0 || period_end_unix <= period_start_unix {
        return Err(HedgingValidationError::InvalidPeriod);
    }
    if due_at_unix <= period_end_unix {
        return Err(HedgingValidationError::InvalidDueAt);
    }
    reference_price.validate()?;
    if reference_price.effective_at_unix != period_end_unix {
        return Err(HedgingValidationError::ReferencePriceOutsidePeriod {
            effective_at_unix: reference_price.effective_at_unix,
            period_end_unix,
        });
    }
    if lines.is_empty() {
        return Err(HedgingValidationError::NoBillingLines);
    }
    if lines.len() > MAX_BILLING_LINES {
        return Err(HedgingValidationError::ResourceLimitExceeded {
            field: "lines",
            count: lines.len(),
            max: MAX_BILLING_LINES,
        });
    }
    validate_optional_digest("previous_statement_id", previous_statement_id)?;
    lines.sort_by_key(|line| line.line_id);
    let totals = BillingTotals::from_lines(&lines, &reference_price.xor_usd_price)?;
    let mut statement = BillingStatementV1 {
        version: BILLING_STATEMENT_VERSION_V1,
        statement_id: [0_u8; 32],
        account_id,
        period_start_unix,
        period_end_unix,
        due_at_unix,
        reference_price,
        lines,
        total_debit_xor: totals.total_debit_xor,
        total_credit_xor: totals.total_credit_xor,
        net_due_xor: totals.net_due_xor,
        total_debit_usd: totals.total_debit_usd,
        total_credit_usd: totals.total_credit_usd,
        net_due_usd: totals.net_due_usd,
        previous_statement_id,
    };
    statement.statement_id = billing_statement_id_v1(&statement)?;
    if statement.previous_statement_id == Some(statement.statement_id) {
        return Err(HedgingValidationError::SelfReferentialStatement);
    }
    statement.validate()?;
    Ok(statement)
}

/// Validate exact linkage and contiguous periods for a statement series.
pub fn validate_billing_statement_transition(
    previous: Option<&BillingStatementV1>,
    next: &BillingStatementV1,
) -> Result<(), HedgingValidationError> {
    next.validate()?;
    match previous {
        Some(previous) => {
            previous.validate()?;
            if next.previous_statement_id != Some(previous.statement_id) {
                return Err(HedgingValidationError::PreviousStatementMismatch);
            }
            if next.account_id != previous.account_id {
                return Err(HedgingValidationError::StatementAccountMismatch);
            }
            if next.period_start_unix != previous.period_end_unix {
                return Err(HedgingValidationError::NonContiguousStatementPeriod {
                    expected_start: previous.period_end_unix,
                    actual_start: next.period_start_unix,
                });
            }
        }
        None if next.previous_statement_id.is_some() => {
            return Err(HedgingValidationError::UnexpectedInitialStatementPredecessor);
        }
        None => {}
    }
    Ok(())
}

/// Convert an exact XOR amount into USD using an exact USD-per-XOR price.
pub fn xor_to_usd(
    amount: &XorQuantity,
    reference_price_xor_usd: &Quantity,
) -> Result<Quantity, HedgingValidationError> {
    if reference_price_xor_usd.is_zero() {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    amount
        .as_quantity()
        .try_mul_decimal(reference_price_xor_usd.as_numeric())
        .map_err(HedgingValidationError::from)
}

/// Deterministically derive a reference-price decision id.
pub fn reference_price_decision_id_v1(
    decision: &HedgingReferencePriceDecisionV1,
) -> Result<[u8; 32], HedgingValidationError> {
    let mut body = decision.clone();
    body.decision_id = [0_u8; 32];
    hash_norito(REFERENCE_PRICE_DECISION_ID_DOMAIN_V1, &body)
}

/// Deterministically derive a billing line id.
pub fn billing_line_item_id_v1(
    line: &BillingLineItemV1,
) -> Result<[u8; 32], HedgingValidationError> {
    let mut body = line.clone();
    body.line_id = [0_u8; 32];
    hash_norito(BILLING_LINE_ITEM_ID_DOMAIN_V1, &body)
}

/// Deterministically derive a billing statement id.
pub fn billing_statement_id_v1(
    statement: &BillingStatementV1,
) -> Result<[u8; 32], HedgingValidationError> {
    let mut body = statement.clone();
    body.statement_id = [0_u8; 32];
    hash_norito(BILLING_STATEMENT_ID_DOMAIN_V1, &body)
}

fn divergence_bps(
    feed_price: &Quantity,
    reference_price: &Quantity,
) -> Result<u16, HedgingValidationError> {
    if reference_price.is_zero() {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    let delta = if feed_price >= reference_price {
        feed_price.checked_sub(reference_price)?
    } else {
        reference_price.checked_sub(feed_price)?
    };
    let bps = delta.as_numeric().try_decimal_mul_div_round(
        &Numeric::from(u64::from(HEDGING_BASIS_POINTS)),
        reference_price.as_numeric(),
        0,
        RoundingMode::TowardZero,
    )?;
    bps.try_mantissa_u128()
        .and_then(|value| value.try_into().ok())
        .ok_or(HedgingValidationError::AmountOverflow)
}

fn hash_norito<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], HedgingValidationError> {
    let bytes = norito::to_bytes(value)?;
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| HedgingValidationError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&bytes);
    Ok(hash_to_array(hasher.finalize()))
}

fn hash_to_array(hash: Hash) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), HedgingValidationError> {
    if value.is_empty()
        || value.len() > MAX_HEDGING_IDENTIFIER_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b':')
        })
    {
        return Err(HedgingValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_human_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), HedgingValidationError> {
    if value.len() > max_bytes {
        return Err(HedgingValidationError::TextTooLong {
            field,
            length: value.len(),
            max: max_bytes,
        });
    }
    if value.is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(HedgingValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_canonical_feed_order(
    feeds: &[HedgingPriceFeedV1],
) -> Result<(), HedgingValidationError> {
    if feeds.is_empty() {
        return Err(HedgingValidationError::NoPriceFeeds);
    }
    if feeds.len() > MAX_HEDGING_PRICE_FEEDS {
        return Err(HedgingValidationError::ResourceLimitExceeded {
            field: "feeds",
            count: feeds.len(),
            max: MAX_HEDGING_PRICE_FEEDS,
        });
    }
    let mut weight_sum = 0_u128;
    for (index, feed) in feeds.iter().enumerate() {
        feed.validate()?;
        if index > 0 {
            match feeds[index - 1].feed_id.cmp(&feed.feed_id) {
                std::cmp::Ordering::Equal => {
                    return Err(HedgingValidationError::DuplicateFeedId {
                        feed_id: feed.feed_id.clone(),
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(HedgingValidationError::NonCanonicalOrder { field: "feeds" });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        if feeds[..index]
            .iter()
            .any(|previous| previous.source == feed.source)
        {
            return Err(HedgingValidationError::DuplicateFeedSource {
                feed_source: feed.source.clone(),
            });
        }
        weight_sum = weight_sum
            .checked_add(u128::from(feed.weight_bps))
            .ok_or(HedgingValidationError::AmountOverflow)?;
    }
    if weight_sum != u128::from(HEDGING_BASIS_POINTS) {
        return Err(HedgingValidationError::InvalidFeedWeightSum {
            total_bps: weight_sum,
        });
    }
    Ok(())
}

fn try_clone_feeds(
    feeds: &[HedgingPriceFeedV1],
) -> Result<Vec<HedgingPriceFeedV1>, HedgingValidationError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(feeds.len()).map_err(|_| {
        HedgingValidationError::AllocationFailed {
            context: "price feed replay",
        }
    })?;
    for feed in feeds {
        cloned.push(HedgingPriceFeedV1 {
            version: feed.version,
            feed_id: try_clone_text(&feed.feed_id, "feed id replay")?,
            source: try_clone_text(&feed.source, "feed source replay")?,
            observed_at_unix: feed.observed_at_unix,
            xor_usd_price: feed.xor_usd_price.clone(),
            weight_bps: feed.weight_bps,
            evidence_digest: feed.evidence_digest,
            status: feed.status,
        });
    }
    Ok(cloned)
}

fn try_clone_text(value: &str, context: &'static str) -> Result<String, HedgingValidationError> {
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(value.len())
        .map_err(|_| HedgingValidationError::AllocationFailed { context })?;
    cloned.push_str(value);
    Ok(cloned)
}

fn validate_line_semantics(line: &BillingLineItemV1) -> Result<(), HedgingValidationError> {
    let expected_direction = match line.kind {
        BillingLineItemKindV1::Storage
        | BillingLineItemKindV1::Egress
        | BillingLineItemKindV1::ReserveRent
        | BillingLineItemKindV1::SettlementFee
        | BillingLineItemKindV1::Penalty => Some(BillingLineDirectionV1::Debit),
        BillingLineItemKindV1::IncentiveCredit => Some(BillingLineDirectionV1::Credit),
        BillingLineItemKindV1::Adjustment => None,
    };
    if expected_direction.is_some_and(|expected| line.direction != expected) {
        return Err(HedgingValidationError::BillingLineDirectionMismatch);
    }
    if matches!(
        line.kind,
        BillingLineItemKindV1::Storage
            | BillingLineItemKindV1::Egress
            | BillingLineItemKindV1::ReserveRent
    ) && line.quantity_units == 0
    {
        return Err(HedgingValidationError::ZeroBillingQuantity);
    }
    Ok(())
}

fn validate_digest(field: &'static str, digest: [u8; 32]) -> Result<(), HedgingValidationError> {
    if digest.iter().all(|byte| *byte == 0) {
        return Err(HedgingValidationError::InvalidDigest { field });
    }
    Ok(())
}

fn validate_optional_digest(
    field: &'static str,
    digest: Option<[u8; 32]>,
) -> Result<(), HedgingValidationError> {
    if let Some(digest) = digest {
        validate_digest(field, digest)?;
    }
    Ok(())
}

fn validate_bps(field: &'static str, value: u16) -> Result<(), HedgingValidationError> {
    if value > HEDGING_BASIS_POINTS {
        return Err(HedgingValidationError::InvalidBasisPoints { field, value });
    }
    Ok(())
}

fn validate_degradation_reasons(reasons: &[String]) -> Result<(), HedgingValidationError> {
    if reasons.len() > MAX_HEDGING_DEGRADATION_REASONS {
        return Err(HedgingValidationError::ResourceLimitExceeded {
            field: "degradation_reasons",
            count: reasons.len(),
            max: MAX_HEDGING_DEGRADATION_REASONS,
        });
    }
    let mut previous: Option<&str> = None;
    for reason in reasons {
        validate_identifier("degradation_reasons", reason)?;
        if let Some(previous) = previous {
            if previous == reason {
                return Err(HedgingValidationError::DuplicateDegradationReason);
            }
            if previous > reason.as_str() {
                return Err(HedgingValidationError::NonCanonicalOrder {
                    field: "degradation_reasons",
                });
            }
        }
        previous = Some(reason);
    }
    Ok(())
}

/// Failure to decode an attacker-controlled hedging or billing archive canonically.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HedgingPayloadDecodeError {
    /// The archive exceeds the payload-kind byte ceiling.
    #[error("hedging payload is {length} bytes; maximum canonical size is {maximum}")]
    PayloadTooLarge {
        /// Supplied archive length.
        length: usize,
        /// Maximum accepted canonical length.
        maximum: usize,
    },
    /// Norito rejected the archive under the bounded decode budget.
    #[error("failed to decode hedging payload: {reason}")]
    Decode {
        /// Codec diagnostic.
        reason: String,
    },
    /// Re-encoding the decoded value failed.
    #[error("failed to encode canonical hedging payload: {reason}")]
    CanonicalEncoding {
        /// Codec diagnostic.
        reason: String,
    },
    /// The archive decoded but was not the exact canonical Norito encoding.
    #[error("hedging payload is not the exact canonical Norito encoding")]
    NonCanonicalEncoding,
}

/// Validation and build errors for SoraFS hedging/billing payloads.
#[derive(Debug, Error)]
pub enum HedgingValidationError {
    /// Unsupported price-feed version.
    #[error("unsupported hedging price-feed version {found}")]
    UnsupportedPriceFeedVersion {
        /// Observed version.
        found: u8,
    },
    /// Unsupported reference-price decision version.
    #[error("unsupported hedging reference-price decision version {found}")]
    UnsupportedReferencePriceDecisionVersion {
        /// Observed version.
        found: u8,
    },
    /// Unsupported billing-line version.
    #[error("unsupported billing line version {found}")]
    UnsupportedBillingLineVersion {
        /// Observed version.
        found: u8,
    },
    /// Unsupported billing-statement version.
    #[error("unsupported billing statement version {found}")]
    UnsupportedBillingStatementVersion {
        /// Observed version.
        found: u8,
    },
    /// Required text field is blank.
    #[error("{field} must not be blank")]
    InvalidText {
        /// Field name.
        field: &'static str,
    },
    /// Text exceeds its schema byte bound.
    #[error("{field} length {length} exceeds maximum {max}")]
    TextTooLong {
        /// Field name.
        field: &'static str,
        /// Observed UTF-8 byte length.
        length: usize,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// A collection or byte string exceeds its schema bound.
    #[error("{field} count {count} exceeds maximum {max}")]
    ResourceLimitExceeded {
        /// Field name.
        field: &'static str,
        /// Observed count.
        count: usize,
        /// Maximum accepted count.
        max: usize,
    },
    /// A canonical sequence is not strictly ordered.
    #[error("{field} must be in canonical order")]
    NonCanonicalOrder {
        /// Sequence field name.
        field: &'static str,
    },
    /// Required digest is all zeroes.
    #[error("{field} digest must not be zero")]
    InvalidDigest {
        /// Field name.
        field: &'static str,
    },
    /// Timestamp field is zero or invalid for its context.
    #[error("{field} timestamp is invalid")]
    InvalidTimestamp {
        /// Field name.
        field: &'static str,
    },
    /// Basis-point field exceeds the supported range.
    #[error("{field} basis points {value} exceed 10_000")]
    InvalidBasisPoints {
        /// Field name.
        field: &'static str,
        /// Observed value.
        value: u16,
    },
    /// No accepted price feeds were supplied.
    #[error("at least one price feed is required")]
    NoPriceFeeds,
    /// Duplicate feed id was supplied.
    #[error("duplicate price-feed id `{feed_id}`")]
    DuplicateFeedId {
        /// Duplicate feed id.
        feed_id: String,
    },
    /// Two feed identifiers reuse the same governed source.
    #[error("duplicate price-feed source `{feed_source}`")]
    DuplicateFeedSource {
        /// Duplicate source.
        feed_source: String,
    },
    /// Aggregate feed weights do not form one exact basis-point unit.
    #[error("price-feed weights sum to {total_bps}; expected 10_000")]
    InvalidFeedWeightSum {
        /// Observed total basis points.
        total_bps: u128,
    },
    /// A rejected feed was supplied to the decision engine.
    #[error("rejected feed `{feed_id}` cannot enter a reference-price decision")]
    RejectedFeed {
        /// Rejected feed id.
        feed_id: String,
    },
    /// A feed timestamp is newer than the decision timestamp.
    #[error("feed `{feed_id}` was observed in the future relative to the decision")]
    FutureFeed {
        /// Feed id.
        feed_id: String,
    },
    /// A feed is older than the decision policy allows.
    #[error("feed `{feed_id}` age {age_secs}s exceeds {max_feed_age_secs}s")]
    StaleFeed {
        /// Feed id.
        feed_id: String,
        /// Observed age.
        age_secs: u64,
        /// Maximum accepted age.
        max_feed_age_secs: u64,
    },
    /// Maximum feed age must be non-zero.
    #[error("max_feed_age_secs must be non-zero")]
    ZeroMaxFeedAge,
    /// Reference price must be non-zero.
    #[error("reference price must be non-zero")]
    ZeroReferencePrice,
    /// Decision replay produced a different reference price.
    #[error("reference price mismatch: expected {expected}, got {actual}")]
    ReferencePriceMismatch {
        /// Replayed price.
        expected: Quantity,
        /// Stored price.
        actual: Quantity,
    },
    /// Decision replay produced different degradation metadata.
    #[error("degradation metadata does not match replayed decision")]
    DegradationMismatch,
    /// Duplicate degradation reason was supplied.
    #[error("duplicate degradation reason")]
    DuplicateDegradationReason,
    /// Billing amount must be non-zero.
    #[error("billing amount must be non-zero")]
    ZeroBillingAmount,
    /// A metered billing category has no quantity.
    #[error("metered billing line quantity must be non-zero")]
    ZeroBillingQuantity,
    /// Billing category and debit/credit direction disagree.
    #[error("billing line direction is invalid for its category")]
    BillingLineDirectionMismatch,
    /// USD amount must be non-zero.
    #[error("USD amount must be non-zero")]
    ZeroUsdAmount,
    /// Statement account id is missing.
    #[error("statement account id must not be empty")]
    EmptyAccountId,
    /// Statement period bounds are invalid.
    #[error("statement period is invalid")]
    InvalidPeriod,
    /// Statement due-at timestamp is invalid.
    #[error("statement due_at must be after the period end")]
    InvalidDueAt,
    /// Reference price is not anchored exactly at the statement period end.
    #[error(
        "reference price effective_at {effective_at_unix} does not equal period end {period_end_unix}"
    )]
    ReferencePriceOutsidePeriod {
        /// Reference decision effective timestamp.
        effective_at_unix: u64,
        /// Statement period end.
        period_end_unix: u64,
    },
    /// Statement has no billing lines.
    #[error("statement must contain at least one billing line")]
    NoBillingLines,
    /// Duplicate billing line id was supplied.
    #[error("duplicate billing line id")]
    DuplicateLineId,
    /// Multiple lines claim the same source event.
    #[error("duplicate billing source `{source_id}`")]
    DuplicateBillingSource {
        /// Duplicate source identifier.
        source_id: String,
    },
    /// Statement points to itself as predecessor.
    #[error("billing statement must not reference itself as predecessor")]
    SelfReferentialStatement,
    /// Statement does not extend the exact retained predecessor.
    #[error("billing statement predecessor does not match the retained head")]
    PreviousStatementMismatch,
    /// Statement series changed account identity.
    #[error("billing statement account does not match its predecessor")]
    StatementAccountMismatch,
    /// Statement periods overlap or contain a gap.
    #[error("billing statement period must start at {expected_start}, found {actual_start}")]
    NonContiguousStatementPeriod {
        /// Required next start.
        expected_start: u64,
        /// Observed next start.
        actual_start: u64,
    },
    /// Initial statement unexpectedly names a predecessor.
    #[error("initial billing statement must not name a predecessor")]
    UnexpectedInitialStatementPredecessor,
    /// Billing totals do not match line replay.
    #[error("billing totals do not match line replay")]
    BillingTotalsMismatch,
    /// Billing line USD equivalent was not derived from the statement reference price.
    #[error(
        "billing line `{source_id}` USD equivalent mismatch: expected {expected}, got {actual}"
    )]
    BillingLineUsdMismatch {
        /// Source id for the mismatched line.
        source_id: String,
        /// Replayed exact USD amount.
        expected: Quantity,
        /// Stored exact USD amount.
        actual: Quantity,
    },
    /// Statement credits exceed its debits, so the non-negative net due is undefined.
    #[error("billing credits exceed debits")]
    CreditsExceedDebits,
    /// Amount arithmetic overflowed.
    #[error("amount overflow")]
    AmountOverflow,
    /// Amount arithmetic underflowed.
    #[error("amount underflow")]
    AmountUnderflow,
    /// A signed value was supplied for a nominal XOR amount.
    #[error("XOR amount cannot be negative")]
    NegativeAmount,
    /// An XOR amount exceeded the canonical fractional precision bound.
    #[error("XOR amount scale {scale} exceeds maximum {max}")]
    AmountScaleOverflow {
        /// Observed fractional digit count.
        scale: u32,
        /// Maximum accepted fractional digit count.
        max: u32,
    },
    /// Amount cannot be represented by the V1 micro-XOR accounting domain.
    #[error("amount has precision below one micro-XOR")]
    InexactAmountPrecision,
    /// Canonical payload length cannot be represented in the hash preimage.
    #[error("canonical hedging payload length overflow")]
    LengthOverflow,
    /// A bounded allocation failed.
    #[error("hedging allocation failed for {context}")]
    AllocationFailed {
        /// Allocation context.
        context: &'static str,
    },
    /// Digest binding did not replay.
    #[error("{field} digest does not match canonical payload")]
    DigestMismatch {
        /// Field name.
        field: &'static str,
    },
    /// Norito serialization failed.
    #[error("norito serialization failed: {0}")]
    Norito(#[from] NoritoError),
    /// Exact decimal arithmetic failed.
    #[error("numeric arithmetic failed: {0}")]
    Numeric(#[from] NumericOperationError),
}

impl From<DealAmountError> for HedgingValidationError {
    fn from(error: DealAmountError) -> Self {
        match error {
            DealAmountError::Overflow => Self::AmountOverflow,
            DealAmountError::Underflow => Self::AmountUnderflow,
            DealAmountError::NegativeQuantity => Self::NegativeAmount,
            DealAmountError::InexactMicroProjection => Self::InexactAmountPrecision,
            DealAmountError::ScaleOverflow { scale, max } => {
                Self::AmountScaleOverflow { scale, max }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantity(value: &str) -> Quantity {
        value.parse().expect("canonical quantity")
    }

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    #[test]
    fn deal_amount_errors_keep_negative_and_scale_failures_distinct() {
        assert!(matches!(
            HedgingValidationError::from(DealAmountError::NegativeQuantity),
            HedgingValidationError::NegativeAmount
        ));
        assert!(matches!(
            HedgingValidationError::from(DealAmountError::ScaleOverflow { scale: 10, max: 9 }),
            HedgingValidationError::AmountScaleOverflow { scale: 10, max: 9 }
        ));
    }

    fn digest(label: &str) -> [u8; 32] {
        hash_to_array(blake3::hash(label.as_bytes()))
    }

    fn feed(feed_id: &str, price: &str, observed_at_unix: u64) -> HedgingPriceFeedV1 {
        HedgingPriceFeedV1 {
            version: HEDGING_PRICE_FEED_VERSION_V1,
            feed_id: feed_id.into(),
            source: format!("{feed_id}-source"),
            observed_at_unix,
            xor_usd_price: quantity(price),
            weight_bps: 5_000,
            evidence_digest: digest(feed_id),
            status: HedgingFeedStatusV1::Ok,
        }
    }

    fn single_feed(feed_id: &str, price: u64, observed_at_unix: u64) -> HedgingPriceFeedV1 {
        let whole = price / 1_000_000;
        let fractional = price % 1_000_000;
        let canonical_price = format!("{whole}.{fractional:06}");
        let mut feed = feed(feed_id, &canonical_price, observed_at_unix);
        feed.weight_bps = HEDGING_BASIS_POINTS;
        feed
    }

    #[test]
    fn reference_price_decision_is_deterministic_and_flags_divergence() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![feed("secondary", "1", 1_760), feed("primary", "1.2", 1_770)],
            120,
            500,
        )
        .expect("decision");

        assert_eq!(decision.xor_usd_price, quantity("1.1"));
        assert!(decision.degraded);
        assert_eq!(decision.degradation_reasons.len(), 2);
        decision.validate().expect("valid decision");
        assert_eq!(
            decision.decision_id,
            reference_price_decision_id_v1(&decision).unwrap()
        );
    }

    #[test]
    fn reference_price_uses_unbounded_weighted_intermediates() {
        let maximum = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![
                feed("secondary", maximum, 1_760),
                feed("primary", maximum, 1_770),
            ],
            120,
            500,
        )
        .expect("the bounded final average is representable");

        assert_eq!(decision.xor_usd_price, quantity(maximum));
        assert!(!decision.degraded);
        decision.validate().expect("maximum decision replays");
    }

    #[test]
    fn stale_feed_is_rejected_before_decision() {
        let err =
            derive_reference_price_decision_v1(1_800, vec![feed("primary", "1", 1_000)], 120, 500)
                .expect_err("stale feed");

        assert!(matches!(err, HedgingValidationError::StaleFeed { .. }));
    }

    #[test]
    fn billing_line_item_converts_xor_exactly() {
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "deal-1",
            xor("1.000001"),
            &quantity("2"),
            3_600,
            None,
        )
        .expect("line");

        assert_eq!(line.usd_amount, quantity("2.000002"));
        line.validate().expect("valid line");
    }

    #[test]
    fn billing_line_item_preserves_sub_micro_precision() {
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "sub-micro-deal",
            xor("0.0000001"),
            &quantity("2"),
            1,
            None,
        )
        .expect("sub-micro billing line");

        assert_eq!(line.xor_amount, xor("0.0000001"));
        assert_eq!(line.usd_amount, quantity("0.0000002"));
        line.validate().expect("sub-micro line remains valid");
    }

    #[test]
    fn billing_line_item_rejects_exact_numeric_overflow() {
        let maximum = xor(
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047",
        );

        assert!(matches!(
            build_billing_line_item_v1(
                BillingLineItemKindV1::Storage,
                BillingLineDirectionV1::Debit,
                "overflow-deal",
                maximum,
                &quantity("2"),
                1,
                None,
            ),
            Err(HedgingValidationError::Numeric(
                NumericOperationError::MantissaOverflow
            ))
        ));
    }

    #[test]
    fn billing_statement_totals_and_roundtrip_are_deterministic() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![feed("primary", "2", 1_790), feed("secondary", "2", 1_785)],
            120,
            500,
        )
        .expect("decision");
        let storage = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "deal-storage",
            xor("10"),
            &decision.xor_usd_price,
            86_400,
            Some("weekly storage".into()),
        )
        .expect("storage line");
        let credit = build_billing_line_item_v1(
            BillingLineItemKindV1::IncentiveCredit,
            BillingLineDirectionV1::Credit,
            "incentive-1",
            xor("2"),
            &decision.xor_usd_price,
            1,
            None,
        )
        .expect("credit line");
        let statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_000,
            1_800,
            2_000,
            decision,
            vec![storage, credit],
            Some(digest("previous")),
        )
        .expect("statement");

        assert_eq!(statement.total_debit_xor, xor("10"));
        assert_eq!(statement.total_credit_xor, xor("2"));
        assert_eq!(statement.net_due_xor, xor("8"));
        assert_eq!(statement.total_debit_usd, quantity("20"));
        assert_eq!(statement.total_credit_usd, quantity("4"));
        assert_eq!(statement.net_due_usd, quantity("16"));
        statement.validate().expect("valid statement");

        let bytes = norito::to_bytes(&statement).expect("encode statement");
        let decoded =
            norito::decode_from_bytes::<BillingStatementV1>(&bytes).expect("decode statement");
        assert_eq!(decoded, statement);
        let json = norito::json::to_string(&statement).expect("json statement");
        let decoded_json =
            norito::json::from_str::<BillingStatementV1>(&json).expect("decode json");
        assert_eq!(decoded_json, statement);
    }

    #[test]
    fn billing_statement_rejects_tampered_totals() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![single_feed("primary", 2_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Egress,
            BillingLineDirectionV1::Debit,
            "egress-1",
            xor("1"),
            &decision.xor_usd_price,
            1024,
            None,
        )
        .expect("line");
        let mut statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_000,
            1_800,
            2_000,
            decision,
            vec![line],
            None,
        )
        .expect("statement");
        statement.total_debit_usd = statement
            .total_debit_usd
            .checked_add(&quantity("0.000001"))
            .expect("tampered total remains representable");

        let err = statement.validate().expect_err("tampered totals");
        assert!(matches!(err, HedgingValidationError::BillingTotalsMismatch));
    }

    #[test]
    fn billing_statement_rejects_line_usd_mismatch() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![single_feed("primary", 2_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Egress,
            BillingLineDirectionV1::Debit,
            "egress-1",
            xor("1"),
            &decision.xor_usd_price,
            1024,
            None,
        )
        .expect("line");
        let mut statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_000,
            1_800,
            2_000,
            decision,
            vec![line],
            None,
        )
        .expect("statement");
        statement.lines[0].usd_amount = statement.lines[0]
            .usd_amount
            .checked_add(&quantity("0.000001"))
            .expect("tampered amount remains representable");
        statement.lines[0].line_id =
            billing_line_item_id_v1(&statement.lines[0]).expect("rebind tampered line");
        statement.total_debit_usd = statement
            .total_debit_usd
            .checked_add(&quantity("0.000001"))
            .expect("tampered debit remains representable");
        statement.net_due_usd = statement
            .net_due_usd
            .checked_add(&quantity("0.000001"))
            .expect("tampered net remains representable");
        statement.statement_id = billing_statement_id_v1(&statement).expect("rebind statement");

        let err = statement.validate().expect_err("tampered line USD");
        assert!(matches!(
            err,
            HedgingValidationError::BillingLineUsdMismatch { .. }
        ));
    }

    #[test]
    fn billing_statement_rejects_credits_above_debits() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![single_feed("primary", 2_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let debit = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "debit",
            xor("1"),
            &decision.xor_usd_price,
            1,
            None,
        )
        .expect("debit");
        let credit = build_billing_line_item_v1(
            BillingLineItemKindV1::IncentiveCredit,
            BillingLineDirectionV1::Credit,
            "credit",
            xor("2"),
            &decision.xor_usd_price,
            1,
            None,
        )
        .expect("credit");

        let error = build_billing_statement_v1(
            b"alice".to_vec(),
            1_000,
            1_800,
            2_000,
            decision,
            vec![debit, credit],
            None,
        )
        .expect_err("credits above debits must fail");
        assert!(matches!(error, HedgingValidationError::CreditsExceedDebits));
    }

    #[test]
    fn feeds_reject_noncanonical_text_sources_weights_order_and_cardinality() {
        let mut padded = single_feed("primary", 1_000_000, 1_790);
        padded.feed_id = " primary".into();
        assert!(matches!(
            padded.validate(),
            Err(HedgingValidationError::InvalidText { field: "feed_id" })
        ));

        let mut duplicate_source_a = feed("a", "1", 1_790);
        duplicate_source_a.source = "same-source".into();
        let mut duplicate_source_b = feed("b", "1", 1_790);
        duplicate_source_b.source = "same-source".into();
        assert!(matches!(
            derive_reference_price_decision_v1(
                1_800,
                vec![duplicate_source_a, duplicate_source_b],
                120,
                500,
            ),
            Err(HedgingValidationError::DuplicateFeedSource { .. })
        ));

        assert!(matches!(
            derive_reference_price_decision_v1(1_800, vec![feed("primary", "1", 1_790)], 120, 500,)
                .expect_err("partial weight budget"),
            HedgingValidationError::InvalidFeedWeightSum { total_bps: 5_000 }
        ));

        let mut decision = derive_reference_price_decision_v1(
            1_800,
            vec![feed("primary", "1", 1_790), feed("secondary", "1", 1_790)],
            120,
            500,
        )
        .expect("canonical decision");
        decision.feeds.swap(0, 1);
        assert!(matches!(
            decision.validate(),
            Err(HedgingValidationError::NonCanonicalOrder { field: "feeds" })
        ));

        let feeds = (0..=MAX_HEDGING_PRICE_FEEDS)
            .map(|index| {
                let mut feed = feed(&format!("feed-{index:02}"), "1", 1_790);
                feed.weight_bps = 1;
                feed
            })
            .collect();
        assert!(matches!(
            derive_reference_price_decision_v1(1_800, feeds, 120, 500).expect_err("feed count cap"),
            HedgingValidationError::ResourceLimitExceeded {
                field: "feeds",
                count,
                max,
            } if count == MAX_HEDGING_PRICE_FEEDS + 1 && max == MAX_HEDGING_PRICE_FEEDS
        ));
    }

    #[test]
    fn billing_lines_reject_direction_quantity_notes_and_duplicate_sources() {
        assert!(matches!(
            build_billing_line_item_v1(
                BillingLineItemKindV1::Storage,
                BillingLineDirectionV1::Credit,
                "storage-source",
                xor("0.000001"),
                &quantity("1"),
                1,
                None,
            )
            .expect_err("storage credit"),
            HedgingValidationError::BillingLineDirectionMismatch
        ));
        assert!(matches!(
            build_billing_line_item_v1(
                BillingLineItemKindV1::Egress,
                BillingLineDirectionV1::Debit,
                "egress-source",
                xor("0.000001"),
                &quantity("1"),
                0,
                None,
            )
            .expect_err("zero metered quantity"),
            HedgingValidationError::ZeroBillingQuantity
        ));
        assert!(matches!(
            build_billing_line_item_v1(
                BillingLineItemKindV1::Adjustment,
                BillingLineDirectionV1::Debit,
                "adjustment-source",
                xor("0.000001"),
                &quantity("1"),
                0,
                Some("x".repeat(MAX_HEDGING_NOTE_BYTES + 1)),
            ),
            Err(HedgingValidationError::TextTooLong { field: "note", .. })
        ));

        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![single_feed("primary", 1_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let debit = build_billing_line_item_v1(
            BillingLineItemKindV1::Adjustment,
            BillingLineDirectionV1::Debit,
            "same-source",
            xor("0.000002"),
            &decision.xor_usd_price,
            0,
            None,
        )
        .expect("debit");
        let credit = build_billing_line_item_v1(
            BillingLineItemKindV1::Adjustment,
            BillingLineDirectionV1::Credit,
            "same-source",
            xor("0.000001"),
            &decision.xor_usd_price,
            0,
            None,
        )
        .expect("credit");
        assert!(matches!(
            build_billing_statement_v1(
                b"alice".to_vec(),
                1_000,
                1_800,
                2_000,
                decision,
                vec![debit, credit],
                None,
            ),
            Err(HedgingValidationError::DuplicateBillingSource { .. })
        ));
    }

    #[test]
    fn billing_statement_transition_binds_head_account_and_contiguous_period() {
        fn statement(
            account: &[u8],
            start: u64,
            end: u64,
            previous: Option<[u8; 32]>,
            source: &str,
        ) -> BillingStatementV1 {
            let decision = derive_reference_price_decision_v1(
                end,
                vec![single_feed("primary", 1_000_000, end - 1)],
                120,
                500,
            )
            .expect("decision");
            let line = build_billing_line_item_v1(
                BillingLineItemKindV1::Adjustment,
                BillingLineDirectionV1::Debit,
                source,
                xor("0.000001"),
                &decision.xor_usd_price,
                0,
                None,
            )
            .expect("line");
            build_billing_statement_v1(
                account.to_vec(),
                start,
                end,
                end + 100,
                decision,
                vec![line],
                previous,
            )
            .expect("statement")
        }

        let first = statement(b"alice", 1_000, 1_800, None, "source-one");
        validate_billing_statement_transition(None, &first).expect("initial statement");
        let second = statement(
            b"alice",
            1_800,
            2_600,
            Some(first.statement_id),
            "source-two",
        );
        validate_billing_statement_transition(Some(&first), &second).expect("contiguous successor");

        let wrong_head = statement(b"alice", 1_800, 2_600, Some([0x99; 32]), "source-three");
        assert!(matches!(
            validate_billing_statement_transition(Some(&first), &wrong_head),
            Err(HedgingValidationError::PreviousStatementMismatch)
        ));
        let wrong_account = statement(
            b"bob",
            1_800,
            2_600,
            Some(first.statement_id),
            "source-four",
        );
        assert!(matches!(
            validate_billing_statement_transition(Some(&first), &wrong_account),
            Err(HedgingValidationError::StatementAccountMismatch)
        ));
        let gap = statement(
            b"alice",
            1_801,
            2_600,
            Some(first.statement_id),
            "source-five",
        );
        assert!(matches!(
            validate_billing_statement_transition(Some(&first), &gap),
            Err(HedgingValidationError::NonContiguousStatementPeriod { .. })
        ));
        assert!(matches!(
            validate_billing_statement_transition(None, &second),
            Err(HedgingValidationError::UnexpectedInitialStatementPredecessor)
        ));
    }

    #[test]
    fn bounded_hedging_decoder_accepts_exact_canonical_archive() {
        let feed = single_feed("primary", 1_000_000, 1_799);
        let encoded = norito::to_bytes(&feed).expect("encode feed");
        assert_eq!(
            decode_hedging_price_feed_v1(&encoded).expect("decode canonical feed"),
            feed
        );
    }

    #[test]
    fn bounded_hedging_decoder_rejects_oversize_and_trailing_bytes() {
        let archive = vec![0_u8; HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1 + 1];
        assert_eq!(
            decode_hedging_price_feed_v1(&archive),
            Err(HedgingPayloadDecodeError::PayloadTooLarge {
                length: archive.len(),
                maximum: HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1,
            })
        );

        let mut encoded =
            norito::to_bytes(&single_feed("primary", 1_000_000, 1_799)).expect("encode feed");
        encoded.push(0);
        assert!(decode_hedging_price_feed_v1(&encoded).is_err());
    }
}
