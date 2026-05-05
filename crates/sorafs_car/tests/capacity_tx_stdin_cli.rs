#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_data_model::{
    isi::{Instruction, sorafs::RegisterCapacityDeclaration},
    prelude::InstructionBox,
};
use norito::{
    decode_from_bytes,
    json::{self, Value},
    to_bytes,
};
use sorafs_manifest::{
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
        ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
        ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
    },
    provider_advert::{CapabilityType, StakePointer},
};
use tempfile::tempdir;

#[test]
fn tx_stdin_builder_wraps_capacity_declaration_requests() {
    let temp = tempdir().expect("tempdir");
    let request_path = temp.path().join("declaration_request.json");
    let declaration_b64 =
        BASE64_STD.encode(to_bytes(&sample_declaration()).expect("serialize declaration"));
    fs::write(
        &request_path,
        format!(
            concat!(
                "{{\n",
                "  \"declaration_b64\": \"{declaration_b64}\",\n",
                "  \"registered_epoch\": 580,\n",
                "  \"valid_from_epoch\": 580,\n",
                "  \"valid_until_epoch\": 10580,\n",
                "  \"metadata\": [\n",
                "    {{ \"key\": \"sorafs.owner_account_id\", \"value\": \"testuExampleCanary\" }}\n",
                "  ]\n",
                "}}\n"
            ),
            declaration_b64 = declaration_b64,
        ),
    )
    .expect("write declaration request");

    let payload = run_builder([
        "capacity-declaration-request".to_owned(),
        format!("--request={}", request_path.display()),
    ]);
    let instruction = decode_single_instruction(payload);
    let declaration = instruction
        .as_any()
        .downcast_ref::<RegisterCapacityDeclaration>()
        .expect("register capacity declaration");
    assert_eq!(declaration.record.provider_id.as_bytes(), &[0x11; 32],);
    assert_eq!(declaration.record.registered_epoch, 580);
    assert_eq!(declaration.record.valid_until_epoch, 10_580);
}

#[test]
fn tx_stdin_builder_wraps_replication_order_requests() {
    let temp = tempdir().expect("tempdir");
    let request_path = temp.path().join("order_request.json");
    let order_b64 =
        BASE64_STD.encode(to_bytes(&sample_replication_order()).expect("serialize order"));
    fs::write(
        &request_path,
        format!("{{\n  \"order_b64\": \"{order_b64}\"\n}}\n"),
    )
    .expect("write order request");

    let payload = run_builder([
        "replication-order-request".to_owned(),
        format!("--request={}", request_path.display()),
        "--issued-epoch=580".to_owned(),
        "--deadline-epoch=2580".to_owned(),
    ]);
    let instruction = decode_single_instruction(payload);
    let order = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::sorafs::IssueReplicationOrder>()
        .expect("issue replication order");
    assert_eq!(order.issued_epoch, 580);
    assert_eq!(order.deadline_epoch, 2_580);
    assert_eq!(order.order_id.as_bytes(), &[0x55; 32]);
}

#[test]
fn tx_stdin_builder_emits_completion_instruction() {
    let payload = run_builder([
        "complete-order".to_owned(),
        "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555"
            .to_owned(),
        "--completion-epoch=777".to_owned(),
    ]);
    let instruction = decode_single_instruction(payload);
    let completion = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::sorafs::CompleteReplicationOrder>()
        .expect("complete replication order");
    assert_eq!(completion.completion_epoch, 777);
    assert_eq!(completion.order_id.as_bytes(), &[0x55; 32]);
}

fn run_builder(args: impl IntoIterator<Item = String>) -> Value {
    let mut cmd = cargo_bin_cmd!("sorafs_tx_stdin_builder");
    cmd.args(args);
    let output = cmd.assert().success().get_output().stdout.clone();
    json::from_slice(&output).expect("parse tx stdin json")
}

fn decode_single_instruction(payload: Value) -> InstructionBox {
    let entries = payload.as_array().expect("tx stdin array");
    assert_eq!(entries.len(), 1);
    let encoded = entries[0].as_str().expect("base64 instruction");
    let bytes = BASE64_STD
        .decode(encoded.as_bytes())
        .expect("decode instruction base64");
    decode_from_bytes(&bytes).expect("decode instruction")
}

fn sample_declaration() -> CapacityDeclarationV1 {
    CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x11; 32],
        stake: StakePointer {
            pool_id: [0x22; 32],
            stake_amount: 1,
        },
        committed_capacity_gib: 1,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: None,
            committed_gib: 1,
            capability_refs: vec![CapabilityType::ToriiGateway],
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "global".to_owned(),
            max_gib: 1,
        }],
        pricing: None,
        valid_from: 1_700_000_000,
        valid_until: 1_700_086_400,
        metadata: vec![CapacityMetadataEntry {
            key: "sorafs.owner_account_id".to_owned(),
            value: "testuExampleCanary".to_owned(),
        }],
    }
}

fn sample_replication_order() -> ReplicationOrderV1 {
    ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: [0x55; 32],
        manifest_cid: vec![1, 2, 3, 4],
        manifest_digest: [0x66; 32],
        chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
        target_replicas: 1,
        assignments: vec![ReplicationAssignmentV1 {
            provider_id: [0x11; 32],
            slice_gib: 1,
            lane: Some("global".to_owned()),
        }],
        issued_at: 1_700_000_000,
        deadline_at: 1_700_003_600,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 300,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 99_000,
        },
        metadata: vec![CapacityMetadataEntry {
            key: "service".to_owned(),
            value: "ton-indexer".to_owned(),
        }],
    }
}
