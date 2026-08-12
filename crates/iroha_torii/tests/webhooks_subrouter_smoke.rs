#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test that Webhooks endpoints are exposed via the merged sub-router.
#![cfg(feature = "app_api")]

use http::StatusCode;
use iroha_core::state::World;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

#[tokio::test]
async fn webhooks_endpoints_exposed() {
    // Minimal Torii setup
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.webhooks_enabled = true;
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());

    let app = torii.router();

    // POST /v1/webhooks — create a webhook
    let resp = fixtures::request(
        &app,
        fixtures::post_json_request(
            &("/v1/webhooks"),
            axum::body::Body::from("{\"url\":\"https://example.com/webhook\",\"active\":true}"),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::CREATED | StatusCode::TOO_MANY_REQUESTS
    ));

    // GET /v1/webhooks — list webhooks
    let resp = app
        .oneshot(fixtures::get_request(&("/v1/webhooks")))
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
}

#[tokio::test]
async fn webhooks_endpoints_disabled_by_default() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());

    let app = torii.router();
    let resp = app
        .oneshot(fixtures::get_request(&("/v1/webhooks")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
