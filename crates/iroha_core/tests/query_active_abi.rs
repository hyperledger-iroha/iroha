//! Tests for `FindActiveAbiVersions` singular query.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::state::State;

#[test]
fn find_active_abi_versions_default_is_v1() {
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::ValidSingularQuery,
    };

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let world = iroha_core::state::World::default();
    let state = State::new_for_testing(world, kura, query_handle);

    let q = iroha_data_model::query::runtime::prelude::FindActiveAbiVersions;
    let resp = q.execute(&state.view()).expect("execute query");
    assert!(
        resp.active_versions.contains(&1),
        "v1 must be active by default"
    );
    assert_eq!(resp.default_compile_target, 1);
}
