//! Genuine Windows direct-output custody and intentional-overwrite controls.

use std::{fs, io::BufWriter, path::Path, process::Command};

use super::*;
use crate::{PrivateDirectory, RetainedFile};

fn fixture() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("output");
    drop(PrivateDirectory::open_or_create(&path).unwrap());
    (temporary, OwnerDirectory::open(path).unwrap())
}

fn original(parent: &OwnerDirectory, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = parent.path().join(name);
    let mut file = RetainedFile::create_new_private(&path).unwrap();
    file.file_mut().write_all(bytes).unwrap();
    file.file().sync_all().unwrap();
    file.revalidate().unwrap();
    drop(file);
    path
}

fn grant(path: &Path, permission: &str) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", permission])
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture ACL mutation failed");
}

fn acl(path: &Path) -> Vec<u8> {
    let result = Command::new("icacls.exe").arg(path).output().unwrap();
    assert!(result.status.success(), "fixture ACL observation failed");
    result.stdout
}

fn junction(path: &Path, target: &Path) {
    let result = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture junction creation failed");
}

#[test]
fn native_output_creates_private_bytes_and_reuses_exact_public_original() {
    let (_temporary, parent) = fixture();
    let path = parent.path().join("new");
    let mut file = parent.open_truncating_output("new").unwrap();
    file.write_all(b"private\0bytes").unwrap();
    file.flush().unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert_eq!(
        crate::read_private(&path, 32).unwrap().as_slice(),
        b"private\0bytes"
    );
    let existing = original(&parent, "public", b"previous longer output");
    grant(&existing, "*S-1-1-0:(R)");
    let permissions = acl(&existing);
    let identity = RetainedFile::open_regular(&existing)
        .unwrap()
        .identity()
        .unwrap();
    let mut writer = BufWriter::new(parent.open_truncating_output("public").unwrap());
    writer.write_all(b"new").unwrap();
    writer.flush().unwrap();
    writer.get_ref().sync_all().unwrap();
    drop(writer);
    assert_eq!(fs::read(&existing).unwrap(), b"new");
    assert_eq!(
        RetainedFile::open_regular(&existing)
            .unwrap()
            .identity()
            .unwrap(),
        identity
    );
    assert_eq!(acl(&existing), permissions);
    assert!(
        RetainedFile::open_private(&existing).is_err(),
        "public read ACL was preserved"
    );
}

#[test]
fn native_output_refuses_invalid_names_before_creation() {
    let (_temporary, parent) = fixture();
    for name in ["", ".", "..", "../escape", "child/name", "alternate:stream"] {
        assert!(parent.open_truncating_output(name).is_err(), "{name}");
    }
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn native_output_refuses_aliases_reparse_points_and_nonfiles_before_truncation() {
    let (temporary, parent) = fixture();
    let path = original(&parent, "payload", b"original bytes");
    fs::hard_link(&path, parent.path().join("alias")).unwrap();
    assert!(parent.open_truncating_output("payload").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original bytes");
    assert_eq!(
        fs::read(parent.path().join("alias")).unwrap(),
        b"original bytes"
    );
    fs::remove_file(parent.path().join("alias")).unwrap();
    let directory = parent.path().join("directory");
    drop(PrivateDirectory::open_or_create(&directory).unwrap());
    assert!(parent.open_truncating_output("directory").is_err());
    let outside = temporary.path().join("outside");
    drop(PrivateDirectory::open_or_create(&outside).unwrap());
    let outside_file = outside.join("kept");
    fs::write(&outside_file, b"outside original").unwrap();
    junction(&parent.path().join("junction"), &outside);
    assert!(parent.open_truncating_output("junction").is_err());
    assert!(OwnerDirectory::open(parent.path().join("junction")).is_err());
    assert_eq!(fs::read(outside_file).unwrap(), b"outside original");
    assert_eq!(fs::read(path).unwrap(), b"original bytes");
}

#[test]
fn native_output_refuses_unsafe_acl_and_live_writer_without_changing_bytes() {
    let (_temporary, parent) = fixture();
    let path = original(&parent, "unsafe", b"unsafe original");
    grant(&path, "*S-1-1-0:(W)");
    let permissions = acl(&path);
    assert!(parent.open_truncating_output("unsafe").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"unsafe original");
    assert_eq!(acl(&path), permissions);
    let active = original(&parent, "active", b"active original");
    let writer = fs::OpenOptions::new().write(true).open(&active).unwrap();
    assert!(parent.open_truncating_output("active").is_err());
    assert_eq!(fs::read(&active).unwrap(), b"active original");
    drop(writer);
    let output = parent.open_truncating_output("active").unwrap();
    drop(output);
    assert!(fs::read(active).unwrap().is_empty());
}

#[test]
fn native_output_keeps_original_and_ancestry_after_parent_owner_drops() {
    let (temporary, parent) = fixture();
    let root = parent.path().to_owned();
    let path = root.join("output");
    let mut writer = parent.open_truncating_output("output").unwrap();
    drop(parent);
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&path, root.join("moved")).is_err());
    assert!(fs::rename(&root, temporary.path().join("moved-root")).is_err());
    writer.write_all(b"retained exact bytes").unwrap();
    writer.sync_all().unwrap();
    writer.revalidate().unwrap();
    drop(writer);
    assert_eq!(fs::read(path).unwrap(), b"retained exact bytes");
    fs::rename(&root, temporary.path().join("moved-root")).unwrap();
}

#[test]
fn native_output_rechecks_original_after_admission_before_truncation() {
    let (_temporary, parent) = fixture();
    let path = original(&parent, "output", b"must stay complete");
    let mut reached = false;
    let result = parent
        .inner
        .open_output_observed(OsStr::new("output"), |_| {
            reached = true;
            grant(&path, "*S-1-1-0:(W)");
            Ok(())
        });
    assert!(reached);
    assert!(result.is_err());
    assert_eq!(fs::read(path).unwrap(), b"must stay complete");
}

#[test]
fn native_output_final_custody_failure_does_not_acknowledge_written_bytes() {
    let (_temporary, parent) = fixture();
    let path = parent.path().join("output");
    let mut writer = parent.open_truncating_output("output").unwrap();
    let result = writer.observed(|file| {
        file.write_all(b"complete but unacknowledged")?;
        grant(&path, "*S-1-1-0:(R)");
        Ok(())
    });
    assert!(result.is_err());
    assert!(writer.flush().is_err());
    assert!(writer.sync_all().is_err());
    assert!(writer.write_all(b"must not append").is_err());
    drop(writer);
    assert_eq!(fs::read(path).unwrap(), b"complete but unacknowledged");
}

#[test]
fn native_output_preserves_first_io_error_when_final_custody_also_refuses() {
    let (_temporary, parent) = fixture();
    let path = parent.path().join("output");
    let mut writer = parent.open_truncating_output("output").unwrap();
    let result: io::Result<()> = writer.observed(|file| {
        file.write_all(b"partial")?;
        grant(&path, "*S-1-1-0:(R)");
        Err(io::Error::other("original write refusal"))
    });
    assert_eq!(result.unwrap_err().to_string(), "original write refusal");
    drop(writer);
    assert_eq!(fs::read(path).unwrap(), b"partial");
}
