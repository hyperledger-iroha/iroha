#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]
//! Integration tests exercising the SNS registrar API surface.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    metadata::Metadata,
    peer::PeerId,
    sns::{
        DOMAIN_NAME_SUFFIX_ID, FreezeNameRequestV1, GovernanceHookV1, NameControllerV1,
        NameFrozenStateV1, NameRecordV1, NameSelectorV1, NameStatus, PaymentProofV1,
        RegisterNameRequestV1, RenewNameRequestV1, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
};
use iroha_primitives::json::Json;
use iroha_torii::{Torii, test_utils};
use norito::{codec::Encode as _, json::JsonSerialize};
use tokio::sync::broadcast;
use tower::util::ServiceExt as _;

#[path = "fixtures.rs"]
mod torii_fixtures;

#[cfg(feature = "telemetry")]
type TestMetrics = Arc<iroha_telemetry::metrics::Metrics>;
#[cfg(not(feature = "telemetry"))]
type TestMetrics = ();

struct SeededDomainRecord {
    literal: String,
    status: NameStatus,
}

fn test_router() -> Router {
    #[cfg(feature = "telemetry")]
    let metrics = torii_fixtures::shared_metrics();
    #[cfg(not(feature = "telemetry"))]
    let metrics = ();

    test_router_with_metrics_and_domain_records(metrics, Vec::new())
}

fn test_router_with_domain_records(records: Vec<SeededDomainRecord>) -> Router {
    #[cfg(feature = "telemetry")]
    let metrics = torii_fixtures::shared_metrics();
    #[cfg(not(feature = "telemetry"))]
    let metrics = ();

    test_router_with_metrics_and_domain_records(metrics, records)
}

#[cfg(feature = "telemetry")]
fn test_router_with_metrics(metrics: TestMetrics) -> Router {
    test_router_with_metrics_and_domain_records(metrics, Vec::new())
}

fn test_router_with_metrics_and_domain_records(
    metrics: TestMetrics,
    records: Vec<SeededDomainRecord>,
) -> Router {
    #[cfg(not(feature = "telemetry"))]
    let _ = metrics;

    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    torii_fixtures::seed_peer(&mut world, local_peer_id.clone());
    for record in records {
        seed_domain_name_record(&mut world, record);
    }
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let (_mh, ts) =
            iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    torii.api_router_for_tests()
}

fn sample_owner() -> AccountId {
    let public_key: PublicKey =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
            .parse()
            .expect("key parses");
    AccountId::new(public_key)
}

fn secondary_owner() -> AccountId {
    let public_key: PublicKey =
        "ed0120C70416DC2D60D9AB2F0C6CED829837F1006DDED2DE794E9D5091A60663FA8C11"
            .parse()
            .expect("key parses");
    AccountId::new(public_key)
}

fn controller_for(owner: &AccountId) -> NameControllerV1 {
    let address = AccountAddress::from_account_id(owner).expect("address encode");
    NameControllerV1::account(&address)
}

fn seeded_domain_record(label: &str, status: NameStatus) -> SeededDomainRecord {
    SeededDomainRecord {
        literal: domain_literal(label),
        status,
    }
}

fn seed_domain_name_record(world: &mut World, seeded: SeededDomainRecord) {
    let owner = sample_owner();
    let selector =
        NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, seeded.literal).expect("domain selector");
    let mut record = NameRecordV1::new(
        selector.clone(),
        owner.clone(),
        vec![controller_for(&owner)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        Metadata::default(),
    );
    record.status = seeded.status;
    world.smart_contract_state_mut_for_testing().insert(
        iroha_core::sns::record_storage_key(&selector),
        record.encode(),
    );
}

const DEFAULT_SNS_LEASE_PAYMENT_AMOUNT: u64 = 500_000_000;

fn build_payment_with_amount(payer: AccountId, amount: u64) -> PaymentProofV1 {
    PaymentProofV1 {
        asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
        gross_amount: amount,
        net_amount: amount,
        settlement_tx: Json::from("dummy-tx"),
        payer,
        signature: Json::from("dummy-signature"),
    }
}

fn build_payment_with(payer: AccountId) -> PaymentProofV1 {
    build_payment_with_amount(payer, DEFAULT_SNS_LEASE_PAYMENT_AMOUNT)
}

fn build_register_request_with(label: &str, owner: &AccountId) -> RegisterNameRequestV1 {
    let selector =
        NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain_literal(label)).expect("selector");
    RegisterNameRequestV1 {
        selector,
        owner: owner.clone(),
        controllers: vec![controller_for(owner)],
        term_years: 1,
        pricing_class_hint: Some(0),
        payment: build_payment_with(owner.clone()),
        governance: None,
        metadata: Metadata::default(),
    }
}

fn build_register_request() -> RegisterNameRequestV1 {
    let owner = sample_owner();
    build_register_request_with("makoto", &owner)
}

async fn request_json<T: JsonSerialize + ?Sized>(
    app: &Router,
    method: &str,
    uri: impl AsRef<str>,
    payload: &T,
) -> axum::response::Response {
    let body = norito::json::to_vec(payload).expect("serialize json payload");
    app.clone()
        .oneshot(
            Request::builder()
                .uri(uri.as_ref())
                .method(method)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .expect("json response")
}

async fn get(app: &Router, uri: impl AsRef<str>) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(uri.as_ref())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("get response")
}

async fn post_register_name(
    app: &Router,
    payload: &RegisterNameRequestV1,
) -> axum::response::Response {
    request_json(app, "POST", "/v1/sns/names", payload).await
}

async fn response_text(resp: axum::response::Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).expect("response body should be utf-8")
}

async fn assert_direct_mutation_disabled(resp: axum::response::Response, op: &str) {
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_text(resp).await;
    assert!(
        body.contains(&format!(
            "SNS {op} must be submitted as a consensus transaction"
        )),
        "unexpected SNS mutation rejection body: {body}"
    );
    assert!(
        body.contains("direct Torii WSV mutation is disabled"),
        "SNS mutation rejection should explain why direct mutation is unavailable: {body}"
    );
}

fn domain_name_path(label: &str) -> String {
    format!("/v1/sns/names/domain/{}", domain_literal(label))
}

fn domain_literal(label: &str) -> String {
    format!("{label}.universal")
}

#[tokio::test]
async fn sns_register_route_rejects_direct_wsv_mutation() {
    let app = test_router();
    let payload = build_register_request();
    let resp = post_register_name(&app, &payload).await;
    assert_direct_mutation_disabled(resp, "register_name").await;
}

#[tokio::test]
async fn sns_fetch_seeded_record_and_policy_round_trip() {
    let app =
        test_router_with_domain_records(vec![seeded_domain_record("makoto", NameStatus::Active)]);

    let record_resp = get(&app, domain_name_path("makoto")).await;
    assert_eq!(record_resp.status(), StatusCode::OK);
    let record: NameRecordV1 =
        norito::json::from_slice(&record_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode record");
    assert_eq!(record.selector.normalized_label(), domain_literal("makoto"));
    assert!(matches!(record.status, NameStatus::Active));

    let policy_resp = get(&app, format!("/v1/sns/policies/{DOMAIN_NAME_SUFFIX_ID}")).await;
    assert_eq!(policy_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn sns_fetch_accepts_noncanonical_domain_path_literal() {
    let app =
        test_router_with_domain_records(vec![seeded_domain_record("casepath", NameStatus::Active)]);

    let record_resp = get(&app, "/v1/sns/names/domain/CASEPATH.UNIVERSAL").await;
    assert_eq!(record_resp.status(), StatusCode::OK);
    let record: NameRecordV1 =
        norito::json::from_slice(&record_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode record");
    assert_eq!(record.selector.normalized_label(), "casepath.universal");
}

#[tokio::test]
async fn sns_fetch_returns_seeded_frozen_status() {
    let app = test_router_with_domain_records(vec![seeded_domain_record(
        "frozen",
        NameStatus::Frozen(NameFrozenStateV1 {
            reason: "audit".into(),
            until_ms: u64::MAX,
        }),
    )]);

    let record_resp = get(&app, domain_name_path("frozen")).await;
    assert_eq!(record_resp.status(), StatusCode::OK);
    let record: NameRecordV1 =
        norito::json::from_slice(&record_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode record");
    assert!(matches!(record.status, NameStatus::Frozen(_)));
}

#[tokio::test]
async fn sns_fetch_rejects_bare_domain_literal() {
    let app = test_router();
    let record_resp = get(&app, "/v1/sns/names/domain/lookupcanon").await;
    assert_eq!(record_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sns_fetch_missing_name_returns_not_found() {
    let app = test_router();
    let record_resp = get(&app, "/v1/sns/names/domain/missing.universal").await;
    assert_eq!(record_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sns_fetch_rejects_unknown_namespace() {
    let app = test_router();
    let record_resp = get(&app, "/v1/sns/names/not-a-namespace/casepath.universal").await;
    assert_eq!(record_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sns_missing_policy_returns_not_found() {
    let app = test_router();
    let policy_resp = get(&app, "/v1/sns/policies/65535").await;
    assert_eq!(policy_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sns_mutation_routes_reject_direct_wsv_mutation() {
    let app = test_router();
    let owner = sample_owner();

    let renew = RenewNameRequestV1 {
        term_years: 1,
        payment: build_payment_with(owner.clone()),
    };
    let renew_resp = request_json(
        &app,
        "POST",
        "/v1/sns/names/domain/missing.universal/renew",
        &renew,
    )
    .await;
    assert_direct_mutation_disabled(renew_resp, "renew_name").await;

    let transfer = TransferNameRequestV1 {
        new_owner: secondary_owner(),
        governance: GovernanceHookV1 {
            proposal_id: "proposal-transfer".into(),
            council_vote_hash: Json::from("council-hash"),
            dao_vote_hash: Json::from("dao-hash"),
            steward_ack: Json::from("steward-ack"),
            guardian_clearance: None,
        },
    };
    let transfer_resp = request_json(
        &app,
        "POST",
        "/v1/sns/names/domain/missing.universal/transfer",
        &transfer,
    )
    .await;
    assert_direct_mutation_disabled(transfer_resp, "transfer_name").await;

    let controllers = UpdateControllersRequestV1 {
        controllers: vec![controller_for(&owner)],
    };
    let controllers_resp = request_json(
        &app,
        "POST",
        "/v1/sns/names/domain/missing.universal/controllers",
        &controllers,
    )
    .await;
    assert_direct_mutation_disabled(controllers_resp, "update_name_controllers").await;

    let freeze = FreezeNameRequestV1 {
        reason: "missing-name".into(),
        until_ms: u64::MAX,
        guardian_ticket: Json::from("ticket"),
    };
    let freeze_resp = request_json(
        &app,
        "POST",
        "/v1/sns/names/domain/missing.universal/freeze",
        &freeze,
    )
    .await;
    assert_direct_mutation_disabled(freeze_resp, "freeze_name").await;

    let unfreeze = GovernanceHookV1 {
        proposal_id: "proposal-unfreeze".into(),
        council_vote_hash: Json::from("council-hash"),
        dao_vote_hash: Json::from("dao-hash"),
        steward_ack: Json::from("steward-ack"),
        guardian_clearance: Some(Json::from("guardian")),
    };
    let unfreeze_resp = request_json(
        &app,
        "DELETE",
        "/v1/sns/names/domain/missing.universal/freeze",
        &unfreeze,
    )
    .await;
    assert_direct_mutation_disabled(unfreeze_resp, "unfreeze_name").await;
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn sns_register_rejection_emits_error_status_metric() {
    let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
    let app = test_router_with_metrics(Arc::clone(&metrics));
    let counter = metrics
        .sns_registrar_status_total
        .with_label_values(&["error", "domain"]);
    let before = counter.get();

    let resp = post_register_name(&app, &build_register_request()).await;
    assert_direct_mutation_disabled(resp, "register_name").await;

    let after = counter.get();
    assert_eq!(
        after,
        before + 1,
        "rejected registration should increment sns_registrar_status_total{{result=\"error\",suffix=\"domain\"}}"
    );
}
