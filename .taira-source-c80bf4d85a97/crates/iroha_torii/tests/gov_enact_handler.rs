#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler test for governance enact endpoint.
#![cfg(feature = "app_api")]
#![allow(clippy::redundant_closure_for_method_calls)]

use std::sync::Arc;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_torii::{EnactDto, NoritoJson, handle_gov_enact};

fn mk_basic_context() -> Arc<State> {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    Arc::new(State::new_for_testing(World::default(), kura, query))
}

#[tokio::test]
async fn gov_enact_handler_rejects_missing_proposal() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: Torii gov enact handler test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let state = mk_basic_context();
    let dto = EnactDto {
        proposal_id: "aa".repeat(32),
    };
    let error = handle_gov_enact(state, NoritoJson(dto))
        .await
        .expect_err("missing proposal must be rejected");
    assert!(error.to_string().contains("not found"));
}
