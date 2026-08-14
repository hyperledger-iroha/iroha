//! CLI regression tests for the production SoraFS provider-advert builder.
#![cfg(feature = "cli")]
use assert_cmd::cargo::cargo_bin_cmd;
use ed25519_dalek::{Signer, SigningKey};
use iroha_crypto::sha256;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use tempfile::{Builder, TempDir};
fn canonical_temp_base() -> PathBuf {
    env::temp_dir()
        .canonicalize()
        .expect("canonical system temp dir")
}
fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-provider-advert-cli-")
        .tempdir_in(canonical_temp_base())
}
fn repeated_hex(byte: &str, count: usize) -> String {
    byte.repeat(count)
}
fn body_args() -> Vec<String> {
    vec![
        "--chunker-profile=sorafs.sf1@1.0.0".to_string(),
        format!("--provider-id={}", repeated_hex("11", 32)),
        format!("--stake-pool-id={}", repeated_hex("22", 32)),
        "--stake-amount=5000000".to_string(),
        "--availability=hot".to_string(),
        "--max-latency-ms=1500".to_string(),
        "--max-streams=4".to_string(),
        "--capability=torii".to_string(),
        "--endpoint=torii:storage.example.com".to_string(),
        "--topic=sorafs.sf1.primary:global".to_string(),
        "--issued-at=1700000000".to_string(),
    ]
}
struct SigningFixture {
    signing_key: SigningKey,
    public_key_path: PathBuf,
    fingerprint: String,
}
fn signing_fixture(temp: &TempDir, seed: u8, name: &str) -> SigningFixture {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let public_key = signing_key.verifying_key().to_bytes();
    let public_key_path = temp.path().join(format!("{name}.pub"));
    fs::write(&public_key_path, public_key).expect("write public key");
    SigningFixture {
        signing_key,
        public_key_path,
        fingerprint: hex::encode(sha256(public_key)),
    }
}
fn reviewed_key_args(fixture: &SigningFixture) -> Vec<String> {
    vec![
        format!("--public-key-file={}", fixture.public_key_path.display()),
        format!("--public-key-fingerprint-sha256={}", fixture.fingerprint),
    ]
}
fn prepare_args(
    temp: &TempDir,
    fixture: &SigningFixture,
    payload_name: &str,
) -> (Vec<String>, PathBuf, PathBuf) {
    let payload_path = temp.path().join(payload_name);
    let report_path = temp.path().join(format!("{payload_name}.json"));
    let mut args = vec!["--prepare".to_string()];
    args.extend(body_args());
    args.extend(reviewed_key_args(fixture));
    args.push(format!("--signing-payload-out={}", payload_path.display()));
    args.push(format!("--json-out={}", report_path.display()));
    (args, payload_path, report_path)
}
fn prepare_and_sign(temp: &TempDir, fixture: &SigningFixture, prefix: &str) -> (PathBuf, PathBuf) {
    let (args, payload_path, _) = prepare_args(temp, fixture, &format!("{prefix}.signing-payload"));
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(args)
        .output()
        .expect("prepare provider advert signing payload");
    assert!(
        output.status.success(),
        "prepare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = fs::read(&payload_path).expect("read signing payload");
    let signature = fixture.signing_key.sign(&payload).to_bytes();
    let signature_path = temp.path().join(format!("{prefix}.sig"));
    fs::write(&signature_path, signature).expect("write external signature");
    (payload_path, signature_path)
}
fn emit_args(
    temp: &TempDir,
    fixture: &SigningFixture,
    signing_payload_path: &Path,
    signature_path: &Path,
    prefix: &str,
) -> (Vec<String>, PathBuf, PathBuf) {
    let advert_path = temp.path().join(format!("{prefix}.advert"));
    let report_path = temp.path().join(format!("{prefix}.report.json"));
    let mut args = vec!["--emit".to_string()];
    args.extend(body_args());
    args.extend(reviewed_key_args(fixture));
    args.push(format!(
        "--signing-payload-file={}",
        signing_payload_path.display()
    ));
    args.push(format!("--signature-file={}", signature_path.display()));
    args.push(format!("--advert-out={}", advert_path.display()));
    args.push(format!("--json-out={}", report_path.display()));
    (args, advert_path, report_path)
}
#[test]
fn provider_advert_external_signing_round_trip_is_deterministic() {
    let first = tempdir().expect("first tempdir");
    let second = tempdir().expect("second tempdir");
    let first_signer = signing_fixture(&first, 0x33, "provider");
    let second_signer = signing_fixture(&second, 0x33, "provider");
    let (first_payload, first_signature) = prepare_and_sign(&first, &first_signer, "provider");
    let (second_payload, second_signature) = prepare_and_sign(&second, &second_signer, "provider");
    assert_eq!(
        fs::read(&first_payload).expect("read first payload"),
        fs::read(&second_payload).expect("read second payload"),
        "canonical signing payload must be byte-identical"
    );
    assert_eq!(
        fs::read(&first_signature).expect("read first signature"),
        fs::read(&second_signature).expect("read second signature"),
        "Ed25519 signature must be deterministic"
    );
    let (first_args, first_advert, first_report) = emit_args(
        &first,
        &first_signer,
        &first_payload,
        &first_signature,
        "provider",
    );
    let first_output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(first_args)
        .output()
        .expect("emit first provider advert");
    assert!(
        first_output.status.success(),
        "first emit failed: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );
    let (second_args, second_advert, second_report) = emit_args(
        &second,
        &second_signer,
        &second_payload,
        &second_signature,
        "provider",
    );
    let second_output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(second_args)
        .output()
        .expect("emit second provider advert");
    assert!(
        second_output.status.success(),
        "second emit failed: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );
    assert_eq!(
        fs::read(&first_advert).expect("read first advert"),
        fs::read(&second_advert).expect("read second advert"),
        "final Norito adverts must be byte-identical"
    );
    assert_eq!(
        fs::read(&first_report).expect("read first report"),
        fs::read(&second_report).expect("read second report"),
        "final JSON reports must be byte-identical"
    );
    let verify_output = cargo_bin_cmd!("sorafs_provider_advert")
        .arg("--verify")
        .arg(format!("--advert={}", first_advert.display()))
        .args(reviewed_key_args(&first_signer))
        .arg("--now=1700000000")
        .output()
        .expect("verify provider advert");
    assert!(
        verify_output.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );
}
#[test]
fn provider_advert_emit_rejects_unsigned_production_output() {
    let temp = tempdir().expect("tempdir");
    let fixture = signing_fixture(&temp, 0x33, "provider");
    let (prepare, payload_path, _) = prepare_args(&temp, &fixture, "provider.signing-payload");
    cargo_bin_cmd!("sorafs_provider_advert")
        .args(prepare)
        .assert()
        .success();
    let advert_path = temp.path().join("unsigned.advert");
    let mut args = vec!["--emit".to_string()];
    args.extend(body_args());
    args.extend(reviewed_key_args(&fixture));
    args.push(format!("--signing-payload-file={}", payload_path.display()));
    args.push(format!("--advert-out={}", advert_path.display()));
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(args)
        .output()
        .expect("run unsigned provider advert");
    assert!(!output.status.success(), "unsigned emit must fail closed");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--signature-file"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!advert_path.exists(), "unsigned advert must not be written");
}
#[test]
fn provider_advert_cli_rejects_direct_software_signing_options() {
    let temp = tempdir().expect("tempdir");
    let fixture = signing_fixture(&temp, 0x33, "provider");
    for forbidden in [
        format!("--signing-key={}", repeated_hex("33", 32)),
        format!("--signing-key-file={}", temp.path().join("seed").display()),
        format!("--public-key={}", repeated_hex("44", 32)),
        format!("--signature={}", repeated_hex("55", 64)),
    ] {
        let (mut args, payload_path, report_path) =
            prepare_args(&temp, &fixture, "forbidden.signing-payload");
        args.push(forbidden.clone());
        let output = cargo_bin_cmd!("sorafs_provider_advert")
            .args(args)
            .output()
            .expect("run forbidden software signing option");
        assert!(!output.status.success(), "{forbidden} must fail closed");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unknown option"),
            "unexpected stderr for {forbidden}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!payload_path.exists());
        assert!(!report_path.exists());
    }
}
#[test]
fn provider_advert_cli_rejects_wrong_key_fingerprint_and_signature() {
    let temp = tempdir().expect("tempdir");
    let fixture = signing_fixture(&temp, 0x33, "provider");
    let other = signing_fixture(&temp, 0x44, "other");
    let (mut wrong_fingerprint_args, payload_path, _) =
        prepare_args(&temp, &fixture, "wrong-fingerprint.payload");
    let fingerprint_arg = wrong_fingerprint_args
        .iter_mut()
        .find(|arg| arg.starts_with("--public-key-fingerprint-sha256="))
        .expect("fingerprint argument");
    *fingerprint_arg = format!("--public-key-fingerprint-sha256={}", repeated_hex("00", 32));
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(wrong_fingerprint_args)
        .output()
        .expect("run wrong fingerprint");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("reviewed fingerprint"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!payload_path.exists());
    let (reviewed_payload, signature_path) = prepare_and_sign(&temp, &fixture, "wrong-key");
    let (wrong_key_args, advert_path, _) = emit_args(
        &temp,
        &other,
        &reviewed_payload,
        &signature_path,
        "wrong-key",
    );
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(wrong_key_args)
        .output()
        .expect("run wrong key");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("reviewed external signing payload"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!advert_path.exists());
    let (reviewed_payload, _) = prepare_and_sign(&temp, &fixture, "wrong-signature");
    let wrong_signature_path = temp.path().join("wrong-signature.sig");
    let payload = fs::read(&reviewed_payload).expect("read reviewed payload");
    fs::write(
        &wrong_signature_path,
        other.signing_key.sign(&payload).to_bytes(),
    )
    .expect("write wrong signature");
    let (wrong_signature_args, advert_path, _) = emit_args(
        &temp,
        &fixture,
        &reviewed_payload,
        &wrong_signature_path,
        "wrong-signature",
    );
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(wrong_signature_args)
        .output()
        .expect("run wrong signature");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("signature validation failed"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!advert_path.exists());
}
#[test]
fn provider_advert_cli_rejects_malformed_signature() {
    let temp = tempdir().expect("tempdir");
    let fixture = signing_fixture(&temp, 0x33, "provider");
    let (payload_path, _) = prepare_and_sign(&temp, &fixture, "malformed");
    let signature_path = temp.path().join("malformed-short.sig");
    fs::write(&signature_path, [0x55; 63]).expect("write short signature");
    let (args, advert_path, _) =
        emit_args(&temp, &fixture, &payload_path, &signature_path, "malformed");
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(args)
        .output()
        .expect("run malformed signature");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exactly 64 raw bytes"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!advert_path.exists());
}
#[cfg(unix)]
#[test]
fn provider_advert_cli_rejects_symlink_and_hardlink_inputs() {
    let temp = tempdir().expect("tempdir");
    let fixture = signing_fixture(&temp, 0x33, "provider");
    let symlink_path = temp.path().join("linked.pub");
    std::os::unix::fs::symlink(&fixture.public_key_path, &symlink_path)
        .expect("create public-key symlink");
    let symlink_fixture = SigningFixture {
        signing_key: SigningKey::from_bytes(&[0x33; 32]),
        public_key_path: symlink_path,
        fingerprint: fixture.fingerprint.clone(),
    };
    let (args, payload_path, _) = prepare_args(&temp, &symlink_fixture, "symlink.payload");
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(args)
        .output()
        .expect("run symlink input");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("direct regular file"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!payload_path.exists());
    let hardlink_path = temp.path().join("hardlinked.pub");
    fs::hard_link(&fixture.public_key_path, &hardlink_path).expect("create public-key hard link");
    let (args, payload_path, _) = prepare_args(&temp, &fixture, "hardlink.payload");
    let output = cargo_bin_cmd!("sorafs_provider_advert")
        .args(args)
        .output()
        .expect("run hardlink input");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exactly one hard link"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!payload_path.exists());
}
#[test]
fn provider_advert_cli_rejects_noncanonical_hex_before_outputs() {
    for (bad_arg, expected) in [
        (
            format!("--provider-id={}", repeated_hex("11", 31)),
            "expected exactly 32 hex bytes",
        ),
        (
            format!("--stake-pool-id={}", repeated_hex("22", 33)),
            "expected exactly 32 hex bytes",
        ),
        (
            format!("--provider-id={}", repeated_hex("AA", 32)),
            "lowercase even-length hex",
        ),
        (
            "--capability=vendor:ABCDEF".to_string(),
            "lowercase even-length hex",
        ),
        (
            "--capability=torii:a".to_string(),
            "lowercase even-length hex",
        ),
        (
            "--endpoint-meta=tls_fingerprint:ABCDEF".to_string(),
            "lowercase even-length hex",
        ),
    ] {
        let temp = tempdir().expect("tempdir");
        let fixture = signing_fixture(&temp, 0x33, "provider");
        let (mut args, payload_path, report_path) =
            prepare_args(&temp, &fixture, "provider.signing-payload");
        args.push(bad_arg.clone());
        let output = cargo_bin_cmd!("sorafs_provider_advert")
            .args(args)
            .output()
            .expect("run provider advert builder");
        assert!(
            !output.status.success(),
            "provider advert unexpectedly succeeded for {bad_arg}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {bad_arg}, got: {stderr}"
        );
        assert!(!payload_path.exists());
        assert!(!report_path.exists());
    }
}
#[test]
fn provider_advert_cli_rejects_v1_selector_aliases_before_outputs() {
    for (bad_arg, expected) in [
        ("--profile-id=1", "unknown option"),
        ("--availability=HOT", "unknown availability tier"),
        ("--capability=torii-gateway", "unknown capability type"),
        ("--capability=quic-noise", "unknown capability type"),
        ("--capability=potr_mldsa:11", "unknown capability type"),
        ("--capability=soranet:guard", "use --soranet-pq"),
        ("--soranet-pq=stage-a", "expected exactly"),
        (
            "--range-capability=max_chunk_span=16,min_granularity=4",
            "unknown range-capability field",
        ),
        (
            "--stream-budget=max-in-flight=2,max_bytes_per_sec=1024",
            "unknown stream-budget field",
        ),
        (
            "--transport-hint=torii_http:0",
            "unknown transport protocol",
        ),
        (
            "--endpoint=noritorpc:storage.example",
            "unknown endpoint kind",
        ),
        ("--endpoint-meta=tls:11", "unknown endpoint metadata key"),
        (
            "--allow-unknown-capabilities=yes",
            "expected boolean true|false",
        ),
    ] {
        let temp = tempdir().expect("tempdir");
        let fixture = signing_fixture(&temp, 0x33, "provider");
        let (mut args, payload_path, report_path) =
            prepare_args(&temp, &fixture, "provider.signing-payload");
        args.push(bad_arg.to_string());
        let output = cargo_bin_cmd!("sorafs_provider_advert")
            .args(args)
            .output()
            .expect("run provider advert builder");
        assert!(
            !output.status.success(),
            "provider advert unexpectedly succeeded for {bad_arg}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {bad_arg}, got: {stderr}"
        );
        assert!(!payload_path.exists());
        assert!(!report_path.exists());
    }
}
