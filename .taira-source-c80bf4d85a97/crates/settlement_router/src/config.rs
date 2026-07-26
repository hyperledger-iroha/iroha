//! Static configuration for the settlement router.

use derive_more::Display;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use time::Duration;

/// Safety margin expressed in basis points (1/100th of a percent).
///
/// The value is applied on top of the shadow price so inclusion always
/// over-collects slightly and avoids falling short once conversions execute.
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
#[display("{} bps", value)]
pub struct EpsilonBps {
    value: u16,
}

impl EpsilonBps {
    /// Basis points representing 100% (used when circuit breakers clamp the
    /// price).
    pub const FULL: Self = Self { value: 10_000 };

    /// Construct a new margin expressed in basis points.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self { value }
    }

    /// Raw basis point value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.value
    }
}

/// Static knobs controlling shadow-price computation and buffer sizing.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SettlementConfig {
    /// Rolling window used for TWAP quotes.
    pub twap_window: crate::DurationSeconds,
    /// Base epsilon (safety margin) added to the computed XOR due.
    pub epsilon: EpsilonBps,
    /// Maximum amount of time the DS buffer is allowed to cover in hours.
    pub buffer_horizon_hours: u16,
}

impl SettlementConfig {
    /// Default configuration matching the roadmap baseline (60s TWAP, 25 bps
    /// epsilon, 72 h buffer horizon).
    #[must_use]
    pub const fn roadmap_default() -> Self {
        Self {
            twap_window: crate::DurationSeconds::new(Duration::seconds(60)),
            epsilon: EpsilonBps::new(25),
            buffer_horizon_hours: 72,
        }
    }
}
