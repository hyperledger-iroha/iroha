//! Retained original files and bounded publication failure tests.
use super::*;
use std::os::unix::fs::{PermissionsExt as _, symlink};

fn root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(temp.path()).unwrap().join("outputs");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    (temp, path)
}
fn private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn hook(root: &Path, event: &'static str, name: &str, action: impl FnOnce() + 'static) {
    RETAINED_TEST_HOOK.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(RetainedTestHook {
            root: root.to_owned(),
            event,
            name: name.to_owned(),
            action: Box::new(action),
        });
    });
}
#[test]
fn retained_originals_and_new_publication_have_exact_bounded_identities() {
    let (_temp, path) = root();
    private(&path.join("original.toml"), b"private original");
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    let index = files
        .capture("original.toml", 32, Some(b"private original"))
        .unwrap();
    assert_eq!(files.bytes(index).unwrap(), b"private original");
    assert!(files.bytes(99).is_err());
    files.write_new("receipt.json", b"{}\n", 32).unwrap();
    assert_eq!(
        files.identities().collect::<Vec<_>>(),
        vec![
            (
                "original.toml",
                iroha_crypto::sha256(b"private original"),
                16
            ),
            ("receipt.json", iroha_crypto::sha256(b"{}\n"), 3),
        ]
    );
    files.check().unwrap();
    assert!(files.capture("original.toml", 32, None).is_err());
    assert!(files.write_new("receipt.json", b"changed", 32).is_err());
    assert_eq!(fs::read(path.join("receipt.json")).unwrap(), b"{}\n");
}
#[test]
fn retained_admission_rejects_bad_names_bounds_permissions_and_expected_bytes() {
    let (_temp, path) = root();
    private(&path.join("original"), b"original");
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    for name in ["", ".", "..", "a/b", "../original", "/original", "UPPER"] {
        assert!(files.capture(name, 32, None).is_err(), "{name}");
    }
    for bound in [0, 7, MAX_RETAINED_PRIVATE_BYTES + 1] {
        assert!(files.capture("original", bound, None).is_err());
    }
    assert!(files.capture("original", 32, Some(b"foreign!")).is_err());
    fs::set_permissions(path.join("original"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(files.capture("original", 32, None).is_err());
    assert!(RetainedPrivateFiles::new(Path::new("relative")).is_err());
}
#[test]
fn retained_admission_rejects_symlink_hardlink_directory_and_empty_leaf() {
    let (_temp, path) = root();
    private(&path.join("original"), b"original");
    symlink(path.join("original"), path.join("symlink")).unwrap();
    fs::hard_link(path.join("original"), path.join("hardlink")).unwrap();
    fs::create_dir(path.join("directory")).unwrap();
    private(&path.join("empty"), b"");
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    for name in ["original", "symlink", "hardlink", "directory", "empty"] {
        assert!(files.capture(name, 32, None).is_err(), "{name}");
    }
}
#[test]
fn retained_check_rejects_equal_byte_inode_replacement_and_in_place_change() {
    for replace in [false, true] {
        let (_temp, path) = root();
        private(&path.join("original"), b"original");
        let mut files = RetainedPrivateFiles::new(&path).unwrap();
        files.capture("original", 32, None).unwrap();
        if replace {
            private(&path.join("replacement"), b"original");
            fs::rename(path.join("replacement"), path.join("original")).unwrap();
        } else {
            private(&path.join("original"), b"modified");
        }
        assert!(files.check().is_err());
    }
}
#[test]
fn retained_check_rejects_earlier_leaf_swap_during_later_scan() {
    let (_temp, path) = root();
    private(&path.join("first"), b"first");
    private(&path.join("last"), b"last");
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    files.capture("first", 32, None).unwrap();
    files.capture("last", 32, None).unwrap();
    let changed = path.clone();
    hook(&path, "check", "last", move || {
        private(&changed.join("replacement"), b"first");
        fs::rename(changed.join("replacement"), changed.join("first")).unwrap();
    });
    assert!(files.check().is_err());
}
#[test]
fn retained_publication_rejects_equal_bytes_replaced_before_capture() {
    let (_temp, path) = root();
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    let changed = path.clone();
    hook(&path, "published", "receipt.json", move || {
        private(&changed.join("replacement"), b"{}\n");
        fs::rename(changed.join("replacement"), changed.join("receipt.json")).unwrap();
    });
    assert!(files.write_new("receipt.json", b"{}\n", 32).is_err());
    assert_eq!(fs::read(path.join("receipt.json")).unwrap(), b"{}\n");
}
#[test]
fn retained_publication_reserves_count_and_bytes_before_creation() {
    let (_temp, path) = root();
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    for (bytes, bound) in [
        (b"".as_slice(), 1),
        (b"too long".as_slice(), 1),
        (b"a".as_slice(), 0),
        (b"a".as_slice(), 1024 * 1024 + 1),
    ] {
        assert!(files.write_new("bad", bytes, bound).is_err());
        assert!(!path.join("bad").exists());
    }
    files.total = MAX_RETAINED_PRIVATE_BYTES;
    assert!(files.write_new("bad", b"a", 1).is_err());
    assert!(!path.join("bad").exists());
    files.total = 0;
    for index in 0..MAX_RETAINED_PRIVATE_FILES {
        files.write_new(&format!("f{index}"), b"a", 1).unwrap();
    }
    assert!(files.write_new("overflow", b"a", 1).is_err());
    assert!(!path.join("overflow").exists());
}
#[test]
fn retained_ancestor_substitution_is_rejected() {
    let (temp, path) = root();
    private(&path.join("original"), b"original");
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    files.capture("original", 32, None).unwrap();
    let moved = temp.path().join("moved");
    fs::rename(&path, &moved).unwrap();
    symlink(&moved, &path).unwrap();
    assert!(files.check().is_err());
}

fn directory(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn retained_namespace_seals_exact_files_and_initial_empty_directories() {
    let (_temp, path) = root();
    directory(&path.join("storage"));
    directory(&path.join("storage/peer0"));
    directory(&path.join("storage/peer0/kura"));
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    for name in ["storage", "storage/peer0", "storage/peer0/kura"] {
        files.capture_directory(name).unwrap();
    }
    files.write_new("receipt.json", b"{}\n", 32).unwrap();
    files.seal_namespace().unwrap();
    files.check().unwrap();
    assert!(files.seal_namespace().is_err());
    assert!(files.write_new("late", b"a", 1).is_err());
    assert!(!path.join("late").exists());
    private(&path.join("storage/peer0/kura/foreign"), b"a");
    assert!(files.check().is_err());
}
#[test]
fn retained_namespace_rejects_unlisted_root_and_nested_entries() {
    for nested in [false, true] {
        let (_temp, path) = root();
        directory(&path.join("storage"));
        let mut files = RetainedPrivateFiles::new(&path).unwrap();
        files.capture_directory("storage").unwrap();
        private(
            &path.join(if nested { "storage/foreign" } else { "foreign" }),
            b"a",
        );
        assert!(files.seal_namespace().is_err());
    }
}
#[test]
fn retained_directories_reject_missing_parent_alias_and_replacement() {
    let (temp, path) = root();
    directory(&path.join("storage"));
    directory(&path.join("storage/state"));
    let mut files = RetainedPrivateFiles::new(&path).unwrap();
    assert!(files.capture_directory("storage/state").is_err());
    for invalid in [
        "../storage",
        "/storage",
        "storage/../state",
        "storage//state",
    ] {
        assert!(files.capture_directory(invalid).is_err());
    }
    files.capture_directory("storage").unwrap();
    files.capture_directory("storage/state").unwrap();
    assert!(files.capture_directory("storage").is_err());
    fs::rename(path.join("storage/state"), temp.path().join("moved")).unwrap();
    directory(&path.join("storage/state"));
    assert!(files.check().is_err());
}
