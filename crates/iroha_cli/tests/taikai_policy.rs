//! Coverage for the Taikai policy/crypto CLI helpers.
//!
//! These tests prove that `iroha app taikai cek-rotate` and `iroha app taikai rpt-attest`
//! emit deterministic Norito artefacts with the expected digests and metadata.

use std::{
    borrow::Cow,
    fs::{self, File},
    io::Read,
    path::Path,
    process::Command,
};

use blake3::Hasher;
use iroha_data_model::taikai::{CekRotationReceiptV1, ReplicationProofTokenV1};
use norito::json::Value;
use tempfile::tempdir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_iroha")
}

#[test]
#[allow(clippy::too_many_lines)]
fn taikai_cli_emits_cek_receipt_and_rpt() {
    let dir = tempdir().expect("tempdir");
    let gar_path = dir.path().join("gar.json");
    fs::write(&gar_path, br#"{"gar":"demo"}"#).expect("write gar");
    let bundle_path = dir.path().join("bundle.bin");
    fs::write(&bundle_path, b"bundle-bytes").expect("write bundle");

    let receipt_path = dir.path().join("cek_receipt.to");
    let receipt_json = dir.path().join("cek_receipt.json");
    let hkdf_salt_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let receipt_status = Command::new(cli_binary())
        .args([
            "app",
            "taikai",
            "cek-rotate",
            "--event-id",
            "demo-event",
            "--stream-id",
            "stream-1",
            "--kms-profile",
            "kms-demo",
            "--new-wrap-key-label",
            "wrap-v2",
            "--previous-wrap-key-label",
            "wrap-v1",
            "--effective-segment",
            "42",
            "--hkdf-salt",
            hkdf_salt_hex,
            "--issued-at-unix",
            "1700000001",
            "--notes",
            "rotation-plan",
            "--out",
            receipt_path.to_str().expect("utf8 path"),
            "--json-out",
            receipt_json.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("spawn cek-rotate");
    assert!(receipt_status.success(), "cek-rotate command failed");

    let receipt_bytes = fs::read(&receipt_path).expect("read receipt");
    let receipt: CekRotationReceiptV1 =
        norito::decode_from_bytes(&receipt_bytes).expect("decode cek receipt Norito");
    assert_eq!(receipt.event_id.as_name().as_ref(), "demo-event");
    assert_eq!(receipt.stream_id.as_name().as_ref(), "stream-1");
    assert_eq!(receipt.kms_profile, "kms-demo");
    assert_eq!(receipt.new_wrap_key_label, "wrap-v2");
    assert_eq!(receipt.previous_wrap_key_label.as_deref(), Some("wrap-v1"));
    assert_eq!(receipt.effective_segment_sequence, 42);
    assert_eq!(receipt.issued_at_unix, 1_700_000_001);
    assert_eq!(receipt.notes.as_deref(), Some("rotation-plan"));
    assert_eq!(receipt.hkdf_salt, [0xaa; 32]);

    let receipt_json_value: Value =
        norito::json::from_slice(&fs::read(&receipt_json).expect("read receipt json"))
            .expect("parse receipt json");
    assert_eq!(
        receipt_json_value
            .get("kms_profile")
            .and_then(Value::as_str),
        Some("kms-demo")
    );

    let rpt_path = dir.path().join("rpt.to");
    let rpt_json = dir.path().join("rpt.json");
    let rpt_status = Command::new(cli_binary())
        .args([
            "app",
            "taikai",
            "rpt-attest",
            "--event-id",
            "demo-event",
            "--stream-id",
            "stream-1",
            "--rendition-id",
            "1080p-main",
            "--gar",
            gar_path.to_str().expect("utf8 gar"),
            "--cek-receipt",
            receipt_path.to_str().expect("utf8 receipt"),
            "--bundle",
            bundle_path.to_str().expect("utf8 bundle"),
            "--valid-from-unix",
            "1700000100",
            "--valid-until-unix",
            "1700000200",
            "--policy-label",
            "pilot",
            "--policy-label",
            "taikai",
            "--notes",
            "rollout-check",
            "--out",
            rpt_path.to_str().expect("utf8 rpt"),
            "--json-out",
            rpt_json.to_str().expect("utf8 rpt json"),
        ])
        .status()
        .expect("spawn rpt-attest");
    assert!(rpt_status.success(), "rpt-attest command failed");

    let rpt_bytes = fs::read(&rpt_path).expect("read rpt");
    let rpt: ReplicationProofTokenV1 =
        norito::decode_from_bytes(&rpt_bytes).expect("decode rpt Norito");
    assert_eq!(rpt.event_id.as_name().as_ref(), "demo-event");
    assert_eq!(rpt.stream_id.as_name().as_ref(), "stream-1");
    assert_eq!(rpt.rendition_id.as_name().as_ref(), "1080p-main");
    assert_eq!(rpt.policy_labels, ["pilot".to_owned(), "taikai".to_owned()]);
    assert_eq!(rpt.valid_from_unix, 1_700_000_100);
    assert_eq!(rpt.valid_until_unix, 1_700_000_200);
    assert_eq!(rpt.notes.as_deref(), Some("rollout-check"));
    assert_eq!(rpt.gar_digest, digest_file_for_cli(&gar_path));
    assert_eq!(rpt.cek_receipt_digest, digest_file_for_cli(&receipt_path));
    assert_eq!(
        rpt.distribution_bundle_digest,
        digest_file_for_cli(&bundle_path)
    );

    let rpt_json_value: Value =
        norito::json::from_slice(&fs::read(&rpt_json).expect("read rpt json"))
            .expect("parse rpt json");
    assert_eq!(
        rpt_json_value
            .get("policy_labels")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

fn digest_file_for_cli(path: &Path) -> [u8; 32] {
    let mut hasher = Hasher::new();
    let label: Cow<'_, str> = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(Cow::from)
        .map_or_else(|| Cow::from("."), Cow::from);
    hasher.update(label.as_bytes());
    hasher.update(&[0xFF, b'F']);

    let mut file = File::open(path).expect("open file for digest");
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer).expect("read file for digest");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    *hasher.finalize().as_bytes()
}
