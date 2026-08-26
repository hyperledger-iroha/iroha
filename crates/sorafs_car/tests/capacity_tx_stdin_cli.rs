//! Integration coverage for SoraFS capacity transaction stdin construction.
#![cfg(feature = "cli")]
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
    canonical_manifest_root_cid,
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
        ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
        ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
    },
    provider_advert::{CapabilityType, StakePointer},
};
use std::fs;
use tempfile::tempdir;
#[test]
fn tx_stdin_builder_wraps_capacity_declaration_summaries() {
    let temp = tempdir().expect("tempdir");
    let summary_path = temp.path().join("declaration_summary.json");
    let declaration_b64 =
        BASE64_STD.encode(to_bytes(&sample_declaration()).expect("serialize declaration"));
    fs::write(
        &summary_path,
        format!(
            concat!(
                "{{\n",
                "  \"declaration_b64\": \"{declaration_b64}\",\n",
                "  \"registered_epoch\": 580,\n",
                "  \"metadata\": {{\n",
                "    \"sorafs.owner_account_id\": \"testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"\n",
                "  }}\n",
                "}}\n"
            ),
            declaration_b64 = declaration_b64
        ),
    )
    .expect("write declaration summary");
    let payload = run_builder([
        "capacity-declaration".to_owned(),
        format!("--summary={}", summary_path.display()),
    ]);
    let instruction = decode_single_instruction(payload);
    let declaration = instruction
        .as_any()
        .downcast_ref::<RegisterCapacityDeclaration>()
        .expect("register capacity declaration");
    assert_eq!(declaration.record.provider_id.as_bytes(), &[0x11; 32],);
    assert_eq!(declaration.record.registered_epoch, 580);
    assert_eq!(declaration.record.valid_from_epoch, 1_700_000_000);
    assert_eq!(declaration.record.valid_until_epoch, 1_700_086_400);
}
#[test]
fn tx_stdin_builder_rejects_redundant_capacity_validity_summary() {
    let temp = tempdir().expect("tempdir");
    let summary_path = temp.path().join("redundant_declaration_summary.json");
    let declaration_b64 =
        BASE64_STD.encode(to_bytes(&sample_declaration()).expect("serialize declaration"));
    fs::write(
        &summary_path,
        format!(
            "{{\n  \"declaration_b64\": \"{declaration_b64}\",\n  \"registered_epoch\": 580,\n  \"valid_from_epoch\": 580\n}}\n"
        ),
    )
    .expect("write declaration summary");
    let stderr = run_builder_failure([
        "capacity-declaration".to_owned(),
        format!("--summary={}", summary_path.display()),
    ]);
    assert!(
        stderr.contains("valid_from_epoch") && stderr.contains("must be omitted"),
        "payload validity must be authoritative: {stderr}"
    );
}
#[test]
fn tx_stdin_builder_wraps_replication_order_summaries() {
    let temp = tempdir().expect("tempdir");
    let summary_path = temp.path().join("order_summary.json");
    let order_b64 =
        BASE64_STD.encode(to_bytes(&sample_replication_order()).expect("serialize order"));
    fs::write(
        &summary_path,
        format!("{{\n  \"replication_order_b64\": \"{order_b64}\"\n}}\n"),
    )
    .expect("write order summary");
    let payload = run_builder([
        "replication-order".to_owned(),
        format!("--summary={}", summary_path.display()),
    ]);
    let instruction = decode_single_instruction(payload);
    let order = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::sorafs::IssueReplicationOrder>()
        .expect("issue replication order");
    assert_eq!(order.issued_epoch, 1_700_000_000);
    assert_eq!(order.deadline_epoch, 1_700_003_600);
    assert_eq!(order.order_id.as_bytes(), &[0x55; 32]);
    assert_eq!(order.musubi_archive, None);
}
#[test]
fn tx_stdin_builder_binds_replication_order_to_musubi_archive() {
    let temp = tempdir().expect("tempdir");
    let summary_path = temp.path().join("order_summary.json");
    let order_b64 =
        BASE64_STD.encode(to_bytes(&sample_replication_order()).expect("serialize order"));
    fs::write(
        &summary_path,
        format!("{{\n  \"replication_order_b64\": \"{order_b64}\"\n}}\n"),
    )
    .expect("write order summary");
    let payload = run_builder([
        "replication-order".to_owned(),
        format!("--summary={}", summary_path.display()),
        format!("--musubi-archive-id-hex={}", "a5".repeat(32)),
    ]);
    let instruction = decode_single_instruction(payload);
    let order = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::sorafs::IssueReplicationOrder>()
        .expect("issue replication order");
    assert_eq!(
        order.musubi_archive,
        Some(iroha_data_model::musubi::ArchiveId::new([0xa5; 32]))
    );
}
#[test]
fn tx_stdin_builder_rejects_reserved_automatic_order_id() {
    let temp = tempdir().expect("tempdir");
    let summary_path = temp.path().join("reserved_order_summary.json");
    let mut order = sample_replication_order();
    order.order_id[0] |= 0x80;
    let order_b64 = BASE64_STD.encode(to_bytes(&order).expect("serialize reserved order"));
    fs::write(
        &summary_path,
        format!("{{\n  \"replication_order_b64\": \"{order_b64}\"\n}}\n"),
    )
    .expect("write order summary");
    let stderr = run_builder_failure([
        "replication-order".to_owned(),
        format!("--summary={}", summary_path.display()),
    ]);
    assert!(
        stderr.contains("reserved automatic order id"),
        "generic builder must reject the automatic namespace: {stderr}"
    );
}
#[test]
fn tx_stdin_builder_rejects_redundant_replication_epoch_options() {
    for option in ["--issued-epoch=580", "--deadline-epoch=2580"] {
        let stderr = run_builder_failure([
            "replication-order".to_owned(),
            "--summary=unused.json".to_owned(),
            option.to_owned(),
        ]);
        assert!(
            stderr.contains("unknown option") && stderr.contains(option.split('=').next().unwrap()),
            "payload timestamps are authoritative and {option} must be rejected: {stderr}"
        );
    }
}
#[test]
fn tx_stdin_builder_rejects_noncanonical_musubi_archive_id_hex() {
    for value in [
        "a5".repeat(31),
        format!("0x{}", "a5".repeat(32)),
        "A5".repeat(32),
        "00".repeat(32),
    ] {
        let stderr = run_builder_failure([
            "replication-order".to_owned(),
            "--summary=unused.json".to_owned(),
            format!("--musubi-archive-id-hex={value}"),
        ]);
        assert!(
            stderr.contains("musubi_archive_id_hex"),
            "stderr should name rejected Musubi archive id {value}, got: {stderr}"
        );
    }
}
#[test]
fn tx_stdin_builder_emits_completion_instruction() {
    let payload = run_builder([
        "complete-order".to_owned(),
        "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555"
            .to_owned(),
        "--provider-id-hex=6666666666666666666666666666666666666666666666666666666666666666"
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
    assert_eq!(completion.provider_id.as_bytes(), &[0x66; 32]);
}
#[test]
fn tx_stdin_builder_emits_expiration_instruction() {
    let payload = run_builder([
        "expire-order".to_owned(),
        "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555"
            .to_owned(),
        "--expiration-epoch=778".to_owned(),
    ]);
    let instruction = decode_single_instruction(payload);
    let expiration = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::sorafs::ExpireReplicationOrder>()
        .expect("expire replication order");
    assert_eq!(expiration.expiration_epoch, 778);
    assert_eq!(expiration.order_id.as_bytes(), &[0x55; 32]);
}
#[test]
fn tx_stdin_builder_rejects_noncanonical_epoch_flags() {
    for (args, expected) in [
        (
            vec![
                "complete-order",
                "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555",
                "--completion-epoch=0777",
            ],
            "--completion-epoch",
        ),
        (
            vec![
                "expire-order",
                "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555",
                "--expiration-epoch=0778",
            ],
            "--expiration-epoch",
        ),
    ] {
        let stderr = run_builder_failure(args.into_iter().map(str::to_owned));
        assert!(
            stderr.contains(expected) && stderr.contains("canonical unsigned"),
            "stderr should reject {expected} canonically, got: {stderr}"
        );
    }
}
#[test]
fn tx_stdin_builder_rejects_noncanonical_order_id_hex() {
    for value in [
        "5555",
        "0x5555555555555555555555555555555555555555555555555555555555555555",
        "555555555555555555555555555555555555555555555555555555555555555A",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        let stderr = run_builder_failure([
            "complete-order".to_owned(),
            format!("--order-id-hex={value}"),
            "--completion-epoch=777".to_owned(),
        ]);
        assert!(
            stderr.contains("order_id_hex"),
            "stderr should name rejected order_id_hex {value}, got: {stderr}"
        );
    }
}
#[test]
fn tx_stdin_builder_rejects_duplicate_options() {
    for (args, expected) in [
        (
            vec![
                "capacity-declaration",
                "--summary=one.json",
                "--summary=two.json",
            ],
            "--summary",
        ),
        (
            vec![
                "replication-order",
                "--summary=one.json",
                "--summary=two.json",
            ],
            "--summary",
        ),
        (
            vec![
                "complete-order",
                "--order-id-hex=5555555555555555555555555555555555555555555555555555555555555555",
                "--order-id-hex=6666666666666666666666666666666666666666666666666666666666666666",
            ],
            "--order-id-hex",
        ),
    ] {
        let stderr = run_builder_failure(args.into_iter().map(str::to_owned));
        assert!(
            stderr.contains("duplicate") && stderr.contains(expected),
            "stderr should reject duplicate {expected}, got: {stderr}"
        );
    }
}
fn run_builder(args: impl IntoIterator<Item = String>) -> Value {
    let mut cmd = cargo_bin_cmd!("sorafs_tx_stdin_builder");
    cmd.args(args);
    let output = cmd.assert().success().get_output().stdout.clone();
    json::from_slice(&output).expect("parse tx stdin json")
}
fn run_builder_failure(args: impl IntoIterator<Item = String>) -> String {
    let mut cmd = cargo_bin_cmd!("sorafs_tx_stdin_builder");
    cmd.args(args);
    let output = cmd.assert().failure().get_output().stderr.clone();
    String::from_utf8(output).expect("stderr should be utf8")
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
            stake_amount: "1".parse().expect("canonical XOR quantity"),
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
        metadata: vec![
            CapacityMetadataEntry {
                key: "sorafs.owner_account_id".to_owned(),
                value: "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".to_owned(),
            },
            CapacityMetadataEntry {
                key: "sorafs.storage_class".to_owned(),
                value: "hot".to_owned(),
            },
        ],
    }
}
fn sample_replication_order() -> ReplicationOrderV1 {
    ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: [0x55; 32],
        manifest_cid: canonical_manifest_root_cid([0x77; 32]),
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
