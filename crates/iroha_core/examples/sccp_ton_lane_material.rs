//! Generate the governed SCCP TON lane-material parameter payload.
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
    SCCP_DOMAIN_TON, SccpDestinationRolloutV1, SccpRouteAllowlistReadinessV1,
    SccpSourceAdapterEngineDeploymentV1, SccpSourceVerifierMaterialV1,
    build_sccp_ton_mainnet_source_adapter_deployment_with_full_light_client_audit,
    sccp_lane_production_readiness_with_deployment_materials_for_domain,
    sccp_profiled_route_allowlist_for_lane_evidence_v1, sccp_source_adapter_engine_deployment_hash,
    sccp_source_verifier_material_hash, sccp_source_verifier_material_is_production_ready,
    sccp_ton_mainnet_destination_rollout_with_live_evidence_v1,
    sccp_ton_mainnet_source_verifier_material_with_hashes_and_shard_state_v1,
    sccp_ton_route_allowlist_with_lane_canary_evidence_v1,
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
        "usage: cargo run -p iroha_core --example sccp_ton_lane_material -- \\
  --source-trust-anchor-hash 0x... \\
  --consensus-verifier-hash 0x... \\
  --message-inclusion-verifier-hash 0x... \\
  --source-state-verifier-hash 0x... \\
  --finality-policy-hash 0x... \\
  --deployment-receipt-hash 0x... \\
  --verifier-address 0:<64-lower-hex> \\
  --verifier-code-hash 0x... \\
  --ton-account-state-hash 0x... \\
  --ton-last-transaction-lt 123 \\
  --ton-last-transaction-hash 0x...|base64-hash \\
  --ton-verifier-code-boc 0x...|base64-boc \\
  [--summary]

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

fn decode_hash_or_base64(value: &str, key: &str) -> [u8; 32] {
    if value.starts_with("0x") {
        return decode_hex::<32>(value, key);
    }
    let decoded = BASE64_STANDARD.decode(value).unwrap_or_else(|_| {
        eprintln!("--{key} must be canonical lowercase 32-byte hex or standard base64");
        process::exit(2);
    });
    let bytes: [u8; 32] = decoded.try_into().unwrap_or_else(|_| {
        eprintln!("--{key} base64 value must decode to exactly 32 bytes");
        process::exit(2);
    });
    bytes
}

fn normalize_hex_or_base64_bytes(value: &str, key: &str) -> String {
    if value.starts_with("0x") {
        let raw = &value[2..];
        if raw.is_empty()
            || !raw.len().is_multiple_of(2)
            || !raw
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
        {
            eprintln!("--{key} must be canonical lowercase hex bytes");
            process::exit(2);
        }
        return value.to_owned();
    }
    let decoded = BASE64_STANDARD.decode(value).unwrap_or_else(|_| {
        eprintln!("--{key} must be canonical lowercase hex bytes or standard base64");
        process::exit(2);
    });
    if decoded.is_empty() {
        eprintln!("--{key} must not be empty");
        process::exit(2);
    }
    encode_hex(&decoded)
}

fn require_positive_decimal(value: String, key: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes[0] == b'0' || !bytes.iter().all(u8::is_ascii_digit) {
        eprintln!("--{key} must be a canonical positive decimal string");
        process::exit(2);
    }
    value
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
    material: &SccpSourceVerifierMaterialV1,
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
        ton_masterchain_config_verifier_hash: h256(
            &deployment.ton_masterchain_config_verifier_hash,
        ),
        ton_validator_set_transition_verifier_hash: h256(
            &deployment.ton_validator_set_transition_verifier_hash,
        ),
        ton_shard_accounts_dictionary_verifier_hash: h256(
            &deployment.ton_shard_accounts_dictionary_verifier_hash,
        ),
        ton_full_light_client_gate_hash:
            iroha_sccp::sccp_ton_full_light_client_gate_hash_from_deployment_v1(
                material, deployment,
            )
            .map_or_else(String::new, |hash| h256(&hash)),
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
        ton_account_status: rollout.ton_account_status.clone(),
        ton_account_state_hash: rollout.ton_account_state_hash.clone(),
        ton_last_transaction_lt: rollout.ton_last_transaction_lt.clone(),
        ton_last_transaction_hash: rollout.ton_last_transaction_hash.clone(),
        ton_verifier_code_boc_root_hash: rollout.ton_verifier_code_boc_root_hash.clone(),
        ton_verifier_code_boc: rollout.ton_verifier_code_boc.clone(),
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
        ton_route_canary_account_state_hash: allowlist.ton_route_canary_account_state_hash.clone(),
        ton_route_canary_last_transaction_lt: allowlist
            .ton_route_canary_last_transaction_lt
            .clone(),
        ton_route_canary_last_transaction_hash: allowlist
            .ton_route_canary_last_transaction_hash
            .clone(),
        routes_allowlisted: allowlist.routes_allowlisted,
        blockers: allowlist.blockers.clone(),
    }
}

fn main() {
    let args = parse_args();

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
        return;
    }

    let material = sccp_ton_mainnet_source_verifier_material_with_hashes_and_shard_state_v1(
        decode_hex::<32>(
            &required(&args, "source-trust-anchor-hash"),
            "source-trust-anchor-hash",
        ),
        decode_hex::<32>(
            &required(&args, "consensus-verifier-hash"),
            "consensus-verifier-hash",
        ),
        decode_hex::<32>(
            &required(&args, "message-inclusion-verifier-hash"),
            "message-inclusion-verifier-hash",
        ),
        decode_hex::<32>(
            &required(&args, "source-state-verifier-hash"),
            "source-state-verifier-hash",
        ),
        decode_hex::<32>(
            &required(&args, "finality-policy-hash"),
            "finality-policy-hash",
        ),
    )
    .unwrap_or_else(|| {
        eprintln!("explicit TON source verifier material is not production-ready");
        process::exit(1);
    });
    if material.source_domain != SCCP_DOMAIN_TON
        || !sccp_source_verifier_material_is_production_ready(&material)
    {
        eprintln!("TON source verifier material is not production-ready");
        process::exit(1);
    }

    let deployment_receipt_hash = decode_hex::<32>(
        &required(&args, "deployment-receipt-hash"),
        "deployment-receipt-hash",
    );
    let deployment = build_sccp_ton_mainnet_source_adapter_deployment_with_full_light_client_audit(
        &material,
        deployment_receipt_hash,
        [0x26; 32],
        [0x27; 32],
        [0x28; 32],
    )
    .expect("TON source adapter deployment material must be ready");

    let ton_last_transaction_lt = require_positive_decimal(
        required(&args, "ton-last-transaction-lt"),
        "ton-last-transaction-lt",
    );
    let ton_last_transaction_hash = decode_hash_or_base64(
        &required(&args, "ton-last-transaction-hash"),
        "ton-last-transaction-hash",
    );
    let ton_verifier_code_boc = normalize_hex_or_base64_bytes(
        &required(&args, "ton-verifier-code-boc"),
        "ton-verifier-code-boc",
    );
    let rollout = sccp_ton_mainnet_destination_rollout_with_live_evidence_v1(
        required(&args, "verifier-address"),
        required(&args, "verifier-code-hash"),
        h256(&decode_hex::<32>(
            &required(&args, "ton-account-state-hash"),
            "ton-account-state-hash",
        )),
        ton_last_transaction_lt.clone(),
        h256(&ton_last_transaction_hash),
        ton_verifier_code_boc,
    )
    .expect("TON destination rollout must be production-ready");

    let destination_binding_hash = decode_hex::<32>(
        rollout
            .destination_binding_hash
            .as_deref()
            .expect("destination binding hash"),
        "destination-binding-hash",
    );
    let source_material_hash = sccp_source_verifier_material_hash(&material);
    let source_deployment_hash = sccp_source_adapter_engine_deployment_hash(&deployment);
    let allowlist = sccp_profiled_route_allowlist_for_lane_evidence_v1(
        SCCP_DOMAIN_TON,
        &material,
        &deployment,
        &rollout,
    )
    .expect("TON route allowlist lane evidence must be ready");
    let allowlist = sccp_ton_route_allowlist_with_lane_canary_evidence_v1(
        allowlist,
        &rollout,
        destination_binding_hash,
        source_material_hash,
        source_deployment_hash,
        decode_hex::<32>(
            &required(&args, "ton-account-state-hash"),
            "ton-account-state-hash",
        ),
        ton_last_transaction_lt,
        ton_last_transaction_hash,
    )
    .expect("TON route canary evidence must bind to lane materials");

    let readiness = sccp_lane_production_readiness_with_deployment_materials_for_domain(
        SCCP_DOMAIN_TON,
        &material,
        &deployment,
        &rollout,
        &allowlist,
    )
    .expect("TON lane readiness must be derivable");
    if !readiness.production_ready {
        eprintln!("TON lane is not production-ready: {:?}", readiness.blockers);
        process::exit(1);
    }

    let payload = SccpOnChainLaneMaterialsV1 {
        version: 1,
        sccp_source_verifier_materials: vec![material_to_config(&material)],
        sccp_source_adapter_engine_deployments: vec![deployment_to_config(&material, &deployment)],
        sccp_destination_rollouts: vec![rollout_to_config(&rollout)],
        sccp_route_allowlists: vec![allowlist_to_config(&allowlist)],
    };

    if args.contains_key("--summary") {
        eprintln!("source_material_hash={}", h256(&source_material_hash));
        eprintln!(
            "source_adapter_deployment_hash={}",
            h256(&source_deployment_hash)
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
