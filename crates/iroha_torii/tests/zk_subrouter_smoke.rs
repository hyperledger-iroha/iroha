#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test that ZK endpoints (verify, attachments) are exposed via the merged sub-router.
#![cfg(feature = "app_api")]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use axum::http::Request;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{
    Registrable, ValidationFail,
    account::{Account, AccountId},
    domain::{Domain, DomainId},
    peer::PeerId,
};
#[cfg(feature = "telemetry")]
use iroha_primitives::time::TimeSource;
use iroha_torii::Torii;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn attachments_smoke_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("attachments smoke lock")
}

fn request_with_headers(
    method: &str,
    uri: &str,
    headers: &axum::http::HeaderMap,
    body: &[u8],
) -> Request<axum::body::Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder.body(axum::body::Body::from(body.to_vec())).unwrap()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_verify_and_attachments_endpoints_exposed() {
    let _guard = attachments_smoke_lock();
    // Minimal Torii setup (no telemetry requirement for these endpoints)
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.zk_attachments_enabled = true;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let account_id = AccountId::new(cfg.common.key_pair.public_key().clone());
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let mut world = World::with([domain], [account], []);
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

    // Optional telemetry used elsewhere; not needed here, but keep setup consistent
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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

    // POST /v1/zk/verify with minimal JSON; accept OK or 429
    let resp = app
        .clone()
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/zk/verify")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // GET /v1/zk/attachments (signed; empty list by default); accept OK or 429
    let request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .uri("/v1/zk/attachments")
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let resp = app.clone().oneshot(request).await.unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // GET /v1/zk/attachments/{id} with a placeholder id; signed request accepts 404 or 429.
    let request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .uri("/v1/zk/attachments/placeholder-id")
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let resp = app.oneshot(request).await.unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST | StatusCode::TOO_MANY_REQUESTS
    ));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_attachments_endpoints_disabled_by_default() {
    let _guard = attachments_smoke_lock();
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
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

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
    for request in [
        Request::builder()
            .uri("/v1/zk/attachments")
            .body(axum::body::Body::empty())
            .unwrap(),
        Request::builder()
            .uri("/v1/zk/attachments/count")
            .body(axum::body::Body::empty())
            .unwrap(),
        Request::builder()
            .uri(format!("/v1/zk/attachments/{}", "0".repeat(64)))
            .body(axum::body::Body::empty())
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!("/v1/zk/attachments/{}", "0".repeat(64)))
            .body(axum::body::Body::empty())
            .unwrap(),
    ] {
        let resp = app.clone().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_attachments_count_and_delete_endpoints_exposed_for_signed_requests() {
    let _guard = attachments_smoke_lock();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.zk_attachments_enabled = true;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let account_id = AccountId::new(cfg.common.key_pair.public_key().clone());
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let mut world = World::with([domain], [account], []);
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

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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

    let count_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .uri("/v1/zk/attachments/count")
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let count_resp = app.clone().oneshot(count_request).await.unwrap();
    if count_resp.status() == StatusCode::OK {
        let body = count_resp.into_body().collect().await.unwrap().to_bytes();
        let json: norito::json::Value = norito::json::from_slice(&body).expect("json count body");
        assert_eq!(json.get("count").and_then(|value| value.as_u64()), Some(0));
    } else {
        assert_eq!(count_resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    let missing_id = "0".repeat(64);
    let delete_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .method("DELETE")
            .uri(format!("/v1/zk/attachments/{missing_id}"))
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let delete_resp = app.oneshot(delete_request).await.unwrap();
    assert!(matches!(
        delete_resp.status(),
        StatusCode::NOT_FOUND | StatusCode::TOO_MANY_REQUESTS
    ));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_attachments_create_roundtrip_and_replay_rejected_for_signed_requests() {
    let _guard = attachments_smoke_lock();
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.zk_attachments_enabled = true;
    cfg.torii.attachments_sanitizer_mode =
        iroha_config::parameters::actual::AttachmentSanitizerMode::InProcess;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let account_id = AccountId::new(cfg.common.key_pair.public_key().clone());
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let mut world = World::with([domain], [account], []);
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

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
    let body = br#"{"backend":"demo","proof":{"bytes":[7,8,9]}}"#;
    let signed_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .method("POST")
            .uri("/v1/zk/attachments")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body.to_vec()))
            .unwrap(),
        body,
    );
    let signed_headers = signed_request.headers().clone();

    let create_resp = app
        .clone()
        .oneshot(request_with_headers(
            "POST",
            "/v1/zk/attachments",
            &signed_headers,
            body,
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&create_body).expect("json meta");
    let id = meta
        .get("id")
        .and_then(|value| value.as_str())
        .expect("attachment id")
        .to_owned();
    assert_eq!(
        meta.get("content_type").and_then(|value| value.as_str()),
        Some("application/json")
    );

    let replay_resp = app
        .clone()
        .oneshot(request_with_headers(
            "POST",
            "/v1/zk/attachments",
            &signed_headers,
            body,
        ))
        .await
        .unwrap();
    assert_eq!(replay_resp.status(), StatusCode::FORBIDDEN);
    let replay_body = replay_resp.into_body().collect().await.unwrap().to_bytes();
    let replay_validation: ValidationFail =
        norito::decode_from_bytes(&replay_body).expect("validation fail payload");
    assert!(matches!(
        replay_validation,
        ValidationFail::NotPermitted(ref message) if message.contains("nonce already used")
    ));

    let get_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .uri(format!("/v1/zk/attachments/{id}"))
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let get_resp = app.clone().oneshot(get_request).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    assert!(
        get_resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json")),
        "unexpected content type: {:?}",
        get_resp.headers().get(axum::http::header::CONTENT_TYPE)
    );
    let get_body = get_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        String::from_utf8(get_body.to_vec()).unwrap(),
        std::str::from_utf8(body).unwrap()
    );

    let delete_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .method("DELETE")
            .uri(format!("/v1/zk/attachments/{id}"))
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let delete_resp = app.clone().oneshot(delete_request).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let get_after_delete_request = fixtures::app_signed_request(
        &account_id,
        &cfg.common.key_pair,
        Request::builder()
            .uri(format!("/v1/zk/attachments/{id}"))
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let get_after_delete_resp = app.oneshot(get_after_delete_request).await.unwrap();
    assert_eq!(get_after_delete_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_attachments_endpoints_require_signed_headers_when_enabled() {
    let _guard = attachments_smoke_lock();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.zk_attachments_enabled = true;
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

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
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

    for request in [
        Request::builder()
            .uri("/v1/zk/attachments")
            .body(axum::body::Body::empty())
            .unwrap(),
        Request::builder()
            .uri("/v1/zk/attachments/count")
            .body(axum::body::Body::empty())
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!("/v1/zk/attachments/{}", "0".repeat(64)))
            .body(axum::body::Body::empty())
            .unwrap(),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let validation: ValidationFail =
            norito::decode_from_bytes(&body).expect("validation fail payload");
        assert!(matches!(
            validation,
            ValidationFail::NotPermitted(ref message)
                if message.contains("signed account headers are required")
        ));
    }
}
