//! Router smoke tests for the first-release offline API.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]

use std::{convert::Infallible, sync::Arc};

use axum::body::{Body, Bytes};
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
    const OFFLINE_COMMAND_BODY_LIMIT: u64 = 2_200_000;

    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = mk_minimal_root_cfg();
    cfg.torii.max_content_len = OFFLINE_COMMAND_BODY_LIMIT.into();
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
        Some(&HeaderValue::from_static("application/json; charset=utf-8"))
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

    for query in [
        "asset_definition_id=first&asset_definition_id=second",
        "asset_definition_id=first&asset_definition_id=first",
        "asset_definition_id=first&asset%5fdefinition%5fid=second",
    ] {
        let duplicate = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/offline/readiness?{query}"))
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(Body::empty())
                    .expect("duplicate readiness query request"),
            )
            .await
            .expect("duplicate readiness query response");
        assert_eq!(
            duplicate.status(),
            StatusCode::BAD_REQUEST,
            "a repeated asset_definition_id must be rejected before the readiness handler: {query}"
        );
        let duplicate_body = duplicate
            .into_body()
            .collect()
            .await
            .expect("collect duplicate-query error")
            .to_bytes();
        let duplicate_error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&duplicate_body).expect("decode duplicate-query error");
        assert_eq!(
            duplicate_error.code(),
            "request_query_invalid",
            "query={query}"
        );
        assert!(
            duplicate_error.message().contains("duplicate field"),
            "query={query}, error={duplicate_error:?}"
        );
    }

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

        for (content_type, expected_code) in [
            ("application/json", "request_json_invalid"),
            ("application/x-norito", "request_norito_invalid"),
        ] {
            let empty = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri(path)
                        .header(CONTENT_TYPE, content_type)
                        .header(ACCEPT, "application/json")
                        .extension(connect_info())
                        .body(axum::body::Body::empty())
                        .expect("empty typed request"),
                )
                .await
                .expect("empty typed response");
            assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
            let body = empty
                .into_body()
                .collect()
                .await
                .expect("collect empty-body response")
                .to_bytes();
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode empty-body error");
            assert_eq!(error.code(), expected_code, "path={path}");
        }
    }

    for path in ["/v1/offline/top-up", "/v1/offline/redeem"] {
        let oversized_len = usize::try_from(OFFLINE_COMMAND_BODY_LIMIT)
            .expect("test limit fits usize")
            .checked_add(1)
            .expect("test limit can be incremented");
        for (content_type, expected_code) in [
            (None, "request_content_type_missing"),
            (Some("text/plain"), "request_content_type_unsupported"),
        ] {
            let mut request = Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(ACCEPT, "application/json")
                .extension(connect_info());
            if let Some(content_type) = content_type {
                request = request.header(CONTENT_TYPE, content_type);
            }
            let response = app
                .clone()
                .oneshot(
                    request
                        .body(Body::from(vec![b' '; oversized_len]))
                        .expect("oversized request with rejected content type"),
                )
                .await
                .expect("content-type rejection response");
            assert_eq!(
                response.status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "content-type validation must precede body collection for path={path}"
            );
            let body = response
                .into_body()
                .collect()
                .await
                .expect("collect content-type rejection")
                .to_bytes();
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode typed content-type rejection");
            assert_eq!(error.code(), expected_code, "path={path}");
        }

        let above_axum_default = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(Body::from(vec![
                        b' ';
                        usize::try_from(OFFLINE_COMMAND_BODY_LIMIT)
                            .expect("test limit fits usize")
                    ]))
                    .expect("large request within the configured limit"),
            )
            .await
            .expect("large in-limit response");
        assert_eq!(
            above_axum_default.status(),
            StatusCode::BAD_REQUEST,
            "{path} must use Torii's configured limit rather than Axum's 2 MiB default"
        );
        let body = above_axum_default
            .into_body()
            .collect()
            .await
            .expect("collect large in-limit response")
            .to_bytes();
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode large in-limit error");
        assert_eq!(error.code(), "request_json_invalid", "path={path}");

        let body_chunks = futures::stream::iter([
            Ok::<_, Infallible>(Bytes::from(vec![
                b' ';
                usize::try_from(OFFLINE_COMMAND_BODY_LIMIT)
                    .expect("test limit fits usize")
            ])),
            Ok(Bytes::from_static(b" ")),
        ]);
        let above_configured_limit = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(Body::from_stream(body_chunks))
                    .expect("request above the configured limit"),
            )
            .await
            .expect("over-limit response");
        assert_eq!(
            above_configured_limit.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "path={path}"
        );
        let body = above_configured_limit
            .into_body()
            .collect()
            .await
            .expect("collect over-limit response")
            .to_bytes();
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode typed over-limit error");
        assert_eq!(error.code(), "request_payload_too_large", "path={path}");
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
        (Method::GET, "/v1/offline/v2/readiness/"),
        (Method::GET, "/v1/offline//v2/readiness"),
        (Method::GET, "/v1/OFFLINE/v2/readiness"),
        (Method::GET, "/v1/offline/%76%32/readiness"),
        (Method::GET, "/v1/offline/v2%2Freadiness"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json")
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
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect retired-route response")
            .to_bytes();
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode retired-route error");
        assert_eq!(
            error.code(),
            "route_not_found",
            "normalization or parameter capture must not resolve a retired route: {method} {path}"
        );
    }

    for (method, path) in [
        (Method::POST, "/v1/offline/readiness".to_owned()),
        (Method::GET, "/v1/offline/top-up".to_owned()),
        (Method::GET, "/v1/offline/redeem".to_owned()),
        (
            Method::POST,
            format!("/v1/offline/operations/{}", "11".repeat(32)),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri(path.as_str())
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(axum::body::Body::empty())
                    .expect("wrong-method request"),
            )
            .await
            .expect("wrong-method response");
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "a mounted path under the wrong method must report 405: {method} {path}"
        );
        assert!(
            response.headers().contains_key(axum::http::header::ALLOW),
            "405 must advertise the allowed method: {method} {path}"
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect wrong-method response")
            .to_bytes();
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode wrong-method error");
        assert_eq!(error.code(), "method_not_allowed");
    }
}
