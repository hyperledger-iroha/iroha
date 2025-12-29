//! Social incentive helpers and escrow records for viral reward flows.

use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, oracle::KeyedHash};

/// Rolling reward budget tracked per Unix day (milliseconds / `86_400_000`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralRewardBudget {
    /// Day identifier derived from `timestamp_ms / 86_400_000`.
    pub day: u64,
    /// Amount already spent for the day.
    pub spent: Numeric,
}

impl Default for ViralRewardBudget {
    fn default() -> Self {
        Self {
            day: 0,
            spent: Numeric::zero(),
        }
    }
}

/// Daily reward counter for a UAID.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralDailyCounter {
    /// Day identifier derived from `timestamp_ms / 86_400_000`.
    pub day: u64,
    /// Number of rewards claimed within the day.
    pub claims: u32,
}

/// Rolling promotion budget tracked across the entire campaign (not reset daily).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralCampaignBudget {
    /// Total amount spent since the campaign began.
    pub spent: Numeric,
}

impl Default for ViralCampaignBudget {
    fn default() -> Self {
        Self {
            spent: Numeric::zero(),
        }
    }
}

/// Pending escrow created by `SendToTwitter`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralEscrowRecord {
    /// Targeted Twitter binding hash (pseudonymous).
    pub binding_hash: KeyedHash,
    /// Account that funded the escrow.
    pub sender: AccountId,
    /// Amount held in escrow.
    pub amount: Numeric,
    /// Unix timestamp (milliseconds) when the escrow was created.
    pub created_at_ms: u64,
}

/// Prelude exports for social incentive helpers.
pub mod prelude {
    pub use super::{ViralCampaignBudget, ViralDailyCounter, ViralEscrowRecord, ViralRewardBudget};
}
