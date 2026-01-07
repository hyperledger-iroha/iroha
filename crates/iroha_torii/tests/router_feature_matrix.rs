//! Router build sanity across feature flags.
//!
//! This test exercises `Torii::api_router_for_tests()` to ensure the router can be
//! instantiated under different compile-time feature combinations (`telemetry/app_api/connect`,
//! etc.). Each cfg-gated block runs only when the corresponding feature is enabled.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use axum::http::{Request, StatusCode, Uri};
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    prelude::World,
    query::store::LiveQueryStore,
    state::State,
    sumeragi::consensus::{Evidence, EvidenceKind, EvidencePayload, Phase, Vote},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::ChainId;
use norito::json;
use tower::ServiceExt as _; // for Router::oneshot

#[path = "fixtures.rs"]
mod fixtures;

/// Candidate paths that may expose an `OpenAPI` document.
const OPENAPI_CANDIDATES: &[&str] = &[
    "/openapi.json",
    "/openapi",
    "/swagger.json",
    "/swagger/v1/swagger.json",
    "/schema",
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

fn sample_evidence_hex() -> String {
    fn vote_with_seed(seed: u8) -> Vote {
        let hash = Hash::prehashed([seed; 32]);
        Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::from_untyped_unchecked(hash),
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        }
    }
    let v1 = vote_with_seed(0x11);
    let v2 = vote_with_seed(0x22);
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };
    hex::encode(norito::to_bytes(&evidence).expect("encode evidence"))
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
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
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
                    ts,
                    false,
                )
                .0
            };
            iroha_torii::Torii::new(
                ChainId::from("test-chain"),
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
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/sumeragi/evidence/count"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp1.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    let post_body = format!("{{\"evidence_hex\":\"{}\"}}", sample_evidence_hex());
    let resp_post = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/sumeragi/evidence"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(post_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp_post.status(),
            StatusCode::ACCEPTED
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::FORBIDDEN
                | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status from evidence POST: {}",
        resp_post.status()
    );

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/peers"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Depending on rate-limits/test timing, allow OK or 429
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

    #[cfg(feature = "connect")]
    {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
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
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
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
            ts,
            false,
        )
        .0
    };
    let torii = iroha_torii::Torii::new(
        ChainId::from("test-chain"),
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
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR
    ));
}
