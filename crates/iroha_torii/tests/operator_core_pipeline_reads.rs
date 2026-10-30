//! Router-level authentication regressions for node-local core and pipeline reads.
#[path = "common/norito_rpc_harness.rs"]
mod norito_rpc_harness;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
};
use http_body_util::BodyExt as _;
use iroha_data_model::NetworkId;
use iroha_torii_shared::ErrorEnvelope;
use norito_rpc_harness::NoritoRpcHarness;
use tower::ServiceExt as _;
fn request(uri: Uri, headers: HeaderMap) -> Request<Body> {
    request_with_body(uri, headers, Body::empty())
}
fn request_with_body(uri: Uri, headers: HeaderMap, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(body)
        .expect("operator read request");
    *request.headers_mut() = headers;
    request
        .extensions_mut()
        .insert(norito_rpc_harness::loopback_connect_info());
    request
}
fn foreign_network_id() -> NetworkId {
    "hash:0000000000000000000000000000000000000000000000000000000000000003#E54C"
        .parse()
        .expect("canonical foreign NetworkId")
}
#[tokio::test]
async fn core_and_pipeline_reads_reject_missing_or_inexact_operator_auth_before_handlers() {
    let harness = NoritoRpcHarness::new(|_| {});
    for path in [
        "/v1/peers",
        "/v1/time/status",
        "/v1/pipeline/preflight",
        "/v1/policy",
        "/v1/proofs/retention",
        "/v1/pipeline/recovery/0",
    ] {
        let response = harness
            .app
            .clone()
            .oneshot(request(path.parse().expect("route URI"), HeaderMap::new()))
            .await
            .expect("missing-signature response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    let peers_uri: Uri = "/v1/peers".parse().expect("peers URI");
    let wrong_network_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &foreign_network_id(),
        &Method::GET,
        &peers_uri,
        &[],
    )
    .expect("foreign-network signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(peers_uri.clone(), wrong_network_headers))
        .await
        .expect("wrong-network response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let method_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::POST,
        &peers_uri,
        &[],
    )
    .expect("method-bound signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(peers_uri.clone(), method_headers))
        .await
        .expect("method-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let empty_body_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &peers_uri,
        &[],
    )
    .expect("empty-body signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request_with_body(
            peers_uri,
            empty_body_headers,
            Body::from("{}"),
        ))
        .await
        .expect("body-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let signed_path: Uri = "/v1/pipeline/preflight"
        .parse()
        .expect("signed preflight URI");
    let path_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &signed_path,
        &[],
    )
    .expect("path-bound signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(
            "/v1/policy".parse().expect("policy URI"),
            path_headers,
        ))
        .await
        .expect("path-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let retention_uri: Uri = "/v1/proofs/retention".parse().expect("retention URI");
    let query_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &retention_uri,
        &[],
    )
    .expect("query-bound signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(
            "/v1/proofs/retention?detail=full"
                .parse()
                .expect("retention query URI"),
            query_headers,
        ))
        .await
        .expect("query-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    harness.shutdown().await;
}
#[tokio::test]
async fn core_operator_read_rejects_an_exact_signature_replay() {
    let harness = NoritoRpcHarness::new(|_| {});
    let uri: Uri = "/v1/time/status".parse().expect("time status URI");
    let headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &uri,
        &[],
    )
    .expect("time status signature fixture");
    let first = harness
        .app
        .clone()
        .oneshot(request(uri.clone(), headers.clone()))
        .await
        .expect("first signed response");
    assert_eq!(first.status(), StatusCode::OK);
    let replay = harness
        .app
        .clone()
        .oneshot(request(uri, headers))
        .await
        .expect("replayed response");
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    harness.shutdown().await;
}
#[tokio::test]
async fn operator_reads_still_require_the_listener_api_token_when_tokens_are_required() {
    const API_TOKEN: &str = "operator-read-listener-token-0000";
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.require_api_token = true;
        cfg.torii.api_tokens = vec![API_TOKEN.to_owned()].into();
    });
    for path in [
        "/v1/configuration",
        "/v1/peers",
        "/v1/time/status",
        "/v1/pipeline/preflight",
        "/v1/policy",
        "/v1/proofs/retention",
        "/v1/pipeline/recovery/0",
    ] {
        let uri: Uri = path.parse().expect("operator route URI");
        let operator_headers = || {
            iroha_torii::operator_signed_request_headers(
                &harness.cfg.common.key_pair,
                &harness.network_id,
                &Method::GET,
                &uri,
                &[],
            )
            .expect("operator signature fixture")
        };
        // The signature authenticates the operator; the listener token admits the request.
        let response = harness
            .app
            .clone()
            .oneshot(request(uri.clone(), operator_headers()))
            .await
            .expect("token-free operator route response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect token rejection body")
            .to_bytes();
        let envelope: ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode JSON error envelope");
        assert_eq!(envelope.code(), "api_token_required", "{path}");
        let mut headers = operator_headers();
        headers.insert("x-api-token", HeaderValue::from_static(API_TOKEN));
        let response = harness
            .app
            .clone()
            .oneshot(request(uri, headers))
            .await
            .expect("operator route response");
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert_ne!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
    harness.shutdown().await;
}
