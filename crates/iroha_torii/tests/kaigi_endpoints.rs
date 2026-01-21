//! Telemetry integration tests covering Kaigi relay endpoints.
#![cfg(feature = "telemetry")]

use std::{str::FromStr, sync::Arc};

use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use iroha_core::{kiso::KisoHandle, kura::Kura, query::store::LiveQueryStore, state::State};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId, Registrable,
    account::Account,
    domain::Domain,
    kaigi::{
        KaigiId, KaigiRelayFeedback, KaigiRelayHealthStatus, KaigiRelayRegistration,
        kaigi_relay_feedback_key, kaigi_relay_metadata_key,
    },
    metadata::Metadata,
    prelude::{AccountId, DomainId, Name},
};
use iroha_primitives::{json::Json, time::TimeSource};
use tower::ServiceExt;

#[path = "fixtures.rs"]
mod fixtures;

fn build_app() -> (axum::Router, AccountId, AccountId) {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();

    // Prepare domain, accounts, and metadata.
    let domain_id: DomainId = "kaigi".parse().expect("domain id");
    let owner_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let owner_id = AccountId::new(domain_id.clone(), owner_kp.public_key().clone());
    let owner = Account::new(owner_id.clone()).build(&owner_id);

    let relay_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let relay_id = AccountId::new(domain_id.clone(), relay_kp.public_key().clone());
    let relay = Account::new(relay_id.clone()).build(&owner_id);

    let registration = KaigiRelayRegistration {
        relay_id: relay_id.clone(),
        hpke_public_key: vec![0xAA, 0xBB, 0xCC],
        bandwidth_class: 5,
    };

    let feedback = KaigiRelayFeedback {
        relay_id: relay_id.clone(),
        call: KaigiId::new(
            domain_id.clone(),
            Name::from_str("demo").expect("call name"),
        ),
        reported_by: owner_id.clone(),
        status: KaigiRelayHealthStatus::Healthy,
        reported_at_ms: 1_650_000,
        notes: Some("operational".to_string()),
    };

    let mut metadata = Metadata::default();
    metadata.insert(
        kaigi_relay_metadata_key(&relay_id).expect("metadata key"),
        Json::try_new(registration).expect("registration json"),
    );
    metadata.insert(
        kaigi_relay_feedback_key(&relay_id).expect("feedback key"),
        Json::try_new(feedback).expect("feedback json"),
    );

    let domain = Domain::new(domain_id.clone())
        .with_metadata(metadata)
        .build(&owner_id);

    let world = iroha_core::prelude::World::with(
        [domain],
        [owner, relay],
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );

    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(64).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let metrics = fixtures::shared_metrics();
    let domain_label = domain_id.to_string();
    metrics
        .kaigi_relay_registered_total
        .with_label_values(&[domain_label.as_str()])
        .inc();
    metrics
        .kaigi_relay_health_reports_total
        .with_label_values(&[domain_label.as_str(), "healthy"])
        .inc();
    metrics
        .kaigi_relay_health_state
        .with_label_values(&[domain_label.as_str(), relay_id.to_string().as_str()])
        .set(KaigiRelayHealthStatus::Healthy.metric_value());

    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::from_secs(0));
    let telemetry_handle = iroha_core::telemetry::start(
        metrics,
        state.clone(),
        kura.clone(),
        queue.clone(),
        peers_rx.clone(),
        time_source,
        false,
    )
    .0;

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = iroha_torii::Torii::new(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(64).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        telemetry_handle,
        true,
    );

    (torii.api_router_for_tests(), relay_id, owner_id)
}

fn compressed_literal(account: &AccountId) -> String {
    let address = account
        .to_account_address()
        .expect("account address encoding");
    address
        .to_compressed_sora()
        .expect("compressed literal encoding")
}

#[tokio::test]
async fn kaigi_endpoints_report_metadata() {
    let (app, relay_account, _owner_account) = build_app();
    let relay_literal = relay_account.to_string();

    // Summary endpoint
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let summary: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(summary["total"].as_u64(), Some(1));
    assert_eq!(
        summary["items"][0]["relay_id"].as_str().unwrap(),
        relay_literal
    );

    // Detail endpoint
    let detail_path = format!("/v1/kaigi/relays/{relay_literal}");
    let detail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(detail_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let detail_bytes = detail_resp.into_body().collect().await.unwrap().to_bytes();
    let detail: norito::json::Value = norito::json::from_slice(&detail_bytes).unwrap();
    assert_eq!(
        detail["relay"]["hpke_fingerprint_hex"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(detail["hpke_public_key_b64"].as_str().unwrap(), "qrvM");

    // Health snapshot
    let health_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_resp.status(), StatusCode::OK);
    let health_bytes = health_resp.into_body().collect().await.unwrap().to_bytes();
    let health: norito::json::Value = norito::json::from_slice(&health_bytes).unwrap();
    assert_eq!(health["healthy_total"].as_u64(), Some(1));
    assert!(!health["domains"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn kaigi_endpoints_support_compressed_address_format() {
    let (app, relay_account, owner_account) = build_app();
    let relay_literal = relay_account.to_string();
    let relay_compressed = compressed_literal(&relay_account);
    let owner_compressed = compressed_literal(&owner_account);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let summary: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        summary["items"][0]["relay_id"].as_str(),
        Some(relay_compressed.as_str())
    );

    let detail_path = format!("/v1/kaigi/relays/{relay_literal}?address_format=compressed");
    let detail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(detail_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let detail_bytes = detail_resp.into_body().collect().await.unwrap().to_bytes();
    let detail: norito::json::Value = norito::json::from_slice(&detail_bytes).unwrap();
    assert_eq!(
        detail["relay"]["relay_id"].as_str(),
        Some(relay_compressed.as_str())
    );
    assert_eq!(
        detail["reported_by"].as_str(),
        Some(owner_compressed.as_str())
    );
}

#[tokio::test]
async fn kaigi_endpoints_honor_json_accept_header() {
    let (app, relay_account, _owner_account) = build_app();
    let relay_literal = relay_account.to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays")
                .header("Accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let summary: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        summary["items"][0]["relay_id"].as_str(),
        Some(relay_literal.as_str())
    );

    let detail_path = format!("/v1/kaigi/relays/{relay_literal}");
    let detail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(detail_path)
                .header("Accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    assert_eq!(
        detail_resp.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    let detail_bytes = detail_resp.into_body().collect().await.unwrap().to_bytes();
    let detail: norito::json::Value = norito::json::from_slice(&detail_bytes).unwrap();
    assert_eq!(
        detail["relay"]["relay_id"].as_str(),
        Some(relay_literal.as_str())
    );
}

#[tokio::test]
async fn kaigi_sse_accepts_compressed_relay_filter() {
    let (app, relay_account, _owner_account) = build_app();
    let relay_compressed = compressed_literal(&relay_account);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/kaigi/relays/events?relay={relay_compressed}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn kaigi_sse_rejects_invalid_relay_filter() {
    let (app, _relay_account, _owner_account) = build_app();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays/events?relay=snx1invalid@kaigi")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
