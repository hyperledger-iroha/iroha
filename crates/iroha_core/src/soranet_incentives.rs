//! Relay incentive scoring and payout helpers for SoraNet (SNNet-7).
//!
//! This module evaluates relay performance metrics, applies staking and compliance policy checks,
//! and emits deterministic reward instructions destined for the XOR treasury. The scoring model
//! operates on per-mille ratios so results remain deterministic across hardware while allowing
//! fine-grained weighting of availability, bandwidth, and compliance signals.

use std::{collections::BTreeMap, str::FromStr};

#[cfg(test)]
use iroha_data_model::isi::transfer::TransferBox;
use iroha_data_model::{
    account::AccountId,
    isi::InstructionBox,
    metadata::Metadata,
    name::Name,
    soranet::{
        Digest32, RelayId,
        incentives::{
            RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1,
            RelayEpochMetricsV1, RelayRewardDisputeV1, RelayRewardInstructionV1,
        },
    },
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity, RoundingMode},
};
use thiserror::Error;

/// Weight distribution (basis points) applied to each reward component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardWeights {
    /// Weight applied to the relay availability ratio.
    pub availability_bps: u16,
    /// Weight applied to the verified bandwidth ratio.
    pub bandwidth_bps: u16,
    /// Weight applied to the compliance multiplier.
    pub compliance_bps: u16,
}

impl RewardWeights {
    const MAX_TOTAL: u16 = 10_000;

    /// Returns the total configured weight (basis points).
    #[must_use]
    pub const fn total(self) -> u16 {
        self.availability_bps
            .saturating_add(self.bandwidth_bps)
            .saturating_add(self.compliance_bps)
    }

    /// Validate the weight distribution.
    ///
    /// # Errors
    ///
    /// Returns [`RelayIncentiveError::InvalidWeights`] when the sum is zero or exceeds
    /// [`Self::MAX_TOTAL`].
    pub fn validate(self) -> Result<(), RelayIncentiveError> {
        let total = self.total();
        if total == 0 || total > Self::MAX_TOTAL {
            return Err(RelayIncentiveError::InvalidWeights { total });
        }
        Ok(())
    }
}

/// Normalised reward components expressed in per-mille (0‒1000).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardComponents {
    /// Availability ratio (0‒1000).
    pub availability_per_mille: u16,
    /// Bandwidth ratio (0‒1000).
    pub bandwidth_per_mille: u16,
    /// Compliance multiplier (0‒1000).
    pub compliance_per_mille: u16,
}

impl RewardComponents {
    /// All-zero components helper.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            availability_per_mille: 0,
            bandwidth_per_mille: 0,
            compliance_per_mille: 0,
        }
    }
}

/// Reason a relay was deemed ineligible for payout in the current epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardSkipReason {
    /// Relay bond does not satisfy the minimum exit requirement.
    InsufficientBond,
    /// Relay is suspended or otherwise not eligible for payouts.
    ComplianceSuspended,
    /// Relay bond uses an unexpected asset, preventing payoff in XOR credits.
    BondAssetMismatch,
    /// Score reduced to zero by the weighting function (insufficient performance).
    ZeroScore,
}

/// Outcome of reward evaluation for a relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardDecision {
    /// Relay qualifies for a payout; instruction encapsulates the treasury transfer.
    Rewarded {
        /// Signed instruction for the XOR treasury.
        instruction: RelayRewardInstructionV1,
        /// Final score expressed in per-mille.
        score_per_mille: u16,
        /// Component ratios used to derive the score.
        components: RewardComponents,
    },
    /// Relay did not qualify for payout; contains diagnostic context.
    Skipped {
        /// Zero-amount instruction emitted for bookkeeping (metadata populated for diagnostics).
        instruction: RelayRewardInstructionV1,
        /// Reason the relay was skipped.
        reason: RewardSkipReason,
        /// Score (per-mille) computed prior to the denial (may be zero).
        score_per_mille: u16,
        /// Component ratios observed for the relay.
        components: RewardComponents,
    },
}

/// Immutable configuration backing the reward calculator.
#[derive(Debug, Clone)]
pub struct RewardConfig {
    /// Bonding/slashing policy enforced by the treasury.
    pub policy: RelayBondPolicyV1,
    /// Weighting applied to availability/bandwidth/compliance signals.
    pub weights: RewardWeights,
    /// Target bytes per epoch required to saturate the bandwidth component.
    pub bandwidth_target_bytes: u128,
    /// Maximum XOR payout allocated per relay/epoch (full score == this amount).
    pub epoch_base_payout: Quantity,
    /// Penalty applied to the compliance component when the relay is under warning.
    pub warning_penalty_per_mille: u16,
    /// Bonus applied when the relay advertises exit capability (per-mille).
    pub exit_bonus_per_mille: u16,
}

/// Errors surfaced by the reward calculator.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RelayIncentiveError {
    /// Weight distribution is invalid (sum is zero or exceeds `10_000` basis points).
    #[error("reward weights must be >0 and <=10000 basis points (found {total})")]
    InvalidWeights {
        /// Total basis points supplied by the caller.
        total: u16,
    },
    /// Bandwidth target must be non-zero to derive ratios.
    #[error("bandwidth target must be non-zero")]
    ZeroBandwidthTarget,
    /// XOR payouts cannot carry precision finer than nano-XOR.
    #[error("base payout scale {scale} exceeds nano-XOR scale 9")]
    InvalidBasePayoutScale {
        /// Configured decimal scale.
        scale: u32,
    },
    /// Numeric multiplication or rounding overflowed.
    #[error("numeric overflow while computing payout")]
    NumericOverflow,
    /// Conversion of score into a fractional numeric failed.
    #[error("numeric scale unsupported while computing payout")]
    NumericScale,
}

/// Deterministic `SoraNet` relay reward calculator.
#[derive(Debug, Clone)]
pub struct RelayRewardCalculator {
    config: RewardConfig,
    total_weight: u16,
}

impl RelayRewardCalculator {
    const SCORE_SCALE_PER_MILLE: u32 = 3;
    const PAYOUT_SCALE: u32 = 9;

    /// Create a new calculator using the supplied configuration.
    ///
    /// # Errors
    ///
    /// Returns [`RelayIncentiveError::InvalidWeights`],
    /// [`RelayIncentiveError::ZeroBandwidthTarget`], or
    /// [`RelayIncentiveError::InvalidBasePayoutScale`] when configuration
    /// invariants are violated.
    pub fn new(config: RewardConfig) -> Result<Self, RelayIncentiveError> {
        config.weights.validate()?;
        if config.bandwidth_target_bytes == 0 {
            return Err(RelayIncentiveError::ZeroBandwidthTarget);
        }
        if config.epoch_base_payout.scale() > Self::PAYOUT_SCALE {
            return Err(RelayIncentiveError::InvalidBasePayoutScale {
                scale: config.epoch_base_payout.scale(),
            });
        }
        Ok(Self {
            total_weight: config.weights.total(),
            config,
        })
    }

    /// Returns a reference to the calculator configuration.
    #[must_use]
    pub fn config(&self) -> &RewardConfig {
        &self.config
    }

    /// Evaluate the relay metrics and derive a payout decision.
    ///
    /// # Errors
    ///
    /// Returns [`RelayIncentiveError::NumericOverflow`] when payout calculation saturates
    /// and [`RelayIncentiveError::NumericScale`] when the score cannot be represented with the
    /// selected numeric scale.
    pub fn evaluate(
        &self,
        metrics: &RelayEpochMetricsV1,
        bond: &RelayBondLedgerEntryV1,
        beneficiary: &AccountId,
        issued_at_unix: u64,
        budget_approval_id: Option<Digest32>,
        extra_metadata: Option<&Metadata>,
    ) -> Result<RewardDecision, RelayIncentiveError> {
        let relay_id = metrics.relay_id;
        let epoch = metrics.epoch;
        let components = self.compute_components(metrics);
        let base_score = self.apply_weights(components);
        let mut score = self.apply_exit_bonus(base_score, bond.exit_capable);

        let make_instruction = |payout_amount: Quantity, reward_score: u16, metadata: Metadata| {
            RelayRewardInstructionV1 {
                relay_id,
                epoch,
                beneficiary: beneficiary.clone(),
                payout_asset_id: self.config.policy.bond_asset_id.clone(),
                payout_amount,
                reward_score: u64::from(reward_score),
                budget_approval_id,
                metadata,
            }
        };

        let make_skip = |reason: RewardSkipReason, current_score: u16| {
            let mut metadata = build_metadata(
                components,
                base_score,
                current_score,
                &Quantity::zero(),
                bond.exit_capable,
                issued_at_unix,
                extra_metadata,
            );
            if let Ok(name) = Name::from_str("skip_reason") {
                let _ = metadata.insert(name, Json::new(skip_reason_label(reason)));
            }
            let instruction = make_instruction(Quantity::zero(), current_score, metadata);
            Ok(RewardDecision::Skipped {
                instruction,
                reason,
                score_per_mille: current_score,
                components,
            })
        };

        if bond.bond_asset_id != self.config.policy.bond_asset_id {
            return make_skip(RewardSkipReason::BondAssetMismatch, score);
        }

        if !bond.meets_exit_minimum(&self.config.policy) {
            return make_skip(RewardSkipReason::InsufficientBond, score);
        }

        if matches!(metrics.compliance, RelayComplianceStatusV1::Suspended) {
            score = 0;
            return make_skip(RewardSkipReason::ComplianceSuspended, score);
        }

        if score == 0 {
            return make_skip(RewardSkipReason::ZeroScore, score);
        }

        let payout = self.compute_payout(score)?;
        let metadata = build_metadata(
            components,
            base_score,
            score,
            &payout,
            bond.exit_capable,
            issued_at_unix,
            extra_metadata,
        );

        let instruction = make_instruction(payout, score, metadata);

        Ok(RewardDecision::Rewarded {
            instruction,
            score_per_mille: score,
            components,
        })
    }

    fn compute_components(&self, metrics: &RelayEpochMetricsV1) -> RewardComponents {
        let availability = metrics.uptime_ratio_per_mille();
        let bandwidth = {
            let scaled = metrics.verified_bandwidth_bytes.saturating_mul(1_000);
            let ratio = scaled / self.config.bandwidth_target_bytes;
            ratio.min(1_000) as u16
        };
        let penalty = self.config.warning_penalty_per_mille.min(1_000);
        let compliance = match metrics.compliance {
            RelayComplianceStatusV1::Clean => 1_000,
            RelayComplianceStatusV1::Warning => 1_000u16.saturating_sub(penalty),
            RelayComplianceStatusV1::Suspended => 0,
        };
        RewardComponents {
            availability_per_mille: availability,
            bandwidth_per_mille: bandwidth,
            compliance_per_mille: compliance,
        }
    }

    fn apply_weights(&self, components: RewardComponents) -> u16 {
        let total_weight = u32::from(self.total_weight);
        let availability = u32::from(components.availability_per_mille)
            .saturating_mul(u32::from(self.config.weights.availability_bps));
        let bandwidth = u32::from(components.bandwidth_per_mille)
            .saturating_mul(u32::from(self.config.weights.bandwidth_bps));
        let compliance = u32::from(components.compliance_per_mille)
            .saturating_mul(u32::from(self.config.weights.compliance_bps));
        let weighted = availability
            .saturating_add(bandwidth)
            .saturating_add(compliance);
        if total_weight == 0 {
            0
        } else {
            (weighted / total_weight).min(1_000) as u16
        }
    }

    fn apply_exit_bonus(&self, score: u16, exit_capable: bool) -> u16 {
        if exit_capable && self.config.exit_bonus_per_mille > 0 {
            score
                .saturating_add(self.config.exit_bonus_per_mille)
                .min(1_000)
        } else {
            score
        }
    }

    fn compute_payout(&self, score: u16) -> Result<Quantity, RelayIncentiveError> {
        let score_numeric = Numeric::try_new(u128::from(score), Self::SCORE_SCALE_PER_MILLE)
            .map_err(|_| RelayIncentiveError::NumericScale)?;
        self.config
            .epoch_base_payout
            .try_mul_div_decimal_round(
                &score_numeric,
                &Numeric::one(),
                Self::PAYOUT_SCALE,
                RoundingMode::TowardZero,
            )
            .map_err(|_| RelayIncentiveError::NumericOverflow)
    }
}

/// Helper that converts reward instructions into ledger-ready transfers.
#[derive(Debug, Clone)]
pub struct RelayPayoutLedger {
    treasury_account: AccountId,
}

impl RelayPayoutLedger {
    /// Create a payout ledger that debits the specified treasury account.
    #[must_use]
    pub fn new(treasury_account: AccountId) -> Self {
        Self { treasury_account }
    }

    /// Access the treasury account identifier.
    #[must_use]
    pub fn treasury_account(&self) -> &AccountId {
        &self.treasury_account
    }

    /// Convert the reward instruction into a transfer instruction, skipping zero-amount payouts.
    #[must_use]
    pub fn to_transfer(&self, instruction: &RelayRewardInstructionV1) -> Option<InstructionBox> {
        if instruction.is_zero_amount() {
            return None;
        }
        Some(instruction.to_transfer_instruction(&self.treasury_account))
    }

    /// Create a dispute record linked to the original payout instruction.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn open_dispute(
        &self,
        instruction: RelayRewardInstructionV1,
        requested_amount: Quantity,
        submitted_by: AccountId,
        submitted_at_unix: u64,
        reason: impl Into<String>,
    ) -> RelayRewardDisputeV1 {
        RelayRewardDisputeV1::new(
            instruction.relay_id,
            instruction.epoch,
            submitted_by,
            submitted_at_unix,
            instruction,
            requested_amount,
            reason,
        )
    }
}

/// Aggregates relay earnings to feed operator dashboards and telemetry.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RelayEarningsEntry {
    /// Total number of payouts issued to the relay.
    pub payout_count: u64,
    /// Total XOR amount paid to the relay.
    pub payout_amount: Quantity,
}

/// Failure while aggregating relay payout telemetry.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RelayEarningsError {
    /// The preserved payout counter exceeded its fixed-width domain.
    #[error("relay payout count overflow")]
    PayoutCountOverflow,
    /// Exact quantity addition exceeded the bounded numeric domain.
    #[error("relay payout amount overflow")]
    PayoutAmountOverflow,
}

/// Accumulates payout entries keyed by relay identifier.
#[derive(Debug, Default)]
pub struct RelayEarningsAccumulator {
    entries: BTreeMap<RelayId, RelayEarningsEntry>,
}

impl RelayEarningsAccumulator {
    /// Record a non-zero payout for aggregation.
    pub fn record(
        &mut self,
        instruction: &RelayRewardInstructionV1,
    ) -> Result<(), RelayEarningsError> {
        if instruction.is_zero_amount() {
            return Ok(());
        }
        let entry = self.entries.entry(instruction.relay_id).or_default();
        let payout_count = entry
            .payout_count
            .checked_add(1)
            .ok_or(RelayEarningsError::PayoutCountOverflow)?;
        let payout_amount = entry
            .payout_amount
            .checked_add(&instruction.payout_amount)
            .map_err(|_| RelayEarningsError::PayoutAmountOverflow)?;
        entry.payout_count = payout_count;
        entry.payout_amount = payout_amount;
        Ok(())
    }

    /// Retrieve the aggregated earnings map.
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<RelayId, RelayEarningsEntry> {
        &self.entries
    }
}

fn build_metadata(
    components: RewardComponents,
    base_score: u16,
    final_score: u16,
    payout_amount: &Quantity,
    exit_capable: bool,
    issued_at_unix: u64,
    extra_metadata: Option<&Metadata>,
) -> Metadata {
    let mut metadata = Metadata::default();

    metadata_insert(
        &mut metadata,
        "availability_per_mille",
        Json::new(u64::from(components.availability_per_mille)),
    );
    metadata_insert(
        &mut metadata,
        "bandwidth_per_mille",
        Json::new(u64::from(components.bandwidth_per_mille)),
    );
    metadata_insert(
        &mut metadata,
        "compliance_per_mille",
        Json::new(u64::from(components.compliance_per_mille)),
    );
    metadata_insert(
        &mut metadata,
        "score_per_mille",
        Json::new(u64::from(final_score)),
    );
    metadata_insert(
        &mut metadata,
        "base_score_per_mille",
        Json::new(u64::from(base_score)),
    );
    metadata_insert(
        &mut metadata,
        "payout_amount",
        Json::new(payout_amount.to_string()),
    );
    metadata_insert(
        &mut metadata,
        "exit_bonus_applied",
        Json::new(exit_capable && final_score > base_score),
    );
    metadata_insert(&mut metadata, "issued_at_unix", Json::new(issued_at_unix));

    if let Some(extra) = extra_metadata {
        for (key, value) in extra.iter() {
            let _ = metadata.insert(key.clone(), value.clone());
        }
    }

    metadata
}

fn metadata_insert(metadata: &mut Metadata, key: &str, value: impl Into<Json>) {
    if let Ok(name) = Name::from_str(key) {
        metadata.insert(name, value);
    }
}

fn skip_reason_label(reason: RewardSkipReason) -> &'static str {
    match reason {
        RewardSkipReason::InsufficientBond => "insufficient_bond",
        RewardSkipReason::ComplianceSuspended => "compliance_suspended",
        RewardSkipReason::BondAssetMismatch => "bond_asset_mismatch",
        RewardSkipReason::ZeroScore => "zero_score",
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        metadata::Metadata,
        soranet::incentives::{
            RelayComplianceStatusV1, RelayRewardDisputeStatusV1, RelayRewardInstructionV1,
        },
    };
    use iroha_primitives::BigInt;

    use super::*;

    fn quantity(value: u64) -> Quantity {
        Quantity::from(value)
    }

    fn scaled_quantity(mantissa: u128, scale: u32) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(mantissa, scale))
            .expect("non-negative test quantity")
    }

    fn default_policy() -> RelayBondPolicyV1 {
        RelayBondPolicyV1 {
            minimum_exit_bond: quantity(1_000),
            bond_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        }
    }

    fn bond(exit_capable: bool, amount: u64) -> RelayBondLedgerEntryV1 {
        RelayBondLedgerEntryV1 {
            relay_id: [0_u8; 32],
            bonded_amount: quantity(amount),
            bond_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            bonded_since_unix: 1_000,
            exit_capable,
        }
    }

    fn metrics(
        compliance: RelayComplianceStatusV1,
        uptime: u64,
        scheduled: u64,
        bandwidth_bytes: u128,
    ) -> RelayEpochMetricsV1 {
        RelayEpochMetricsV1 {
            relay_id: [0_u8; 32],
            epoch: 42,
            uptime_seconds: uptime,
            scheduled_uptime_seconds: scheduled,
            verified_bandwidth_bytes: bandwidth_bytes,
            compliance,
            reward_score: 0,
            confidence_floor_per_mille: 1_000,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        }
    }

    fn sample_account(domain: &str) -> AccountId {
        let key_pair = KeyPair::try_from_seed(domain.as_bytes().to_vec(), Algorithm::Ed25519)
            .expect("derive deterministic sample account key");
        AccountId::new(key_pair.public_key().clone())
    }

    fn budget_id() -> [u8; 32] {
        [0xB7; 32]
    }

    fn config() -> RewardConfig {
        RewardConfig {
            policy: default_policy(),
            weights: RewardWeights {
                availability_bps: 5_000,
                bandwidth_bps: 4_000,
                compliance_bps: 1_000,
            },
            bandwidth_target_bytes: 1_000_000,
            epoch_base_payout: quantity(10),
            warning_penalty_per_mille: 200,
            exit_bonus_per_mille: 100,
        }
    }

    #[test]
    fn rewarded_when_metrics_full_and_bond_sufficient() {
        let calc = RelayRewardCalculator::new(config()).expect("config valid");
        let metrics = metrics(RelayComplianceStatusV1::Clean, 3_600, 3_600, 1_000_000);
        let bond = bond(true, 2_000);
        let beneficiary = sample_account("relay");
        let decision = calc
            .evaluate(&metrics, &bond, &beneficiary, 1_234, None, None)
            .expect("evaluation succeeds");
        let RewardDecision::Rewarded {
            instruction,
            score_per_mille,
            components,
        } = decision
        else {
            panic!("expected rewarded outcome");
        };
        assert_eq!(score_per_mille, 1_000);
        assert_eq!(
            components,
            RewardComponents {
                availability_per_mille: 1_000,
                bandwidth_per_mille: 1_000,
                compliance_per_mille: 1_000
            }
        );
        assert_eq!(instruction.reward_score, 1_000);
        assert_eq!(instruction.payout_amount, quantity(10));
        let key = Name::from_str("score_per_mille").expect("name");
        assert_eq!(instruction.metadata.get(&key), Some(&Json::new(1_000_u64)));
    }

    #[test]
    fn skip_when_bond_insufficient() {
        let calc = RelayRewardCalculator::new(config()).expect("config valid");
        let metrics = metrics(RelayComplianceStatusV1::Clean, 3_600, 3_600, 1_000_000);
        let bond = bond(true, 100);
        let beneficiary = sample_account("relay");
        let decision = calc
            .evaluate(&metrics, &bond, &beneficiary, 1_234, None, None)
            .expect("evaluation succeeds");
        let RewardDecision::Skipped { reason, .. } = decision else {
            panic!("expected skipped decision");
        };
        assert_eq!(reason, RewardSkipReason::InsufficientBond);
    }

    #[test]
    fn skip_when_compliance_suspended() {
        let calc = RelayRewardCalculator::new(config()).expect("config valid");
        let metrics = metrics(RelayComplianceStatusV1::Suspended, 3_600, 3_600, 1_000_000);
        let bond = bond(true, 2_000);
        let beneficiary = sample_account("relay");
        let decision = calc
            .evaluate(&metrics, &bond, &beneficiary, 1_234, None, None)
            .expect("evaluation succeeds");
        let RewardDecision::Skipped {
            reason,
            score_per_mille,
            ..
        } = decision
        else {
            panic!("expected skipped decision");
        };
        assert_eq!(reason, RewardSkipReason::ComplianceSuspended);
        assert_eq!(score_per_mille, 0);
    }

    #[test]
    fn skip_when_asset_mismatch() {
        let mut cfg = config();
        cfg.policy.bond_asset_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("sora", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
        let calc = RelayRewardCalculator::new(cfg).expect("config valid");
        let metrics = metrics(RelayComplianceStatusV1::Clean, 3_600, 3_600, 1_000_000);
        let bond = bond(true, 2_000);
        let beneficiary = sample_account("relay");
        let decision = calc
            .evaluate(&metrics, &bond, &beneficiary, 1_234, None, None)
            .expect("evaluation succeeds");
        let RewardDecision::Skipped { reason, .. } = decision else {
            panic!("expected skip");
        };
        assert_eq!(reason, RewardSkipReason::BondAssetMismatch);
    }

    #[test]
    fn warning_penalty_reduces_score() {
        let calc = RelayRewardCalculator::new(config()).expect("config valid");
        let metrics = metrics(RelayComplianceStatusV1::Warning, 3_600, 3_600, 1_000_000);
        let bond = bond(false, 2_000);
        let beneficiary = sample_account("relay");
        let decision = calc
            .evaluate(&metrics, &bond, &beneficiary, 1_234, None, None)
            .expect("evaluation succeeds");
        let RewardDecision::Rewarded { instruction, .. } = decision else {
            panic!("expected rewarded outcome");
        };
        println!(
            "warning payout={} score={}",
            instruction.payout_amount, instruction.reward_score
        );
        assert!(
            instruction.payout_amount < quantity(10),
            "payout should shrink under warning penalty (got {})",
            instruction.payout_amount
        );
        assert_eq!(instruction.reward_score, 980);
    }

    #[test]
    fn zero_weights_rejected() {
        let mut cfg = config();
        cfg.weights = RewardWeights {
            availability_bps: 0,
            bandwidth_bps: 0,
            compliance_bps: 0,
        };
        let err = RelayRewardCalculator::new(cfg).expect_err("should fail");
        matches!(err, RelayIncentiveError::InvalidWeights { .. });
    }

    #[test]
    fn zero_bandwidth_target_rejected() {
        let mut cfg = config();
        cfg.bandwidth_target_bytes = 0;
        let err = RelayRewardCalculator::new(cfg).expect_err("should fail");
        assert_eq!(err, RelayIncentiveError::ZeroBandwidthTarget);
    }

    #[test]
    fn sub_nano_base_payout_is_rejected() {
        let mut cfg = config();
        cfg.epoch_base_payout = scaled_quantity(1, 10);
        let err = RelayRewardCalculator::new(cfg).expect_err("sub-nano payout must fail");
        assert_eq!(
            err,
            RelayIncentiveError::InvalidBasePayoutScale { scale: 10 }
        );
    }

    #[test]
    fn payout_rounds_toward_zero_at_nano_xor_boundary() {
        let mut cfg = config();
        cfg.epoch_base_payout = scaled_quantity(1, 9);
        let calculator = RelayRewardCalculator::new(cfg).expect("nano-XOR payout config");

        assert_eq!(
            calculator.compute_payout(500).expect("rounded payout"),
            Quantity::zero(),
            "half a nano-XOR must be rounded explicitly toward zero"
        );
    }

    #[test]
    fn payout_rounding_bounds_only_the_final_nano_xor_result() {
        let mut bytes = [0xff_u8; 64];
        bytes[63] = 0x7f;
        let maximum = BigInt::from_twos_bytes(&bytes).expect("signed 512-bit maximum");
        let mut cfg = config();
        cfg.epoch_base_payout = Quantity::from_canonical_numeric(Numeric::new(maximum.clone(), 9))
            .expect("maximum nano-XOR mantissa is a quantity");
        let calculator = RelayRewardCalculator::new(cfg).expect("boundary payout config");

        let payout = calculator
            .compute_payout(500)
            .expect("wide half-product rounds before the final bound");
        let expected_mantissa = maximum
            .checked_div_rem(&BigInt::from(2_i32))
            .expect("nonzero divisor")
            .0;
        let expected = Quantity::from_canonical_numeric(Numeric::new(expected_mantissa, 9))
            .expect("rounded half fits");
        assert_eq!(payout, expected);
    }

    #[test]
    fn payout_ledger_handles_transfers_and_disputes() {
        let treasury = sample_account("treasury");
        let ledger = RelayPayoutLedger::new(treasury.clone());
        let instruction = RelayRewardInstructionV1 {
            relay_id: [0xAA; 32],
            epoch: 11,
            beneficiary: sample_account("relay"),
            payout_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            payout_amount: scaled_quantity(12_345, 3),
            reward_score: 812,
            budget_approval_id: Some(budget_id()),
            metadata: Metadata::default(),
        };
        let transfer_box = ledger.to_transfer(&instruction).expect("transfer present");
        let transfer = transfer_box
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer box");
        let TransferBox::Asset(asset_transfer) = transfer else {
            panic!("expected asset transfer");
        };
        assert_eq!(asset_transfer.source.account(), &treasury);
        assert_eq!(asset_transfer.destination, instruction.beneficiary);

        let dispute = ledger.open_dispute(
            instruction.clone(),
            scaled_quantity(12_500, 3),
            treasury.clone(),
            1_700_000_002,
            "throughput discrepancy",
        );
        assert_eq!(dispute.status, RelayRewardDisputeStatusV1::Pending);
        assert_eq!(dispute.original_instruction, instruction);
    }

    #[test]
    fn earnings_accumulator_sums_payouts() {
        let mut accumulator = RelayEarningsAccumulator::default();
        let base_instruction = RelayRewardInstructionV1 {
            relay_id: [0xBB; 32],
            epoch: 21,
            beneficiary: sample_account("relay"),
            payout_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            payout_amount: scaled_quantity(7_500, 2),
            reward_score: 650,
            budget_approval_id: Some(budget_id()),
            metadata: Metadata::default(),
        };
        accumulator
            .record(&base_instruction)
            .expect("first payout aggregates");
        let mut second = base_instruction.clone();
        second.epoch = 22;
        second.payout_amount = scaled_quantity(1_250, 1);
        accumulator
            .record(&second)
            .expect("second payout aggregates");

        let entry = accumulator
            .entries()
            .get(&base_instruction.relay_id)
            .expect("earnings present");
        assert_eq!(entry.payout_count, 2);
        assert_eq!(entry.payout_amount, Quantity::from(200_u64));
    }

    #[test]
    fn earnings_accumulator_counter_overflow_is_atomic() {
        let relay_id = [0xCC; 32];
        let original_amount = Quantity::from(7_u64);
        let mut accumulator = RelayEarningsAccumulator::default();
        accumulator.entries.insert(
            relay_id,
            RelayEarningsEntry {
                payout_count: u64::MAX,
                payout_amount: original_amount.clone(),
            },
        );
        let instruction = RelayRewardInstructionV1 {
            relay_id,
            epoch: 1,
            beneficiary: sample_account("overflow"),
            payout_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            payout_amount: Quantity::one(),
            reward_score: 1_000,
            budget_approval_id: Some(budget_id()),
            metadata: Metadata::default(),
        };

        assert_eq!(
            accumulator.record(&instruction),
            Err(RelayEarningsError::PayoutCountOverflow)
        );
        let entry = accumulator.entries.get(&relay_id).expect("entry retained");
        assert_eq!(entry.payout_count, u64::MAX);
        assert_eq!(entry.payout_amount, original_amount);
    }
}
