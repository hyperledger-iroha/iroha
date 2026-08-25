//! Operator-authentication regressions for Kaigi relay diagnostic reads.
#![cfg(feature = "app_api")]
#[path = "common/norito_rpc_harness.rs"]
mod norito_rpc_harness;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
};
use iroha_data_model::NetworkId;
use iroha_torii_shared::route_catalog::{
    AdmissionPolicy, AuthenticationPolicy, RouteEffect, RouteProjections, application_api,
};
use norito_rpc_harness::NoritoRpcHarness;
use tower::ServiceExt as _;
fn request(uri: Uri, headers: HeaderMap) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("Kaigi operator request");
    *request.headers_mut() = headers;
    request
        .extensions_mut()
        .insert(norito_rpc_harness::loopback_connect_info());
    request
}
#[tokio::test]
async fn kaigi_relay_diagnostics_reject_legacy_or_precomputed_auth_headers() {
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.require_api_token = true;
        cfg.torii.api_tokens = vec!["legacy-kaigi-token".to_owned()];
    });
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-token",
        HeaderValue::from_static("legacy-kaigi-token"),
    );
    headers.insert(
        "x-iroha-operator-nonce",
        HeaderValue::from_static("precomputed"),
    );
    let response = harness
        .app
        .clone()
        .oneshot(request(
            "/v1/kaigi/relays".parse().expect("Kaigi list URI"),
            headers,
        ))
        .await
        .expect("legacy-auth response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
fn foreign_network_id() -> NetworkId {
    "0000000000000000000000000000000000000000000000000000000000000003"
        .parse()
        .expect("canonical foreign NetworkId")
}
#[test]
fn kaigi_relay_diagnostics_are_operator_only_and_classified_by_cost() {
    for route in [
        application_api::KAIGI_RELAYS_GET,
        application_api::KAIGI_RELAYS_BY_RELAY_ID_GET,
        application_api::KAIGI_RELAYS_HEALTH_GET,
    ] {
        assert_eq!(
            route.admission(),
            AdmissionPolicy::Operator,
            "{}",
            route.path()
        );
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature,
            "{}",
            route.path()
        );
        assert_eq!(
            route.projections(),
            RouteProjections::OPENAPI_AND_SDK,
            "{} must not invent an MCP projection",
            route.path()
        );
    }
    assert_eq!(
        application_api::KAIGI_RELAYS_GET.effect(),
        RouteEffect::ExpensiveCompute
    );
    assert_eq!(
        application_api::KAIGI_RELAYS_BY_RELAY_ID_GET.effect(),
        RouteEffect::ReadOnly
    );
    assert_eq!(
        application_api::KAIGI_RELAYS_HEALTH_GET.effect(),
        RouteEffect::ExpensiveCompute
    );
}
#[tokio::test]
async fn kaigi_relay_diagnostics_reject_missing_or_inexact_auth_before_handlers() {
    let harness = NoritoRpcHarness::new(|_| {});
    for path in [
        "/v1/kaigi/relays",
        "/v1/kaigi/relays/relay-id",
        "/v1/kaigi/relays/health",
    ] {
        let response = harness
            .app
            .clone()
            .oneshot(request(
                path.parse().expect("Kaigi route URI"),
                HeaderMap::new(),
            ))
            .await
            .expect("missing-signature response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    let list_uri: Uri = "/v1/kaigi/relays".parse().expect("list URI");
    let foreign_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &foreign_network_id(),
        &Method::GET,
        &list_uri,
        &[],
    )
    .expect("foreign-network signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(list_uri.clone(), foreign_headers))
        .await
        .expect("foreign-network response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let health_uri: Uri = "/v1/kaigi/relays/health".parse().expect("health URI");
    let wrong_path_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &health_uri,
        &[],
    )
    .expect("path-bound signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(list_uri.clone(), wrong_path_headers))
        .await
        .expect("path-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let exact_headers = iroha_torii::operator_signed_request_headers(
        &harness.cfg.common.key_pair,
        &harness.network_id,
        &Method::GET,
        &list_uri,
        &[],
    )
    .expect("query-bound signature fixture");
    let response = harness
        .app
        .clone()
        .oneshot(request(
            "/v1/kaigi/relays?format=json".parse().expect("query URI"),
            exact_headers,
        ))
        .await
        .expect("query-mismatch response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
