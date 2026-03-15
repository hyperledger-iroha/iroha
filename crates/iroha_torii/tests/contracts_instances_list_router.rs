#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for /v1/contracts/instances/{ns}
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn contracts_instances_list_returns_instances() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: contracts instances list test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let mut state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    // Seed contract instance
    let key = iroha_crypto::Hash::prehashed([0xCD; 32]);
    iroha_core::query::insert_contract_instance_for_test(&mut state, "apps", "calc.v1", key);

    let state = Arc::new(state);

    let app = Router::new().route(
        "/v1/contracts/instances/{ns}",
        get({
            let state = state.clone();
            move |_headers: axum::http::HeaderMap,
                  axum::extract::Path(ns): axum::extract::Path<String>,
                  q: iroha_torii::NoritoQuery<iroha_torii::InstancesQuery>| async move {
                // no rate limit in test
                iroha_torii::handle_gov_instances_by_ns(state, axum::extract::Path(ns), q).await
            }
        }),
    );

    let req = http::Request::builder()
        .method("GET")
        .uri("/v1/contracts/instances/apps")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let arr = v
        .get("instances")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(arr.len(), 1);
}
