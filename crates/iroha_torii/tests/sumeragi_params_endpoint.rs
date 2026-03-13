#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v2/sumeragi/params

#[cfg(feature = "telemetry")]
use axum::http::Request;
#[cfg(feature = "telemetry")]
use iroha_config::parameters::actual::TelemetryProfile;
#[cfg(feature = "telemetry")]
use tower::ServiceExt as _;

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn sumeragi_params_endpoint_shape() {
    // Build Torii router using consolidated config
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = iroha_core::kiso::KisoHandle::start(cfg.clone());
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let state = std::sync::Arc::new(iroha_core::state::State::new_for_testing(
        iroha_core::state::World::default(),
        kura.clone(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = std::sync::Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry_handle =
        iroha_torii::MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    #[cfg(not(feature = "telemetry"))]
    let telemetry_handle = iroha_torii::MaybeTelemetry::disabled();

    let torii = iroha_torii::Torii::new_with_handle(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        iroha_core::query::store::LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        telemetry_handle,
    );
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v2/sumeragi/params")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    // Presence checks only (defaults come from configuration and may change)
    for k in [
        "block_time_ms",
        "commit_time_ms",
        "max_clock_drift_ms",
        "collectors_k",
        "redundant_send_r",
        "da_enabled",
        "chain_height",
    ] {
        assert!(v.get(k).is_some(), "missing key {k}");
    }
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn configuration_endpoint_includes_confidential_gas() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = iroha_core::kiso::KisoHandle::start(cfg.clone());
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let state = std::sync::Arc::new(iroha_core::state::State::new_for_testing(
        iroha_core::state::World::default(),
        kura.clone(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = std::sync::Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry_handle =
        iroha_torii::MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    #[cfg(not(feature = "telemetry"))]
    let telemetry_handle = iroha_torii::MaybeTelemetry::disabled();

    let torii = iroha_torii::Torii::new_with_handle(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        iroha_core::query::store::LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        telemetry_handle,
    );
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v2/configuration")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let gas = v
        .get("confidential_gas")
        .and_then(|value| value.as_object())
        .expect("confidential_gas JSON object present");
    for key in [
        "proof_base",
        "per_public_input",
        "per_proof_byte",
        "per_nullifier",
        "per_commitment",
    ] {
        assert!(gas.get(key).is_some(), "confidential_gas missing key {key}");
    }
}
