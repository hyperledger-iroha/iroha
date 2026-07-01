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
    let source_deployment_receipt_hash = hex32(0x51);
    let source_material = Json::new(norito::json!({
        "version": 1,
        "source_domain": 2,
        "target_domain": 0,
        "source_chain": "bsc"
    }));
    let source_deployment = Json::new(norito::json!({
        "version": 1,
        "source_domain": 2,
        "target_domain": 0,
        "source_chain": "bsc",
        "deployment_receipt_hash": source_deployment_receipt_hash
    }));
    SccpRouteManifest {
        version: 1,
        route_id: "taira_bsc_xor".to_owned(),
        asset_key: "xor".to_owned(),
        tron_network: "bsc-testnet".to_owned(),
        chain: "bsc-testnet".to_owned(),
        chain_id_hex: "0x61".to_owned(),
        explorer_url: Some("https://testnet.bscscan.com".to_owned()),
        explorer_host: Some("testnet.bscscan.com".to_owned()),
        counterparty_account_codec: Some(2),
        counterparty_account_codec_key: Some("evm_hex".to_owned()),
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
        native_evm_prover_bundle: Some(iroha_primitives::json::Json::new(norito::json!({
            "schema": "sccp-bsc-native-evm-prover-bundle/v1",
            "routeId": "taira_bsc_xor",
            "assetKey": "xor"
        }))),
        source_verifier_material: Some(source_material.clone()),
        source_adapter_engine_deployment: Some(source_deployment),
        source_adapter_engine: Some(source_material),
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

fn ton_raw(seed: u8) -> String {
    format!("0:{}", hex::encode([seed; 32]))
}

fn production_ton_route_manifest() -> SccpRouteManifest {
    let destination_binding_hash =
        "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799".to_owned();
    let proof_artifact_hash = hex32(0xcc);
    let source_deployment_receipt_hash = hex32(0x51);
    let source_material = Json::new(norito::json!({
        "version": 1,
        "source_domain": 4,
        "target_domain": 0,
        "source_chain": "ton-testnet"
    }));
    let source_deployment = Json::new(norito::json!({
        "version": 1,
        "source_domain": 4,
        "target_domain": 0,
        "source_chain": "ton-testnet",
        "deployment_receipt_hash": source_deployment_receipt_hash
    }));
    SccpRouteManifest {
        version: 1,
        route_id: "taira_ton_xor".to_owned(),
        asset_key: "xor".to_owned(),
        tron_network: "testnet".to_owned(),
        chain: "ton-testnet".to_owned(),
        chain_id_hex: "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd"
            .to_owned(),
        explorer_url: Some("https://testnet.tonscan.org".to_owned()),
        explorer_host: Some("testnet.tonscan.org".to_owned()),
        counterparty_account_codec: Some(4),
        counterparty_account_codec_key: Some("ton_raw".to_owned()),
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_TON,
        verifier_target: "TonContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd"
            .to_owned(),
        taira_xor_token_address: ton_raw(0x11),
        taira_xor_bridge_address: ton_raw(0x22),
        sccp_tron_source_bridge_address: ton_raw(0x33),
        tron_verifier_address: ton_raw(0x44),
        verifier_code_hash: hex32(0xca),
        verifier_key_hash: hex32(0xcb),
        proof_artifact_hash: Some(proof_artifact_hash.clone()),
        proving_key_hash: Some(hex32(0xcd)),
        native_evm_prover_bundle_hash: None,
        native_evm_prover_bundle: None,
        source_verifier_material: Some(source_material.clone()),
        source_adapter_engine_deployment: Some(source_deployment),
        source_adapter_engine: Some(source_material),
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
        deployment_evidence_sha256: Some(hex32(0xce)),
        destination_binding_key: "sccp:0:4:ton:ton-contract-v1:3".to_owned(),
        destination_binding_hash,
        taira_burn_record_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        taira_burn_record_contract_artifact_b64: "QUJDREVGRw==".to_owned(),
        taira_burn_record_artifact_sha256: hex32(0xcf),
        taira_burn_record_code_hash: hex32(0xd1),
        taira_burn_record_vk_backend: "halo2/ipa".to_owned(),
        taira_burn_record_vk_name: "taira_xor_burn_record_v1".to_owned(),
        taira_burn_record_gas_limit: 2_000_000,
        settlement_contract_address: None,
        settlement_contract_alias: Some("taira_ton_xor_burn_record".to_owned()),
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_bridge_config_hash: Some(hex32(0xd2)),
        post_deploy_source_event_transaction_id: Some(hex32(0xd3)),
        post_deploy_source_event_explorer_url: None,
        post_deploy_route_canary_evidence_hash: Some(hex32(0xd4)),
        post_deploy_route_canary_transaction_id: Some(hex32(0xd5)),
        post_deploy_route_canary_explorer_url: None,
        post_deploy_offline_full_toml_sha256: Some(hex32(0xd6)),
    }
}

fn production_tron_route_manifest() -> SccpRouteManifest {
    let network_id_hex = "0x000000000000000000000000000000000000000000000000000000002b6653dc";
    let verifier_address = "TKJtY3UFssmhUSg1FPdXyxWcHKS9SWVtCJ";
    let verifier_code_hash = hex32(0xab);
    let verifier_key_hash = hex32(0xac);
    let destination_binding_key = format!(
        "tron:0:5:{}:{verifier_address}:{verifier_code_hash}:{verifier_key_hash}",
        network_id_hex
            .strip_prefix("0x")
            .expect("fixture network id is 0x-prefixed")
    );
    SccpRouteManifest {
        version: 1,
        route_id: "taira_tron_xor".to_owned(),
        asset_key: "xor".to_owned(),
        tron_network: "mainnet".to_owned(),
        chain: "tron-mainnet".to_owned(),
        chain_id_hex: "0x2b6653dc".to_owned(),
        explorer_url: None,
        explorer_host: None,
        counterparty_account_codec: None,
        counterparty_account_codec_key: None,
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_TRON,
        verifier_target: "TronContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: network_id_hex.to_owned(),
        taira_xor_token_address: "TT1DaQcqzoJEzEaHDU8nsmiKtiyhXHaSKD".to_owned(),
        taira_xor_bridge_address: "TWvqVD8cuSTqisoDrPKfwkkrpAsziL3XFh".to_owned(),
        sccp_tron_source_bridge_address: "TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned(),
        tron_verifier_address: verifier_address.to_owned(),
        verifier_code_hash,
        verifier_key_hash,
        proof_artifact_hash: None,
        proving_key_hash: None,
        native_evm_prover_bundle_hash: None,
        native_evm_prover_bundle: None,
        source_verifier_material: None,
        source_adapter_engine_deployment: None,
        source_adapter_engine: None,
        destination_browser_prover: None,
        source_browser_prover: None,
        deployment_evidence_sha256: None,
        destination_binding_key,
        destination_binding_hash:
            "0x4c5b208d148cee784d611f77434a7dfac6b22a37b86faf82063d371ba7d3a1bc".to_owned(),
        taira_burn_record_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        taira_burn_record_contract_artifact_b64: "QUJDREVGRw==".to_owned(),
        taira_burn_record_artifact_sha256: hex32(0xae),
        taira_burn_record_code_hash: hex32(0xaf),
        taira_burn_record_vk_backend: "halo2/ipa".to_owned(),
        taira_burn_record_vk_name: "taira_xor_burn_record_v1".to_owned(),
        taira_burn_record_gas_limit: 2_000_000,
        settlement_contract_address: None,
        settlement_contract_alias: Some("taira_xor_burn_record".to_owned()),
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_bridge_config_hash: Some(hex32(0xb1)),
        post_deploy_source_event_transaction_id: Some(hex32(0xb2)),
        post_deploy_source_event_explorer_url: None,
        post_deploy_route_canary_evidence_hash: Some(hex32(0xb3)),
        post_deploy_route_canary_transaction_id: Some(hex32(0xb4)),
        post_deploy_route_canary_explorer_url: None,
        post_deploy_offline_full_toml_sha256: Some(hex32(0xb5)),
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

    let mut replayed_deployment_hash = production_bsc_route_manifest();
    replayed_deployment_hash.deployment_evidence_sha256 =
        Some(replayed_deployment_hash.verifier_code_hash.clone());
    let err = UpsertSccpRouteManifest::new(replayed_deployment_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("deployment evidence hash must not replay verifier code hash");
    assert!(
        format!("{err:?}").contains("deployment_evidence_sha256 must not equal verifier_code_hash"),
        "unexpected error: {err:?}"
    );

    let mut replayed_post_deploy_hash = production_bsc_route_manifest();
    replayed_post_deploy_hash.post_deploy_route_canary_evidence_hash = replayed_post_deploy_hash
        .post_deploy_source_bridge_config_hash
        .clone();
    let err = UpsertSccpRouteManifest::new(replayed_post_deploy_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("route canary evidence hash must not replay source bridge config hash");
    assert!(
        format!("{err:?}").contains(
            "post_deploy_route_canary_evidence_hash must not equal \
             post_deploy_source_bridge_config_hash"
        ),
        "unexpected error: {err:?}"
    );

    assert!(
        stx.zk.sccp_route_manifests.is_empty(),
        "rejected route manifests must not mutate state transaction"
    );
}

#[test]
fn production_ton_route_manifest_isi_accepts_and_rejects_foreign_payloads() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);

    let manifest = production_ton_route_manifest();
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("production TON route manifest should parse and insert");
    assert_eq!(stx.zk.sccp_route_manifests.len(), 1);
    assert_eq!(stx.zk.sccp_route_manifests[0].route_id, "taira_ton_xor");
    assert_eq!(
        stx.zk.sccp_route_manifests[0].counterparty_domain,
        iroha_sccp::SCCP_DOMAIN_TON
    );
    assert_eq!(
        stx.zk.sccp_route_manifests[0].chain_id_hex,
        "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd"
    );

    let mut wrong_route = manifest.clone();
    wrong_route.route_id = "taira_ton_usdt".to_owned();
    let err = UpsertSccpRouteManifest::new(wrong_route)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TON route must be taira_ton_xor/xor");
    assert!(
        format!("{err:?}").contains("taira_ton_xor"),
        "unexpected error: {err:?}"
    );

    let mut wrong_target = manifest;
    wrong_target.verifier_target = "EvmContract".to_owned();
    let err = UpsertSccpRouteManifest::new(wrong_target)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TON route must use TonContract");
    assert!(
        format!("{err:?}").contains("TonContract"),
        "unexpected error: {err:?}"
    );

    assert_eq!(
        stx.zk.sccp_route_manifests.len(),
        1,
        "rejected TON route manifests must not mutate state transaction"
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

    let mut padded_specifier = manifest.clone();
    padded_specifier
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .module_specifier = Some(" @sora/sccp-bsc-destination-prover ".to_owned());
    let err = UpsertSccpRouteManifest::new(padded_specifier)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("padded browser prover module specifiers must be rejected");
    assert!(
        format!("{err:?}").contains("module_specifier must be a non-empty canonical string"),
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

    let mut replayed_module_hash = manifest.clone();
    let replay_hash = replayed_module_hash.verifier_code_hash.clone();
    replayed_module_hash
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .module_hash = replay_hash;
    let err = UpsertSccpRouteManifest::new(replayed_module_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("browser prover module hash must not replay verifier code hash");
    assert!(
        format!("{err:?}")
            .contains("destination_browser_prover.module_hash must not equal verifier_code_hash"),
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

#[test]
fn production_tron_route_manifest_isi_enforces_mainnet_binding_without_mutating_state() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    let manifest = production_tron_route_manifest();
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("insert TRON route manifest");
    assert_eq!(stx.zk.sccp_route_manifests.len(), 1);
    assert_eq!(stx.zk.sccp_route_manifests[0].route_id, "taira_tron_xor");
    assert_eq!(
        stx.zk.sccp_route_manifests[0].counterparty_domain,
        iroha_sccp::SCCP_DOMAIN_TRON
    );

    let mut wrong_route = manifest.clone();
    wrong_route.route_id = "foreign_tron_xor".to_owned();
    let err = UpsertSccpRouteManifest::new(wrong_route)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TRON route must be taira_tron_xor/xor");
    assert!(
        format!("{err:?}").contains("production_ready requires route_id = taira_tron_xor"),
        "unexpected error: {err:?}"
    );

    let mut wrong_domain = manifest.clone();
    wrong_domain.counterparty_domain = 6;
    let err = UpsertSccpRouteManifest::new(wrong_domain)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TRON route must keep the TRON counterparty domain");
    assert!(
        format!("{err:?}").contains("production_ready requires counterparty_domain = 5"),
        "unexpected error: {err:?}"
    );

    let mut replayed_post_deploy_hash = manifest.clone();
    replayed_post_deploy_hash.post_deploy_route_canary_evidence_hash = replayed_post_deploy_hash
        .post_deploy_source_bridge_config_hash
        .clone();
    let err = UpsertSccpRouteManifest::new(replayed_post_deploy_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("TRON route canary evidence hash must not replay source bridge config hash");
    assert!(
        format!("{err:?}").contains(
            "post_deploy_route_canary_evidence_hash must not equal \
             post_deploy_source_bridge_config_hash"
        ),
        "unexpected error: {err:?}"
    );

    assert_eq!(
        stx.zk.sccp_route_manifests.len(),
        1,
        "rejected TRON route manifests must not replace the existing manifest"
    );
    assert_eq!(
        stx.zk.sccp_route_manifests[0].destination_binding_hash,
        manifest.destination_binding_hash
    );
}
