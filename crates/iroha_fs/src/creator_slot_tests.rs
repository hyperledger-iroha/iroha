//! Exclusive creation preserves the original descriptor across post-create failure.

use super::*;
use std::io::Write as _;

#[test]
fn exclusive_creator_keeps_original_and_refuses_slot_or_name_reuse() {
    let root = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open(root.path()).unwrap();
    let mut original = None;
    owner
        .create_private_file_into("segment", &mut original)
        .unwrap();
    let mut file = original.as_ref().unwrap();
    let identity = FileMetadata::of(file).unwrap().identity();
    file.write_all(b"original").unwrap();
    owner.require_same_lock("segment", file).unwrap();
    assert!(
        owner
            .create_private_file_into("other", &mut original)
            .is_err()
    );
    assert!(!root.path().join("other").exists());
    let mut collision = None;
    assert_eq!(
        owner
            .create_private_file_into("segment", &mut collision)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(collision.is_none());
    let file = original.as_ref().unwrap();
    assert_eq!(FileMetadata::of(file).unwrap().identity(), identity);
    let mut bytes = [0; 8];
    read_exact_at(file, &mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"original");
}

#[test]
fn post_creation_failure_retains_exact_original_in_owner_slot() {
    let root = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open(root.path()).unwrap();
    let mut original = None;
    let mut created_identity = None;
    let error = owner
        .create_private_file_into_observed(OsStr::new("uncertain"), &mut original, |mut file| {
            created_identity = Some(FileMetadata::of(file)?.identity());
            file.write_all(b"retained")?;
            Err(io::Error::other("injected post-create durability failure"))
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    let file = original
        .as_ref()
        .expect("failed creation must keep its original owner");
    assert_eq!(
        Some(FileMetadata::of(file).unwrap().identity()),
        created_identity
    );
    owner.require_same_lock("uncertain", file).unwrap();
    FileSnapshot::private_journal(file).unwrap();
    let mut bytes = [0; 8];
    read_exact_at(file, &mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"retained");
    file.sync_all().unwrap();
    owner.sync().unwrap();
}

#[test]
fn post_creation_link_violation_preserves_original_and_forensic_names() {
    let root = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open(root.path()).unwrap();
    let mut original = None;
    assert!(
        owner
            .create_private_file_into_observed(OsStr::new("segment"), &mut original, |_| {
                std::fs::hard_link(
                    root.path().join("segment"),
                    root.path().join("foreign-link"),
                )
            })
            .is_err()
    );
    let file = original
        .as_ref()
        .expect("real custody failure must retain created descriptor");
    let metadata = FileMetadata::of(file).unwrap();
    assert_eq!(metadata.links(), 2);
    for name in ["segment", "foreign-link"] {
        assert_eq!(
            FileMetadata::symlink_metadata(root.path().join(name))
                .unwrap()
                .identity(),
            metadata.identity()
        );
    }
    assert!(FileSnapshot::private_journal(file).is_err());
}
