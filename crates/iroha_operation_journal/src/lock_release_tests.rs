//! Native lock release follows the owning journal, including refused admission.

use super::*;
use std::{cell::RefCell, fs::TryLockError, io};

#[test]
fn journal_release_does_not_wait_for_duplicated_descriptors() {
    for prepared in [false, true] {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("operation");
        let original = norito::json!({"signed": "retained-original"});
        let first = if prepared {
            let journal = Journal::create_prepared(&path, &original).unwrap();
            assert!(journal.record_submission(&original).unwrap());
            journal
        } else {
            Journal::create(&path).unwrap()
        };
        let directory_identity = first.directory.identity().unwrap();
        let lock_identity = FileIdentity::of(&first.lock).unwrap();
        let original_bytes = prepared.then(|| {
            ["operation.json", "submission.json"]
                .map(|name| std::fs::read(path.join(name)).unwrap())
        });
        // A duplicate retains the same open file description as an unrelated
        // child between fork and exec. No timing, child process or unsafe code
        // is needed to hold that exact native lock beyond the owner's lifetime.
        let old_duplicate = first.lock.try_clone().unwrap();
        for _ in 0..2 {
            let error = Journal::open(&path).unwrap_err();
            assert!(matches!(
                error.downcast_ref::<TryLockError>(),
                Some(TryLockError::WouldBlock)
            ));
        }
        drop(first);
        let replacement = Journal::open(&path)
            .expect("owner release must unlock while the old duplicate remains open");
        assert_eq!(
            replacement.directory.identity().unwrap(),
            directory_identity
        );
        assert_eq!(FileIdentity::of(&replacement.lock).unwrap(), lock_identity);
        assert_eq!(
            replacement
                .read_native::<norito::json::Value>(NativeRecord::Operation)
                .unwrap(),
            prepared.then_some(original.clone())
        );
        if let Some(before) = &original_bytes {
            assert!(replacement.submission_recorded(&original).unwrap());
            let after = ["operation.json", "submission.json"]
                .map(|name| std::fs::read(path.join(name)).unwrap());
            assert_eq!(&after, before);
        }
        let replacement_duplicate = replacement.lock.try_clone().unwrap();
        drop(old_duplicate);
        for _ in 0..2 {
            let error = Journal::open(&path).unwrap_err();
            assert!(matches!(
                error.downcast_ref::<TryLockError>(),
                Some(TryLockError::WouldBlock)
            ));
        }
        drop(replacement);
        // The reopened owner has the same release obligation as initial creation.
        let final_owner =
            Journal::open(&path).expect("reopened owner must release before its duplicate closes");
        drop(replacement_duplicate);
        let error = Journal::open(&path).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        assert_eq!(FileIdentity::of(&final_owner.lock).unwrap(), lock_identity);
        final_owner.verify_native_inventory().unwrap();
        assert_eq!(
            std::fs::read_dir(&path).unwrap().count(),
            if prepared { 3 } else { 1 }
        );
        drop(final_owner);
        Journal::open(&path).expect("final owner releases its lock");
    }
}

#[test]
fn journal_sync_refusal_releases_acquired_lock_with_a_retained_duplicate() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = OwnerDirectory::open(temporary.path()).unwrap();
    let directory = parent.create_private_child("operation").unwrap();
    let path = directory.path().to_owned();
    let inherited = RefCell::new(None);
    let failure = Journal::lock_directory_with_sync(directory, true, |journal| {
        journal.directory.sync()?;
        // Capture the actually acquired description before reporting refusal at
        // the same injected durability boundary used by the existing controls.
        *inherited.borrow_mut() = Some(journal.lock.try_clone()?);
        let contender = Journal::open(&path).unwrap_err();
        assert!(matches!(
            contender.downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        Err(io::Error::other("injected new-journal durability failure"))
    })
    .unwrap_err();
    assert_eq!(
        failure.downcast_ref::<io::Error>().unwrap().to_string(),
        "injected new-journal durability failure"
    );
    let duplicate = inherited
        .into_inner()
        .expect("acquired descriptor captured");
    let lock_identity = FileIdentity::of(&duplicate).unwrap();
    assert!(path.join("lock").is_file());
    assert!(!path.join("operation.json").exists());
    assert!(Journal::create(&path).is_err());
    let reopened = Journal::open(&path)
        .expect("post-acquisition refusal releases before its duplicate closes");
    assert_eq!(FileIdentity::of(&reopened.lock).unwrap(), lock_identity);
    drop(duplicate);
    let contender = Journal::open(&path).unwrap_err();
    assert!(matches!(
        contender.downcast_ref::<TryLockError>(),
        Some(TryLockError::WouldBlock)
    ));
    let original = norito::json!({"signed": "same-journal-after-refusal"});
    reopened.write_operation(&original).unwrap();
    assert!(reopened.write_operation(&original).is_err());
    drop(reopened);
    let retained = Journal::open(&path).unwrap();
    assert_eq!(
        retained.read_operation::<norito::json::Value>().unwrap(),
        original
    );
    assert_eq!(FileIdentity::of(&retained.lock).unwrap(), lock_identity);
}
