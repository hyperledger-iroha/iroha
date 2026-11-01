//! Native durability after reopening a directory with retained read-only ancestors.

use super::*;
use crate::{OwnerDirectory, PrivateDirectory};

#[test]
fn reopened_owner_removal_syncs_its_original_read_only_parent() {
    let temporary = tempfile::tempdir().unwrap();
    let parent_path = temporary.path().join("parent");
    let parent = OwnerDirectory::open_or_create(&parent_path).unwrap();
    let child = parent.create_child("child").unwrap();
    let child_path = child.path().to_path_buf();
    let parent_id = parent.identity().unwrap();
    drop(child);
    drop(parent);
    let original = OwnerDirectory::open(&child_path).unwrap();
    original
        .remove_empty()
        .expect("delete and sync the retained parent");
    assert!(!child_path.exists());
    assert_eq!(
        OwnerDirectory::open(&parent_path)
            .unwrap()
            .identity()
            .unwrap(),
        parent_id
    );
}

#[test]
fn reopened_private_removal_syncs_its_original_read_only_parent() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("parent")).unwrap();
    let child = parent.create_child("child").unwrap();
    let path = child.path().to_path_buf();
    drop(child);
    drop(parent);
    PrivateDirectory::open(&path)
        .unwrap()
        .remove_empty()
        .unwrap();
    assert!(!path.exists());
}

#[test]
fn scoped_parent_sync_and_read_pin_sync_preserve_the_original_identity() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("parent")).unwrap();
    let child = parent.create_child("child").unwrap();
    let path = child.path().to_path_buf();
    let identity = child.identity().unwrap();
    drop(child);
    let read_pin = Directory::open_reader(&path).unwrap();
    read_pin.sync_parent().unwrap();
    read_pin.sync().unwrap();
    read_pin.revalidate().unwrap();
    assert_eq!(read_pin.identity().unwrap(), identity);
    assert!(std::fs::rename(&path, parent.path().join("substitute")).is_err());
}

#[test]
fn reopened_source_move_syncs_both_original_parents_after_publication() {
    let temporary = tempfile::tempdir().unwrap();
    let source_parent = OwnerDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let child = source_parent.create_child("staging").unwrap();
    let original_path = child.path().to_path_buf();
    let original_id = child.identity().unwrap();
    let source_parent_path = source_parent.path().to_path_buf();
    let source_parent_id = source_parent.identity().unwrap();
    drop(child);
    drop(source_parent);
    let destination = OwnerDirectory::open_or_create(temporary.path().join("destination")).unwrap();
    let original = OwnerDirectory::open(&original_path).unwrap();
    let moved = original.move_no_replace(&destination, "complete").unwrap();
    moved.require_source_absent().unwrap();
    assert_eq!(moved.identity().unwrap(), original_id);
    assert_eq!(
        OwnerDirectory::open(source_parent_path)
            .unwrap()
            .identity()
            .unwrap(),
        source_parent_id
    );
    assert!(!original_path.exists());
    assert!(destination.path().join("complete").is_dir());
}
