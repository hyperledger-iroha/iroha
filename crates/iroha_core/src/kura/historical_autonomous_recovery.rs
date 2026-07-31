const HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1: &str = "historical_autonomous_recoveries_v1";
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1: u16 = 1;
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS: usize = 4_096;
const HISTORICAL_AUTONOMOUS_RECOVERY_MAX_AGGREGATE_BYTES: u64 =
    V2_PENDING_CONTROL_SIDECAR_BYTES.get() as u64;

fn accumulate_historical_autonomous_recovery_bytes(
    current: u64,
    encoded_len: usize,
    exact_duplicate: bool,
) -> Option<u64> {
    if exact_duplicate {
        return (current <= HISTORICAL_AUTONOMOUS_RECOVERY_MAX_AGGREGATE_BYTES).then_some(current);
    }
    current
        .checked_add(u64::try_from(encoded_len).ok()?)
        .filter(|total| *total <= HISTORICAL_AUTONOMOUS_RECOVERY_MAX_AGGREGATE_BYTES)
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
            let Some(snapshot) = self.read_regular_sidecar_snapshot(
                path,
                directory,
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
            )? else {
                return Ok(None);
            };
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
            for entry in entries {
                let directory = Self::historical_autonomous_recovery_directory_for_entry(
                    &entry,
                    &self.store_root,
                );
                if self.canonical_sidecar_directory(&directory)?.is_none() {
                    continue;
                }
                let mut directory_entries = std::fs::read_dir(&directory)
                    .map_err(|error| Error::IO(error, directory.clone()))?
                    .collect::<std::io::Result<Vec<_>>>()
                    .map_err(|error| Error::IO(error, directory.clone()))?;
                directory_entries.sort_by_key(std::fs::DirEntry::file_name);
                for directory_entry in directory_entries {
                    let path = directory_entry.path();
                    let name = directory_entry.file_name().into_string().map_err(|_| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery namespace contains a non-UTF-8 entry",
                        )
                    })?;
                    let canonical_name = name
                        .strip_suffix(".norito")
                        .is_some_and(|stem| {
                            stem.len() == Hash::LENGTH * 2
                                && stem.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                        });
                    if name.starts_with(".kura-sidecar-") || !canonical_name {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path,
                            "historical autonomous recovery namespace contains a temporary or unknown entry",
                        ));
                    }
                    let metadata = std::fs::symlink_metadata(&path)
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    if metadata.file_type().is_symlink()
                        || !metadata.file_type().is_file()
                        || !Self::sidecar_is_single_link(&metadata)
                    {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path,
                            "historical autonomous recovery namespace contains a linked or non-regular entry",
                        ));
                    }
                    aggregate_bytes = aggregate_bytes.checked_add(metadata.len()).ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            directory.clone(),
                            "historical autonomous recovery aggregate byte count overflowed",
                        )
                    })?;
                    if aggregate_bytes > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_AGGREGATE_BYTES {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            directory,
                            "historical autonomous recovery namespace exceeds its aggregate byte limit",
                        ));
                    }
                    if records.len() >= limit {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path,
                            "historical autonomous recovery namespace exceeds caller capacity",
                        ));
                    }
                    let record = self
                        .read_historical_autonomous_recovery_record(&path, &directory)?
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

        /// Resolve the sole immutable recovery owner for one exact FIFO group.
        /// The bounded inventory rejects any slot, proposal, transaction, or
        /// recovery-ID collision before this lookup can return a record.
        pub(crate) fn historical_autonomous_lane_recovery_record_for_group(
            &self,
            group: &LaneQueueReservationReconciliationGroupV1,
        ) -> Result<Option<HistoricalAutonomousLaneRecoveryRecordV1>> {
            let records = self.historical_autonomous_lane_recovery_records_bounded(
                HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            )?;
            Ok(records
                .into_iter()
                .find(|record| record.reservation_group == *group))
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

        pub(crate) fn preflight_historical_autonomous_lane_recovery_records(
            &self,
            incoming: &[HistoricalAutonomousLaneRecoveryRecordV1],
        ) -> Result<()> {
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
            let mut combined_encoded_bytes = 0_u64;
            for existing in &combined {
                let encoded = norito::encode_canonical(existing).map_err(Error::NoritoFrame)?;
                combined_encoded_bytes = accumulate_historical_autonomous_recovery_bytes(
                    combined_encoded_bytes,
                    encoded.len(),
                    false,
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
                if let Some(existing) = combined
                    .iter()
                    .find(|existing| existing.recovery_id == record.recovery_id)
                {
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
                    )
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery bytes exceed their aggregate bound",
                        )
                    })?;
                    self.validate_historical_autonomous_recovery_dependencies(existing, &path)?;
                } else {
                    combined_encoded_bytes = accumulate_historical_autonomous_recovery_bytes(
                        combined_encoded_bytes,
                        encoded.len(),
                        false,
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
                    combined.push(record.clone());
                }
            }
            if combined.len() > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS {
                return Err(Self::invalid_historical_autonomous_recovery(
                    self.store_root.clone(),
                    "historical autonomous recovery batch exceeds namespace capacity",
                ));
            }
            self.validate_historical_autonomous_recovery_inventory_collisions(&combined)
        }

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

        /// Persist carrier-extracted executable bytes through the ordinary
        /// autonomous lane path, then persist the execution input, and only then
        /// publish the separate immutable recovery seal.
        pub(crate) fn persist_historical_autonomous_lane_recovery_record(
            &self,
            record: &HistoricalAutonomousLaneRecoveryRecordV1,
        ) -> Result<HistoricalAutonomousLaneRecoveryPersistOutcome> {
            self.ensure_prune_recovery_not_required()?;
            self.durable_mutation_authorized()?;
            self.preflight_historical_autonomous_lane_recovery_records(std::slice::from_ref(
                record,
            ))?;
            if self.historical_autonomous_lane_recovery_record_matches(record)? {
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
    };
}

#[cfg(test)]
mod historical_autonomous_recovery_bound_tests {
    use super::*;

    #[test]
    fn aggregate_byte_bound_is_exact_and_duplicate_aware() {
        let limit = HISTORICAL_AUTONOMOUS_RECOVERY_MAX_AGGREGATE_BYTES;
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(limit - 1, 1, false),
            Some(limit),
        );
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(limit, 1, false),
            None,
            "one unique byte beyond the aggregate bound must fail",
        );
        assert_eq!(
            accumulate_historical_autonomous_recovery_bytes(
                limit,
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
                true,
            ),
            Some(limit),
            "an exact immutable duplicate must not consume aggregate bytes twice",
        );
    }
}
