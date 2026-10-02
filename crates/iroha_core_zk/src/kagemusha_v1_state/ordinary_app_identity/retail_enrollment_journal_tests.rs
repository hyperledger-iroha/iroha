//! Real-WAL bounded replay admission tests; canonical rows confer no native retail authority.

use super::*;
use crate::kagemusha_v1_state::{PrivateJournalError, private_journal::FRAME_HEADER_BYTES};
use std::{fs::OpenOptions, path::PathBuf};

fn encoded(record: &Record) -> Vec<u8> {
    norito::encode_canonical(record).unwrap()
}

fn write_wal(rows: &[Vec<u8>]) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("retail");
    let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
    for row in rows {
        journal.append(row).unwrap();
    }
    drop(journal);
    (temp, path)
}

#[test]
fn bounded_replay_adopts_complete_original_prefix_then_scans_without_writing_or_unlocking() {
    for count in 1..=MAX_ROWS {
        // These canonical rows test byte admission only; their repetition is not a retail owner.
        let rows = vec![encoded(&Record::Invoked); count];
        let (_temp, path) = write_wal(&rows);
        let file = path.join(FORMAT.filename);
        let before = std::fs::read(&file).unwrap();
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        let mut premature_callback = false;
        assert_eq!(
            journal.scan_complete(|_, _| {
                premature_callback = true;
                Ok(())
            }),
            Err(PrivateJournalError::Corrupt)
        );
        assert!(!premature_callback);
        replay_complete_bounded(&mut journal).unwrap();
        let prefix = journal.recovery_prefix().unwrap();
        assert_eq!(prefix.sequence, count as u64);
        assert_eq!(prefix.byte_len, before.len() as u64);
        assert_ne!(prefix.head, [0; 32]);
        assert_eq!(journal.replay_next().unwrap(), None);
        let mut scanned = Vec::new();
        assert_eq!(
            journal.scan_complete(|sequence, raw| {
                assert_eq!(sequence, scanned.len() as u64);
                assert!(matches!(
                    PrivateJournal::open_existing(&path, FORMAT),
                    Err(PrivateJournalError::AlreadyOpen)
                ));
                scanned.push(raw.to_vec());
                Ok(())
            }),
            Ok(prefix)
        );
        assert_eq!(scanned, rows);
        assert_eq!(journal.recovery_prefix().unwrap(), prefix);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
}

#[test]
fn bounded_replay_refuses_fifth_complete_record_and_leaves_sixth_unread() {
    for count in [MAX_ROWS + 1, MAX_ROWS + 2] {
        let rows = vec![encoded(&Record::Invoked); count];
        let (_temp, path) = write_wal(&rows);
        let before = std::fs::read(path.join(FORMAT.filename)).unwrap();
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(matches!(
            replay_complete_bounded(&mut journal),
            Err(Custody)
        ));
        if count == MAX_ROWS + 2 {
            assert_eq!(
                journal.replay_next().unwrap(),
                Some((MAX_ROWS as u64 + 1, rows[MAX_ROWS + 1].clone()))
            );
        }
        assert_eq!(journal.replay_next().unwrap(), None);
        assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), before);
    }
}

#[test]
fn bounded_replay_refuses_empty_torn_framing_hash_and_noncanonical_prefixes() {
    let (_empty_temp, empty) = write_wal(&[]);
    let mut journal = PrivateJournal::open_existing(&empty, FORMAT).unwrap();
    assert!(matches!(
        replay_complete_bounded(&mut journal),
        Err(Custody)
    ));
    drop(journal);
    for mutation in 0..7 {
        let (_temp, path) = write_wal(&[encoded(&Record::Invoked)]);
        let file = path.join(FORMAT.filename);
        let mut bytes = std::fs::read(&file).unwrap();
        match mutation {
            0 => bytes.truncate(1),
            1 => bytes.truncate(FRAME_HEADER_BYTES - 1),
            2 => bytes.truncate(bytes.len() - 1),
            3 => bytes[16] ^= 1,
            4 => bytes[24] ^= 1,
            5 => bytes[56] ^= 1,
            _ => *bytes.last_mut().unwrap() ^= 1,
        }
        std::fs::write(&file, &bytes).unwrap();
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(
            matches!(replay_complete_bounded(&mut journal), Err(Custody)),
            "mutation {mutation}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
    }
    let mut trailing = encoded(&Record::Invoked);
    trailing.push(0);
    for row in [b"invalid canonical record".to_vec(), trailing] {
        let (_temp, path) = write_wal(&[row]);
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(matches!(
            replay_complete_bounded(&mut journal),
            Err(Custody)
        ));
    }
}

#[test]
fn bounded_replay_refuses_a_replaced_or_changed_original_descriptor() {
    for replace in [false, true] {
        let (_temp, path) = write_wal(&[encoded(&Record::Invoked)]);
        let file = path.join(FORMAT.filename);
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        if replace {
            let displaced = path.join("displaced.wal");
            std::fs::rename(&file, &displaced).unwrap();
            std::fs::copy(&displaced, &file).unwrap();
        } else {
            OpenOptions::new()
                .write(true)
                .open(&file)
                .unwrap()
                .set_len((FRAME_HEADER_BYTES - 1) as u64)
                .unwrap();
        }
        let changed = std::fs::read(&file).unwrap();
        assert!(matches!(
            replay_complete_bounded(&mut journal),
            Err(Custody)
        ));
        assert!(journal.recovery_prefix().is_err());
        assert_eq!(std::fs::read(&file).unwrap(), changed);
    }
}
