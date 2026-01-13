//! Subscription metadata schemas for trigger-based billing.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    AssetDefinitionId, DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId, name::Name,
    nft::NftId, trigger::TriggerId,
};

/// Metadata key storing a subscription plan payload on an asset definition.
pub const SUBSCRIPTION_PLAN_METADATA_KEY: &str = "subscription_plan";
/// Metadata key storing a subscription state payload on an NFT.
pub const SUBSCRIPTION_METADATA_KEY: &str = "subscription";
/// Metadata key storing a subscription invoice payload on an NFT.
pub const SUBSCRIPTION_INVOICE_METADATA_KEY: &str = "subscription_invoice";
/// Metadata key storing a trigger reference to a subscription.
pub const SUBSCRIPTION_TRIGGER_REF_METADATA_KEY: &str = "subscription_ref";

/// Subscription plan metadata stored on asset definitions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionPlan {
    /// Provider account issuing the plan.
    pub provider: AccountId,
    /// Billing schedule and retry rules.
    pub billing: SubscriptionBilling,
    /// Pricing rules for fixed or usage billing.
    pub pricing: SubscriptionPricing,
}

/// Billing schedule and retry policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionBilling {
    /// Cadence definition.
    pub cadence: SubscriptionCadence,
    /// Whether the charge covers the previous or next period.
    pub bill_for: SubscriptionBillFor,
    /// Retry delay in milliseconds for failed charges.
    pub retry_backoff_ms: u64,
    /// Maximum failure count before suspension.
    pub max_failures: u32,
    /// Grace window in milliseconds before suspension.
    pub grace_ms: u64,
}

/// Calendar-month cadence detail payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionMonthlyCalendarCadence {
    /// Anchor day for calendar-month cadence (1..=31).
    pub anchor_day: u8,
    /// Anchor time-of-day in UTC milliseconds from 00:00.
    pub anchor_time_ms: u32,
}

/// Fixed-period cadence detail payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionFixedPeriodCadence {
    /// Fixed period in milliseconds for non-calendar cadence.
    pub period_ms: u64,
}

/// Billing cadence settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum SubscriptionCadence {
    /// Calendar-month cadence with an anchor day/time (UTC).
    MonthlyCalendar(SubscriptionMonthlyCalendarCadence),
    /// Fixed-length cadence expressed in milliseconds.
    FixedPeriod(SubscriptionFixedPeriodCadence),
}

/// Selects which billing period the charge applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "period", content = "value", rename_all = "snake_case")
)]
pub enum SubscriptionBillFor {
    /// Charge for the period that just ended.
    PreviousPeriod,
    /// Charge for the upcoming period.
    NextPeriod,
}

/// Fixed-amount pricing detail payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionFixedPricing {
    /// Fixed amount for the period.
    pub amount: Numeric,
    /// Asset definition used for charging.
    pub asset_definition: AssetDefinitionId,
}

/// Usage-based pricing detail payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionUsagePricing {
    /// Unit price for usage billing.
    pub unit_price: Numeric,
    /// Usage accumulator key for usage billing.
    pub unit_key: Name,
    /// Asset definition used for charging.
    pub asset_definition: AssetDefinitionId,
}

/// Usage increment payload for subscription usage recording.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionUsageDelta {
    /// Subscription NFT identifier.
    pub subscription_nft_id: NftId,
    /// Usage counter key to update.
    pub unit_key: Name,
    /// Usage increment (must be non-negative).
    pub delta: Numeric,
}

/// Pricing rules for a subscription plan.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum SubscriptionPricing {
    /// Fixed-amount pricing.
    Fixed(SubscriptionFixedPricing),
    /// Usage-based pricing.
    Usage(SubscriptionUsagePricing),
}

/// Subscription state stored on a subscription NFT.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionState {
    /// Asset definition ID of the plan.
    pub plan_id: AssetDefinitionId,
    /// Provider account issuing the plan.
    pub provider: AccountId,
    /// Subscriber account.
    pub subscriber: AccountId,
    /// Current subscription status.
    pub status: SubscriptionStatus,
    /// Current period start time (UTC ms).
    pub current_period_start_ms: u64,
    /// Current period end time (UTC ms).
    pub current_period_end_ms: u64,
    /// Next charge time (UTC ms).
    pub next_charge_ms: u64,
    /// Whether the subscription is scheduled to cancel at period end.
    #[norito(default)]
    pub cancel_at_period_end: bool,
    /// Timestamp in UTC ms when the subscription should cancel.
    #[norito(default)]
    pub cancel_at_ms: Option<u64>,
    /// Consecutive failure count.
    pub failure_count: u32,
    /// Usage counters accumulated during the period.
    #[norito(default)]
    pub usage_accumulated: BTreeMap<Name, Numeric>,
    /// Billing trigger ID.
    pub billing_trigger_id: TriggerId,
}

/// Status of a subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum SubscriptionStatus {
    /// Subscription is active and billing.
    Active,
    /// Subscription is paused.
    Paused,
    /// Subscription is past due and retrying.
    PastDue,
    /// Subscription is canceled.
    Canceled,
    /// Subscription is suspended after failures.
    Suspended,
}

/// Trigger metadata referencing the subscription NFT.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionTriggerRef {
    /// Subscription NFT identifier.
    pub subscription_nft_id: NftId,
}

/// Subscription invoice metadata stored on subscription or invoice NFTs.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SubscriptionInvoice {
    /// Subscription NFT identifier.
    pub subscription_nft_id: NftId,
    /// Period start (UTC ms).
    pub period_start_ms: u64,
    /// Period end (UTC ms).
    pub period_end_ms: u64,
    /// Timestamp of the charge attempt (UTC ms).
    pub attempted_at_ms: u64,
    /// Charged amount.
    pub amount: Numeric,
    /// Asset definition charged.
    pub asset_definition: AssetDefinitionId,
    /// Invoice status.
    pub status: SubscriptionInvoiceStatus,
    /// Optional transaction hash for successful charges.
    #[norito(default)]
    pub tx_hash: Option<Hash>,
}

/// Status of a subscription invoice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum SubscriptionInvoiceStatus {
    /// Charge succeeded and funds moved.
    Paid,
    /// Charge failed or was rejected.
    Failed,
}

/// Re-exports of commonly used subscription types.
pub mod prelude {
    //! Subscription prelude re-exports.
    pub use super::{
        SubscriptionBillFor, SubscriptionBilling, SubscriptionCadence,
        SubscriptionFixedPeriodCadence, SubscriptionFixedPricing, SubscriptionInvoice,
        SubscriptionInvoiceStatus, SubscriptionMonthlyCalendarCadence, SubscriptionPlan,
        SubscriptionPricing, SubscriptionState, SubscriptionStatus, SubscriptionTriggerRef,
        SubscriptionUsageDelta, SubscriptionUsagePricing,
    };
}
