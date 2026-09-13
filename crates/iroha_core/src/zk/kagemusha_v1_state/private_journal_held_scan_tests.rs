//! Descriptor-held complete-scan boundary tests; callbacks never confer recovery authority.

use super::*;
use std::{fs::OpenOptions, os::fd::AsRawFd as _, panic::AssertUnwindSafe};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "held-scan.wal",
    magic: b"IKGSCAN1",
    hash_domain: b"test-only:held-journal-scan\0",
    maximum_payload_bytes: 1024,
};

fn fixture() -> (tempfile::TempDir, PathBuf, PrivateJournal) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("journal");
    let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
    journal.append(b"initialize").unwrap();
    journal.append(b"selected operation").unwrap();
    (root, path, journal)
}

fn state(
    journal: &mut PrivateJournal,
) -> (
    u64,
    u64,
    u64,
    DigestV1,
    u64,
    i32,
    (u64, u64),
    JournalFileVersion,
) {
    (
        journal.acknowledged_bytes,
        journal.read_bytes,
        journal.next_sequence,
        journal.previous_frame_hash,
        journal.journal.stream_position().unwrap(),
        journal.journal.as_raw_fd(),
        journal.file_identity,
        journal.observed_version,
    )
}

fn replay_all(path: &Path) -> (PrivateJournal, Vec<(u64, Vec<u8>)>) {
    let mut journal = PrivateJournal::open_existing(path, FORMAT).unwrap();
    let mut records = Vec::new();
    while let Some(record) = journal.replay_next().unwrap() {
        records.push(record);
    }
    (journal, records)
}

fn assert_poisoned(journal: &mut PrivateJournal) {
    assert!(journal.poisoned.get());
    assert!(!journal.scanning.get());
    assert_eq!(journal.verified_recovery_prefix.get(), None);
    assert_eq!(journal.check_owned(), Err(PrivateJournalError::Uncertain));
    assert_eq!(
        journal.recovery_prefix(),
        Err(PrivateJournalError::Uncertain)
    );
    assert_eq!(
        journal.scan_complete(|_, _| panic!("poisoned scan callback")),
        Err(PrivateJournalError::Uncertain)
    );
    assert_eq!(
        journal.append(b"must not write"),
        Err(PrivateJournalError::Uncertain)
    );
}

#[test]
fn complete_scan_visits_selected_prefix_and_suffix_without_moving_cursor_or_releasing_lock() {
    let (_root, path, mut journal) = fixture();
    let selected = journal.recovery_prefix().unwrap();
    journal.append(b"unselected complete suffix").unwrap();
    assert!(journal.contains_recovery_prefix(selected).unwrap());
    let complete = journal.recovery_prefix().unwrap();
    let before = state(&mut journal);
    let bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
    let mut actual = Vec::new();
    assert_eq!(
        journal.scan_complete(|sequence, payload| {
            assert!(matches!(
                PrivateJournal::open_existing(&path, FORMAT),
                Err(PrivateJournalError::AlreadyOpen)
            ));
            actual.push((sequence, payload.to_vec()));
            Ok(())
        }),
        Ok(complete)
    );
    assert_eq!(
        actual,
        vec![
            (0, b"initialize".to_vec()),
            (1, b"selected operation".to_vec()),
            (2, b"unselected complete suffix".to_vec())
        ]
    );
    assert_eq!(state(&mut journal), before);
    assert_eq!(journal.verified_recovery_prefix.get(), Some(selected));
    assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
    assert!(!journal.poisoned.get());
    assert!(!journal.scanning.get());
    journal.append(b"ordinary later append").unwrap();
    let extended = journal.recovery_prefix().unwrap();
    drop(journal);
    let (mut reopened, replayed) = replay_all(&path);
    assert_eq!(&replayed[..3], actual.as_slice());
    assert_eq!(replayed[3], (3, b"ordinary later append".to_vec()));
    let before = state(&mut reopened);
    let mut rescanned = Vec::new();
    assert_eq!(
        reopened.scan_complete(|s, p| {
            rescanned.push((s, p.to_vec()));
            Ok(())
        }),
        Ok(extended)
    );
    assert_eq!(rescanned, replayed);
    assert_eq!(state(&mut reopened), before);
}

#[test]
fn incomplete_replay_refuses_admission_then_normal_replay_and_scan_remain_usable() {
    let (_root, path, journal) = fixture();
    drop(journal);
    let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
    assert_eq!(
        journal.replay_next().unwrap(),
        Some((0, b"initialize".to_vec()))
    );
    let before = state(&mut journal);
    assert_eq!(
        journal.scan_complete(|_, _| panic!("incomplete replay callback")),
        Err(PrivateJournalError::Corrupt)
    );
    assert_eq!(state(&mut journal), before);
    assert!(!journal.poisoned.get());
    assert!(!journal.scanning.get());
    assert_eq!(
        journal.replay_next().unwrap(),
        Some((1, b"selected operation".to_vec()))
    );
    assert_eq!(journal.replay_next().unwrap(), None);
    let expected = journal.recovery_prefix().unwrap();
    assert_eq!(journal.scan_complete(|_, _| Ok(())), Ok(expected));
}

#[test]
fn recursive_scan_refuses_before_materializing_another_record_and_outer_scan_can_finish() {
    let (_root, _path, mut journal) = fixture();
    let expected = journal.recovery_prefix().unwrap();
    let before = state(&mut journal);
    let mut outer_calls = 0;
    assert_eq!(
        journal.scan_complete(|_, _| {
            outer_calls += 1;
            assert!(journal.scanning.get());
            assert_eq!(
                journal.scan_complete(|_, _| panic!("nested record allocation")),
                Err(PrivateJournalError::Corrupt)
            );
            assert!(!journal.poisoned.get());
            Ok(())
        }),
        Ok(expected)
    );
    assert_eq!(outer_calls, 2);
    assert_eq!(state(&mut journal), before);
    assert!(!journal.scanning.get());
    journal
        .append(b"usable after handled nested refusal")
        .unwrap();
}

#[test]
fn callback_error_and_unwind_poison_clear_cache_preserve_bytes_and_release_lock_on_drop() {
    for unwind in [false, true] {
        let (_root, path, mut journal) = fixture();
        let selected = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        let before = state(&mut journal);
        let bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
        let mut calls = 0;
        if unwind {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                journal.scan_complete(|_, _| {
                    calls += 1;
                    panic!("test consumer unwind");
                })
            }));
            assert!(result.is_err());
        } else {
            assert_eq!(
                journal.scan_complete(|_, _| {
                    calls += 1;
                    Err(PrivateJournalError::AlreadyOpen)
                }),
                Err(PrivateJournalError::AlreadyOpen)
            );
        }
        assert_eq!(calls, 1);
        assert_eq!(state(&mut journal), before);
        assert_poisoned(&mut journal);
        assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
        assert!(matches!(
            PrivateJournal::open_existing(&path, FORMAT),
            Err(PrivateJournalError::AlreadyOpen)
        ));
        drop(journal);
        let (reopened, records) = replay_all(&path);
        assert_eq!(records.len(), 2);
        assert_eq!(reopened.scan_complete(|_, _| Ok(())), Ok(selected));
    }
}

#[test]
fn callback_mutations_refuse_valid_append_replacement_truncation_and_same_length_edit() {
    for mutation in 0..4 {
        let (_root, path, mut journal) = fixture();
        let selected = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        let before = state(&mut journal);
        let file = path.join(FORMAT.filename);
        let initial = std::fs::read(&file).unwrap();
        let mut calls = 0;
        let mut externally_written = Vec::new();
        let result = journal.scan_complete(|_, _| {
            calls += 1;
            match mutation {
                0 => {
                    // Construct a genuinely valid next frame. Its validity does not let the
                    // held scan silently adopt a later complete end than it captured.
                    let payload = b"valid external suffix";
                    let mut header = [0; FRAME_HEADER_BYTES];
                    header[..8].copy_from_slice(FORMAT.magic);
                    header[8..16].copy_from_slice(&(payload.len() as u64).to_le_bytes());
                    header[16..24].copy_from_slice(&selected.sequence.to_le_bytes());
                    header[24..56].copy_from_slice(&selected.head);
                    let hash = journal.frame_hash(&header[..56], payload);
                    header[56..].copy_from_slice(&hash);
                    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
                    writer.write_all(&header).unwrap();
                    writer.write_all(payload).unwrap();
                }
                1 => {
                    let displaced = path.join("displaced.wal");
                    std::fs::rename(&file, &displaced).unwrap();
                    std::fs::copy(&displaced, &file).unwrap();
                }
                2 => {
                    OpenOptions::new()
                        .write(true)
                        .open(&file)
                        .unwrap()
                        .set_len(0)
                        .unwrap();
                }
                _ => {
                    let mut changed = initial.clone();
                    *changed.last_mut().unwrap() ^= 1;
                    std::fs::write(&file, changed).unwrap();
                }
            }
            externally_written = std::fs::read(&file).unwrap();
            Ok(())
        });
        assert_eq!(result, Err(PrivateJournalError::Corrupt));
        assert_eq!(
            calls, 1,
            "never deliver any subsequent record after mutation"
        );
        assert_eq!(state(&mut journal), before);
        assert_poisoned(&mut journal);
        assert_eq!(
            std::fs::read(&file).unwrap(),
            externally_written,
            "scan and rejected append never repair or retire bytes"
        );
        drop(journal);
        if mutation == 0 {
            let (reopened, records) = replay_all(&path);
            assert_eq!(records[2], (2, b"valid external suffix".to_vec()));
            assert!(reopened.contains_recovery_prefix(selected).unwrap());
            assert_eq!(reopened.scan_complete(|_, _| Ok(())).unwrap().sequence, 3);
        } else if mutation == 1 {
            let (reopened, records) = replay_all(&path);
            assert_eq!(records.len(), 2);
            assert_eq!(reopened.recovery_prefix().unwrap(), selected);
            assert_eq!(externally_written, initial);
        }
    }
}

#[test]
fn ownership_change_is_checked_even_when_callback_returns_an_error() {
    let (_root, path, mut journal) = fixture();
    let result = journal.scan_complete(|_, _| {
        OpenOptions::new()
            .write(true)
            .open(path.join(FORMAT.filename))
            .unwrap()
            .set_len(0)
            .unwrap();
        Err(PrivateJournalError::AlreadyOpen)
    });
    assert_eq!(
        result,
        Err(PrivateJournalError::Corrupt),
        "changed ownership takes precedence over callback error"
    );
    assert_poisoned(&mut journal);
    assert_eq!(
        std::fs::metadata(path.join(FORMAT.filename)).unwrap().len(),
        0
    );
}

#[test]
fn shared_frame_grammar_rejects_corrupt_and_torn_records_before_delivering_them() {
    for defect in 0..9 {
        let (_root, path, mut journal) = fixture();
        let selected = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        let file = path.join(FORMAT.filename);
        let mut bytes = std::fs::read(&file).unwrap();
        match defect {
            0 => bytes[0] ^= 1,
            1 => bytes[8..16].copy_from_slice(&0_u64.to_le_bytes()),
            2 => bytes[8..16].copy_from_slice(&(FORMAT.maximum_payload_bytes + 1).to_le_bytes()),
            3 => bytes[16..24].copy_from_slice(&1_u64.to_le_bytes()),
            4 => bytes[24] ^= 1,
            5 => bytes[56] ^= 1,
            6 => bytes[FRAME_HEADER_BYTES] ^= 1,
            7 => bytes.truncate(FRAME_HEADER_BYTES - 1),
            _ => {
                bytes.pop();
            }
        }
        std::fs::write(&file, &bytes).unwrap();
        // Test-only adoption bypasses the separate ownership-generation rejection, so this
        // test exercises the shared frame parser and captured-boundary checks themselves.
        // No production caller can assign these private replay/ownership fields.
        let metadata = journal.journal.metadata().unwrap();
        journal.observed_version = JournalFileVersion::from_metadata(&metadata);
        journal.acknowledged_bytes = metadata.len();
        journal.read_bytes = metadata.len();
        let mut calls = 0;
        assert_eq!(
            journal.scan_complete(|_, _| {
                calls += 1;
                Ok(())
            }),
            Err(PrivateJournalError::Corrupt)
        );
        assert_eq!(calls, usize::from(defect == 8));
        assert_poisoned(&mut journal);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        drop(journal);
        // Ordinary replay is the grammar oracle for the same malformed fixture.
        let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        if defect == 8 {
            assert!(reopened.replay_next().unwrap().is_some());
        }
        assert_eq!(reopened.replay_next(), Err(PrivateJournalError::Corrupt));
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
    }
}

#[test]
fn scan_checks_captured_complete_head_and_exact_record_count() {
    for defect in 0..3 {
        let (_root, path, mut journal) = fixture();
        match defect {
            0 => journal.previous_frame_hash[0] ^= 1,
            1 => journal.next_sequence -= 1,
            _ => journal.next_sequence += 1,
        }
        let bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
        let mut calls = 0;
        assert_eq!(
            journal.scan_complete(|_, _| {
                calls += 1;
                Ok(())
            }),
            Err(PrivateJournalError::Corrupt)
        );
        assert_eq!(calls, if defect == 1 { 1 } else { 2 });
        assert_poisoned(&mut journal);
        assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
    }
}

#[test]
fn positional_read_failure_poison_is_explicit_without_unsafe_descriptor_operations() {
    let (_root, path, mut journal) = fixture();
    let selected = journal.recovery_prefix().unwrap();
    assert!(journal.contains_recovery_prefix(selected).unwrap());
    let bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
    // Fixture-only read-error injection: the replacement descriptor names the exact same
    // inode/version but is write-only. Retain the original locked descriptor until drop;
    // no production read hook, unsafe close, subprocess or signal is needed.
    let write_only = OpenOptions::new()
        .write(true)
        .open(path.join(FORMAT.filename))
        .unwrap();
    let locked_readable = std::mem::replace(&mut journal.journal, write_only);
    let original_offset = (&locked_readable).stream_position().unwrap();
    assert_eq!(journal.check_owned(), Ok(()));
    assert_eq!(
        journal.scan_complete(|_, _| panic!("failed read delivered bytes")),
        Err(PrivateJournalError::StorageUnavailable)
    );
    assert_poisoned(&mut journal);
    assert_eq!(
        (&locked_readable).stream_position().unwrap(),
        original_offset
    );
    assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
    assert!(matches!(
        PrivateJournal::open_existing(&path, FORMAT),
        Err(PrivateJournalError::AlreadyOpen)
    ));
    drop(journal);
    drop(locked_readable);
    let (reopened, records) = replay_all(&path);
    assert_eq!(records.len(), 2);
    assert_eq!(reopened.scan_complete(|_, _| Ok(())), Ok(selected));
}

#[test]
fn maximum_sized_records_are_borrowed_one_at_a_time_without_changing_persistence() {
    let (_root, path, mut journal) = fixture();
    let payload = vec![93; FORMAT.maximum_payload_bytes as usize];
    for _ in 0..24 {
        journal.append(&payload).unwrap();
    }
    let expected = journal.recovery_prefix().unwrap();
    let bytes = std::fs::read(path.join(FORMAT.filename)).unwrap();
    let before = state(&mut journal);
    let mut count = 0;
    let mut largest = 0;
    assert_eq!(
        journal.scan_complete(|sequence, current| {
            count += 1;
            largest = largest.max(current.len());
            assert!(current.len() <= FORMAT.maximum_payload_bytes as usize);
            if sequence >= 2 {
                assert_eq!(current, payload.as_slice());
            }
            assert_eq!(
                journal.scan_complete(|_, _| panic!("second live scan payload")),
                Err(PrivateJournalError::Corrupt)
            );
            Ok(())
        }),
        Ok(expected)
    );
    assert_eq!(count, 26);
    assert_eq!(largest, FORMAT.maximum_payload_bytes as usize);
    assert_eq!(state(&mut journal), before);
    assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
}

#[test]
fn callback_destructor_mutation_and_unwind_happen_before_scan_completion() {
    struct ConsumerDrop<'a> {
        path: &'a Path,
        unwind: bool,
        calls: &'a Cell<usize>,
        dropped: &'a Cell<bool>,
    }
    impl ConsumerDrop<'_> {
        fn touch(&self) {
            self.calls.set(self.calls.get() + 1);
        }
    }
    impl Drop for ConsumerDrop<'_> {
        fn drop(&mut self) {
            self.dropped.set(true);
            if self.unwind {
                panic!("test callback destructor unwind");
            }
            OpenOptions::new()
                .write(true)
                .open(self.path.join(FORMAT.filename))
                .unwrap()
                .set_len(0)
                .unwrap();
        }
    }
    for unwind in [false, true] {
        let (_root, path, mut journal) = fixture();
        let selected = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        let initial = std::fs::read(path.join(FORMAT.filename)).unwrap();
        let calls = Cell::new(0);
        let dropped = Cell::new(false);
        let consumer = ConsumerDrop {
            path: &path,
            unwind,
            calls: &calls,
            dropped: &dropped,
        };
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            journal.scan_complete(move |_, _| {
                consumer.touch();
                Ok(())
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(PrivateJournalError::Corrupt));
        }
        assert_eq!(calls.get(), 2);
        assert!(dropped.get());
        assert_poisoned(&mut journal);
        let actual = std::fs::read(path.join(FORMAT.filename)).unwrap();
        if unwind {
            assert_eq!(actual, initial);
        } else {
            assert!(actual.is_empty());
        }
    }
}
