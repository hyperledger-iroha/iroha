// Repair-disabled Kura projections used by operator status and diagnostics.
// One row beyond the public 128-row cap lets callers detect overflow without
// ever turning the diagnostic suffix read into an unbounded history scan.
const PASSIVE_DIAGNOSTIC_CERTIFIED_RESULT_BUDGET: usize = 129;
const PASSIVE_DIAGNOSTIC_CERTIFIED_SCAN_BUDGET: usize = 1_032;
impl MergeLedgerLog {
    fn entry_by_hash_without_append_repair(
        &mut self,
        hash: HashOf<MergeLedgerEntry>,
    ) -> Result<Option<MergeLedgerEntry>> {
        if self.append_recovery_offset.is_some() {
            return Err(Error::MergeCarrierConflict(
                "merge ledger has an unresolved append tail; passive diagnostics cannot repair it"
                    .to_owned(),
            ));
        }
        self.entry_by_hash_with_append_repair_policy(hash, false)
    }
}
impl Kura {
    fn lane_block_artifact_is_canonical_hash_only_snapshot_anchor(
        &self,
        proposal: &LaneBlockProposalV1,
        artifact: &LaneBlockArtifact,
        repair_missing_sidecar: bool,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let canonical = if repair_missing_sidecar {
            self.read_lane_block_artifact(descriptor.lane_id, descriptor.lane_block_height)
        } else {
            self.read_lane_block_artifact_without_sidecar_repair(
                descriptor.lane_id,
                descriptor.lane_block_height,
            )
        };
        Self::lane_block_artifact_matches_descriptor(&artifact.ownership, descriptor)
            && canonical.is_some_and(|canonical| canonical == *artifact)
            && self.lane_block_artifact_has_hash_only_snapshot_anchor(artifact)
    }
    fn lane_block_application_receipt_has_hash_only_snapshot_anchor(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
        repair_missing_sidecar: bool,
    ) -> bool {
        if artifact.application_block_height != artifact.artifact.ownership.proposal_height
            || artifact.application_block_hash != artifact.artifact.proposal_block_hash
        {
            return false;
        }
        artifact.format == LaneBlockApplicationReceiptArtifactFormat::Current
            && self.lane_block_artifact_is_canonical_hash_only_snapshot_anchor(
                &artifact.proposal,
                &artifact.artifact,
                repair_missing_sidecar,
            )
    }
    /// Read a recovered execution input without publishing any missing sidecar.
    pub(crate) fn read_lane_block_execution_input_without_sidecar_repair(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Option<LaneBlockExecutionInputArtifact> {
        self.read_lane_block_execution_input_with_repair_policy(lane_id, lane_block_height, false)
    }
    pub(crate) fn lane_block_execution_input_available_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        self.read_lane_block_execution_input_without_sidecar_repair(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .is_some_and(|artifact| artifact.proposal == *proposal)
    }
    /// Read a direct-execution preflight without publishing missing evidence.
    pub(crate) fn read_lane_block_execution_preflight_without_sidecar_repair(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Option<LaneBlockExecutionPreflightArtifact> {
        self.read_lane_block_execution_preflight_with_repair_policy(
            lane_id,
            lane_block_height,
            false,
        )
    }
    pub(crate) fn lane_block_execution_preflight_has_rejections_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
        current_state_height: u64,
        current_state_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<bool> {
        let artifact = self.read_lane_block_execution_preflight_without_sidecar_repair(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )?;
        (artifact.proposal == *proposal
            && artifact.preflight_state_height == current_state_height
            && artifact.preflight_state_hash == current_state_hash)
            .then(|| artifact.has_rejections())
    }
    pub(crate) fn read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
        current_state_height: u64,
        current_state_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<LaneBlockExecutionInputArtifact> {
        if !self
            .lane_block_predecessor_application_receipt_available_without_sidecar_repair(proposal)
            || self.lane_block_application_receipt_available_without_sidecar_repair(proposal)
            || self.lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair(
                proposal,
            )
        {
            return None;
        }
        let descriptor = &proposal.descriptor;
        let preflight = self.read_lane_block_execution_preflight_without_sidecar_repair(
            descriptor.lane_id,
            descriptor.lane_block_height,
        )?;
        if preflight.proposal != *proposal
            || preflight.preflight_state_height != current_state_height
            || preflight.preflight_state_hash != current_state_hash
            || preflight.has_rejections()
        {
            return None;
        }
        let input = self.read_lane_block_execution_input_without_sidecar_repair(
            descriptor.lane_id,
            descriptor.lane_block_height,
        )?;
        (input.proposal == preflight.proposal
            && input.artifact == preflight.artifact
            && input.entrypoint_hashes == preflight.entrypoint_hashes)
            .then_some(input)
    }
    /// Resolve pending or committed merge evidence without repairing a failed append tail.
    pub(crate) fn merge_entry_by_hash_without_append_repair(
        &self,
        hash: HashOf<MergeLedgerEntry>,
    ) -> Result<Option<MergeLedgerEntry>> {
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let path = self.pending_merge_entry_path(hash);
        let pending = {
            let _guard = self.sidecar_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            self.read_pending_merge_entry_path(&path, Some(hash))?
        };
        self.ensure_prune_recovery_not_required()?;
        if pending.is_some() {
            return Ok(pending);
        }
        let mut merge_log = self.merge_log.lock();
        self.ensure_prune_recovery_not_required()?;
        let entry = merge_log.entry_by_hash_without_append_repair(hash)?;
        self.ensure_prune_recovery_not_required()?;
        Ok(entry)
    }
    fn validate_merge_carrier_record_against_entry_without_append_repair(
        &self,
        record: MergeLedgerCarrierRecord,
        header: &BlockHeader,
        entry: &MergeLedgerEntry,
    ) -> Result<()> {
        let persisted = self
            .read_merge_carrier_path(&self.merge_carrier_path(record.block_height))?
            .ok_or_else(|| {
                Error::MergeCarrierConflict(format!(
                    "cached carrier record for block {} is missing",
                    record.block_height
                ))
            })?;
        let height = NonZeroUsize::new(usize::try_from(record.block_height)?)
            .ok_or_else(|| Error::MergeCarrierConflict("carrier height is zero".to_owned()))?;
        if persisted != record
            || self.get_durable_block_hash(height) != Some(record.block_hash)
            || entry.canonical_hash() != record.entry_hash
            || header.hash() != record.block_hash
            || entry.epoch_id != record.epoch_id
            || entry.merge_qc.carrier_height != header.height().get()
            || entry.merge_qc.carrier_height != record.block_height
            || Some(entry.merge_qc.carrier_parent_hash) != header.prev_block_hash()
            || entry.merge_qc.view != header.view_change_index()
        {
            return Err(Error::MergeCarrierConflict(format!(
                "carrier block {} differs from its record, canonical header, or merge entry",
                record.block_height
            )));
        }
        Ok(())
    }
    fn validate_merge_carrier_record_without_append_repair_under_prune_and_canonical_guards(
        &self,
        record: MergeLedgerCarrierRecord,
        entry: &MergeLedgerEntry,
    ) -> Result<()> {
        let height = NonZeroUsize::new(usize::try_from(record.block_height)?)
            .ok_or_else(|| Error::MergeCarrierConflict("carrier height is zero".to_owned()))?;
        let block = self.get_block_without_merge_sidecar(height);
        let finality = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(
                record.block_height,
            )?
            .map(|(header, finality, _)| (header, finality));
        let (header, finality) = match (block, finality) {
            (Some(block), Some((header, finality))) => {
                self.validate_merge_carrier_record_against_entry_without_append_repair(
                    record,
                    &block.header(),
                    entry,
                )?;
                let reference = Self::block_merge_reference(block.as_ref()).ok_or_else(|| {
                    Error::MergeCarrierConflict(format!(
                        "carrier block {} no longer contains a compact merge reference",
                        record.block_height
                    ))
                })?;
                if header != block.header()
                    || reference.entry_hash != record.entry_hash
                    || reference.epoch_id != record.epoch_id
                    || !reference.matches_entry(entry)
                {
                    return Err(Error::MergeCarrierConflict(format!(
                        "carrier block {} body differs from its record or retained header",
                        record.block_height
                    )));
                }
                (header, finality)
            }
            (None, Some((header, finality))) => {
                self.validate_merge_carrier_record_against_entry_without_append_repair(
                    record, &header, entry,
                )?;
                (header, finality)
            }
            (Some(_), None) | (None, None) => {
                return Err(Error::MergeCarrierConflict(format!(
                    "carrier block {} has no durable finality-authenticated merge identity",
                    record.block_height
                )));
            }
        };
        Self::validate_merge_carrier_finality_projection(record, entry, &header, &finality)
    }
    fn merge_carrier_for_entry_without_append_repair_under_prune_and_canonical_guards(
        &self,
        entry_hash: HashOf<MergeLedgerEntry>,
        entry: &MergeLedgerEntry,
    ) -> Result<Option<MergeLedgerCarrierRecord>> {
        self.ensure_prune_recovery_not_required()?;
        let record = {
            let _guard = self.merge_carrier_lock.lock();
            self.ensure_merge_carrier_index_initialized_unlocked()?;
            self.merge_carrier_index
                .lock()
                .by_entry
                .get(&entry_hash)
                .copied()
        };
        if let Some(record) = record {
            self
                .validate_merge_carrier_record_without_append_repair_under_prune_and_canonical_guards(
                    record, entry,
                )?;
        }
        self.ensure_prune_recovery_not_required()?;
        Ok(record)
    }
    fn lane_block_application_receipt_matches_merge_log_without_sidecar_repair(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
    ) -> bool {
        let _prune_guard = self.prune_lock.lock();
        self
            .lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_guard(
                artifact,
            )
    }
    fn lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_guard(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
    ) -> bool {
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self
            .lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_and_canonical_guards(
                artifact,
            )
    }
    fn lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_and_canonical_guards(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
    ) -> bool {
        let (Some(epoch_id), Some(entry_hash), Some(carrier_height), Some(carrier_hash)) = (
            artifact.merge_epoch_id,
            artifact.merge_entry_hash,
            artifact.merge_carrier_block_height,
            artifact.merge_carrier_block_hash,
        ) else {
            return false;
        };
        let Ok(Some(entry)) = self
            .merge_log
            .lock()
            .entry_by_hash_without_append_repair(entry_hash)
        else {
            return false;
        };
        if entry.epoch_id != epoch_id
            || self
                .merge_carrier_for_entry_without_append_repair_under_prune_and_canonical_guards(
                    entry_hash, &entry,
                )
                .ok()
                .flatten()
                != Some(MergeLedgerCarrierRecord {
                    version: 1,
                    entry_hash,
                    epoch_id,
                    block_height: carrier_height,
                    block_hash: carrier_hash,
                })
        {
            return false;
        }
        let Some(batch) = entry.execution_batch.as_ref() else {
            return false;
        };
        let Some(execution) = batch
            .lanes
            .iter()
            .find(|execution| execution.proposal == artifact.proposal)
        else {
            return false;
        };
        LaneBlockApplicationReceiptArtifact::new_merge_execution(
            &entry,
            batch,
            execution,
            Self::merge_lane_block_artifact(execution),
            carrier_height,
            carrier_hash,
        ) == *artifact
    }
    fn read_active_lane_block_artifact_from_bound_without_repair_locked(
        &self,
        entry: &LaneConfigEntry,
        lane_block_height: u64,
        bound: &mut BoundProgressSidecar,
    ) -> Option<LaneBlockArtifact> {
        let artifact = Self::read_indexed_sidecar_from_open_files(
            lane_block_height,
            &mut bound.data,
            &mut bound.index,
            &bound.namespace.data_path,
            &bound.namespace.index_path,
            norito::decode_canonical::<LaneBlockArtifact>,
            "lane block artifact",
        )?;
        let ownership = &artifact.ownership;
        if ownership.lane_id != entry.lane_id
            || ownership.lane_block_height != lane_block_height
            || ownership.validate_replay_material().is_err()
        {
            return None;
        }
        self.require_active_lane_ownership_artifact(entry, ownership)
            .ok()?;
        Some(artifact)
    }
    /// Return the newest canonical ownership without promoting recovery artifacts.
    pub(crate) fn latest_lane_block_artifact_matching_without_sidecar_repair<F>(
        &self,
        lane_id: LaneId,
        mut accept: F,
    ) -> Option<LaneBlockArtifact>
    where
        F: FnMut(&LaneBlockArtifact) -> bool,
    {
        if self.prune_recovery_is_required() {
            return None;
        }
        let geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id).ok()?;
        let (data_path, index_path) = Self::lane_artifact_paths_for_entry(&entry, &self.store_root);
        let candidates = {
            let sidecar_guard = self.sidecar_lock.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            let namespace = self
                .open_bound_progress_namespace(&data_path, &index_path)
                .ok()?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &namespace,
                &data_path,
                &index_path,
                "lane block artifact",
            )
            .ok()?;
            let mut pair = self
                .open_bound_progress_pair(&data_path, &index_path)
                .ok()?;
            let candidates = match &mut pair {
                BoundProgressPair::Absent(_) => Vec::new(),
                BoundProgressPair::Present(bound) => {
                    let heights = self
                        .bound_indexed_sidecar_height_range(bound, "lane block artifact")
                        .ok()?
                        .into_iter()
                        .flatten();
                    heights
                        .rev()
                        .take(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET)
                        .filter_map(|lane_block_height| {
                            self.read_active_lane_block_artifact_from_bound_without_repair_locked(
                                &entry,
                                lane_block_height,
                                bound,
                            )
                        })
                        .collect::<Vec<_>>()
                }
            };
            if let BoundProgressPair::Present(bound) = &pair
                && !self.bound_progress_sidecar_unchanged(bound)
            {
                return None;
            }
            drop(sidecar_guard);
            candidates
        };
        drop(geometry_guard);
        let artifact = candidates
            .into_iter()
            .filter_map(|artifact| self.validate_lane_block_artifact_canonical(artifact))
            .find(|artifact| accept(artifact))?;
        let confirmed = self.read_lane_block_artifact_without_sidecar_repair(
            lane_id,
            artifact.ownership.lane_block_height,
        )?;
        (confirmed == artifact && !self.prune_recovery_is_required()).then_some(artifact)
    }
    /// Return a bounded certified suffix without repair, sync, or cache publication.
    pub(crate) fn latest_certified_lane_block_artifacts_matching_without_sidecar_repair<F>(
        &self,
        lane_id: LaneId,
        limit: usize,
        mut accept: F,
    ) -> Vec<CertifiedLaneBlockArtifact>
    where
        F: FnMut(&CertifiedLaneBlockArtifact) -> bool,
    {
        let result_limit = limit.min(PASSIVE_DIAGNOSTIC_CERTIFIED_RESULT_BUDGET);
        if result_limit == 0 || self.prune_recovery_is_required() {
            return Vec::new();
        }
        let geometry_guard = self.lane_geometry_lock.lock();
        let Ok(entry) = self.lane_storage_entry(lane_id) else {
            return Vec::new();
        };
        let (data_path, index_path) =
            Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
        let sidecar_guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return Vec::new();
        }
        let Ok(namespace) = self.open_bound_progress_namespace(&data_path, &index_path) else {
            return Vec::new();
        };
        if self
            .ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &namespace,
                &data_path,
                &index_path,
                "certified lane block",
            )
            .is_err()
        {
            return Vec::new();
        }
        let Ok(mut pair) = self.open_bound_progress_pair(&data_path, &index_path) else {
            return Vec::new();
        };
        let scan_budget = result_limit
            .saturating_mul(8)
            .max(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET)
            .min(PASSIVE_DIAGNOSTIC_CERTIFIED_SCAN_BUDGET);
        let candidates = match &mut pair {
            BoundProgressPair::Absent(_) => Vec::new(),
            BoundProgressPair::Present(bound) => {
                let Ok(heights) =
                    self.bound_indexed_sidecar_height_range(bound, "certified lane block")
                else {
                    return Vec::new();
                };
                heights
                    .into_iter()
                    .flatten()
                    .rev()
                    .take(scan_budget)
                    .filter_map(|lane_block_height| {
                        self.read_active_certified_lane_block_artifact_from_bound_locked(
                            &entry,
                            lane_block_height,
                            bound,
                        )
                    })
                    .collect::<Vec<_>>()
            }
        };
        if let BoundProgressPair::Present(bound) = &pair
            && !self.bound_progress_sidecar_unchanged(bound)
        {
            return Vec::new();
        }
        drop(sidecar_guard);
        drop(geometry_guard);
        let mut artifacts = candidates
            .into_iter()
            .filter(|artifact| accept(artifact))
            .take(result_limit)
            .collect::<Vec<_>>();
        artifacts.reverse();
        artifacts
    }
}
impl Kura {
    fn validate_lane_new_view_certificate_for_artifact(
        artifact: &AutonomousLaneBlockArtifact,
        durable_certificate: &DurableLaneBlockNewViewCertificateV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        slot_path: &Path,
    ) -> Result<(LaneBlockProposalV1, LaneBlockProposalV1)> {
        let current = Self::validate_autonomous_lane_block_artifact(
            artifact,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(slot_path.to_path_buf(), message))?;
        let target = crate::lane_consensus::retarget_lane_block_proposal_view(
            &current,
            durable_certificate.certificate.body.target_view,
        )
        .map_err(|err| {
            Self::invalid_lane_artifact_error(
                slot_path.to_path_buf(),
                format!("invalid autonomous lane NewView target: {err}"),
            )
        })?;
        crate::lane_consensus::validate_lane_block_new_view_transition(
            &current,
            &target,
            &artifact.executable_payload,
            durable_certificate,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|err| {
            Self::invalid_lane_artifact_error(
                slot_path.to_path_buf(),
                format!("invalid autonomous lane NewView certificate: {err}"),
            )
        })?;
        Ok((current, target))
    }
    fn read_autonomous_lane_block_record_read_only_latest_locked(
        &self,
        entry: &LaneConfigEntry,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockDurableRecord>> {
        if let Some(pointer) =
            self.read_autonomous_lane_block_latest_attempt_locked(entry, lane_block_height)?
        {
            if pointer.lane_id != lane_id {
                return Err(Self::invalid_lane_artifact_error(
                    Self::autonomous_lane_block_latest_attempt_path_for_entry(
                        entry,
                        &self.store_root,
                        lane_block_height,
                    ),
                    "autonomous lane latest attempt belongs to a different lane",
                ));
            }
            return self
                .read_autonomous_lane_block_attempt_artifact_with_view_state_mode_locked(
                    entry,
                    &pointer,
                    expected_network_id,
                    expected_epoch,
                    AutonomousLaneBlockViewStateReadMode::LatestReadOnly,
                )
                .map(Some);
        }
        Ok(None)
    }
    /// Read one exact durability-attested receipt while the caller holds
    /// `prune_lock`. This path never repairs progress-sidecar artifacts.
    fn read_exact_lane_block_application_receipt_under_prune_guard(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<LaneBlockApplicationReceiptArtifact> {
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.read_exact_lane_block_application_receipt_under_prune_and_canonical_guards(proposal)
    }
    /// Read one exact durability-attested receipt while the caller holds
    /// `prune_lock` and `canonical_chain_lock`, in that order. This path never
    /// repairs progress-sidecar artifacts or reacquires either outer lock.
    fn read_exact_lane_block_application_receipt_under_prune_and_canonical_guards(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<LaneBlockApplicationReceiptArtifact> {
        let descriptor = &proposal.descriptor;
        let artifact = self.read_active_lane_block_application_receipt_structural(
            descriptor.lane_id,
            descriptor.lane_block_height,
            false,
        )?;
        if artifact.proposal != *proposal
            || !self
                .lane_block_application_receipt_matches_available_evidence_under_prune_and_canonical_guards(
                    &artifact,
                    false,
                )
        {
            return None;
        }
        let confirmed = self.read_active_lane_block_application_receipt_structural(
            descriptor.lane_id,
            descriptor.lane_block_height,
            false,
        )?;
        (confirmed == artifact && !self.prune_recovery_is_required()).then_some(artifact)
    }
    fn lane_block_application_receipt_available_under_prune_guard(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        self.read_exact_lane_block_application_receipt_under_prune_guard(proposal)
            .is_some()
    }
    fn lane_block_application_receipt_available_under_prune_and_canonical_guards(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        self.read_exact_lane_block_application_receipt_under_prune_and_canonical_guards(proposal)
            .is_some()
    }
    fn lane_block_application_receipt_matches_available_evidence(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
        repair_missing_sidecars: bool,
    ) -> bool {
        match artifact.format {
            LaneBlockApplicationReceiptArtifactFormat::Current => self
                .lane_block_application_receipt_matches_canonical_results(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::DirectExecution => self
                .lane_block_application_receipt_matches_direct_preflight(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::MergeExecution => {
                if repair_missing_sidecars {
                    self.lane_block_application_receipt_matches_merge_log(artifact)
                } else {
                    self.lane_block_application_receipt_matches_merge_log_without_sidecar_repair(
                        artifact,
                    )
                }
            }
        }
    }
    fn lane_block_application_receipt_matches_available_evidence_under_prune_guard(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
        repair_missing_sidecars: bool,
    ) -> bool {
        match artifact.format {
            LaneBlockApplicationReceiptArtifactFormat::Current => self
                .lane_block_application_receipt_matches_canonical_results(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::DirectExecution => self
                .lane_block_application_receipt_matches_direct_preflight(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::MergeExecution => {
                if repair_missing_sidecars {
                    self.lane_block_application_receipt_matches_merge_log_under_prune_guard(
                        artifact,
                    )
                } else {
                    self.lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_guard(
                        artifact,
                    )
                }
            }
        }
    }
    fn lane_block_application_receipt_matches_available_evidence_under_prune_and_canonical_guards(
        &self,
        artifact: &LaneBlockApplicationReceiptArtifact,
        repair_missing_sidecars: bool,
    ) -> bool {
        match artifact.format {
            LaneBlockApplicationReceiptArtifactFormat::Current => self
                .lane_block_application_receipt_matches_canonical_results(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::DirectExecution => self
                .lane_block_application_receipt_matches_direct_preflight(
                    artifact,
                    repair_missing_sidecars,
                ),
            LaneBlockApplicationReceiptArtifactFormat::MergeExecution => self
                .lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(
                    artifact,
                ),
        }
    }
}
