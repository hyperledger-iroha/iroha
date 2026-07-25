//! Reserve-plus-rent policy quoting (SFM-6 / DA-7).
//!
//! These types translate the economics specification captured in
//! `docs/source/sorafs_reserve_rent_plan.md` into deterministic payloads so
//! governance, CLI tooling, and ledger ISIs can derive the same rent and
//! reserve requirements. The quoting logic intentionally mirrors the formulas
//! documented in the roadmap: monthly rent is computed per storage class and
//! duration, underwriting ratios determine collateral requirements, and credit
//! line caps / APR values track the assigned provider tier.

use core::num::NonZeroU64;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::deal::{BASIS_POINTS_PER_UNIT, DealAmountError, XorQuantity};
use thiserror::Error;

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    account::AccountId,
    asset::AssetDefinitionId,
    events::data::sorafs::SorafsReserveLedgerEvent,
    sorafs::{capacity::ProviderId, pin_registry::StorageClass},
};

/// Schema version for [`ReservePolicyV1`].
pub const RESERVE_POLICY_VERSION_V1: u8 = 1;
/// First-release chain authority-policy version.
pub const RESERVE_AUTHORITY_POLICY_VERSION_V1: u8 = 1;
/// Hard ceiling for one reserve appeal reason or governance rationale.
pub const RESERVE_MAX_REASON_BYTES_V1: usize = 2_048;
/// Hard ceiling for pending reserve movements per provider.
pub const RESERVE_MAX_PENDING_MOVEMENTS_V1: u32 = 256;
/// Hard ceiling for open reserve appeals per provider.
pub const RESERVE_MAX_OPEN_APPEALS_V1: u32 = 16;
/// Hard ceiling for one page of finalized reserve-ledger events.
pub const RESERVE_QUERY_MAX_ITEMS_V1: u32 = 128;
/// Hard ceiling for one encoded finalized reserve-ledger event page.
pub const RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 1024 * 1024;
/// Hard ceiling for one persisted reserve-ledger event record.
pub const RESERVE_COMMITTED_EVENT_MAX_BYTES_V1: usize = 16 * 1024;
/// Domain separator for authoritative reserve-policy digests.
pub const RESERVE_AUTHORITY_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.reserve.authority-policy.v1";

/// Reserve tiers referenced by the Reserve+Rent policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ClassRentRate {
    /// Storage class (`Hot`, `Warm`, `Cold`).
    pub storage_class: StorageClass,
    /// Exact rent in XOR charged per GiB-month.
    pub rent_per_gib_month: XorQuantity,
}

impl ClassRentRate {
    /// Construct a rent rate entry.
    #[must_use]
    pub fn new(storage_class: StorageClass, rent_per_gib_month: XorQuantity) -> Self {
        Self {
            storage_class,
            rent_per_gib_month,
        }
    }
}

/// Duration factors encoded as basis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
                "12".parse().expect("default is canonical"),
            ),
            ClassRentRate::new(
                StorageClass::Warm,
                "6".parse().expect("default is canonical"),
            ),
            ClassRentRate::new(
                StorageClass::Cold,
                "2".parse().expect("default is canonical"),
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
    pub monthly_rent: XorQuantity,
    /// Required reserve (underwriting ratio × monthly rent).
    pub reserve_requirement: XorQuantity,
    /// Effective rent charged after considering the reserve balance.
    pub effective_rent: XorQuantity,
    /// Reserve balance supplied in the quote input.
    pub reserve_balance: XorQuantity,
    /// Portion of rent offset by the reserve balance.
    pub reserve_offset: XorQuantity,
    /// Reserve balance threshold that triggers top-up alerts.
    pub top_up_threshold: XorQuantity,
    /// Credit line cap applied to this tier (if automatic).
    #[cfg_attr(feature = "json", norito(default))]
    pub credit_line_cap: Option<XorQuantity>,
    /// Annual percentage rate for credit usage (basis points).
    pub interest_apr_bps: u16,
    /// Tier underwriting ratio (basis points).
    pub underwriting_ratio_bps: u32,
}

/// Ledger-oriented projection derived from a [`ReserveQuote`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveLedgerProjection {
    /// Effective rent that must be settled for the period.
    pub rent_due: XorQuantity,
    /// Additional reserve required to satisfy the underwriting ratio.
    pub reserve_shortfall: XorQuantity,
    /// Top-up amount required to reach the alert threshold.
    pub top_up_shortfall: XorQuantity,
    /// Whether the current reserve balance satisfies the underwriting ratio.
    #[cfg_attr(feature = "json", norito(default))]
    pub meets_underwriting: bool,
    /// Whether the balance fell below the configured top-up threshold.
    #[cfg_attr(feature = "json", norito(default))]
    pub needs_top_up_alert: bool,
}

/// Lifecycle stage derived from a reserve quote and payment aging inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "stage", content = "value"))]
pub enum ReserveLifecycleStage {
    /// Provider is current and reserve balance clears policy thresholds.
    Active,
    /// Provider should be warned and new manifest intake restricted.
    Warning,
    /// Rent is overdue but still within the automatic credit grace window.
    Grace,
    /// Rent is past grace and accrues penalty interest.
    Delinquent,
    /// Provider should be removed from advert rotation and escalated.
    Default,
}

/// Deterministic reserve lifecycle projection for service and CLI automation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[allow(clippy::struct_excessive_bools)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveLifecycleProjection {
    /// Derived lifecycle stage.
    pub stage: ReserveLifecycleStage,
    /// Days since the current rent obligation became due.
    pub days_past_due: u16,
    /// Grace window before delinquency.
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    pub default_after_days: u16,
    /// Effective rent due for the period.
    pub rent_due: XorQuantity,
    /// Reserve amount still required to satisfy underwriting.
    pub reserve_shortfall: XorQuantity,
    /// Amount required to clear the top-up warning threshold.
    pub top_up_shortfall: XorQuantity,
    /// Automatic credit draw applied to overdue rent.
    pub credit_draw: XorQuantity,
    /// Remaining automatic credit capacity after the draw.
    #[cfg_attr(feature = "json", norito(default))]
    pub credit_available_after_draw: Option<XorQuantity>,
    /// Uncovered rent after applying automatic credit.
    pub credit_shortfall: XorQuantity,
    /// Pro-rated penalty interest accrued after the grace window.
    pub accrued_interest: XorQuantity,
    /// Rent still payable after automatic credit plus accrued interest.
    pub total_due_after_credit: XorQuantity,
    /// Whether new manifest intake should be restricted.
    #[cfg_attr(feature = "json", norito(default))]
    pub restrict_new_manifests: bool,
    /// Whether provider adverts should be disabled.
    #[cfg_attr(feature = "json", norito(default))]
    pub disable_adverts: bool,
    /// Whether governance notification is required.
    #[cfg_attr(feature = "json", norito(default))]
    pub requires_governance_notification: bool,
    /// Whether manual credit approval is required for this tier.
    #[cfg_attr(feature = "json", norito(default))]
    pub requires_manual_credit_approval: bool,
}

impl ReserveQuote {
    /// Project ledger-facing rent/reserve deltas based on the quote.
    ///
    /// # Errors
    ///
    /// Returns an arithmetic-domain error rather than silently defaulting a
    /// malformed or unrepresentable durable amount.
    pub fn ledger_projection(&self) -> Result<ReserveLedgerProjection, ReservePolicyError> {
        let reserve_shortfall = capped_sub(&self.reserve_requirement, &self.reserve_balance)?;
        let top_up_shortfall = capped_sub(&self.top_up_threshold, &self.reserve_balance)?;
        let meets_underwriting = reserve_shortfall.is_zero();
        let needs_top_up_alert = !top_up_shortfall.is_zero();
        Ok(ReserveLedgerProjection {
            rent_due: self.effective_rent.clone(),
            reserve_shortfall,
            top_up_shortfall,
            meets_underwriting,
            needs_top_up_alert,
        })
    }

    /// Project reserve lifecycle state from deterministic aging thresholds.
    ///
    /// The projection intentionally computes only policy state and transfer
    /// amounts. It does not mutate provider status or submit ledger
    /// instructions, keeping every node able to recompute the same result.
    ///
    /// # Errors
    ///
    /// Returns [`ReservePolicyError::InvalidLifecycleWindow`] when the grace
    /// window is not strictly before the default threshold, or
    /// [`ReservePolicyError::Overflow`] when interest arithmetic overflows.
    pub fn lifecycle_projection(
        &self,
        days_past_due: u16,
        grace_period_days: u16,
        default_after_days: u16,
    ) -> Result<ReserveLifecycleProjection, ReservePolicyError> {
        if grace_period_days >= default_after_days {
            return Err(ReservePolicyError::InvalidLifecycleWindow {
                grace_period_days,
                default_after_days,
            });
        }

        let ledger = self.ledger_projection()?;
        let automatic_credit_cap = self.credit_line_cap.clone();
        let credit_draw = if days_past_due == 0 {
            XorQuantity::zero()
        } else {
            automatic_credit_cap
                .as_ref()
                .map_or_else(XorQuantity::zero, |cap| {
                    XorQuantity::min(&ledger.rent_due, cap)
                })
        };
        let credit_available_after_draw = automatic_credit_cap
            .as_ref()
            .map(|cap| cap.checked_sub(&credit_draw))
            .transpose()?;
        let credit_shortfall = capped_sub(&ledger.rent_due, &credit_draw)?;
        let delinquent_days = days_past_due.saturating_sub(grace_period_days);
        let accrued_interest =
            prorated_interest(&credit_draw, self.interest_apr_bps, delinquent_days)?;
        let total_due_after_credit = credit_shortfall.checked_add(&accrued_interest)?;
        let requires_manual_credit_approval =
            days_past_due > 0 && automatic_credit_cap.is_none() && !ledger.rent_due.is_zero();
        let stage = if days_past_due > default_after_days
            || (days_past_due > 0 && !credit_shortfall.is_zero())
        {
            ReserveLifecycleStage::Default
        } else if days_past_due > grace_period_days {
            ReserveLifecycleStage::Delinquent
        } else if days_past_due > 0 {
            ReserveLifecycleStage::Grace
        } else if ledger.needs_top_up_alert || !ledger.meets_underwriting {
            ReserveLifecycleStage::Warning
        } else {
            ReserveLifecycleStage::Active
        };
        let restrict_new_manifests = !matches!(stage, ReserveLifecycleStage::Active);
        let disable_adverts = matches!(stage, ReserveLifecycleStage::Default);
        let requires_governance_notification = matches!(
            stage,
            ReserveLifecycleStage::Delinquent | ReserveLifecycleStage::Default
        );

        Ok(ReserveLifecycleProjection {
            stage,
            days_past_due,
            grace_period_days,
            default_after_days,
            rent_due: ledger.rent_due,
            reserve_shortfall: ledger.reserve_shortfall,
            top_up_shortfall: ledger.top_up_shortfall,
            credit_draw,
            credit_available_after_draw,
            credit_shortfall,
            accrued_interest,
            total_due_after_credit,
            restrict_new_manifests,
            disable_adverts,
            requires_governance_notification,
            requires_manual_credit_approval,
        })
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
    /// Policy must contain exactly one positive entry for every class and tier.
    #[error(
        "reserve policy must contain exactly one positive rate/config for every class and tier"
    )]
    IncompletePolicy,
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
    /// Amount cannot be projected to the legacy micro-XOR adapter exactly.
    #[error("reserve amount has precision below one micro-XOR")]
    InexactAmountPrecision,
    /// A signed amount was supplied to the non-negative reserve domain.
    #[error("reserve amount cannot be negative")]
    NegativeAmount,
    /// Amount exceeds the canonical XOR fractional precision bound.
    #[error("reserve amount scale {scale} exceeds maximum {max}")]
    AmountScaleOverflow {
        /// Observed fractional digit count.
        scale: u32,
        /// Maximum accepted fractional digit count.
        max: u32,
    },
    /// Lifecycle grace/default windows are invalid.
    #[error(
        "reserve lifecycle grace period ({grace_period_days}) must be before default threshold ({default_after_days})"
    )]
    InvalidLifecycleWindow {
        /// Configured grace period in days.
        grace_period_days: u16,
        /// Configured default threshold in days.
        default_after_days: u16,
    },
}

impl From<DealAmountError> for ReservePolicyError {
    fn from(value: DealAmountError) -> Self {
        match value {
            DealAmountError::Overflow | DealAmountError::Underflow => Self::Overflow,
            DealAmountError::NegativeQuantity => Self::NegativeAmount,
            DealAmountError::ScaleOverflow { scale, max } => {
                Self::AmountScaleOverflow { scale, max }
            }
            DealAmountError::InexactMicroProjection => Self::InexactAmountPrecision,
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
    /// Commitment-duration discount factor.
    DurationFactor,
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
            Self::DurationFactor => "duration_factor_bps",
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
        reserve_balance: XorQuantity,
    ) -> Result<ReserveQuote, ReservePolicyError> {
        self.validate()?;
        if capacity_gib == 0 {
            return Err(ReservePolicyError::ZeroCapacity);
        }
        let rent_rate = self.rent_rate_for(storage_class)?;
        let tier_config = self.tier_config(tier)?;
        let duration_factor = u32::from(self.duration_factors.factor_bps(duration).max(1_u16));

        let base_rent = rent_rate.checked_mul_u64(capacity_gib)?;
        let monthly_rent = apply_basis_points_u32(&base_rent, duration_factor)?;
        let reserve_requirement =
            apply_basis_points_u32(&monthly_rent, tier_config.underwriting_ratio_bps)?;
        let reserve_offset =
            divide_amount_by_ratio(&reserve_balance, tier_config.underwriting_ratio_bps)?;
        let effective_offset = XorQuantity::min(&reserve_offset, &monthly_rent);
        let effective_rent = monthly_rent.checked_sub(&effective_offset)?;
        let top_up_threshold =
            apply_basis_points_u32(&reserve_requirement, u32::from(self.top_up_threshold_bps))?;
        let credit_line_cap = match tier_config.credit_line_cap_bps {
            Some(bps) => Some(apply_basis_points_u32(&monthly_rent, bps)?),
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

    fn rent_rate_for(
        &self,
        storage_class: StorageClass,
    ) -> Result<XorQuantity, ReservePolicyError> {
        self.rent_rates
            .iter()
            .find(|rate| rate.storage_class == storage_class)
            .map(|rate| rate.rent_per_gib_month.clone())
            .ok_or(ReservePolicyError::MissingRentRate(storage_class))
    }

    fn tier_config(&self, tier: ReserveTier) -> Result<ReserveTierConfig, ReservePolicyError> {
        self.tiers
            .iter()
            .copied()
            .find(|config| config.tier == tier)
            .ok_or(ReservePolicyError::MissingTierConfig(tier))
    }

    /// Return the validated configuration for one provider tier.
    ///
    /// # Errors
    ///
    /// Returns [`ReservePolicyError`] when the policy or tier set is invalid.
    pub fn tier_configuration(
        &self,
        tier: ReserveTier,
    ) -> Result<ReserveTierConfig, ReservePolicyError> {
        self.validate()?;
        self.tier_config(tier)
    }

    /// Validate that every class, duration factor, and provider tier is present
    /// exactly once and stays within the first-release ratio bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ReservePolicyError`] for unsupported versions, incomplete or
    /// duplicate policy tables, zero rent, or invalid ratios.
    pub fn validate(&self) -> Result<(), ReservePolicyError> {
        if self.version != RESERVE_POLICY_VERSION_V1 {
            return Err(ReservePolicyError::UnsupportedVersion {
                found: self.version,
            });
        }
        validate_ratio(
            u32::from(self.top_up_threshold_bps),
            ReserveRatioField::TopUpThreshold,
        )?;
        if self.rent_rates.len() != 3 || self.tiers.len() != 3 {
            return Err(ReservePolicyError::IncompletePolicy);
        }
        let mut seen_classes = Vec::with_capacity(self.rent_rates.len());
        for rate in &self.rent_rates {
            if rate.rent_per_gib_month.is_zero() || seen_classes.contains(&rate.storage_class) {
                return Err(ReservePolicyError::IncompletePolicy);
            }
            seen_classes.push(rate.storage_class);
        }
        let mut seen_tiers = Vec::with_capacity(self.tiers.len());
        for tier in &self.tiers {
            if seen_tiers.contains(&tier.tier) {
                return Err(ReservePolicyError::IncompletePolicy);
            }
            seen_tiers.push(tier.tier);
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
        for factor in [
            self.duration_factors.monthly_bps,
            self.duration_factors.quarterly_bps,
            self.duration_factors.annual_bps,
        ] {
            validate_ratio(u32::from(factor), ReserveRatioField::DurationFactor)?;
        }
        Ok(())
    }
}

/// Governance envelope that makes reserve economics and custody chain-authoritative.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveAuthorityPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Monotonic governance revision, beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding revision.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Deterministic rent, underwriting, credit-cap, and APR policy.
    pub economics: ReservePolicyV1,
    /// Asset definition used for reserve custody, rent, and credit.
    pub asset_definition: AssetDefinitionId,
    /// Pooled protocol custody account. Per-provider partitions remain in
    /// [`ReserveProviderAccountV1`] and are changed atomically with transfers.
    pub custody_account: AccountId,
    /// Governance treasury receiving rent and credit repayments.
    pub treasury_account: AccountId,
    /// Grace period before delinquency.
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    pub default_after_days: u16,
    /// Absolute debt ceiling per provider in addition to tier credit caps.
    pub max_provider_debt: XorQuantity,
    /// Maximum pending movements retained per provider.
    pub max_pending_movements_per_provider: u32,
    /// Maximum open appeals retained per provider.
    pub max_open_appeals_per_provider: u32,
}

impl ReserveAuthorityPolicyV1 {
    /// Validate first-release governance and economics bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ReserveAuthorityPolicyError`] for an invalid version,
    /// revision chain shape, custody binding, lifecycle window, bounded-count
    /// limit, debt cap, or nested economics policy.
    pub fn validate(&self) -> Result<(), ReserveAuthorityPolicyError> {
        if self.version != RESERVE_AUTHORITY_POLICY_VERSION_V1 {
            return Err(ReserveAuthorityPolicyError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.revision == 0 {
            return Err(ReserveAuthorityPolicyError::ZeroRevision);
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => {}
            (1, Some(_)) => return Err(ReserveAuthorityPolicyError::UnexpectedPredecessor),
            (_, Some(digest)) if digest != [0; 32] => {}
            _ => return Err(ReserveAuthorityPolicyError::MissingPredecessor),
        }
        self.economics
            .validate()
            .map_err(ReserveAuthorityPolicyError::Economics)?;
        if self.custody_account == self.treasury_account {
            return Err(ReserveAuthorityPolicyError::CustodyEqualsTreasury);
        }
        if self.grace_period_days >= self.default_after_days {
            return Err(ReserveAuthorityPolicyError::InvalidLifecycleWindow {
                grace_period_days: self.grace_period_days,
                default_after_days: self.default_after_days,
            });
        }
        if self.max_provider_debt.is_zero() {
            return Err(ReserveAuthorityPolicyError::ZeroDebtCap);
        }
        if !(1..=RESERVE_MAX_PENDING_MOVEMENTS_V1)
            .contains(&self.max_pending_movements_per_provider)
        {
            return Err(ReserveAuthorityPolicyError::InvalidPendingMovementLimit {
                found: self.max_pending_movements_per_provider,
            });
        }
        if !(1..=RESERVE_MAX_OPEN_APPEALS_V1).contains(&self.max_open_appeals_per_provider) {
            return Err(ReserveAuthorityPolicyError::InvalidOpenAppealLimit {
                found: self.max_open_appeals_per_provider,
            });
        }
        Ok(())
    }

    /// Compute the exact domain-separated digest of this policy.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical serialization fails.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(RESERVE_AUTHORITY_POLICY_DIGEST_DOMAIN_V1);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Validation errors for an authoritative reserve governance policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReserveAuthorityPolicyError {
    /// Unsupported schema version.
    #[error("unsupported reserve authority policy version {found}")]
    UnsupportedVersion {
        /// Supplied version.
        found: u8,
    },
    /// Revision zero is invalid.
    #[error("reserve authority policy revision must be non-zero")]
    ZeroRevision,
    /// Revision one unexpectedly carries a predecessor.
    #[error("reserve authority policy revision one must not carry a predecessor")]
    UnexpectedPredecessor,
    /// A later revision lacks a non-zero predecessor.
    #[error("reserve authority policy revision after one requires a non-zero predecessor")]
    MissingPredecessor,
    /// Nested economics policy is invalid.
    #[error("invalid reserve economics policy: {0}")]
    Economics(ReservePolicyError),
    /// Custody and treasury accounts must be different.
    #[error("reserve custody account must differ from treasury account")]
    CustodyEqualsTreasury,
    /// Lifecycle window is inverted or empty.
    #[error(
        "reserve grace period ({grace_period_days}) must be before default threshold ({default_after_days})"
    )]
    InvalidLifecycleWindow {
        /// Grace period.
        grace_period_days: u16,
        /// Default threshold.
        default_after_days: u16,
    },
    /// Debt cap is zero.
    #[error("reserve maximum provider debt must be non-zero")]
    ZeroDebtCap,
    /// Pending-movement limit is outside the hard bound.
    #[error("invalid reserve pending movement limit {found}")]
    InvalidPendingMovementLimit {
        /// Supplied limit.
        found: u32,
    },
    /// Open-appeal limit is outside the hard bound.
    #[error("invalid reserve open appeal limit {found}")]
    InvalidOpenAppealLimit {
        /// Supplied limit.
        found: u32,
    },
}

/// Activated reserve policy with governance provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveAuthorityPolicyRecordV1 {
    /// Activated policy body.
    pub policy: ReserveAuthorityPolicyV1,
    /// Canonical policy digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Governance authority that activated the policy.
    pub activated_by: AccountId,
    /// Block timestamp assigned to activation.
    pub activated_at_unix: u64,
}

/// Immutable provider underwriting terms used to derive rent and credit caps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveProviderTermsV1 {
    /// Provider registry identifier.
    pub provider_id: ProviderId,
    /// Provider account controlling requests and appeals.
    pub provider_account: AccountId,
    /// Underwriting tier.
    pub tier: ReserveTier,
    /// Storage class used for recurring rent.
    pub storage_class: StorageClass,
    /// Commitment duration.
    pub duration: ReserveDuration,
    /// Capacity covered by rent and underwriting.
    pub capacity_gib: u64,
}

/// Authoritative per-provider reserve, debt, and lifecycle partition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveProviderAccountV1 {
    /// Immutable underwriting terms.
    pub terms: ReserveProviderTermsV1,
    /// Policy digest under which the account was last projected.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Compare-and-set account revision.
    pub revision: u64,
    /// Provider's partition of pooled reserve custody.
    pub reserve_balance: XorQuantity,
    /// Outstanding credit principal.
    pub debt_principal: XorQuantity,
    /// Accrued unpaid interest.
    pub accrued_interest: XorQuantity,
    /// Effective draw ceiling after tier/global caps, floored at outstanding
    /// principal while a governance cap reduction is being repaid.
    pub credit_cap: XorQuantity,
    /// Current lifecycle stage.
    pub lifecycle_stage: ReserveLifecycleStage,
    /// Days past due used for the latest lifecycle projection.
    pub days_past_due: u16,
    /// Pending movement request count.
    pub pending_movements: u32,
    /// Open appeal count.
    pub open_appeals: u32,
    /// Last whole-day interest accrual anchor.
    pub interest_accrued_at_unix: u64,
    /// Block timestamp of the latest mutation.
    pub updated_at_unix: u64,
}

impl ReserveProviderAccountV1 {
    /// Return principal plus accrued interest.
    ///
    /// # Errors
    ///
    /// Returns [`ReservePolicyError::Overflow`] if the sum is unrepresentable.
    pub fn total_debt(&self) -> Result<XorQuantity, ReservePolicyError> {
        self.debt_principal
            .checked_add(&self.accrued_interest)
            .map_err(ReservePolicyError::from)
    }

    /// Return unused credit capacity after principal draw.
    ///
    /// # Errors
    ///
    /// Returns an arithmetic error for malformed state.
    pub fn available_credit(&self) -> Result<XorQuantity, ReservePolicyError> {
        capped_sub(&self.credit_cap, &self.debt_principal)
    }

    /// Accrue whole-day simple interest on outstanding principal.
    ///
    /// The anchor advances only by complete elapsed days, so repeated calls in
    /// the same day are idempotent and validator wall-clock precision cannot
    /// change the result.
    ///
    /// # Errors
    ///
    /// Returns an arithmetic error on timestamp or amount overflow.
    pub fn accrue_interest(
        &mut self,
        interest_apr_bps: u16,
        now_unix: u64,
    ) -> Result<XorQuantity, ReservePolicyError> {
        let elapsed = now_unix
            .checked_sub(self.interest_accrued_at_unix)
            .ok_or(ReservePolicyError::Overflow)?;
        let elapsed_days = elapsed / 86_400;
        if elapsed_days == 0 {
            return Ok(XorQuantity::zero());
        }
        let days = u16::try_from(elapsed_days).map_err(|_| ReservePolicyError::Overflow)?;
        let next_anchor = self
            .interest_accrued_at_unix
            .checked_add(
                elapsed_days
                    .checked_mul(86_400)
                    .ok_or(ReservePolicyError::Overflow)?,
            )
            .ok_or(ReservePolicyError::Overflow)?;
        if self.debt_principal.is_zero() {
            self.interest_accrued_at_unix = next_anchor;
            return Ok(XorQuantity::zero());
        }
        let interest = prorated_interest(&self.debt_principal, interest_apr_bps, days)?;
        let next_interest = self.accrued_interest.checked_add(&interest)?;
        self.accrued_interest = next_interest;
        self.interest_accrued_at_unix = next_anchor;
        Ok(interest)
    }
}

/// Reserve custody movement direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum ReserveMovementKindV1 {
    /// Move provider funds into reserve custody.
    TopUp,
    /// Return available reserve funds to the provider.
    Withdrawal,
}

/// Decision lifecycle for a reserve movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum ReserveMovementStatusV1 {
    /// Awaiting a governance decision.
    Pending,
    /// Approved and atomically applied to native custody.
    Approved,
    /// Rejected without custody mutation.
    Rejected,
}

/// Authoritative reserve top-up or withdrawal request and decision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveMovementRecordV1 {
    /// Globally unique movement identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub movement_id: [u8; 32],
    /// Provider reserve partition.
    pub provider_id: ProviderId,
    /// Movement direction.
    pub kind: ReserveMovementKindV1,
    /// Exact custody amount.
    pub amount: XorQuantity,
    /// Provider account that requested the movement.
    pub requested_by: AccountId,
    /// Provider revision on which the request is conditional.
    pub expected_provider_revision: u64,
    /// Policy digest on which the request is conditional.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Decision lifecycle.
    pub status: ReserveMovementStatusV1,
    /// Block timestamp assigned to request admission.
    pub requested_at_unix: u64,
    /// Governance decision account.
    pub decided_by: Option<AccountId>,
    /// Block timestamp assigned to the terminal decision.
    pub decided_at_unix: Option<u64>,
    /// Bounded governance rationale.
    pub rationale: Option<String>,
}

/// Appeal lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum ReserveAppealStatusV1 {
    /// Awaiting governance decision.
    Pending,
    /// Accepted and applied to the provider lifecycle.
    Accepted,
    /// Rejected without provider lifecycle mutation.
    Rejected,
}

/// Authoritative reserve lifecycle appeal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveAppealRecordV1 {
    /// Globally unique appeal identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub appeal_id: [u8; 32],
    /// Appealing provider.
    pub provider_id: ProviderId,
    /// Provider account that submitted the appeal.
    pub submitted_by: AccountId,
    /// Requested lifecycle stage.
    pub requested_stage: ReserveLifecycleStage,
    /// Bounded provider reason.
    pub reason: String,
    /// Optional external evidence digest.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub evidence_digest: Option<[u8; 32]>,
    /// Provider revision on which the appeal is conditional.
    pub expected_provider_revision: u64,
    /// Appeal lifecycle.
    pub status: ReserveAppealStatusV1,
    /// Block timestamp assigned at submission.
    pub submitted_at_unix: u64,
    /// Governance decision account.
    pub decided_by: Option<AccountId>,
    /// Block timestamp assigned at decision.
    pub decided_at_unix: Option<u64>,
    /// Bounded governance rationale.
    pub rationale: Option<String>,
}

/// Finalized block anchor for one coherent reserve-ledger query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same immutable state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// Exclusive cursor for one committed reserve-ledger event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveFinalizedEventCursorV1 {
    /// Monotonic reserve-event sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Finalized block hash resolved only after the block commits.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Reserve-event index within the committing block.
    pub event_index: u32,
}

/// Typed reserve-ledger event with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveFinalizedEventV1 {
    /// Monotonic reserve-event sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash resolved from finalized state.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Reserve-event index within the committing block.
    pub event_index: u32,
    /// Existing typed, payload-free native reserve-ledger event.
    pub event: SorafsReserveLedgerEvent,
}

impl ReserveFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> ReserveFinalizedEventCursorV1 {
        ReserveFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }
}

/// Cursor-bounded page of typed committed reserve-ledger events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReserveFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: ReserveFinalizedCursorV1,
    /// Events in strictly increasing sequence and block/index order.
    pub events: Vec<ReserveFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present only when `has_more` is true.
    pub next_after: Option<ReserveFinalizedEventCursorV1>,
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

fn capped_sub(
    amount: &XorQuantity,
    deduction: &XorQuantity,
) -> Result<XorQuantity, ReservePolicyError> {
    if amount < deduction {
        Ok(XorQuantity::zero())
    } else {
        amount
            .checked_sub(deduction)
            .map_err(ReservePolicyError::from)
    }
}

fn apply_basis_points_u32(
    amount: &XorQuantity,
    basis_points: u32,
) -> Result<XorQuantity, ReservePolicyError> {
    amount
        .checked_mul_basis_points_u32(basis_points)
        .map_err(ReservePolicyError::from)
}

fn divide_amount_by_ratio(
    amount: &XorQuantity,
    ratio_bps: u32,
) -> Result<XorQuantity, ReservePolicyError> {
    let denominator =
        NonZeroU64::new(u64::from(ratio_bps)).ok_or(ReservePolicyError::InvalidRatio {
            field: ReserveRatioField::UnderwritingRatio,
            basis_points: ratio_bps,
        })?;
    amount
        .checked_mul_ratio(u64::from(BASIS_POINTS_PER_UNIT), denominator)
        .map_err(ReservePolicyError::from)
}

fn prorated_interest(
    principal: &XorQuantity,
    apr_bps: u16,
    days: u16,
) -> Result<XorQuantity, ReservePolicyError> {
    if principal.is_zero() || apr_bps == 0 || days == 0 {
        return Ok(XorQuantity::zero());
    }
    let numerator = u64::from(apr_bps)
        .checked_mul(u64::from(days))
        .ok_or(ReservePolicyError::Overflow)?;
    let denominator = NonZeroU64::new(u64::from(BASIS_POINTS_PER_UNIT) * 365)
        .expect("annual basis-point denominator is non-zero");
    principal
        .checked_mul_ratio(numerator, denominator)
        .map_err(ReservePolicyError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_amount_scale_overflow_preserves_diagnostic_context() {
        let error = ReservePolicyError::from(DealAmountError::ScaleOverflow { scale: 10, max: 9 });

        assert_eq!(
            error,
            ReservePolicyError::AmountScaleOverflow { scale: 10, max: 9 }
        );
        assert_eq!(
            error.to_string(),
            "reserve amount scale 10 exceeds maximum 9"
        );
    }

    #[test]
    fn default_policy_renders_expected_quote() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");

        assert_eq!(
            quote
                .monthly_rent
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
        assert_eq!(
            quote
                .reserve_requirement
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            240_000_000
        );
        assert_eq!(
            quote
                .effective_rent
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
        assert_eq!(
            quote
                .top_up_threshold
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            192_000_000
        );
        assert_eq!(
            quote
                .credit_line_cap
                .expect("tier A credit line")
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            240_000_000
        );
        assert_eq!(quote.interest_apr_bps, 300);
    }

    #[test]
    fn reserve_balance_reduces_effective_rent() {
        let policy = ReservePolicyV1::default();
        let balance = XorQuantity::try_from_micro(1_500_000)
            .expect("legacy micro-XOR value is representable"); // 1.5 XOR
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                balance,
            )
            .expect("quote succeeds");

        assert_eq!(
            quote
                .reserve_offset
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            750_000
        );
        assert_eq!(
            quote
                .effective_rent
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            119_250_000
        );
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
                XorQuantity::zero(),
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
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let projection = quote.ledger_projection().expect("ledger projection");
        assert_eq!(
            projection
                .rent_due
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            quote
                .effective_rent
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation")
        );
        assert_eq!(
            projection
                .reserve_shortfall
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            quote
                .reserve_requirement
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation")
        );
        assert_eq!(
            projection
                .top_up_shortfall
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            quote
                .top_up_threshold
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation")
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
                XorQuantity::zero(),
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
        let projection = quote.ledger_projection().expect("ledger projection");
        assert!(projection.reserve_shortfall.is_zero());
        assert!(projection.meets_underwriting);
        assert!(projection.top_up_shortfall.is_zero());
        assert!(!projection.needs_top_up_alert);
    }

    #[test]
    fn lifecycle_projection_warns_before_grace_when_reserve_is_low() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let lifecycle = quote
            .lifecycle_projection(0, 7, 30)
            .expect("lifecycle projection");

        assert_eq!(lifecycle.stage, ReserveLifecycleStage::Warning);
        assert!(lifecycle.restrict_new_manifests);
        assert!(!lifecycle.disable_adverts);
        assert!(lifecycle.credit_draw.is_zero());
    }

    #[test]
    fn lifecycle_projection_draws_credit_during_grace() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let lifecycle = quote
            .lifecycle_projection(3, 7, 30)
            .expect("lifecycle projection");

        assert_eq!(lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(
            lifecycle
                .credit_draw
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
        assert!(lifecycle.credit_shortfall.is_zero());
        assert_eq!(
            lifecycle
                .credit_available_after_draw
                .expect("credit capacity")
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
    }

    #[test]
    fn lifecycle_projection_accrues_interest_after_grace() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Warm,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierB,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let lifecycle = quote
            .lifecycle_projection(12, 7, 30)
            .expect("lifecycle projection");

        assert_eq!(lifecycle.stage, ReserveLifecycleStage::Delinquent);
        assert_eq!(
            lifecycle
                .credit_draw
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            60_000_000
        );
        assert_eq!(
            lifecycle.accrued_interest,
            "0.049315068"
                .parse::<XorQuantity>()
                .expect("rounded nanounit interest is canonical")
        );
        assert_eq!(
            lifecycle.accrued_interest.try_to_micro(),
            Err(DealAmountError::InexactMicroProjection)
        );
        assert_eq!(
            lifecycle.total_due_after_credit,
            "0.049315068"
                .parse::<XorQuantity>()
                .expect("rounded nanounit total is canonical")
        );
        assert!(lifecycle.requires_governance_notification);
    }

    #[test]
    fn lifecycle_projection_defaults_when_credit_cannot_cover_rent() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierC,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let lifecycle = quote
            .lifecycle_projection(1, 7, 30)
            .expect("lifecycle projection");

        assert_eq!(lifecycle.stage, ReserveLifecycleStage::Default);
        assert!(lifecycle.requires_manual_credit_approval);
        assert!(lifecycle.disable_adverts);
        assert_eq!(
            lifecycle
                .credit_shortfall
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
    }

    #[test]
    fn lifecycle_projection_rejects_invalid_windows() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                1,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote succeeds");
        let error = quote
            .lifecycle_projection(0, 30, 30)
            .expect_err("invalid windows should fail");
        assert!(matches!(
            error,
            ReservePolicyError::InvalidLifecycleWindow { .. }
        ));
    }

    fn reserve_account() -> ReserveProviderAccountV1 {
        let provider_account = AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("provider public key"),
        );
        ReserveProviderAccountV1 {
            terms: ReserveProviderTermsV1 {
                provider_id: ProviderId::new([0x31; 32]),
                provider_account,
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 1,
            },
            policy_digest: [0x41; 32],
            revision: 1,
            reserve_balance: XorQuantity::zero(),
            debt_principal: XorQuantity::zero(),
            accrued_interest: XorQuantity::zero(),
            credit_cap: XorQuantity::try_from_micro(1_000_000_000).expect("credit cap fixture"),
            lifecycle_stage: ReserveLifecycleStage::Warning,
            days_past_due: 0,
            pending_movements: 0,
            open_appeals: 0,
            interest_accrued_at_unix: 86_400,
            updated_at_unix: 86_400,
        }
    }

    #[test]
    fn zero_debt_accrual_advances_anchor_before_a_later_draw() {
        let mut account = reserve_account();
        assert!(
            account
                .accrue_interest(10_000, 3 * 86_400)
                .expect("zero-debt accrual")
                .is_zero()
        );
        assert_eq!(account.interest_accrued_at_unix, 3 * 86_400);

        account.debt_principal =
            XorQuantity::try_from_micro(365_000_000).expect("principal fixture");
        let interest = account
            .accrue_interest(10_000, 4 * 86_400)
            .expect("one-day accrual");
        assert_eq!(
            interest
                .try_to_micro()
                .expect("interest has exact micro representation"),
            1_000_000
        );
        assert_eq!(account.interest_accrued_at_unix, 4 * 86_400);
    }

    #[test]
    fn interest_timestamp_rollback_is_rejected_without_mutation() {
        let mut account = reserve_account();
        account.debt_principal =
            XorQuantity::try_from_micro(365_000_000).expect("principal fixture");
        let before = account.clone();

        assert_eq!(
            account
                .accrue_interest(10_000, 86_399)
                .expect_err("timestamp rollback must fail"),
            ReservePolicyError::Overflow
        );
        assert_eq!(account, before);
    }

    #[test]
    fn finalized_reserve_event_page_round_trips_canonically() {
        use crate::events::data::sorafs::SorafsReserveLedgerEventKind;

        let finalized_cursor = ReserveFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        };
        let event = ReserveFinalizedEventV1 {
            sequence: 11,
            block_height: finalized_cursor.height,
            block_hash: finalized_cursor.block_hash,
            event_index: 3,
            event: SorafsReserveLedgerEvent {
                kind: SorafsReserveLedgerEventKind::MovementApproved,
                provider_id: Some(ProviderId::new([0x31; 32])),
                operation_id: Some([0x41; 32]),
                policy_digest: [0x51; 32],
                provider_revision: 9,
                authority: reserve_account().terms.provider_account,
                occurred_at_unix_ms: 12_345,
            },
        };
        let event_cursor = event.cursor();
        let page = ReserveFinalizedEventPageV1 {
            finalized_cursor,
            events: vec![event],
            has_more: true,
            next_after: Some(event_cursor),
        };

        for encoded in [
            norito::to_bytes(&finalized_cursor).expect("encode finalized cursor"),
            norito::to_bytes(&event_cursor).expect("encode event cursor"),
            norito::to_bytes(&page).expect("encode event page"),
        ] {
            assert!(!encoded.is_empty());
        }
        let encoded = norito::to_bytes(&page).expect("encode canonical event page");
        let decoded: ReserveFinalizedEventPageV1 =
            norito::decode_from_bytes(&encoded).expect("decode canonical event page");
        assert_eq!(decoded, page);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical event page"),
            encoded
        );

        #[cfg(feature = "json")]
        {
            let encoded =
                norito::json::to_vec(&page).expect("encode finalized reserve event page JSON");
            let decoded: ReserveFinalizedEventPageV1 = norito::json::from_slice(&encoded)
                .expect("decode finalized reserve event page JSON");
            assert_eq!(decoded, page);
        }
    }
}
