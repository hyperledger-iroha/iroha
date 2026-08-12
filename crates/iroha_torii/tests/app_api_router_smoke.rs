#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test for App API routes wiring.
#![allow(clippy::too_many_lines)]
//!
//! Builds a minimal Torii instance and checks that a couple of App API
//! endpoints are reachable via the consolidated helper-built router.

use axum::http::{Request, StatusCode, Uri, header::CONTENT_TYPE};
use iroha_core::prelude::World;
use tower::ServiceExt as _; // for Router::oneshot

#[path = "fixtures.rs"]
mod fixtures;

// Minimal root config for starting Kiso and wiring Torii
fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}

async fn assert_route_is_not_auth_denied(
    app: axum::Router,
    request: Request<axum::body::Body>,
) -> StatusCode {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    assert!(
        !matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        "route unexpectedly denied access with {status}"
    );
    status
}

async fn assert_sse_route_is_not_auth_denied(app: &axum::Router, path: &str) {
    let response = fixtures::request_get(app, path).await.unwrap();
    assert!(!matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ));
    if response.status() == StatusCode::OK {
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|header| header.to_str().ok())
            .unwrap_or("");
        assert!(content_type.contains("text/event-stream"));
    }
}

async fn assert_get_route_is_not_auth_denied(app: &axum::Router, path: &str) -> StatusCode {
    assert_route_is_not_auth_denied(app.clone(), fixtures::get_request(path)).await
}

#[tokio::test]
async fn app_api_router_smoke() {
    // Start Kiso and minimal components for Torii
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = mk_minimal_root_cfg();
    cfg.torii.webhooks_enabled = true;
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());

    let app = torii.router();

    // The v1 alias VOPRF-shaped hash helper was retired before release because it was
    // neither keyed nor verifiable. Every legacy request shape must remain unroutable,
    // including malformed input, a replay, cross-domain material, and wrong-key/proof fields.
    let retired_alias_voprf_attempts = [
        ("malformed", "{"),
        ("replay-first", r#"{"blinded_element_hex":"deadbeef"}"#),
        ("replay-second", r#"{"blinded_element_hex":"deadbeef"}"#),
        (
            "cross-domain",
            r#"{"blinded_element_hex":"deadbeef","domain":"other-ledger"}"#,
        ),
        (
            "wrong-key",
            r#"{"blinded_element_hex":"deadbeef","key_id":"wrong","proof":"00"}"#,
        ),
    ];
    for (case, body) in retired_alias_voprf_attempts {
        let response = fixtures::request(
            &app,
            Request::builder()
                .method("POST")
                .uri("/v1/aliases/voprf/evaluate")
                .header(CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body))
                .expect("legacy alias VOPRF request"),
        )
        .await
        .expect("router response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "retired alias VOPRF route accepted {case} request"
        );
    }

    // 1) App API: GET /v1/accounts/{account_id}/assets — use a bogus id to avoid
    // state setup; we only care that the route exists and responds deterministically.
    assert_get_route_is_not_auth_denied(&app, "/v1/accounts/bogus_account_id/assets?offset=0")
        .await;

    // 2) App API: GET /v1/events/sse — endpoint exists; allow OK or 429 depending on rate limits
    assert_sse_route_is_not_auth_denied(&app, "/v1/events/sse").await;

    // 2b) App API: GET /v1/explorer/blocks/stream — endpoint exists; allow OK or 429.
    assert_sse_route_is_not_auth_denied(&app, "/v1/explorer/blocks/stream").await;

    // 2c) App API: GET /v1/gov/stream — endpoint exists; allow OK/429.
    assert_sse_route_is_not_auth_denied(&app, "/v1/gov/stream").await;

    // 2d) App API: GET /v1/telemetry/live — endpoint exists; allow OK/429/403.
    let resp_telemetry_live = fixtures::request_get(&app, "/v1/telemetry/live")
        .await
        .unwrap();
    #[cfg(feature = "telemetry")]
    {
        assert!(matches!(
            resp_telemetry_live.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN
        ));
        if resp_telemetry_live.status() == StatusCode::OK {
            let ct = resp_telemetry_live
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            assert!(ct.contains("text/event-stream"));
        }
    }
    #[cfg(not(feature = "telemetry"))]
    {
        assert!(!matches!(
            resp_telemetry_live.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ));
    }

    // 2e) App API: GET /v1/telemetry/propagation — endpoint exists; allow OK/429/403.
    let resp_telemetry_propagation = fixtures::request_get(&app, "/v1/telemetry/propagation")
        .await
        .unwrap();
    #[cfg(feature = "telemetry")]
    {
        assert!(matches!(
            resp_telemetry_propagation.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN
        ));
    }
    #[cfg(not(feature = "telemetry"))]
    {
        assert!(!matches!(
            resp_telemetry_propagation.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ));
    }

    // 3) App API: GET /v1/webhooks — ensure route exists; allow OK or 429
    assert_get_route_is_not_auth_denied(&app, "/v1/webhooks").await;

    // 4) App API: GET /v1/assets/{definition_id}/holders — use percent-encoded '#'
    // in the definition id (bogus#wonderland) to ensure parsing is exercised.
    assert_get_route_is_not_auth_denied(&app, "/v1/assets/bogus%23wonderland/holders?offset=0")
        .await;

    // 5) App API: POST /v1/webhooks — create a webhook (write path)
    let body = r#"{
  "url": "https://example.com/callback",
  "secret": null,
  "active": true
}"#;
    assert_route_is_not_auth_denied(
        app.clone(),
        fixtures::post_json_request(&("/v1/webhooks"), axum::body::Body::from(body)),
    )
    .await;

    assert_get_route_is_not_auth_denied(
        &app,
        "/v1/contracts/rollups/swaps/fills?authority=not-a-real-authority",
    )
    .await;

    assert_get_route_is_not_auth_denied(
        &app,
        "/v1/contracts/rollups/swaps/candles?authority=not-a-real-authority",
    )
    .await;

    assert_get_route_is_not_auth_denied(
        &app,
        "/v1/contracts/rollups/uranai/markets/history?market_id=not-a-real-market",
    )
    .await;

    assert_get_route_is_not_auth_denied(
        &app,
        "/v1/contracts/rollups/trader/activity?authority=not-a-real-authority",
    )
    .await;

    assert_get_route_is_not_auth_denied(
        &app,
        "/v1/contracts/rollups/trader/account?authority=not-a-real-authority",
    )
    .await;
}

#[tokio::test]
async fn contract_routes_honor_api_token_requirement() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = mk_minimal_root_cfg();
    cfg.torii.require_api_token = true;
    cfg.torii.api_tokens = vec!["test-token".to_owned()];
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());

    let app = torii.router();

    for (method, path) in [
        ("POST", "/v1/contracts/deploy"),
        ("POST", "/v1/contracts/deploy-bundle"),
        (
            "GET",
            "/v1/contracts/deploy-bundles/not-a-real-bundle-digest",
        ),
    ] {
        let response = fixtures::request(
            &app,
            Request::builder()
                .method(method)
                .uri(path)
                .header("x-api-token", "test-token")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "path {path}");
    }

    assert_route_is_not_auth_denied(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(Uri::from_static("/v1/contracts/call"))
            .header("x-api-token", "test-token")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap(),
    )
    .await;

    assert_route_is_not_auth_denied(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(Uri::from_static("/v1/contracts/view"))
            .header("x-api-token", "test-token")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap(),
    )
    .await;

    assert_route_is_not_auth_denied(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(Uri::from_static("/v1/contracts/view/batch"))
            .header("x-api-token", "test-token")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap(),
    )
    .await;

    assert_route_is_not_auth_denied(
        app.clone(),
        Request::builder()
            .uri(Uri::from_static("/v1/contracts/state"))
            .header("x-api-token", "test-token")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
}
