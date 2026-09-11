// Real per-call namespace substitutions; no global hooks or current-directory mutation.
mod tests {
    use super::*;
    use std::{
        io::Write as _,
        os::unix::{
            fs::{MetadataExt as _, PermissionsExt as _, symlink},
            net::UnixListener,
        },
    };

    struct Fixture {
        _directory: tempfile::TempDir,
        ancestor: PathBuf,
        parent: PathBuf,
        target: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            // Keep socket paths below macOS sockaddr_un's small limit.
            let directory = tempfile::Builder::new()
                .prefix("out-")
                .tempdir_in("/tmp")
                .expect("test directory");
            let ancestor = directory
                .path()
                .canonicalize()
                .expect("actual absolute parent")
                .join("a");
            let parent = ancestor.join("p");
            std::fs::create_dir_all(&parent).unwrap();
            let target = parent.join("trace.json");
            Self {
                _directory: directory,
                ancestor,
                parent,
                target,
            }
        }
        fn stage(&self) -> PathBuf {
            self.parent.join("trace.json.collecting")
        }
        fn output(&self) -> TraceOutput {
            let mut owned = TraceOutput::create(&self.target, 1024).unwrap();
            owned.file_mut().write_all(b"complete trace\n").unwrap();
            owned
        }
        fn replace_directory(&self, ancestor: bool) {
            let path = if ancestor {
                &self.ancestor
            } else {
                &self.parent
            };
            std::fs::rename(path, path.with_extension("old")).unwrap();
            std::fs::create_dir(path).unwrap();
        }
    }
    fn fifo(path: &Path) -> File {
        let status = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
        File::from(
            rustix::fs::open(
                path,
                OFlags::RDWR | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        )
    }
    #[test]
    fn owned_journal_uses_actual_nonblocking_exclusive_private_descriptor_and_parent_sync() {
        let fixture = Fixture::new();
        let mut phases = Vec::new();
        let mut file = new_owned_file_with_hook(&fixture.target, |phase| {
            phases.push(phase);
            Ok(())
        })
        .unwrap();
        let flags = rustix::fs::fcntl_getfl(&file).unwrap();
        assert!(flags.contains(OFlags::NONBLOCK));
        assert_eq!(flags & OFlags::ACCMODE, OFlags::WRONLY);
        assert!(
            rustix::io::fcntl_getfd(&file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert_eq!(file.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(file.metadata().unwrap().nlink(), 1);
        assert!(phases.contains(&Phase::BeforeDirectorySync));
        assert!(phases.contains(&Phase::AfterDirectorySync));
        file.write_all(b"journal\n").unwrap();
        file.sync_all().unwrap();
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"journal\n");
        assert!(new_owned_file(&fixture.target).is_err());
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"journal\n");
    }
    #[test]
    fn owned_trace_completes_file_sync_atomic_rename_and_retained_directory_sync() {
        let fixture = Fixture::new();
        let output = fixture.output();
        let inode = output.file.metadata().unwrap().ino();
        let mut phases = Vec::new();
        output
            .publish_with_hook(|phase| {
                phases.push(phase);
                Ok(())
            })
            .unwrap();
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"complete trace\n");
        assert!(!fixture.stage().exists());
        let metadata = std::fs::metadata(&fixture.target).unwrap();
        assert_eq!(metadata.ino(), inode);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(
            phases,
            vec![
                Phase::BeforeFileSync,
                Phase::AfterFileSync,
                Phase::BeforeRename,
                Phase::RenameReady,
                Phase::AfterRename,
                Phase::BeforeDirectorySync,
                Phase::AfterDirectorySync
            ]
        );
    }
    #[test]
    fn output_rejects_relative_dot_parent_and_symlink_paths_before_create() {
        let fixture = Fixture::new();
        for path in [
            PathBuf::from("relative.json"),
            fixture.parent.join(".").join("trace.json"),
            fixture.parent.join("..").join("trace.json"),
        ] {
            assert!(new_owned_file(&path).is_err());
        }
        let alias = fixture.ancestor.join("alias");
        symlink(&fixture.parent, &alias).unwrap();
        assert!(new_owned_file(&alias.join("trace.json")).is_err());
        assert!(!fixture.target.exists());
        let long = PathBuf::from(format!("/{}", "x".repeat(MAX_PATH_BYTES)));
        assert!(new_owned_file(&long).is_err());
        let deep = PathBuf::from(format!("/{}leaf", "p/".repeat(MAX_COMPONENTS)));
        assert!(new_owned_file(&deep).is_err());
    }
    #[test]
    fn output_parent_and_ancestor_replacements_fail_during_admission_and_creation() {
        for ancestor in [false, true] {
            for phase in [
                Phase::ParentAdmitted,
                Phase::ParentRetained,
                Phase::BeforeCreate,
                Phase::AfterCreate,
                Phase::BeforeDirectorySync,
                Phase::AfterDirectorySync,
            ] {
                let fixture = Fixture::new();
                let mut fired = false;
                let result = new_owned_file_with_hook(&fixture.target, |event| {
                    if event == phase && !fired {
                        fired = true;
                        fixture.replace_directory(ancestor);
                    }
                    Ok(())
                });
                assert!(fired);
                assert!(result.is_err(), "ancestor={ancestor} phase={phase:?}");
                assert!(!fixture.target.exists());
            }
        }
    }
    #[test]
    fn output_directory_open_rejects_fifo_socket_symlink_and_regular_substitutes() {
        for kind in 0..4 {
            let fixture = Fixture::new();
            let mut peer = None;
            let mut listener = None;
            let mut fired = false;
            let result = new_owned_file_with_hook(&fixture.target, |phase| {
                if phase == Phase::ParentAdmitted && !fired {
                    fired = true;
                    std::fs::rename(&fixture.parent, fixture.parent.with_extension("old")).unwrap();
                    match kind {
                        0 => peer = Some(fifo(&fixture.parent)),
                        1 => listener = Some(UnixListener::bind(&fixture.parent).unwrap()),
                        2 => {
                            symlink(fixture.parent.with_extension("old"), &fixture.parent).unwrap()
                        }
                        _ => std::fs::write(&fixture.parent, b"racer").unwrap(),
                    }
                }
                Ok(())
            });
            assert!(fired);
            assert!(result.is_err(), "substitute {kind}");
            drop(peer);
            drop(listener);
        }
    }
    #[test]
    fn output_create_exclusive_preserves_existing_regular_fifo_socket_and_symlink() {
        for kind in 0..4 {
            let fixture = Fixture::new();
            let mut peer = None;
            let mut listener = None;
            let original = fixture.parent.join("original");
            std::fs::write(&original, b"original bytes").unwrap();
            match kind {
                0 => std::fs::write(&fixture.target, b"racer bytes").unwrap(),
                1 => peer = Some(fifo(&fixture.target)),
                2 => listener = Some(UnixListener::bind(&fixture.target).unwrap()),
                _ => symlink(&original, &fixture.target).unwrap(),
            }
            let before = std::fs::symlink_metadata(&fixture.target).unwrap();
            assert!(new_owned_file(&fixture.target).is_err());
            let after = std::fs::symlink_metadata(&fixture.target).unwrap();
            assert_eq!(before.ino(), after.ino());
            assert_eq!(before.mode(), after.mode());
            assert_eq!(std::fs::read(&original).unwrap(), b"original bytes");
            if kind == 0 {
                assert_eq!(std::fs::read(&fixture.target).unwrap(), b"racer bytes");
            }
            drop(peer);
            drop(listener);
        }
    }
    #[test]
    fn output_rejects_post_create_inode_mode_hardlink_and_length_changes_without_cleanup() {
        for kind in 0..4 {
            let fixture = Fixture::new();
            let copy = fixture.parent.join("racer");
            let mut fired = false;
            let result = new_owned_file_with_hook(&fixture.target, |phase| {
                if phase == Phase::AfterCreate {
                    fired = true;
                    match kind {
                        0 => {
                            std::fs::rename(&fixture.target, &copy).unwrap();
                            std::fs::write(&fixture.target, b"new inode").unwrap();
                        }
                        1 => std::fs::set_permissions(
                            &fixture.target,
                            std::fs::Permissions::from_mode(0o640),
                        )
                        .unwrap(),
                        2 => std::fs::hard_link(&fixture.target, &copy).unwrap(),
                        _ => std::fs::write(&fixture.target, b"unexpected bytes").unwrap(),
                    }
                }
                Ok(())
            });
            assert!(fired);
            assert!(result.is_err());
            assert!(fixture.target.exists());
            if kind == 0 {
                assert_eq!(std::fs::read(&fixture.target).unwrap(), b"new inode");
                assert!(copy.exists());
            }
            if kind == 2 {
                assert_eq!(std::fs::metadata(&fixture.target).unwrap().nlink(), 2);
                assert!(copy.exists());
            }
        }
    }
    #[test]
    fn trace_rejects_replaced_parent_and_ancestor_at_every_publication_boundary() {
        for ancestor in [false, true] {
            for phase in [
                Phase::BeforeFileSync,
                Phase::AfterFileSync,
                Phase::BeforeRename,
                Phase::AfterRename,
                Phase::BeforeDirectorySync,
                Phase::AfterDirectorySync,
            ] {
                let fixture = Fixture::new();
                let output = fixture.output();
                let mut fired = false;
                let result = output.publish_with_hook(|event| {
                    if event == phase && !fired {
                        fired = true;
                        fixture.replace_directory(ancestor);
                    }
                    Ok(())
                });
                assert!(fired);
                assert!(result.is_err(), "ancestor={ancestor} phase={phase:?}");
                assert!(!fixture.target.exists());
            }
        }
    }
    #[test]
    fn trace_noreplace_preserves_existing_and_racing_destination_bytes() {
        for racing in [false, true] {
            let fixture = Fixture::new();
            let output = fixture.output();
            if !racing {
                std::fs::write(&fixture.target, b"winner").unwrap();
            }
            let result = output.publish_with_hook(|phase| {
                if racing && phase == Phase::BeforeRename {
                    std::fs::write(&fixture.target, b"winner").unwrap();
                }
                Ok(())
            });
            assert!(result.is_err());
            assert_eq!(std::fs::read(&fixture.target).unwrap(), b"winner");
            assert_eq!(std::fs::read(fixture.stage()).unwrap(), b"complete trace\n");
        }
    }
    #[test]
    fn trace_noreplace_preserves_destination_fifo_socket_symlink_and_hardlink() {
        for kind in 0..4 {
            let fixture = Fixture::new();
            let output = fixture.output();
            let original = fixture.parent.join("original");
            std::fs::write(&original, b"original").unwrap();
            let mut peer = None;
            let mut listener = None;
            match kind {
                0 => peer = Some(fifo(&fixture.target)),
                1 => listener = Some(UnixListener::bind(&fixture.target).unwrap()),
                2 => symlink(&original, &fixture.target).unwrap(),
                _ => std::fs::hard_link(&original, &fixture.target).unwrap(),
            }
            let before = std::fs::symlink_metadata(&fixture.target).unwrap();
            assert!(output.publish().is_err());
            let after = std::fs::symlink_metadata(&fixture.target).unwrap();
            assert_eq!(before.ino(), after.ino());
            assert_eq!(before.mode(), after.mode());
            assert_eq!(std::fs::read(&original).unwrap(), b"original");
            assert!(fixture.stage().is_file());
            drop(peer);
            drop(listener);
        }
    }
    #[test]
    fn trace_stage_replacements_and_hardlinks_fail_without_unowned_removal() {
        for kind in 0..5 {
            let fixture = Fixture::new();
            let output = fixture.output();
            let stage = fixture.stage();
            let saved = fixture.parent.join("saved");
            let mut peer = None;
            let mut listener = None;
            let result = output.publish_with_hook(|phase| {
                if phase == Phase::BeforeRename {
                    if kind == 4 {
                        std::fs::hard_link(&stage, &saved).unwrap();
                    } else {
                        std::fs::rename(&stage, &saved).unwrap();
                        match kind {
                            0 => std::fs::write(&stage, b"racer").unwrap(),
                            1 => peer = Some(fifo(&stage)),
                            2 => listener = Some(UnixListener::bind(&stage).unwrap()),
                            _ => symlink(&saved, &stage).unwrap(),
                        }
                    }
                }
                Ok(())
            });
            assert!(result.is_err());
            assert!(!fixture.target.exists());
            assert!(std::fs::symlink_metadata(&stage).is_ok());
            assert_eq!(std::fs::read(&saved).unwrap(), b"complete trace\n");
            drop(peer);
            drop(listener);
        }
    }
    #[test]
    fn trace_stage_bytes_and_postrename_inode_drift_fail_closed() {
        for phase in [
            Phase::BeforeFileSync,
            Phase::AfterFileSync,
            Phase::BeforeRename,
            Phase::AfterRename,
            Phase::BeforeDirectorySync,
            Phase::AfterDirectorySync,
        ] {
            let fixture = Fixture::new();
            let output = fixture.output();
            let mut fired = false;
            let result = output.publish_with_hook(|event| {
                if event == phase && !fired {
                    fired = true;
                    let path = if matches!(
                        phase,
                        Phase::BeforeFileSync | Phase::AfterFileSync | Phase::BeforeRename
                    ) {
                        fixture.stage()
                    } else {
                        fixture.target.clone()
                    };
                    std::fs::write(path, b"changed bytes with a distinct size").unwrap();
                }
                Ok(())
            });
            assert!(fired);
            assert!(result.is_err(), "byte drift {phase:?}");
        }
        let fixture = Fixture::new();
        let output = fixture.output();
        let result = output.publish_with_hook(|phase| {
            if phase == Phase::AfterRename {
                std::fs::rename(&fixture.target, fixture.target.with_extension("saved")).unwrap();
                std::fs::write(&fixture.target, b"replacement").unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"replacement");
    }
    #[test]
    fn trace_sync_faults_keep_failed_evidence_and_never_clobber_destinations() {
        for phase in [
            Phase::BeforeFileSync,
            Phase::AfterFileSync,
            Phase::BeforeRename,
            Phase::AfterRename,
            Phase::BeforeDirectorySync,
            Phase::AfterDirectorySync,
        ] {
            let fixture = Fixture::new();
            let output = fixture.output();
            let result = output.publish_with_hook(|event| {
                if event == phase {
                    return Err(eyre!("injected owner I/O boundary"));
                }
                Ok(())
            });
            assert!(result.is_err());
            if matches!(
                phase,
                Phase::BeforeFileSync | Phase::AfterFileSync | Phase::BeforeRename
            ) {
                assert!(!fixture.target.exists());
                assert!(fixture.stage().is_file());
            } else {
                assert_eq!(std::fs::read(&fixture.target).unwrap(), b"complete trace\n");
                assert!(!fixture.stage().exists());
            }
        }
    }
    #[test]
    fn trace_allocation_bounds_and_failed_creation_preserve_partial_diagnostics() {
        let fixture = Fixture::new();
        assert!(TraceOutput::create(&fixture.target, 0).is_err());
        assert!(
            TraceOutput::create(&fixture.target, super::super::super::MAX_FILE_BYTES + 1).is_err()
        );
        assert!(!fixture.stage().exists());
        let mut exact = TraceOutput::create(&fixture.target, 3).unwrap();
        exact.file_mut().write_all(b"abc").unwrap();
        exact.publish().unwrap();
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"abc");
        let fixture = Fixture::new();
        let mut over = TraceOutput::create(&fixture.target, 2).unwrap();
        over.file_mut().write_all(b"abc").unwrap();
        assert!(over.publish().is_err());
        assert!(!fixture.target.exists());
        assert_eq!(std::fs::read(fixture.stage()).unwrap(), b"abc");
        let fixture = Fixture::new();
        let failed = TraceOutput::create_with_hook(&fixture.target, 10, |phase| {
            if phase == Phase::AfterCreate {
                return Err(eyre!("partial setup"));
            }
            Ok(())
        });
        assert!(failed.is_err());
        assert!(fixture.stage().is_file());
        assert!(TraceOutput::create(&fixture.target, 10).is_err());
        assert_eq!(std::fs::metadata(fixture.stage()).unwrap().len(), 0);
    }
    #[test]
    fn trace_final_check_rename_source_race_is_rejected_without_unowned_cleanup() {
        let fixture = Fixture::new();
        let output = fixture.output();
        let saved = fixture.parent.join("saved-owned");
        let result = output.publish_with_hook(|phase| {
            if phase == Phase::RenameReady {
                std::fs::rename(fixture.stage(), &saved).unwrap();
                std::fs::write(fixture.stage(), b"unowned racer").unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        // NOREPLACE cannot condition its source on the original FD. A raced name
        // may move, but it is rejected and never removed as if it were ours.
        assert_eq!(std::fs::read(&fixture.target).unwrap(), b"unowned racer");
        assert_eq!(std::fs::read(saved).unwrap(), b"complete trace\n");
        assert!(!fixture.stage().exists());
    }
    #[test]
    fn trace_stage_recreation_after_rename_fails_and_preserves_the_new_name() {
        for boundary in [
            Phase::AfterRename,
            Phase::BeforeDirectorySync,
            Phase::AfterDirectorySync,
        ] {
            let fixture = Fixture::new();
            let output = fixture.output();
            let result = output.publish_with_hook(|phase| {
                if phase == boundary {
                    std::fs::write(fixture.stage(), b"new stage racer").unwrap();
                }
                Ok(())
            });
            assert!(result.is_err());
            assert_eq!(std::fs::read(fixture.stage()).unwrap(), b"new stage racer");
            assert_eq!(std::fs::read(&fixture.target).unwrap(), b"complete trace\n");
        }
    }
    #[test]
    fn output_allows_canonical_system_parent_without_claiming_directory_ownership() {
        // The system /tmp may be root-owned; only the new file must belong to us.
        // The short unique directory protects the chosen sibling spelling from
        // another test's namespace without changing process CWD or umask.
        let fixture = Fixture::new();
        let canonical = fixture._directory.path().canonicalize().unwrap();
        let target = canonical.with_extension("journal");
        let parent = target.parent().unwrap();
        let before = std::fs::metadata(parent).unwrap();
        let mut file = new_owned_file(&target).unwrap();
        assert_eq!(
            file.metadata().unwrap().uid(),
            rustix::process::geteuid().as_raw()
        );
        assert_eq!(file.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(file.metadata().unwrap().nlink(), 1);
        assert_eq!(std::fs::metadata(parent).unwrap().uid(), before.uid());
        assert_eq!(std::fs::metadata(parent).unwrap().gid(), before.gid());
        file.write_all(b"system parent journal\n").unwrap();
        file.sync_all().unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"system parent journal\n");
        drop(file);
        // This is the fixture's exact newly-created unique file, not owner cleanup.
        std::fs::remove_file(&target).unwrap();
    }
}
