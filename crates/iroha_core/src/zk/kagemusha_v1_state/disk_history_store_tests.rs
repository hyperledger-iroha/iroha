//! Real-file and process-restart tests for the production disk history store.

use super::*;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
    process::Command,
};

type Store = KagemushaDiskAuthenticatedHistoryStoreV1;

fn digest(label: &[u8]) -> DigestV1 {
    Sha256::digest(label).into()
}
fn key() -> SigningKey {
    SigningKey::from_bytes((&[0x5a; 32]).into()).unwrap()
}
fn public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}
fn credentials() -> KagemushaHistoryDeviceCredentialsV1 {
    KagemushaHistoryDeviceCredentialsV1::new(digest(b"disk-profile"), [(7, public_key(&key()))])
        .unwrap()
}
fn binding() -> DigestV1 {
    digest(b"disk-lane")
}
fn location() -> (tempfile::TempDir, PathBuf) {
    let parent = tempfile::tempdir().unwrap();
    let path = parent.path().canonicalize().unwrap().join("history");
    (parent, path)
}
fn create(path: &Path) -> Store {
    Store::create_new(path, binding(), credentials(), u64::MAX).unwrap()
}
fn reopen(path: &Path) -> Store {
    Store::open_existing(path, binding(), credentials(), u64::MAX).unwrap()
}
fn certificate(
    tx: &KagemushaPreparedHistoryCasV1,
    counter: u128,
) -> VerifiedKagemushaHistoryRootSelectionV1 {
    signed_by(tx, counter, &key())
}
fn signed_by(
    tx: &KagemushaPreparedHistoryCasV1,
    counter: u128,
    key: &SigningKey,
) -> VerifiedKagemushaHistoryRootSelectionV1 {
    let subject =
        KagemushaHistoryRootSelectionSubjectV1::new(tx, digest(b"disk-profile"), 7, counter);
    let signature: Signature = key.sign(&subject.signing_bytes().unwrap());
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaHistoryRootSelectionCertificateV1::new(
        subject,
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref()).unwrap(),
    )
    .verify(digest(b"disk-profile"), &public_key(key))
    .unwrap()
}
fn transaction(label: &[u8], roots: KagemushaHistoryRootsV1) -> KagemushaPreparedHistoryCasV1 {
    let replay = KagemushaHistoryNodeRecordV1::leaf(
        KagemushaHistoryTreeV1::Replay,
        digest(&[label, b"replay"].concat()),
        digest(b"replay value"),
    )
    .unwrap();
    let decision = KagemushaHistoryNodeRecordV1::leaf(
        KagemushaHistoryTreeV1::TerminalDecision,
        digest(&[label, b"decision"].concat()),
        digest(b"decision value"),
    )
    .unwrap();
    KagemushaPreparedHistoryCasV1::new(
        KagemushaHistoryRootSelectionV1::both(
            KagemushaHistoryRootCasV1::new(roots.replay(), replay.content_address().unwrap()),
            KagemushaHistoryRootCasV1::new(
                roots.terminal_decision(),
                decision.content_address().unwrap(),
            ),
        ),
        vec![replay, decision],
        digest(b"disk-history-test-attempt"),
    )
    .unwrap()
}

#[test]
fn disk_history_store_reopens_prepare_and_dual_commit_with_exact_retries() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"dual", store.committed_roots());
    let selected = tx.successor_roots_from(store.committed_roots()).unwrap();
    let bytes = tx.wal_bytes().unwrap();
    assert_eq!(
        store.prepare_cas(tx.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::Prepared
    );
    assert_eq!(store.overlay_usage().live_bytes(), bytes);
    let prepared_len = fs::metadata(path.join(JOURNAL_FILE)).unwrap().len();
    drop(store);
    let mut store = reopen(&path);
    assert_eq!(
        store.prepare_cas(tx.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
    );
    assert_eq!(
        fs::metadata(path.join(JOURNAL_FILE)).unwrap().len(),
        prepared_len
    );
    assert_eq!(
        store.recover_prepared(certificate(&tx, 1)).unwrap(),
        KagemushaHistoryRecoveryOutcomeV1::Committed {
            committed_roots: selected
        }
    );
    assert_eq!(store.overlay_usage().live_bytes(), 0);
    drop(store);
    let mut store = reopen(&path);
    let committed_len = fs::metadata(path.join(JOURNAL_FILE)).unwrap().len();
    assert_eq!(store.committed_roots(), selected);
    assert_eq!(validate_committed_history_v1(&store).unwrap(), selected);
    for tree in [
        KagemushaHistoryTreeV1::Replay,
        KagemushaHistoryTreeV1::TerminalDecision,
    ] {
        let root = selected.for_tree(tree);
        assert!(store.read_node(root).unwrap().is_some());
        assert!(
            matches!(store.read_committed_root(tree).unwrap(), KagemushaCommittedRootReadV1::Available { root: actual, node: Some(_) } if actual == root)
        );
    }
    assert_eq!(store.read_node(digest(b"absent")).unwrap(), None);
    assert_eq!(
        store.commit_prepared(certificate(&tx, 1)).unwrap(),
        KagemushaHistoryCommitOutcomeV1::AlreadyCommitted {
            committed_roots: selected
        }
    );
    assert_eq!(
        store.recover_prepared(certificate(&tx, 1)).unwrap(),
        KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted {
            committed_roots: selected
        }
    );
    assert_eq!(
        store.abort_prepared(tx.transaction_id()).unwrap(),
        KagemushaHistoryAbortOutcomeV1::AlreadyCommitted {
            committed_roots: selected
        }
    );
    assert_eq!(
        store.prepare_cas(tx).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyCommitted {
            committed_roots: selected
        }
    );
    assert_eq!(
        fs::metadata(path.join(JOURNAL_FILE)).unwrap().len(),
        committed_len
    );
}

#[test]
fn disk_history_store_retains_abort_tombstones_and_stale_cas() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"abort", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    assert_eq!(
        store.abort_prepared(tx.transaction_id()).unwrap(),
        KagemushaHistoryAbortOutcomeV1::Aborted
    );
    drop(store);
    let mut store = reopen(&path);
    assert_eq!(
        store.abort_prepared(tx.transaction_id()).unwrap(),
        KagemushaHistoryAbortOutcomeV1::AlreadyAborted
    );
    assert_eq!(
        store.prepare_cas(tx.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyAborted
    );
    assert_eq!(
        store.commit_prepared(certificate(&tx, 1)).unwrap(),
        KagemushaHistoryCommitOutcomeV1::Aborted
    );
    assert_eq!(
        store.recover_prepared(certificate(&tx, 1)).unwrap(),
        KagemushaHistoryRecoveryOutcomeV1::Aborted
    );
    let one = transaction(b"one", store.committed_roots());
    let stale = transaction(b"stale", store.committed_roots());
    store.prepare_cas(one.clone()).unwrap();
    store.prepare_cas(stale.clone()).unwrap();
    store.commit_prepared(certificate(&one, 1)).unwrap();
    let roots = store.committed_roots();
    assert!(matches!(
        store.commit_prepared(certificate(&stale, 2)),
        Err(KagemushaHistoryStoreErrorV1::CasConflict { .. })
    ));
    assert_eq!(store.committed_roots(), roots);
    assert_eq!(
        store.abort_prepared(stale.transaction_id()).unwrap(),
        KagemushaHistoryAbortOutcomeV1::Aborted
    );
}

#[test]
fn disk_history_store_reauthenticates_certificates_and_current_epoch_keys() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"authenticated", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    let foreign = SigningKey::from_bytes((&[0x33; 32]).into()).unwrap();
    assert_eq!(
        store.commit_prepared(signed_by(&tx, 1, &foreign)),
        Err(KagemushaHistoryStoreErrorV1::InvalidCertificate)
    );
    store.commit_prepared(certificate(&tx, 1)).unwrap();
    let newer = transaction(b"counter", store.committed_roots());
    store.prepare_cas(newer.clone()).unwrap();
    assert_eq!(
        store.commit_prepared(certificate(&newer, 1)),
        Err(KagemushaHistoryStoreErrorV1::CertificateMismatch)
    );
    drop(store);
    let wrong_keys = KagemushaHistoryDeviceCredentialsV1::new(
        digest(b"disk-profile"),
        [(7, public_key(&foreign))],
    )
    .unwrap();
    assert!(matches!(
        Store::open_existing(&path, binding(), wrong_keys, 0),
        Err(KagemushaHistoryStoreErrorV1::InvalidCertificate)
    ));
    let keys = credentials();
    let reference =
        iroha_data_model::kagemusha::kagemusha_device_key_reference_v1(&public_key(&key()));
    assert!(
        keys.require_current_binding(digest(b"disk-profile"), 7, reference)
            .is_ok()
    );
    assert!(
        keys.require_current_binding(digest(b"other-profile"), 7, reference)
            .is_err()
    );
    assert!(
        keys.require_current_binding(digest(b"disk-profile"), 8, reference)
            .is_err()
    );
    assert!(
        keys.require_current_binding(digest(b"disk-profile"), 7, digest(b"other-key"))
            .is_err()
    );
    assert!(KagemushaHistoryDeviceCredentialsV1::new([0; 32], [(7, public_key(&key()))]).is_err());
    assert!(KagemushaHistoryDeviceCredentialsV1::new(digest(b"profile"), []).is_err());
    assert!(
        KagemushaHistoryDeviceCredentialsV1::new(
            digest(b"profile"),
            [(7, public_key(&key())), (7, public_key(&key()))]
        )
        .is_err()
    );
}

#[test]
fn disk_history_store_lowered_overlay_limit_never_blocks_retained_work() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"capacity", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    drop(store);
    let mut store = Store::open_existing(&path, binding(), credentials(), 0).unwrap();
    assert_eq!(store.overlay_usage().capacity_bytes(), 0);
    assert_eq!(
        store.prepare_cas(tx.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
    );
    let another = transaction(b"new", store.committed_roots());
    assert!(matches!(
        store.prepare_cas(another),
        Err(KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded { .. })
    ));
    store.recover_prepared(certificate(&tx, 1)).unwrap();
    let roots = store.committed_roots();
    drop(store);
    let store = Store::open_existing(&path, binding(), credentials(), 0).unwrap();
    assert_eq!(store.committed_roots(), roots);
    assert_eq!(validate_committed_history_v1(&store).unwrap(), roots);
}

#[test]
fn disk_history_store_identity_map_is_exact_after_restart() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let key = digest(b"credit identity");
    let value = digest(b"envelope");
    let tx = match prepare_history_identity_insert_v1(
        &mut store,
        KagemushaHistoryTreeV1::Replay,
        key,
        value,
        digest(b"disk-history-test-attempt"),
    )
    .unwrap()
    {
        KagemushaHistoryInsertPreparationV1::Prepared { transaction, .. } => transaction,
        other => panic!("expected prepare, got {other:?}"),
    };
    store.commit_prepared(certificate(&tx, 1)).unwrap();
    drop(store);
    let mut store = reopen(&path);
    assert_eq!(
        prepare_history_identity_insert_v1(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            key,
            value,
            digest(b"disk-history-test-attempt")
        )
        .unwrap(),
        KagemushaHistoryInsertPreparationV1::ExactDuplicate
    );
    assert!(
        matches!(prepare_history_identity_insert_v1(&mut store, KagemushaHistoryTreeV1::Replay, key, digest(b"conflict"), digest(b"disk-history-test-attempt")).unwrap(), KagemushaHistoryInsertPreparationV1::Conflict { existing_value_digest } if existing_value_digest == value)
    );
}

#[test]
fn disk_history_store_refuses_missing_replaced_linked_or_unowned_storage() {
    let (_parent, path) = location();
    assert!(Store::open_existing(&path, binding(), credentials(), 1).is_err());
    let mut store = create(&path);
    assert!(matches!(
        Store::open_existing(&path, binding(), credentials(), 1),
        Err(KagemushaHistoryStoreErrorV1::StoreAlreadyOpen)
    ));
    assert!(Store::create_new(&path, binding(), credentials(), 1).is_err());
    let tx = transaction(b"owner", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    let journal = path.join(JOURNAL_FILE);
    let held = path.join("replaced.wal");
    fs::rename(&journal, &held).unwrap();
    fs::copy(&held, &journal).unwrap();
    assert!(store.abort_prepared(tx.transaction_id()).is_err());
    assert!(matches!(
        store
            .read_committed_root(KagemushaHistoryTreeV1::Replay)
            .unwrap(),
        KagemushaCommittedRootReadV1::Unavailable { .. }
    ));
    assert!(store.read_node(digest(b"x")).is_err());
    drop(store);
    fs::remove_file(&journal).unwrap();
    symlink(&held, &journal).unwrap();
    assert!(Store::open_existing(&path, binding(), credentials(), 1).is_err());
    fs::remove_file(&journal).unwrap();
    fs::hard_link(&held, &journal).unwrap();
    assert!(Store::open_existing(&path, binding(), credentials(), 1).is_err());
    fs::remove_file(&held).unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Store::open_existing(&path, binding(), credentials(), 1).is_err());
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&journal).unwrap();
    assert!(Store::open_existing(&path, binding(), credentials(), 1).is_err());
    assert!(Store::create_new(&path, binding(), credentials(), 1).is_err());
    let alias = path.with_file_name("history-link");
    symlink(&path, &alias).unwrap();
    assert!(Store::open_existing(&alias, binding(), credentials(), 1).is_err());
}

#[test]
fn disk_history_store_corrupt_and_partial_frames_never_select_empty_history() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"corrupt", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    store.commit_prepared(certificate(&tx, 1)).unwrap();
    drop(store);
    let file = path.join(JOURNAL_FILE);
    let valid = fs::read(&file).unwrap();
    for length in [0, 1, FRAME_HEADER_BYTES - 1, valid.len() - 1] {
        fs::write(&file, &valid[..length]).unwrap();
        assert!(
            Store::open_existing(&path, binding(), credentials(), 0).is_err(),
            "truncated at {length}"
        );
    }
    for offset in [0, 8, 16, 24, 56, FRAME_HEADER_BYTES, valid.len() - 1] {
        let mut corrupt = valid.clone();
        corrupt[offset] ^= 1;
        fs::write(&file, corrupt).unwrap();
        assert!(
            Store::open_existing(&path, binding(), credentials(), 0).is_err(),
            "changed byte {offset}"
        );
    }
    fs::write(&file, &valid).unwrap();
    assert!(Store::open_existing(&path, digest(b"other-lane"), credentials(), 0).is_err());
    let roots = reopen(&path).committed_roots();
    assert_ne!(roots, KagemushaHistoryRootsV1::empty());
}

#[test]
fn disk_history_store_failed_write_or_sync_poison_without_acknowledging_roots() {
    for failure in [
        TestPersistenceFailure::PartialWrite,
        TestPersistenceFailure::BeforeSync,
        TestPersistenceFailure::AfterSync,
    ] {
        let (_parent, path) = location();
        let mut store = create(&path);
        let roots = store.committed_roots();
        let tx = transaction(b"fault", roots);
        store.prepare_cas(tx.clone()).unwrap();
        store.wal.failure.set(Some(failure));
        assert_eq!(
            store.commit_prepared(certificate(&tx, 1)),
            Err(KagemushaHistoryStoreErrorV1::DurabilityUncertain)
        );
        assert_eq!(store.committed_roots(), roots);
        assert_eq!(
            store.abort_prepared(tx.transaction_id()),
            Err(KagemushaHistoryStoreErrorV1::DurabilityUncertain)
        );
        drop(store);
        if failure == TestPersistenceFailure::PartialWrite {
            assert!(Store::open_existing(&path, binding(), credentials(), 0).is_err());
        } else {
            let mut store = reopen(&path);
            let selected = tx.successor_roots_from(roots).unwrap();
            assert_eq!(
                store.recover_prepared(certificate(&tx, 1)).unwrap(),
                KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted {
                    committed_roots: selected
                }
            );
        }
    }
}

fn child(path: &Path, mode: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "zk::kagemusha_v1_state::sparse_merkle::authenticated_history::disk_history_store::tests::disk_history_store_process_probe", "--nocapture"])
        .env("IROHA_HISTORY_TEST_PATH", path)
        .env("IROHA_HISTORY_TEST_MODE", mode)
        .output().unwrap()
}

#[test]
fn disk_history_store_process_probe() {
    let Some(path) = std::env::var_os("IROHA_HISTORY_TEST_PATH") else {
        return;
    };
    let path = PathBuf::from(path);
    let mode = std::env::var("IROHA_HISTORY_TEST_MODE").unwrap();
    match mode.as_str() {
        "prepare-crash" => {
            let mut store = create(&path);
            let tx = transaction(b"process", store.committed_roots());
            store.prepare_cas(tx).unwrap();
            std::process::exit(73);
        }
        "commit-crash" => {
            let mut store = reopen(&path);
            let tx = transaction(b"process", store.committed_roots());
            store.commit_prepared(certificate(&tx, 1)).unwrap();
            std::process::exit(73);
        }
        "probe" => {
            let mut store = reopen(&path);
            let tx = transaction(b"process", KagemushaHistoryRootsV1::empty());
            let roots = tx
                .successor_roots_from(KagemushaHistoryRootsV1::empty())
                .unwrap();
            assert_eq!(store.committed_roots(), roots);
            assert_eq!(
                store.recover_prepared(certificate(&tx, 1)).unwrap(),
                KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted {
                    committed_roots: roots
                }
            );
        }
        "locked" => {
            assert!(matches!(
                Store::open_existing(&path, binding(), credentials(), 0),
                Err(KagemushaHistoryStoreErrorV1::StoreAlreadyOpen)
            ));
        }
        _ => panic!("unknown test subprocess mode"),
    }
}

#[test]
fn disk_history_store_process_restart_and_crash_preserve_both_roots() {
    let (_parent, path) = location();
    let prepare = child(&path, "prepare-crash");
    assert_eq!(
        prepare.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&prepare.stderr)
    );
    let store = reopen(&path);
    assert!(store.overlay_usage().live_bytes() > 0);
    let locked = child(&path, "locked");
    assert!(
        locked.status.success(),
        "{}",
        String::from_utf8_lossy(&locked.stderr)
    );
    drop(store);
    let commit = child(&path, "commit-crash");
    assert_eq!(
        commit.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let probe = child(&path, "probe");
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
}

#[test]
fn disk_history_store_recovery_commitment_matches_memory_and_rejects_same_root_rollback() {
    let (_parent, path) = location();
    let mut disk = create(&path);
    let mut memory = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
    let file = path.join(JOURNAL_FILE);
    let initial = fs::read(&file).unwrap();
    assert_eq!(
        disk.recovery_commitment().unwrap(),
        memory.recovery_commitment().unwrap()
    );
    let tx = transaction(b"retained-abort", disk.committed_roots());
    disk.prepare_cas(tx.clone()).unwrap();
    memory.prepare_cas(tx.clone()).unwrap();
    assert_eq!(
        disk.recovery_commitment().unwrap(),
        memory.recovery_commitment().unwrap()
    );
    disk.abort_prepared(tx.transaction_id()).unwrap();
    memory.abort_prepared(tx.transaction_id()).unwrap();
    let sealed = disk.recovery_commitment().unwrap();
    let roots = disk.committed_roots();
    assert_eq!(sealed, memory.recovery_commitment().unwrap());
    drop(disk);
    let disk = reopen(&path);
    assert_eq!(disk.recovery_commitment().unwrap(), sealed);
    drop(disk);
    fs::write(&file, initial).unwrap();
    let old = reopen(&path);
    assert_eq!(old.committed_roots(), roots);
    assert!(matches!(
        crate::zk::kagemusha_v1_state::KagemushaStateAuthenticatedHistoryV1::recover(
            old, roots, sealed,
        ),
        Err(KagemushaHistoryStoreErrorV1::RecoveryCommitmentMismatch)
    ));
}

#[test]
fn disk_history_store_replay_cannot_promote_a_forged_commit_frame() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"forged frame", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    let foreign = SigningKey::from_bytes((&[0x34; 32]).into()).unwrap();
    // Simulate host-written canonical framing/checksums. They never authenticate the signature.
    store
        .persist(&JournalRecordV1::Commit(
            signed_by(&tx, 1, &foreign).certificate,
        ))
        .unwrap();
    drop(store);
    assert!(matches!(
        Store::open_existing(&path, binding(), credentials(), 0),
        Err(KagemushaHistoryStoreErrorV1::InvalidCertificate)
    ));
}

#[test]
fn disk_history_store_same_length_tamper_poison_prevents_new_acknowledgment() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"tamper", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    let journal = path.join(JOURNAL_FILE);
    let original_len = fs::metadata(&journal).unwrap().len();
    let original_version = store.wal.observed_version();
    let mut data = fs::read(&journal).unwrap();
    data[FRAME_HEADER_BYTES] ^= 1;
    fs::write(&journal, &data).unwrap();
    assert_eq!(fs::metadata(&journal).unwrap().len(), original_len);
    assert_ne!(
        JournalFileVersion::from_metadata(&fs::metadata(&journal).unwrap()),
        original_version
    );
    assert_eq!(
        store.commit_prepared(certificate(&tx, 1)),
        Err(KagemushaHistoryStoreErrorV1::JournalCorrupt)
    );
    assert_eq!(store.committed_roots(), KagemushaHistoryRootsV1::empty());
    assert_eq!(fs::metadata(&journal).unwrap().len(), original_len);
}

#[test]
fn disk_history_store_rechecks_file_identity_and_length_after_sync() {
    for failure in [
        TestPersistenceFailure::ReplaceAfterSync,
        TestPersistenceFailure::TruncateAfterSync,
    ] {
        let (_parent, path) = location();
        let mut store = create(&path);
        let tx = transaction(b"publication race", store.committed_roots());
        store.prepare_cas(tx.clone()).unwrap();
        store.wal.failure.set(Some(failure));
        assert_eq!(
            store.commit_prepared(certificate(&tx, 1)),
            Err(KagemushaHistoryStoreErrorV1::DurabilityUncertain)
        );
        assert_eq!(store.committed_roots(), KagemushaHistoryRootsV1::empty());
        assert!(store.recovery_commitment().is_err());
    }
}

#[test]
fn disk_history_store_current_anchor_before_prepare_can_resume_speculative_suffix() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let anchored_roots = store.committed_roots();
    let anchored_head = store.recovery_commitment().unwrap();
    let tx = transaction(b"crash before next anchor", anchored_roots);
    store.prepare_cas(tx.clone()).unwrap();
    let pending_head = store.recovery_commitment().unwrap();
    drop(store);
    let store = reopen(&path);
    let facade = crate::zk::kagemusha_v1_state::KagemushaStateAuthenticatedHistoryV1::recover(
        store,
        anchored_roots,
        anchored_head,
    )
    .expect("current hardware checkpoint still matches; speculative work is retained");
    let mut store = facade.into_store();
    assert_eq!(store.recovery_commitment().unwrap(), pending_head);
    assert!(store.overlay_usage().live_bytes() > 0);
    assert_eq!(
        store.prepare_cas(tx.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
    );
    store.recover_prepared(certificate(&tx, 1)).unwrap();
    // A hardware-authorized commit cannot be treated as speculative under an old snapshot,
    // even if an attacker supplies its new roots beside that older commitment.
    assert_eq!(
        store.validate_recovery_checkpoint(anchored_head),
        Err(KagemushaHistoryStoreErrorV1::RecoveryCommitmentMismatch)
    );
}

#[test]
fn disk_history_store_aborted_attempt_can_retry_fresh_without_rewriting_tombstone() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let roots = store.committed_roots();
    let anchor = store.recovery_commitment().unwrap();
    let first = transaction(b"retry attempt", roots);
    store.prepare_cas(first.clone()).unwrap();
    store.abort_prepared(first.transaction_id()).unwrap();
    drop(store);
    let mut store = crate::zk::kagemusha_v1_state::KagemushaStateAuthenticatedHistoryV1::recover(
        reopen(&path),
        roots,
        anchor,
    )
    .unwrap()
    .into_store();
    let next = KagemushaPreparedHistoryCasV1::new(
        first.root_selection(),
        first
            .node_writes
            .iter()
            .map(|write| write.node.clone())
            .collect(),
        digest(b"fresh authenticated attempt"),
    )
    .unwrap();
    assert_ne!(first.transaction_id(), next.transaction_id());
    assert_eq!(
        store.prepare_cas(next.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::Prepared
    );
    let bytes = fs::metadata(path.join(JOURNAL_FILE)).unwrap().len();
    assert_eq!(
        store.prepare_cas(next.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
    );
    assert_eq!(
        store.prepare_cas(first.clone()).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyAborted
    );
    assert_eq!(fs::metadata(path.join(JOURNAL_FILE)).unwrap().len(), bytes);
    drop(store);
    let mut store = reopen(&path);
    assert_eq!(
        store.prepare_cas(first).unwrap(),
        KagemushaHistoryPrepareOutcomeV1::AlreadyAborted
    );
    store.commit_prepared(certificate(&next, 1)).unwrap();
    assert_eq!(
        store.commit_prepared(certificate(&next, 2)),
        Err(KagemushaHistoryStoreErrorV1::CertificateMismatch)
    );
    drop(store);
    let mut store = reopen(&path);
    assert!(matches!(
        store.commit_prepared(certificate(&next, 1)).unwrap(),
        KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { .. }
    ));
    assert_eq!(
        store.recover_prepared(certificate(&next, 2)),
        Err(KagemushaHistoryStoreErrorV1::CertificateMismatch)
    );
}

#[test]
fn disk_history_store_hardware_preflight_preserves_integrity_failure() {
    let (_parent, path) = location();
    let mut store = create(&path);
    let tx = transaction(b"hardware preflight", store.committed_roots());
    store.prepare_cas(tx.clone()).unwrap();
    store.require_prepared(&tx).unwrap();
    let journal_path = path.join(JOURNAL_FILE);
    fs::set_permissions(&journal_path, fs::Permissions::from_mode(0o400)).unwrap();
    assert_eq!(
        store.require_prepared(&tx),
        Err(KagemushaHistoryStoreErrorV1::JournalCorrupt)
    );
    assert_eq!(
        store.require_prepared(&tx),
        Err(KagemushaHistoryStoreErrorV1::DurabilityUncertain)
    );
}
