//! Helpers for emitting deterministic Nexus settlement evidence.
//!
//! This module provides conversions from the internal settlement router
//! parameters to the Norito-friendly data model types published as part of the
//! `LaneBlockCommitment` payload.

use iroha_data_model::block::consensus::{
    LaneLiquidityProfile, LaneSwapMetadata, LaneVolatilityClass,
};
use rust_decimal::Decimal;
use settlement_router::{VolatilityBucket, haircut::LiquidityProfile};

/// Deterministic snapshot of the conversion parameters used for XOR settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapEvidence {
    /// Basis-point safety margin applied when quoting XOR dues.
    pub epsilon_bps: u16,
    /// TWAP window length expressed in seconds.
    pub twap_window_seconds: u32,
    /// Liquidity profile guiding haircut tier selection.
    pub liquidity_profile: LiquidityProfile,
    /// Time-weighted price expressed as local token units per XOR.
    pub twap_local_per_xor: Decimal,
    /// Volatility bucket recorded when adding extra safety margin.
    pub volatility_bucket: VolatilityBucket,
}

impl SwapEvidence {
    /// Convert the evidence into a Norito-friendly metadata structure.
    #[must_use]
    pub fn into_lane_metadata(self) -> LaneSwapMetadata {
        LaneSwapMetadata {
            epsilon_bps: self.epsilon_bps,
            twap_window_seconds: self.twap_window_seconds,
            liquidity_profile: convert_liquidity_profile(self.liquidity_profile),
            twap_local_per_xor: decimal_to_canonical_string(&self.twap_local_per_xor),
            volatility_class: convert_volatility_bucket(self.volatility_bucket),
        }
    }

    /// Borrowing variant of [`SwapEvidence::into_lane_metadata`].
    #[must_use]
    pub fn to_lane_metadata(&self) -> LaneSwapMetadata {
        (*self).into_lane_metadata()
    }
}

/// Convert the settlement-router liquidity profile into the public data-model enum.
#[must_use]
pub fn convert_liquidity_profile(profile: LiquidityProfile) -> LaneLiquidityProfile {
    match profile {
        LiquidityProfile::Tier1 => LaneLiquidityProfile::Tier1,
        LiquidityProfile::Tier2 => LaneLiquidityProfile::Tier2,
        LiquidityProfile::Tier3 => LaneLiquidityProfile::Tier3,
    }
}

/// Convert the settlement-router volatility bucket into the public data-model enum.
#[must_use]
pub fn convert_volatility_bucket(bucket: VolatilityBucket) -> LaneVolatilityClass {
    match bucket {
        VolatilityBucket::Stable => LaneVolatilityClass::Stable,
        VolatilityBucket::Elevated => LaneVolatilityClass::Elevated,
        VolatilityBucket::Dislocated => LaneVolatilityClass::Dislocated,
    }
}

fn decimal_to_canonical_string(value: &Decimal) -> String {
    let mut output = value.to_string();
    if let Some(dot_pos) = output.find('.') {
        while output.ends_with('0') {
            output.pop();
        }
        if output.ends_with('.') {
            output.pop();
        }
        if output.len() == dot_pos {
            output.push('0');
        }
    }
    if output.is_empty() {
        output.push('0');
    }
    output
}

#[cfg(test)]
mod tests {
    use iroha_data_model::block::consensus::LaneVolatilityClass;
    use rust_decimal::Decimal;
    use settlement_router::{VolatilityBucket, haircut::LiquidityProfile};

    use super::{SwapEvidence, convert_liquidity_profile, convert_volatility_bucket};

    #[test]
    fn converts_liquidity_profile() {
        assert!(matches!(
            convert_liquidity_profile(LiquidityProfile::Tier1),
            iroha_data_model::block::consensus::LaneLiquidityProfile::Tier1
        ));
        assert!(matches!(
            convert_liquidity_profile(LiquidityProfile::Tier2),
            iroha_data_model::block::consensus::LaneLiquidityProfile::Tier2
        ));
        assert!(matches!(
            convert_liquidity_profile(LiquidityProfile::Tier3),
            iroha_data_model::block::consensus::LaneLiquidityProfile::Tier3
        ));
    }

    #[test]
    fn swap_evidence_to_metadata() {
        let evidence = SwapEvidence {
            epsilon_bps: 25,
            twap_window_seconds: 60,
            liquidity_profile: LiquidityProfile::Tier2,
            twap_local_per_xor: Decimal::new(1250, 2), // 12.50
            volatility_bucket: VolatilityBucket::Elevated,
        };

        let metadata = evidence.to_lane_metadata();
        assert_eq!(metadata.epsilon_bps, 25);
        assert_eq!(metadata.twap_window_seconds, 60);
        assert_eq!(metadata.twap_local_per_xor, "12.5");
        assert!(matches!(
            metadata.volatility_class,
            LaneVolatilityClass::Elevated
        ));
    }

    #[test]
    fn converts_volatility_bucket() {
        assert!(matches!(
            convert_volatility_bucket(VolatilityBucket::Stable),
            LaneVolatilityClass::Stable
        ));
        assert!(matches!(
            convert_volatility_bucket(VolatilityBucket::Dislocated),
            LaneVolatilityClass::Dislocated
        ));
    }
}
