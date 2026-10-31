//! Integration coverage for the runtime change-set endpoints.
#![cfg(feature = "app_api")]
use axum::{extract::Path as AxPath, response::IntoResponse as _};
use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::State};
use iroha_torii::{handle_runtime_cancel_upgrade, handle_runtime_upgrades_list};
use std::sync::Arc;

fn conversion_message(error: iroha_torii::Error) -> String {
    match error {
        iroha_torii::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
        )) => message,
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn runtime_cancel_validates_identifiers_and_drafts_a_proposal() {
    let error = handle_runtime_cancel_upgrade(AxPath("xyz".to_string()))
        .await
        .expect_err("malformed hex");
    assert_eq!(conversion_message(error), "invalid id");
    let error = handle_runtime_cancel_upgrade(AxPath("aa".repeat(31)))
        .await
        .expect_err("31 bytes");
    assert_eq!(conversion_message(error), "invalid id length");

    let draft = handle_runtime_cancel_upgrade(AxPath(hex::encode([0x11; 32])))
        .await
        .expect("a well-formed change-set identity");
    let [instruction] = draft.tx_instructions.as_slice() else {
        panic!("one drafted instruction");
    };
    assert_eq!(
        instruction.wire_id,
        "iroha.instruction.v1::governance::ProposeRuntimeUpgradeProposal"
    );
    assert_eq!(draft.into_response().status(), http::StatusCode::OK);
}

#[tokio::test]
async fn runtime_upgrades_list_reports_the_founded_selection() {
    let state = State::new_for_testing(
        iroha_core::state::World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let response = handle_runtime_upgrades_list(Arc::new(state))
        .await
        .expect("list");
    assert!(
        response.items.is_empty(),
        "a new World records no change set"
    );
    assert_eq!(
        response.selection_id_hex,
        hex::encode(response.selection.id().expect("selection identity").0)
    );
    assert_eq!(
        response.selection,
        iroha_core::runtime::release_catalog()
            .expect("release catalog")
            .baseline,
        "a new World executes the baseline"
    );
}
