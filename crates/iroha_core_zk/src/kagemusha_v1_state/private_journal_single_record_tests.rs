//! Original single-record transport checks. Synthetic bytes confer no native authority.

use super::*;
use std::{fs::OpenOptions, os::fd::AsRawFd as _};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "original-record.wal",
    magic: b"IKGORIG1",
    hash_domain: b"test-only:single-original-record\0",
    maximum_payload_bytes: 32 * 1024,
};

fn payload() -> Vec<u8> {
    (0..(2 * 8192 + 37))
        .map(|index| (index % 251) as u8)
        .collect()
}

fn fixture(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, PrivateJournal) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("journal");
    let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
    journal.append(bytes).unwrap();
    (root, path, journal)
}

fn state(journal: &mut PrivateJournal) -> (u64, u64, u64, DigestV1, u64, i32) {
    (
        journal.acknowledged_bytes,
        journal.read_bytes,
        journal.next_sequence,
        journal.previous_frame_hash,
        journal.journal.stream_position().unwrap(),
        journal.journal.as_raw_fd(),
    )
}

fn assert_poisoned(journal: &mut PrivateJournal, expected: &[u8]) {
    assert!(journal.poisoned.get());
    assert_eq!(journal.verified_recovery_prefix.get(), None);
    assert_eq!(journal.check_owned(), Err(PrivateJournalError::Uncertain));
    assert_eq!(
        journal.require_single_record(expected),
        Err(PrivateJournalError::Uncertain)
    );
    assert_eq!(
        journal.append(b"must not publish after uncertainty"),
        Err(PrivateJournalError::Uncertain)
    );
}

#[test]
fn exact_multichunk_original_preserves_cursor_descriptor_lock_bytes_and_selected_prefix() {
    let original = payload();
    let (_root, path, mut journal) = fixture(&original);
    let prefix = journal.recovery_prefix().unwrap();
    assert!(journal.contains_recovery_prefix(prefix).unwrap());
    let before = state(&mut journal);
    let file_bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
    for _ in 0..3 {
        assert_eq!(journal.require_single_record(&original), Ok(()));
        assert_eq!(state(&mut journal), before);
        assert_eq!(journal.recovery_prefix().unwrap(), prefix);
        assert_eq!(journal.verified_recovery_prefix.get(), Some(prefix));
        assert!(matches!(
            PrivateJournal::open_existing(&path, FORMAT),
            Err(PrivateJournalError::AlreadyOpen)
        ));
    }
    assert_eq!(
        std::fs::read(path.join(FORMAT.filename)).unwrap(),
        file_bytes
    );
    assert!(!journal.poisoned.get());
}

#[test]
fn reopened_complete_original_requires_replay_then_preserves_owned_transport() {
    let original = payload();
    let (_root, path, journal) = fixture(&original);
    let prefix = journal.recovery_prefix().unwrap();
    drop(journal);
    let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
    assert_eq!(journal.replay_next().unwrap(), Some((0, original.clone())));
    assert_eq!(journal.replay_next().unwrap(), None);
    let before = state(&mut journal);
    assert_eq!(journal.require_single_record(&original), Ok(()));
    assert_eq!(journal.recovery_prefix().unwrap(), prefix);
    assert_eq!(state(&mut journal), before);
    // Replayed hashes and matching bytes are transport facts, not Core/hardware authority.
}

#[test]
fn incomplete_replay_cannot_admit_original_and_poisons_the_pending_owner() {
    let original = payload();
    let (_root, path, journal) = fixture(&original);
    drop(journal);
    let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
    assert_eq!(
        journal.require_single_record(&original),
        Err(PrivateJournalError::Corrupt)
    );
    assert_poisoned(&mut journal, &original);
    // A new locked owner must replay independently; a failed pending owner is never reused.
    drop(journal);
    let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
    assert_eq!(reopened.replay_next().unwrap(), Some((0, original.clone())));
    assert_eq!(reopened.replay_next().unwrap(), None);
    assert_eq!(reopened.require_single_record(&original), Ok(()));
}

#[test]
fn expected_byte_substitutions_and_shape_changes_never_accept_a_matching_transport_hash() {
    let original = payload();
    for change in 0..7 {
        let (_root, path, mut journal) = fixture(&original);
        let prefix = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(prefix).unwrap());
        let file_bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
        let mut expected = original.clone();
        match change {
            0 => expected[0] ^= 1,
            1 => expected[8192] ^= 1,
            2 => *expected.last_mut().unwrap() ^= 1,
            3 => expected.pop().map(|_| ()).unwrap(),
            4 => expected.push(7),
            5 => expected.clear(),
            _ => expected.resize(FORMAT.maximum_payload_bytes as usize + 1, 0),
        }
        assert_eq!(
            journal.require_single_record(&expected),
            Err(PrivateJournalError::Corrupt)
        );
        assert_poisoned(&mut journal, &original);
        assert_eq!(
            std::fs::read(path.join(FORMAT.filename)).unwrap(),
            file_bytes
        );
    }
}

#[test]
fn complete_second_frame_is_not_an_immutable_single_record_even_with_valid_ancestry() {
    let original = payload();
    let (_root, path, mut journal) = fixture(&original);
    let selected = journal.recovery_prefix().unwrap();
    journal
        .append(b"valid but forbidden publication suffix")
        .unwrap();
    assert!(journal.contains_recovery_prefix(selected).unwrap());
    let file_bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
    assert_eq!(
        journal.require_single_record(&original),
        Err(PrivateJournalError::Corrupt)
    );
    assert_poisoned(&mut journal, &original);
    assert_eq!(
        std::fs::read(path.join(FORMAT.filename)).unwrap(),
        file_bytes
    );
}

#[test]
fn actual_descriptor_payload_is_compared_even_when_cached_metadata_is_test_refreshed() {
    let original = payload();
    for recompute_transport_hash in [false, true] {
        let (_root, path, mut journal) = fixture(&original);
        let mut file_bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
        file_bytes[FRAME_HEADER_BYTES + original.len() - 1] ^= 1;
        if recompute_transport_hash {
            let mut digest = Sha256::new();
            digest.update(FORMAT.hash_domain);
            digest.update(&file_bytes[..56]);
            digest.update(&file_bytes[FRAME_HEADER_BYTES..]);
            let hash: DigestV1 = digest.finalize().into();
            file_bytes[56..88].copy_from_slice(&hash);
            // Test-only removal of the metadata/head fast guards proves the original-byte
            // comparison remains independent even for an internally consistent replacement.
            journal.previous_frame_hash = hash;
        }
        std::fs::write(path.join(FORMAT.filename), &file_bytes).unwrap();
        journal.observed_version =
            JournalFileVersion::from_metadata(&journal.journal.metadata().unwrap());
        assert_eq!(journal.check_owned(), Ok(()));
        assert_eq!(
            journal.require_single_record(&original),
            Err(PrivateJournalError::Corrupt)
        );
        assert_poisoned(&mut journal, &original);
    }
}

#[test]
fn actual_descriptor_header_is_checked_even_when_cached_metadata_is_test_refreshed() {
    let original = payload();
    for header_byte in [0, 8, 16, 24, 56] {
        let (_root, path, mut journal) = fixture(&original);
        let mut file_bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
        file_bytes[header_byte] ^= 1;
        std::fs::write(path.join(FORMAT.filename), &file_bytes).unwrap();
        journal.observed_version =
            JournalFileVersion::from_metadata(&journal.journal.metadata().unwrap());
        assert_eq!(journal.check_owned(), Ok(()));
        assert_eq!(
            journal.require_single_record(&original),
            Err(PrivateJournalError::Corrupt)
        );
        assert_poisoned(&mut journal, &original);
    }
}

#[test]
fn inplace_same_length_edit_is_rejected_by_original_metadata_custody() {
    let original = payload();
    let (_root, path, mut journal) = fixture(&original);
    let file = OpenOptions::new()
        .write(true)
        .open(path.join(FORMAT.filename))
        .unwrap();
    file.write_all_at(&[original[8192] ^ 1], (FRAME_HEADER_BYTES + 8192) as u64)
        .unwrap();
    file.sync_all().unwrap();
    assert!(journal.require_single_record(&original).is_err());
    assert_poisoned(&mut journal, &original);
}

#[test]
fn identical_named_file_replacement_cannot_redirect_the_held_original() {
    let original = payload();
    let (_root, path, mut journal) = fixture(&original);
    let file = path.join(FORMAT.filename);
    let displaced = path.join("displaced-original.wal");
    let original_bytes = std::fs::read(&file).unwrap();
    std::fs::rename(&file, &displaced).unwrap();
    std::fs::copy(&displaced, &file).unwrap();
    assert_ne!(
        std::fs::metadata(&file).unwrap().ino(),
        journal.file_identity.1
    );
    assert!(journal.require_single_record(&original).is_err());
    assert_poisoned(&mut journal, &original);
    assert_eq!(std::fs::read(&displaced).unwrap(), original_bytes);
    assert_eq!(std::fs::read(&file).unwrap(), original_bytes);
}

#[test]
fn identical_directory_replacement_cannot_redirect_the_held_original() {
    let original = payload();
    let (root, path, mut journal) = fixture(&original);
    let displaced = root.path().join("displaced-directory");
    std::fs::rename(&path, &displaced).unwrap();
    std::fs::create_dir(&path).unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::copy(displaced.join(FORMAT.filename), path.join(FORMAT.filename)).unwrap();
    assert!(journal.require_single_record(&original).is_err());
    assert_poisoned(&mut journal, &original);
}

#[test]
fn uncertain_complete_append_never_reuses_owner_and_reopen_only_adopts_durable_bytes() {
    let original = payload();
    for failure in [
        TestPersistenceFailure::BeforeSync,
        TestPersistenceFailure::AfterSync,
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("journal");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        journal.failure.set(Some(failure));
        assert_eq!(
            journal.append(&original),
            Err(PrivateJournalError::Uncertain)
        );
        assert_poisoned(&mut journal, &original);
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(reopened.replay_next().unwrap(), Some((0, original.clone())));
        assert_eq!(reopened.replay_next().unwrap(), None);
        assert_eq!(reopened.require_single_record(&original), Ok(()));
        // Adoption does not establish a Core predecessor, successor, CAS or current selection.
    }
}

#[test]
fn torn_uncertain_append_cannot_be_adopted_as_a_complete_original() {
    let original = payload();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("journal");
    let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
    journal
        .failure
        .set(Some(TestPersistenceFailure::PartialWrite));
    assert_eq!(
        journal.append(&original),
        Err(PrivateJournalError::Uncertain)
    );
    assert_poisoned(&mut journal, &original);
    drop(journal);
    let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
    assert!(reopened.replay_next().is_err());
    assert!(reopened.require_single_record(&original).is_err());
    assert_eq!(
        std::fs::metadata(path.join(FORMAT.filename)).unwrap().len(),
        11
    );
}
