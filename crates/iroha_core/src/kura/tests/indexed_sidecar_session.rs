// Scoped real storage adapters for the generic component controls.

#[derive(Default)]
pub(super) struct IndexedSidecarSession {
    #[cfg(windows)]
    native: Option<super::native_sidecar_transaction::NativeSidecarTransaction>,
}

pub(super) fn sidecar_test_directory() -> TempDir {
    let temporary = TempDir::new().unwrap();
    #[cfg(windows)]
    {
        // This is only the fresh empty fixture, never an existing shipping root.
        fs::remove_dir(temporary.path()).unwrap();
        drop(iroha_fs::PrivateDirectory::open_or_create(temporary.path()).unwrap());
    }
    temporary
}

pub(super) fn sidecar_test_write(
    path: impl AsRef<Path>,
    bytes: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    #[cfg(not(windows))]
    {
        fs::write(path, bytes)
    }
    #[cfg(windows)]
    {
        let path = path.as_ref();
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("fixture has no parent"))?;
        let name = path
            .file_name()
            .ok_or_else(|| std::io::Error::other("fixture has no leaf"))?;
        iroha_fs::PrivateDirectory::open(parent)?.write_atomic(
            name,
            bytes.as_ref(),
            iroha_fs::PublishMode::Replace,
        )
    }
}

fn sidecar_test_create(path: impl AsRef<Path>) -> std::io::Result<std::fs::File> {
    #[cfg(not(windows))]
    {
        std::fs::File::create(path)
    }
    #[cfg(windows)]
    {
        let path = path.as_ref();
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("fixture has no parent"))?;
        let name = path
            .file_name()
            .ok_or_else(|| std::io::Error::other("fixture has no leaf"))?;
        let directory = iroha_fs::OwnerDirectory::open(parent)?;
        let mut slot = None;
        directory.create_private_journal_into(name, &mut slot)?;
        let file = slot.ok_or_else(|| std::io::Error::other("fixture original missing"))?;
        directory.require_same_lock(name, &file)?;
        Ok(file)
    }
}

#[cfg_attr(
    not(windows),
    allow(
        clippy::unused_self,
        reason = "the same test API retains native retry ownership while Unix delegates unchanged path operations"
    )
)]
impl IndexedSidecarSession {
    #[cfg(windows)]
    fn run(
        &mut self,
        data: &Path,
        index: &Path,
        action: impl FnOnce(
            &mut super::native_sidecar_transaction::NativeSidecarTransaction,
        ) -> std::io::Result<()>,
    ) -> bool {
        if self.native.is_none() {
            let result = (|| {
                let root = data
                    .parent()
                    .ok_or_else(|| std::io::Error::other("sidecar has no parent"))?;
                if index.parent() != Some(root) {
                    return Err(std::io::Error::other("sidecar parents differ"));
                }
                let directory = iroha_fs::OwnerDirectory::open(root)?;
                let original = Kura::bind_native_progress_directory(root, root, directory)
                    .map_err(std::io::Error::other)?;
                super::native_sidecar_transaction::NativeSidecarTransaction::capture(
                    BoundProgressNamespace {
                        data_path: data.to_path_buf(),
                        index_path: index.to_path_buf(),
                        directories: vec![original],
                    },
                )
            })();
            match result {
                Ok(owner) => self.native = Some(owner),
                Err(_) => return false,
            }
        }
        let owner = self.native.as_mut().expect("captured native session");
        if !owner.matches(data, index) {
            return false;
        }
        if action(owner).is_err() {
            return false;
        }
        self.native = None;
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_indexed_sidecar(
        &mut self,
        data: &Path,
        index: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
        fsync: FsyncMode,
        retention: Option<NonZeroUsize>,
    ) -> bool {
        #[cfg(not(windows))]
        {
            Kura::append_indexed_sidecar(data, index, height, payload, kind, fsync, retention)
        }
        #[cfg(windows)]
        {
            self.run(data, index, |owner| {
                owner.append_with_retention(
                    height,
                    payload,
                    matches!(fsync, FsyncMode::Always),
                    retention,
                    kind,
                )
            })
        }
    }

    fn recover_indexed_sidecar_artifacts(&mut self, data: &Path, index: &Path, kind: &str) -> bool {
        #[cfg(not(windows))]
        {
            Kura::recover_indexed_sidecar_artifacts(data, index, kind)
        }
        #[cfg(windows)]
        {
            let _ = kind;
            self.run(data, index, |owner| owner.recover())
        }
    }

    fn recover_indexed_sidecar_artifacts_with_required_heights(
        &mut self,
        data: &Path,
        index: &Path,
        required: &BTreeSet<u64>,
        kind: &str,
    ) -> bool {
        #[cfg(not(windows))]
        {
            Kura::recover_indexed_sidecar_artifacts_with_required_heights(
                data, index, required, kind,
            )
        }
        #[cfg(windows)]
        {
            let _ = kind;
            self.run(data, index, |owner| owner.recover_required(required))
        }
    }

    fn prune_indexed_sidecars(
        &mut self,
        data: &Path,
        index: &Path,
        retention: NonZeroUsize,
        kind: &str,
    ) -> bool {
        self.prune_indexed_sidecars_with_pinned_height(data, index, retention, None, kind)
    }

    fn prune_indexed_sidecars_with_pinned_height(
        &mut self,
        data: &Path,
        index: &Path,
        retention: NonZeroUsize,
        pinned: Option<u64>,
        kind: &str,
    ) -> bool {
        #[cfg(not(windows))]
        {
            Kura::prune_indexed_sidecars_with_pinned_height(data, index, retention, pinned, kind)
        }
        #[cfg(windows)]
        {
            self.run(data, index, |owner| {
                owner.rewrite(
                    IndexedSidecarRewrite::Newest {
                        retention,
                        pinned_height: pinned,
                    },
                    kind,
                )
            })
        }
    }

    pub(super) fn prune_indexed_sidecars_to_retention_window(
        &mut self,
        data: &Path,
        index: &Path,
        retention: NonZeroUsize,
        kind: &str,
    ) -> bool {
        #[cfg(not(windows))]
        {
            Kura::prune_indexed_sidecars_to_retention_window(data, index, retention, kind)
        }
        #[cfg(windows)]
        {
            self.run(data, index, |owner| {
                owner.rewrite(IndexedSidecarRewrite::NewestWindow { retention }, kind)
            })
        }
    }

    fn prune_indexed_sidecars_through_terminal_frontier(
        &mut self,
        data: &Path,
        index: &Path,
        terminal: u64,
        retention: NonZeroUsize,
        kind: &str,
    ) -> bool {
        self.prune_indexed_sidecars_through_terminal_frontier_with_required_heights(
            data,
            index,
            terminal,
            retention,
            &BTreeSet::new(),
            kind,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "preserve the canonical policy arguments in the scoped storage adapter"
    )]
    fn prune_indexed_sidecars_through_terminal_frontier_with_required_heights(
        &mut self,
        data: &Path,
        index: &Path,
        terminal: u64,
        retention: NonZeroUsize,
        required: &BTreeSet<u64>,
        kind: &str,
    ) -> bool {
        #[cfg(not(windows))]
        {
            Kura::prune_indexed_sidecars_through_terminal_frontier_with_required_heights(
                data, index, terminal, retention, required, kind,
            )
        }
        #[cfg(windows)]
        {
            self.run(data, index, |owner| {
                owner.rewrite(
                    IndexedSidecarRewrite::AfterTerminalFrontier {
                        terminal_height: terminal,
                        retention,
                        required_heights: required,
                    },
                    kind,
                )
            })
        }
    }
}

#[cfg(windows)]
mod native_generic_session_tests {
    use super::*;

    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            super::super::native_sidecar_transaction::set_rewrite_index_create_failure_for_tests(
                false,
            );
            FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC.with(|flag| flag.set(false));
        }
    }

    fn append_retention_retry(index_creation_failure: bool) {
        let _reset = Reset;
        let directory = sidecar_test_directory();
        let data = directory.path().join("retention.norito");
        let index = directory.path().join("retention.index");
        let mut session = IndexedSidecarSession::default();
        assert!(session.append_indexed_sidecar(
            &data,
            &index,
            1,
            b"old",
            "retention",
            FsyncMode::Always,
            None
        ));
        if index_creation_failure {
            super::super::native_sidecar_transaction::set_rewrite_index_create_failure_for_tests(
                true,
            );
        } else {
            fail_next_sidecar_temp_marker_dir_sync_for_tests();
        }
        let retention = NonZeroUsize::new(1);
        assert!(!session.append_indexed_sidecar(
            &data,
            &index,
            2,
            b"kept",
            "retention",
            FsyncMode::Always,
            retention
        ));
        assert_eq!(fs::read(&data).unwrap(), b"oldkept");
        let temporary_data = data.with_extension("norito.tmp");
        let retained =
            iroha_fs::FileIdentity::of(&std::fs::File::open(&temporary_data).unwrap()).unwrap();
        assert!(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&temporary_data)
                .is_err()
        );
        assert!(!session.append_indexed_sidecar(
            &data,
            &index,
            2,
            b"different",
            "retention",
            FsyncMode::Always,
            retention
        ));
        assert_eq!(fs::read(&data).unwrap(), b"oldkept");
        assert!(session.append_indexed_sidecar(
            &data,
            &index,
            2,
            b"kept",
            "retention",
            FsyncMode::Always,
            retention
        ));
        assert_eq!(
            fs::read(&data).unwrap(),
            b"kept",
            "completed append must not append its payload again"
        );
        assert_eq!(
            iroha_fs::FileIdentity::of(&std::fs::File::open(&data).unwrap()).unwrap(),
            retained
        );
        assert!(!temporary_data.exists());
        assert!(!index.with_extension("index.tmp").exists());
        assert!(session.append_indexed_sidecar(
            &data,
            &index,
            3,
            b"next",
            "retention",
            FsyncMode::Always,
            retention
        ));
        assert_eq!(fs::read(&data).unwrap(), b"next");
    }

    #[test]
    fn native_session_append_retention_resumes_partial_creator_without_duplicate_payload() {
        append_retention_retry(true);
    }

    #[test]
    fn native_session_append_retention_resumes_ready_marker_without_duplicate_payload() {
        append_retention_retry(false);
    }
}
