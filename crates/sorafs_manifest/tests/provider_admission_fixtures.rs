use std::{env, fs};

use assert_cmd::Command;
use norito::json::Value;
use tempfile::{Builder, TempDir};

fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-provider-admission-fixtures-")
        .tempdir_in(env::temp_dir().canonicalize()?)
}

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
        "75ae7423ac495a9417be3b7a3ec3ef641a51632fbfa46833a3c68b43c9d1025a"
    );
    assert_eq!(
        metadata
            .get("envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("envelope digest"),
        "80b9f9285062bee7cc2d9b5f29948cbfd26c1cbfea274b6937f9e2f2279defbf"
    );
    assert_eq!(
        metadata
            .get("renewal_envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("renewal envelope digest"),
        "e838cc856d1b70a34c8503154dd0f858328365a1b3f1c36f1560f002ee9494b1"
    );
    assert_eq!(
        metadata
            .get("revocation_digest_hex")
            .and_then(Value::as_str)
            .expect("revocation digest"),
        "9c04ca10c8573ab653000a83388871beeb90b9b9f53f1208bc3b695521eda5c3"
    );

    let renewal_text =
        fs::read_to_string(out_dir.join("renewal_v1.json")).expect("read renewal_v1.json");
    let renewal: Value = norito::json::from_str(&renewal_text).expect("parse renewal_v1.json");
    assert_eq!(
        renewal
            .get("previous_envelope_digest_hex")
            .and_then(Value::as_str),
        Some("80b9f9285062bee7cc2d9b5f29948cbfd26c1cbfea274b6937f9e2f2279defbf")
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
        Some("80b9f9285062bee7cc2d9b5f29948cbfd26c1cbfea274b6937f9e2f2279defbf")
    );
    assert_eq!(
        revocation
            .get("revocation_digest_hex")
            .and_then(Value::as_str),
        Some("9c04ca10c8573ab653000a83388871beeb90b9b9f53f1208bc3b695521eda5c3")
    );
    assert_eq!(
        revocation.get("revoked_at").and_then(Value::as_u64),
        Some(970)
    );
}
