//! Borrowed cross-parent journal originals, exact predecessors and failure-phase controls.

use super::*;
use std::{fs, io::Write as _, panic::AssertUnwindSafe, process::Command};

fn roots() -> (tempfile::TempDir, OwnerDirectory, OwnerDirectory) {
    let temporary = tempfile::Builder::new()
        .prefix("iroha-cross-parent-journal-")
        .tempdir_in(std::env::temp_dir())
        .unwrap();
    let source = OwnerDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let destination = OwnerDirectory::open_or_create(temporary.path().join("destination")).unwrap();
    (temporary, source, destination)
}

fn create(directory: &OwnerDirectory, name: &str, bytes: &[u8]) -> File {
    let mut slot = None;
    directory
        .create_private_journal_into(name, &mut slot)
        .unwrap();
    let mut original = slot.unwrap();
    original.write_all(bytes).unwrap();
    original.sync_all().unwrap();
    directory.require_same_lock(name, &original).unwrap();
    original
}

fn grant(path: &Path, access: &str) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", access])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cross_parent_create_preserves_original_and_both_parent_identities() {
    let (_temporary, source, destination) = roots();
    let parents = (source.identity().unwrap(), destination.identity().unwrap());
    let original = create(&source, "record", b"complete original");
    let identity = FileIdentity::of(&original).unwrap();
    let mut published = false;
    // Identical leaf names in distinct retained parents are not the same namespace slot.
    destination
        .publish_private_journal_from_observed(
            (&source, "record"),
            "record",
            &original,
            None,
            &mut published,
            |_| Ok(()),
        )
        .unwrap();
    assert!(published);
    assert!(!source.path().join("record").exists());
    destination.require_same_lock("record", &original).unwrap();
    assert_eq!(FileIdentity::of(&original).unwrap(), identity);
    assert_eq!(
        fs::read(destination.path().join("record")).unwrap(),
        b"complete original"
    );
    original.sync_all().unwrap();
    source.sync().unwrap();
    destination.sync().unwrap();
    assert_eq!(
        (source.identity().unwrap(), destination.identity().unwrap()),
        parents
    );
}

#[test]
fn cross_parent_replace_keeps_displaced_and_new_originals_live() {
    let (_temporary, source, destination) = roots();
    let old = create(&destination, "record", b"old exact");
    let old_id = FileIdentity::of(&old).unwrap();
    let new = create(&source, "pending", b"new exact");
    let new_id = FileIdentity::of(&new).unwrap();
    let mut published = false;
    destination
        .publish_private_journal_from_observed(
            (&source, "pending"),
            "record",
            &new,
            Some(&old),
            &mut published,
            |_| Ok(()),
        )
        .unwrap();
    assert!(published);
    destination.require_retired_private_journal(&old).unwrap();
    destination.require_same_lock("record", &new).unwrap();
    assert_eq!(FileIdentity::of(&old).unwrap(), old_id);
    assert_eq!(FileIdentity::of(&new).unwrap(), new_id);
    let mut old_bytes = [0_u8; 9];
    crate::read_exact_at(&old, &mut old_bytes, 0).unwrap();
    assert_eq!(&old_bytes, b"old exact");
    assert_eq!(
        fs::read(destination.path().join("record")).unwrap(),
        b"new exact"
    );
    assert!(!source.path().join("pending").exists());
    source.revalidate().unwrap();
    destination.revalidate().unwrap();
}

#[test]
fn cross_parent_occupied_absence_and_wrong_predecessor_never_publish() {
    let (_temporary, source, destination) = roots();
    let old = create(&destination, "record", b"same bytes");
    let pending = create(&source, "pending", b"new bytes");
    let mut published = false;
    let mut callbacks = 0;
    let error = destination
        .publish_private_journal_from_observed(
            (&source, "pending"),
            "record",
            &pending,
            None,
            &mut published,
            |_| {
                callbacks += 1;
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(callbacks, 0);
    assert!(!published);
    fs::rename(
        destination.path().join("record"),
        destination.path().join("displaced"),
    )
    .unwrap();
    let substitute = create(&destination, "record", b"same bytes");
    assert!(
        destination
            .publish_private_journal_from_observed(
                (&source, "pending"),
                "record",
                &pending,
                Some(&old),
                &mut published,
                |_| Ok(())
            )
            .is_err()
    );
    assert!(!published);
    source.require_same_lock("pending", &pending).unwrap();
    destination
        .require_same_lock("record", &substitute)
        .unwrap();
    destination.require_same_lock("displaced", &old).unwrap();
}

#[test]
fn cross_parent_source_substitution_preserves_both_named_objects() {
    let (_temporary, source, destination) = roots();
    let pending = create(&source, "pending", b"same bytes");
    let mut substitute = None;
    let mut published = false;
    let result = destination.publish_private_journal_from_observed(
        (&source, "pending"),
        "record",
        &pending,
        None,
        &mut published,
        |step| {
            assert_eq!(step, AtomicWriteStep::Publish);
            fs::rename(
                source.path().join("pending"),
                source.path().join("displaced"),
            )?;
            substitute = Some(create(&source, "pending", b"same bytes"));
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!published);
    source.require_same_lock("displaced", &pending).unwrap();
    source
        .require_same_lock("pending", substitute.as_ref().unwrap())
        .unwrap();
    assert!(!destination.path().join("record").exists());
    assert_eq!(
        fs::read(source.path().join("pending")).unwrap(),
        b"same bytes"
    );
    assert_eq!(
        fs::read(source.path().join("displaced")).unwrap(),
        b"same bytes"
    );
}

#[test]
fn cross_parent_prepublication_error_retries_only_the_same_originals() {
    let (_temporary, source, destination) = roots();
    let old = create(&destination, "record", b"old");
    let pending = create(&source, "pending", b"new");
    let identities = (
        FileIdentity::of(&old).unwrap(),
        FileIdentity::of(&pending).unwrap(),
    );
    let mut published = false;
    let error = destination
        .publish_private_journal_from_observed(
            (&source, "pending"),
            "record",
            &pending,
            Some(&old),
            &mut published,
            |step| {
                assert_eq!(step, AtomicWriteStep::Publish);
                Err(io::Error::other("first prepublication refusal"))
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "first prepublication refusal");
    assert!(!published);
    source.require_same_lock("pending", &pending).unwrap();
    destination.require_same_lock("record", &old).unwrap();
    destination
        .publish_private_journal_from_observed(
            (&source, "pending"),
            "record",
            &pending,
            Some(&old),
            &mut published,
            |_| Ok(()),
        )
        .unwrap();
    assert!(published);
    assert_eq!(
        (
            FileIdentity::of(&old).unwrap(),
            FileIdentity::of(&pending).unwrap()
        ),
        identities
    );
    destination.require_retired_private_journal(&old).unwrap();
    destination.require_same_lock("record", &pending).unwrap();
}

#[test]
fn cross_parent_postpublication_error_latches_and_refuses_a_second_rename() {
    let (_temporary, source, destination) = roots();
    let old = create(&destination, "record", b"old");
    let pending = create(&source, "pending", b"complete");
    let id = FileIdentity::of(&pending).unwrap();
    let mut published = false;
    let error = destination
        .publish_private_journal_from_observed(
            (&source, "pending"),
            "record",
            &pending,
            Some(&old),
            &mut published,
            |step| {
                if step == AtomicWriteStep::Published {
                    Err(io::Error::other("first published refusal"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "first published refusal");
    assert!(published);
    destination.require_retired_private_journal(&old).unwrap();
    destination.require_same_lock("record", &pending).unwrap();
    assert_eq!(FileIdentity::of(&pending).unwrap(), id);
    let unrelated = create(&source, "pending", b"unrelated");
    let mut callbacks = 0;
    assert!(
        destination
            .publish_private_journal_from_observed(
                (&source, "pending"),
                "record",
                &pending,
                Some(&old),
                &mut published,
                |_| {
                    callbacks += 1;
                    Ok(())
                }
            )
            .is_err()
    );
    assert_eq!(callbacks, 0);
    source.require_same_lock("pending", &unrelated).unwrap();
    destination.require_same_lock("record", &pending).unwrap();
    pending.sync_all().unwrap();
    source.sync().unwrap();
    destination.sync().unwrap();
    assert_eq!(
        fs::read(destination.path().join("record")).unwrap(),
        b"complete"
    );
}

#[test]
fn cross_parent_each_parent_custody_failure_preserves_the_first_error() {
    for source_side in [false, true] {
        for initiating_error in [false, true] {
            let (_temporary, source, destination) = roots();
            let pending = create(&source, "pending", b"complete");
            let selected = if source_side { &source } else { &destination };
            let mut published = false;
            let error = destination
                .publish_private_journal_from_observed(
                    (&source, "pending"),
                    "record",
                    &pending,
                    None,
                    &mut published,
                    |step| {
                        assert_eq!(step, AtomicWriteStep::Publish);
                        grant(selected.path(), "*S-1-1-0:(W)");
                        if initiating_error {
                            Err(io::Error::other("first parent observation"))
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
            if initiating_error {
                assert_eq!(error.to_string(), "first parent observation");
            }
            assert!(!published);
            assert!(selected.revalidate().is_err());
            assert!(!destination.path().join("record").exists());
            assert_eq!(
                fs::read(source.path().join("pending")).unwrap(),
                b"complete"
            );
        }
    }
}

#[test]
fn cross_parent_private_source_drift_and_postpublication_unwind_keep_authority() {
    let (_temporary, source, destination) = roots();
    let pending = create(&source, "pending", b"complete");
    let mut published = false;
    assert!(
        destination
            .publish_private_journal_from_observed(
                (&source, "pending"),
                "record",
                &pending,
                None,
                &mut published,
                |step| {
                    assert_eq!(step, AtomicWriteStep::Publish);
                    grant(&source.path().join("pending"), "*S-1-1-0:(R)");
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!published);
    assert!(FileSnapshot::private_journal(&pending).is_err());
    assert!(!destination.path().join("record").exists());
    let next = create(&source, "next", b"next complete");
    let id = FileIdentity::of(&next).unwrap();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = destination.publish_private_journal_from_observed(
            (&source, "next"),
            "record",
            &next,
            None,
            &mut published,
            |step| {
                assert_ne!(
                    step,
                    AtomicWriteStep::Published,
                    "actual postpublication unwind"
                );
                Ok(())
            },
        );
    }));
    assert!(result.is_err());
    assert!(published);
    assert_eq!(FileIdentity::of(&next).unwrap(), id);
    destination.require_same_lock("record", &next).unwrap();
    source.revalidate().unwrap();
    destination.revalidate().unwrap();
}
