//! Shadow-price calculation for deterministic XOR liabilities.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use rust_decimal::Decimal;

use crate::{
    MicroXor, config::SettlementConfig, haircut::HaircutTier, volatility::VolatilityBucket,
};

/// Result of a shadow-price computation.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ShadowPrice {
    /// XOR amount (micro units) that must be debited immediately.
    pub xor_due: MicroXor,
    /// Total micro-XOR including the applied haircut (post-conversion expectation).
    pub xor_with_haircut: MicroXor,
}

/// Calculator used by the router to determine per-transaction liabilities.
#[derive(Clone, Debug)]
pub struct ShadowPriceCalculator {
    config: SettlementConfig,
}

impl ShadowPriceCalculator {
    /// Construct a new calculator from configuration.
    #[must_use]
    pub const fn new(config: SettlementConfig) -> Self {
        Self { config }
    }

    /// Access immutable configuration.
    #[must_use]
    pub const fn config(&self) -> &SettlementConfig {
        &self.config
    }

    /// Compute the XOR due given a local-token notional amount and the current
    /// TWAP quote.
    ///
    /// * `local_amount` is denominated in the DS gas token (micro units).
    /// * `twap_price` is expressed as `local_token / XOR`.
    #[must_use]
    pub fn compute(
        &self,
        local_amount: Decimal,
        twap_price: Decimal,
        haircut: HaircutTier,
        volatility: VolatilityBucket,
    ) -> ShadowPrice {
        let raw = if twap_price.is_zero() {
            Decimal::ZERO
        } else {
            local_amount / twap_price
        };

        let margin_multiplier = Decimal::from(10_000_u16 + self.effective_epsilon_bps(volatility))
            / Decimal::from(10_000_u16);
        let xor_due = Self::ceil_micro_xor(raw * margin_multiplier);

        let haircut_multiplier =
            Decimal::from(10_000_u16 - haircut.effective_bps()) / Decimal::from(10_000_u16);
        let xor_with_haircut = Self::ceil_micro_xor(Decimal::from(xor_due) * haircut_multiplier);

        ShadowPrice {
            xor_due,
            xor_with_haircut,
        }
    }

    /// Effective epsilon (base margin + volatility bucket) expressed in basis points.
    #[must_use]
    pub const fn effective_epsilon_bps(&self, volatility: VolatilityBucket) -> u16 {
        let base = self.config.epsilon.as_u16();
        let extra = volatility.extra_margin_bps();
        let total = base.saturating_add(extra);
        let cap = VolatilityBucket::max_total_margin_bps();
        if total > cap { cap } else { total }
    }

    fn ceil_micro_xor(value: Decimal) -> MicroXor {
        if value.fract().is_zero() {
            MicroXor::from(value)
        } else {
            let integer = value.trunc();
            MicroXor::from(integer + Decimal::ONE)
        }
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use rust_decimal::{Decimal, prelude::ToPrimitive};
    use time::Duration;

    use crate::{
        EpsilonBps,
        config::SettlementConfig,
        haircut::{HaircutTier, LiquidityProfile},
        price::ShadowPriceCalculator,
        volatility::VolatilityBucket,
    };

    #[test]
    fn applies_margin_and_haircut() {
        let calculator = ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(60)),
            epsilon: EpsilonBps::new(25),
            buffer_horizon_hours: 72,
        });

        let local = Decimal::new(1_000_000, 0); // 1_000_000 micro local units
        let twap = Decimal::new(50, 0); // 50 local per XOR
        let result = calculator.compute(
            local,
            twap,
            HaircutTier::new(LiquidityProfile::Tier2),
            VolatilityBucket::Stable,
        );

        expect!["20050"].assert_eq(
            &result
                .xor_due
                .to_u64()
                .expect("decimal fits in u64")
                .to_string(),
        );
        expect!["20000"].assert_eq(
            &result
                .xor_with_haircut
                .to_u64()
                .expect("decimal fits in u64")
                .to_string(),
        );
    }

    #[test]
    fn effective_margin_respects_volatility_bucket() {
        let calculator = ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(120)),
            epsilon: EpsilonBps::new(30),
            buffer_horizon_hours: 24,
        });

        assert_eq!(
            calculator.effective_epsilon_bps(VolatilityBucket::Stable),
            30
        );
        assert_eq!(
            calculator.effective_epsilon_bps(VolatilityBucket::Elevated),
            55
        );
        // Dislocated adds 50 bps but the total must respect the global 75 bps cap.
        assert_eq!(
            calculator.effective_epsilon_bps(VolatilityBucket::Dislocated),
            VolatilityBucket::max_total_margin_bps()
        );
    }
}
