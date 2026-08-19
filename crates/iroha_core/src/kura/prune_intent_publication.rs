// No current root-level publication owns the generic atomic-temporary
// namespace. Rejecting it here prevents an unbound artifact from evading exact
// prune-intent recovery and disk accounting.
const FORBIDDEN_ROOT_ATOMIC_TEMP_PREFIX: &str = ".kura-sidecar-";
/// One authenticated canonical-prune publication name.
#[derive(Debug)]
struct CanonicalPruneIntentArtifact {
    path: PathBuf,
    metadata: std::fs::Metadata,
    file: std::fs::File,
    bytes: Vec<u8>,
    intent: KuraPruneIntentV3,
    links: u64,
}
/// Exact allowlisted canonical-prune publication inventory.
#[derive(Debug, Default)]
struct CanonicalPruneIntentArtifactInventory {
    stable: Option<CanonicalPruneIntentArtifact>,
    temporary: Option<CanonicalPruneIntentArtifact>,
}
impl CanonicalPruneIntentArtifactInventory {
    fn tracked_bytes(&self) -> Result<u64> {
        // `tempfile::persist_noclobber` may crash after creating the stable
        // hard link but before unlinking the exact temporary name. Both names
        // are authenticated below, while disk accounting counts that one
        // physical inode once so the 4 KiB maintenance reserve remains exact.
        if let (Some(stable), Some(temporary)) = (&self.stable, &self.temporary)
            && Self::same_physical_object(stable, temporary)
        {
            return Ok(stable.metadata.len());
        }
        self.stable
            .iter()
            .chain(self.temporary.iter())
            .try_fold(0_u64, |total, artifact| {
                total.checked_add(artifact.metadata.len()).ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "canonical prune-intent disk accounting overflowed".to_owned(),
                    )
                })
            })
    }
    fn same_physical_object(
        stable: &CanonicalPruneIntentArtifact,
        temporary: &CanonicalPruneIntentArtifact,
    ) -> bool {
        Kura::sidecar_metadata_same_object(&stable.metadata, &temporary.metadata)
    }
}
impl Kura {
    fn prune_intent_path_for(store_root: &Path) -> PathBuf {
        store_root.join(PRUNE_INTENT_FILE_NAME)
    }
    fn decode_prune_intent(path: &Path, bytes: &[u8]) -> Result<KuraPruneIntentV3> {
        if bytes.is_empty() || bytes.len() > PRUNE_INTENT_MAX_BYTES {
            return Err(Error::PruneIntentConflict(format!(
                "intent {} has invalid byte length {}",
                path.display(),
                bytes.len()
            )));
        }
        let intent = norito::decode_canonical::<KuraPruneIntentV3>(bytes).map_err(|err| {
            Error::PruneIntentConflict(format!(
                "intent {} failed exact Norito decode: {err}",
                path.display()
            ))
        })?;
        if intent.version != 3
            || intent.target_height > intent.source_height
            || (intent.source_height == 0) != intent.source_tip_hash.is_none()
            || (intent.target_height == 0) != intent.target_tip_hash.is_none()
            || (intent.retained_merge_entries == 0) != intent.retained_merge_tip_hash.is_none()
            || !intent.sidecar_rewrite.is_canonical()
            || !intent.capacity.is_canonical(intent.sidecar_rewrite)
            || (intent.target_height == intent.source_height
                && (intent.target_tip_hash != intent.source_tip_hash
                    || !intent.sidecar_rewrite.has_work()))
        {
            return Err(Error::PruneIntentConflict(format!(
                "intent {} has a non-canonical identity",
                path.display()
            )));
        }
        Ok(intent)
    }
    fn read_prune_intent(store_root: &Path) -> Result<Option<KuraPruneIntentV3>> {
        Self::recover_canonical_prune_intent_artifacts(store_root)
    }
    fn persist_prune_intent(&self, intent: &KuraPruneIntentV3) -> Result<()> {
        let bytes = norito::encode_canonical(intent).map_err(Error::NoritoFrame)?;
        if bytes.is_empty() || bytes.len() > PRUNE_INTENT_MAX_BYTES {
            return Err(Error::PruneIntentConflict(
                "encoded prune intent exceeds its hard byte limit".to_owned(),
            ));
        }
        if Self::decode_prune_intent(&Self::prune_intent_path_for(&self.store_root), &bytes)?
            != *intent
        {
            return Err(Error::PruneIntentConflict(
                "encoded prune intent failed exact V3 roundtrip validation".to_owned(),
            ));
        }
        // The prune caller preflights the maintenance reserve before it takes
        // `block_data`; repeating that pending-block snapshot here would invert
        // the canonical lock order. Track cleanup/publication itself under the
        // aggregate disk-usage mutation guard.
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let before =
            Self::canonical_prune_intent_artifact_inventory(&self.store_root)?.tracked_bytes()?;
        if Self::recover_canonical_prune_intent_artifacts(&self.store_root)?.is_some() {
            let after = Self::canonical_prune_intent_artifact_inventory(&self.store_root)?
                .tracked_bytes()?;
            self.update_disk_usage_delta(before, after);
            self.prune_recovery_required.store(true, Ordering::Release);
            accounting_mutation.finish();
            return Err(Error::PruneIntentConflict(
                "another prune intent is already active".to_owned(),
            ));
        }
        let after_recovery =
            Self::canonical_prune_intent_artifact_inventory(&self.store_root)?.tracked_bytes()?;
        self.update_disk_usage_delta(before, after_recovery);
        if let Err(error) = self.publish_canonical_prune_intent_exact(intent, &bytes) {
            if std::fs::symlink_metadata(Self::prune_intent_path_for(&self.store_root)).is_ok()
                || std::fs::symlink_metadata(Self::prune_intent_temp_path_for(&self.store_root))
                    .is_ok()
            {
                self.prune_recovery_required.store(true, Ordering::Release);
            }
            return Err(error);
        }
        let after_publication =
            Self::canonical_prune_intent_artifact_inventory(&self.store_root)?.tracked_bytes()?;
        self.update_disk_usage_delta(after_recovery, after_publication);
        self.prune_recovery_required.store(true, Ordering::Release);
        accounting_mutation.finish();
        Ok(())
    }
    fn clear_prune_intent(&self) -> Result<()> {
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let before =
            Self::canonical_prune_intent_artifact_inventory(&self.store_root)?.tracked_bytes()?;
        let _ = Self::recover_canonical_prune_intent_artifacts(&self.store_root)?;
        let inventory = Self::canonical_prune_intent_artifact_inventory(&self.store_root)?;
        if let Some(stable) = inventory.stable.as_ref() {
            let namespace = Self::canonical_prune_intent_namespace_for(&self.store_root)?;
            Self::remove_canonical_prune_intent_artifact(&namespace, stable)?;
            Self::sync_canonical_prune_intent_root(&namespace)?;
        }
        let after = Self::canonical_prune_intent_artifact_inventory(&self.store_root)?;
        if after.stable.is_some() || after.temporary.is_some() {
            return Err(Error::PruneIntentConflict(
                "canonical prune-intent artifacts remain after exact clearance".to_owned(),
            ));
        }
        self.update_disk_usage_delta(before, 0);
        accounting_mutation.finish();
        Ok(())
    }
    fn finish_prune_intent(&self) -> Result<()> {
        self.clear_prune_intent()?;
        self.prune_recovery_required.store(false, Ordering::Release);
        Ok(())
    }
    fn block_hash_from_store(
        block_store: &mut BlockStore,
        height: u64,
    ) -> Result<Option<HashOf<BlockHeader>>> {
        if height == 0 || block_store.read_durable_index_count()? < height {
            return Ok(None);
        }
        Ok(block_store
            .read_block_hashes(height.saturating_sub(1), 1)?
            .into_iter()
            .next())
    }
    fn apply_prune_intent_to_block_store(
        block_store: &mut BlockStore,
        intent: &KuraPruneIntentV3,
    ) -> Result<()> {
        let current = block_store.read_durable_index_count()?;
        if current != intent.source_height && current != intent.target_height {
            return Err(Error::PruneIntentConflict(format!(
                "durable block height {current} matches neither prune source {} nor target {}",
                intent.source_height, intent.target_height
            )));
        }
        let expected_tip = if current == intent.source_height {
            intent.source_tip_hash
        } else {
            intent.target_tip_hash
        };
        if Self::block_hash_from_store(block_store, current)? != expected_tip {
            return Err(Error::PruneIntentConflict(format!(
                "durable block tip at height {current} differs from the prune intent"
            )));
        }
        if intent.target_height > 0
            && Self::block_hash_from_store(block_store, intent.target_height)?
                != intent.target_tip_hash
        {
            return Err(Error::PruneIntentConflict(
                "durable target hash differs from the prune intent".to_owned(),
            ));
        }
        // Re-run the idempotent block-store prune even when the target marker
        // was already published. A crash immediately after that marker can
        // leave the index/data/hash or DA-sidecar stages incomplete; the
        // marker alone must not cause forward recovery to skip those stages.
        block_store.prune(intent.target_height)?;
        Ok(())
    }
    /// Configured-capacity bytes kept available for a canonical prune intent.
    ///
    /// Normal publications and reservations include this once. Canonical prune
    /// publication itself consumes this reserve and therefore does not add a
    /// second copy of it to its capacity projection.
    const fn canonical_prune_intent_maintenance_headroom_bytes() -> u64 {
        PRUNE_INTENT_MAX_BYTES as u64
    }
    fn prune_intent_temp_path_for(store_root: &Path) -> PathBuf {
        store_root.join(PRUNE_INTENT_TEMP_FILE_NAME)
    }
    fn invalid_canonical_prune_intent_artifact(
        path: &Path,
        detail: impl std::fmt::Display,
    ) -> Error {
        Error::PruneIntentConflict(format!(
            "canonical prune-intent artifact {} {detail}",
            path.display()
        ))
    }
    fn canonical_prune_intent_namespace_for(store_root: &Path) -> Result<BoundProgressNamespace> {
        let stable = Self::prune_intent_path_for(store_root);
        let temporary = Self::prune_intent_temp_path_for(store_root);
        let directory = Self::open_bound_progress_directory(store_root, store_root)?;
        Ok(BoundProgressNamespace {
            data_path: stable,
            index_path: temporary,
            directories: vec![directory],
        })
    }
    #[cfg(unix)]
    fn canonical_prune_intent_metadata_unchanged(
        left: &std::fs::Metadata,
        right: &std::fs::Metadata,
        links: u64,
    ) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        Self::sidecar_metadata_same_object(left, right)
            && left.nlink() == links
            && right.nlink() == links
            && left.len() == right.len()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(windows)]
    fn canonical_prune_intent_metadata_unchanged(
        left: &std::fs::Metadata,
        right: &std::fs::Metadata,
        links: u64,
    ) -> bool {
        use std::os::windows::fs::MetadataExt as _;
        u32::try_from(links).ok().is_some_and(|links| {
            Self::sidecar_metadata_same_object(left, right)
                && left.number_of_links() == Some(links)
                && right.number_of_links() == Some(links)
                && left.file_size() == right.file_size()
                && left.last_write_time() == right.last_write_time()
                && left.creation_time() == right.creation_time()
        })
    }
    #[cfg(all(not(unix), not(windows)))]
    fn canonical_prune_intent_metadata_unchanged(
        _left: &std::fs::Metadata,
        _right: &std::fs::Metadata,
        _links: u64,
    ) -> bool {
        false
    }
    fn canonical_prune_intent_link_count(metadata: &std::fs::Metadata) -> Option<u64> {
        if Self::sidecar_has_link_count(metadata, 1) {
            Some(1)
        } else if Self::sidecar_has_link_count(metadata, 2) {
            Some(2)
        } else {
            None
        }
    }
    fn validate_canonical_prune_intent_reserved_names(
        store_root: &Path,
        namespace: &BoundProgressNamespace,
    ) -> Result<()> {
        let entries = std::fs::read_dir(store_root)
            .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
            let name = entry.file_name();
            let allowed = name == std::ffi::OsStr::new(PRUNE_INTENT_FILE_NAME)
                || name == std::ffi::OsStr::new(PRUNE_INTENT_TEMP_FILE_NAME);
            let name_lossy = name.to_string_lossy();
            if !allowed
                && (name_lossy.starts_with(PRUNE_INTENT_FILE_NAME)
                    || name_lossy.starts_with(FORBIDDEN_ROOT_ATOMIC_TEMP_PREFIX))
            {
                return Err(Self::invalid_canonical_prune_intent_artifact(
                    &entry.path(),
                    "uses an unexpected reserved publication name",
                ));
            }
        }
        if !Self::progress_mutation_namespace_unchanged(namespace) {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                store_root,
                "root identity changed during reserved-name validation",
            ));
        }
        Ok(())
    }
    fn read_canonical_prune_intent_artifact(
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<CanonicalPruneIntentArtifact>> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            Self::invalid_canonical_prune_intent_artifact(
                path,
                "has no descriptor-bound root namespace",
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                "escapes its descriptor-bound root namespace",
            ));
        }
        let before = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        };
        if before.file_type().is_symlink() || !before.file_type().is_file() {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                "is not a regular no-follow file",
            ));
        }
        let Some(links) = Self::canonical_prune_intent_link_count(&before) else {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                "has an unexpected hard-link count",
            ));
        };
        if before.len() == 0
            || before.len() > u64::try_from(PRUNE_INTENT_MAX_BYTES).unwrap_or(u64::MAX)
        {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                format_args!("has invalid byte length {}", before.len()),
            ));
        }
        let name = path.file_name().ok_or_else(|| {
            Self::invalid_canonical_prune_intent_artifact(path, "has no direct entry name")
        })?;
        #[cfg(unix)]
        let mut file = std::fs::File::from(
            rustix::fs::openat(
                &immediate.file,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?,
        );
        #[cfg(not(unix))]
        let mut file = {
            let mut options = std::fs::OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
                options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
            }
            options
                .open(path)
                .map_err(|error| Error::IO(error, path.to_path_buf()))?
        };
        let opened_before = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !opened_before.is_file()
            || !Self::canonical_prune_intent_metadata_unchanged(&before, &opened_before, links)
        {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                "changed while being opened without following links",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        (&mut file)
            .take(u64::try_from(PRUNE_INTENT_MAX_BYTES).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened_after = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let after = std::fs::symlink_metadata(path)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if bytes.len() != usize::try_from(before.len()).unwrap_or(usize::MAX)
            || !Self::canonical_prune_intent_metadata_unchanged(&before, &opened_after, links)
            || !Self::canonical_prune_intent_metadata_unchanged(&opened_after, &after, links)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                path,
                "changed during authenticated bounded read",
            ));
        }
        let intent = Self::decode_prune_intent(path, &bytes)?;
        Ok(Some(CanonicalPruneIntentArtifact {
            path: path.to_path_buf(),
            metadata: after,
            file,
            bytes,
            intent,
            links,
        }))
    }
    fn canonical_prune_intent_artifact_inventory(
        store_root: &Path,
    ) -> Result<CanonicalPruneIntentArtifactInventory> {
        let namespace = Self::canonical_prune_intent_namespace_for(store_root)?;
        Self::validate_canonical_prune_intent_reserved_names(store_root, &namespace)?;
        let stable_path = Self::prune_intent_path_for(store_root);
        let temporary_path = Self::prune_intent_temp_path_for(store_root);
        let stable = Self::read_canonical_prune_intent_artifact(&namespace, &stable_path)?;
        let temporary = Self::read_canonical_prune_intent_artifact(&namespace, &temporary_path)?;
        match (&stable, &temporary) {
            (None, None) => {}
            (Some(stable), None) if stable.links == 1 => {}
            (None, Some(temporary)) if temporary.links == 1 => {}
            (Some(stable), Some(temporary))
                if stable.links == 2
                    && temporary.links == 2
                    && Self::sidecar_metadata_same_object(
                        &stable.metadata,
                        &temporary.metadata,
                    )
                    && stable.bytes == temporary.bytes
                    && stable.intent == temporary.intent => {}
            (Some(stable), Some(temporary)) => {
                return Err(Error::PruneIntentConflict(format!(
                    "canonical prune-intent stable {} and temporary {} artifacts are not one authenticated two-link publication object",
                    stable.path.display(),
                    temporary.path.display()
                )));
            }
            (Some(stable), None) => {
                return Err(Self::invalid_canonical_prune_intent_artifact(
                    &stable.path,
                    "is a multiply-linked lone stable artifact",
                ));
            }
            (None, Some(temporary)) => {
                return Err(Self::invalid_canonical_prune_intent_artifact(
                    &temporary.path,
                    "is a multiply-linked lone temporary artifact",
                ));
            }
        }
        Ok(CanonicalPruneIntentArtifactInventory { stable, temporary })
    }
    fn remove_canonical_prune_intent_artifact(
        namespace: &BoundProgressNamespace,
        artifact: &CanonicalPruneIntentArtifact,
    ) -> Result<()> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            Self::invalid_canonical_prune_intent_artifact(
                &artifact.path,
                "has no descriptor-bound root namespace for removal",
            )
        })?;
        let name = artifact.path.file_name().ok_or_else(|| {
            Self::invalid_canonical_prune_intent_artifact(
                &artifact.path,
                "has no direct entry name for removal",
            )
        })?;
        let opened = artifact
            .file
            .metadata()
            .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        if !Self::canonical_prune_intent_metadata_unchanged(
            &artifact.metadata,
            &opened,
            artifact.links,
        ) {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &artifact.path,
                "changed before exact-object removal",
            ));
        }
        #[cfg(unix)]
        {
            let current =
                rustix::fs::statat(&immediate.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .map_err(|error| Error::IO(error, artifact.path.clone()))?;
            use std::os::unix::fs::MetadataExt as _;
            if rustix::fs::FileType::from_raw_mode(current.st_mode)
                != rustix::fs::FileType::RegularFile
                || current.st_dev as u64 != opened.dev()
                || current.st_ino as u64 != opened.ino()
                || current.st_nlink as u64 != artifact.links
            {
                return Err(Self::invalid_canonical_prune_intent_artifact(
                    &artifact.path,
                    "changed before descriptor-relative removal",
                ));
            }
            rustix::fs::unlinkat(&immediate.file, name, rustix::fs::AtFlags::empty())
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        }
        #[cfg(not(unix))]
        {
            let current = std::fs::symlink_metadata(&artifact.path)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
            if current.file_type().is_symlink()
                || !current.is_file()
                || !Self::canonical_prune_intent_metadata_unchanged(
                    &artifact.metadata,
                    &current,
                    artifact.links,
                )
            {
                return Err(Self::invalid_canonical_prune_intent_artifact(
                    &artifact.path,
                    "changed before no-follow removal",
                ));
            }
            std::fs::remove_file(&artifact.path)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        }
        Ok(())
    }
    fn sync_canonical_prune_intent_root(namespace: &BoundProgressNamespace) -> Result<()> {
        let root = namespace.directories.first().ok_or_else(|| {
            Error::PruneIntentConflict(
                "canonical prune-intent publication has no bound Kura root".to_owned(),
            )
        })?;
        root.file
            .sync_all()
            .map_err(|error| Error::IO(error, root.expected_path.clone()))?;
        if !Self::progress_mutation_namespace_unchanged(namespace) {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &root.expected_path,
                "root identity changed during directory synchronization",
            ));
        }
        Ok(())
    }
    fn recover_canonical_prune_intent_artifacts(
        store_root: &Path,
    ) -> Result<Option<KuraPruneIntentV3>> {
        let inventory = Self::canonical_prune_intent_artifact_inventory(store_root)?;
        match (&inventory.stable, &inventory.temporary) {
            (None, None) => Ok(None),
            (Some(stable), None) => Ok(Some(stable.intent.clone())),
            (None, Some(temporary)) => {
                let namespace = Self::canonical_prune_intent_namespace_for(store_root)?;
                Self::remove_canonical_prune_intent_artifact(&namespace, temporary)?;
                Self::sync_canonical_prune_intent_root(&namespace)?;
                let after = Self::canonical_prune_intent_artifact_inventory(store_root)?;
                if after.stable.is_some() || after.temporary.is_some() {
                    return Err(Self::invalid_canonical_prune_intent_artifact(
                        &temporary.path,
                        "did not disappear after unpublished-temp recovery",
                    ));
                }
                Ok(None)
            }
            (Some(stable), Some(temporary)) => {
                let expected = stable.intent.clone();
                let expected_bytes = stable.bytes.clone();
                let namespace = Self::canonical_prune_intent_namespace_for(store_root)?;
                Self::remove_canonical_prune_intent_artifact(&namespace, temporary)?;
                Self::sync_canonical_prune_intent_root(&namespace)?;
                let after = Self::canonical_prune_intent_artifact_inventory(store_root)?;
                let Some(after_stable) = after.stable else {
                    return Err(Self::invalid_canonical_prune_intent_artifact(
                        &stable.path,
                        "disappeared while normalizing the no-clobber crash window",
                    ));
                };
                if after.temporary.is_some()
                    || after_stable.links != 1
                    || after_stable.intent != expected
                    || after_stable.bytes != expected_bytes
                {
                    return Err(Self::invalid_canonical_prune_intent_artifact(
                        &after_stable.path,
                        "did not become the exact single-link stable object after crash recovery",
                    ));
                }
                Ok(Some(expected))
            }
        }
    }
    fn publish_canonical_prune_intent_exact(
        &self,
        intent: &KuraPruneIntentV3,
        bytes: &[u8],
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        let stable_path = Self::prune_intent_path_for(&self.store_root);
        let temporary_path = Self::prune_intent_temp_path_for(&self.store_root);
        let namespace = Self::canonical_prune_intent_namespace_for(&self.store_root)?;
        let inventory = Self::canonical_prune_intent_artifact_inventory(&self.store_root)?;
        if inventory.stable.is_some() || inventory.temporary.is_some() {
            return Err(Error::PruneIntentConflict(
                "another canonical prune-intent publication is already active".to_owned(),
            ));
        }
        let temporary_file = Self::create_new_bound_progress_temp(&namespace, &temporary_path)
            .map_err(|error| Error::IO(error, temporary_path.clone()))?;
        let temporary_token = tempfile::TempPath::try_from_path(&temporary_path)
            .map_err(|error| Error::IO(error, temporary_path.clone()))?;
        let mut temporary = tempfile::NamedTempFile::from_parts(temporary_file, temporary_token);
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.flush())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| Error::IO(error, temporary_path.clone()))?;
        let written = Self::canonical_prune_intent_artifact_inventory(&self.store_root)?;
        if written.stable.is_some()
            || written.temporary.as_ref().is_none_or(|artifact| {
                artifact.links != 1 || artifact.bytes != bytes || artifact.intent != *intent
            })
        {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &temporary_path,
                "failed exact readback after temporary fsync",
            ));
        }
        #[cfg(test)]
        if self
            .fail_next_atomic_write_after_temporary_sync
            .swap(false, Ordering::Relaxed)
        {
            let (_file, kept_path) = temporary
                .keep()
                .map_err(|error| Error::IO(error.error, temporary_path.clone()))?;
            return Err(Error::IO(
                std::io::Error::other(
                    "injected canonical prune-intent failure after exact temporary fsync and before no-clobber promotion",
                ),
                kept_path,
            ));
        }
        let persisted = match temporary.persist_noclobber(&stable_path) {
            Ok(file) => file,
            Err(error) => {
                let kind = error.error.kind();
                let publication_error = error.error;
                error
                    .file
                    .close()
                    .map_err(|cleanup| Error::IO(cleanup, temporary_path.clone()))?;
                Self::sync_canonical_prune_intent_root(&namespace)?;
                if kind == ErrorKind::AlreadyExists {
                    return Err(Error::PruneIntentConflict(
                        "another canonical prune intent won no-clobber publication".to_owned(),
                    ));
                }
                return Err(Error::IO(publication_error, stable_path));
            }
        };
        persisted
            .sync_all()
            .map_err(|error| Error::IO(error, stable_path.clone()))?;
        Self::sync_canonical_prune_intent_root(&namespace)?;
        let recovered = Self::recover_canonical_prune_intent_artifacts(&self.store_root)?;
        if recovered.as_ref() != Some(intent) {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &stable_path,
                "does not read back as the exact published stable intent",
            ));
        }
        Ok(())
    }
    fn prune_indexed_sidecar_has_exact_temp_residue(
        data_path: &Path,
        index_path: &Path,
        kind: &'static str,
    ) -> Result<bool> {
        let mut present = false;
        for path in [
            data_path.with_extension("norito.tmp"),
            index_path.with_extension("index.tmp"),
        ] {
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink()
                        || !metadata.file_type().is_file()
                        || !Self::sidecar_is_single_link(&metadata)
                    {
                        return Err(Error::PruneIntentConflict(format!(
                            "{kind} temporary path {} is not an exact single-link regular file",
                            path.display()
                        )));
                    }
                    present = true;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, path)),
            }
        }
        Ok(present)
    }
    fn project_prune_indexed_sidecar_pair(
        data_path: &Path,
        index_path: &Path,
        target_height: u64,
        kind: &'static str,
    ) -> Result<KuraPruneSidecarPairProjectionV3> {
        let Some(layout) = Self::validate_indexed_sidecar_pair(
            data_path,
            index_path,
            u64::MAX,
            kind,
            false,
            true,
        )?
        else {
            return Ok(KuraPruneSidecarPairProjectionV3::default());
        };
        let retained_entries = target_height
            .checked_sub(layout.base_height)
            .and_then(|relative| relative.checked_add(1))
            .unwrap_or(0)
            .min(layout.entry_count);
        let mut index = std::fs::File::open(index_path)
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
        index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
        let mut retained_data_bytes = 0_u64;
        let mut retained_is_compact = true;
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for _ in 0..retained_entries {
            index
                .read_exact(&mut entry_bytes)
                .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
            let entry = SidecarIndexEntry::from_bytes(entry_bytes);
            if entry.len == 0 {
                continue;
            }
            if entry.offset != retained_data_bytes {
                retained_is_compact = false;
            }
            retained_data_bytes = retained_data_bytes.checked_add(entry.len).ok_or_else(|| {
                Error::PruneIntentConflict(format!("{kind} retained payload projection overflowed"))
            })?;
        }
        let data_bytes = std::fs::symlink_metadata(data_path)
            .map_err(|error| Error::IO(error, data_path.to_path_buf()))?
            .len();
        let requires_rewrite = retained_entries != layout.entry_count
            || !retained_is_compact
            || retained_data_bytes != data_bytes;
        if !requires_rewrite {
            return Ok(KuraPruneSidecarPairProjectionV3::default());
        }
        let retained_index_bytes = retained_entries
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
            .and_then(|entries| entries.checked_add(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64))
            .ok_or_else(|| {
                Error::PruneIntentConflict(format!("{kind} retained index projection overflowed"))
            })?;
        let index_bytes = std::fs::symlink_metadata(index_path)
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?
            .len();
        if retained_data_bytes > data_bytes || retained_index_bytes > index_bytes {
            return Err(Error::PruneIntentConflict(format!(
                "{kind} retained rewrite projection would grow its canonical pair"
            )));
        }
        Ok(KuraPruneSidecarPairProjectionV3 {
            required: true,
            retained_data_bytes,
            retained_index_bytes,
        })
    }
    /// Reconcile at most one sequential rewrite crash residue and project the
    /// exact remaining retained data/index pair. The caller holds
    /// `sidecar_lock` throughout reconciliation and projection.
    fn reconcile_and_project_prune_sidecar_rewrites_locked(
        &self,
        target_height: u64,
    ) -> Result<KuraPruneSidecarRewriteProjectionV3> {
        let directory = self.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
        let pipeline_data = directory.join(PIPELINE_SIDECARS_DATA_FILE);
        let pipeline_index = directory.join(PIPELINE_SIDECARS_INDEX_FILE);
        Self::prune_indexed_sidecar_has_exact_temp_residue(
            &pipeline_data,
            &pipeline_index,
            "pipeline recovery sidecar",
        )?;
        self.reconcile_prune_indexed_sidecar_temps(
            &pipeline_data,
            &pipeline_index,
            target_height,
            "pipeline recovery sidecar",
        )?;
        let pipeline = Self::project_prune_indexed_sidecar_pair(
            &pipeline_data,
            &pipeline_index,
            target_height,
            "pipeline recovery sidecar",
        )?;
        let sequential_peak_bytes = pipeline.temp_pair_bytes().ok_or_else(|| {
            Error::PruneIntentConflict(
                "canonical sidecar rewrite peak projection overflowed".to_owned(),
            )
        })?;
        let projection = KuraPruneSidecarRewriteProjectionV3 {
            pipeline,
            sequential_peak_bytes,
        };
        if !projection.is_canonical() {
            return Err(Error::PruneIntentConflict(
                "canonical sidecar rewrite projection is internally inconsistent".to_owned(),
            ));
        }
        Ok(projection)
    }
    fn canonical_prune_capacity_admission_snapshot(
        &self,
        pending_blocks: u64,
        marker_temporary_bytes: u64,
        marker_stable_growth_bytes: u64,
    ) -> Result<KuraPruneCapacityAdmissionV3> {
        let used = self.kura_disk_usage_bytes()?;
        let post_wsv = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
        let certified_bundles = self.certified_bundle_capacity_reserved_bytes()?;
        let autonomous_terminals = self.autonomous_global_terminal_outcome_reserved_bytes()?;
        Ok(KuraPruneCapacityAdmissionV3 {
            source_physical_bytes: used,
            pending_canonical_bytes: pending_blocks,
            post_wsv_reserved_bytes: post_wsv,
            certified_bundle_reserved_bytes: certified_bundles,
            autonomous_terminal_reserved_bytes: autonomous_terminals,
            intent_bytes: 0,
            marker_temporary_bytes,
            marker_stable_growth_bytes,
            admitted_peak_bytes: 0,
        })
    }
    fn canonical_prune_commit_marker_projection(&self, target_height: u64) -> Result<(u64, u64)> {
        let mut store = self.block_store.lock();
        let _ = store.read_commit_marker()?;
        let marker = store.commit_marker_for_count(target_height)?;
        let marker_bytes = norito::encode_canonical(&marker).map_err(Error::NoritoFrame)?;
        let marker_temporary_bytes = u64::try_from(marker_bytes.len())?;
        if marker_temporary_bytes == 0
            || marker_temporary_bytes
                > u64::try_from(MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES).unwrap_or(u64::MAX)
        {
            return Err(Error::PruneIntentConflict(
                "canonical prune marker exceeds its encoded hard bound".to_owned(),
            ));
        }
        let marker_path = store.commit_marker_path();
        let marker_stable_bytes = match std::fs::symlink_metadata(&marker_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.file_type().is_file()
                    || !Self::sidecar_is_single_link(&metadata)
                    || metadata.len()
                        > u64::try_from(MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES).unwrap_or(u64::MAX)
                {
                    return Err(Error::PruneIntentConflict(
                        "canonical block marker is not a bounded single-link regular file"
                            .to_owned(),
                    ));
                }
                metadata.len()
            }
            Err(error) if error.kind() == ErrorKind::NotFound => 0,
            Err(error) => return Err(Error::IO(error, marker_path)),
        };
        let temporary_path = marker_path.with_extension("norito.tmp");
        match std::fs::symlink_metadata(&temporary_path) {
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(Error::PruneIntentConflict(
                    "canonical block marker retains an unresolved deterministic temporary"
                        .to_owned(),
                ));
            }
            Err(error) => return Err(Error::IO(error, temporary_path)),
        }
        Ok((
            marker_temporary_bytes,
            marker_temporary_bytes.saturating_sub(marker_stable_bytes),
        ))
    }
    fn seal_and_validate_canonical_prune_capacity_admission(
        &self,
        mut intent: KuraPruneIntentV3,
    ) -> Result<KuraPruneIntentV3> {
        let provisional = norito::encode_canonical(&intent).map_err(Error::NoritoFrame)?;
        let intent_bytes = u64::try_from(provisional.len())?;
        if intent_bytes == 0 || intent_bytes > PRUNE_INTENT_MAX_BYTES as u64 {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &self.store_root,
                "encoded V3 admission exceeds the prune-intent hard bound",
            ));
        }
        intent.capacity.intent_bytes = intent_bytes;
        intent.capacity.admitted_peak_bytes = intent
            .capacity
            .required_peak_bytes(intent.sidecar_rewrite)
            .ok_or_else(|| {
                Self::invalid_canonical_prune_intent_artifact(
                    &self.store_root,
                    "configured publication-capacity accounting overflowed",
                )
            })?;
        let final_bytes = norito::encode_canonical(&intent).map_err(Error::NoritoFrame)?;
        if u64::try_from(final_bytes.len())? != intent_bytes
            || !intent.capacity.is_canonical(intent.sidecar_rewrite)
        {
            return Err(Self::invalid_canonical_prune_intent_artifact(
                &self.store_root,
                "capacity admission does not have a stable canonical encoding",
            ));
        }
        if self.max_disk_usage_bytes > 0
            && intent.capacity.admitted_peak_bytes > self.max_disk_usage_bytes
        {
            return Err(Error::StorageBudgetExceeded {
                limit: self.max_disk_usage_bytes,
                used: intent.capacity.source_physical_bytes,
                required: intent.capacity.admitted_peak_bytes,
            });
        }
        Ok(intent)
    }
    /// Recheck the exact remaining authenticated stages without consuming the
    /// capacity envelopes that were outstanding when the intent was admitted.
    fn validate_recovered_prune_capacity(
        &self,
        intent: &KuraPruneIntentV3,
        remaining_sidecar: KuraPruneSidecarRewriteProjectionV3,
    ) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let used = self.kura_disk_usage_bytes()?;
        let required = intent
            .capacity
            .remaining_required_bytes(used, remaining_sidecar)
            .ok_or_else(|| {
                Self::invalid_canonical_prune_intent_artifact(
                    &self.store_root,
                    "recovered prune-capacity accounting overflowed",
                )
            })?;
        if required > self.max_disk_usage_bytes {
            return Err(Error::StorageBudgetExceeded {
                limit: self.max_disk_usage_bytes,
                used,
                required,
            });
        }
        Ok(())
    }
}
