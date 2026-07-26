//! Appeal pricing and settlement utilities for moderation finance workflows
//! (MINFO-7).
//!
//! The congestion-aware deposit formula matches the specification in
//! `docs/source/sorafs_appeal_pricing_plan.md`. The settlement helpers wire in
//! the initial escrow/payout policy so treasury dashboards, CLI tools, and SDKs
//! can deterministically compute refund/slash amounts and panel rewards.

use std::{collections::BTreeMap, fmt, str::FromStr};

use iroha_data_model::account::AccountId;
use iroha_primitives::numeric::{
    Numeric, NumericOperationError, Quantity, RoundingMode, XOR_QUANTITY_SCALE, XorQuantity,
    XorQuantityError,
};
use norito::json::{Map as JsonMap, Value};
use thiserror::Error;

const APPEAL_CALCULATION_SCALE: u32 = 28;

/// Supported appeal classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppealClass {
    /// Content / policy violations.
    Content,
    /// Account access or gating disputes.
    Access,
    /// Fraud or high-risk disputes.
    Fraud,
    /// Fallback bucket for specialised workflows.
    Other,
}

impl AppealClass {
    /// Stable string identifier used in configs and telemetry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Access => "access",
            Self::Fraud => "fraud",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for AppealClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error surfaced when parsing [`AppealClass`] values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unknown appeal class `{raw}` (expected content|access|fraud|other)")]
pub struct AppealClassParseError {
    raw: String,
}

impl FromStr for AppealClass {
    type Err = AppealClassParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "content" => Ok(Self::Content),
            "access" => Ok(Self::Access),
            "fraud" => Ok(Self::Fraud),
            "other" => Ok(Self::Other),
            _ => Err(AppealClassParseError {
                raw: s.trim().to_string(),
            }),
        }
    }
}

/// Urgency hint supplied by moderators when quoting a deposit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppealUrgency {
    /// Standard review path.
    Normal,
    /// Elevated SLA approved by moderators.
    High,
}

impl AppealUrgency {
    /// Stable string identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

impl fmt::Display for AppealUrgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error surfaced when parsing [`AppealUrgency`] values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unknown urgency `{raw}` (expected normal|high)")]
pub struct AppealUrgencyParseError {
    raw: String,
}

impl FromStr for AppealUrgency {
    type Err = AppealUrgencyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            _ => Err(AppealUrgencyParseError {
                raw: s.trim().to_string(),
            }),
        }
    }
}

/// Governance-supplied parameters for a given appeal class.
#[derive(Clone, Debug)]
pub struct AppealClassConfig {
    pub base_rate_xor: Quantity,
    pub backlog_target: u32,
    pub backlog_cap: Numeric,
    pub size_divisor_mb: Numeric,
    pub size_cap: Numeric,
    pub min_deposit_xor: Quantity,
    pub max_deposit_xor: Quantity,
    pub surge_multiplier: Numeric,
}

impl AppealClassConfig {
    /// Construct a class configuration.
    #[must_use]
    pub fn new(
        base_rate_xor: Quantity,
        backlog_target: u32,
        backlog_cap: Numeric,
        size_divisor_mb: Numeric,
        size_cap: Numeric,
        min_deposit_xor: Quantity,
        max_deposit_xor: Quantity,
    ) -> Self {
        Self {
            base_rate_xor,
            backlog_target,
            backlog_cap,
            size_divisor_mb,
            size_cap,
            min_deposit_xor,
            max_deposit_xor,
            surge_multiplier: Numeric::one(),
        }
    }

    /// Apply a surge multiplier override.
    #[must_use]
    pub fn with_surge_multiplier(mut self, multiplier: Numeric) -> Self {
        self.surge_multiplier = multiplier;
        self
    }
}

/// Pricing configuration spanning all appeal classes.
#[derive(Clone, Debug)]
pub struct AppealPricingConfig {
    version: String,
    quote_ttl_secs: u64,
    default_panel_size: u32,
    urgency_normal_multiplier: Numeric,
    urgency_high_multiplier: Numeric,
    classes: BTreeMap<AppealClass, AppealClassConfig>,
}

impl AppealPricingConfig {
    /// Baseline configuration derived from the roadmap specification (rev 2026-03-11).
    /// Governance-managed manifests can be loaded via [`Self::from_manifest_value`].
    #[must_use]
    pub fn baseline_v1() -> Self {
        let mut classes = BTreeMap::new();
        classes.insert(
            AppealClass::Content,
            AppealClassConfig::new(
                Quantity::from(150u32),
                50,
                Numeric::one(),
                Numeric::from(100u32),
                Numeric::from(2u32),
                Quantity::from(100u32),
                Quantity::from(2_500u32),
            ),
        );
        classes.insert(
            AppealClass::Access,
            AppealClassConfig::new(
                Quantity::from(200u32),
                30,
                Numeric::one(),
                Numeric::from(50u32),
                Numeric::from(2u32),
                Quantity::from(100u32),
                Quantity::from(2_500u32),
            ),
        );
        classes.insert(
            AppealClass::Fraud,
            AppealClassConfig::new(
                Quantity::from(500u32),
                20,
                Numeric::one(),
                Numeric::from(50u32),
                Numeric::from(2u32),
                Quantity::from(100u32),
                Quantity::from(5_000u32),
            ),
        );
        classes.insert(
            AppealClass::Other,
            AppealClassConfig::new(
                Quantity::from(120u32),
                40,
                Numeric::one(),
                Numeric::from(100u32),
                Numeric::from(2u32),
                Quantity::from(100u32),
                Quantity::from(2_500u32),
            ),
        );

        Self {
            version: "baseline-v1".to_string(),
            quote_ttl_secs: 15 * 60,
            default_panel_size: 7,
            urgency_normal_multiplier: Numeric::one(),
            urgency_high_multiplier: "1.2".parse().expect("canonical baseline multiplier"),
            classes,
        }
    }

    /// Access the configured version label.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Validity window for quotes (seconds).
    #[must_use]
    pub fn quote_ttl_secs(&self) -> u64 {
        self.quote_ttl_secs
    }

    /// Default panel size used for `panel_multiplier` when callers omit overrides.
    #[must_use]
    pub fn default_panel_size(&self) -> u32 {
        self.default_panel_size
    }

    /// Construct a configuration from a governance-managed JSON manifest.
    pub fn from_manifest_value(manifest: &Value) -> Result<Self, AppealPricingManifestError> {
        let root = manifest.as_object().ok_or_else(|| {
            AppealPricingManifestError::new("appeal pricing manifest must be a JSON object")
        })?;
        let version = require_string_field(root, "version")?.to_string();
        let quote_ttl_secs = parse_u64_field(root, "quote_ttl_secs")?;
        let default_panel_size = parse_u32_field(root, "default_panel_size")?;
        if default_panel_size == 0 {
            return Err(AppealPricingManifestError::new(
                "`default_panel_size` must be greater than zero",
            ));
        }

        let urgency_obj = require_object_field(root, "urgency_multipliers")?;
        let urgency_normal =
            parse_numeric_from_map(urgency_obj, "normal", "urgency_multipliers.normal")?;
        if urgency_normal <= Numeric::zero() {
            return Err(AppealPricingManifestError::new(
                "`urgency_multipliers.normal` must be greater than zero",
            ));
        }
        let urgency_high = parse_numeric_from_map(urgency_obj, "high", "urgency_multipliers.high")?;
        if urgency_high <= Numeric::zero() {
            return Err(AppealPricingManifestError::new(
                "`urgency_multipliers.high` must be greater than zero",
            ));
        }

        let classes_obj = require_object_field(root, "classes")?;
        if classes_obj.is_empty() {
            return Err(AppealPricingManifestError::new(
                "`classes` must contain at least one entry",
            ));
        }

        let mut classes = BTreeMap::new();
        for (class_label, entry) in classes_obj {
            let class = class_label.parse::<AppealClass>().map_err(|err| {
                AppealPricingManifestError::new(format!(
                    "unknown appeal class `{}` in manifest: {err}",
                    class_label
                ))
            })?;
            let class_obj = entry.as_object().ok_or_else(|| {
                AppealPricingManifestError::new(format!(
                    "`classes.{class_label}` must be an object"
                ))
            })?;
            let base_rate = parse_quantity_from_map(
                class_obj,
                "base_rate_xor",
                &format!("classes.{class_label}.base_rate_xor"),
            )?;
            if base_rate.is_zero() {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.base_rate_xor` must be greater than zero"
                )));
            }
            let backlog_target = parse_u32_field(class_obj, "backlog_target")?;
            if backlog_target == 0 {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.backlog_target` must be greater than zero"
                )));
            }
            let backlog_cap = parse_numeric_from_map(
                class_obj,
                "backlog_cap",
                &format!("classes.{class_label}.backlog_cap"),
            )?;
            if backlog_cap < Numeric::zero() {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.backlog_cap` must not be negative"
                )));
            }
            let size_divisor = parse_numeric_from_map(
                class_obj,
                "size_divisor_mb",
                &format!("classes.{class_label}.size_divisor_mb"),
            )?;
            if size_divisor <= Numeric::zero() {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.size_divisor_mb` must be greater than zero"
                )));
            }
            let size_cap = parse_numeric_from_map(
                class_obj,
                "size_cap",
                &format!("classes.{class_label}.size_cap"),
            )?;
            if size_cap < Numeric::zero() {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.size_cap` must not be negative"
                )));
            }
            let min_deposit = parse_quantity_from_map(
                class_obj,
                "min_deposit_xor",
                &format!("classes.{class_label}.min_deposit_xor"),
            )?;
            let max_deposit = parse_quantity_from_map(
                class_obj,
                "max_deposit_xor",
                &format!("classes.{class_label}.max_deposit_xor"),
            )?;
            if max_deposit.is_zero() {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.max_deposit_xor` must be greater than zero"
                )));
            }
            if max_deposit < min_deposit {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.max_deposit_xor` must be >= min deposit"
                )));
            }
            let surge_multiplier = if let Some(value) = class_obj.get("surge_multiplier") {
                let parsed =
                    parse_numeric_value(value, &format!("classes.{class_label}.surge_multiplier"))?;
                if parsed <= Numeric::zero() {
                    return Err(AppealPricingManifestError::new(format!(
                        "`classes.{class_label}.surge_multiplier` must be greater than zero"
                    )));
                }
                parsed
            } else {
                Numeric::one()
            };

            let config = AppealClassConfig::new(
                base_rate,
                backlog_target,
                backlog_cap,
                size_divisor,
                size_cap,
                min_deposit,
                max_deposit,
            )
            .with_surge_multiplier(surge_multiplier);
            classes.insert(class, config);
        }

        Ok(Self {
            version,
            quote_ttl_secs,
            default_panel_size,
            urgency_normal_multiplier: urgency_normal,
            urgency_high_multiplier: urgency_high,
            classes,
        })
    }

    /// Borrow a class configuration.
    #[must_use]
    pub fn class_config(&self, class: AppealClass) -> Option<&AppealClassConfig> {
        self.classes.get(&class)
    }

    /// Quote the required deposit for `input`.
    ///
    /// # Errors
    ///
    /// Returns [`AppealPricingError`] when the configuration lacks the provided class,
    /// the targets are misconfigured, or the inputs are invalid.
    pub fn quote(&self, input: AppealQuoteInput) -> Result<AppealQuote, AppealPricingError> {
        if input.panel_size == 0 {
            return Err(AppealPricingError::InvalidPanelSize);
        }
        if self.default_panel_size == 0 {
            return Err(AppealPricingError::InvalidDefaultPanelSize);
        }

        let class_cfg = self
            .class_config(input.class)
            .ok_or(AppealPricingError::MissingClassConfig { class: input.class })?;
        for (field, amount) in [
            ("base_rate_xor", &class_cfg.base_rate_xor),
            ("min_deposit_xor", &class_cfg.min_deposit_xor),
            ("max_deposit_xor", &class_cfg.max_deposit_xor),
        ] {
            XorQuantity::try_from_quantity(amount.clone()).map_err(|reason| {
                AppealPricingError::InvalidXorQuantity {
                    class: input.class,
                    field,
                    reason,
                }
            })?;
        }

        if class_cfg.backlog_target == 0 {
            return Err(AppealPricingError::InvalidBacklogTarget { class: input.class });
        }
        if class_cfg.size_divisor_mb.is_zero() {
            return Err(AppealPricingError::InvalidSizeDivisor { class: input.class });
        }

        let backlog_factor = {
            let target = Numeric::from(class_cfg.backlog_target);
            let ratio = Numeric::from(input.backlog).try_decimal_div_round(
                &target,
                APPEAL_CALCULATION_SCALE,
                RoundingMode::NearestEven,
            )?;
            clamp_numeric(ratio, Numeric::zero(), class_cfg.backlog_cap.clone())
        };
        let size_multiplier = {
            let ratio = Numeric::from(input.evidence_size_mb).try_decimal_div_round(
                &class_cfg.size_divisor_mb,
                APPEAL_CALCULATION_SCALE,
                RoundingMode::NearestEven,
            )?;
            Numeric::one().try_decimal_add(&clamp_numeric(
                ratio,
                Numeric::zero(),
                class_cfg.size_cap.clone(),
            ))?
        };
        let urgency_multiplier = match input.urgency {
            AppealUrgency::Normal => &self.urgency_normal_multiplier,
            AppealUrgency::High => &self.urgency_high_multiplier,
        };
        let panel_multiplier = Numeric::from(input.panel_size).try_decimal_div_round(
            &Numeric::from(self.default_panel_size),
            APPEAL_CALCULATION_SCALE,
            RoundingMode::NearestEven,
        )?;

        let backlog_multiplier = Numeric::one().try_decimal_add(&backlog_factor)?;
        let raw = class_cfg.base_rate_xor.try_product_decimals_round(
            [
                &backlog_multiplier,
                &size_multiplier,
                urgency_multiplier,
                &panel_multiplier,
                &class_cfg.surge_multiplier,
            ],
            XOR_QUANTITY_SCALE,
            RoundingMode::NearestEven,
        )?;
        let clamped = clamp_quantity(
            raw.clone(),
            class_cfg.min_deposit_xor.clone(),
            class_cfg.max_deposit_xor.clone(),
        );

        Ok(AppealQuote {
            deposit_xor: clamped,
            breakdown: AppealQuoteBreakdown {
                base_rate_xor: class_cfg.base_rate_xor.clone(),
                backlog_factor,
                size_multiplier,
                urgency_multiplier: urgency_multiplier.clone(),
                panel_multiplier,
                surge_multiplier: class_cfg.surge_multiplier.clone(),
                raw_deposit_xor: raw,
                min_deposit_xor: class_cfg.min_deposit_xor.clone(),
                max_deposit_xor: class_cfg.max_deposit_xor.clone(),
            },
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AppealQuoteInput {
    pub class: AppealClass,
    pub backlog: u32,
    pub evidence_size_mb: u32,
    pub urgency: AppealUrgency,
    pub panel_size: u32,
}

/// Detailed multiplier breakdown for diagnostics.
#[derive(Clone, Debug)]
pub struct AppealQuoteBreakdown {
    pub base_rate_xor: Quantity,
    pub backlog_factor: Numeric,
    pub size_multiplier: Numeric,
    pub urgency_multiplier: Numeric,
    pub panel_multiplier: Numeric,
    pub surge_multiplier: Numeric,
    pub raw_deposit_xor: Quantity,
    pub min_deposit_xor: Quantity,
    pub max_deposit_xor: Quantity,
}

/// Quote output.
#[derive(Clone, Debug)]
pub struct AppealQuote {
    pub deposit_xor: Quantity,
    pub breakdown: AppealQuoteBreakdown,
}

/// Settlement disposition for a resolved appeal deposit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppealVerdict {
    /// Panel decision describing the outcome of the moderation case.
    Decision(AppealDecision),
    /// Appeal withdrawn before the panel started deliberations.
    WithdrawnBeforePanel,
    /// Appeal withdrawn after jurors were seated / deliberation started.
    WithdrawnAfterPanel,
    /// Marked as frivolous by the moderation service.
    Frivolous,
    /// Escalated / pending follow-up, funds remain in escrow.
    Escalated,
}

/// Panel decision outcome as described by the moderation roadmap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppealDecision {
    Uphold,
    Overturn,
    Modify,
}

impl AppealDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uphold => "uphold",
            Self::Overturn => "overturn",
            Self::Modify => "modify",
        }
    }
}

impl fmt::Display for AppealDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing [`AppealDecision`] values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unknown decision `{raw}` (expected uphold|overturn|modify)")]
pub struct AppealDecisionParseError {
    raw: String,
}

impl FromStr for AppealDecision {
    type Err = AppealDecisionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "uphold" => Ok(Self::Uphold),
            "overturn" => Ok(Self::Overturn),
            "modify" => Ok(Self::Modify),
            _ => Err(AppealDecisionParseError {
                raw: s.trim().to_string(),
            }),
        }
    }
}

/// Error returned when parsing [`AppealVerdict`] values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error(
    "unknown appeal verdict `{raw}` (expected uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated)"
)]
pub struct AppealVerdictParseError {
    raw: String,
}

impl FromStr for AppealVerdict {
    type Err = AppealVerdictParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "uphold" => Ok(Self::Decision(AppealDecision::Uphold)),
            "overturn" => Ok(Self::Decision(AppealDecision::Overturn)),
            "modify" => Ok(Self::Decision(AppealDecision::Modify)),
            "withdrawn_before_panel"
            | "withdrawn-before-panel"
            | "withdrawn_pre"
            | "withdrawn-pre" => Ok(Self::WithdrawnBeforePanel),
            "withdrawn_after_panel"
            | "withdrawn-after-panel"
            | "withdrawn_post"
            | "withdrawn-post" => Ok(Self::WithdrawnAfterPanel),
            "frivolous" => Ok(Self::Frivolous),
            "escalated" | "pending" => Ok(Self::Escalated),
            _ => Err(AppealVerdictParseError {
                raw: s.trim().to_string(),
            }),
        }
    }
}

/// Mapping of refund/slash ratios for a particular verdict.
#[derive(Clone, Debug)]
pub struct AppealSettlementRule {
    refund_rate: Numeric,
    treasury_rate: Numeric,
}

impl AppealSettlementRule {
    fn new(
        refund_rate: Numeric,
        treasury_rate: Numeric,
    ) -> Result<Self, AppealSettlementManifestError> {
        if refund_rate < Numeric::zero() || refund_rate > Numeric::one() {
            return Err(AppealSettlementManifestError::new(
                "`refund_rate` must be between 0 and 1",
            ));
        }
        if treasury_rate < Numeric::zero() || treasury_rate > Numeric::one() {
            return Err(AppealSettlementManifestError::new(
                "`treasury_rate` must be between 0 and 1",
            ));
        }
        if refund_rate
            .try_decimal_add(&treasury_rate)
            .map_err(AppealSettlementManifestError::arithmetic)?
            > Numeric::one()
        {
            return Err(AppealSettlementManifestError::new(
                "refund_rate + treasury_rate must not exceed 1",
            ));
        }
        Ok(Self {
            refund_rate,
            treasury_rate,
        })
    }

    fn refund_component(&self, deposit: &Quantity) -> Result<Quantity, NumericOperationError> {
        deposit.try_product_decimals_round(
            [&self.refund_rate],
            XOR_QUANTITY_SCALE,
            RoundingMode::TowardZero,
        )
    }

    fn treasury_component(&self, deposit: &Quantity) -> Result<Quantity, NumericOperationError> {
        deposit.try_product_decimals_round(
            [&self.treasury_rate],
            XOR_QUANTITY_SCALE,
            RoundingMode::TowardZero,
        )
    }
}

impl fmt::Display for AppealVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decision(decision) => write!(f, "{}", decision.as_str()),
            Self::WithdrawnBeforePanel => f.write_str("withdrawn_before_panel"),
            Self::WithdrawnAfterPanel => f.write_str("withdrawn_after_panel"),
            Self::Frivolous => f.write_str("frivolous"),
            Self::Escalated => f.write_str("escalated"),
        }
    }
}

/// Per-panel reward configuration used to calculate juror stipends.
#[derive(Clone, Debug)]
pub struct PanelRewardConfig {
    stipend_per_juror_xor: Quantity,
    case_bonus_xor: Quantity,
}

impl PanelRewardConfig {
    #[must_use]
    pub fn new(stipend_per_juror_xor: Quantity, case_bonus_xor: Quantity) -> Self {
        Self {
            stipend_per_juror_xor,
            case_bonus_xor,
        }
    }

    /// Reward per juror for a single case.
    #[must_use]
    pub fn stipend_per_juror(&self) -> &Quantity {
        &self.stipend_per_juror_xor
    }

    /// Case-level bonus paid out once per case.
    #[must_use]
    pub fn case_bonus(&self) -> &Quantity {
        &self.case_bonus_xor
    }

    /// Total reward for a panel size.
    pub fn total_reward(&self, panel_size: u32) -> Result<Quantity, NumericOperationError> {
        if panel_size == 0 {
            return Ok(Quantity::zero());
        }
        self.stipend_per_juror_xor
            .try_mul_decimal(&Numeric::from(panel_size))?
            .try_add(&self.case_bonus_xor)
    }
}

/// Settlement configuration sourced from governance manifests.
#[derive(Clone, Debug)]
pub struct AppealSettlementConfig {
    version: String,
    default_panel_size: u32,
    panel_rewards: PanelRewardConfig,
    decision_rules: BTreeMap<AppealDecision, AppealSettlementRule>,
    withdrawn_before_panel: AppealSettlementRule,
    withdrawn_after_panel: AppealSettlementRule,
    frivolous: AppealSettlementRule,
    escalated: AppealSettlementRule,
}

impl AppealSettlementConfig {
    /// Baseline configuration derived from the moderation finance roadmap.
    #[must_use]
    pub fn baseline_v1() -> Self {
        let mut decision_rules = BTreeMap::new();
        decision_rules.insert(
            AppealDecision::Overturn,
            AppealSettlementRule::new(Numeric::one(), Numeric::zero()).expect("valid rule"),
        );
        decision_rules.insert(
            AppealDecision::Modify,
            AppealSettlementRule::new(Numeric::one(), Numeric::zero()).expect("valid rule"),
        );
        decision_rules.insert(
            AppealDecision::Uphold,
            AppealSettlementRule::new(Numeric::zero(), Numeric::one()).expect("valid rule"),
        );
        Self {
            version: "baseline-v1".to_string(),
            default_panel_size: 7,
            panel_rewards: PanelRewardConfig::new(Quantity::from(25u32), Quantity::from(10u32)),
            decision_rules,
            withdrawn_before_panel: AppealSettlementRule::new(
                "0.9".parse().expect("canonical baseline rate"),
                Numeric::zero(),
            )
            .expect("valid rule"),
            withdrawn_after_panel: AppealSettlementRule::new(Numeric::zero(), Numeric::one())
                .expect("valid rule"),
            frivolous: AppealSettlementRule::new(
                "0.5".parse().expect("canonical baseline rate"),
                "0.5".parse().expect("canonical baseline rate"),
            )
            .expect("valid rule"),
            escalated: AppealSettlementRule::new(Numeric::zero(), Numeric::zero())
                .expect("valid rule"),
        }
    }

    /// Load configuration from a governance-managed JSON manifest.
    pub fn from_manifest_value(manifest: &Value) -> Result<Self, AppealSettlementManifestError> {
        let root = manifest.as_object().ok_or_else(|| {
            AppealSettlementManifestError::new("appeal settlement manifest must be a JSON object")
        })?;
        let version = require_string_field_settlement(root, "version")?.to_string();
        let default_panel_size = parse_u32_field_settlement(root, "default_panel_size")?;
        if default_panel_size == 0 {
            return Err(AppealSettlementManifestError::new(
                "`default_panel_size` must be greater than zero",
            ));
        }

        let panel_obj = require_object_field_settlement(root, "panel_rewards")?;
        let stipend_per_juror = parse_quantity_from_map_settlement(
            panel_obj,
            "stipend_per_juror_xor",
            "panel_rewards.stipend_per_juror_xor",
        )?;
        let case_bonus = parse_quantity_from_map_settlement(
            panel_obj,
            "case_bonus_xor",
            "panel_rewards.case_bonus_xor",
        )?;
        let rules_obj = require_object_field_settlement(root, "rules")?;
        let decisions_obj = require_object_field_settlement(rules_obj, "decisions")?;
        if decisions_obj.is_empty() {
            return Err(AppealSettlementManifestError::new(
                "`rules.decisions` must contain at least one entry",
            ));
        }
        let mut decision_rules = BTreeMap::new();
        for (label, rule_value) in decisions_obj {
            let decision = label.parse::<AppealDecision>().map_err(|_| {
                AppealSettlementManifestError::new(format!(
                    "unknown decision `{label}` in rules.decisions"
                ))
            })?;
            decision_rules.insert(
                decision,
                parse_settlement_rule(rule_value, &format!("rules.decisions.{label}"))?,
            );
        }
        ensure_decision_rule(&decision_rules, AppealDecision::Uphold)?;
        ensure_decision_rule(&decision_rules, AppealDecision::Overturn)?;
        ensure_decision_rule(&decision_rules, AppealDecision::Modify)?;

        Ok(Self {
            version,
            default_panel_size,
            panel_rewards: PanelRewardConfig::new(stipend_per_juror, case_bonus),
            decision_rules,
            withdrawn_before_panel: parse_required_rule(rules_obj, "withdrawn_before_panel")?,
            withdrawn_after_panel: parse_required_rule(rules_obj, "withdrawn_after_panel")?,
            frivolous: parse_required_rule(rules_obj, "frivolous")?,
            escalated: parse_required_rule(rules_obj, "escalated")?,
        })
    }

    /// Human-readable version label.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Default panel size used for CLI helpers.
    #[must_use]
    pub fn default_panel_size(&self) -> u32 {
        self.default_panel_size
    }

    /// Panel reward configuration.
    #[must_use]
    pub fn panel_rewards(&self) -> &PanelRewardConfig {
        &self.panel_rewards
    }

    /// Compute the settlement breakdown for a deposit.
    pub fn settle(
        &self,
        deposit_xor: Quantity,
        panel_size: u32,
        verdict: AppealVerdict,
    ) -> Result<AppealSettlementBreakdown, AppealSettlementError> {
        if panel_size == 0 {
            return Err(AppealSettlementError::InvalidPanelSize);
        }
        for (field, amount) in [
            ("deposit_xor", &deposit_xor),
            (
                "panel_rewards.stipend_per_juror_xor",
                self.panel_rewards.stipend_per_juror(),
            ),
            (
                "panel_rewards.case_bonus_xor",
                self.panel_rewards.case_bonus(),
            ),
        ] {
            XorQuantity::try_from_quantity(amount.clone())
                .map_err(|reason| AppealSettlementError::InvalidXorQuantity { field, reason })?;
        }
        let rule = match verdict {
            AppealVerdict::Decision(decision) => self
                .decision_rules
                .get(&decision)
                .ok_or(AppealSettlementError::MissingDecisionRule { decision })?,
            AppealVerdict::WithdrawnBeforePanel => &self.withdrawn_before_panel,
            AppealVerdict::WithdrawnAfterPanel => &self.withdrawn_after_panel,
            AppealVerdict::Frivolous => &self.frivolous,
            AppealVerdict::Escalated => &self.escalated,
        };
        let refund = rule.refund_component(&deposit_xor)?;
        let treasury = rule.treasury_component(&deposit_xor)?;
        let held = deposit_xor.try_sub(&refund)?.try_sub(&treasury)?;
        let panel_reward_total = self.panel_rewards.total_reward(panel_size)?;
        Ok(AppealSettlementBreakdown {
            refund_xor: refund,
            treasury_xor: treasury,
            held_xor: held,
            panel_reward_per_juror_xor: self.panel_rewards.stipend_per_juror().clone(),
            panel_reward_total_xor: panel_reward_total,
        })
    }

    /// Compute the full disbursement plan, including deposit flows and panel rewards.
    pub fn disburse(
        &self,
        input: AppealDisbursementInput<'_>,
    ) -> Result<AppealDisbursementPlan, AppealDisbursementError> {
        let settlement = self.settle(input.deposit_xor.clone(), input.panel_size, input.verdict)?;
        if input.jurors.is_empty() {
            return Err(AppealDisbursementError::NoJurorsProvided);
        }

        let panel_size_usize = usize::try_from(input.panel_size).map_err(|_| {
            AppealDisbursementError::PanelSizeOverflow {
                provided: input.panel_size as usize,
            }
        })?;
        if input.jurors.len() != panel_size_usize {
            return Err(AppealDisbursementError::PanelSizeMismatch {
                expected: input.panel_size,
                provided: input.jurors.len(),
            });
        }

        let mut seen: BTreeMap<AccountId, ()> = BTreeMap::new();
        for juror in input.jurors {
            if seen.insert(juror.clone(), ()).is_some() {
                return Err(AppealDisbursementError::DuplicateJuror(juror.clone()));
            }
        }

        let mut no_show_set: BTreeMap<AccountId, ()> = BTreeMap::new();
        for account in input.no_shows {
            if !seen.contains_key(account) {
                return Err(AppealDisbursementError::NoShowNotInPanel(account.clone()));
            }
            if no_show_set.insert(account.clone(), ()).is_some() {
                return Err(AppealDisbursementError::DuplicateNoShow(account.clone()));
            }
        }

        let attending: Vec<AccountId> = input
            .jurors
            .iter()
            .filter(|juror| !no_show_set.contains_key(*juror))
            .cloned()
            .collect();
        if attending.is_empty() {
            return Err(AppealDisbursementError::NoAttendingJurors);
        }
        let attending_count_u32 = u32::try_from(attending.len()).map_err(|_| {
            AppealDisbursementError::PanelSizeOverflow {
                provided: attending.len(),
            }
        })?;
        let attending_count = Numeric::from(attending_count_u32);

        let stipend = self.panel_rewards.stipend_per_juror();
        let bonus = self.panel_rewards.case_bonus();
        // Round toward zero so aggregate juror payouts can never exceed the
        // configured reward pool. The exact remainder is sent to treasury.
        let bonus_share = bonus
            .try_div_decimal_round(
                &attending_count,
                XOR_QUANTITY_SCALE,
                RoundingMode::TowardZero,
            )
            .map_err(AppealDisbursementError::Arithmetic)?;

        let mut juror_payouts = Vec::with_capacity(attending.len());
        for juror in &attending {
            juror_payouts.push(JurorPayout {
                juror: juror.clone(),
                stipend_xor: stipend.clone(),
                bonus_xor: bonus_share.clone(),
            });
        }

        let payout_per_juror = stipend.try_add(&bonus_share)?;
        let rewards_paid_total_xor =
            payout_per_juror.try_mul_decimal(&Numeric::from(attending_count_u32))?;
        let rewards_available_xor = settlement.panel_reward_total_xor.clone();
        let rewards_forfeited_treasury_xor = rewards_available_xor
            .try_sub(&rewards_paid_total_xor)
            .map_err(AppealDisbursementError::Arithmetic)?;
        let total_treasury_xor = settlement
            .treasury_xor
            .try_add(&rewards_forfeited_treasury_xor)?;

        Ok(AppealDisbursementPlan {
            deposit_xor: input.deposit_xor,
            verdict: input.verdict,
            panel_size: input.panel_size,
            settlement,
            refund_account: input.refund_account.clone(),
            treasury_account: input.treasury_account.clone(),
            escrow_account: input.escrow_account.clone(),
            no_show_accounts: no_show_set.keys().cloned().collect(),
            juror_payouts,
            rewards_available_xor,
            rewards_paid_total_xor,
            rewards_forfeited_treasury_xor,
            total_treasury_xor,
        })
    }
}

fn ensure_decision_rule(
    rules: &BTreeMap<AppealDecision, AppealSettlementRule>,
    decision: AppealDecision,
) -> Result<(), AppealSettlementManifestError> {
    if rules.contains_key(&decision) {
        Ok(())
    } else {
        Err(AppealSettlementManifestError::new(format!(
            "missing `{}` rule in rules.decisions",
            decision.as_str()
        )))
    }
}

fn parse_settlement_rule(
    value: &Value,
    label: &str,
) -> Result<AppealSettlementRule, AppealSettlementManifestError> {
    let obj = value.as_object().ok_or_else(|| {
        AppealSettlementManifestError::new(format!("`{label}` must be an object"))
    })?;
    let refund =
        parse_numeric_from_map_settlement(obj, "refund_rate", &format!("{label}.refund_rate"))?;
    let treasury =
        parse_numeric_from_map_settlement(obj, "treasury_rate", &format!("{label}.treasury_rate"))?;
    AppealSettlementRule::new(refund, treasury)
}

fn parse_required_rule(
    rules_obj: &JsonMap,
    key: &'static str,
) -> Result<AppealSettlementRule, AppealSettlementManifestError> {
    let value = rules_obj.get(key).ok_or_else(|| {
        AppealSettlementManifestError::new(format!("missing `rules.{key}` in manifest"))
    })?;
    parse_settlement_rule(value, &format!("rules.{key}"))
}

/// Resulting settlement breakdown for treasury tooling / dashboards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppealSettlementBreakdown {
    pub refund_xor: Quantity,
    pub treasury_xor: Quantity,
    pub held_xor: Quantity,
    pub panel_reward_per_juror_xor: Quantity,
    pub panel_reward_total_xor: Quantity,
}

/// Inputs required to derive per-account disbursement flows.
pub struct AppealDisbursementInput<'a> {
    pub deposit_xor: Quantity,
    pub panel_size: u32,
    pub verdict: AppealVerdict,
    pub jurors: &'a [AccountId],
    pub no_shows: &'a [AccountId],
    pub refund_account: &'a AccountId,
    pub treasury_account: &'a AccountId,
    pub escrow_account: &'a AccountId,
}

/// Per-juror payout detail (stipend + bonus share).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JurorPayout {
    pub juror: AccountId,
    pub stipend_xor: Quantity,
    pub bonus_xor: Quantity,
}

impl JurorPayout {
    /// Total payout for the juror.
    pub fn total(&self) -> Result<Quantity, NumericOperationError> {
        self.stipend_xor.try_add(&self.bonus_xor)
    }
}

/// Deterministic disbursement plan combining deposit settlement and panel rewards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppealDisbursementPlan {
    pub deposit_xor: Quantity,
    pub verdict: AppealVerdict,
    pub panel_size: u32,
    pub settlement: AppealSettlementBreakdown,
    pub refund_account: AccountId,
    pub treasury_account: AccountId,
    pub escrow_account: AccountId,
    pub no_show_accounts: Vec<AccountId>,
    pub juror_payouts: Vec<JurorPayout>,
    pub rewards_available_xor: Quantity,
    pub rewards_paid_total_xor: Quantity,
    pub rewards_forfeited_treasury_xor: Quantity,
    pub total_treasury_xor: Quantity,
}

impl AppealDisbursementPlan {
    /// Number of jurors that receive payouts.
    #[must_use]
    pub fn attending_count(&self) -> usize {
        self.juror_payouts.len()
    }
}

/// Errors surfaced when computing disbursement plans.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppealDisbursementError {
    /// Settlement inputs were invalid.
    #[error("{0}")]
    Settlement(#[from] AppealSettlementError),
    /// No jurors were provided.
    #[error("at least one juror must be supplied")]
    NoJurorsProvided,
    /// Juror roster exceeds supported size.
    #[error("panel size `{provided}` exceeds supported range")]
    PanelSizeOverflow { provided: usize },
    /// Juror list does not match the declared panel size.
    #[error("panel size mismatch: expected {expected}, got {provided}")]
    PanelSizeMismatch { expected: u32, provided: usize },
    /// Juror list contained duplicates.
    #[error("duplicate juror entry for {0}")]
    DuplicateJuror(AccountId),
    /// No-show list contained duplicates.
    #[error("duplicate no-show entry for {0}")]
    DuplicateNoShow(AccountId),
    /// No-show entry not present in the juror roster.
    #[error("no-show `{0}` is not part of the juror roster")]
    NoShowNotInPanel(AccountId),
    /// All jurors were marked as no-shows.
    #[error("no attending jurors; at least one juror must participate to disburse rewards")]
    NoAttendingJurors,
    /// Bounded decimal arithmetic failed.
    #[error("appeal disbursement arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
}

/// Errors produced by the pricing engine.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppealPricingError {
    /// The requested class has no configuration entry.
    #[error("no pricing configuration registered for {class}")]
    MissingClassConfig { class: AppealClass },
    /// The baseline panel size is zero.
    #[error("default panel size must be greater than zero")]
    InvalidDefaultPanelSize,
    /// The caller provided `panel_size = 0`.
    #[error("panel size must be greater than zero")]
    InvalidPanelSize,
    /// The backlog target is misconfigured.
    #[error("backlog target for {class} must be greater than zero")]
    InvalidBacklogTarget { class: AppealClass },
    /// Size divisor misconfiguration.
    #[error("size divisor for {class} must be greater than zero")]
    InvalidSizeDivisor { class: AppealClass },
    /// A configured monetary amount violates XOR's canonical precision.
    #[error("invalid `{field}` XOR quantity for {class}: {reason}")]
    InvalidXorQuantity {
        /// Appeal class containing the invalid amount.
        class: AppealClass,
        /// Stable configuration field label.
        field: &'static str,
        /// Nominal XOR-domain validation failure.
        reason: XorQuantityError,
    },
    /// Bounded decimal arithmetic failed.
    #[error("appeal pricing arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
}

/// Errors surfaced when computing settlement outcomes.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppealSettlementError {
    /// Panel size must be greater than zero.
    #[error("panel size must be greater than zero")]
    InvalidPanelSize,
    /// No rule was configured for the supplied decision.
    #[error("no settlement rule registered for {decision:?}")]
    MissingDecisionRule { decision: AppealDecision },
    /// A monetary input or configured reward violates XOR precision.
    #[error("invalid `{field}` XOR quantity: {reason}")]
    InvalidXorQuantity {
        /// Stable input or configuration field label.
        field: &'static str,
        /// Nominal XOR-domain validation failure.
        reason: XorQuantityError,
    },
    /// Bounded decimal arithmetic failed.
    #[error("appeal settlement arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
}

fn clamp_numeric(value: Numeric, min: Numeric, max: Numeric) -> Numeric {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn clamp_quantity(value: Quantity, min: Quantity, max: Quantity) -> Quantity {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Error returned when parsing a canonical XOR quantity literal.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("failed to parse `{label}` quantity `{raw}`: {reason}")]
pub struct AppealQuantityParseError {
    label: String,
    raw: String,
    reason: String,
}

/// Parse a user-supplied canonical XOR quantity without JSON number heuristics.
pub fn parse_appeal_quantity_literal(
    label: impl Into<String>,
    raw: &str,
) -> Result<Quantity, AppealQuantityParseError> {
    let label = label.into();
    if raw != raw.trim() {
        return Err(AppealQuantityParseError {
            label,
            raw: raw.to_owned(),
            reason: "quantity must not contain surrounding whitespace".to_owned(),
        });
    }
    let parsed = Quantity::from_str(raw).map_err(|err| AppealQuantityParseError {
        label: label.clone(),
        raw: raw.to_string(),
        reason: err.to_string(),
    })?;
    if parsed.to_string() != raw {
        return Err(AppealQuantityParseError {
            label,
            raw: raw.to_string(),
            reason: "quantity must use its canonical decimal spelling".to_owned(),
        });
    }
    XorQuantity::try_from_quantity(parsed.clone()).map_err(|err| AppealQuantityParseError {
        label,
        raw: raw.to_string(),
        reason: err.to_string(),
    })?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::domain::DomainId;

    use super::*;

    fn make_account(label: u8, _domain: &DomainId) -> AccountId {
        let seed = [label; ed25519_dalek::SECRET_KEY_LENGTH];
        let signer = SigningKey::from_bytes(&seed);
        let pk_bytes = signer.verifying_key().to_bytes();
        let pk =
            PublicKey::from_bytes(Algorithm::Ed25519, pk_bytes.as_slice()).expect("public key");
        AccountId::new(pk)
    }

    #[test]
    fn baseline_content_quote_matches_spec() {
        let config = AppealPricingConfig::baseline_v1();
        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Content,
                backlog: 28,
                evidence_size_mb: 45,
                urgency: AppealUrgency::Normal,
                panel_size: config.default_panel_size(),
            })
            .expect("quote must succeed");
        assert_eq!(
            quote.deposit_xor,
            "339.3".parse::<Quantity>().expect("canonical quantity"),
            "expected 339.3 XOR"
        );
    }

    #[test]
    fn baseline_quote_rounds_the_aggregate_product_once() {
        let config = AppealPricingConfig::baseline_v1();
        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Content,
                backlog: 3,
                evidence_size_mb: 8,
                urgency: AppealUrgency::High,
                panel_size: 5,
            })
            .expect("repeating panel ratio must produce a bounded deterministic quote");
        assert_eq!(quote.deposit_xor.to_string(), "147.188571429");
        assert_eq!(quote.breakdown.raw_deposit_xor, quote.deposit_xor);
        XorQuantity::try_from_quantity(quote.deposit_xor)
            .expect("quoted deposit must fit the canonical nano-XOR domain");
    }

    #[test]
    fn fraud_quote_respects_maximum() {
        let config = AppealPricingConfig::baseline_v1();
        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Fraud,
                backlog: 200,
                evidence_size_mb: 10_000,
                urgency: AppealUrgency::High,
                panel_size: 15,
            })
            .expect("quote must succeed");
        assert_eq!(
            quote.deposit_xor,
            Quantity::from(5_000u32),
            "fraud class max clamp"
        );
    }

    #[test]
    fn quote_uses_one_wide_product_before_enforcing_quantity_bounds() {
        let boundary: Quantity =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042046"
                .parse()
                .expect("largest even quantity below the signed 512-bit maximum");
        assert_eq!(
            boundary.try_mul_decimal(&Numeric::from(2u32)),
            Err(NumericOperationError::MantissaOverflow),
            "the formerly staged growth factor must cross the public bound"
        );

        let mut classes = BTreeMap::new();
        classes.insert(
            AppealClass::Content,
            AppealClassConfig::new(
                boundary.clone(),
                1,
                Numeric::one(),
                Numeric::one(),
                Numeric::zero(),
                Quantity::zero(),
                boundary.clone(),
            ),
        );
        let config = AppealPricingConfig {
            version: "wide-product-regression".to_owned(),
            quote_ttl_secs: 60,
            default_panel_size: 1,
            urgency_normal_multiplier: "0.5".parse().expect("canonical reduction factor"),
            urgency_high_multiplier: Numeric::one(),
            classes,
        };

        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Content,
                backlog: 1,
                evidence_size_mb: 0,
                urgency: AppealUrgency::Normal,
                panel_size: 1,
            })
            .expect("2x growth and 0.5x reduction have a representable exact product");
        assert_eq!(quote.breakdown.raw_deposit_xor, boundary);
        assert_eq!(quote.deposit_xor, quote.breakdown.raw_deposit_xor);
    }

    #[test]
    fn quote_normalizes_scale_after_all_factors_are_applied() {
        let tiny: Numeric = "0.0000000000000000000000000001"
            .parse()
            .expect("scale-28 reduction factor");
        let panel_third = Numeric::one()
            .try_decimal_div_round(
                &Numeric::from(3u32),
                APPEAL_CALCULATION_SCALE,
                RoundingMode::NearestEven,
            )
            .expect("rounded one-third panel factor");
        assert_eq!(
            Quantity::one()
                .try_mul_decimal(&tiny)
                .and_then(|value| value.try_mul_decimal(&panel_third)),
            Err(NumericOperationError::ScaleOverflow),
            "the formerly staged reductions must cross the public scale bound"
        );

        let mut class = AppealClassConfig::new(
            Quantity::one(),
            1,
            Numeric::zero(),
            Numeric::one(),
            Numeric::zero(),
            Quantity::zero(),
            Quantity::one(),
        );
        class.surge_multiplier = "10000000000000000000000000000"
            .parse()
            .expect("scale-cancelling growth factor");
        let mut classes = BTreeMap::new();
        classes.insert(AppealClass::Content, class);
        let config = AppealPricingConfig {
            version: "scale-normalization-regression".to_owned(),
            quote_ttl_secs: 60,
            default_panel_size: 3,
            urgency_normal_multiplier: tiny,
            urgency_high_multiplier: Numeric::one(),
            classes,
        };

        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Content,
                backlog: 0,
                evidence_size_mb: 0,
                urgency: AppealUrgency::Normal,
                panel_size: 1,
            })
            .expect("the final conceptual product rounds once at nano-XOR precision");
        assert_eq!(quote.breakdown.raw_deposit_xor.to_string(), "0.333333333");
    }

    #[test]
    fn invalid_panel_size_rejected() {
        let config = AppealPricingConfig::baseline_v1();
        let err = config
            .quote(AppealQuoteInput {
                class: AppealClass::Access,
                backlog: 1,
                evidence_size_mb: 1,
                urgency: AppealUrgency::Normal,
                panel_size: 0,
            })
            .expect_err("zero panel size must fail");
        assert_eq!(err, AppealPricingError::InvalidPanelSize);
    }

    #[test]
    fn manifest_loader_matches_baseline() {
        let manifest = norito::json!({
            "version": "governance-baseline",
            "quote_ttl_secs": 900,
            "default_panel_size": 7,
            "urgency_multipliers": {
                "normal": "1",
                "high": "1.2"
            },
            "classes": {
                "content": {
                    "base_rate_xor": "150",
                    "backlog_target": 50,
                    "backlog_cap": "1",
                    "size_divisor_mb": "100",
                    "size_cap": "2",
                    "min_deposit_xor": "100",
                    "max_deposit_xor": "2500"
                }
            }
        });
        let config =
            AppealPricingConfig::from_manifest_value(&manifest).expect("manifest should load");
        assert_eq!(config.version(), "governance-baseline");
        assert_eq!(config.default_panel_size(), 7);
        let quote = config
            .quote(AppealQuoteInput {
                class: AppealClass::Content,
                backlog: 10,
                evidence_size_mb: 10,
                urgency: AppealUrgency::High,
                panel_size: 7,
            })
            .expect("quote");
        assert!(quote.deposit_xor > Quantity::zero());
    }

    #[test]
    fn manifest_loader_rejects_unknown_class() {
        let manifest = norito::json!({
            "version": "bad",
            "quote_ttl_secs": 60,
            "default_panel_size": 7,
            "urgency_multipliers": {
                "normal": "1",
                "high": "1.1"
            },
            "classes": {
                "unknown": {
                    "base_rate_xor": "1",
                    "backlog_target": 1,
                    "backlog_cap": "1",
                    "size_divisor_mb": "1",
                    "size_cap": "1",
                    "min_deposit_xor": "1",
                    "max_deposit_xor": "2"
                }
            }
        });
        let err = AppealPricingConfig::from_manifest_value(&manifest)
            .expect_err("unknown class should fail");
        assert!(err.0.contains("unknown appeal class"));
    }

    #[test]
    fn settlement_baseline_refunds_overturn() {
        let config = AppealSettlementConfig::baseline_v1();
        let panel = config.default_panel_size();
        let deposit = Quantity::from(400u32);
        let breakdown = config
            .settle(
                deposit.clone(),
                panel,
                AppealVerdict::Decision(AppealDecision::Overturn),
            )
            .expect("settlement must succeed");
        assert_eq!(breakdown.refund_xor, deposit);
        assert_eq!(breakdown.treasury_xor, Quantity::zero());
        assert_eq!(breakdown.held_xor, Quantity::zero());
        assert_eq!(
            breakdown.panel_reward_total_xor,
            config
                .panel_rewards()
                .total_reward(panel)
                .expect("bounded baseline reward")
        );
    }

    #[test]
    fn appeal_verdict_parser_accepts_decisions_and_aliases() {
        assert_eq!(
            "overturn".parse::<AppealVerdict>().expect("decision"),
            AppealVerdict::Decision(AppealDecision::Overturn)
        );
        assert_eq!(
            "withdrawn-post".parse::<AppealVerdict>().expect("alias"),
            AppealVerdict::WithdrawnAfterPanel
        );
        assert_eq!(
            "pending".parse::<AppealVerdict>().expect("alias"),
            AppealVerdict::Escalated
        );
        assert!("unknown".parse::<AppealVerdict>().is_err());
    }

    #[test]
    fn appeal_quantity_literal_parser_accepts_only_canonical_nonnegative_strings() {
        assert_eq!(
            parse_appeal_quantity_literal("deposit_xor", "339.3").expect("quantity"),
            "339.3".parse::<Quantity>().expect("canonical quantity")
        );
        for invalid in [
            "339.30",
            "-1",
            "not-a-number",
            "1e3",
            " 1",
            "1 ",
            "0.0000000001",
        ] {
            assert!(
                parse_appeal_quantity_literal("deposit_xor", invalid).is_err(),
                "`{invalid}` must be rejected"
            );
        }
    }

    #[test]
    fn pricing_manifest_rejects_sub_nano_xor_amounts() {
        let manifest = norito::json!({
            "version": "over-precision",
            "quote_ttl_secs": 900,
            "default_panel_size": 7,
            "urgency_multipliers": {
                "normal": "1",
                "high": "1.2"
            },
            "classes": {
                "content": {
                    "base_rate_xor": "0.0000000001",
                    "backlog_target": 50,
                    "backlog_cap": "1",
                    "size_divisor_mb": "100",
                    "size_cap": "2",
                    "min_deposit_xor": "0",
                    "max_deposit_xor": "2500"
                }
            }
        });
        let error = AppealPricingConfig::from_manifest_value(&manifest)
            .expect_err("sub-nano base rates must fail closed");
        assert!(error.to_string().contains("valid XOR quantity"));
    }

    #[test]
    fn settlement_rounds_partitions_toward_zero_and_conserves_nano_dust() {
        let config = AppealSettlementConfig::baseline_v1();
        let deposit: Quantity = "1.000000001".parse().expect("nano-XOR deposit");
        let breakdown = config
            .settle(deposit.clone(), 7, AppealVerdict::Frivolous)
            .expect("bounded settlement");

        assert_eq!(breakdown.refund_xor.to_string(), "0.5");
        assert_eq!(breakdown.treasury_xor.to_string(), "0.5");
        assert_eq!(breakdown.held_xor.to_string(), "0.000000001");
        assert_eq!(
            breakdown
                .refund_xor
                .try_add(&breakdown.treasury_xor)
                .and_then(|partitioned| partitioned.try_add(&breakdown.held_xor))
                .expect("bounded conservation sum"),
            deposit
        );
        for amount in [
            &breakdown.refund_xor,
            &breakdown.treasury_xor,
            &breakdown.held_xor,
            &breakdown.panel_reward_per_juror_xor,
            &breakdown.panel_reward_total_xor,
        ] {
            XorQuantity::try_from_quantity(amount.clone())
                .expect("every settlement output must fit nano-XOR precision");
        }
    }

    #[test]
    fn settlement_manifest_rejects_sub_nano_reward_amounts() {
        let manifest = norito::json!({
            "version": "over-precision",
            "default_panel_size": 7,
            "panel_rewards": {
                "stipend_per_juror_xor": "0.0000000001",
                "case_bonus_xor": "10"
            },
            "rules": {
                "decisions": {
                    "uphold": { "refund_rate": "0", "treasury_rate": "1" },
                    "overturn": { "refund_rate": "1", "treasury_rate": "0" },
                    "modify": { "refund_rate": "1", "treasury_rate": "0" }
                },
                "withdrawn_before_panel": { "refund_rate": "0.9", "treasury_rate": "0" },
                "withdrawn_after_panel": { "refund_rate": "0", "treasury_rate": "1" },
                "frivolous": { "refund_rate": "0.5", "treasury_rate": "0.5" },
                "escalated": { "refund_rate": "0", "treasury_rate": "0" }
            }
        });
        let error = AppealSettlementConfig::from_manifest_value(&manifest)
            .expect_err("sub-nano rewards must fail closed");
        assert!(error.to_string().contains("valid XOR quantity"));
    }

    #[test]
    fn settlement_manifest_loader_supports_rules() {
        let manifest = norito::json!({
            "version": "governance-baseline",
            "default_panel_size": 9,
            "panel_rewards": {
                "stipend_per_juror_xor": "30",
                "case_bonus_xor": "5"
            },
            "rules": {
                "decisions": {
                    "uphold": { "refund_rate": "0", "treasury_rate": "1" },
                    "overturn": { "refund_rate": "1", "treasury_rate": "0" },
                    "modify": { "refund_rate": "1", "treasury_rate": "0" }
                },
                "withdrawn_before_panel": { "refund_rate": "0.9", "treasury_rate": "0" },
                "withdrawn_after_panel": { "refund_rate": "0", "treasury_rate": "1" },
                "frivolous": { "refund_rate": "0.5", "treasury_rate": "0.5" },
                "escalated": { "refund_rate": "0", "treasury_rate": "0" }
            }
        });
        let config =
            AppealSettlementConfig::from_manifest_value(&manifest).expect("manifest should parse");
        assert_eq!(config.version(), "governance-baseline");
        assert_eq!(config.default_panel_size(), 9);
        let deposit = Quantity::from(200u32);
        let breakdown = config
            .settle(deposit.clone(), 9, AppealVerdict::WithdrawnBeforePanel)
            .expect("withdrawn settlement");
        assert_eq!(breakdown.refund_xor, Quantity::from(180u32));
        assert_eq!(breakdown.treasury_xor, Quantity::zero());
    }

    #[test]
    fn disbursement_handles_no_shows_and_forfeits_rewards() {
        let config = AppealSettlementConfig::baseline_v1();
        let panel_size = 7;
        let domain: DomainId = DomainId::try_new("panel", "universal").expect("domain id");
        let jurors: Vec<AccountId> = (0..panel_size)
            .map(|i| make_account(u8::try_from(i).expect("fits"), &domain))
            .collect();
        let refund_account = make_account(100, &domain);
        let treasury_account = make_account(101, &domain);
        let escrow_account = make_account(102, &domain);
        let no_shows = vec![jurors[0].clone(), jurors[1].clone()];

        let plan = config
            .disburse(AppealDisbursementInput {
                deposit_xor: Quantity::from(420u32),
                panel_size,
                verdict: AppealVerdict::Decision(AppealDecision::Overturn),
                jurors: &jurors,
                no_shows: &no_shows,
                refund_account: &refund_account,
                treasury_account: &treasury_account,
                escrow_account: &escrow_account,
            })
            .expect("disbursement");

        assert_eq!(plan.settlement.refund_xor, Quantity::from(420u32));
        assert_eq!(plan.rewards_available_xor, Quantity::from(185u32));
        assert_eq!(plan.juror_payouts.len(), 5);
        for payout in &plan.juror_payouts {
            assert_eq!(payout.stipend_xor, Quantity::from(25u32));
            assert_eq!(payout.bonus_xor, Quantity::from(2u32));
        }
        assert_eq!(plan.rewards_forfeited_treasury_xor, Quantity::from(50u32));
        assert_eq!(plan.total_treasury_xor, plan.rewards_forfeited_treasury_xor);
        assert!(
            plan.no_show_accounts
                .iter()
                .all(|account| no_shows.contains(account))
        );
    }

    #[test]
    fn disbursement_routes_fractional_bonus_dust_to_treasury() {
        let config = AppealSettlementConfig::baseline_v1();
        let panel_size = 3;
        let domain: DomainId = DomainId::try_new("panel-dust", "universal").expect("domain id");
        let jurors: Vec<AccountId> = (0..panel_size)
            .map(|i| make_account(u8::try_from(i).expect("fits"), &domain))
            .collect();
        let refund_account = make_account(100, &domain);
        let treasury_account = make_account(101, &domain);
        let escrow_account = make_account(102, &domain);

        let plan = config
            .disburse(AppealDisbursementInput {
                deposit_xor: Quantity::from(100_u32),
                panel_size,
                verdict: AppealVerdict::Decision(AppealDecision::Overturn),
                jurors: &jurors,
                no_shows: &[],
                refund_account: &refund_account,
                treasury_account: &treasury_account,
                escrow_account: &escrow_account,
            })
            .expect("bounded disbursement");

        assert_eq!(plan.juror_payouts.len(), 3);
        for payout in &plan.juror_payouts {
            assert_eq!(payout.bonus_xor.to_string(), "3.333333333");
            XorQuantity::try_from_quantity(payout.total().expect("bounded juror payout"))
                .expect("juror payout must fit nano-XOR precision");
        }
        assert_eq!(plan.rewards_available_xor.to_string(), "85");
        assert_eq!(plan.rewards_paid_total_xor.to_string(), "84.999999999");
        assert_eq!(
            plan.rewards_forfeited_treasury_xor.to_string(),
            "0.000000001"
        );
        assert_eq!(plan.total_treasury_xor, plan.rewards_forfeited_treasury_xor);
    }

    #[test]
    fn disbursement_rejects_panel_mismatch() {
        let config = AppealSettlementConfig::baseline_v1();
        let domain: DomainId = DomainId::try_new("panel", "universal").expect("domain id");
        let jurors: Vec<AccountId> = (0..3)
            .map(|i| make_account(u8::try_from(i).expect("fits"), &domain))
            .collect();
        let refund_account = make_account(120, &domain);
        let treasury_account = make_account(121, &domain);
        let escrow_account = make_account(122, &domain);

        let err = config
            .disburse(AppealDisbursementInput {
                deposit_xor: Quantity::from(100u32),
                panel_size: 4,
                verdict: AppealVerdict::Decision(AppealDecision::Uphold),
                jurors: &jurors,
                no_shows: &[],
                refund_account: &refund_account,
                treasury_account: &treasury_account,
                escrow_account: &escrow_account,
            })
            .expect_err("mismatched panel size must fail");
        assert!(matches!(
            err,
            AppealDisbursementError::PanelSizeMismatch { .. }
        ));
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct AppealPricingManifestError(String);

impl AppealPricingManifestError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct AppealSettlementManifestError(String);

impl AppealSettlementManifestError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }

    fn arithmetic(error: NumericOperationError) -> Self {
        Self(format!(
            "appeal settlement arithmetic is outside the numeric domain: {error}"
        ))
    }
}

fn require_string_field<'a>(
    map: &'a JsonMap,
    field: &'static str,
) -> Result<&'a str, AppealPricingManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{field}` in appeal pricing manifest"))
    })?;
    value
        .as_str()
        .ok_or_else(|| AppealPricingManifestError::new(format!("`{field}` must be a string")))
}

fn require_object_field<'a>(
    map: &'a JsonMap,
    field: &'static str,
) -> Result<&'a JsonMap, AppealPricingManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{field}` in appeal pricing manifest"))
    })?;
    value
        .as_object()
        .ok_or_else(|| AppealPricingManifestError::new(format!("`{field}` must be an object")))
}

fn parse_u64_field(map: &JsonMap, field: &'static str) -> Result<u64, AppealPricingManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{field}` in appeal pricing manifest"))
    })?;
    match value {
        Value::Number(num) => num.as_u64().ok_or_else(|| {
            AppealPricingManifestError::new(format!("`{field}` must be a non-negative integer"))
        }),
        _ => Err(AppealPricingManifestError::new(format!(
            "`{field}` must be a number"
        ))),
    }
}

fn parse_u32_field(map: &JsonMap, field: &'static str) -> Result<u32, AppealPricingManifestError> {
    let value = parse_u64_field(map, field)?;
    u32::try_from(value).map_err(|_| {
        AppealPricingManifestError::new(format!(
            "`{field}` must fit within a 32-bit unsigned integer"
        ))
    })
}

fn parse_numeric_from_map(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Numeric, AppealPricingManifestError> {
    let value = map.get(key).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{label}` in appeal pricing manifest"))
    })?;
    parse_numeric_value(value, label)
}

fn parse_numeric_value(value: &Value, label: &str) -> Result<Numeric, AppealPricingManifestError> {
    let raw = value.as_str().ok_or_else(|| {
        AppealPricingManifestError::new(format!("`{label}` must be a canonical decimal string"))
    })?;
    let parsed = raw.parse::<Numeric>().map_err(|err| {
        AppealPricingManifestError::new(format!("failed to parse `{label}` as decimal: {err}"))
    })?;
    if parsed.to_string() != raw {
        return Err(AppealPricingManifestError::new(format!(
            "`{label}` must use canonical decimal spelling `{parsed}`"
        )));
    }
    Ok(parsed)
}

fn parse_quantity_from_map(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Quantity, AppealPricingManifestError> {
    let value = map.get(key).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{label}` in appeal pricing manifest"))
    })?;
    let raw = value.as_str().ok_or_else(|| {
        AppealPricingManifestError::new(format!("`{label}` must be a canonical quantity string"))
    })?;
    let parsed = raw.parse::<Quantity>().map_err(|err| {
        AppealPricingManifestError::new(format!("failed to parse `{label}` as quantity: {err}"))
    })?;
    if parsed.to_string() != raw {
        return Err(AppealPricingManifestError::new(format!(
            "`{label}` must use canonical quantity spelling `{parsed}`"
        )));
    }
    XorQuantity::try_from_quantity(parsed.clone()).map_err(|err| {
        AppealPricingManifestError::new(format!("`{label}` is not a valid XOR quantity: {err}"))
    })?;
    Ok(parsed)
}

fn require_string_field_settlement<'a>(
    map: &'a JsonMap,
    field: &'static str,
) -> Result<&'a str, AppealSettlementManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "missing `{field}` in appeal settlement manifest"
        ))
    })?;
    value.as_str().ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "`{field}` must be a string in appeal settlement manifest"
        ))
    })
}

fn require_object_field_settlement<'a>(
    map: &'a JsonMap,
    field: &'static str,
) -> Result<&'a JsonMap, AppealSettlementManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "missing `{field}` in appeal settlement manifest"
        ))
    })?;
    value.as_object().ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "`{field}` must be an object in appeal settlement manifest"
        ))
    })
}

fn parse_u32_field_settlement(
    map: &JsonMap,
    field: &'static str,
) -> Result<u32, AppealSettlementManifestError> {
    let value = map.get(field).ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "missing `{field}` in appeal settlement manifest"
        ))
    })?;
    match value {
        Value::Number(num) => num
            .as_u64()
            .ok_or_else(|| {
                AppealSettlementManifestError::new(format!(
                    "`{field}` must be a non-negative integer"
                ))
            })
            .and_then(|raw| {
                u32::try_from(raw).map_err(|_| {
                    AppealSettlementManifestError::new(format!(
                        "`{field}` must fit within a 32-bit unsigned integer"
                    ))
                })
            }),
        _ => Err(AppealSettlementManifestError::new(format!(
            "`{field}` must be a number"
        ))),
    }
}

fn parse_numeric_from_map_settlement(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Numeric, AppealSettlementManifestError> {
    let value = map.get(key).ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "missing `{label}` in appeal settlement manifest"
        ))
    })?;
    parse_numeric_value(value, label).map_err(|err| AppealSettlementManifestError(err.0))
}

fn parse_quantity_from_map_settlement(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Quantity, AppealSettlementManifestError> {
    parse_quantity_from_map(map, key, label).map_err(|err| AppealSettlementManifestError(err.0))
}
