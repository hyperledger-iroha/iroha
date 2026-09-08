//! Retained parent-directory setup and deterministic substitution controls without children.

use super::*;
use std::os::unix::{
    fs::{PermissionsExt, symlink},
    net::UnixListener,
};

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let parent = temporary.path().canonicalize().unwrap().join("parent");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&parent)
        .unwrap();
    (temporary, parent)
}

#[test]
fn capture_setup_retains_nonblocking_parent_and_allows_its_own_child_metadata_change() {
    let (_temporary, parent) = fixture();
    let admitted = std::fs::symlink_metadata(&parent).unwrap();
    let captures = Captures::create(&parent.join("captures")).unwrap();
    let held = captures.parent.directory.metadata().unwrap();
    assert_eq!((held.dev(), held.ino()), (admitted.dev(), admitted.ino()));
    assert!(
        rustix::fs::fcntl_getfl(&captures.parent.directory)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    assert!(
        rustix::fs::fcntl_getfl(&captures.directory)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    assert!(captures.check().is_ok());
    std::fs::write(parent.join("unrelated"), b"different directory metadata").unwrap();
    assert!(captures.parent.check().is_ok());
    assert_eq!(
        captures.directory.metadata().unwrap().mode() & 0o7777,
        0o700
    );
    assert!(Captures::create(&parent.join("captures")).is_err());
}

#[test]
fn capture_setup_rejects_special_parent_substitution_at_admission_and_retention() {
    for stage in [
        CaptureSetupStep::ParentAdmitted,
        CaptureSetupStep::ParentRetained,
    ] {
        let (_temporary, parent) = fixture();
        let moved = parent.with_file_name("retained-original-parent");
        let mut socket = None;
        let mut substituted = false;
        let result = Captures::create_with_hook(&parent.join("captures"), |point| {
            if point == stage {
                std::fs::rename(&parent, &moved).unwrap();
                socket = Some(UnixListener::bind(&parent).unwrap());
                substituted = true;
            }
            Ok(())
        });
        assert!(substituted);
        assert!(result.is_err());
        assert!(socket.is_some());
        assert!(!moved.join("captures").exists());
        assert!(!std::fs::symlink_metadata(&parent).unwrap().is_dir());
    }
}

#[test]
fn capture_setup_never_syncs_a_replacement_parent_path() {
    for stage in [CaptureSetupStep::BeforeSync, CaptureSetupStep::AfterSync] {
        for replace_with_link in [false, true] {
            let (_temporary, parent) = fixture();
            let moved = parent.with_file_name("retained-original-parent");
            let mut substituted = false;
            let result = Captures::create_with_hook(&parent.join("captures"), |point| {
                if point == stage {
                    std::fs::rename(&parent, &moved).unwrap();
                    if replace_with_link {
                        symlink(&moved, &parent).unwrap();
                    } else {
                        std::fs::DirBuilder::new()
                            .mode(0o700)
                            .create(&parent)
                            .unwrap();
                    }
                    substituted = true;
                }
                Ok(())
            });
            assert!(substituted);
            assert!(result.is_err());
            assert!(moved.join("captures").is_dir());
            if !replace_with_link {
                assert!(!parent.join("captures").exists());
            }
        }
    }
}

#[test]
fn capture_setup_rejects_socket_parent_at_the_former_bare_sync_boundary() {
    let (_temporary, parent) = fixture();
    let moved = parent.with_file_name("retained-original-parent");
    let mut socket = None;
    let result = Captures::create_with_hook(&parent.join("captures"), |point| {
        if point == CaptureSetupStep::BeforeSync {
            std::fs::rename(&parent, &moved).unwrap();
            socket = Some(UnixListener::bind(&parent).unwrap());
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(socket.is_some());
    assert!(moved.join("captures").is_dir());
}

#[test]
fn capture_setup_rejects_direct_non_directory_parent_and_permission_drift() {
    let (temporary, parent) = fixture();
    let file = temporary.path().canonicalize().unwrap().join("file-parent");
    std::fs::write(&file, b"not a directory").unwrap();
    assert!(Captures::create(&file.join("captures")).is_err());
    let socket_path = temporary
        .path()
        .canonicalize()
        .unwrap()
        .join("socket-parent");
    let socket = UnixListener::bind(&socket_path).unwrap();
    assert!(Captures::create(&socket_path.join("captures")).is_err());
    drop(socket);
    let result = Captures::create_with_hook(&parent.join("captures"), |point| {
        if point == CaptureSetupStep::ParentRetained {
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(!parent.join("captures").exists());
}

#[test]
fn established_capture_keeps_parent_identity_and_setup_errors_have_no_fallback() {
    let (_temporary, parent) = fixture();
    let captures = Captures::create(&parent.join("captures")).unwrap();
    let moved = parent.with_file_name("retained-original-parent");
    std::fs::rename(&parent, &moved).unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&parent)
        .unwrap();
    assert!(captures.check().is_err());
    assert_eq!(
        captures.parent.directory.metadata().unwrap().ino(),
        std::fs::metadata(&moved).unwrap().ino()
    );
    let error_path = parent.join("error-captures");
    let result = Captures::create_with_hook(&error_path, |point| {
        if point == CaptureSetupStep::BeforeSync {
            bail!("injected setup failure");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(error_path.is_dir());
    assert!(Captures::create(&error_path).is_err());
}
