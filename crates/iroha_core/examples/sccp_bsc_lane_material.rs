//! Generate the governed SCCP BSC lane-material parameter payload.
//!
//! This is an operator helper: it does not sign or submit anything. It builds
//! the exact `SetParameter(Custom)` JSON accepted by `iroha_cli parameter set`
//! from live deployment/canary evidence and validates SCCP readiness locally.

use std::{collections::BTreeMap, env, fs, process};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::parameters::actual::{
    SccpDestinationRollout, SccpRouteAllowlist, SccpSourceAdapterEngineDeployment,
    SccpSourceVerifierMaterial,
};
use iroha_data_model::{
    isi::{InstructionBox, SetParameter},
    parameter::{
        Parameter,
        custom::{CustomParameter, CustomParameterId},
    },
};
use iroha_primitives::json::Json;
use iroha_sccp::{
    NexusSccpMessageProofV1, SCCP_DOMAIN_BSC, SCCP_DOMAIN_SORA, SccpDestinationRolloutV1,
    SccpRouteAllowlistReadinessV1, SccpSourceAdapterEngineDeploymentV1,
    SccpSourceVerifierMaterialV1, build_sccp_bsc_mainnet_source_adapter_deployment,
    sccp_bsc_source_verifier_material_with_hashes_emitter_and_config_v1,
    sccp_evm_family_mainnet_source_verifier_material_v1,
    sccp_evm_mainnet_destination_rollout_with_binding_v1,
    sccp_evm_route_allowlist_with_lane_canary_evidence_v1,
    sccp_lane_production_readiness_with_deployment_materials_for_domain,
    sccp_profiled_route_allowlist_for_lane_evidence_v1, sccp_source_adapter_engine_deployment_hash,
    sccp_source_verifier_material_from_message_bundle_evidence, sccp_source_verifier_material_hash,
    sccp_source_verifier_material_is_production_ready,
};

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct SccpOnChainLaneMaterialsV1 {
    version: u8,
    sccp_source_verifier_materials: Vec<SccpSourceVerifierMaterial>,
    sccp_source_adapter_engine_deployments: Vec<SccpSourceAdapterEngineDeployment>,
    sccp_destination_rollouts: Vec<SccpDestinationRollout>,
    sccp_route_allowlists: Vec<SccpRouteAllowlist>,
}

fn usage() -> ! {
    eprintln!(
        "usage: cargo run -p iroha_core --example sccp_bsc_lane_material -- \\
  --source-material-bundle path/to/bsc-return-bundle.json \\
  --deployment-receipt-hash 0x... \\
  --verifier-address 0x... \\
  --verifier-code-hash 0x... \\
  --verifier-key-hash 0x... \\
  --destination-network-id 0x... \\
  --destination-bridge-address 0x... \\
  --canary-tx-hash 0x... \\
  --canary-log-index 1 \\
  --canary-receipt-block-number 123 \\
  --canary-receipt-block-hash 0x... \\
  --canary-block-receipts-root 0x... \\
  --canary-call-data-sha256 0x... \\
  --canary-message-id 0x... \\
  --canary-payload-hash 0x... \\
  --canary-statement-hash 0x... \\
  --canary-commitment-root 0x... \\
  --canary-finality-height 0x... \\
  --canary-finality-block-hash 0x... \\
  [--canary-target-domain 2] \\
  [--canary-proof-version 1] \\
  [--canary-proof-source-domain 0] \\
  [--summary]

fallback without --source-material-bundle also accepts:
  --source-bridge-emitter-address 0x... \\
  --source-bridge-emitter-code-hash 0x... \\
  --source-bridge-network-id 0x... \\
  --source-bridge-owner-address 0x... \\
  [--source-bridge-config-hash 0x...] \\
  --source-trust-anchor-hash 0x... \\
  --consensus-verifier-hash 0x... \\
  --message-inclusion-verifier-hash 0x... \\
  --finality-policy-hash 0x...

flags:
  --parameter-json-in path/to/set-parameter-custom.json
  --summary
  --instruction-base64   emit the canonical SetParameter(Custom) Norito instruction as base64"
    );
    process::exit(2);
}

fn parse_args() -> BTreeMap<String, String> {
    let mut args = env::args().skip(1);
    let mut out = BTreeMap::new();
    while let Some(arg) = args.next() {
        if arg == "--summary" || arg == "--instruction-base64" {
            out.insert(arg, String::from("true"));
            continue;
        }
        let Some(key) = arg.strip_prefix("--") else {
            usage();
        };
        let Some(value) = args.next() else {
            usage();
        };
        out.insert(key.to_owned(), value);
    }
    out
}

fn print_set_parameter_instruction_base64(parameter: Parameter) {
    let instruction: InstructionBox = SetParameter::new(parameter).into();
    let bytes = norito::to_bytes(&instruction)
        .expect("SCCP lane-material SetParameter instruction must encode");
    println!("{}", BASE64_STANDARD.encode(bytes));
}

fn required(args: &BTreeMap<String, String>, key: &str) -> String {
    args.get(key).cloned().unwrap_or_else(|| {
        eprintln!("missing required argument --{key}");
        usage();
    })
}

fn optional_u32(args: &BTreeMap<String, String>, key: &str, default: u32) -> u32 {
    args.get(key)
        .map_or(Ok(default), |value| value.parse::<u32>())
        .unwrap_or_else(|_| {
            eprintln!("--{key} must be an unsigned integer");
            process::exit(2);
        })
}

fn required_u32(args: &BTreeMap<String, String>, key: &str) -> u32 {
    required(args, key).parse::<u32>().unwrap_or_else(|_| {
        eprintln!("--{key} must be an unsigned integer");
        process::exit(2);
    })
}

fn required_u64(args: &BTreeMap<String, String>, key: &str) -> u64 {
    required(args, key).parse::<u64>().unwrap_or_else(|_| {
        eprintln!("--{key} must be an unsigned integer");
        process::exit(2);
    })
}

fn decode_hex<const N: usize>(value: &str, key: &str) -> [u8; N] {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if raw.len() != N * 2
        || !raw
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
    {
        eprintln!("--{key} must be canonical lowercase {N}-byte hex");
        process::exit(2);
    }
    let mut out = [0u8; N];
    hex::decode_to_slice(raw, &mut out).unwrap_or_else(|_| {
        eprintln!("--{key} must be valid hex");
        process::exit(2);
    });
    out
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("write to String");
    }
    out
}

fn h256(value: &[u8; 32]) -> String {
    encode_hex(value)
}

fn bytes_hex(value: &[u8]) -> String {
    if value.is_empty() {
        String::new()
    } else {
        encode_hex(value)
    }
}

fn material_to_config(material: &SccpSourceVerifierMaterialV1) -> SccpSourceVerifierMaterial {
    SccpSourceVerifierMaterial {
        version: material.version,
        source_domain: material.source_domain,
        source_chain: material.source_chain.clone(),
        source_proof_plan: material.source_proof_plan.as_str().to_owned(),
        finality_model: material.finality_model.as_str().to_owned(),
        adapter_circuit_id: material.adapter_circuit_id.clone(),
        source_trust_anchor_id: material.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: h256(&material.source_trust_anchor_hash),
        consensus_verifier_id: material.consensus_verifier_id.clone(),
        consensus_verifier_hash: h256(&material.consensus_verifier_hash),
        message_inclusion_verifier_id: material.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: h256(&material.message_inclusion_verifier_hash),
        source_state_verifier_id: material.source_state_verifier_id.clone(),
        source_state_verifier_hash: h256(&material.source_state_verifier_hash),
        source_bridge_emitter_id: material.source_bridge_emitter_id.clone(),
        source_bridge_emitter_address: bytes_hex(&material.source_bridge_emitter_address),
        source_bridge_emitter_code_hash: h256(&material.source_bridge_emitter_code_hash),
        source_bridge_network_id: h256(&material.source_bridge_network_id),
        source_bridge_owner_address: bytes_hex(&material.source_bridge_owner_address),
        source_bridge_config_hash: h256(&material.source_bridge_config_hash),
        finality_policy_id: material.finality_policy_id.clone(),
        finality_policy_hash: h256(&material.finality_policy_hash),
        placeholder_material: material.placeholder_material,
    }
}

fn deployment_to_config(
    deployment: &SccpSourceAdapterEngineDeploymentV1,
) -> SccpSourceAdapterEngineDeployment {
    SccpSourceAdapterEngineDeployment {
        version: deployment.version,
        source_domain: deployment.source_domain,
        target_domain: deployment.target_domain,
        source_chain: deployment.source_chain.clone(),
        source_proof_plan: deployment.source_proof_plan.as_str().to_owned(),
        finality_model: deployment.finality_model.as_str().to_owned(),
        adapter_proof_family: deployment.adapter_proof_family.clone(),
        adapter_circuit_id: deployment.adapter_circuit_id.clone(),
        adapter_verifier_vk_hash: h256(&deployment.adapter_verifier_vk_hash),
        source_trust_anchor_id: deployment.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: h256(&deployment.source_trust_anchor_hash),
        consensus_verifier_id: deployment.consensus_verifier_id.clone(),
        consensus_verifier_hash: h256(&deployment.consensus_verifier_hash),
        message_inclusion_verifier_id: deployment.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: h256(&deployment.message_inclusion_verifier_hash),
        source_state_verifier_id: deployment.source_state_verifier_id.clone(),
        source_state_verifier_hash: h256(&deployment.source_state_verifier_hash),
        source_bridge_emitter_id: deployment.source_bridge_emitter_id.clone(),
        source_bridge_emitter_address: bytes_hex(&deployment.source_bridge_emitter_address),
        source_bridge_emitter_code_hash: h256(&deployment.source_bridge_emitter_code_hash),
        source_bridge_network_id: h256(&deployment.source_bridge_network_id),
        source_bridge_owner_address: bytes_hex(&deployment.source_bridge_owner_address),
        source_bridge_config_hash: h256(&deployment.source_bridge_config_hash),
        finality_policy_id: deployment.finality_policy_id.clone(),
        finality_policy_hash: h256(&deployment.finality_policy_hash),
        deployment_receipt_hash: h256(&deployment.deployment_receipt_hash),
        solana_tower_replay_verifier_hash: String::new(),
        solana_full_accountsdb_lattice_verifier_hash: String::new(),
        solana_bank_fork_choice_verifier_hash: String::new(),
        solana_full_light_client_gate_hash: String::new(),
        ton_masterchain_config_verifier_hash: String::new(),
        ton_validator_set_transition_verifier_hash: String::new(),
        ton_shard_accounts_dictionary_verifier_hash: String::new(),
        ton_full_light_client_gate_hash: String::new(),
        tron_dpos_source_gate_hash: String::new(),
    }
}

fn rollout_to_config(rollout: &SccpDestinationRolloutV1) -> SccpDestinationRollout {
    SccpDestinationRollout {
        version: rollout.version,
        domain: rollout.domain,
        chain: rollout.chain.clone(),
        verifier_plan: rollout.verifier_plan.as_str().to_owned(),
        immutable_verifier_ready: rollout.immutable_verifier_ready,
        anchors_ready: rollout.anchors_ready,
        verifier_identity: rollout.verifier_identity.clone(),
        verifier_code_hash: rollout.verifier_code_hash.clone(),
        verifier_key_hash: rollout.verifier_key_hash.clone(),
        destination_network_id: rollout.destination_network_id.clone(),
        destination_bridge_address: rollout.destination_bridge_address.clone(),
        destination_binding_key: rollout.destination_binding_key.clone(),
        destination_binding_hash: rollout.destination_binding_hash.clone(),
        anchor_id: rollout.anchor_id.clone(),
        solana_rpc_commitment: None,
        solana_program_owner: None,
        solana_programdata_owner: None,
        solana_program_immutable: None,
        solana_program_account_data_base64: None,
        solana_programdata_address: None,
        solana_programdata_slot: None,
        solana_expected_programdata_slot: None,
        solana_program_account_context_slot: None,
        solana_programdata_account_context_slot: None,
        solana_programdata_metadata_blake2b256: None,
        solana_programdata_metadata_base64: None,
        solana_programdata_executable_blake2b256: None,
        solana_programdata_executable_base64: None,
        ton_account_status: None,
        ton_account_state_hash: None,
        ton_last_transaction_lt: None,
        ton_last_transaction_hash: None,
        ton_verifier_code_boc_root_hash: None,
        ton_verifier_code_boc: None,
        blockers: rollout.blockers.clone(),
    }
}

fn allowlist_to_config(allowlist: &SccpRouteAllowlistReadinessV1) -> SccpRouteAllowlist {
    SccpRouteAllowlist {
        version: allowlist.version,
        domain: allowlist.domain,
        chain: allowlist.chain.clone(),
        activation_policy: allowlist.activation_policy.as_str().to_owned(),
        route_allowlist_id: allowlist.route_allowlist_id.clone(),
        route_allowlist_hash: allowlist.route_allowlist_hash.clone(),
        route_canary_status: allowlist.route_canary_status.clone(),
        route_canary_evidence_hash: allowlist.route_canary_evidence_hash.clone(),
        route_canary_route_allowlist_hash: allowlist.route_canary_route_allowlist_hash.clone(),
        route_canary_destination_binding_hash: allowlist
            .route_canary_destination_binding_hash
            .clone(),
        evm_route_canary_transaction_hash: allowlist.evm_route_canary_transaction_hash.clone(),
        evm_route_canary_log_index: allowlist.evm_route_canary_log_index,
        evm_route_canary_receipt_block_number: allowlist.evm_route_canary_receipt_block_number,
        evm_route_canary_receipt_block_hash: allowlist.evm_route_canary_receipt_block_hash.clone(),
        evm_route_canary_receipt_block_finalized: allowlist
            .evm_route_canary_receipt_block_finalized,
        evm_route_canary_block_receipts_root: allowlist
            .evm_route_canary_block_receipts_root
            .clone(),
        evm_route_canary_call_data_sha256: allowlist.evm_route_canary_call_data_sha256.clone(),
        evm_route_canary_message_id: allowlist.evm_route_canary_message_id.clone(),
        evm_route_canary_payload_hash: allowlist.evm_route_canary_payload_hash.clone(),
        evm_route_canary_target_domain: allowlist.evm_route_canary_target_domain,
        evm_route_canary_statement_hash: allowlist.evm_route_canary_statement_hash.clone(),
        evm_route_canary_commitment_root: allowlist.evm_route_canary_commitment_root.clone(),
        evm_route_canary_finality_height: allowlist.evm_route_canary_finality_height.clone(),
        evm_route_canary_finality_block_hash: allowlist
            .evm_route_canary_finality_block_hash
            .clone(),
        evm_route_canary_proof_version: allowlist.evm_route_canary_proof_version,
        evm_route_canary_proof_source_domain: allowlist.evm_route_canary_proof_source_domain,
        evm_route_canary_used_message_proof: allowlist.evm_route_canary_used_message_proof,
        tron_route_canary_transaction_id: None,
        tron_route_canary_transaction_owner_address: None,
        tron_route_canary_block_number: None,
        tron_route_canary_block_timestamp: None,
        tron_route_canary_log_index: None,
        tron_route_canary_message_id: None,
        tron_route_canary_call_data_sha256: None,
        tron_route_canary_payload_hash: None,
        tron_route_canary_target_domain: None,
        tron_route_canary_statement_hash: None,
        tron_route_canary_commitment_root: None,
        tron_route_canary_finality_height: None,
        tron_route_canary_finality_block_hash: None,
        tron_route_canary_proof_version: None,
        tron_route_canary_proof_source_domain: None,
        tron_route_canary_used_message_proof: None,
        tron_route_canary_raw_data_owner_matches_transaction: None,
        tron_route_canary_signature_sha256: None,
        tron_route_canary_signature_recovered_address: None,
        tron_route_canary_signature_recovers_to_owner: None,
        ton_route_canary_account_state_hash: None,
        ton_route_canary_last_transaction_lt: None,
        ton_route_canary_last_transaction_hash: None,
        routes_allowlisted: allowlist.routes_allowlisted,
        blockers: allowlist.blockers.clone(),
    }
}

fn material_from_bundle(path: &str) -> SccpSourceVerifierMaterialV1 {
    let raw = fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("failed to read --source-material-bundle {path}: {error}");
        process::exit(2);
    });
    let bundle: NexusSccpMessageProofV1 = norito::json::from_str(&raw).unwrap_or_else(|error| {
        eprintln!("failed to parse --source-material-bundle {path}: {error}");
        process::exit(2);
    });
    sccp_source_verifier_material_from_message_bundle_evidence(&bundle).unwrap_or_else(|| {
        eprintln!("--source-material-bundle {path} does not contain SCCP source material evidence");
        process::exit(2);
    })
}

fn maybe_emit_parameter_json_input(args: &BTreeMap<String, String>) -> bool {
    if let Some(path) = args.get("parameter-json-in") {
        let text = fs::read_to_string(path).unwrap_or_else(|error| {
            eprintln!("failed to read --parameter-json-in {path}: {error}");
            process::exit(2);
        });
        let parameter = norito::json::from_str::<Parameter>(&text).unwrap_or_else(|error| {
            eprintln!("failed to parse --parameter-json-in as Parameter JSON: {error}");
            process::exit(2);
        });
        if args.contains_key("--instruction-base64") {
            print_set_parameter_instruction_base64(parameter);
        } else {
            let parameter_json = norito::json::to_string_pretty(&parameter)
                .expect("SCCP lane-material parameter must serialize");
            println!("{parameter_json}");
        }
        return true;
    }

    false
}

fn has_explicit_source_material_hashes(args: &BTreeMap<String, String>) -> bool {
    args.contains_key("source-trust-anchor-hash")
        || args.contains_key("consensus-verifier-hash")
        || args.contains_key("message-inclusion-verifier-hash")
        || args.contains_key("finality-policy-hash")
}

fn explicit_source_material_from_args(
    args: &BTreeMap<String, String>,
) -> SccpSourceVerifierMaterialV1 {
    let material = sccp_bsc_source_verifier_material_with_hashes_emitter_and_config_v1(
        decode_hex::<32>(
            &required(args, "source-trust-anchor-hash"),
            "source-trust-anchor-hash",
        ),
        decode_hex::<32>(
            &required(args, "consensus-verifier-hash"),
            "consensus-verifier-hash",
        ),
        decode_hex::<32>(
            &required(args, "message-inclusion-verifier-hash"),
            "message-inclusion-verifier-hash",
        ),
        decode_hex::<32>(
            &required(args, "finality-policy-hash"),
            "finality-policy-hash",
        ),
        decode_hex::<20>(
            &required(args, "source-bridge-emitter-address"),
            "source-bridge-emitter-address",
        ),
        decode_hex::<32>(
            &required(args, "source-bridge-emitter-code-hash"),
            "source-bridge-emitter-code-hash",
        ),
        decode_hex::<32>(
            &required(args, "source-bridge-network-id"),
            "source-bridge-network-id",
        ),
        decode_hex::<20>(
            &required(args, "source-bridge-owner-address"),
            "source-bridge-owner-address",
        ),
    )
    .unwrap_or_else(|| {
        eprintln!("explicit BSC source verifier material is not production-ready");
        process::exit(1);
    });
    if let Some(expected_config_hash) = args.get("source-bridge-config-hash") {
        let expected_config_hash =
            decode_hex::<32>(expected_config_hash, "source-bridge-config-hash");
        if material.source_bridge_config_hash != expected_config_hash {
            eprintln!(
                "--source-bridge-config-hash does not match computed BSC source bridge config hash"
            );
            process::exit(1);
        }
    }
    material
}

fn template_source_material_from_args(
    args: &BTreeMap<String, String>,
) -> SccpSourceVerifierMaterialV1 {
    let mut material = sccp_evm_family_mainnet_source_verifier_material_v1(SCCP_DOMAIN_BSC)
        .expect("BSC source verifier material template");
    material.source_bridge_emitter_address = decode_hex::<20>(
        &required(args, "source-bridge-emitter-address"),
        "source-bridge-emitter-address",
    )
    .to_vec();
    material.source_bridge_emitter_code_hash = decode_hex::<32>(
        &required(args, "source-bridge-emitter-code-hash"),
        "source-bridge-emitter-code-hash",
    );
    material
}

fn source_material_from_args(args: &BTreeMap<String, String>) -> SccpSourceVerifierMaterialV1 {
    match args.get("source-material-bundle") {
        Some(path) => material_from_bundle(path),
        None if has_explicit_source_material_hashes(args) => {
            explicit_source_material_from_args(args)
        }
        None => template_source_material_from_args(args),
    }
}

fn checked_bsc_source_material_from_args(
    args: &BTreeMap<String, String>,
) -> SccpSourceVerifierMaterialV1 {
    let material = source_material_from_args(args);
    if material.source_domain != SCCP_DOMAIN_BSC
        || !sccp_source_verifier_material_is_production_ready(&material)
    {
        eprintln!("BSC source verifier material is not production-ready");
        process::exit(1);
    }
    material
}

fn bsc_source_adapter_deployment_from_args(
    args: &BTreeMap<String, String>,
    material: &SccpSourceVerifierMaterialV1,
) -> SccpSourceAdapterEngineDeploymentV1 {
    let deployment_receipt_hash = decode_hex::<32>(
        &required(args, "deployment-receipt-hash"),
        "deployment-receipt-hash",
    );
    build_sccp_bsc_mainnet_source_adapter_deployment(material, deployment_receipt_hash)
        .expect("BSC source adapter deployment material must be ready")
}

fn bsc_destination_rollout_from_args(args: &BTreeMap<String, String>) -> SccpDestinationRolloutV1 {
    sccp_evm_mainnet_destination_rollout_with_binding_v1(
        SCCP_DOMAIN_BSC,
        required(args, "verifier-address"),
        required(args, "verifier-code-hash"),
        required(args, "verifier-key-hash"),
        required(args, "destination-network-id"),
        required(args, "destination-bridge-address"),
    )
    .expect("BSC destination rollout must be production-ready")
}

fn rollout_destination_binding_hash(rollout: &SccpDestinationRolloutV1) -> [u8; 32] {
    decode_hex::<32>(
        rollout
            .destination_binding_hash
            .as_deref()
            .expect("destination binding hash"),
        "destination-binding-hash",
    )
}

fn bsc_allowlist_from_args(
    args: &BTreeMap<String, String>,
    material: &SccpSourceVerifierMaterialV1,
    deployment: &SccpSourceAdapterEngineDeploymentV1,
    rollout: &SccpDestinationRolloutV1,
    destination_binding_hash: [u8; 32],
    source_material_hash: [u8; 32],
    source_deployment_hash: [u8; 32],
) -> SccpRouteAllowlistReadinessV1 {
    let allowlist = sccp_profiled_route_allowlist_for_lane_evidence_v1(
        SCCP_DOMAIN_BSC,
        material,
        deployment,
        rollout,
    )
    .expect("BSC route allowlist lane evidence must be ready");
    sccp_evm_route_allowlist_with_lane_canary_evidence_v1(
        allowlist,
        rollout,
        destination_binding_hash,
        source_material_hash,
        source_deployment_hash,
        decode_hex::<32>(&required(args, "canary-tx-hash"), "canary-tx-hash"),
        required_u32(args, "canary-log-index"),
        required_u64(args, "canary-receipt-block-number"),
        decode_hex::<32>(
            &required(args, "canary-receipt-block-hash"),
            "canary-receipt-block-hash",
        ),
        true,
        decode_hex::<32>(
            &required(args, "canary-block-receipts-root"),
            "canary-block-receipts-root",
        ),
        decode_hex::<32>(
            &required(args, "canary-call-data-sha256"),
            "canary-call-data-sha256",
        ),
        decode_hex::<32>(&required(args, "canary-message-id"), "canary-message-id"),
        decode_hex::<32>(
            &required(args, "canary-payload-hash"),
            "canary-payload-hash",
        ),
        optional_u32(args, "canary-target-domain", SCCP_DOMAIN_BSC),
        decode_hex::<32>(
            &required(args, "canary-statement-hash"),
            "canary-statement-hash",
        ),
        decode_hex::<32>(
            &required(args, "canary-commitment-root"),
            "canary-commitment-root",
        ),
        decode_hex::<32>(
            &required(args, "canary-finality-height"),
            "canary-finality-height",
        ),
        decode_hex::<32>(
            &required(args, "canary-finality-block-hash"),
            "canary-finality-block-hash",
        ),
        optional_u32(args, "canary-proof-version", 1),
        optional_u32(args, "canary-proof-source-domain", SCCP_DOMAIN_SORA),
        true,
    )
    .expect("BSC route canary evidence must bind to lane materials")
}

fn ensure_bsc_lane_readiness(
    material: &SccpSourceVerifierMaterialV1,
    deployment: &SccpSourceAdapterEngineDeploymentV1,
    rollout: &SccpDestinationRolloutV1,
    allowlist: &SccpRouteAllowlistReadinessV1,
) {
    let readiness = sccp_lane_production_readiness_with_deployment_materials_for_domain(
        SCCP_DOMAIN_BSC,
        material,
        deployment,
        rollout,
        allowlist,
    )
    .expect("BSC lane readiness must be derivable");
    if !readiness.production_ready {
        eprintln!("BSC lane is not production-ready: {:?}", readiness.blockers);
        process::exit(1);
    }
}

fn lane_payload(
    material: &SccpSourceVerifierMaterialV1,
    deployment: &SccpSourceAdapterEngineDeploymentV1,
    rollout: &SccpDestinationRolloutV1,
    allowlist: &SccpRouteAllowlistReadinessV1,
) -> SccpOnChainLaneMaterialsV1 {
    SccpOnChainLaneMaterialsV1 {
        version: 1,
        sccp_source_verifier_materials: vec![material_to_config(material)],
        sccp_source_adapter_engine_deployments: vec![deployment_to_config(deployment)],
        sccp_destination_rollouts: vec![rollout_to_config(rollout)],
        sccp_route_allowlists: vec![allowlist_to_config(allowlist)],
    }
}

fn emit_summary(
    source_material_hash: &[u8; 32],
    source_deployment_hash: &[u8; 32],
    rollout: &SccpDestinationRolloutV1,
    allowlist: &SccpRouteAllowlistReadinessV1,
) {
    eprintln!("source_material_hash={}", h256(source_material_hash));
    eprintln!(
        "source_adapter_deployment_hash={}",
        h256(source_deployment_hash)
    );
    eprintln!(
        "destination_binding_hash={}",
        rollout
            .destination_binding_hash
            .as_deref()
            .unwrap_or_default()
    );
    eprintln!(
        "route_allowlist_hash={}",
        allowlist
            .route_allowlist_hash
            .as_deref()
            .unwrap_or_default()
    );
    eprintln!(
        "route_canary_evidence_hash={}",
        allowlist
            .route_canary_evidence_hash
            .as_deref()
            .unwrap_or_default()
    );
}

fn emit_lane_payload(args: &BTreeMap<String, String>, payload: SccpOnChainLaneMaterialsV1) {
    if args.contains_key("--instruction-base64") {
        let id = CustomParameterId::new(
            "sccp_lane_materials_v1"
                .parse()
                .expect("static custom parameter id must parse"),
        );
        let custom = CustomParameter::new(id, Json::new(payload));
        let instruction: InstructionBox = SetParameter::new(Parameter::Custom(custom)).into();
        let bytes = norito::to_bytes(&instruction)
            .expect("SCCP lane-material SetParameter instruction must encode");
        println!("{}", BASE64_STANDARD.encode(bytes));
    } else {
        let payload_json = norito::json::to_string_pretty(&payload)
            .expect("SCCP on-chain lane material payload must serialize");
        println!("{{\"Custom\":{{\"id\":\"sccp_lane_materials_v1\",\"payload\":{payload_json}}}}}");
    }
}

fn main() {
    let args = parse_args();
    if maybe_emit_parameter_json_input(&args) {
        return;
    }

    let material = checked_bsc_source_material_from_args(&args);
    let deployment = bsc_source_adapter_deployment_from_args(&args, &material);
    let rollout = bsc_destination_rollout_from_args(&args);
    let destination_binding_hash = rollout_destination_binding_hash(&rollout);
    let source_material_hash = sccp_source_verifier_material_hash(&material);
    let source_deployment_hash = sccp_source_adapter_engine_deployment_hash(&deployment);
    let allowlist = bsc_allowlist_from_args(
        &args,
        &material,
        &deployment,
        &rollout,
        destination_binding_hash,
        source_material_hash,
        source_deployment_hash,
    );
    ensure_bsc_lane_readiness(&material, &deployment, &rollout, &allowlist);

    if args.contains_key("--summary") {
        emit_summary(
            &source_material_hash,
            &source_deployment_hash,
            &rollout,
            &allowlist,
        );
    }

    let payload = lane_payload(&material, &deployment, &rollout, &allowlist);
    emit_lane_payload(&args, payload);
}
