#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level tests for `GET /v1/sumeragi/vrf/penalties/{epoch}`.
#![cfg(feature = "telemetry")]
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::get,
};
use http_body_util::BodyExt;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World},
};
use iroha_data_model::consensus::VrfEpochRecord;
use std::sync::Arc;
use tower::ServiceExt as _;

fn vrf_epoch_record(
    epoch: u64,
    roster_len: u32,
    committed_no_reveal: Vec<u32>,
    no_participation: Vec<u32>,
    finalized: bool,
) -> VrfEpochRecord {
    VrfEpochRecord {
        epoch,
        seed: [0xA5; 32],
        epoch_length: 100,
        commit_deadline_offset: 20,
        reveal_deadline_offset: 40,
        roster_len,
        finalized,
        updated_at_height: epoch.saturating_mul(100).saturating_add(99),
        participants: Vec::new(),
        late_reveals: Vec::new(),
        committed_no_reveal,
        no_participation,
        penalties_applied: false,
        penalties_applied_at_height: None,
        validator_election: None,
    }
}

fn state_with_vrf_epoch(record: Option<VrfEpochRecord>) -> Arc<CoreState> {
    let mut world = World::new();
    if let Some(record) = record {
        let epoch = record.epoch;
        let mut block = world.block();
        block.vrf_epochs_mut_for_testing().insert(epoch, record);
        block.commit();
    }
    Arc::new(CoreState::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ))
}

fn vrf_penalties_router(state: Arc<CoreState>) -> Router {
    Router::new()
        .route(
            "/v1/sumeragi/vrf/penalties/{epoch}",
            get(
                |State(state): State<Arc<CoreState>>,
                 path: axum::extract::Path<String>| async move {
                    iroha_torii::handle_v1_sumeragi_vrf_penalties(State(state), path)
                        .await
                        .map(axum::response::IntoResponse::into_response)
                },
            ),
        )
        .with_state(state)
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_reads_exact_finalized_record_from_committed_state() {
    let epoch: u64 = 42;
    let state = state_with_vrf_epoch(Some(vrf_epoch_record(epoch, 7, vec![1, 3], vec![2], true)));
    let app = vrf_penalties_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/sumeragi/vrf/penalties/{epoch}"))
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
    let object = payload
        .as_object()
        .expect("penalty report must be a JSON object");
    assert_eq!(
        object.len(),
        4,
        "penalty report must contain only the current first-release fields"
    );
    for field in [
        "epoch",
        "roster_len",
        "committed_no_reveal",
        "no_participation",
    ] {
        assert!(object.contains_key(field), "missing current field: {field}");
    }
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
    for retired in [
        "vrf_penalty_epoch",
        "vrf_committed_no_reveal_total",
        "vrf_no_participation_total",
        "vrf_late_reveals_total",
    ] {
        assert!(
            payload.get(retired).is_none(),
            "retired process-local counter field leaked: {retired}"
        );
    }
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_returns_not_found_when_missing() {
    let missing_epoch: u64 = 113;
    let app = vrf_penalties_router(state_with_vrf_epoch(None));
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/sumeragi/vrf/penalties/{missing_epoch}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    assert!(
        body.is_empty(),
        "missing reports must not synthesize a payload"
    );
}
#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_rejects_unfinalized_persisted_record() {
    let epoch = 114;
    let state = state_with_vrf_epoch(Some(vrf_epoch_record(
        epoch,
        4,
        Vec::new(),
        Vec::new(),
        false,
    )));
    let app = vrf_penalties_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/sumeragi/vrf/penalties/{epoch}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert!(
        body.is_empty(),
        "an unfinalized epoch must not publish a penalty report"
    );
}

#[tokio::test]
async fn sumeragi_vrf_penalties_endpoint_parses_hex_epochs() {
    let epoch: u64 = 54;
    let state = state_with_vrf_epoch(Some(vrf_epoch_record(
        epoch,
        5,
        Vec::new(),
        vec![4, 7],
        true,
    )));
    let app = vrf_penalties_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/vrf/penalties/0x36")
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
