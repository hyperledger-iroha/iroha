//! Native immutable publication, exact pending-original recovery and retention controls.

use super::*;
use std::fs;

const SUFFIX: &str = ".archive.to";
fn archive(maximum: u64) -> (tempfile::TempDir, NativeArchive) {
    let temporary = tempfile::tempdir().unwrap();
    let archive = NativeArchive::open(
        &temporary.path().join("archive"),
        &[("records", SUFFIX)],
        maximum,
        64,
    )
    .unwrap();
    (temporary, archive)
}
fn target(archive: &NativeArchive, byte: char) -> std::path::PathBuf {
    archive.directories[0]
        .owner
        .path()
        .join(format!("{}{SUFFIX}", byte.to_string().repeat(64)))
}
fn publish(archive: &NativeArchive, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = &archive.directories[0].owner;
    archive.publish(directory.path(), directory.identity()?, path, bytes)
}
fn stage(directory: &Directory, index: u64, bytes: &[u8]) -> OsString {
    let name = OsString::from(format!(".staged-00000001-{index:016x}"));
    let mut slot = None;
    directory
        .owner
        .create_private_journal_into(&name, &mut slot)
        .unwrap();
    let mut file = slot.unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    name
}

#[test]
fn immutable_native_publication_replay_and_conflict_keep_full_identity() {
    let (_temporary, archive) = archive(1024);
    let path = target(&archive, 'a');
    publish(&archive, &path, b"canonical bytes").unwrap();
    let original = RetainedFile::open_private(&path).unwrap();
    let identity = original.identity().unwrap();
    assert_eq!(read(&path, 1024).unwrap(), b"canonical bytes");
    publish(&archive, &path, b"canonical bytes").unwrap();
    assert!(publish(&archive, &path, b"conflict").is_err());
    assert_eq!(
        FileMetadata::symlink_metadata(&path).unwrap().identity(),
        identity
    );
    assert_eq!(read(&path, 1024).unwrap(), b"canonical bytes");
    assert_eq!(
        archive.directories[0].owner.entries(8).unwrap(),
        vec![path.file_name().unwrap().to_owned()]
    );
}

#[test]
fn unpublished_error_keeps_original_then_retires_only_its_stage() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'b');
    let error = archive
        .publish_observed(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            &path,
            b"complete",
            |step| {
                if step == AtomicWriteStep::Publish {
                    Err(io::Error::other("before publication"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "before publication");
    let (name, identity) = {
        let pending = directory.pending.lock().unwrap();
        let phase = pending.as_ref().unwrap();
        assert!(!phase.published);
        let original = phase.original.as_ref().unwrap();
        assert_eq!(read_original(original, 1024).unwrap(), b"complete");
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(directory.owner.path().join(&phase.name))
                .is_err()
        );
        (
            phase.name.clone(),
            FileMetadata::of(original).unwrap().identity(),
        )
    };
    assert_eq!(
        FileMetadata::symlink_metadata(directory.owner.path().join(&name))
            .unwrap()
            .identity(),
        identity
    );
    assert!(!path.exists());
    archive.revalidate().unwrap();
    assert!(!directory.owner.path().join(name).exists());
    assert!(directory.pending.lock().unwrap().is_none());
    publish(&archive, &path, b"complete").unwrap();
}

#[test]
fn postpublication_error_keeps_published_original_and_first_error() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'c');
    let error = archive
        .publish_observed(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            &path,
            b"published",
            |step| {
                if step == AtomicWriteStep::Published {
                    let mut permissions = fs::metadata(&path)?.permissions();
                    permissions.set_readonly(true);
                    fs::set_permissions(&path, permissions)?;
                    Err(io::Error::other("first after publication"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "first after publication");
    let identity = {
        let pending = directory.pending.lock().unwrap();
        let phase = pending.as_ref().unwrap();
        assert!(phase.published);
        let original = phase.original.as_ref().unwrap();
        let identity = FileMetadata::of(original).unwrap().identity();
        let mut permissions = original.metadata().unwrap().permissions();
        permissions.set_readonly(false);
        original.set_permissions(permissions).unwrap();
        identity
    };
    assert!(RetainedFile::open_private(&path).is_err());
    archive.revalidate().unwrap();
    assert_eq!(read(&path, 1024).unwrap(), b"published");
    assert_eq!(
        FileMetadata::symlink_metadata(&path).unwrap().identity(),
        identity
    );
    publish(&archive, &path, b"published").unwrap();
}

#[test]
fn no_replace_race_preserves_competing_target_and_pending_stage() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'd');
    let result = archive.publish_observed(
        directory.owner.path(),
        directory.owner.identity().unwrap(),
        &path,
        b"proposal",
        |step| {
            if step == AtomicWriteStep::Publish {
                let mut competitor = RetainedFile::create_new_private(&path)?;
                competitor.file_mut().write_all(b"competitor")?;
                competitor.file().sync_all()?;
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(
        !directory
            .pending
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .published
    );
    archive.revalidate().unwrap();
    assert_eq!(read(&path, 1024).unwrap(), b"competitor");
    assert!(publish(&archive, &path, b"proposal").is_err());
}

#[test]
fn native_staged_recovery_handles_more_than_sixteen_and_keeps_canonical() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'e');
    publish(&archive, &path, b"canonical").unwrap();
    let identity = FileMetadata::symlink_metadata(&path).unwrap().identity();
    for index in 0..20 {
        stage(directory, index, b"abandoned complete stage");
    }
    archive
        .recover(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            1024,
            SUFFIX,
        )
        .unwrap();
    assert_eq!(
        directory.owner.entries(64).unwrap(),
        vec![path.file_name().unwrap().to_owned()]
    );
    assert_eq!(read(&path, 1024).unwrap(), b"canonical");
    assert_eq!(
        FileMetadata::symlink_metadata(path).unwrap().identity(),
        identity
    );
}

#[test]
fn native_recovery_refuses_foreign_names_hardlinks_and_oversized_originals() {
    for kind in ["foreign", "hardlink", "oversized"] {
        let (_temporary, archive) = archive(8);
        let directory = &archive.directories[0];
        let name = if kind == "foreign" {
            let name = OsString::from(".staged-not-ours");
            let mut slot = None;
            directory
                .owner
                .create_private_journal_into(&name, &mut slot)
                .unwrap();
            drop(slot);
            name
        } else {
            stage(
                directory,
                1,
                if kind == "oversized" {
                    b"123456789"
                } else {
                    b"original"
                },
            )
        };
        let path = directory.owner.path().join(&name);
        if kind == "hardlink" {
            fs::hard_link(&path, directory.owner.path().join("outside-alias")).unwrap();
        }
        let identity = FileMetadata::symlink_metadata(&path).unwrap().identity();
        assert!(
            archive
                .recover(
                    directory.owner.path(),
                    directory.owner.identity().unwrap(),
                    8,
                    SUFFIX
                )
                .is_err()
        );
        assert_eq!(
            FileMetadata::symlink_metadata(&path).unwrap().identity(),
            identity
        );
        assert!(path.exists());
    }
}

#[test]
fn native_retention_keeps_delete_pending_original_until_retry_acknowledges() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'f');
    publish(&archive, &path, b"obsolete").unwrap();
    let identity = FileMetadata::symlink_metadata(&path).unwrap().identity();
    let error = directory
        .unlink_observed(path.file_name().unwrap(), Some(b"obsolete"), |original| {
            let mut permissions = original.metadata()?.permissions();
            permissions.set_readonly(true);
            original.set_permissions(permissions)?;
            Err(io::Error::other("after disposition"))
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "after disposition");
    {
        let pending = directory.pending.lock().unwrap();
        let phase = pending.as_ref().unwrap();
        assert!(phase.disposed);
        let original = phase.original.as_ref().unwrap();
        assert!(
            directory
                .owner
                .require_retired_private_journal(original)
                .is_err()
        );
        let mut permissions = original.metadata().unwrap().permissions();
        permissions.set_readonly(false);
        original.set_permissions(permissions).unwrap();
        directory
            .owner
            .require_retired_private_journal(original)
            .unwrap();
        assert_eq!(FileMetadata::of(original).unwrap().identity(), identity);
    }
    archive.revalidate().unwrap();
    assert!(!path.exists());
    assert!(directory.pending.lock().unwrap().is_some());
    archive
        .unlink(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            &path,
            b"obsolete",
        )
        .unwrap();
    assert!(directory.pending.lock().unwrap().is_none());
    assert!(!path.exists());
}

#[test]
fn native_retirement_does_not_delete_a_replacement_after_close() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, '1');
    publish(&archive, &path, b"obsolete").unwrap();
    assert!(
        directory
            .unlink_observed(path.file_name().unwrap(), Some(b"obsolete"), |_| Err(
                io::Error::other("pending close")
            ))
            .is_err()
    );
    archive.revalidate().unwrap();
    let mut replacement = RetainedFile::create_new_private(&path).unwrap();
    replacement.file_mut().write_all(b"replacement").unwrap();
    replacement.file().sync_all().unwrap();
    let replacement_id = replacement.identity().unwrap();
    drop(replacement);
    assert!(
        archive
            .unlink(
                directory.owner.path(),
                directory.owner.identity().unwrap(),
                &path,
                b"obsolete"
            )
            .is_err()
    );
    assert_eq!(read(&path, 1024).unwrap(), b"replacement");
    assert_eq!(
        FileMetadata::symlink_metadata(&path).unwrap().identity(),
        replacement_id
    );
}

#[test]
fn native_bounds_private_custody_and_parent_binding_refuse_before_publication() {
    let (temporary, archive) = archive(8);
    let directory = &archive.directories[0];
    let path = target(&archive, '2');
    assert!(publish(&archive, &path, b"123456789").is_err());
    assert!(!path.exists());
    assert!(
        publish(
            &archive,
            &temporary.path().join(path.file_name().unwrap()),
            b"bytes"
        )
        .is_err()
    );
    assert!(fs::rename(directory.owner.path(), temporary.path().join("moved")).is_err());
    fs::write(&path, b"public").unwrap();
    assert!(publish(&archive, &path, b"public").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"public");
}

#[test]
fn native_retention_refuses_same_byte_substitution_and_wrong_expected_bytes() {
    let (temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, '3');
    publish(&archive, &path, b"authenticated").unwrap();
    let identity = FileMetadata::symlink_metadata(&path).unwrap().identity();
    let moved = temporary.path().join("selected-original");
    fs::rename(&path, &moved).unwrap();
    let mut replacement = RetainedFile::create_new_private(&path).unwrap();
    replacement.file_mut().write_all(b"authenticated").unwrap();
    replacement.file().sync_all().unwrap();
    let other_identity = replacement.identity().unwrap();
    assert_ne!(other_identity, identity);
    drop(replacement);
    assert!(
        archive
            .unlink(
                directory.owner.path(),
                directory.owner.identity().unwrap(),
                &path,
                b"authenticated"
            )
            .is_err()
    );
    assert_eq!(
        FileMetadata::symlink_metadata(&path).unwrap().identity(),
        other_identity
    );
    assert_eq!(
        FileMetadata::symlink_metadata(&moved).unwrap().identity(),
        identity
    );
    // The rejected original remains held; no caller silently upgrades its authority.
    assert!(
        directory
            .pending
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .original
            .is_some()
    );

    let (_temporary, other) = archive_fixture_for_retention_bytes();
    let owner = &other.directories[0].owner;
    let other_path = target(&other, '4');
    assert!(
        other
            .unlink(
                owner.path(),
                owner.identity().unwrap(),
                &other_path,
                b"wrong bytes"
            )
            .is_err()
    );
    let pending = other.directories[0].pending.lock().unwrap();
    assert!(!pending.as_ref().unwrap().disposed);
    assert_eq!(
        read_original(pending.as_ref().unwrap().original.as_ref().unwrap(), 1024).unwrap(),
        b"authenticated"
    );
}

fn archive_fixture_for_retention_bytes() -> (tempfile::TempDir, NativeArchive) {
    let (temporary, archive) = archive(1024);
    let path = target(&archive, '4');
    publish(&archive, &path, b"authenticated").unwrap();
    (temporary, archive)
}

#[test]
fn failed_precreation_observation_leaves_no_pending_original_and_can_retry() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, '5');
    let error = archive
        .publish_observed(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            &path,
            b"retry",
            |step| {
                if step == AtomicWriteStep::CreateTemp {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "before native creation",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "before native creation");
    assert!(directory.pending.lock().unwrap().is_none());
    assert!(directory.owner.entries(8).unwrap().is_empty());
    publish(&archive, &path, b"retry").unwrap();
    assert_eq!(read(&path, 1024).unwrap(), b"retry");
}

#[test]
fn missing_selected_original_is_not_recreated_by_publication() {
    let (_temporary, archive) = archive(1024);
    let path = target(&archive, '6');
    publish(&archive, &path, b"selected").unwrap();
    fs::remove_file(&path).unwrap();
    assert!(publish(&archive, &path, b"selected").is_err());
    assert!(!path.exists());
    assert!(archive.directories[0].owner.entries(8).unwrap().is_empty());
    assert!(archive.directories[0].pending.lock().unwrap().is_none());
}

#[test]
fn native_retention_rescan_completion_preserves_repeated_failure_intent() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'a');
    publish(&archive, &path, b"obsolete").unwrap();
    let identity = FileMetadata::symlink_metadata(&path).unwrap().identity();
    for message in ["first post-close barrier", "second post-close barrier"] {
        let error = archive
            .unlink_after_close_observed(
                directory.owner.path(),
                directory.owner.identity().unwrap(),
                &path,
                b"obsolete",
                || Err(io::Error::other(message)),
            )
            .unwrap_err();
        assert_eq!(error.to_string(), message);
        assert!(!path.exists());
        let pending = directory.pending.lock().unwrap();
        let phase = pending.as_ref().unwrap();
        assert!(phase.retention && phase.disposed);
        assert_eq!(phase.identity, Some(identity));
        assert_eq!(phase.expected, b"obsolete");
        assert!(phase.original.is_none());
    }
    let next = target(&archive, 'b');
    assert!(publish(&archive, &next, b"next").is_err());
    assert!(!next.exists());
    assert!(archive.finish_retention().unwrap());
    assert!(directory.pending.lock().unwrap().is_none());
    assert!(!archive.finish_retention().unwrap());
    publish(&archive, &next, b"next").unwrap();
    assert_eq!(read(&next, 1024).unwrap(), b"next");
}

#[test]
fn native_retention_rescan_completion_refuses_replacement_without_deleting_it() {
    let (_temporary, archive) = archive(1024);
    let directory = &archive.directories[0];
    let path = target(&archive, 'c');
    publish(&archive, &path, b"obsolete").unwrap();
    archive
        .unlink_after_close_observed(
            directory.owner.path(),
            directory.owner.identity().unwrap(),
            &path,
            b"obsolete",
            || Err(io::Error::other("post-close failure")),
        )
        .unwrap_err();
    let mut replacement = RetainedFile::create_new_private(&path).unwrap();
    replacement.file_mut().write_all(b"replacement").unwrap();
    replacement.file().sync_all().unwrap();
    let identity = replacement.identity().unwrap();
    drop(replacement);
    for _ in 0..2 {
        assert!(archive.finish_retention().is_err());
        assert!(directory.pending.lock().unwrap().is_some());
        assert_eq!(read(&path, 1024).unwrap(), b"replacement");
        assert_eq!(
            FileMetadata::symlink_metadata(&path).unwrap().identity(),
            identity
        );
    }
    let next = target(&archive, 'd');
    assert!(publish(&archive, &next, b"next").is_err());
    assert!(!next.exists());
}
