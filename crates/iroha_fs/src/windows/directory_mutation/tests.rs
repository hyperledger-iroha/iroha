//! Actual NTFS original, publication, empty rollback and failure-latch controls.

use super::*;

fn fixture() -> (tempfile::TempDir, PathBuf, DirectoryMutation) {
    let temporary = tempfile::tempdir().expect("native temporary directory");
    let root = std::fs::canonicalize(temporary.path()).expect("local native root");
    let owner = DirectoryMutation::open(&root).expect("retained mutation root");
    (temporary, root, owner)
}

fn child(parent: &DirectoryMutation, path: &Path) -> DirectoryMutation {
    let mut slot = None;
    parent
        .create_child_into(path, &mut slot)
        .expect("exclusive private original");
    slot.expect("creation retains original")
}

#[test]
fn created_original_survives_postcreation_error_and_orders_directory() {
    let (_temporary, root, parent) = fixture();
    let path = root.join("created");
    let mut slot = None;
    let error = parent
        .create_child_into_observed(&path, &mut slot, |step| {
            assert_eq!(step, DirectoryMutationStep::Created);
            Err(io::Error::other("postcreate observer"))
        })
        .expect_err("injected after original installation");
    assert_eq!(error.to_string(), "postcreate observer");
    let original = slot.as_ref().expect("error retains original");
    let id = original.identity().expect("created identity");
    original.require_at(&path).expect("same private original");
    snapshot(&original.original.file, true, true).expect("protected creation ACL");
    original.sync_at(&path).expect("actual metadata barrier");
    parent.sync_at(&root).expect("parent metadata barrier");
    parent
        .create_child_into(&path, &mut slot)
        .expect_err("never create through occupied slot");
    assert_eq!(slot.as_ref().unwrap().identity().unwrap(), id);
}

#[test]
fn occupied_create_never_adopts_or_changes_existing_directory() {
    let (_temporary, root, parent) = fixture();
    let path = root.join("occupied");
    std::fs::create_dir(&path).expect("foreign fixture directory");
    std::fs::write(path.join("marker"), b"unchanged").expect("marker");
    let existing = DirectoryMutation::open(&path).expect("observe identity");
    let id = existing.identity().unwrap();
    let mut slot = None;
    parent
        .create_child_into(&path, &mut slot)
        .expect_err("exclusive create refuses collision");
    assert!(slot.is_none());
    existing
        .require_at(&path)
        .expect("same preexisting original");
    assert_eq!(existing.identity().unwrap(), id);
    assert_eq!(std::fs::read(path.join("marker")).unwrap(), b"unchanged");
    let other = child(&parent, &root.join("next"));
    other
        .require_at(root.join("next"))
        .expect("refusal does not poison parent");
}

#[test]
fn rename_error_latches_same_original_and_reverse_uses_retained_parents() {
    let (_temporary, root, parent) = fixture();
    let left = child(&parent, &root.join("left"));
    let right = child(&parent, &root.join("right"));
    let source = root.join("left/item");
    let target = root.join("right/item");
    let original = child(&left, &source);
    std::fs::write(source.join("bytes"), b"same contents").unwrap();
    let id = original.identity().unwrap();
    let mut published = false;
    let error = original
        .rename_no_replace_observed(&left, &source, &right, &target, &mut published, |_| {
            Err(io::Error::other("postrename observer"))
        })
        .expect_err("post-publication refusal");
    assert_eq!(error.to_string(), "postrename observer");
    assert!(published);
    assert!(!source.exists());
    original
        .require_at(&target)
        .expect("moved original retained");
    assert_eq!(original.identity().unwrap(), id);
    assert_eq!(
        std::fs::read(target.join("bytes")).unwrap(),
        b"same contents"
    );
    original
        .rename_no_replace(&left, &source, &right, &target, &mut published)
        .expect_err("latched rename must not run again");
    right.sync_at(root.join("right")).unwrap();
    left.sync_at(root.join("left")).unwrap();
    let mut reversed = false;
    original
        .rename_no_replace(&right, &target, &left, &source, &mut reversed)
        .expect("reverse same original");
    assert!(reversed);
    original.require_at(&source).unwrap();
    assert_eq!(original.identity().unwrap(), id);
}

#[test]
fn occupied_rename_preserves_both_originals_and_allows_same_owner_retry() {
    let (_temporary, root, parent) = fixture();
    let source = root.join("source");
    let target = root.join("target");
    let original = child(&parent, &source);
    let occupied = child(&parent, &target);
    let id = original.identity().unwrap();
    let occupied_id = occupied.identity().unwrap();
    let collision = rename_relative(
        &original.original.file,
        &parent.original.file,
        OsStr::new("target"),
    )
    .expect_err("kernel no-replace still refuses a target occupied after userspace admission");
    assert_eq!(collision.raw_os_error(), Some(183));
    original.require_at(&source).unwrap();
    occupied.require_at(&target).unwrap();
    let mut published = false;
    original
        .rename_no_replace(&parent, &source, &parent, &target, &mut published)
        .expect_err("no replacement allowed");
    assert!(!published);
    original.require_at(&source).unwrap();
    occupied.require_at(&target).unwrap();
    assert_eq!(occupied.identity().unwrap(), occupied_id);
    std::fs::rename(&target, root.join("preserved")).expect("same-user external move");
    original
        .rename_no_replace(&parent, &source, &parent, &target, &mut published)
        .unwrap();
    assert!(published);
    assert_eq!(original.identity().unwrap(), id);
    occupied.require_at(root.join("preserved")).unwrap();
}

#[test]
fn empty_removal_refuses_contents_then_retains_original_after_namespace_removal() {
    let (_temporary, root, parent) = fixture();
    let path = root.join("child");
    let original = child(&parent, &path);
    let id = original.identity().unwrap();
    std::fs::write(path.join("must-survive"), b"not empty").unwrap();
    let mut removed = false;
    let nonempty = original
        .remove_empty(&parent, &path, &mut removed)
        .expect_err("kernel refuses nonempty directory");
    assert_eq!(nonempty.raw_os_error(), Some(145));
    assert!(!removed);
    original.require_at(&path).unwrap();
    assert_eq!(
        std::fs::read(path.join("must-survive")).unwrap(),
        b"not empty"
    );
    std::fs::remove_file(path.join("must-survive")).unwrap();
    let error = original
        .remove_empty_observed(&parent, &path, &mut removed, |_| {
            Err(io::Error::other("postremove observer"))
        })
        .expect_err("after removal error");
    assert_eq!(error.to_string(), "postremove observer");
    assert!(removed);
    assert!(!path.exists());
    original
        .require_removed()
        .expect("same live removed original");
    assert_eq!(original.identity().unwrap(), id);
    parent.sync_at(&root).expect("same parent barrier retry");
    original
        .remove_empty(&parent, &path, &mut removed)
        .expect_err("no second disposition");
    assert_eq!(original.identity().unwrap(), id);
}

#[test]
fn nested_empty_rollback_does_not_close_child_original_to_remove_parent() {
    let (_temporary, root, anchor) = fixture();
    let parent_path = root.join("parent");
    let parent = child(&anchor, &parent_path);
    let path = parent_path.join("child");
    let original = child(&parent, &path);
    let child_id = original.identity().unwrap();
    original.remove_empty(&parent, &path, &mut false).unwrap();
    parent.sync_at(&parent_path).unwrap();
    parent
        .remove_empty(&anchor, &parent_path, &mut false)
        .expect("removed child handle must not require a close/reopen gap");
    assert!(!parent_path.exists());
    assert_eq!(original.identity().unwrap(), child_id);
    original
        .require_removed()
        .expect("removed child retains authenticated removed ancestors");
    parent.require_removed().unwrap();
    anchor.sync_at(&root).unwrap();
}

#[test]
fn parent_or_leaf_replacement_refuses_without_mutating_foreign_names() {
    let (_temporary, root, anchor) = fixture();
    let parent_path = root.join("parent");
    let parent = child(&anchor, &parent_path);
    let path = parent_path.join("child");
    let original = child(&parent, &path);
    std::fs::rename(&path, parent_path.join("saved")).unwrap();
    std::fs::create_dir(&path).unwrap();
    let mut removed = false;
    original
        .remove_empty(&parent, &path, &mut removed)
        .expect_err("foreign leaf refused");
    assert!(!removed);
    assert!(path.exists());
    assert!(parent_path.join("saved").exists());
    // NTFS refuses moving a directory with open descendants. Keep the leaf-drift
    // originals above alive and test parent drift independently on an empty parent.
    let parent_path = root.join("empty-parent");
    let parent = child(&anchor, &parent_path);
    std::fs::rename(&parent_path, root.join("saved-parent")).unwrap();
    std::fs::create_dir(&parent_path).unwrap();
    let mut slot = None;
    parent
        .create_child_into(&parent_path.join("new"), &mut slot)
        .expect_err("foreign parent refused");
    assert!(slot.is_none());
    assert!(!parent_path.join("new").exists());
    assert!(!root.join("saved-parent/new").exists());
}

#[test]
fn postcreate_name_substitution_preserves_first_error_and_created_original() {
    let (_temporary, root, parent) = fixture();
    let path = root.join("created");
    let displaced = root.join("displaced");
    let mut slot = None;
    let error = parent
        .create_child_into_observed(&path, &mut slot, |_| {
            std::fs::rename(&path, &displaced)?;
            std::fs::create_dir(&path)?;
            Err(io::Error::other("original failure"))
        })
        .expect_err("both observer and final name fail");
    assert_eq!(error.to_string(), "original failure");
    let original = slot.expect("created original remains available on refusal");
    original
        .require_at(&path)
        .expect_err("new name never adopted");
    original
        .require_at(&displaced)
        .expect("same original still held");
    assert!(path.exists());
}

#[test]
fn externally_delete_pending_original_is_not_admitted_as_owned_removal() {
    let (_temporary, root, parent) = fixture();
    let path = root.join("external");
    let original = child(&parent, &path);
    let id = original.identity().unwrap();
    std::fs::remove_dir(&path).expect("external same-user disposition");
    original
        .require_removed()
        .expect_err("only an exact owned disposition sets the latch");
    assert_eq!(original.identity().unwrap(), id);
}
