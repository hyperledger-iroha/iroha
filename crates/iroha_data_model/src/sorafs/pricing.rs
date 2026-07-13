//! Pricing schedule and credit policy records for SoraFS (SF-8a).
//!
//! These types describe the governance-controlled pricing surface for storage
//! providers together with the collateral and credit settlement policies used by
//! the deal engine. The schedule is stored on-ledger so governance proposals
//! can update pricing deterministically without relying on out-of-band config.
//! Public pin admission fees are computed here, while provider credit deposits,
//! settlement, and slashing remain separate ledger flows handled by the deal
//! engine.

use std::collections::BTreeSet;

use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity, RoundingMode};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    metadata::Metadata,
    sorafs::{capacity::ProviderId, deal as sorafs_deal, pin_registry::StorageClass},
};

/// First-version schema identifier for [`PricingScheduleRecord`].
pub const PRICING_SCHEDULE_VERSION_V1: u16 = 1;
/// Seconds used for billing a "month" when converting average utilisation to GiB·month.
pub const SECONDS_PER_BILLING_MONTH: u64 = 30 * 24 * 60 * 60;
/// Seconds per week, used for default settlement windows.
pub const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60;
/// Ledger precision used by XOR-denominated SoraFS economic records.
pub const XOR_QUANTITY_SCALE: u32 = 9;

/// Pricing for a single storage class (GiB-month + egress).
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Ord, PartialOrd, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TierRate {
    /// Storage class the tier applies to.
    pub storage_class: StorageClass,
    /// Nominal price per GiB·month.
    pub storage_price_per_gib_month: Quantity,
    /// Nominal price per GiB of egress.
    pub egress_price_per_gib: Quantity,
}

impl TierRate {
    /// Construct a tier rate.
    #[must_use]
    pub fn new(
        storage_class: StorageClass,
        storage_price_per_gib_month: Quantity,
        egress_price_per_gib: Quantity,
    ) -> Self {
        Self {
            storage_class,
            storage_price_per_gib_month,
            egress_price_per_gib,
        }
    }
}

/// Collateral policy controlling minimum bonded amounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CollateralPolicy {
    /// Multiplier (in basis points) applied to monthly storage revenue.
    pub multiplier_bps: u32,
    /// Discount (in basis points) while the onboarding period is active.
    pub onboarding_discount_bps: u32,
    /// Duration (seconds) of the onboarding period where the discount applies.
    pub onboarding_period_secs: u64,
}

impl CollateralPolicy {
    /// Compute the discount multiplier (basis points) that should be applied to the
    /// collateral requirement at `now_epoch`, given the onboarding start epoch.
    fn discount_multiplier_bps(&self, onboarding_epoch: u64, now_epoch: u64) -> u32 {
        if now_epoch.saturating_sub(onboarding_epoch) < self.onboarding_period_secs {
            self.onboarding_discount_bps.min(10_000)
        } else {
            10_000
        }
    }
}

impl Default for CollateralPolicy {
    fn default() -> Self {
        Self {
            multiplier_bps: 30_000,                    // 3× monthly storage earnings
            onboarding_discount_bps: 5_000,            // 50% collateral during onboarding
            onboarding_period_secs: 30 * 24 * 60 * 60, // 30 days
        }
    }
}

/// Credit settlement configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CreditPolicy {
    /// Length of a settlement window (seconds).
    pub settlement_window_secs: u64,
    /// Additional grace period (seconds) after the settlement deadline.
    pub settlement_grace_secs: u64,
    /// Threshold (basis points of expected settlement charge) that triggers low-credit alerts.
    pub low_balance_alert_bps: u16,
}

impl CreditPolicy {
    /// Returns the total duration (settlement + grace).
    #[must_use]
    pub fn window_with_grace_secs(&self) -> u64 {
        self.settlement_window_secs
            .saturating_add(self.settlement_grace_secs)
    }
}

impl Default for CreditPolicy {
    fn default() -> Self {
        Self {
            settlement_window_secs: SECONDS_PER_WEEK,
            settlement_grace_secs: 2 * 24 * 60 * 60, // 2 days
            low_balance_alert_bps: 2_000,            // alert at <20% of expected spend
        }
    }
}

/// Commitment-based discount tier (e.g., loyalty or capacity commitment).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CommitmentDiscountTier {
    /// Minimum committed GiB·month required for the discount.
    pub minimum_commitment_gib_month: u64,
    /// Discount applied when commitment >= threshold (basis points).
    pub discount_bps: u16,
}

/// Discount schedule applied on top of base tier pricing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DiscountSchedule {
    /// Months of uninterrupted participation required for loyalty discount.
    pub loyalty_months_required: u16,
    /// Loyalty discount applied after the requirement is met (basis points).
    pub loyalty_discount_bps: u16,
    /// Additional commitment-based discount tiers.
    pub commitment_tiers: Vec<CommitmentDiscountTier>,
}

/// Governance-controlled pricing schedule and credit policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PricingScheduleRecord {
    /// Schema version (see [`PRICING_SCHEDULE_VERSION_V1`]).
    pub version: u16,
    /// Three-letter lowercase currency code (currently `xor` only).
    pub currency_code: String,
    /// Default storage class used when utilisation telemetry omits class hints.
    pub default_storage_class: StorageClass,
    /// Tier pricing by storage class.
    pub tiers: Vec<TierRate>,
    /// Collateral policy applied when calculating required bonds.
    pub collateral: CollateralPolicy,
    /// Credit settlement policy (window + alerts).
    pub credit: CreditPolicy,
    /// Optional discount schedule (commitment / loyalty).
    pub discounts: DiscountSchedule,
    /// Optional governance notes embedded with the schedule.
    #[cfg_attr(feature = "json", norito(default))]
    pub notes: Option<String>,
}

impl PricingScheduleRecord {
    /// Default launch schedule matching the SF-8a specification draft.
    #[must_use]
    pub fn launch_default() -> Self {
        let quantity_nanos = |value| {
            Quantity::from_canonical_numeric(Numeric::new(value, 9))
                .expect("launch nano-XOR tariff fits Quantity")
        };
        let tiers = vec![
            TierRate::new(
                StorageClass::Hot,
                quantity_nanos(500_000_000_u128),
                quantity_nanos(50_000_000_u128),
            ),
            TierRate::new(
                StorageClass::Warm,
                quantity_nanos(200_000_000_u128),
                quantity_nanos(20_000_000_u128),
            ),
            TierRate::new(
                StorageClass::Cold,
                quantity_nanos(50_000_000_u128),
                quantity_nanos(10_000_000_u128),
            ),
        ];
        let collateral = CollateralPolicy::default();
        let credit = CreditPolicy::default();
        let discounts = DiscountSchedule {
            loyalty_months_required: 12,
            loyalty_discount_bps: 1_000, // 10%
            commitment_tiers: vec![
                CommitmentDiscountTier {
                    minimum_commitment_gib_month: 500,
                    discount_bps: 500,
                },
                CommitmentDiscountTier {
                    minimum_commitment_gib_month: 2_000,
                    discount_bps: 1_500,
                },
            ],
        };
        let schedule = Self {
            version: PRICING_SCHEDULE_VERSION_V1,
            currency_code: "xor".to_string(),
            default_storage_class: StorageClass::Hot,
            tiers,
            collateral,
            credit,
            discounts,
            notes: Some(
                "Launch pricing schedule (0.50/0.20/0.05 XOR GiB·month; egress 0.05/0.02/0.01 XOR)"
                    .to_string(),
            ),
        };
        schedule
    }

    /// Lookup tier rates for a storage class, falling back to the schedule default.
    #[must_use]
    pub fn tier_rate(&self, class: StorageClass) -> &TierRate {
        self.tiers
            .iter()
            .find(|tier| tier.storage_class == class)
            .or_else(|| {
                self.tiers
                    .iter()
                    .find(|tier| tier.storage_class == self.default_storage_class)
            })
            .expect("pricing schedule must include default storage class tier")
    }

    /// Validate invariants for the pricing schedule.
    ///
    /// # Errors
    ///
    /// Returns [`PricingValidationError`] when the schedule violates currency or tier constraints.
    pub fn validate(&self) -> Result<(), PricingValidationError> {
        if self.version != PRICING_SCHEDULE_VERSION_V1 {
            return Err(PricingValidationError::UnsupportedVersion(self.version));
        }
        if self.currency_code.trim().len() != 3
            || !self
                .currency_code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(PricingValidationError::InvalidCurrencyCode(
                self.currency_code.clone(),
            ));
        }
        if self.tiers.is_empty() {
            return Err(PricingValidationError::MissingTiers);
        }
        let mut seen = BTreeSet::new();
        for tier in &self.tiers {
            if tier.storage_price_per_gib_month.is_zero() {
                return Err(PricingValidationError::ZeroStoragePrice(tier.storage_class));
            }
            if tier.egress_price_per_gib.is_zero() {
                return Err(PricingValidationError::ZeroEgressPrice(tier.storage_class));
            }
            if !seen.insert(tier.storage_class) {
                return Err(PricingValidationError::DuplicateTier(tier.storage_class));
            }
        }
        if !seen.contains(&self.default_storage_class) {
            return Err(PricingValidationError::MissingDefaultTier(
                self.default_storage_class,
            ));
        }
        if self.collateral.multiplier_bps == 0 {
            return Err(PricingValidationError::InvalidCollateralMultiplier);
        }
        if self.collateral.onboarding_discount_bps == 0 {
            return Err(PricingValidationError::InvalidOnboardingDiscount);
        }
        if self.credit.settlement_window_secs == 0 {
            return Err(PricingValidationError::InvalidSettlementWindow);
        }
        if self.credit.low_balance_alert_bps == 0 || self.credit.low_balance_alert_bps > 10_000 {
            return Err(PricingValidationError::InvalidLowBalanceThreshold(
                self.credit.low_balance_alert_bps,
            ));
        }
        Ok(())
    }

    /// Compute nominal storage charges for a telemetry window.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn storage_charge(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
        window_secs: u64,
    ) -> Result<Quantity, NumericOperationError> {
        if window_secs == 0 || avg_utilised_gib == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate(class);
        let gib_seconds = u128::from(avg_utilised_gib) * u128::from(window_secs);
        multiply_ratio(
            &tier.storage_price_per_gib_month,
            gib_seconds,
            u128::from(SECONDS_PER_BILLING_MONTH),
            RoundingMode::NearestAway,
        )
    }

    /// Compute the prepaid storage fee for admitting a public pin.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn public_pin_fee(
        &self,
        class: StorageClass,
        content_length_bytes: u64,
        min_replicas: u16,
        submitted_epoch: u64,
        retention_epoch: u64,
    ) -> Result<Quantity, NumericOperationError> {
        if content_length_bytes == 0 || min_replicas == 0 {
            return Ok(Quantity::zero());
        }
        let bytes_per_gib = sorafs_deal::BYTES_PER_GIB;
        let gib = (u128::from(content_length_bytes) + bytes_per_gib - 1) / bytes_per_gib;
        let replicated_gib = gib * u128::from(min_replicas);
        let requested_window = retention_epoch.saturating_sub(submitted_epoch);
        let billing_window = requested_window.max(self.credit.settlement_window_secs);
        let tier = self.tier_rate(class);
        multiply_ratio(
            &tier.storage_price_per_gib_month,
            replicated_gib * u128::from(billing_window),
            u128::from(SECONDS_PER_BILLING_MONTH),
            RoundingMode::NearestAway,
        )
    }

    /// Compute egress charges for `egress_gib` volume.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn egress_charge(
        &self,
        class: StorageClass,
        egress_gib: u64,
    ) -> Result<Quantity, NumericOperationError> {
        if egress_gib == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate(class);
        tier.egress_price_per_gib
            .try_mul_decimal(&Numeric::from(egress_gib))
    }

    /// Compute egress charges for `egress_bytes` volume.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn egress_charge_bytes(
        &self,
        class: StorageClass,
        egress_bytes: u64,
    ) -> Result<Quantity, NumericOperationError> {
        if egress_bytes == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate(class);
        multiply_ratio(
            &tier.egress_price_per_gib,
            u128::from(egress_bytes),
            sorafs_deal::BYTES_PER_GIB,
            RoundingMode::TowardZero,
        )
    }

    /// Expected storage charge for one settlement window at the current utilisation.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn expected_settlement_storage_charge(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
    ) -> Result<Quantity, NumericOperationError> {
        self.storage_charge(class, avg_utilised_gib, self.credit.settlement_window_secs)
    }

    /// Required nominal bonded collateral for the given utilisation.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn required_collateral(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
        onboarding_epoch: u64,
        now_epoch: u64,
    ) -> Result<Quantity, NumericOperationError> {
        let monthly_charge =
            self.storage_charge(class, avg_utilised_gib, SECONDS_PER_BILLING_MONTH)?;
        let base = multiply_ratio(
            &monthly_charge,
            u128::from(self.collateral.multiplier_bps),
            10_000,
            RoundingMode::NearestAway,
        )?;
        let discount_bps = self
            .collateral
            .discount_multiplier_bps(onboarding_epoch, now_epoch)
            .max(1);
        multiply_ratio(
            &base,
            u128::from(discount_bps),
            10_000,
            RoundingMode::NearestAway,
        )
    }

    /// Low-balance alert threshold derived from the expected settlement charge.
    ///
    /// # Errors
    /// Returns a bounded-decimal arithmetic error.
    pub fn low_balance_threshold(
        &self,
        expected_settlement_charge: &Quantity,
    ) -> Result<Quantity, NumericOperationError> {
        multiply_ratio(
            expected_settlement_charge,
            u128::from(self.credit.low_balance_alert_bps),
            10_000,
            RoundingMode::NearestAway,
        )
    }
}

impl Default for PricingScheduleRecord {
    fn default() -> Self {
        Self::launch_default()
    }
}

/// Validation error surfaced when verifying a [`PricingScheduleRecord`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PricingValidationError {
    /// Unsupported version number.
    #[error("unsupported pricing schedule version {0}")]
    UnsupportedVersion(u16),
    /// Invalid or unsupported currency code.
    #[error("invalid pricing currency code `{0}`")]
    InvalidCurrencyCode(String),
    /// Pricing tiers must be provided.
    #[error("pricing schedule must include at least one tier")]
    MissingTiers,
    /// Duplicate tier definitions detected.
    #[error("duplicate pricing tier for storage class {0:?}")]
    DuplicateTier(StorageClass),
    /// Missing tier for the configured default storage class.
    #[error("default storage class tier {0:?} not present in pricing schedule")]
    MissingDefaultTier(StorageClass),
    /// Storage price must be non-zero.
    #[error("storage price may not be zero for tier {0:?}")]
    ZeroStoragePrice(StorageClass),
    /// Egress price must be non-zero.
    #[error("egress price may not be zero for tier {0:?}")]
    ZeroEgressPrice(StorageClass),
    /// Invalid collateral multiplier.
    #[error("collateral multiplier must be non-zero")]
    InvalidCollateralMultiplier,
    /// Invalid onboarding discount (must be non-zero).
    #[error("onboarding collateral discount must be non-zero")]
    InvalidOnboardingDiscount,
    /// Settlement window must be non-zero.
    #[error("settlement window must be non-zero")]
    InvalidSettlementWindow,
    /// Invalid low-balance alert threshold.
    #[error("low-balance alert threshold must be within 1..=10000 basis points (found {0})")]
    InvalidLowBalanceThreshold(u16),
}

/// Credit ledger record persisted for each provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProviderCreditRecord {
    /// Provider identifier this credit entry belongs to.
    pub provider_id: ProviderId,
    /// Available nominal credit after accounting for pending charges.
    pub available_credit: Quantity,
    /// Collateral currently bonded for the provider.
    pub bonded: Quantity,
    /// Required collateral computed during the last telemetry window.
    pub required_bond: Quantity,
    /// Expected settlement charge (storage + egress) for the next window.
    pub expected_settlement: Quantity,
    /// Epoch (seconds) when the onboarding period started.
    pub onboarding_epoch: u64,
    /// Epoch (seconds) when the last settlement completed.
    pub last_settlement_epoch: u64,
    /// Epoch when the credit balance last fell below the alert threshold (if any).
    pub low_balance_since_epoch: Option<u64>,
    /// Total nominal collateral slashed because of under-delivery.
    #[cfg_attr(feature = "json", norito(default))]
    pub slashed: Quantity,
    /// Consecutive under-delivery strike counter.
    #[cfg_attr(feature = "json", norito(default))]
    pub under_delivery_strikes: u32,
    /// Epoch (seconds) when the last penalty was applied.
    #[cfg_attr(feature = "json", norito(default))]
    pub last_penalty_epoch: Option<u64>,
    /// Optional metadata annotations.
    pub metadata: Metadata,
}

impl ProviderCreditRecord {
    /// Create a new provider credit record.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        available_credit: Quantity,
        bonded: Quantity,
        required_bond: Quantity,
        expected_settlement: Quantity,
        onboarding_epoch: u64,
        last_settlement_epoch: u64,
        metadata: Metadata,
    ) -> Self {
        Self {
            provider_id,
            available_credit,
            bonded,
            required_bond,
            expected_settlement,
            onboarding_epoch,
            last_settlement_epoch,
            low_balance_since_epoch: None,
            slashed: Quantity::zero(),
            under_delivery_strikes: 0,
            last_penalty_epoch: None,
            metadata,
        }
    }

    /// Apply a debit (charge) against the available credit.
    ///
    /// The debit is explicitly capped at the available balance. This retains
    /// the ledger's no-debt policy without relying on saturating integer math.
    ///
    /// # Errors
    /// Returns a bounded-domain error without mutating the record.
    pub fn apply_charge(
        &mut self,
        debit: &Quantity,
        epoch: u64,
    ) -> Result<(), NumericOperationError> {
        let debit = if debit > &self.available_credit {
            self.available_credit.clone()
        } else {
            debit.clone()
        };
        let available_credit = self.available_credit.checked_sub(&debit)?;
        self.available_credit = available_credit;
        self.last_settlement_epoch = epoch;
        Ok(())
    }

    /// Update low-balance tracking depending on whether the threshold is crossed.
    pub fn track_low_balance(&mut self, threshold: &Quantity, epoch: u64) {
        if &self.available_credit <= threshold {
            if self.low_balance_since_epoch.is_none() {
                self.low_balance_since_epoch = Some(epoch);
            }
        } else {
            self.low_balance_since_epoch = None;
        }
    }

    /// Record an under-delivery strike.
    pub fn add_strike(&mut self) {
        self.under_delivery_strikes = self.under_delivery_strikes.saturating_add(1);
    }

    /// Clear consecutive strike tracking.
    pub fn reset_strikes(&mut self) {
        self.under_delivery_strikes = 0;
    }

    /// Apply a penalty to the bonded collateral and track totals.
    ///
    /// The penalty is explicitly capped at the bonded balance.
    ///
    /// # Errors
    /// Returns a bounded-domain error without mutating the record.
    pub fn apply_penalty(
        &mut self,
        penalty: &Quantity,
        epoch: u64,
    ) -> Result<(), NumericOperationError> {
        if penalty.is_zero() {
            return Ok(());
        }
        let penalty = if penalty > &self.bonded {
            self.bonded.clone()
        } else {
            penalty.clone()
        };
        let bonded = self.bonded.checked_sub(&penalty)?;
        let slashed = self.slashed.checked_add(&penalty)?;
        self.bonded = bonded;
        self.slashed = slashed;
        self.last_penalty_epoch = Some(epoch);
        self.reset_strikes();
        Ok(())
    }
}

fn multiply_ratio(
    value: &Quantity,
    multiplier: u128,
    divisor: u128,
    mode: RoundingMode,
) -> Result<Quantity, NumericOperationError> {
    value.try_mul_div_decimal_round(
        &Numeric::new(multiplier, 0),
        &Numeric::new(divisor, 0),
        XOR_QUANTITY_SCALE,
        mode,
    )
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use super::*;

    fn quantity_nanos(value: u128) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, XOR_QUANTITY_SCALE))
            .expect("u128 nano-XOR fixture fits Quantity")
    }

    fn maximum_quantity() -> Quantity {
        "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("signed 512-bit maximum quantity")
    }

    #[derive(Encode)]
    struct ForgedTierRate {
        storage_class: StorageClass,
        storage_price_per_gib_month: Numeric,
        egress_price_per_gib: Quantity,
    }

    #[test]
    fn ratio_helper_bounds_only_the_final_result() {
        let maximum = maximum_quantity();
        assert_eq!(
            multiply_ratio(&maximum, u128::MAX, u128::MAX, RoundingMode::NearestEven,),
            Ok(maximum)
        );
    }

    #[test]
    fn default_schedule_validates() {
        let schedule = PricingScheduleRecord::launch_default();
        assert!(schedule.validate().is_ok());
    }

    #[test]
    fn storage_charge_scales_with_duration() {
        let schedule = PricingScheduleRecord::launch_default();
        let charge_week = schedule
            .storage_charge(StorageClass::Hot, 100, SECONDS_PER_WEEK)
            .expect("bounded weekly charge");
        let charge_month = schedule
            .storage_charge(StorageClass::Hot, 100, SECONDS_PER_BILLING_MONTH)
            .expect("bounded monthly charge");
        assert!(charge_month > charge_week);
        assert_eq!(charge_month, quantity_nanos(50_000_000_000));
    }

    #[test]
    fn egress_charge_scales_with_bytes() {
        let schedule = PricingScheduleRecord::launch_default();
        let bytes_per_gib =
            u64::try_from(sorafs_deal::BYTES_PER_GIB).expect("BYTES_PER_GIB fits within u64");
        let per_gib = schedule
            .egress_charge_bytes(StorageClass::Hot, bytes_per_gib)
            .expect("bounded egress charge");
        assert_eq!(
            per_gib,
            schedule
                .tier_rate(StorageClass::Hot)
                .egress_price_per_gib
                .clone()
        );

        let half_bytes =
            u64::try_from(sorafs_deal::BYTES_PER_GIB / 2).expect("half GiB fits within u64");
        let half = schedule
            .egress_charge_bytes(StorageClass::Hot, half_bytes)
            .expect("bounded half-GiB charge");
        assert!(!half.is_zero());
        assert_eq!(
            half.try_mul_decimal(&Numeric::from(2_u32))
                .expect("bounded doubled charge"),
            per_gib
        );
    }

    #[test]
    fn collateral_discount_applies_during_onboarding() {
        let schedule = PricingScheduleRecord::launch_default();
        let requirement_no_discount = schedule
            .required_collateral(StorageClass::Hot, 256, 0, 90 * 24 * 60 * 60)
            .expect("bounded collateral");
        let requirement_discount = schedule
            .required_collateral(StorageClass::Hot, 256, 0, 10 * 24 * 60 * 60)
            .expect("bounded discounted collateral");
        assert!(requirement_discount < requirement_no_discount);
    }

    #[test]
    fn provider_credit_low_balance_tracking() {
        let mut credit = ProviderCreditRecord::new(
            ProviderId::default(),
            quantity_nanos(1_000),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        credit.track_low_balance(&quantity_nanos(2_000), 10);
        assert_eq!(credit.low_balance_since_epoch, Some(10));
        credit.available_credit = quantity_nanos(5_000);
        credit.track_low_balance(&quantity_nanos(2_000), 20);
        assert_eq!(credit.low_balance_since_epoch, None);
    }

    #[test]
    fn tier_rate_rejects_forged_negative_price() {
        let forged = ForgedTierRate {
            storage_class: StorageClass::Hot,
            storage_price_per_gib_month: Numeric::new(-1_i32, 0),
            egress_price_per_gib: quantity_nanos(1),
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            <TierRate as Decode>::decode(&mut input).is_err(),
            "pricing tier must reject a forged negative storage price"
        );
    }

    #[test]
    fn provider_credit_debits_and_penalties_are_explicitly_capped() {
        let mut credit = ProviderCreditRecord::new(
            ProviderId::default(),
            quantity_nanos(100),
            quantity_nanos(80),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        credit
            .apply_charge(&quantity_nanos(150), 1)
            .expect("capped debit");
        credit
            .apply_penalty(&quantity_nanos(100), 2)
            .expect("capped penalty");
        assert_eq!(credit.available_credit, Quantity::zero());
        assert_eq!(credit.bonded, Quantity::zero());
        assert_eq!(credit.slashed, quantity_nanos(80));
    }
}
