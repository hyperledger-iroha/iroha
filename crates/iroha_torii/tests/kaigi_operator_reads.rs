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
use std::collections::BTreeSet;
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
    const LEGACY_API_TOKEN: &str = "legacy-kaigi-token-00000000000000";
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.require_api_token = true;
        cfg.torii.api_tokens = vec![LEGACY_API_TOKEN.to_owned()].into();
    });
    let mut headers = HeaderMap::new();
    headers.insert("x-api-token", HeaderValue::from_static(LEGACY_API_TOKEN));
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
    "hash:0000000000000000000000000000000000000000000000000000000000000003#E54C"
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
#[test]
fn kaigi_relay_openapi_requires_the_exact_operator_signature_tuple() {
    let document = iroha_torii::openapi::generate_spec();
    let paths = document["paths"].as_object().expect("OpenAPI paths");
    let expected_headers = BTreeSet::from([
        "X-Iroha-Operator-Nonce",
        "X-Iroha-Operator-Public-Key",
        "X-Iroha-Operator-Signature",
        "X-Iroha-Operator-Timestamp-Ms",
    ]);
    for path in [
        "/v1/kaigi/relays",
        "/v1/kaigi/relays/{relay_id}",
        "/v1/kaigi/relays/health",
    ] {
        let operation = paths[path]["get"]
            .as_object()
            .expect("Kaigi relay GET operation");
        assert_eq!(
            operation
                .get("x-iroha-tool-effect")
                .and_then(norito::json::Value::as_str),
            Some("operator"),
            "{path}"
        );
        let headers = operation["parameters"]
            .as_array()
            .expect("Kaigi relay parameters")
            .iter()
            .filter_map(|parameter| {
                let parameter = parameter.as_object()?;
                (parameter.get("in").and_then(norito::json::Value::as_str) == Some("header"))
                    .then_some(parameter)
            })
            .map(|parameter| {
                assert_eq!(
                    parameter.get("required"),
                    Some(&norito::json::Value::Bool(true))
                );
                parameter["name"].as_str().expect("operator header name")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(headers, expected_headers, "{path}");
    }
}
#[test]
fn kaigi_relay_openapi_matches_runtime_output_bounds() {
    let document = iroha_torii::openapi::generate_spec();
    let event_responses = document["paths"]["/v1/kaigi/relays/events"]["get"]["responses"]
        .as_object()
        .expect("Kaigi relay event responses");
    assert_eq!(
        event_responses
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["200"],
        "the event-broadcast stream is not gated by the metrics telemetry profile"
    );
    let schemas = document["components"]["schemas"]
        .as_object()
        .expect("OpenAPI schemas");
    let summary = schemas["KaigiRelaySummary"]["properties"]
        .as_object()
        .expect("Kaigi relay summary properties");
    assert_eq!(summary["bandwidth_class"]["minimum"].as_u64(), Some(1));
    assert_eq!(summary["bandwidth_class"]["maximum"].as_u64(), Some(255));
    assert_eq!(
        summary["hpke_fingerprint_hex"]["pattern"].as_str(),
        Some("^[0-9a-f]{64}$")
    );
    assert!(
        summary["hpke_fingerprint_hex"]["description"]
            .as_str()
            .is_some_and(|description| description.contains("marked Blake2b-32"))
    );
    assert_eq!(
        schemas["KaigiRelaySummaryList"]["properties"]["items"]["maxItems"].as_u64(),
        Some(500)
    );
    assert_eq!(
        schemas["KaigiRelaySummaryList"]["properties"]["total"]["maximum"].as_u64(),
        Some(500)
    );
    assert_eq!(
        schemas["KaigiRelayHealthSnapshot"]["properties"]["domains"]["maxItems"].as_u64(),
        Some(500)
    );
    assert_eq!(
        schemas["KaigiRelayDetail"]["properties"]["notes"]["maxLength"].as_u64(),
        Some(512)
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
