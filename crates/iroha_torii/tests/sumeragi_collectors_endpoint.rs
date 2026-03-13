#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Router-level test for GET /v2/sumeragi/collectors"]
#![cfg(feature = "telemetry")]
#![allow(unexpected_cfgs)]

use std::{collections::HashSet, sync::Arc};

use axum::{Router, extract::State, routing::get};
use http_body_util::BodyExt;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World},
};
use iroha_data_model::{
    parameter::{Parameter, SumeragiParameter},
    prelude::*,
};
use nonzero_ext::nonzero;
use tower::ServiceExt;

#[tokio::test]
async fn sumeragi_collectors_endpoint_shape() {
    // Build minimal State
    let kura = Kura::blank_kura_for_testing();
    let qh = LiveQueryStore::start_test();
    let raw_state = CoreState::new_for_testing(World::default(), kura, qh);

    // Populate commit_topology with 4 peers
    let peers: Vec<PeerId> = (0..4)
        .map(|_| PeerId::new(iroha_crypto::KeyPair::random().public_key().clone()))
        .collect();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut sb = raw_state.block(header);
        sb.transactions
            .insert_block(HashSet::new(), nonzero!(1usize));
        *sb.commit_topology.get_mut() = peers.clone();
        // Set collectors_k = 2, r = 1 in on-chain params
        sb.world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Sumeragi(SumeragiParameter::CollectorsK(2)));
        sb.world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Sumeragi(SumeragiParameter::RedundantSendR(1)));
        sb.commit().expect("commit state block");
    }

    let state = Arc::new(raw_state);

    let app =
        Router::new().route(
            "/v2/sumeragi/collectors",
            get({
                let state = state.clone();
                move || async move {
                    iroha_torii::handle_v1_sumeragi_collectors(State(state), None).await
                }
            }),
        );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/collectors")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        v.get("topology_len").and_then(norito::json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        v.get("mode").and_then(norito::json::Value::as_str),
        Some("Permissioned")
    );
    assert_eq!(
        v.get("height").and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(v.get("view").and_then(norito::json::Value::as_u64), Some(0));
    assert_eq!(
        v.get("redundant_send_r")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert!(matches!(
        v.get("epoch_seed"),
        Some(norito::json::Value::Null)
    ));
    assert!(
        v.get("collectors")
            .and_then(norito::json::Value::as_array)
            .is_some()
    );
    assert_eq!(
        v.get("consensus_mode")
            .and_then(norito::json::Value::as_str),
        Some("Permissioned")
    );
    let prf = v
        .get("prf")
        .and_then(norito::json::Value::as_object)
        .expect("prf object present");
    assert_eq!(
        prf.get("height")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or_default(),
        0
    );
    assert_eq!(
        prf.get("view")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or_default(),
        0
    );
    assert!(
        prf.get("epoch_seed")
            .is_none_or(norito::json::native::Value::is_null)
    );
}
