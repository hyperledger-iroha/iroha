//! Volatility bucketing derived from oracle signals.
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
/// Rolling-volatility classification that drives extra settlement margin.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "bucket", content = "value")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "settlement_router::volatility::VolatilityBucket")]
pub enum VolatilityBucket {
    /// Normal trading conditions (no extra margin).
    #[default]
    Stable,
    /// Elevated volatility (add one 25 bps step).
    Elevated,
    /// Dislocated markets (add two 25 bps steps).
    Dislocated,
}
impl core::fmt::Display for VolatilityBucket {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Stable => "stable",
            Self::Elevated => "elevated",
            Self::Dislocated => "dislocated",
        })
    }
}
impl VolatilityBucket {
    /// Additional safety margin expressed in basis points (25 bps steps).
    #[must_use]
    pub const fn extra_margin_bps(self) -> u16 {
        match self {
            Self::Stable => 0,
            Self::Elevated => 25,
            Self::Dislocated => 50,
        }
    }
    /// Maximum total epsilon (base + bucket) expressed in basis points.
    #[must_use]
    pub const fn max_total_margin_bps() -> u16 {
        75
    }
}
