#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test for App API routes wiring.
#![allow(clippy::too_many_lines)]
//!
//! Builds a minimal Torii instance and checks that a couple of App API
//! endpoints are reachable via the consolidated helper-built router.

use std::sync::Arc;

use axum::http::{Request, StatusCode, Uri};
// use iroha_config::base::WithOrigin; // unused in this smoke test
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, state::State,
};
use iroha_data_model::{ChainId, peer::PeerId};
use tower::ServiceExt as _; // for Router::oneshot
// use iroha_primitives::addr::socket_addr; // unused in this smoke test

#[path = "fixtures.rs"]
mod fixtures;

// Minimal root config for starting Kiso and wiring Torii
fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}

#[tokio::test]
async fn app_api_router_smoke() {
    // Start Kiso and minimal components for Torii
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
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

    // Build Torii (telemetry optional)
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            use iroha_core::telemetry as core_telemetry;
            use iroha_primitives::time::TimeSource;
            let metrics = fixtures::shared_metrics();
            let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
            let telemetry = core_telemetry::start(
                metrics,
                state.clone(),
                kura.clone(),
                queue.clone(),
                peers_rx.clone(),
                local_peer_id,
                ts,
                false,
            )
            .0;
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
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let app = torii.api_router_for_tests();

    // 1) App API: GET /v1/accounts/{account_id}/assets — use a bogus id to avoid
    // state setup; we only care that the route exists and responds deterministically.
    let resp_assets = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static(
                    "/v1/accounts/bogus_account_id/assets?offset=0",
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_assets.status(),
        StatusCode::OK
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_REQUEST
    ));

    // 2) App API: GET /v1/events/sse — endpoint exists; allow OK or 429 depending on rate limits
    let resp_sse = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/events/sse"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_sse.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // 3) App API: GET /v1/webhooks — ensure route exists; allow OK or 429
    let resp_webhooks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/webhooks"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_webhooks.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // 4) App API: GET /v1/assets/{definition_id}/holders — use percent-encoded '#'
    // in the definition id (bogus#wonderland) to ensure parsing is exercised.
    let resp_holders = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static(
                    "/v1/assets/bogus%23wonderland/holders?offset=0",
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_holders.status(),
        StatusCode::OK
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::NOT_FOUND
    ));

    // 5) App API: POST /v1/webhooks — create a webhook (write path)
    let body = r#"{
  "url": "https://example.com/callback",
  "secret": null,
  "active": true
}"#;
    let resp_webhook_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/webhooks"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp_webhook_create.status();
    assert!(matches!(
        status,
        StatusCode::CREATED | StatusCode::TOO_MANY_REQUESTS
    ));
}
