//! Cargo hardlink retirement, original substitution and native sharing controls.

use super::*;
use std::{cell::RefCell, fs};

type Hook = Box<dyn FnOnce()>;
std::thread_local! {
    static BEFORE_NAMED: RefCell<Option<Hook>> = const { RefCell::new(None) };
}

pub(super) fn before_named_removal(_name: &OsStr) {
    let hook = BEFORE_NAMED.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

fn with_hook<T>(hook: impl FnOnce() + 'static, body: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            BEFORE_NAMED.with(|slot| slot.borrow_mut().take());
        }
    }
    BEFORE_NAMED.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(hook));
    });
    let _reset = Reset;
    let result = body();
    BEFORE_NAMED.with(|slot| assert!(slot.borrow().is_none()));
    result
}

fn fixture() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("release")).unwrap();
    fs::write(owner.path().join("output"), b"original cargo bytes").unwrap();
    fs::hard_link(owner.path().join("output"), owner.path().join("deps-alias")).unwrap();
    (temporary, owner)
}

#[test]
fn build_output_removes_only_admitted_name_preserving_other_hardlinks() {
    let (_temporary, owner) = fixture();
    let alias = owner.path().join("deps-alias");
    let before = FileMetadata::symlink_metadata(&alias).unwrap();
    assert_eq!(before.links(), 2);
    assert!(owner.remove_regular("output").is_err());
    assert!(owner.remove_build_output("output").unwrap());
    assert!(!owner.remove_build_output("output").unwrap());
    assert!(!owner.path().join("output").exists());
    let after = FileMetadata::symlink_metadata(&alias).unwrap();
    assert_eq!(before.identity(), after.identity());
    assert_eq!(after.links(), 1);
    assert_eq!(fs::read(&alias).unwrap(), b"original cargo bytes");
    assert!(owner.remove_build_output("deps-alias").unwrap());
    assert!(!alias.exists());
    for name in ["", ".", "..", "../output", "child/output"] {
        assert_eq!(
            owner.remove_build_output(name).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    let child = owner.ensure_child("child").unwrap();
    let identity = child.identity().unwrap();
    assert!(owner.remove_build_output("child").is_err());
    assert_eq!(child.identity().unwrap(), identity);
}

#[test]
fn build_output_refuses_link_count_change_after_original_admission() {
    let (_temporary, owner) = fixture();
    let output = owner.path().join("output");
    let added = owner.path().join("added-alias");
    let source = output.clone();
    with_hook(
        move || fs::hard_link(source, added).unwrap(),
        || owner.remove_build_output("output"),
    )
    .unwrap_err();
    assert_eq!(FileMetadata::symlink_metadata(&output).unwrap().links(), 3);
    assert_eq!(fs::read(output).unwrap(), b"original cargo bytes");
}

#[cfg(unix)]
#[test]
fn build_output_refuses_equal_byte_name_substitution_and_late_absence() {
    for replacement in [false, true] {
        let (_temporary, owner) = fixture();
        let output = owner.path().join("output");
        let saved = owner.path().join("saved");
        let original = FileMetadata::symlink_metadata(&output).unwrap().identity();
        let moving = output.clone();
        let destination = saved.clone();
        let error = with_hook(
            move || {
                fs::rename(&moving, destination).unwrap();
                if replacement {
                    fs::write(moving, b"original cargo bytes").unwrap();
                }
            },
            || owner.remove_build_output("output"),
        )
        .unwrap_err();
        if replacement {
            assert_ne!(
                FileMetadata::symlink_metadata(&output).unwrap().identity(),
                original
            );
            assert_eq!(fs::read(output).unwrap(), b"original cargo bytes");
        } else {
            // Rename may first invalidate the held original's change time;
            // otherwise the later name lookup must report its disappearance.
            assert!(
                error.kind() == io::ErrorKind::NotFound
                    || error.to_string() == changed().to_string()
            );
            assert!(!output.exists());
        }
        assert_eq!(
            FileMetadata::symlink_metadata(&saved).unwrap().identity(),
            original
        );
        assert_eq!(fs::read(saved).unwrap(), b"original cargo bytes");
    }
}

#[cfg(windows)]
#[test]
fn build_output_delete_original_blocks_competing_rename_and_alias_writes() {
    let (_temporary, owner) = fixture();
    let output = owner.path().join("output");
    let alias = owner.path().join("deps-alias");
    let saved = owner.path().join("saved");
    let original = FileMetadata::symlink_metadata(&alias).unwrap().identity();
    let writing = alias.clone();
    assert!(
        with_hook(
            move || {
                assert!(fs::rename(output, saved).is_err());
                assert!(fs::OpenOptions::new().write(true).open(writing).is_err());
            },
            || owner.remove_build_output("output")
        )
        .unwrap()
    );
    assert_eq!(
        FileMetadata::symlink_metadata(&alias).unwrap().identity(),
        original
    );
    assert_eq!(fs::read(alias).unwrap(), b"original cargo bytes");
}
