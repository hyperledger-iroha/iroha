#![allow(unexpected_cfgs)]

//! Deterministic SoraFS hedging and billing payload foundations.

use std::collections::BTreeSet;

use blake3::{Hash, Hasher};
use norito::{
    core::Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::deal::{BASIS_POINTS_PER_UNIT, DealAmountError, MICRO_XOR_PER_XOR, XorAmount};

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
    /// USD micro-units per one XOR.
    pub xor_usd_micros: u64,
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
        validate_text("feed_id", &self.feed_id)?;
        validate_text("source", &self.source)?;
        if self.observed_at_unix == 0 {
            return Err(HedgingValidationError::InvalidTimestamp {
                field: "observed_at_unix",
            });
        }
        if self.xor_usd_micros == 0 {
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
    /// Weighted USD micro-units per one XOR.
    pub xor_usd_micros: u64,
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
        if self.xor_usd_micros == 0 {
            return Err(HedgingValidationError::ZeroReferencePrice);
        }
        if self.max_feed_age_secs == 0 {
            return Err(HedgingValidationError::ZeroMaxFeedAge);
        }
        validate_bps("max_divergence_bps", self.max_divergence_bps)?;
        validate_degradation_reasons(&self.degradation_reasons)?;

        let replay = derive_reference_price_decision_v1(
            self.effective_at_unix,
            self.feeds.clone(),
            self.max_feed_age_secs,
            self.max_divergence_bps,
        )?;
        if self.xor_usd_micros != replay.xor_usd_micros {
            return Err(HedgingValidationError::ReferencePriceMismatch {
                expected: replay.xor_usd_micros,
                actual: self.xor_usd_micros,
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
    /// XOR amount for the line, in micro-XOR.
    pub xor_amount: XorAmount,
    /// USD equivalent in micro-units at the statement reference price.
    pub usd_micros: u128,
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
        validate_text("source_id", &self.source_id)?;
        if self.xor_amount.is_zero() {
            return Err(HedgingValidationError::ZeroBillingAmount);
        }
        if self.usd_micros == 0 {
            return Err(HedgingValidationError::ZeroUsdAmount);
        }
        if let Some(note) = &self.note {
            validate_text("note", note)?;
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
    /// Total debits in micro-XOR.
    pub total_debit_xor: XorAmount,
    /// Total credits in micro-XOR.
    pub total_credit_xor: XorAmount,
    /// Net amount due in micro-XOR after credits.
    pub net_due_xor: XorAmount,
    /// Total debits in USD micro-units.
    pub total_debit_usd_micros: u128,
    /// Total credits in USD micro-units.
    pub total_credit_usd_micros: u128,
    /// Net amount due in USD micro-units after credits.
    pub net_due_usd_micros: u128,
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
        if self.period_start_unix == 0 || self.period_end_unix <= self.period_start_unix {
            return Err(HedgingValidationError::InvalidPeriod);
        }
        if self.due_at_unix <= self.period_end_unix {
            return Err(HedgingValidationError::InvalidDueAt);
        }
        self.reference_price.validate()?;
        if self.lines.is_empty() {
            return Err(HedgingValidationError::NoBillingLines);
        }
        validate_optional_digest("previous_statement_id", self.previous_statement_id)?;

        let totals = BillingTotals::from_lines(&self.lines, self.reference_price.xor_usd_micros)?;
        if self.total_debit_xor != totals.total_debit_xor
            || self.total_credit_xor != totals.total_credit_xor
            || self.net_due_xor != totals.net_due_xor
            || self.total_debit_usd_micros != totals.total_debit_usd_micros
            || self.total_credit_usd_micros != totals.total_credit_usd_micros
            || self.net_due_usd_micros != totals.net_due_usd_micros
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BillingTotals {
    total_debit_xor: XorAmount,
    total_credit_xor: XorAmount,
    net_due_xor: XorAmount,
    total_debit_usd_micros: u128,
    total_credit_usd_micros: u128,
    net_due_usd_micros: u128,
}

impl BillingTotals {
    fn from_lines(
        lines: &[BillingLineItemV1],
        reference_price_xor_usd_micros: u64,
    ) -> Result<Self, HedgingValidationError> {
        let mut line_ids = BTreeSet::new();
        let mut total_debit_xor = XorAmount::zero();
        let mut total_credit_xor = XorAmount::zero();
        let mut total_debit_usd_micros = 0_u128;
        let mut total_credit_usd_micros = 0_u128;
        for line in lines {
            line.validate()?;
            if !line_ids.insert(line.line_id) {
                return Err(HedgingValidationError::DuplicateLineId);
            }
            let expected_usd_micros =
                xor_to_usd_micros(line.xor_amount, reference_price_xor_usd_micros)?;
            if line.usd_micros != expected_usd_micros {
                return Err(HedgingValidationError::BillingLineUsdMismatch {
                    source_id: line.source_id.clone(),
                    expected: expected_usd_micros,
                    actual: line.usd_micros,
                });
            }
            match line.direction {
                BillingLineDirectionV1::Debit => {
                    total_debit_xor = total_debit_xor.checked_add(line.xor_amount)?;
                    total_debit_usd_micros = total_debit_usd_micros
                        .checked_add(line.usd_micros)
                        .ok_or(HedgingValidationError::AmountOverflow)?;
                }
                BillingLineDirectionV1::Credit => {
                    total_credit_xor = total_credit_xor.checked_add(line.xor_amount)?;
                    total_credit_usd_micros = total_credit_usd_micros
                        .checked_add(line.usd_micros)
                        .ok_or(HedgingValidationError::AmountOverflow)?;
                }
            }
        }
        let net_due_xor = total_debit_xor.saturating_sub(total_credit_xor);
        let net_due_usd_micros = total_debit_usd_micros.saturating_sub(total_credit_usd_micros);
        Ok(Self {
            total_debit_xor,
            total_credit_xor,
            net_due_xor,
            total_debit_usd_micros,
            total_credit_usd_micros,
            net_due_usd_micros,
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
    if max_feed_age_secs == 0 {
        return Err(HedgingValidationError::ZeroMaxFeedAge);
    }
    validate_bps("max_divergence_bps", max_divergence_bps)?;
    feeds.sort_by(|left, right| left.feed_id.cmp(&right.feed_id));

    let mut seen = BTreeSet::new();
    let mut weighted_sum = 0_u128;
    let mut weight_sum = 0_u128;
    let mut degraded = false;
    let mut degradation_reasons = Vec::new();
    for feed in &feeds {
        feed.validate()?;
        if !seen.insert(feed.feed_id.clone()) {
            return Err(HedgingValidationError::DuplicateFeedId {
                feed_id: feed.feed_id.clone(),
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
        let weight = u128::from(feed.weight_bps);
        weighted_sum = weighted_sum
            .checked_add(u128::from(feed.xor_usd_micros).saturating_mul(weight))
            .ok_or(HedgingValidationError::AmountOverflow)?;
        weight_sum = weight_sum
            .checked_add(weight)
            .ok_or(HedgingValidationError::AmountOverflow)?;
    }
    if weight_sum == 0 {
        return Err(HedgingValidationError::NoPriceFeeds);
    }
    let xor_usd_micros = (weighted_sum / weight_sum)
        .try_into()
        .map_err(|_| HedgingValidationError::AmountOverflow)?;
    for feed in &feeds {
        let divergence = divergence_bps(feed.xor_usd_micros, xor_usd_micros)?;
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
        xor_usd_micros,
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
    xor_amount: XorAmount,
    reference_price_xor_usd_micros: u64,
    quantity_units: u128,
    note: Option<String>,
) -> Result<BillingLineItemV1, HedgingValidationError> {
    if xor_amount.is_zero() {
        return Err(HedgingValidationError::ZeroBillingAmount);
    }
    if reference_price_xor_usd_micros == 0 {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    if let Some(note) = &note {
        validate_text("note", note)?;
    }
    let usd_micros = xor_to_usd_micros(xor_amount, reference_price_xor_usd_micros)?;
    let mut line = BillingLineItemV1 {
        version: BILLING_LINE_ITEM_VERSION_V1,
        line_id: [0_u8; 32],
        kind,
        direction,
        source_id: source_id.into(),
        xor_amount,
        usd_micros,
        quantity_units,
        note,
    };
    validate_text("source_id", &line.source_id)?;
    line.line_id = billing_line_item_id_v1(&line)?;
    Ok(line)
}

/// Build a deterministic billing statement from validated line items.
pub fn build_billing_statement_v1(
    account_id: Vec<u8>,
    period_start_unix: u64,
    period_end_unix: u64,
    due_at_unix: u64,
    reference_price: HedgingReferencePriceDecisionV1,
    lines: Vec<BillingLineItemV1>,
    previous_statement_id: Option<[u8; 32]>,
) -> Result<BillingStatementV1, HedgingValidationError> {
    if account_id.is_empty() {
        return Err(HedgingValidationError::EmptyAccountId);
    }
    if period_start_unix == 0 || period_end_unix <= period_start_unix {
        return Err(HedgingValidationError::InvalidPeriod);
    }
    if due_at_unix <= period_end_unix {
        return Err(HedgingValidationError::InvalidDueAt);
    }
    reference_price.validate()?;
    if lines.is_empty() {
        return Err(HedgingValidationError::NoBillingLines);
    }
    validate_optional_digest("previous_statement_id", previous_statement_id)?;
    let totals = BillingTotals::from_lines(&lines, reference_price.xor_usd_micros)?;
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
        total_debit_usd_micros: totals.total_debit_usd_micros,
        total_credit_usd_micros: totals.total_credit_usd_micros,
        net_due_usd_micros: totals.net_due_usd_micros,
        previous_statement_id,
    };
    statement.statement_id = billing_statement_id_v1(&statement)?;
    Ok(statement)
}

/// Convert micro-XOR into USD micro-units using a USD-micro/XOR reference price.
pub fn xor_to_usd_micros(
    amount: XorAmount,
    reference_price_xor_usd_micros: u64,
) -> Result<u128, HedgingValidationError> {
    if reference_price_xor_usd_micros == 0 {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    let numerator = amount
        .as_micro()
        .checked_mul(u128::from(reference_price_xor_usd_micros))
        .ok_or(HedgingValidationError::AmountOverflow)?;
    Ok(numerator.div_ceil(MICRO_XOR_PER_XOR))
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

fn divergence_bps(feed_price: u64, reference_price: u64) -> Result<u16, HedgingValidationError> {
    if reference_price == 0 {
        return Err(HedgingValidationError::ZeroReferencePrice);
    }
    let delta = feed_price.abs_diff(reference_price);
    let bps = (u128::from(delta) * u128::from(HEDGING_BASIS_POINTS)) / u128::from(reference_price);
    bps.try_into()
        .map_err(|_| HedgingValidationError::AmountOverflow)
}

fn hash_norito<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], HedgingValidationError> {
    let bytes = norito::to_bytes(value)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&bytes);
    Ok(hash_to_array(hasher.finalize()))
}

fn hash_to_array(hash: Hash) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

fn validate_text(field: &'static str, value: &str) -> Result<(), HedgingValidationError> {
    if value.trim().is_empty() {
        return Err(HedgingValidationError::InvalidText { field });
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
    let mut seen = BTreeSet::new();
    for reason in reasons {
        validate_text("degradation_reasons", reason)?;
        if !seen.insert(reason) {
            return Err(HedgingValidationError::DuplicateDegradationReason);
        }
    }
    Ok(())
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
        expected: u64,
        /// Stored price.
        actual: u64,
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
    /// Statement has no billing lines.
    #[error("statement must contain at least one billing line")]
    NoBillingLines,
    /// Duplicate billing line id was supplied.
    #[error("duplicate billing line id")]
    DuplicateLineId,
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
        /// Replayed USD micro-units.
        expected: u128,
        /// Stored USD micro-units.
        actual: u128,
    },
    /// Amount arithmetic overflowed.
    #[error("amount overflow")]
    AmountOverflow,
    /// Amount arithmetic underflowed.
    #[error("amount underflow")]
    AmountUnderflow,
    /// Digest binding did not replay.
    #[error("{field} digest does not match canonical payload")]
    DigestMismatch {
        /// Field name.
        field: &'static str,
    },
    /// Norito serialization failed.
    #[error("norito serialization failed: {0}")]
    Norito(#[from] NoritoError),
}

impl From<DealAmountError> for HedgingValidationError {
    fn from(error: DealAmountError) -> Self {
        match error {
            DealAmountError::Overflow => Self::AmountOverflow,
            DealAmountError::Underflow => Self::AmountUnderflow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> [u8; 32] {
        hash_to_array(blake3::hash(label.as_bytes()))
    }

    fn feed(feed_id: &str, price: u64, observed_at_unix: u64) -> HedgingPriceFeedV1 {
        HedgingPriceFeedV1 {
            version: HEDGING_PRICE_FEED_VERSION_V1,
            feed_id: feed_id.into(),
            source: format!("{feed_id}-source"),
            observed_at_unix,
            xor_usd_micros: price,
            weight_bps: 5_000,
            evidence_digest: digest(feed_id),
            status: HedgingFeedStatusV1::Ok,
        }
    }

    #[test]
    fn reference_price_decision_is_deterministic_and_flags_divergence() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![
                feed("secondary", 1_000_000, 1_760),
                feed("primary", 1_200_000, 1_770),
            ],
            120,
            500,
        )
        .expect("decision");

        assert_eq!(decision.xor_usd_micros, 1_100_000);
        assert!(decision.degraded);
        assert_eq!(decision.degradation_reasons.len(), 2);
        decision.validate().expect("valid decision");
        assert_eq!(
            decision.decision_id,
            reference_price_decision_id_v1(&decision).unwrap()
        );
    }

    #[test]
    fn stale_feed_is_rejected_before_decision() {
        let err = derive_reference_price_decision_v1(
            1_800,
            vec![feed("primary", 1_000_000, 1_000)],
            120,
            500,
        )
        .expect_err("stale feed");

        assert!(matches!(err, HedgingValidationError::StaleFeed { .. }));
    }

    #[test]
    fn billing_line_item_converts_xor_with_ceil_rounding() {
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "deal-1",
            XorAmount::from_micro(MICRO_XOR_PER_XOR + 1),
            2_000_000,
            3_600,
            None,
        )
        .expect("line");

        assert_eq!(line.usd_micros, 2_000_002);
        line.validate().expect("valid line");
    }

    #[test]
    fn billing_statement_totals_and_roundtrip_are_deterministic() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![
                feed("primary", 2_000_000, 1_790),
                feed("secondary", 2_000_000, 1_785),
            ],
            120,
            500,
        )
        .expect("decision");
        let storage = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "deal-storage",
            XorAmount::from_micro(10 * MICRO_XOR_PER_XOR),
            decision.xor_usd_micros,
            86_400,
            Some("weekly storage".into()),
        )
        .expect("storage line");
        let credit = build_billing_line_item_v1(
            BillingLineItemKindV1::IncentiveCredit,
            BillingLineDirectionV1::Credit,
            "incentive-1",
            XorAmount::from_micro(2 * MICRO_XOR_PER_XOR),
            decision.xor_usd_micros,
            1,
            None,
        )
        .expect("credit line");
        let statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_700_000_000,
            1_700_604_800,
            1_700_691_200,
            decision,
            vec![storage, credit],
            Some(digest("previous")),
        )
        .expect("statement");

        assert_eq!(statement.total_debit_xor.as_micro(), 10 * MICRO_XOR_PER_XOR);
        assert_eq!(statement.total_credit_xor.as_micro(), 2 * MICRO_XOR_PER_XOR);
        assert_eq!(statement.net_due_xor.as_micro(), 8 * MICRO_XOR_PER_XOR);
        assert_eq!(statement.total_debit_usd_micros, 20_000_000);
        assert_eq!(statement.total_credit_usd_micros, 4_000_000);
        assert_eq!(statement.net_due_usd_micros, 16_000_000);
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
            vec![feed("primary", 2_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Egress,
            BillingLineDirectionV1::Debit,
            "egress-1",
            XorAmount::from_micro(MICRO_XOR_PER_XOR),
            decision.xor_usd_micros,
            1024,
            None,
        )
        .expect("line");
        let mut statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_700_000_000,
            1_700_604_800,
            1_700_691_200,
            decision,
            vec![line],
            None,
        )
        .expect("statement");
        statement.total_debit_usd_micros += 1;

        let err = statement.validate().expect_err("tampered totals");
        assert!(matches!(err, HedgingValidationError::BillingTotalsMismatch));
    }

    #[test]
    fn billing_statement_rejects_line_usd_mismatch() {
        let decision = derive_reference_price_decision_v1(
            1_800,
            vec![feed("primary", 2_000_000, 1_790)],
            120,
            500,
        )
        .expect("decision");
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Egress,
            BillingLineDirectionV1::Debit,
            "egress-1",
            XorAmount::from_micro(MICRO_XOR_PER_XOR),
            decision.xor_usd_micros,
            1024,
            None,
        )
        .expect("line");
        let mut statement = build_billing_statement_v1(
            b"alice".to_vec(),
            1_700_000_000,
            1_700_604_800,
            1_700_691_200,
            decision,
            vec![line],
            None,
        )
        .expect("statement");
        statement.lines[0].usd_micros += 1;
        statement.lines[0].line_id =
            billing_line_item_id_v1(&statement.lines[0]).expect("rebind tampered line");
        statement.total_debit_usd_micros += 1;
        statement.net_due_usd_micros += 1;
        statement.statement_id = billing_statement_id_v1(&statement).expect("rebind statement");

        let err = statement.validate().expect_err("tampered line USD");
        assert!(matches!(
            err,
            HedgingValidationError::BillingLineUsdMismatch { .. }
        ));
    }
}
