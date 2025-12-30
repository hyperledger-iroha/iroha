//! Volatility bucketing derived from oracle signals.

use derive_more::Display;
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
    Display,
    Eq,
    PartialEq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "bucket", content = "value")]
pub enum VolatilityBucket {
    /// Normal trading conditions (no extra margin).
    #[default]
    #[display("stable")]
    Stable,
    /// Elevated volatility (add one 25 bps step).
    #[display("elevated")]
    Elevated,
    /// Dislocated markets (add two 25 bps steps).
    #[display("dislocated")]
    Dislocated,
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
