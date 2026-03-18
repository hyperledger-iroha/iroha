use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use iroha_crypto::{Algorithm, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    soradns::{
        GatewayHostSet, HttpTransportV1, PaddingPolicyV1, ResolverAttestationDocumentV1,
        ResolverDirectoryRecordV1, ResolverTlsBundle, ResolverTransportBundle, RotationPolicyV1,
        TlsProvisioningProfile, TlsTransportV1,
    },
};
use iroha_primitives::soradns::derive_gateway_hosts;
use norito::{
    json::{self, Number, Value},
    to_bytes,
};
use soradns_resolver::{
    canonical::{canonicalize_json_bytes, sha256_digest},
    directory::signing_payload_bytes,
    rad::compute_rad_digest,
};
use tempfile::TempDir;

#[test]
fn rad_verify_accepts_valid_entries() {
    let temp = TempDir::new().expect("tempdir");
    let rad_path = temp.path().join("rad.norito");
    write_rad_file(&rad_path, base_rad());

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args(["rad", "verify", rad_path.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn rad_verify_rejects_invalid_entries() {
    let temp = TempDir::new().expect("tempdir");
    let mut rad = base_rad();
    rad.canonical_hosts.pretty_host = "example.com".to_string();
    let bad_path = temp.path().join("bad-rad.norito");
    write_rad_file(&bad_path, rad);

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args(["rad", "verify", bad_path.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn directory_fetch_downloads_and_verifies_bundle() {
    let temp = TempDir::new().expect("tempdir");
    let (_, directory_bytes, record_bytes) = sample_directory_bundle();
    let record_path = temp.path().join("record.json");
    fs::write(&record_path, &record_bytes).expect("write record");
    let directory_path = temp.path().join("directory.json");
    fs::write(&directory_path, &directory_bytes).expect("write directory");

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args([
            "directory",
            "fetch",
            "--record-file",
            record_path.to_str().unwrap(),
            "--directory-file",
            directory_path.to_str().unwrap(),
            "--output",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(temp.path().join("record.json").exists());
    assert!(temp.path().join("directory.json").exists());
}

#[test]
fn directory_fetch_fails_on_digest_mismatch() {
    let temp = TempDir::new().expect("tempdir");
    let (_, directory_bytes, record_bytes) = sample_directory_bundle();
    let record_path = temp.path().join("record.json");
    fs::write(&record_path, &record_bytes).expect("write record");
    let bad_record_path = temp.path().join("record-bad.json");
    let mut corrupted = record_bytes.clone();
    let idx = corrupted.len() / 2;
    corrupted[idx] ^= 0xFF;
    fs::write(&bad_record_path, corrupted).expect("write corrupt record");
    let directory_path = temp.path().join("directory.json");
    fs::write(&directory_path, &directory_bytes).expect("write directory");

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args([
            "directory",
            "fetch",
            "--record-file",
            bad_record_path.to_str().unwrap(),
            "--directory-file",
            directory_path.to_str().unwrap(),
            "--output",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
}

#[test]
fn directory_verify_accepts_valid_bundle() {
    let temp = TempDir::new().expect("tempdir");
    write_sample_bundle(temp.path());

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args([
            "directory",
            "verify",
            "--bundle",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn directory_verify_detects_missing_rad() {
    let temp = TempDir::new().expect("tempdir");
    let rad_path = write_sample_bundle(temp.path());
    fs::remove_file(&rad_path).expect("remove rad file");

    Command::new(assert_cmd::cargo::cargo_bin!("soradns-resolver"))
        .args([
            "directory",
            "verify",
            "--bundle",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
}

fn base_rad() -> ResolverAttestationDocumentV1 {
    let bindings = derive_gateway_hosts("docs.sora").expect("derive hosts");
    let operator_account = {
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key literal");
        AccountId::new(public_key)
    };

    ResolverAttestationDocumentV1 {
        version: 1,
        resolver_id: [1; 32],
        fqdn: "docs.sora".to_string(),
        canonical_hosts: GatewayHostSet::from(&bindings),
        transport: ResolverTransportBundle {
            doh: Some(HttpTransportV1 {
                endpoint: "https://docs.sora/dns-query".to_string(),
                supports_get: true,
                supports_post: true,
                max_response_bytes: 2_048,
            }),
            dot: Some(TlsTransportV1 {
                endpoint: "tls://docs.sora:853".to_string(),
                alpn_protocols: vec!["dot".to_string()],
                cipher_suites: vec!["TLS_AES_256_GCM_SHA384".to_string()],
            }),
            doq: None,
            odoh_relay: None,
            soranet_bridge: None,
            qname_minimisation: true,
            padding_policy: PaddingPolicyV1 {
                min_bytes: 32,
                max_bytes: 64,
                pad_to_block: 16,
            },
        },
        tls: ResolverTlsBundle {
            provisioning_profiles: vec![TlsProvisioningProfile::Dns01],
            certificate_fingerprints: vec!["fp".to_string()],
            wildcard_hosts: vec!["*.gw.sora.id".to_string()],
            not_after_unix: 1_800_000_000,
        },
        resolver_manifest_hash: [2; 32],
        gar_manifest_hash: [3; 32],
        issued_at_unix: 1_700_000_000,
        valid_from_unix: 1_700_000_000,
        valid_until_unix: 1_700_086_400,
        operator_account,
        operator_signature: Signature::from_bytes(&[0; 64]),
        governance_signature: Signature::from_bytes(&[1; 64]),
        rotation_policy: RotationPolicyV1 {
            max_lifetime_days: 30,
            required_overlap_seconds: 86_400,
            require_dual_signatures: true,
        },
        telemetry_endpoint: None,
    }
}

fn write_rad_file(path: &Path, rad: ResolverAttestationDocumentV1) {
    let bytes = to_bytes(&vec![rad]).expect("serialize rad");
    fs::write(path, bytes).expect("write rad");
}

fn sample_directory_bundle() -> (ResolverAttestationDocumentV1, Vec<u8>, Vec<u8>) {
    let rad = base_rad();
    let resolver_id_hex = hex::encode(rad.resolver_id);
    let rad_digest = compute_rad_digest(&rad).expect("digest");
    let leaf_hash = hash_leaf(&rad_digest);
    let root_hash = leaf_hash;

    let directory_json = Value::Object({
        let mut map = json::Map::new();
        map.insert("version".into(), Value::Number(Number::U64(1)));
        map.insert(
            "created_at_ms".into(),
            Value::Number(Number::U64(1_700_000_500)),
        );
        map.insert("rad_count".into(), Value::Number(Number::U64(1)));
        map.insert("merkle_root".into(), Value::String(hex::encode(root_hash)));
        map.insert(
            "rad".into(),
            Value::Array(vec![Value::Object({
                let mut rad_map = json::Map::new();
                rad_map.insert("resolver_id".into(), Value::String(resolver_id_hex));
                rad_map.insert("rad_sha256".into(), Value::String(hex::encode(rad_digest)));
                rad_map.insert("leaf_hash".into(), Value::String(hex::encode(leaf_hash)));
                rad_map.insert(
                    "file".into(),
                    Value::String(format!("rad/{}.norito", hex::encode(rad.resolver_id))),
                );
                rad_map
            })]),
        );
        map
    });
    let (directory_bytes, _) =
        canonicalize_json_bytes(&json::to_vec(&directory_json).expect("serialize directory"))
            .expect("canonicalize directory");
    let directory_sha = sha256_digest(&directory_bytes);

    let builder_keys = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
    let mut record = ResolverDirectoryRecordV1 {
        root_hash,
        record_version: 1,
        created_at_ms: 1_700_000_500,
        rad_count: 1,
        directory_json_sha256: directory_sha,
        previous_root: None,
        published_at_block: 1,
        published_at_unix: 1_700_000_600,
        proof_manifest_cid: "bafy-directory".parse().expect("cid"),
        builder_public_key: builder_keys.public_key().clone(),
        builder_signature: Signature::from_bytes(&[0; 64]),
    };
    let payload = signing_payload_bytes(&record).expect("signing payload");
    record.builder_signature = Signature::new(builder_keys.private_key(), &payload);
    let record_bytes = norito::json::to_vec(&record).expect("serialize record");

    (rad, directory_bytes, record_bytes)
}

fn write_sample_bundle(root: &Path) -> PathBuf {
    let (rad, directory_bytes, record_bytes) = sample_directory_bundle();
    let record_path = root.join("record.json");
    fs::write(&record_path, &record_bytes).expect("write record");
    let directory_path = root.join("directory.json");
    fs::write(&directory_path, &directory_bytes).expect("write directory");
    let rad_dir = root.join("rad");
    fs::create_dir_all(&rad_dir).expect("create rad dir");
    let resolver_id_hex = hex::encode(rad.resolver_id);
    let rad_path = rad_dir.join(format!("{resolver_id_hex}.norito"));
    write_rad_file(&rad_path, rad);
    rad_path
}

fn hash_leaf(rad_digest: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(rad_digest);
    hasher.finalize().into()
}
