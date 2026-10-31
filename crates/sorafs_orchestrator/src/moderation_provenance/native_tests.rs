//! Real Windows provenance custody, canonical storage, lock and publication controls.

use super::*;
use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::sorafs::moderation::{
    MODERATION_SIGNED_RESULT_VERSION_V1, ModerationSignedScreeningBodyV1,
    ModerationSignedScreeningResultV1,
};
use std::{fs, process::Command};

fn signed_payload(timestamp: u64) -> ModerationProvenancePayloadV1 {
    let keypair = KeyPair::try_random().expect("keypair");
    let mut body = ModerationSignedScreeningBodyV1 {
        schema_version: MODERATION_SIGNED_RESULT_VERSION_V1,
        manifest_id: [1; 16],
        manifest_digest: [2; 32],
        runner_hash: [3; 32],
        trust_policy_id: [4; 16],
        trust_policy_digest: [5; 32],
        subject: "cid:provenance-test".to_string(),
        subject_digest: [6; 32],
        model_scores: Vec::new(),
        combined_score_bps: 0,
        verdict: "pass".to_string(),
        screened_at_unix: timestamp,
        expires_at_unix: timestamp + 60,
        policy_digest: [7; 32],
        evidence_digest: [0; 32],
        notes: None,
    };
    body.refresh_evidence_digest().expect("evidence digest");
    let signature = SignatureOf::try_new(keypair.private_key(), &body).expect("signature");
    ModerationProvenancePayloadV1::SignedScreeningResult(ModerationSignedScreeningResultV1 {
        body,
        signer_public_key: keypair.public_key().clone(),
        signature,
    })
}

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("native provenance fixture");
    let root = temp
        .path()
        .canonicalize()
        .expect("canonical native fixture");
    (temp, root.join("provenance.to"))
}

fn lock_path(path: &Path) -> PathBuf {
    path.parent().unwrap().join(".provenance.to.lock")
}

fn append(log: &mut ModerationProvenanceLogV1) -> Result<(), ModerationProvenanceStoreError> {
    let timestamp = 100 + log.entries.len() as u64;
    log.append(signed_payload(timestamp), timestamp + 1)?;
    Ok(())
}

fn acl(path: &Path, args: &[&str]) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(args)
        .output()
        .expect("native ACL operation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn junction(path: &Path, target: &Path) {
    let output = Command::new("cmd.exe")
        .current_dir(path.parent().expect("junction parent"))
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create native junction");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_store_creates_private_bytes_and_reopens_canonical_chain() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [1; 16]).expect("create");
    store.update(append).expect("append first");
    let other = ModerationProvenanceStoreV1::open(&path, [1; 16]).expect("reopen");
    other.update(append).expect("append latest snapshot");
    let snapshot = store.snapshot().expect("read latest");
    assert_eq!(snapshot.entries.len(), 2);
    snapshot.validate_chain().expect("canonical chain");
    assert_eq!(
        fs::read(&path).unwrap(),
        norito::to_bytes(&snapshot).unwrap()
    );
    drop(iroha_fs::RetainedFile::open_private(&path).expect("new output is private"));
    drop(iroha_fs::RetainedFile::open_private(lock_path(&path)).expect("lock is private"));
}

#[test]
fn native_persistent_lock_excludes_another_handle_and_reopens_after_close() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [2; 16]).unwrap();
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let original = parent.open_existing_lock(".provenance.to.lock").unwrap();
    let identity = iroha_fs::FileIdentity::of(&original).unwrap();
    original.try_lock().expect("independent writer lock");
    assert!(
        matches!(store.snapshot(), Err(ModerationProvenanceStoreError::Locked(ref p)) if p == &path)
    );
    drop(original);
    store
        .snapshot()
        .expect("stale persistent filename is not a held lock");
    let reopened = parent.open_existing_lock(".provenance.to.lock").unwrap();
    assert_eq!(iroha_fs::FileIdentity::of(&reopened).unwrap(), identity);
    reopened
        .try_lock()
        .expect("prior transaction released original lock");
    parent
        .require_same_lock(".provenance.to.lock", &reopened)
        .unwrap();
}

#[test]
fn native_operation_error_releases_lock_without_changing_bytes() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [3; 16]).unwrap();
    let before = fs::read(&path).unwrap();
    let result: Result<(), _> = store.update(|_| {
        Err(ModerationProvenanceStoreError::Encode(
            "first operation error".into(),
        ))
    });
    assert!(
        matches!(result, Err(ModerationProvenanceStoreError::Encode(ref message)) if message == "first operation error")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    store
        .update(append)
        .expect("lock released on failed operation");
}

#[test]
fn native_wrong_log_id_and_corrupt_chain_do_not_overwrite_originals() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [4; 16]).unwrap();
    store.update(append).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        ModerationProvenanceStoreV1::open(&path, [5; 16]),
        Err(ModerationProvenanceStoreError::LogIdMismatch)
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut corrupt = store.snapshot().unwrap();
    corrupt.head_digest[0] ^= 1;
    let bytes = norito::to_bytes(&corrupt).unwrap();
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    parent
        .write_atomic("provenance.to", &bytes, PublishMode::Replace)
        .unwrap();
    assert!(matches!(
        store.snapshot(),
        Err(ModerationProvenanceStoreError::InvalidChain(_))
    ));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn native_empty_truncated_and_trailing_bytes_fail_closed() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [6; 16]).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    for invalid in [Vec::new(), bytes[..bytes.len() / 2].to_vec(), trailing] {
        parent
            .write_atomic("provenance.to", &invalid, PublishMode::Replace)
            .unwrap();
        assert!(matches!(
            store.snapshot(),
            Err(ModerationProvenanceStoreError::InvalidFile { .. }
                | ModerationProvenanceStoreError::Decode { .. }
                | ModerationProvenanceStoreError::NonCanonical(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }
}

#[test]
fn native_oversized_extent_refuses_before_materialization() {
    let (_temp, path) = fixture();
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let original = parent.open_lock("provenance.to").unwrap();
    original.set_len(MAX_PROVENANCE_FILE_BYTES + 1).unwrap();
    drop(original);
    let err = ModerationProvenanceStoreV1::open(&path, [7; 16]).expect_err("oversized original");
    assert!(
        matches!(err, ModerationProvenanceStoreError::InvalidFile { ref reason, .. } if reason.contains("outside 1..="))
    );
    assert_eq!(
        fs::metadata(path).unwrap().len(),
        MAX_PROVENANCE_FILE_BYTES + 1
    );
}

#[test]
fn native_public_read_permissions_do_not_require_private_input_labeling() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [8; 16]).unwrap();
    acl(path.parent().unwrap(), &["/grant", "*S-1-1-0:(RX)"]);
    acl(&path, &["/grant", "*S-1-1-0:(R)"]);
    let before = fs::read(&path).unwrap();
    store.snapshot().expect("safe public readers are allowed");
    assert_eq!(fs::read(&path).unwrap(), before);
    store
        .update(append)
        .expect("replace with private canonical bytes");
    drop(iroha_fs::RetainedFile::open_private(&path).expect("published output is private"));
}

#[test]
fn native_unsafe_parent_refuses_before_creating_lock_or_segment() {
    let (_temp, path) = fixture();
    acl(path.parent().unwrap(), &["/grant", "*S-1-1-0:(W)"]);
    assert!(ModerationProvenanceStoreV1::open(&path, [9; 16]).is_err());
    assert!(!path.exists());
    assert!(!lock_path(&path).exists());
}

#[test]
fn native_segment_and_lock_foreign_write_permissions_refuse() {
    for target_is_lock in [false, true] {
        let (_temp, path) = fixture();
        let store = ModerationProvenanceStoreV1::open(&path, [10; 16]).unwrap();
        let bytes = fs::read(&path).unwrap();
        let changed = if target_is_lock {
            lock_path(&path)
        } else {
            path.clone()
        };
        acl(&changed, &["/grant", "*S-1-1-0:(W)"]);
        assert!(store.snapshot().is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn native_segment_and_lock_hardlinks_refuse_without_mutation() {
    for target_is_lock in [false, true] {
        let (_temp, path) = fixture();
        let store = ModerationProvenanceStoreV1::open(&path, [11; 16]).unwrap();
        let original = if target_is_lock {
            lock_path(&path)
        } else {
            path.clone()
        };
        let bytes = fs::read(&original).unwrap();
        let alias = path.with_file_name("outside-alias");
        fs::hard_link(&original, &alias).unwrap();
        assert!(store.snapshot().is_err());
        assert_eq!(fs::read(&original).unwrap(), bytes);
        assert_eq!(fs::read(alias).unwrap(), bytes);
    }
}

#[test]
fn native_parent_and_leaf_junctions_refuse_without_outside_mutation() {
    for kind in ["parent", "segment", "lock"] {
        let (_temp, path) = fixture();
        let root = path.parent().unwrap();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        let sentinel = outside.join("sentinel");
        fs::write(&sentinel, b"preserve outside").unwrap();
        let target = match kind {
            "parent" => {
                let linked = root.join("linked");
                junction(&linked, &outside);
                linked.join("provenance.to")
            }
            "segment" => {
                junction(&path, &outside);
                path.clone()
            }
            _ => {
                junction(&lock_path(&path), &outside);
                path.clone()
            }
        };
        assert!(ModerationProvenanceStoreV1::open(&target, [12; 16]).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"preserve outside");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }
}

#[test]
fn native_retained_parent_blocks_replacement_during_transaction() {
    let (_temp, path) = fixture();
    let root = path.parent().unwrap().join("owned");
    fs::create_dir(&root).unwrap();
    let path = root.join("provenance.to");
    let store = ModerationProvenanceStoreV1::open(&path, [13; 16]).unwrap();
    let replacement = root.with_file_name("moved");
    store
        .update(|log| {
            assert!(
                fs::rename(&root, &replacement).is_err(),
                "retained parent must exclude replacement"
            );
            append(log)
        })
        .unwrap();
    assert!(!replacement.exists());
    assert_eq!(store.snapshot().unwrap().entries.len(), 1);
}

#[test]
fn native_lock_and_parent_custody_changes_prevent_publication() {
    for change_parent in [false, true] {
        let (_temp, path) = fixture();
        let store = ModerationProvenanceStoreV1::open(&path, [14; 16]).unwrap();
        let bytes = fs::read(&path).unwrap();
        let changed = if change_parent {
            path.parent().unwrap().to_path_buf()
        } else {
            lock_path(&path)
        };
        let result = store.update(|log| {
            append(log)?;
            acl(&changed, &["/grant", "*S-1-1-0:(W)"]);
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn native_first_operation_error_survives_final_custody_refusal() {
    let (_temp, path) = fixture();
    let store = ModerationProvenanceStoreV1::open(&path, [15; 16]).unwrap();
    let before = fs::read(&path).unwrap();
    let result: Result<(), _> = store.update(|_| {
        acl(&lock_path(&path), &["/grant", "*S-1-1-0:(W)"]);
        Err(ModerationProvenanceStoreError::Encode(
            "original first failure".into(),
        ))
    });
    assert!(
        matches!(result, Err(ModerationProvenanceStoreError::Encode(ref text)) if text == "original first failure")
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn native_publication_failures_preserve_complete_old_or_new_chain_and_release_lock() {
    for failure in [
        AtomicWriteStep::Publish,
        AtomicWriteStep::Published,
        AtomicWriteStep::SyncDirectory,
    ] {
        let (_temp, path) = fixture();
        let store = ModerationProvenanceStoreV1::open(&path, [16; 16]).unwrap();
        let before = fs::read(&path).unwrap();
        let mut published = false;
        let result = transact_observed(&path, [16; 16], true, append, |step, _| {
            if step == AtomicWriteStep::Published {
                published = true;
            }
            if step == failure {
                Err(io::Error::other("injected original publication failure"))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(ModerationProvenanceStoreError::Io { ref source, .. }) if source.to_string() == "injected original publication failure")
        );
        let after = store
            .snapshot()
            .expect("release lock and reconcile actual current bytes");
        after.validate_chain().unwrap();
        assert_eq!(after.entries.len(), usize::from(published));
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes, norito::to_bytes(&after).unwrap());
        if !published {
            assert_eq!(bytes, before);
        }
        let mut names = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                OsString::from(".provenance.to.lock"),
                OsString::from("provenance.to")
            ]
        );
    }
}

#[test]
fn native_original_remains_retained_through_canonical_inspection() {
    let (_temp, path) = fixture();
    ModerationProvenanceStoreV1::open(&path, [17; 16]).unwrap();
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let decoded = read_original(&parent, &path, |bytes| {
        assert!(
            fs::OpenOptions::new().write(true).open(&path).is_err(),
            "live original must exclude another data writer during decode"
        );
        assert!(
            fs::rename(&path, path.with_file_name("replacement")).is_err(),
            "live original must exclude name replacement during decode"
        );
        decode_log(&path, bytes)
    })
    .unwrap()
    .unwrap();
    assert_eq!(decoded.log_id, [17; 16]);
    assert!(!path.with_file_name("replacement").exists());
    drop(
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("reader closes after inspection"),
    );
}

#[test]
fn native_custody_change_during_decode_refuses_before_returning_log() {
    let (_temp, path) = fixture();
    ModerationProvenanceStoreV1::open(&path, [18; 16]).unwrap();
    let parent = OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let result = read_original(&parent, &path, |bytes| {
        let log = decode_log(&path, bytes)?;
        acl(&path, &["/grant", "*S-1-1-0:(W)"]);
        Ok(log)
    });
    assert!(
        result.is_err(),
        "successful ACL mutation invalidates retained read"
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
