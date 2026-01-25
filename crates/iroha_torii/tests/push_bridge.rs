//! Push bridge endpoints (FCM/APNS) – feature/config gating and happy-path smoke.
#![cfg(all(feature = "app_api", feature = "push"))]

use std::sync::Arc;

use axum::{
    body::to_bytes,
    http::{Request, StatusCode},
};
use iroha_config::parameters::actual;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::ChainId;
use iroha_data_model::peer::PeerId;
use iroha_test_samples::ALICE_ID;
use iroha_torii::{OnlinePeersProvider, Torii};
use tower::ServiceExt as _; // for Router::oneshot

#[path = "fixtures.rs"]
mod fixtures;

fn build_torii(push: actual::Push) -> (Torii, axum::Router) {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.push = push;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    world.peers.mutate_vec(|peers| {
        let _ = peers.push(local_peer_id.clone());
    });
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(4).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events.clone(),
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let da_receipt_signer = cfg.common.key_pair.clone();

    #[cfg(feature = "telemetry")]
    let torii = {
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
            local_peer_id.clone(),
            ts,
            false,
        )
        .0;
        Torii::new(
            ChainId::from("test-chain"),
            kiso,
            cfg.torii.clone(),
            queue,
            events,
            LiveQueryStore::start_test(),
            kura,
            state,
            da_receipt_signer.clone(),
            OnlinePeersProvider::new(peers_rx),
            telemetry,
            true,
        )
    };

    #[cfg(not(feature = "telemetry"))]
    let torii = Torii::new(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        events,
        LiveQueryStore::start_test(),
        kura,
        state,
        da_receipt_signer,
        OnlinePeersProvider::new(peers_rx),
    );

    let router = torii.api_router_for_tests();
    (torii, router)
}

fn register_device_request() -> Request<axum::body::Body> {
    let account_id = ALICE_ID.to_string();
    Request::builder()
        .method("POST")
        .uri("/v1/notify/devices")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(format!(
            r#"{{"account_id":"{account_id}","platform":"FCM","token":"t0"}}"#
        )))
        .unwrap()
}

async fn status_and_body(
    router: axum::Router,
    req: Request<axum::body::Body>,
) -> (StatusCode, String) {
    let resp = router.oneshot(req).await.expect("request succeeds");
    let status = resp.status();
    let body_bytes = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let body = String::from_utf8_lossy(&body_bytes).to_string();
    (status, body)
}

#[tokio::test]
async fn push_registration_rejected_when_disabled() {
    let push_cfg = actual::Push {
        enabled: false,
        ..Default::default()
    };
    let (_torii, router) = build_torii(push_cfg);

    let (status, body) = status_and_body(router, register_device_request()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "body: {body}");
}

#[tokio::test]
async fn push_registration_rejected_without_credentials() {
    let push_cfg = actual::Push {
        enabled: true,
        fcm_api_key: None,
        ..Default::default()
    };
    let (_torii, router) = build_torii(push_cfg);

    let (status, body) = status_and_body(router, register_device_request()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "body: {body}");
}

#[tokio::test]
async fn push_registration_succeeds_with_credentials() {
    let push_cfg = actual::Push {
        enabled: true,
        fcm_api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let (torii, router) = build_torii(push_cfg);

    let (status, body) = status_and_body(router, register_device_request()).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let devices = torii
        .push_bridge_for_tests()
        .expect("push enabled")
        .device_count();
    assert_eq!(devices, 1);
}
