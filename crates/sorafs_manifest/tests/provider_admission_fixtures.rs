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
        "2bc1b6aa4269d8a1201064a935efedbe0b92dd29ad4de10b38507081ccc2d076"
    );
    assert_eq!(
        metadata
            .get("envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("envelope digest"),
        "49a04ef708e0ac41e9c1a0dd7c44d5f264470425c5686cb6a00645d4d7374afc"
    );
    assert_eq!(
        metadata
            .get("renewal_envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("renewal envelope digest"),
        "5b5a18be046e3e0ed9af8be4fd6e718226c0c4481177f340c7e890fbb3bebcdd"
    );
    assert_eq!(
        metadata
            .get("revocation_digest_hex")
            .and_then(Value::as_str)
            .expect("revocation digest"),
        "d524070c2162a6f4666be911a632afce7ead28f484c071c00d062296e2a08539"
    );

    let renewal_text =
        fs::read_to_string(out_dir.join("renewal_v1.json")).expect("read renewal_v1.json");
    let renewal: Value = norito::json::from_str(&renewal_text).expect("parse renewal_v1.json");
    assert_eq!(
        renewal
            .get("previous_envelope_digest_hex")
            .and_then(Value::as_str),
        Some("49a04ef708e0ac41e9c1a0dd7c44d5f264470425c5686cb6a00645d4d7374afc")
    );
    assert_eq!(
        renewal.get("retention_epoch").and_then(Value::as_u64),
        Some(900)
    );

    let revocation_text =
        fs::read_to_string(out_dir.join("revocation_v1.json")).expect("read revocation_v1.json");
    let revocation: Value =
        norito::json::from_str(&revocation_text).expect("parse revocation_v1.json");
    assert_eq!(
        revocation
            .get("envelope_digest_hex")
            .and_then(Value::as_str),
        Some("49a04ef708e0ac41e9c1a0dd7c44d5f264470425c5686cb6a00645d4d7374afc")
    );
    assert_eq!(
        revocation
            .get("revocation_digest_hex")
            .and_then(Value::as_str),
        Some("d524070c2162a6f4666be911a632afce7ead28f484c071c00d062296e2a08539")
    );
    assert_eq!(
        revocation.get("revoked_at").and_then(Value::as_u64),
        Some(970)
    );
}
