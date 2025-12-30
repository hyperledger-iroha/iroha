//! Repo/reverse-repo style swap lines that supplement AMM liquidity.

use derive_more::{Display, From};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use rust_decimal::Decimal;

use crate::MicroXor;

/// Uniquely identifies a swap line (per dataspace + collateral flavour).
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
    /// The DS's native CBDC.
    #[display("cbdc")]
    Cbdc,
    /// XOR posted by the treasury (reverse repo).
    #[display("xor")]
    Xor,
    /// Governance-approved stablecoin.
    #[display("stable")]
    Stable,
}

/// Static configuration for a swap line.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SwapLineConfig {
    /// Identifier referenced in manifests and telemetry.
    pub id: SwapLineId,
    /// Maximum outstanding XOR that can be borrowed at once.
    pub limit_xor: MicroXor,
    /// Minimum collateral ratio (haircut) expressed in basis points.
    pub collateral_haircut_bps: u16,
    /// Interest or fee schedule in basis points per annum.
    pub fee_rate_bps: u16,
    /// Collateral kind posted by the counterparty.
    pub collateral_kind: CollateralKind,
    /// Whether the facility uses fee-based remuneration (Shariah-compliant) or
    /// classic interest accrual.
    pub uses_fee_schedule: bool,
}

impl SwapLineConfig {
    /// Return the minimum collateral required for a given outstanding balance.
    #[must_use]
    pub fn required_collateral(&self, outstanding: &MicroXor) -> MicroXor {
        let haircut_multiplier =
            Decimal::from(10_000_u16 + self.collateral_haircut_bps) / Decimal::from(10_000_u16);
        MicroXor::from(outstanding.into_decimal() * haircut_multiplier)
    }

    /// Evaluate utilisation (ratio 0-1) based on the current outstanding amount.
    #[must_use]
    pub fn utilisation(&self, outstanding: &MicroXor) -> Decimal {
        if self.limit_xor.is_zero() {
            Decimal::ZERO
        } else {
            outstanding.into_decimal() / self.limit_xor.into_decimal()
        }
    }
}

/// Runtime view of a swap line, tracking outstanding notional and collateral.
#[derive(Clone, Debug, Eq, PartialEq, JsonSerialize, JsonDeserialize)]
pub struct SwapLineExposure {
    /// Current XOR borrowed via the swap line.
    pub outstanding_xor: MicroXor,
    /// Collateral posted against the swap line.
    pub collateral_value: MicroXor,
}

impl SwapLineExposure {
    /// Determine whether the swap line is within the governed limits.  Returns
    /// `true` when both utilisation and collateral ratio are acceptable.
    #[must_use]
    pub fn is_healthy(&self, config: &SwapLineConfig) -> bool {
        let utilisation = config.utilisation(&self.outstanding_xor);
        if utilisation > Decimal::ONE {
            return false;
        }

        let required = config.required_collateral(&self.outstanding_xor);
        self.collateral_value >= required
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{CollateralKind, SwapLineConfig, SwapLineExposure, SwapLineId};
    use crate::MicroXor;

    fn micro(value: i64) -> MicroXor {
        MicroXor::new(Decimal::from(value))
    }

    #[test]
    fn utilisation_and_health() {
        let config = SwapLineConfig {
            id: SwapLineId(7),
            limit_xor: micro(1_000_000),
            collateral_haircut_bps: 100,
            fee_rate_bps: 250,
            collateral_kind: CollateralKind::Cbdc,
            uses_fee_schedule: false,
        };

        let exposure = SwapLineExposure {
            outstanding_xor: micro(400_000),
            collateral_value: micro(450_000),
        };

        assert!(config.utilisation(&exposure.outstanding_xor) < Decimal::ONE);
        assert!(exposure.is_healthy(&config));
    }
}
