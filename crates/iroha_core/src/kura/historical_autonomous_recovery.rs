const HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1: &str = "historical_autonomous_recoveries_v1";
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1: u16 = 1;
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS: usize = 4_096;
const HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES: u64 =
    V2_PENDING_CONTROL_SIDECAR_BYTES_MAX as u64;

fn historical_autonomous_recovery_record_name_is_canonical(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_suffix(".norito"))
        .is_some_and(|stem| {
            stem.len() == Hash::LENGTH * 2
                && stem
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

/// Enumerate one historical-recovery namespace without ever retaining more
/// than the caller's remaining global record/byte budget.
///
/// The caller supplies the identity binder so startup replay can retain exact
/// canonical file identities while ordinary readers and disk accounting can
/// reuse the same filename, file-kind, link-count, per-file, count, and
/// aggregate-byte gates.
fn bounded_historical_autonomous_recovery_entries<T>(
    directory: &Path,
    record_limit: usize,
    aggregate_byte_limit: u64,
    mut bind: impl FnMut(&Path) -> Result<(T, std::fs::Metadata)>,
) -> Result<(Vec<(PathBuf, T)>, u64)> {
    if record_limit > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
        || aggregate_byte_limit > HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "historical autonomous recovery scan exceeds its hard bounds",
            ),
            directory.to_path_buf(),
        ));
    }

    let before = std::fs::symlink_metadata(directory)
        .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
    if before.file_type().is_symlink() || !before.file_type().is_dir() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "historical autonomous recovery namespace is not a direct directory",
            ),
            directory.to_path_buf(),
        ));
    }
    let entries =
        std::fs::read_dir(directory).map_err(|error| Error::IO(error, directory.to_path_buf()))?;
    let mut bounded = Vec::with_capacity(record_limit.min(64));
    let mut encoded_bytes = 0_u64;
    for entry in entries {
        let entry = entry.map_err(|error| Error::IO(error, directory.to_path_buf()))?;
        let path = entry.path();
        if bounded.len() >= record_limit {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "historical autonomous recovery record count exceeds its hard bound",
                ),
                path,
            ));
        }
        if path.parent() != Some(directory)
            || !historical_autonomous_recovery_record_name_is_canonical(&entry.file_name())
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "historical autonomous recovery namespace has a temporary, noncanonical, or unknown entry",
                ),
                path,
            ));
        }
        let (bound, metadata) = bind(&path)?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || !Kura::sidecar_is_single_link(&metadata)
            || metadata.len() == 0
            || metadata.len() > u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "historical autonomous recovery entry is empty, linked, non-regular, or oversized",
                ),
                path,
            ));
        }
        encoded_bytes = encoded_bytes
            .checked_add(metadata.len())
            .filter(|bytes| *bytes <= aggregate_byte_limit)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "historical autonomous recovery bytes exceed their hard bound",
                    ),
                    directory.to_path_buf(),
                )
            })?;
        let after_bind = std::fs::symlink_metadata(&path)
            .map_err(|error| Error::IO(error, path.clone()))?;
        if !Kura::sidecar_file_metadata_unchanged(&metadata, &after_bind) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "historical autonomous recovery entry changed during its bounded scan",
                ),
                path,
            ));
        }
        bounded.push((path, bound, metadata));
    }
    let after = std::fs::symlink_metadata(directory)
        .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
    if after.file_type().is_symlink()
        || !after.file_type().is_dir()
        || !Kura::sidecar_directory_metadata_unchanged(&before, &after)
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "historical autonomous recovery namespace changed during its bounded scan",
            ),
            directory.to_path_buf(),
        ));
    }
    for (path, _, accounted) in &bounded {
        let current = std::fs::symlink_metadata(path)
            .map_err(|error| Error::IO(error, path.clone()))?;
        if !Kura::sidecar_file_metadata_unchanged(accounted, &current) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "historical autonomous recovery entry changed after bounded accounting",
                ),
                path.clone(),
            ));
        }
    }
    bounded.sort_by(|(left, _, _), (right, _, _)| left.file_name().cmp(&right.file_name()));
    Ok((
        bounded
            .into_iter()
            .map(|(path, bound, _)| (path, bound))
            .collect(),
        encoded_bytes,
    ))
}

fn accumulate_historical_autonomous_recovery_bytes(
    current: u64,
    encoded_len: usize,
    exact_duplicate: bool,
    aggregate_byte_limit: u64,
) -> Option<u64> {
    if exact_duplicate {
        return (current <= aggregate_byte_limit).then_some(current);
    }
    current
        .checked_add(u64::try_from(encoded_len).ok()?)
        .filter(|total| *total <= aggregate_byte_limit)
}

fn historical_autonomous_recovery_read_matches_accounting(
    accounted: &std::fs::Metadata,
    read: &StableSidecarRead,
) -> bool {
    u64::try_from(read.bytes.len()).ok() == Some(accounted.len())
        && Kura::sidecar_file_metadata_unchanged(accounted, &read.metadata.file)
}

/// Immutable, self-contained Kura seal for historical autonomous lane work.
///
/// Records live below the exact active lane-incarnation directory. Lane
/// relabel, retirement, archive GC, and recreation therefore move or retire
/// the record with the rest of that incarnation instead of leaving a global
/// orphan which a later incarnation could accidentally hydrate.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct HistoricalAutonomousLaneRecoveryRecordV1 {
    pub(crate) version: u16,
    pub(crate) recovery_id: Hash,
    pub(crate) canonical_body: crate::sumeragi::message::CanonicalExecutedBlockNeedV1,
    pub(crate) historical_context: HeightContext,
    pub(crate) historical_context_id: HeightContextId,
    pub(crate) historical_context_hash: HashOf<HeightContext>,
    pub(crate) carrier_view: u64,
    pub(crate) payload: LaneExecutablePayloadV1,
    pub(crate) reservation_group: LaneQueueReservationReconciliationGroupV1,
    /// Complete PoP vector in `payload.origin_proposal.descriptor.validator_set`
    /// order. It is intentionally not a sparse signer map: this record must be
    /// enough to resume the unfinished two-phase lane session after pruning.
    pub(crate) validator_pops: Vec<Vec<u8>>,
}

impl HistoricalAutonomousLaneRecoveryRecordV1 {
    pub(crate) fn from_install(
        install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        validator_pops: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            version: HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1,
            recovery_id: install.recovery_id,
            canonical_body: install.canonical_body,
            historical_context: install.historical_context.clone(),
            historical_context_id: install.historical_context_id,
            historical_context_hash: install.historical_context_hash,
            carrier_view: install.carrier_view,
            payload: install.payload.clone(),
            reservation_group: install.reservation_group.clone(),
            validator_pops,
        }
    }

    pub(crate) fn installation_input(
        &self,
    ) -> crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1 {
        crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1 {
            version: crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1::VERSION,
            recovery_id: self.recovery_id,
            canonical_body: self.canonical_body,
            historical_context: self.historical_context.clone(),
            historical_context_id: self.historical_context_id,
            historical_context_hash: self.historical_context_hash,
            carrier_view: self.carrier_view,
            payload: self.payload.clone(),
            reservation_group: self.reservation_group.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoricalAutonomousLaneRecoveryPersistOutcome {
    Installed,
    AlreadyInstalled,
}

macro_rules! kura_historical_autonomous_recovery_methods {
    () => {
        fn historical_autonomous_recovery_aggregate_byte_limit(&self) -> u64 {
            u64::try_from(self.pending_control_sidecar_limits.aggregate_bytes)
                .expect("validated pending-control sidecar bytes fit u64")
        }

        #[cfg(test)]
        pub(crate) fn historical_autonomous_recovery_inventory_scans_for_test(&self) -> usize {
            self.historical_autonomous_recovery_inventory_scans
                .load(Ordering::Relaxed)
        }

        #[cfg(test)]
        pub(crate) fn reset_historical_autonomous_recovery_inventory_scans_for_test(&self) {
            self.historical_autonomous_recovery_inventory_scans
                .store(0, Ordering::Relaxed);
        }

        fn historical_autonomous_recovery_directory_for_entry(
            entry: &LaneConfigEntry,
            store_root: &Path,
        ) -> PathBuf {
            Self::lane_artifact_dir(&entry.blocks_dir(store_root))
                .join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
        }

        fn historical_autonomous_recovery_path_for_entry(
            entry: &LaneConfigEntry,
            store_root: &Path,
            recovery_id: Hash,
        ) -> PathBuf {
            Self::historical_autonomous_recovery_directory_for_entry(entry, store_root)
                .join(format!("{}.norito", hex::encode(recovery_id.as_ref())))
        }

        fn invalid_historical_autonomous_recovery(
            path: PathBuf,
            detail: impl Into<String>,
        ) -> Error {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidData, detail.into()),
                path,
            )
        }

        fn validate_historical_autonomous_recovery_record_shape(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
            path: &Path,
        ) -> Result<()> {
            let install = record.installation_input();
            let descriptor = &record.payload.origin_proposal.descriptor;
            let identity = &record.reservation_group.identity;
            let hint = record
                .payload
                .origin_proposal
                .payload_block_hint
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        "historical autonomous recovery payload has no canonical carrier hint",
                    )
                })?;
            if record.version != HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1
                || !install.has_valid_identity()
                || record.canonical_body.height == 0
                || record.canonical_body.executed_block_wire_len == 0
                || record.canonical_body.executed_block_wire_len > STRICT_INIT_MAX_BLOCK_BYTES
                || record.canonical_body.executed_block_wire_len
                    != record
                        .canonical_body
                        .execution_commitment
                        .executed_block_wire_len
                || record.canonical_body.executed_block_wire_hash
                    != record
                        .canonical_body
                        .execution_commitment
                        .executed_block_wire_hash
                || record.canonical_body.execution_commitment.validate().is_err()
                || record.historical_context.validate().is_err()
                || record.historical_context.height != record.canonical_body.height
                || record.historical_context.id() != record.historical_context_id
                || HashOf::new(&record.historical_context) != record.historical_context_hash
                || record.carrier_view != hint.proposal_view
                || hint.proposal_height != record.canonical_body.height
                || hint.proposal_block_hash != record.canonical_body.block_hash
                || descriptor.lane_id != identity.lane_id
                || descriptor.dataspace_id != identity.dataspace_id
                || descriptor.lane_incarnation != identity.lane_incarnation
                || descriptor.proposal_height != identity.proposal_height
                || descriptor.lane_block_height != identity.lane_block_height
                || descriptor.lane_block_view != identity.lane_block_view
                || record.payload.reservation_keys != record.reservation_group.ordered_keys
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery record has invalid context, carrier, slot, or execution bindings",
                ));
            }
            Self::validate_autonomous_reservation_reconciliation_group(&record.reservation_group)
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        format!("historical autonomous recovery group is invalid: {error}"),
                    )
                })?;
            record
                .payload
                .validate(record.payload.chain_id_hash, record.payload.epoch)
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        format!("historical autonomous recovery payload is invalid: {error}"),
                    )
                })?;
            if descriptor.validator_set.is_empty()
                || descriptor.validator_set.len() > crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS
                || record.validator_pops.len() != descriptor.validator_set.len()
                || descriptor
                    .validator_set
                    .iter()
                    .zip(&record.validator_pops)
                    .any(|(validator, pop)| {
                        pop.len() != crate::lane_consensus::LANE_BLS_PROOF_BYTES
                            || iroha_crypto::bls_normal_pop_verify(
                                validator.public_key(),
                                pop.as_slice(),
                            )
                            .is_err()
                    })
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery PoPs are missing, misordered, oversized, or invalid",
                ));
            }
            let bytes = record.encode();
            if bytes.is_empty() || bytes.len() > HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery record exceeds its hard byte limit",
                ));
            }
            Ok(())
        }

        fn read_historical_autonomous_recovery_record(
            &self,
            path: &Path,
            directory: &Path,
        ) -> Result<Option<HistoricalAutonomousLaneRecoveryRecordV1>> {
            self.read_historical_autonomous_recovery_record_from_inventory(
                path, directory, None,
            )
        }

        fn read_historical_autonomous_recovery_record_from_inventory(
            &self,
            path: &Path,
            directory: &Path,
            accounted: Option<&std::fs::Metadata>,
        ) -> Result<Option<HistoricalAutonomousLaneRecoveryRecordV1>> {
            let Some(snapshot) = self.read_regular_sidecar_snapshot(
                path,
                directory,
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
            )? else {
                return Ok(None);
            };
            if accounted.is_some_and(|accounted| {
                !historical_autonomous_recovery_read_matches_accounting(accounted, &snapshot)
            }) {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery record changed after bounded accounting",
                ));
            }
            let mut cursor = snapshot.bytes.as_slice();
            let record = HistoricalAutonomousLaneRecoveryRecordV1::decode_all(&mut cursor)
                .map_err(Error::NoritoFrame)?;
            let expected_name = format!("{}.norito", hex::encode(record.recovery_id.as_ref()));
            if record.encode() != snapshot.bytes
                || path.file_name().and_then(std::ffi::OsStr::to_str)
                    != Some(expected_name.as_str())
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery record is noncanonical, unsupported, or mis-associated",
                ));
            }
            self.validate_historical_autonomous_recovery_record_shape(&record, path)?;
            Ok(Some(record))
        }

        fn validate_historical_autonomous_recovery_dependencies(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
            path: &Path,
        ) -> Result<()> {
            let descriptor = &record.payload.origin_proposal.descriptor;
            let durable_payload = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let entry = self.lane_storage_entry(descriptor.lane_id)?;
                self.require_active_lane_artifact(&entry, descriptor)?;
                let _sidecar_guard = self.sidecar_lock.lock();
                let durable = self
                    .read_current_autonomous_lane_block_record_self_context_locked(
                        &entry,
                        descriptor.lane_block_height,
                        false,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.to_path_buf(),
                            "historical autonomous recovery payload is not independently durable",
                        )
                    })?;
                if durable.retirement.is_some() {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        "historical autonomous recovery payload was durably retired",
                    ));
                }
                durable.artifact.executable_payload
            };
            if durable_payload != record.payload {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery payload differs from durable Kura bytes",
                ));
            }
            let recovered = self
                .recover_autonomous_lane_block_payload_with_sidecar_repair(
                    &record.payload.origin_proposal,
                    record.payload.chain_id_hash,
                    record.payload.epoch,
                    false,
                )
                .map_err(|availability| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        format!(
                            "historical autonomous execution input recovery failed: {availability:?}"
                        ),
                    )
                })?;
            let input = self
                .read_lane_block_execution_input_with_repair_policy(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    false,
                )
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        "historical autonomous execution input is not durably readable",
                    )
                })?;
            if input != LaneBlockExecutionInputArtifact::new(recovered) {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous execution input differs from the recovered payload",
                ));
            }
            Ok(())
        }

        /// Revalidate one record returned by the bounded inventory against its
        /// exact immutable file plus the ordinary autonomous payload and
        /// execution-input sidecars. Holding the prune lock across the direct
        /// record read and dependency checks prevents retirement from turning
        /// a successfully hydrated record into an archived owner mid-check.
        pub(crate) fn validate_historical_autonomous_lane_recovery_record_dependencies(
            &self,
            expected: &HistoricalAutonomousLaneRecoveryRecordV1,
        ) -> Result<()> {
            let _prune_guard = self.prune_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            let descriptor = &expected.payload.origin_proposal.descriptor;
            let (path, directory) = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let entry = self.lane_storage_entry(descriptor.lane_id)?;
                self.require_active_lane_artifact(&entry, descriptor)?;
                (
                    Self::historical_autonomous_recovery_path_for_entry(
                        &entry,
                        &self.store_root,
                        expected.recovery_id,
                    ),
                    Self::historical_autonomous_recovery_directory_for_entry(
                        &entry,
                        &self.store_root,
                    ),
                )
            };
            let actual = self
                .read_historical_autonomous_recovery_record(&path, &directory)?
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous recovery record disappeared during hydration",
                    )
                })?;
            if actual != *expected {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.clone(),
                    "historical autonomous recovery record changed after bounded inventory",
                ));
            }
            self.validate_historical_autonomous_recovery_dependencies(expected, &path)
        }

        fn validate_historical_autonomous_recovery_inventory_collisions(
            &self,
            records: &[HistoricalAutonomousLaneRecoveryRecordV1],
        ) -> Result<()> {
            let mut by_recovery = BTreeMap::new();
            let mut by_slot = BTreeMap::new();
            let mut by_proposal = BTreeMap::new();
            let mut by_transaction = BTreeMap::new();
            for record in records {
                let descriptor = &record.payload.origin_proposal.descriptor;
                let record_hash = HashOf::new(record);
                let slot = (
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.lane_block_height,
                );
                let path = self.store_root.clone();
                if by_recovery
                    .insert(record.recovery_id, record_hash)
                    .is_some_and(|existing| existing != record_hash)
                {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "historical autonomous recovery ID aliases different canonical record bytes",
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
                            path,
                            "historical autonomous recovery inventory contains a path, slot, or proposal collision",
                        ));
                    }
                }
                for key in &record.reservation_group.ordered_keys {
                    if by_transaction
                        .insert(key.signed_transaction_hash, record.recovery_id)
                        .is_some_and(|existing| existing != record.recovery_id)
                    {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            self.store_root.clone(),
                            "historical autonomous recovery inventory aliases FIFO transaction ownership",
                        ));
                    }
                }
            }
            Ok(())
        }

        /// Read the complete active-incarnation recovery namespace under both
        /// caller and hard limits. Unknown, temporary, linked, or noncanonical
        /// entries fail closed; archived incarnations are deliberately outside
        /// this active namespace.
        pub(crate) fn historical_autonomous_lane_recovery_records_bounded(
            &self,
            limit: usize,
        ) -> Result<Vec<HistoricalAutonomousLaneRecoveryRecordV1>> {
            #[cfg(test)]
            self.historical_autonomous_recovery_inventory_scans
                .fetch_add(1, Ordering::Relaxed);
            if limit == 0 || limit > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS {
                return Err(Self::invalid_historical_autonomous_recovery(
                    self.store_root.clone(),
                    "historical autonomous recovery reader has an invalid record limit",
                ));
            }
            let _prune_guard = self.prune_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            let _geometry_guard = self.lane_geometry_lock.lock();
            let entries = self
                .lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>();
            let _sidecar_guard = self.sidecar_lock.lock();
            let mut records = Vec::new();
            let mut aggregate_bytes = 0_u64;
            let aggregate_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
            for entry in entries {
                let directory = Self::historical_autonomous_recovery_directory_for_entry(
                    &entry,
                    &self.store_root,
                );
                if self.canonical_sidecar_directory(&directory)?.is_none() {
                    continue;
                }
                let remaining_records = limit.saturating_sub(records.len());
                let remaining_bytes = aggregate_byte_limit.checked_sub(aggregate_bytes).ok_or_else(
                    || {
                        Self::invalid_historical_autonomous_recovery(
                            directory.clone(),
                            "historical autonomous recovery aggregate byte count overflowed",
                        )
                    },
                )?;
                let (directory_entries, directory_bytes) =
                    bounded_historical_autonomous_recovery_entries(
                        &directory,
                        remaining_records,
                        remaining_bytes,
                        |path| {
                            let metadata = std::fs::symlink_metadata(path)
                                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                            Ok((metadata.clone(), metadata))
                        },
                    )?;
                aggregate_bytes = aggregate_bytes
                    .checked_add(directory_bytes)
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            directory.clone(),
                            "historical autonomous recovery aggregate byte count overflowed",
                        )
                    })?;
                for (path, accounted) in directory_entries {
                    let record = self
                        .read_historical_autonomous_recovery_record_from_inventory(
                            &path,
                            &directory,
                            Some(&accounted),
                        )?
                        .ok_or_else(|| {
                            Self::invalid_historical_autonomous_recovery(
                                path.clone(),
                                "historical autonomous recovery record disappeared during bounded inventory",
                            )
                        })?;
                    if record.payload.origin_proposal.descriptor.lane_id != entry.lane_id
                        || record.payload.origin_proposal.descriptor.dataspace_id != entry.dataspace_id
                    {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path,
                            "historical autonomous recovery record is stored in another lane namespace",
                        ));
                    }
                    self.require_active_lane_artifact(
                        &entry,
                        &record.payload.origin_proposal.descriptor,
                    )?;
                    records.push(record);
                }
            }
            records.sort_by_key(|record| {
                let descriptor = &record.payload.origin_proposal.descriptor;
                (
                    descriptor.proposal_height,
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    record.recovery_id,
                )
            });
            self.validate_historical_autonomous_recovery_inventory_collisions(&records)?;
            Ok(records)
        }

        /// Prove that normal autonomous-payload and execution-input writers
        /// will accept this record before an all-item runner batch mutates its
        /// first file. This is deliberately read-only; crash repair remains the
        /// responsibility of the later guarded persistence calls.
        fn preflight_historical_autonomous_recovery_install_dependencies(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
            path: &Path,
        ) -> Result<()> {
            let descriptor = &record.payload.origin_proposal.descriptor;
            let expected_input = Self::autonomous_lane_block_execution_input_candidate(
                &record.payload,
                record.payload.chain_id_hash,
                record.payload.epoch,
            )
            .map_err(|availability| {
                Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    format!(
                        "historical autonomous execution-input preflight failed: {availability:?}"
                    ),
                )
            })?;
            let _geometry_guard = self.lane_geometry_lock.lock();
            let entry = self.lane_storage_entry(descriptor.lane_id)?;
            self.require_active_lane_artifact(&entry, descriptor)?;
            let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
                &entry,
                &self.store_root,
                descriptor.lane_block_height,
                descriptor.proposal_height,
            );
            let (input_data_path, input_index_path) =
                Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
            let _sidecar_guard = self.sidecar_lock.lock();
            if let Some(existing) = self
                .read_current_autonomous_lane_block_record_self_context_locked(
                    &entry,
                    descriptor.lane_block_height,
                    false,
                )?
            {
                let existing_payload = &existing.artifact.executable_payload;
                let exact_or_promotable = existing_payload == &record.payload
                    || (existing.retirement.is_none()
                        && existing_payload
                            .origin_proposal
                            .payload_block_hint
                            .is_none()
                        && record
                            .payload
                            .origin_proposal
                            .payload_block_hint
                            .is_some()
                        && existing_payload
                            .attach_global_hint_exact(
                                record
                                    .payload
                                    .origin_proposal
                                    .payload_block_hint
                                    .expect("checked present"),
                                record.payload.chain_id_hash,
                                record.payload.epoch,
                            )
                            .is_ok_and(|promoted| promoted == record.payload));
                if existing.retirement.is_some() || !exact_or_promotable {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        attempt_path,
                        "historical autonomous payload conflicts with the active lane-height owner",
                    ));
                }
            }
            self.preflight_autonomous_lane_entrypoint_claims_locked(
                &record.payload,
                MAX_AUTONOMOUS_LANE_CLAIM_FILES,
            )?;

            let existing_input = Self::read_indexed_sidecar_from_paths(
                descriptor.lane_block_height,
                &input_data_path,
                &input_index_path,
                norito::decode_canonical::<LaneBlockExecutionInputArtifact>,
                "historical autonomous lane block execution input preflight",
            );
            match existing_input {
                Some(existing) if existing != expected_input => {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        input_data_path,
                        "historical autonomous execution input conflicts with durable bytes",
                    ));
                }
                Some(_) => {}
                None => {
                    let tracked = Self::sidecar_tracked_bytes(
                        &input_data_path,
                        &input_index_path,
                        None,
                    )?;
                    if tracked != 0 {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            input_data_path,
                            "historical autonomous execution-input sidecar is non-empty but unreadable",
                        ));
                    }
                }
            }
            Ok(())
        }

        fn preflight_historical_autonomous_lane_recovery_records_with_inventory(
            &self,
            incoming: &[HistoricalAutonomousLaneRecoveryRecordV1],
        ) -> Result<BTreeSet<Hash>> {
            if incoming.is_empty()
                || incoming.len() > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    self.store_root.clone(),
                    "historical autonomous recovery batch is empty or exceeds its hard record limit",
                ));
            }
            let mut combined = self.historical_autonomous_lane_recovery_records_bounded(
                HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            )?;
            let existing_recovery_ids = combined
                .iter()
                .map(|record| record.recovery_id)
                .collect::<BTreeSet<_>>();
            let mut recovery_positions = combined
                .iter()
                .enumerate()
                .map(|(index, record)| (record.recovery_id, index))
                .collect::<BTreeMap<_, _>>();
            let mut combined_encoded_bytes = 0_u64;
            let aggregate_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
            for existing in &combined {
                let encoded = norito::encode_canonical(existing).map_err(Error::NoritoFrame)?;
                combined_encoded_bytes = accumulate_historical_autonomous_recovery_bytes(
                    combined_encoded_bytes,
                    encoded.len(),
                    false,
                    aggregate_byte_limit,
                )
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        self.store_root.clone(),
                        "existing historical autonomous recovery bytes exceed their aggregate bound",
                    )
                })?;
            }
            for record in incoming {
                let descriptor = &record.payload.origin_proposal.descriptor;
                let entry = self.lane_storage_entry(descriptor.lane_id)?;
                let directory = Self::historical_autonomous_recovery_directory_for_entry(
                    &entry,
                    &self.store_root,
                );
                let path = Self::historical_autonomous_recovery_path_for_entry(
                    &entry,
                    &self.store_root,
                    record.recovery_id,
                );
                self.validate_historical_autonomous_recovery_record_shape(record, &path)?;
                let encoded = norito::encode_canonical(record).map_err(Error::NoritoFrame)?;
                if let Some(parent) = directory.parent() {
                    self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            parent.to_path_buf(),
                            "historical autonomous recovery lane-artifact directory is unavailable",
                        )
                    })?;
                }
                if let Some(existing_index) = recovery_positions
                    .get(&record.recovery_id)
                    .copied()
                {
                    let existing = &combined[existing_index];
                    if existing != record {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery path conflicts with existing immutable bytes",
                        ));
                    }
                    combined_encoded_bytes = accumulate_historical_autonomous_recovery_bytes(
                        combined_encoded_bytes,
                        encoded.len(),
                        true,
                        aggregate_byte_limit,
                    )
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery bytes exceed their aggregate bound",
                        )
                    })?;
                    if existing_recovery_ids.contains(&record.recovery_id) {
                        self.validate_historical_autonomous_recovery_dependencies(
                            existing, &path,
                        )?;
                    }
                } else {
                    combined_encoded_bytes = accumulate_historical_autonomous_recovery_bytes(
                        combined_encoded_bytes,
                        encoded.len(),
                        false,
                        aggregate_byte_limit,
                    )
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery batch exceeds its aggregate byte bound",
                        )
                    })?;
                    self.preflight_historical_autonomous_recovery_install_dependencies(
                        record, &path,
                    )?;
                    recovery_positions.insert(record.recovery_id, combined.len());
                    combined.push(record.clone());
                }
            }
            if combined.len() > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS {
                return Err(Self::invalid_historical_autonomous_recovery(
                    self.store_root.clone(),
                    "historical autonomous recovery batch exceeds namespace capacity",
                ));
            }
            self.validate_historical_autonomous_recovery_inventory_collisions(&combined)?;
            Ok(existing_recovery_ids)
        }

        #[cfg(test)]
        pub(crate) fn historical_autonomous_lane_recovery_record_matches(
            &self,
            expected: &HistoricalAutonomousLaneRecoveryRecordV1,
        ) -> Result<bool> {
            let records = self.historical_autonomous_lane_recovery_records_bounded(
                HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            )?;
            let Some(record) = records
                .into_iter()
                .find(|record| record.recovery_id == expected.recovery_id)
            else {
                return Ok(false);
            };
            let descriptor = &record.payload.origin_proposal.descriptor;
            let entry = self.lane_storage_entry(descriptor.lane_id)?;
            let path = Self::historical_autonomous_recovery_path_for_entry(
                &entry,
                &self.store_root,
                record.recovery_id,
            );
            if &record != expected {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record conflicts with requested immutable bytes",
                ));
            }
            self.validate_historical_autonomous_recovery_dependencies(&record, &path)?;
            Ok(true)
        }

        /// Match an in-memory planner DTO to its complete durable record. The
        /// record reader still validates the stored ordered PoPs; the DTO does
        /// not get to supply or override that historical authority.
        #[cfg(test)]
        pub(crate) fn historical_autonomous_lane_recovery_matches(
            &self,
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        ) -> Result<bool> {
            let records = self.historical_autonomous_lane_recovery_records_bounded(
                HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            )?;
            let Some(record) = records
                .into_iter()
                .find(|record| record.recovery_id == install.recovery_id)
            else {
                return Ok(false);
            };
            let descriptor = &record.payload.origin_proposal.descriptor;
            let entry = self.lane_storage_entry(descriptor.lane_id)?;
            let path = Self::historical_autonomous_recovery_path_for_entry(
                &entry,
                &self.store_root,
                record.recovery_id,
            );
            if record.installation_input() != *install {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record conflicts with requested installation",
                ));
            }
            self.validate_historical_autonomous_recovery_dependencies(&record, &path)?;
            Ok(true)
        }

        /// Persist one record after the caller completed the all-item namespace
        /// preflight. This path performs only direct dependency and immutable
        /// file checks; it never rescans the complete recovery namespace.
        fn persist_preflighted_historical_autonomous_lane_recovery_record(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
            existing_from_preflight: bool,
        ) -> Result<HistoricalAutonomousLaneRecoveryPersistOutcome> {
            if existing_from_preflight {
                self.validate_historical_autonomous_lane_recovery_record_dependencies(record)?;
                return Ok(HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled);
            }

            self.persist_lane_executable_payload(
                &record.payload,
                record.payload.chain_id_hash,
                record.payload.epoch,
            )?;
            let recovered = self
                .recover_autonomous_lane_block_payload(
                    &record.payload.origin_proposal,
                    record.payload.chain_id_hash,
                    record.payload.epoch,
                )
                .map_err(|availability| {
                    Self::invalid_historical_autonomous_recovery(
                        self.store_root.clone(),
                        format!(
                            "historical autonomous execution input recovery failed after payload persistence: {availability:?}"
                        ),
                    )
                })?;
            self.persist_lane_block_execution_input(&recovered)?;

            let descriptor = &record.payload.origin_proposal.descriptor;
            let provisional_entry = self.lane_storage_entry(descriptor.lane_id)?;
            let provisional_path = Self::historical_autonomous_recovery_path_for_entry(
                &provisional_entry,
                &self.store_root,
                record.recovery_id,
            );
            self.validate_historical_autonomous_recovery_dependencies(
                record,
                &provisional_path,
            )?;
            let bytes = record.encode();

            let accounting_mutation = self.begin_total_disk_usage_mutation();
            let _geometry_guard = self.lane_geometry_lock.lock();
            let entry = self.lane_storage_entry(descriptor.lane_id)?;
            self.require_active_lane_artifact(&entry, descriptor)?;
            let directory = Self::historical_autonomous_recovery_directory_for_entry(
                &entry,
                &self.store_root,
            );
            let path = Self::historical_autonomous_recovery_path_for_entry(
                &entry,
                &self.store_root,
                record.recovery_id,
            );
            let _sidecar_guard = self.sidecar_lock.lock();
            if self.canonical_sidecar_directory(&directory)?.is_none() {
                let parent = directory.parent().ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        directory.clone(),
                        "historical autonomous recovery directory has no parent",
                    )
                })?;
                self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        parent.to_path_buf(),
                        "historical autonomous recovery parent is unavailable",
                    )
                })?;
                create_dir_all_with_context(&directory)?;
                self.canonical_sidecar_directory(&directory)?.ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        directory.clone(),
                        "historical autonomous recovery directory disappeared after creation",
                    )
                })?;
                sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
            }

            if let Some(existing) =
                self.read_historical_autonomous_recovery_record(&path, &directory)?
            {
                if existing != *record {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "immutable historical autonomous recovery seal conflicts after dependency persistence",
                    ));
                }
                accounting_mutation.finish();
                return Ok(HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled);
            }
            let wrote = self.write_atomic_synced_noclobber(&path, &bytes)?;
            if wrote {
                self.update_disk_usage_delta(0, u64::try_from(bytes.len())?);
            } else {
                let existing = self
                    .read_historical_autonomous_recovery_record(&path, &directory)?
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery no-clobber collision disappeared",
                        )
                    })?;
                if existing != *record {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "historical autonomous recovery no-clobber collision has conflicting bytes",
                    ));
                }
            }
            let confirmed = self
                .read_historical_autonomous_recovery_record(&path, &directory)?
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous recovery record is absent after publication",
                    )
                })?;
            if confirmed != *record {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record changed during read-back",
                ));
            }
            accounting_mutation.finish();
            self.note_committed_lane_status_change();
            Ok(if wrote {
                HistoricalAutonomousLaneRecoveryPersistOutcome::Installed
            } else {
                HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled
            })
        }

        /// Persist one all-or-restart batch with exactly one complete bounded
        /// inventory/preflight pass. Collision checks use ordered indexes;
        /// subsequent per-record work uses direct paths and immutable readback.
        pub(crate) fn persist_historical_autonomous_lane_recovery_records(
            &self,
            records: &[HistoricalAutonomousLaneRecoveryRecordV1],
        ) -> Result<Vec<HistoricalAutonomousLaneRecoveryPersistOutcome>> {
            let _historical_recovery_guard =
                self.historical_autonomous_recovery_mutation_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            self.durable_mutation_authorized()?;
            let existing_recovery_ids = self
                .preflight_historical_autonomous_lane_recovery_records_with_inventory(records)?;
            let mut outcomes = Vec::new();
            outcomes.try_reserve_exact(records.len())?;
            for record in records {
                outcomes.push(
                    self.persist_preflighted_historical_autonomous_lane_recovery_record(
                        record,
                        existing_recovery_ids.contains(&record.recovery_id),
                    )?,
                );
            }
            Ok(outcomes)
        }

        /// Persist carrier-extracted executable bytes through the ordinary
        /// autonomous lane path, then persist the execution input, and only then
        /// publish the separate immutable recovery seal.
        pub(crate) fn persist_historical_autonomous_lane_recovery_record(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
        ) -> Result<HistoricalAutonomousLaneRecoveryPersistOutcome> {
            self.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(record))?
                .pop()
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        self.store_root.clone(),
                        "single historical autonomous recovery persistence produced no outcome",
                    )
                })
        }
    };
}

#[cfg(test)]
mod historical_autonomous_recovery_bound_tests {
    use super::*;

    const TEST_AGGREGATE_BYTE_LIMIT: u64 = V2_PENDING_CONTROL_SIDECAR_BYTES.get() as u64;

    fn canonical_record_name(index: usize) -> String {
        format!(
            "{index:0width$x}.norito",
            width = Hash::LENGTH.saturating_mul(2)
        )
    }

    fn scan_metadata(
        directory: &Path,
        record_limit: usize,
        aggregate_byte_limit: u64,
    ) -> Result<(Vec<(PathBuf, ())>, u64)> {
        bounded_historical_autonomous_recovery_entries(
            directory,
            record_limit,
            aggregate_byte_limit,
            |path| {
                let metadata = std::fs::symlink_metadata(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok(((), metadata))
            },
        )
    }

    #[test]
    fn aggregate_byte_bound_is_exact_and_duplicate_aware() {
        let limit = V2_PENDING_CONTROL_SIDECAR_BYTES_MIN as u64;
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(limit - 1, 1, false, limit),
            Some(limit),
        );
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(limit, 1, false, limit),
            None,
            "one unique byte beyond the aggregate bound must fail",
        );
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(
                limit,
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
                true,
                limit,
            ),
            Some(limit),
            "an exact immutable duplicate must not consume aggregate bytes twice",
        );
    }

    #[test]
    fn bounded_namespace_honors_lower_and_higher_configured_limits() {
        let lower = tempfile::tempdir().expect("temporary lower-bound namespace");
        std::fs::write(lower.path().join(canonical_record_name(0)), [0_u8, 1_u8])
            .expect("write lower-bound historical recovery record");
        scan_metadata(lower.path(), 1, 1)
            .expect_err("the caller's configured lower byte bound must be enforced");

        let higher = tempfile::tempdir().expect("temporary higher-bound namespace");
        std::fs::write(higher.path().join(canonical_record_name(0)), [0_u8])
            .expect("write higher-bound historical recovery record");
        let configured_higher = HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES;
        assert!(configured_higher > TEST_AGGREGATE_BYTE_LIMIT);
        let (records, bytes) = scan_metadata(higher.path(), 1, configured_higher)
            .expect("a valid configured limit above the release default must be accepted");
        assert_eq!(records.len(), 1);
        assert_eq!(bytes, 1);

        scan_metadata(
            higher.path(),
            1,
            HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES.saturating_add(1),
        )
        .expect_err("a configured byte limit above the hard maximum must fail closed");
    }

    #[test]
    fn bounded_namespace_rejects_same_path_mutation_during_accounting() {
        let temp = tempfile::tempdir().expect("temporary mutation namespace");
        let path = temp.path().join(canonical_record_name(0));
        std::fs::write(&path, [0_u8]).expect("write historical recovery record");

        bounded_historical_autonomous_recovery_entries(
            temp.path(),
            1,
            TEST_AGGREGATE_BYTE_LIMIT,
            |path| {
                let accounted = std::fs::symlink_metadata(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                std::fs::write(path, [0_u8, 1_u8])
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok(((), accounted))
            },
        )
        .expect_err("same-path mutation after metadata accounting must fail closed");
    }

    #[test]
    fn bounded_namespace_rechecks_every_file_after_enumeration() {
        let temp = tempfile::tempdir().expect("temporary post-enumeration mutation namespace");
        for index in 0..2 {
            std::fs::write(temp.path().join(canonical_record_name(index)), [0_u8])
                .expect("write historical recovery record");
        }
        let mut first_accounted_path: Option<PathBuf> = None;

        bounded_historical_autonomous_recovery_entries(
            temp.path(),
            2,
            TEST_AGGREGATE_BYTE_LIMIT,
            |path| {
                let accounted = std::fs::symlink_metadata(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                if let Some(first) = &first_accounted_path {
                    std::fs::write(first, [0_u8, 1_u8])
                        .map_err(|error| Error::IO(error, first.to_path_buf()))?;
                } else {
                    first_accounted_path = Some(path.to_path_buf());
                }
                Ok(((), accounted))
            },
        )
        .expect_err(
            "an earlier entry changed while a later entry was bound must fail final accounting",
        );
    }

    #[test]
    fn decoded_bytes_must_match_the_scanner_accounted_identity_and_length() {
        let temp = tempfile::tempdir().expect("temporary decode-accounting namespace");
        let path = temp.path().join(canonical_record_name(0));
        std::fs::write(&path, [0_u8]).expect("write accounted historical recovery record");
        let accounted = std::fs::symlink_metadata(&path).expect("accounted record metadata");
        let directory =
            std::fs::symlink_metadata(temp.path()).expect("recovery directory metadata");
        let canonical_path = std::fs::canonicalize(&path).expect("canonical recovery path");

        let length_drift = vec![0_u8, 1_u8];
        let length_mismatch = StableSidecarRead {
            bytes_hash: Hash::new(&length_drift),
            bytes: length_drift,
            metadata: StableSidecarMetadata {
                canonical_path: canonical_path.clone(),
                file: accounted.clone(),
                directory: directory.clone(),
            },
        };
        assert!(
            !historical_autonomous_recovery_read_matches_accounting(
                &accounted,
                &length_mismatch,
            ),
            "bytes with a different scanner-accounted length must never reach decoding",
        );

        let replacement = temp.path().join("replacement");
        let replacement_bytes = vec![1_u8];
        std::fs::write(&replacement, &replacement_bytes).expect("write same-length replacement");
        std::fs::remove_file(&path).expect("remove scanner-accounted record");
        std::fs::rename(&replacement, &path).expect("install same-path replacement");
        let identity_mismatch = StableSidecarRead {
            bytes_hash: Hash::new(&replacement_bytes),
            bytes: replacement_bytes,
            metadata: StableSidecarMetadata {
                canonical_path,
                file: std::fs::symlink_metadata(&path).expect("replacement record metadata"),
                directory,
            },
        };
        assert!(
            !historical_autonomous_recovery_read_matches_accounting(
                &accounted,
                &identity_mismatch,
            ),
            "same-path and same-length replacement metadata must never reach decoding",
        );
    }

    #[test]
    fn bounded_namespace_accepts_exact_record_and_aggregate_limits() {
        let temp = tempfile::tempdir().expect("temporary historical recovery namespace");
        for index in 0..HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS {
            std::fs::write(temp.path().join(canonical_record_name(index)), [0_u8])
                .expect("write bounded historical recovery record");
        }
        let (records, bytes) = scan_metadata(
            temp.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect("the exact record-count boundary is valid");
        assert_eq!(records.len(), HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS);
        assert_eq!(
            bytes,
            u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
                .expect("record count fits u64")
        );

        let aggregate = tempfile::tempdir().expect("temporary aggregate-bound namespace");
        let mut remaining = TEST_AGGREGATE_BYTE_LIMIT;
        let mut index = 0_usize;
        while remaining != 0 {
            let len = remaining.min(
                u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)
                    .expect("record byte limit fits u64"),
            );
            let file = std::fs::File::create(aggregate.path().join(canonical_record_name(index)))
                .expect("create sparse historical recovery record");
            file.set_len(len)
                .expect("size sparse historical recovery record");
            remaining -= len;
            index = index.saturating_add(1);
        }
        let (records, bytes) = scan_metadata(
            aggregate.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect("the exact aggregate-byte boundary is valid");
        assert_eq!(records.len(), index);
        assert_eq!(bytes, TEST_AGGREGATE_BYTE_LIMIT);
    }

    #[test]
    fn bounded_namespace_rejects_count_size_and_aggregate_overflow() {
        let count = tempfile::tempdir().expect("temporary count-overflow namespace");
        for index in 0..=HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS {
            std::fs::write(count.path().join(canonical_record_name(index)), [0_u8])
                .expect("write count-overflow historical recovery record");
        }
        scan_metadata(
            count.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("the 4,097th historical recovery record must fail");

        let oversized = tempfile::tempdir().expect("temporary oversized-record namespace");
        let file = std::fs::File::create(oversized.path().join(canonical_record_name(0)))
            .expect("create oversized sparse historical recovery record");
        file.set_len(
            u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)
                .expect("record byte limit fits u64")
                .saturating_add(1),
        )
        .expect("size oversized sparse historical recovery record");
        scan_metadata(
            oversized.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("one byte beyond the per-record bound must fail");

        let aggregate = tempfile::tempdir().expect("temporary aggregate-overflow namespace");
        let per_record = u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)
            .expect("record byte limit fits u64");
        let full_records = TEST_AGGREGATE_BYTE_LIMIT / per_record;
        for index in 0..usize::try_from(full_records).expect("record count fits usize") {
            let file = std::fs::File::create(aggregate.path().join(canonical_record_name(index)))
                .expect("create aggregate sparse historical recovery record");
            file.set_len(per_record)
                .expect("size aggregate sparse historical recovery record");
        }
        std::fs::write(
            aggregate.path().join(canonical_record_name(
                usize::try_from(full_records).expect("count fits"),
            )),
            [0_u8],
        )
        .expect("write one byte beyond aggregate historical recovery limit");
        scan_metadata(
            aggregate.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("one byte beyond the aggregate bound must fail");
    }

    #[test]
    fn bounded_namespace_rejects_noncanonical_and_nested_entries() {
        for name in [
            ".kura-sidecar-pending".to_owned(),
            "ABCDEF.norito".to_owned(),
            format!("{}.norito", "A".repeat(Hash::LENGTH.saturating_mul(2))),
        ] {
            let temp = tempfile::tempdir().expect("temporary malformed-name namespace");
            std::fs::write(temp.path().join(name), [0_u8])
                .expect("write malformed historical recovery entry");
            scan_metadata(
                temp.path(),
                HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
                TEST_AGGREGATE_BYTE_LIMIT,
            )
            .expect_err("a temporary, short, or uppercase filename must fail");
        }

        let nested = tempfile::tempdir().expect("temporary nested-entry namespace");
        std::fs::create_dir(nested.path().join(canonical_record_name(0)))
            .expect("create nested historical recovery entry");
        scan_metadata(
            nested.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("a nested directory must fail");
    }

    #[cfg(unix)]
    #[test]
    fn bounded_namespace_rejects_symlinks_and_hardlinks() {
        use std::os::unix::fs::symlink;

        let symlinked = tempfile::tempdir().expect("temporary symlink namespace");
        let outside = tempfile::NamedTempFile::new().expect("symlink target");
        symlink(
            outside.path(),
            symlinked.path().join(canonical_record_name(0)),
        )
        .expect("create historical recovery symlink");
        scan_metadata(
            symlinked.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("a record symlink must fail");

        let linked = tempfile::tempdir().expect("temporary hardlink namespace");
        let first = linked.path().join(canonical_record_name(0));
        let second = linked.path().join(canonical_record_name(1));
        std::fs::write(&first, [0_u8]).expect("write hardlink source");
        std::fs::hard_link(&first, &second).expect("create historical recovery hardlink");
        scan_metadata(
            linked.path(),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("a hardlinked historical recovery record must fail");

        let namespace_parent = tempfile::tempdir().expect("temporary namespace parent");
        let real_namespace = tempfile::tempdir().expect("real historical recovery namespace");
        symlink(
            real_namespace.path(),
            namespace_parent
                .path()
                .join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1),
        )
        .expect("create historical recovery namespace symlink");
        scan_metadata(
            &namespace_parent
                .path()
                .join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1),
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect_err("a namespace symlink must fail");
    }

    #[test]
    fn block_store_accounting_counts_recognized_nested_records_once() {
        let temp = tempfile::tempdir().expect("temporary block store");
        let lane_artifacts = temp.path().join(LANE_ARTIFACTS_DIR_NAME);
        std::fs::create_dir(&lane_artifacts).expect("create lane artifacts");
        let before = Kura::block_store_bytes_with_historical_limit(
            temp.path(),
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect("measure empty block store");

        let historical = lane_artifacts.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
        std::fs::create_dir(&historical).expect("create historical recovery namespace");
        let record = historical.join(canonical_record_name(0));
        let record_bytes = b"bounded historical recovery accounting";
        std::fs::write(&record, record_bytes).expect("write historical recovery record");
        let after = Kura::block_store_bytes_with_historical_limit(
            temp.path(),
            TEST_AGGREGATE_BYTE_LIMIT,
        )
        .expect("measure nested recovery record");
        assert_eq!(
            after.checked_sub(before),
            Some(u64::try_from(record_bytes.len()).expect("record length fits u64")),
            "the recognized nested record must be counted exactly once",
        );

        std::fs::create_dir(lane_artifacts.join("unexpected_nested_namespace"))
            .expect("create unexpected nested namespace");
        Kura::block_store_bytes_with_historical_limit(temp.path(), TEST_AGGREGATE_BYTE_LIMIT)
            .expect_err("an unrecognized nested lane-artifact namespace must fail closed");
    }
}
