//! Pure autonomous-attempt observation and consuming durability attestation.
//!
//! The caller retains the geometry scan's existing mutation guards. Observation
//! and drop do not repair or synchronize storage. This owner binds the original
//! namespace and its bounded file evidence; it is not a geometry reservation or
//! authority to publish State, and does not retain one descriptor per artifact.

use super::*;
use crate::kura::{AutonomousTerminalReceiptReadMode, BoundProgressSidecar, StableSidecarMetadata};

type AutonomousAttempts = BTreeMap<u64, (AutonomousLaneBlockArtifact, LaneBlockProposalV1, bool)>;

struct ObservedAutonomousFile {
    path: PathBuf,
    metadata: StableSidecarMetadata,
    bytes_hash: Hash,
}

/// Read-only evidence retaining one original namespace and its semantic result.
/// External canonical/claim joins remain protected by the caller's scan guards.
pub(super) struct ObservedAutonomousAttemptNamespace<'kura> {
    kura: &'kura Kura,
    path: PathBuf,
    // When absent, bind the nearest existing ancestor without creating a path.
    directory: BoundProgressDirectory,
    present: bool,
    files: Vec<ObservedAutonomousFile>,
    process_generation: Option<ObservedAutonomousFile>,
    // One shared receipt pair per route, retained without open file handles.
    terminal_receipts: Option<[StableSidecarMetadata; 2]>,
    attempts: AutonomousAttempts,
    entry_limit: usize,
    // The old reader synchronized the directory only for nonempty attempts.
    sync_directory: bool,
}

impl Kura {
    #[cfg(test)]
    pub(in crate::kura) fn observe_geometry_autonomous_namespace_for_tests(
        &self,
        lane_id: LaneId,
        attest: bool,
    ) -> Result<usize> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let observed = self.observe_geometry_autonomous_attempt_namespace(
            &directory,
            lane_id,
            Some(entry.dataspace_id),
            entry.incarnation,
            entry.activation_height,
            Some(&entry),
            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
            true,
        )?;
        if attest {
            observed.attest().map(|attempts| attempts.len())
        } else {
            observed.into_observed().map(|attempts| attempts.len())
        }
    }

    /// Authenticate the existing bounded attempt census without durability effects.
    /// The caller must retain the same Kura mutation guards through consumption.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_geometry_autonomous_attempt_namespace<'kura>(
        &'kura self,
        lane_artifacts: &Path,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        active_entry: Option<&LaneStorageEntry>,
        entry_limit: usize,
        require_terminal_lifecycle: bool,
    ) -> Result<ObservedAutonomousAttemptNamespace<'kura>> {
        let entry_limit = entry_limit.min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES);
        let mut ancestor = lane_artifacts;
        loop {
            match secure_file_metadata::from_path(ancestor) {
                Ok(_) => break,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    if ancestor == self.store_root {
                        return Err(Error::IO(error, ancestor.to_path_buf()));
                    }
                    ancestor = ancestor.parent().ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous observation has no existing Kura ancestor",
                        )
                    })?;
                }
                Err(error) => return Err(Error::IO(error, ancestor.to_path_buf())),
            }
        }
        let directory = Self::open_bound_progress_directory(&self.store_root, ancestor)?;
        let present = ancestor == lane_artifacts;
        let files = if present {
            self.observe_autonomous_namespace_files(lane_artifacts, entry_limit)?
        } else {
            Vec::new()
        };
        let generation_path =
            Self::autonomous_lifecycle_process_generation_path_for(&self.store_root);
        let process_generation = self
            .read_regular_sidecar_snapshot(
                &generation_path,
                &self.store_root,
                super::super::AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
            )?
            .map(|read| ObservedAutonomousFile {
                path: generation_path,
                metadata: read.metadata,
                bytes_hash: read.bytes_hash,
            });
        let mut observed = ObservedAutonomousAttemptNamespace {
            kura: self,
            path: lane_artifacts.to_path_buf(),
            directory,
            present,
            files,
            process_generation,
            terminal_receipts: None,
            attempts: BTreeMap::new(),
            entry_limit,
            sync_directory: false,
        };
        observed.ensure_unchanged()?;
        let mut terminal_receipts = None;
        observed.attempts = self.read_geometry_autonomous_attempt_namespace_without_attestation(
            lane_artifacts,
            lane_id,
            expected_dataspace_id,
            expected_incarnation,
            activation_height,
            active_entry,
            entry_limit,
            require_terminal_lifecycle,
            &mut terminal_receipts,
        )?;
        observed.terminal_receipts = terminal_receipts;
        observed.sync_directory = !observed.attempts.is_empty();
        observed.ensure_unchanged()?;
        Ok(observed)
    }

    fn observe_autonomous_namespace_files(
        &self,
        directory: &Path,
        entry_limit: usize,
    ) -> Result<Vec<ObservedAutonomousFile>> {
        let mut files = Vec::new();
        let mut bytes = 0_u64;
        for entry in
            fs::read_dir(directory).map_err(|error| Error::IO(error, directory.to_owned()))?
        {
            let entry = entry.map_err(|error| Error::IO(error, directory.to_owned()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace contains a non-UTF-8 artifact",
                )
            })?;
            let quarantine = Self::validate_autonomous_publication_quarantine(
                &self.store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
                "geometry bootstrap quarantine",
            )?;
            if Self::is_unresolved_autonomous_publication_temporary_name(
                &name,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            ) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace contains a bootstrap atomic temporary",
                ));
            }
            if !name.starts_with("autonomous_") && !quarantine {
                continue;
            }
            if files.len() >= entry_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds its bounded entry limit",
                ));
            }
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace contains a non-regular, linked, or symlinked artifact",
                ));
            }
            bytes = bytes.checked_add(metadata.len()).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous namespace byte count overflows",
                )
            })?;
            if bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds the shared sidecar aggregate byte budget",
                ));
            }
            let read = self
                .read_regular_sidecar_snapshot(&path, directory, usize::try_from(metadata.len())?)?
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous attempt disappeared during geometry observation",
                    )
                })?;
            if !Self::sidecar_file_metadata_unchanged(&metadata, &read.metadata.file) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt changed during geometry observation",
                ));
            }
            files.push(ObservedAutonomousFile {
                path,
                metadata: read.metadata,
                bytes_hash: read.bytes_hash,
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    fn observe_geometry_terminal_receipt_pair(
        &self,
        directory: &Path,
        observed: &mut Option<[StableSidecarMetadata; 2]>,
    ) -> Result<()> {
        if observed.is_some() {
            return Ok(());
        }
        let data_path = directory.join(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE);
        let index_path = directory.join(LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE);
        let BoundProgressPair::Present(bound) =
            self.open_bound_progress_pair(&data_path, &index_path)?
        else {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "terminal receipt pair disappeared during observation",
            ));
        };
        *observed = Some([bound.data_metadata, bound.index_metadata]);
        Ok(())
    }
}

impl ObservedAutonomousAttemptNamespace<'_> {
    /// Borrow the authenticated attempt projection without claiming durability.
    #[cfg(test)]
    pub(super) fn attempts(&self) -> &AutonomousAttempts {
        &self.attempts
    }

    /// Consume the unchanged observation without claiming durability.
    #[cfg(test)]
    pub(super) fn into_observed(self) -> Result<AutonomousAttempts> {
        self.ensure_unchanged()?;
        Ok(self.attempts)
    }

    /// Reject namespace, file-content, or process-generation replacement.
    pub(super) fn ensure_unchanged(&self) -> Result<()> {
        if !self
            .kura
            .geometry_bound_progress_directory_unchanged(&self.directory)
        {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt directory changed after observation",
            ));
        }
        if self.present {
            let current = self
                .kura
                .observe_autonomous_namespace_files(&self.path, self.entry_limit)?;
            if current.len() != self.files.len()
                || current.iter().zip(&self.files).any(|(current, original)| {
                    current.path != original.path
                        || current.bytes_hash != original.bytes_hash
                        || !Kura::stable_sidecar_metadata_unchanged(
                            &original.metadata,
                            &current.metadata,
                        )
                })
            {
                return Err(self.kura.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt files changed after observation",
                ));
            }
        } else {
            match secure_file_metadata::from_path(&self.path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, self.path.clone())),
                Ok(_) => {
                    return Err(self.kura.geometry_error(
                        ErrorKind::InvalidData,
                        "absent autonomous attempt directory appeared after observation",
                    ));
                }
            }
        }
        let path = Kura::autonomous_lifecycle_process_generation_path_for(&self.kura.store_root);
        let current = self.kura.read_regular_sidecar_snapshot(
            &path,
            &self.kura.store_root,
            super::super::AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES,
        )?;
        let matches = match (&self.process_generation, current) {
            (None, None) => true,
            (Some(original), Some(current)) => {
                original.bytes_hash == current.bytes_hash
                    && Kura::stable_sidecar_metadata_unchanged(
                        &original.metadata,
                        &current.metadata,
                    )
            }
            _ => false,
        };
        if !matches {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "autonomous lifecycle process generation changed after observation",
            ));
        }
        drop(self.unchanged_terminal_receipt_pair()?);
        if !self
            .kura
            .geometry_bound_progress_directory_unchanged(&self.directory)
        {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt directory changed during observation recheck",
            ));
        }
        Ok(())
    }

    // Indexed receipt history has its own existing bounds. Reopen only its two
    // handles and compare the original strong metadata instead of copying the
    // entire history or imposing the autonomous-file byte budget on it.
    fn unchanged_terminal_receipt_pair(&self) -> Result<Option<BoundProgressSidecar>> {
        let Some([data, index]) = &self.terminal_receipts else {
            return Ok(None);
        };
        let data_path = self.path.join(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE);
        let index_path = self.path.join(LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE);
        let BoundProgressPair::Present(bound) = self
            .kura
            .open_bound_progress_pair(&data_path, &index_path)?
        else {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "terminal receipt disappeared after observation",
            ));
        };
        if !Kura::stable_sidecar_metadata_unchanged(data, &bound.data_metadata)
            || !Kura::stable_sidecar_metadata_unchanged(index, &bound.index_metadata)
        {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "terminal receipt changed after observation",
            ));
        }
        Ok(Some(bound))
    }

    /// Synchronize only observed objects after exact revalidation, then release evidence.
    pub(super) fn attest(self) -> Result<AutonomousAttempts> {
        self.ensure_unchanged()?;
        if let Some(bound) = self.unchanged_terminal_receipt_pair()?
            && !self.kura.emergency_fast_startup_enabled()
            && !self
                .kura
                .sync_bound_progress_sidecar(&bound, "lane block application receipt")
        {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "terminal receipt durability attestation failed",
            ));
        }
        for original in &self.files {
            let file = OpenOptions::new()
                .read(true)
                .open(&original.path)
                .map_err(|error| Error::IO(error, original.path.clone()))?;
            let opened = secure_file_metadata::from_file(&file)
                .map_err(|error| Error::IO(error, original.path.clone()))?;
            if !Kura::sidecar_file_metadata_unchanged(&original.metadata.file, &opened) {
                return Err(self.kura.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt changed before durability attestation",
                ));
            }
            file.sync_all()
                .map_err(|error| Error::IO(error, original.path.clone()))?;
        }
        if self.sync_directory {
            self.directory
                .file
                .sync_all()
                .map_err(|error| Error::IO(error, self.path.clone()))?;
        }
        self.ensure_unchanged()?;
        Ok(self.attempts)
    }
}

impl Kura {
    #[allow(clippy::too_many_arguments)]
    fn read_geometry_autonomous_attempt_namespace_without_attestation(
        &self,
        lane_artifacts: &Path,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        active_entry: Option<&LaneStorageEntry>,
        entry_limit: usize,
        require_terminal_lifecycle: bool,
        terminal_receipts: &mut Option<[StableSidecarMetadata; 2]>,
    ) -> Result<BTreeMap<u64, (AutonomousLaneBlockArtifact, LaneBlockProposalV1, bool)>> {
        let entry_limit = entry_limit.min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES);
        let lifecycle_process_generation = self
            .read_autonomous_lifecycle_process_generation_record()?
            .map(|(record, _)| record);
        let mut attempts = BTreeMap::<
            u64,
            Vec<(
                AutonomousLaneBlockLatestAttemptV1,
                AutonomousLaneBlockArtifact,
                LaneBlockProposalV1,
                bool,
            )>,
        >::new();
        let mut attempt_identities = BTreeSet::new();
        let mut view_identities = BTreeSet::new();
        let mut height_pointers = BTreeMap::new();
        let mut route_pointer = None;
        let mut lifecycle_cursors = BTreeMap::<(u64, u64), AutonomousLifecycleCursorV1>::new();
        let mut lifecycle_terminal_outcomes =
            BTreeMap::<(u64, u64), (PathBuf, AutonomousLifecycleTerminalOutcomeV1)>::new();
        let mut lifecycle_bootstraps =
            BTreeMap::<(u64, u64), (PathBuf, AutonomousLifecycleBootstrapV1)>::new();
        let entries = match fs::read_dir(lane_artifacts) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(Error::IO(error, lane_artifacts.to_path_buf())),
        };
        let mut related_entries = 0_usize;
        let mut related_bytes = 0_u64;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, lane_artifacts.to_path_buf()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a non-UTF-8 artifact",
                    ),
                    path.clone(),
                )
            })?;
            let bootstrap_quarantine = Self::validate_autonomous_publication_quarantine(
                &self.store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
                "geometry bootstrap quarantine",
            )?;
            if Self::is_unresolved_autonomous_publication_temporary_name(
                &name,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            ) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a bootstrap atomic temporary",
                    ),
                    path,
                ));
            }
            if !name.starts_with("autonomous_") && !bootstrap_quarantine {
                continue;
            }
            related_entries = related_entries.checked_add(1).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace entry count overflows",
                )
            })?;
            if related_entries > entry_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds its bounded entry limit",
                ));
            }
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a non-regular, linked, or symlinked artifact",
                    ),
                    path,
                ));
            }
            related_bytes = related_bytes.checked_add(metadata.len()).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace byte count overflows",
                )
            })?;
            if related_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds the shared sidecar aggregate byte budget",
                ));
            }
            if bootstrap_quarantine {
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lane_block_attempt_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous attempt disappeared during geometry validation",
                        )
                    })?;
                let mut artifact = norito::decode_from_bytes::<AutonomousLaneBlockArtifact>(&bytes)
                    .map_err(Error::NoritoFrame)?;
                if artifact.encode_framed().map_err(Error::NoritoFrame)? != bytes {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous attempt is not canonical framed Norito",
                        ),
                        path,
                    ));
                }
                let pointer =
                    AutonomousLaneBlockLatestAttemptV1::from_payload(&artifact.executable_payload);
                let descriptor = &artifact.executable_payload.origin_proposal.descriptor;
                if pointer.lane_id != lane_id
                    || pointer.lane_block_height != lane_block_height
                    || pointer.proposal_height != proposal_height
                    || descriptor.lane_incarnation != expected_incarnation
                    || descriptor.proposal_height <= activation_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous attempt has a stale or namespace-conflicting route identity",
                        ),
                        path,
                    ));
                }
                if let Some(active_entry) = active_entry {
                    self.require_active_lane_artifact(active_entry, descriptor)
                        .map_err(|error| {
                            self.geometry_error_owned(
                                ErrorKind::InvalidData,
                                format!("autonomous attempt has a stale active binding: {error}"),
                            )
                        })?;
                }
                let view_path = lane_artifacts.join(format!(
                    "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{proposal_height:020}.norito"
                ));
                let view_state = self.read_autonomous_lane_block_view_state_locked(
                    &artifact.executable_payload,
                    &view_path,
                    super::super::AutonomousLaneBlockViewStateReadMode::MainOnly,
                )?;
                let retired = view_state
                    .as_ref()
                    .is_some_and(|state| state.retirement.is_some());
                if let Some(state) = view_state {
                    artifact.availability_certificate = state.availability_certificate;
                    artifact.view_checkpoint = state.checkpoint;
                    artifact.new_view_certificates = state.certificates;
                }
                let current = Self::validate_autonomous_lane_block_artifact(
                    &artifact,
                    artifact.executable_payload.network_id,
                    artifact.executable_payload.epoch,
                )
                .map_err(|message| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("autonomous attempt is invalid: {message}"),
                    )
                })?;
                attempt_identities.insert((lane_block_height, proposal_height));
                let attempts_at_height = attempts.entry(lane_block_height).or_default();
                attempts_at_height.push((pointer, artifact, current, retired));
                if attempts_at_height.len() > self.lane_history_retention().get() {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous proposal-height attempts exceed the configured lane-history retention bound",
                    ));
                }
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lifecycle_bootstrap_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap disappeared during geometry validation",
                        )
                    })?;
                let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&path, &bytes)?;
                let process_generation = lifecycle_process_generation.as_ref().ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap exists without a Kura-root process generation",
                        ),
                        path.clone(),
                    )
                })?;
                Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                    process_generation,
                    &bootstrap,
                )
                .map_err(|message| {
                    Error::IO(
                        std::io::Error::new(ErrorKind::InvalidData, message),
                        path.clone(),
                    )
                })?;
                let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
                if descriptor.lane_id != lane_id
                    || descriptor.lane_incarnation != expected_incarnation
                    || descriptor.proposal_height <= activation_height
                    || descriptor.lane_block_height != lane_block_height
                    || descriptor.proposal_height != proposal_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                if active_entry.is_none() {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap must never be present in an archive",
                    ));
                }
                if require_terminal_lifecycle {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "lane retirement is blocked by an unfinished lifecycle bootstrap",
                    ));
                }
                if lifecycle_bootstraps
                    .insert(
                        (lane_block_height, proposal_height),
                        (path.clone(), bootstrap),
                    )
                    .is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap has a duplicate identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lifecycle_cursor_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle cursor disappeared during geometry validation",
                        )
                    })?;
                let cursor = Self::decode_autonomous_lifecycle_cursor(&path, &bytes)?;
                let binding = cursor.binding();
                if binding.lane_id != lane_id
                    || binding.lane_incarnation != expected_incarnation
                    || binding.proposal_height <= activation_height
                    || binding.lane_block_height != lane_block_height
                    || binding.proposal_height != proposal_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| binding.dataspace_id != dataspace_id)
                    || lifecycle_cursors
                        .insert((lane_block_height, proposal_height), cursor)
                        .is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle cursor has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lifecycle_terminal_outcome_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle terminal outcome disappeared during geometry validation",
                        )
                    })?;
                let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
                let binding = outcome.binding();
                if binding.lane_id != lane_id
                    || binding.lane_incarnation != expected_incarnation
                    || binding.proposal_height <= activation_height
                    || binding.lane_block_height != lane_block_height
                    || binding.proposal_height != proposal_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| binding.dataspace_id != dataspace_id)
                    || lifecycle_terminal_outcomes
                        .insert(
                            (lane_block_height, proposal_height),
                            (path.clone(), outcome),
                        )
                        .is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle terminal outcome has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if let Some(identity) = Self::autonomous_two_height_coordinates(
                &name,
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
            ) {
                view_identities.insert(identity);
                continue;
            }
            if let Some(lane_block_height) = Self::autonomous_one_height_coordinate(
                &name,
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
            ) {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        super::super::AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous latest pointer disappeared during geometry validation",
                        )
                    })?;
                let pointer = Self::decode_autonomous_lane_block_latest_attempt(&path, &bytes)?;
                if pointer.lane_id != lane_id
                    || pointer.lane_block_height != lane_block_height
                    || pointer.lane_incarnation != expected_incarnation
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| pointer.dataspace_id != dataspace_id)
                    || height_pointers.insert(lane_block_height, pointer).is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous latest pointer has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        super::super::AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous route pointer disappeared during geometry validation",
                        )
                    })?;
                let pointer = Self::decode_autonomous_lane_block_latest_attempt(&path, &bytes)?;
                if pointer.lane_id != lane_id
                    || pointer.lane_incarnation != expected_incarnation
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| pointer.dataspace_id != dataspace_id)
                    || route_pointer.replace(pointer).is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous route pointer has a stale or duplicate identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unexpected or obsolete autonomous persistence artifact",
                ),
                path,
            ));
        }
        if !view_identities.is_subset(&attempt_identities) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an orphan view state",
            ));
        }
        let lifecycle_identities = lifecycle_cursors.keys().copied().collect::<BTreeSet<_>>();
        if !lifecycle_identities.is_subset(&attempt_identities) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an orphan lifecycle cursor",
            ));
        }
        let canonical_replica_identities = lifecycle_terminal_outcomes
            .iter()
            .filter_map(|(identity, (_, outcome))| {
                matches!(
                    outcome.basis(),
                    AutonomousLifecycleTerminalOutcomeBasisV1::CanonicalReplica { .. }
                )
                .then_some(*identity)
            })
            .collect::<Vec<_>>();
        for identity in canonical_replica_identities {
            let (path, outcome) = lifecycle_terminal_outcomes
                .get(&identity)
                .expect("canonical replica identity was collected above");
            if attempt_identities.contains(&identity)
                || lifecycle_cursors.contains_key(&identity)
                || lifecycle_bootstraps.contains_key(&identity)
                || view_identities.contains(&identity)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "canonical replica terminal outcome overlaps owned lifecycle custody",
                    ),
                    path.clone(),
                ));
            }
            let replica_data_path =
                lane_artifacts.join(CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE);
            let replica_index_path =
                lane_artifacts.join(CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE);
            let receipt_data_path = lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE);
            let receipt_index_path =
                lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE);
            self.observe_geometry_terminal_receipt_pair(lane_artifacts, terminal_receipts)?;
            self.validate_canonical_replica_terminal_outcome_from_paths_with_receipt_read_mode_locked(
                lane_id,
                outcome,
                &replica_data_path,
                &replica_index_path,
                &receipt_data_path,
                &receipt_index_path,
                self.local_peer_id.get(),
                AutonomousTerminalReceiptReadMode::ReadOnly,
            )?;
            if require_terminal_lifecycle && !outcome.is_complete() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::WouldBlock,
                        "lane retirement is blocked by a Pending canonical replica terminal outcome",
                    ),
                    path.clone(),
                ));
            }
            lifecycle_terminal_outcomes.remove(&identity);
        }
        let terminal_outcome_identities = lifecycle_terminal_outcomes
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        if !terminal_outcome_identities.is_subset(&attempt_identities)
            || !terminal_outcome_identities.is_subset(&lifecycle_identities)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an orphan lifecycle terminal outcome",
            ));
        }
        let mut payload_only_bootstrap_identities = BTreeSet::new();
        for (identity, (path, bootstrap)) in &lifecycle_bootstraps {
            let process_generation = lifecycle_process_generation.as_ref().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap exists without a Kura-root process generation",
                    ),
                    path.clone(),
                )
            })?;
            Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                process_generation,
                bootstrap,
            )
            .map_err(|message| {
                Error::IO(
                    std::io::Error::new(ErrorKind::InvalidData, message),
                    path.clone(),
                )
            })?;
            let active_entry = active_entry.expect(
                "an autonomous lifecycle bootstrap was rejected above without an active entry",
            );
            let stage =
                self.classify_autonomous_lifecycle_bootstrap_locked(active_entry, bootstrap)?;
            let payload_present = attempts.get(&identity.0).is_some_and(|attempts_at_height| {
                attempts_at_height.iter().any(|(pointer, artifact, _, _)| {
                    pointer.proposal_height == identity.1
                        && artifact.executable_payload == bootstrap.body.executable_payload
                })
            });
            let cursor = lifecycle_cursors.get(identity);
            let stage_matches = match stage {
                AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly => {
                    !payload_present && cursor.is_none()
                }
                AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable => {
                    payload_only_bootstrap_identities.insert(*identity);
                    payload_present && cursor.is_none()
                }
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable => {
                    payload_present && cursor == Some(&bootstrap.body.prepared_activate)
                }
                AutonomousLifecycleBootstrapRecoveryStage::LiveDurable => {
                    payload_present && cursor == Some(&bootstrap.body.live_activate)
                }
            };
            if !stage_matches {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap crash boundary conflicts with geometry inventory",
                    ),
                    path.clone(),
                ));
            }
        }
        if attempt_identities.iter().any(|identity| {
            !lifecycle_cursors.contains_key(identity)
                && !payload_only_bootstrap_identities.contains(identity)
        }) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous payload attempt lacks its exact lifecycle cursor or signed payload-durable bootstrap",
            ));
        }
        let attempt_payloads_by_identity = attempts
            .iter()
            .flat_map(|(lane_block_height, attempts_at_height)| {
                attempts_at_height
                    .iter()
                    .map(move |(pointer, artifact, _, _)| {
                        (
                            (*lane_block_height, pointer.proposal_height),
                            (&artifact.executable_payload, pointer),
                        )
                    })
            })
            .collect::<BTreeMap<_, _>>();
        let mut validated_terminal_outcome_identities = BTreeSet::new();
        // Every initial Prepared cursor requires its exact signed bootstrap authority.
        for (lane_block_height, attempts_at_height) in &attempts {
            for (pointer, artifact, _, _) in attempts_at_height {
                let identity = (*lane_block_height, pointer.proposal_height);
                let Some(cursor) = lifecycle_cursors.get(&identity) else {
                    continue;
                };
                cursor
                    .validate_for_payload(&artifact.executable_payload)
                    .map_err(|message| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("autonomous lifecycle cursor is invalid: {message}"),
                        )
                    })?;
                let process_generation =
                    lifecycle_process_generation.as_ref().ok_or_else(|| {
                        self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle cursor exists without a Kura-root process generation",
                    )
                    })?;
                Self::validate_autonomous_lifecycle_cursor_process_generation(
                    process_generation,
                    cursor,
                )
                .map_err(|message| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!(
                            "autonomous lifecycle cursor process generation is invalid: {message}"
                        ),
                    )
                })?;
                if cursor.sequence() == 1
                    && cursor.phase_kind() == AutonomousLifecycleCursorPhaseKindV1::Prepared
                    && !lifecycle_bootstraps.contains_key(&identity)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "initial Prepared lifecycle cursor is orphaned from its signed bootstrap",
                    ));
                }
                let outcome = lifecycle_terminal_outcomes.get(&identity);
                if let Some((path, outcome)) = outcome {
                    if outcome.basis() != AutonomousLifecycleTerminalOutcomeBasisV1::OwnedLifecycle
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "owned lifecycle attempt has a non-owning terminal basis",
                            ),
                            path.clone(),
                        ));
                    }
                    validated_terminal_outcome_identities.insert(identity);
                    outcome
                        .validate_for_payload(&artifact.executable_payload)
                        .map_err(|message| {
                            self.geometry_error_owned(
                                ErrorKind::InvalidData,
                                format!(
                                    "autonomous lifecycle terminal outcome is invalid: {message}"
                                ),
                            )
                        })?;
                    if outcome.binding() != cursor.binding() {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "autonomous lifecycle terminal outcome differs from its signed cursor binding",
                            ),
                            path.clone(),
                        ));
                    }
                    match outcome.source() {
                        source @ AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
                            ..
                        } => {
                            let receipt_data_path =
                                lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE);
                            let receipt_index_path =
                                lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE);
                            self.observe_geometry_terminal_receipt_pair(lane_artifacts, terminal_receipts)?;
                            self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_with_receipt_read_mode_locked(
                                &artifact.executable_payload,
                                source,
                                &receipt_data_path,
                                &receipt_index_path,
                                AutonomousTerminalReceiptReadMode::ReadOnly,
                            )?;
                        }
                        source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
                            retirement_hash,
                        } => {
                            let view_path = lane_artifacts.join(format!(
                                "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{:020}.norito",
                                pointer.proposal_height,
                            ));
                            let view_state = self
                                .read_autonomous_lane_block_view_state_locked(
                                    &artifact.executable_payload,
                                    &view_path,
                                    super::super::AutonomousLaneBlockViewStateReadMode::MainOnly,
                                )?
                                .ok_or_else(|| {
                                    self.geometry_error(
                                        ErrorKind::InvalidData,
                                        "release terminal outcome has no durable view state",
                                    )
                                })?;
                            let retirement = view_state.retirement.as_ref().ok_or_else(|| {
                                self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "release terminal outcome has no durable retirement",
                                )
                            })?;
                            if !retirement.matches_payload(&artifact.executable_payload)
                                || retirement.digest()? != retirement_hash
                            {
                                return Err(self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "release terminal outcome differs from its exact retirement",
                                ));
                            }
                            if let Some(active_entry) = active_entry {
                                self.autonomous_lifecycle_terminal_source_matches_release_locked(
                                    None,
                                    active_entry,
                                    &artifact.executable_payload,
                                    Some(retirement),
                                    source,
                                )?;
                            }
                        }
                        source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                            retirement_hash,
                            queue_disposition,
                        } => {
                            let view_path = lane_artifacts.join(format!(
                                "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{:020}.norito",
                                pointer.proposal_height,
                            ));
                            let view_state = self
                                .read_autonomous_lane_block_view_state_locked(
                                    &artifact.executable_payload,
                                    &view_path,
                                    super::super::AutonomousLaneBlockViewStateReadMode::MainOnly,
                                )?
                                .ok_or_else(|| {
                                    self.geometry_error(
                                        ErrorKind::InvalidData,
                                        "replica terminal outcome has no durable view state",
                                    )
                                })?;
                            let retirement = view_state.retirement.as_ref().ok_or_else(|| {
                                self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "replica terminal outcome has no durable retirement",
                                )
                            })?;
                            if !retirement.matches_payload(&artifact.executable_payload)
                                || retirement.digest()? != retirement_hash
                            {
                                return Err(self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "replica terminal outcome differs from its exact retirement",
                                ));
                            }
                            let (_, local_actor) = cursor.binding().local_validator_identity();
                            if local_actor == cursor.binding().producer_actor_projection() {
                                return Err(self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "producer lifecycle cursor cannot claim a replica Queue disposition",
                                ));
                            }
                            if let Some(active_entry) = active_entry {
                                self.autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                                    None,
                                    active_entry,
                                    &artifact.executable_payload,
                                    Some(retirement),
                                    source,
                                )?;
                                if require_terminal_lifecycle && outcome.is_complete() {
                                    self.require_autonomous_lane_entrypoint_claims_replica_complete_locked(
                                        active_entry,
                                        &artifact.executable_payload,
                                        retirement,
                                        queue_disposition,
                                        outcome.outcome_hash,
                                    )?;
                                }
                            }
                        }
                    }
                }
                if require_terminal_lifecycle {
                    let complete_canonical = outcome.is_some_and(|(_, outcome)| {
                        outcome.is_complete() && outcome.source().is_canonical_carrier()
                    });
                    // A certified slot cannot acquire release-path retirement evidence.
                    // Its independently authenticated Complete canonical-carrier outcome
                    // proves the exact merge receipt and Queue terminal projection above.
                    if active_entry.is_some() && !complete_canonical {
                        let view_path = lane_artifacts.join(format!(
                            "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{:020}.norito",
                            pointer.proposal_height,
                        ));
                        let view_state = self
                            .read_autonomous_lane_block_view_state_locked(
                                &artifact.executable_payload,
                                &view_path,
                                super::super::AutonomousLaneBlockViewStateReadMode::MainOnly,
                            )?
                            .ok_or_else(|| {
                                self.geometry_error(
                                    ErrorKind::InvalidData,
                                    "terminal lane attempt has no durable view state",
                                )
                            })?;
                        let retirement = view_state.retirement.as_ref().ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::WouldBlock,
                                "lane attempt cannot archive before its slot retirement is durable",
                            )
                        })?;
                        if !retirement.matches_payload(&artifact.executable_payload) {
                            return Err(self.geometry_error(
                                ErrorKind::InvalidData,
                                "lane attempt retirement differs from its executable payload",
                            ));
                        }
                        let retirement_hash = retirement.digest()?;
                        let descriptor = &artifact.executable_payload.origin_proposal.descriptor;
                        for entrypoint_hash in &artifact.executable_payload.entrypoint_hashes {
                            let claim_path = Self::autonomous_lane_entrypoint_claim_path(
                                &self.store_root,
                                &artifact.executable_payload.network_id,
                                entrypoint_hash,
                            );
                            let claim = Self::decode_autonomous_lane_entrypoint_claim(&claim_path)
                                .map_err(|message| {
                                    Self::invalid_lane_artifact_error(claim_path.clone(), message)
                                })?;
                            let temp_path =
                                Self::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
                            if Self::autonomous_lane_entrypoint_claim_file_exists(&temp_path)? {
                                return Err(self.geometry_error(
                                    ErrorKind::WouldBlock,
                                    "lane attempt cannot archive with a staged successor claim",
                                ));
                            }
                            if !self
                                .autonomous_lane_entrypoint_claim_path_matches(&claim, &claim_path)
                            {
                                return Err(Self::invalid_lane_artifact_error(
                                    claim_path,
                                    "prearchive entrypoint claim has a mismatched hash path",
                                ));
                            }
                            let mut expected_old = AutonomousLaneEntrypointClaimV1::new(
                                &artifact.executable_payload,
                                *entrypoint_hash,
                            );
                            expected_old.state = claim.state;
                            if claim == expected_old {
                                match claim.state {
                                    AutonomousLaneEntrypointClaimStateV1::Released(hash)
                                        if hash == retirement_hash => {}
                                    AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(
                                        hash,
                                        queue_disposition,
                                        terminal_outcome_hash,
                                    ) if hash == retirement_hash
                                        && outcome.is_some_and(|(_, outcome)| {
                                            outcome.is_complete()
                                                && outcome.outcome_hash == terminal_outcome_hash
                                                && outcome.source()
                                                    == AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                                                        retirement_hash,
                                                        queue_disposition,
                                                    }
                                        }) => {}
                                    AutonomousLaneEntrypointClaimStateV1::Active
                                    | AutonomousLaneEntrypointClaimStateV1::ReleasePending(_)
                                    | AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(_, _) => {
                                        return Err(self.geometry_error(
                                            ErrorKind::WouldBlock,
                                            "lane attempt has an exact nonreplaceable entrypoint claim",
                                        ));
                                    }
                                    AutonomousLaneEntrypointClaimStateV1::Released(_)
                                    | AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(
                                        ..
                                    ) => {
                                        return Err(self.geometry_error(
                                            ErrorKind::InvalidData,
                                            "terminal entrypoint claim differs from its retirement or Complete outcome",
                                        ));
                                    }
                                }
                                continue;
                            }
                            if claim.network_id != artifact.executable_payload.network_id
                                || claim.epoch < artifact.executable_payload.epoch
                                || claim.entrypoint_hash != *entrypoint_hash
                                || claim.lane_id != descriptor.lane_id
                                || claim.dataspace_id != descriptor.dataspace_id
                                || claim.lane_incarnation != descriptor.lane_incarnation
                                || claim.lane_block_height != descriptor.lane_block_height
                                || claim.proposal_height <= descriptor.proposal_height
                            {
                                return Err(Self::invalid_lane_artifact_error(
                                    claim_path,
                                    "prearchive claim is neither exact nor a monotonic successor",
                                ));
                            }
                            let newer_payload = attempt_payloads_by_identity
                                .get(&(claim.lane_block_height, claim.proposal_height))
                                .and_then(|(payload, pointer)| {
                                    (pointer.network_id == claim.network_id
                                        && pointer.epoch == claim.epoch)
                                        .then_some(*payload)
                                })
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        claim_path.clone(),
                                        "prearchive successor claim lacks its durable attempt",
                                    )
                                })?;
                            let mut expected_new = AutonomousLaneEntrypointClaimV1::new(
                                newer_payload,
                                *entrypoint_hash,
                            );
                            expected_new.state = claim.state;
                            if claim != expected_new {
                                return Err(Self::invalid_lane_artifact_error(
                                    claim_path,
                                    "prearchive successor claim differs from its durable payload",
                                ));
                            }
                        }
                    }
                    let signed_terminal = matches!(
                        cursor.phase(),
                        AutonomousLifecycleCursorPhaseV1::Terminal { .. }
                    );
                    let complete_replica = outcome.is_some_and(|(_, outcome)| {
                        outcome.is_complete()
                            && matches!(
                                outcome.source(),
                                AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                                    ..
                                }
                            )
                    });
                    if outcome.is_some_and(|(_, outcome)| !outcome.is_complete()) {
                        return Err(self.geometry_error(
                            ErrorKind::WouldBlock,
                            "lane retirement is blocked by a Pending autonomous lifecycle terminal outcome",
                        ));
                    }
                    if !signed_terminal && !complete_canonical && !complete_replica {
                        return Err(self.geometry_error(
                            ErrorKind::WouldBlock,
                            "lane retirement requires a signed terminal cursor or authenticated Complete terminal outcome",
                        ));
                    }
                }
            }
        }
        for identity in validated_terminal_outcome_identities {
            lifecycle_terminal_outcomes.remove(&identity);
        }
        if !lifecycle_terminal_outcomes.is_empty() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an unconsumed terminal outcome",
            ));
        }
        if attempts.is_empty() {
            if !height_pointers.is_empty()
                || route_pointer.is_some()
                || !lifecycle_cursors.is_empty()
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous pointer exists without an immutable payload attempt",
                ));
            }
            return Ok(BTreeMap::new());
        }
        let mut latest_by_height = BTreeMap::new();
        let mut route_identity: Option<AutonomousLaneBlockLatestAttemptV1> = None;
        for (lane_block_height, attempts_at_height) in &mut attempts {
            attempts_at_height.sort_by_key(|(pointer, _, _, _)| pointer.proposal_height);
            for adjacent in attempts_at_height.windows(2) {
                let (previous_pointer, previous_artifact, _, previous_retired) = &adjacent[0];
                let (successor_pointer, successor_artifact, _, _) = &adjacent[1];
                let previous = &previous_artifact
                    .executable_payload
                    .origin_proposal
                    .descriptor;
                let successor = &successor_artifact
                    .executable_payload
                    .origin_proposal
                    .descriptor;
                if !previous_retired
                    || successor_pointer.proposal_height <= previous_pointer.proposal_height
                    || successor.lane_id != previous.lane_id
                    || successor.dataspace_id != previous.dataspace_id
                    || successor.lane_incarnation != previous.lane_incarnation
                    || successor.lane_block_height != previous.lane_block_height
                    || successor.previous_lane_block_height != previous.previous_lane_block_height
                    || successor.previous_lane_block_descriptor_hash
                        != previous.previous_lane_block_descriptor_hash
                    || successor_pointer.network_id != previous_pointer.network_id
                    || successor_pointer.epoch < previous_pointer.epoch
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous attempts are not a retired monotonic proposal-height chain",
                    ));
                }
            }
            let (pointer, artifact, current, retired) = attempts_at_height
                .last()
                .expect("non-empty autonomous attempt group");
            if height_pointers.get(lane_block_height) != Some(pointer) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous lane-height pointer does not select the exact latest attempt",
                ));
            }
            if let Some(route) = route_identity.as_ref()
                && (pointer.network_id != route.network_id
                    || pointer.lane_id != route.lane_id
                    || pointer.dataspace_id != route.dataspace_id
                    || pointer.lane_incarnation != route.lane_incarnation
                    || pointer.proposal_height < route.proposal_height
                    || pointer.epoch < route.epoch)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace regresses its route or global context",
                ));
            }
            route_identity = Some(pointer.clone());
            latest_by_height.insert(
                *lane_block_height,
                (artifact.clone(), current.clone(), *retired),
            );
        }
        if height_pointers.len() != latest_by_height.len()
            || route_pointer.as_ref() != route_identity.as_ref()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace has an orphan or stale latest pointer",
            ));
        }
        Ok(latest_by_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Arc<Kura>, PathBuf, PathBuf) {
        let kura = Kura::blank_kura_for_testing();
        let directory = kura.store_root.join("autonomous-observation-fixture");
        fs::create_dir(&directory).unwrap();
        let bytes = b"retained bootstrap quarantine";
        let path = directory.join(format!(
            "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}quarantine-{}",
            Hash::new(bytes),
        ));
        fs::write(&path, bytes).unwrap();
        (kura, directory, path)
    }

    fn observe<'a>(
        kura: &'a Kura,
        directory: &Path,
        entry_limit: usize,
    ) -> Result<ObservedAutonomousAttemptNamespace<'a>> {
        kura.observe_geometry_autonomous_attempt_namespace(
            directory,
            LaneId::new(1),
            Some(DataSpaceId::new(1)),
            Hash::new(b"observation incarnation"),
            0,
            None,
            entry_limit,
            false,
        )
    }

    #[test]
    fn autonomous_observation_drop_preserves_files_and_wrapper_attests() {
        let (kura, directory, path) = fixture();
        let before = secure_file_metadata::from_path(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let observed = observe(&kura, &directory, 1).unwrap();
        assert!(observed.attempts().is_empty());
        observed.ensure_unchanged().unwrap();
        drop(observed);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(Kura::sidecar_file_metadata_unchanged(
            &before,
            &secure_file_metadata::from_path(&path).unwrap(),
        ));
        assert!(
            observe(&kura, &directory, 1)
                .unwrap()
                .attest()
                .unwrap()
                .is_empty()
        );
        assert!(
            kura.read_geometry_autonomous_attempt_namespace(
                &directory,
                LaneId::new(1),
                Some(DataSpaceId::new(1)),
                Hash::new(b"observation incarnation"),
                0,
                None,
                1,
                false,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn autonomous_attestation_rejects_changed_retained_bytes() {
        let (kura, directory, path) = fixture();
        let observed = observe(&kura, &directory, 1).unwrap();
        let mut changed = fs::read(&path).unwrap();
        changed[0] ^= 1;
        fs::write(&path, &changed).unwrap();
        assert!(observed.attest().is_err());
        assert_eq!(fs::read(&path).unwrap(), changed);
    }

    #[test]
    fn autonomous_attestation_rejects_same_bytes_replacement_and_new_sibling() {
        let (kura, directory, path) = fixture();
        let observed = observe(&kura, &directory, 1).unwrap();
        let bytes = fs::read(&path).unwrap();
        let displaced = directory.join("displaced-original");
        fs::rename(&path, &displaced).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(observed.attest().is_err());
        assert_eq!(fs::read(&displaced).unwrap(), bytes);
        let observed = observe(&kura, &directory, 1).unwrap();
        fs::write(directory.join("unrelated-new-sibling"), b"new").unwrap();
        assert!(observed.attest().is_err());
    }

    #[test]
    fn autonomous_observation_keeps_entry_bound_and_missing_directory_identity() {
        let (kura, directory, _) = fixture();
        assert!(observe(&kura, &directory, 0).is_err());
        let absent = directory.join("absent");
        let observed = observe(&kura, &absent, 0).unwrap();
        assert!(observed.attempts().is_empty());
        drop(observed);
        assert!(!absent.exists());
        assert!(
            observe(&kura, &absent, 0)
                .unwrap()
                .attest()
                .unwrap()
                .is_empty()
        );
        let observed = observe(&kura, &absent, 0).unwrap();
        fs::create_dir(&absent).unwrap();
        assert!(observed.attest().is_err());
    }
}
