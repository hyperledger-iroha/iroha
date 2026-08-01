/// Complete hash-addressed evidence required to execute one autonomous lane
/// block in a canonical merge batch on a validator that missed original
/// committee fanout.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLaneMergeBundleV1 {
    /// Bundle schema version. Only version one is accepted.
    pub(crate) version: u8,
    /// Immutable payload, origin availability proof, and authenticated cursor chain.
    pub(crate) autonomous: AutonomousLaneBlockArtifact,
    /// Immutable origin proposal with prepare/commit QCs and signer PoPs.
    pub(crate) certified: CertifiedLaneBlockArtifact,
}

impl AutonomousLaneMergeBundleV1 {
    /// Exact coordinated first-release layout accepted by Kura and merge transport.
    pub(crate) const VERSION: u8 = 1;
    /// Stable persistence label for the independently durable canonical bundle pair.
    pub(crate) const FORMAT_LABEL: &'static str = "lane.autonomous_merge_bundle.v1";

    /// Canonical framed bytes used by authenticated bundle transport and merge logs.
    pub(crate) fn encode_framed(&self) -> Result<Vec<u8>> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
            return Err(Error::NoritoFrame(norito::Error::Message(
                "autonomous lane merge bundle exceeds hard byte limit".to_owned(),
            )));
        }
        Ok(bytes)
    }

    /// Domain-separated digest committed by canonical merge batches.
    pub(crate) fn bundle_hash(&self) -> Result<Hash> {
        let bytes = self.encode_framed()?;
        Ok(Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
            &bytes,
        ]))
    }

    /// Exact producer-authenticated executable payload.
    pub(crate) const fn executable_payload(&self) -> &LaneExecutablePayloadV1 {
        &self.autonomous.executable_payload
    }
}

/// Exact durable autonomous source admitted to canonical merge construction.
///
/// Construction requires both the independently durable canonical bundle
/// data/index slot and its separately durable autonomous attempt, READY
/// certificate, certified slot, and execution-input slot. The bundle bytes
/// must exactly reconstruct from those components under active lane geometry;
/// neither the persisted copy nor the derived view is trusted on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DurableAutonomousLaneMergeSource {
    /// Fully authenticated producer payload and lane certificate.
    pub(crate) bundle: AutonomousLaneMergeBundleV1,
    /// Exact canonical bytes carried unchanged into the merge transcript.
    pub(crate) source_bundle: Vec<u8>,
    /// Domain-separated digest of `source_bundle`.
    pub(crate) bundle_hash: Hash,
    /// Exact independently durable execution input used for deterministic replay.
    pub(crate) input: LaneBlockExecutionInputArtifact,
}

/// Move-only authority to sign one autonomous lane READY vote.
///
/// Kura is the only module that can construct this value. Construction follows
/// a canonical, repair-disabled execution-input readback and binds the exact
/// durable artifact to its proposal, executable payload, FIFO reservation
/// group, validator, and height-context session. The lane signer consumes the
/// value, so retaining an in-memory READY body is not sufficient authority.
#[must_use = "a durable lane READY authorization must be consumed by the exact signer session"]
pub(crate) struct LaneReadyAuthorization {
    durable_execution_input_hash: Hash,
    proposal: LaneBlockProposalV1,
    availability_body: LanePayloadAvailabilityBodyV1,
    reservation_group: LaneQueueReservationGroupBindingV1,
    producer: PeerId,
    signer: PeerId,
    height_context_id: HeightContextId,
}

impl LaneReadyAuthorization {
    /// Return whether this one-shot authority names the exact READY signing
    /// request and still has a structurally complete durable-input binding.
    pub(crate) fn matches_signing_request(
        &self,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let group = self.reservation_group;
        self.proposal == *proposal
            && self.availability_body == *availability_body
            && self.signer == *signer
            && self.height_context_id == height_context_id
            && self
                .durable_execution_input_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0)
            && group.identity.lane_id == descriptor.lane_id
            && group.identity.dataspace_id == descriptor.dataspace_id
            && group.identity.lane_incarnation == descriptor.lane_incarnation
            && group.identity.proposal_height == descriptor.proposal_height
            && group.identity.lane_block_height == descriptor.lane_block_height
            && group.identity.lane_block_view == descriptor.lane_block_view
            && group.reservation_count
                == u64::try_from(descriptor.accepted_transaction_hashes.len()).unwrap_or(u64::MAX)
    }

    /// Consume this exact durable-input authority at the READY signature
    /// boundary after rechecking the complete first-release projection.
    pub(crate) fn consume_signing_request(
        self,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> bool {
        if !self.matches_signing_request(proposal, availability_body, signer, height_context_id) {
            return false;
        }
        let descriptor = &proposal.descriptor;
        let Ok(validator_count) = u8::try_from(descriptor.validator_set.len()) else {
            return false;
        };
        if validator_count == 0 || validator_count > 128 {
            return false;
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let Some(producer_index) = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &self.producer)
        else {
            return false;
        };
        let Some(signer_index) = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == signer)
        else {
            return false;
        };
        let Some(producer) = u32::try_from(producer_index)
            .ok()
            .and_then(|index| 1_u128.checked_shl(index))
        else {
            return false;
        };
        let Some(actor) = u32::try_from(signer_index)
            .ok()
            .and_then(|index| 1_u128.checked_shl(index))
        else {
            return false;
        };
        let selected_count = self.reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return false;
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let payload_owners = producer | actor;
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: actor,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: actor,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v4: true,
                ever_reservation_v5: true,
                ever_execution_input_durable: actor,
                ever_ready_authorized: actor,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.history.ready_signed = actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY,
            actor,
            target: 0,
            before,
            after,
        };
        check_production_in_flight_first_release_transition(projection)
            .is_some_and(|checked| checked.into_projection() == projection)
    }
}

impl Kura {
    fn autonomous_lane_merge_bundle_paths_for_entry(
        entry: &LaneConfigEntry,
        store_root: &Path,
    ) -> (PathBuf, PathBuf) {
        let dir = Self::lane_artifact_dir(&entry.blocks_dir(store_root));
        (
            dir.join(AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE),
            dir.join(AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE),
        )
    }
    pub(crate) fn validate_certified_lane_block_artifact(
        artifact: &CertifiedLaneBlockArtifact,
    ) -> std::result::Result<(), &'static str> {
        #[cfg(test)]
        if FAIL_NEXT_CERTIFIED_LANE_BLOCK_ARTIFACT_VALIDATION.with(|flag| flag.replace(false)) {
            return Err("injected certified lane block artifact validation failure");
        }
        artifact
            .encode_framed()
            .map_err(|_| "certified lane block exceeds the merge source envelope byte limit")?;
        crate::lane_consensus::validate_lane_block_proposal(&artifact.proposal)
            .map_err(|_| "invalid lane block proposal")?;
        crate::lane_consensus::validate_lane_block_qc(&artifact.prepare_qc)
            .map_err(|_| "invalid prepare lane block QC")?;
        crate::lane_consensus::validate_lane_block_qc(&artifact.commit_qc)
            .map_err(|_| "invalid commit lane block QC")?;

        let descriptor = &artifact.proposal.descriptor;
        let prepare_body = artifact.proposal.vote_body(CertPhase::Prepare);
        let commit_body = artifact.proposal.vote_body(CertPhase::Commit);
        if artifact.prepare_qc.body != prepare_body {
            return Err("prepare QC body does not match proposal");
        }
        if artifact.commit_qc.body != commit_body {
            return Err("commit QC body does not match proposal");
        }
        for qc in [&artifact.prepare_qc, &artifact.commit_qc] {
            if qc.validator_set_hash_version != descriptor.validator_set_hash_version
                || qc.validator_set_hash != descriptor.validator_set_hash
                || qc.validator_set != descriptor.validator_set
            {
                return Err("QC validator set does not match proposal");
            }
        }
        let mut expected_pops = Self::lane_block_qc_signer_keys(&artifact.prepare_qc)?;
        expected_pops.extend(Self::lane_block_qc_signer_keys(&artifact.commit_qc)?);
        let actual_pops = artifact
            .signer_pops
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_pops != expected_pops {
            return Err("certified lane block signer PoPs do not match QC signers");
        }
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &artifact.prepare_qc,
            &artifact.signer_pops,
        )
        .map_err(|_| "invalid prepare lane block QC aggregate")?;
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &artifact.commit_qc,
            &artifact.signer_pops,
        )
        .map_err(|_| "invalid commit lane block QC aggregate")?;
        Ok(())
    }

    fn validate_autonomous_lane_block_artifact(
        artifact: &AutonomousLaneBlockArtifact,
        expected_chain_id_hash: Hash,
        expected_epoch: u64,
    ) -> std::result::Result<LaneBlockProposalV1, &'static str> {
        artifact
            .encode_framed()
            .map_err(|_| "autonomous lane block exceeds the merge source byte limit")?;
        match artifact.format {
            AutonomousLaneBlockArtifactFormat::Current => {}
        }
        artifact
            .executable_payload
            .validate(expected_chain_id_hash, expected_epoch)
            .map_err(|_| "invalid autonomous executable payload")?;
        if artifact
            .executable_payload
            .origin_proposal
            .descriptor
            .lane_block_view
            != 0
        {
            return Err("autonomous executable payload must originate at lane view zero");
        }
        if artifact.new_view_certificates.len() > MAX_LANE_NEW_VIEW_CERTIFICATES {
            return Err("autonomous lane NewView certificate limit exceeded");
        }
        if let Some(certificate) = &artifact.availability_certificate {
            crate::lane_consensus::validate_lane_payload_availability_certificate(
                certificate,
                &artifact.executable_payload,
                expected_chain_id_hash,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane payload availability certificate")?;
        }

        let mut current = artifact.executable_payload.origin_proposal.clone();
        if let Some(checkpoint) = &artifact.view_checkpoint {
            crate::lane_consensus::validate_lane_block_view_checkpoint(
                checkpoint,
                &artifact.executable_payload,
                expected_chain_id_hash,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane view checkpoint")?;
            current = checkpoint.target_proposal.clone();
        }
        for durable in &artifact.new_view_certificates {
            let target = crate::lane_consensus::retarget_lane_block_proposal_view(
                &current,
                durable.certificate.body.target_view,
            )
            .map_err(|_| "autonomous lane NewView target is not contiguous")?;
            crate::lane_consensus::validate_lane_block_new_view_transition(
                &current,
                &target,
                &artifact.executable_payload,
                durable,
                expected_chain_id_hash,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane NewView transition")?;
            current = target;
        }
        Ok(current)
    }

    /// Validate a complete autonomous merge source without consulting mutable
    /// committee state or local sidecars.
    pub(crate) fn validate_autonomous_lane_merge_bundle(
        bundle: &AutonomousLaneMergeBundleV1,
        expected_chain_id_hash: Hash,
        expected_epoch: u64,
    ) -> std::result::Result<(), &'static str> {
        if bundle.version != AutonomousLaneMergeBundleV1::VERSION {
            return Err("unsupported autonomous lane merge bundle version");
        }
        if bundle
            .encode_framed()
            .map_err(|_| "oversized autonomous lane merge bundle")?
            .len()
            > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES
        {
            return Err("autonomous lane merge bundle exceeds hard byte limit");
        }
        if bundle.autonomous.availability_certificate.is_none() {
            return Err("autonomous lane merge bundle lacks a durable availability certificate");
        }
        let _cursor = Self::validate_autonomous_lane_block_artifact(
            &bundle.autonomous,
            expected_chain_id_hash,
            expected_epoch,
        )?;
        Self::validate_certified_lane_block_artifact(&bundle.certified)?;
        let availability = bundle
            .autonomous
            .availability_certificate
            .as_ref()
            .ok_or("autonomous lane merge bundle lacks a durable availability certificate")?;
        let origin = &bundle.autonomous.executable_payload.origin_proposal;
        if &bundle.certified.proposal != origin {
            return Err("autonomous lane merge bundle must certify the immutable origin proposal");
        }
        if availability.certificate != bundle.certified.prepare_qc
            || bundle.certified.prepare_qc.body != origin.vote_body(CertPhase::Prepare)
        {
            return Err("payload availability certificate is not the exact origin prepare QC");
        }
        Ok(())
    }

    /// Decode exact canonical framed bundle bytes and verify all embedded proofs.
    pub(crate) fn decode_autonomous_lane_merge_bundle(
        bytes: &[u8],
        expected_chain_id_hash: Hash,
        expected_epoch: u64,
    ) -> std::result::Result<AutonomousLaneMergeBundleV1, &'static str> {
        if bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
            return Err("autonomous lane merge bundle exceeds hard byte limit");
        }
        let bundle =
            norito::decode_canonical::<AutonomousLaneMergeBundleV1>(bytes).map_err(|error| {
                match error {
                    norito::Error::NonCanonicalEncoding => {
                        "autonomous lane merge bundle is not canonical framed Norito"
                    }
                    _ => "autonomous lane merge bundle is not valid framed Norito",
                }
            })?;
        Self::validate_autonomous_lane_merge_bundle(
            &bundle,
            expected_chain_id_hash,
            expected_epoch,
        )?;
        Ok(bundle)
    }

    fn autonomous_lane_merge_bundle_pair_entry_limit(&self) -> usize {
        self.roster_sidecar_retention
            .get()
            .saturating_add(usize::try_from(MAX_INDEXED_SIDECAR_GAP_ENTRIES).unwrap_or(usize::MAX))
            .min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)
    }

    fn autonomous_lane_merge_bundle_pair_byte_limit(&self) -> usize {
        self.pending_control_sidecar_limits.aggregate_bytes
    }

    /// Validate the whole bundle pair before any exact-slot admission.
    ///
    /// The configured retention window plus the existing bounded sparse-gap
    /// allowance limits index work. The established autonomous sidecar byte
    /// budget limits payload exposure. Entries must describe one contiguous,
    /// append-only data image, so truncated, overlapping, trailing, and
    /// oversized pairs fail before any payload allocation.
    fn validate_autonomous_lane_merge_bundle_pair_layout_locked(
        &self,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<(SidecarIndexLayout, BTreeSet<u64>), &'static str> {
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed before bounded validation");
        }
        let index_len = bound
            .index
            .metadata()
            .map_err(|_| "autonomous merge bundle index metadata is unreadable")?
            .len();
        let layout = SidecarIndexLayout::read_from(&mut bound.index, index_len)
            .map_err(|_| "autonomous merge bundle index is malformed")?;
        if layout.aligned_len != index_len {
            return Err("autonomous merge bundle index has trailing or partial bytes");
        }
        if usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
            > self.autonomous_lane_merge_bundle_pair_entry_limit()
        {
            return Err("autonomous merge bundle index exceeds its bounded entry count");
        }
        let data_len = bound
            .data
            .metadata()
            .map_err(|_| "autonomous merge bundle data metadata is unreadable")?
            .len();
        if data_len
            > u64::try_from(self.autonomous_lane_merge_bundle_pair_byte_limit()).unwrap_or(u64::MAX)
        {
            return Err("autonomous merge bundle data exceeds its aggregate byte budget");
        }

        bound
            .index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|_| "autonomous merge bundle index entries are unreadable")?;
        let mut heights = BTreeSet::new();
        let mut indexed_end = 0_u64;
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for offset in 0..layout.entry_count {
            bound
                .index
                .read_exact(&mut entry_bytes)
                .map_err(|_| "autonomous merge bundle index entry is unreadable")?;
            let entry = SidecarIndexEntry::from_bytes(entry_bytes);
            if entry.len == 0 {
                if entry.offset != 0 {
                    return Err("empty autonomous merge bundle index entry has a non-zero offset");
                }
                continue;
            }
            if entry.len
                > u64::try_from(MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES).unwrap_or(u64::MAX)
            {
                return Err("autonomous merge bundle entry exceeds its hard byte limit");
            }
            if entry.offset != indexed_end {
                return Err(
                    "autonomous merge bundle data ranges are overlapping, gapped, or reordered",
                );
            }
            indexed_end = entry
                .offset
                .checked_add(entry.len)
                .ok_or("autonomous merge bundle data range overflows")?;
            if indexed_end > data_len {
                return Err("autonomous merge bundle entry extends beyond its data file");
            }
            let height = layout
                .base_height
                .checked_add(offset)
                .ok_or("autonomous merge bundle index height overflows")?;
            heights.insert(height);
        }
        if indexed_end != data_len {
            return Err("autonomous merge bundle data has an unindexed suffix");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed during bounded validation");
        }
        Ok((layout, heights))
    }

    /// Read one exact bundle slot from an already bound progress pair.
    ///
    /// Empty sparse-index entries are absence. Every non-empty entry must be
    /// bounded, canonical, self-validating, and identify its exact lane slot;
    /// malformed bytes are never treated as a repairable miss.
    fn read_autonomous_lane_merge_bundle_from_bound_locked(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<Option<(AutonomousLaneMergeBundleV1, Vec<u8>)>, &'static str> {
        let (layout, populated_heights) =
            self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)?;
        if !populated_heights.contains(&lane_block_height) {
            return Ok(None);
        }
        let Some(entry_position) = layout.entry_position(lane_block_height) else {
            return Ok(None);
        };
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        bound
            .index
            .seek(SeekFrom::Start(entry_position))
            .and_then(|_| bound.index.read_exact(&mut entry_bytes))
            .map_err(|_| "autonomous merge bundle index entry is unreadable")?;
        let entry = SidecarIndexEntry::from_bytes(entry_bytes);
        if entry.len == 0 {
            return Ok(None);
        }
        if entry.len > u64::try_from(MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES).unwrap_or(u64::MAX) {
            return Err("autonomous merge bundle entry exceeds its hard byte limit");
        }
        let payload_len = usize::try_from(entry.len)
            .map_err(|_| "autonomous merge bundle entry length is not representable")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(payload_len)
            .map_err(|_| "autonomous merge bundle allocation exceeds process limits")?;
        bytes.resize(payload_len, 0);
        bound
            .data
            .seek(SeekFrom::Start(entry.offset))
            .and_then(|_| bound.data.read_exact(&mut bytes))
            .map_err(|_| "autonomous merge bundle entry is unreadable")?;
        let bundle = norito::decode_canonical::<AutonomousLaneMergeBundleV1>(&bytes)
            .map_err(|_| "autonomous merge bundle entry is not canonical framed Norito")?;
        let payload = bundle.executable_payload();
        Self::validate_autonomous_lane_merge_bundle(&bundle, payload.chain_id_hash, payload.epoch)
            .map_err(|_| "autonomous merge bundle entry is invalid")?;
        let descriptor = &bundle.certified.proposal.descriptor;
        if descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height {
            return Err("autonomous merge bundle entry names another lane slot");
        }
        if bundle
            .encode_framed()
            .map_err(|_| "autonomous merge bundle entry cannot be re-encoded")?
            != bytes
        {
            return Err("autonomous merge bundle entry is not canonically stable");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed during exact lookup");
        }
        Ok(Some((bundle, bytes)))
    }

    #[allow(clippy::too_many_lines)]
    fn durable_autonomous_lane_merge_source_under_prune_guard(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_chain_id_hash: Hash,
        expected_epoch: u64,
        certified_override: Option<&CertifiedLaneBlockArtifact>,
        require_persisted_bundle: bool,
    ) -> std::result::Result<DurableAutonomousLaneMergeSource, &'static str> {
        if self.prune_recovery_is_required() {
            return Err("Kura prune recovery blocks autonomous merge-source admission");
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self
            .lane_storage_entry(lane_id)
            .map_err(|_| "autonomous merge source has no active lane storage")?;
        let _sidecar_guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return Err("Kura prune recovery blocks autonomous merge-source admission");
        }

        let autonomous_record = self
            .read_autonomous_lane_block_record_locked(
                &entry,
                lane_id,
                lane_block_height,
                expected_chain_id_hash,
                expected_epoch,
                false,
            )
            .map_err(|_| "autonomous merge payload failed repair-disabled readback")?
            .ok_or("autonomous lane merge payload is unavailable")?;
        if autonomous_record.retirement.is_some() {
            return Err("retired autonomous lane slot is not merge eligible");
        }
        let view_state_path = &autonomous_record.view_state_path;
        let view_state_parent = view_state_path
            .parent()
            .ok_or("autonomous lane view state has no parent directory")?;
        let view_state_temp = Self::autonomous_lane_block_view_state_temp_path(view_state_path);
        if self
            .regular_sidecar_metadata(&view_state_temp, view_state_parent)
            .map_err(|_| "autonomous lane view recovery artifact is invalid")?
            .is_some()
        {
            return Err("autonomous lane view state has unresolved recovery state");
        }
        let autonomous = autonomous_record.artifact;

        let certified = if let Some(certified) = certified_override {
            self.require_active_lane_artifact(&entry, &certified.proposal.descriptor)
                .map_err(|_| "autonomous merge certificate targets stale lane geometry")?;
            certified.clone()
        } else {
            let (data_path, index_path) =
                Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
            let namespace = self
                .open_bound_progress_namespace(&data_path, &index_path)
                .map_err(|_| "certified lane block pair could not be bound")?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &namespace,
                &data_path,
                &index_path,
                "certified lane block pair",
            )
            .map_err(|_| "certified lane block pair has unresolved recovery state")?;
            let mut pair = self
                .open_bound_progress_pair(&data_path, &index_path)
                .map_err(|_| "certified lane block pair could not be opened")?;
            let certified = match &mut pair {
                BoundProgressPair::Absent(_) => None,
                BoundProgressPair::Present(bound) => {
                    self.bound_indexed_sidecar_height_range(bound, "certified lane block")
                        .map_err(|_| "certified lane block pair has a malformed index")?;
                    self.read_active_certified_lane_block_artifact_from_bound_locked(
                        &entry,
                        lane_block_height,
                        bound,
                    )
                }
            }
            .ok_or("certified lane block pair lacks the exact autonomous slot")?;
            if let BoundProgressPair::Present(bound) = &pair
                && !self.bound_progress_sidecar_unchanged(bound)
            {
                return Err("certified lane block pair changed during bundle admission");
            }
            certified
        };
        let frontier_read = self
            .read_latest_certified_lane_block_frontier_structural_locked(&entry, false)
            .map_err(|_| "latest certified frontier failed repair-disabled readback")?;
        if certified_override.is_none() && frontier_read.is_none() {
            return Err("certified lane block pair lacks its mandatory durable frontier");
        }
        if let Some(frontier_read) = frontier_read {
            let frontier_artifact = &frontier_read.frontier.artifact;
            let frontier_descriptor = &frontier_artifact.proposal.descriptor;
            self.require_active_lane_artifact(&entry, frontier_descriptor)
                .map_err(|_| "latest certified frontier targets stale lane geometry")?;
            if frontier_descriptor.lane_block_height < lane_block_height {
                return Err("certified lane block pair is ahead of its durable frontier");
            }
            if frontier_descriptor.lane_block_height == lane_block_height
                && frontier_artifact != &certified
            {
                return Err("certified lane block pair conflicts with its exact frontier");
            }
            self.confirm_latest_certified_lane_block_frontier_read_locked(
                &entry,
                &frontier_read.snapshot,
            )
            .map_err(|_| "latest certified frontier changed during bundle admission")?;
        }

        let (input_data_path, input_index_path) =
            Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
        let input_namespace = self
            .open_bound_progress_namespace(&input_data_path, &input_index_path)
            .map_err(|_| "lane execution input pair could not be bound")?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &input_namespace,
            &input_data_path,
            &input_index_path,
            "lane block execution input",
        )
        .map_err(|_| "lane execution input pair has unresolved recovery state")?;
        let mut input_pair = self
            .open_bound_progress_pair(&input_data_path, &input_index_path)
            .map_err(|_| "lane execution input pair could not be opened")?;
        let input = match &mut input_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => {
                self.bound_indexed_sidecar_height_range(bound, "lane block execution input")
                    .map_err(|_| "lane execution input pair has a malformed index")?;
                Self::read_indexed_sidecar_from_open_files(
                    lane_block_height,
                    &mut bound.data,
                    &mut bound.index,
                    &bound.namespace.data_path,
                    &bound.namespace.index_path,
                    norito::decode_canonical::<LaneBlockExecutionInputArtifact>,
                    "lane block execution input",
                )
            }
        }
        .ok_or("durable autonomous execution input is unavailable")?;
        if let BoundProgressPair::Present(bound) = &input_pair
            && !self.bound_progress_sidecar_unchanged(bound)
        {
            return Err("lane execution input pair changed during bundle admission");
        }
        Self::validate_lane_block_execution_input_artifact(&input)
            .map_err(|_| "durable autonomous execution input is invalid")?;
        self.require_active_lane_artifact(&entry, &input.proposal.descriptor)
            .map_err(|_| "autonomous execution input targets stale lane geometry")?;

        let bundle = AutonomousLaneMergeBundleV1 {
            version: AutonomousLaneMergeBundleV1::VERSION,
            autonomous,
            certified,
        };
        Self::validate_autonomous_lane_merge_bundle(
            &bundle,
            expected_chain_id_hash,
            expected_epoch,
        )?;
        let expected_input = Self::autonomous_lane_block_execution_input_candidate(
            bundle.executable_payload(),
            expected_chain_id_hash,
            expected_epoch,
        )
        .map_err(|_| "autonomous payload cannot reconstruct its canonical execution input")?;
        if input != expected_input {
            return Err("durable execution input differs from the certified autonomous payload");
        }
        let source_bundle = bundle
            .encode_framed()
            .map_err(|_| "autonomous merge bundle cannot be canonically encoded")?;
        if require_persisted_bundle {
            let (bundle_data_path, bundle_index_path) =
                Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root);
            let bundle_namespace = self
                .open_bound_progress_namespace(&bundle_data_path, &bundle_index_path)
                .map_err(|_| "autonomous merge bundle pair could not be bound")?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &bundle_namespace,
                &bundle_data_path,
                &bundle_index_path,
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            )
            .map_err(|_| "autonomous merge bundle pair has unresolved recovery state")?;
            let mut bundle_pair = self
                .open_bound_progress_pair(&bundle_data_path, &bundle_index_path)
                .map_err(|_| "autonomous merge bundle pair could not be opened")?;
            let persisted = match &mut bundle_pair {
                BoundProgressPair::Absent(_) => None,
                BoundProgressPair::Present(bound) => self
                    .read_autonomous_lane_merge_bundle_from_bound_locked(
                        lane_id,
                        lane_block_height,
                        bound,
                    )
                    .map_err(|_| "persisted autonomous merge bundle is malformed")?,
            }
            .ok_or("durable autonomous merge bundle is unavailable")?;
            if let BoundProgressPair::Present(bound) = &bundle_pair
                && !self.bound_progress_sidecar_unchanged(bound)
            {
                return Err("autonomous merge bundle pair changed during source admission");
            }
            Self::validate_autonomous_lane_merge_bundle(
                &persisted.0,
                expected_chain_id_hash,
                expected_epoch,
            )
            .map_err(|_| "persisted autonomous merge bundle is invalid")?;
            if persisted.0 != bundle || persisted.1 != source_bundle {
                return Err(
                    "persisted autonomous merge bundle differs from exact durable components",
                );
            }
        }
        let bundle_hash = bundle
            .bundle_hash()
            .map_err(|_| "autonomous merge bundle cannot be canonically hashed")?;

        // Extract one lossless first-release trace from the exact durable
        // bundle before it becomes merge eligible. The bitmap projection uses
        // the certificate's canonical committee order; no proposer-local or
        // current-topology state participates.
        let descriptor = &bundle.certified.proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "autonomous merge committee exceeds the refinement width")?;
        if validator_count == 0 || validator_count > 128 {
            return Err("autonomous merge committee is outside the 1..=128 refinement width");
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &bundle.executable_payload().producer)
            .ok_or("autonomous payload producer is absent from its certified committee")?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index)
                    .map_err(|_| "autonomous producer index exceeds the refinement width")?,
            )
            .ok_or("autonomous producer index exceeds the refinement width")?;
        let bitmap_mask = |bitmap: &[u8]| {
            if bitmap.len() != descriptor.validator_set.len().div_ceil(8) {
                return Err("autonomous certificate bitmap has a noncanonical length");
            }
            let mut mask = 0_u128;
            for (byte_index, byte) in bitmap.iter().copied().enumerate() {
                for bit_index in 0..8_usize {
                    if byte & (1_u8 << bit_index) == 0 {
                        continue;
                    }
                    let index = byte_index
                        .checked_mul(8)
                        .and_then(|base| base.checked_add(bit_index))
                        .ok_or("autonomous certificate bitmap index overflows")?;
                    if index >= descriptor.validator_set.len() {
                        return Err("autonomous certificate bitmap selects a padding bit");
                    }
                    mask |= 1_u128
                        .checked_shl(u32::try_from(index).map_err(
                            |_| "autonomous certificate signer exceeds the refinement width",
                        )?)
                        .ok_or("autonomous certificate signer exceeds the refinement width")?;
                }
            }
            Ok(mask)
        };
        let availability_qc = bundle
            .certified
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .ok_or("autonomous prepare QC lacks its durable READY certificate")?;
        if availability_qc.validator_set != descriptor.validator_set {
            return Err("autonomous READY committee differs from the lane certificate");
        }
        let ready_signers = bitmap_mask(&availability_qc.signers_bitmap)?;
        let commit_signers = bitmap_mask(&bundle.certified.commit_qc.signers_bitmap)?;
        let lane_commit_candidates = ready_signers & commit_signers;
        if lane_commit_candidates == 0 {
            return Err("autonomous READY and Commit QCs have no common authenticated signer");
        }
        let lane_commit_actor = 1_u128
            .checked_shl(lane_commit_candidates.trailing_zeros())
            .ok_or("autonomous lane commit signer exceeds the refinement width")?;
        let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
            bundle.executable_payload().reservation_keys.iter(),
        )
        .map_err(|_| "autonomous reservation group is not canonical")?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err("autonomous reservation count is outside the first-release bound");
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = ready_signers | producer;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous payload ownership exceeds its certified committee");
        }
        let trace_base = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: ready_signers,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: ready_signers,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v4: true,
                ever_reservation_v5: true,
                ever_execution_input_durable: ready_signers,
                ever_ready_authorized: ready_signers,
                ready_signed: ready_signers,
                ever_ready_qc_durable: false,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };

        // The source reader observes an already-durable input. PersistExecutionInput
        // is therefore an idempotent named step for one authenticated READY/Commit
        // witness, while the following two steps extract the durable QC and lane
        // decision carried by the exact same source bytes.
        let mut input_after = trace_base;
        input_after.carrier.execution_input_durable |= lane_commit_actor;
        input_after.history.ever_execution_input_durable |= lane_commit_actor;
        let input_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
            actor: lane_commit_actor,
            target: 0,
            before: trace_base,
            after: input_after,
        };
        let checked_input = check_production_in_flight_first_release_transition(input_projection)
            .ok_or(
            "durable execution input failed the composed first-release transition gate",
        )?;
        if checked_input.into_projection() != input_projection {
            return Err("checked durable execution-input projection changed before admission");
        }

        let mut ready_qc_after = input_after;
        ready_qc_after.carrier.ready_qc_durable = true;
        ready_qc_after.history.ever_ready_qc_durable = true;
        let ready_qc_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
            actor: 0,
            target: 0,
            before: input_after,
            after: ready_qc_after,
        };
        let checked_ready_qc =
            check_production_in_flight_first_release_transition(ready_qc_projection)
                .ok_or("durable READY QC failed the composed first-release transition gate")?;
        if checked_ready_qc.into_projection() != ready_qc_projection {
            return Err("checked durable READY-QC projection changed before admission");
        }

        let mut lane_commit_after = ready_qc_after;
        lane_commit_after.decision.lane_commit_scope = binding_a;
        lane_commit_after.decision.lane_commit_owner = lane_commit_actor;
        let lane_commit_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
            actor: lane_commit_actor,
            target: 0,
            before: ready_qc_after,
            after: lane_commit_after,
        };
        let checked_lane_commit =
            check_production_in_flight_first_release_transition(lane_commit_projection)
                .ok_or("lane CommitQC failed the composed first-release transition gate")?;
        if checked_lane_commit.into_projection() != lane_commit_projection {
            return Err("checked lane-commit projection changed before merge admission");
        }
        Ok(DurableAutonomousLaneMergeSource {
            bundle,
            source_bundle,
            bundle_hash,
            input,
        })
    }

    /// Revalidate the exact independently durable source admitted to merge.
    ///
    /// This read never repairs an execution-input or certified data/index
    /// pair. Startup recovery must complete those barriers explicitly before
    /// merge readiness can become visible.
    pub(crate) fn durable_autonomous_lane_merge_source(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_chain_id_hash: Hash,
        expected_epoch: u64,
    ) -> std::result::Result<DurableAutonomousLaneMergeSource, &'static str> {
        let _prune_guard = self.prune_lock.lock();
        self.durable_autonomous_lane_merge_source_under_prune_guard(
            lane_id,
            lane_block_height,
            expected_chain_id_hash,
            expected_epoch,
            None,
            true,
        )
    }

    /// Publish one exact canonical autonomous bundle through an independent
    /// strict data/index/directory durability barrier.
    ///
    /// The caller holds `prune_lock`; the source was assembled from the exact
    /// component set protected by that guard. Conflicting active-slot bytes are
    /// immutable corruption and are never overwritten.
    fn persist_autonomous_lane_merge_bundle_under_prune_guard(
        &self,
        source: &DurableAutonomousLaneMergeSource,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        let descriptor = &source.bundle.certified.proposal.descriptor;
        Self::validate_autonomous_lane_merge_bundle(
            &source.bundle,
            source.bundle.executable_payload().chain_id_hash,
            source.bundle.executable_payload().epoch,
        )
        .map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
        })?;
        if source.bundle.encode_framed()? != source.source_bundle
            || source.bundle.bundle_hash()? != source.bundle_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous merge source bytes or hash differ from its canonical bundle",
            ));
        }

        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let (data_path, index_path) =
            Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root);
        let directory = data_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "autonomous merge bundle path has no parent directory",
            )
        })?;
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::MkDir(error, directory.clone()))?;
        let _sidecar_guard = self.sidecar_lock.lock();
        if !self.recover_bound_progress_sidecar_artifacts(
            &data_path,
            &index_path,
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
        ) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle pair recovery did not reach a durable fixed point",
            ));
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        let mut existing_pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let existing_layout = match &mut existing_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => Some(
                self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                    })?
                    .0,
            ),
        };
        if let BoundProgressPair::Present(bound) = &mut existing_pair
            && let Some((existing, existing_bytes)) = self
                .read_autonomous_lane_merge_bundle_from_bound_locked(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    bound,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                })?
        {
            if existing != source.bundle || existing_bytes != source.source_bundle {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "active autonomous merge bundle slot contains conflicting canonical bytes",
                ));
            }
            if !self.sync_bound_progress_sidecar(bound, AutonomousLaneMergeBundleV1::FORMAT_LABEL) {
                return Err(Error::IO(
                    std::io::Error::other(
                        "failed to make existing autonomous merge bundle durable",
                    ),
                    data_path,
                ));
            }
            return Ok(());
        }
        drop(existing_pair);

        let projected_entry_count = match existing_layout {
            None | Some(SidecarIndexLayout { entry_count: 0, .. }) => 1_u64,
            Some(layout) if descriptor.lane_block_height < layout.base_height => layout
                .entry_count
                .checked_add(layout.base_height - descriptor.lane_block_height)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.clone(),
                        "autonomous merge bundle index growth overflows",
                    )
                })?,
            Some(layout) => layout.entry_count.max(
                descriptor
                    .lane_block_height
                    .checked_sub(layout.base_height)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            index_path.clone(),
                            "autonomous merge bundle index height overflows",
                        )
                    })?,
            ),
        };
        if usize::try_from(projected_entry_count).unwrap_or(usize::MAX)
            > self.autonomous_lane_merge_bundle_pair_entry_limit()
        {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "autonomous merge bundle index would exceed its bounded entry count",
            ));
        }
        let projected_data_len = Self::file_len_or_zero(&data_path)?
            .checked_add(u64::try_from(source.source_bundle.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "autonomous merge bundle data growth overflows",
                )
            })?;
        if projected_data_len
            > u64::try_from(self.autonomous_lane_merge_bundle_pair_byte_limit()).unwrap_or(u64::MAX)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle data would exceed its aggregate byte budget",
            ));
        }

        #[cfg(test)]
        if FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_PERSISTENCE.with(|flag| flag.replace(false)) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "injected autonomous merge bundle publication failure",
            ));
        }

        let before_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        if !Self::append_indexed_progress_sidecar(
            &data_path,
            &index_path,
            descriptor.lane_block_height,
            &source.source_bundle,
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            None,
            SidecarIndexOrigin::FirstWrite,
            &namespace,
        ) {
            return Err(Error::IO(
                std::io::Error::other("failed to persist autonomous merge bundle"),
                data_path,
            ));
        }
        let mut readback_pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let readback = match &mut readback_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => self
                .read_autonomous_lane_merge_bundle_from_bound_locked(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    bound,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                })?,
        }
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "autonomous merge bundle disappeared after strict publication",
            )
        })?;
        if readback.0 != source.bundle || readback.1 != source.source_bundle {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle changed before durable readback",
            ));
        }
        let after_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        accounting_mutation.finish();
        self.note_committed_lane_status_change();
        Ok(())
    }
    /// Reconcile independently durable autonomous merge bundles with the
    /// exact active certified slots that authorize them.
    ///
    /// A crash may publish the certified frontier/pair and stop before the
    /// bundle pair crosses its own data/index/directory barrier. Startup is
    /// the only repair path: it reconstructs such a missing slot from the
    /// authenticated autonomous payload, certificate, and execution input.
    /// Existing conflicting or orphan bundle bytes always fail closed.
    fn repair_autonomous_lane_merge_bundles_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let entries = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            self.lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };

        for entry in entries {
            let (certified, persisted_bundles) = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let active_entry = self.lane_storage_entry(entry.lane_id)?;
                if active_entry != entry {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "lane geometry changed during autonomous merge bundle startup repair",
                    ));
                }
                let _sidecar_guard = self.sidecar_lock.lock();
                self.ensure_prune_recovery_not_required()?;

                let frontier =
                    self.read_latest_certified_lane_block_frontier_locked(&active_entry, true)?;
                if let Some(frontier) = frontier.as_ref() {
                    self.recover_certified_lane_block_pair_from_frontier_locked(
                        &active_entry,
                        &frontier.frontier.artifact,
                        None,
                    )?;
                    self.confirm_latest_certified_lane_block_frontier_read_locked(
                        &active_entry,
                        &frontier.snapshot,
                    )?;
                }

                let (certified_data_path, certified_index_path) =
                    Self::certified_lane_block_paths_for_entry(&active_entry, &self.store_root);
                if !self.recover_bound_progress_sidecar_artifacts(
                    &certified_data_path,
                    &certified_index_path,
                    CertifiedLaneBlockArtifact::FORMAT_LABEL,
                ) {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_data_path,
                        "certified lane block pair failed startup recovery before bundle repair",
                    ));
                }
                let mut certified_pair =
                    self.open_bound_progress_pair(&certified_data_path, &certified_index_path)?;
                let certified = match &mut certified_pair {
                    BoundProgressPair::Absent(namespace) => {
                        if !self.sync_bound_progress_absence(
                            namespace,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_data_path.clone(),
                                "certified lane block absence is not durable during bundle repair",
                            ));
                        }
                        BTreeMap::new()
                    }
                    BoundProgressPair::Present(bound) => {
                        let heights = self.bound_indexed_sidecar_payload_heights(
                            bound,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
                        )?;
                        let mut artifacts = BTreeMap::new();
                        for lane_block_height in heights {
                            let artifact = self
                                .read_active_certified_lane_block_artifact_from_bound_locked(
                                    &active_entry,
                                    lane_block_height,
                                    bound,
                                )
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        certified_data_path.clone(),
                                        "certified lane block slot is malformed during autonomous bundle repair",
                                    )
                                })?;
                            artifacts.insert(lane_block_height, artifact);
                        }
                        if !self.sync_bound_progress_sidecar(
                            bound,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_data_path.clone(),
                                "certified lane block pair is not durable during bundle repair",
                            ));
                        }
                        artifacts
                    }
                };
                if frontier.is_none() && !certified.is_empty() {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_data_path,
                        "certified lane block history exists without its mandatory durable frontier",
                    ));
                }

                let (bundle_data_path, bundle_index_path) =
                    Self::autonomous_lane_merge_bundle_paths_for_entry(
                        &active_entry,
                        &self.store_root,
                    );
                if !self.recover_bound_progress_sidecar_artifacts(
                    &bundle_data_path,
                    &bundle_index_path,
                    AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                ) {
                    return Err(Self::invalid_lane_artifact_error(
                        bundle_data_path,
                        "autonomous merge bundle pair failed startup recovery",
                    ));
                }
                let mut bundle_pair =
                    self.open_bound_progress_pair(&bundle_data_path, &bundle_index_path)?;
                let bundles = match &mut bundle_pair {
                    BoundProgressPair::Absent(namespace) => {
                        if !self.sync_bound_progress_absence(
                            namespace,
                            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                "autonomous merge bundle absence is not durable during startup repair",
                            ));
                        }
                        BTreeMap::new()
                    }
                    BoundProgressPair::Present(bound) => {
                        let (_, heights) = self
                            .validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                            .map_err(|message| {
                                Self::invalid_lane_artifact_error(
                                    bundle_data_path.clone(),
                                    message.to_owned(),
                                )
                            })?;
                        let mut bundles = BTreeMap::new();
                        for lane_block_height in heights {
                            let (bundle, _) = self
                                .read_autonomous_lane_merge_bundle_from_bound_locked(
                                    active_entry.lane_id,
                                    lane_block_height,
                                    bound,
                                )
                                .map_err(|message| {
                                    Self::invalid_lane_artifact_error(
                                        bundle_data_path.clone(),
                                        message.to_owned(),
                                    )
                                })?
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        bundle_data_path.clone(),
                                        "enumerated autonomous merge bundle slot disappeared",
                                    )
                                })?;
                            self.require_active_lane_artifact(
                                &active_entry,
                                &bundle.certified.proposal.descriptor,
                            )?;
                            bundles.insert(lane_block_height, bundle);
                        }
                        if !self.sync_bound_progress_sidecar(
                            bound,
                            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                "autonomous merge bundle pair is not durable during startup repair",
                            ));
                        }
                        bundles
                    }
                };
                (certified, bundles)
            };

            for lane_block_height in persisted_bundles.keys() {
                let Some(artifact) = certified.get(lane_block_height) else {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous merge bundle exists without its exact certified lane slot",
                    ));
                };
                if artifact.prepare_qc.payload_availability_qc.is_none() {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous merge bundle exists for a non-autonomous certificate",
                    ));
                }
            }

            for (lane_block_height, artifact) in certified {
                let Some(availability) = artifact.prepare_qc.payload_availability_qc.as_ref()
                else {
                    continue;
                };
                if let Some(bundle) = persisted_bundles.get(&lane_block_height) {
                    let input = self
                        .read_active_lane_block_execution_input_structural(
                            entry.lane_id,
                            lane_block_height,
                            false,
                        )
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                self.store_root.clone(),
                                "persisted autonomous merge bundle has no exact durable execution input",
                            )
                        })?;
                    let expected_input =
                        Self::autonomous_lane_block_execution_input_candidate(
                            bundle.executable_payload(),
                            availability.body.chain_id_hash,
                            availability.body.epoch,
                        )
                        .map_err(|_| {
                            Self::invalid_lane_artifact_error(
                                self.store_root.clone(),
                                "persisted autonomous merge bundle cannot reconstruct its execution input",
                            )
                        })?;
                    if bundle.certified != artifact || input != expected_input {
                        return Err(Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "persisted autonomous merge bundle differs from its certified slot or execution input",
                        ));
                    }
                    continue;
                }
                let source = self
                    .durable_autonomous_lane_merge_source_under_prune_guard(
                        entry.lane_id,
                        lane_block_height,
                        availability.body.chain_id_hash,
                        availability.body.epoch,
                        None,
                        false,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            format!(
                                "autonomous merge bundle startup reconstruction failed: {message}"
                            ),
                        )
                    })?;
                if source.bundle.certified != artifact {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup autonomous merge source retained another certificate",
                    ));
                }
                self.persist_autonomous_lane_merge_bundle_under_prune_guard(&source)?;
                let published = self
                    .durable_autonomous_lane_merge_source_under_prune_guard(
                        entry.lane_id,
                        lane_block_height,
                        availability.body.chain_id_hash,
                        availability.body.epoch,
                        None,
                        true,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            format!("autonomous merge bundle startup readback failed: {message}"),
                        )
                    })?;
                if published != source {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup autonomous merge bundle changed during durable publication",
                    ));
                }
            }
        }
        Ok(())
    }
    /// Authenticate the exact durable execution-input readback and mint one
    /// move-only autonomous READY signing authority.
    ///
    /// This read is deliberately repair-disabled: a missing input must cross
    /// the normal persistence barriers before READY authorization. The
    /// returned value binds the complete canonical artifact, ordered
    /// reservation group, proposal/payload identity, signer, and historical or
    /// current height-context session.
    pub(crate) fn mint_lane_ready_authorization(
        &self,
        payload: &LaneExecutablePayloadV1,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> std::result::Result<LaneReadyAuthorization, &'static str> {
        let descriptor = &proposal.descriptor;
        if payload.origin_proposal != *proposal {
            return Err("READY payload does not name the exact proposal");
        }
        let expected_availability =
            lane_payload_availability_body(payload, proposal, payload.chain_id_hash, payload.epoch)
                .map_err(|_| "READY payload or proposal is invalid")?;
        if expected_availability != *availability_body {
            return Err("READY body differs from the exact payload and proposal");
        }
        if !descriptor.validator_set.contains(signer)
            || signer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return Err("READY signer is not a BLS-normal member of the exact committee");
        }
        let context_suffix = format!(
            "::height-context:{}::epoch:{}::lane-relay:v1:{}:{}",
            hex::encode(height_context_id.0.as_ref()),
            payload.epoch,
            descriptor.dataspace_id.as_u64(),
            descriptor.lane_id.as_u32(),
        );
        if !availability_body.qc_mode_tag.ends_with(&context_suffix) {
            return Err("READY height-context session differs from the proposal");
        }

        let durable = self
            .read_lane_block_execution_input_with_repair_policy(
                descriptor.lane_id,
                descriptor.lane_block_height,
                false,
            )
            .ok_or("READY execution input is not durably readable")?;
        if durable.proposal != *proposal {
            return Err("READY execution input names another proposal or incarnation");
        }
        if durable.autonomous_chain_id_hash != Some(payload.chain_id_hash)
            || durable.autonomous_epoch != Some(payload.epoch)
            || durable.autonomous_payload_hash != Some(payload.payload_hash)
            || durable.entrypoint_hashes != payload.entrypoint_hashes
            || durable.entrypoints != payload.entrypoints
            || durable.reservation_keys != payload.reservation_keys
            || durable.routing_plans != payload.routing_plans
            || durable.native_amx_receipts != payload.native_amx_receipts
        {
            return Err("READY execution input differs from the executable payload");
        }
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(durable.reservation_keys.iter())
                .map_err(|_| "READY execution input has an invalid reservation group")?;
        let (reservation_owner_hash, proposal_identity_hash) =
            autonomous_lane_reservation_identity_hashes_for_proposal(
                payload.chain_id_hash,
                height_context_id,
                payload.epoch,
                proposal,
                &payload.producer,
            )
            .map_err(|_| "READY reservation group has an invalid signer session")?;
        if reservation_group.identity.lane_id != descriptor.lane_id
            || reservation_group.identity.dataspace_id != descriptor.dataspace_id
            || reservation_group.identity.lane_incarnation != descriptor.lane_incarnation
            || reservation_group.identity.proposal_height != descriptor.proposal_height
            || reservation_group.identity.lane_block_height != descriptor.lane_block_height
            || reservation_group.identity.lane_block_view != descriptor.lane_block_view
            || reservation_group.identity.reservation_owner_hash != reservation_owner_hash
            || reservation_group.identity.proposal_identity_hash != proposal_identity_hash
            || reservation_group.reservation_count
                != u64::try_from(payload.entrypoints.len()).unwrap_or(u64::MAX)
        {
            return Err("READY reservation group differs from the proposal session");
        }
        let durable_bytes = norito::encode_canonical(&durable)
            .map_err(|_| "READY execution input cannot be canonically hashed")?;
        let durable_execution_input_hash = Hash::new_from_chunks(&[
            LANE_READY_EXECUTION_INPUT_AUTHORIZATION_DOMAIN_V1,
            durable_bytes.as_slice(),
        ]);

        // Project the exact committee positions named by the authenticated
        // payload and signing request. The checked token is consumed only
        // after the repair-disabled durable input and full reservation group
        // have both been revalidated.
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "READY committee exceeds the refinement width")?;
        if validator_count == 0 || validator_count > 128 {
            return Err("READY committee is outside the 1..=128 refinement width");
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .ok_or("READY payload producer is absent from its committee")?;
        let signer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == signer)
            .ok_or("READY signer is absent from its committee")?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index)
                    .map_err(|_| "READY producer index exceeds the refinement width")?,
            )
            .ok_or("READY producer index exceeds the refinement width")?;
        let actor = 1_u128
            .checked_shl(
                u32::try_from(signer_index)
                    .map_err(|_| "READY signer index exceeds the refinement width")?,
            )
            .ok_or("READY signer index exceeds the refinement width")?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err("READY reservation count is outside the first-release bound");
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = producer | actor;
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: actor,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: 0,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v4: true,
                ever_reservation_v5: true,
                ever_execution_input_durable: actor,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.session.ready_authorized = actor;
        after.history.ever_ready_authorized = actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY,
            actor,
            target: 0,
            before,
            after,
        };
        let checked = check_production_in_flight_first_release_transition(projection)
            .ok_or("READY authorization failed the composed first-release transition gate")?;
        if checked.into_projection() != projection {
            return Err("checked READY authorization projection changed before minting");
        }
        Ok(LaneReadyAuthorization {
            durable_execution_input_hash,
            proposal: proposal.clone(),
            availability_body: availability_body.clone(),
            reservation_group,
            producer: payload.producer.clone(),
            signer: signer.clone(),
            height_context_id,
        })
    }
}
