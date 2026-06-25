#![doc = "SCCP route-manifest ISI execution tests."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::num::NonZeroU64;

use iroha_core::{kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State};
use iroha_data_model::{
    block::BlockHeader,
    isi::{
        Grant,
        bridge::{
            RemoveSccpRouteManifest, SccpRouteBrowserProverManifestRef, SccpRouteManifest,
            UpsertSccpRouteManifest,
        },
    },
    permission::Permission,
};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;

#[path = "common/world_fixture.rs"]
mod test_world;

const CAN_MANAGE_SCCP_ROUTE_MANIFESTS: &str = "CanManageSccpRouteManifests";

fn hex32(byte: u8) -> String {
    format!("0x{}", hex::encode([byte; 32]))
}

fn browser_prover_ref(
    seed: u8,
    route_hash: &str,
    proof_hash: &str,
) -> SccpRouteBrowserProverManifestRef {
    SccpRouteBrowserProverManifestRef {
        module_url: format!("https://provers.sora.org/bsc-{seed}.js"),
        module_specifier: Some(format!("@sora/sccp-bsc-prover/{seed}")),
        module_hash: hex32(seed),
        manifest_hash: hex32(seed + 1),
        expected_exports: vec![
            "bscSccpProve".to_owned(),
            "bscSccpNativeProverSelfTest".to_owned(),
        ],
        bound_route_hash: route_hash.to_owned(),
        bound_proof_hash: proof_hash.to_owned(),
    }
}

fn production_bsc_route_manifest() -> SccpRouteManifest {
    let destination_binding_hash = hex32(0x47);
    let proof_artifact_hash = hex32(0x4c);
    let source_event_transaction_id = hex32(0x4b);
    let route_canary_transaction_id = hex32(0x4d);
    SccpRouteManifest {
        version: 1,
        route_id: "taira_bsc_xor".to_owned(),
        asset_key: "xor".to_owned(),
        tron_network: "bsc-testnet".to_owned(),
        chain: "bsc-testnet".to_owned(),
        chain_id_hex: "0x61".to_owned(),
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_BSC,
        verifier_target: "EvmContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: format!("0x{}", "61".repeat(32)),
        taira_xor_token_address: "0x1111111111111111111111111111111111111111".to_owned(),
        taira_xor_bridge_address: "0x2222222222222222222222222222222222222222".to_owned(),
        sccp_tron_source_bridge_address: "0x3333333333333333333333333333333333333333".to_owned(),
        tron_verifier_address: "0x4444444444444444444444444444444444444444".to_owned(),
        verifier_code_hash: hex32(0x45),
        verifier_key_hash: hex32(0x46),
        proof_artifact_hash: Some(proof_artifact_hash.clone()),
        proving_key_hash: Some(hex32(0x55)),
        native_evm_prover_bundle_hash: Some(hex32(0x50)),
        destination_browser_prover: Some(browser_prover_ref(
            0x60,
            &destination_binding_hash,
            &proof_artifact_hash,
        )),
        source_browser_prover: Some(browser_prover_ref(
            0x70,
            &destination_binding_hash,
            &proof_artifact_hash,
        )),
        deployment_evidence_sha256: Some(hex32(0x4f)),
        destination_binding_key: "evm:0:2:test-binding".to_owned(),
        destination_binding_hash,
        taira_burn_record_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        taira_burn_record_contract_artifact_b64: "QUJDREVGRw==".to_owned(),
        taira_burn_record_artifact_sha256: hex32(0x48),
        taira_burn_record_code_hash: hex32(0x49),
        taira_burn_record_vk_backend: "halo2_ipa".to_owned(),
        taira_burn_record_vk_name: "taira_bsc_xor_burn_record_v1".to_owned(),
        taira_burn_record_gas_limit: 2_000_000,
        settlement_contract_address: None,
        settlement_contract_alias: None,
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_bridge_config_hash: Some(hex32(0x4a)),
        post_deploy_source_event_transaction_id: Some(source_event_transaction_id.clone()),
        post_deploy_source_event_explorer_url: Some(format!(
            "https://testnet.bscscan.com/tx/{source_event_transaction_id}"
        )),
        post_deploy_route_canary_evidence_hash: Some(hex32(0x4e)),
        post_deploy_route_canary_transaction_id: Some(route_canary_transaction_id.clone()),
        post_deploy_route_canary_explorer_url: Some(format!(
            "https://testnet.bscscan.com/tx/{route_canary_transaction_id}"
        )),
        post_deploy_offline_full_toml_sha256: Some(hex32(0x56)),
    }
}

fn test_state() -> State {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query_handle)
}

fn test_header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0)
}

fn grant_route_manifest_permission(stx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let permission = Permission::new(
        CAN_MANAGE_SCCP_ROUTE_MANIFESTS.parse().unwrap(),
        Json::new(()),
    );
    Grant::account_permission(permission, ALICE_ID.clone())
        .execute(&ALICE_ID.clone(), stx)
        .expect("grant route manifest permission");
}

#[test]
fn sccp_route_manifest_isi_requires_permission_and_mutates_state_transaction() {
    let state = test_state();
    let mut block = state.block(test_header());
    let manifest = production_bsc_route_manifest();

    {
        let mut denied_tx = block.transaction();
        let denied = UpsertSccpRouteManifest::new(manifest.clone())
            .execute(&ALICE_ID.clone(), &mut denied_tx)
            .expect_err("upsert must require CanManageSccpRouteManifests");
        assert!(
            format!("{denied:?}").contains(CAN_MANAGE_SCCP_ROUTE_MANIFESTS),
            "unexpected denial: {denied:?}"
        );
    }

    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("upsert with permission");
    assert_eq!(stx.zk.sccp_route_manifests.len(), 1);
    assert_eq!(stx.zk.sccp_route_manifests[0].route_id, "taira_bsc_xor");
    assert_eq!(stx.zk.sccp_route_manifests[0].chain_id_hex, "0x61");

    let mut replacement = manifest.clone();
    replacement.taira_xor_bridge_address = "0x5555555555555555555555555555555555555555".to_owned();
    UpsertSccpRouteManifest::new(replacement)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("replace same route key");
    assert_eq!(
        stx.zk.sccp_route_manifests.len(),
        1,
        "same route_id/asset/domain/chain key must replace, not append"
    );
    assert_eq!(
        stx.zk.sccp_route_manifests[0].taira_xor_bridge_address,
        "0x5555555555555555555555555555555555555555"
    );

    RemoveSccpRouteManifest::new(
        "taira_bsc_xor".to_owned(),
        "xor".to_owned(),
        iroha_sccp::SCCP_DOMAIN_BSC,
        "0X61".to_owned(),
    )
    .execute(&ALICE_ID.clone(), &mut stx)
    .expect("remove with case-normalized chain id");
    assert!(stx.zk.sccp_route_manifests.is_empty());
}

#[test]
fn production_bsc_route_manifest_isi_rejects_incomplete_or_foreign_payloads() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);

    let mut missing_native_hash = production_bsc_route_manifest();
    missing_native_hash.native_evm_prover_bundle_hash = None;
    let err = UpsertSccpRouteManifest::new(missing_native_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production BSC route must include native bundle hash");
    assert!(
        format!("{err:?}").contains("native_evm_prover_bundle_hash"),
        "unexpected error: {err:?}"
    );

    let mut wrong_route = production_bsc_route_manifest();
    wrong_route.route_id = "taira_bsc_usdt".to_owned();
    let err = UpsertSccpRouteManifest::new(wrong_route)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production BSC route must be taira_bsc_xor/xor");
    assert!(
        format!("{err:?}").contains("taira_bsc_xor/xor"),
        "unexpected error: {err:?}"
    );

    assert!(
        stx.zk.sccp_route_manifests.is_empty(),
        "rejected route manifests must not mutate state transaction"
    );
}

#[test]
fn sccp_route_manifest_remove_missing_target_errors_without_mutating_state() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    let manifest = production_bsc_route_manifest();
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("insert route manifest");

    let err = RemoveSccpRouteManifest::new(
        "taira_bsc_xor".to_owned(),
        "xor".to_owned(),
        iroha_sccp::SCCP_DOMAIN_BSC,
        "0x62".to_owned(),
    )
    .execute(&ALICE_ID.clone(), &mut stx)
    .expect_err("remove must fail when no route manifest key matches");
    assert!(
        format!("{err:?}").contains("removal target was not found"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        stx.zk.sccp_route_manifests.len(),
        1,
        "failed removal must preserve existing route manifest"
    );
    assert_eq!(
        stx.zk.sccp_route_manifests[0].taira_xor_bridge_address,
        manifest.taira_xor_bridge_address
    );
}

#[test]
fn production_bsc_route_manifest_isi_rejects_untrusted_browser_prover_material_without_mutating_state()
 {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    let manifest = production_bsc_route_manifest();
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("insert route manifest");

    let mut credentialed_url = manifest.clone();
    credentialed_url
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .module_url = "https://operator:secret@provers.sora.org/bsc.js".to_owned();
    let err = UpsertSccpRouteManifest::new(credentialed_url)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("credentialed browser prover URLs must be rejected");
    assert!(
        format!("{err:?}").contains("module_url must not contain credentials"),
        "unexpected error: {err:?}"
    );

    let mut empty_exports = manifest.clone();
    empty_exports
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .expected_exports = Vec::new();
    let err = UpsertSccpRouteManifest::new(empty_exports)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("empty browser prover export lists must be rejected");
    assert!(
        format!("{err:?}").contains("expected_exports must not be empty"),
        "unexpected error: {err:?}"
    );

    let mut wrong_route_binding = manifest.clone();
    wrong_route_binding
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .bound_route_hash = hex32(0x99);
    let err = UpsertSccpRouteManifest::new(wrong_route_binding)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("browser prover route binding must match the route manifest");
    assert!(
        format!("{err:?}").contains("bound_route_hash must match destination_binding_hash"),
        "unexpected error: {err:?}"
    );

    let mut wrong_proof_binding = manifest.clone();
    wrong_proof_binding
        .source_browser_prover
        .as_mut()
        .expect("source prover")
        .bound_proof_hash = hex32(0x98);
    let err = UpsertSccpRouteManifest::new(wrong_proof_binding)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("browser prover proof binding must match the route manifest");
    assert!(
        format!("{err:?}").contains("bound_proof_hash must match proof_artifact_hash"),
        "unexpected error: {err:?}"
    );

    assert_eq!(
        stx.zk.sccp_route_manifests.len(),
        1,
        "rejected browser prover material must not replace the existing manifest"
    );
    assert_eq!(
        stx.zk.sccp_route_manifests[0].taira_xor_bridge_address,
        manifest.taira_xor_bridge_address
    );
}
