//! Appeal pricing and settlement utilities for moderation finance workflows
//! (MINFO-7).
//!
//! The congestion-aware deposit formula matches the specification in
//! `docs/source/sorafs_appeal_pricing_plan.md`. The settlement helpers wire in
//! the initial escrow/payout policy so treasury dashboards, CLI tools, and SDKs
//! can deterministically compute refund/slash amounts and panel rewards.

use std::{collections::BTreeMap, fmt, str::FromStr};

use iroha_data_model::account::AccountId;
use norito::json::{Map as JsonMap, Value, native::Number};
use rust_decimal::Decimal;
use thiserror::Error;

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
    pub base_rate_xor: Decimal,
    pub backlog_target: u32,
    pub backlog_cap: Decimal,
    pub size_divisor_mb: Decimal,
    pub size_cap: Decimal,
    pub min_deposit_xor: Decimal,
    pub max_deposit_xor: Decimal,
    pub surge_multiplier: Decimal,
}

impl AppealClassConfig {
    /// Construct a class configuration.
    #[must_use]
    pub fn new(
        base_rate_xor: Decimal,
        backlog_target: u32,
        backlog_cap: Decimal,
        size_divisor_mb: Decimal,
        size_cap: Decimal,
        min_deposit_xor: Decimal,
        max_deposit_xor: Decimal,
    ) -> Self {
        Self {
            base_rate_xor,
            backlog_target,
            backlog_cap,
            size_divisor_mb,
            size_cap,
            min_deposit_xor,
            max_deposit_xor,
            surge_multiplier: Decimal::ONE,
        }
    }

    /// Apply a surge multiplier override.
    #[must_use]
    pub fn with_surge_multiplier(mut self, multiplier: Decimal) -> Self {
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
    urgency_normal_multiplier: Decimal,
    urgency_high_multiplier: Decimal,
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
                Decimal::from(150u32),
                50,
                Decimal::ONE,
                Decimal::from(100u32),
                Decimal::from(2),
                Decimal::from(100u32),
                Decimal::from(2_500u32),
            ),
        );
        classes.insert(
            AppealClass::Access,
            AppealClassConfig::new(
                Decimal::from(200u32),
                30,
                Decimal::ONE,
                Decimal::from(50u32),
                Decimal::from(2),
                Decimal::from(100u32),
                Decimal::from(2_500u32),
            ),
        );
        classes.insert(
            AppealClass::Fraud,
            AppealClassConfig::new(
                Decimal::from(500u32),
                20,
                Decimal::ONE,
                Decimal::from(50u32),
                Decimal::from(2),
                Decimal::from(100u32),
                Decimal::from(5_000u32),
            ),
        );
        classes.insert(
            AppealClass::Other,
            AppealClassConfig::new(
                Decimal::from(120u32),
                40,
                Decimal::ONE,
                Decimal::from(100u32),
                Decimal::from(2),
                Decimal::from(100u32),
                Decimal::from(2_500u32),
            ),
        );

        Self {
            version: "baseline-v1".to_string(),
            quote_ttl_secs: 15 * 60,
            default_panel_size: 7,
            urgency_normal_multiplier: Decimal::ONE,
            urgency_high_multiplier: Decimal::new(12, 1), // 1.2×
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
            parse_decimal_from_map(urgency_obj, "normal", "urgency_multipliers.normal")?;
        if urgency_normal <= Decimal::ZERO {
            return Err(AppealPricingManifestError::new(
                "`urgency_multipliers.normal` must be greater than zero",
            ));
        }
        let urgency_high = parse_decimal_from_map(urgency_obj, "high", "urgency_multipliers.high")?;
        if urgency_high <= Decimal::ZERO {
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
            let base_rate = parse_decimal_from_map(
                class_obj,
                "base_rate_xor",
                &format!("classes.{class_label}.base_rate_xor"),
            )?;
            if base_rate <= Decimal::ZERO {
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
            let backlog_cap = parse_decimal_from_map(
                class_obj,
                "backlog_cap",
                &format!("classes.{class_label}.backlog_cap"),
            )?;
            if backlog_cap < Decimal::ZERO {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.backlog_cap` must not be negative"
                )));
            }
            let size_divisor = parse_decimal_from_map(
                class_obj,
                "size_divisor_mb",
                &format!("classes.{class_label}.size_divisor_mb"),
            )?;
            if size_divisor <= Decimal::ZERO {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.size_divisor_mb` must be greater than zero"
                )));
            }
            let size_cap = parse_decimal_from_map(
                class_obj,
                "size_cap",
                &format!("classes.{class_label}.size_cap"),
            )?;
            if size_cap < Decimal::ZERO {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.size_cap` must not be negative"
                )));
            }
            let min_deposit = parse_decimal_from_map(
                class_obj,
                "min_deposit_xor",
                &format!("classes.{class_label}.min_deposit_xor"),
            )?;
            if min_deposit < Decimal::ZERO {
                return Err(AppealPricingManifestError::new(format!(
                    "`classes.{class_label}.min_deposit_xor` must not be negative"
                )));
            }
            let max_deposit = parse_decimal_from_map(
                class_obj,
                "max_deposit_xor",
                &format!("classes.{class_label}.max_deposit_xor"),
            )?;
            if max_deposit <= Decimal::ZERO {
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
                    parse_decimal_value(value, &format!("classes.{class_label}.surge_multiplier"))?;
                if parsed <= Decimal::ZERO {
                    return Err(AppealPricingManifestError::new(format!(
                        "`classes.{class_label}.surge_multiplier` must be greater than zero"
                    )));
                }
                parsed
            } else {
                Decimal::ONE
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

        if class_cfg.backlog_target == 0 {
            return Err(AppealPricingError::InvalidBacklogTarget { class: input.class });
        }
        if class_cfg.size_divisor_mb.is_zero() {
            return Err(AppealPricingError::InvalidSizeDivisor { class: input.class });
        }

        let backlog_factor = {
            let target = Decimal::from(class_cfg.backlog_target);
            let ratio = Decimal::from(input.backlog) / target;
            clamp_decimal(ratio, Decimal::ZERO, class_cfg.backlog_cap)
        };
        let size_multiplier = {
            let ratio = Decimal::from(input.evidence_size_mb) / class_cfg.size_divisor_mb;
            Decimal::ONE + clamp_decimal(ratio, Decimal::ZERO, class_cfg.size_cap)
        };
        let urgency_multiplier = match input.urgency {
            AppealUrgency::Normal => self.urgency_normal_multiplier,
            AppealUrgency::High => self.urgency_high_multiplier,
        };
        let panel_multiplier =
            Decimal::from(input.panel_size) / Decimal::from(self.default_panel_size);

        let raw = class_cfg.base_rate_xor
            * (Decimal::ONE + backlog_factor)
            * size_multiplier
            * urgency_multiplier
            * panel_multiplier
            * class_cfg.surge_multiplier;
        let clamped = clamp_decimal(raw, class_cfg.min_deposit_xor, class_cfg.max_deposit_xor);

        Ok(AppealQuote {
            deposit_xor: clamped,
            breakdown: AppealQuoteBreakdown {
                base_rate_xor: class_cfg.base_rate_xor,
                backlog_factor,
                size_multiplier,
                urgency_multiplier,
                panel_multiplier,
                surge_multiplier: class_cfg.surge_multiplier,
                raw_deposit_xor: raw,
                min_deposit_xor: class_cfg.min_deposit_xor,
                max_deposit_xor: class_cfg.max_deposit_xor,
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
#[derive(Clone, Copy, Debug)]
pub struct AppealQuoteBreakdown {
    pub base_rate_xor: Decimal,
    pub backlog_factor: Decimal,
    pub size_multiplier: Decimal,
    pub urgency_multiplier: Decimal,
    pub panel_multiplier: Decimal,
    pub surge_multiplier: Decimal,
    pub raw_deposit_xor: Decimal,
    pub min_deposit_xor: Decimal,
    pub max_deposit_xor: Decimal,
}

/// Quote output.
#[derive(Clone, Copy, Debug)]
pub struct AppealQuote {
    pub deposit_xor: Decimal,
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

/// Mapping of refund/slash ratios for a particular verdict.
#[derive(Clone, Debug)]
pub struct AppealSettlementRule {
    refund_rate: Decimal,
    treasury_rate: Decimal,
}

impl AppealSettlementRule {
    fn new(
        refund_rate: Decimal,
        treasury_rate: Decimal,
    ) -> Result<Self, AppealSettlementManifestError> {
        if refund_rate < Decimal::ZERO || refund_rate > Decimal::ONE {
            return Err(AppealSettlementManifestError::new(
                "`refund_rate` must be between 0 and 1",
            ));
        }
        if treasury_rate < Decimal::ZERO || treasury_rate > Decimal::ONE {
            return Err(AppealSettlementManifestError::new(
                "`treasury_rate` must be between 0 and 1",
            ));
        }
        if refund_rate + treasury_rate > Decimal::ONE {
            return Err(AppealSettlementManifestError::new(
                "refund_rate + treasury_rate must not exceed 1",
            ));
        }
        Ok(Self {
            refund_rate,
            treasury_rate,
        })
    }

    fn refund_component(&self, deposit: Decimal) -> Decimal {
        deposit * self.refund_rate
    }

    fn treasury_component(&self, deposit: Decimal) -> Decimal {
        deposit * self.treasury_rate
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
    stipend_per_juror_xor: Decimal,
    case_bonus_xor: Decimal,
}

impl PanelRewardConfig {
    #[must_use]
    pub fn new(stipend_per_juror_xor: Decimal, case_bonus_xor: Decimal) -> Self {
        Self {
            stipend_per_juror_xor,
            case_bonus_xor,
        }
    }

    /// Reward per juror for a single case.
    #[must_use]
    pub fn stipend_per_juror(&self) -> Decimal {
        self.stipend_per_juror_xor
    }

    /// Case-level bonus paid out once per case.
    #[must_use]
    pub fn case_bonus(&self) -> Decimal {
        self.case_bonus_xor
    }

    /// Total reward for a panel size.
    #[must_use]
    pub fn total_reward(&self, panel_size: u32) -> Decimal {
        if panel_size == 0 {
            return Decimal::ZERO;
        }
        let size = Decimal::from(panel_size);
        (self.stipend_per_juror_xor * size) + self.case_bonus_xor
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
            AppealSettlementRule::new(Decimal::ONE, Decimal::ZERO).expect("valid rule"),
        );
        decision_rules.insert(
            AppealDecision::Modify,
            AppealSettlementRule::new(Decimal::ONE, Decimal::ZERO).expect("valid rule"),
        );
        decision_rules.insert(
            AppealDecision::Uphold,
            AppealSettlementRule::new(Decimal::ZERO, Decimal::ONE).expect("valid rule"),
        );
        Self {
            version: "baseline-v1".to_string(),
            default_panel_size: 7,
            panel_rewards: PanelRewardConfig::new(Decimal::from(25u32), Decimal::from(10u32)),
            decision_rules,
            withdrawn_before_panel: AppealSettlementRule::new(Decimal::new(9, 1), Decimal::ZERO)
                .expect("valid rule"),
            withdrawn_after_panel: AppealSettlementRule::new(Decimal::ZERO, Decimal::ONE)
                .expect("valid rule"),
            frivolous: AppealSettlementRule::new(Decimal::new(5, 1), Decimal::new(5, 1))
                .expect("valid rule"),
            escalated: AppealSettlementRule::new(Decimal::ZERO, Decimal::ZERO).expect("valid rule"),
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
        let stipend_per_juror = parse_decimal_from_map_settlement(
            panel_obj,
            "stipend_per_juror_xor",
            "panel_rewards.stipend_per_juror_xor",
        )?;
        if stipend_per_juror < Decimal::ZERO {
            return Err(AppealSettlementManifestError::new(
                "`panel_rewards.stipend_per_juror_xor` must be non-negative",
            ));
        }
        let case_bonus = parse_decimal_from_map_settlement(
            panel_obj,
            "case_bonus_xor",
            "panel_rewards.case_bonus_xor",
        )?;
        if case_bonus < Decimal::ZERO {
            return Err(AppealSettlementManifestError::new(
                "`panel_rewards.case_bonus_xor` must be non-negative",
            ));
        }

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
        deposit_xor: Decimal,
        panel_size: u32,
        verdict: AppealVerdict,
    ) -> Result<AppealSettlementBreakdown, AppealSettlementError> {
        if deposit_xor < Decimal::ZERO {
            return Err(AppealSettlementError::InvalidDeposit);
        }
        if panel_size == 0 {
            return Err(AppealSettlementError::InvalidPanelSize);
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
        let refund = rule.refund_component(deposit_xor);
        let treasury = rule.treasury_component(deposit_xor);
        let mut held = deposit_xor - refund - treasury;
        if held < Decimal::ZERO {
            held = Decimal::ZERO;
        }
        let panel_reward_total = self.panel_rewards.total_reward(panel_size);
        Ok(AppealSettlementBreakdown {
            refund_xor: refund,
            treasury_xor: treasury,
            held_xor: held,
            panel_reward_per_juror_xor: self.panel_rewards.stipend_per_juror(),
            panel_reward_total_xor: panel_reward_total,
        })
    }

    /// Compute the full disbursement plan, including deposit flows and panel rewards.
    pub fn disburse(
        &self,
        input: AppealDisbursementInput<'_>,
    ) -> Result<AppealDisbursementPlan, AppealDisbursementError> {
        let settlement = self.settle(input.deposit_xor, input.panel_size, input.verdict)?;
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
        let attending_count = Decimal::from(attending_count_u32);

        let stipend = self.panel_rewards.stipend_per_juror();
        let bonus = self.panel_rewards.case_bonus();
        let bonus_share = bonus
            .checked_div(attending_count)
            .ok_or(AppealDisbursementError::BonusSplit)?;

        let mut juror_payouts = Vec::with_capacity(attending.len());
        for juror in &attending {
            juror_payouts.push(JurorPayout {
                juror: juror.clone(),
                stipend_xor: stipend,
                bonus_xor: bonus_share,
            });
        }

        let rewards_paid_total_xor = (stipend + bonus_share) * Decimal::from(attending_count_u32);
        let rewards_available_xor = settlement.panel_reward_total_xor;
        let rewards_forfeited_treasury_xor = clamp_decimal(
            rewards_available_xor - rewards_paid_total_xor,
            Decimal::ZERO,
            rewards_available_xor,
        );
        let total_treasury_xor = settlement.treasury_xor + rewards_forfeited_treasury_xor;

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
        parse_decimal_from_map_settlement(obj, "refund_rate", &format!("{label}.refund_rate"))?;
    let treasury =
        parse_decimal_from_map_settlement(obj, "treasury_rate", &format!("{label}.treasury_rate"))?;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppealSettlementBreakdown {
    pub refund_xor: Decimal,
    pub treasury_xor: Decimal,
    pub held_xor: Decimal,
    pub panel_reward_per_juror_xor: Decimal,
    pub panel_reward_total_xor: Decimal,
}

/// Inputs required to derive per-account disbursement flows.
pub struct AppealDisbursementInput<'a> {
    pub deposit_xor: Decimal,
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
    pub stipend_xor: Decimal,
    pub bonus_xor: Decimal,
}

impl JurorPayout {
    /// Total payout for the juror.
    #[must_use]
    pub fn total(&self) -> Decimal {
        self.stipend_xor + self.bonus_xor
    }
}

/// Deterministic disbursement plan combining deposit settlement and panel rewards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppealDisbursementPlan {
    pub deposit_xor: Decimal,
    pub verdict: AppealVerdict,
    pub panel_size: u32,
    pub settlement: AppealSettlementBreakdown,
    pub refund_account: AccountId,
    pub treasury_account: AccountId,
    pub escrow_account: AccountId,
    pub no_show_accounts: Vec<AccountId>,
    pub juror_payouts: Vec<JurorPayout>,
    pub rewards_available_xor: Decimal,
    pub rewards_paid_total_xor: Decimal,
    pub rewards_forfeited_treasury_xor: Decimal,
    pub total_treasury_xor: Decimal,
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
    /// Bonus allocation could not be divided among attendees.
    #[error("failed to divide case bonus among attending jurors")]
    BonusSplit,
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
}

/// Errors surfaced when computing settlement outcomes.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppealSettlementError {
    /// Deposits must be non-negative.
    #[error("deposit must be non-negative")]
    InvalidDeposit,
    /// Panel size must be greater than zero.
    #[error("panel size must be greater than zero")]
    InvalidPanelSize,
    /// No rule was configured for the supplied decision.
    #[error("no settlement rule registered for {decision:?}")]
    MissingDecisionRule { decision: AppealDecision },
}

fn clamp_decimal(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
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
            Decimal::new(3393, 1),
            "expected 339.3 XOR"
        );
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
            Decimal::from(5_000u32),
            "fraud class max clamp"
        );
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
                "normal": "1.0",
                "high": "1.2"
            },
            "classes": {
                "content": {
                    "base_rate_xor": "150",
                    "backlog_target": 50,
                    "backlog_cap": "1.0",
                    "size_divisor_mb": "100",
                    "size_cap": "2.0",
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
        assert!(quote.deposit_xor > Decimal::ZERO);
    }

    #[test]
    fn manifest_loader_rejects_unknown_class() {
        let manifest = norito::json!({
            "version": "bad",
            "quote_ttl_secs": 60,
            "default_panel_size": 7,
            "urgency_multipliers": {
                "normal": "1.0",
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
        let deposit = Decimal::from(400u32);
        let breakdown = config
            .settle(
                deposit,
                panel,
                AppealVerdict::Decision(AppealDecision::Overturn),
            )
            .expect("settlement must succeed");
        assert_eq!(breakdown.refund_xor, deposit);
        assert_eq!(breakdown.treasury_xor, Decimal::ZERO);
        assert_eq!(breakdown.held_xor, Decimal::ZERO);
        assert_eq!(
            breakdown.panel_reward_total_xor,
            config.panel_rewards().total_reward(panel)
        );
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
        let deposit = Decimal::from(200u32);
        let breakdown = config
            .settle(deposit, 9, AppealVerdict::WithdrawnBeforePanel)
            .expect("withdrawn settlement");
        assert_eq!(breakdown.refund_xor, Decimal::new(9, 1) * deposit);
        assert_eq!(breakdown.treasury_xor, Decimal::ZERO);
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
                deposit_xor: Decimal::from(420u32),
                panel_size,
                verdict: AppealVerdict::Decision(AppealDecision::Overturn),
                jurors: &jurors,
                no_shows: &no_shows,
                refund_account: &refund_account,
                treasury_account: &treasury_account,
                escrow_account: &escrow_account,
            })
            .expect("disbursement");

        assert_eq!(plan.settlement.refund_xor, Decimal::from(420u32));
        assert_eq!(plan.rewards_available_xor, Decimal::from(185u32));
        assert_eq!(plan.juror_payouts.len(), 5);
        for payout in &plan.juror_payouts {
            assert_eq!(payout.stipend_xor, Decimal::from(25u32));
            assert_eq!(payout.bonus_xor, Decimal::from(2u32));
        }
        assert_eq!(plan.rewards_forfeited_treasury_xor, Decimal::from(50u32));
        assert_eq!(plan.total_treasury_xor, plan.rewards_forfeited_treasury_xor);
        assert!(
            plan.no_show_accounts
                .iter()
                .all(|account| no_shows.contains(account))
        );
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
                deposit_xor: Decimal::from(100u32),
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

fn parse_decimal_from_map(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Decimal, AppealPricingManifestError> {
    let value = map.get(key).ok_or_else(|| {
        AppealPricingManifestError::new(format!("missing `{label}` in appeal pricing manifest"))
    })?;
    parse_decimal_value(value, label)
}

fn parse_decimal_value(value: &Value, label: &str) -> Result<Decimal, AppealPricingManifestError> {
    match value {
        Value::String(raw) => Decimal::from_str(raw).map_err(|err| {
            AppealPricingManifestError::new(format!("failed to parse `{label}` as decimal: {err}"))
        }),
        Value::Number(number) => decimal_from_number(number, label),
        _ => Err(AppealPricingManifestError::new(format!(
            "`{label}` must be a string or number"
        ))),
    }
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

fn parse_decimal_from_map_settlement(
    map: &JsonMap,
    key: &'static str,
    label: &str,
) -> Result<Decimal, AppealSettlementManifestError> {
    let value = map.get(key).ok_or_else(|| {
        AppealSettlementManifestError::new(format!(
            "missing `{label}` in appeal settlement manifest"
        ))
    })?;
    parse_decimal_value(value, label).map_err(|err| AppealSettlementManifestError(err.0))
}

fn decimal_from_number(
    number: &Number,
    label: &str,
) -> Result<Decimal, AppealPricingManifestError> {
    match number {
        Number::I64(value) => Ok(Decimal::from(*value)),
        Number::U64(value) => Ok(Decimal::from(*value)),
        Number::F64(value) => Decimal::try_from(*value).map_err(|_| {
            AppealPricingManifestError::new(format!("`{label}` float `{value}` is not finite"))
        }),
    }
}
