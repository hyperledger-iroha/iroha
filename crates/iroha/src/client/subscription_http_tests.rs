// Subscription app-API HTTP client contract tests.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use http::StatusCode;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::gen_account_in;
use norito::json::{JsonSerialize, Value as JsonValue};

use super::evidence_http_tests::{
    SnapshotStore, base_url, client_with_base_url, json_response, with_mock_http,
};
use super::{SubscriptionActionRequest, SubscriptionCreateRequest, SubscriptionPlanCreateRequest};
use super::{
    SubscriptionActionResponse, SubscriptionCreateResponse, SubscriptionGetResponse,
    SubscriptionListParams, SubscriptionListResponse, SubscriptionPlanListParams,
    SubscriptionPlanListResponse, SubscriptionUsageRequest, SubscriptionUsageResponse,
};
use crate::{
    data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        nft::NftId,
        subscription::{
            SubscriptionBilling, SubscriptionCadence, SubscriptionFixedPeriodCadence,
            SubscriptionFixedPricing, SubscriptionPlan, SubscriptionState, SubscriptionStatus,
        },
        trigger::TriggerId,
    },
    http::{Method as HttpMethod, Response as HttpResponse},
    http_default::RequestSnapshot,
    subscriptions::{
        SubscriptionActionDraftDetails, SubscriptionInstructionDraft, SubscriptionListItem,
        SubscriptionPlanCreateResponse, SubscriptionPlanListItem,
    },
};

fn encode_json<T: JsonSerialize>(value: &T) -> String {
    norito::json::to_json(value).expect("encode json")
}

fn match_body<T: JsonSerialize>(snapshot: &RequestSnapshot, expected: &T) {
    let body: JsonValue = norito::json::from_slice(&snapshot.body).expect("decode request body");
    let expected = norito::json::to_value(expected).expect("encode expected body");
    assert_eq!(
        body, expected,
        "unexpected request body for {}",
        snapshot.url
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn subscription_endpoints_build_requests() {
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let client = client_with_base_url(base_url());
    let provider = client.account.clone();
    let subscriber = client.account.clone();
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
    let subscription_id: NftId = "sub-1$subscriptions.universal".parse().unwrap();
    let billing_trigger_id: TriggerId = "sub-1-bill".parse().unwrap();
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let unit_key: Name = "compute_ms".parse().unwrap();
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                period_ms: 1_000,
            }),
            bill_for: crate::data_model::subscription::SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms: 100,
            max_failures: 3,
            grace_ms: 500,
        },
        pricing: crate::data_model::subscription::SubscriptionPricing::Fixed(
            SubscriptionFixedPricing {
                amount: Quantity::from(5_u32),
                asset_definition: charge_asset_id.clone(),
            },
        ),
    };

    let subscription_state = SubscriptionState {
        plan_id: plan_id.clone(),
        provider: provider.clone(),
        subscriber: subscriber.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: 0,
        current_period_end_ms: 1,
        next_charge_ms: 1,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 0,
        usage_accumulated: BTreeMap::new(),
        billing_trigger_id: billing_trigger_id.clone(),
    };

    let plan_request = SubscriptionPlanCreateRequest {
        authority: provider.clone(),
        plan_id: plan_id.clone(),
        plan: plan.clone(),
    };
    let plan_list_params = SubscriptionPlanListParams {
        provider: Some(provider.to_string()),
        limit: Some(10),
        offset: 5,
        count_mode: Some("exact".to_string()),
    };
    let subscription_request = SubscriptionCreateRequest {
        authority: subscriber.clone(),
        subscription_id: subscription_id.clone(),
        plan_id: plan_id.clone(),
        billing_trigger_id: Some(billing_trigger_id.clone()),
        usage_trigger_id: None,
        first_charge_ms: Some(42),
        grant_usage_to_provider: Some(true),
    };
    let subscription_list_params = SubscriptionListParams {
        owned_by: Some(subscriber.to_string()),
        provider: Some(provider.to_string()),
        status: Some("active".to_string()),
        limit: Some(25),
        offset: 2,
        count_mode: Some("exact".to_string()),
    };
    let action_request = SubscriptionActionRequest {
        authority: subscriber.clone(),
        charge_at_ms: Some(170),
        cancel_mode: None,
    };
    let usage_request = SubscriptionUsageRequest {
        authority: subscriber.clone(),
        unit_key: unit_key.clone(),
        delta: Quantity::from(3_u32),
        usage_trigger_id: None,
    };

    let plan_create_response = SubscriptionPlanCreateResponse {
        submitted: false,
        plan_id: plan_id.clone(),
        transaction_payload_b64: "AA==".to_string(),
        signing_message_b64: "AA==".to_string(),
    };
    let plan_list_response = SubscriptionPlanListResponse {
        items: vec![SubscriptionPlanListItem {
            plan_id: plan_id.clone(),
            plan: plan.clone(),
        }],
        total: Some(1),
        has_more: false,
        count_mode: "exact".to_string(),
    };
    let subscription_create_response = SubscriptionCreateResponse {
        version: 1,
        authority: subscriber.clone(),
        action: "create".to_string(),
        subscription_id: subscription_id.clone(),
        plan_id: plan_id.clone(),
        billing_trigger_id: billing_trigger_id.clone(),
        usage_trigger_id: None,
        first_charge_ms: 42,
        provider_usage_grant_included: true,
        resulting_subscription: subscription_state.clone(),
        tx_instructions: vec![SubscriptionInstructionDraft {
            wire_id: "register_nft".to_string(),
            payload_hex: "00".to_string(),
        }],
    };
    let subscription_list_response = SubscriptionListResponse {
        items: vec![SubscriptionListItem {
            subscription_id: subscription_id.clone(),
            subscription: subscription_state.clone(),
            invoice: None,
            plan: Some(plan.clone()),
        }],
        total: Some(1),
        has_more: false,
        count_mode: "exact".to_string(),
    };
    let subscription_get_response = SubscriptionGetResponse {
        subscription_id: subscription_id.clone(),
        subscription: subscription_state.clone(),
        invoice: None,
        plan: Some(plan),
    };
    let action_response = SubscriptionActionResponse {
        version: 1,
        authority: subscriber.clone(),
        action: "pause".to_string(),
        subscription_id: subscription_id.clone(),
        details: SubscriptionActionDraftDetails {
            billing_trigger_id,
            billing_trigger_operation: "unregister".to_string(),
            effective_charge_ms: None,
            cancel_mode: None,
            resulting_subscription: subscription_state,
        },
        tx_instructions: vec![SubscriptionInstructionDraft {
            wire_id: "set_key_value".to_string(),
            payload_hex: "00".to_string(),
        }],
    };
    let usage_response = SubscriptionUsageResponse {
        submitted: false,
        subscription_id: subscription_id.clone(),
        transaction_payload_b64: "AA==".to_string(),
        signing_message_b64: "AA==".to_string(),
    };

    let responder = {
        let store = Arc::clone(&store);
        let plan_create_json = encode_json(&plan_create_response);
        let plan_list_json = encode_json(&plan_list_response);
        let subscription_create_json = encode_json(&subscription_create_response);
        let subscription_list_json = encode_json(&subscription_list_response);
        let subscription_get_json = encode_json(&subscription_get_response);
        let action_json = encode_json(&action_response);
        let usage_json = encode_json(&usage_response);
        move |snapshot: RequestSnapshot| {
            store
                .lock()
                .expect("lock snapshot store")
                .push(snapshot.clone());
            let path = snapshot.url.path();
            let response = match (snapshot.method.clone(), path) {
                (HttpMethod::POST, "/v1/subscriptions/plans") => {
                    json_response(StatusCode::OK, &plan_create_json)
                }
                (HttpMethod::GET, "/v1/subscriptions/plans") => {
                    json_response(StatusCode::OK, &plan_list_json)
                }
                (HttpMethod::POST, "/v1/subscriptions") => {
                    json_response(StatusCode::OK, &subscription_create_json)
                }
                (HttpMethod::GET, "/v1/subscriptions") => {
                    json_response(StatusCode::OK, &subscription_list_json)
                }
                (HttpMethod::GET, "/v1/subscriptions/sub-1$subscriptions.universal") => {
                    json_response(StatusCode::OK, &subscription_get_json)
                }
                (
                    HttpMethod::POST,
                    "/v1/subscriptions/sub-1$subscriptions.universal/pause"
                    | "/v1/subscriptions/sub-1$subscriptions.universal/resume"
                    | "/v1/subscriptions/sub-1$subscriptions.universal/cancel"
                    | "/v1/subscriptions/sub-1$subscriptions.universal/charge-now",
                ) => json_response(StatusCode::OK, &action_json),
                (HttpMethod::POST, "/v1/subscriptions/sub-1$subscriptions.universal/usage") => {
                    json_response(StatusCode::OK, &usage_json)
                }
                _ => HttpResponse::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .expect("response build"),
            };
            Ok(response)
        }
    };

    with_mock_http(responder, || {
        client
            .create_subscription_plan(&plan_request)
            .expect("create subscription plan");
        client
            .list_subscription_plans(&plan_list_params)
            .expect("list subscription plans");
        client
            .create_subscription(&subscription_request)
            .expect("create subscription");
        client
            .list_subscriptions(&subscription_list_params)
            .expect("list subscriptions");
        client
            .get_subscription(&subscription_id)
            .expect("get subscription");
        client
            .pause_subscription(&subscription_id, &action_request)
            .expect("pause subscription");
        client
            .resume_subscription(&subscription_id, &action_request)
            .expect("resume subscription");
        client
            .cancel_subscription(&subscription_id, &action_request)
            .expect("cancel subscription");
        client
            .charge_subscription_now(&subscription_id, &action_request)
            .expect("charge subscription now");
        client
            .record_subscription_usage(&subscription_id, &usage_request)
            .expect("record usage");
    });

    let snapshots = store.lock().expect("lock snapshots").clone();
    assert_eq!(snapshots.len(), 10);
    let signed = snapshots
        .iter()
        .filter(|snapshot| snapshot.method == HttpMethod::POST);
    for snapshot in signed {
        super::tests::assert_canonical_account_signed_json_request(&client, snapshot);
    }
    for snapshot in &snapshots {
        match (snapshot.method.clone(), snapshot.url.path()) {
            (HttpMethod::POST, "/v1/subscriptions/plans") => {
                match_body(snapshot, &plan_request);
            }
            (HttpMethod::GET, "/v1/subscriptions/plans") => {
                let params: Vec<(String, String)> = snapshot
                    .url
                    .query_pairs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                assert_eq!(
                    params,
                    vec![
                        ("provider".to_string(), provider.to_string()),
                        ("limit".to_string(), "10".to_string()),
                        ("offset".to_string(), "5".to_string()),
                    ]
                );
            }
            (HttpMethod::POST, "/v1/subscriptions") => {
                match_body(snapshot, &subscription_request);
            }
            (HttpMethod::GET, "/v1/subscriptions") => {
                let params: Vec<(String, String)> = snapshot
                    .url
                    .query_pairs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                assert_eq!(
                    params,
                    vec![
                        ("owned_by".to_string(), subscriber.to_string()),
                        ("provider".to_string(), provider.to_string()),
                        ("status".to_string(), "active".to_string()),
                        ("limit".to_string(), "25".to_string()),
                        ("offset".to_string(), "2".to_string()),
                    ]
                );
            }
            (HttpMethod::GET, "/v1/subscriptions/sub-1$subscriptions.universal") => {
                assert!(snapshot.body.is_empty());
            }
            (
                HttpMethod::POST,
                "/v1/subscriptions/sub-1$subscriptions.universal/pause"
                | "/v1/subscriptions/sub-1$subscriptions.universal/resume"
                | "/v1/subscriptions/sub-1$subscriptions.universal/cancel"
                | "/v1/subscriptions/sub-1$subscriptions.universal/charge-now",
            ) => {
                match_body(snapshot, &action_request);
            }
            (HttpMethod::POST, "/v1/subscriptions/sub-1$subscriptions.universal/usage") => {
                match_body(snapshot, &usage_request);
            }
            _ => {}
        }
    }
}

#[test]
fn subscription_authority_mismatch_fails_before_transport() {
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let client = client_with_base_url(base_url());
    let (other, _) = gen_account_in("other");
    let request = SubscriptionUsageRequest {
        authority: other,
        unit_key: "compute_ms".parse().expect("unit key"),
        delta: Quantity::from(1_u32),
        usage_trigger_id: None,
    };
    let subscription_id: NftId = "sub-1$subscriptions.universal"
        .parse()
        .expect("subscription id");
    let captured = Arc::clone(&store);
    let responder = move |snapshot: RequestSnapshot| {
        captured.lock().expect("snapshot lock").push(snapshot);
        Ok(HttpResponse::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new())
            .expect("response"))
    };

    let error = with_mock_http(responder, || {
        client
            .record_subscription_usage(&subscription_id, &request)
            .expect_err("a mismatched request authority must fail")
    });
    assert!(error.to_string().contains("authenticated client account"));
    assert!(store.lock().expect("snapshot lock").is_empty());
}
