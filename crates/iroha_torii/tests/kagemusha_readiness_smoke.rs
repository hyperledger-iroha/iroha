//! Router smoke tests for the first-release Kagemusha API.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]
use axum::body::{Body, Bytes};
use axum::extract::connect_info::ConnectInfo;
use axum::http::{
    HeaderValue, Method, Request, StatusCode,
    header::{ACCEPT, CONTENT_TYPE},
};
use iroha_core::prelude::World;
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
#[path = "fixtures.rs"]
mod fixtures;
fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}
fn connect_info() -> ConnectInfo<std::net::SocketAddr> {
    ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}
fn tracked_oversized_body(limit: u64) -> (Body, Arc<AtomicUsize>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let body_polls = Arc::clone(&polls);
    let oversized_len = usize::try_from(limit)
        .expect("test body limit fits usize")
        .checked_add(1)
        .expect("test body limit can be incremented");
    let body = Body::from_stream(futures::stream::once(async move {
        body_polls.fetch_add(1, Ordering::SeqCst);
        Ok::<_, Infallible>(Bytes::from(vec![b' '; oversized_len]))
    }));
    (body, polls)
}
#[tokio::test]
async fn kagemusha_router_exposes_only_the_final_first_release_contract() {
    const OFFLINE_COMMAND_BODY_LIMIT: u64 = 2_200_000;
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = mk_minimal_root_cfg();
    cfg.torii.max_content_len = OFFLINE_COMMAND_BODY_LIMIT.into();
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::default());
    let app = torii.router();
    let readiness = fixtures::request(
        &app,
        Request::builder()
            .uri("/v1/kagemusha/readiness")
            .header(ACCEPT, "application/json")
            .extension(connect_info())
            .body(axum::body::Body::empty())
            .expect("readiness request"),
    )
    .await
    .expect("readiness response");
    assert_eq!(readiness.status(), StatusCode::OK);
    assert_eq!(
        readiness.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json; charset=utf-8"))
    );
    let readiness_body =
        fixtures::response_body(readiness, "collect universal Kagemusha readiness").await;
    let capability: iroha_torii_shared::kagemusha_api::KagemushaReadinessV1 =
        norito::json::from_slice(&readiness_body).expect("decode universal Kagemusha readiness");
    assert_eq!(capability.kagemusha_handoff_capability, "kagemusha_handoff_v1");
    assert_eq!(capability.wire_version, 1);
    assert_eq!(capability.device_lifecycle_version, 1);
    assert!(capability.ready);
    let legacy_selector = fixtures::request(
        &app,
        Request::builder()
            .uri("/v1/kagemusha/readiness?asset_definition_id=xor%23wonderland")
            .header(ACCEPT, "application/json")
            .extension(connect_info())
            .body(axum::body::Body::empty())
            .expect("readiness request"),
    )
    .await
    .expect("legacy selector response");
    assert_eq!(legacy_selector.status(), StatusCode::BAD_REQUEST);
    let legacy_selector_body =
        fixtures::response_body(legacy_selector, "collect rejected legacy selector").await;
    let error: iroha_torii_shared::ErrorEnvelope =
        norito::json::from_slice(&legacy_selector_body).expect("decode selector rejection");
    assert_eq!(error.code(), "kagemusha_readiness_query_unsupported");
    for path in ["/v1/kagemusha/top-up", "/v1/kagemusha/redeem"] {
        enum RejectedHeaders {
            MissingIdempotency,
            DuplicateIdempotency,
            ForbiddenCanonicalAuth,
        }
        for (case, expected_status, expected_code) in [
            (
                RejectedHeaders::MissingIdempotency,
                StatusCode::BAD_REQUEST,
                "idempotency_key_missing",
            ),
            (
                RejectedHeaders::DuplicateIdempotency,
                StatusCode::BAD_REQUEST,
                "idempotency_key_invalid",
            ),
            (
                RejectedHeaders::ForbiddenCanonicalAuth,
                StatusCode::FORBIDDEN,
                "kagemusha_auth_header_unsupported",
            ),
        ] {
            let (body, body_polls) = tracked_oversized_body(OFFLINE_COMMAND_BODY_LIMIT);
            let mut request = Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/x-norito")
                .header(ACCEPT, "application/json")
                .extension(connect_info())
                .body(body)
                .expect("pre-body admission request");
            let idempotency_key: HeaderValue = "11"
                .repeat(32)
                .parse()
                .expect("canonical idempotency key fixture");
            match case {
                RejectedHeaders::MissingIdempotency => {}
                RejectedHeaders::DuplicateIdempotency => {
                    request
                        .headers_mut()
                        .append("idempotency-key", idempotency_key.clone());
                    request
                        .headers_mut()
                        .append("idempotency-key", idempotency_key);
                }
                RejectedHeaders::ForbiddenCanonicalAuth => {
                    request
                        .headers_mut()
                        .insert("idempotency-key", idempotency_key);
                    request.headers_mut().insert(
                        "x-iroha-account",
                        HeaderValue::from_static("untrusted-kagemusha-caller"),
                    );
                }
            }
            let response = fixtures::request(&app, request)
                .await
                .expect("pre-body admission response");
            assert_eq!(response.status(), expected_status, "path={path}");
            assert_eq!(
                body_polls.load(Ordering::SeqCst),
                0,
                "body stream was polled before rejecting headers for path={path}"
            );
            let body =
                fixtures::response_body(response, "collect pre-body admission response").await;
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode pre-body admission error");
            assert_eq!(error.code(), expected_code, "path={path}");
        }
        let json = fixtures::request(
            &app,
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "application/json")
                .header("idempotency-key", "11".repeat(32))
                .extension(connect_info())
                .body(axum::body::Body::from("{}"))
                .expect("typed JSON request"),
        )
        .await
        .expect("typed JSON response");
        assert_eq!(json.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let body = fixtures::response_body(json, "collect JSON media rejection").await;
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode JSON media rejection");
        assert_eq!(
            error.code(),
            "request_content_type_unsupported",
            "path={path}"
        );
        let norito = fixtures::request(
            &app,
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/x-norito")
                .header(ACCEPT, "application/x-norito")
                .header("idempotency-key", "11".repeat(32))
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
        let missing_content_type = fixtures::request(
            &app,
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
        for (content_type, expected_status, expected_code) in [
            (
                "application/problem+json",
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "request_content_type_unsupported",
            ),
            (
                "application/json",
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "request_content_type_unsupported",
            ),
            (
                "application/x-norito; profile=kagemusha",
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "request_content_type_unsupported",
            ),
            (
                "application/json; charset =utf-8",
                StatusCode::BAD_REQUEST,
                "request_content_type_invalid",
            ),
        ] {
            let response = fixtures::request(
                &app,
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, content_type)
                    .header(ACCEPT, "application/json")
                    .extension(connect_info())
                    .body(Body::from("{"))
                    .expect("request with adversarial content type"),
            )
            .await
            .expect("content-type classification response");
            assert_eq!(response.status(), expected_status, "path={path}");
            let body = fixtures::response_body(response, "collect content-type error").await;
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode content-type error");
            assert_eq!(error.code(), expected_code, "path={path}");
        }
        let json_with_charset = fixtures::request(
            &app,
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/json; charset=\"UTF-8\"")
                .header(ACCEPT, "application/json")
                .header("idempotency-key", "11".repeat(32))
                .extension(connect_info())
                .body(Body::from("{}"))
                .expect("JSON request with supported charset"),
        )
        .await
        .expect("supported charset response");
        assert_eq!(
            json_with_charset.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        let body =
            fixtures::response_body(json_with_charset, "collect supported-charset decode error")
                .await;
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode supported-charset error");
        assert_eq!(
            error.code(),
            "request_content_type_unsupported",
            "path={path}"
        );
        for (content_type, expected_code) in [("application/x-norito", "request_norito_invalid")] {
            let empty = fixtures::request(
                &app,
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(CONTENT_TYPE, content_type)
                    .header(ACCEPT, "application/json")
                    .header("idempotency-key", "11".repeat(32))
                    .extension(connect_info())
                    .body(axum::body::Body::empty())
                    .expect("empty typed request"),
            )
            .await
            .expect("empty typed response");
            assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
            let body = fixtures::response_body(empty, "collect empty-body response").await;
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode empty-body error");
            assert_eq!(error.code(), expected_code, "path={path}");
        }
    }
    for path in ["/v1/kagemusha/top-up", "/v1/kagemusha/redeem"] {
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
            let response = fixtures::request(
                &app,
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
            assert!(
                response.headers().get("x-iroha-reject-code").is_none(),
                "Kagemusha 415 is a transport-media rejection, not an exact application rejection: path={path}"
            );
            let body = fixtures::response_body(response, "collect content-type rejection").await;
            let error: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode typed content-type rejection");
            assert_eq!(error.code(), expected_code, "path={path}");
        }
        let above_axum_default = fixtures::request(
            &app,
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/x-norito")
                .header(ACCEPT, "application/json")
                .header("idempotency-key", "11".repeat(32))
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
        let body =
            fixtures::response_body(above_axum_default, "collect large in-limit response").await;
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode large in-limit error");
        assert_eq!(error.code(), "request_norito_invalid", "path={path}");
        let body_chunks = futures::stream::iter([
            Ok::<_, Infallible>(Bytes::from(vec![
                b' ';
                usize::try_from(OFFLINE_COMMAND_BODY_LIMIT)
                    .expect("test limit fits usize")
            ])),
            Ok(Bytes::from_static(b" ")),
        ]);
        let above_configured_limit = fixtures::request(
            &app,
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/x-norito")
                .header(ACCEPT, "application/json")
                .header("idempotency-key", "11".repeat(32))
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
        assert!(
            above_configured_limit
                .headers()
                .get("x-iroha-reject-code")
                .is_none(),
            "Kagemusha 413 is a body-extractor rejection, not an exact application rejection: path={path}"
        );
        let body =
            fixtures::response_body(above_configured_limit, "collect over-limit response").await;
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode typed over-limit error");
        assert_eq!(error.code(), "request_payload_too_large", "path={path}");
    }
    let invalid_operation_id = fixtures::request(
        &app,
        Request::builder()
            .uri("/v1/kagemusha/operations/not-hex")
            .header(ACCEPT, "application/json")
            .extension(connect_info())
            .body(axum::body::Body::empty())
            .expect("operation request"),
    )
    .await
    .expect("operation response");
    assert_eq!(invalid_operation_id.status(), StatusCode::BAD_REQUEST);
    let missing_operation = fixtures::request(
        &app,
        Request::builder()
            .uri(format!("/v1/kagemusha/operations/{}", "11".repeat(32)))
            .header(ACCEPT, "application/json")
            .extension(connect_info())
            .body(axum::body::Body::empty())
            .expect("operation request"),
    )
    .await
    .expect("operation response");
    assert_eq!(missing_operation.status(), StatusCode::NOT_FOUND);
    for (method, path) in [
        (Method::POST, "/v1/kagemusha/readiness".to_owned()),
        (Method::GET, "/v1/kagemusha/top-up".to_owned()),
        (Method::GET, "/v1/kagemusha/redeem".to_owned()),
        (
            Method::POST,
            format!("/v1/kagemusha/operations/{}", "11".repeat(32)),
        ),
    ] {
        let response = fixtures::request(
            &app,
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
        let body = fixtures::response_body(response, "collect wrong-method response").await;
        let error: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode wrong-method error");
        assert_eq!(error.code(), "method_not_allowed");
    }
    app.shutdown().await;
}
