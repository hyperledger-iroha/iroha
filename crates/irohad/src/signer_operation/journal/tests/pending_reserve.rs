//! Interrupted pending-Reserve staging leaves one ID closed without hiding other records.

use super::super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

const FIRST: [u8; 32] = [0x61; 32];
const INTERRUPTED: [u8; 32] = [0x62; 32];
const LATER: [u8; 32] = [0x63; 32];

fn pending_directory() -> (tempfile::TempDir, PathBuf) {
    let parent = tempfile::tempdir().unwrap();
    fs::set_permissions(parent.path(), Permissions::from_mode(0o700)).unwrap();
    let path = parent.path().join(PENDING_RESERVE_DIRECTORY);
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
    (parent, path.canonicalize().unwrap())
}

fn in_progress_file(path: &Path, operation_id: [u8; 32]) -> PathBuf {
    path.join(format!(
        "{}{}",
        hex::encode(operation_id),
        PENDING_RESERVE_IN_PROGRESS_SUFFIX
    ))
}

fn final_file(path: &Path, operation_id: [u8; 32]) -> PathBuf {
    path.join(format!(
        "{}{}",
        hex::encode(operation_id),
        PENDING_RESERVE_SUFFIX
    ))
}

#[test]
fn enospc_after_durable_tombstone_blocks_only_that_operation_after_restart() {
    let (_parent, path) = pending_directory();
    let files = SignerPendingReserveFilesV1::open(&path).unwrap();
    let first = files.stage(FIRST, b"first signed Reserve").unwrap();
    assert!(final_file(&path, FIRST).exists());
    assert!(!in_progress_file(&path, FIRST).exists());
    assert!(files.stage(FIRST, b"replacement").is_err());
    let mut reached = false;
    assert!(
        files
            .inner
            .stage_pending_reserve_with(INTERRUPTED, b"second signed Reserve", |checkpoint| {
                if checkpoint == PendingReserveCheckpoint::TombstoneDurable {
                    reached = true;
                    Err(rustix::io::Errno::NOSPC)
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert!(reached);
    let tombstone = in_progress_file(&path, INTERRUPTED);
    assert_eq!(fs::metadata(&tombstone).unwrap().len(), 0);
    // A crash after a short write leaves the same operation tombstoned.
    fs::set_permissions(&tombstone, Permissions::from_mode(0o600)).unwrap();
    fs::write(&tombstone, b"partial signed").unwrap();
    assert!(!final_file(&path, INTERRUPTED).exists());
    assert!(files.recover(INTERRUPTED).is_err());
    assert!(files.stage(INTERRUPTED, b"replacement").is_err());
    drop(first);
    drop(files);

    let reopened = SignerPendingReserveFilesV1::open(&path).unwrap();
    assert_eq!(
        reopened.recover(FIRST).unwrap().bytes(),
        b"first signed Reserve"
    );
    assert!(reopened.recover(INTERRUPTED).is_err());
    assert!(reopened.stage(INTERRUPTED, b"replacement").is_err());
    assert_eq!(
        reopened
            .stage(LATER, b"later signed Reserve")
            .unwrap()
            .bytes(),
        b"later signed Reserve"
    );
}

#[test]
fn interruption_after_signed_bytes_sync_never_publishes_an_unfinished_record() {
    let (_parent, path) = pending_directory();
    let files = SignerPendingReserveFilesV1::open(&path).unwrap();
    let first = files.stage(FIRST, b"first signed Reserve").unwrap();
    assert!(
        files
            .inner
            .stage_pending_reserve_with(INTERRUPTED, b"second signed Reserve", |checkpoint| {
                if checkpoint == PendingReserveCheckpoint::SignedBytesDurable {
                    Err(rustix::io::Errno::NOSPC)
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    let tombstone = in_progress_file(&path, INTERRUPTED);
    assert_eq!(fs::read(&tombstone).unwrap(), b"second signed Reserve");
    assert_eq!(fs::metadata(&tombstone).unwrap().mode() & 0o7777, 0o400);
    assert!(!final_file(&path, INTERRUPTED).exists());
    drop(first);
    drop(files);

    let reopened = SignerPendingReserveFilesV1::open(&path).unwrap();
    assert_eq!(
        reopened.recover(FIRST).unwrap().bytes(),
        b"first signed Reserve"
    );
    assert!(reopened.recover(INTERRUPTED).is_err());
    assert!(reopened.stage(INTERRUPTED, b"replacement").is_err());
}

#[test]
fn no_replace_publish_preserves_a_conflicting_final_record() {
    let (_parent, path) = pending_directory();
    let files = SignerPendingReserveFilesV1::open(&path).unwrap();
    let conflicting = final_file(&path, INTERRUPTED);
    assert!(
        files
            .inner
            .stage_pending_reserve_with(INTERRUPTED, b"new signed Reserve", |checkpoint| {
                if checkpoint == PendingReserveCheckpoint::SignedBytesDurable {
                    fs::write(&conflicting, b"original signed Reserve").unwrap();
                    fs::set_permissions(&conflicting, Permissions::from_mode(0o400)).unwrap();
                }
                Ok(())
            })
            .is_err()
    );
    assert_eq!(fs::read(&conflicting).unwrap(), b"original signed Reserve");
    // A duplicate same-ID final/tombstone pair indicates tampering, not a completed record.
    drop(files);
    assert!(SignerPendingReserveFilesV1::open(&path).is_err());
}

#[test]
fn in_progress_tombstones_consume_inventory_and_reject_unsafe_files() {
    let (_parent, path) = pending_directory();
    let mut profile = JournalProfile::PENDING_RESERVE;
    profile.max_records = 1;
    let inner = JournalInner::open(&path, profile).unwrap();
    assert!(
        inner
            .stage_pending_reserve_with(INTERRUPTED, b"interrupted", |_| {
                Err(rustix::io::Errno::NOSPC)
            })
            .is_err()
    );
    assert_eq!(inner.inventory().unwrap().0, 1);
    assert!(inner.stage_pending_reserve(LATER, b"later").is_err());
    drop(inner);

    let tombstone = in_progress_file(&path, INTERRUPTED);
    fs::set_permissions(&tombstone, Permissions::from_mode(0o000)).unwrap();
    assert!(SignerPendingReserveFilesV1::open(&path).is_ok());
    fs::set_permissions(&tombstone, Permissions::from_mode(0o600)).unwrap();
    for attack in ["world-readable", "hardlink", "symlink"] {
        let other = path.parent().unwrap().join("external-link");
        match attack {
            "world-readable" => {
                fs::set_permissions(&tombstone, Permissions::from_mode(0o644)).unwrap();
            }
            "hardlink" => fs::hard_link(&tombstone, &other).unwrap(),
            "symlink" => {
                fs::rename(&tombstone, &other).unwrap();
                std::os::unix::fs::symlink(&other, &tombstone).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            SignerPendingReserveFilesV1::open(&path).is_err(),
            "{attack}"
        );
        match attack {
            "world-readable" => {
                fs::set_permissions(&tombstone, Permissions::from_mode(0o600)).unwrap();
            }
            "hardlink" => fs::remove_file(&other).unwrap(),
            "symlink" => {
                fs::remove_file(&tombstone).unwrap();
                fs::rename(&other, &tombstone).unwrap();
            }
            _ => unreachable!(),
        }
    }
    assert!(SignerPendingReserveFilesV1::open(&path).is_ok());
}
