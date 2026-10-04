//! Borrowed authority, bounded I/O and immutable publication regressions on Unix.

use super::*;
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Seek as _, SeekFrom, Write as _},
    os::unix::fs::{PermissionsExt as _, symlink},
};

fn store() -> (tempfile::TempDir, PrivateDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
    (temporary, directory)
}

#[test]
fn borrowed_create_seal_reopen_preserve_original_object_and_bounds() {
    let (_temporary, directory) = store();
    let name = OsString::from("receipt");
    let mut writer = directory.create_borrowed_private(&name, 13).unwrap();
    let identity = FileIdentity::of(&File::open(directory.path().join(&name)).unwrap()).unwrap();
    writer.write_all(b"exact receipt").unwrap();
    writer.flush().unwrap();
    assert!(directory.create_borrowed_private(&name, 13).is_err());
    assert!(directory.open_borrowed_read_only(&name, 13).is_err());
    let mut sealed = writer.seal_read_only().unwrap();
    assert_eq!(sealed.identity().unwrap(), identity);
    assert_eq!(sealed.len().unwrap(), 13);
    assert!(!sealed.is_empty().unwrap());
    assert_eq!(sealed.stream_position().unwrap(), 13);
    assert_eq!(sealed.seek(SeekFrom::Start(0)).unwrap(), 0);
    let snapshot = sealed.snapshot().unwrap();
    let mut bytes = [0; 13];
    sealed.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"exact receipt");
    assert_eq!(sealed.snapshot().unwrap(), snapshot);
    sealed.revalidate().unwrap();
    assert!(directory.open_borrowed_read_only(&name, 12).is_err());
    let mut reopened = directory.open_borrowed_read_only(&name, 13).unwrap();
    assert_eq!(reopened.identity().unwrap(), identity);
    assert_eq!(reopened.snapshot().unwrap(), snapshot);
    reopened.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"exact receipt");
    assert_eq!(
        fs::metadata(directory.path().join(&name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o400
    );
}

#[test]
fn borrowed_sparse_io_never_crosses_original_extent() {
    let (_temporary, directory) = store();
    let mut writer = directory
        .create_borrowed_private(OsStr::new("bounded"), 8)
        .unwrap();
    assert!(writer.write_all(b"too many bytes").is_err());
    assert_eq!(writer.stream_position().unwrap(), 0);
    assert!(writer.seek(SeekFrom::Start(9)).is_err());
    assert!(writer.seek(SeekFrom::Current(-1)).is_err());
    assert!(writer.seek(SeekFrom::End(i64::MAX)).is_err());
    assert_eq!(writer.seek(SeekFrom::End(7)).unwrap(), 7);
    writer.write_all(b"x").unwrap();
    assert!(writer.write_all(b"y").is_err());
    assert_eq!(writer.stream_position().unwrap(), 8);
    let mut sealed = writer.seal_read_only().unwrap();
    assert_eq!(sealed.len().unwrap(), 8);
    assert_eq!(sealed.seek(SeekFrom::End(-1)).unwrap(), 7);
    let mut bytes = [0; 16];
    assert_eq!(sealed.read(&mut bytes).unwrap(), 1);
    assert_eq!(bytes[0], b'x');
    assert_eq!(sealed.read(&mut bytes).unwrap(), 0);
    assert!(sealed.seek(SeekFrom::Start(9)).is_err());
    assert!(sealed.seek(SeekFrom::Current(i64::MAX)).is_err());
    sealed.revalidate().unwrap();
    let mut zero = directory
        .create_borrowed_private(OsStr::new("zero"), 0)
        .unwrap();
    assert!(zero.write_all(b"x").is_err());
    let mut zero = zero.seal_read_only().unwrap();
    assert!(zero.is_empty().unwrap());
    assert_eq!(zero.read(&mut bytes).unwrap(), 0);
}

#[test]
fn borrowed_publication_moves_only_original_created_file_once_without_replacement() {
    let (_temporary, directory) = store();
    let staged = OsString::from("staged");
    let final_name = OsString::from("receipt");
    let mut writer = directory.create_borrowed_private(&staged, 64).unwrap();
    writer.write_all(b"original").unwrap();
    let sealed = writer.seal_read_only().unwrap();
    let identity = sealed.identity().unwrap();
    let published = sealed.publish_new_name(&final_name).unwrap();
    assert_eq!(published.identity().unwrap(), identity);
    published.revalidate().unwrap();
    assert!(directory.open_borrowed_read_only(&staged, 64).is_err());
    let reopened = directory.open_borrowed_read_only(&final_name, 64).unwrap();
    assert!(reopened.publish_new_name(OsStr::new("moved")).is_err());
    let mut conflict = directory
        .create_borrowed_private(OsStr::new("conflict"), 64)
        .unwrap();
    conflict.write_all(b"replacement").unwrap();
    assert!(
        conflict
            .seal_read_only()
            .unwrap()
            .publish_new_name(&final_name)
            .is_err()
    );
    assert_eq!(
        directory
            .open_borrowed_read_only(&final_name, 64)
            .unwrap()
            .identity()
            .unwrap(),
        identity
    );
    assert_eq!(
        fs::read(directory.path().join(&final_name)).unwrap(),
        b"original"
    );
    assert_eq!(
        fs::read(directory.path().join("conflict")).unwrap(),
        b"replacement"
    );
    assert!(published.publish_new_name(OsStr::new("second")).is_err());
}

#[test]
fn borrowed_interrupted_claims_stay_private_and_are_not_repaired_on_reopen() {
    let (_temporary, directory) = store();
    let name = OsStr::new("incomplete");
    let writer = directory.create_borrowed_private(name, 64).unwrap();
    drop(writer);
    assert!(directory.create_borrowed_private(name, 64).is_err());
    assert!(directory.open_borrowed_read_only(name, 64).is_err());
    let metadata = fs::metadata(directory.path().join(name)).unwrap();
    assert_eq!(metadata.len(), 0);
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
    let mut partial = directory
        .create_borrowed_private(OsStr::new("partial"), 64)
        .unwrap();
    partial.write_all(b"prefix").unwrap();
    drop(partial);
    assert!(
        directory
            .open_borrowed_read_only(OsStr::new("partial"), 64)
            .is_err()
    );
    assert_eq!(
        fs::read(directory.path().join("partial")).unwrap(),
        b"prefix"
    );
}

#[test]
fn borrowed_open_and_publication_reject_invalid_names() {
    let (_temporary, directory) = store();
    for name in ["", ".", "..", "nested/receipt", "/receipt"] {
        let name = OsStr::new(name);
        assert!(directory.create_borrowed_private(name, 64).is_err());
        assert!(directory.open_borrowed_read_only(name, 64).is_err());
    }
    let sealed = directory
        .create_borrowed_private(OsStr::new("staged"), 64)
        .unwrap()
        .seal_read_only()
        .unwrap();
    assert!(sealed.publish_new_name(OsStr::new("../escape")).is_err());
    assert!(
        directory
            .open_borrowed_read_only(OsStr::new("staged"), 64)
            .is_ok()
    );
}

#[test]
fn borrowed_handles_reject_truncation_growth_and_strict_mode_changes() {
    let (_temporary, directory) = store();
    let name = OsStr::new("truncated");
    let mut writer = directory.create_borrowed_private(name, 8).unwrap();
    writer.write_all(b"original").unwrap();
    let hostile = OpenOptions::new()
        .write(true)
        .open(directory.path().join(name))
        .unwrap();
    let sealed = writer.seal_read_only().unwrap();
    hostile.set_len(0).unwrap();
    assert!(sealed.snapshot().is_err());
    assert!(sealed.revalidate().is_err());

    let name = OsStr::new("grown");
    let writer = directory.create_borrowed_private(name, 8).unwrap();
    let hostile = OpenOptions::new()
        .write(true)
        .open(directory.path().join(name))
        .unwrap();
    hostile.set_len(9).unwrap();
    assert!(writer.seal_read_only().is_err());
    assert!(directory.open_borrowed_read_only(name, 8).is_err());
    assert_eq!(fs::metadata(directory.path().join(name)).unwrap().len(), 9);

    let name = OsStr::new("loosened");
    let sealed = directory
        .create_borrowed_private(name, 8)
        .unwrap()
        .seal_read_only()
        .unwrap();
    fs::set_permissions(
        directory.path().join(name),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(sealed.revalidate().is_err());
    assert!(directory.open_borrowed_read_only(name, 8).is_err());
    assert_eq!(
        fs::metadata(directory.path().join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
}

#[test]
fn borrowed_custody_rejects_file_substitution_and_links() {
    let (_temporary, directory) = store();
    let name = OsStr::new("receipt");
    let sealed = directory
        .create_borrowed_private(name, 64)
        .unwrap()
        .seal_read_only()
        .unwrap();
    let replacement = directory
        .create_borrowed_private(OsStr::new("replacement"), 64)
        .unwrap()
        .seal_read_only()
        .unwrap();
    let identity = sealed.identity().unwrap();
    fs::rename(
        directory.path().join(name),
        directory.path().join("displaced"),
    )
    .unwrap();
    fs::rename(
        directory.path().join("replacement"),
        directory.path().join(name),
    )
    .unwrap();
    assert!(sealed.revalidate().is_err());
    assert!(replacement.revalidate().is_err());
    assert_ne!(
        directory
            .open_borrowed_read_only(name, 64)
            .unwrap()
            .identity()
            .unwrap(),
        identity
    );

    let target = directory.path().join(name);
    symlink(name, directory.path().join("symlink")).unwrap();
    assert!(
        directory
            .open_borrowed_read_only(OsStr::new("symlink"), 64)
            .is_err()
    );
    assert!(
        directory
            .create_borrowed_private(OsStr::new("symlink"), 64)
            .is_err()
    );
    fs::hard_link(&target, directory.path().join("hardlink")).unwrap();
    assert!(directory.open_borrowed_read_only(name, 64).is_err());
    assert!(
        directory
            .open_borrowed_read_only(OsStr::new("hardlink"), 64)
            .is_err()
    );
}

#[test]
fn borrowed_authority_rechecks_original_directory_after_namespace_substitution() {
    let (temporary, directory) = store();
    let name = OsStr::new("receipt");
    let sealed = directory
        .create_borrowed_private(name, 64)
        .unwrap()
        .seal_read_only()
        .unwrap();
    fs::rename(directory.path(), temporary.path().join("displaced")).unwrap();
    fs::create_dir(directory.path()).unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(sealed.revalidate().is_err());
    assert!(sealed.snapshot().is_err());
    assert!(directory.open_borrowed_read_only(name, 64).is_err());
    assert!(
        directory
            .create_borrowed_private(OsStr::new("new"), 64)
            .is_err()
    );
}
