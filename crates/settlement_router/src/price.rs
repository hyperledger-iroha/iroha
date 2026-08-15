//! Shadow-price calculation for deterministic XOR liabilities.
use crate::{
    Numeric, NumericOperationError, Quantity, RoundingMode, XOR_QUANTITY_SCALE, XorQuantity,
    XorQuantityError, config::SettlementConfig, haircut::HaircutTier, volatility::VolatilityBucket,
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
/// Result of a shadow-price computation.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ShadowPrice {
    /// Exact XOR amount that must be debited immediately.
    pub xor_due: XorQuantity,
    /// Exact XOR expected after applying the configured haircut.
    pub xor_with_haircut: XorQuantity,
}
/// Consensus-visible shadow-price failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ShadowPriceError {
    /// The local-token-per-XOR TWAP must be strictly positive.
    #[error("TWAP local-token-per-XOR price must be positive")]
    NonPositiveTwap,
    /// A haircut cannot deduct more than the full value.
    #[error("haircut {basis_points} bps exceeds the 10,000 bps maximum")]
    InvalidHaircut {
        /// Rejected haircut.
        basis_points: u16,
    },
    /// Exact decimal arithmetic failed.
    #[error("shadow-price arithmetic failed: {0}")]
    Numeric(#[from] NumericOperationError),
    /// The result violates XOR's quantity domain.
    #[error("shadow-price XOR result is invalid: {0}")]
    Xor(#[from] XorQuantityError),
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
    /// Compute the XOR due from an exact local-token amount and a positive
    /// local-token-per-XOR TWAP.
    ///
    /// Both margin and haircut stages round toward positive infinity at XOR's
    /// nine-digit precision boundary. This guarantees that conversion never
    /// under-collects while avoiding any host floating-point behavior.
    ///
    /// # Errors
    /// Rejects non-positive TWAPs, haircuts above 100%, and arithmetic results
    /// outside the bounded exact-decimal or XOR-scale domain.
    pub fn compute(
        &self,
        local_amount: &Quantity,
        twap_price: &Numeric,
        haircut: HaircutTier,
        volatility: VolatilityBucket,
    ) -> Result<ShadowPrice, ShadowPriceError> {
        if twap_price <= &Numeric::zero() {
            return Err(ShadowPriceError::NonPositiveTwap);
        }
        let haircut_bps = haircut.effective_bps();
        if haircut_bps > 10_000 {
            return Err(ShadowPriceError::InvalidHaircut {
                basis_points: haircut_bps,
            });
        }
        let margin_bps = 10_000_u64 + u64::from(self.effective_epsilon_bps(volatility));
        let margin_factor = Numeric::new(margin_bps, 4);
        let xor_due = local_amount.try_mul_div_decimal_round(
            &margin_factor,
            twap_price,
            XOR_QUANTITY_SCALE,
            RoundingMode::Ceil,
        )?;
        let xor_due = XorQuantity::try_from_quantity(xor_due)?;
        let retained_bps = 10_000_u64 - u64::from(haircut_bps);
        let xor_with_haircut = xor_due.checked_mul_ratio_round(
            retained_bps,
            core::num::NonZeroU64::new(10_000).expect("basis-point denominator is non-zero"),
            XOR_QUANTITY_SCALE,
            RoundingMode::Ceil,
        )?;
        Ok(ShadowPrice {
            xor_due,
            xor_with_haircut,
        })
    }
    /// Effective epsilon (base margin + volatility bucket) in basis points.
    #[must_use]
    pub const fn effective_epsilon_bps(&self, volatility: VolatilityBucket) -> u16 {
        let base = self.config.epsilon.as_u16();
        let extra = volatility.extra_margin_bps();
        let total = base.saturating_add(extra);
        let cap = VolatilityBucket::max_total_margin_bps();
        if total > cap { cap } else { total }
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        EpsilonBps, Numeric, Quantity,
        config::SettlementConfig,
        haircut::{HaircutTier, LiquidityProfile},
        price::{ShadowPriceCalculator, ShadowPriceError},
        volatility::VolatilityBucket,
    };
    use expect_test::expect;
    use time::Duration;
    fn calculator() -> ShadowPriceCalculator {
        ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(60)),
            epsilon: EpsilonBps::new(25),
            buffer_horizon_hours: 72,
        })
    }
    #[test]
    fn applies_margin_and_haircut_exactly() {
        let result = calculator()
            .compute(
                &Quantity::from(1_000_000_u64),
                &Numeric::from(50_u32),
                HaircutTier::new(LiquidityProfile::Tier2),
                VolatilityBucket::Stable,
            )
            .expect("valid shadow price");
        expect!["20050"].assert_eq(&result.xor_due.to_string());
        expect!["19999.875"].assert_eq(&result.xor_with_haircut.to_string());
    }
    #[test]
    fn preserves_sub_micro_xor_precision_with_explicit_ceil() {
        let result = calculator()
            .compute(
                &"0.000000001".parse().expect("local quantity"),
                &Numeric::from(2_u32),
                HaircutTier::new(LiquidityProfile::Tier1),
                VolatilityBucket::Stable,
            )
            .expect("valid shadow price");
        assert_eq!(result.xor_due.to_string(), "0.000000001");
        assert_eq!(result.xor_with_haircut.to_string(), "0.000000001");
    }
    #[test]
    fn bounds_only_the_final_shadow_price_after_margin_and_twap_cancel() {
        let maximum = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse::<Quantity>()
            .expect("signed-domain maximum is a quantity");
        let result = calculator()
            .compute(
                &maximum,
                &"1.0025".parse().expect("matching exact TWAP"),
                HaircutTier::new(LiquidityProfile::Tier1),
                VolatilityBucket::Stable,
            )
            .expect("cancelling conceptual intermediates leave a bounded result");
        assert_eq!(result.xor_due.as_quantity(), &maximum);
        assert_eq!(result.xor_with_haircut.as_quantity(), &maximum);
    }
    #[test]
    fn rejects_zero_and_negative_twap() {
        for twap in [Numeric::zero(), "-1".parse().expect("negative decimal")] {
            assert_eq!(
                calculator().compute(
                    &Quantity::one(),
                    &twap,
                    HaircutTier::new(LiquidityProfile::Tier1),
                    VolatilityBucket::Stable,
                ),
                Err(ShadowPriceError::NonPositiveTwap)
            );
        }
    }
    #[test]
    fn rejects_haircut_above_one_hundred_percent() {
        assert_eq!(
            calculator().compute(
                &Quantity::one(),
                &Numeric::one(),
                HaircutTier::new(LiquidityProfile::Tier1).with_override(10_001),
                VolatilityBucket::Stable,
            ),
            Err(ShadowPriceError::InvalidHaircut {
                basis_points: 10_001
            })
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
        assert_eq!(
            calculator.effective_epsilon_bps(VolatilityBucket::Dislocated),
            VolatilityBucket::max_total_margin_bps()
        );
    }
}
