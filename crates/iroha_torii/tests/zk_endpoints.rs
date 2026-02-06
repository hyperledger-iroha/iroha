#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level tests for ZK convenience endpoints.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, extract::State, routing::post};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World},
};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn zk_roots_endpoint_returns_200() {
    // Minimal in-memory state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(CoreState::new_for_testing(World::default(), kura, query));

    // Wire only the tested route
    let app = Router::new().route(
        "/v1/zk/roots",
        post({
            let state = state.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::ZkRootsGetRequestDto>| async move {
                iroha_torii::handle_v1_zk_roots(state, None, req).await
            }
        }),
    );

    let body_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset_id", "rose#wonderland"),
        iroha_torii::json_entry("max", 10u64),
    ]);
    let body = norito::json::to_string(&body_value).expect("serialize roots request");
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/roots")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    // Basic shape keys
    assert!(v.get("latest").is_some());
    assert!(v.get("roots").is_some());
    assert!(v.get("height").is_some());
}

#[tokio::test]
async fn zk_vote_tally_endpoint_returns_200() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(CoreState::new_for_testing(World::default(), kura, query));

    let app = Router::new().route(
        "/v1/zk/vote/tally",
        post({
            let state = state.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::ZkVoteGetTallyRequestDto>| async move {
                iroha_torii::handle_v1_zk_vote_tally(State(state), None, req).await
            }
        }),
    );

    let body_value =
        iroha_torii::json_object(vec![iroha_torii::json_entry("election_id", "nonexistent")]);
    let body = norito::json::to_string(&body_value).expect("serialize tally request");
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/vote/tally")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert!(v.get("finalized").is_some());
    assert!(v.get("tally").is_some());
}
