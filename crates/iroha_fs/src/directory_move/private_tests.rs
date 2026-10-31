//! Native private-directory move and publication-boundary regressions.

use super::*;

#[test]
fn private_move_preserves_original_identity_contents_and_private_custody() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let source = parent.create_child("staging").unwrap();
    source
        .write_atomic("chunk", b"original bytes", PublishMode::CreateNew)
        .unwrap();
    let identity = source.identity().unwrap();
    let moved = source
        .move_no_replace_observed(&parent, "complete", |_| Ok(()))
        .unwrap();
    assert_eq!(moved.identity().unwrap(), identity);
    moved.require_source_absent().unwrap();
    let complete = parent.open_child("complete").unwrap();
    assert_eq!(&*complete.read("chunk", 14).unwrap(), b"original bytes");
    assert_eq!(complete.identity().unwrap(), identity);
    assert!(!parent.path().join("staging").exists());
}

#[test]
fn private_move_never_replaces_existing_destination() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let source = parent.create_child("staging").unwrap();
    let destination = parent.create_child("complete").unwrap();
    let original_destination = destination.identity().unwrap();
    let error = source
        .move_no_replace_observed(&parent, "complete", |_| Ok(()))
        .unwrap_err();
    assert!(!error.publication_may_be_visible());
    assert_eq!(destination.identity().unwrap(), original_destination);
    assert!(parent.path().join("staging").is_dir());
}

#[test]
fn private_move_reports_actual_native_publication_before_observer_failure() {
    for failure in [DirectoryMoveStep::Rename, DirectoryMoveStep::Published] {
        let temporary = tempfile::tempdir().unwrap();
        let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let source = parent.create_child("staging").unwrap();
        let identity = source.identity().unwrap();
        let error = source
            .move_no_replace_observed(&parent, "complete", |step| {
                if step == failure {
                    Err(io::Error::other("injected move-boundary failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        let published = failure == DirectoryMoveStep::Published;
        assert_eq!(error.publication_may_be_visible(), published);
        let name = if published { "complete" } else { "staging" };
        assert_eq!(
            parent.open_child(name).unwrap().identity().unwrap(),
            identity
        );
        assert_eq!(parent.path().join("complete").exists(), published);
    }
}

#[test]
fn private_move_refuses_live_descendant_custody() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let source = parent.create_child("staging").unwrap();
    source
        .write_atomic("chunk", b"original", PublishMode::CreateNew)
        .unwrap();
    let file = source.open_retained_private("chunk").unwrap();
    let error = source
        .move_no_replace_observed(&parent, "complete", |_| Ok(()))
        .unwrap_err();
    assert!(!error.publication_may_be_visible());
    file.revalidate().unwrap();
    assert!(!parent.path().join("complete").exists());
}

#[cfg(windows)]
#[test]
fn private_move_does_not_adopt_owner_only_policy_after_acl_drift() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let source = parent.create_child("staging").unwrap();
    let path = source.path().to_path_buf();
    let error = source
        .move_no_replace_observed(&parent, "complete", |step| {
            assert_eq!(step, DirectoryMoveStep::Rename);
            let result = std::process::Command::new("icacls.exe")
                .arg(&path)
                .args(["/grant", "*S-1-5-18:(F)"])
                .output()?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            // SYSTEM rights may be admissible for an OwnerDirectory but cannot
            // replace the private policy of this original source or destination.
            assert!(PrivateDirectory::open(&path).is_err());
            Ok(())
        })
        .unwrap_err();
    assert!(!error.publication_may_be_visible());
    assert!(path.is_dir());
    assert!(!parent.path().join("complete").exists());
}
