//! Canonical subscription HTTP records shared by Torii and client SDKs.
//! Preparation responses carry unsigned drafts; they never establish submission or finality.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// First-release subscription mutation draft layout version.
pub const SUBSCRIPTION_MUTATION_DRAFT_VERSION_V1: u16 = 1;

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// One canonical framed instruction returned for local transaction signing.
#[norito(schema_name = "iroha_torii::routing::SubscriptionInstructionDraftDto")]
pub struct SubscriptionInstructionDraft {
    /// Registered instruction wire identifier.
    pub wire_id: String,
    /// Lowercase hexadecimal canonical framed instruction bytes.
    pub payload_hex: String,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Request payload for creating a subscription plan.
#[norito(deny_unknown_fields)]
#[norito(schema_name = "iroha_torii::routing::SubscriptionPlanCreateDto")]
pub struct SubscriptionPlanCreateRequest {
    /// Account authorizing the transaction (plan provider).
    pub authority: iroha_data_model::account::AccountId,
    /// Asset definition id used to store the plan metadata.
    pub plan_id: iroha_data_model::asset::AssetDefinitionId,
    /// Subscription plan payload stored on the asset definition.
    pub plan: iroha_data_model::subscription::SubscriptionPlan,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Canonical unsigned transaction draft for registering a subscription plan.
#[norito(schema_name = "iroha_torii::routing::SubscriptionPlanCreateResponseDto")]
pub struct SubscriptionPlanCreateResponse {
    /// Always `false`; Torii has not submitted this transaction.
    pub submitted: bool,
    /// Plan asset definition id.
    pub plan_id: iroha_data_model::asset::AssetDefinitionId,
    /// Canonical Norito `TransactionPayload` bytes encoded as padded base64.
    pub transaction_payload_b64: String,
    /// Signature message (`HashOf<TransactionPayload>`) encoded as padded base64.
    pub signing_message_b64: String,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Query parameters for listing subscription plans.
#[norito(schema_name = "iroha_torii::routing::SubscriptionPlanListParams")]
#[derive(Default)]
pub struct SubscriptionPlanListParams {
    /// Optional plan provider filter using a canonical I105 id or on-chain alias.
    pub provider: Option<String>,
    /// Optional limit for pagination.
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    #[norito(default)]
    pub offset: u64,
    /// Count mode: "bounded" omits exact totals; "exact" preserves total counts.
    #[norito(default)]
    pub count_mode: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Subscription plan list item.
#[norito(schema_name = "iroha_torii::routing::SubscriptionPlanListItem")]
pub struct SubscriptionPlanListItem {
    /// Plan asset definition id.
    pub plan_id: iroha_data_model::asset::AssetDefinitionId,
    /// Plan metadata payload.
    pub plan: iroha_data_model::subscription::SubscriptionPlan,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Response payload for listing subscription plans.
#[norito(schema_name = "iroha_torii::routing::SubscriptionPlanListResponseDto")]
pub struct SubscriptionPlanListResponse {
    /// Plan items.
    pub items: Vec<SubscriptionPlanListItem>,
    /// Total number of matching plans.
    #[norito(default)]
    pub total: Option<u64>,
    /// Whether more items are available after this page.
    pub has_more: bool,
    /// Count mode used to produce pagination metadata.
    pub count_mode: String,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request payload for creating a subscription.
#[norito(schema_name = "iroha_torii::routing::SubscriptionCreateDto")]
pub struct SubscriptionCreateRequest {
    /// Account authorizing the transaction (subscriber).
    pub authority: iroha_data_model::account::AccountId,
    /// Subscription NFT id to register.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Asset definition id for the subscription plan.
    pub plan_id: iroha_data_model::asset::AssetDefinitionId,
    /// Optional billing trigger id; derived when omitted.
    #[norito(default)]
    pub billing_trigger_id: Option<iroha_data_model::trigger::TriggerId>,
    /// Optional usage trigger id for usage plans; derived when omitted.
    #[norito(default)]
    pub usage_trigger_id: Option<iroha_data_model::trigger::TriggerId>,
    /// Optional first charge timestamp in UTC milliseconds.
    #[norito(default)]
    pub first_charge_ms: Option<u64>,
    /// Grant `CanExecuteTrigger` to the plan provider for usage recording.
    #[norito(default)]
    pub grant_usage_to_provider: Option<bool>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Exact unsigned subscription creation draft.
#[norito(schema_name = "iroha_torii::routing::SubscriptionCreateResponseDto")]
pub struct SubscriptionCreateResponse {
    /// Response layout version.
    pub version: u16,
    /// Account that must be used as the transaction authority.
    pub authority: iroha_data_model::account::AccountId,
    /// Exact mutation action (`create`).
    pub action: String,
    /// Subscription NFT id.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Plan asset definition bound to the subscription.
    pub plan_id: iroha_data_model::asset::AssetDefinitionId,
    /// Billing trigger id assigned to the subscription.
    pub billing_trigger_id: iroha_data_model::trigger::TriggerId,
    /// Usage trigger id (present for usage plans).
    #[norito(default)]
    pub usage_trigger_id: Option<iroha_data_model::trigger::TriggerId>,
    /// First charge time in UTC milliseconds.
    pub first_charge_ms: u64,
    /// Whether the draft includes a provider `CanExecuteTrigger` grant.
    pub provider_usage_grant_included: bool,
    /// Exact subscription state produced when the draft instructions commit.
    pub resulting_subscription: iroha_data_model::subscription::SubscriptionState,
    /// Canonical instructions the authority must sign and submit.
    pub tx_instructions: Vec<SubscriptionInstructionDraft>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Query parameters for listing subscriptions.
#[norito(schema_name = "iroha_torii::routing::SubscriptionListParams")]
#[derive(Default)]
pub struct SubscriptionListParams {
    /// Optional subscriber filter using a canonical I105 id or on-chain alias.
    pub owned_by: Option<String>,
    /// Optional provider filter using a canonical I105 id or on-chain alias.
    pub provider: Option<String>,
    /// Optional status filter (active, paused, past_due, canceled, suspended).
    pub status: Option<String>,
    /// Optional limit for pagination.
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    #[norito(default)]
    pub offset: u64,
    /// Count mode: "bounded" omits exact totals; "exact" preserves total counts.
    #[norito(default)]
    pub count_mode: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Subscription list item payload.
#[norito(schema_name = "iroha_torii::routing::SubscriptionListItem")]
pub struct SubscriptionListItem {
    /// Subscription NFT id.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Subscription state metadata.
    pub subscription: iroha_data_model::subscription::SubscriptionState,
    /// Optional latest invoice metadata.
    #[norito(default)]
    pub invoice: Option<iroha_data_model::subscription::SubscriptionInvoice>,
    /// Optional plan metadata payload.
    #[norito(default)]
    pub plan: Option<iroha_data_model::subscription::SubscriptionPlan>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Response payload for listing subscriptions.
#[norito(schema_name = "iroha_torii::routing::SubscriptionListResponseDto")]
pub struct SubscriptionListResponse {
    /// Subscription items.
    pub items: Vec<SubscriptionListItem>,
    /// Total number of matching subscriptions.
    #[norito(default)]
    pub total: Option<u64>,
    /// Whether more items are available after this page.
    pub has_more: bool,
    /// Count mode used to produce pagination metadata.
    pub count_mode: String,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Response payload for fetching a subscription.
#[norito(schema_name = "iroha_torii::routing::SubscriptionGetResponseDto")]
pub struct SubscriptionGetResponse {
    /// Subscription NFT id.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Subscription state metadata.
    pub subscription: iroha_data_model::subscription::SubscriptionState,
    /// Optional latest invoice metadata.
    #[norito(default)]
    pub invoice: Option<iroha_data_model::subscription::SubscriptionInvoice>,
    /// Optional plan metadata payload.
    #[norito(default)]
    pub plan: Option<iroha_data_model::subscription::SubscriptionPlan>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request payload for subscription status updates.
#[norito(schema_name = "iroha_torii::routing::SubscriptionActionDto")]
pub struct SubscriptionActionRequest {
    /// Account authorizing the transaction (subscriber).
    pub authority: iroha_data_model::account::AccountId,
    /// Optional charge time override in UTC milliseconds.
    #[norito(default)]
    pub charge_at_ms: Option<u64>,
    /// Optional cancel mode (`immediate` or `period_end`).
    #[norito(default)]
    pub cancel_mode: Option<SubscriptionCancelMode>,
}
#[derive(
    Clone,
    Debug,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(
    tag = "mode",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
/// Cancelation mode for subscription cancel requests.
#[norito(schema_name = "iroha_torii::routing::SubscriptionCancelMode")]
pub enum SubscriptionCancelMode {
    /// Cancel immediately.
    Immediate,
    /// Cancel at the end of the current billing period.
    PeriodEnd,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Request payload for recording subscription usage.
#[norito(deny_unknown_fields)]
#[norito(schema_name = "iroha_torii::routing::SubscriptionUsageRequestDto")]
pub struct SubscriptionUsageRequest {
    /// Account authorizing the transaction (usage reporter).
    pub authority: iroha_data_model::account::AccountId,
    /// Usage counter key to update.
    pub unit_key: iroha_data_model::name::Name,
    /// Non-negative usage increment.
    pub delta: iroha_primitives::numeric::Quantity,
    /// Optional usage trigger id; derived when omitted.
    #[norito(default)]
    pub usage_trigger_id: Option<iroha_data_model::trigger::TriggerId>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Exact details projected by a subscription action draft.
#[norito(schema_name = "iroha_torii::routing::SubscriptionActionDraftDetailsDto")]
pub struct SubscriptionActionDraftDetails {
    /// Billing trigger affected by the action.
    pub billing_trigger_id: iroha_data_model::trigger::TriggerId,
    /// Exact trigger operation (`none`, `register`, `unregister`, or `replace`).
    pub billing_trigger_operation: String,
    /// Resolved charge time for resume and charge-now actions.
    #[norito(default)]
    pub effective_charge_ms: Option<u64>,
    /// Explicit cancellation mode for cancel actions.
    #[norito(default)]
    pub cancel_mode: Option<SubscriptionCancelMode>,
    /// Exact subscription state produced when the draft instructions commit.
    pub resulting_subscription: iroha_data_model::subscription::SubscriptionState,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Exact unsigned subscription action draft.
#[norito(schema_name = "iroha_torii::routing::SubscriptionActionResponseDto")]
pub struct SubscriptionActionResponse {
    /// Response layout version.
    pub version: u16,
    /// Account that must be used as the transaction authority.
    pub authority: iroha_data_model::account::AccountId,
    /// Exact route action (`pause`, `resume`, `cancel`, `keep`, or `charge_now`).
    pub action: String,
    /// Subscription NFT id.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Exact projected action details.
    pub details: SubscriptionActionDraftDetails,
    /// Canonical instructions the authority must sign and submit.
    pub tx_instructions: Vec<SubscriptionInstructionDraft>,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
/// Canonical unsigned transaction draft for recording subscription usage.
#[norito(schema_name = "iroha_torii::routing::SubscriptionUsageResponseDto")]
pub struct SubscriptionUsageResponse {
    /// Always `false`; Torii has not submitted this transaction.
    pub submitted: bool,
    /// Subscription NFT id.
    pub subscription_id: iroha_data_model::nft::NftId,
    /// Canonical Norito `TransactionPayload` bytes encoded as padded base64.
    pub transaction_payload_b64: String,
    /// Signature message (`HashOf<TransactionPayload>`) encoded as padded base64.
    pub signing_message_b64: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_records_keep_their_exact_nominal_schema_names() {
        fn identity<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
            name: &str,
        ) {
            let expected = norito::core::schema_hash_for_name(name);
            assert_eq!(<T as norito::NoritoSerialize>::schema_hash(), expected);
            assert_eq!(
                <T as norito::NoritoDeserialize<'_>>::schema_hash(),
                expected
            );
        }
        identity::<SubscriptionInstructionDraft>(
            "iroha_torii::routing::SubscriptionInstructionDraftDto",
        );
        identity::<SubscriptionPlanCreateRequest>(
            "iroha_torii::routing::SubscriptionPlanCreateDto",
        );
        identity::<SubscriptionPlanCreateResponse>(
            "iroha_torii::routing::SubscriptionPlanCreateResponseDto",
        );
        identity::<SubscriptionPlanListParams>("iroha_torii::routing::SubscriptionPlanListParams");
        identity::<SubscriptionPlanListItem>("iroha_torii::routing::SubscriptionPlanListItem");
        identity::<SubscriptionPlanListResponse>(
            "iroha_torii::routing::SubscriptionPlanListResponseDto",
        );
        identity::<SubscriptionCreateRequest>("iroha_torii::routing::SubscriptionCreateDto");
        identity::<SubscriptionCreateResponse>(
            "iroha_torii::routing::SubscriptionCreateResponseDto",
        );
        identity::<SubscriptionListParams>("iroha_torii::routing::SubscriptionListParams");
        identity::<SubscriptionListItem>("iroha_torii::routing::SubscriptionListItem");
        identity::<SubscriptionListResponse>("iroha_torii::routing::SubscriptionListResponseDto");
        identity::<SubscriptionGetResponse>("iroha_torii::routing::SubscriptionGetResponseDto");
        identity::<SubscriptionActionRequest>("iroha_torii::routing::SubscriptionActionDto");
        identity::<SubscriptionCancelMode>("iroha_torii::routing::SubscriptionCancelMode");
        identity::<SubscriptionUsageRequest>("iroha_torii::routing::SubscriptionUsageRequestDto");
        identity::<SubscriptionActionDraftDetails>(
            "iroha_torii::routing::SubscriptionActionDraftDetailsDto",
        );
        identity::<SubscriptionActionResponse>(
            "iroha_torii::routing::SubscriptionActionResponseDto",
        );
        identity::<SubscriptionUsageResponse>("iroha_torii::routing::SubscriptionUsageResponseDto");
    }

    #[test]
    fn cancellation_has_one_exact_json_and_binary_shape() {
        for (mode, text) in [
            (SubscriptionCancelMode::Immediate, "immediate"),
            (SubscriptionCancelMode::PeriodEnd, "period_end"),
        ] {
            let expected = format!("{{\"mode\":\"{text}\",\"value\":null}}");
            assert_eq!(norito::json::to_json(&mode).unwrap(), expected);
            assert_eq!(
                norito::json::from_json::<SubscriptionCancelMode>(&expected).unwrap(),
                mode
            );
            let bytes = norito::to_bytes(&mode).unwrap();
            assert_eq!(
                norito::decode_from_bytes::<SubscriptionCancelMode>(&bytes).unwrap(),
                mode
            );
        }
        for invalid in [
            "\"immediate\"",
            "{\"mode\":\"period-end\"}",
            "{\"mode\":\"immediate\",\"unknown\":1}",
        ] {
            assert!(norito::json::from_json::<SubscriptionCancelMode>(invalid).is_err());
        }
    }

    #[test]
    fn list_defaults_and_count_mode_roundtrip_without_codec_changes() {
        let params: SubscriptionListParams = norito::json::from_json("{}").unwrap();
        assert_eq!(params.offset, 0);
        assert!(params.count_mode.is_none());
        let params = SubscriptionListParams {
            provider: Some("merchant@paynet".to_owned()),
            count_mode: Some("exact".to_owned()),
            limit: Some(10),
            offset: 3,
            ..params
        };
        let bytes = norito::to_bytes(&params).unwrap();
        let decoded: SubscriptionListParams = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(
            norito::json::to_json(&decoded).unwrap(),
            norito::json::to_json(&params).unwrap()
        );
    }
}
