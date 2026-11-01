//! Native public-file custody feeding the unchanged archived finality verifier.

use super::*;
use iroha_fs::{OwnerDirectory, PublishMode};
use std::{fs, process::Command};

fn directory() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::Builder::new()
        .prefix("iroha-retained-network-")
        .tempdir_in(std::env::temp_dir())
        .unwrap();
    let directory = OwnerDirectory::open_or_create(temporary.path().join("public-inputs")).unwrap();
    (temporary, directory)
}

fn reference(directory: &OwnerDirectory, name: &str, bytes: &[u8]) -> PublicFile {
    let path = directory.path().join(name);
    let original = iroha_fs::StrictPublicInput::open(&path).unwrap();
    let identity = WindowsIdentity::from_snapshot(&original.snapshot().unwrap());
    assert_eq!(original.read(bytes.len()).unwrap(), bytes);
    PublicFile {
        path,
        sha256: hex::encode(Sha256::digest(bytes)),
        bytes: bytes.len() as u64,
        identity: NativeIdentity::Windows(identity),
    }
}

fn grant(path: &Path, rights: &str) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", rights])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_tagged_identity_roundtrips_all_original_security_fields() {
    let (_temporary, directory) = directory();
    let bytes = b"independently selected public input";
    directory
        .write_atomic("record", bytes, PublishMode::CreateNew)
        .unwrap();
    let reference = reference(&directory, "record", bytes);
    let encoded = json::to_json(&reference).unwrap();
    let object: json::Value = json::from_slice(encoded.as_bytes()).unwrap();
    assert_eq!(object["identity"]["platform"].as_str(), Some("windows"));
    assert!(
        object["identity"]["metadata"]["volume_serial"]
            .as_u64()
            .is_some()
    );
    assert_eq!(
        object["identity"]["metadata"]["file_id"]
            .as_str()
            .unwrap()
            .len(),
        32
    );
    let decoded: PublicFile = json::from_slice(encoded.as_bytes()).unwrap();
    assert_eq!(decoded.identity, reference.identity);
    assert_eq!(decoded.sha256, reference.sha256);
    assert_eq!(decoded.bytes, reference.bytes);
    let held = HeldPublic::open(&decoded).unwrap();
    assert_eq!(held.bytes, bytes);
    held.check().unwrap();
}

#[test]
fn native_identity_rejects_every_changed_field_and_retired_or_foreign_metadata() {
    let (_temporary, directory) = directory();
    let bytes = b"exact public original";
    directory
        .write_atomic("record", bytes, PublishMode::CreateNew)
        .unwrap();
    let reference = reference(&directory, "record", bytes);
    for changed in 0..13 {
        let mut foreign = reference.clone();
        let NativeIdentity::Windows(identity) = &mut foreign.identity else {
            unreachable!()
        };
        match changed {
            0 => identity.volume_serial ^= 1,
            1 => {
                identity.file_id = if identity.file_id == "00".repeat(16) {
                    "11".repeat(16)
                } else {
                    "00".repeat(16)
                }
            }
            2 => identity.length += 1,
            3 => identity.attributes ^= 1,
            4 => identity.links += 1,
            5 => identity.modified_100ns += 1,
            6 => identity.changed_100ns += 1,
            7 => identity.owner_sid.push_str("00"),
            8 => identity.group_sid = Some("00".repeat(8)),
            9 => identity.dacl.push_str("00"),
            10 => identity.control ^= 1,
            11 => identity.file_id = "AA".repeat(16),
            _ => identity.file_id.pop().map(|_| ()).unwrap(),
        }
        assert!(
            HeldPublic::open(&foreign).is_err(),
            "changed field {changed}"
        );
    }
    let mut foreign = reference.clone();
    foreign.identity = NativeIdentity::Unix(UnixIdentity::from_array([1; 9]));
    assert!(HeldPublic::open(&foreign).is_err());
    foreign = reference.clone();
    foreign.sha256 = "01".repeat(32);
    assert!(HeldPublic::open(&foreign).is_err());
    foreign = reference;
    foreign.bytes = MAX_FILE + 1;
    assert!(HeldPublic::open(&foreign).is_err());
}

#[test]
fn native_retained_public_proofs_preserve_signatures_prefix_and_original_windows() {
    let (_temporary, directory) = directory();
    let (mut peer, attestation, fixture) = tests::synthetic_peer();
    let genesis_bytes = json::to_json(fixture.genesis_proof()).unwrap().into_bytes();
    let tip_bytes = json::to_json(fixture.latest()).unwrap().into_bytes();
    let attestation_bytes = json::to_json(&attestation).unwrap().into_bytes();
    for (name, bytes) in [
        ("genesis-proof", &genesis_bytes),
        ("tip-proof", &tip_bytes),
        ("attestation", &attestation_bytes),
    ] {
        directory
            .write_atomic(name, bytes, PublishMode::CreateNew)
            .unwrap();
    }
    let genesis =
        HeldPublic::open(&reference(&directory, "genesis-proof", &genesis_bytes)).unwrap();
    let tip = HeldPublic::open(&reference(&directory, "tip-proof", &tip_bytes)).unwrap();
    peer.attestation = reference(&directory, "attestation", &attestation_bytes);
    let retained = HeldPublic::open(&peer.attestation).unwrap();
    let validators = iroha_genesis::signed_genesis_validator_pops(fixture.genesis())
        .unwrap()
        .into_iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key,
            proof_of_possession,
        })
        .collect();
    let mut verifier =
        SumeragiFinalityVerifier::new(fixture.genesis(), fixture.chain_id(), validators).unwrap();
    verifier
        .verify(&json::from_slice::<SumeragiFinalityProof>(&genesis.bytes).unwrap())
        .unwrap();
    let decoded: SumeragiFinalityAttestation = json::from_slice(&retained.bytes).unwrap();
    assert!(verify_peer(&peer, &decoded, fixture.network_id(), &verifier).is_err());
    verifier
        .verify(&json::from_slice::<SumeragiFinalityProof>(&tip.bytes).unwrap())
        .unwrap();
    verify_peer(&peer, &decoded, fixture.network_id(), &verifier).unwrap();
    let mut wrong_window = peer.clone();
    wrong_window.read_finish_ns = 999_998_000_000;
    assert!(verify_peer(&wrong_window, &decoded, fixture.network_id(), &verifier).is_err());
    let mut wrong_challenge = peer.clone();
    wrong_challenge.challenge = "08".repeat(32);
    assert!(verify_peer(&wrong_challenge, &decoded, fixture.network_id(), &verifier).is_err());
    let mut unsigned_change = decoded;
    unsigned_change.body.observed_at_unix_ms += 1;
    assert!(verify_peer(&peer, &unsigned_change, fixture.network_id(), &verifier).is_err());
    for original in [genesis, tip, retained] {
        original.check().unwrap();
    }
}

#[test]
fn native_same_bytes_replacement_requires_new_identity_and_independent_request_digest() {
    let (_temporary, directory) = directory();
    let bytes = b"public archived bytes";
    directory
        .write_atomic("record", bytes, PublishMode::CreateNew)
        .unwrap();
    let old = reference(&directory, "record", bytes);
    directory
        .write_atomic("record", bytes, PublishMode::Replace)
        .unwrap();
    assert!(HeldPublic::open(&old).is_err());
    let new = reference(&directory, "record", bytes);
    assert_eq!(old.sha256, new.sha256);
    assert_ne!(old.identity, new.identity);
    let old_request = json::to_json(&old).unwrap();
    let new_request = json::to_json(&new).unwrap();
    assert_ne!(
        Sha256::digest(old_request.as_bytes()),
        Sha256::digest(new_request.as_bytes())
    );
    HeldPublic::open(&new).unwrap().check().unwrap();
}

#[test]
fn native_public_custody_rejects_hardlinks_reparse_routes_and_foreign_writes() {
    let (temporary, directory) = directory();
    let bytes = b"private local public data";
    directory
        .write_atomic("record", bytes, PublishMode::CreateNew)
        .unwrap();
    let reference = reference(&directory, "record", bytes);
    let alias = directory.path().join("alias");
    fs::hard_link(&reference.path, &alias).unwrap();
    assert!(HeldPublic::open(&reference).is_err());
    fs::remove_file(alias).unwrap();
    let junction = temporary.path().join("junction");
    let output = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut redirected = reference.clone();
    redirected.path = junction.join("record");
    let refused = HeldPublic::open(&redirected).is_err();
    fs::remove_dir(junction).unwrap();
    assert!(refused);
    grant(&reference.path, "*S-1-1-0:(W)");
    assert!(HeldPublic::open(&reference).is_err());
}

#[test]
fn native_retained_request_security_and_private_parent_changes_refuse_final_checks() {
    for parent_change in [false, true] {
        let (_temporary, directory) = directory();
        let bytes = b"public request fixture";
        directory
            .write_atomic("request", bytes, PublishMode::CreateNew)
            .unwrap();
        let path = directory.path().join("request");
        let held = HeldPublic::open_request(&path, &hex::encode(Sha256::digest(bytes))).unwrap();
        assert_eq!(held.bytes, bytes);
        if parent_change {
            directory
                .write_atomic("unrelated", b"new namespace", PublishMode::CreateNew)
                .unwrap();
        } else {
            grant(&path, "*S-1-1-0:(R)");
        }
        assert!(held.check().is_err());
    }
}

#[test]
fn native_failed_request_never_reports_verification_and_releases_originals() {
    let (_temporary, directory) = directory();
    let bytes = br#"{"schema":"iroha.bridge.retained-network-request.v1"}"#;
    directory
        .write_atomic("request", bytes, PublishMode::CreateNew)
        .unwrap();
    let path = directory.path().join("request");
    for digest in [hex::encode(Sha256::digest(bytes)), "01".repeat(32)] {
        let mut context = crate::tests::test_context(CliOutputFormat::Json);
        assert!(
            run(
                &mut context,
                Args {
                    request: path.clone(),
                    request_sha256: digest
                }
            )
            .is_err()
        );
        assert!(context.write.is_empty());
        assert!(context.err_write.is_empty());
        let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
        drop(writer);
    }
}
