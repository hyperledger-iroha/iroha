#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test for App API routes wiring.
#![allow(clippy::too_many_lines)]
//!
//! Builds a minimal Torii instance and checks that a couple of App API
//! endpoints are reachable via the consolidated helper-built router.

use std::sync::Arc;

use axum::http::{Request, StatusCode, Uri, header::CONTENT_TYPE};
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

    // 1) App API: GET /v2/accounts/{account_id}/assets — use a bogus id to avoid
    // state setup; we only care that the route exists and responds deterministically.
    let resp_assets = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static(
                    "/v2/accounts/bogus_account_id/assets?offset=0",
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

    // 2) App API: GET /v2/events/sse — endpoint exists; allow OK or 429 depending on rate limits
    let resp_sse = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/events/sse"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_sse.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    if resp_sse.status() == StatusCode::OK {
        let ct = resp_sse
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("text/event-stream"));
    }

    // 2b) App API: GET /v2/explorer/blocks/stream — endpoint exists; allow OK or 429.
    let resp_blocks_sse = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/explorer/blocks/stream"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_blocks_sse.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    if resp_blocks_sse.status() == StatusCode::OK {
        let ct = resp_blocks_sse
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("text/event-stream"));
    }

    // 2c) App API: GET /v2/gov/stream — endpoint exists; allow OK/429.
    let resp_gov_sse = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/gov/stream"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_gov_sse.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    if resp_gov_sse.status() == StatusCode::OK {
        let ct = resp_gov_sse
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("text/event-stream"));
    }

    // 2d) App API: GET /v2/telemetry/live — endpoint exists; allow OK/429/403.
    let resp_telemetry_live = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/telemetry/live"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
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
        assert_eq!(resp_telemetry_live.status(), StatusCode::NOT_FOUND);
    }

    // 2e) App API: GET /v2/telemetry/propagation — endpoint exists; allow OK/429/403.
    let resp_telemetry_propagation = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/telemetry/propagation"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
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
        assert_eq!(resp_telemetry_propagation.status(), StatusCode::NOT_FOUND);
    }

    // 3) App API: GET /v2/webhooks — ensure route exists; allow OK or 429
    let resp_webhooks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v2/webhooks"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp_webhooks.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // 4) App API: GET /v2/assets/{definition_id}/holders — use percent-encoded '#'
    // in the definition id (bogus#wonderland) to ensure parsing is exercised.
    let resp_holders = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static(
                    "/v2/assets/bogus%23wonderland/holders?offset=0",
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

    // 5) App API: POST /v2/webhooks — create a webhook (write path)
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
                .uri(Uri::from_static("/v2/webhooks"))
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
