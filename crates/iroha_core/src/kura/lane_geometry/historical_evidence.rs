//! Bounded historical recovery observation with explicit durability ownership.
//!
//! Observation and drop perform no storage writes. Only consuming attestation
//! synchronizes the captured file objects. This is not a geometry reservation.

use super::*;
use crate::kura::{HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES, StableSidecarMetadata};

struct ObservedHistoricalRecoveryFile {
    path: PathBuf,
    metadata: StableSidecarMetadata,
    bytes_hash: Hash,
}

struct ObservedHistoricalRecoveryNamespace {
    directory: BoundProgressDirectory,
    files: Vec<ObservedHistoricalRecoveryFile>,
}

/// Immutable authenticated evidence tied to the caller's exact outer inventory.
pub(super) struct ObservedHistoricalRecoveryEvidence<'kura> {
    kura: &'kura Kura,
    outer: BoundProgressDirectory,
    outer_inventory: &'kura BoundProgressDirectorySnapshot,
    namespace: Option<ObservedHistoricalRecoveryNamespace>,
    records: Vec<HistoricalAutonomousLaneRecoveryRecordV1>,
    encoded_bytes: u64,
    context: &'kura str,
}

impl Kura {
    /// Authenticate bounded historical recovery without synchronizing or repairing files.
    /// The caller retains the geometry scan's existing mutation guards.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_geometry_historical_autonomous_recovery_records<'observed>(
        &'observed self,
        lane_artifacts: &Path,
        artifact_snapshot: &'observed BoundProgressDirectorySnapshot,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        entry_limit: usize,
        aggregate_byte_limit: u64,
        context: &'observed str,
    ) -> Result<ObservedHistoricalRecoveryEvidence<'observed>> {
        let outer = Self::open_bound_progress_directory(&self.store_root, lane_artifacts)?;
        let mut observed = ObservedHistoricalRecoveryEvidence {
            kura: self,
            outer,
            outer_inventory: artifact_snapshot,
            namespace: None,
            records: Vec::new(),
            encoded_bytes: 0,
            context,
        };
        observed.ensure_unchanged()?;
        let raw_name = std::ffi::OsStr::new(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
        let Some(snapshot) = artifact_snapshot.get(raw_name) else {
            return Ok(observed);
        };
        let directory = lane_artifacts.join(raw_name);
        if snapshot.kind != BoundProgressDirectoryEntryKind::Directory {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{context} historical recovery namespace is not a directory"),
                ),
                directory,
            ));
        }
        let before = secure_file_metadata::from_path(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?;
        if before.file_type().is_symlink()
            || !before.file_type().is_dir()
            || geometry_file_identity(&before) != snapshot.identity
            || self.canonical_sidecar_directory(&directory)?.is_none()
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{context} historical recovery directory changed or escaped Kura"),
                ),
                directory,
            ));
        }
        let bound_directory = Self::open_bound_progress_directory(&self.store_root, &directory)?;
        let entry_limit = entry_limit.min(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS);
        let (entries, encoded_bytes) = bounded_historical_autonomous_recovery_entries(
            &directory,
            entry_limit,
            aggregate_byte_limit,
            |path| {
                let metadata = secure_file_metadata::from_path(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok((metadata.clone(), metadata))
            },
        )?;
        let mut records = Vec::new();
        let mut files = Vec::new();
        records.try_reserve_exact(entries.len()).map_err(|_| {
            self.geometry_error(
                ErrorKind::OutOfMemory,
                "cannot retain bounded historical recovery records",
            )
        })?;
        files.try_reserve_exact(entries.len()).map_err(|_| {
            self.geometry_error(
                ErrorKind::OutOfMemory,
                "cannot retain bounded historical recovery identities",
            )
        })?;
        for (path, accounted) in entries {
            let (record, read) = self
                .read_historical_autonomous_recovery_record_with_identity(
                    &path,
                    &directory,
                    Some(&accounted),
                )?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} historical recovery record disappeared"),
                        ),
                        path.clone(),
                    )
                })?;
            let descriptor = &record.payload.origin_proposal.descriptor;
            if descriptor.lane_id != lane_id
                || descriptor.lane_incarnation != expected_incarnation
                || descriptor.proposal_height <= activation_height
                || expected_dataspace_id
                    .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} historical recovery record has a stale lane binding"),
                    ),
                    path,
                ));
            }
            let (retained_header, finality, _) = self
                .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(
                    record.canonical_body.height,
                )?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} historical recovery finality is unavailable"),
                        ),
                        path.clone(),
                    )
                })?;
            if retained_header.hash() != record.canonical_body.block_hash
                || retained_header.height().get() != record.canonical_body.height
                || retained_header.view_change_index() != record.carrier_view
                || finality.height != record.canonical_body.height
                || finality.block_hash != record.canonical_body.block_hash
                || HashOf::new(&finality) != record.canonical_body.finality_artifact_hash
                || finality.commit_qc.execution_commitment
                    != record.canonical_body.execution_commitment
                || finality.height_context != record.historical_context
                || finality.verify().is_err()
                || finality.validate_for_header(&retained_header).is_err()
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} historical recovery has conflicting retained finality"),
                    ),
                    path,
                ));
            }
            files.push(ObservedHistoricalRecoveryFile {
                path,
                metadata: read.metadata,
                bytes_hash: read.bytes_hash,
            });
            records.push(record);
        }
        self.validate_historical_autonomous_recovery_inventory_collisions(&records)?;
        observed.namespace = Some(ObservedHistoricalRecoveryNamespace {
            directory: bound_directory,
            files,
        });
        observed.records = records;
        observed.encoded_bytes = encoded_bytes;
        observed.ensure_unchanged()?;
        Ok(observed)
    }

    /// Explicit durability barrier for the current lock-scoped geometry callers.
    /// TODO: retain the full pure retirement observation and capacity reservation
    /// before carrying admission across consensus or canonical block durability.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_and_attest_geometry_historical_autonomous_recovery_records(
        &self,
        lane_artifacts: &Path,
        artifact_snapshot: &BoundProgressDirectorySnapshot,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        entry_limit: usize,
        aggregate_byte_limit: u64,
        context: &str,
    ) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {
        self.observe_geometry_historical_autonomous_recovery_records(
            lane_artifacts,
            artifact_snapshot,
            lane_id,
            expected_dataspace_id,
            expected_incarnation,
            activation_height,
            entry_limit,
            aggregate_byte_limit,
            context,
        )?
        .attest()
    }
}

impl ObservedHistoricalRecoveryEvidence<'_> {
    /// Consume exact read evidence without declaring it durable or authorizing retirement.
    pub(super) fn into_observed(
        self,
    ) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {
        self.ensure_unchanged()?;
        Ok((self.records, self.encoded_bytes))
    }

    /// Re-enumerate within the already admitted count/byte budget, including absence.
    fn ensure_unchanged(&self) -> Result<()> {
        let kura = self.kura;
        if &kura.geometry_bound_progress_directory_snapshot(
            &self.outer,
            self.outer_inventory.len(),
            self.context,
        )? != self.outer_inventory
        {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "historical recovery outer namespace changed after observation",
            ));
        }
        let Some(namespace) = &self.namespace else {
            return Ok(());
        };
        if !kura.geometry_bound_progress_directory_unchanged(&namespace.directory) {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "historical recovery namespace changed after observation",
            ));
        }
        let (entries, encoded_bytes) = bounded_historical_autonomous_recovery_entries(
            &namespace.directory.expected_path,
            namespace.files.len(),
            self.encoded_bytes,
            |path| {
                let metadata = secure_file_metadata::from_path(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok((metadata.clone(), metadata))
            },
        )?;
        if entries.len() != namespace.files.len()
            || encoded_bytes != self.encoded_bytes
            || entries
                .iter()
                .zip(&namespace.files)
                .any(|((path, metadata), observed)| {
                    path != &observed.path
                        || !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, metadata)
                })
        {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "historical recovery files changed after bounded observation",
            ));
        }
        Ok(())
    }

    /// Synchronize only the captured objects; release evidence after every recheck passes.
    pub(super) fn attest(self) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {
        self.ensure_unchanged()?;
        if let Some(namespace) = &self.namespace {
            for observed in &namespace.files {
                let path = &observed.path;
                let file = OpenOptions::new()
                    .read(true)
                    .open(path)
                    .map_err(|error| Error::IO(error, path.clone()))?;
                let opened = secure_file_metadata::from_file(&file)
                    .map_err(|error| Error::IO(error, path.clone()))?;
                if !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, &opened) {
                    return Err(self.kura.geometry_error(
                        ErrorKind::InvalidData,
                        "historical recovery file changed before durability sync",
                    ));
                }
                file.sync_all()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                let after = self
                    .kura
                    .read_regular_sidecar_snapshot(
                        path,
                        &namespace.directory.expected_path,
                        HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.kura.geometry_error(
                            ErrorKind::InvalidData,
                            "historical recovery file disappeared after durability sync",
                        )
                    })?;
                if after.bytes_hash != observed.bytes_hash
                    || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata)
                {
                    return Err(self.kura.geometry_error(
                        ErrorKind::InvalidData,
                        "historical recovery file changed during durability sync",
                    ));
                }
            }
            sync_dir(&namespace.directory.expected_path)
                .map_err(|error| Error::IO(error, namespace.directory.expected_path.clone()))?;
        }
        self.ensure_unchanged()?;
        Ok((self.records, self.encoded_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kura::tests::historical_geometry_observation_fixture;
    use tempfile::TempDir;

    fn tree(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
        fn visit(root: &Path, path: &Path, out: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
            let mut entries = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                let is_dir = fs::symlink_metadata(&path).unwrap().is_dir();
                out.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    is_dir,
                    if is_dir {
                        Vec::new()
                    } else {
                        fs::read(&path).unwrap()
                    },
                ));
                if is_dir {
                    visit(root, &path, out);
                }
            }
        }
        let mut out = Vec::new();
        visit(root, root, &mut out);
        out
    }

    fn inventory(kura: &Kura, outer: &Path) -> BoundProgressDirectorySnapshot {
        let directory = Kura::open_bound_progress_directory(&kura.store_root(), outer).unwrap();
        kura.geometry_bound_progress_directory_snapshot(
            &directory,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "historical observation test",
        )
        .unwrap()
    }

    fn observe<'a>(
        kura: &'a Kura,
        outer: &Path,
        inventory: &'a BoundProgressDirectorySnapshot,
        record: &HistoricalAutonomousLaneRecoveryRecordV1,
        byte_limit: u64,
    ) -> Result<ObservedHistoricalRecoveryEvidence<'a>> {
        let descriptor = &record.payload.origin_proposal.descriptor;
        kura.observe_geometry_historical_autonomous_recovery_records(
            outer,
            inventory,
            descriptor.lane_id,
            Some(descriptor.dataspace_id),
            descriptor.lane_incarnation,
            0,
            1,
            byte_limit,
            "historical observation test",
        )
    }

    #[test]
    fn historical_geometry_observation_and_drop_preserve_exact_files() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let length = fs::metadata(&path).unwrap().len();
        let before = tree(temp.path());
        let observed = observe(&kura, outer, &inventory, &record, length).unwrap();
        assert_eq!(observed.records.as_slice(), std::slice::from_ref(&record));
        assert_eq!(observed.encoded_bytes, length);
        drop(observed);
        assert_eq!(tree(temp.path()), before);
        let observed = observe(&kura, outer, &inventory, &record, length).unwrap();
        assert_eq!(observed.attest().unwrap(), (vec![record.clone()], length));
        let descriptor = &record.payload.origin_proposal.descriptor;
        assert_eq!(
            kura.read_and_attest_geometry_historical_autonomous_recovery_records(
                outer,
                &inventory,
                descriptor.lane_id,
                Some(descriptor.dataspace_id),
                descriptor.lane_incarnation,
                0,
                1,
                length,
                "historical wrapper test",
            )
            .unwrap(),
            (vec![record], length)
        );
        assert_eq!(tree(temp.path()), before);
    }

    #[test]
    fn historical_geometry_attestation_rejects_changed_record() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let length = fs::metadata(&path).unwrap().len();
        let observed = observe(&kura, outer, &inventory, &record, length).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.last_mut().unwrap();
        *last ^= 1;
        fs::write(&path, bytes).unwrap();
        let changed = tree(temp.path());
        assert!(observed.attest().is_err());
        assert_eq!(tree(temp.path()), changed);
    }

    #[test]
    fn historical_geometry_attestation_rejects_new_sibling() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let length = fs::metadata(&path).unwrap().len();
        let observed = observe(&kura, outer, &inventory, &record, length).unwrap();
        fs::write(
            path.parent().unwrap().join("unowned.norito.tmp"),
            b"pending",
        )
        .unwrap();
        let changed = tree(temp.path());
        assert!(observed.attest().is_err());
        assert_eq!(tree(temp.path()), changed);
    }

    #[test]
    fn historical_geometry_observation_leaves_temporary_for_maintenance() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        fs::rename(&path, path.with_extension("norito.tmp")).unwrap();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let before = tree(temp.path());
        let error = observe(
            &kura,
            outer,
            &inventory,
            &record,
            kura.historical_autonomous_recovery_aggregate_byte_limit(),
        )
        .err()
        .unwrap();
        assert!(
            error
                .to_string()
                .contains("temporary, noncanonical, or unknown"),
            "{error}"
        );
        assert_eq!(tree(temp.path()), before);
    }

    #[test]
    fn historical_geometry_observation_enforces_exact_byte_budget() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let length = fs::metadata(&path).unwrap().len();
        let before = tree(temp.path());
        let error = observe(&kura, outer, &inventory, &record, length - 1)
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("bytes exceed their hard bound"),
            "{error}"
        );
        assert_eq!(tree(temp.path()), before);
        assert!(observe(&kura, outer, &inventory, &record, length).is_ok());
    }

    #[test]
    fn historical_geometry_observation_binds_absent_namespace() {
        let temp = TempDir::new().unwrap();
        let (kura, path, record) = historical_geometry_observation_fixture(&temp);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        fs::remove_file(&path).unwrap();
        fs::remove_dir(path.parent().unwrap()).unwrap();
        let outer = path.parent().unwrap().parent().unwrap();
        let inventory = inventory(&kura, outer);
        let observed = observe(&kura, outer, &inventory, &record, 0).unwrap();
        assert!(observed.records.is_empty());
        fs::create_dir(path.parent().unwrap()).unwrap();
        let changed = tree(temp.path());
        assert!(observed.attest().is_err());
        assert_eq!(tree(temp.path()), changed);
    }
}
