#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensure Torii account endpoints accept IH58 (preferred)/sora (second-best) path segments.
#![cfg(all(feature = "app_api", feature = "telemetry"))]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{Account, AccountAddressErrorCode, AccountId},
    asset::AssetDefinition,
    domain::{Domain, DomainId},
    peer::PeerId,
};
#[cfg(feature = "telemetry")]
use iroha_primitives::time::TimeSource;
use iroha_telemetry::metrics::Metrics;
use iroha_torii::{
    Torii,
    filter::{FieldPath, FilterExpr, Pagination, QueryEnvelope},
};
use norito::json;
use prometheus::core::Collector;
use tower::ServiceExt as _;
use urlencoding::encode;

#[path = "fixtures.rs"]
mod fixtures;

const ACCOUNT_SIGNATORY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const IH58_PREFIX: u16 = 0x002A;
const EMPTY_QUERY_ENVELOPE: &str = r#"{
    "filter": null,
    "sort": [],
    "pagination": {"limit": 1, "offset": 0},
    "fetch_size": null,
    "select": null
}"#;
const ACCOUNTS_TRANSACTIONS_CTX: &str = "/v1/accounts/{account_id}/transactions";
const ACCOUNTS_TRANSACTIONS_QUERY_CTX: &str = "/v1/accounts/{account_id}/transactions/query";
const ACCOUNTS_PERMISSIONS_CTX: &str = "/v1/accounts/{account_id}/permissions";
const ACCOUNTS_ASSETS_CTX: &str = "/v1/accounts/{account_id}/assets";
const ACCOUNTS_ASSETS_QUERY_CTX: &str = "/v1/accounts/{account_id}/assets/query";
const KAIGI_RELAY_DETAIL_ENDPOINT: &str = "/v1/kaigi/relays/{relay_id}";
const KAIGI_RELAY_DETAIL_CTX: &str = "/v1/kaigi/relays/{relay_id}";
const NEXUS_PUBLIC_LANE_STAKE_ENDPOINT: &str = "/v1/nexus/public_lanes/{lane_id}/stake";
const NEXUS_PUBLIC_LANE_STAKE_CTX: &str = "/v1/nexus/public_lanes/{lane_id}/stake";
const NEXUS_PUBLIC_LANE_VALIDATORS_ENDPOINT: &str = "/v1/nexus/public_lanes/{lane_id}/validators";
const REPO_AGREEMENTS_ENDPOINT: &str = "/v1/repo/agreements";
const REPO_AGREEMENTS_QUERY_ENDPOINT: &str = "/v1/repo/agreements/query";
const OFFLINE_ALLOWANCES_ENDPOINT: &str = "/v1/offline/allowances";
const OFFLINE_ALLOWANCES_QUERY_ENDPOINT: &str = "/v1/offline/allowances/query";
const OFFLINE_TRANSFERS_ENDPOINT: &str = "/v1/offline/transfers";
const OFFLINE_TRANSFERS_QUERY_ENDPOINT: &str = "/v1/offline/transfers/query";
const OFFLINE_SUMMARIES_ENDPOINT: &str = "/v1/offline/summaries";
const OFFLINE_SUMMARIES_QUERY_ENDPOINT: &str = "/v1/offline/summaries/query";
const OFFLINE_REVOCATIONS_ENDPOINT: &str = "/v1/offline/revocations";
const OFFLINE_REVOCATIONS_QUERY_ENDPOINT: &str = "/v1/offline/revocations/query";

fn query_envelope_with_address_format(fmt: &str) -> String {
    format!(
        r#"{{"filter":null,"sort":[],"pagination":{{"limit":1,"offset":0}},"fetch_size":null,"select":null,"address_format":"{fmt}"}}"#
    )
}

fn query_envelope_with_account_filter(field: &str, literal: &str) -> Vec<u8> {
    let filter = FilterExpr::Eq(FieldPath(field.to_string()), json::Value::from(literal));
    let envelope = QueryEnvelope {
        filter: Some(filter),
        pagination: Pagination {
            limit: Some(1),
            ..Pagination::default()
        },
        ..QueryEnvelope::default()
    };
    norito::json::to_vec(&envelope).expect("serialize envelope")
}

fn encode_filter(expr: &FilterExpr) -> String {
    let raw_value = json::to_value(expr).expect("serialize filter expr");
    let raw = json::to_string(&raw_value).expect("serialize filter expr");
    encode(&raw).into_owned()
}

fn encoded_issuer_filter(literal: &str) -> String {
    let expr = FilterExpr::Eq(
        FieldPath("issuer_id".to_string()),
        json::Value::from(literal),
    );
    encode_filter(&expr)
}

#[tokio::test]
async fn transactions_endpoint_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for segment in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/transactions"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn transactions_endpoint_rejects_invalid_account_segment() {
    let app = test_router();
    let literal = "not-an-ih58@wonderland";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/transactions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transactions_endpoint_accepts_address_format_param() {
    let app = test_router();
    let (canonical, _, _) = account_segments();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/accounts/{canonical}/transactions?address_format=compressed"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} when applying address_format",
        resp.status()
    );
}

#[tokio::test]
async fn transactions_endpoint_accepts_default_domain_without_suffix() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for segment in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/transactions"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn transactions_query_accepts_default_domain_without_suffix() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for segment in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/accounts/{segment}/transactions/query"))
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(EMPTY_QUERY_ENVELOPE))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain transactions/query segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn transactions_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let (canonical, _, _) = account_segments();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/accounts/{canonical}/transactions?address_format=unknown"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transactions_endpoint_accepts_public_key_segments() {
    let (app, metrics) = test_router_with_metrics();
    let literal = format!("{ACCOUNT_SIGNATORY}@wonderland");
    let before = counter_total(&metrics.torii_address_invalid_total);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/transactions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for public-key segment {literal}",
        resp.status()
    );

    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "public-key segments must not increment invalid counter"
    );
}

#[tokio::test]
async fn invalid_account_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/transactions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_CTX, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch"
    );
}

#[tokio::test]
async fn local8_segments_increment_invalid_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sn1short";
    let reason = AccountId::parse(literal)
        .expect_err("short Local-8 literal must fail to parse")
        .reason();
    let before_invalid = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/transactions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after_invalid = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_CTX, reason])
        .get();
    assert_eq!(
        after_invalid,
        before_invalid + 1,
        "torii_address_invalid_total delta mismatch for Local-8"
    );
}

#[tokio::test]
async fn transactions_query_endpoint_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for segment in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/accounts/{segment}/transactions/query"))
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(EMPTY_QUERY_ENVELOPE))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for transactions/query segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn transactions_query_endpoint_rejects_invalid_account_segment() {
    let app = test_router();
    let literal = "sorainvalid";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{literal}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transactions_query_invalid_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_QUERY_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{literal}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_QUERY_CTX, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for transactions/query"
    );
}

#[tokio::test]
async fn transactions_query_rejects_unknown_address_format() {
    let app = test_router();
    let (canonical, _, _) = account_segments();
    let envelope = query_envelope_with_address_format("invalid");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{canonical}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transactions_query_rejects_checksum_mismatch() {
    let (app, metrics) = test_router_with_metrics();
    let bad_literal = tampered_tx_query_literal();
    let reason = AccountAddressErrorCode::ChecksumMismatch.as_str();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_QUERY_CTX, reason]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{bad_literal}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.get(),
        before + 1,
        "checksum mismatches should increment invalid counter"
    );
}

#[tokio::test]
async fn transactions_query_placeholder_literal_rejected_without_shim() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "ignored@wonderland";
    let reason = AccountId::parse(literal)
        .expect_err("placeholder literal should fail checksum validation")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_TRANSACTIONS_QUERY_CTX, reason]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{literal}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.get(),
        before + 1,
        "placeholder literal should be treated as invalid"
    );
}

#[tokio::test]
async fn transactions_query_valid_literals_do_not_bump_invalid_metrics() {
    let (app, metrics) = test_router_with_metrics();
    let envelope =
        query_envelope_with_account_filter("authority", &fixtures::TX_QUERY_ACCOUNT.canonical);
    let before = counter_total(&metrics.torii_address_invalid_total);

    for segment in [
        fixtures::TX_QUERY_ACCOUNT.canonical.clone(),
        fixtures::TX_QUERY_ACCOUNT.compressed.clone(),
        fixtures::TX_QUERY_ACCOUNT.raw_public_key.clone(),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/accounts/{segment}/transactions/query"))
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(envelope.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "valid literal should not be rejected: segment={segment}, status={}",
            resp.status()
        );
    }

    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "valid tx query literals must not increment invalid counter"
    );
}

#[tokio::test]
async fn assets_endpoint_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for segment in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/assets?limit=1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for assets segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn assets_endpoint_accepts_default_domain_without_suffix() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for segment in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/assets?limit=1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain assets segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn assets_endpoint_rejects_invalid_segment() {
    let app = test_router();
    let literal = "sorainvalid";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/assets?limit=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn assets_endpoint_invalid_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_ASSETS_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/assets?limit=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_ASSETS_CTX, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for assets endpoint"
    );
}

#[tokio::test]
async fn assets_endpoint_accepts_public_key_segments() {
    let (app, metrics) = test_router_with_metrics();
    let literal = format!("{ACCOUNT_SIGNATORY}@wonderland");
    let before = counter_total(&metrics.torii_address_invalid_total);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/assets?limit=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for public-key asset segment {literal}",
        resp.status()
    );

    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "public-key asset segments must not increment invalid counter"
    );
}

#[tokio::test]
async fn assets_query_endpoint_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for segment in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/accounts/{segment}/assets/query"))
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(EMPTY_QUERY_ENVELOPE))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for assets/query segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn assets_query_endpoint_invalid_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_ASSETS_QUERY_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{literal}/assets/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_ASSETS_QUERY_CTX, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for assets/query"
    );
}

#[tokio::test]
async fn assets_query_endpoint_accepts_public_key_segments() {
    let (app, metrics) = test_router_with_metrics();
    let literal = format!("{ACCOUNT_SIGNATORY}@wonderland");
    let before = counter_total(&metrics.torii_address_invalid_total);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{literal}/assets/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(EMPTY_QUERY_ENVELOPE))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for public-key assets/query segment {literal}",
        resp.status()
    );

    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "public-key assets/query segments must not increment invalid counter"
    );
}

#[tokio::test]
async fn permissions_endpoint_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for segment in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/permissions?limit=1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for permissions segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn permissions_endpoint_accepts_default_domain_without_suffix() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for segment in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/accounts/{segment}/permissions?limit=1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain permissions segment {segment}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn permissions_endpoint_invalid_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_PERMISSIONS_CTX, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{literal}/permissions?limit=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[ACCOUNTS_PERMISSIONS_CTX, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for permissions endpoint"
    );
}

#[tokio::test]
async fn explorer_domains_query_accepts_encoded_account_params() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/explorer/domains?limit=1&owned_by={literal}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for explorer domains owned_by literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn explorer_domains_query_invalid_account_param_records_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorainvalid";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let context = "/v1/explorer/domains?owned_by";
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[context, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/explorer/domains?limit=1&owned_by={literal}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[context, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for explorer domains query"
    );
}

#[tokio::test]
async fn explorer_account_detail_accepts_encoded_account_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/explorer/accounts/{literal}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for explorer accounts literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn explorer_account_detail_invalid_segments_increment_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorainvalid";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let context = "/v1/explorer/accounts/{account_id}";
    let before = metrics
        .torii_address_invalid_total
        .with_label_values(&[context, reason])
        .get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/explorer/accounts/{literal}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let after = metrics
        .torii_address_invalid_total
        .with_label_values(&[context, reason])
        .get();
    assert_eq!(
        after,
        before + 1,
        "torii_address_invalid_total delta mismatch for explorer account detail"
    );
}

#[tokio::test]
async fn explorer_transactions_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&["/v1/explorer/transactions", "compressed"]);
    let before = counter.get();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/transactions?page=1&per_page=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for explorer transactions address_format",
        resp.status()
    );

    let after = counter.get();
    assert_eq!(
        after,
        before + 1,
        "expected address_format telemetry for explorer transactions"
    );
}

#[tokio::test]
async fn explorer_transactions_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/transactions?page=1&per_page=1&address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn explorer_instructions_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&["/v1/explorer/instructions", "compressed"]);
    let before = counter.get();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/instructions?page=1&per_page=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for explorer instructions address_format",
        resp.status()
    );
    let after = counter.get();
    assert_eq!(
        after,
        before + 1,
        "expected address_format telemetry for explorer instructions"
    );
}

#[tokio::test]
async fn explorer_transaction_detail_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&["/v1/explorer/transactions/{hash}", "compressed"]);
    let before = counter.get();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/transactions/deadbeef?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let after = counter.get();
    assert_eq!(
        after,
        before + 1,
        "expected address_format telemetry for explorer transaction detail"
    );
}

#[tokio::test]
async fn explorer_transaction_detail_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/transactions/deadbeef?address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn explorer_instruction_detail_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&["/v1/explorer/instructions/{hash}/{index}", "compressed"]);
    let before = counter.get();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/instructions/deadbeef/0?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let after = counter.get();
    assert_eq!(
        after,
        before + 1,
        "expected address_format telemetry for explorer instruction detail"
    );
}

#[tokio::test]
async fn explorer_instruction_detail_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/explorer/instructions/deadbeef/0?address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn explorer_account_qr_returns_svg_literal() {
    let app = test_router();
    let (canonical, _, _) = account_segments();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/explorer/accounts/{canonical}/qr"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        return;
    }
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("qr response body")
        .to_bytes();
    let parsed: json::Value = json::from_slice(&body).expect("qr payload json");
    let literal = parsed
        .get("literal")
        .and_then(json::Value::as_str)
        .expect("literal field");
    assert_eq!(literal, canonical);
    let svg = parsed
        .get("svg")
        .and_then(json::Value::as_str)
        .expect("svg field");
    assert!(
        svg.trim_start().starts_with("<svg"),
        "qr response should include svg payload"
    );
}

#[tokio::test]
async fn explorer_account_qr_respects_address_format_param() {
    let app = test_router();
    let (canonical, _, compressed) = account_segments();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/explorer/accounts/{canonical}/qr?address_format=compressed"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        return;
    }
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("qr response body")
        .to_bytes();
    let parsed: json::Value = json::from_slice(&body).expect("qr payload json");
    let literal = parsed
        .get("literal")
        .and_then(json::Value::as_str)
        .expect("literal field");
    assert_eq!(literal, compressed);
    let fmt = parsed
        .get("address_format")
        .and_then(json::Value::as_str)
        .expect("format field");
    assert_eq!(fmt, "compressed");
}

#[tokio::test]
async fn accounts_endpoint_accepts_address_format_param() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/accounts?limit=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for accounts endpoint with address_format",
        resp.status()
    );
}

#[tokio::test]
async fn accounts_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/accounts?limit=1&address_format=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn accounts_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("bad-format");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn asset_holders_endpoint_accepts_address_format_param() {
    let app = test_router();
    let def_param = "rose%23wonderland";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/assets/{def_param}/holders?limit=1&address_format=compressed"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for asset holders with address_format",
        resp.status()
    );
}

#[tokio::test]
async fn asset_holders_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let def_param = "rose%23wonderland";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/assets/{def_param}/holders?limit=1&address_format=bogus"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn asset_holders_query_rejects_unknown_address_format() {
    let app = test_router();
    let def_param = "rose%23wonderland";
    let envelope = query_envelope_with_address_format("bogus");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/assets/{def_param}/holders/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn repo_agreements_endpoint_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[REPO_AGREEMENTS_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/repo/agreements?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for repo agreements address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn repo_agreements_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/repo/agreements?address_format=unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn repo_agreements_query_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[REPO_AGREEMENTS_QUERY_ENDPOINT, "compressed"]);
    let before = counter.get();
    let envelope = query_envelope_with_address_format("compressed");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repo/agreements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for repo agreements query address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn repo_agreements_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("invalid");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repo/agreements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn repo_agreements_query_filter_accepts_encoded_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let body = query_envelope_with_account_filter("initiator", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/repo/agreements/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for initiator literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn repo_agreements_query_filter_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let body = query_envelope_with_account_filter("initiator", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/repo/agreements/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain initiator literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn repo_agreements_query_filter_rejects_invalid_literal() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[REPO_AGREEMENTS_ENDPOINT, reason]);
    let before = counter.get();
    let body = query_envelope_with_account_filter("initiator", literal);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repo/agreements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.get(),
        before + 1,
        "invalid repo filter literal should increment torii_address_invalid_total"
    );
}

#[tokio::test]
async fn repo_agreements_query_filter_rejects_local8_literal() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sn1short";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let invalid_counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[REPO_AGREEMENTS_ENDPOINT, reason]);
    let invalid_before = invalid_counter.get();
    let body = query_envelope_with_account_filter("initiator", literal);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repo/agreements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_counter.get(),
        invalid_before + 1,
        "invalid repo filter literal should increment torii_address_invalid_total"
    );
}

#[tokio::test]
async fn offline_allowances_endpoint_accepts_controller_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/allowances?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for offline allowances controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_allowances_endpoint_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/allowances?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_allowances_endpoint_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_ALLOWANCES_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/allowances?limit=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline allowances address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_allowances_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/allowances?limit=1&address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_allowances_endpoint_accepts_public_key_controller() {
    let (app, metrics) = test_router_with_metrics();
    let literal = format!("{ACCOUNT_SIGNATORY}@wonderland");
    let before = counter_total(&metrics.torii_address_invalid_total);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/offline/allowances?limit=1&controller_id={literal}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for public-key controller literal {literal}",
        resp.status()
    );
    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "public-key controller literals must not increment invalid counter"
    );
}

#[tokio::test]
async fn offline_allowances_query_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_ALLOWANCES_QUERY_ENDPOINT, "compressed"]);
    let before = counter.get();
    let envelope = query_envelope_with_address_format("compressed");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/allowances/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline allowances query address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_allowances_query_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let body = query_envelope_with_account_filter("controller_id", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/offline/allowances/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller filter literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_allowances_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("unknown");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/allowances/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_transfers_endpoint_accepts_controller_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/transfers?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for offline transfers controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_transfers_endpoint_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/transfers?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_transfers_endpoint_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_TRANSFERS_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/transfers?limit=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline transfers address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_transfers_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/transfers?limit=1&address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_transfers_query_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_TRANSFERS_QUERY_ENDPOINT, "compressed"]);
    let before = counter.get();
    let envelope = query_envelope_with_address_format("compressed");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/transfers/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline transfers query address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_transfers_query_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let body = query_envelope_with_account_filter("controller_id", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/offline/transfers/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller filter literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_transfers_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("unknown");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/transfers/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_summaries_endpoint_accepts_controller_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/summaries?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for offline summaries controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_summaries_endpoint_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/offline/summaries?limit=1&controller_id={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_summaries_endpoint_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_SUMMARIES_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/summaries?limit=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline summaries address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_summaries_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/summaries?limit=1&address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_summaries_query_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_SUMMARIES_QUERY_ENDPOINT, "compressed"]);
    let before = counter.get();
    let envelope = query_envelope_with_address_format("compressed");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/summaries/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline summaries query address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_summaries_query_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let body = query_envelope_with_account_filter("controller_id", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/offline/summaries/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain controller filter literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_summaries_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("unknown");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/summaries/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_revocations_endpoint_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_REVOCATIONS_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/revocations?limit=1&address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline revocations address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_revocations_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/offline/revocations?limit=1&address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_revocations_endpoint_accepts_filter_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let filter = encoded_issuer_filter(&literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/offline/revocations?filter={filter}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for issuer_id filter literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_revocations_endpoint_accepts_default_domain_filter_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let filter = encoded_issuer_filter(&literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/offline/revocations?filter={filter}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain issuer_id literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_revocations_endpoint_filter_rejects_invalid_literal() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[OFFLINE_REVOCATIONS_ENDPOINT, reason]);
    let before = counter.get();
    let filter = encoded_issuer_filter(literal);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/offline/revocations?filter={filter}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.get(),
        before + 1,
        "invalid issuer_id filter literal should increment torii_address_invalid_total"
    );
}

#[tokio::test]
async fn offline_revocations_query_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[OFFLINE_REVOCATIONS_QUERY_ENDPOINT, "compressed"]);
    let before = counter.get();
    let envelope = query_envelope_with_address_format("compressed");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/revocations/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for offline revocations query address_format",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn offline_revocations_query_rejects_unknown_address_format() {
    let app = test_router();
    let envelope = query_envelope_with_address_format("unknown");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/revocations/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offline_revocations_query_filter_accepts_encoded_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let body = query_envelope_with_account_filter("issuer_id", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/offline/revocations/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for issuer_id filter literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_revocations_query_filter_accepts_default_domain_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let body = query_envelope_with_account_filter("issuer_id", &literal);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/offline/revocations/query")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain issuer_id literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn offline_revocations_query_filter_rejects_invalid_literal() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorabaddigest";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[OFFLINE_REVOCATIONS_QUERY_ENDPOINT, reason]);
    let before = counter.get();
    let body = query_envelope_with_account_filter("issuer_id", literal);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/revocations/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.get(),
        before + 1,
        "invalid literal should increment torii_address_invalid_total"
    );
}

#[tokio::test]
async fn offline_revocations_query_filter_rejects_local8_literal() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sn1short";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let invalid_counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[OFFLINE_REVOCATIONS_QUERY_ENDPOINT, reason]);
    let invalid_before = invalid_counter.get();
    let body = query_envelope_with_account_filter("issuer_id", literal);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/revocations/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_counter.get(),
        invalid_before + 1,
        "invalid literal should increment torii_address_invalid_total"
    );
}

#[tokio::test]
async fn kaigi_relays_endpoint_accepts_address_format_param() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for kaigi relays address_format",
        resp.status()
    );
}

#[tokio::test]
async fn kaigi_relays_endpoint_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/kaigi/relays?address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kaigi_relay_detail_accepts_encoded_segments() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/kaigi/relays/{literal}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::NOT_FOUND | StatusCode::TOO_MANY_REQUESTS
            ),
            "expected kaigi relay detail to accept literal {literal}, got {}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn kaigi_relay_detail_rejects_invalid_segment() {
    let app = test_router();
    let literal = "sorainvalid";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/kaigi/relays/{literal}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kaigi_relay_detail_invalid_segment_increments_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorainvalid";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[KAIGI_RELAY_DETAIL_CTX, reason]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/kaigi/relays/{literal}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn kaigi_relay_detail_local8_segment_increments_invalid_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sn1short";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail to parse")
        .reason();
    let invalid_counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[KAIGI_RELAY_DETAIL_CTX, reason]);
    let invalid_before = invalid_counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/kaigi/relays/{literal}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(invalid_counter.get(), invalid_before + 1);
}

#[tokio::test]
async fn kaigi_relay_detail_rejects_unknown_address_format() {
    let app = test_router();
    let (canonical, _, _) = account_segments();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/kaigi/relays/{canonical}?address_format=bogus"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kaigi_relay_detail_address_format_metric_tracks_selection() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[KAIGI_RELAY_DETAIL_ENDPOINT, "compressed"]);
    let before = counter.get();
    let (canonical, _, _) = account_segments();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/kaigi/relays/{canonical}?address_format=compressed"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::NOT_FOUND | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} while recording kaigi address_format metric",
        resp.status()
    );
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn nexus_public_lane_stake_accepts_validator_literals() {
    let app = test_router();
    let (canonical, canonical_hex, compressed) = account_segments();

    for literal in [canonical, canonical_hex, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/nexus/public_lanes/42/stake?validator={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for validator literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn nexus_public_lane_stake_rejects_invalid_validator_literal() {
    let app = test_router();
    let literal = "sorainvalid";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/public_lanes/42/stake?validator={literal}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nexus_public_lane_stake_accepts_default_domain_validator_literals() {
    let app = test_router();
    let (ih58, compressed) = default_domain_segments_without_domain();

    for literal in [ih58, compressed] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/nexus/public_lanes/42/stake?validator={literal}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                resp.status(),
                StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
            ),
            "unexpected status {} for default-domain validator literal {literal}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn nexus_public_lane_stake_accepts_public_key_validator() {
    let (app, metrics) = test_router_with_metrics();
    let literal = format!("{ACCOUNT_SIGNATORY}@wonderland");
    let before = counter_total(&metrics.torii_address_invalid_total);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/public_lanes/42/stake?validator={literal}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} for public-key validator literal {literal}",
        resp.status()
    );
    let after = counter_total(&metrics.torii_address_invalid_total);
    assert_eq!(
        after, before,
        "public-key validator literals must not increment invalid counter"
    );
}

#[tokio::test]
async fn nexus_public_lane_stake_invalid_literal_increments_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sorainvalid";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[NEXUS_PUBLIC_LANE_STAKE_CTX, reason]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/public_lanes/42/stake?validator={literal}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(counter.get(), before + 1);
}

#[tokio::test]
async fn nexus_public_lane_stake_local8_literal_increments_invalid_metric() {
    let (app, metrics) = test_router_with_metrics();
    let literal = "sn1short";
    let reason = AccountId::parse(literal)
        .expect_err("literal must fail")
        .reason();
    let invalid_counter = metrics
        .torii_address_invalid_total
        .with_label_values(&[NEXUS_PUBLIC_LANE_STAKE_CTX, reason]);
    let invalid_before = invalid_counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/public_lanes/42/stake?validator={literal}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(invalid_counter.get(), invalid_before + 1);
}

#[tokio::test]
async fn nexus_public_lane_stake_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[NEXUS_PUBLIC_LANE_STAKE_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/42/stake?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} when requesting compressed literals",
        resp.status()
    );
    assert_eq!(
        counter.get(),
        before + 1,
        "address_format telemetry should record the selection"
    );
}

#[tokio::test]
async fn nexus_public_lane_stake_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/42/stake?address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nexus_public_lane_validators_accepts_address_format_param() {
    let (app, metrics) = test_router_with_metrics();
    let counter = metrics
        .torii_address_format_total
        .with_label_values(&[NEXUS_PUBLIC_LANE_VALIDATORS_ENDPOINT, "compressed"]);
    let before = counter.get();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/42/validators?address_format=compressed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ),
        "unexpected status {} when requesting compressed literals",
        resp.status()
    );
    assert_eq!(
        counter.get(),
        before + 1,
        "address_format telemetry should record the selection"
    );
}

#[tokio::test]
async fn nexus_public_lane_validators_rejects_unknown_address_format() {
    let app = test_router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/42/validators?address_format=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

fn test_router() -> Router {
    build_test_router().0
}

fn test_router_with_metrics() -> (Router, Arc<Metrics>) {
    build_test_router()
}

fn build_test_router() -> (Router, Arc<Metrics>) {
    use iroha_data_model::Registrable;

    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let domain_id: DomainId = "wonderland".parse().expect("domain parses");
    let signatory: PublicKey = ACCOUNT_SIGNATORY.parse().expect("key parses");
    let account_id = AccountId::new(domain_id.clone(), signatory);
    let account = Account::new(account_id.clone()).build(&account_id);
    let domain = Domain::new(domain_id).build(&account_id);
    let mut world = World::with([domain], [account], Vec::<AssetDefinition>::new());
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
    let metrics = fixtures::reset_shared_metrics();
    #[cfg(not(feature = "telemetry"))]
    let metrics = fixtures::reset_shared_metrics();
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            Arc::clone(&metrics),
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            ts,
            true,
        )
        .0
    };
    let da_receipt_signer = cfg.common.key_pair.clone();
    #[cfg(feature = "telemetry")]
    let torii = Torii::new(
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
    );
    #[cfg(not(feature = "telemetry"))]
    let torii = Torii::new(
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
    );

    (torii.api_router_for_tests(), metrics)
}

fn account_segments() -> (String, String, String) {
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::{AccountAddress, AccountId},
        domain::DomainId,
    };
    let domain: DomainId = "wonderland".parse().expect("domain parses");
    let public_key: PublicKey = ACCOUNT_SIGNATORY.parse().expect("key parses");
    let account = AccountId::new(domain.clone(), public_key);
    let address = AccountAddress::from_account_id(&account).expect("address encode");
    let canonical = account.to_string();
    let canonical_hex = account.to_canonical_hex().expect("canonical hex encoding");
    let compressed = address.to_compressed_sora().expect("compressed encode");
    (canonical, canonical_hex, compressed)
}

fn default_domain_segments_without_domain() -> (String, String) {
    use iroha_crypto::PublicKey;
    use iroha_data_model::account::{AccountAddress, AccountId, address};

    let domain_label = address::default_domain_name();
    let domain = domain_label
        .as_ref()
        .parse()
        .expect("default domain parses");
    let public_key: PublicKey = ACCOUNT_SIGNATORY.parse().expect("key parses");
    let account = AccountId::new(domain, public_key);
    let address = AccountAddress::from_account_id(&account).expect("address encode");
    let ih58 = address
        .to_ih58(IH58_PREFIX)
        .expect("ih58 encode for default domain");
    let compressed = address
        .to_compressed_sora()
        .expect("compressed encode for default domain");
    (ih58, compressed)
}

fn tampered_tx_query_literal() -> String {
    let mut mutated = fixtures::TX_QUERY_ACCOUNT.compressed.clone();
    let last = mutated.pop().unwrap_or('0');
    let replacement = if last == 'a' { 'b' } else { 'a' };
    mutated.push(replacement);
    mutated
}

fn counter_total(counter: &prometheus::IntCounterVec) -> u64 {
    counter
        .collect()
        .iter()
        .flat_map(prometheus::proto::MetricFamily::get_metric)
        .map(|metric| {
            let value = metric.get_counter().get_value();
            let rounded = value.round();
            assert!(
                rounded.is_finite() && rounded >= 0.0,
                "counter value must be finite and non-negative: {value}"
            );
            rounded
                .to_string()
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("counter value {value} must be integral"))
        })
        .sum()
}
