//! Events emitted by viral incentive flows (Twitter follow rewards and escrows).

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    nexus::UniversalAccountId,
    oracle::KeyedHash,
    social::{ViralCampaignBudget, ViralEscrowRecord, ViralRewardBudget},
};

/// Social incentive lifecycle events.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, derive_more::From,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "event", content = "payload"))]
pub enum SocialEvent {
    /// Promotional reward paid for a valid binding.
    RewardPaid(ViralRewardApplied),
    /// Escrow created for a target handle.
    EscrowCreated(ViralEscrowCreated),
    /// Escrow delivered to a newly bound UAID.
    EscrowReleased(ViralEscrowReleased),
    /// Escrow cancelled and refunded to the sender.
    EscrowCancelled(ViralEscrowCancelled),
}

/// Reward payment emitted when a binding claim succeeds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralRewardApplied {
    /// UAID receiving the payout.
    pub uaid: UniversalAccountId,
    /// Account credited for the reward.
    pub account: AccountId,
    /// Binding hash that proved the follow.
    pub binding_hash: KeyedHash,
    /// Amount transferred.
    pub amount: iroha_primitives::numeric::Numeric,
    /// Budget snapshot after the payout.
    pub budget: ViralRewardBudget,
    /// Campaign-wide spend snapshot after the payout (tracked even when the cap is unlimited).
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub campaign: Option<ViralCampaignBudget>,
    /// Campaign cap configured for the promo (0 = unlimited).
    pub campaign_cap: iroha_primitives::numeric::Numeric,
    /// Whether the promotion window was active for this payout.
    pub promo_active: bool,
    /// Whether governance halted viral incentives at the time of payout.
    pub halted: bool,
    /// Unix timestamp (milliseconds) when the reward was recorded.
    pub recorded_at_ms: u64,
}

/// Escrow creation event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralEscrowCreated {
    /// Escrow record captured at creation.
    pub escrow: ViralEscrowRecord,
}

/// Escrow delivery event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralEscrowReleased {
    /// Delivered escrow record.
    pub escrow: ViralEscrowRecord,
    /// UAID bound to the handle when released.
    pub uaid: UniversalAccountId,
    /// Account credited for the release.
    pub account: AccountId,
    /// Whether the sender bonus was paid.
    pub bonus_paid: bool,
    /// Unix timestamp (milliseconds) when the release was recorded.
    pub released_at_ms: u64,
}

/// Escrow cancellation/refund event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ViralEscrowCancelled {
    /// Escrow record being refunded.
    pub escrow: ViralEscrowRecord,
    /// Unix timestamp (milliseconds) when the refund was recorded.
    pub cancelled_at_ms: u64,
}

/// Prelude exports for social incentive events.
pub mod prelude {
    pub use super::{
        SocialEvent, ViralEscrowCancelled, ViralEscrowCreated, ViralEscrowReleased,
        ViralRewardApplied,
    };
}
