//! Pricing schedule and credit policy records for SoraFS (SF-8a).
//!
//! These types describe the governance-controlled pricing surface for storage
//! providers together with the collateral and credit settlement policies used by
//! native orderbook, reserve/rent, and billing services. The schedule is stored
//! on-ledger so governance proposals can update pricing deterministically without
//! relying on out-of-band config.
//! Public pin admission fees are computed here, while provider credit deposits,
//! settlement, and slashing remain separate authority-checked ledger flows.

use std::collections::BTreeSet;

use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity, RoundingMode};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    metadata::Metadata,
    sorafs::{capacity::ProviderId, pin_registry::StorageClass},
};

/// First-version schema identifier for [`PricingScheduleRecord`].
pub const PRICING_SCHEDULE_VERSION_V1: u16 = 1;
/// Seconds used for billing a "month" when converting average utilisation to GiB·month.
pub const SECONDS_PER_BILLING_MONTH: u64 = 30 * 24 * 60 * 60;
/// Seconds per week, used for default settlement windows.
pub const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60;
/// Ledger precision used by XOR-denominated `SoraFS` economic records.
pub const XOR_QUANTITY_SCALE: u32 = 9;
/// Canonical orderbook byte count per gibibyte, widened for exact fee arithmetic.
const BYTES_PER_GIB: u128 = sorafs_manifest::orderbook::BYTES_PER_GIB as u128;
/// Maximum commitment-discount tiers accepted in one governance schedule.
pub const MAX_COMMITMENT_DISCOUNT_TIERS: usize = 64;
/// Maximum UTF-8 byte length of governance pricing notes.
pub const MAX_PRICING_NOTES_BYTES: usize = 4_096;

const STORAGE_CLASSES: [StorageClass; 3] =
    [StorageClass::Hot, StorageClass::Warm, StorageClass::Cold];

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
    fn discount_multiplier_bps(
        &self,
        onboarding_epoch: u64,
        now_epoch: u64,
    ) -> Result<u32, PricingComputationError> {
        let elapsed = now_epoch.checked_sub(onboarding_epoch).ok_or(
            PricingComputationError::EpochBeforeOnboarding {
                onboarding_epoch,
                now_epoch,
            },
        )?;
        if elapsed < self.onboarding_period_secs {
            Ok(self.onboarding_discount_bps)
        } else {
            Ok(10_000)
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
    ///
    /// # Errors
    ///
    /// Returns [`PricingComputationError::ArithmeticOverflow`] when the two
    /// configured durations do not fit in a `u64`.
    pub fn window_with_grace_secs(&self) -> Result<u64, PricingComputationError> {
        self.settlement_window_secs
            .checked_add(self.settlement_grace_secs)
            .ok_or(PricingComputationError::ArithmeticOverflow(
                "settlement window plus grace",
            ))
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
    /// Default launch schedule for the canonical SF-8a pricing policy.
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

        Self {
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
        }
    }

    /// Lookup the exact tier rate for a storage class.
    ///
    /// # Errors
    ///
    /// Returns [`PricingComputationError`] when the schedule is invalid or the
    /// requested class is missing. First-release pricing never silently falls
    /// back to another storage class.
    pub fn tier_rate(&self, class: StorageClass) -> Result<&TierRate, PricingComputationError> {
        self.validate()?;
        self.tier_rate_validated(class)
    }

    fn tier_rate_validated(
        &self,
        class: StorageClass,
    ) -> Result<&TierRate, PricingComputationError> {
        self.tiers
            .iter()
            .find(|tier| tier.storage_class == class)
            .ok_or(PricingComputationError::MissingTier(class))
    }

    /// Validate invariants for the pricing schedule.
    ///
    /// # Errors
    ///
    /// Returns [`PricingValidationError`] when the schedule violates currency or tier constraints.
    #[expect(
        clippy::too_many_lines,
        reason = "one fail-closed validator keeps all first-release pricing invariants together"
    )]
    pub fn validate(&self) -> Result<(), PricingValidationError> {
        if self.version != PRICING_SCHEDULE_VERSION_V1 {
            return Err(PricingValidationError::UnsupportedVersion(self.version));
        }
        if self.currency_code != "xor" {
            return Err(PricingValidationError::InvalidCurrencyCode(
                self.currency_code.clone(),
            ));
        }
        if self.tiers.len() != STORAGE_CLASSES.len() {
            return Err(PricingValidationError::InvalidTierCount {
                found: self.tiers.len(),
                expected: STORAGE_CLASSES.len(),
            });
        }
        let mut seen = BTreeSet::new();
        for (index, (tier, expected_class)) in self.tiers.iter().zip(STORAGE_CLASSES).enumerate() {
            if tier.storage_class != expected_class {
                return Err(PricingValidationError::NonCanonicalTierOrder {
                    index,
                    expected: expected_class,
                    found: tier.storage_class,
                });
            }
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
        if self.collateral.onboarding_discount_bps > 10_000 {
            return Err(PricingValidationError::OnboardingDiscountOutOfRange(
                self.collateral.onboarding_discount_bps,
            ));
        }
        if self.collateral.onboarding_period_secs == 0 {
            return Err(PricingValidationError::InvalidOnboardingPeriod);
        }
        if self.credit.settlement_window_secs == 0 {
            return Err(PricingValidationError::InvalidSettlementWindow);
        }
        self.credit
            .window_with_grace_secs()
            .map_err(|_| PricingValidationError::SettlementWindowOverflow)?;
        if self.credit.low_balance_alert_bps == 0 || self.credit.low_balance_alert_bps > 10_000 {
            return Err(PricingValidationError::InvalidLowBalanceThreshold(
                self.credit.low_balance_alert_bps,
            ));
        }
        if self.discounts.loyalty_discount_bps > 10_000 {
            return Err(PricingValidationError::InvalidLoyaltyDiscount(
                self.discounts.loyalty_discount_bps,
            ));
        }
        if self.discounts.loyalty_discount_bps > 0 && self.discounts.loyalty_months_required == 0 {
            return Err(PricingValidationError::InvalidLoyaltyPeriod);
        }
        if self.discounts.commitment_tiers.len() > MAX_COMMITMENT_DISCOUNT_TIERS {
            return Err(PricingValidationError::TooManyCommitmentDiscountTiers {
                found: self.discounts.commitment_tiers.len(),
                maximum: MAX_COMMITMENT_DISCOUNT_TIERS,
            });
        }
        let mut previous_commitment = 0u64;
        let mut previous_discount = 0u16;
        for (index, tier) in self.discounts.commitment_tiers.iter().enumerate() {
            if tier.minimum_commitment_gib_month == 0
                || tier.minimum_commitment_gib_month <= previous_commitment
            {
                return Err(PricingValidationError::NonCanonicalCommitmentTierOrder { index });
            }
            if tier.discount_bps == 0
                || tier.discount_bps > 10_000
                || tier.discount_bps < previous_discount
            {
                return Err(PricingValidationError::InvalidCommitmentDiscount {
                    index,
                    discount_bps: tier.discount_bps,
                });
            }
            previous_commitment = tier.minimum_commitment_gib_month;
            previous_discount = tier.discount_bps;
        }
        if u32::from(self.discounts.loyalty_discount_bps)
            .checked_add(u32::from(previous_discount))
            .is_none_or(|combined| combined > 10_000)
        {
            return Err(PricingValidationError::CombinedDiscountExceedsFullPrice);
        }
        if let Some(notes) = &self.notes
            && (notes.is_empty()
                || notes != notes.trim()
                || notes.len() > MAX_PRICING_NOTES_BYTES
                || notes.chars().any(char::is_control))
        {
            return Err(PricingValidationError::InvalidNotes {
                found: notes.len(),
                maximum: MAX_PRICING_NOTES_BYTES,
            });
        }
        Ok(())
    }

    /// Compute nominal storage charges for a telemetry window.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when the schedule is invalid, its
    /// tier is missing, or bounded-decimal arithmetic fails.
    pub fn storage_charge(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
        window_secs: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        self.storage_charge_validated(class, u128::from(avg_utilised_gib), window_secs)
    }

    fn storage_charge_validated(
        &self,
        class: StorageClass,
        avg_utilised_gib: u128,
        window_secs: u64,
    ) -> Result<Quantity, PricingComputationError> {
        if window_secs == 0 || avg_utilised_gib == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate_validated(class)?;
        let gib_seconds = avg_utilised_gib
            .checked_mul(u128::from(window_secs))
            .ok_or(PricingComputationError::ArithmeticOverflow(
                "storage GiB-seconds",
            ))?;
        Ok(multiply_ratio(
            &tier.storage_price_per_gib_month,
            gib_seconds,
            u128::from(SECONDS_PER_BILLING_MONTH),
            RoundingMode::NearestAway,
        )?)
    }

    /// Compute the prepaid storage fee for admitting a public pin.
    ///
    /// Empty payloads are charged as one GiB because even an empty collection
    /// consumes manifest, indexing, and replica capacity.
    ///
    /// # Errors
    ///
    /// Returns [`PricingComputationError`] for an invalid schedule, zero replica
    /// target, non-forward retention window, or arithmetic overflow.
    pub fn public_pin_fee(
        &self,
        class: StorageClass,
        content_length_bytes: u64,
        min_replicas: u16,
        submitted_epoch: u64,
        retention_epoch: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        if min_replicas == 0 {
            return Err(PricingComputationError::ZeroReplicaCount);
        }
        let requested_window = retention_epoch
            .checked_sub(submitted_epoch)
            .filter(|v| *v > 0)
            .ok_or(PricingComputationError::InvalidRetentionWindow {
                submitted_epoch,
                retention_epoch,
            })?;
        let bytes_per_gib = BYTES_PER_GIB;
        let gib = u128::from(content_length_bytes)
            .checked_add(bytes_per_gib - 1)
            .ok_or(PricingComputationError::ArithmeticOverflow(
                "public pin GiB rounding",
            ))?
            .checked_div(bytes_per_gib)
            .ok_or(PricingComputationError::DivisionByZero(
                "public pin bytes per GiB",
            ))?
            .max(1);
        let replicated_gib = gib.checked_mul(u128::from(min_replicas)).ok_or(
            PricingComputationError::ArithmeticOverflow("public pin replicated GiB"),
        )?;
        let billing_window = requested_window.max(self.credit.settlement_window_secs);
        self.storage_charge_validated(class, replicated_gib, billing_window)
    }

    /// Compute egress charges for `egress_gib` volume.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when validation or exact arithmetic fails.
    pub fn egress_charge(
        &self,
        class: StorageClass,
        egress_gib: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        if egress_gib == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate_validated(class)?;
        Ok(tier
            .egress_price_per_gib
            .try_mul_decimal(&Numeric::from(egress_gib))?)
    }

    /// Compute egress charges for `egress_bytes` volume.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when validation or exact arithmetic fails.
    pub fn egress_charge_bytes(
        &self,
        class: StorageClass,
        egress_bytes: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        if egress_bytes == 0 {
            return Ok(Quantity::zero());
        }
        let tier = self.tier_rate_validated(class)?;
        Ok(multiply_ratio(
            &tier.egress_price_per_gib,
            u128::from(egress_bytes),
            BYTES_PER_GIB,
            RoundingMode::TowardZero,
        )?)
    }

    /// Expected storage charge for one settlement window at the current utilisation.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when validation or exact arithmetic fails.
    pub fn expected_settlement_storage_charge(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.storage_charge(class, avg_utilised_gib, self.credit.settlement_window_secs)
    }

    /// Required nominal bonded collateral for the given utilisation.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when validation or exact arithmetic
    /// fails, or `now_epoch` predates onboarding.
    pub fn required_collateral(
        &self,
        class: StorageClass,
        avg_utilised_gib: u64,
        onboarding_epoch: u64,
        now_epoch: u64,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        let monthly_charge = self.storage_charge_validated(
            class,
            u128::from(avg_utilised_gib),
            SECONDS_PER_BILLING_MONTH,
        )?;
        let base = multiply_ratio(
            &monthly_charge,
            u128::from(self.collateral.multiplier_bps),
            10_000,
            RoundingMode::NearestAway,
        )?;
        let discount_bps = self
            .collateral
            .discount_multiplier_bps(onboarding_epoch, now_epoch)?;
        Ok(multiply_ratio(
            &base,
            u128::from(discount_bps),
            10_000,
            RoundingMode::NearestAway,
        )?)
    }

    /// Low-balance alert threshold derived from the expected settlement charge.
    ///
    /// # Errors
    /// Returns [`PricingComputationError`] when validation or exact arithmetic fails.
    pub fn low_balance_threshold(
        &self,
        expected_settlement_charge: &Quantity,
    ) -> Result<Quantity, PricingComputationError> {
        self.validate()?;
        Ok(multiply_ratio(
            expected_settlement_charge,
            u128::from(self.credit.low_balance_alert_bps),
            10_000,
            RoundingMode::NearestAway,
        )?)
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
    /// The first release requires exactly one tier for every storage class.
    #[error("pricing schedule has {found} tiers; expected exactly {expected}")]
    InvalidTierCount {
        /// Number of tiers supplied.
        found: usize,
        /// Required number of tiers.
        expected: usize,
    },
    /// Tier rows must use the canonical storage-class order.
    #[error(
        "pricing tier {index} is out of canonical order: expected {expected:?}, found {found:?}"
    )]
    NonCanonicalTierOrder {
        /// Offending tier index.
        index: usize,
        /// Expected class at the index.
        expected: StorageClass,
        /// Supplied class at the index.
        found: StorageClass,
    },
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
    /// Onboarding discounts cannot exceed the full collateral requirement.
    #[error("onboarding collateral discount must be within 1..=10000 bps (found {0})")]
    OnboardingDiscountOutOfRange(u32),
    /// Onboarding period must be positive.
    #[error("onboarding collateral period must be positive")]
    InvalidOnboardingPeriod,
    /// Settlement window must be non-zero.
    #[error("settlement window must be non-zero")]
    InvalidSettlementWindow,
    /// Settlement plus grace must fit in the epoch representation.
    #[error("settlement window plus grace overflows u64")]
    SettlementWindowOverflow,
    /// Invalid low-balance alert threshold.
    #[error("low-balance alert threshold must be within 1..=10000 basis points (found {0})")]
    InvalidLowBalanceThreshold(u16),
    /// Loyalty discount is outside the basis-point range.
    #[error("loyalty discount must be within 0..=10000 basis points (found {0})")]
    InvalidLoyaltyDiscount(u16),
    /// A nonzero loyalty discount requires a positive participation period.
    #[error("a nonzero loyalty discount requires a positive loyalty period")]
    InvalidLoyaltyPeriod,
    /// Commitment-discount rows exceed the hard resource bound.
    #[error("pricing schedule has {found} commitment tiers; maximum is {maximum}")]
    TooManyCommitmentDiscountTiers {
        /// Number of rows supplied.
        found: usize,
        /// Maximum permitted rows.
        maximum: usize,
    },
    /// Commitment thresholds must be positive, distinct, and strictly increasing.
    #[error("commitment discount tiers are not canonical at index {index}")]
    NonCanonicalCommitmentTierOrder {
        /// Offending row index.
        index: usize,
    },
    /// Commitment discount values must be positive, bounded, and monotonic.
    #[error("invalid commitment discount {discount_bps} bps at index {index}")]
    InvalidCommitmentDiscount {
        /// Offending row index.
        index: usize,
        /// Supplied basis-point discount.
        discount_bps: u16,
    },
    /// Stacked discounts may not erase more than the full price.
    #[error("combined loyalty and commitment discounts exceed 10000 basis points")]
    CombinedDiscountExceedsFullPrice,
    /// Notes must be canonical, bounded, and control-free.
    #[error(
        "pricing notes must be non-empty canonical control-free UTF-8 of at most {maximum} bytes (found {found})"
    )]
    InvalidNotes {
        /// Supplied byte length.
        found: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
}

/// Errors raised while calculating deterministic `SoraFS` pricing values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PricingComputationError {
    /// The stored pricing schedule itself is invalid.
    #[error("invalid pricing schedule: {0}")]
    InvalidSchedule(#[from] PricingValidationError),
    /// The requested class has no exact tier.
    #[error("pricing schedule is missing storage class tier {0:?}")]
    MissingTier(StorageClass),
    /// Exact arithmetic exceeded the target integer representation.
    #[error("pricing arithmetic overflow while computing {0}")]
    ArithmeticOverflow(&'static str),
    /// An internal divisor was zero.
    #[error("pricing division by zero while computing {0}")]
    DivisionByZero(&'static str),
    /// Public pins must request at least one replica.
    #[error("public pin replica count must be positive")]
    ZeroReplicaCount,
    /// Retention must be strictly later than submission.
    #[error(
        "public pin retention epoch {retention_epoch} must be greater than submission epoch {submitted_epoch}"
    )]
    InvalidRetentionWindow {
        /// Submission epoch.
        submitted_epoch: u64,
        /// Requested retention epoch.
        retention_epoch: u64,
    },
    /// Collateral cannot be evaluated before provider onboarding.
    #[error("collateral epoch {now_epoch} predates onboarding epoch {onboarding_epoch}")]
    EpochBeforeOnboarding {
        /// Provider onboarding epoch.
        onboarding_epoch: u64,
        /// Evaluation epoch.
        now_epoch: u64,
    },
    /// Bounded nominal quantity arithmetic failed.
    #[error("pricing quantity arithmetic failed: {0}")]
    Quantity(#[from] NumericOperationError),
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
    /// Unslashed collateral currently locked in the authoritative native reserve.
    ///
    /// This field is not a funding source. Core accepts it only when it exactly
    /// matches the owner-funded reserve partition net of treasury-funded
    /// principal and the custody-backed `slashed` lien.
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
    /// Total collateral held under a custody-backed slash lien for under-delivery.
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
    /// # Errors
    ///
    /// Returns [`CreditMutationError`] without changing the record when the
    /// debit exceeds available credit or the settlement epoch is not newer.
    pub fn apply_charge(
        &mut self,
        debit: &Quantity,
        epoch: u64,
    ) -> Result<(), CreditMutationError> {
        if epoch <= self.last_settlement_epoch {
            return Err(CreditMutationError::NonMonotonicSettlementEpoch {
                previous: self.last_settlement_epoch,
                proposed: epoch,
            });
        }
        if debit > &self.available_credit {
            return Err(CreditMutationError::InsufficientCredit {
                available: self.available_credit.clone(),
                requested: debit.clone(),
            });
        }
        let available_credit = self.available_credit.checked_sub(debit)?;
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
    ///
    /// # Errors
    ///
    /// Returns [`CreditMutationError::StrikeOverflow`] without mutation when
    /// the counter is exhausted.
    pub fn add_strike(&mut self) -> Result<(), CreditMutationError> {
        self.under_delivery_strikes = self
            .under_delivery_strikes
            .checked_add(1)
            .ok_or(CreditMutationError::StrikeOverflow)?;
        Ok(())
    }

    /// Clear consecutive strike tracking.
    pub fn reset_strikes(&mut self) {
        self.under_delivery_strikes = 0;
    }

    /// Move a penalty from the usable bond into its custody-backed slash lien.
    ///
    /// # Errors
    ///
    /// Returns [`CreditMutationError`] without mutation for an overdraw,
    /// cumulative overflow, or a repeated/backdated penalty epoch.
    pub fn apply_penalty(
        &mut self,
        penalty: &Quantity,
        epoch: u64,
    ) -> Result<(), CreditMutationError> {
        if penalty.is_zero() {
            return Ok(());
        }
        if let Some(previous) = self.last_penalty_epoch
            && epoch <= previous
        {
            return Err(CreditMutationError::NonMonotonicPenaltyEpoch {
                previous,
                proposed: epoch,
            });
        }
        if penalty > &self.bonded {
            return Err(CreditMutationError::PenaltyExceedsBond {
                bonded: self.bonded.clone(),
                requested: penalty.clone(),
            });
        }
        let bonded = self.bonded.checked_sub(penalty)?;
        let slashed = self
            .slashed
            .checked_add(penalty)
            .map_err(|_| CreditMutationError::SlashedTotalOverflow)?;
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

/// Errors raised while applying provider-credit mutations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CreditMutationError {
    /// Settlement epochs must advance strictly.
    #[error("settlement epoch {proposed} must be greater than previous epoch {previous}")]
    NonMonotonicSettlementEpoch {
        /// Previously committed epoch.
        previous: u64,
        /// Proposed epoch.
        proposed: u64,
    },
    /// A debit cannot exceed available credit.
    #[error("credit debit {requested} exceeds available balance {available}")]
    InsufficientCredit {
        /// Available balance.
        available: Quantity,
        /// Requested debit.
        requested: Quantity,
    },
    /// Strike counter exhausted its representation.
    #[error("under-delivery strike counter overflow")]
    StrikeOverflow,
    /// Penalties cannot exceed bonded collateral.
    #[error("penalty {requested} exceeds bonded collateral {bonded}")]
    PenaltyExceedsBond {
        /// Bonded collateral.
        bonded: Quantity,
        /// Requested penalty.
        requested: Quantity,
    },
    /// Cumulative slash accounting overflowed.
    #[error("cumulative slashed collateral overflow")]
    SlashedTotalOverflow,
    /// Penalty epochs must advance strictly.
    #[error("penalty epoch {proposed} must be greater than previous epoch {previous}")]
    NonMonotonicPenaltyEpoch {
        /// Previously committed epoch.
        previous: u64,
        /// Proposed epoch.
        proposed: u64,
    },
    /// Bounded nominal quantity arithmetic failed.
    #[error("provider credit quantity arithmetic failed: {0}")]
    Quantity(#[from] NumericOperationError),
}

fn checked_mul_div_floor(
    value: u128,
    multiplier: u128,
    divisor: u128,
    context: &'static str,
) -> Result<u128, PricingComputationError> {
    checked_mul_div_parts(value, multiplier, divisor, context).map(|(quotient, _)| quotient)
}

/// Compute `(value × multiplier) / divisor`, rounded down, without overflowing
/// intermediate `u128` products.
///
/// # Errors
///
/// Returns [`PricingComputationError::DivisionByZero`] for a zero divisor and
/// [`PricingComputationError::ArithmeticOverflow`] only when the final quotient
/// cannot fit in `u128`.
pub fn checked_mul_div_floor_u128(
    value: u128,
    multiplier: u128,
    divisor: u128,
) -> Result<u128, PricingComputationError> {
    checked_mul_div_floor(value, multiplier, divisor, "u128 multiply/divide")
}

/// Compute `(value × multiplier) / divisor`, rounded half-up, without
/// overflowing intermediate `u128` products.
///
/// # Errors
///
/// Returns [`PricingComputationError::DivisionByZero`] for a zero divisor and
/// [`PricingComputationError::ArithmeticOverflow`] only when the final rounded
/// result itself cannot fit in `u128`.
pub fn checked_mul_div_round_u128(
    value: u128,
    multiplier: u128,
    divisor: u128,
) -> Result<u128, PricingComputationError> {
    checked_mul_div_round(value, multiplier, divisor, "u128 multiply/divide")
}

fn checked_mul_div_round(
    value: u128,
    multiplier: u128,
    divisor: u128,
    context: &'static str,
) -> Result<u128, PricingComputationError> {
    let (quotient, remainder) = checked_mul_div_parts(value, multiplier, divisor, context)?;
    let round_up_at = divisor
        .checked_sub(divisor / 2)
        .ok_or(PricingComputationError::ArithmeticOverflow(context))?;
    if remainder >= round_up_at {
        quotient
            .checked_add(1)
            .ok_or(PricingComputationError::ArithmeticOverflow(context))
    } else {
        Ok(quotient)
    }
}

fn checked_mul_div_parts(
    value: u128,
    multiplier: u128,
    divisor: u128,
    context: &'static str,
) -> Result<(u128, u128), PricingComputationError> {
    if divisor == 0 {
        return Err(PricingComputationError::DivisionByZero(context));
    }

    let whole = value / divisor;
    let remainder = value % divisor;
    let high = whole
        .checked_mul(multiplier)
        .ok_or(PricingComputationError::ArithmeticOverflow(context))?;
    let (low, product_remainder) = multiply_remainder_div(remainder, multiplier, divisor, context)?;
    let quotient = high
        .checked_add(low)
        .ok_or(PricingComputationError::ArithmeticOverflow(context))?;
    Ok((quotient, product_remainder))
}

fn multiply_remainder_div(
    value: u128,
    multiplier: u128,
    divisor: u128,
    context: &'static str,
) -> Result<(u128, u128), PricingComputationError> {
    debug_assert!(divisor > 0);
    debug_assert!(value < divisor);

    let mut quotient = 0u128;
    let mut remainder = 0u128;
    for bit in (0..u128::BITS).rev() {
        quotient = quotient
            .checked_mul(2)
            .ok_or(PricingComputationError::ArithmeticOverflow(context))?;
        let (next_remainder, carry) = add_mod(remainder, remainder, divisor);
        remainder = next_remainder;
        quotient = quotient
            .checked_add(u128::from(carry))
            .ok_or(PricingComputationError::ArithmeticOverflow(context))?;

        if (multiplier >> bit) & 1 == 1 {
            let (next_remainder, carry) = add_mod(remainder, value, divisor);
            remainder = next_remainder;
            quotient = quotient
                .checked_add(u128::from(carry))
                .ok_or(PricingComputationError::ArithmeticOverflow(context))?;
        }
    }
    Ok((quotient, remainder))
}

fn add_mod(left: u128, right: u128, modulus: u128) -> (u128, bool) {
    debug_assert!(modulus > 0);
    debug_assert!(left < modulus);
    debug_assert!(right < modulus);
    if left >= modulus - right {
        (left - (modulus - right), true)
    } else {
        (left + right, false)
    }
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
        let bytes_per_gib = u64::try_from(BYTES_PER_GIB).expect("BYTES_PER_GIB fits within u64");
        let per_gib = schedule
            .egress_charge_bytes(StorageClass::Hot, bytes_per_gib)
            .expect("bounded egress charge");
        assert_eq!(
            per_gib,
            schedule
                .tier_rate(StorageClass::Hot)
                .expect("hot tier")
                .egress_price_per_gib
                .clone()
        );

        let half_bytes = u64::try_from(BYTES_PER_GIB / 2).expect("half GiB fits within u64");
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
    fn schedule_validation_rejects_noncanonical_governance_inputs() {
        let mut currency = PricingScheduleRecord::launch_default();
        currency.currency_code = "XOR".to_owned();
        assert!(matches!(
            currency.validate(),
            Err(PricingValidationError::InvalidCurrencyCode(_))
        ));

        let mut missing_tier = PricingScheduleRecord::launch_default();
        missing_tier.tiers.pop();
        assert!(matches!(
            missing_tier.validate(),
            Err(PricingValidationError::InvalidTierCount { .. })
        ));

        let mut reordered = PricingScheduleRecord::launch_default();
        reordered.tiers.swap(0, 1);
        assert!(matches!(
            reordered.validate(),
            Err(PricingValidationError::NonCanonicalTierOrder { .. })
        ));

        let mut excessive_onboarding = PricingScheduleRecord::launch_default();
        excessive_onboarding.collateral.onboarding_discount_bps = 10_001;
        assert!(matches!(
            excessive_onboarding.validate(),
            Err(PricingValidationError::OnboardingDiscountOutOfRange(10_001))
        ));

        let mut zero_onboarding_period = PricingScheduleRecord::launch_default();
        zero_onboarding_period.collateral.onboarding_period_secs = 0;
        assert!(matches!(
            zero_onboarding_period.validate(),
            Err(PricingValidationError::InvalidOnboardingPeriod)
        ));

        let mut window_overflow = PricingScheduleRecord::launch_default();
        window_overflow.credit.settlement_window_secs = u64::MAX;
        window_overflow.credit.settlement_grace_secs = 1;
        assert!(matches!(
            window_overflow.validate(),
            Err(PricingValidationError::SettlementWindowOverflow)
        ));

        let mut notes = PricingScheduleRecord::launch_default();
        notes.notes = Some("x".repeat(MAX_PRICING_NOTES_BYTES + 1));
        assert!(matches!(
            notes.validate(),
            Err(PricingValidationError::InvalidNotes { .. })
        ));
    }

    #[test]
    fn schedule_validation_rejects_discount_ambiguity_and_floods() {
        let mut reversed = PricingScheduleRecord::launch_default();
        reversed.discounts.commitment_tiers.reverse();
        assert!(matches!(
            reversed.validate(),
            Err(PricingValidationError::NonCanonicalCommitmentTierOrder { .. })
        ));

        let mut decreasing_discount = PricingScheduleRecord::launch_default();
        decreasing_discount.discounts.commitment_tiers[1].discount_bps = 100;
        assert!(matches!(
            decreasing_discount.validate(),
            Err(PricingValidationError::InvalidCommitmentDiscount { .. })
        ));

        let mut combined = PricingScheduleRecord::launch_default();
        combined.discounts.loyalty_discount_bps = 9_000;
        combined.discounts.commitment_tiers[1].discount_bps = 2_000;
        assert!(matches!(
            combined.validate(),
            Err(PricingValidationError::CombinedDiscountExceedsFullPrice)
        ));

        let mut flood = PricingScheduleRecord::launch_default();
        flood.discounts.commitment_tiers = (1..=MAX_COMMITMENT_DISCOUNT_TIERS + 1)
            .map(|index| CommitmentDiscountTier {
                minimum_commitment_gib_month: u64::try_from(index)
                    .expect("test tier index fits u64"),
                discount_bps: 1,
            })
            .collect();
        assert!(matches!(
            flood.validate(),
            Err(PricingValidationError::TooManyCommitmentDiscountTiers { .. })
        ));
    }

    #[test]
    fn checked_mul_div_handles_wide_intermediates_and_overflow() {
        assert_eq!(
            checked_mul_div_round(u128::MAX, u128::MAX, u128::MAX, "test"),
            Ok(u128::MAX)
        );
        assert_eq!(
            checked_mul_div_floor(u128::MAX, 2, 3, "test"),
            Ok((u128::MAX / 3) * 2)
        );
        assert!(matches!(
            checked_mul_div_round(u128::MAX, u128::MAX, 1, "test"),
            Err(PricingComputationError::ArithmeticOverflow("test"))
        ));
        assert!(matches!(
            checked_mul_div_round(1, 1, 0, "test"),
            Err(PricingComputationError::DivisionByZero("test"))
        ));
    }

    #[test]
    fn checked_mul_div_matches_native_arithmetic_across_adversarial_sweep() {
        let mut state = 0xD1B5_4A32_D192_ED03u64;
        for case in 0..4_096 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let value = state;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let multiplier = state;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let divisor = state.max(1);

            let product = u128::from(value) * u128::from(multiplier);
            let divisor = u128::from(divisor);
            assert_eq!(
                checked_mul_div_floor(u128::from(value), u128::from(multiplier), divisor, "sweep",),
                Ok(product / divisor),
                "floor mismatch in case {case}",
            );
            assert_eq!(
                checked_mul_div_round(u128::from(value), u128::from(multiplier), divisor, "sweep",),
                Ok((product + divisor / 2) / divisor),
                "rounding mismatch in case {case}",
            );
        }
    }

    #[test]
    fn public_pin_fee_rejects_invalid_windows_and_charges_empty_payloads() {
        let schedule = PricingScheduleRecord::launch_default();
        let empty_fee = schedule
            .public_pin_fee(StorageClass::Hot, 0, 1, 10, 11)
            .expect("empty payload receives minimum capacity charge");
        assert!(!empty_fee.is_zero());

        assert!(matches!(
            schedule.public_pin_fee(StorageClass::Hot, 1, 0, 10, 11),
            Err(PricingComputationError::ZeroReplicaCount)
        ));
        assert!(matches!(
            schedule.public_pin_fee(StorageClass::Hot, 1, 1, 10, 10),
            Err(PricingComputationError::InvalidRetentionWindow { .. })
        ));
        assert!(matches!(
            schedule.required_collateral(StorageClass::Hot, 1, 11, 10),
            Err(PricingComputationError::EpochBeforeOnboarding { .. })
        ));
    }

    #[test]
    fn public_pin_fee_scales_with_bytes_replicas_and_retention() {
        let schedule = PricingScheduleRecord::launch_default();
        let one_gib = u64::try_from(BYTES_PER_GIB).expect("GiB constant fits u64");
        let base = schedule
            .public_pin_fee(StorageClass::Hot, one_gib, 1, 0, SECONDS_PER_WEEK)
            .expect("base public pin fee");
        let larger = schedule
            .public_pin_fee(
                StorageClass::Hot,
                one_gib.checked_mul(2).expect("two GiB fits u64"),
                1,
                0,
                SECONDS_PER_WEEK,
            )
            .expect("larger public pin fee");
        let replicated = schedule
            .public_pin_fee(StorageClass::Hot, one_gib, 2, 0, SECONDS_PER_WEEK)
            .expect("replicated public pin fee");
        let longer = schedule
            .public_pin_fee(
                StorageClass::Hot,
                one_gib,
                1,
                0,
                SECONDS_PER_WEEK
                    .checked_mul(2)
                    .expect("two settlement windows fit u64"),
            )
            .expect("longer public pin fee");

        assert!(larger > base, "stored bytes must increase the prepaid fee");
        assert!(
            replicated > base,
            "replica count must increase the prepaid fee"
        );
        assert!(
            longer > base,
            "retention duration must increase the prepaid fee"
        );
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
    fn provider_credit_mutations_fail_atomically() {
        let mut credit = ProviderCreditRecord::new(
            ProviderId::default(),
            quantity_nanos(100),
            quantity_nanos(50),
            Quantity::zero(),
            Quantity::zero(),
            0,
            5,
            Metadata::default(),
        );

        let before = credit.clone();
        assert!(matches!(
            credit.apply_charge(&quantity_nanos(101), 6),
            Err(CreditMutationError::InsufficientCredit { .. })
        ));
        assert_eq!(credit, before);

        assert!(matches!(
            credit.apply_charge(&quantity_nanos(1), 5),
            Err(CreditMutationError::NonMonotonicSettlementEpoch { .. })
        ));
        assert_eq!(credit, before);

        credit.under_delivery_strikes = u32::MAX;
        assert!(matches!(
            credit.add_strike(),
            Err(CreditMutationError::StrikeOverflow)
        ));
        assert_eq!(credit.under_delivery_strikes, u32::MAX);

        let before_penalty = credit.clone();
        assert!(matches!(
            credit.apply_penalty(&quantity_nanos(51), 10),
            Err(CreditMutationError::PenaltyExceedsBond { .. })
        ));
        assert_eq!(credit, before_penalty);

        credit.slashed = maximum_quantity();
        let before_overflow = credit.clone();
        assert!(matches!(
            credit.apply_penalty(&quantity_nanos(1), 10),
            Err(CreditMutationError::SlashedTotalOverflow)
        ));
        assert_eq!(credit, before_overflow);
    }

    #[test]
    fn provider_credit_mutations_enforce_epoch_progression() {
        let mut credit = ProviderCreditRecord::new(
            ProviderId::default(),
            quantity_nanos(100),
            quantity_nanos(50),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        credit
            .apply_charge(&quantity_nanos(10), 1)
            .expect("first charge");
        assert_eq!(credit.available_credit, quantity_nanos(90));
        credit
            .apply_penalty(&quantity_nanos(5), 2)
            .expect("first penalty");
        let committed = credit.clone();
        assert!(matches!(
            credit.apply_penalty(&quantity_nanos(1), 2),
            Err(CreditMutationError::NonMonotonicPenaltyEpoch { .. })
        ));
        assert_eq!(credit, committed);
    }
}
