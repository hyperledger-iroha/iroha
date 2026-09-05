// Consensus reads distinguish a proved absence from unreadable durable evidence.
impl Kura {
    /// Read an authenticated finalized body without consulting or publishing caches.
    ///
    /// An uncommitted height, authenticated imported prefix, or authenticated
    /// evicted body without a local replica is absent. Invalid occupied storage
    /// is an error. An occupied append whose finality is not yet published is
    /// an explicit `MissingV2FinalityArtifact` error, never finalized evidence.
    /// The reader never repairs or synchronizes storage.
    pub(crate) fn read_block_body(&self, height: NonZeroUsize) -> Result<Option<Arc<SignedBlock>>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.read_block_body_under_prune_and_canonical_guards(height)
    }

    fn read_block_body_under_prune_and_canonical_guards(
        &self,
        height: NonZeroUsize,
    ) -> Result<Option<Arc<SignedBlock>>> {
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let height_u64 = u64::try_from(height.get())?;
        let position = height_u64 - 1;
        let mut store = self.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        if height_u64 > count {
            return Ok(None);
        }
        if self.is_hard_fork_hash_only_block(height.get() - 1) {
            self.ensure_snapshot_bootstrap_authenticated()?;
            return Ok(None);
        }
        let hash = Self::read_durable_hash_at_height(&mut store, height_u64)?
            .ok_or(Error::HashesFileHeightMismatch)?;
        let parent = if position == 0 {
            None
        } else {
            Some(
                Self::read_durable_hash_at_height(&mut store, position)?
                    .ok_or(Error::HashesFileHeightMismatch)?,
            )
        };
        let slot = store.read_block_index(position)?;
        if slot.length == 0 || slot.length > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::CorruptedBlockLength {
                length: slot.length,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        let (wire_len, wire_hash) = self
            .verified_v2_finality_wire_hash_for_eviction(
                &store.path_to_blockchain,
                height_u64,
                hash,
            )?
            .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
        if slot.length != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let bytes = if slot.is_evicted() {
            let Some(bytes) = store.read_optional_da_cache(height_u64)? else {
                return Ok(None);
            };
            bytes
        } else {
            let mut bytes = vec![0; usize::try_from(slot.length)?];
            store.read_block_data(slot.start, &mut bytes)?;
            bytes
        };
        if u64::try_from(bytes.len())? != wire_len || Hash::new(&bytes) != wire_hash {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let block = decode_framed_signed_block(&bytes)?;
        if block.hash() != hash
            || block.header().height().get() != height_u64
            || block.header().prev_block_hash() != parent
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let confirmed_slot = store.read_block_index(position)?;
        if store.read_exact_durable_index_count()? != count
            || confirmed_slot.start != slot.start
            || confirmed_slot.length != slot.length
            || Self::read_durable_hash_at_height(&mut store, height_u64)? != Some(hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        Ok(Some(Arc::new(block)))
    }

    /// Reattest a strictly decoded certificate before live lane completion.
    pub(crate) fn read_lane_completion_certificate(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Result<Option<CertifiedLaneBlockArtifact>> {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "lane completion durability",
            });
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.read_certified_lane_block_artifact_read_only_under_prune_and_canonical_guards(
            lane_id,
            lane_block_height,
            true,
        )
    }

    /// Read and reattest the exact applied receipt for live completion.
    pub(crate) fn read_lane_completion_receipt(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "lane completion durability",
            });
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.read_lane_completion_receipt_under_guards(proposal, true)
    }

    fn read_lane_completion_receipt_under_guards(
        &self,
        proposal: &LaneBlockProposalV1,
        attest_durability: bool,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        let Some(artifact) =
            self.read_lane_completion_receipt_structural(proposal, attest_durability)?
        else {
            return Ok(None);
        };
        if let Some(height) = usize::try_from(artifact.application_block_height)
            .ok()
            .and_then(NonZeroUsize::new)
        {
            self.read_block_body_under_prune_and_canonical_guards(height)?;
        }
        if !self.lane_block_application_receipt_matches_available_evidence_under_prune_and_canonical_guards(&artifact, false) {
            return Err(Self::invalid_lane_artifact_error(self.store_root.clone(), "occupied lane receipt conflicts with canonical execution evidence"));
        }
        if self
            .read_lane_completion_receipt_structural(proposal, false)?
            .as_ref()
            != Some(&artifact)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "lane receipt changed during execution evidence authentication",
            ));
        }
        Ok(Some(artifact))
    }

    fn read_lane_completion_receipt_structural(
        &self,
        proposal: &LaneBlockProposalV1,
        attest_durability: bool,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        let _geometry = self.lane_geometry_lock.lock();
        let descriptor = &proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.active_lane_incarnation_marker(&entry)?;
        let (data_path, index_path) =
            Self::lane_block_application_receipt_paths_for_entry(&entry, &self.store_root);
        let _sidecar = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        if self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
            return Ok(None);
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            &data_path,
            &index_path,
            "lane receipt",
        )?;
        let mut pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let artifact = match &mut pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => self.read_populated_consensus_lane_slot(
                bound,
                descriptor.lane_block_height,
                "lane receipt",
                |bound| {
                    self.read_lane_block_application_receipt_from_bound_locked(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                        bound,
                    )
                },
            )?,
        };
        if let Some(artifact) = &artifact {
            self.require_active_lane_artifact(&entry, &artifact.proposal.descriptor)?;
            if artifact.proposal != *proposal {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "occupied lane receipt conflicts with the exact finalized proposal",
                ));
            }
            if attest_durability
                && let BoundProgressPair::Present(bound) = &pair
                && !self.sync_bound_progress_sidecar(bound, "lane receipt")
            {
                return Err(Self::invalid_lane_artifact_error(
                    index_path,
                    "lane receipt durability barrier failed",
                ));
            }
        }
        Ok(artifact)
    }

    /// Read exact autonomous completion evidence without view-state recovery.
    pub(crate) fn read_lane_completion_autonomous_artifact(
        &self,
        proposal: &LaneBlockProposalV1,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockArtifact>> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_lane_completion_autonomous_artifact_under_guards(proposal, network_id, epoch)
    }

    fn read_lane_completion_autonomous_artifact_under_guards(
        &self,
        proposal: &LaneBlockProposalV1,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockArtifact>> {
        let _geometry = self.lane_geometry_lock.lock();
        let descriptor = &proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        let _sidecar = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let Some(record) = self.read_autonomous_lane_block_record_read_only_latest_locked(
            &entry,
            descriptor.lane_id,
            descriptor.lane_block_height,
            network_id,
            epoch,
        )?
        else {
            return Ok(None);
        };
        if record.artifact.executable_payload.origin_proposal != *proposal {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "occupied autonomous completion slot conflicts with the finalized proposal",
            ));
        }
        Ok(record.retirement.is_none().then_some(record.artifact))
    }

    /// Decode an occupied indexed slot, keeping malformed vacancy and decode
    /// failures distinct from a canonical empty slot. The caller owns geometry
    /// and sidecar locks and has already rejected writer recovery artifacts.
    fn read_populated_consensus_lane_slot<T>(
        &self,
        pair: &mut BoundProgressSidecar,
        height: u64,
        kind: &str,
        decode: impl FnOnce(&mut BoundProgressSidecar) -> Option<T>,
    ) -> Result<Option<T>> {
        let index_path = pair.namespace.index_path.clone();
        let invalid = |detail: &str| {
            Self::invalid_lane_artifact_error(index_path.clone(), format!("{kind}: {detail}"))
        };
        if height == 0 {
            return Err(invalid("zero lane slot height"));
        }
        let Some(range) = self.bound_indexed_sidecar_height_range(pair, kind)? else {
            return Ok(None);
        };
        if !range.contains(&height) {
            return Ok(None);
        }
        let length = pair
            .index
            .metadata()
            .map_err(|error| Error::IO(error, index_path.clone()))?
            .len();
        let layout = SidecarIndexLayout::read_from(&mut pair.index, length)
            .map_err(|reason| invalid(reason))?;
        let position = layout
            .entry_position(height)
            .ok_or_else(|| invalid("indexed slot disappeared"))?;
        let mut bytes = [0; PIPELINE_INDEX_ENTRY_SIZE];
        pair.index
            .seek(SeekFrom::Start(position))
            .and_then(|_| pair.index.read_exact(&mut bytes))
            .map_err(|error| Error::IO(error, index_path.clone()))?;
        let slot = SidecarIndexEntry::from_bytes(bytes);
        let value = if slot.len == 0 {
            if slot.offset != 0 {
                return Err(invalid("empty indexed slot has a nonzero offset"));
            }
            None
        } else {
            Some(decode(pair).ok_or_else(|| invalid("occupied indexed slot is unreadable, malformed, or conflicts with its authority"))?)
        };
        if !self.bound_progress_sidecar_unchanged(pair) {
            return Err(invalid(
                "indexed storage changed during slot authentication",
            ));
        }
        Ok(value)
    }

    /// Read one exact active lane artifact without repairing sidecars.
    ///
    /// Only an authenticated empty slot is absent. An occupied malformed slot,
    /// stale incarnation, or changed canonical anchor is a storage error.
    pub(crate) fn read_lane_block_artifact_read_only(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Result<Option<LaneBlockArtifact>> {
        self.read_consensus_lane_artifact_matching(lane_id, Some(lane_block_height), |_| true)
    }

    /// Read a canonical active lane frontier through a bounded, read-only scan.
    ///
    /// Only a complete valid scan proves absence. Malformed occupied slots,
    /// stale geometry, local recovery state, and an exhausted scan are errors.
    pub(crate) fn latest_lane_block_artifact_matching<F>(
        &self,
        lane_id: LaneId,
        accept: F,
    ) -> Result<Option<LaneBlockArtifact>>
    where
        F: FnMut(&LaneBlockArtifact) -> bool,
    {
        self.read_consensus_lane_artifact_matching(lane_id, None, accept)
    }

    fn read_consensus_lane_artifact_matching<F>(
        &self,
        lane_id: LaneId,
        requested_height: Option<u64>,
        mut accept: F,
    ) -> Result<Option<LaneBlockArtifact>>
    where
        F: FnMut(&LaneBlockArtifact) -> bool,
    {
        if requested_height == Some(0) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "zero lane artifact height",
            ));
        }
        self.ensure_prune_recovery_not_required()?;
        let geometry = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let marker = self.active_lane_incarnation_marker(&entry)?;
        let (data_path, index_path) = Self::lane_artifact_paths_for_entry(&entry, &self.store_root);
        let sidecar = self.sidecar_lock.lock();
        if self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
            self.ensure_prune_recovery_not_required()?;
            return Ok(None);
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            &data_path,
            &index_path,
            "lane frontier",
        )?;
        let mut pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let mut candidates = Vec::new();
        let mut complete_scan = true;
        if let BoundProgressPair::Present(bound) = &mut pair {
            if let Some(range) = self.bound_indexed_sidecar_height_range(bound, "lane frontier")? {
                let range = requested_height.map_or(range, |height| height..=height);
                complete_scan = range.end().saturating_sub(*range.start()).saturating_add(1)
                    <= u64::try_from(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET).unwrap_or(u64::MAX);
                for height in range.rev().take(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET) {
                    if let Some(artifact) = self.read_populated_consensus_lane_slot(
                        bound,
                        height,
                        "lane frontier",
                        |bound| {
                            self.read_active_lane_block_artifact_from_bound_without_repair_locked(
                                &entry, height, bound,
                            )
                        },
                    )? {
                        candidates.push(artifact);
                    }
                }
            }
        }
        drop(sidecar);
        drop(geometry);
        // Canonical block locks must never be taken under sidecar_lock.
        let mut selected = None;
        for candidate in candidates {
            let canonical = self.validate_consensus_lane_block_artifact_canonical(candidate)?;
            if accept(&canonical) {
                selected = Some(canonical);
                break;
            }
        }
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let stable = match &pair {
            BoundProgressPair::Present(bound) => self.bound_progress_sidecar_unchanged(bound),
            BoundProgressPair::Absent(bound) => {
                self.bound_progress_namespace_unchanged(bound)
                    && self
                        .open_optional_bound_progress_file(bound, &data_path)?
                        .is_none()
                    && self
                        .open_optional_bound_progress_file(bound, &index_path)?
                        .is_none()
            }
        };
        let current = self.lane_storage_entry(lane_id)?;
        if !stable
            || current.dataspace_id != entry.dataspace_id
            || Self::lane_artifact_paths_for_entry(&current, &self.store_root)
                != (data_path.clone(), index_path.clone())
            || self.active_lane_incarnation_marker(&current)? != marker
        {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "lane frontier changed during canonical authentication",
            ));
        }
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            &data_path,
            &index_path,
            "lane frontier",
        )?;
        if selected.is_none() && !complete_scan {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "bounded lane frontier scan could not prove absence",
            ));
        }
        Ok(selected)
    }

    fn validate_consensus_lane_block_artifact_canonical(
        &self,
        artifact: LaneBlockArtifact,
    ) -> Result<LaneBlockArtifact> {
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let height = artifact.ownership.proposal_height;
        let mut store = self.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        if height == 0
            || height > count
            || Self::read_durable_hash_at_height(&mut store, height)?
                != Some(artifact.proposal_block_hash)
            || store.read_exact_durable_index_count()? != count
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "lane artifact conflicts with its exact durable canonical anchor",
            ));
        }
        self.ensure_prune_recovery_not_required()?;
        Ok(artifact)
    }
}
