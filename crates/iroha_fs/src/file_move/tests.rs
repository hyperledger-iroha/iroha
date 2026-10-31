//! Original identity, interrupted namespace publication and adversarial move controls.

use super::*;
use std::{fs, io::Write as _};

#[test]
fn private_snapshot_cleanup_preserves_a_same_byte_substitute() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let mut first = directory.create_retained_private("first", 16).unwrap();
    first.write_all(b"same bytes").unwrap();
    let mut first = first.finish_private().unwrap();
    let expected = first.snapshot().unwrap();
    first
        .rename_private_no_replace_observed("backup", |_| Ok(()))
        .unwrap();
    let mut second = directory.create_retained_private("first", 16).unwrap();
    second.write_all(b"same bytes").unwrap();
    let second = second.finish_private().unwrap();
    let substitute = second.snapshot().unwrap();
    drop(second);
    assert!(
        directory
            .remove_private_if_snapshot("first", &expected)
            .is_err()
    );
    assert_eq!(
        directory
            .open_movable_private("first")
            .unwrap()
            .snapshot()
            .unwrap(),
        substitute
    );
    assert!(
        directory
            .remove_private_if_snapshot("first", &substitute)
            .unwrap()
    );
    let backup = first.snapshot().unwrap();
    drop(first);
    assert!(
        directory
            .remove_private_if_snapshot("backup", &backup)
            .unwrap()
    );
}

#[test]
fn private_rename_errors_retain_original_for_exact_rollback() {
    for failure in [
        FileMoveStep::Rename,
        FileMoveStep::Published,
        FileMoveStep::SyncDestination,
        FileMoveStep::SyncSource,
        FileMoveStep::Validate,
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let owner = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let mut writer = owner.create_retained_private("original", 32).unwrap();
        writer.write_all(b"original bytes").unwrap();
        let mut original = writer.finish_private().unwrap();
        let identity = original.identity().unwrap();
        let mut published = false;
        assert!(
            original
                .rename_private_no_replace_observed("backup", |step| {
                    published |= step == FileMoveStep::Published;
                    if step == failure {
                        Err(io::Error::other("injected interruption"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        assert_eq!(original.identity().unwrap(), identity);
        if published {
            original
                .rename_private_no_replace_observed("original", |_| Ok(()))
                .unwrap();
        }
        original.rewind().unwrap();
        let mut bytes = Vec::new();
        original.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original bytes");
        assert_eq!(original.identity().unwrap(), identity);
        drop(original);
        let recovered = owner.open_movable_private("original").unwrap();
        assert_eq!(recovered.identity().unwrap(), identity);
        assert!(!owner.path().join("backup").exists());
    }
}

#[test]
fn private_rename_refuses_occupied_names_and_generic_admission() {
    let temporary = tempfile::tempdir().unwrap();
    let private = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    for name in ["old", "occupied"] {
        private
            .create_retained_private(name, 1)
            .unwrap()
            .finish_private()
            .unwrap();
    }
    let mut old = private.open_movable_private("old").unwrap();
    let identity = old.identity().unwrap();
    assert!(
        old.rename_private_no_replace_observed("occupied", |_| Ok(()))
            .is_err()
    );
    assert_eq!(old.identity().unwrap(), identity);
    drop(old);
    let owner = OwnerDirectory::open(private.path()).unwrap();
    let mut generic = owner.open_movable_regular("old").unwrap();
    assert!(
        generic
            .rename_private_no_replace_observed("backup", |_| Ok(()))
            .is_err()
    );
    assert_eq!(generic.identity().unwrap(), identity);
}

fn fixture() -> (tempfile::TempDir, OwnerDirectory, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let source = OwnerDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let destination = OwnerDirectory::open_or_create(temporary.path().join("backup")).unwrap();
    source
        .write_atomic("original", b"original bytes", PublishMode::CreateNew)
        .unwrap();
    (temporary, source, destination)
}

#[test]
fn original_identity_snapshots_bytes_and_both_namespaces_survive_move() {
    let (_temporary, source, destination) = fixture();
    let mut original = source.open_movable_regular("original").unwrap();
    let identity = original.identity().unwrap();
    let snapshot = original.snapshot().unwrap();
    let mut bytes = [0; 8];
    original.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"original");
    original.rewind().unwrap();
    let mut steps = Vec::new();
    let mut moved = original
        .move_no_replace_observed(&destination, "saved", |step| {
            steps.push(step);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        steps,
        [
            FileMoveStep::Rename,
            FileMoveStep::Published,
            FileMoveStep::SyncDestination,
            FileMoveStep::SyncSource,
            FileMoveStep::Validate
        ]
    );
    assert_eq!(moved.identity().unwrap(), identity);
    assert_eq!(moved.original_snapshot(), snapshot);
    let mut expected = snapshot.local_parts();
    let actual = moved.snapshot().unwrap().local_parts();
    expected.5 = actual.5;
    assert_eq!(expected, actual);
    moved.require_source_absent().unwrap();
    moved.rewind().unwrap();
    let mut bytes = Vec::new();
    moved.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original bytes");
    assert!(!source.path().join("original").exists());
    assert_eq!(fs::read(destination.path().join("saved")).unwrap(), bytes);
}

#[test]
fn existing_and_racing_destinations_are_never_replaced_or_removed() {
    for raced in [false, true] {
        let (_temporary, source, destination) = fixture();
        if !raced {
            destination
                .write_atomic("saved", b"other", PublishMode::CreateNew)
                .unwrap();
        }
        let original = source.open_movable_regular("original").unwrap();
        let identity = original.identity().unwrap();
        let result = original.move_no_replace_observed(&destination, "saved", |step| {
            assert!(raced);
            assert_eq!(step, FileMoveStep::Rename);
            destination.write_atomic("saved", b"other", PublishMode::CreateNew)
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read(destination.path().join("saved")).unwrap(),
            b"other"
        );
        let original = source.open_movable_regular("original").unwrap();
        assert_eq!(original.identity().unwrap(), identity);
        assert_eq!(
            fs::read(source.path().join("original")).unwrap(),
            b"original bytes"
        );
    }
}

#[test]
fn interrupted_operations_preserve_original_or_moved_object_without_cleanup() {
    for interrupted in [
        FileMoveStep::Rename,
        FileMoveStep::Published,
        FileMoveStep::SyncDestination,
        FileMoveStep::SyncSource,
        FileMoveStep::Validate,
    ] {
        let (_temporary, source, destination) = fixture();
        let original = source.open_movable_regular("original").unwrap();
        let identity = original.identity().unwrap();
        let mut publication_seen = false;
        let result = original.move_no_replace_observed(&destination, "saved", |step| {
            if step == FileMoveStep::Published {
                publication_seen = true;
            }
            if step == interrupted {
                Err(io::Error::other("injected interruption"))
            } else {
                Ok(())
            }
        });
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("injected interruption")
        );
        assert_eq!(publication_seen, interrupted != FileMoveStep::Rename);
        let (parent, name, absent) = if interrupted == FileMoveStep::Rename {
            (&source, "original", destination.path().join("saved"))
        } else {
            (&destination, "saved", source.path().join("original"))
        };
        assert!(!absent.exists());
        let recovered = parent.open_movable_regular(name).unwrap();
        assert_eq!(recovered.identity().unwrap(), identity);
        assert_eq!(
            fs::read(parent.path().join(name)).unwrap(),
            b"original bytes"
        );
    }
}

#[test]
fn recreated_source_is_detected_without_invalidating_retained_backup() {
    let (_temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    let identity = original.identity().unwrap();
    let moved = original.move_no_replace(&destination, "saved").unwrap();
    source
        .write_atomic("original", b"new bytes", PublishMode::CreateNew)
        .unwrap();
    assert!(moved.require_source_absent().is_err());
    moved.revalidate().unwrap();
    assert_eq!(moved.identity().unwrap(), identity);
    assert_eq!(
        fs::read(source.path().join("original")).unwrap(),
        b"new bytes"
    );
    assert_eq!(
        fs::read(destination.path().join("saved")).unwrap(),
        b"original bytes"
    );
}

#[test]
fn source_recreation_during_publication_fails_without_removing_either_object() {
    let (_temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    let result = original.move_no_replace_observed(&destination, "saved", |step| {
        if step == FileMoveStep::SyncSource {
            source.write_atomic("original", b"racer", PublishMode::CreateNew)?;
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(fs::read(source.path().join("original")).unwrap(), b"racer");
    assert_eq!(
        fs::read(destination.path().join("saved")).unwrap(),
        b"original bytes"
    );
}

#[test]
fn sealed_creator_moves_by_original_handle_and_recovered_file_cannot_upgrade() {
    let (_temporary, source, destination) = fixture();
    let mut pending = destination.create_retained_private("stage", 9).unwrap();
    pending.write_all(b"new bytes").unwrap();
    let sealed = pending.seal_read_only().unwrap();
    let identity = sealed.identity().unwrap();
    let fresh = sealed.into_movable().unwrap();
    let old = source
        .open_movable_regular("original")
        .unwrap()
        .move_no_replace(&destination, "saved")
        .unwrap();
    let mut installed = fresh.move_no_replace(&source, "original").unwrap();
    installed.require_source_absent().unwrap();
    old.revalidate().unwrap();
    assert!(old.require_source_absent().is_err());
    assert_eq!(installed.identity().unwrap(), identity);
    installed.rewind().unwrap();
    let mut bytes = Vec::new();
    installed.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"new bytes");
    drop(installed);
    let private = PrivateDirectory::open(source.path()).unwrap();
    let recovered = private.open_retained_read_only("original", 9).unwrap();
    assert!(recovered.into_movable().is_err());
    assert_eq!(
        fs::read(source.path().join("original")).unwrap(),
        b"new bytes"
    );
}

#[test]
fn private_creator_finish_preserves_policy_identity_bytes_and_original_move_authority() {
    let (_temporary, source, destination) = fixture();
    let mut pending = source.create_retained_private("stage", 9).unwrap();
    pending.write_all(b"new bytes").unwrap();
    assert!(pending.write_all(b"too much").is_err());
    let identity = FileMetadata::symlink_metadata(source.path().join("stage"))
        .unwrap()
        .identity();
    let mut finished = pending.finish_private().unwrap();
    assert_eq!(finished.identity().unwrap(), identity);
    finished.rewind().unwrap();
    let mut bytes = Vec::new();
    finished.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"new bytes");
    let moved = finished.move_no_replace(&destination, "private").unwrap();
    assert_eq!(moved.identity().unwrap(), identity);
    moved.require_source_absent().unwrap();
    drop(moved);
    let private = PrivateDirectory::open(destination.path()).unwrap();
    assert!(private.open_retained_read_only("private", 9).is_err());
    assert!(
        ReaderDirectory::open(destination.path())
            .unwrap()
            .open_sealed_executable("private", 9)
            .is_err()
    );
    assert_eq!(private.read("private", 9).unwrap().as_slice(), b"new bytes");
    // The opaque move owner consumed write authority without sealing the file's policy.
    fs::write(destination.path().join("private"), b"rewritten").unwrap();
    assert_eq!(private.read("private", 9).unwrap().as_slice(), b"rewritten");
}

#[test]
fn private_creator_finish_refuses_lost_original_or_native_sharing_prevents_loss() {
    let (_temporary, source, _destination) = fixture();
    let mut pending = source.create_retained_private("stage", 8).unwrap();
    pending.write_all(b"original").unwrap();
    let moved = fs::rename(source.path().join("stage"), source.path().join("displaced"));
    if moved.is_ok() {
        source
            .write_atomic("stage", b"replaced", PublishMode::CreateNew)
            .unwrap();
        assert!(pending.finish_private().is_err());
        assert_eq!(fs::read(source.path().join("stage")).unwrap(), b"replaced");
        assert_eq!(
            fs::read(source.path().join("displaced")).unwrap(),
            b"original"
        );
    } else {
        pending.finish_private().unwrap().revalidate().unwrap();
        assert_eq!(fs::read(source.path().join("stage")).unwrap(), b"original");
    }
    let private = PrivateDirectory::open(source.path()).unwrap();
    let recovered = private.open_retained_private("original").unwrap();
    assert!(recovered.inner.finish_private_for_move().is_err());
}

#[cfg(unix)]
#[test]
fn private_creator_finish_rejects_extent_growth_without_cleanup() {
    let (_temporary, source, _destination) = fixture();
    let mut pending = source.create_retained_private("stage", 8).unwrap();
    pending.write_all(b"original").unwrap();
    fs::write(source.path().join("stage"), b"over original bound").unwrap();
    assert!(pending.finish_private().is_err());
    assert_eq!(
        fs::read(source.path().join("stage")).unwrap(),
        b"over original bound"
    );
}

#[test]
fn published_observer_precedes_native_postrename_custody_checks() {
    let (_temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    let identity = original.identity().unwrap();
    let mut observed = Vec::new();
    let result = original.move_no_replace_observed(&destination, "saved", |step| {
        observed.push(step);
        if step == FileMoveStep::Published {
            source.write_atomic("original", b"replacement", PublishMode::CreateNew)?;
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(observed, [FileMoveStep::Rename, FileMoveStep::Published]);
    assert_eq!(
        fs::read(source.path().join("original")).unwrap(),
        b"replacement"
    );
    let original = destination.open_movable_regular("saved").unwrap();
    assert_eq!(original.identity().unwrap(), identity);
    assert_eq!(
        fs::read(destination.path().join("saved")).unwrap(),
        b"original bytes"
    );
}

#[test]
fn invalid_names_and_shared_source_links_fail_without_mutation() {
    let (_temporary, source, destination) = fixture();
    for name in ["../original", "alternate:stream", "CON", ""] {
        assert!(source.open_movable_regular(name).is_err());
        assert!(
            source
                .open_movable_regular("original")
                .unwrap()
                .move_no_replace(&destination, name)
                .is_err()
        );
    }
    fs::hard_link(source.path().join("original"), source.path().join("alias")).unwrap();
    assert!(source.open_movable_regular("original").is_err());
    assert_eq!(
        fs::read(source.path().join("original")).unwrap(),
        b"original bytes"
    );
    assert!(destination.entries(10).unwrap().is_empty());
}

#[test]
fn changed_source_bytes_are_refused_or_native_sharing_prevents_change() {
    let (_temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    let mut changed = false;
    let result = original.move_no_replace_observed(&destination, "saved", |step| {
        if step == FileMoveStep::Rename {
            changed = fs::write(source.path().join("original"), b"changed bytes!").is_ok();
        }
        Ok(())
    });
    if changed {
        assert!(result.is_err());
        assert!(!destination.path().join("saved").exists());
        assert_eq!(
            fs::read(source.path().join("original")).unwrap(),
            b"changed bytes!"
        );
    } else {
        let moved = result.unwrap();
        moved.require_source_absent().unwrap();
        assert_eq!(
            fs::read(destination.path().join("saved")).unwrap(),
            b"original bytes"
        );
    }
}

#[test]
fn retained_move_outlives_directory_owners_and_checks_later_mutation() {
    let (_temporary, source, destination) = fixture();
    let destination_path = destination.path().join("saved");
    let mut moved = source
        .open_movable_regular("original")
        .unwrap()
        .move_no_replace(&destination, "saved")
        .unwrap();
    drop(source);
    drop(destination);
    moved.require_source_absent().unwrap();
    if fs::write(&destination_path, b"changed bytes!").is_ok() {
        assert!(moved.revalidate().is_err());
        assert!(moved.snapshot().is_err());
        assert!(moved.identity().is_err());
        assert!(moved.read(&mut [0; 1]).is_err());
        assert!(moved.rewind().is_err());
    } else {
        moved.revalidate().unwrap();
        assert_eq!(fs::read(destination_path).unwrap(), b"original bytes");
    }
}

#[cfg(unix)]
#[test]
fn indirect_source_and_destination_are_never_followed() {
    use std::os::unix::fs::symlink;

    let (_temporary, source, destination) = fixture();
    symlink("original", source.path().join("alias")).unwrap();
    assert!(source.open_movable_regular("alias").is_err());
    symlink(
        source.path().join("original"),
        destination.path().join("saved"),
    )
    .unwrap();
    assert!(
        source
            .open_movable_regular("original")
            .unwrap()
            .move_no_replace(&destination, "saved")
            .is_err()
    );
    assert!(
        fs::symlink_metadata(destination.path().join("saved"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        fs::read(source.path().join("original")).unwrap(),
        b"original bytes"
    );
}

#[cfg(unix)]
#[test]
fn changed_source_identity_and_parent_are_refused_before_move() {
    for replace_parent in [false, true] {
        let (temporary, source, destination) = fixture();
        let original = source.open_movable_regular("original").unwrap();
        let result = original.move_no_replace_observed(&destination, "saved", |step| {
            assert_eq!(step, FileMoveStep::Rename);
            if replace_parent {
                fs::rename(source.path(), temporary.path().join("displaced"))?;
                fs::create_dir(source.path())?;
            } else {
                fs::rename(
                    source.path().join("original"),
                    source.path().join("displaced"),
                )?;
            }
            fs::write(source.path().join("original"), b"replacement")
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read(source.path().join("original")).unwrap(),
            b"replacement"
        );
        assert!(destination.entries(10).unwrap().is_empty());
    }
}

#[cfg(unix)]
#[test]
fn changed_destination_parent_after_move_preserves_displaced_original() {
    let (temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    let displaced = temporary.path().join("displaced");
    let result = original.move_no_replace_observed(&destination, "saved", |step| {
        if step == FileMoveStep::SyncDestination {
            fs::rename(destination.path(), &displaced)?;
            fs::create_dir(destination.path())?;
            fs::write(destination.path().join("saved"), b"replacement")?;
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(displaced.join("saved")).unwrap(),
        b"original bytes"
    );
    assert_eq!(
        fs::read(destination.path().join("saved")).unwrap(),
        b"replacement"
    );
    assert!(!source.path().join("original").exists());
}

#[cfg(windows)]
#[test]
fn native_source_and_both_parent_names_remain_pinned() {
    let (temporary, source, destination) = fixture();
    let original = source.open_movable_regular("original").unwrap();
    assert!(
        fs::rename(
            source.path().join("original"),
            source.path().join("displaced")
        )
        .is_err()
    );
    assert!(fs::rename(source.path(), temporary.path().join("displaced-source")).is_err());
    assert!(
        fs::rename(
            destination.path(),
            temporary.path().join("displaced-backup")
        )
        .is_err()
    );
    let moved = original.move_no_replace(&destination, "saved").unwrap();
    assert!(
        fs::rename(
            destination.path().join("saved"),
            destination.path().join("displaced")
        )
        .is_err()
    );
    moved.require_source_absent().unwrap();
}
