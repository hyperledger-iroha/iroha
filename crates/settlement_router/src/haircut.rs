//! Liquidity haircut tiers applied to conversion results.

use derive_more::{Display, From};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};

/// Available liquidity profiles for deterministic haircut application.
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
    From,
)]
#[norito(tag = "profile", content = "value")]
pub enum LiquidityProfile {
    /// Deep pools with negligible slippage.
    #[display("tier1-deep")]
    Tier1,
    /// Medium depth pools with moderate slippage.
    #[display("tier2-medium")]
    Tier2,
    /// Thin pools or credit-constrained venues.
    #[display("tier3-thin")]
    Tier3,
}

impl LiquidityProfile {
    /// Return the default haircut in basis points for the profile.
    #[must_use]
    pub const fn haircut_bps(self) -> u16 {
        match self {
            Self::Tier1 => 0,
            Self::Tier2 => 25,
            Self::Tier3 => 75,
        }
    }
}

/// Haircut tier derived from a liquidity profile and governance overrides.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct HaircutTier {
    profile: LiquidityProfile,
    /// Explicit override in basis points, when governance tightens/loosens a
    /// profile due to market conditions.
    override_bps: Option<u16>,
}

impl HaircutTier {
    /// Construct from a profile with no override.
    #[must_use]
    pub const fn new(profile: LiquidityProfile) -> Self {
        Self {
            profile,
            override_bps: None,
        }
    }

    /// Apply a governance override (in basis points).
    #[must_use]
    pub const fn with_override(mut self, value: u16) -> Self {
        self.override_bps = Some(value);
        self
    }

    /// Basis points to deduct from the realised XOR amount.
    #[must_use]
    pub const fn effective_bps(self) -> u16 {
        match self.override_bps {
            Some(value) => value,
            None => self.profile.haircut_bps(),
        }
    }
}
