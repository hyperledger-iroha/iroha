#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests for offline rejection telemetry endpoint.
#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle, kura::Kura, query::store::LiveQueryStore, queue::Queue, state::State,
};
use iroha_data_model::peer::PeerId;
use iroha_primitives::time::TimeSource;
use iroha_telemetry::metrics::Metrics;
use iroha_torii::Torii;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn build_app(metrics: Arc<Metrics>) -> Router {
    build_app_with_metrics(metrics, true)
}

fn build_app_with_metrics(metrics: Arc<Metrics>, telemetry_enabled: bool) -> Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = iroha_core::prelude::World::with(
        Vec::new(),
        Vec::new(),
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(64).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::from_secs(0));
    let telemetry_handle = iroha_core::telemetry::start(
        metrics,
        state.clone(),
        kura.clone(),
        queue.clone(),
        peers_rx.clone(),
        local_peer_id,
        time_source,
        false,
    )
    .0;

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = Torii::new(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        state,
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        telemetry_handle,
        telemetry_enabled,
    );

    torii.api_router_for_tests()
}

#[tokio::test]
async fn offline_rejections_reports_counters() {
    let metrics = fixtures::shared_metrics();
    metrics.record_offline_transfer_rejection(
        iroha_data_model::offline::OfflineTransferRejectionPlatform::Apple,
        iroha_data_model::offline::OfflineTransferRejectionReason::PlatformAttestationInvalid,
    );
    metrics.record_offline_transfer_rejection(
        iroha_data_model::offline::OfflineTransferRejectionPlatform::Apple,
        iroha_data_model::offline::OfflineTransferRejectionReason::PlatformAttestationInvalid,
    );
    metrics.record_offline_transfer_rejection(
        iroha_data_model::offline::OfflineTransferRejectionPlatform::Android,
        iroha_data_model::offline::OfflineTransferRejectionReason::CounterViolation,
    );

    let app = build_app(metrics);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v2/offline/rejections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(body["total"].as_u64(), Some(3));
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    let mut apple_count = None;
    let mut android_count = None;
    let mut apple_reason = None;
    let mut android_reason = None;
    for entry in items {
        match entry["platform"].as_str() {
            Some("apple") => {
                apple_count = entry["count"].as_u64();
                apple_reason = entry["reason"].as_str().map(str::to_owned);
            }
            Some("android") => {
                android_count = entry["count"].as_u64();
                android_reason = entry["reason"].as_str().map(str::to_owned);
            }
            _ => {}
        }
    }
    assert_eq!(apple_count, Some(2));
    assert_eq!(
        apple_reason.as_deref(),
        Some("platform_attestation_invalid")
    );
    assert_eq!(android_count, Some(1));
    assert_eq!(android_reason.as_deref(), Some("counter_conflict"));
}

#[tokio::test]
async fn offline_rejections_require_metrics_profile() {
    let metrics = fixtures::shared_metrics();
    let app = build_app_with_metrics(metrics, false);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v2/offline/rejections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    if let Ok(body) = norito::json::from_slice::<norito::json::Value>(&bytes) {
        assert_eq!(
            body["code"].as_str(),
            Some("telemetry_profile_restricted"),
            "unexpected error envelope: {body:?}"
        );
        return;
    }

    let view = norito::core::from_bytes_view(&bytes).unwrap();
    let (code, message) = view.decode::<(String, String)>().unwrap();
    assert_eq!(code, "telemetry_profile_restricted");
    assert!(
        message.contains("offline_rejections"),
        "unexpected telemetry error message: {message}"
    );
}
