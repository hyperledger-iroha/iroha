//! Repo/reverse-repo style swap lines that supplement AMM liquidity.
use derive_more::{Display, From};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use crate::{Numeric, NumericOperationError, RoundingMode, XorQuantity, XorQuantityError};
/// Uniquely identifies a swap line (per dataspace and collateral flavour).
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    From,
)]
#[display("{_0}")]
pub struct SwapLineId(pub u32);
/// Asset class eligible for posting as collateral against a swap line.
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    Eq,
    PartialEq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "collateral", content = "value")]
pub enum CollateralKind {
    /// The dataspace's native CBDC.
    #[display("cbdc")]
    Cbdc,
    /// XOR posted by the treasury (reverse repo).
    #[display("xor")]
    Xor,
    /// Governance-approved stablecoin.
    #[display("stable")]
    Stable,
}
/// Swap-line calculation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SwapLineError {
    /// A facility with zero limit is invalid.
    #[error("swap-line limit must be positive")]
    ZeroLimit,
    /// Exact XOR arithmetic failed.
    #[error("swap-line XOR arithmetic failed: {0}")]
    Xor(#[from] XorQuantityError),
    /// Exact ratio arithmetic failed.
    #[error("swap-line ratio arithmetic failed: {0}")]
    Numeric(#[from] NumericOperationError),
}
/// Static configuration for a swap line.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SwapLineConfig {
    /// Identifier referenced in manifests and telemetry.
    pub id: SwapLineId,
    /// Maximum outstanding XOR that can be borrowed at once.
    pub limit_xor: XorQuantity,
    /// Minimum collateral uplift expressed in basis points.
    pub collateral_haircut_bps: u16,
    /// Interest or fee schedule in basis points per annum.
    pub fee_rate_bps: u16,
    /// Collateral kind posted by the counterparty.
    pub collateral_kind: CollateralKind,
    /// Whether the facility uses fee-based remuneration or classic interest.
    pub uses_fee_schedule: bool,
}
impl SwapLineConfig {
    /// Validate static invariants.
    pub fn validate(&self) -> Result<(), SwapLineError> {
        if self.limit_xor.is_zero() {
            return Err(SwapLineError::ZeroLimit);
        }
        Ok(())
    }
    /// Return the minimum collateral required for an outstanding balance.
    ///
    /// The result rounds upward at XOR's precision boundary so collateral is
    /// never understated.
    pub fn required_collateral(
        &self,
        outstanding: &XorQuantity,
    ) -> Result<XorQuantity, SwapLineError> {
        self.validate()?;
        outstanding
            .checked_mul_ratio_round(
                10_000_u64 + u64::from(self.collateral_haircut_bps),
                core::num::NonZeroU64::new(10_000).expect("basis-point denominator is non-zero"),
                crate::XOR_QUANTITY_SCALE,
                RoundingMode::Ceil,
            )
            .map_err(SwapLineError::from)
    }
    /// Evaluate utilisation as an exact decimal rounded to 28 fractional
    /// digits using nearest-even ties.
    pub fn utilisation(&self, outstanding: &XorQuantity) -> Result<Numeric, SwapLineError> {
        self.validate()?;
        outstanding
            .as_quantity()
            .try_ratio_round(
                self.limit_xor.as_quantity(),
                crate::MAX_DECIMAL_SCALE,
                RoundingMode::NearestEven,
            )
            .map_err(SwapLineError::from)
    }
}
/// Runtime view of a swap line, tracking outstanding notional and collateral.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SwapLineExposure {
    /// Current XOR borrowed via the swap line.
    pub outstanding_xor: XorQuantity,
    /// Collateral posted against the swap line.
    pub collateral_value: XorQuantity,
}
impl SwapLineExposure {
    /// Determine whether utilisation and collateral satisfy the configuration.
    pub fn is_healthy(&self, config: &SwapLineConfig) -> Result<bool, SwapLineError> {
        config.validate()?;
        if self.outstanding_xor > config.limit_xor {
            return Ok(false);
        }
        let required = config.required_collateral(&self.outstanding_xor)?;
        Ok(self.collateral_value >= required)
    }
}
#[cfg(test)]
mod tests {
    use super::{CollateralKind, SwapLineConfig, SwapLineError, SwapLineExposure, SwapLineId};
    use crate::{Numeric, XorQuantity};
    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    fn config() -> SwapLineConfig {
        SwapLineConfig {
            id: SwapLineId(7),
            limit_xor: xor("1000000"),
            collateral_haircut_bps: 100,
            fee_rate_bps: 250,
            collateral_kind: CollateralKind::Cbdc,
            uses_fee_schedule: false,
        }
    }
    #[test]
    fn utilisation_and_health_are_exact() {
        let config = config();
        let exposure = SwapLineExposure {
            outstanding_xor: xor("400000.000000001"),
            collateral_value: xor("404000.000000002"),
        };
        assert!(
            config
                .utilisation(&exposure.outstanding_xor)
                .expect("utilisation")
                < Numeric::one()
        );
        assert_eq!(exposure.is_healthy(&config), Ok(true));
    }
    #[test]
    fn rejects_zero_limit_instead_of_reporting_zero_utilisation() {
        let mut invalid = config();
        invalid.limit_xor = XorQuantity::zero();
        assert_eq!(
            invalid.utilisation(&xor("1")),
            Err(SwapLineError::ZeroLimit)
        );
        assert_eq!(
            SwapLineExposure {
                outstanding_xor: XorQuantity::zero(),
                collateral_value: XorQuantity::zero(),
            }
            .is_healthy(&invalid),
            Err(SwapLineError::ZeroLimit)
        );
    }
    #[test]
    fn over_limit_and_undercollateralized_exposures_are_unhealthy() {
        let config = config();
        assert_eq!(
            SwapLineExposure {
                outstanding_xor: xor("1000000.000000001"),
                collateral_value: xor("999999999"),
            }
            .is_healthy(&config),
            Ok(false)
        );
        assert_eq!(
            SwapLineExposure {
                outstanding_xor: xor("10"),
                collateral_value: xor("10.099999999"),
            }
            .is_healthy(&config),
            Ok(false)
        );
    }
}
