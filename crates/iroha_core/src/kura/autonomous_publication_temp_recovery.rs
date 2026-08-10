impl Kura {
    fn is_autonomous_publication_quarantine_name(name: &str, temporary_prefix: &str) -> bool {
        name.strip_prefix(temporary_prefix)
            .and_then(|suffix| suffix.strip_prefix("quarantine-"))
            .is_some_and(|suffix| {
                let mut digests = suffix.split('-');
                let valid = |digest: &str| {
                    digest.len() == Hash::LENGTH * 2
                        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                };
                digests.next().is_some_and(valid)
                    && digests.next().is_none_or(valid)
                    && digests.next().is_none()
            })
    }

    fn is_unresolved_autonomous_publication_temporary_name(
        name: &str,
        temporary_prefix: &str,
    ) -> bool {
        name.starts_with(temporary_prefix)
            && !Self::is_autonomous_publication_quarantine_name(name, temporary_prefix)
    }

    fn validate_autonomous_publication_quarantine(
        store_root: &Path,
        path: &Path,
        max_bytes: usize,
        temporary_prefix: &str,
        kind: &str,
    ) -> Result<bool> {
        let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            return Ok(false);
        };
        if !Self::is_autonomous_publication_quarantine_name(name, temporary_prefix) {
            if name
                .strip_prefix(temporary_prefix)
                .is_some_and(|suffix| suffix.starts_with("quarantine-"))
            {
                return Err(Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} has a malformed reserved quarantine name"),
                ));
            }
            return Ok(false);
        }
        let (_namespace, _file, _metadata, bytes) =
            Self::bind_autonomous_publication_temporary(store_root, path, max_bytes, kind)?;
        let expected = format!("{temporary_prefix}quarantine-{}", Hash::new(&bytes));
        let duplicate_prefix = format!("{expected}-");
        if name != expected
            && !name.strip_prefix(&duplicate_prefix).is_some_and(|digest| {
                digest.len() == Hash::LENGTH * 2
                    && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} quarantine digest does not bind its retained bytes"),
            ));
        }
        Ok(true)
    }

    fn autonomous_lifecycle_process_generation_publication_residue_bytes(
        store_root: &Path,
    ) -> Result<u64> {
        let entries = match std::fs::read_dir(store_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(Error::IO(error, store_root.to_path_buf())),
        };
        let mut entries_seen = 0_usize;
        let mut bytes = 0_u64;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
            entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    store_root.to_path_buf(),
                    "process-generation residue accounting count overflows",
                )
            })?;
            if entries_seen > AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ROOT_ENTRY_LIMIT {
                return Err(Self::invalid_lane_artifact_error(
                    store_root.to_path_buf(),
                    "process-generation residue accounting exceeds its root entry bound",
                ));
            }
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !name.starts_with(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX) {
                continue;
            }
            let quarantine = Self::validate_autonomous_publication_quarantine(
                store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
                "process-generation publication residue",
            )?;
            if !quarantine
                && name
                    .strip_prefix(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX)
                    .is_none_or(str::is_empty)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "process-generation publication residue has an empty temporary identity",
                ));
            }
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
                || metadata.len() == 0
                || metadata.len() > AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES as u64
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "process-generation publication residue is not one bounded regular object",
                ));
            }
            bytes = bytes.checked_add(metadata.len()).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    store_root.to_path_buf(),
                    "process-generation residue byte accounting overflows",
                )
            })?;
        }
        Ok(bytes)
    }

    fn read_optional_bound_publication_bytes(
        store_root: &Path,
        path: &Path,
        max_bytes: usize,
        kind: &str,
    ) -> Result<Option<Vec<u8>>> {
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::IO(error, path.to_path_buf())),
            Ok(_) => Self::bind_autonomous_publication_temporary(store_root, path, max_bytes, kind)
                .map(|(_, _, _, bytes)| Some(bytes)),
        }
    }

    fn read_validated_autonomous_lifecycle_bootstrap_quarantine(
        &self,
        path: &Path,
        process_generation: Option<&AutonomousLifecycleProcessGenerationRecordV1>,
        kind: &str,
    ) -> Result<Option<(Vec<u8>, AutonomousLifecycleBootstrapV1)>> {
        if !Self::validate_autonomous_publication_quarantine(
            &self.store_root,
            path,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            kind,
        )? {
            return Ok(None);
        }
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has no parent directory"),
            )
        })?;
        let bytes = self
            .read_regular_sidecar_bytes(path, parent, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared during validation"),
                )
            })?;
        let bootstrap = norito::decode_canonical::<AutonomousLifecycleBootstrapV1>(&bytes)
            .map_err(Error::NoritoFrame)?;
        bootstrap
            .validate_structure()
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        let generation = process_generation.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} lacks process-generation authority"),
            )
        })?;
        Self::validate_autonomous_lifecycle_bootstrap_process_generation(generation, &bootstrap)
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        Ok(Some((bytes, bootstrap)))
    }

    fn validate_autonomous_lifecycle_bootstrap_quarantine_for_active_entry(
        &self,
        entry: &LaneConfigEntry,
        process_generation: Option<&AutonomousLifecycleProcessGenerationRecordV1>,
        path: &Path,
        kind: &str,
    ) -> Result<bool> {
        let Some((bytes, bootstrap)) = self
            .read_validated_autonomous_lifecycle_bootstrap_quarantine(
                path,
                process_generation,
                kind,
            )?
        else {
            return Ok(false);
        };
        let (active_incarnation, activation_height) = self.active_lane_incarnation_marker(entry)?;
        Self::validate_autonomous_lifecycle_bootstrap_quarantine_route(
            &bootstrap,
            path,
            entry.lane_id,
            Some(entry.dataspace_id),
            active_incarnation,
            activation_height,
            kind,
        )?;
        Self::validate_autonomous_lifecycle_bootstrap_quarantine_for_entry(
            &self.store_root,
            entry,
            process_generation,
            path,
            &bytes,
        )?;
        Ok(true)
    }

    fn validate_autonomous_lifecycle_bootstrap_quarantine_route(
        bootstrap: &AutonomousLifecycleBootstrapV1,
        path: &Path,
        expected_lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        kind: &str,
    ) -> Result<()> {
        let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
        if descriptor.lane_id != expected_lane_id
            || expected_dataspace_id.is_some_and(|id| descriptor.dataspace_id != id)
            || descriptor.lane_incarnation != expected_incarnation
            || descriptor.proposal_height <= activation_height
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has a stale route or incarnation"),
            ));
        }
        Ok(())
    }

    fn validate_autonomous_lifecycle_bootstrap_quarantine_domain(
        store_root: &Path,
        lane_config: &LaneConfig,
        process_generation: Option<&AutonomousLifecycleProcessGenerationRecordV1>,
        path: &Path,
        bytes: &[u8],
    ) -> Result<AutonomousLifecycleBootstrapV1> {
        let raw = norito::decode_canonical::<AutonomousLifecycleBootstrapV1>(bytes)
            .map_err(Error::NoritoFrame)?;
        let descriptor = &raw.body.executable_payload.origin_proposal.descriptor;
        let entry = lane_config.entry(descriptor.lane_id).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "bootstrap quarantine targets an unconfigured lane",
            )
        })?;
        Self::validate_autonomous_lifecycle_bootstrap_quarantine_for_entry(
            store_root,
            entry,
            process_generation,
            path,
            bytes,
        )?;
        Ok(raw)
    }

    fn validate_autonomous_lifecycle_bootstrap_quarantine_for_entry(
        store_root: &Path,
        entry: &LaneConfigEntry,
        process_generation: Option<&AutonomousLifecycleProcessGenerationRecordV1>,
        path: &Path,
        bytes: &[u8],
    ) -> Result<()> {
        let raw = norito::decode_canonical::<AutonomousLifecycleBootstrapV1>(bytes)
            .map_err(Error::NoritoFrame)?;
        raw.validate_structure()
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        if raw.encode_framed().map_err(Error::NoritoFrame)? != bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "bootstrap quarantine is not canonical Norito",
            ));
        }
        let descriptor = &raw.body.executable_payload.origin_proposal.descriptor;
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "bootstrap quarantine has no parent",
            )
        })?;
        if entry.lane_id != descriptor.lane_id
            || entry.dataspace_id != descriptor.dataspace_id
            || parent != Self::lane_artifact_dir(&entry.blocks_dir(store_root))
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "bootstrap quarantine was moved outside its exact route namespace",
            ));
        }
        let stable_path = Self::autonomous_lifecycle_bootstrap_path_for_entry(
            entry,
            store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&stable_path, bytes)?;
        let process_generation = process_generation.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "bootstrap quarantine lacks process-generation authority",
            )
        })?;
        Self::validate_autonomous_lifecycle_bootstrap_process_generation(
            process_generation,
            &bootstrap,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;

        let stable = Self::read_optional_bound_publication_bytes(
            store_root,
            &stable_path,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            "stable bootstrap paired with quarantine",
        )?;
        if let Some(stable) = stable.as_deref()
            && Self::decode_autonomous_lifecycle_bootstrap(&stable_path, stable)? != bootstrap
        {
            return Err(Self::invalid_lane_artifact_error(
                stable_path,
                "bootstrap quarantine conflicts with its stable retry",
            ));
        }
        let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
            entry,
            store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let attempt = Self::read_optional_bound_publication_bytes(
            store_root,
            &attempt_path,
            MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
            "payload attempt paired with bootstrap quarantine",
        )?
        .map(|bytes| {
            norito::decode_canonical::<AutonomousLaneBlockArtifact>(&bytes)
                .map_err(Error::NoritoFrame)
        })
        .transpose()?;
        if attempt
            .as_ref()
            .is_some_and(|attempt| attempt.executable_payload != bootstrap.body.executable_payload)
        {
            return Err(Self::invalid_lane_artifact_error(
                attempt_path,
                "bootstrap quarantine conflicts with its payload retry",
            ));
        }
        let cursor_path = Self::autonomous_lifecycle_cursor_path_for_entry(
            entry,
            store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let cursor = Self::read_optional_bound_publication_bytes(
            store_root,
            &cursor_path,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
            "cursor paired with bootstrap quarantine",
        )?
        .map(|bytes| Self::decode_autonomous_lifecycle_cursor(&cursor_path, &bytes))
        .transpose()?;
        if let Some(cursor) = cursor.as_ref() {
            cursor
                .validate_for_payload(&bootstrap.body.executable_payload)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(cursor_path.clone(), message)
                })?;
            Self::validate_autonomous_lifecycle_cursor_process_generation(
                process_generation,
                cursor,
            )
            .map_err(|message| Self::invalid_lane_artifact_error(cursor_path.clone(), message))?;
            if cursor.binding() != &bootstrap.body.binding || attempt.is_none() {
                return Err(Self::invalid_lane_artifact_error(
                    cursor_path,
                    "bootstrap quarantine has a conflicting or orphan retry cursor",
                ));
            }
        }
        if stable.is_none() && attempt.is_some() && cursor.is_none() {
            return Err(Self::invalid_lane_artifact_error(
                attempt_path,
                "bootstrap quarantine has an incomplete retry without its stable bootstrap",
            ));
        }
        Ok(())
    }

    fn bind_autonomous_publication_temporary(
        store_root: &Path,
        path: &Path,
        max_bytes: usize,
        kind: &str,
    ) -> Result<(
        BoundProgressNamespace,
        std::fs::File,
        StableSidecarMetadata,
        Vec<u8>,
    )> {
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has no parent directory"),
            )
        })?;
        let directory = Self::open_bound_progress_directory(store_root, parent)?;
        let namespace = BoundProgressNamespace {
            data_path: path.to_path_buf(),
            index_path: path.to_path_buf(),
            directories: vec![directory],
        };
        let metadata =
            Self::regular_sidecar_metadata_for(store_root, path, parent)?.ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared during exact-object binding"),
                )
            })?;
        let len = usize::try_from(metadata.file.len())?;
        if len == 0 || len > max_bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has an empty or oversized payload"),
            ));
        }
        let mut file = Self::open_bound_progress_file(&namespace, path, &metadata)?;
        let mut bytes = Vec::with_capacity(len);
        std::io::Read::by_ref(&mut file)
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let current =
            Self::regular_sidecar_metadata_for(store_root, path, parent)?.ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared while reading its exact object"),
                )
            })?;
        if bytes.len() != len
            || !Self::stable_sidecar_metadata_unchanged(&metadata, &current)
            || !Self::progress_mutation_namespace_unchanged(&namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} identity changed while binding its exact object"),
            ));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        Ok((namespace, file, metadata, bytes))
    }

    fn retain_bound_autonomous_publication_temporary_as_quarantine(
        store_root: &Path,
        namespace: &BoundProgressNamespace,
        path: &Path,
        mut file: std::fs::File,
        metadata: &StableSidecarMetadata,
        expected_bytes: &[u8],
        max_bytes: usize,
        temporary_prefix: &str,
        kind: &str,
    ) -> Result<()> {
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let mut readback = Vec::with_capacity(expected_bytes.len());
        std::io::Read::by_ref(&mut file)
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut readback)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} lost its parent directory"),
            )
        })?;
        let current =
            Self::regular_sidecar_metadata_for(store_root, path, parent)?.ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared before exact-object quarantine"),
                )
            })?;
        if readback != expected_bytes
            || !Self::stable_sidecar_metadata_unchanged(metadata, &current)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} identity changed before exact-object quarantine"),
            ));
        }

        Self::quarantine_and_retain_bound_autonomous_publication_temporary(
            store_root,
            namespace,
            path,
            &file,
            metadata,
            expected_bytes,
            max_bytes,
            temporary_prefix,
            kind,
        )
    }

    /// Atomically isolate a classified residue as an inert forensic tombstone.
    ///
    /// Verification before a path-based unlink is insufficient: the entry can
    /// be exchanged between the metadata check and `unlinkat`. A no-clobber
    /// rename in the pinned parent first removes the residue from its published
    /// temporary name. The quarantined entry is compared with the already-open
    /// file and retained. Retention is deliberate: portable POSIX has no unlink
    /// by open file handle, so a final pathname unlink would reintroduce the
    /// exchange race this protocol closes.
    #[allow(clippy::too_many_arguments)]
    fn quarantine_and_retain_bound_autonomous_publication_temporary(
        store_root: &Path,
        namespace: &BoundProgressNamespace,
        path: &Path,
        file: &std::fs::File,
        metadata: &StableSidecarMetadata,
        expected_bytes: &[u8],
        max_bytes: usize,
        temporary_prefix: &str,
        kind: &str,
    ) -> Result<()> {
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} lost its parent directory"),
            )
        })?;
        let immediate = namespace.directories.first().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has no pinned immediate directory"),
            )
        })?;
        if parent != immediate.expected_path {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} is outside its pinned immediate directory"),
            ));
        }
        let current_name = path.file_name().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has no entry name"),
            )
        })?;
        let current_name_text = current_name.to_str().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has a non-UTF-8 entry name"),
            )
        })?;
        if !current_name_text
            .strip_prefix(temporary_prefix)
            .is_some_and(|suffix| !suffix.is_empty())
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has an unexpected entry name"),
            ));
        }
        let primary_quarantine_name =
            format!("{temporary_prefix}quarantine-{}", Hash::new(expected_bytes));
        let quarantine_name = if current_name_text
            .starts_with(&format!("{temporary_prefix}quarantine-"))
        {
            if !Self::validate_autonomous_publication_quarantine(
                store_root,
                path,
                max_bytes,
                temporary_prefix,
                kind,
            )? {
                return Err(Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} has an invalid retained quarantine name"),
                ));
            }
            current_name_text.to_owned()
        } else {
            let primary_path = parent.join(&primary_quarantine_name);
            if Self::regular_sidecar_metadata_for(store_root, &primary_path, parent)?.is_some() {
                if !Self::validate_autonomous_publication_quarantine(
                    store_root,
                    &primary_path,
                    max_bytes,
                    temporary_prefix,
                    kind,
                )? {
                    return Err(Self::invalid_lane_artifact_error(
                        primary_path,
                        format!("{kind} found an invalid primary quarantine"),
                    ));
                }
                format!(
                    "{primary_quarantine_name}-{}",
                    Hash::new(current_name_text.as_bytes())
                )
            } else {
                primary_quarantine_name
            }
        };
        let quarantine_path = parent.join(&quarantine_name);

        #[cfg(not(any(
            target_os = "android",
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
            target_os = "redox",
        )))]
        {
            let _ = (store_root, file, metadata, quarantine_path);
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} exact-object quarantine is unsupported on this platform"),
            ));
        }

        #[cfg(any(
            target_os = "android",
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
            target_os = "redox",
        ))]
        {
            use std::os::unix::fs::MetadataExt as _;

            if current_name_text != quarantine_name {
                rustix::fs::renameat_with(
                    &immediate.file,
                    current_name,
                    &immediate.file,
                    quarantine_name.as_str(),
                    rustix::fs::RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, quarantine_path.clone()))?;
            }
            if !Self::sync_bound_progress_mutation_directories(namespace, kind) {
                return Err(Self::invalid_lane_artifact_error(
                    quarantine_path,
                    format!("{kind} parent identity changed after quarantine"),
                ));
            }

            let held = file
                .metadata()
                .map_err(|error| Error::IO(error, quarantine_path.clone()))?;
            let quarantined = rustix::fs::statat(
                &immediate.file,
                quarantine_name.as_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, quarantine_path.clone()))?;
            if !Self::sidecar_file_metadata_unchanged_across_rename(&metadata.file, &held)
                || rustix::fs::FileType::from_raw_mode(quarantined.st_mode)
                    != rustix::fs::FileType::RegularFile
                || held.nlink() != 1
                || quarantined.st_dev as u64 != held.dev()
                || quarantined.st_ino as u64 != held.ino()
                || quarantined.st_nlink as u64 != 1
            {
                return Err(Self::invalid_lane_artifact_error(
                    quarantine_path,
                    format!("{kind} quarantine identity mismatch; residue retained"),
                ));
            }

            let retained = rustix::fs::statat(
                &immediate.file,
                quarantine_name.as_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, quarantine_path.clone()))?;
            let held_after = file
                .metadata()
                .map_err(|error| Error::IO(error, quarantine_path.clone()))?;
            if !Self::sidecar_file_metadata_unchanged_across_rename(&metadata.file, &held_after)
                || retained.st_dev as u64 != held_after.dev()
                || retained.st_ino as u64 != held_after.ino()
                || retained.st_nlink as u64 != 1
                || held_after.nlink() != 1
            {
                return Err(Self::invalid_lane_artifact_error(
                    quarantine_path,
                    format!(
                        "{kind} quarantine identity changed after verification; tombstones retained"
                    ),
                ));
            }
        }

        if !Self::sync_bound_progress_mutation_directories(namespace, kind) {
            return Err(Self::invalid_lane_artifact_error(
                parent.to_path_buf(),
                format!("{kind} parent identity changed during durable quarantine"),
            ));
        }
        if current_name_text == quarantine_name {
            if Self::regular_sidecar_metadata_for(store_root, &quarantine_path, parent)?.is_none() {
                return Err(Self::invalid_lane_artifact_error(
                    quarantine_path,
                    format!("{kind} disappeared after exact-object quarantine verification"),
                ));
            }
        } else if Self::regular_sidecar_metadata_for(store_root, path, parent)?.is_some() {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} reappeared after exact-object quarantine"),
            ));
        }
        Ok(())
    }

    /// Bind an initial durable claim to every authenticated retained generation-one residue.
    fn validate_retained_initial_process_generation_claim(
        &self,
        stable_present: bool,
        candidate: &AutonomousLifecycleProcessGenerationRecordV1,
    ) -> Result<()> {
        if stable_present {
            return Ok(());
        }
        if candidate.body.generation != 1 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "initial process-generation claim does not carry canonical generation one",
            ));
        }
        let entries = std::fs::read_dir(&self.store_root)
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        let mut entries_seen = 0_usize;
        let mut retained_authority = None;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, self.store_root.clone()))?;
            entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "Kura-root initial process-generation authority inventory count overflows",
                )
            })?;
            if entries_seen > AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ROOT_ENTRY_LIMIT {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "Kura-root initial process-generation authority inventory exceeds its hard entry limit",
                ));
            }
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name = file_name.to_str().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "Kura-root initial process-generation authority inventory contains a non-UTF-8 entry",
                )
            })?;
            if !file_name.starts_with(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX) {
                continue;
            }
            if !Self::validate_autonomous_publication_quarantine(
                &self.store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
                "initial process-generation authority quarantine",
            )? {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "initial process-generation authority changed before durable claim",
                ));
            }
            let (_, _, _, bytes) = Self::bind_autonomous_publication_temporary(
                &self.store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
                "initial process-generation authority quarantine",
            )?;
            let expected_name = format!(
                "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
                Hash::new(&bytes),
            );
            let duplicate_prefix = format!("{expected_name}-");
            if file_name != expected_name
                && !file_name
                    .strip_prefix(&duplicate_prefix)
                    .is_some_and(|digest| {
                        digest.len() == Hash::LENGTH * 2
                            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "initial process-generation authority quarantine changed while binding",
                ));
            }
            let record =
                Self::decode_autonomous_lifecycle_process_generation_record(&path, &bytes)?;
            if record.body.generation != 1
                || retained_authority
                    .as_ref()
                    .is_some_and(|authority| authority != &record)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "retained initial process-generation authority is ambiguous",
                ));
            }
            retained_authority = Some(record);
        }
        if retained_authority
            .as_ref()
            .is_some_and(|authority| authority != candidate)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "initial process-generation claim conflicts with retained canonical authority",
            ));
        }
        Ok(())
    }

    /// Quarantine one fully classified process-generation atomic-write residue.
    ///
    /// The caller owns the exclusive Kura-root OS lock. An atomic temporary
    /// still bearing its temporary name was synced before, but never renamed
    /// into the stable namespace. The stable record therefore remains the
    /// authority: an initial temporary must contain generation one, while a
    /// replacement temporary must contain its exact successor.
    fn recover_autonomous_lifecycle_process_generation_atomic_temporary_on_startup(
        store_root: &Path,
        _store_root_lock_file: &std::fs::File,
        allow_recovery_mutation: bool,
    ) -> Result<()> {
        let entries = std::fs::read_dir(store_root)
            .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        let mut entries_seen = 0_usize;
        let mut temporary_path = None;
        let mut quarantine_records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
            entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    store_root.to_path_buf(),
                    "Kura-root process-generation recovery inventory count overflows",
                )
            })?;
            if entries_seen > AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ROOT_ENTRY_LIMIT {
                return Err(Self::invalid_lane_artifact_error(
                    store_root.to_path_buf(),
                    "Kura-root process-generation recovery inventory exceeds its hard entry limit",
                ));
            }
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name = file_name.to_str().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "Kura-root process-generation recovery inventory contains a non-UTF-8 entry",
                )
            })?;
            if file_name == AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_TEMP_FILE {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "legacy deterministic process-generation temporary requires fail-closed operator recovery",
                ));
            }
            if Self::validate_autonomous_publication_quarantine(
                store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
                "process-generation quarantine",
            )? {
                let (_, _, _, bytes) = Self::bind_autonomous_publication_temporary(
                    store_root,
                    &path,
                    AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
                    "process-generation quarantine",
                )?;
                quarantine_records.push((
                    path.clone(),
                    Self::decode_autonomous_lifecycle_process_generation_record(&path, &bytes)?,
                ));
                continue;
            }
            if file_name.starts_with("autonomous_lifecycle_process_generation_")
                && file_name != AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_FILE
                && !file_name
                    .starts_with(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle process-generation artifact has an unexpected or legacy layout",
                ));
            }
            let Some(suffix) =
                file_name.strip_prefix(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX)
            else {
                continue;
            };
            if suffix.is_empty() || temporary_path.replace(path.clone()).is_some() {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "process-generation recovery requires exactly one unambiguous atomic temporary",
                ));
            }
        }
        let stable =
            Self::read_autonomous_lifecycle_process_generation_stable_record_for(store_root)?;
        let mut retained_initial_authority = None::<AutonomousLifecycleProcessGenerationRecordV1>;
        for (path, quarantine) in quarantine_records {
            let valid = match stable.as_ref() {
                None if quarantine.body.generation == 1 => {
                    if let Some(expected) = retained_initial_authority.as_ref() {
                        expected == &quarantine
                    } else {
                        retained_initial_authority = Some(quarantine.clone());
                        true
                    }
                }
                None => false,
                Some((stable, _)) => {
                    quarantine.body.network_id == stable.body.network_id
                        && quarantine.body.local_peer_id == stable.body.local_peer_id
                        && quarantine.body.generation
                            <= stable.body.generation.checked_add(1).unwrap_or(u64::MAX)
                }
            };
            if !valid {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "retained process-generation quarantine is not authenticated by stable generation authority",
                ));
            }
        }
        let Some(temporary_path) = temporary_path else {
            return Ok(());
        };
        if !allow_recovery_mutation {
            return Err(Self::invalid_lane_artifact_error(
                temporary_path,
                "process-generation atomic temporary requires authenticated non-provisional startup recovery",
            ));
        }
        let (namespace, file, metadata, bytes) = Self::bind_autonomous_publication_temporary(
            store_root,
            &temporary_path,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
            "process-generation atomic temporary",
        )?;
        let temporary =
            Self::decode_autonomous_lifecycle_process_generation_record(&temporary_path, &bytes)?;
        match stable {
            None if temporary.body.generation == 1
                && retained_initial_authority
                    .as_ref()
                    .is_none_or(|authority| authority == &temporary) => {}
            Some((stable, _))
                if temporary.body.network_id == stable.body.network_id
                    && temporary.body.local_peer_id == stable.body.local_peer_id
                    && stable
                        .body
                        .generation
                        .checked_add(1)
                        .is_some_and(|next| temporary.body.generation == next) => {}
            _ => {
                return Err(Self::invalid_lane_artifact_error(
                    temporary_path,
                    "process-generation atomic temporary is not the exact unpublished successor of stable state",
                ));
            }
        }
        Self::retain_bound_autonomous_publication_temporary_as_quarantine(
            store_root,
            &namespace,
            &temporary_path,
            file,
            &metadata,
            &bytes,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
            "process-generation atomic temporary",
        )?;
        Ok(())
    }

    /// Quarantine one authenticated bootstrap that was synced but not renamed.
    ///
    /// Bootstrap publication is no-clobber and precedes both the payload and
    /// lifecycle cursor. Recovery therefore accepts only one canonical signed
    /// temporary whose exact stable target, payload, and cursor are all absent.
    fn recover_autonomous_lifecycle_bootstrap_atomic_temporary_on_startup(
        store_root: &Path,
        lane_config: &LaneConfig,
        _store_root_lock_file: &std::fs::File,
        allow_recovery_mutation: bool,
    ) -> Result<()> {
        let process_generation =
            Self::read_autonomous_lifecycle_process_generation_record_for(store_root)?
                .map(|(record, _)| record);
        let root = store_root.join("blocks");
        let mut pending = vec![(root, 0_usize)];
        let mut entries_seen = 0_usize;
        let mut temporary_path = None;
        let mut quarantine_identities = BTreeMap::new();
        while let Some((directory, depth)) = pending.pop() {
            if depth > AUTONOMOUS_LIFECYCLE_GENERATION_AUDIT_MAX_DEPTH {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "bootstrap atomic-temporary recovery exceeds its directory-depth bound",
                ));
            }
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::IO(error, directory)),
            };
            for entry in entries {
                let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
                entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        directory.clone(),
                        "bootstrap atomic-temporary recovery entry count overflows",
                    )
                })?;
                if entries_seen > AUTONOMOUS_LIFECYCLE_GENERATION_AUDIT_MAX_ENTRIES {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "bootstrap atomic-temporary recovery exceeds its hard entry bound",
                    ));
                }
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                if file_type.is_symlink() {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "bootstrap atomic-temporary recovery encountered a symlink",
                    ));
                }
                if file_type.is_dir() {
                    pending.push((path, depth.saturating_add(1)));
                    continue;
                }
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if Self::validate_autonomous_publication_quarantine(
                    store_root,
                    &path,
                    AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                    AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
                    "bootstrap quarantine",
                )? {
                    let (_, _, _, bytes) = Self::bind_autonomous_publication_temporary(
                        store_root,
                        &path,
                        AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                        "bootstrap quarantine",
                    )?;
                    let bootstrap =
                        Self::validate_autonomous_lifecycle_bootstrap_quarantine_domain(
                            store_root,
                            lane_config,
                            process_generation.as_ref(),
                            &path,
                            &bytes,
                        )?;
                    let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
                    let identity = (
                        descriptor.lane_id,
                        descriptor.dataspace_id,
                        descriptor.lane_incarnation,
                        descriptor.lane_block_height,
                        descriptor.proposal_height,
                    );
                    let digest = Hash::new(&bytes);
                    if quarantine_identities
                        .insert(identity, digest)
                        .is_some_and(|existing| existing != digest)
                    {
                        return Err(Self::invalid_lane_artifact_error(
                            path,
                            "bootstrap quarantines conflict for one lifecycle identity",
                        ));
                    }
                    continue;
                }
                let Some(suffix) =
                    name.strip_prefix(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX)
                else {
                    continue;
                };
                if suffix.is_empty() || temporary_path.replace(path.clone()).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "bootstrap recovery requires exactly one unambiguous atomic temporary",
                    ));
                }
            }
        }
        let Some(temporary_path) = temporary_path else {
            return Ok(());
        };
        if !allow_recovery_mutation {
            return Err(Self::invalid_lane_artifact_error(
                temporary_path,
                "bootstrap atomic temporary requires authenticated non-provisional startup recovery",
            ));
        }
        let parent = temporary_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                temporary_path.clone(),
                "bootstrap atomic temporary has no parent directory",
            )
        })?;
        if parent.file_name().and_then(std::ffi::OsStr::to_str) != Some(LANE_ARTIFACTS_DIR_NAME) {
            return Err(Self::invalid_lane_artifact_error(
                temporary_path,
                "bootstrap atomic temporary is outside the canonical lane-artifact namespace",
            ));
        }
        let (namespace, file, metadata, bytes) = Self::bind_autonomous_publication_temporary(
            store_root,
            &temporary_path,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            "bootstrap atomic temporary",
        )?;
        let raw = norito::decode_canonical::<AutonomousLifecycleBootstrapV1>(&bytes).map_err(
            |error| match error {
                norito::Error::NonCanonicalEncoding => Self::invalid_lane_artifact_error(
                    temporary_path.clone(),
                    "bootstrap atomic temporary is not canonical Norito",
                ),
                other => Error::NoritoFrame(other),
            },
        )?;
        let descriptor = &raw.body.executable_payload.origin_proposal.descriptor;
        let lane_entry = lane_config.entry(descriptor.lane_id).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                temporary_path.clone(),
                "bootstrap atomic temporary targets an unconfigured lane",
            )
        })?;
        let expected_parent = Self::lane_artifact_dir(&lane_entry.blocks_dir(store_root));
        if lane_entry.dataspace_id != descriptor.dataspace_id || parent != expected_parent {
            return Err(Self::invalid_lane_artifact_error(
                temporary_path,
                "bootstrap atomic temporary route identity was swapped into another path",
            ));
        }
        let stable_path = parent.join(format!(
            "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_PREFIX}_{:020}_{:020}.norito",
            descriptor.lane_block_height, descriptor.proposal_height,
        ));
        let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&stable_path, &bytes)?;
        let process_generation = process_generation.as_ref().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                temporary_path.clone(),
                "bootstrap atomic temporary lacks a durable process generation",
            )
        })?;
        Self::validate_autonomous_lifecycle_bootstrap_process_generation(
            process_generation,
            &bootstrap,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(temporary_path.clone(), message))?;
        let attempt_path = parent.join(format!(
            "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX}_{:020}_{:020}.norito",
            descriptor.lane_block_height, descriptor.proposal_height,
        ));
        let cursor_path = parent.join(format!(
            "{AUTONOMOUS_LIFECYCLE_CURSOR_PREFIX}_{:020}_{:020}.norito",
            descriptor.lane_block_height, descriptor.proposal_height,
        ));
        for conflicting_path in [&stable_path, &attempt_path, &cursor_path] {
            match std::fs::symlink_metadata(conflicting_path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(Self::invalid_lane_artifact_error(
                        temporary_path,
                        "bootstrap atomic temporary conflicts with stable attempt state",
                    ));
                }
                Err(error) => return Err(Error::IO(error, conflicting_path.clone())),
            }
        }
        Self::retain_bound_autonomous_publication_temporary_as_quarantine(
            store_root,
            &namespace,
            &temporary_path,
            file,
            &metadata,
            &bytes,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            "bootstrap atomic temporary",
        )?;
        Ok(())
    }
}
