//! Subscription app API DTOs and helpers.

use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    nft::NftId,
    prelude::ExposedPrivateKey,
    subscription::{SubscriptionInvoice, SubscriptionPlan, SubscriptionState},
    trigger::TriggerId,
};
use iroha_primitives::numeric::Numeric;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Request payload for creating a subscription plan.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanCreateRequest {
    /// Account authorizing the transaction (plan provider).
    pub authority: AccountId,
    /// Signing key for submitting the transaction.
    pub private_key: ExposedPrivateKey,
    /// Asset definition id used to store the plan metadata.
    pub plan_id: AssetDefinitionId,
    /// Subscription plan payload stored on the asset definition.
    pub plan: SubscriptionPlan,
}

/// Response payload returned after registering a subscription plan.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanCreateResponse {
    /// Whether the plan registration succeeded.
    pub ok: bool,
    /// Plan asset definition id.
    pub plan_id: AssetDefinitionId,
    /// Hex-encoded transaction hash submitted to the queue.
    pub tx_hash_hex: String,
}

/// Query parameters for listing subscription plans.
#[derive(Clone, Debug, Default, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanListParams {
    /// Optional plan provider filter.
    pub provider: Option<String>,
    /// Optional limit for pagination.
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    pub offset: u64,
}

/// Subscription plan list item.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanListItem {
    /// Plan asset definition id.
    pub plan_id: AssetDefinitionId,
    /// Plan metadata payload.
    pub plan: SubscriptionPlan,
}

/// Response payload for listing subscription plans.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanListResponse {
    /// Plan items.
    pub items: Vec<SubscriptionPlanListItem>,
    /// Total number of matching plans.
    pub total: u64,
}

/// Request payload for creating a subscription.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionCreateRequest {
    /// Account authorizing the transaction (subscriber).
    pub authority: AccountId,
    /// Signing key for submitting the transaction.
    pub private_key: ExposedPrivateKey,
    /// Subscription NFT id to register.
    pub subscription_id: NftId,
    /// Asset definition id for the subscription plan.
    pub plan_id: AssetDefinitionId,
    /// Optional billing trigger id; derived when omitted.
    pub billing_trigger_id: Option<TriggerId>,
    /// Optional usage trigger id for usage plans; derived when omitted.
    pub usage_trigger_id: Option<TriggerId>,
    /// Optional first charge timestamp in UTC milliseconds.
    pub first_charge_ms: Option<u64>,
    /// Grant `CanExecuteTrigger` to the plan provider for usage recording.
    pub grant_usage_to_provider: Option<bool>,
}

/// Response payload returned after creating a subscription.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionCreateResponse {
    /// Whether the subscription creation succeeded.
    pub ok: bool,
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Billing trigger id assigned to the subscription.
    pub billing_trigger_id: TriggerId,
    /// Usage trigger id (present for usage plans).
    pub usage_trigger_id: Option<TriggerId>,
    /// First charge time in UTC milliseconds.
    pub first_charge_ms: u64,
    /// Hex-encoded transaction hash submitted to the queue.
    pub tx_hash_hex: String,
}

/// Query parameters for listing subscriptions.
#[derive(Clone, Debug, Default, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionListParams {
    /// Optional subscriber filter.
    pub owned_by: Option<String>,
    /// Optional provider filter.
    pub provider: Option<String>,
    /// Optional status filter (active, paused, past_due, canceled, suspended).
    pub status: Option<String>,
    /// Optional limit for pagination.
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    pub offset: u64,
}

/// Subscription list item payload.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionListItem {
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Subscription state metadata.
    pub subscription: SubscriptionState,
    /// Optional latest invoice metadata.
    pub invoice: Option<SubscriptionInvoice>,
    /// Optional plan metadata payload.
    pub plan: Option<SubscriptionPlan>,
}

/// Response payload for listing subscriptions.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionListResponse {
    /// Subscription items.
    pub items: Vec<SubscriptionListItem>,
    /// Total number of matching subscriptions.
    pub total: u64,
}

/// Response payload for fetching a subscription.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionGetResponse {
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Subscription state metadata.
    pub subscription: SubscriptionState,
    /// Optional latest invoice metadata.
    pub invoice: Option<SubscriptionInvoice>,
    /// Optional plan metadata payload.
    pub plan: Option<SubscriptionPlan>,
}

/// Request payload for subscription status updates.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionActionRequest {
    /// Account authorizing the transaction (subscriber).
    pub authority: AccountId,
    /// Signing key for submitting the transaction.
    pub private_key: ExposedPrivateKey,
    /// Optional charge time override in UTC milliseconds.
    pub charge_at_ms: Option<u64>,
}

/// Request payload for recording subscription usage.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionUsageRequest {
    /// Account authorizing the transaction (usage reporter).
    pub authority: AccountId,
    /// Signing key for submitting the transaction.
    pub private_key: ExposedPrivateKey,
    /// Usage counter key to update.
    pub unit_key: Name,
    /// Usage increment (must be non-negative).
    pub delta: Numeric,
    /// Optional usage trigger id; derived when omitted.
    pub usage_trigger_id: Option<TriggerId>,
}

/// Response payload for subscription actions.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionActionResponse {
    /// Whether the action succeeded.
    pub ok: bool,
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Hex-encoded transaction hash submitted to the queue.
    pub tx_hash_hex: String,
}
