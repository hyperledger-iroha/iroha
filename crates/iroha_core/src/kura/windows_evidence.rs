// Windows evidence publication through original Kura directory and creator capabilities.

impl Kura {
    fn bind_native_progress_directory(
        store_root: &Path,
        expected_path: &Path,
        native_owner: iroha_fs::OwnerDirectory,
    ) -> Result<BoundProgressDirectory> {
        use std::os::windows::fs::OpenOptionsExt as _;
        let io_error = |error| Error::IO(error, expected_path.to_path_buf());
        native_owner.revalidate().map_err(io_error)?;
        let (canonical_path, _) = Self::canonical_sidecar_directory_for(store_root, expected_path)?
            .ok_or_else(|| io_error(std::io::Error::from(ErrorKind::NotFound)))?;
        // This descriptor only observes the directory pinned by the original owner.
        // It never replaces the retained native authority with a freshly resolved path.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x0020_0000 | 0x0200_0000)
            .open(expected_path)
            .map_err(io_error)?;
        let observed = iroha_fs::FileMetadata::of(&file).map_err(io_error)?;
        if !observed.is_dir() || observed.identity() != native_owner.identity().map_err(io_error)? {
            return Err(io_error(std::io::Error::other(
                "native progress observer differs from its original owner",
            )));
        }
        let metadata = secure_file_metadata::from_file(&file).map_err(io_error)?;
        native_owner.revalidate().map_err(io_error)?;
        Ok(BoundProgressDirectory {
            expected_path: expected_path.to_path_buf(),
            canonical_path,
            entry_name: None,
            file,
            metadata,
            native_owner,
        })
    }

    fn open_or_create_bound_storage_child_directory(
        &self,
        parent: &BoundProgressDirectory,
        name: &std::ffi::OsStr,
    ) -> Result<BoundProgressDirectory> {
        let expected_path = parent.expected_path.join(name);
        if !self.bound_storage_directory_unchanged(parent) {
            return Err(Self::invalid_lane_artifact_error(
                parent.expected_path.clone(),
                "native storage parent changed before creation",
            ));
        }
        let owner = parent
            .native_owner
            .ensure_child(name)
            .map_err(|error| Error::IO(error, expected_path.clone()))?;
        let mut child =
            Self::bind_native_progress_directory(&self.store_root, &expected_path, owner)?;
        child.entry_name = Some(name.to_os_string());
        parent
            .native_owner
            .sync()
            .map_err(|error| Error::IO(error, parent.expected_path.clone()))?;
        if !self.bound_storage_directory_unchanged(parent) {
            return Err(Self::invalid_lane_artifact_error(
                parent.expected_path.clone(),
                "native storage parent changed during creation",
            ));
        }
        Ok(child)
    }

    fn native_evidence_parent<'a>(
        namespace: &'a BoundProgressNamespace,
        path: &Path,
    ) -> Result<&'a iroha_fs::OwnerDirectory> {
        let immediate = namespace
            .directories
            .first()
            .filter(|directory| {
                path.parent() == Some(directory.expected_path.as_path())
                    && path.file_name().is_some()
            })
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "native evidence path escapes its original directory",
                )
            })?;
        if !Self::progress_mutation_namespace_unchanged(namespace) {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "native evidence namespace changed",
            ));
        }
        Ok(&immediate.native_owner)
    }

    fn native_evidence_original(
        namespace: &BoundProgressNamespace,
        path: &Path,
        file: &std::fs::File,
    ) -> Result<iroha_fs::FileSnapshot> {
        let owner = Self::native_evidence_parent(namespace, path)?;
        let io_error = |error| Error::IO(error, path.to_path_buf());
        let snapshot = iroha_fs::FileSnapshot::of(file, true).map_err(io_error)?;
        owner
            .require_same_lock(path.file_name().expect("native leaf"), file)
            .map_err(io_error)?;
        if iroha_fs::FileSnapshot::of(file, true).map_err(io_error)? != snapshot {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "native evidence original changed",
            ));
        }
        Ok(snapshot)
    }

    fn read_bound_regular_file_bytes_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        max_bytes: usize,
        kind: &str,
    ) -> Result<Option<Vec<u8>>> {
        let owner = Self::native_evidence_parent(namespace, path)?;
        let io_error = |error| Error::IO(error, path.to_path_buf());
        let name = path.file_name().expect("native parent validated the leaf");
        let Some(mut original) = owner
            .open_retained_regular_optional(name)
            .map_err(io_error)?
        else {
            Self::native_evidence_parent(namespace, path)?;
            return Ok(None);
        };
        let before = iroha_fs::FileSnapshot::of(original.file(), true).map_err(io_error)?;
        let length = original.file().metadata().map_err(io_error)?.len();
        let length = usize::try_from(length)?;
        if length == 0 || length > max_bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has an empty or oversized payload"),
            ));
        }
        let mut bytes = Vec::with_capacity(length);
        original
            .file_mut()
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        original.revalidate().map_err(io_error)?;
        if bytes.len() != length
            || iroha_fs::FileSnapshot::of(original.file(), true).map_err(io_error)? != before
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed while reading its original"),
            ));
        }
        Self::native_evidence_parent(namespace, path)?;
        original.revalidate().map_err(io_error)?;
        Ok(Some(bytes))
    }

    fn publish_bound_noclobber_file_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        temp_path: &Path,
        bytes: &[u8],
        kind: &str,
    ) -> Result<bool> {
        self.durable_mutation_authorized()?;
        let owner = Self::native_evidence_parent(namespace, path)?;
        Self::native_evidence_parent(namespace, temp_path)?;
        if self
            .read_bound_regular_file_bytes_locked(namespace, path, bytes.len().max(1), kind)?
            .is_some()
        {
            return Ok(false);
        }
        let temp_name = temp_path
            .file_name()
            .expect("native parent validated the leaf");
        let target_name = path.file_name().expect("native parent validated the leaf");
        // CREATE_NEW refuses unresolved temporaries without reopening them as writers.
        let mut pending = owner
            .create_retained_private(temp_name, bytes.len().max(1))
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
        #[cfg(test)]
        if let Some(prefix_len) =
            FAIL_AFTER_NEXT_BOUND_EVIDENCE_TEMP_PREFIX.with(|flag| flag.take())
        {
            assert!(
                prefix_len < bytes.len(),
                "crash cut must precede the complete artifact"
            );
            pending
                .write_all(&bytes[..prefix_len])
                .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
            return Err(Error::IO(
                std::io::Error::other(
                    "injected Native evidence interruption during temporary write",
                ),
                temp_path.to_path_buf(),
            ));
        }
        pending
            .write_all(bytes)
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
        let original = pending
            .finish_private()
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
        let original_snapshot = original
            .snapshot()
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
        #[cfg(test)]
        if FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_SYNC.with(|flag| flag.replace(false)) {
            return Err(Error::IO(
                std::io::Error::other(
                    "injected Native evidence interruption after temporary fsync",
                ),
                temp_path.to_path_buf(),
            ));
        }
        let mut published = false;
        let result = original.move_no_replace_observed(owner, target_name, |step| {
            if step == iroha_fs::FileMoveStep::Published {
                // Set before any fallible snapshot, namespace check or durability hook.
                published = true;
            }
            Ok(())
        });
        let mut moved = match result {
            Ok(moved) => moved,
            Err(error) => {
                if !published {
                    let _ = owner.remove_private_if_snapshot(temp_name, &original_snapshot);
                }
                return Err(Error::IO(error, path.to_path_buf()));
            }
        };
        // The creator's exact original handle survives publication and readback. Its
        // retained write/delete access deliberately excludes a second writable opener.
        moved
            .rewind()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let mut readback = Vec::with_capacity(bytes.len());
        std::io::Read::by_ref(&mut moved)
            .take(u64::try_from(bytes.len())?.saturating_add(1))
            .read_to_end(&mut readback)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if readback != bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed during publication"),
            ));
        }
        moved
            .require_source_absent()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.sync_bound_evidence_namespace(namespace, kind)?;
        moved
            .snapshot()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        moved
            .require_source_absent()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        Self::native_evidence_parent(namespace, path)?;
        Ok(true)
    }
}
