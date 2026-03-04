#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler test for governance enact endpoint.
#![cfg(feature = "app_api")]
#![allow(clippy::redundant_closure_for_method_calls)]

use std::sync::Arc;

use axum::response::IntoResponse;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_data_model::ChainId;
use iroha_torii::{EnactDto, MaybeTelemetry, NoritoJson, handle_gov_enact};

fn mk_basic_context() -> (Arc<State>, Arc<Queue>, Arc<ChainId>) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let events = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    let chain_id: ChainId = "chain".parse().expect("chain id");
    (state, queue, Arc::new(chain_id))
}

#[tokio::test]
async fn gov_enact_handler_builds_instruction() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: Torii gov enact handler test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let (state, queue, chain_id) = mk_basic_context();
    let dto = EnactDto {
        proposal_id: "aa".repeat(32),
        preimage_hash: Some("00".repeat(32)),
        window: Some(iroha_torii::AtWindowDto { lower: 1, upper: 2 }),
        authority: None,
        private_key: None,
    };
    let resp = handle_gov_enact(
        chain_id,
        queue,
        state,
        MaybeTelemetry::disabled(),
        NoritoJson(dto),
    )
    .await
    .expect("ok")
    .into_response();
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).expect("json parse");
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let arr = v
        .get("tx_instructions")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(arr.len(), 1);
}
