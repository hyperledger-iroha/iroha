#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify that Connect routes are stable while runtime configuration controls availability.

use std::sync::Arc;

use axum::http::{Request, StatusCode, Uri};
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, queue::Queue,
    state::State,
};
use nonzero_ext::nonzero;
use tower::ServiceExt;

fn request_with_loopback_connect_info(
    request: Request<axum::body::Body>,
) -> Request<axum::body::Body> {
    let mut request = request;
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    request
}

fn spawn_test_server(listener: tokio::net::TcpListener, app: axum::Router) {
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
}

fn checked_connect_key_fixture() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_random().expect("generate checked connect fixture keypair")
}

#[test]
fn connect_config_fixture_uses_checked_key_generation() {
    let key_pair = checked_connect_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture connect public key has a valid algorithm");

    assert_eq!(algorithm, iroha_crypto::Algorithm::Ed25519);
}

fn minimal_actual_config(connect_enabled: bool) -> iroha_config::parameters::actual::Root {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.connect.enabled = connect_enabled;
    cfg
}

fn build_torii(cfg: &iroha_config::parameters::actual::Root) -> iroha_torii::Torii {
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let (_mh, time_source) =
        iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: nonzero!(1usize),
        capacity_per_user: nonzero!(1usize),
        transaction_time_to_live: core::time::Duration::from_secs(1),
        ..Default::default()
    };
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = (peers_tx, time_source);

    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    iroha_torii::Torii::new_with_handle(
        cfg.common.chain.clone(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        Kura::blank_kura_for_testing(),
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        telemetry,
    )
}

#[tokio::test]
async fn connect_endpoints_report_typed_unavailability_when_disabled() {
    let cfg = minimal_actual_config(false);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    // The WebSocket route remains mounted but rejects the upgrade while disabled.
    let resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/ws?sid=AA&role=app"))
                .body(axum::body::Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    // Ordinary REST routes return the shared typed error envelope.
    let resp = app
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .header(axum::http::header::ACCEPT, "application/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("connect disabled response body")
        .to_bytes();
    let error: norito::json::Value =
        norito::json::from_slice(&body).expect("typed Connect disabled error envelope");
    assert_eq!(
        error.get("code").and_then(norito::json::Value::as_str),
        Some("connect_disabled")
    );
}

#[tokio::test]
async fn connect_status_present_when_enabled() {
    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("status should be valid JSON");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    assert_eq!(
        p2p_rebroadcasts_total, 0,
        "fresh status snapshot should start with zero rebroadcasts"
    );
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[tokio::test]
async fn connect_status_forces_unknown_relay_strategy_to_local_only() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "bogus_strategy";
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("status should be valid JSON");
    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    assert_eq!(relay_strategy, "local_only");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[tokio::test]
async fn connect_status_normalizes_relay_strategy_aliases() {
    for (raw_strategy, expected) in [
        ("local_only", "local_only"),
        ("local-only", "local_only"),
        ("local", "local_only"),
        ("  BROADCAST  ", "broadcast"),
    ] {
        let mut cfg = minimal_actual_config(true);
        cfg.torii.connect.relay_strategy = raw_strategy;
        let torii = build_torii(&cfg);
        let app = torii.api_router_for_tests();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_strategy = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_strategy"))
            .and_then(norito::json::Value::as_str)
            .expect("connect status should include policy.relay_strategy");
        let p2p_rebroadcasts_total = payload
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .expect("connect status should include p2p_rebroadcasts_total");
        let p2p_rebroadcast_skipped_total = payload
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .expect("connect status should include p2p_rebroadcast_skipped_total");
        let relay_effective_strategy = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .expect("connect status should include policy.relay_effective_strategy");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        assert_eq!(
            relay_strategy, expected,
            "raw relay strategy {raw_strategy:?} should normalize"
        );
        assert_eq!(
            p2p_rebroadcasts_total, 0,
            "status-only probe should not rebroadcast p2p frames"
        );
        assert_eq!(p2p_rebroadcast_skipped_total, 0);
        assert_eq!(
            relay_effective_strategy, "local_only",
            "without a connected P2P network, status should report effective local-only relay"
        );
        assert!(!relay_p2p_attached);
    }
}

#[tokio::test]
async fn connect_status_reports_broadcast_effective_when_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
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

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "broadcast");
    assert_eq!(relay_effective_strategy, "broadcast");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_status_reports_local_only_when_relay_disabled_with_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_enabled = false;
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
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

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "broadcast");
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_status_reports_unknown_strategy_as_local_only_with_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "bogus_strategy";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
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

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "local_only");
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_session_delete_endpoint_removes_tokens() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x24u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let create_resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body.clone()))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present")
        .to_owned();
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management present")
        .to_owned();

    let delete_uri = format!("/v1/connect/session/{sid}");
    let missing_token_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_token_resp.status(), StatusCode::UNAUTHORIZED);

    let delete_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let delete_again = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_again.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn connect_session_status_requires_management_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x34u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let create_resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present");
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management present");

    let status_uri = format!("/v1/connect/status?sid={sid}");
    let missing_token_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(status_uri.as_str())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_token_resp.status(), StatusCode::UNAUTHORIZED);

    let status_resp = app
        .oneshot(
            Request::builder()
                .uri(status_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(status_resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("session status should be JSON");
    assert_eq!(payload.get("sid").and_then(|x| x.as_str()), Some(sid));
    assert_eq!(
        payload.get("app_attached").and_then(|x| x.as_bool()),
        Some(false)
    );
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_session_delete_rejects_ws_attach() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_session_delete_rejects_ws_attach: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    // Use a second router handle for in-process REST calls.
    let app2 = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x44u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let create_resp = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present");
    assert_eq!(sid, sid_fixed);
    let token_app = payload
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management");

    // Delete the session through REST and ensure it reports success.
    let delete_uri = format!("/v1/connect/session/{sid}");
    let delete_resp = app2
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.clone())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    // Attempt to attach over WS using the stale token; expect 401.
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    match tokio_tungstenite::connect_async(request).await {
        Ok(_) => panic!("ws handshake should fail after session deletion"),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }
        Err(err) => panic!("unexpected ws failure: {err:?}"),
    }
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_handshake_succeeds_when_enabled() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    // Build enabled config and Torii router
    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();
    // Serve on an ephemeral port
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_handshake_succeeds_when_enabled: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    // Create a session via in-process router call to obtain tokens and sid
    let app2 = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x52u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    // Attempt WS connect using the provided sid/token
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    let (_ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_accepts_protocol_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::header};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_accepts_protocol_token: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x62u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    let encoded = B64.encode(token_app.as_bytes());
    request.headers_mut().insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("iroha-connect.token.v1.{encoded}")
            .parse()
            .expect("protocol header"),
    );
    let (_ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_closes_on_role_direction_mismatch() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::{SinkExt, StreamExt};
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep, timeout};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_closes_on_role_direction_mismatch: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0x92u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Attach as app, then send a mismatched direction (WalletToApp).
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    let (mut ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let mismatch = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::WalletToApp,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
    };
    let payload = proto::encode_connect_frame_bare(&mismatch).expect("encode frame");
    ws.send(Message::Binary(payload.into()))
        .await
        .expect("send mismatch frame");

    let mut saw_connect_close = false;
    let mut saw_ws_close = false;
    for _ in 0..5 {
        let maybe_msg = timeout(Duration::from_millis(400), ws.next()).await;
        let Some(msg) = maybe_msg.unwrap_or(None) else {
            continue;
        };
        match msg {
            Ok(Message::Binary(bytes)) => {
                if let Ok(frame) = proto::decode_connect_frame_bare(&bytes) {
                    if let proto::FrameKind::Control(proto::ConnectControlV1::Close {
                        reason,
                        ..
                    }) = frame.kind
                    {
                        if reason == "connect_role_direction_mismatch" {
                            saw_connect_close = true;
                            break;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                saw_ws_close = true;
                break;
            }
            Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed) => {
                saw_ws_close = true;
                break;
            }
            _ => {}
        }
    }
    assert!(
        saw_connect_close || saw_ws_close,
        "expected websocket termination after role/direction mismatch"
    );

    // Poll status until mismatch closure is reflected.
    let mut mismatch_total = 0u64;
    let mut sessions_total = u64::MAX;
    for _ in 0..20 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&body).unwrap();
        mismatch_total = status_json
            .get("role_direction_mismatch_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        sessions_total = status_json
            .get("sessions_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(u64::MAX);
        if mismatch_total >= 1 && sessions_total == 0 {
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }
    assert!(mismatch_total >= 1, "mismatch counter should increment");
    assert_eq!(sessions_total, 0, "session should be terminated");
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_duplicate_frame_does_not_close_session() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::{SinkExt, StreamExt};
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_duplicate_frame_does_not_close_session: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let sid_fixed = B64.encode([0xA3u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");
    let token_wallet = v
        .get("token_wallet")
        .and_then(|x| x.as_str())
        .expect("token_wallet");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Connect app role.
    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    // Connect wallet role.
    let wallet_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=wallet");
    let mut wallet_req = wallet_url.into_client_request().expect("wallet ws request");
    wallet_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_wallet}")
            .parse()
            .expect("wallet authorization header"),
    );
    let (mut wallet_ws, wallet_resp) = tokio_tungstenite::connect_async(wallet_req)
        .await
        .expect("wallet ws handshake ok");
    assert_eq!(wallet_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 41 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    // Wallet should receive first frame.
    let first = timeout(Duration::from_millis(500), wallet_ws.next())
        .await
        .expect("wallet recv timeout")
        .expect("wallet recv closed")
        .expect("wallet recv error");
    let first_frame = match first {
        Message::Binary(bytes) => proto::decode_connect_frame_bare(&bytes).expect("decode first"),
        other => panic!("expected binary frame, got {other:?}"),
    };
    assert_eq!(first_frame.seq, 1);

    // Send duplicate seq=1; dedupe should drop it and keep session alive.
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode duplicate")
                .into(),
        ))
        .await
        .expect("send duplicate seq1");
    assert!(
        timeout(Duration::from_millis(200), wallet_ws.next())
            .await
            .is_err(),
        "duplicate frame should not be delivered to wallet"
    );
    assert!(
        timeout(Duration::from_millis(200), app_ws.next())
            .await
            .is_err(),
        "duplicate frame should not close app websocket"
    );

    let seq2 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 2,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 42 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq2)
                .expect("encode seq2")
                .into(),
        ))
        .await
        .expect("send seq2");
    let second = timeout(Duration::from_millis(500), wallet_ws.next())
        .await
        .expect("wallet recv seq2 timeout")
        .expect("wallet recv seq2 closed")
        .expect("wallet recv seq2 error");
    let second_frame = match second {
        Message::Binary(bytes) => proto::decode_connect_frame_bare(&bytes).expect("decode second"),
        other => panic!("expected binary frame, got {other:?}"),
    };
    assert_eq!(second_frame.seq, 2);

    let status = app2
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body = http_body_util::BodyExt::collect(status.into_body())
        .await
        .unwrap()
        .to_bytes();
    let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
    let dedupe_drops = status_json
        .get("dedupe_drops_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let sequence_violation_closes = status_json
        .get("sequence_violation_closes_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    assert!(dedupe_drops >= 1, "expected duplicate drop to be counted");
    assert_eq!(
        sequence_violation_closes, 0,
        "duplicate frame must not trigger sequence-violation close"
    );
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_broadcast_relay_updates_p2p_rebroadcast_counter() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    cfg.torii.connect.p2p_ttl_hops = 1;
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_broadcast_relay_updates_p2p_rebroadcast_counter: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let sid_fixed = B64.encode([0xB4u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait until async bus attachment reports active P2P relay wiring.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 7 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts >= 1 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert!(rebroadcasts >= 1, "expected at least one p2p rebroadcast");
    assert_eq!(
        skipped, 0,
        "p2p attached relay should not count skipped sends"
    );
    assert_eq!(relay_effective_strategy, "broadcast");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_broadcast_without_p2p_increments_skipped_rebroadcast_counter() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    cfg.torii.connect.p2p_ttl_hops = 1;
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "skipping connect_ws_broadcast_without_p2p_increments_skipped_rebroadcast_counter: {err}"
            );
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let sid_fixed = B64.encode([0xC5u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 8 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    let mut relay_p2p_attached = true;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(true);
        if skipped >= 1 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert!(
        skipped >= 1,
        "expected missing-p2p rebroadcast skips to be counted"
    );
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_local_only_with_p2p_does_not_rebroadcast() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "local_only";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_local_only_with_p2p_does_not_rebroadcast: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let sid_fixed = B64.encode([0xD6u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait for async P2P bus attachment before sending frames.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 9 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts > 0 || skipped > 0 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert_eq!(skipped, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_relay_disabled_with_p2p_does_not_rebroadcast() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_enabled = false;
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_relay_disabled_with_p2p_does_not_rebroadcast: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let sid_fixed = B64.encode([0xE7u8; 32]);
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_fixed.clone())),
        ("node", Option::<String>::None),
    ]))
    .expect("json serialization");
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait for async P2P bus attachment before sending frames.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 10 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts > 0 || skipped > 0 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert_eq!(skipped, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_rejects_query_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_rejects_query_token: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let sid = B64.encode([0x72u8; 32]);
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app&token=deadbeef");
    let err = tokio_tungstenite::connect_async(&url)
        .await
        .expect_err("ws handshake should reject query token");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_handshake_fails_when_disabled() {
    use tokio::net::TcpListener;
    // Build disabled config and Torii router
    let cfg = minimal_actual_config(false);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();
    // Serve
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_handshake_fails_when_disabled: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);
    // Attempt WS connect directly; expect failure
    let url = format!(
        "ws://{}/v1/connect/ws?sid={}&role=app",
        addr, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
    let err = tokio_tungstenite::connect_async(&url)
        .await
        .expect_err("ws handshake should fail when connect disabled");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(response) => response.status(),
        other => panic!("unexpected WebSocket error: {other:?}"),
    };
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
