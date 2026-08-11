//! Subscription app API DTOs and helpers.

use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    nft::NftId,
    subscription::{SubscriptionInvoice, SubscriptionPlan, SubscriptionState},
    trigger::TriggerId,
};
use iroha_primitives::numeric::Quantity;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Request payload for creating a subscription plan.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanCreateRequest {
    /// Account authorizing the transaction (plan provider).
    pub authority: AccountId,
    /// Asset definition id used to store the plan metadata.
    pub plan_id: AssetDefinitionId,
    /// Subscription plan payload stored on the asset definition.
    pub plan: SubscriptionPlan,
}

/// Unsigned transaction draft returned for subscription plan registration.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionPlanCreateResponse {
    /// Always false because Torii does not submit the transaction.
    pub submitted: bool,
    /// Plan asset definition id.
    pub plan_id: AssetDefinitionId,
    /// Canonical Norito transaction payload encoded as padded base64.
    pub transaction_payload_b64: String,
    /// Transaction-payload hash encoded as padded base64.
    pub signing_message_b64: String,
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
    /// Count mode: "bounded" omits exact totals; "exact" preserves total counts.
    pub count_mode: Option<String>,
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
    /// Total number of matching plans when `count_mode` is "exact".
    pub total: Option<u64>,
    /// Whether more items are available after this page.
    pub has_more: bool,
    /// Count mode used to produce pagination metadata.
    pub count_mode: String,
}

/// Request payload for creating a subscription.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionCreateRequest {
    /// Account authorizing the transaction (subscriber).
    pub authority: AccountId,
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

/// One canonical framed instruction returned for local signing.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionInstructionDraft {
    /// Registered instruction wire identifier.
    pub wire_id: String,
    /// Lowercase hexadecimal canonical framed instruction bytes.
    pub payload_hex: String,
}

/// Exact unsigned subscription creation draft.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionCreateResponse {
    /// Response layout version.
    pub version: u16,
    /// Account that must sign the returned instructions.
    pub authority: AccountId,
    /// Exact mutation action (`create`).
    pub action: String,
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Plan asset definition bound to the subscription.
    pub plan_id: AssetDefinitionId,
    /// Billing trigger id assigned to the subscription.
    pub billing_trigger_id: TriggerId,
    /// Usage trigger id (present for usage plans).
    pub usage_trigger_id: Option<TriggerId>,
    /// First charge time in UTC milliseconds.
    pub first_charge_ms: u64,
    /// Whether the draft includes a provider usage-trigger grant.
    pub provider_usage_grant_included: bool,
    /// Exact subscription state produced by the draft.
    pub resulting_subscription: SubscriptionState,
    /// Canonical instructions for local transaction signing.
    pub tx_instructions: Vec<SubscriptionInstructionDraft>,
}

/// Query parameters for listing subscriptions.
#[derive(Clone, Debug, Default, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionListParams {
    /// Optional subscriber filter.
    pub owned_by: Option<String>,
    /// Optional provider filter.
    pub provider: Option<String>,
    /// Optional status filter (active, paused, `past_due`, canceled, suspended).
    pub status: Option<String>,
    /// Optional limit for pagination.
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    pub offset: u64,
    /// Count mode: "bounded" omits exact totals; "exact" preserves total counts.
    pub count_mode: Option<String>,
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
    /// Total number of matching subscriptions when `count_mode` is "exact".
    pub total: Option<u64>,
    /// Whether more items are available after this page.
    pub has_more: bool,
    /// Count mode used to produce pagination metadata.
    pub count_mode: String,
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
    /// Optional charge time override in UTC milliseconds.
    pub charge_at_ms: Option<u64>,
    /// Optional cancel mode (`immediate` or `period_end`) for cancel requests.
    pub cancel_mode: Option<SubscriptionCancelMode>,
}

/// Cancelation mode for subscription cancel requests.
#[derive(Clone, Copy, Debug, JsonDeserialize, JsonSerialize, PartialEq, Eq)]
#[norito(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum SubscriptionCancelMode {
    /// Cancel the subscription immediately.
    Immediate,
    /// Cancel the subscription at the end of the current billing period.
    PeriodEnd,
}

/// Request payload for recording subscription usage.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionUsageRequest {
    /// Account authorizing the transaction (usage reporter).
    pub authority: AccountId,
    /// Usage counter key to update.
    pub unit_key: Name,
    /// Non-negative usage increment.
    pub delta: Quantity,
    /// Optional usage trigger id; derived when omitted.
    pub usage_trigger_id: Option<TriggerId>,
}

/// Exact projected details of a subscription action draft.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionActionDraftDetails {
    /// Billing trigger affected by the action.
    pub billing_trigger_id: TriggerId,
    /// Exact trigger operation.
    pub billing_trigger_operation: String,
    /// Resolved charge time for resume and charge-now actions.
    pub effective_charge_ms: Option<u64>,
    /// Explicit cancellation mode for cancel actions.
    pub cancel_mode: Option<SubscriptionCancelMode>,
    /// Exact subscription state produced by the draft.
    pub resulting_subscription: SubscriptionState,
}

/// Exact unsigned subscription action draft.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionActionResponse {
    /// Response layout version.
    pub version: u16,
    /// Account that must sign the returned instructions.
    pub authority: AccountId,
    /// Exact action name.
    pub action: String,
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Exact projected action details.
    pub details: SubscriptionActionDraftDetails,
    /// Canonical instructions for local transaction signing.
    pub tx_instructions: Vec<SubscriptionInstructionDraft>,
}

/// Unsigned transaction draft for recording subscription usage.
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
pub struct SubscriptionUsageResponse {
    /// Always false because Torii does not submit the transaction.
    pub submitted: bool,
    /// Subscription NFT id.
    pub subscription_id: NftId,
    /// Canonical Norito transaction payload encoded as padded base64.
    pub transaction_payload_b64: String,
    /// Transaction-payload hash encoded as padded base64.
    pub signing_message_b64: String,
}
