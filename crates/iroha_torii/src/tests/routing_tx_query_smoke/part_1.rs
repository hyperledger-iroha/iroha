use super::*;
use axum::http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, state::World, sumeragi::network_topology::Topology,
    tx::AcceptedTransaction,
};
use iroha_data_model::prelude as dm;
use std::{borrow::Cow, sync::Arc};
// use tower::ServiceExt; // not needed in this module
fn checked_smoke_keypair(
    seed: u8,
    algorithm: iroha_crypto::Algorithm,
    context: &'static str,
) -> KeyPair {
    checked_routing_fixture_keypair(seed, algorithm, context)
}
fn checked_smoke_account(seed: u8, context: &'static str) -> (dm::AccountId, KeyPair) {
    let kp = checked_smoke_keypair(seed, iroha_crypto::Algorithm::Ed25519, context);
    let account = dm::AccountId::new(kp.public_key().clone());
    (account, kp)
}
fn account_with_key() -> (dm::AccountId, KeyPair) {
    checked_smoke_account(0x40, "derive transaction query smoke fixture account key")
}
fn log_instruction() -> dm::InstructionBox {
    dm::Log::new(dm::Level::INFO, "test".to_string()).into()
}
#[tokio::test]
async fn handle_v1_contracts_activity_returns_contract_call_metadata() {
    use iroha_crypto::Algorithm;
    let _kura = Kura::blank_kura_for_testing();
    let _query = LiveQueryStore::start_test();
    let (authority, keypair) = account_with_key();
    let world = World::with([], [dm::Account::new(authority.clone()).build(&authority)], []);
    let mut native_chain = iroha_core::sumeragi::test_chain::CertifiedTestChain::start(
        iroha_core::sumeragi::test_chain::TestChainConfig::new(world, 1),
    )
    .expect("original native query fixture genesis");
    let state = Arc::clone(native_chain.state());
    let _kura = Arc::clone(native_chain.kura());
    let leader0 = checked_smoke_keypair(
        0x4B,
        Algorithm::BlsNormal,
        "derive contract-activity setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![iroha_model_base::peer::PeerId::new(
        leader0.public_key().clone(),
    )]);
    let st_block0 = state.block(iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(native_chain.height() + 1).unwrap(),
        state.view().latest_block_hash(),
        None,
        1_000,
        0,
    ));
    st_block0
        .commit_world_overlay_for_testing()
        .expect("seed query fixture World");
    let network_id = *state.network_id_ref();
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "contract_address".parse().unwrap(),
        dm::Json::new("irohac1fixturedlmmrouter"),
    );
    metadata.insert(
        "contract_alias".parse().unwrap(),
        dm::Json::new("dlmm_router"),
    );
    metadata.insert(
        "contract_entrypoint".parse().unwrap(),
        dm::Json::new("route_swap"),
    );
    metadata.insert(
        "contract_payload".parse().unwrap(),
        dm::Json::new(norito::json!({
            "amount_in": 100,
            "min_out": 95
        })),
    );
    let gas_asset_id = test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400aa");
    let fee_payment = dm::FeePaymentIntent::sponsor(
        dm::FeeSponsorProgramId::new(
            authority.clone(),
            "contract-activity".parse().expect("program name"),
        ),
        1,
        vec![dm::FeeChargeLimit::new(
            dm::FeeChargeKind::PipelineGas,
            gas_asset_id.clone(),
            dm::Quantity::from(1_000_u32),
        )],
        std::num::NonZeroU64::new(100_000),
    );
    let mut tx_builder = dm::TransactionBuilder::new(network_id, authority.clone(), fee_payment);
    tx_builder.set_creation_time(core::time::Duration::from_millis(1_710_000_000_000));
    let signed = tx_builder
        .with_metadata(metadata)
        .with_instructions([log_instruction()])
        .sign(keypair.private_key());
    let entry_hash = format!("{}", signed.hash_as_entrypoint());
    let tx = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
    let leader = checked_smoke_keypair(
        0x4C,
        Algorithm::BlsNormal,
        "derive contract-activity transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![iroha_model_base::peer::PeerId::new(
        leader.public_key().clone(),
    )]);
    let _committed = crate::test_utils::commit_native_accepted_inputs(&mut native_chain, vec![tx]);
    let resp = handle_v1_contracts_activity_get(
        Arc::clone(&state),
        DataspaceReadVisibility::all_for_tests(),
        iroha_torii_shared::list_query::ListQuery::new()
            .limit(10)
            .filter(
                iroha_torii_shared::list_query::field("authority").eq(authority.to_string())
                    & iroha_torii_shared::list_query::field("contract_alias").eq("dlmm_router")
                    & iroha_torii_shared::list_query::field("contract_entrypoint").eq("route_swap")
                    // The fixture names an unregistered sponsor program, so native
                    // execution records a rejection; committed history retains it.
                    & iroha_torii_shared::list_query::field("result_ok").eq(false),
            ),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert!(parsed.get("total").is_none());
    assert!(parsed["next_cursor"].is_null());
    assert_eq!(items.len(), 1, "the committed rejected call remains queryable");
    assert_eq!(items[0]["result_ok"].as_bool(), Some(false));
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash.as_str())
    );
    assert_eq!(items[0]["contract_alias"].as_str(), Some("dlmm_router"));
    assert_eq!(items[0]["contract_entrypoint"].as_str(), Some("route_swap"));
    assert_eq!(
        items[0]["contract_payload"]["amount_in"].as_u64(),
        Some(100)
    );
    assert_eq!(items[0]["fee_payment"]["payer"].as_str(), Some("sponsor"));
    assert_eq!(
        items[0]["fee_payment"]["value"]["gas_limit"].as_u64(),
        Some(100_000)
    );
    assert_eq!(
        items[0]["fee_payment"]["value"]["charge_limits"][0]["asset_definition_id"]
            .as_str()
            .expect("projected fee asset"),
        gas_asset_id.to_string()
    );
    let response = handle_v1_contracts_events_get(
        state,
        DataspaceReadVisibility::all_for_tests(),
        iroha_torii_shared::list_query::ListQuery::new()
            .filter(iroha_torii_shared::list_query::field("contract_alias").eq("dlmm_router")),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("event collection")
    .into_response();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let events: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        events["items"][0]["tx_hash_hex"].as_str(),
        Some(entry_hash.as_str())
    );
    assert!(events["items"][0]["block_index"].as_u64().is_some());
    assert!(events["next_cursor"].is_null());
}
// The production app path always uses the typed server-side predicate and
// then applies the authoritative endpoint filter to returned candidates.
// Typed server predicates mirror the authoritative endpoint semantics for
// authority and entrypoint-hash equality and membership operators.
