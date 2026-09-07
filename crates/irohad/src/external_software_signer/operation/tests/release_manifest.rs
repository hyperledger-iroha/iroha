//! Real private-journal staging with simulated opaque hardware and authoritative completion.

use super::super::release_manifest::*;
use super::*;
use sorafs_manifest::signer::{
    protocol::{SignerOperationActionV1, signer_operation_signatures_digest_v1},
    receipt::{
        SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1,
        SignerReleaseManifestExpectedV1, signer_release_manifest_digest_v1,
        verify_release_manifest_signer_receipt_v1,
    },
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
};

fn manifest() -> &'static [u8] {
    b"{\"schema\":\"iroha.release-manifest.v1\",\"version\":\"1.0.0\"}\n"
}
fn expected() -> SignerReleaseManifestExpectedV1 {
    SignerReleaseManifestExpectedV1 {
        operation_id: [0x91; 32],
        manifest_digest: signer_release_manifest_digest_v1(manifest()),
        manifest_size: manifest().len() as u64,
    }
}
fn ceremony(
    directory: &std::path::Path,
) -> (SignerReleaseManifestServiceV1, Arc<Source>, Arc<Provider>) {
    let fixture = fixture_for(
        SignerRoleV1::ReleaseManifest,
        SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into(),
        },
    );
    fixture.source.state.lock().unwrap().expected_journal = Some(directory.to_owned());
    let service = SignerReleaseManifestServiceV1::new(
        fixture.coordinator,
        expected(),
        intent(SignerOperationActionV1::Sign).previous_audit,
        SignerReleaseManifestJournalV1::open(directory).unwrap(),
    )
    .unwrap();
    (service, fixture.source, fixture.provider)
}
fn private_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

#[test]
fn exact_opaque_signatures_are_durable_before_commit_and_recover_without_key_use() {
    let directory = private_directory();
    let canonical = directory.path().canonicalize().unwrap();
    let (service, source, provider) = ceremony(&canonical);
    let receipt = service.sign(manifest()).unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(source.state.lock().unwrap().journal_checked, 1);
    let bytes = norito::encode_canonical(&receipt).unwrap();
    assert_eq!(
        fs::read(canonical.join(format!(
            "{}.receipt.norito",
            hex::encode(expected().operation_id)
        )))
        .unwrap(),
        bytes
    );
    assert_eq!(service.recover(manifest()).unwrap(), receipt);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert!(service.sign(manifest()).is_err());
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        4,
        "duplicate id cannot trigger any key retry"
    );
    let state = source.state.lock().unwrap();
    let completion = SignerCompletedOperationV1 {
        operation_id: receipt.intent.operation_id,
        intent_digest: receipt.intent.digest().unwrap(),
        original_custody: receipt.request.original_custody,
        reservation: receipt.reservation,
        commitment: receipt.commitment,
        signatures_digest: signer_operation_signatures_digest_v1(&receipt.signatures).unwrap(),
        completed_at_unix_ms: state.context.now_unix_ms,
        anchor: SignerOperationFinalizedAnchorV1 {
            height: state.context.current_anchor.height,
            block_hash: state.context.current_anchor.block_hash,
            operation_state_digest: [0x92; 32],
        },
    };
    let record: SignerCustodyRecordV1 = norito::decode_canonical(&receipt.custody_record).unwrap();
    let trust = SignerCustodyTrustV1 {
        authority: record.statement.authority.clone(),
        public_key: key(0x31).public_key().clone(),
        active_from_unix_ms: 500,
        active_until_unix_ms: 3_000,
        max_validity_ms: 2_000,
        max_anchor_age_ms: 500,
    };
    verify_release_manifest_signer_receipt_v1(
        &bytes,
        manifest(),
        &receipt.signatures[0].signature,
        &expected(),
        &source.binding,
        &trust,
        &state.context,
        &completion,
    )
    .expect("runtime producer emits the exact shared public receipt contract");
}

#[test]
fn wrong_reviewed_bytes_and_wrong_purpose_never_invoke_hardware() {
    let directory = private_directory();
    let canonical = directory.path().canonicalize().unwrap();
    let (service, _, provider) = ceremony(&canonical);
    assert!(service.sign(b"{\"version\":\"substituted\"}\n").is_err());
    assert!(service.recover(b"unreviewed").is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    drop(service);
    let fixture = fixture();
    assert!(
        SignerReleaseManifestServiceV1::new(
            fixture.coordinator,
            expected(),
            intent(SignerOperationActionV1::Sign).previous_audit,
            SignerReleaseManifestJournalV1::open(&canonical).unwrap()
        )
        .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn failed_commit_leaves_pending_tombstone_but_never_releases_or_retries_signatures() {
    let directory = private_directory();
    let canonical = directory.path().canonicalize().unwrap();
    let (service, source, provider) = ceremony(&canonical);
    source.state.lock().unwrap().fail_commit = true;
    assert!(service.sign(manifest()).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(fs::read_dir(&canonical).unwrap().count(), 1);
    assert!(service.recover(manifest()).is_err());
    assert!(service.sign(manifest()).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn post_commit_journal_substitution_and_revocation_prevent_release_and_recovery() {
    for tamper_journal in [false, true] {
        let directory = private_directory();
        let canonical = directory.path().canonicalize().unwrap();
        let (service, source, provider) = ceremony(&canonical);
        {
            let mut state = source.state.lock().unwrap();
            if tamper_journal {
                state.mutate_journal_after_commit = true;
            } else {
                state.mutate_after_commit = Some(Mutation::SignerRevoked);
            }
        }
        assert!(service.sign(manifest()).is_err());
        assert_eq!(source.state.lock().unwrap().commits, 1);
        assert!(service.recover(manifest()).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    }
}

#[test]
fn journal_refuses_insecure_paths_links_permissions_and_unexpected_entries() {
    let directory = private_directory();
    let canonical = directory.path().canonicalize().unwrap();
    let target = canonical.join("journal");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(SignerReleaseManifestJournalV1::open(&target).is_ok());
    assert!(SignerReleaseManifestJournalV1::open(std::path::Path::new("relative")).is_err());
    let link = canonical.join("link");
    symlink(&target, &link).unwrap();
    assert!(SignerReleaseManifestJournalV1::open(&link).is_err());
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(SignerReleaseManifestJournalV1::open(&target).is_err());
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(target.join("unexpected"), b"not a receipt").unwrap();
    assert!(SignerReleaseManifestJournalV1::open(&target).is_err());
}

#[test]
fn every_ancestor_and_original_receipt_inode_stay_pinned_through_recovery() {
    for replace_ancestor in [false, true] {
        let directory = private_directory();
        let canonical = directory.path().canonicalize().unwrap();
        let ancestor = canonical.join("parent");
        fs::create_dir(&ancestor).unwrap();
        fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
        let leaf = ancestor.join("journal");
        fs::create_dir(&leaf).unwrap();
        fs::set_permissions(&leaf, fs::Permissions::from_mode(0o700)).unwrap();
        let (service, _, provider) = ceremony(&leaf);
        let receipt = service.sign(manifest()).unwrap();
        if replace_ancestor {
            fs::rename(&ancestor, canonical.join("original")).unwrap();
            fs::create_dir(&ancestor).unwrap();
            fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
            symlink(canonical.join("original/journal"), &leaf).unwrap();
        } else {
            let name = leaf.join(format!(
                "{}.receipt.norito",
                hex::encode(expected().operation_id)
            ));
            fs::rename(&name, canonical.join("original-receipt")).unwrap();
            symlink(canonical.join("original-receipt"), &name).unwrap();
        }
        assert!(service.recover(manifest()).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
        assert_eq!(receipt.signatures.len(), 4);
    }
}

#[test]
fn linked_or_public_receipt_and_truncated_tombstone_cannot_be_recovered() {
    for mode in [0, 1, 2] {
        let directory = private_directory();
        let canonical = directory.path().canonicalize().unwrap();
        let (service, _, provider) = ceremony(&canonical);
        service.sign(manifest()).unwrap();
        let path = canonical.join(format!(
            "{}.receipt.norito",
            hex::encode(expected().operation_id)
        ));
        match mode {
            0 => fs::hard_link(&path, canonical.join("substituted-link")).unwrap(),
            1 => fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap(),
            _ => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&path, b"partial").unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            }
        }
        assert!(service.recover(manifest()).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    }
}

#[test]
fn journal_lease_is_exclusive_across_instances_and_processes() {
    const CHILD_DIRECTORY: &str = "IROHA_RELEASE_JOURNAL_LEASE_TEST_CHILD_DIRECTORY";
    if let Some(path) = std::env::var_os(CHILD_DIRECTORY) {
        assert!(
            SignerReleaseManifestJournalV1::open(std::path::Path::new(&path)).is_err(),
            "separate process must not acquire an already held journal"
        );
        return;
    }
    let directory = private_directory();
    let canonical = directory.path().canonicalize().unwrap();
    let owner = SignerReleaseManifestJournalV1::open(&canonical).unwrap();
    assert!(
        SignerReleaseManifestJournalV1::open(&canonical).is_err(),
        "independent descriptor in the same process must not bypass the retention lease"
    );
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("external_software_signer::operation::tests::release_manifest::journal_lease_is_exclusive_across_instances_and_processes")
        .arg("--nocapture")
        .env(CHILD_DIRECTORY, &canonical)
        .output().unwrap();
    assert!(
        child.status.success(),
        "child lock contention regression failed"
    );
    drop(owner);
    SignerReleaseManifestJournalV1::open(&canonical)
        .expect("lease releases only after the original owner drops");
}
