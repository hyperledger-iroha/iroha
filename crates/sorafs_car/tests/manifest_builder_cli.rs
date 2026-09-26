//! CLI regression tests for the SoraFS manifest builder.
#![cfg(feature = "cli")]
use assert_cmd::cargo::cargo_bin_cmd;
use ed25519_dalek::SigningKey;
use std::{env, fs, path::PathBuf};
use tempfile::{Builder, TempDir};
fn canonical_temp_base() -> PathBuf {
    env::temp_dir()
        .canonicalize()
        .expect("canonical system temp dir")
}
fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-manifest-builder-cli-")
        .tempdir_in(canonical_temp_base())
}
#[test]
fn manifest_builder_rejects_noncanonical_operator_inputs() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.bin");
    fs::write(&payload_path, b"manifest-builder parser boundary").expect("write payload");
    for (arg, expected) in [
        ("--dag-codec=0X71", "canonical unsigned"),
        ("--car-size=000", "canonical unsigned"),
        ("--chunker-profile-id=01", "canonical unsigned"),
        ("--chunker-profile= sorafs.sf1@1.0.0", "whitespace"),
        ("--min-replicas=03", "canonical unsigned"),
        ("--retention-epoch=01", "canonical unsigned"),
        ("--por-sample=0", "greater than zero"),
        ("--por-sample=03", "canonical unsigned"),
        ("--por-sample-seed=0x01", "canonical unsigned"),
        ("--por-proof=01:0:0", "canonical unsigned"),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg(&payload_path)
            .arg(arg)
            .output()
            .expect("run manifest builder");
        assert!(!output.status.success(), "{arg} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {arg}, got {stderr}"
        );
    }
}
#[test]
fn manifest_builder_requires_one_positive_retention_epoch() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.bin");
    fs::write(&payload_path, b"manifest retention policy").expect("write payload");
    for (args, expected) in [
        (Vec::new(), "missing required option --retention-epoch"),
        (
            vec!["--retention-epoch=0"],
            "--retention-epoch must be greater than zero",
        ),
        (
            vec!["--retention-epoch=1", "--retention-epoch=2"],
            "--retention-epoch may only be specified once",
        ),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg(&payload_path)
            .args(args)
            .output()
            .expect("run manifest builder");
        assert!(
            !output.status.success(),
            "invalid retention policy should fail"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?}, got {stderr}"
        );
    }
}
#[test]
fn provider_admission_proposal_rejects_noncanonical_operator_inputs() {
    for (arg, expected) in [
        ("--chunker-profile= sorafs.sf1@1.0.0", "whitespace"),
        ("--chunker-profile=sorafs/sf1@1.0.0", "not canonical"),
        ("--jurisdiction=US", "unknown option"),
        ("--jurisdiction-code=us", "uppercase"),
        ("--capability= range:64", "ASCII whitespace"),
        ("--capability=range:064", "use --range-capability"),
        ("--capability=torii-gateway", "unknown capability"),
        ("--capability=soranet:guard", "use --soranet-pq"),
        ("--soranet-pq=stage-a", "expected exactly"),
        (
            "--range-capability=max_chunk_span=64,min_granularity=4",
            "unknown range-capability field",
        ),
        (
            "--stream-budget=max_in_flight=02,max_bytes_per_sec=1024",
            "canonical unsigned",
        ),
        (
            "--stream-budget=max_in_flight=2, max_bytes_per_sec=1024",
            "ASCII whitespace",
        ),
        (
            "--stream-budget=max-in-flight=2,max_bytes_per_sec=1024",
            "unknown stream-budget field",
        ),
        ("--transport-hint=torii:01", "canonical unsigned"),
        ("--transport-hint= torii:1", "ASCII whitespace"),
        (
            "--transport-hint=torii_http:1",
            "unknown transport protocol",
        ),
        (
            "--endpoint=noritorpc:storage.example",
            "unknown endpoint kind",
        ),
        ("--endpoint-kind=mtls", "unknown option"),
        ("--endpoint-attested-at=1", "unknown option"),
        ("--endpoint-expires-at=2", "unknown option"),
        ("--endpoint-leaf=leaf.der", "unknown option"),
        ("--endpoint-leaf-hex=11", "unknown option"),
        ("--endpoint-alpn=h2", "unknown option"),
        ("--endpoint-report=report.bin", "unknown option"),
        ("--endpoint-report-hex=11", "unknown option"),
        ("--endpoint-intermediate=chain.der", "unknown option"),
        ("--endpoint-intermediate-hex=11", "unknown option"),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg("provider-admission")
            .arg("proposal")
            .arg(arg)
            .output()
            .expect("run provider-admission proposal");
        assert!(!output.status.success(), "{arg} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {arg}, got {stderr}"
        );
    }
}

#[test]
fn provider_admission_sign_and_verify_require_exact_network() {
    let temp = tempdir().expect("tempdir");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/sorafs_manifest/provider_admission");
    let fixture_envelope: sorafs_manifest::ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(&fs::read(fixtures.join("envelope_v1.to")).expect("fixture"))
            .expect("decode fixture envelope");
    let envelope_out = temp.path().join("signed-envelope.to");
    let sign_args = vec![
        "provider-admission".to_string(),
        "sign".to_string(),
        format!("--proposal={}", fixtures.join("proposal_v1.to").display()),
        format!("--advert={}", fixtures.join("advert_v1.to").display()),
        format!("--issued-at={}", fixture_envelope.issued_at),
        format!("--retention-epoch={}", fixture_envelope.retention_epoch),
        format!("--policy-id={}", hex::encode(fixture_envelope.policy_id)),
        format!("--policy-revision={}", fixture_envelope.policy_revision),
        format!(
            "--policy-digest={}",
            hex::encode(fixture_envelope.policy_digest)
        ),
        format!(
            "--admission-revision={}",
            fixture_envelope.admission_revision
        ),
        format!("--council-secret-key={}", "45".repeat(32)),
        format!("--envelope-out={}", envelope_out.display()),
    ];
    let missing_sign = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&sign_args)
        .output()
        .expect("sign without network");
    assert!(!missing_sign.status.success());
    assert!(String::from_utf8_lossy(&missing_sign.stderr).contains("missing option --network-id"));
    assert!(!envelope_out.exists());

    let mut foreign_sign_args = sign_args.clone();
    foreign_sign_args.push(format!("--network-id={}", "b2".repeat(32)));
    let foreign_sign = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&foreign_sign_args)
        .output()
        .expect("sign against foreign network");
    assert!(!foreign_sign.status.success());
    assert!(String::from_utf8_lossy(&foreign_sign.stderr).contains("advert network id differs"));
    assert!(!envelope_out.exists());

    let mut missing_policy_sign_args = sign_args.clone();
    missing_policy_sign_args.push(format!("--network-id={}", "a1".repeat(32)));
    missing_policy_sign_args.retain(|arg| !arg.starts_with("--policy-digest="));
    let missing_policy_sign = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&missing_policy_sign_args)
        .output()
        .expect("sign without policy digest");
    assert!(!missing_policy_sign.status.success());
    assert!(
        String::from_utf8_lossy(&missing_policy_sign.stderr)
            .contains("missing option --policy-digest")
    );
    assert!(!envelope_out.exists());

    let mut local_sign_args = sign_args;
    local_sign_args.push(format!("--network-id={}", "a1".repeat(32)));
    let local_sign = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&local_sign_args)
        .output()
        .expect("sign local admission");
    assert!(
        local_sign.status.success(),
        "{}",
        String::from_utf8_lossy(&local_sign.stderr)
    );
    assert!(envelope_out.exists());

    let council_key = SigningKey::from_bytes(&[0x45; 32]);
    let verify_args = vec![
        "provider-admission".to_string(),
        "verify".to_string(),
        format!("--envelope={}", envelope_out.display()),
        format!(
            "--trusted-council-key={}",
            hex::encode(council_key.verifying_key().to_bytes())
        ),
        "--signature-threshold=1".to_string(),
    ];
    let missing_verify = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&verify_args)
        .output()
        .expect("verify without network");
    assert!(!missing_verify.status.success());
    assert!(
        String::from_utf8_lossy(&missing_verify.stderr).contains("missing option --network-id")
    );
    let mut foreign_verify_args = verify_args.clone();
    foreign_verify_args.push(format!("--network-id={}", "b2".repeat(32)));
    let foreign_verify = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&foreign_verify_args)
        .output()
        .expect("verify against foreign network");
    assert!(!foreign_verify.status.success());
    assert!(String::from_utf8_lossy(&foreign_verify.stderr).contains("network id differs"));
    let mut local_verify_args = verify_args;
    local_verify_args.push(format!("--network-id={}", "a1".repeat(32)));
    let local_verify = cargo_bin_cmd!("sorafs_manifest_builder")
        .args(&local_verify_args)
        .output()
        .expect("verify local admission");
    assert!(
        local_verify.status.success(),
        "{}",
        String::from_utf8_lossy(&local_verify.stderr)
    );
}
