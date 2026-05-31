#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
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
use iroha_data_model::peer::PeerId;
use iroha_data_model::{ChainId, Registrable};
use iroha_data_model::{
    account::{Account, AccountId},
    domain::{Domain, DomainId},
};
use iroha_torii::{OnlinePeersProvider, Torii};
use tower::ServiceExt as _; // for Router::oneshot

#[path = "fixtures.rs"]
mod fixtures;

fn world_with_account(account_id: &AccountId) -> World {
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(account_id);
    let account = Account::new(account_id.clone()).build(account_id);
    World::with([domain], [account], [])
}

fn push_identity(seed: u8) -> (iroha_crypto::KeyPair, AccountId) {
    let key_pair =
        iroha_crypto::KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519);
    let account_id = AccountId::new(key_pair.public_key().clone());
    (key_pair, account_id)
}

fn push_config() -> actual::Push {
    actual::Push {
        enabled: true,
        fcm_project_id: Some("project".to_string()),
        fcm_service_account_path: Some(std::path::PathBuf::from("/tmp/service-account.json")),
        ..Default::default()
    }
}

fn build_torii(
    push: actual::Push,
    account_id: &AccountId,
) -> (
    Torii,
    axum::Router,
    iroha_torii::test_utils::TestDataDirGuard,
) {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    cfg.torii.data_dir = data_dir.path().to_path_buf();
    cfg.torii.push = push;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = world_with_account(account_id);
    fixtures::seed_peer(&mut world, local_peer_id.clone());
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
    (torii, router, data_dir)
}

fn register_device_request(
    account_id: &AccountId,
    key_pair: &iroha_crypto::KeyPair,
    token: &str,
) -> Request<axum::body::Body> {
    let body = format!(
        r#"{{"account_id":"{}","platform":"FCM","token":"{}"}}"#,
        account_id, token
    );
    let request = Request::builder()
        .method("POST")
        .uri("/v1/notify/devices")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.clone()))
        .unwrap();
    fixtures::app_signed_request(account_id, key_pair, request, body.as_bytes())
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
    let (key_pair, account_id) = push_identity(11);
    let push_cfg = actual::Push {
        enabled: false,
        ..Default::default()
    };
    let (_torii, router, _data_dir) = build_torii(push_cfg, &account_id);

    let (status, body) = status_and_body(
        router,
        register_device_request(&account_id, &key_pair, "t0"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "body: {body}");
}

#[tokio::test]
async fn push_registration_rejected_without_credentials() {
    let (key_pair, account_id) = push_identity(12);
    let push_cfg = actual::Push {
        enabled: true,
        ..Default::default()
    };
    let (_torii, router, _data_dir) = build_torii(push_cfg, &account_id);

    let (status, body) = status_and_body(
        router,
        register_device_request(&account_id, &key_pair, "t0"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "body: {body}");
}

#[tokio::test]
async fn push_registration_succeeds_with_credentials() {
    let (key_pair, account_id) = push_identity(13);
    let (torii, router, _data_dir) = build_torii(push_config(), &account_id);

    let (status, body) = status_and_body(
        router,
        register_device_request(&account_id, &key_pair, "t0"),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let devices = torii
        .push_bridge_for_tests()
        .expect("push enabled")
        .device_count();
    assert_eq!(devices, 1);
}
