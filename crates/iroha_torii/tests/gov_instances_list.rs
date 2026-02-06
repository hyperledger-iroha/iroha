#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler test for listing contract instances by namespace.
#![cfg(feature = "app_api")]

use axum::response::IntoResponse;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};

#[tokio::test]
async fn instances_list_returns_code_hashes() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: Torii gov instances list test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Build minimal state
    let mut st_raw = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let key = iroha_crypto::Hash::prehashed([0xAB; 32]);
    iroha_core::query::insert_contract_instance_for_test(&mut st_raw, "apps", "calc.v1", key);
    let state = std::sync::Arc::new(st_raw);

    let resp = iroha_torii::handle_gov_instances_by_ns(
        state,
        axum::extract::Path("apps".to_string()),
        iroha_torii::NoritoQuery(iroha_torii::InstancesQuery::default()),
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
    assert_eq!(v.get("namespace").and_then(|x| x.as_str()), Some("apps"));
    let arr = v
        .get("instances")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(arr.len(), 1);
}
