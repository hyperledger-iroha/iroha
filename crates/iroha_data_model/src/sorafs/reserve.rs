//! Reserve-plus-rent policy quoting (SFM-6 / DA-7).
//!
//! These types translate the economics specification captured in
//! `docs/source/sorafs_reserve_rent_plan.md` into deterministic payloads so
//! governance, CLI tooling, and ledger ISIs can derive the same rent and
//! reserve requirements. The quoting logic intentionally mirrors the formulas
//! documented in the roadmap: monthly rent is computed per storage class and
//! duration, underwriting ratios determine collateral requirements, and credit
//! line caps / APR values track the assigned provider tier.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::deal::{BASIS_POINTS_PER_UNIT, DealAmountError, MICRO_XOR_PER_XOR, XorAmount};
use thiserror::Error;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, sorafs::pin_registry::StorageClass};

/// Schema version for [`ReservePolicyV1`].
pub const RESERVE_POLICY_VERSION_V1: u8 = 1;

/// Reserve tiers referenced by the Reserve+Rent policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "tier", content = "value"))]
pub enum ReserveTier {
    /// Tier A — preferred operators with track record (2× underwriting).
    TierA,
    /// Tier B — baseline operators (3× underwriting, smaller credit line).
    TierB,
    /// Tier C — new entrants/manual approval lanes (4.5× underwriting, manual credit).
    TierC,
}

impl ReserveTier {}

/// Rental commitment duration (`monthly`, `quarterly`, `annual`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "duration", content = "value"))]
pub enum ReserveDuration {
    /// Monthly commitment (no discount).
    Monthly,
    /// Quarterly commitment (10% discount).
    Quarterly,
    /// Annual commitment (25% discount).
    Annual,
}

/// Rent rate per storage class (GiB-month basis).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ClassRentRate {
    /// Storage class (`Hot`, `Warm`, `Cold`).
    pub storage_class: StorageClass,
    /// Rent in XOR (micro units) charged per GiB-month.
    pub rent_per_gib_month: XorAmount,
}

impl ClassRentRate {
    /// Construct a rent rate entry.
    #[must_use]
    pub const fn new(storage_class: StorageClass, rent_per_gib_month: XorAmount) -> Self {
        Self {
            storage_class,
            rent_per_gib_month,
        }
    }
}

/// Duration factors encoded as basis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DurationFactorSet {
    /// Monthly factor (defaults to 1.0 = `10_000` bps).
    pub monthly_bps: u16,
    /// Quarterly factor (defaults to 0.9 = `9_000` bps).
    pub quarterly_bps: u16,
    /// Annual factor (defaults to 0.75 = `7_500` bps).
    pub annual_bps: u16,
}

impl DurationFactorSet {
    const fn factor_bps(self, duration: ReserveDuration) -> u16 {
        match duration {
            ReserveDuration::Monthly => self.monthly_bps,
            ReserveDuration::Quarterly => self.quarterly_bps,
            ReserveDuration::Annual => self.annual_bps,
        }
    }
}

impl Default for DurationFactorSet {
    fn default() -> Self {
        Self {
            monthly_bps: BASIS_POINTS_PER_UNIT,
            quarterly_bps: 9_000,
            annual_bps: 7_500,
        }
    }
}

/// Per-tier underwriting + credit configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveTierConfig {
    /// Tier identifier.
    pub tier: ReserveTier,
    /// Underwriting ratio (basis points). Allows values above 100% (e.g., `20_000` means 2× rent).
    pub underwriting_ratio_bps: u32,
    /// Credit line cap multiplier (basis points) relative to the monthly rent.
    #[cfg_attr(feature = "json", norito(default))]
    pub credit_line_cap_bps: Option<u32>,
    /// Annual percentage rate applied to credit usage (basis points).
    pub interest_apr_bps: u16,
}

impl ReserveTierConfig {
    /// Construct a tier configuration.
    #[must_use]
    pub const fn new(
        tier: ReserveTier,
        underwriting_ratio_bps: u32,
        credit_line_cap_bps: Option<u32>,
        interest_apr_bps: u16,
    ) -> Self {
        Self {
            tier,
            underwriting_ratio_bps,
            credit_line_cap_bps,
            interest_apr_bps,
        }
    }
}

/// Reserve + rent policy payload (mirrors `sorafs_reserve_rent_plan.md`).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReservePolicyV1 {
    /// Schema version (`RESERVE_POLICY_VERSION_V1`).
    pub version: u8,
    /// Rent rates per storage class (GiB-month basis).
    pub rent_rates: Vec<ClassRentRate>,
    /// Duration discount factors.
    pub duration_factors: DurationFactorSet,
    /// Tier underwriting / credit configuration.
    pub tiers: Vec<ReserveTierConfig>,
    /// Reserve top-up threshold (basis points of required reserve).
    pub top_up_threshold_bps: u16,
}

impl Default for ReservePolicyV1 {
    fn default() -> Self {
        let rent_rates = vec![
            ClassRentRate::new(
                StorageClass::Hot,
                XorAmount::from_micro(12 * MICRO_XOR_PER_XOR),
            ),
            ClassRentRate::new(
                StorageClass::Warm,
                XorAmount::from_micro(6 * MICRO_XOR_PER_XOR),
            ),
            ClassRentRate::new(
                StorageClass::Cold,
                XorAmount::from_micro(2 * MICRO_XOR_PER_XOR),
            ),
        ];
        let tiers = vec![
            ReserveTierConfig::new(ReserveTier::TierA, 20_000, Some(20_000), 300),
            ReserveTierConfig::new(ReserveTier::TierB, 30_000, Some(10_000), 600),
            ReserveTierConfig::new(ReserveTier::TierC, 45_000, None, 0),
        ];
        Self {
            version: RESERVE_POLICY_VERSION_V1,
            rent_rates,
            duration_factors: DurationFactorSet::default(),
            tiers,
            top_up_threshold_bps: 8_000,
        }
    }
}

/// Quoted rent/reserve breakdown for a provider + tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveQuote {
    /// Storage class for the commitment.
    pub storage_class: StorageClass,
    /// Provider tier.
    pub tier: ReserveTier,
    /// Commitment duration.
    pub duration: ReserveDuration,
    /// Logical GiB covered by the quote.
    pub capacity_gib: u64,
    /// Monthly rent before reserve offsets.
    pub monthly_rent: XorAmount,
    /// Required reserve (underwriting ratio × monthly rent).
    pub reserve_requirement: XorAmount,
    /// Effective rent charged after considering the reserve balance.
    pub effective_rent: XorAmount,
    /// Reserve balance supplied in the quote input.
    pub reserve_balance: XorAmount,
    /// Portion of rent offset by the reserve balance.
    pub reserve_offset: XorAmount,
    /// Reserve balance threshold that triggers top-up alerts.
    pub top_up_threshold: XorAmount,
    /// Credit line cap applied to this tier (if automatic).
    #[cfg_attr(feature = "json", norito(default))]
    pub credit_line_cap: Option<XorAmount>,
    /// Annual percentage rate for credit usage (basis points).
    pub interest_apr_bps: u16,
    /// Tier underwriting ratio (basis points).
    pub underwriting_ratio_bps: u32,
}

/// Ledger-oriented projection derived from a [`ReserveQuote`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveLedgerProjection {
    /// Effective rent that must be settled for the period.
    pub rent_due: XorAmount,
    /// Additional reserve required to satisfy the underwriting ratio.
    pub reserve_shortfall: XorAmount,
    /// Top-up amount required to reach the alert threshold.
    pub top_up_shortfall: XorAmount,
    /// Whether the current reserve balance satisfies the underwriting ratio.
    #[cfg_attr(feature = "json", norito(default))]
    pub meets_underwriting: bool,
    /// Whether the balance fell below the configured top-up threshold.
    #[cfg_attr(feature = "json", norito(default))]
    pub needs_top_up_alert: bool,
}

impl ReserveQuote {
    /// Project ledger-facing rent/reserve deltas based on the quote.
    #[must_use]
    pub fn ledger_projection(&self) -> ReserveLedgerProjection {
        let reserve_shortfall = self
            .reserve_requirement
            .saturating_sub(self.reserve_balance);
        let top_up_shortfall = self.top_up_threshold.saturating_sub(self.reserve_balance);
        ReserveLedgerProjection {
            rent_due: self.effective_rent,
            reserve_shortfall,
            top_up_shortfall,
            meets_underwriting: reserve_shortfall.is_zero(),
            needs_top_up_alert: !top_up_shortfall.is_zero(),
        }
    }
}

/// Errors emitted during reserve quoting or validation.
#[allow(variant_size_differences)]
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReservePolicyError {
    /// Unsupported policy version encountered.
    #[error("unsupported reserve policy version {found}")]
    UnsupportedVersion {
        /// Reported version.
        found: u8,
    },
    /// Capacity in GiB must be non-zero.
    #[error("capacity must be greater than zero")]
    ZeroCapacity,
    /// Missing rent rate for the provided storage class.
    #[error("rent rate not configured for storage class {0:?}")]
    MissingRentRate(StorageClass),
    /// Missing tier configuration.
    #[error("tier configuration not found for {0:?}")]
    MissingTierConfig(ReserveTier),
    /// Threshold ratio is invalid.
    #[error("{field_label} basis points value must be between 1 and 1_000_000 (found {basis_points})", field_label = field.label())]
    InvalidRatio {
        /// Ratio identifier.
        field: ReserveRatioField,
        /// Supplied basis points value.
        basis_points: u32,
    },
    /// Arithmetic overflow while computing the quote.
    #[error("reserve computation overflowed")]
    Overflow,
}

impl From<DealAmountError> for ReservePolicyError {
    fn from(value: DealAmountError) -> Self {
        match value {
            DealAmountError::Overflow | DealAmountError::Underflow => Self::Overflow,
        }
    }
}

/// Identifiers used when validating basis-point ratios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReserveRatioField {
    /// `top_up_threshold_bps`.
    TopUpThreshold,
    /// Tier A underwriting ratio.
    TierAUnderwriting,
    /// Tier B underwriting ratio.
    TierBUnderwriting,
    /// Tier C underwriting ratio.
    TierCUnderwriting,
    /// Tier credit line multiplier.
    CreditLineCap,
    /// Underwriting ratio encountered while dividing reserve amounts.
    UnderwritingRatio,
}

impl ReserveRatioField {
    const fn label(self) -> &'static str {
        match self {
            Self::TopUpThreshold => "top_up_threshold_bps",
            Self::TierAUnderwriting => "tier_a_underwriting_bps",
            Self::TierBUnderwriting => "tier_b_underwriting_bps",
            Self::TierCUnderwriting => "tier_c_underwriting_bps",
            Self::CreditLineCap => "credit_line_cap_bps",
            Self::UnderwritingRatio => "underwriting_ratio_bps",
        }
    }
}

impl ReservePolicyV1 {
    /// Quote the rent/reserve breakdown for the provided parameters.
    ///
    /// # Errors
    ///
    /// Returns [`ReservePolicyError`] when the policy is invalid, required
    /// configuration is missing, or arithmetic overflows occur.
    pub fn quote(
        &self,
        storage_class: StorageClass,
        capacity_gib: u64,
        duration: ReserveDuration,
        tier: ReserveTier,
        reserve_balance: XorAmount,
    ) -> Result<ReserveQuote, ReservePolicyError> {
        self.validate()?;
        if capacity_gib == 0 {
            return Err(ReservePolicyError::ZeroCapacity);
        }
        let rent_rate = self.rent_rate_for(storage_class)?;
        let tier_config = self.tier_config(tier)?;
        let duration_factor = u32::from(self.duration_factors.factor_bps(duration).max(1_u16));

        let base_rent = rent_rate.checked_mul_u64(capacity_gib)?;
        let monthly_rent = apply_basis_points_u32(base_rent, duration_factor)?;
        let reserve_requirement =
            apply_basis_points_u32(monthly_rent, tier_config.underwriting_ratio_bps)?;
        let reserve_offset =
            divide_amount_by_ratio(reserve_balance, tier_config.underwriting_ratio_bps)?;
        let effective_offset = reserve_offset.min(monthly_rent);
        let effective_rent = monthly_rent.checked_sub(effective_offset)?;
        let top_up_threshold =
            apply_basis_points_u32(reserve_requirement, u32::from(self.top_up_threshold_bps))?;
        let credit_line_cap = match tier_config.credit_line_cap_bps {
            Some(bps) => Some(apply_basis_points_u32(monthly_rent, bps)?),
            None => None,
        };

        Ok(ReserveQuote {
            storage_class,
            tier,
            duration,
            capacity_gib,
            monthly_rent,
            reserve_requirement,
            effective_rent,
            reserve_balance,
            reserve_offset: effective_offset,
            top_up_threshold,
            credit_line_cap,
            interest_apr_bps: tier_config.interest_apr_bps,
            underwriting_ratio_bps: tier_config.underwriting_ratio_bps,
        })
    }

    fn rent_rate_for(&self, storage_class: StorageClass) -> Result<XorAmount, ReservePolicyError> {
        self.rent_rates
            .iter()
            .find(|rate| rate.storage_class == storage_class)
            .map(|rate| rate.rent_per_gib_month)
            .ok_or(ReservePolicyError::MissingRentRate(storage_class))
    }

    fn tier_config(&self, tier: ReserveTier) -> Result<ReserveTierConfig, ReservePolicyError> {
        self.tiers
            .iter()
            .copied()
            .find(|config| config.tier == tier)
            .ok_or(ReservePolicyError::MissingTierConfig(tier))
    }

    fn validate(&self) -> Result<(), ReservePolicyError> {
        if self.version != RESERVE_POLICY_VERSION_V1 {
            return Err(ReservePolicyError::UnsupportedVersion {
                found: self.version,
            });
        }
        validate_ratio(
            u32::from(self.top_up_threshold_bps),
            ReserveRatioField::TopUpThreshold,
        )?;
        for tier in &self.tiers {
            let ratio_field = match tier.tier {
                ReserveTier::TierA => ReserveRatioField::TierAUnderwriting,
                ReserveTier::TierB => ReserveRatioField::TierBUnderwriting,
                ReserveTier::TierC => ReserveRatioField::TierCUnderwriting,
            };
            validate_ratio(tier.underwriting_ratio_bps, ratio_field)?;
            if let Some(cap_bps) = tier.credit_line_cap_bps {
                validate_ratio(cap_bps, ReserveRatioField::CreditLineCap)?;
            }
        }
        Ok(())
    }
}

fn validate_ratio(value: u32, field: ReserveRatioField) -> Result<(), ReservePolicyError> {
    if value == 0 || value > 1_000_000 {
        return Err(ReservePolicyError::InvalidRatio {
            field,
            basis_points: value,
        });
    }
    Ok(())
}

fn apply_basis_points_u32(
    amount: XorAmount,
    basis_points: u32,
) -> Result<XorAmount, ReservePolicyError> {
    if basis_points == 0 {
        return Ok(XorAmount::zero());
    }
    let micro = amount.as_micro();
    let scaled = micro
        .checked_mul(u128::from(basis_points))
        .ok_or(ReservePolicyError::Overflow)?;
    Ok(XorAmount::from_micro(
        scaled / u128::from(BASIS_POINTS_PER_UNIT),
    ))
}

fn divide_amount_by_ratio(
    amount: XorAmount,
    ratio_bps: u32,
) -> Result<XorAmount, ReservePolicyError> {
    if ratio_bps == 0 {
        return Err(ReservePolicyError::InvalidRatio {
            field: ReserveRatioField::UnderwritingRatio,
            basis_points: ratio_bps,
        });
    }
    let numerator = amount
        .as_micro()
        .checked_mul(u128::from(BASIS_POINTS_PER_UNIT))
        .ok_or(ReservePolicyError::Overflow)?;
    Ok(XorAmount::from_micro(numerator / u128::from(ratio_bps)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_renders_expected_quote() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorAmount::zero(),
            )
            .expect("quote succeeds");

        assert_eq!(quote.monthly_rent.as_micro(), 120_000_000);
        assert_eq!(quote.reserve_requirement.as_micro(), 240_000_000);
        assert_eq!(quote.effective_rent.as_micro(), 120_000_000);
        assert_eq!(quote.top_up_threshold.as_micro(), 192_000_000);
        assert_eq!(
            quote
                .credit_line_cap
                .expect("tier A credit line")
                .as_micro(),
            240_000_000
        );
        assert_eq!(quote.interest_apr_bps, 300);
    }

    #[test]
    fn reserve_balance_reduces_effective_rent() {
        let policy = ReservePolicyV1::default();
        let balance = XorAmount::from_micro(1_500_000); // 1.5 XOR
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                balance,
            )
            .expect("quote succeeds");

        assert_eq!(quote.reserve_offset.as_micro(), 750_000);
        assert_eq!(quote.effective_rent.as_micro(), 119_250_000);
    }

    #[test]
    fn tier_c_has_no_credit_line_cap() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Warm,
                1,
                ReserveDuration::Annual,
                ReserveTier::TierC,
                XorAmount::zero(),
            )
            .expect("quote succeeds");
        assert!(quote.credit_line_cap.is_none());
    }

    #[test]
    fn ledger_projection_identifies_shortfalls() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorAmount::zero(),
            )
            .expect("quote succeeds");
        let projection = quote.ledger_projection();
        assert_eq!(
            projection.rent_due.as_micro(),
            quote.effective_rent.as_micro()
        );
        assert_eq!(
            projection.reserve_shortfall.as_micro(),
            quote.reserve_requirement.as_micro()
        );
        assert_eq!(
            projection.top_up_shortfall.as_micro(),
            quote.top_up_threshold.as_micro()
        );
        assert!(!projection.meets_underwriting);
        assert!(projection.needs_top_up_alert);
    }

    #[test]
    fn ledger_projection_marks_satisfied_underwriting() {
        let policy = ReservePolicyV1::default();
        let baseline_quote = policy
            .quote(
                StorageClass::Hot,
                5,
                ReserveDuration::Quarterly,
                ReserveTier::TierB,
                XorAmount::zero(),
            )
            .expect("quote");
        let balance = baseline_quote.reserve_requirement;
        let quote = policy
            .quote(
                StorageClass::Hot,
                5,
                ReserveDuration::Quarterly,
                ReserveTier::TierB,
                balance,
            )
            .expect("quote with reserve");
        let projection = quote.ledger_projection();
        assert!(projection.reserve_shortfall.is_zero());
        assert!(projection.meets_underwriting);
        assert!(projection.top_up_shortfall.is_zero());
        assert!(!projection.needs_top_up_alert);
    }
}
