//! Original-directory optional regular readers preserve native immutable custody.

use super::*;
use std::fs;

fn store() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("store")).unwrap();
    let owner = OwnerDirectory::open(directory.path()).unwrap();
    owner
        .write_atomic("record", b"original", PublishMode::CreateNew)
        .unwrap();
    (temporary, owner)
}

#[test]
fn optional_regular_original_preserves_absence_bytes_and_single_link_custody() {
    let (_temporary, directory) = store();
    assert!(
        directory
            .open_retained_regular_optional("missing")
            .unwrap()
            .is_none()
    );
    assert!(
        directory
            .open_retained_regular_optional("../missing")
            .is_err()
    );
    let mut original = directory
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    let before = original.snapshot().unwrap();
    let mut bytes = Vec::new();
    original.file_mut().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    assert_eq!(original.snapshot().unwrap(), before);
    fs::hard_link(
        directory.path().join("record"),
        directory.path().join("alias"),
    )
    .unwrap();
    assert!(original.revalidate().is_err());
    assert!(directory.open_retained_regular_optional("record").is_err());
}

#[cfg(unix)]
#[test]
fn optional_regular_original_accepts_public_readers_but_not_private_admission() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_temporary, directory) = store();
    fs::set_permissions(
        directory.path().join("record"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let original = directory
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    original.revalidate().unwrap();
    let private = PrivateDirectory::open(directory.path()).unwrap();
    assert!(private.open_retained_private_optional("record").is_err());
}

#[cfg(unix)]
#[test]
fn optional_regular_original_late_disappearance_never_becomes_absence() {
    let (_temporary, directory) = store();
    let path = directory.path().join("record");
    let result = platform::with_readonly_named_hook(
        "record",
        move || fs::remove_file(path).unwrap(),
        || directory.open_retained_regular_optional("record"),
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(
        directory
            .open_retained_regular_optional("record")
            .unwrap()
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn optional_regular_original_refuses_displaced_parent_even_for_missing_child() {
    let (temporary, directory) = store();
    fs::rename(directory.path(), temporary.path().join("displaced")).unwrap();
    fs::create_dir(directory.path()).unwrap();
    assert!(directory.open_retained_regular_optional("missing").is_err());
}

#[cfg(windows)]
#[test]
fn optional_regular_original_excludes_native_writers_and_namespace_changes() {
    let (temporary, directory) = store();
    let path = directory.path().join("record");
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(directory.open_retained_regular_optional("record").is_err());
    drop(writer);
    let original = directory
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::rename(&path, directory.path().join("renamed")).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(directory.path(), temporary.path().join("displaced")).is_err());
    original.revalidate().unwrap();
    drop(original);
    fs::write(&path, b"replaced").unwrap();
}

#[cfg(windows)]
#[test]
fn optional_regular_original_accepts_native_readers_and_rejects_acl_write_drift() {
    let (_temporary, directory) = store();
    let path = directory.path().join("record");
    let grant = |access: &str| {
        assert!(
            std::process::Command::new("icacls")
                .arg(&path)
                .arg("/grant")
                .arg(format!("*S-1-1-0:{access}"))
                .status()
                .unwrap()
                .success()
        );
    };
    grant("R");
    let original = directory
        .open_retained_regular_optional("record")
        .unwrap()
        .unwrap();
    original.revalidate().unwrap();
    let private = PrivateDirectory::open(directory.path()).unwrap();
    assert!(private.open_retained_private_optional("record").is_err());
    grant("W");
    assert!(original.revalidate().is_err());
    assert!(directory.open_retained_regular_optional("record").is_err());
}
