//! Real native read-only public originals and exact security/ancestor refusal controls.

use super::*;
use crate::{OwnerDirectory, PublishMode};
use std::{fs, process::Command};

fn root() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::Builder::new()
        .prefix("iroha-strict-public-")
        .tempdir_in(std::env::temp_dir())
        .unwrap();
    let directory = OwnerDirectory::open_or_create(temporary.path().join("inputs")).unwrap();
    directory
        .write_atomic("record", b"exact original", PublishMode::CreateNew)
        .unwrap();
    (temporary, directory)
}

fn grant(path: &Path, access: &str) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", access])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn strict_public_input_reads_current_owner_private_and_public_originals() {
    for public in [false, true] {
        let (_temporary, directory) = root();
        let path = directory.path().join("record");
        if public {
            grant(&path, "*S-1-1-0:(R)");
        }
        let original = StrictPublicInput::open(&path).unwrap();
        let before = original.snapshot().unwrap();
        assert_eq!(before.metadata_parts().0, 2);
        assert_eq!(before.metadata_parts().2, 14);
        assert_eq!(before.metadata_parts().3[1], 1);
        let security = before.security_parts();
        assert!(!security.0.is_empty());
        assert!(!security.2.is_empty());
        assert_eq!(original.read(14).unwrap(), b"exact original");
        assert!(original.read(13).is_err());
        assert_eq!(original.snapshot().unwrap(), before);
        original.revalidate().unwrap();
    }
}

#[test]
fn strict_public_input_excludes_writers_deletion_and_parent_replacement() {
    let (temporary, directory) = root();
    let path = directory.path().join("record");
    let original = StrictPublicInput::open(&path).unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&path, directory.path().join("moved-record")).is_err());
    assert!(fs::rename(directory.path(), temporary.path().join("moved-parent")).is_err());
    assert_eq!(original.read(14).unwrap(), b"exact original");
}

#[test]
fn strict_public_input_refuses_hardlinks_and_junction_parents_without_skips() {
    let (temporary, directory) = root();
    let path = directory.path().join("record");
    let alias = directory.path().join("alias");
    fs::hard_link(&path, &alias).unwrap();
    assert!(StrictPublicInput::open(&path).is_err());
    fs::remove_file(alias).unwrap();
    let junction = temporary.path().join("junction");
    let output = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let refused = StrictPublicInput::open(junction.join("record")).is_err();
    fs::remove_dir(&junction).unwrap();
    assert!(refused);
    assert_eq!(fs::read(path).unwrap(), b"exact original");
}

#[test]
fn strict_public_input_refuses_new_foreign_write_permissions() {
    let (_temporary, directory) = root();
    let path = directory.path().join("record");
    grant(&path, "*S-1-1-0:(W)");
    assert!(StrictPublicInput::open(path).is_err());
}

#[test]
fn strict_public_input_security_drift_refuses_even_if_reader_policy_stays_safe() {
    let (_temporary, directory) = root();
    let path = directory.path().join("record");
    let original = StrictPublicInput::open(&path).unwrap();
    grant(&path, "*S-1-1-0:(R)");
    // The ordinary safe-reader policy still admits this ACL. The exact admitted
    // descriptor is different, so this original must refuse nevertheless.
    validate_acl(original.original.file(), false, false).unwrap();
    assert!(original.revalidate().is_err());
    assert!(original.read(14).is_err());
}

#[test]
fn strict_public_input_pins_private_parent_metadata_but_not_shared_namespace_times() {
    for shared in [false, true] {
        let (_temporary, directory) = root();
        if shared {
            grant(directory.path(), "*S-1-1-0:(R)");
        }
        let original = StrictPublicInput::open(directory.path().join("record")).unwrap();
        assert_eq!(original.parents.last().unwrap().owner_only, !shared);
        directory
            .write_atomic("unrelated", b"independent", PublishMode::CreateNew)
            .unwrap();
        if shared {
            original.revalidate().unwrap();
            assert_eq!(original.read(14).unwrap(), b"exact original");
        } else {
            assert!(original.revalidate().is_err());
        }
    }
}

#[test]
fn strict_public_input_shared_parent_security_remains_exact() {
    let (_temporary, directory) = root();
    grant(directory.path(), "*S-1-1-0:(R)");
    let original = StrictPublicInput::open(directory.path().join("record")).unwrap();
    assert!(!original.parents.last().unwrap().owner_only);
    grant(directory.path(), "*S-1-5-11:(R)");
    directory.revalidate().unwrap();
    assert!(original.revalidate().is_err());
}

#[test]
fn strict_public_input_rejects_ambiguous_path_spellings_before_io() {
    for path in [
        r"relative",
        r"C:relative",
        r"\\server\share\record",
        r"\\.\C:\record",
        r"C:\inputs\..\record",
        r"C:\inputs\.\record",
        r"C:\inputs\\record",
        r"C:\inputs\record:",
        r"C:\inputs\record:stream",
        r"C:\inputs\record.",
        r"C:\inputs\CON",
        r"C:\inputs\record\",
    ] {
        assert!(StrictPublicInput::validate_path(path).is_err(), "{path}");
    }
    StrictPublicInput::validate_path(r"C:\inputs\record").unwrap();
    StrictPublicInput::validate_path(r"\\?\C:\inputs\record").unwrap();
}
