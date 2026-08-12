#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router build sanity across feature flags.
//!
//! This test exercises `Torii::api_router_for_tests()` to ensure the router can be
//! instantiated under different compile-time feature combinations (`telemetry/app_api/connect`,
//! etc.). Each cfg-gated block runs only when the corresponding feature is enabled.
#![allow(clippy::too_many_lines)]

use axum::http::{Request, StatusCode, Uri};
use iroha_core::prelude::World;
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
    iroha_torii_shared::uri::SCHEMA,
];

async fn fetch_generated_openapi(app: &axum::Router) -> Option<String> {
    for path in OPENAPI_CANDIDATES {
        let request = Request::builder()
            .uri(*path)
            .body(axum::body::Body::empty())
            .expect("valid request builder");
        let response = fixtures::request(&app, request).await.ok()?;
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

    // Minimal in-memory components required by Torii
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());

    let app = torii.router();

    diff_openapi_if_available(&app).await;

    // A couple of smoke GETs that are present regardless of features
    let resp1 = fixtures::request(
        &app,
        fixtures::operator_signed_request(
            &cfg.common.key_pair,
            fixtures::get_request(&("/v1/sumeragi/evidence/count")),
            &[],
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        resp1.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    for (path, expected_status) in [
        ("/v1/sumeragi/evidence", StatusCode::METHOD_NOT_ALLOWED),
        ("/v1/sumeragi/vrf/commit", StatusCode::NOT_FOUND),
        ("/v1/sumeragi/vrf/reveal", StatusCode::NOT_FOUND),
    ] {
        let response = fixtures::request(
            &app,
            fixtures::operator_signed_request(
                &cfg.common.key_pair,
                Request::builder()
                    .method("POST")
                    .uri(Uri::from_static(path))
                    .body(axum::body::Body::empty())
                    .unwrap(),
                &[],
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            response.status(),
            expected_status,
            "retired Sumeragi mutation route {path} must remain absent"
        );
    }

    let resp2 = fixtures::request(
        &app,
        fixtures::get_request(&(iroha_torii_shared::uri::PEERS)),
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
        let resp = fixtures::request_get(&app, "/v1/domains").await.unwrap();
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
            let response = fixtures::request_get(&app, path).await.unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        }
    }

    #[cfg(feature = "connect")]
    {
        let resp = fixtures::request_get(&app, "/v1/connect/status")
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[cfg(not(feature = "profiling"))]
    {
        let resp = fixtures::request(
            &app,
            fixtures::get_request(&(iroha_torii_shared::uri::PROFILE)),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(not(feature = "schema"))]
    {
        let resp = fixtures::request(
            &app,
            fixtures::get_request(&(iroha_torii_shared::uri::SCHEMA)),
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
            let resp = fixtures::request_get(&app, path).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[cfg(not(feature = "zk-verify-batch"))]
    {
        let resp = fixtures::request(
            &app,
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
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());
    let app = torii.router();

    let resp = app
        .oneshot(fixtures::operator_signed_request(
            &cfg.common.key_pair,
            fixtures::get_request(&("/status")),
            &[],
        ))
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR
    ));
}
