//! Native existing-file durability, sharing and final-custody controls.

use std::{fs, io::Write as _, path::Path, process::Command};

use super::*;
use crate::{PrivateDirectory, RetainedFile};

fn fixture() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("private");
    drop(PrivateDirectory::open_or_create(&path).unwrap());
    (temporary, OwnerDirectory::open(path).unwrap())
}

fn write(parent: &OwnerDirectory, name: &str) -> std::path::PathBuf {
    let path = parent.path().join(name);
    let mut original = RetainedFile::create_new_private(&path).unwrap();
    original
        .file_mut()
        .write_all(b"retained exact bytes")
        .unwrap();
    original.file().sync_all().unwrap();
    original.revalidate().unwrap();
    path
}

fn grant(path: &Path) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(output.status.success(), "actual ACL mutation failed");
}

#[test]
fn native_durability_retains_exact_bytes_flush_and_exclusion_after_parent_drop() {
    let (temporary, parent) = fixture();
    let root = parent.path().to_owned();
    let path = write(&parent, "payload");
    let mut original = parent.open_private_durability("payload").unwrap();
    let before = original.snapshot().unwrap();
    original.require_name(&parent, "payload").unwrap();
    assert!(parent.open_retained_regular_optional("payload").is_err());
    drop(parent);
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&root, temporary.path().join("moved")).is_err());
    let mut bytes = Vec::new();
    original.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"retained exact bytes");
    original.rewind().unwrap();
    original.sync_all().unwrap();
    assert_eq!(original.snapshot().unwrap(), before);
    drop(original);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::rename(root, temporary.path().join("moved")).unwrap();
}

#[test]
fn native_durability_missing_invalid_and_nonprivate_files_remain_unchanged() {
    let (_temporary, parent) = fixture();
    for name in ["missing", "../escape", "alternate:stream", "."] {
        assert!(parent.open_private_durability(name).is_err());
    }
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    let path = write(&parent, "public");
    grant(&path);
    assert!(parent.open_private_durability("public").is_err());
    assert_eq!(fs::read(path).unwrap(), b"retained exact bytes");
    let inherited = parent.path().join("inherited");
    fs::write(&inherited, b"inherited").unwrap();
    assert!(parent.open_private_durability("inherited").is_err());
    assert_eq!(fs::read(inherited).unwrap(), b"inherited");
}

#[test]
fn native_durability_refuses_aliases_directories_and_live_writers() {
    let (_temporary, parent) = fixture();
    let path = write(&parent, "payload");
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(parent.open_private_durability("payload").is_err());
    drop(writer);
    let alias = parent.path().join("alias");
    fs::hard_link(&path, &alias).unwrap();
    assert!(parent.open_private_durability("payload").is_err());
    fs::remove_file(alias).unwrap();
    drop(PrivateDirectory::open_or_create(parent.path().join("directory")).unwrap());
    assert!(parent.open_private_durability("directory").is_err());
    parent
        .open_private_durability("payload")
        .unwrap()
        .sync_all()
        .unwrap();
    assert_eq!(fs::read(path).unwrap(), b"retained exact bytes");
}

#[test]
fn native_durability_rejects_other_parent_or_equal_byte_leaf() {
    let (_temporary, parent) = fixture();
    let (_other_temporary, other) = fixture();
    write(&parent, "first");
    write(&parent, "second");
    write(&other, "first");
    let original = parent.open_private_durability("first").unwrap();
    assert!(original.require_name(&parent, "second").is_err());
    assert!(original.require_name(&other, "first").is_err());
    original.require_name(&parent, "first").unwrap();
}

#[test]
fn native_durability_final_custody_refuses_success_and_preserves_first_error() {
    for fail_operation in [false, true] {
        let (_temporary, parent) = fixture();
        let path = write(&parent, "payload");
        let mut original = parent.open_private_durability("payload").unwrap();
        let result = original.observed(|file| {
            let mut byte = [0];
            file.read_exact(&mut byte)?;
            grant(&path);
            if fail_operation {
                Err(io::Error::other("initiating read refusal"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        if fail_operation {
            assert_eq!(result.unwrap_err().to_string(), "initiating read refusal");
        }
        assert!(original.revalidate().is_err());
        assert!(original.sync_all().is_err());
        drop(original);
        assert_eq!(fs::read(path).unwrap(), b"retained exact bytes");
    }
}
