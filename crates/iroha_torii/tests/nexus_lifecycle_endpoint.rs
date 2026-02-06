#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level regression tests for `/v1/nexus/lifecycle`.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    EventsSender,
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, Queue, QueueLimits, SingleLaneRouter},
    state::State,
};
use iroha_torii::Torii;
use iroha_torii_shared::uri::NEXUS_LANE_LIFECYCLE;
use tower::ServiceExt as _;

fn decode_norito_json(bytes: &[u8]) -> norito::json::Value {
    let decoded: String = norito::decode_from_bytes(bytes).expect("decode Norito JSON");
    norito::json::from_str(&decoded).expect("parse JSON payload")
}

fn build_app(nexus_enabled: bool) -> Router {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.nexus.enabled = nexus_enabled;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = iroha_core::prelude::World::with(
        Vec::new(),
        Vec::new(),
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );
    let mut state = State::new_for_testing(world, kura.clone(), query);
    state
        .set_nexus(cfg.nexus.clone())
        .expect("apply initial Nexus config");
    let state = Arc::new(state);

    let events_sender: EventsSender = tokio::sync::broadcast::channel(64).0;
    let router: Arc<dyn iroha_core::queue::LaneRouter> = if nexus_enabled {
        Arc::new(ConfigLaneRouter::new(
            cfg.nexus.routing_policy.clone(),
            cfg.nexus.dataspace_catalog.clone(),
            cfg.nexus.lane_catalog.clone(),
        ))
    } else {
        Arc::new(SingleLaneRouter::new())
    };
    let lane_catalog = Arc::new(cfg.nexus.lane_catalog.clone());
    let dataspace_catalog = Arc::new(cfg.nexus.dataspace_catalog.clone());
    let queue_limits = QueueLimits::from_nexus(&cfg.nexus);
    let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
        QueueConfig::default(),
        events_sender.clone(),
        router,
        queue_limits,
        &lane_catalog,
        &dataspace_catalog,
        None,
    ));
    let view = state.view();
    queue.reconfigure_nexus(&state.nexus_snapshot(), &view, None);

    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let da_receipt_signer = cfg.common.key_pair.clone();
    #[cfg(feature = "telemetry")]
    let torii = {
        let telemetry = iroha_core::telemetry::Telemetry::new(
            Arc::new(iroha_telemetry::metrics::Metrics::default()),
            false,
        );
        Torii::new(
            iroha_data_model::ChainId::from("test-chain"),
            kiso,
            cfg.torii.clone(),
            queue,
            events_sender,
            LiveQueryStore::start_test(),
            kura,
            Arc::clone(&state),
            da_receipt_signer,
            iroha_torii::OnlinePeersProvider::new(peers_rx),
            telemetry,
            false,
        )
    };
    #[cfg(not(feature = "telemetry"))]
    let torii = Torii::new(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
    );

    torii.api_router_for_tests()
}

#[tokio::test]
async fn nexus_lifecycle_applies_plan_and_reports_lane_count() {
    let app = build_app(true);
    let body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["ok"].as_bool(), Some(true));
    assert_eq!(payload["lane_count"].as_u64(), Some(2));
}

#[tokio::test]
async fn nexus_lifecycle_rejects_when_disabled() {
    let app = build_app(false);
    let body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nexus_lifecycle_supports_retire_after_add() {
    let app = build_app(true);
    let add_body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("content-type", "application/json")
                .body(Body::from(add_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::ACCEPTED);

    let retire_body = r#"{"additions":[],"retire":[1]}"#;
    let retire_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("content-type", "application/json")
                .body(Body::from(retire_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retire_resp.status(), StatusCode::ACCEPTED);
    let bytes = retire_resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));
}
