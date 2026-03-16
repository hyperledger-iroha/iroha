#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke tests for Torii asset definitions endpoints.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::http::Request;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::KeyPair;
use iroha_data_model::Registrable as _;
use iroha_data_model::prelude as dm;
use iroha_torii::Torii;
use tower::ServiceExt as _;

fn seeded_state() -> Arc<State> {
    let authority = dm::AccountId::new(KeyPair::random().public_key().clone());
    let domain_id: dm::DomainId = "wonderland".parse().expect("valid domain");
    let domain = dm::Domain::new(domain_id.clone()).build(&authority);
    let account = dm::Account::new(authority.clone().to_account_id(domain_id)).build(&authority);

    let mut pkr = dm::AssetDefinition::numeric(
        "aid:550e8400e29b41d4a7164466554400dd"
            .parse()
            .expect("asset definition id"),
    )
    .with_name("PKR".to_owned())
    .build(&authority);
    pkr.alias = Some("pkr#sbp".parse().expect("asset alias"));

    let usd = dm::AssetDefinition::numeric(
        "aid:550e8400e29b41d4a7164466554400ee"
            .parse()
            .expect("asset definition id"),
    )
    .with_name("USD".to_owned())
    .build(&authority);

    Arc::new(State::new_for_testing(
        World::with([domain], [account], [pkr, usd]),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ))
}

fn build_app(state: Arc<State>) -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    Torii::new_with_handle(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        iroha_torii::MaybeTelemetry::disabled(),
    )
    .api_router_for_tests()
}

#[tokio::test]
async fn asset_definitions_endpoints_return_name_and_alias() {
    let app = build_app(seeded_state());

    // GET /v1/assets/definitions
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/assets/definitions?offset=0")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let items = doc["items"].as_array().expect("items");
    assert_eq!(doc["total"].as_u64(), Some(2));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("PKR") && item["alias"].as_str() == Some("pkr#sbp")
    }));
    assert!(
        items
            .iter()
            .any(|item| item["name"].as_str() == Some("USD") && item["alias"].is_null())
    );

    // POST /v1/assets/definitions/query
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/assets/definitions/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let items = doc["items"].as_array().expect("items");
    assert_eq!(doc["total"].as_u64(), Some(2));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("PKR") && item["alias"].as_str() == Some("pkr#sbp")
    }));
    assert!(
        items
            .iter()
            .any(|item| item["name"].as_str() == Some("USD") && item["alias"].is_null())
    );
}
