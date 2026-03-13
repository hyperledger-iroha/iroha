#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level tests for `GET /v2/sumeragi/vrf/penalties/{epoch}`.
#![cfg(feature = "telemetry")]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use http_body_util::BodyExt;
use iroha_core::sumeragi::{epoch_report, epoch_report::VrfPenaltiesReport};
use tower::ServiceExt as _;

fn vrf_penalties_router() -> Router {
    Router::new().route(
        "/v2/sumeragi/vrf/penalties/:epoch",
        get(|path: axum::extract::Path<String>| async move {
            iroha_torii::handle_v1_sumeragi_vrf_penalties(path)
                .await
                .map(axum::response::IntoResponse::into_response)
        }),
    )
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_returns_report() {
    let epoch: u64 = 42;
    epoch_report::update(VrfPenaltiesReport {
        epoch,
        roster_len: 7,
        committed_no_reveal: vec![1, 3],
        no_participation: vec![2],
    });

    let app = vrf_penalties_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v2/sumeragi/vrf/penalties/{epoch}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok()),
        Some("application/json")
    );

    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        payload
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        epoch
    );
    assert_eq!(
        payload
            .get("roster_len")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        7
    );
    let no_reveal = payload
        .get("committed_no_reveal")
        .and_then(norito::json::Value::as_array)
        .expect("committed_no_reveal array present");
    assert_eq!(
        no_reveal
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let no_participation = payload
        .get("no_participation")
        .and_then(norito::json::Value::as_array)
        .expect("no_participation array present");
    assert_eq!(
        no_participation
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![2]
    );
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_returns_empty_when_missing() {
    let missing_epoch: u64 = 113;
    let app = vrf_penalties_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v2/sumeragi/vrf/penalties/{missing_epoch}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        payload
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        missing_epoch
    );
    assert_eq!(
        payload
            .get("roster_len")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        0
    );
    assert!(
        payload
            .get("committed_no_reveal")
            .and_then(norito::json::Value::as_array)
            .is_none_or(std::vec::Vec::is_empty)
    );
    assert!(
        payload
            .get("no_participation")
            .and_then(norito::json::Value::as_array)
            .is_none_or(std::vec::Vec::is_empty)
    );
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_parses_hex_epochs() {
    let epoch: u64 = 54;
    epoch_report::update(VrfPenaltiesReport {
        epoch,
        roster_len: 5,
        committed_no_reveal: vec![],
        no_participation: vec![4, 7],
    });
    let app = vrf_penalties_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v2/sumeragi/vrf/penalties/0x36")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        payload
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        epoch
    );
    let no_participation = payload
        .get("no_participation")
        .and_then(norito::json::Value::as_array)
        .expect("no_participation array present");
    assert_eq!(
        no_participation
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![4, 7]
    );
}
