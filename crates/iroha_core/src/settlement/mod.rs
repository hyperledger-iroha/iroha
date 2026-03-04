//! Unified XOR settlement helpers used by block production.
//!
//! The implementation here wraps the primitives provided by the
//! `settlement_router` crate so the rest of `iroha_core` interacts with a
//! single façade.  The façade keeps the logic deterministic and hides
//! serialization details (Norito receipts, decimal arithmetic) from the rest of
//! the code base.  Integration with Kura buffers and swap execution will be
//! layered on top in follow-up patches.

use std::collections::BTreeMap;

use iroha_config::parameters::actual as config;
use iroha_crypto::HashOf;
use iroha_data_model::{
    asset::AssetDefinitionId, block::consensus::LaneSettlementReceipt,
    transaction::SignedTransaction,
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
pub use settlement_router::VolatilityBucket;
use settlement_router::{
    MicroXor, ShadowPriceCalculator,
    config::SettlementConfig,
    haircut::{HaircutTier, LiquidityProfile},
    policy::{BufferPolicy, BufferStatus},
    receipt::SettlementReceipt,
};
use time::Duration as TimeDuration;

/// Error returned when quoting settlement amounts fails.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum QuoteError {
    /// Local token amounts exceeded the Decimal range.
    #[error("local amount {0} cannot be represented as Decimal")]
    LocalAmountOverflow(u128),
    /// TWAP was zero, producing an undefined price.
    #[error("twap price must be non-zero")]
    ZeroTwap,
}

/// Result of a settlement quote.
#[derive(Debug, Clone)]
pub struct SettlementQuote {
    /// Shadow price output used for buffer debits.
    pub receipt: SettlementReceipt,
    /// XOR due immediately after the inclusion (micro units).
    pub xor_due: Decimal,
    /// XOR expected after haircuts (micro units).
    pub xor_after_haircut: Decimal,
    /// Effective safety margin (base + volatility) in basis points.
    pub effective_epsilon_bps: u16,
}

/// Deterministic XOR settlement engine.
#[derive(Debug, Clone)]
pub struct SettlementEngine {
    calculator: ShadowPriceCalculator,
    buffer_policy: BufferPolicy,
}

fn duration_from_std(duration: std::time::Duration) -> TimeDuration {
    let secs = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    let mut converted = TimeDuration::seconds(secs);
    if duration.subsec_nanos() != 0 {
        converted += TimeDuration::nanoseconds(i64::from(duration.subsec_nanos()));
    }
    converted
}

impl SettlementEngine {
    /// Create an engine using roadmap defaults (60s TWAP, 25 bps margin,
    /// 72 h buffer horizon).  Used primarily in tests.
    #[must_use]
    pub fn new_roadmap_default() -> Self {
        Self::from_router_config(&config::Router::default())
    }

    /// Create an engine from configuration provided via `iroha_config`.
    #[must_use]
    pub fn from_router_config(router: &config::Router) -> Self {
        let settlement_config = SettlementConfig {
            twap_window: duration_from_std(router.twap_window).into(),
            epsilon: settlement_router::config::EpsilonBps::new(router.epsilon_bps),
            buffer_horizon_hours: router.buffer_horizon_hours,
        };
        let buffer_policy = BufferPolicy {
            alert: router.buffer_alert_pct,
            throttle: router.buffer_throttle_pct,
            xor_only: router.buffer_xor_only_pct,
            halt: router.buffer_halt_pct,
        };
        Self {
            calculator: ShadowPriceCalculator::new(settlement_config),
            buffer_policy,
        }
    }

    /// Access the current buffer policy thresholds.
    #[must_use]
    pub const fn buffer_policy(&self) -> &BufferPolicy {
        &self.buffer_policy
    }

    /// Evaluate the remaining buffer against the configured guard rails.
    #[must_use]
    pub fn evaluate_buffer(&self, remaining: &MicroXor, capacity: &MicroXor) -> BufferStatus {
        self.buffer_policy.evaluate(remaining, capacity)
    }

    /// Access the current settlement configuration used by this engine.
    #[must_use]
    pub const fn config(&self) -> &SettlementConfig {
        self.calculator.config()
    }

    /// Quote a settlement given the local gas amount (micro units), a TWAP
    /// price, and the liquidity profile of the conversion path.
    ///
    /// # Errors
    ///
    /// Returns [`QuoteError::LocalAmountOverflow`] if the provided micro amount
    /// does not fit into the internal decimal representation, or
    /// [`QuoteError::ZeroTwap`] when the time-weighted price is zero.
    pub fn quote(
        &self,
        source_id: [u8; 32],
        local_amount_micro: u128,
        twap_local_per_xor: Decimal,
        liquidity: LiquidityProfile,
        volatility: VolatilityBucket,
        timestamp_ms: u64,
    ) -> Result<SettlementQuote, QuoteError> {
        let local_decimal = Decimal::from_u128(local_amount_micro)
            .ok_or(QuoteError::LocalAmountOverflow(local_amount_micro))?;

        if twap_local_per_xor.is_zero() {
            return Err(QuoteError::ZeroTwap);
        }

        let shadow = self.calculator.compute(
            local_decimal,
            twap_local_per_xor,
            HaircutTier::new(liquidity),
            volatility,
        );
        let effective_epsilon_bps = self.calculator.effective_epsilon_bps(volatility);

        let receipt = SettlementReceipt::new_with_timestamp_ms(
            source_id,
            local_amount_micro,
            &shadow,
            timestamp_ms,
        );

        Ok(SettlementQuote {
            xor_due: Decimal::from(shadow.xor_due),
            xor_after_haircut: Decimal::from(shadow.xor_with_haircut),
            effective_epsilon_bps,
            receipt,
        })
    }
}

/// Pending settlement record keyed by transaction hash.
#[derive(Debug, Clone)]
pub struct PendingSettlement {
    /// Caller-specified source identifier (typically transaction hash bytes).
    pub source_id: [u8; 32],
    /// Asset definition backing the local gas token.
    pub asset_definition_id: AssetDefinitionId,
    /// Local gas-token amount debited (micro units).
    pub local_amount_micro: u128,
    /// XOR amount booked immediately (micro units).
    pub xor_due_micro: u128,
    /// XOR amount expected after haircuts (micro units).
    pub xor_after_haircut_micro: u128,
    /// Variance between due and post-haircut XOR (micro units).
    pub xor_variance_micro: u128,
    /// UTC timestamp associated with the transaction (milliseconds).
    pub timestamp_ms: u64,
    /// Liquidity profile applied during settlement.
    pub liquidity_profile: LiquidityProfile,
    /// Volatility bucket applied when computing the safety margin.
    pub volatility_bucket: VolatilityBucket,
    /// TWAP value used when quoting the settlement.
    pub twap_local_per_xor: Decimal,
    /// Basis-point safety margin applied when quoting.
    pub epsilon_bps: u16,
    /// TWAP window length (seconds) used when computing the quote.
    pub twap_window_seconds: u32,
    /// UTC timestamp for the oracle price sample used during quoting (milliseconds).
    pub oracle_timestamp_ms: u64,
}

impl PendingSettlement {
    /// Convert the pending record into a lane-level settlement receipt.
    #[must_use]
    pub fn into_lane_receipt(self) -> LaneSettlementReceipt {
        LaneSettlementReceipt {
            source_id: self.source_id,
            local_amount_micro: self.local_amount_micro,
            xor_due_micro: self.xor_due_micro,
            xor_after_haircut_micro: self.xor_after_haircut_micro,
            xor_variance_micro: self.xor_variance_micro,
            timestamp_ms: self.timestamp_ms,
        }
    }
}

/// Accumulates settlement receipts for transactions processed in the current block.
#[derive(Debug, Default, Clone)]
pub struct SettlementAccumulator {
    records: BTreeMap<HashOf<SignedTransaction>, PendingSettlement>,
}

impl SettlementAccumulator {
    /// Record a settlement receipt for the given transaction hash.
    pub fn record(&mut self, tx_hash: HashOf<SignedTransaction>, record: PendingSettlement) {
        self.records.insert(tx_hash, record);
    }

    /// Drain the accumulated receipts, returning ownership of the internal map.
    pub fn drain(&mut self) -> BTreeMap<HashOf<SignedTransaction>, PendingSettlement> {
        core::mem::take(&mut self.records)
    }

    /// Whether the accumulator currently stores no receipts.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use settlement_router::MicroXor;

    use super::*;

    fn micro(value: i64) -> MicroXor {
        MicroXor::new(Decimal::from(value))
    }

    #[test]
    fn quote_roadmap_defaults() {
        let engine = SettlementEngine::new_roadmap_default();
        let quote = engine
            .quote(
                [0x11; 32],
                2_000_000, // local micro units
                Decimal::new(50, 0),
                LiquidityProfile::Tier2,
                VolatilityBucket::Stable,
                1,
            )
            .expect("quote must succeed");

        assert!(quote.xor_due > Decimal::ZERO);
        assert!(quote.xor_after_haircut <= quote.xor_due);
        assert_eq!(quote.receipt.local_amount_micro, 2_000_000);
        assert_eq!(quote.effective_epsilon_bps, 25);
    }

    #[test]
    fn zero_twap_errors() {
        let engine = SettlementEngine::new_roadmap_default();
        let err = engine
            .quote(
                [0xFF; 32],
                1_000,
                Decimal::ZERO,
                LiquidityProfile::Tier1,
                VolatilityBucket::Stable,
                1,
            )
            .expect_err("zero twap should fail");
        matches!(err, QuoteError::ZeroTwap);
    }

    #[test]
    fn accumulator_records_and_drains() {
        let mut accumulator = SettlementAccumulator::default();
        let tx_hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x11; Hash::LENGTH]));
        let record = PendingSettlement {
            source_id: [0x22; 32],
            asset_definition_id: "xor#sora".parse().expect("valid asset definition id"),
            local_amount_micro: 10,
            xor_due_micro: 7,
            xor_after_haircut_micro: 6,
            xor_variance_micro: 1,
            timestamp_ms: 42,
            liquidity_profile: LiquidityProfile::Tier1,
            volatility_bucket: VolatilityBucket::Stable,
            twap_local_per_xor: Decimal::ONE,
            epsilon_bps: 25,
            twap_window_seconds: 60,
            oracle_timestamp_ms: 40,
        };
        let record_copy = record.clone();
        accumulator.record(tx_hash, record);
        let drained = accumulator.drain();
        assert!(accumulator.is_empty());
        let entry = drained.get(&tx_hash).expect("record present");
        assert_eq!(entry.local_amount_micro, record_copy.local_amount_micro);
        assert_eq!(entry.xor_due_micro, record_copy.xor_due_micro);
        assert_eq!(
            entry.xor_after_haircut_micro,
            record_copy.xor_after_haircut_micro
        );
        assert_eq!(entry.xor_variance_micro, record_copy.xor_variance_micro);
        assert_eq!(entry.timestamp_ms, record_copy.timestamp_ms);
        let receipt = entry.clone().into_lane_receipt();
        assert_eq!(receipt.source_id, record_copy.source_id);
        assert_eq!(receipt.xor_variance_micro, record_copy.xor_variance_micro);
    }

    #[test]
    fn evaluate_buffer_matches_policy_thresholds() {
        let engine = SettlementEngine::new_roadmap_default();
        let capacity = micro(1_000_000);

        assert_eq!(
            engine.evaluate_buffer(&micro(2_000_000), &capacity),
            BufferStatus::Normal
        );
        assert_eq!(
            engine.evaluate_buffer(&micro(700_000), &capacity),
            BufferStatus::Alert
        );
        assert_eq!(
            engine.evaluate_buffer(&micro(200_000), &capacity),
            BufferStatus::Throttle
        );
        assert_eq!(
            engine.evaluate_buffer(&micro(50_000), &capacity),
            BufferStatus::XorOnly
        );
        assert_eq!(
            engine.evaluate_buffer(&micro(10_000), &capacity),
            BufferStatus::Halt
        );
    }

    #[test]
    fn engine_from_router_config_applies_knobs() {
        let router = config::Router {
            epsilon_bps: 75,
            buffer_alert_pct: 80,
            buffer_throttle_pct: 60,
            buffer_xor_only_pct: 40,
            buffer_halt_pct: 5,
            twap_window: std::time::Duration::from_secs(90),
            buffer_horizon_hours: 96,
        };

        let engine = SettlementEngine::from_router_config(&router);
        assert_eq!(engine.config().epsilon.as_u16(), 75);
        assert_eq!(
            engine.config().twap_window.whole_seconds(),
            i64::try_from(router.twap_window.as_secs()).expect("twap_window seconds fit in i64")
        );
        assert_eq!(engine.config().buffer_horizon_hours, 96);
        assert_eq!(engine.buffer_policy().alert, 80);
        assert_eq!(engine.buffer_policy().throttle, 60);
        assert_eq!(engine.buffer_policy().xor_only, 40);
        assert_eq!(engine.buffer_policy().halt, 5);
    }
}
