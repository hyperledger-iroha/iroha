macro_rules! kura_autonomous_reservation_classifier_methods {
    () => {
    fn autonomous_reservation_key_matches_group(
        key: &LaneQueueReservationKeyV2,
        identity: &LaneQueueReservationGroupIdentityV1,
    ) -> bool {
        key.lane_id == identity.lane_id
            && key.dataspace_id == identity.dataspace_id
            && key.lane_incarnation == identity.lane_incarnation
            && key.proposal_height == identity.proposal_height
            && key.lane_block_height == identity.lane_block_height
            && key.lane_block_view == identity.lane_block_view
            && key.reservation_owner_hash == identity.reservation_owner_hash
            && key.proposal_identity_hash == identity.proposal_identity_hash
    }
    fn validate_autonomous_reservation_reconciliation_group(
        group: &LaneQueueReservationReconciliationGroupV1,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        if group.ordered_keys.is_empty()
            || group.ordered_keys.len() > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
        {
            return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                "reservation membership is empty or exceeds the executable-entrypoint bound",
            ));
        }
        let mut digests = BTreeSet::new();
        for key in &group.ordered_keys {
            key.validate()
                .map_err(AutonomousLaneReservationEvidenceError::InvalidGroup)?;
            if !Self::autonomous_reservation_key_matches_group(key, &group.identity) {
                return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                    "reservation membership does not match the proposal-slot identity",
                ));
            }
            if !digests.insert(key.digest()) {
                return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                    "reservation membership contains a duplicate identity",
                ));
            }
        }
        Ok(())
    }
    fn charge_autonomous_reservation_attempt_read(
        inventory: &AutonomousReservationLaneInventory,
        lane_block_height: u64,
        proposal_height: u64,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        let attempt_bytes = inventory
            .attempts
            .get(&(lane_block_height, proposal_height))
            .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
        let view_bytes = inventory
            .view_states
            .get(&(lane_block_height, proposal_height))
            .copied()
            .unwrap_or(0);
        *decoded_bytes = decoded_bytes
            .checked_add(*attempt_bytes)
            .and_then(|bytes| bytes.checked_add(view_bytes))
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        Ok(())
    }
    fn charge_autonomous_reservation_attempt_payload_read(
        inventory: &AutonomousReservationLaneInventory,
        lane_block_height: u64,
        proposal_height: u64,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        let attempt_bytes = inventory
            .attempts
            .get(&(lane_block_height, proposal_height))
            .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
        *decoded_bytes = decoded_bytes
            .checked_add(*attempt_bytes)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn load_autonomous_reservation_attempt_locked(
        &self,
        entry: &LaneConfigEntry,
        inventory: &AutonomousReservationLaneInventory,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        decoded_bytes: &mut u64,
        attempts: &mut BTreeMap<(LaneId, u64, u64), AutonomousReservationAttemptRead>,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        let coordinate = (lane_id, lane_block_height, proposal_height);
        if let Some(existing) = attempts.get(&coordinate) {
            if existing.payload.network_id != expected_network_id
                || existing.payload.epoch != expected_epoch
            {
                return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
            }
            return Ok(());
        }
        if !inventory
            .attempts
            .contains_key(&(lane_block_height, proposal_height))
        {
            return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
        }
        // The exact helper authenticates the namespace payload first and then
        // rereads it through the shared artifact validator. Charge both reads.
        Self::charge_autonomous_reservation_attempt_payload_read(
            inventory,
            lane_block_height,
            proposal_height,
            decoded_bytes,
        )?;
        Self::charge_autonomous_reservation_attempt_read(
            inventory,
            lane_block_height,
            proposal_height,
            decoded_bytes,
        )?;
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                entry,
                lane_id,
                lane_block_height,
                proposal_height,
                expected_network_id,
                expected_epoch,
                None,
            )?
            .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
        attempts.insert(
            coordinate,
            AutonomousReservationAttemptRead {
                payload: record.artifact.executable_payload,
                retirement: record.retirement,
            },
        );
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn load_autonomous_reservation_attempt_self_context_locked(
        &self,
        entry: &LaneConfigEntry,
        inventory: &AutonomousReservationLaneInventory,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        decoded_bytes: &mut u64,
        attempts: &mut BTreeMap<(LaneId, u64, u64), AutonomousReservationAttemptRead>,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        let coordinate = (lane_id, lane_block_height, proposal_height);
        if attempts.contains_key(&coordinate) {
            return Ok(());
        }
        Self::charge_autonomous_reservation_attempt_payload_read(
            inventory,
            lane_block_height,
            proposal_height,
            decoded_bytes,
        )?;
        let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
            entry,
            &self.store_root,
            lane_block_height,
            proposal_height,
        );
        let parent = attempt_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                attempt_path.clone(),
                "autonomous reservation attempt path has no parent directory",
            )
        })?;
        let bytes = self
            .read_regular_sidecar_bytes(
                &attempt_path,
                parent,
                MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
            )?
            .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
        let artifact = norito::decode_canonical::<AutonomousLaneBlockArtifact>(&bytes).map_err(
            |error| match error {
                norito::Error::NonCanonicalEncoding => Self::invalid_lane_artifact_error(
                    attempt_path.clone(),
                    "autonomous reservation attempt payload is not canonical Norito",
                ),
                other => Error::NoritoFrame(other),
            },
        )?;
        let pointer =
            AutonomousLaneBlockLatestAttemptV1::from_payload(&artifact.executable_payload);
        if pointer.network_id != expected_network_id
            || pointer.lane_id != lane_id
            || pointer.lane_block_height != lane_block_height
            || pointer.proposal_height != proposal_height
        {
            return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
        }
        Self::charge_autonomous_reservation_attempt_read(
            inventory,
            lane_block_height,
            proposal_height,
            decoded_bytes,
        )?;
        let record = self.read_autonomous_lane_block_attempt_artifact_locked(
            entry,
            &pointer,
            expected_network_id,
            pointer.epoch,
            None,
        )?;
        attempts.insert(
            coordinate,
            AutonomousReservationAttemptRead {
                payload: record.artifact.executable_payload,
                retirement: record.retirement,
            },
        );
        Ok(())
    }
    fn read_autonomous_reservation_claim_locked(
        &self,
        path: &Path,
        scanned_entries: &mut usize,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<
        Option<AutonomousLaneEntrypointClaimV3>,
        AutonomousLaneReservationEvidenceError,
    > {
        if !Self::autonomous_lane_entrypoint_claim_file_exists(path)? {
            return Ok(None);
        }
        *scanned_entries = scanned_entries
            .checked_add(1)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *scanned_entries > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
            || *scanned_entries > MAX_AUTONOMOUS_LANE_CLAIM_FILES
        {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        *decoded_bytes = decoded_bytes
            .checked_add(metadata.len())
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let claim = Self::decode_autonomous_lane_entrypoint_claim(path)
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        Ok(Some(claim))
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn preflight_autonomous_reservation_claims_locked(
        &self,
        entry: &LaneConfigEntry,
        inventory: &AutonomousReservationLaneInventory,
        payload: &LaneExecutablePayloadV1,
        retirement: Option<&AutonomousLaneSlotRetirementV1>,
        current_lane_height_attempt: bool,
        attempts: &mut BTreeMap<(LaneId, u64, u64), AutonomousReservationAttemptRead>,
        claim_probes: &mut usize,
        scanned_entries: &mut usize,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<(), AutonomousLaneReservationEvidenceError> {
        let conflict = |path: &Path| {
            AutonomousLaneReservationEvidenceError::EntrypointClaimConflict {
                path: path.to_path_buf(),
            }
        };
        if retirement.is_none() && !current_lane_height_attempt {
            return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
        }
        let probes = payload
            .entrypoint_hashes
            .len()
            .checked_mul(2)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        *claim_probes = claim_probes
            .checked_add(probes)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *claim_probes > MAX_AUTONOMOUS_LANE_CLAIM_FILES {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let retirement_hash = retirement.map(AutonomousLaneSlotRetirementV1::digest).transpose()?;
        let descriptor = &payload.origin_proposal.descriptor;
        for entrypoint_hash in &payload.entrypoint_hashes {
            let path = Self::autonomous_lane_entrypoint_claim_path(
                &self.store_root,
                &payload.network_id,
                entrypoint_hash,
            );
            let temp_path = Self::autonomous_lane_entrypoint_claim_temp_path(&path);
            let existing = self.read_autonomous_reservation_claim_locked(
                &path,
                scanned_entries,
                decoded_bytes,
            )?;
            let staged = self.read_autonomous_reservation_claim_locked(
                &temp_path,
                scanned_entries,
                decoded_bytes,
            )?;
            let expected_active =
                AutonomousLaneEntrypointClaimV3::new(payload, *entrypoint_hash);
            if existing.as_ref().is_some_and(|claim| {
                !self.autonomous_lane_entrypoint_claim_path_matches(claim, &path)
            }) {
                return Err(conflict(&path));
            }
            if staged.as_ref().is_some_and(|claim| {
                claim != &expected_active
                    || !self.autonomous_lane_entrypoint_claim_path_matches(claim, &path)
            }) {
                return Err(conflict(&temp_path));
            }
            let Some(_retirement) = retirement else {
                if existing.as_ref().is_some_and(|claim| claim != &expected_active)
                    || existing.is_none() && staged.is_none()
                {
                    return Err(conflict(&path));
                }
                continue;
            };
            let retirement_hash = retirement_hash.expect("retirement digest exists");
            let pending = AutonomousLaneEntrypointClaimV3::release_pending_for_payload(
                payload,
                *entrypoint_hash,
                retirement_hash,
            );
            let released = AutonomousLaneEntrypointClaimV3::released_for_payload(
                payload,
                *entrypoint_hash,
                retirement_hash,
            );
            if current_lane_height_attempt {
                if existing.as_ref().is_some_and(|claim| {
                    claim != &expected_active && claim != &pending && claim != &released
                }) || existing.is_none() && staged.is_none()
                {
                    return Err(conflict(&path));
                }
                continue;
            }
            // Historical retirement replay never mutates claims. Its exact
            // owner must already be Released, or the hash-addressed claim must
            // be authenticatedly superseded by a later attempt. Any temp is
            // ambiguous and is rejected exactly like the writer-side proof.
            if staged.is_some() {
                return Err(conflict(&temp_path));
            }
            let Some(existing) = existing else {
                return Err(conflict(&path));
            };
            if existing == released {
                continue;
            }
            if existing.network_id != payload.network_id
                || existing.epoch < payload.epoch
                || existing.entrypoint_hash != *entrypoint_hash
                || existing.lane_id != descriptor.lane_id
                || existing.dataspace_id != descriptor.dataspace_id
                || existing.lane_incarnation != descriptor.lane_incarnation
                || existing.lane_block_height != descriptor.lane_block_height
                || existing.proposal_height <= descriptor.proposal_height
            {
                return Err(conflict(&path));
            }
            self.load_autonomous_reservation_attempt_locked(
                entry,
                inventory,
                existing.lane_id,
                existing.lane_block_height,
                existing.proposal_height,
                existing.network_id,
                existing.epoch,
                decoded_bytes,
                attempts,
            )?;
            let newer = attempts
                .get(&(
                    existing.lane_id,
                    existing.lane_block_height,
                    existing.proposal_height,
                ))
                .expect("superseding attempt was loaded");
            if !existing.owns_payload(&newer.payload) {
                return Err(conflict(&path));
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn read_requested_certified_lane_blocks_strict_locked(
        &self,
        entry: &LaneConfigEntry,
        requested_heights: &BTreeSet<u64>,
        bound: &mut BoundProgressSidecar,
        scanned_entries: &mut usize,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<
        BTreeMap<u64, CertifiedLaneBlockArtifact>,
        AutonomousLaneReservationEvidenceError,
    > {
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err(Self::invalid_lane_artifact_error(
                bound.namespace.index_path.clone(),
                "certified reservation evidence changed before strict index inspection",
            )
            .into());
        }
        let index_len = bound
            .index
            .metadata()
            .map_err(|error| Error::IO(error, bound.namespace.index_path.clone()))?
            .len();
        *decoded_bytes = decoded_bytes
            .checked_add(index_len)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let layout =
            SidecarIndexLayout::read_from(&mut bound.index, index_len).map_err(|reason| {
                Self::invalid_lane_artifact_error(
                    bound.namespace.index_path.clone(),
                    format!("certified reservation evidence index is malformed: {reason}"),
                )
            })?;
        // Legacy layout detection reads its first entry before the complete
        // entry walk below rereads it. Charge that actual additional read to
        // the one batch-wide budget.
        if !layout.is_based() && index_len >= PIPELINE_INDEX_ENTRY_SIZE_U64 {
            *decoded_bytes = decoded_bytes
                .checked_add(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
        }
        if layout.aligned_len != index_len
            || usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
                > MAX_AUTONOMOUS_RESERVATION_CERTIFIED_INDEX_ENTRIES
        {
            return Err(Self::invalid_lane_artifact_error(
                bound.namespace.index_path.clone(),
                "certified reservation evidence index is misaligned or exceeds its hard entry bound",
            )
            .into());
        }
        let entry_count = usize::try_from(layout.entry_count)
            .map_err(|_| AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        *scanned_entries = scanned_entries
            .checked_add(entry_count)
            .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
        if *scanned_entries > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let data_len = bound
            .data
            .metadata()
            .map_err(|error| Error::IO(error, bound.namespace.data_path.clone()))?
            .len();
        bound
            .index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|error| Error::IO(error, bound.namespace.index_path.clone()))?;
        let mut requested_ranges = BTreeMap::<u64, (u64, u64)>::new();
        let mut encoded = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        let mut prior_payload_end = None;
        let mut indexed_end = 0_u64;
        for offset in 0..layout.entry_count {
            bound
                .index
                .read_exact(&mut encoded)
                .map_err(|error| Error::IO(error, bound.namespace.index_path.clone()))?;
            let sidecar_entry = SidecarIndexEntry::from_bytes(encoded);
            if sidecar_entry.len == 0 {
                if sidecar_entry.offset != 0 {
                    return Err(Self::invalid_lane_artifact_error(
                        bound.namespace.index_path.clone(),
                        "certified reservation evidence has a non-canonical empty index entry",
                    )
                    .into());
                }
                continue;
            }
            let Some(end) = sidecar_entry.offset.checked_add(sidecar_entry.len) else {
                return Err(Self::invalid_lane_artifact_error(
                    bound.namespace.index_path.clone(),
                    "certified reservation evidence payload range overflows",
                )
                .into());
            };
            if sidecar_entry.len > STRICT_INIT_MAX_BLOCK_BYTES
                || end > data_len
                || prior_payload_end.is_some_and(|prior_end| sidecar_entry.offset < prior_end)
            {
                return Err(Self::invalid_lane_artifact_error(
                    bound.namespace.index_path.clone(),
                    "certified reservation evidence contains an oversized, overlapping, or out-of-range payload",
                )
                .into());
            }
            prior_payload_end = Some(end);
            indexed_end = indexed_end.max(end);
            let height = layout.base_height.checked_add(offset).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    bound.namespace.index_path.clone(),
                    "certified reservation evidence height overflows",
                )
            })?;
            if requested_heights.contains(&height) {
                requested_ranges.insert(height, (sidecar_entry.offset, sidecar_entry.len));
            }
        }
        if data_len != indexed_end {
            return Err(Self::invalid_lane_artifact_error(
                bound.namespace.data_path.clone(),
                "certified reservation evidence has an unindexed suffix requiring recovery",
            )
            .into());
        }
        let mut requested = BTreeMap::new();
        for (height, (offset, len)) in requested_ranges {
            *decoded_bytes = decoded_bytes
                .checked_add(len)
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
            let len = usize::try_from(len)
                .map_err(|_| AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            let mut bytes = vec![0_u8; len];
            bound
                .data
                .seek(SeekFrom::Start(offset))
                .and_then(|_| bound.data.read_exact(&mut bytes))
                .map_err(|error| Error::IO(error, bound.namespace.data_path.clone()))?;
            let artifact = norito::decode_canonical::<CertifiedLaneBlockArtifact>(&bytes).map_err(
                |error| {
                    Self::invalid_lane_artifact_error(
                        bound.namespace.data_path.clone(),
                        format!(
                            "certified reservation evidence failed exact Norito decode: {error}"
                        ),
                    )
                },
            )?;
            Self::validate_certified_lane_block_artifact(&artifact).map_err(|message| {
                Self::invalid_lane_artifact_error(
                    bound.namespace.data_path.clone(),
                    format!("certified reservation evidence is invalid: {message}"),
                )
            })?;
            let descriptor = &artifact.proposal.descriptor;
            if descriptor.lane_id != entry.lane_id || descriptor.lane_block_height != height {
                return Err(Self::invalid_lane_artifact_error(
                    bound.namespace.data_path.clone(),
                    "certified reservation evidence does not match its exact indexed coordinates",
                )
                .into());
            }
            self.require_active_lane_artifact(entry, descriptor)?;
            requested.insert(height, artifact);
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err(Self::invalid_lane_artifact_error(
                bound.namespace.index_path.clone(),
                "certified reservation evidence changed during strict index inspection",
            )
            .into());
        }
        Ok(requested)
    }
    fn autonomous_reservation_certified_lane_snapshot_locked(
        &self,
        entry: &LaneConfigEntry,
        inventory: &AutonomousReservationLaneInventory,
        requested_heights: &BTreeSet<u64>,
        scanned_entries: &mut usize,
        decoded_bytes: &mut u64,
    ) -> std::result::Result<
        AutonomousReservationCertifiedLaneSnapshot,
        AutonomousLaneReservationEvidenceError,
    > {
        let (data_path, index_path) =
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
        if !inventory.directory_present {
            let directory = data_path.parent().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "certified reservation data path has no artifact directory",
                )
            })?;
            let before = self.stable_sidecar_directory_metadata(directory)?;
            let after = self.stable_sidecar_directory_metadata(directory)?;
            if before.metadata.is_none()
                && Self::stable_sidecar_directory_metadata_unchanged(&before, &after)
            {
                return Ok(AutonomousReservationCertifiedLaneSnapshot::default());
            }
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        let temporary_paths = [
            data_path.with_extension("norito.tmp"),
            index_path.with_extension("index.tmp"),
            index_path.with_extension("index.prepend.tmp"),
            Self::bound_progress_append_build_path(&index_path),
            Self::bound_progress_append_intent_path(&index_path),
        ];
        for path in temporary_paths {
            if self
                .open_optional_bound_progress_file(&namespace, &path)?
                .is_some()
            {
                return Err(AutonomousLaneReservationEvidenceError::UnresolvedTemporary { path });
            }
        }
        let frontier_read = self.read_latest_certified_lane_block_frontier_locked(entry, false)?;
        if let Some(read) = &frontier_read {
            let two_reads = u64::try_from(read.snapshot.bytes.len())
                .ok()
                .and_then(|bytes| bytes.checked_mul(2))
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            *decoded_bytes = decoded_bytes
                .checked_add(two_reads)
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if *decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
        }
        let frontier = frontier_read
            .as_ref()
            .map(|read| read.frontier.artifact.clone());
        if let Some(frontier) = &frontier {
            Self::validate_certified_lane_block_artifact(frontier).map_err(|message| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    format!("latest certified reservation frontier is invalid: {message}"),
                )
            })?;
        }
        let mut pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let (requested, pair_nonempty) = match &mut pair {
            BoundProgressPair::Absent(_) => (BTreeMap::new(), false),
            BoundProgressPair::Present(bound) => {
                let pair_nonempty = bound
                    .data
                    .metadata()
                    .map_err(|error| Error::IO(error, data_path.clone()))?
                    .len()
                    != 0
                    || bound
                        .index
                        .metadata()
                        .map_err(|error| Error::IO(error, index_path.clone()))?
                        .len()
                        != 0;
                let requested = self.read_requested_certified_lane_blocks_strict_locked(
                    entry,
                    requested_heights,
                    bound,
                    scanned_entries,
                    decoded_bytes,
                )?;
                (requested, pair_nonempty)
            }
        };
        if pair_nonempty && frontier.is_none() {
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        match frontier_read {
            Some(read) => self
                .confirm_latest_certified_lane_block_frontier_read_locked(entry, &read.snapshot)?,
            None => {
                if self
                    .read_latest_certified_lane_block_frontier_locked(entry, false)?
                    .is_some()
                {
                    return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
                }
            }
        }
        Ok(AutonomousReservationCertifiedLaneSnapshot {
            requested,
            frontier,
        })
    }
    fn autonomous_reservation_certification_for_payload(
        snapshot: &AutonomousReservationCertifiedLaneSnapshot,
        payload: &LaneExecutablePayloadV1,
    ) -> std::result::Result<
        AutonomousLaneReservationCertificationV1,
        AutonomousLaneReservationEvidenceError,
    > {
        let proposal = &payload.origin_proposal;
        let height = proposal.descriptor.lane_block_height;
        let indexed = snapshot.requested.get(&height);
        if indexed.is_some_and(|artifact| artifact.proposal != *proposal) {
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        let frontier_at_height = snapshot
            .frontier
            .as_ref()
            .filter(|artifact| artifact.proposal.descriptor.lane_block_height == height);
        if frontier_at_height.is_some_and(|artifact| artifact.proposal != *proposal) {
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        if let (Some(indexed), Some(frontier)) = (indexed, frontier_at_height)
            && indexed != frontier
        {
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        if indexed.is_some()
            && snapshot
                .frontier
                .as_ref()
                .is_none_or(|frontier| frontier.proposal.descriptor.lane_block_height < height)
        {
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        if indexed.is_none()
            && snapshot
                .frontier
                .as_ref()
                .is_some_and(|frontier| frontier.proposal.descriptor.lane_block_height > height)
        {
            // A newer frontier cannot prove that this older exact attempt was
            // never certified before its indexed entry was pruned. Releasing
            // on that absence would turn evidence retention into a safety
            // decision, so classification remains fail-closed.
            return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
        }
        Ok(indexed.or(frontier_at_height).cloned().map_or(
            AutonomousLaneReservationCertificationV1::Uncertified,
            AutonomousLaneReservationCertificationV1::Exact,
        ))
    }
    /// Classify a complete immutable Queue reconciliation snapshot under one
    /// Kura evidence window.
    ///
    /// All input groups are validated first, then every touched lane namespace
    /// and certified index is inventoried once while Kura holds
    /// `prune_lock -> lane_geometry_lock -> sidecar_lock`. Results preserve
    /// input order. The aligned epoch slice is supplied by State before Kura
    /// locks are acquired, avoiding callbacks or cross-component locks inside
    /// the evidence snapshot.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn classify_autonomous_lane_reservation_groups(
        &self,
        groups: &[LaneQueueReservationReconciliationGroupV1],
        expected_network_id: iroha_data_model::NetworkId,
        expected_epochs: &[u64],
    ) -> std::result::Result<
        Vec<AutonomousLaneReservationEvidenceV1>,
        AutonomousLaneReservationEvidenceError,
    > {
        if groups.len() != expected_epochs.len() {
            return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                "group and epoch vectors are not aligned",
            ));
        }
        if groups.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
        }
        let mut unique_groups = BTreeSet::new();
        let mut coordinate_epochs = BTreeMap::new();
        let mut reservation_members = 0_usize;
        for (group, expected_epoch) in groups.iter().zip(expected_epochs) {
            Self::validate_autonomous_reservation_reconciliation_group(group)?;
            reservation_members = reservation_members
                .checked_add(group.ordered_keys.len())
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if reservation_members > MAX_AUTONOMOUS_LANE_CLAIM_FILES {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
            if !unique_groups.insert(group.identity) {
                return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                    "duplicate proposal-slot group",
                ));
            }
            let coordinate = (
                group.identity.lane_id,
                group.identity.lane_block_height,
                group.identity.proposal_height,
            );
            if coordinate_epochs
                .insert(coordinate, *expected_epoch)
                .is_some()
            {
                return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                    "one exact proposal-height attempt was split across multiple Queue groups",
                ));
            }
        }
        if groups.is_empty() {
            return Ok(Vec::new());
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let mut entries = BTreeMap::<LaneId, LaneConfigEntry>::new();
        let mut requested_certified_heights = BTreeMap::<LaneId, BTreeSet<u64>>::new();
        for group in groups {
            let entry = match entries.get(&group.identity.lane_id) {
                Some(entry) => entry.clone(),
                None => {
                    let entry = self.lane_storage_entry(group.identity.lane_id)?;
                    entries.insert(group.identity.lane_id, entry.clone());
                    entry
                }
            };
            if entry.dataspace_id != group.identity.dataspace_id {
                return Err(AutonomousLaneReservationEvidenceError::InvalidGroup(
                    "group dataspace differs from active lane storage",
                ));
            }
            self.require_active_lane_incarnation(
                &entry,
                group.identity.lane_incarnation,
                group.identity.proposal_height,
            )?;
            requested_certified_heights
                .entry(group.identity.lane_id)
                .or_default()
                .insert(group.identity.lane_block_height);
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return Err(Error::PruneRecoveryRequired.into());
        }
        let mut scanned_entries = 0_usize;
        let mut decoded_bytes = 0_u64;
        let mut inventories = BTreeMap::<LaneId, AutonomousReservationLaneInventory>::new();
        let mut attempts =
            BTreeMap::<(LaneId, u64, u64), AutonomousReservationAttemptRead>::new();
        let mut latest_attempts =
            BTreeMap::<(LaneId, u64), AutonomousLaneBlockLatestAttemptV1>::new();
        for (lane_id, entry) in &entries {
            let inventory =
                self.autonomous_reservation_lane_inventory_locked(entry, &mut scanned_entries)?;
            for (height, pointer_bytes) in &inventory.lane_latest {
                decoded_bytes = decoded_bytes
                    .checked_add(*pointer_bytes)
                    .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
                if decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                    return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
                }
                let pointer = self
                    .read_autonomous_lane_block_latest_attempt_locked(entry, *height)?
                    .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
                if pointer.network_id != expected_network_id
                    || !inventory
                        .attempts
                        .contains_key(&(pointer.lane_block_height, pointer.proposal_height))
                {
                    return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                }
                let coordinate = (
                    *lane_id,
                    pointer.lane_block_height,
                    pointer.proposal_height,
                );
                if let Some(cached) = attempts.get(&coordinate) {
                    if !pointer.matches_payload(&cached.payload) {
                        return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                    }
                } else {
                    Self::charge_autonomous_reservation_attempt_read(
                        &inventory,
                        pointer.lane_block_height,
                        pointer.proposal_height,
                        &mut decoded_bytes,
                    )?;
                    // Pointers never select a Queue group's attempt, but every
                    // present pointer is still authenticated against its exact
                    // immutable target so a decodable hash/epoch corruption
                    // cannot be mistaken for harmless metadata.
                    let record = self.read_autonomous_lane_block_attempt_artifact_locked(
                        entry,
                        &pointer,
                        expected_network_id,
                        pointer.epoch,
                        None,
                    )?;
                    attempts.insert(
                        coordinate,
                        AutonomousReservationAttemptRead {
                            payload: record.artifact.executable_payload,
                            retirement: record.retirement,
                        },
                    );
                }
                latest_attempts.insert((*lane_id, *height), pointer);
            }
            if let Some(pointer_bytes) = inventory.route_latest_bytes {
                decoded_bytes = decoded_bytes
                    .checked_add(pointer_bytes)
                    .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
                if decoded_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                    return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
                }
                let pointer = self
                    .read_autonomous_lane_route_latest_attempt_locked(entry)?
                    .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
                if pointer.network_id != expected_network_id
                    || !inventory
                        .attempts
                        .contains_key(&(pointer.lane_block_height, pointer.proposal_height))
                {
                    return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                }
                let coordinate = (
                    *lane_id,
                    pointer.lane_block_height,
                    pointer.proposal_height,
                );
                if let Some(cached) = attempts.get(&coordinate) {
                    if !pointer.matches_payload(&cached.payload) {
                        return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                    }
                } else {
                    Self::charge_autonomous_reservation_attempt_read(
                        &inventory,
                        pointer.lane_block_height,
                        pointer.proposal_height,
                        &mut decoded_bytes,
                    )?;
                    let record = self.read_autonomous_lane_block_attempt_artifact_locked(
                        entry,
                        &pointer,
                        expected_network_id,
                        pointer.epoch,
                        None,
                    )?;
                    attempts.insert(
                        coordinate,
                        AutonomousReservationAttemptRead {
                            payload: record.artifact.executable_payload,
                            retirement: record.retirement,
                        },
                    );
                }
            }
            if inventory
                .view_states
                .keys()
                .any(|coordinates| !inventory.attempts.contains_key(coordinates))
            {
                return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
            }
            inventories.insert(*lane_id, inventory);
        }
        let mut certified_snapshots = BTreeMap::new();
        for (lane_id, requested_heights) in &requested_certified_heights {
            let entry = entries
                .get(lane_id)
                .expect("requested certification lane has a storage entry");
            let inventory = inventories
                .get(lane_id)
                .expect("requested certification lane has an inventory");
            let snapshot = self.autonomous_reservation_certified_lane_snapshot_locked(
                entry,
                inventory,
                requested_heights,
                &mut scanned_entries,
                &mut decoded_bytes,
            )?;
            certified_snapshots.insert(*lane_id, snapshot);
        }
        for (coordinate, expected_epoch) in &coordinate_epochs {
            let (lane_id, lane_block_height, proposal_height) = *coordinate;
            let entry = entries
                .get(&lane_id)
                .expect("attempt coordinate lane has a storage entry");
            let inventory = inventories
                .get(&lane_id)
                .expect("attempt coordinate lane has an inventory");
            if inventory
                .attempts
                .contains_key(&(lane_block_height, proposal_height))
            {
                self.load_autonomous_reservation_attempt_locked(
                    entry,
                    inventory,
                    lane_id,
                    lane_block_height,
                    proposal_height,
                    expected_network_id,
                    *expected_epoch,
                    &mut decoded_bytes,
                    &mut attempts,
                )?;
            }
        }
        let mut classifications = Vec::with_capacity(groups.len());
        let mut claim_probes = 0_usize;
        let mut preflighted_claim_attempts = BTreeSet::new();
        for group in groups {
            let coordinate = (
                group.identity.lane_id,
                group.identity.lane_block_height,
                group.identity.proposal_height,
            );
            let inventory = inventories
                .get(&group.identity.lane_id)
                .expect("validated group lane has an inventory");
            let Some(attempt) = attempts.get(&coordinate).cloned() else {
                let same_height_attempt = inventory
                    .attempts
                    .keys()
                    .any(|(height, _)| *height == group.identity.lane_block_height);
                let same_height_view = inventory
                    .view_states
                    .keys()
                    .any(|(height, _)| *height == group.identity.lane_block_height);
                let same_height_pointer = inventory
                    .lane_latest
                    .contains_key(&group.identity.lane_block_height);
                let certified = certified_snapshots
                    .get(&group.identity.lane_id)
                    .and_then(|snapshot| {
                        snapshot
                            .requested
                            .get(&group.identity.lane_block_height)
                            .or_else(|| {
                                snapshot.frontier.as_ref().filter(|artifact| {
                                    artifact.proposal.descriptor.lane_block_height
                                        == group.identity.lane_block_height
                                })
                            })
                    })
                    .is_some();
                let certification_may_be_pruned = certified_snapshots
                    .get(&group.identity.lane_id)
                    .and_then(|snapshot| snapshot.frontier.as_ref())
                    .is_some_and(|frontier| {
                        frontier.proposal.descriptor.lane_block_height
                            > group.identity.lane_block_height
                    });
                if same_height_attempt || same_height_view || same_height_pointer {
                    return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                }
                if certified || certification_may_be_pruned {
                    return Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict);
                }
                classifications.push(AutonomousLaneReservationEvidenceV1::StrictlyAbsent);
                continue;
            };
            let entry = entries
                .get(&group.identity.lane_id)
                .expect("validated group lane has a storage entry");
            let latest = latest_attempts
                .get(&(
                    group.identity.lane_id,
                    group.identity.lane_block_height,
                ))
                .ok_or(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)?;
            let current_lane_height_attempt = latest.matches_payload(&attempt.payload);
            let descriptor = &attempt.payload.origin_proposal.descriptor;
            if !current_lane_height_attempt
                && (attempt.retirement.is_none()
                    || latest.proposal_height <= descriptor.proposal_height
                    || latest.epoch < attempt.payload.epoch
                    || latest.lane_incarnation != descriptor.lane_incarnation)
            {
                return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
            }
            let requested_bytes =
                norito::encode_canonical(&group.ordered_keys).map_err(Error::NoritoFrame)?;
            let durable_bytes = norito::encode_canonical(&attempt.payload.reservation_keys)
                .map_err(Error::NoritoFrame)?;
            if requested_bytes != durable_bytes {
                return Err(AutonomousLaneReservationEvidenceError::ReservationVectorConflict);
            }
            let certified_snapshot = certified_snapshots
                .get(&group.identity.lane_id)
                .expect("validated group lane has a certified snapshot");
            let certification = Self::autonomous_reservation_certification_for_payload(
                certified_snapshot,
                &attempt.payload,
            )?;
            if preflighted_claim_attempts.insert(coordinate) {
                self.preflight_autonomous_reservation_claims_locked(
                    entry,
                    inventory,
                    &attempt.payload,
                    attempt.retirement.as_ref(),
                    current_lane_height_attempt,
                    &mut attempts,
                    &mut claim_probes,
                    &mut scanned_entries,
                    &mut decoded_bytes,
                )?;
            }
            if attempt.retirement.is_none() {
                let other_coordinates = inventory
                    .attempts
                    .keys()
                    .filter_map(|(lane_block_height, proposal_height)| {
                        (*lane_block_height == group.identity.lane_block_height
                            && *proposal_height != group.identity.proposal_height)
                            .then_some((
                                group.identity.lane_id,
                                *lane_block_height,
                                *proposal_height,
                            ))
                    })
                    .collect::<Vec<_>>();
                for other_coordinate in other_coordinates {
                    self.load_autonomous_reservation_attempt_self_context_locked(
                        entry,
                        inventory,
                        other_coordinate.0,
                        other_coordinate.1,
                        other_coordinate.2,
                        expected_network_id,
                        &mut decoded_bytes,
                        &mut attempts,
                    )?;
                    let other = attempts
                        .get(&other_coordinate)
                        .cloned()
                        .expect("same-height attempt was loaded");
                    if other.retirement.is_none()
                        || other
                            .payload
                            .origin_proposal
                            .descriptor
                            .proposal_height
                            >= descriptor.proposal_height
                    {
                        return Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict);
                    }
                    if preflighted_claim_attempts.insert(other_coordinate) {
                        self.preflight_autonomous_reservation_claims_locked(
                            entry,
                            inventory,
                            &other.payload,
                            other.retirement.as_ref(),
                            false,
                            &mut attempts,
                            &mut claim_probes,
                            &mut scanned_entries,
                            &mut decoded_bytes,
                        )?;
                    }
                }
            }
            let AutonomousReservationAttemptRead {
                payload,
                retirement,
            } = attempt;
            classifications.push(match retirement {
                Some(retirement) => AutonomousLaneReservationEvidenceV1::ExactRetired {
                    payload,
                    retirement,
                    certification,
                },
                None => AutonomousLaneReservationEvidenceV1::ExactLive {
                    payload,
                    certification,
                },
            });
        }
        Ok(classifications)
    }
    #[cfg(test)]
    fn classify_autonomous_lane_reservation_group(
        &self,
        group: &LaneQueueReservationReconciliationGroupV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<
        AutonomousLaneReservationEvidenceV1,
        AutonomousLaneReservationEvidenceError,
    > {
        self.classify_autonomous_lane_reservation_groups(
            std::slice::from_ref(group),
            expected_network_id,
            std::slice::from_ref(&expected_epoch),
        )?
        .pop()
        .ok_or(AutonomousLaneReservationEvidenceError::InvalidGroup(
            "single-group classification produced no result",
        ))
    }
    };
}
