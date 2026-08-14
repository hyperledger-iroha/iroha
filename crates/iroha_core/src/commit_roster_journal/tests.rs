use std::{num::NonZeroU64, path::Path};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::{block::BlockHeader, consensus::VALIDATOR_SET_HASH_VERSION_V1, peer::PeerId};
use iroha_primitives::numeric::Quantity;
use tempfile::tempdir;
use super::*;
use crate::sumeragi::{
    consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, QcAggregate},
    stake_snapshot::CommitStakeSnapshotEntry,
};
fn sample_cert(view: u64) -> (Qc, ValidatorSetCheckpoint) {
    cert_with_height(2, view)
}
fn checked_random_bls_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("generate checked commit roster journal BLS fixture keypair")
}
fn cert_with_height(height: u64, view: u64) -> (Qc, ValidatorSetCheckpoint) {
    let kp = checked_random_bls_keypair();
    let peer = PeerId::new(kp.public_key().clone());
    cert_with_height_and_roster(height, view, vec![peer])
}
fn cert_with_height_and_roster(
    height: u64,
    view: u64,
    roster: Vec<PeerId>,
) -> (Qc, ValidatorSetCheckpoint) {
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block_hash = header.hash();
    let signers_bitmap = vec![0b0000_0001];
    let bls_aggregate_signature = vec![0xAB; 96];
    let cert = Qc {
        phase: Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        height,
        view,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster.clone(),
        aggregate: QcAggregate {
            signers_bitmap: signers_bitmap.clone(),
            bls_aggregate_signature: bls_aggregate_signature.clone(),
        },
    };
    let checkpoint = ValidatorSetCheckpoint::new(
        height,
        view,
        block_hash,
        cert.parent_state_root,
        cert.post_state_root,
        roster,
        signers_bitmap,
        bls_aggregate_signature,
        VALIDATOR_SET_HASH_VERSION_V1,
        None,
    );
    (cert, checkpoint)
}
fn sample_stake_snapshot(roster: &[PeerId]) -> CommitStakeSnapshot {
    CommitStakeSnapshot {
        validator_set_hash: HashOf::new(&roster.to_vec()),
        entries: roster
            .iter()
            .map(|peer| CommitStakeSnapshotEntry {
                peer_id: peer.clone(),
                stake: Quantity::from(10_u32),
            })
            .collect(),
    }
}
fn retention(limit: usize) -> NonZeroUsize {
    NonZeroUsize::new(limit).expect("non-zero retention")
}
fn write_test_generation(path: &Path, bytes: &[u8]) -> PathBuf {
    let generations = path.join(CommitRosterJournal::GENERATIONS_DIR);
    std::fs::create_dir_all(&generations).expect("create generation directory");
    let digest = hex::encode(Sha256::digest(bytes));
    let generation = generations.join(format!("{digest}.norito"));
    std::fs::write(&generation, bytes).expect("write generation payload");
    std::fs::write(
        path.join(CommitRosterJournal::CURRENT_FILE),
        format!("{digest}\n"),
    )
    .expect("write current pointer");
    generation
}
fn read_test_generation(path: &Path) -> Vec<u8> {
    let digest =
        CommitRosterJournal::read_current_digest(&path.join(CommitRosterJournal::CURRENT_FILE))
            .expect("read current digest");
    std::fs::read(
        path.join(CommitRosterJournal::GENERATIONS_DIR)
            .join(format!("{digest}.norito")),
    )
    .expect("read generation payload")
}
#[test]
fn canonical_snapshot_validation_rejects_signed_subject_mismatch() {
    let (commit_qc, validator_checkpoint) = sample_cert(1);
    let snapshot = CommitRosterSnapshot {
        commit_qc,
        validator_checkpoint,
        stake_snapshot: None,
    };
    assert!(CommitRosterJournal::snapshot_is_canonical(&snapshot));
    let mut mismatched = snapshot;
    mismatched.validator_checkpoint.post_state_root =
        iroha_crypto::Hash::new(b"mismatched post-state root");
    assert!(!CommitRosterJournal::snapshot_is_canonical(&mismatched));
}
#[test]
fn canonical_snapshot_validation_accepts_indexed_npos_and_rejects_roster_mismatch() {
    let kp = checked_random_bls_keypair();
    let roster = vec![PeerId::new(kp.public_key().clone())];
    let (mut commit_qc, validator_checkpoint) = cert_with_height_and_roster(2, 0, roster.clone());
    commit_qc.mode_tag = NPOS_TAG.to_owned();
    let snapshot = CommitRosterSnapshot {
        commit_qc,
        validator_checkpoint,
        stake_snapshot: Some(sample_stake_snapshot(&roster)),
    };
    assert!(CommitRosterJournal::snapshot_is_canonical(&snapshot));
    let other = PeerId::new(checked_random_bls_keypair().public_key().clone());
    let mut mismatched = snapshot;
    mismatched.stake_snapshot = Some(sample_stake_snapshot(&[other]));
    assert!(!CommitRosterJournal::snapshot_is_canonical(&mismatched));
}
#[test]
fn storage_unknown_fence_rejects_use_until_reload() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.mark_storage_unknown();
    assert!(journal.storage_is_unknown());
    assert!(!journal.upsert(cert.clone(), checkpoint.clone(), None));
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::StorageUnknown { .. })
    ));
    assert!(!journal.durable_entry_matches_exact(&cert, &checkpoint, None));
    let reloaded = CommitRosterJournal::load(path, retention(4)).expect("restart reload");
    assert!(
        !reloaded.storage_is_unknown(),
        "only reconstruction from the resolved post-crash namespace clears the process fence"
    );
}
#[test]
fn journal_roundtrips_entries() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert.clone(), checkpoint.clone(), None);
    journal.persist().expect("persist");
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
    let snapshots = loaded.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots[0],
        CommitRosterSnapshot {
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot: None,
        }
    );
}
#[test]
fn post_publication_namespace_failure_fences_process_until_reload() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
    journal.fail_after_rename_once_for_tests();
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::NamespaceSync { .. })
    ));
    assert!(journal.storage_is_unknown());
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::StorageUnknown { .. })
    ));
    let reloaded = CommitRosterJournal::load(path, retention(4)).expect("restart reload");
    assert_eq!(
        reloaded.get(cert.height, cert.subject_block_hash),
        Some(CommitRosterSnapshot {
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot: None,
        })
    );
}
#[test]
fn durable_pointer_temp_recovers_forward_after_restart() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.fail_pointer_persist_once_for_tests();
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::Write { .. })
    ));
    assert!(journal.storage_is_unknown());
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::StorageUnknown { .. })
    ));
    let reloaded = CommitRosterJournal::load(path, retention(4))
        .expect("durable pointer temp recovers forward on restart");
    assert_eq!(reloaded.snapshots().len(), 1);
    assert!(!reloaded.storage_is_unknown());
}
#[test]
fn durable_generation_temp_without_pointer_rolls_back_after_restart() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.fail_generation_persist_once_for_tests();
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::Write { .. })
    ));
    assert!(journal.storage_is_unknown());
    let reloaded = CommitRosterJournal::load(path.clone(), retention(4))
        .expect("generation-only crash residue is not publication authority");
    assert!(reloaded.snapshots().is_empty());
    assert!(
        std::fs::read_dir(path.join(CommitRosterJournal::GENERATIONS_DIR))
            .expect("read reconciled generation directory")
            .all(|entry| !entry
                .expect("generation entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")),
    );
}
#[test]
fn partial_deterministic_temps_roll_back_to_stable_pointer() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert1, checkpoint1) = cert_with_height(1, 0);
    let (cert2, checkpoint2) = cert_with_height(2, 0);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert1.clone(), checkpoint1, None));
    journal.persist().expect("persist stable journal");
    assert!(journal.upsert(cert2, checkpoint2, None));
    let intended = journal
        .canonical_payload_bytes()
        .expect("encode intended successor generation");
    let digest: [u8; 32] = Sha256::digest(&intended).into();
    let generation_temp = journal.generation_temp_path_for_digest(digest);
    std::fs::write(&generation_temp, &intended[..intended.len() / 2])
        .expect("write partial deterministic generation temp");
    std::fs::write(
        path.join(CommitRosterJournal::CURRENT_TEMP_FILE),
        b"partial",
    )
    .expect("write partial deterministic pointer temp");
    drop(journal);
    let reloaded = CommitRosterJournal::load(path.clone(), retention(4))
        .expect("partial temps roll back without changing stable authority");
    assert_eq!(reloaded.snapshots().len(), 1);
    assert!(
        reloaded
            .get(cert1.height, cert1.subject_block_hash)
            .is_some()
    );
    assert!(!generation_temp.exists());
    assert!(!path.join(CommitRosterJournal::CURRENT_TEMP_FILE).exists());
}
#[test]
fn load_rejects_oversized_and_unexpected_publication_artifacts() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.persist().expect("persist stable journal");
    let current_temp = path.join(CommitRosterJournal::CURRENT_TEMP_FILE);
    std::fs::write(
        &current_temp,
        vec![0_u8; usize::try_from(CommitRosterJournal::POINTER_BYTES + 1).unwrap()],
    )
    .expect("write oversized deterministic pointer temp");
    assert!(CommitRosterJournal::load(path.clone(), retention(4)).is_err());
    std::fs::remove_file(&current_temp).expect("remove oversized pointer temp");
    let unexpected = path.join("current.backup");
    std::fs::write(&unexpected, b"unexpected").expect("write unexpected root artifact");
    assert!(matches!(
        CommitRosterJournal::load(path, retention(4)),
        Err(CommitRosterJournalError::InvalidStorage {
            reason: "unexpected commit-roster publication artifact",
            ..
        })
    ));
}
#[cfg(unix)]
#[test]
fn load_rejects_symlinked_deterministic_publication_temp() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.persist().expect("persist stable journal");
    let external = dir.path().join("external-pointer-temp");
    std::fs::write(&external, b"partial").expect("write external temp target");
    symlink(&external, path.join(CommitRosterJournal::CURRENT_TEMP_FILE))
        .expect("install pointer-temp symlink");
    assert!(CommitRosterJournal::load(path, retention(4)).is_err());
}
#[test]
fn current_pointer_substitution_before_gc_fails_and_fences_process() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.replace_current_before_gc_once_for_tests();
    let error = journal
        .persist()
        .expect_err("post-publication pointer substitution must fail");
    assert!(matches!(
        error,
        CommitRosterJournalError::InvalidStorage {
            reason: "current pointer changed before generation GC",
            ..
        }
    ));
    assert!(journal.storage_is_unknown());
    assert!(matches!(
        journal.persist(),
        Err(CommitRosterJournalError::StorageUnknown { .. })
    ));
    assert!(
        CommitRosterJournal::load(path, retention(4)).is_err(),
        "restart must reject the substituted pointer without its exact generation"
    );
}
#[test]
fn load_rejects_digest_mismatch_and_malformed_pointer() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.persist().expect("persist");
    let digest =
        CommitRosterJournal::read_current_digest(&path.join(CommitRosterJournal::CURRENT_FILE))
            .expect("current digest");
    let generation = path
        .join(CommitRosterJournal::GENERATIONS_DIR)
        .join(format!("{digest}.norito"));
    std::fs::write(&generation, b"same name, different bytes").expect("corrupt generation");
    let error = CommitRosterJournal::load(path.clone(), retention(4))
        .expect_err("digest mismatch must fail closed");
    assert!(matches!(
        error,
        CommitRosterJournalError::InvalidStorage { .. }
    ));
    std::fs::write(
        path.join(CommitRosterJournal::CURRENT_FILE),
        digest.to_uppercase(),
    )
    .expect("write malformed pointer");
    let error = CommitRosterJournal::load(path, retention(4))
        .expect_err("noncanonical pointer must fail closed");
    assert!(matches!(
        error,
        CommitRosterJournalError::InvalidStorage { .. }
    ));
}
#[test]
fn load_ignores_unpublished_generation_after_pointer_loss_without_mutating_disk() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.persist().expect("persist");
    let generations_before = std::fs::read_dir(path.join(CommitRosterJournal::GENERATIONS_DIR))
        .expect("read generations")
        .map(|entry| entry.expect("generation entry").file_name())
        .collect::<Vec<_>>();
    std::fs::remove_file(path.join(CommitRosterJournal::CURRENT_FILE))
        .expect("remove current pointer");
    let loaded = CommitRosterJournal::load(path.clone(), retention(4))
        .expect("unpublished generation is not durable authority");
    assert!(loaded.snapshots().is_empty());
    let generations_after = std::fs::read_dir(path.join(CommitRosterJournal::GENERATIONS_DIR))
        .expect("reread generations")
        .map(|entry| entry.expect("generation entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(generations_after, generations_before);
    assert!(!path.join(CommitRosterJournal::CURRENT_FILE).exists());
}
#[cfg(unix)]
#[test]
fn load_rejects_symlink_and_hardlink_artifacts() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert, checkpoint, None));
    journal.persist().expect("persist");
    let current = path.join(CommitRosterJournal::CURRENT_FILE);
    let direct_pointer = dir.path().join("direct-current");
    std::fs::rename(&current, &direct_pointer).expect("move direct pointer");
    symlink(&direct_pointer, &current).expect("install pointer symlink");
    assert!(CommitRosterJournal::load(path.clone(), retention(4)).is_err());
    std::fs::remove_file(&current).expect("remove pointer symlink");
    std::fs::rename(&direct_pointer, &current).expect("restore direct pointer");
    let digest = CommitRosterJournal::read_current_digest(&current).expect("current digest");
    let generation = path
        .join(CommitRosterJournal::GENERATIONS_DIR)
        .join(format!("{digest}.norito"));
    std::fs::hard_link(&generation, dir.path().join("generation-hardlink"))
        .expect("create generation hardlink");
    assert!(CommitRosterJournal::load(path, retention(4)).is_err());
}
#[test]
fn journal_durable_persist_clears_dirty_state() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert.clone(), checkpoint.clone(), None);
    assert!(journal.needs_persistence());
    journal.persist().expect("persist durable");
    assert!(!journal.needs_persistence());
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
    let snapshot = loaded
        .get(cert.height, cert.subject_block_hash)
        .expect("snapshot must be present");
    assert_eq!(snapshot.commit_qc, cert);
    assert_eq!(snapshot.validator_checkpoint, checkpoint);
}
#[test]
fn journal_truncate_to_height_drops_future_entries() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert1, checkpoint1) = cert_with_height(1, 0);
    let (cert2, checkpoint2) = cert_with_height(2, 0);
    let mut journal = CommitRosterJournal::new(path, retention(4));
    journal.upsert(cert1.clone(), checkpoint1, None);
    journal.upsert(cert2.clone(), checkpoint2, None);
    assert!(journal.has_entries_above(1));
    journal.truncate_to_height(1).expect("truncate to height");
    assert!(!journal.has_entries_above(1));
    assert!(
        journal
            .get(cert1.height, cert1.subject_block_hash)
            .is_some()
    );
    assert!(
        journal
            .get(cert2.height, cert2.subject_block_hash)
            .is_none()
    );
}
#[test]
fn prune_projection_accounts_large_generation_and_pointer_peak_exactly() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let peer = PeerId::new(checked_random_bls_keypair().public_key().clone());
    let mut journal = CommitRosterJournal::new(path, retention(64));
    for height in 1..=48 {
        let (cert, checkpoint) = cert_with_height_and_roster(height, 0, vec![peer.clone()]);
        assert!(journal.upsert(cert, checkpoint, None));
    }
    journal.persist().expect("persist large source journal");
    let projection = journal
        .project_truncate_to_height(24)
        .expect("project large retained journal");
    assert!(projection.required);
    assert!(projection.retained_payload_bytes > 4 * 1024);
    assert_eq!(
        projection.generation_allocation_bytes,
        projection.retained_payload_bytes,
    );
    assert_eq!(
        projection.pointer_temporary_bytes,
        CommitRosterJournal::POINTER_BYTES,
    );
    assert_eq!(projection.current_pointer_growth_bytes, 0);
    assert_eq!(
        projection
            .allocation_peak_with_sidecar(8192)
            .expect("large prune peak fits u64"),
        projection.retained_payload_bytes + 8192,
    );
    journal
        .truncate_to_height_with_projection(24, projection)
        .expect("publish authorized large retained generation");
    let post = journal
        .project_truncate_to_height(24)
        .expect("reproject published large retained generation");
    assert!(!post.required);
    assert!(projection.authorizes(post));
}
#[test]
fn journal_persist_overwrites_existing_file() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert1, checkpoint1) = cert_with_height(2, 1);
    let (cert2, checkpoint2) = cert_with_height(3, 1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert1.clone(), checkpoint1, None);
    journal.persist().expect("persist first journal");
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert2.clone(), checkpoint2.clone(), None);
    journal.persist().expect("persist second journal");
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load journal");
    assert!(
        loaded.get(cert1.height, cert1.subject_block_hash).is_none(),
        "old entry should be overwritten"
    );
    let snapshot = loaded
        .get(cert2.height, cert2.subject_block_hash)
        .expect("new entry should exist");
    assert_eq!(snapshot.commit_qc, cert2);
    assert_eq!(snapshot.validator_checkpoint, checkpoint2);
}
#[test]
fn journal_ignores_unpublished_legacy_temp_next_to_committed_main() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let tmp_path = path.with_extension("norito.tmp");
    let (cert1, checkpoint1) = cert_with_height(2, 1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert1.clone(), checkpoint1.clone(), None);
    journal.persist().expect("persist main journal");
    let (cert2, checkpoint2) = cert_with_height(3, 1);
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![
            CommitRosterRecord {
                height: cert1.height,
                block_hash: cert1.subject_block_hash,
                commit_qc: cert1.clone(),
                validator_checkpoint: checkpoint1.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            },
            CommitRosterRecord {
                height: cert2.height,
                block_hash: cert2.subject_block_hash,
                commit_qc: cert2.clone(),
                validator_checkpoint: checkpoint2.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            },
        ],
    };
    let bytes = to_bytes(&payload).expect("encode temp journal");
    std::fs::write(&tmp_path, bytes).expect("write unpublished temp journal");
    let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load journal");
    assert!(loaded.get(cert2.height, cert2.subject_block_hash).is_none());
    assert!(loaded.get(cert1.height, cert1.subject_block_hash).is_some());
    assert!(
        tmp_path.exists(),
        "read-only load must not promote temp state"
    );
}
#[test]
fn journal_preserves_prepared_tuple_against_higher_view_replacement() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (low_view_cert, low_view_checkpoint) = sample_cert(1);
    let (high_view_cert, high_view_checkpoint) = sample_cert(3);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(low_view_cert.clone(), low_view_checkpoint.clone(), None));
    assert!(
        !journal.upsert(high_view_cert, high_view_checkpoint, None),
        "a divergent higher-view tuple must not replace prepared authority"
    );
    journal.persist().expect("persist");
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
    let snapshots = loaded.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].commit_qc, low_view_cert);
    assert_eq!(snapshots[0].validator_checkpoint, low_view_checkpoint);
}
#[test]
fn journal_exact_retry_is_idempotent() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path, retention(4));
    assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
    journal.persist().expect("persist prepared tuple");
    assert!(!journal.needs_persistence());
    assert!(
        journal.upsert(cert.clone(), checkpoint.clone(), None),
        "an exact retry must be accepted"
    );
    assert!(
        !journal.needs_persistence(),
        "upsert must not manufacture a logical change; the durability boundary still rewrites"
    );
    assert_eq!(
        journal.get(cert.height, cert.subject_block_hash),
        Some(CommitRosterSnapshot {
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot: None,
        })
    );
}
#[test]
fn journal_exact_retry_repersists_deleted_durable_file() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
    journal.persist().expect("persist prepared tuple");
    assert!(!journal.needs_persistence());
    std::fs::remove_dir_all(&path).expect("delete durable journal");
    assert!(
        journal.upsert(cert.clone(), checkpoint.clone(), None),
        "the exact in-memory retry remains admissible"
    );
    assert!(
        !journal.needs_persistence(),
        "the adversary deletes disk state without changing the in-memory tuple"
    );
    journal
        .persist()
        .expect("an exact retry must rewrite and fsync the durable journal");
    assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));
    assert!(
        path.exists(),
        "the exact retry must restore the deleted file"
    );
}
#[test]
fn journal_durable_exact_readback_fails_closed_after_deletion_or_corruption() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
    journal.persist().expect("persist prepared tuple");
    assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));
    std::fs::remove_dir_all(&path).expect("delete durable journal");
    assert!(
        !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
        "stale memory must not hide deletion of the recovery fence"
    );
    std::fs::write(&path, b"corrupted commit roster journal")
        .expect("write corrupt durable journal");
    assert!(
        !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
        "stale memory must not hide corruption of the recovery fence"
    );
}
#[test]
fn journal_rejects_legacy_v1_payload() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let payload = PersistedCommitRosters {
        version: 1,
        stake_snapshots: Vec::new(),
        entries: vec![CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert.clone(),
            validator_checkpoint: checkpoint.clone(),
            stake_snapshot_index: None,
            stake_snapshot: None,
        }],
    };
    let bytes = norito::to_bytes(&payload).expect("encode payload");
    write_test_generation(&path, &bytes);
    let err = CommitRosterJournal::load(path, retention(4)).expect_err("reject v1 journal");
    assert!(
        matches!(
            err,
            CommitRosterJournalError::UnsupportedVersion { version: 1, .. }
        ),
        "unexpected error: {err}"
    );
}
#[test]
fn journal_load_rejects_divergent_duplicate_block_subject_rows() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (first_cert, first_checkpoint) = sample_cert(1);
    let (replacement_cert, replacement_checkpoint) = sample_cert(3);
    assert_eq!(
        first_cert.subject_block_hash, replacement_cert.subject_block_hash,
        "fixture must target one block subject"
    );
    let record = |commit_qc: Qc, validator_checkpoint: ValidatorSetCheckpoint| CommitRosterRecord {
        height: commit_qc.height,
        block_hash: commit_qc.subject_block_hash,
        commit_qc,
        validator_checkpoint,
        stake_snapshot_index: None,
        stake_snapshot: None,
    };
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![
            record(first_cert, first_checkpoint),
            record(replacement_cert, replacement_checkpoint),
        ],
    };
    write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
    let err = CommitRosterJournal::load(path, retention(4))
        .expect_err("divergent duplicate rows must fail closed");
    assert!(
        matches!(
            err,
            CommitRosterJournalError::InvalidEntry {
                reason: "divergent duplicate rows for the same block subject",
                ..
            }
        ),
        "unexpected error: {err}"
    );
}
#[test]
fn journal_load_accepts_exact_duplicate_rows_as_idempotent() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let record = CommitRosterRecord {
        height: cert.height,
        block_hash: cert.subject_block_hash,
        commit_qc: cert.clone(),
        validator_checkpoint: checkpoint.clone(),
        stake_snapshot_index: None,
        stake_snapshot: None,
    };
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![record.clone(), record],
    };
    write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
    let loaded =
        CommitRosterJournal::load(path, retention(4)).expect("exact duplicate rows are idempotent");
    assert_eq!(
        loaded.get(cert.height, cert.subject_block_hash),
        Some(CommitRosterSnapshot {
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot: None,
        })
    );
    assert!(
        loaded.needs_persistence(),
        "the next durability boundary should canonicalize duplicate rows"
    );
}
#[test]
fn journal_rejects_checkpoint_that_differs_from_qc_subject() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, mut checkpoint) = sample_cert(1);
    checkpoint.post_state_root = iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]);
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot_index: None,
            stake_snapshot: None,
        }],
    };
    write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
    let err = CommitRosterJournal::load(path, retention(4))
        .expect_err("reject mismatched signed subject");
    assert!(
        matches!(
            err,
            CommitRosterJournalError::InvalidEntry {
                reason: "checkpoint does not exactly match the signed certificate subject",
                ..
            }
        ),
        "unexpected error: {err}"
    );
}
#[test]
fn journal_rejects_row_key_that_differs_from_qc_subject() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![CommitRosterRecord {
            height: cert.height.saturating_add(1),
            block_hash: cert.subject_block_hash,
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot_index: None,
            stake_snapshot: None,
        }],
    };
    write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
    let err = CommitRosterJournal::load(path, retention(4)).expect_err("reject mismatched row key");
    assert!(
        matches!(
            err,
            CommitRosterJournalError::InvalidEntry {
                reason: "certificate subject does not match row key",
                ..
            }
        ),
        "unexpected error: {err}"
    );
}
#[test]
fn journal_rejects_noncanonical_height_one_finality_metadata() {
    for case in ["nonzero_root", "nonzero_epoch", "nonzero_rechain", "signed"] {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (mut cert, mut checkpoint) = cert_with_height(1, 0);
        cert.aggregate.signers_bitmap.fill(0);
        cert.aggregate.bls_aggregate_signature.clear();
        checkpoint.signers_bitmap.fill(0);
        checkpoint.bls_aggregate_signature.clear();
        match case {
            "nonzero_root" => {
                let root = Hash::prehashed([0xA5; Hash::LENGTH]);
                cert.parent_state_root = root;
                checkpoint.parent_state_root = root;
            }
            "nonzero_epoch" => cert.epoch = 1,
            "nonzero_rechain" => {
                cert.rechain_seq = 1;
                checkpoint.rechain_seq = 1;
            }
            "signed" => {
                cert.aggregate.bls_aggregate_signature = vec![0xA5; 96];
                checkpoint.bls_aggregate_signature = vec![0xA5; 96];
            }
            _ => unreachable!(),
        }
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: 1,
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
        let err = CommitRosterJournal::load(path, retention(4))
            .expect_err("noncanonical height-one metadata must fail closed");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "height-one certificate is not the canonical unsigned genesis stub",
                    ..
                }
            ),
            "unexpected {case} error: {err}"
        );
    }
}
#[test]
fn journal_roundtrips_stake_snapshot() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (mut cert, checkpoint) = sample_cert(1);
    cert.mode_tag = NPOS_TAG.to_string();
    let stake_snapshot = sample_stake_snapshot(&cert.validator_set);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(
        cert.clone(),
        checkpoint.clone(),
        Some(stake_snapshot.clone()),
    );
    journal.persist().expect("persist");
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
    let snapshots = loaded.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].stake_snapshot, Some(stake_snapshot));
}
#[test]
fn journal_rejects_inline_stake_snapshot_representation() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let kp = checked_random_bls_keypair();
    let roster = vec![PeerId::new(kp.public_key().clone())];
    let (mut cert, checkpoint) = cert_with_height_and_roster(2, 0, roster.clone());
    cert.mode_tag = NPOS_TAG.to_owned();
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert,
            validator_checkpoint: checkpoint,
            stake_snapshot_index: None,
            stake_snapshot: Some(sample_stake_snapshot(&roster)),
        }],
    };
    write_test_generation(&path, &to_bytes(&payload).expect("encode inline fixture"));
    let error = CommitRosterJournal::load(path, retention(4))
        .expect_err("inline stake snapshots must fail closed");
    assert!(matches!(
        error,
        CommitRosterJournalError::InvalidEntry {
            reason: "inline stake snapshots are unsupported; use the indexed table",
            ..
        }
    ));
}
#[test]
fn journal_rejects_non_exact_indexed_stake_snapshots() {
    let roster = (0..3)
        .map(|_| PeerId::new(checked_random_bls_keypair().public_key().clone()))
        .collect::<Vec<_>>();
    let (mut cert, checkpoint) = cert_with_height_and_roster(2, 0, roster.clone());
    cert.mode_tag = NPOS_TAG.to_owned();
    let base = sample_stake_snapshot(&roster);
    let mut reordered = base.clone();
    reordered.entries.swap(0, 1);
    let mut duplicate_inflated = base.clone();
    duplicate_inflated.entries[1] = CommitStakeSnapshotEntry {
        peer_id: roster[0].clone(),
        stake: Quantity::from(1_000_000_u64),
    };
    let mut missing = base.clone();
    missing.entries.pop();
    let mut extra = base.clone();
    extra.entries.push(CommitStakeSnapshotEntry {
        peer_id: PeerId::new(checked_random_bls_keypair().public_key().clone()),
        stake: Quantity::from(1_u64),
    });
    let mut zero = base;
    zero.entries[0].stake = Quantity::zero();
    for malformed in [reordered, duplicate_inflated, missing, extra, zero] {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: vec![malformed],
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot_index: Some(0),
                stake_snapshot: None,
            }],
        };
        write_test_generation(
            &path,
            &to_bytes(&payload).expect("encode malformed fixture"),
        );
        let error = CommitRosterJournal::load(path, retention(4))
            .expect_err("non-exact indexed stake snapshot must fail closed");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidEntry {
                reason: "stake snapshot does not match validator set",
                ..
            }
        ));
    }
}
#[test]
fn journal_deduplicates_persisted_stake_snapshots() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let kp = checked_random_bls_keypair();
    let roster = vec![PeerId::new(kp.public_key().clone())];
    let stake_snapshot = sample_stake_snapshot(&roster);
    let (mut cert1, checkpoint1) = cert_with_height_and_roster(2, 0, roster.clone());
    let (mut cert2, checkpoint2) = cert_with_height_and_roster(3, 0, roster.clone());
    cert1.mode_tag = NPOS_TAG.to_string();
    cert2.mode_tag = NPOS_TAG.to_string();
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert1.clone(), checkpoint1, Some(stake_snapshot.clone()));
    journal.upsert(cert2.clone(), checkpoint2, Some(stake_snapshot.clone()));
    journal.persist().expect("persist");
    let bytes = read_test_generation(&path);
    let payload: PersistedCommitRosters =
        decode_from_bytes(&bytes).expect("decode persisted journal");
    assert_eq!(payload.version, CommitRosterJournal::JOURNAL_VERSION);
    assert_eq!(payload.stake_snapshots, vec![stake_snapshot.clone()]);
    assert!(
        payload.entries.iter().all(|entry| {
            entry.stake_snapshot.is_none() && entry.stake_snapshot_index == Some(0)
        })
    );
    let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
    let snapshots = loaded.snapshots();
    assert_eq!(snapshots.len(), 2);
    assert!(
        snapshots
            .iter()
            .all(|snapshot| snapshot.stake_snapshot == Some(stake_snapshot.clone()))
    );
}
#[test]
fn journal_ignores_unpublished_legacy_temp_when_current_is_missing() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let tmp_path = path.with_extension("norito.tmp");
    let (cert, checkpoint) = sample_cert(1);
    let payload = PersistedCommitRosters {
        version: CommitRosterJournal::JOURNAL_VERSION,
        stake_snapshots: Vec::new(),
        entries: vec![CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert.clone(),
            validator_checkpoint: checkpoint.clone(),
            stake_snapshot_index: None,
            stake_snapshot: None,
        }],
    };
    let bytes = norito::to_bytes(&payload).expect("encode payload");
    std::fs::write(&tmp_path, bytes).expect("write temp payload");
    let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
    assert!(loaded.snapshots().is_empty());
    assert!(!path.exists(), "read-only load must not create storage");
    assert!(
        tmp_path.exists(),
        "read-only load must leave unpublished temp state untouched"
    );
}
#[test]
fn journal_rejects_corrupt_root_instead_of_promoting_legacy_temp() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let tmp_path = path.with_extension("norito.tmp");
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert.clone(), checkpoint.clone(), None);
    journal.persist().expect("persist");
    std::fs::rename(&path, &tmp_path).expect("move journal to temp");
    std::fs::write(&path, b"corrupted").expect("write corrupted journal");
    let error = CommitRosterJournal::load(path.clone(), retention(4))
        .expect_err("corrupt storage root must fail closed");
    assert!(matches!(
        error,
        CommitRosterJournalError::InvalidStorage { .. }
    ));
    assert!(
        tmp_path.exists(),
        "read-only load must not promote the legacy temp"
    );
}
#[test]
fn journal_rejects_unsupported_version() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let payload = PersistedCommitRosters {
        version: 3,
        stake_snapshots: Vec::new(),
        entries: Vec::new(),
    };
    let bytes = norito::to_bytes(&payload).expect("encode payload");
    write_test_generation(&path, &bytes);
    let err = CommitRosterJournal::load(path, retention(4)).expect_err("unsupported version");
    assert!(matches!(
        err,
        CommitRosterJournalError::UnsupportedVersion { .. }
    ));
}
#[test]
fn get_returns_matching_snapshot() {
    let (cert, checkpoint) = sample_cert(2);
    let mut journal = CommitRosterJournal::new(PathBuf::from("unused"), retention(4));
    journal.upsert(cert.clone(), checkpoint.clone(), None);
    let found = journal
        .get(cert.height, cert.subject_block_hash)
        .expect("snapshot must be present");
    assert_eq!(found.commit_qc, cert);
    assert_eq!(found.validator_checkpoint, checkpoint);
    assert!(
        journal
            .get(cert.height + 1, cert.subject_block_hash)
            .is_none(),
        "mismatched height should not return a snapshot"
    );
}
#[test]
fn retention_drops_oldest_entries() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let mut journal = CommitRosterJournal::new(path.clone(), retention(2));
    for height in 1..=6 {
        let (mut cert, mut checkpoint) = cert_with_height(height, 0);
        if height == 1 {
            cert.aggregate.signers_bitmap.fill(0);
            cert.aggregate.bls_aggregate_signature.clear();
            checkpoint.signers_bitmap.fill(0);
            checkpoint.bls_aggregate_signature.clear();
        }
        journal.upsert(cert, checkpoint, None);
    }
    let snapshots = journal.snapshots();
    let heights: Vec<_> = snapshots
        .iter()
        .map(|snapshot| snapshot.commit_qc.height)
        .collect();
    assert_eq!(heights, vec![1, 4, 5, 6]);
    journal.persist().expect("persist");
    let reloaded = CommitRosterJournal::load(path, retention(2)).expect("load");
    let reloaded_heights: Vec<_> = reloaded
        .snapshots()
        .into_iter()
        .map(|snapshot| snapshot.commit_qc.height)
        .collect();
    assert_eq!(reloaded_heights, vec![1, 4, 5, 6]);
}
#[test]
fn retention_one_keeps_all_tip_conflicts_and_prepared_successor_durable() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let mut journal = CommitRosterJournal::new(path.clone(), retention(1));
    let (mut genesis_qc, mut genesis_checkpoint) = cert_with_height(1, 0);
    genesis_qc.aggregate.signers_bitmap.fill(0);
    genesis_qc.aggregate.bls_aggregate_signature.clear();
    genesis_checkpoint.signers_bitmap.fill(0);
    genesis_checkpoint.bls_aggregate_signature.clear();
    let (tip_qc, tip_checkpoint) = cert_with_height(2, 0);
    let mut conflicting_tip_qc = tip_qc.clone();
    let conflicting_tip_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]));
    conflicting_tip_qc.subject_block_hash = conflicting_tip_hash;
    conflicting_tip_qc.parent_state_root = Hash::prehashed([0xB5; Hash::LENGTH]);
    let mut conflicting_tip_checkpoint = tip_checkpoint.clone();
    conflicting_tip_checkpoint.block_hash = conflicting_tip_hash;
    conflicting_tip_checkpoint.parent_state_root = conflicting_tip_qc.parent_state_root;
    let (prepared_successor_qc, prepared_successor_checkpoint) = cert_with_height(3, 0);
    assert!(journal.upsert(genesis_qc.clone(), genesis_checkpoint.clone(), None));
    assert!(journal.upsert(tip_qc.clone(), tip_checkpoint.clone(), None));
    assert!(journal.upsert(
        conflicting_tip_qc.clone(),
        conflicting_tip_checkpoint.clone(),
        None,
    ));
    assert!(journal.upsert(
        prepared_successor_qc.clone(),
        prepared_successor_checkpoint.clone(),
        None,
    ));
    journal
        .persist()
        .expect("persist genesis, committed tip, and prepared successor");
    assert!(journal.durable_entry_matches_exact(&genesis_qc, &genesis_checkpoint, None,));
    assert!(journal.durable_entry_matches_exact(&tip_qc, &tip_checkpoint, None));
    assert!(journal.durable_entry_matches_exact(
        &conflicting_tip_qc,
        &conflicting_tip_checkpoint,
        None,
    ));
    assert!(journal.durable_entry_matches_exact(
        &prepared_successor_qc,
        &prepared_successor_checkpoint,
        None,
    ));
    let reloaded = CommitRosterJournal::load(path, retention(1)).expect("reload journal");
    let heights = reloaded
        .snapshots()
        .into_iter()
        .map(|snapshot| snapshot.commit_qc.height)
        .collect::<Vec<_>>();
    assert_eq!(heights, vec![1, 2, 2, 3]);
}
#[test]
fn journal_persist_removes_temp_file() {
    let dir = tempdir().expect("tempdir");
    let path = CommitRosterJournal::journal_path(dir.path());
    let (cert, checkpoint) = sample_cert(1);
    let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
    journal.upsert(cert, checkpoint, None);
    journal.persist().expect("persist");
    let tmp_path = path.with_extension("norito.tmp");
    assert!(!tmp_path.exists(), "temp journal file should be removed");
}
#[test]
fn journal_path_empty_root_is_empty() {
    let path = CommitRosterJournal::journal_path(Path::new(""));
    assert!(path.as_os_str().is_empty());
}
