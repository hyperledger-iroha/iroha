#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level tests for ZK convenience endpoints.
#![cfg(feature = "app_api")]

use std::{collections::HashSet, sync::Arc};

use axum::{Router, extract::State, routing::post};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World, WorldReadOnly},
};
use iroha_data_model::{NewAccount, prelude::*};
use nonzero_ext::nonzero;
use tower::ServiceExt as _; // for Router::oneshot

const ACCOUNT_SIGNATORY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

fn state_with_registered_asset_definition() -> (Arc<CoreState>, String) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = CoreState::new_for_testing(World::new(), kura, query);

    let domain_id = DomainId::try_new("zkd", "universal").expect("domain id");
    let asset_definition_id = AssetDefinitionId::new(
        domain_id.clone(),
        "rose".parse().expect("asset definition name"),
    );
    let owner = AccountId::new(ACCOUNT_SIGNATORY.parse().expect("public key"));
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();

    for instruction in [
        Register::domain(Domain::new(domain_id)).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone()).with_name("rose".to_owned()),
        )
        .into(),
    ] {
        transaction
            .world
            .executor()
            .clone()
            .execute_instruction(&mut transaction, &owner, instruction)
            .expect("seed instruction must succeed");
    }
    transaction.apply();
    block.transactions.insert_block(
        HashSet::<iroha_crypto::HashOf<iroha_data_model::transaction::SignedTransaction>>::new(),
        nonzero!(1_usize),
    );
    let _ = block.commit();

    (Arc::new(state), asset_definition_id.to_string())
}

#[tokio::test]
async fn zk_roots_endpoint_returns_200_for_registered_asset_without_shielded_state() {
    let (state, asset_id) = state_with_registered_asset_definition();
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
        iroha_torii::json_entry("asset_id", asset_id),
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
    assert_eq!(v.get("latest").and_then(|value| value.as_str()), Some(""));
    assert_eq!(
        v.get("roots")
            .and_then(|value| value.as_array())
            .map(std::vec::Vec::len),
        Some(0)
    );
    assert_eq!(v.get("height").and_then(|value| value.as_u64()), Some(0));
}

#[tokio::test]
async fn zk_roots_endpoint_returns_404_for_missing_asset() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(CoreState::new_for_testing(World::default(), kura, query));

    let app = Router::new().route(
        "/v1/zk/roots",
        post({
            let state = state.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::ZkRootsGetRequestDto>| async move {
                iroha_torii::handle_v1_zk_roots(state, None, req).await
            }
        }),
    );

    let missing_asset_id = AssetDefinitionId::new(
        DomainId::try_new("missing", "universal").expect("domain id"),
        "rose".parse().expect("asset definition name"),
    )
    .to_string();
    let body_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset_id", missing_asset_id),
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
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn zk_roots_endpoint_returns_404_for_missing_asset_alias() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(CoreState::new_for_testing(World::default(), kura, query));

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
        iroha_torii::json_entry("asset_id", "rose#missing"),
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
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn zk_roots_endpoint_returns_403_for_invalid_asset_selector() {
    let (state, _) = state_with_registered_asset_definition();

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
        iroha_torii::json_entry("asset_id", "prefix:not-a-real-selector"),
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
    assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn zk_roots_endpoint_returns_403_for_blank_asset_selector() {
    let (state, _) = state_with_registered_asset_definition();

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
        iroha_torii::json_entry("asset_id", "   "),
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
    assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);
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
    assert_eq!(
        resp.headers().get(http::header::CONTENT_TYPE),
        Some(&http::HeaderValue::from_static("application/json"))
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert!(v.get("finalized").is_some());
    assert!(v.get("tally").is_some());
}
