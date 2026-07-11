//! Router smoke tests for the first-release offline API.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::extract::connect_info::ConnectInfo;
use axum::http::{
    HeaderValue, Method, Request, StatusCode,
    header::{ACCEPT, CONTENT_TYPE},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, state::State,
};
use iroha_data_model::{ChainId, peer::PeerId};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}

fn connect_info() -> ConnectInfo<std::net::SocketAddr> {
    ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}

#[tokio::test]
async fn offline_router_exposes_only_the_final_first_release_contract() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let cfg = mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let da_receipt_signer = cfg.common.key_pair.clone();

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

    let missing_readiness_query = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/readiness")
                .header(ACCEPT, "application/json")
                .extension(connect_info())
                .body(axum::body::Body::empty())
                .expect("readiness request"),
        )
        .await
        .expect("readiness response");
    assert_eq!(missing_readiness_query.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        missing_readiness_query.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    let missing_query_body = missing_readiness_query
        .into_body()
        .collect()
        .await
        .expect("collect missing-query error")
        .to_bytes();
    let missing_query_error: iroha_torii_shared::ErrorEnvelope =
        norito::json::from_slice(&missing_query_body).expect("decode missing-query error");
    assert_eq!(missing_query_error.code(), "request_query_invalid");

    let readiness = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/readiness?asset_definition_id=xor%23wonderland")
                .header(ACCEPT, "application/json")
                .extension(connect_info())
                .body(axum::body::Body::empty())
                .expect("readiness request"),
        )
        .await
        .expect("readiness response");
    assert_eq!(
        readiness.status(),
        StatusCode::NOT_FOUND,
        "the route is mounted and rejects an unregistered requested asset"
    );

    for path in ["/v1/offline/top-up", "/v1/offline/redeem"] {
        let json = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(axum::body::Body::from("{}"))
                    .expect("typed JSON request"),
            )
            .await
            .expect("typed JSON response");
        assert_eq!(
            json.status(),
            StatusCode::BAD_REQUEST,
            "{path} must decode the body directly as its typed request"
        );

        let norito = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, "application/x-norito")
                    .header(ACCEPT, "application/x-norito")
                    .extension(connect_info())
                    .body(axum::body::Body::from("not-a-norito-archive"))
                    .expect("typed Norito request"),
            )
            .await
            .expect("typed Norito response");
        assert_eq!(
            norito.status(),
            StatusCode::BAD_REQUEST,
            "{path} must decode a direct typed Norito archive"
        );

        let missing_content_type = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .extension(connect_info())
                    .body(axum::body::Body::from("{}"))
                    .expect("request without content type"),
            )
            .await
            .expect("missing content type response");
        assert_eq!(
            missing_content_type.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }

    let invalid_operation_id = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/operations/not-hex")
                .header(ACCEPT, "application/json")
                .extension(connect_info())
                .body(axum::body::Body::empty())
                .expect("operation request"),
        )
        .await
        .expect("operation response");
    assert_eq!(invalid_operation_id.status(), StatusCode::BAD_REQUEST);

    let missing_operation = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/offline/operations/{}", "11".repeat(32)))
                .header(ACCEPT, "application/json")
                .extension(connect_info())
                .body(axum::body::Body::empty())
                .expect("operation request"),
        )
        .await
        .expect("operation response");
    assert_eq!(missing_operation.status(), StatusCode::NOT_FOUND);

    for (method, path) in [
        (Method::GET, "/v1/offline/v2/readiness"),
        (Method::POST, "/v1/offline/v2/kagemusha/topup"),
        (Method::POST, "/v1/offline/v2/notes/redeem"),
        (Method::POST, "/v1/offline/keys/refill"),
        (Method::POST, "/v1/offline/notes/issue"),
        (Method::POST, "/v1/offline/notes/redeem"),
        (Method::POST, "/v1/offline/cash/load"),
        (Method::POST, "/v1/offline/cash/redeem"),
        (Method::POST, "/v1/offline/audit"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .extension(connect_info())
                    .body(axum::body::Body::from("{}"))
                    .expect("retired route request"),
            )
            .await
            .expect("retired route response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "retired method/path pair must be unregistered: {method} {path}"
        );
    }
}
