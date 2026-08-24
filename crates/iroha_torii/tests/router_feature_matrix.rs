#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router build sanity across feature flags.
//!
//! This test exercises `Torii::api_router_for_tests()` to ensure the router can be
//! instantiated under different compile-time feature combinations (`telemetry/app_api/connect`,
//! etc.). Each cfg-gated block runs only when the corresponding feature is enabled.
#![allow(clippy::too_many_lines)]
use axum::http::{Request, StatusCode, Uri};
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, state::State,
};
use iroha_data_model::{ChainId, peer::PeerId};
use norito::json;
use std::sync::Arc;
use tower::ServiceExt as _; // for Router::oneshot
#[path = "fixtures.rs"]
mod fixtures;
/// Candidate paths that may expose an `OpenAPI` document.
const OPENAPI_CANDIDATES: &[&str] = &[
    "/openapi.json",
    "/openapi",
    "/swagger.json",
    "/swagger/v1/swagger.json",
    iroha_torii_shared::uri::SCHEMA,
];
async fn fetch_generated_openapi(app: &axum::Router) -> Option<String> {
    for path in OPENAPI_CANDIDATES {
        let request = Request::builder()
            .uri(*path)
            .body(axum::body::Body::empty())
            .expect("valid request builder");
        let response = app.clone().oneshot(request).await.ok()?;
        if !response.status().is_success() {
            continue;
        }
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .ok()?
            .to_bytes();
        if body.is_empty() {
            continue;
        }
        if let Ok(text) = String::from_utf8(body.to_vec()) {
            return Some(text);
        }
    }
    None
}
fn canonicalize_json(input: &str) -> Option<String> {
    let value: json::Value = json::from_str(input).ok()?;
    json::to_string_pretty(&value).ok()
}
fn diff_preview(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<_> = expected.lines().collect();
    let actual_lines: Vec<_> = actual.lines().collect();
    let max = expected_lines.len().max(actual_lines.len());
    for idx in 0..max {
        let left = expected_lines.get(idx).copied().unwrap_or("<EOF>");
        let right = actual_lines.get(idx).copied().unwrap_or("<EOF>");
        if left != right {
            return format!(
                "first difference at line {}\n  expected: {}\n    actual: {}",
                idx + 1,
                left,
                right
            );
        }
    }
    "spec contents differ (unable to locate differing line)".to_owned()
}
async fn diff_openapi_if_available(app: &axum::Router) {
    let Some(raw_spec) = fetch_generated_openapi(app).await else {
        assert!(
            std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),
            "IROHA_TORII_OPENAPI_EXPECTED is set but router did not expose an OpenAPI-compatible endpoint"
        );
        return;
    };
    if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {
        if let Some(pretty) = canonicalize_json(&raw_spec) {
            if let Err(err) = tokio::fs::write(&actual_path, pretty.as_bytes()).await {
                eprintln!("failed to write OpenAPI snapshot to {actual_path}: {err}");
            }
        } else if let Err(err) = tokio::fs::write(&actual_path, raw_spec.as_bytes()).await {
            eprintln!("failed to write raw OpenAPI snapshot to {actual_path}: {err}");
        }
    }
    let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {
        return;
    };
    let expected_raw = match tokio::fs::read_to_string(&expected_path).await {
        Ok(contents) => contents,
        Err(err) => panic!("failed to read expected OpenAPI snapshot from {expected_path}: {err}"),
    };
    let Some(expected) = canonicalize_json(&expected_raw) else {
        panic!("expected OpenAPI snapshot at {expected_path} is not valid JSON");
    };
    let Some(actual) = canonicalize_json(&raw_spec) else {
        panic!("generated OpenAPI document is not valid JSON: consider regenerating it");
    };
    if expected != actual {
        let preview = diff_preview(&expected, &actual);
        panic!(
            "generated OpenAPI document mismatched expected snapshot ({expected_path}):\n{preview}"
        );
    }
}
#[allow(clippy::too_many_lines)]
fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.connect.enabled = cfg!(feature = "connect");
    cfg
}
#[tokio::test]
async fn router_builds_under_current_features() {
    // Start a minimal Kiso
    let cfg = mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    // Minimal in-memory components required by Torii
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx; // keep channel alive
    let da_receipt_signer = cfg.common.key_pair.clone();
    // Build Torii. Telemetry handle is only required when the feature is enabled.
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            // Create a dummy Telemetry handle; it won't be used in this test.
            let telemetry = {
                use iroha_core::telemetry as core_telemetry;
                use iroha_primitives::time::TimeSource;
                let metrics = fixtures::shared_metrics();
                let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
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
            iroha_torii::Torii::new(
                ChainId::from("test-chain"),
                iroha_torii::test_utils::signed_query_network_id(),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
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
            iroha_torii::Torii::new(
                ChainId::from("test-chain"),
                iroha_torii::test_utils::signed_query_network_id(),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    let app = torii.api_router_for_tests();
    diff_openapi_if_available(&app).await;
    // A couple of smoke GETs that are present regardless of features
    let resp1 = app
        .clone()
        .oneshot(fixtures::operator_signed_request(
            &cfg.common.key_pair,
            Request::builder()
                .uri(Uri::from_static("/v1/sumeragi/evidence/count"))
                .body(axum::body::Body::empty())
                .unwrap(),
            &[],
        ))
        .await
        .unwrap();
    assert!(matches!(
        resp1.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    for (method, path, expected_status) in [
        (
            "POST",
            "/v1/sumeragi/evidence",
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        ("POST", "/v1/sumeragi/vrf/commit", StatusCode::NOT_FOUND),
        ("GET", "/v1/sumeragi/vrf/epoch/0", StatusCode::NOT_FOUND),
        ("GET", "/v1/sumeragi/vrf/penalties/0", StatusCode::NOT_FOUND),
        ("POST", "/v1/sumeragi/vrf/reveal", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(fixtures::operator_signed_request(
                &cfg.common.key_pair,
                Request::builder()
                    .method(method)
                    .uri(Uri::from_static(path))
                    .body(axum::body::Body::empty())
                    .unwrap(),
                &[],
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            expected_status,
            "retired Sumeragi route {method} {path} must remain absent"
        );
    }
    let resp2 = app
        .clone()
        .oneshot(fixtures::operator_signed_request(
            &cfg.common.key_pair,
            Request::builder()
                .uri(Uri::from_static(iroha_torii_shared::uri::PEERS))
                .body(axum::body::Body::empty())
                .unwrap(),
            &[],
        ))
        .await
        .unwrap();
    // Depending on rate-limits/test timing, allow OK or 429 after operator admission.
    assert!(matches!(
        resp2.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    #[cfg(feature = "app_api")]
    {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/domains"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ));
    }
    #[cfg(all(feature = "app_api", not(feature = "telemetry")))]
    {
        for path in [
            "/v1/kaigi/relays",
            "/v1/kaigi/relays/relay-id",
            "/v1/kaigi/relays/health",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(Uri::from_static(path))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        }
    }
    #[cfg(feature = "connect")]
    {
        let session = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(session.status(), StatusCode::BAD_REQUEST);
        let aggregate = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status/aggregate"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(aggregate.status(), StatusCode::UNAUTHORIZED);
    }
    #[cfg(not(feature = "profiling"))]
    {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static(iroha_torii_shared::uri::PROFILE))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    #[cfg(not(feature = "schema"))]
    {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static(iroha_torii_shared::uri::SCHEMA))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    #[cfg(not(feature = "telemetry"))]
    {
        for path in [
            iroha_torii_shared::uri::STATUS,
            "/status/peers",
            iroha_torii_shared::uri::METRICS,
            iroha_torii_shared::uri::AXT_PROOF_CACHE_STATUS,
            "/v1/debug/witness",
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(Uri::from_static(path))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }
    #[cfg(not(feature = "zk-verify-batch"))]
    {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(Uri::from_static("/v1/zk/verify-batch"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
#[cfg(feature = "telemetry")]
#[tokio::test]
async fn router_exposes_status_when_telemetry_enabled() {
    // Build with telemetry enabled
    let cfg = mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let da_receipt_signer = cfg.common.key_pair.clone();
    // Telemetry handle
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        use iroha_primitives::time::TimeSource;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
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
    let torii = iroha_torii::Torii::new(
        ChainId::from("test-chain"),
        iroha_torii::test_utils::signed_query_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        telemetry,
        true,
    );
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(fixtures::operator_signed_request(
            &cfg.common.key_pair,
            Request::builder()
                .uri(Uri::from_static("/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
            &[],
        ))
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR
    ));
}
