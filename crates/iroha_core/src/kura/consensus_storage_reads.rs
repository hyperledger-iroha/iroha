// Consensus reads distinguish a proved absence from unreadable durable evidence.
#[derive(Clone, Copy)]
enum CanonicalBlockReadAuthority<'a, 'k> {
    Published,
    Apply(&'a crate::block::VerifiedV2FinalityArtifact),
    Startup(&'a V2StartupFinalityVerificationSession<'k>),
}
impl Kura {
    /// Project active lane ownership from one authenticated finalized carrier.
    ///
    /// Genuine missing sidecars are reconstructed in memory. This lookup never
    /// publishes or repairs storage, including in emergency Fast mode. Invalid
    /// occupied evidence, malformed active markers, and changing authority are
    /// errors even when the caller's filter would otherwise hide the ownership.
    pub(crate) fn canonical_lane_block_artifacts_at_proposal_height_matching<F>(
        &self,
        proposal_height: u64,
        limit: usize,
        mut accept: F,
    ) -> Result<Vec<LaneBlockArtifact>>
    where
        F: FnMut(&SumeragiLanePayloadOwnership) -> bool,
    {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let height = NonZeroUsize::new(usize::try_from(proposal_height)?).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "zero canonical carrier height",
            )
        })?;
        let Some(block) = self.read_block_body(height)? else {
            return Ok(Vec::new());
        };
        let Some(bundle) = block.execution_context() else {
            return Ok(Vec::new());
        };
        let mut active = Vec::new();
        {
            let _geometry = self.lane_geometry_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            for ownership in &bundle.lane_payload_ownerships {
                if !Self::lane_payload_ownership_is_durable(ownership) {
                    continue;
                }
                ownership.validate_replay_material().map_err(|error| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), error.to_string())
                })?;
                if ownership.proposal_height != proposal_height {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "canonical ownership names another carrier height",
                    ));
                }
                let entry = self.lane_storage_entry(ownership.lane_id)?;
                let marker = self.active_lane_incarnation_marker(&entry)?;
                if ownership.proposal_height <= marker.1 {
                    // An authenticated newer incarnation retires this historical
                    // carrier. Its old ownership must not enter the active slot.
                    continue;
                }
                self.require_active_lane_ownership_artifact(&entry, ownership)?;
                active.push((
                    LaneBlockArtifact::new(block.hash(), ownership.clone()),
                    entry,
                    marker,
                ));
            }
        }
        // Authenticate present slots before invoking any caller-controlled
        // filter. The exact reader binds the namespace and active marker and
        // never interprets an occupied decode failure as a missing slot.
        for (artifact, _, _) in &active {
            self.confirm_canonical_lane_projection_slot(artifact)?;
        }
        let projected = active
            .iter()
            .filter(|(artifact, _, _)| accept(&artifact.ownership))
            .take(limit)
            .map(|(artifact, _, _)| artifact.clone())
            .collect::<Vec<_>>();
        if self.read_block_body(height)?.as_deref() != Some(block.as_ref()) {
            return Err(Error::CanonicalBlockWireMismatch {
                height: proposal_height,
            });
        }
        for (artifact, _, _) in &active {
            self.confirm_canonical_lane_projection_slot(artifact)?;
        }
        let _geometry = self.lane_geometry_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        for (artifact, entry, marker) in &active {
            let current = self.lane_storage_entry(artifact.ownership.lane_id)?;
            if current.dataspace_id != entry.dataspace_id
                || Self::lane_artifact_paths_for_entry(&current, &self.store_root)
                    != Self::lane_artifact_paths_for_entry(entry, &self.store_root)
                || self.active_lane_incarnation_marker(&current)? != *marker
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical projection active geometry changed",
                ));
            }
        }
        Ok(projected)
    }

    fn confirm_canonical_lane_projection_slot(&self, artifact: &LaneBlockArtifact) -> Result<()> {
        if self
            .read_lane_block_artifact_read_only(
                artifact.ownership.lane_id,
                artifact.ownership.lane_block_height,
            )?
            .is_some_and(|existing| existing != *artifact)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "occupied lane slot conflicts with signed canonical ownership",
            ));
        }
        Ok(())
    }

    /// Restore genuinely absent raw sidecars at an owned finalized recovery boundary.
    ///
    /// Each write reauthenticates the exact signed carrier and active slot
    /// under the prune, canonical, geometry and sidecar locks. Existing corrupt
    /// or conflicting evidence is preserved as an error, never overwritten.
    pub(crate) fn recover_canonical_lane_block_artifacts_at_proposal_height_matching<F>(
        &self,
        proposal_height: u64,
        limit: usize,
        accept: F,
    ) -> Result<Vec<LaneBlockArtifact>>
    where
        F: FnMut(&SumeragiLanePayloadOwnership) -> bool,
    {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "canonical lane artifact recovery",
            });
        }
        let artifacts = self.canonical_lane_block_artifacts_at_proposal_height_matching(
            proposal_height,
            limit,
            accept,
        )?;
        for artifact in &artifacts {
            self.recover_exact_canonical_lane_artifact(artifact)?;
        }
        Ok(artifacts)
    }

    fn recover_exact_canonical_lane_artifact(&self, artifact: &LaneBlockArtifact) -> Result<()> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        let ownership = &artifact.ownership;
        let height =
            NonZeroUsize::new(usize::try_from(ownership.proposal_height)?).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "zero canonical recovery height",
                )
            })?;
        let block = self
            .read_block_body_under_prune_and_canonical_guards(height)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical recovery body is unavailable",
                )
            })?;
        if block.hash() != artifact.proposal_block_hash
            || !block
                .execution_context()
                .is_some_and(|bundle| bundle.lane_payload_ownerships.contains(ownership))
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical recovery ownership differs from signed carrier",
            ));
        }
        let _geometry = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(ownership.lane_id)?;
        self.require_active_lane_ownership_artifact(&entry, ownership)?;
        let (data_path, index_path) = Self::lane_artifact_paths_for_entry(&entry, &self.store_root);
        let _sidecar = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        if !self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
            let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &namespace,
                &data_path,
                &index_path,
                "canonical lane recovery",
            )?;
            if let BoundProgressPair::Present(mut bound) =
                self.open_bound_progress_pair(&data_path, &index_path)?
            {
                if let Some(existing) = self.read_populated_consensus_lane_slot(
                    &mut bound,
                    ownership.lane_block_height,
                    "canonical lane recovery",
                    |bound| {
                        self.read_active_lane_block_artifact_from_bound_without_repair_locked(
                            &entry,
                            ownership.lane_block_height,
                            bound,
                        )
                    },
                )? {
                    return if existing == *artifact {
                        Ok(())
                    } else {
                        Err(Self::invalid_lane_artifact_error(
                            data_path,
                            "canonical recovery slot already contains conflicting evidence",
                        ))
                    };
                }
            }
        }
        let checkpoint = self.write_lane_block_artifact_locked(
            artifact,
            LaneBlockArtifactConflictPolicy::PreserveCanonical,
        )?;
        let BoundProgressPair::Present(mut bound) =
            self.open_bound_progress_pair(&data_path, &index_path)?
        else {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "published canonical lane slot is missing",
            ));
        };
        let confirmed = self.read_populated_consensus_lane_slot(
            &mut bound,
            ownership.lane_block_height,
            "canonical lane recovery readback",
            |bound| {
                self.read_active_lane_block_artifact_from_bound_without_repair_locked(
                    &entry,
                    ownership.lane_block_height,
                    bound,
                )
            },
        )?;
        if confirmed.as_ref() != Some(artifact) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "published canonical lane slot failed exact readback",
            ));
        }
        if checkpoint.is_some() {
            self.note_committed_lane_status_change();
        }
        Ok(())
    }

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

    /// Authenticate an Apply-owned durable append before finality publication.
    ///
    /// The opaque verified certificate selects the exact complete wire. Any
    /// published local finality must independently authenticate and equal that
    /// certificate; corrupt existing evidence is never bypassed.
    pub(crate) fn read_block_body_with_verified_finality(
        &self,
        height: NonZeroUsize,
        authority: &crate::block::VerifiedV2FinalityArtifact,
    ) -> Result<Option<Arc<SignedBlock>>> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_block_body_with_authority_under_guards(
            height,
            CanonicalBlockReadAuthority::Apply(authority),
        )
    }

    fn read_block_body_under_prune_and_canonical_guards(
        &self,
        height: NonZeroUsize,
    ) -> Result<Option<Arc<SignedBlock>>> {
        self.read_block_body_with_authority_under_guards(
            height,
            CanonicalBlockReadAuthority::Published,
        )
    }

    fn read_block_body_with_authority_under_guards(
        &self,
        height: NonZeroUsize,
        authority: CanonicalBlockReadAuthority<'_, '_>,
    ) -> Result<Option<Arc<SignedBlock>>> {
        if let CanonicalBlockReadAuthority::Startup(startup) = authority
            && !std::ptr::eq(self, startup.kura)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "startup canonical body authority belongs to another Kura",
            ));
        }
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
        let artifact = match authority {
            CanonicalBlockReadAuthority::Published => None,
            CanonicalBlockReadAuthority::Apply(authority) => Some(authority.artifact()),
            CanonicalBlockReadAuthority::Startup(startup) => {
                Some(startup.canonical_tip_finality_for_read(
                    self,
                    height_u64,
                    &store.path_to_blockchain,
                )?)
            }
        };
        // A startup session has already authenticated these exact files and
        // excludes canonical writers. Recheck its retained identities instead
        // of decoding the same historical finality and retained record again.
        let published_wire = if matches!(authority, CanonicalBlockReadAuthority::Startup(_)) {
            None
        } else {
            self.verified_v2_finality_wire_hash_for_eviction(
                &store.path_to_blockchain,
                height_u64,
                hash,
            )?
        };
        let (wire_len, wire_hash) = if let Some(artifact) = artifact {
            if artifact.height != height_u64
                || artifact.block_hash != hash
                || artifact.subject.parent_block_hash != parent
            {
                return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
            }
            let commitment = &artifact.commit_qc.execution_commitment;
            let expected = (
                commitment.executed_block_wire_len,
                commitment.executed_block_wire_hash,
            );
            if let Some(published_wire) = published_wire {
                let directory = Self::v2_finality_artifact_dir_for(&store.path_to_blockchain);
                let path =
                    Self::v2_finality_artifact_path_for(&store.path_to_blockchain, height_u64);
                let (record, _) = self
                    .decode_v2_finality_record_at(&path, &directory)?
                    .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
                if published_wire != expected || record.artifact != *artifact {
                    return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
                }
            }
            expected
        } else {
            published_wire.ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?
        };
        if slot.length != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let bytes = if slot.is_evicted() {
            let Some(bytes) = store.read_optional_da_cache(height_u64)? else {
                if let CanonicalBlockReadAuthority::Startup(startup) = authority {
                    startup.canonical_tip_finality_for_read(
                        self,
                        height_u64,
                        &store.path_to_blockchain,
                    )?;
                }
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
        if let Some(artifact) = artifact
            && block.canonical_proposal_wire_hash()? != artifact.subject.payload_hash
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
        if let CanonicalBlockReadAuthority::Startup(startup) = authority {
            startup.canonical_tip_finality_for_read(self, height_u64, &store.path_to_blockchain)?;
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

    /// Authenticate the occupied application slot independently of a candidate proposal.
    ///
    /// A valid receipt for another proposal is ordinary competing evidence. Callers
    /// decide whether that proposal is terminal only after this read authenticates
    /// the exact stored receipt and canonical execution evidence.
    pub(crate) fn read_lane_application_receipt(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "lane completion durability",
            });
        }
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_lane_application_receipt_under_guards(lane_id, lane_block_height, true)
    }

    fn read_lane_completion_receipt_under_guards(
        &self,
        proposal: &LaneBlockProposalV1,
        attest_durability: bool,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        let artifact = self.read_lane_application_receipt_under_guards(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            attest_durability,
        )?;
        if artifact
            .as_ref()
            .is_some_and(|artifact| artifact.proposal != *proposal)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "occupied lane receipt conflicts with the exact finalized proposal",
            ));
        }
        Ok(artifact)
    }

    fn read_lane_application_receipt_under_guards(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        attest_durability: bool,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        let Some(artifact) = self.read_lane_completion_receipt_structural(
            lane_id,
            lane_block_height,
            attest_durability,
        )?
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
            .read_lane_completion_receipt_structural(lane_id, lane_block_height, false)?
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
        lane_id: LaneId,
        lane_block_height: u64,
        attest_durability: bool,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        let _geometry = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
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
                lane_block_height,
                "lane receipt",
                |bound| {
                    self.read_lane_block_application_receipt_from_bound_locked(
                        lane_id,
                        lane_block_height,
                        bound,
                    )
                },
            )?,
        };
        if let Some(artifact) = &artifact {
            self.require_active_lane_artifact(&entry, &artifact.proposal.descriptor)?;
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

    /// Read the active autonomous attempt without recovering or rewriting sidecars.
    ///
    /// Only a genuinely missing current pointer or authenticated retirement is
    /// absent. Occupied invalid pointers, payloads, and view state are errors.
    pub(crate) fn read_current_autonomous_lane_block_artifact(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockArtifact>> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_current_autonomous_lane_block_artifact_under_guards(
            lane_id,
            lane_block_height,
            network_id,
            epoch,
        )
    }

    /// Project the immutable payload and authenticated synthetic NewView cursor.
    pub(crate) fn read_current_autonomous_lane_payload(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    ) -> Result<Option<(LaneExecutablePayloadV1, LaneBlockProposalV1)>> {
        let Some(artifact) = self.read_current_autonomous_lane_block_artifact(
            lane_id,
            lane_block_height,
            network_id,
            epoch,
        )?
        else {
            return Ok(None);
        };
        let current = Self::validate_autonomous_lane_block_artifact(&artifact, network_id, epoch)
            .map_err(|reason| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), reason)
        })?;
        Ok(Some((artifact.executable_payload, current)))
    }

    fn read_current_autonomous_lane_block_artifact_under_guards(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockArtifact>> {
        let _geometry = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let marker = self.active_lane_incarnation_marker(&entry)?;
        let path = Self::autonomous_lane_block_latest_attempt_path_for_entry(
            &entry,
            &self.store_root,
            lane_block_height,
        );
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "autonomous current pointer has no parent directory",
            )
        })?;
        let _sidecar = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let bytes = self.read_regular_sidecar_bytes(
            &path,
            parent,
            AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
        )?;
        let artifact = if let Some(bytes) = &bytes {
            let pointer = Self::decode_autonomous_lane_block_latest_attempt(&path, bytes)?;
            if pointer.lane_id != lane_id
                || pointer.dataspace_id != entry.dataspace_id
                || pointer.lane_block_height != lane_block_height
                || pointer.lane_incarnation != marker.0
                || pointer.proposal_height <= marker.1
            {
                return Err(Self::invalid_lane_artifact_error(
                    path.clone(),
                    "occupied autonomous current pointer conflicts with its active slot",
                ));
            }
            let record = self
                .read_autonomous_lane_block_attempt_artifact_with_view_state_mode_locked(
                    &entry,
                    &pointer,
                    network_id,
                    epoch,
                    AutonomousLaneBlockViewStateReadMode::LatestReadOnly,
                )?;
            record.retirement.is_none().then_some(record.artifact)
        } else {
            None
        };
        if self.active_lane_incarnation_marker(&entry)? != marker
            || self.read_regular_sidecar_bytes(
                &path,
                parent,
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
            )? != bytes
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous current pointer or active marker changed during authentication",
            ));
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
        let artifact = self.read_current_autonomous_lane_block_artifact_under_guards(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            network_id,
            epoch,
        )?;
        if artifact
            .as_ref()
            .is_some_and(|artifact| artifact.executable_payload.origin_proposal != *proposal)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "occupied autonomous completion slot conflicts with the finalized proposal",
            ));
        }
        Ok(artifact)
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
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_lane_block_artifact_under_prune_and_canonical_guards(lane_id, lane_block_height)
    }

    /// The caller owns prune and canonical serialization; the shared strict
    /// indexed reader still binds and revalidates the active namespace.
    fn read_lane_block_artifact_under_prune_and_canonical_guards(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Result<Option<LaneBlockArtifact>> {
        self.read_consensus_lane_artifact_matching_with_authentication(
            lane_id,
            Some(lane_block_height),
            |_| true,
            |artifact| self.validate_consensus_lane_block_artifact_canonical_under_guard(artifact),
        )
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
        accept: F,
    ) -> Result<Option<LaneBlockArtifact>>
    where
        F: FnMut(&LaneBlockArtifact) -> bool,
    {
        self.read_consensus_lane_artifact_matching_with_authentication(
            lane_id,
            requested_height,
            accept,
            |artifact| self.validate_consensus_lane_block_artifact_canonical(artifact),
        )
    }

    fn read_consensus_lane_artifact_matching_with_authentication<F, A>(
        &self,
        lane_id: LaneId,
        requested_height: Option<u64>,
        mut accept: F,
        mut authenticate: A,
    ) -> Result<Option<LaneBlockArtifact>>
    where
        F: FnMut(&LaneBlockArtifact) -> bool,
        A: FnMut(LaneBlockArtifact) -> Result<LaneBlockArtifact>,
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
            let canonical = authenticate(candidate)?;
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
        self.validate_consensus_lane_block_artifact_canonical_under_guard(artifact)
    }

    fn validate_consensus_lane_block_artifact_canonical_under_guard(
        &self,
        artifact: LaneBlockArtifact,
    ) -> Result<LaneBlockArtifact> {
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
