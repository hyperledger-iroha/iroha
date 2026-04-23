#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for Torii subscription endpoints.
#![cfg(feature = "app_api")]

use std::{net::SocketAddr, num::NonZeroU64, str::FromStr as _, sync::Arc};

use axum::{
    body::Body,
    extract::connect_info::ConnectInfo,
    http::{Request, header::CONTENT_TYPE},
    response::Response,
};
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::{Execute, triggers::set::SetReadOnly},
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    ChainId, Registrable,
    account::Account,
    asset::{AssetDefinition, AssetDefinitionId},
    block::BlockHeader,
    domain::{Domain, DomainId},
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    isi::{InstructionBox, Register},
    metadata::Metadata,
    name::Name,
    nft::{Nft, NftId},
    peer::PeerId,
    prelude::{ExposedPrivateKey, Repeats},
    subscription::{
        ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY, AccountAliasAutoRenewMetadata,
        SUBSCRIPTION_INVOICE_METADATA_KEY, SUBSCRIPTION_METADATA_KEY, SubscriptionInvoice,
        SubscriptionInvoiceStatus, SubscriptionState, SubscriptionStatus,
    },
    trigger::{Trigger, TriggerId, action::Action},
};
use iroha_primitives::{
    json::Json as IrohaJson,
    numeric::{Numeric, NumericSpec},
};
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, json_entry, json_object};
use mv::storage::StorageReadOnly;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

struct SubscriptionHarness {
    app: axum::Router,
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: ChainId,
    charge_asset_id: AssetDefinitionId,
    subscription_id: NftId,
    billing_trigger_id: TriggerId,
}

fn build_alias_auto_renew_settings(alias: &str) -> AccountAliasAutoRenewMetadata {
    AccountAliasAutoRenewMetadata {
        alias: alias.to_owned(),
        term_years: 1,
        max_charge_amount: Numeric::new(200_u32, 0),
        retry_backoff_ms: 500,
        max_failures: 3,
    }
}

fn build_alias_subscription_state(
    charge_asset_id: AssetDefinitionId,
    billing_trigger_id: TriggerId,
    status: SubscriptionStatus,
) -> SubscriptionState {
    SubscriptionState {
        plan_id: charge_asset_id,
        provider: ALICE_ID.clone(),
        subscriber: BOB_ID.clone(),
        status,
        current_period_start_ms: 1_000,
        current_period_end_ms: 2_000,
        next_charge_ms: 3_000,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 2,
        usage_accumulated: std::collections::BTreeMap::new(),
        billing_trigger_id,
    }
}

fn build_alias_subscription_invoice(
    subscription_id: NftId,
    charge_asset_id: AssetDefinitionId,
) -> SubscriptionInvoice {
    SubscriptionInvoice {
        subscription_nft_id: subscription_id,
        period_start_ms: 1_000,
        period_end_ms: 2_000,
        attempted_at_ms: 2_500,
        amount: Numeric::new(25_u32, 0),
        asset_definition: charge_asset_id,
        status: SubscriptionInvoiceStatus::Failed,
        tx_hash: None,
    }
}

fn build_existing_billing_trigger(
    trigger_id: TriggerId,
    authority: iroha_data_model::account::AccountId,
) -> Trigger {
    Trigger::new(
        trigger_id,
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            authority,
            TimeEventFilter(ExecutionTime::Schedule(Schedule {
                start_ms: 3_000,
                period_ms: None,
            })),
        ),
    )
}

fn build_subscription_harness(status: SubscriptionStatus) -> SubscriptionHarness {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let charge_asset_id = AssetDefinitionId::new(
        domain_id.clone(),
        Name::from_str("fee").expect("asset name"),
    );
    let subscription_id = NftId::of(
        domain_id.clone(),
        Name::from_str("alias_auto_renew").expect("subscription name"),
    );
    let billing_trigger_id: TriggerId = "bill_alias_router".parse().expect("trigger id");

    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str(SUBSCRIPTION_METADATA_KEY).expect("subscription metadata key"),
        IrohaJson::new(build_alias_subscription_state(
            charge_asset_id.clone(),
            billing_trigger_id.clone(),
            status,
        )),
    );
    metadata.insert(
        Name::from_str(SUBSCRIPTION_INVOICE_METADATA_KEY).expect("invoice metadata key"),
        IrohaJson::new(build_alias_subscription_invoice(
            subscription_id.clone(),
            charge_asset_id.clone(),
        )),
    );
    metadata.insert(
        Name::from_str(ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY).expect("alias metadata key"),
        IrohaJson::new(build_alias_auto_renew_settings("member@universal")),
    );

    let domain = Domain::new(domain_id).build(&ALICE_ID);
    let provider_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let subscriber_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
    let asset_definition =
        AssetDefinition::new(charge_asset_id.clone(), NumericSpec::integer()).build(&ALICE_ID);
    let nft = Nft::new(subscription_id.clone(), metadata).build(&BOB_ID);
    let mut world = World::with_assets(
        [domain],
        [provider_account, subscriber_account],
        [asset_definition],
        [],
        [nft],
    );
    fixtures::seed_peer(&mut world, local_peer_id.clone());

    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    {
        let expected_height = u64::try_from(state.view().height())
            .unwrap_or(0)
            .saturating_add(1);
        let header = BlockHeader::new(
            NonZeroU64::new(expected_height).expect("height > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();
        Register::trigger(build_existing_billing_trigger(
            billing_trigger_id.clone(),
            BOB_ID.clone(),
        ))
        .execute(&BOB_ID, &mut stx)
        .expect("register billing trigger");
        stx.apply();
        block.commit().expect("commit trigger registration");
    }

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let chain_id = ChainId::from("test-chain");
    let torii = Torii::new_with_handle(
        chain_id.clone(),
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state.clone(),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    SubscriptionHarness {
        app: torii.api_router_for_tests(),
        state,
        queue,
        chain_id,
        charge_asset_id,
        subscription_id,
        billing_trigger_id,
    }
}

async fn call_app(app: &axum::Router, mut request: Request<Body>) -> Response {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));
    app.clone().oneshot(request).await.expect("router responds")
}

async fn response_json(resp: Response) -> norito::json::Value {
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    norito::json::from_slice(&body).expect("valid json response")
}

#[tokio::test]
async fn subscription_mutation_routes_are_registered() {
    let harness = build_subscription_harness(SubscriptionStatus::Paused);
    let subscription_id = harness.subscription_id.to_string();
    for uri in [
        "/v1/subscriptions/plans".to_owned(),
        "/v1/subscriptions".to_owned(),
        format!("/v1/subscriptions/{subscription_id}/pause"),
        format!("/v1/subscriptions/{subscription_id}/resume"),
        format!("/v1/subscriptions/{subscription_id}/cancel"),
        format!("/v1/subscriptions/{subscription_id}/keep"),
        format!("/v1/subscriptions/{subscription_id}/usage"),
        format!("/v1/subscriptions/{subscription_id}/charge-now"),
    ] {
        let resp = call_app(
            &harness.app,
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await;
        assert!(
            !matches!(
                resp.status(),
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ),
            "subscription route `{uri}` should be registered"
        );
    }
}

#[tokio::test]
async fn subscription_list_and_get_return_alias_auto_renew_without_plan_metadata() {
    let harness = build_subscription_harness(SubscriptionStatus::Paused);
    let subscription_id = harness.subscription_id.to_string();

    let list_resp = call_app(
        &harness.app,
        Request::builder()
            .uri("/v1/subscriptions?offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_json = response_json(list_resp).await;
    let items = list_json["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["subscription_id"].as_str(),
        Some(subscription_id.as_str())
    );
    assert!(
        items[0]["plan"].is_null(),
        "alias subscriptions should not require a plan payload"
    );
    let list_state: SubscriptionState =
        norito::json::from_value(items[0]["subscription"].clone()).expect("subscription state");
    assert_eq!(list_state.status, SubscriptionStatus::Paused);
    let list_invoice: SubscriptionInvoice =
        norito::json::from_value(items[0]["invoice"].clone()).expect("subscription invoice");
    assert_eq!(list_invoice.asset_definition, harness.charge_asset_id);

    let get_resp = call_app(
        &harness.app,
        Request::builder()
            .uri(format!("/v1/subscriptions/{subscription_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_json = response_json(get_resp).await;
    assert!(
        get_json["plan"].is_null(),
        "alias subscriptions should serialize without a plan payload"
    );
    let get_state: SubscriptionState =
        norito::json::from_value(get_json["subscription"].clone()).expect("subscription state");
    assert_eq!(get_state.status, SubscriptionStatus::Paused);
    let get_invoice: SubscriptionInvoice =
        norito::json::from_value(get_json["invoice"].clone()).expect("subscription invoice");
    assert_eq!(get_invoice.asset_definition, harness.charge_asset_id);
}

#[tokio::test]
async fn subscription_resume_route_supports_alias_auto_renew_nfts() {
    let harness = build_subscription_harness(SubscriptionStatus::Paused);
    let subscription_id = harness.subscription_id.to_string();
    let body = json_object(vec![
        json_entry("authority", BOB_ID.to_string()),
        json_entry(
            "private_key",
            ExposedPrivateKey(BOB_KEYPAIR.private_key().clone()),
        ),
        json_entry("charge_at_ms", 5_000_u64),
    ]);
    let body = norito::json::to_json(&body).expect("serialize resume request");

    let resume_resp = call_app(
        &harness.app,
        Request::builder()
            .method("POST")
            .uri(format!("/v1/subscriptions/{subscription_id}/resume"))
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("request"),
    )
    .await;
    assert_eq!(resume_resp.status(), StatusCode::OK);
    assert_eq!(harness.queue.queued_len(), 1);
    let resume_json = response_json(resume_resp).await;
    assert_eq!(resume_json["ok"].as_bool(), Some(true));
    assert_eq!(
        resume_json["subscription_id"].as_str(),
        Some(subscription_id.as_str())
    );

    let expected_height = u64::try_from(harness.state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    let applied = iroha_torii::test_utils::apply_queued_in_one_block(
        &harness.state,
        &harness.queue,
        &harness.chain_id,
        expected_height,
    );
    assert_eq!(applied, 1, "resume transaction should apply");

    let view = harness.state.view();
    let nft = view
        .world()
        .nft(&harness.subscription_id)
        .expect("subscription nft should exist");
    let resumed_state: SubscriptionState = nft
        .content
        .get(&Name::from_str(SUBSCRIPTION_METADATA_KEY).expect("subscription metadata key"))
        .expect("subscription metadata present")
        .try_into_any_norito()
        .expect("subscription metadata decodes");
    assert_eq!(resumed_state.status, SubscriptionStatus::Active);
    assert_eq!(resumed_state.failure_count, 0);
    assert_eq!(resumed_state.next_charge_ms, 5_000);
    assert_eq!(resumed_state.current_period_start_ms, 1_000);
    assert_eq!(resumed_state.current_period_end_ms, 2_000);
    assert!(
        view.world()
            .triggers()
            .time_triggers()
            .get(&harness.billing_trigger_id)
            .is_some(),
        "resume should re-register the billing trigger"
    );
}
