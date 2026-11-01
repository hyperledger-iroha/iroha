//! Original directory identity, namespace custody and publication interruption controls.

use super::*;
use std::fs;

fn fixture() -> (
    tempfile::TempDir,
    OwnerDirectory,
    OwnerDirectory,
    OwnerDirectory,
) {
    let temporary = tempfile::tempdir().unwrap();
    let source = OwnerDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let destination = OwnerDirectory::open_or_create(temporary.path().join("destination")).unwrap();
    let original = source.create_child("original").unwrap();
    original
        .write_atomic("contents", b"original bytes", PublishMode::CreateNew)
        .unwrap();
    (temporary, source, destination, original)
}

#[test]
fn cross_parent_move_preserves_original_identity_and_durable_namespace_order() {
    let (_temporary, source, destination, original) = fixture();
    let identity = original.identity().unwrap();
    let mut steps = Vec::new();
    let moved = original
        .move_no_replace_observed(&destination, "saved", |step| {
            steps.push(step);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        steps,
        [
            DirectoryMoveStep::Rename,
            DirectoryMoveStep::Published,
            DirectoryMoveStep::SyncDestination,
            DirectoryMoveStep::SyncSource,
            DirectoryMoveStep::Validate
        ]
    );
    assert_eq!(moved.identity().unwrap(), identity);
    moved.require_source_absent().unwrap();
    assert!(!source.path().join("original").exists());
    let mutable = moved.into_directory().unwrap();
    assert_eq!(mutable.identity().unwrap(), identity);
    assert_eq!(mutable.path(), destination.path().join("saved"));
    assert_eq!(
        fs::read(mutable.path().join("contents")).unwrap(),
        b"original bytes"
    );
    mutable
        .write_atomic("next", b"after ownership transfer", PublishMode::CreateNew)
        .unwrap();
}

#[test]
fn existing_and_racing_destinations_preserve_both_original_trees() {
    for racing in [false, true] {
        let (_temporary, source, destination, original) = fixture();
        let identity = original.identity().unwrap();
        if !racing {
            destination
                .create_child("saved")
                .unwrap()
                .write_atomic("other", b"competitor", PublishMode::CreateNew)
                .unwrap();
        }
        let error = original
            .move_no_replace_observed(&destination, "saved", |step| {
                assert!(racing);
                assert_eq!(step, DirectoryMoveStep::Rename);
                destination.create_child("saved")?.write_atomic(
                    "other",
                    b"competitor",
                    PublishMode::CreateNew,
                )
            })
            .unwrap_err();
        assert!(!error.publication_may_be_visible());
        assert_eq!(
            source.open_child("original").unwrap().identity().unwrap(),
            identity
        );
        assert_eq!(
            fs::read(source.path().join("original/contents")).unwrap(),
            b"original bytes"
        );
        assert_eq!(
            fs::read(destination.path().join("saved/other")).unwrap(),
            b"competitor"
        );
    }
}

#[test]
fn interrupted_move_reports_publication_before_any_post_move_failure() {
    for interrupted in [
        DirectoryMoveStep::Rename,
        DirectoryMoveStep::Published,
        DirectoryMoveStep::SyncDestination,
        DirectoryMoveStep::SyncSource,
        DirectoryMoveStep::Validate,
    ] {
        let (_temporary, source, destination, original) = fixture();
        let identity = original.identity().unwrap();
        let error = original
            .move_no_replace_observed(&destination, "saved", |step| {
                if step == interrupted {
                    Err(io::Error::other("injected interruption"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(error.to_string().contains("injected interruption"));
        assert!(error.source().is_some());
        assert_eq!(error.io_error().kind(), io::ErrorKind::Other);
        let published = interrupted != DirectoryMoveStep::Rename;
        assert_eq!(error.publication_may_be_visible(), published);
        let (parent, name, absent_parent, absent_name) = if published {
            (&destination, "saved", &source, "original")
        } else {
            (&source, "original", &destination, "saved")
        };
        assert_eq!(
            parent.open_child(name).unwrap().identity().unwrap(),
            identity
        );
        assert_eq!(
            fs::read(parent.path().join(name).join("contents")).unwrap(),
            b"original bytes"
        );
        assert!(!absent_parent.path().join(absent_name).exists());
    }
}

#[test]
fn source_name_reuse_is_separate_from_moved_original_custody() {
    let (_temporary, source, destination, original) = fixture();
    let identity = original.identity().unwrap();
    let moved = original.move_no_replace(&destination, "saved").unwrap();
    let fresh = source.create_child("original").unwrap();
    fresh
        .write_atomic("fresh", b"new authorized object", PublishMode::CreateNew)
        .unwrap();
    assert_ne!(fresh.identity().unwrap(), identity);
    moved.revalidate().unwrap();
    assert!(moved.require_source_absent().is_err());
    assert_eq!(
        moved.into_directory().unwrap().identity().unwrap(),
        identity
    );
    assert_eq!(
        fs::read(fresh.path().join("fresh")).unwrap(),
        b"new authorized object"
    );
}

#[test]
fn descendant_destination_is_refused_for_shared_and_independently_opened_owners() {
    for independent in [false, true] {
        let (_temporary, source, _destination, original) = fixture();
        let child = original.create_child("child").unwrap();
        let child_path = child.path().to_path_buf();
        let destination = if independent {
            drop(child);
            OwnerDirectory::open(&child_path).unwrap()
        } else {
            child
        };
        let error = original
            .move_no_replace(&destination, "nested")
            .unwrap_err();
        assert!(!error.publication_may_be_visible());
        assert!(source.path().join("original/contents").exists());
        assert!(!child_path.join("nested").exists());
    }
}

#[test]
fn shared_original_and_live_descendant_owners_refuse_move() {
    for descendant in [false, true] {
        let (_temporary, source, destination, original) = fixture();
        let held = if descendant {
            original.create_child("child").unwrap()
        } else {
            original.retain().unwrap()
        };
        let error = original.move_no_replace(&destination, "saved").unwrap_err();
        assert!(!error.publication_may_be_visible());
        held.revalidate().unwrap();
        assert!(source.path().join("original/contents").exists());
        assert!(!destination.path().join("saved").exists());
    }
}

#[test]
fn changed_original_contents_before_native_move_are_not_adopted() {
    let (_temporary, source, destination, original) = fixture();
    let path = original.path().to_path_buf();
    let error = original
        .move_no_replace_observed(&destination, "saved", |step| {
            assert_eq!(step, DirectoryMoveStep::Rename);
            fs::create_dir(path.join("interloper"))
        })
        .unwrap_err();
    assert!(!error.publication_may_be_visible());
    assert!(source.path().join("original/interloper").exists());
    assert!(!destination.path().join("saved").exists());
}

#[test]
fn parent_namespace_substitution_is_refused_or_natively_blocked() {
    for change_source in [false, true] {
        let (temporary, source, destination, original) = fixture();
        let parent = if change_source {
            source.path()
        } else {
            destination.path()
        }
        .to_path_buf();
        let displaced = temporary.path().join("displaced");
        let result = original.move_no_replace_observed(&destination, "saved", |step| {
            if step == DirectoryMoveStep::Rename {
                let changed = fs::rename(&parent, &displaced);
                #[cfg(unix)]
                {
                    changed?;
                    fs::create_dir(&parent)?;
                }
                #[cfg(windows)]
                assert!(
                    changed.is_err(),
                    "retained Windows parent must deny namespace replacement"
                );
            }
            Ok(())
        });
        #[cfg(unix)]
        {
            assert!(!result.unwrap_err().publication_may_be_visible());
            let source_path = if change_source {
                displaced
            } else {
                source.path().to_path_buf()
            };
            assert!(source_path.join("original/contents").exists());
            assert!(!destination.path().join("saved").exists());
        }
        #[cfg(windows)]
        result.unwrap().require_source_absent().unwrap();
    }
}

#[cfg(windows)]
#[test]
fn changed_but_still_admissible_native_acl_is_not_adopted() {
    let (_temporary, source, destination, original) = fixture();
    let path = original.path().to_path_buf();
    let error = original
        .move_no_replace_observed(&destination, "saved", |step| {
            assert_eq!(step, DirectoryMoveStep::Rename);
            let result = std::process::Command::new("icacls.exe")
                .arg(&path)
                .arg("/grant")
                .arg("*S-1-5-18:(F)")
                .output()?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            // SYSTEM custody is ordinarily admissible, but adding it cannot replace the
            // exact security descriptor captured from this original before the observer.
            OwnerDirectory::open(&path)?.revalidate()
        })
        .unwrap_err();
    assert!(!error.publication_may_be_visible());
    assert!(source.path().join("original/contents").exists());
    assert!(!destination.path().join("saved").exists());
}
