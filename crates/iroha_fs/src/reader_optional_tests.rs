//! Read-only optional admission keeps first absence separate from lost original custody.

use super::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, ReaderDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("parent")).unwrap();
    parent.create_child("child").unwrap();
    parent
        .write_atomic("record", b"original", PublishMode::CreateNew)
        .unwrap();
    let reader = ReaderDirectory::open(parent.path()).unwrap();
    (temporary, reader)
}

#[test]
fn optional_reader_preserves_absence_bounds_and_private_original_bytes() {
    let (_temporary, reader) = fixture();
    assert!(reader.open_child_optional("missing").unwrap().is_none());
    assert!(
        reader
            .read_private_optional("missing", 0)
            .unwrap()
            .is_none()
    );
    assert!(reader.open_child_optional("../missing").is_err());
    assert!(reader.read_private_optional("../missing", 0).is_err());
    assert_eq!(
        reader.open_child("missing").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    let child = reader.open_child_optional("child").unwrap().unwrap();
    assert_eq!(child.path(), reader.path().join("child"));
    assert_eq!(
        child.identity().unwrap(),
        OwnerDirectory::open(child.path())
            .unwrap()
            .identity()
            .unwrap()
    );
    assert_ne!(child.identity().unwrap(), reader.identity().unwrap());
    child.revalidate().unwrap();
    assert!(reader.open_child_optional("record").is_err());
    assert!(reader.read_private_optional("record", 7).is_err());
    assert_eq!(
        reader
            .read_private_optional("record", 8)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"original"
    );
    fs::hard_link(reader.path().join("record"), reader.path().join("alias")).unwrap();
    assert!(reader.read_private_optional("record", 8).is_err());
    assert_eq!(fs::read(reader.path().join("alias")).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn optional_reader_needs_no_directory_write_authority_and_rejects_public_leaf() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_temporary, reader) = fixture();
    let path = reader.path().to_owned();
    drop(reader);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
    let reader = ReaderDirectory::open(&path).unwrap();
    assert!(reader.open_child_optional("child").unwrap().is_some());
    assert_eq!(
        reader
            .read_private_optional("record", 8)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"original"
    );
    fs::set_permissions(path.join("record"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(reader.read_private_optional("record", 8).is_err());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o555
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(unix)]
#[test]
fn optional_reader_keeps_post_admission_child_and_leaf_disappearance_as_errors() {
    for child in [false, true] {
        let (_temporary, reader) = fixture();
        let name = if child { "child" } else { "record" };
        let path = reader.path().join(name);
        let saved = reader.path().join("saved");
        let source = path.clone();
        let destination = saved.clone();
        let mutation = move || fs::rename(source, destination).unwrap();
        let result = if child {
            platform::with_child_named_hook(name, mutation, || {
                reader.open_child_optional(name).map(drop)
            })
        } else {
            platform::with_readonly_named_hook(name, mutation, || {
                reader.read_private_optional(name, 8).map(drop)
            })
        };
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
        fs::rename(saved, path).unwrap();
        assert!(reader.open_child_optional("child").unwrap().is_some());
        assert_eq!(
            reader
                .read_private_optional("record", 8)
                .unwrap()
                .unwrap()
                .as_slice(),
            b"original"
        );
    }
}

#[cfg(unix)]
#[test]
fn optional_reader_parent_exit_overrides_initial_child_absence() {
    let (temporary, reader) = fixture();
    let source = reader.path().to_owned();
    let destination = temporary.path().join("displaced");
    let result = platform::with_child_named_hook(
        "missing",
        move || fs::rename(source, destination).unwrap(),
        || reader.open_child_optional("missing"),
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(reader.read_private_optional("missing", 0).is_err());
}

#[cfg(windows)]
#[test]
fn optional_reader_native_parent_pins_and_writer_exclusion_remain_required() {
    let (temporary, reader) = fixture();
    let child = reader.open_child_optional("child").unwrap().unwrap();
    assert!(fs::rename(child.path(), reader.path().join("displaced-child")).is_err());
    assert!(fs::rename(reader.path(), temporary.path().join("displaced")).is_err());
    let path = reader.path().join("record");
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(reader.read_private_optional("record", 8).is_err());
    drop(writer);
    assert_eq!(
        reader
            .read_private_optional("record", 8)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"original"
    );
}

#[cfg(windows)]
#[test]
fn optional_reader_native_private_leaf_refuses_foreign_read_access_without_repair() {
    let (_temporary, reader) = fixture();
    let path = reader.path().join("record");
    assert!(
        std::process::Command::new("icacls.exe")
            .arg(&path)
            .args(["/grant", "*S-1-1-0:(R)"])
            .status()
            .unwrap()
            .success()
    );
    let acl = || {
        std::process::Command::new("icacls.exe")
            .arg(&path)
            .output()
            .unwrap()
    };
    let before = acl();
    assert!(before.status.success());
    assert!(reader.read_private_optional("record", 8).is_err());
    let after = acl();
    assert!(after.status.success());
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(fs::read(path).unwrap(), b"original");
}

#[test]
fn optional_regular_original_reader_preserves_initial_absence_identity_and_links() {
    use std::io::Read as _;
    let (_temporary, reader) = fixture();
    assert!(
        reader
            .open_retained_regular_optional("missing")
            .unwrap()
            .is_none()
    );
    assert!(reader.open_retained_regular_optional("../missing").is_err());
    assert!(reader.open_retained_regular_optional("child").is_err());
    let mut original = reader
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    let before = original.snapshot().unwrap();
    let mut bytes = Vec::new();
    original.file_mut().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    assert_eq!(original.snapshot().unwrap(), before);
    original.revalidate().unwrap();
    drop(original);
    fs::hard_link(reader.path().join("record"), reader.path().join("alias")).unwrap();
    assert!(reader.open_retained_regular_optional("record").is_err());
    assert_eq!(fs::read(reader.path().join("alias")).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn optional_regular_reader_needs_no_writable_parent_and_refuses_late_disappearance() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_temporary, reader) = fixture();
    fs::set_permissions(reader.path(), fs::Permissions::from_mode(0o555)).unwrap();
    reader
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap()
        .revalidate()
        .unwrap();
    fs::set_permissions(reader.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let source = reader.path().join("record");
    let destination = reader.path().join("saved");
    let result = platform::with_readonly_named_hook(
        "record",
        move || fs::rename(source, destination).unwrap(),
        || reader.open_retained_regular_optional("record"),
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[cfg(windows)]
#[test]
fn optional_regular_reader_keeps_native_writer_and_delete_exclusion() {
    let (_temporary, reader) = fixture();
    let path = reader.path().join("record");
    let original = reader
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    original.revalidate().unwrap();
}
