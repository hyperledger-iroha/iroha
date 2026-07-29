//! Source-contract checks for the unconditional mandatory-offline evaluator.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::str::FromStr as _;

use iroha_core::{offline_readiness::ensure_mandatory_offline_ready, zk};
use iroha_crypto::Hash;
use iroha_data_model::{
    asset::AssetDefinitionId,
    domain::DomainId,
    name::Name,
    offline::{
        KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1, KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
        KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4, KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
        KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2, KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
        OfflineActiveTransferVerifier, OfflineAuthenticatedArtifactSet, OfflineReadiness,
        OfflineStatus, OfflineVerifierId,
        kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4,
        kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4,
    },
};

#[test]
fn mandatory_offline_readiness_is_unconditional_and_complete() {
    let source = include_str!("../src/offline_readiness.rs");
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("production source");

    for required in [
        "pub fn evaluate_committed_mandatory_offline",
        "pub fn evaluate_staged_genesis_mandatory_offline",
        "fn evaluate_snapshot",
        "ensure_kagemusha_active_release_material_v4",
        "ensure_offline_device_attestation_policy_ready_v1",
        "world_has_offline_escrow_manager_permission",
        "ensure_confidential_transfer_v2_canonical_vk_box",
        "ensure_kagemusha_topup_shield_v2_canonical_vk_box",
        "ensure_confidential_unshield_v3_canonical_vk_box",
        "verifier_role_distinctness_blockers",
        "state_view.nexus.fees.fee_asset_id",
        "staged_genesis.nexus.fees.fee_asset_id",
        "peer_identity_unavailable",
        "peer_id.map_or_else(String::new, ToString::to_string)",
        "ensure_staged_genesis_headers_match(&header, &staged_genesis._curr_block)",
        "public_status_evidence_blockers",
        "public_asset_evidence_blockers",
        "offline_required_verifier_missing",
        "offline_authenticated_artifact_set_missing",
    ] {
        assert!(
            production.contains(required),
            "authoritative readiness omitted `{required}`"
        );
    }

    for forbidden in [
        "cfg(feature = \"app_api\")",
        "iroha_torii",
        "KagemushaReleaseCatalogV4::empty",
    ] {
        assert!(
            !production.contains(forbidden),
            "authoritative readiness contains forbidden `{forbidden}`"
        );
    }

    let policy_body = production
        .split("pub struct MandatoryOfflinePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl MandatoryOfflinePolicy").next())
        .expect("policy struct body");
    assert!(
        !policy_body.contains("KeyPair"),
        "readiness policy must never retain issuer private material"
    );
    assert!(
        production.matches("evaluate_snapshot(").count() >= 3,
        "both snapshot wrappers must call the single evaluator"
    );
}

#[test]
fn device_readiness_uses_the_release_activation_policy() {
    let source = include_str!("../src/smartcontracts/isi/offline.rs");
    let device_readiness = source
        .split("pub fn ensure_offline_device_attestation_policy_ready_v1")
        .nth(1)
        .and_then(|tail| tail.split("/// Derive the canonical").next())
        .expect("device readiness function body");

    assert!(
        device_readiness.contains("validate_offline_attestation_policy_for_release_activation"),
        "device readiness must enforce the production iOS and Android release policy"
    );

    let release_validation = source
        .split("fn validate_offline_attestation_policy_for_release_activation")
        .nth(1)
        .and_then(|tail| tail.split("fn trusted_root_der_for_platform").next())
        .expect("release activation policy validation body");
    assert!(
        release_validation.contains("trusted_root_is_active")
            && release_validation.contains("revoked_certificate_hashes")
            && release_validation.contains("sha256_bytes(&root.der)"),
        "release readiness must require active, non-revoked platform roots"
    );
}

#[test]
fn empty_catalog_cannot_be_reported_ready() {
    let status = OfflineStatus {
        mandatory: true,
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        ready: true,
        assets: Vec::new(),
        blockers: Vec::new(),
    };

    let error = ensure_mandatory_offline_ready(&status)
        .expect_err("an empty mandatory catalog must remain unready");
    let blocker_codes = error
        .blockers()
        .iter()
        .map(|blocker| blocker.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        blocker_codes,
        vec!["offline_asset_catalog_empty", "offline_status_inconsistent"]
    );
}

fn exact_verifier(
    role: &str,
    circuit_id: &str,
    public_inputs_schema_hash: [u8; 32],
    commitment_byte: u8,
    max_proof_bytes: u32,
    withdrawal_height: Option<u64>,
) -> OfflineActiveTransferVerifier {
    OfflineActiveTransferVerifier {
        id: OfflineVerifierId {
            backend: zk::ZK_BACKEND_HALO2_IPA.to_owned(),
            name: role.to_owned(),
        },
        version: 1,
        circuit_id: circuit_id.to_owned(),
        commitment: hex::encode([commitment_byte; 32]),
        public_inputs_schema_hash: hex::encode(public_inputs_schema_hash),
        max_proof_bytes,
        activation_height: 1,
        withdrawal_height,
    }
}

fn complete_status() -> OfflineStatus {
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("offline", "test").expect("test domain"),
        Name::from_str("ds").expect("test asset name"),
    );
    let transfer_schema: [u8; 32] =
        Hash::new(zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1).into();
    let topup_schema: [u8; 32] =
        Hash::new(zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2).into();
    let unshield_schema: [u8; 32] =
        Hash::new(zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1).into();
    let recursive_max_proof_bytes = 1_024;
    let asset = OfflineReadiness {
        peer_id: "peer-1".to_owned(),
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        asset_definition_id: asset_definition_id.to_string(),
        asset_scale: Some(2),
        evaluated_block_height: 5,
        evaluated_block_hash: hex::encode([0xA0; 32]),
        active_transfer_verifier: Some(exact_verifier(
            KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
            zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            transfer_schema,
            1,
            1_024,
            None,
        )),
        active_topup_shield_verifier: Some(exact_verifier(
            KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
            zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
            topup_schema,
            2,
            1_024,
            None,
        )),
        active_unshield_verifier: Some(exact_verifier(
            KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
            zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            unshield_schema,
            3,
            1_024,
            None,
        )),
        active_recursive_step_eq_verifier: Some(exact_verifier(
            KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4(),
            4,
            recursive_max_proof_bytes,
            Some(10),
        )),
        active_recursive_step_ep_verifier: Some(exact_verifier(
            KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4(),
            5,
            recursive_max_proof_bytes,
            Some(10),
        )),
        artifact_set: Some(OfflineAuthenticatedArtifactSet {
            generation: "release-1".to_owned(),
            manifest_sha256: hex::encode([0x10; 32]),
            release_policy_sha256: hex::encode([0x11; 32]),
            release_attestation_sha256: hex::encode([0x12; 32]),
            activation_height: 1,
            withdrawal_height: 10,
            max_proof_bytes: recursive_max_proof_bytes,
            asset_scale: 2,
        }),
        proof_backend_available: true,
        recursive_lineage_supported: true,
        ready: true,
        blockers: Vec::new(),
    };
    OfflineStatus {
        mandatory: true,
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        ready: true,
        assets: vec![asset],
        blockers: Vec::new(),
    }
}

#[test]
fn complete_public_abi21_v4_evidence_is_required() {
    let status = complete_status();
    ensure_mandatory_offline_ready(&status)
        .expect("complete canonical public readiness evidence must pass");

    let mut wrong_abi = status.clone();
    wrong_abi.required_bridge_abi_version -= 1;
    let error = ensure_mandatory_offline_ready(&wrong_abi)
        .expect_err("a substituted fleet ABI identity must reject");
    assert!(
        error
            .blockers()
            .iter()
            .any(|blocker| blocker.code == "offline_bridge_abi_version_mismatch")
    );

    let mut missing_verifier = status.clone();
    missing_verifier.assets[0].active_recursive_step_ep_verifier = None;
    let error = ensure_mandatory_offline_ready(&missing_verifier)
        .expect_err("fewer than five verifier roles must reject");
    assert!(
        error
            .blockers()
            .iter()
            .any(|blocker| blocker.code == "offline_required_verifier_missing")
    );

    let mut missing_artifact = status.clone();
    missing_artifact.assets[0].artifact_set = None;
    let error = ensure_mandatory_offline_ready(&missing_artifact)
        .expect_err("missing authenticated artifact identity must reject");
    assert!(
        error
            .blockers()
            .iter()
            .any(|blocker| { blocker.code == "offline_authenticated_artifact_set_missing" })
    );
}
