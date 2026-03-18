use std::{env, fs};

use assert_cmd::Command;
use norito::json::Value;
use tempfile::tempdir;

#[test]
fn provider_admission_fixture_generator_outputs_digests() {
    let tempdir = tempdir().expect("tempdir");
    let out_dir = tempdir.path().join("fixtures");

    let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("run")
        .arg("--locked")
        .arg("-p")
        .arg("sorafs_car")
        .arg("--features")
        .arg("cli")
        .arg("--bin")
        .arg("provider_admission_fixtures")
        .arg("--")
        .arg(format!("--out-dir={}", out_dir.display()))
        .env("NORITO_SKIP_BINDINGS_SYNC", "1");
    cmd.assert().success();

    for name in [
        "proposal_v1.to",
        "advert_v1.to",
        "envelope_v1.to",
        "proposal_v2.to",
        "envelope_v2.to",
        "renewal_v1.to",
        "revocation_v1.to",
        "metadata.json",
    ] {
        assert!(
            out_dir.join(name).exists(),
            "{name} missing from fixture output"
        );
    }

    let metadata_text =
        fs::read_to_string(out_dir.join("metadata.json")).expect("read metadata.json");
    let metadata: Value = norito::json::from_str(&metadata_text).expect("parse metadata");
    assert_eq!(
        metadata
            .get("proposal_digest_hex")
            .and_then(Value::as_str)
            .expect("proposal digest"),
        "176019d0bea04483973d97b1fbcbd6e558a4be37a8ba8c918d132472e39d04eb"
    );
    assert_eq!(
        metadata
            .get("envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("envelope digest"),
        "8e4f3177bb02d7d01ed269db15b45b9b8fbf470e55199c827048304966069535"
    );
}
