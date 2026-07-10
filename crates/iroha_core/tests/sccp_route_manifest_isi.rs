#![doc = "SCCP route-manifest ISI execution tests."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        SccpGovernedLaneV1, SccpLaneActivationV1, SccpOnChainRegistryV1, State,
        ValidatedSccpRegistryV1,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    bridge::{
        BridgeNativeProofBackendV1, SccpEvmSourceEmitterV1, SccpLaneIdV1, SccpNativeTrustAnchorV1,
        SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
    },
    isi::{
        Grant,
        bridge::{
            RemoveSccpRouteManifest, SccpRouteBrowserProverManifestRef, SccpRouteManifest,
            UpsertSccpRouteManifest,
        },
    },
    permission::Permission,
};
use iroha_executor_data_model::permission::sccp::CanManageSccpGovernance;
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;

#[path = "common/world_fixture.rs"]
mod test_world;

fn hex32(byte: u8) -> String {
    format!("0x{}", hex::encode([byte; 32]))
}

fn canonical_json_hash(value: &Json) -> String {
    let value = norito::json::parse_value(value.get()).expect("fixture JSON parses");
    let canonical = Json::from_norito_value_ref(&value).expect("fixture JSON canonicalizes");
    format!(
        "0x{}",
        hex::encode(iroha_crypto::sha256(canonical.get().as_bytes()))
    )
}

fn staging_route(mut manifest: SccpRouteManifest) -> SccpRouteManifest {
    manifest.production_ready = false;
    manifest.disabled_reason = Some("awaiting governed lane activation".to_owned());
    if manifest.counterparty_domain == iroha_sccp::SCCP_DOMAIN_BSC {
        manifest.network = "bsc-testnet".to_owned();
        manifest.chain = "bsc-testnet".to_owned();
        manifest.chain_id_hex = "0x61".to_owned();
        manifest.explorer_url = Some("https://testnet.bscscan.com".to_owned());
        manifest.explorer_host = Some("testnet.bscscan.com".to_owned());
        if let Some(transaction_id) = manifest.post_deploy_source_event_transaction_id.as_deref() {
            manifest.post_deploy_source_event_explorer_url =
                Some(format!("https://testnet.bscscan.com/tx/{transaction_id}"));
        }
        if let Some(transaction_id) = manifest.post_deploy_route_canary_transaction_id.as_deref() {
            manifest.post_deploy_route_canary_explorer_url =
                Some(format!("https://testnet.bscscan.com/tx/{transaction_id}"));
        }
    }
    manifest
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
    let native_evm_prover_bundle = Json::new(norito::json!({
        "schema": "sccp-bsc-native-evm-prover-bundle/v1",
        "routeId": "taira_bsc_xor",
        "assetKey": "xor"
    }));
    let native_evm_prover_bundle_hash = canonical_json_hash(&native_evm_prover_bundle);
    SccpRouteManifest {
        version: 1,
        route_id: "taira_bsc_xor".to_owned(),
        asset_key: "xor".to_owned(),
        network: "bsc-mainnet".to_owned(),
        chain: "bsc-mainnet".to_owned(),
        chain_id_hex: "0x38".to_owned(),
        explorer_url: Some("https://bscscan.com".to_owned()),
        explorer_host: Some("bscscan.com".to_owned()),
        counterparty_account_codec: Some(2),
        counterparty_account_codec_key: Some("evm_address20".to_owned()),
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_BSC,
        verifier_target: "EvmContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: format!("0x{}", "61".repeat(32)),
        taira_xor_token_address: "0x1111111111111111111111111111111111111111".to_owned(),
        taira_xor_bridge_address: "0x2222222222222222222222222222222222222222".to_owned(),
        source_bridge_address: "0x3333333333333333333333333333333333333333".to_owned(),
        destination_verifier_address: "0x4444444444444444444444444444444444444444".to_owned(),
        ton_finalize_message_value_nano: None,
        verifier_code_hash: hex32(0x45),
        verifier_key_hash: hex32(0x46),
        proof_artifact_hash: Some(proof_artifact_hash.clone()),
        proving_key_hash: Some(hex32(0x55)),
        native_evm_prover_bundle_hash: Some(native_evm_prover_bundle_hash),
        native_evm_prover_bundle: Some(native_evm_prover_bundle),
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
        sora_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        sora_custody_account_id: ALICE_ID.to_string(),
        payload_amount_scale: 9,
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_identity_hash: Some(hex32(0x4a)),
        post_deploy_source_event_transaction_id: Some(source_event_transaction_id.clone()),
        post_deploy_source_event_explorer_url: Some(format!(
            "https://bscscan.com/tx/{source_event_transaction_id}"
        )),
        post_deploy_route_canary_evidence_hash: Some(hex32(0x4e)),
        post_deploy_route_canary_transaction_id: Some(route_canary_transaction_id.clone()),
        post_deploy_route_canary_explorer_url: Some(format!(
            "https://bscscan.com/tx/{route_canary_transaction_id}"
        )),
        post_deploy_offline_full_toml_sha256: Some(hex32(0x56)),
    }
}

fn ton_account36(seed: u8) -> String {
    format!("0:{}", hex::encode([seed; 32]))
}

fn production_ton_route_manifest() -> SccpRouteManifest {
    let destination_binding_hash =
        "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799".to_owned();
    let proof_artifact_hash = hex32(0xcc);
    SccpRouteManifest {
        version: 1,
        route_id: "taira_ton_xor".to_owned(),
        asset_key: "xor".to_owned(),
        network: "mainnet".to_owned(),
        chain: "ton-mainnet".to_owned(),
        chain_id_hex: "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff11"
            .to_owned(),
        explorer_url: Some("https://tonscan.org".to_owned()),
        explorer_host: Some("tonscan.org".to_owned()),
        counterparty_account_codec: Some(4),
        counterparty_account_codec_key: Some("ton_account36".to_owned()),
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_TON,
        verifier_target: "TonContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff11"
            .to_owned(),
        taira_xor_token_address: ton_account36(0x11),
        taira_xor_bridge_address: ton_account36(0x22),
        source_bridge_address: ton_account36(0x33),
        destination_verifier_address: ton_account36(0x44),
        ton_finalize_message_value_nano: Some("100000000".to_owned()),
        verifier_code_hash: hex32(0xca),
        verifier_key_hash: hex32(0xcb),
        proof_artifact_hash: Some(proof_artifact_hash.clone()),
        proving_key_hash: Some(hex32(0xcd)),
        native_evm_prover_bundle_hash: None,
        native_evm_prover_bundle: None,
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
        sora_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        sora_custody_account_id: ALICE_ID.to_string(),
        payload_amount_scale: 9,
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_identity_hash: Some(hex32(0xd2)),
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
        network: "mainnet".to_owned(),
        chain: "tron-mainnet".to_owned(),
        chain_id_hex: "0x2b6653dc".to_owned(),
        explorer_url: None,
        explorer_host: None,
        counterparty_account_codec: Some(iroha_sccp::SCCP_CODEC_TRON_ADDRESS21),
        counterparty_account_codec_key: Some("tron_address21".to_owned()),
        counterparty_domain: iroha_sccp::SCCP_DOMAIN_TRON,
        verifier_target: "TronContract".to_owned(),
        production_ready: true,
        disabled_reason: None,
        network_id_hex: network_id_hex.to_owned(),
        taira_xor_token_address: "TT1DaQcqzoJEzEaHDU8nsmiKtiyhXHaSKD".to_owned(),
        taira_xor_bridge_address: "TWvqVD8cuSTqisoDrPKfwkkrpAsziL3XFh".to_owned(),
        source_bridge_address: "TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned(),
        destination_verifier_address: verifier_address.to_owned(),
        ton_finalize_message_value_nano: None,
        verifier_code_hash,
        verifier_key_hash,
        proof_artifact_hash: None,
        proving_key_hash: None,
        native_evm_prover_bundle_hash: None,
        native_evm_prover_bundle: None,
        destination_browser_prover: None,
        source_browser_prover: None,
        deployment_evidence_sha256: None,
        destination_binding_key,
        destination_binding_hash:
            "0x4c5b208d148cee784d611f77434a7dfac6b22a37b86faf82063d371ba7d3a1bc".to_owned(),
        sora_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        sora_custody_account_id: ALICE_ID.to_string(),
        payload_amount_scale: 9,
        post_deploy_full_toml_ready: Some(true),
        post_deploy_source_identity_hash: Some(hex32(0xb1)),
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
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0)
}

fn grant_route_manifest_permission(stx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let permission = Permission::from(CanManageSccpGovernance);
    Grant::account_permission(permission, ALICE_ID.clone())
        .execute(&ALICE_ID.clone(), stx)
        .expect("grant route manifest permission");
}

fn bsc_testnet_lane_id() -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source: SccpNetworkV1::BscTestnet,
        target: SccpNetworkV1::SoraTaira,
    }
}

fn install_staged_bsc_native_lane(stx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let lane_id = bsc_testnet_lane_id();
    let registry = SccpOnChainRegistryV1 {
        version: 1,
        lanes: vec![SccpGovernedLaneV1 {
            lane_id,
            source_identity: SccpSourceIdentityV1 {
                lane: lane_id,
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: [0x33; 20],
                    runtime_code_hash: [0x34; 32],
                    route_config_hash: [0x35; 32],
                }),
            },
            activation: SccpLaneActivationV1::Staged,
            native_trust_anchor: Some(SccpNativeTrustAnchorV1 {
                backend: BridgeNativeProofBackendV1::BscParlia,
                anchor_hash: [0x36; 32],
            }),
            destination_rollout: None,
            route_allowlist: None,
            route_manifest: None,
        }],
    };
    stx.sccp_registry =
        ValidatedSccpRegistryV1::try_from_wire(registry).expect("staged typed BSC lane registry");
}

#[test]
fn sccp_route_manifest_isi_requires_permission_and_mutates_state_transaction() {
    let state = test_state();
    let mut block = state.block(test_header());
    let manifest = staging_route(production_bsc_route_manifest());

    {
        let mut denied_tx = block.transaction();
        let denied = UpsertSccpRouteManifest::new(manifest.clone())
            .execute(&ALICE_ID.clone(), &mut denied_tx)
            .expect_err("upsert must require CanManageSccpGovernance");
        assert!(
            format!("{denied:?}").contains("CanManageSccpGovernance"),
            "unexpected denial: {denied:?}"
        );
    }

    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    install_staged_bsc_native_lane(&mut stx);
    UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("upsert with permission");
    let installed = stx
        .sccp_registry
        .route_manifest_for_lane(bsc_testnet_lane_id())
        .expect("installed BSC route");
    assert_eq!(installed.route_id, "taira_bsc_xor");
    assert_eq!(installed.chain_id_hex, "0x61");

    let mut replacement = manifest.clone();
    replacement.taira_xor_bridge_address = "0x5555555555555555555555555555555555555555".to_owned();
    UpsertSccpRouteManifest::new(replacement)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("replace same route key");
    assert_eq!(
        stx.sccp_registry
            .route_manifest_for_lane(bsc_testnet_lane_id())
            .expect("replaced BSC route")
            .taira_xor_bridge_address,
        "0x5555555555555555555555555555555555555555"
    );

    let noncanonical = RemoveSccpRouteManifest::new(
        "taira_bsc_xor".to_owned(),
        "xor".to_owned(),
        iroha_sccp::SCCP_DOMAIN_BSC,
        "0X61".to_owned(),
    )
    .execute(&ALICE_ID.clone(), &mut stx)
    .expect_err("uppercase chain ids must not be normalized at consensus ingress");
    assert!(format!("{noncanonical:?}").contains("canonical lowercase"));
    assert!(
        stx.sccp_registry
            .route_manifest_for_lane(bsc_testnet_lane_id())
            .is_some()
    );

    RemoveSccpRouteManifest::new(
        "taira_bsc_xor".to_owned(),
        "xor".to_owned(),
        iroha_sccp::SCCP_DOMAIN_BSC,
        "0x61".to_owned(),
    )
    .execute(&ALICE_ID.clone(), &mut stx)
    .expect("remove with canonical chain id");
    assert!(
        stx.sccp_registry
            .route_manifest_for_lane(bsc_testnet_lane_id())
            .is_none()
    );
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
        format!("{err:?}").contains("taira_bsc_xor")
            || format!("{err:?}").contains("native_evm_prover_bundle"),
        "unexpected error: {err:?}"
    );

    let mut replayed_deployment_hash = production_bsc_route_manifest();
    replayed_deployment_hash.deployment_evidence_sha256 =
        Some(replayed_deployment_hash.verifier_code_hash.clone());
    let err = UpsertSccpRouteManifest::new(replayed_deployment_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("deployment evidence hash must not replay verifier code hash");
    assert!(
        format!("{err:?}").contains("deployment_evidence_sha256")
            && format!("{err:?}").contains("distinct from verifier_code_hash"),
        "unexpected error: {err:?}"
    );

    let mut replayed_post_deploy_hash = production_bsc_route_manifest();
    replayed_post_deploy_hash.post_deploy_route_canary_evidence_hash = replayed_post_deploy_hash
        .post_deploy_source_identity_hash
        .clone();
    let err = UpsertSccpRouteManifest::new(replayed_post_deploy_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("route canary evidence hash must not replay source identity hash");
    assert!(
        format!("{err:?}").contains("post_deploy_route_canary_evidence_hash")
            && format!("{err:?}").contains("post_deploy_source_identity_hash"),
        "unexpected error: {err:?}"
    );

    assert!(
        stx.sccp_registry.lanes().is_empty(),
        "rejected route manifests must not mutate state transaction"
    );
}

#[test]
fn production_ton_route_manifest_isi_requires_governed_native_lane_and_rejects_foreign_payloads() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);

    let manifest = production_ton_route_manifest();
    let missing_governance = UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TON route must be bound to an active governed native lane");
    assert!(
        format!("{missing_governance:?}").contains("no active exact lane"),
        "unexpected error: {missing_governance:?}"
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

    assert!(
        stx.sccp_registry.lanes().is_empty(),
        "rejected TON route manifests must not mutate state transaction"
    );
}

#[test]
fn sccp_route_manifest_remove_missing_target_errors_without_mutating_state() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    install_staged_bsc_native_lane(&mut stx);
    let manifest = staging_route(production_bsc_route_manifest());
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
        stx.sccp_registry
            .route_manifest_for_lane(bsc_testnet_lane_id())
            .expect("preserved BSC route")
            .taira_xor_bridge_address,
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

    let mut root_relative_url = manifest.clone();
    root_relative_url
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .module_url = "/sccp-bsc/prover.js".to_owned();
    let err = UpsertSccpRouteManifest::new(root_relative_url)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("root-relative browser prover URLs must be rejected");
    assert!(
        format!("{err:?}").contains("module_url must be HTTPS, loopback HTTP, or package-relative"),
        "unexpected error: {err:?}"
    );

    let mut internal_https_url = manifest.clone();
    internal_https_url
        .destination_browser_prover
        .as_mut()
        .expect("destination prover")
        .module_url = "https://localhost/sccp-bsc/prover.js".to_owned();
    let err = UpsertSccpRouteManifest::new(internal_https_url)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("non-public HTTPS browser prover URLs must be rejected");
    assert!(
        format!("{err:?}").contains("module_url HTTPS host must use public DNS"),
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
        format!("{err:?}").contains("destination_browser_prover")
            && format!("{err:?}").contains("distinct from verifier_code_hash"),
        "unexpected error: {err:?}"
    );

    assert!(
        stx.sccp_registry.lanes().is_empty(),
        "rejected browser prover material must not insert a manifest"
    );
}

#[test]
fn production_tron_route_manifest_isi_requires_governed_native_lane_and_rejects_drift() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_route_manifest_permission(&mut stx);
    let manifest = production_tron_route_manifest();
    let missing_governance = UpsertSccpRouteManifest::new(manifest.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TRON route must be bound to an active governed native lane");
    assert!(
        format!("{missing_governance:?}").contains("no active exact lane"),
        "unexpected error: {missing_governance:?}"
    );

    let mut wrong_route = manifest.clone();
    wrong_route.route_id = "foreign_tron_xor".to_owned();
    let err = UpsertSccpRouteManifest::new(wrong_route)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TRON route must be taira_tron_xor/xor");
    assert!(
        format!("{err:?}").contains("taira_tron_xor"),
        "unexpected error: {err:?}"
    );

    let mut wrong_domain = manifest.clone();
    wrong_domain.counterparty_domain = 6;
    let err = UpsertSccpRouteManifest::new(wrong_domain)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("production TRON route must keep the TRON counterparty domain");
    assert!(
        format!("{err:?}").contains("counterparty_domain"),
        "unexpected error: {err:?}"
    );

    let mut replayed_post_deploy_hash = manifest.clone();
    replayed_post_deploy_hash.post_deploy_route_canary_evidence_hash = replayed_post_deploy_hash
        .post_deploy_source_identity_hash
        .clone();
    let err = UpsertSccpRouteManifest::new(replayed_post_deploy_hash)
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect_err("TRON route canary evidence hash must not replay source identity hash");
    assert!(
        format!("{err:?}").contains("post_deploy_route_canary_evidence_hash")
            && format!("{err:?}").contains("post_deploy_source_identity_hash"),
        "unexpected error: {err:?}"
    );

    assert!(
        stx.sccp_registry.lanes().is_empty(),
        "rejected TRON route manifests must not mutate state"
    );
}
