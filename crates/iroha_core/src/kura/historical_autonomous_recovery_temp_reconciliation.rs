const HISTORICAL_AUTONOMOUS_RECOVERY_PUBLICATION_MAX_ARTIFACTS: usize =
    HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS * 2;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HistoricalAutonomousRecoveryPublicationIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u32, index: u64 },
    #[cfg(not(unix))]
    Synthetic(usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoricalAutonomousRecoveryPublicationKind {
    Stable,
    Temporary,
}
struct HistoricalAutonomousRecoveryPublicationSnapshot {
    entry: LaneConfigEntry,
    directory: PathBuf,
    path: PathBuf,
    stable_path: PathBuf,
    metadata: SecureMetadata,
    record: HistoricalAutonomousLaneRecoveryRecordV1,
    links: u64,
    identity: HistoricalAutonomousRecoveryPublicationIdentity,
    kind: HistoricalAutonomousRecoveryPublicationKind,
}
struct BoundHistoricalAutonomousRecoveryPublicationArtifact {
    path: PathBuf,
    metadata: SecureMetadata,
    file: std::fs::File,
    record: HistoricalAutonomousLaneRecoveryRecordV1,
    links: u64,
}
struct HistoricalAutonomousRecoveryPublicationDirectorySnapshot {
    path: PathBuf,
    metadata: SecureMetadata,
}
struct HistoricalAutonomousRecoveryPublicationInventory {
    artifacts: Vec<HistoricalAutonomousRecoveryPublicationSnapshot>,
    directories: Vec<HistoricalAutonomousRecoveryPublicationDirectorySnapshot>,
    temporary_indices: Vec<usize>,
    stable_by_path: BTreeMap<PathBuf, usize>,
}
impl Kura {
    fn historical_autonomous_recovery_publication_kind(
        name: &std::ffi::OsStr,
    ) -> Option<HistoricalAutonomousRecoveryPublicationKind> {
        if historical_autonomous_recovery_record_name_is_canonical(name) {
            return Some(HistoricalAutonomousRecoveryPublicationKind::Stable);
        }
        let name = name.to_str()?;
        name.strip_prefix(HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX)
            .is_some_and(|suffix| !suffix.is_empty())
            .then_some(HistoricalAutonomousRecoveryPublicationKind::Temporary)
    }
    fn historical_autonomous_recovery_publication_link_count(
        metadata: &SecureMetadata,
    ) -> Option<u64> {
        if Self::sidecar_has_link_count(metadata, 1) {
            Some(1)
        } else if Self::sidecar_has_link_count(metadata, 2) {
            Some(2)
        } else {
            None
        }
    }
    fn historical_autonomous_recovery_publication_identity(
        metadata: &SecureMetadata,
        synthetic_index: usize,
    ) -> HistoricalAutonomousRecoveryPublicationIdentity {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let _ = synthetic_index;
            HistoricalAutonomousRecoveryPublicationIdentity::Unix {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
        #[cfg(windows)]
        {
            match (metadata.volume_serial_number(), metadata.file_index()) {
                (Some(volume), Some(index)) => {
                    HistoricalAutonomousRecoveryPublicationIdentity::Windows { volume, index }
                }
                _ => HistoricalAutonomousRecoveryPublicationIdentity::Synthetic(synthetic_index),
            }
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = metadata;
            HistoricalAutonomousRecoveryPublicationIdentity::Synthetic(synthetic_index)
        }
    }
    #[cfg(unix)]
    fn historical_autonomous_recovery_publication_metadata_unchanged(
        left: &SecureMetadata,
        right: &SecureMetadata,
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
    fn historical_autonomous_recovery_publication_metadata_unchanged(
        left: &SecureMetadata,
        right: &SecureMetadata,
        links: u64,
    ) -> bool {
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
    fn historical_autonomous_recovery_publication_metadata_unchanged(
        _left: &SecureMetadata,
        _right: &SecureMetadata,
        _links: u64,
    ) -> bool {
        false
    }
    fn read_bound_historical_autonomous_recovery_publication_artifact(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<BoundHistoricalAutonomousRecoveryPublicationArtifact>> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication namespace has no immediate directory",
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact escapes its bound directory",
            ));
        }
        let before = match secure_file_metadata::from_path(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        };
        if before.file_type().is_symlink() || !before.file_type().is_file() {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact is not a regular no-follow file",
            ));
        }
        let Some(links) = Self::historical_autonomous_recovery_publication_link_count(&before)
        else {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact has an unexpected hard-link count",
            ));
        };
        if before.len() == 0
            || before.len()
                > u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES).unwrap_or(u64::MAX)
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact is empty or oversized",
            ));
        }
        let _name = path.file_name().ok_or_else(|| {
            Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact has no direct entry name",
            )
        })?;
        #[cfg(unix)]
        let mut file = std::fs::File::from(
            rustix::fs::openat(
                &immediate.file,
                _name,
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
        let opened_before = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !opened_before.is_file()
            || !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                &before,
                &opened_before,
                links,
            )
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact changed while opening",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len())?);
        std::io::Read::by_ref(&mut file)
            .take(u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)?.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened_after = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let after = secure_file_metadata::from_path(path)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if bytes.len() != usize::try_from(before.len())?
            || !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                &before,
                &opened_after,
                links,
            )
            || !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                &opened_after,
                &after,
                links,
            )
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact changed during bounded read",
            ));
        }
        let mut cursor = bytes.as_slice();
        let record =
            HistoricalAutonomousLaneRecoveryRecordV1::decode_all(&mut cursor).map_err(|error| {
                Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    format!(
                        "historical recovery publication artifact is not exact Norito: {error}"
                    ),
                )
            })?;
        if historical_autonomous_recovery_record_bytes(&record) != bytes {
            return Err(Self::invalid_historical_autonomous_recovery(
                path.to_path_buf(),
                "historical recovery publication artifact is noncanonical or unsupported",
            ));
        }
        self.validate_historical_autonomous_recovery_record_shape(&record, path)?;
        Ok(Some(BoundHistoricalAutonomousRecoveryPublicationArtifact {
            path: path.to_path_buf(),
            metadata: after,
            file,
            record,
            links,
        }))
    }
    fn historical_autonomous_recovery_publication_snapshot_unchanged(
        snapshot: &HistoricalAutonomousRecoveryPublicationSnapshot,
        current: &BoundHistoricalAutonomousRecoveryPublicationArtifact,
    ) -> bool {
        snapshot.path == current.path
            && snapshot.links == current.links
            && snapshot.record == current.record
            && Self::historical_autonomous_recovery_publication_metadata_unchanged(
                &snapshot.metadata,
                &current.metadata,
                snapshot.links,
            )
    }
    fn validate_historical_autonomous_recovery_publication_inventory_collisions(
        &self,
        artifacts: &[HistoricalAutonomousRecoveryPublicationSnapshot],
    ) -> Result<usize> {
        let mut by_recovery = BTreeMap::new();
        let mut by_slot = BTreeMap::new();
        let mut by_proposal = BTreeMap::new();
        let mut by_entrypoint = BTreeMap::new();
        for artifact in artifacts {
            let record = &artifact.record;
            let descriptor = &record.payload.origin_proposal.descriptor;
            let record_hash = HashOf::new(record);
            let slot = (
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
            );
            if by_recovery
                .insert(record.recovery_id, record_hash)
                .is_some_and(|existing| existing != record_hash)
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication inventory aliases one recovery ID to different bytes",
                ));
            }
            for conflict in [
                by_slot.insert(slot, record.recovery_id),
                by_proposal.insert(
                    record.payload.origin_proposal.proposal_hash,
                    record.recovery_id,
                ),
            ] {
                if conflict.is_some_and(|existing| existing != record.recovery_id) {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        artifact.path.clone(),
                        "historical recovery publication inventory has a slot or proposal collision",
                    ));
                }
            }
            for key in &record.reservation_group.ordered_keys {
                if by_entrypoint
                    .insert(key.entrypoint_hash, record.recovery_id)
                    .is_some_and(|existing| existing != record.recovery_id)
                {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        artifact.path.clone(),
                        "historical recovery publication inventory aliases FIFO transaction ownership",
                    ));
                }
            }
        }
        Ok(by_recovery.len())
    }
    fn validate_historical_autonomous_recovery_publication_aliases(
        &self,
        artifacts: &[HistoricalAutonomousRecoveryPublicationSnapshot],
        aggregate_byte_limit: u64,
    ) -> Result<()> {
        struct Aliases {
            first: usize,
            second: Option<usize>,
            count: usize,
        }
        let mut aliases =
            BTreeMap::<HistoricalAutonomousRecoveryPublicationIdentity, Aliases>::new();
        let mut unique_bytes = 0_u64;
        for (index, artifact) in artifacts.iter().enumerate() {
            match aliases.entry(artifact.identity) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    unique_bytes = unique_bytes
                        .checked_add(artifact.metadata.len())
                        .filter(|bytes| *bytes <= aggregate_byte_limit)
                        .ok_or_else(|| {
                            Self::invalid_historical_autonomous_recovery(
                                artifact.directory.clone(),
                                "historical recovery publication bytes exceed their aggregate bound",
                            )
                        })?;
                    slot.insert(Aliases {
                        first: index,
                        second: None,
                        count: 1,
                    });
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    let aliases = slot.get_mut();
                    aliases.count = aliases.count.saturating_add(1);
                    if aliases.second.is_none() {
                        aliases.second = Some(index);
                    }
                }
            }
        }
        for (index, artifact) in artifacts.iter().enumerate() {
            let aliases = aliases.get(&artifact.identity).ok_or_else(|| {
                Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication identity disappeared during validation",
                )
            })?;
            if artifact.links == 1 {
                if aliases.count != 1 {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        artifact.path.clone(),
                        "single-link historical recovery publication artifact aliases another path",
                    ));
                }
                continue;
            }
            let Some(second) = aliases.second.filter(|_| aliases.count == 2) else {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication hard links are not one exact stable/temporary pair",
                ));
            };
            let other_index = if aliases.first == index {
                second
            } else if second == index {
                aliases.first
            } else {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication hard-link identity is ambiguous",
                ));
            };
            let other = &artifacts[other_index];
            let (stable, temporary) = match (artifact.kind, other.kind) {
                (
                    HistoricalAutonomousRecoveryPublicationKind::Stable,
                    HistoricalAutonomousRecoveryPublicationKind::Temporary,
                ) => (artifact, other),
                (
                    HistoricalAutonomousRecoveryPublicationKind::Temporary,
                    HistoricalAutonomousRecoveryPublicationKind::Stable,
                ) => (other, artifact),
                _ => {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        artifact.path.clone(),
                        "historical recovery publication hard links do not pair one stable and one temporary name",
                    ));
                }
            };
            if stable.links != 2
                || temporary.links != 2
                || stable.directory != temporary.directory
                || temporary.stable_path != stable.path
                || stable.record != temporary.record
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication hard links do not bind one exact target record",
                ));
            }
        }
        Ok(())
    }
    fn revalidate_historical_autonomous_recovery_publication_inventory(
        &self,
        inventory: &HistoricalAutonomousRecoveryPublicationInventory,
    ) -> Result<()> {
        for directory in &inventory.directories {
            let current = secure_file_metadata::from_path(&directory.path)
                .map_err(|error| Error::IO(error, directory.path.clone()))?;
            if current.file_type().is_symlink()
                || !current.file_type().is_dir()
                || !Self::sidecar_directory_metadata_unchanged(&directory.metadata, &current)
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    directory.path.clone(),
                    "historical recovery publication directory changed during whole-inventory preflight",
                ));
            }
        }
        for artifact in &inventory.artifacts {
            let current = secure_file_metadata::from_path(&artifact.path)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
            if !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                &artifact.metadata,
                &current,
                artifact.links,
            ) {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication artifact changed after whole-inventory preflight",
                ));
            }
        }
        Ok(())
    }
    fn historical_autonomous_recovery_publication_inventory_locked(
        &self,
        entries: &[LaneConfigEntry],
    ) -> Result<HistoricalAutonomousRecoveryPublicationInventory> {
        let mut artifacts = Vec::new();
        let mut directories = Vec::new();
        let mut temporary_indices = Vec::new();
        let mut temporary_by_stable_path = BTreeMap::new();
        let mut stable_by_path = BTreeMap::new();
        for entry in entries {
            let directory =
                Self::historical_autonomous_recovery_directory_for_entry(entry, &self.store_root);
            if self.canonical_sidecar_directory(&directory)?.is_none() {
                continue;
            }
            let namespace_directory =
                Self::open_bound_progress_directory(&self.store_root, &directory)?;
            let namespace = BoundProgressNamespace {
                data_path: directory.clone(),
                index_path: directory.clone(),
                directories: vec![namespace_directory],
            };
            let directory_before = secure_file_metadata::from_path(&directory)
                .map_err(|error| Error::IO(error, directory.clone()))?;
            let mut paths = Vec::new();
            for child in std::fs::read_dir(&directory)
                .map_err(|error| Error::IO(error, directory.clone()))?
            {
                let child = child.map_err(|error| Error::IO(error, directory.clone()))?;
                if artifacts.len().saturating_add(paths.len())
                    == HISTORICAL_AUTONOMOUS_RECOVERY_PUBLICATION_MAX_ARTIFACTS
                {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        child.path(),
                        "historical recovery publication inventory exceeds its hard entry bound",
                    ));
                }
                paths.push(child.path());
            }
            paths.sort();
            for path in paths {
                let name = path.file_name().ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical recovery publication entry has no direct name",
                    )
                })?;
                let kind = Self::historical_autonomous_recovery_publication_kind(name).ok_or_else(
                    || {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical recovery directory contains an unknown, malformed, or ambiguous entry",
                        )
                    },
                )?;
                let bound = self
                    .read_bound_historical_autonomous_recovery_publication_artifact(
                        &namespace, &path,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical recovery publication artifact disappeared during inventory",
                        )
                    })?;
                let descriptor = &bound.record.payload.origin_proposal.descriptor;
                if descriptor.lane_id != entry.lane_id
                    || descriptor.dataspace_id != entry.dataspace_id
                {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "historical recovery publication artifact is stored in another route namespace",
                    ));
                }
                self.require_active_lane_artifact(entry, descriptor)?;
                let stable_path = Self::historical_autonomous_recovery_path_for_entry(
                    entry,
                    &self.store_root,
                    bound.record.recovery_id,
                );
                if kind == HistoricalAutonomousRecoveryPublicationKind::Stable
                    && bound.path != stable_path
                {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        bound.path,
                        "historical recovery stable name does not bind its embedded recovery ID",
                    ));
                }
                let artifact_index = artifacts.len();
                let identity = Self::historical_autonomous_recovery_publication_identity(
                    &bound.metadata,
                    artifact_index,
                );
                artifacts.push(HistoricalAutonomousRecoveryPublicationSnapshot {
                    entry: entry.clone(),
                    directory: directory.clone(),
                    path: bound.path,
                    stable_path: stable_path.clone(),
                    metadata: bound.metadata,
                    record: bound.record,
                    links: bound.links,
                    identity,
                    kind,
                });
                if kind == HistoricalAutonomousRecoveryPublicationKind::Temporary {
                    if temporary_by_stable_path
                        .insert(stable_path, artifact_index)
                        .is_some()
                    {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            directory.clone(),
                            "historical recovery publication inventory has multiple temporaries for one stable target",
                        ));
                    }
                    temporary_indices.push(artifact_index);
                } else if stable_by_path.insert(stable_path, artifact_index).is_some() {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        directory.clone(),
                        "historical recovery publication inventory contains duplicate stable targets",
                    ));
                }
            }
            let directory_after = secure_file_metadata::from_path(&directory)
                .map_err(|error| Error::IO(error, directory.clone()))?;
            if !Self::sidecar_directory_metadata_unchanged(&directory_before, &directory_after)
                || !Self::progress_mutation_namespace_unchanged(&namespace)
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    directory,
                    "historical recovery publication directory changed during bounded inventory",
                ));
            }
            directories.push(HistoricalAutonomousRecoveryPublicationDirectorySnapshot {
                path: namespace.directories[0].expected_path.clone(),
                metadata: directory_after,
            });
        }
        let aggregate_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
        self.validate_historical_autonomous_recovery_publication_aliases(
            &artifacts,
            aggregate_byte_limit,
        )?;
        let unique_records = self
            .validate_historical_autonomous_recovery_publication_inventory_collisions(&artifacts)?;
        if unique_records > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
            || temporary_indices.len() > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                self.store_root.clone(),
                "historical recovery publication inventory exceeds its logical record bound",
            ));
        }
        let inventory = HistoricalAutonomousRecoveryPublicationInventory {
            artifacts,
            directories,
            temporary_indices,
            stable_by_path,
        };
        self.revalidate_historical_autonomous_recovery_publication_inventory(&inventory)?;
        Ok(inventory)
    }
    fn remove_bound_historical_autonomous_recovery_publication_artifact(
        namespace: &BoundProgressNamespace,
        artifact: &BoundHistoricalAutonomousRecoveryPublicationArtifact,
    ) -> Result<()> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            Self::invalid_historical_autonomous_recovery(
                artifact.path.clone(),
                "historical recovery publication removal has no bound directory",
            )
        })?;
        if artifact.path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(Self::invalid_historical_autonomous_recovery(
                artifact.path.clone(),
                "historical recovery publication removal escapes its bound directory",
            ));
        }
        let _name = artifact.path.file_name().ok_or_else(|| {
            Self::invalid_historical_autonomous_recovery(
                artifact.path.clone(),
                "historical recovery publication removal target has no direct name",
            )
        })?;
        let opened = secure_file_metadata::from_file(&artifact.file)
            .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        if !Self::historical_autonomous_recovery_publication_metadata_unchanged(
            &artifact.metadata,
            &opened,
            artifact.links,
        ) {
            return Err(Self::invalid_historical_autonomous_recovery(
                artifact.path.clone(),
                "historical recovery publication artifact changed before exact-object removal",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let current = rustix::fs::statat(
                &immediate.file,
                _name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, artifact.path.clone()))?;
            if rustix::fs::FileType::from_raw_mode(current.st_mode)
                != rustix::fs::FileType::RegularFile
                || current.st_dev as u64 != opened.dev()
                || current.st_ino as u64 != opened.ino()
                || current.st_nlink as u64 != artifact.links
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication artifact changed before descriptor-relative removal",
                ));
            }
            rustix::fs::unlinkat(&immediate.file, _name, rustix::fs::AtFlags::empty())
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        }
        #[cfg(not(unix))]
        {
            let current = secure_file_metadata::from_path(&artifact.path)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
            if current.file_type().is_symlink()
                || !current.file_type().is_file()
                || !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                    &artifact.metadata,
                    &current,
                    artifact.links,
                )
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    artifact.path.clone(),
                    "historical recovery publication artifact changed before no-follow removal",
                ));
            }
            std::fs::remove_file(&artifact.path)
                .map_err(|error| Error::IO(error, artifact.path.clone()))?;
        }
        Ok(())
    }
    fn reconcile_one_historical_autonomous_recovery_publication_temporary_locked(
        &self,
        temporary: &HistoricalAutonomousRecoveryPublicationSnapshot,
        expected_stable: Option<&HistoricalAutonomousRecoveryPublicationSnapshot>,
    ) -> Result<()> {
        let namespace_directory =
            Self::open_bound_progress_directory(&self.store_root, &temporary.directory)?;
        let namespace = BoundProgressNamespace {
            data_path: temporary.path.clone(),
            index_path: temporary.stable_path.clone(),
            directories: vec![namespace_directory],
        };
        let current_temporary = self
            .read_bound_historical_autonomous_recovery_publication_artifact(
                &namespace,
                &temporary.path,
            )?
            .ok_or_else(|| {
                Self::invalid_historical_autonomous_recovery(
                    temporary.path.clone(),
                    "historical recovery publication temporary disappeared before reconciliation",
                )
            })?;
        if !Self::historical_autonomous_recovery_publication_snapshot_unchanged(
            temporary,
            &current_temporary,
        ) {
            return Err(Self::invalid_historical_autonomous_recovery(
                temporary.path.clone(),
                "historical recovery publication temporary changed after authenticated preflight",
            ));
        }
        let current_stable = self.read_bound_historical_autonomous_recovery_publication_artifact(
            &namespace,
            &temporary.stable_path,
        )?;
        if let Some(expected_stable) = expected_stable
            && current_stable.as_ref().is_none_or(|current| {
                !Self::historical_autonomous_recovery_publication_snapshot_unchanged(
                    expected_stable,
                    current,
                )
            })
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                temporary.stable_path.clone(),
                "historical recovery stable target changed after authenticated preflight",
            ));
        }
        match current_stable {
            None => {
                if current_temporary.links != 1 {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        temporary.path.clone(),
                        "unpublished historical recovery temporary is not single-link",
                    ));
                }
                Self::promote_bound_progress_temp_noreplace(
                    &namespace,
                    &temporary.path,
                    &temporary.stable_path,
                    &current_temporary.file,
                )
                .map_err(|error| Error::IO(error.source, temporary.stable_path.clone()))?;
            }
            Some(stable) => {
                if stable.record != current_temporary.record {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        temporary.stable_path.clone(),
                        "historical recovery publication temporary conflicts with its stable target",
                    ));
                }
                let same_object = Self::sidecar_metadata_same_object(
                    &stable.metadata,
                    &current_temporary.metadata,
                );
                if same_object {
                    if stable.links != 2 || current_temporary.links != 2 {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            temporary.path.clone(),
                            "historical recovery publication alias has an unexpected link count",
                        ));
                    }
                } else if stable.links != 1 || current_temporary.links != 1 {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        temporary.path.clone(),
                        "separate historical recovery publication duplicates are multiply linked",
                    ));
                }
                Self::remove_bound_historical_autonomous_recovery_publication_artifact(
                    &namespace,
                    &current_temporary,
                )?;
            }
        }
        if !Self::sync_bound_progress_mutation_directories(
            &namespace,
            "historical autonomous recovery publication reconciliation",
        ) {
            return Err(Self::invalid_historical_autonomous_recovery(
                temporary.directory.clone(),
                "historical recovery publication directory durability sync failed",
            ));
        }
        let confirmed = self
            .read_bound_historical_autonomous_recovery_publication_artifact(
                &namespace,
                &temporary.stable_path,
            )?
            .ok_or_else(|| {
                Self::invalid_historical_autonomous_recovery(
                    temporary.stable_path.clone(),
                    "historical recovery stable target is absent after reconciliation",
                )
            })?;
        if confirmed.links != 1 || confirmed.record != temporary.record {
            return Err(Self::invalid_historical_autonomous_recovery(
                temporary.stable_path.clone(),
                "historical recovery stable target failed exact post-reconciliation readback",
            ));
        }
        match std::fs::symlink_metadata(&temporary.path) {
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::IO(error, temporary.path.clone())),
            Ok(_) => Err(Self::invalid_historical_autonomous_recovery(
                temporary.path.clone(),
                "historical recovery temporary remains after reconciliation",
            )),
        }
    }
    /// Reconcile only authenticated historical autonomous recovery-seal
    /// temporaries. This namespace has one immutable payload type whose embedded
    /// recovery ID and active lane route derive the exact no-clobber target.
    /// Generic atomic temporaries in mixed-purpose directories deliberately
    /// remain fail-closed because their random name cannot recover that binding.
    fn reconcile_historical_autonomous_recovery_atomic_temps_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _historical_recovery_guard = self.historical_autonomous_recovery_mutation_lock.lock();
        let entries = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            self.lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let inventory = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            let _sidecar_guard = self.sidecar_lock.lock();
            self.historical_autonomous_recovery_publication_inventory_locked(&entries)?
        };
        if inventory.temporary_indices.is_empty() {
            return Ok(());
        }
        let mut authenticated = BTreeSet::new();
        for index in &inventory.temporary_indices {
            let temporary = &inventory.artifacts[*index];
            if authenticated.insert(temporary.record.recovery_id) {
                self.validate_historical_autonomous_recovery_dependencies(
                    &temporary.record,
                    &temporary.path,
                )?;
            }
        }
        self.revalidate_historical_autonomous_recovery_publication_inventory(&inventory)?;
        self.durable_mutation_authorized()?;
        // Dropping this guard invalidates both cached disk-usage views. Rename
        // preserves physical bytes, while exact-duplicate cleanup can remove a
        // complete inode; the next capacity/read request therefore performs one
        // authoritative rescan rather than applying a guessed logical delta.
        let mut accounting_mutation = self
            .begin_total_disk_usage_mutation()
            .with_resource_children(inventory.temporary_indices.len());
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        for index in &inventory.temporary_indices {
            let temporary = &inventory.artifacts[*index];
            self.require_active_lane_artifact(
                &temporary.entry,
                &temporary.record.payload.origin_proposal.descriptor,
            )?;
            let expected_stable = inventory
                .stable_by_path
                .get(&temporary.stable_path)
                .map(|index| &inventory.artifacts[*index]);
            let resource_child = accounting_mutation
                .resource_child(vec![temporary.path.clone(), temporary.stable_path.clone()]);
            self.reconcile_one_historical_autonomous_recovery_publication_temporary_locked(
                temporary,
                expected_stable,
            )?;
            resource_child.finish();
        }
        drop(_sidecar_guard);
        drop(_geometry_guard);
        let _ = self.historical_autonomous_lane_recovery_records_bounded_under_prune_guard(
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
        )?;
        accounting_mutation.finish_resources_before_disk_rescan();
        self.note_committed_lane_status_change();
        Ok(())
    }
}

/// Physical publication bounds shared by every lane in one disk-accounting scan.
/// Accounting admits an interrupted publication as bytes, never as application evidence.
struct HistoricalAutonomousRecoveryAccountingBudget {
    stable_remaining: usize,
    temporary_remaining: usize,
    bytes_remaining: u64,
}
impl HistoricalAutonomousRecoveryAccountingBudget {
    fn new(bytes_remaining: u64) -> Self {
        Self {
            stable_remaining: HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            temporary_remaining: HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            bytes_remaining,
        }
    }
}
impl Kura {
    /// Count a bounded publication namespace before authoritative secondary-lane recovery.
    ///
    /// Stable records and temporary publications have independent hard counts. A hard link is
    /// accepted only when both names are present in this directory and pair one stable name with
    /// one current temporary name. Their inode contributes bytes once. This metadata-only scan
    /// cannot make any record usable: ordinary readers and recovery retain canonical decoding,
    /// record-count, dependency, incarnation and whole-inventory checks before publication.
    fn historical_autonomous_recovery_publication_accounting_bytes(
        directory: &Path,
        budget: &mut HistoricalAutonomousRecoveryAccountingBudget,
    ) -> Result<u64> {
        if budget.stable_remaining > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
            || budget.temporary_remaining > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
            || budget.bytes_remaining > HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                directory.to_path_buf(),
                "historical publication accounting exceeds its hard bounds",
            ));
        }
        let before = secure_file_metadata::from_path(directory)
            .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
        if before.file_type().is_symlink() || !before.file_type().is_dir() {
            return Err(Self::invalid_historical_autonomous_recovery(
                directory.to_path_buf(),
                "historical publication accounting namespace is not a direct directory",
            ));
        }
        let mut stable_remaining = budget.stable_remaining;
        let mut temporary_remaining = budget.temporary_remaining;
        let mut physical_bytes = 0_u64;
        let mut files = Vec::new();
        let mut identities =
            BTreeMap::<HistoricalAutonomousRecoveryPublicationIdentity, Vec<usize>>::new();
        for entry in std::fs::read_dir(directory)
            .map_err(|error| Error::IO(error, directory.to_path_buf()))?
        {
            let entry = entry.map_err(|error| Error::IO(error, directory.to_path_buf()))?;
            let path = entry.path();
            let kind = Self::historical_autonomous_recovery_publication_kind(&entry.file_name())
                .filter(|_| path.parent() == Some(directory))
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical publication accounting has an unknown or malformed entry",
                    )
                })?;
            let remaining = match kind {
                HistoricalAutonomousRecoveryPublicationKind::Stable => &mut stable_remaining,
                HistoricalAutonomousRecoveryPublicationKind::Temporary => &mut temporary_remaining,
            };
            *remaining = remaining.checked_sub(1).ok_or_else(|| {
                Self::invalid_historical_autonomous_recovery(
                    path.clone(),
                    "historical publication accounting exceeds its per-kind file bound",
                )
            })?;
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || metadata.len() == 0
                || metadata.len() > u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)?
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical publication accounting entry is empty, non-regular or oversized",
                ));
            }
            let links = Self::historical_autonomous_recovery_publication_link_count(&metadata)
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical publication accounting has an unexpected hard-link count",
                    )
                })?;
            let identity =
                Self::historical_autonomous_recovery_publication_identity(&metadata, files.len());
            let aliases = identities.entry(identity).or_default();
            if aliases.is_empty() {
                physical_bytes = physical_bytes
                    .checked_add(metadata.len())
                    .filter(|bytes| *bytes <= budget.bytes_remaining)
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical publication accounting exceeds its physical-byte bound",
                        )
                    })?;
            }
            if aliases.len() >= 2 {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical publication accounting has ambiguous file aliases",
                ));
            }
            aliases.push(files.len());
            files.push((path, metadata, kind, links));
        }
        for aliases in identities.values() {
            let (_, first_metadata, first_kind, first_links) = &files[aliases[0]];
            let valid = match aliases.as_slice() {
                [_] => *first_links == 1,
                [_, second] => {
                    let (_, second_metadata, second_kind, second_links) = &files[*second];
                    *first_links == 2
                        && *second_links == 2
                        && first_kind != second_kind
                        && Self::historical_autonomous_recovery_publication_metadata_unchanged(
                            first_metadata,
                            second_metadata,
                            2,
                        )
                }
                _ => false,
            };
            if !valid {
                return Err(Self::invalid_historical_autonomous_recovery(
                    files[aliases[0]].0.clone(),
                    "historical publication accounting hard links are not one exact stable/temporary pair",
                ));
            }
        }
        let after = secure_file_metadata::from_path(directory)
            .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
        if after.file_type().is_symlink()
            || !after.file_type().is_dir()
            || !Self::sidecar_directory_metadata_unchanged(&before, &after)
        {
            return Err(Self::invalid_historical_autonomous_recovery(
                directory.to_path_buf(),
                "historical publication accounting directory changed during enumeration",
            ));
        }
        for (path, metadata, _, links) in &files {
            let current = secure_file_metadata::from_path(path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if !Self::historical_autonomous_recovery_publication_metadata_unchanged(
                metadata, &current, *links,
            ) {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.clone(),
                    "historical publication accounting entry changed during enumeration",
                ));
            }
        }
        budget.stable_remaining = stable_remaining;
        budget.temporary_remaining = temporary_remaining;
        budget.bytes_remaining -= physical_bytes;
        Ok(physical_bytes)
    }
}

#[cfg(test)]
mod historical_publication_accounting_tests {
    use super::*;

    #[test]
    fn accounting_bounds_each_kind_and_exact_physical_bytes_without_consuming_rejected_budget() {
        let temp = tempfile::tempdir().expect("accounting namespace");
        let stable = temp
            .path()
            .join(format!("{}.norito", "01".repeat(Hash::LENGTH)));
        let pending = temp.path().join(format!(
            "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}pending"
        ));
        std::fs::write(stable, [1_u8; 3]).expect("stable bytes");
        std::fs::write(pending, [2_u8; 4]).expect("pending bytes");
        for (stable_remaining, temporary_remaining, bytes_remaining, accepted) in [
            (1, 1, 7, true),
            (0, 1, 7, false),
            (1, 0, 7, false),
            (1, 1, 6, false),
        ] {
            let mut budget = HistoricalAutonomousRecoveryAccountingBudget {
                stable_remaining,
                temporary_remaining,
                bytes_remaining,
            };
            let result = Kura::historical_autonomous_recovery_publication_accounting_bytes(
                temp.path(),
                &mut budget,
            );
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(result.expect("exact accounting"), 7);
                assert_eq!(
                    (
                        budget.stable_remaining,
                        budget.temporary_remaining,
                        budget.bytes_remaining
                    ),
                    (0, 0, 0)
                );
            } else {
                assert_eq!(
                    (
                        budget.stable_remaining,
                        budget.temporary_remaining,
                        budget.bytes_remaining
                    ),
                    (stable_remaining, temporary_remaining, bytes_remaining)
                );
            }
        }
        let mut budget = HistoricalAutonomousRecoveryAccountingBudget::new(7);
        assert_eq!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                temp.path(),
                &mut budget
            )
            .expect("configured budget"),
            7
        );
        assert_eq!(
            budget.stable_remaining,
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS - 1
        );
    }

    #[test]
    fn accounting_accepts_only_an_internal_stable_temporary_hard_link_pair() {
        let temp = tempfile::tempdir().expect("accounting namespace");
        let outside = tempfile::tempdir().expect("external link directory");
        let stable = temp
            .path()
            .join(format!("{}.norito", "02".repeat(Hash::LENGTH)));
        let pending = temp.path().join(format!(
            "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}linked"
        ));
        std::fs::write(&stable, [3_u8; 7]).expect("stable bytes");
        std::fs::hard_link(&stable, &pending).expect("internal publication pair");
        let mut budget = HistoricalAutonomousRecoveryAccountingBudget::new(7);
        assert_eq!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                temp.path(),
                &mut budget
            )
            .expect("count inode once"),
            7
        );
        assert_eq!(budget.bytes_remaining, 0);
        let second_stable = temp
            .path()
            .join(format!("{}.norito", "05".repeat(Hash::LENGTH)));
        std::fs::rename(&pending, &second_stable)
            .expect("replace the temporary with a second stable name");
        assert!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                temp.path(),
                &mut HistoricalAutonomousRecoveryAccountingBudget::new(7),
            )
            .is_err(),
            "two stable names cannot impersonate a publication pair"
        );
        std::fs::rename(&second_stable, &pending).expect("restore the publication pair");
        std::fs::rename(&pending, outside.path().join("external"))
            .expect("move alias outside the namespace");
        assert!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                temp.path(),
                &mut HistoricalAutonomousRecoveryAccountingBudget::new(7)
            )
            .is_err()
        );
    }

    #[test]
    fn accounting_budget_is_shared_across_secondary_lane_namespaces() {
        let first = tempfile::tempdir().expect("first lane namespace");
        let second = tempfile::tempdir().expect("second lane namespace");
        let name = format!("{}.norito", "06".repeat(Hash::LENGTH));
        std::fs::write(first.path().join(&name), [1_u8; 3]).expect("first lane record");
        std::fs::write(second.path().join(name), [2_u8; 4]).expect("second lane record");
        let mut budget = HistoricalAutonomousRecoveryAccountingBudget {
            stable_remaining: 2,
            temporary_remaining: 0,
            bytes_remaining: 7,
        };
        assert_eq!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                first.path(),
                &mut budget
            )
            .expect("first lane consumes shared budget"),
            3
        );
        assert_eq!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                second.path(),
                &mut budget
            )
            .expect("second lane consumes remaining budget"),
            4
        );
        assert_eq!((budget.stable_remaining, budget.bytes_remaining), (0, 0));
        assert!(
            Kura::historical_autonomous_recovery_publication_accounting_bytes(
                first.path(),
                &mut budget
            )
            .is_err()
        );
    }

    #[test]
    fn accounting_rejects_unknown_empty_oversized_and_symlink_entries() {
        for (name, length) in [
            (".kura-sidecar-obsolete".to_owned(), 1),
            (format!("{}.norito", "03".repeat(Hash::LENGTH)), 0),
            (
                format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}oversized"),
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES as u64 + 1,
            ),
        ] {
            let temp = tempfile::tempdir().expect("accounting namespace");
            let file = std::fs::File::create(temp.path().join(name)).expect("invalid entry");
            file.set_len(length).expect("entry size");
            assert!(
                Kura::historical_autonomous_recovery_publication_accounting_bytes(
                    temp.path(),
                    &mut HistoricalAutonomousRecoveryAccountingBudget::new(
                        HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES
                    )
                )
                .is_err()
            );
        }
        #[cfg(unix)]
        {
            let temp = tempfile::tempdir().expect("accounting namespace");
            let outside = tempfile::NamedTempFile::new().expect("symlink target");
            std::os::unix::fs::symlink(
                outside.path(),
                temp.path()
                    .join(format!("{}.norito", "04".repeat(Hash::LENGTH))),
            )
            .expect("symlink fixture");
            assert!(
                Kura::historical_autonomous_recovery_publication_accounting_bytes(
                    temp.path(),
                    &mut HistoricalAutonomousRecoveryAccountingBudget::new(100)
                )
                .is_err()
            );
        }
    }
}
