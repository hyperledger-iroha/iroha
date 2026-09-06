// Canonical-carrier-derived autonomous merge replicas.

const CANONICAL_AUTONOMOUS_LANE_REPLICA_VERSION_V1: u16 = 1;
const CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL: &str = "lane.canonical_autonomous_replica.v1";
const CANONICAL_AUTONOMOUS_LANE_REPLICA_HASH_DOMAIN_V1: &[u8] =
    b"iroha:kura:canonical-autonomous-lane-replica:v1\0";
const MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_BYTES: usize =
    MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES * 2 + 64 * 1024;
pub(crate) const MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_MATCH_RESULTS: usize = 129;
const MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_MATCH_SCAN: usize = 1_032;

/// One crash-atomic, non-owning copy of all evidence needed to execute a
/// globally finalized autonomous lane payload.
///
/// This record deliberately lives outside the producer/committee artifact
/// pairs. In particular, its presence never implies a Queue reservation, a
/// READY signer, a lifecycle cursor, or authority to advance/retire an
/// autonomous attempt. The certified bundle embeds the exact historical
/// committee roster, the READY certificate's complete PoP vector, and the
/// Prepare/Commit signer PoPs needed for aggregate verification after restart.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct CanonicalAutonomousLaneReplicaV1 {
    version: u16,
    carrier_height: u64,
    carrier_block_hash: HashOf<BlockHeader>,
    carrier_finality_hash: HashOf<V2FinalityArtifact>,
    carrier_execution_commitment: ExecutionCommitment,
    bundle: AutonomousLaneMergeBundleV1,
    input: LaneBlockExecutionInputArtifact,
}

impl CanonicalAutonomousLaneReplicaV1 {
    fn encode_framed(&self) -> Result<Vec<u8>> {
        let bytes = norito::encode_canonical(self).map_err(Error::NoritoFrame)?;
        if bytes.len() > MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_BYTES {
            return Err(Error::NoritoFrame(norito::Error::Message(
                "canonical autonomous lane replica exceeds its hard byte limit".to_owned(),
            )));
        }
        Ok(bytes)
    }

    fn replica_hash(&self) -> Result<Hash> {
        let bytes = self.encode_framed()?;
        Ok(Hash::new_from_chunks(&[
            CANONICAL_AUTONOMOUS_LANE_REPLICA_HASH_DOMAIN_V1,
            &bytes,
        ]))
    }
}

impl Kura {
    fn canonical_autonomous_lane_replica_paths_for_entry(
        entry: &LaneConfigEntry,
        store_root: &Path,
    ) -> (PathBuf, PathBuf) {
        let directory = Self::lane_artifact_dir(&entry.blocks_dir(store_root));
        (
            directory.join(CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE),
            directory.join(CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE),
        )
    }

    fn canonical_autonomous_lane_replica_pair_entry_limit(&self) -> usize {
        self.lane_history_retention
            .get()
            .saturating_add(usize::try_from(MAX_INDEXED_SIDECAR_GAP_ENTRIES).unwrap_or(usize::MAX))
            .min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)
    }

    fn canonical_autonomous_lane_replica_pair_byte_limit(&self) -> usize {
        self.pending_control_sidecar_limits.aggregate_bytes
    }

    fn canonical_autonomous_lane_replica_source(
        record: &CanonicalAutonomousLaneReplicaV1,
    ) -> Result<DurableAutonomousLaneMergeSource> {
        let source_bundle = record.bundle.encode_framed()?;
        let bundle_hash = record.bundle.bundle_hash()?;
        Ok(DurableAutonomousLaneMergeSource {
            bundle: record.bundle.clone(),
            source_bundle,
            bundle_hash,
            input: record.input.clone(),
        })
    }

    fn canonical_autonomous_lane_replica_ownership_matches(
        ownership: &SumeragiLanePayloadOwnership,
        descriptor: &LaneBlockDescriptorV1,
        carrier_view: u64,
    ) -> bool {
        ownership.proposal_height == descriptor.proposal_height
            && ownership.proposal_view == carrier_view
            && ownership.lane_id == descriptor.lane_id
            && ownership.dataspace_id == descriptor.dataspace_id
            && ownership.lane_incarnation == descriptor.lane_incarnation
            && ownership.lane_block_height == descriptor.lane_block_height
            && ownership.lane_block_view == descriptor.lane_block_view
            && ownership.subject_hash == descriptor.subject_hash
            && ownership.payload_ownership_hash == descriptor.payload_ownership_hash
            && ownership.rbc_instance_hash == descriptor.rbc_instance_hash
            && ownership.accepted_candidate_indices == descriptor.accepted_candidate_indices
            && ownership.accepted_transaction_hashes == descriptor.accepted_transaction_hashes
            && ownership.previous_lane_block_height == descriptor.previous_lane_block_height
            && ownership.previous_lane_block_descriptor_hash
                == descriptor.previous_lane_block_descriptor_hash
            && ownership.lane_block_descriptor_hash == Some(descriptor.descriptor_hash)
            && ownership.lane_block_descriptor_validator_set == descriptor.validator_set
            && ownership.lane_block_descriptor_validator_count == descriptor.validator_count
            && ownership.lane_block_descriptor_min_quorum == descriptor.min_quorum
            && ownership.qc_mode_tag == descriptor.qc_mode_tag
            && ownership.validate_replay_material().is_ok()
    }

    fn validate_canonical_autonomous_lane_replica_structure(
        record: &CanonicalAutonomousLaneReplicaV1,
    ) -> std::result::Result<(), &'static str> {
        if record.version != CANONICAL_AUTONOMOUS_LANE_REPLICA_VERSION_V1
            || record.carrier_height == 0
        {
            return Err("canonical autonomous lane replica has an invalid version or height");
        }
        record
            .encode_framed()
            .map_err(|_| "canonical autonomous lane replica exceeds its hard byte limit")?;
        let payload = record.bundle.executable_payload();
        Self::validate_autonomous_lane_merge_bundle(
            &record.bundle,
            payload.network_id,
            payload.epoch,
        )?;
        Self::validate_lane_block_execution_input_artifact(&record.input)?;
        let hint = payload
            .origin_proposal
            .payload_block_hint
            .ok_or("canonical autonomous lane replica payload has no carrier hint")?;
        if hint.proposal_height != record.carrier_height
            || hint.proposal_block_hash != record.carrier_block_hash
            || record.bundle.certified.proposal != payload.origin_proposal
        {
            return Err(
                "canonical autonomous lane replica identity differs from its carrier or certificate",
            );
        }
        let expected_input = Self::autonomous_lane_block_execution_input_candidate(
            payload,
            payload.network_id,
            payload.epoch,
        )
        .map_err(|_| "canonical autonomous lane replica payload cannot reconstruct its input")?;
        if record.input != expected_input {
            return Err(
                "canonical autonomous lane replica input differs from its authenticated payload",
            );
        }
        Ok(())
    }

    /// Return whether two READY certificates authenticate the same immutable
    /// availability subject while differing only in quorum proof bytes.
    fn canonical_autonomous_lane_replica_ready_qcs_certify_same_subject(
        left: Option<&iroha_data_model::block::consensus::LanePayloadAvailabilityQcV1>,
        right: Option<&iroha_data_model::block::consensus::LanePayloadAvailabilityQcV1>,
    ) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.body == right.body
                    && left.validator_set_hash_version == right.validator_set_hash_version
                    && left.validator_set_hash == right.validator_set_hash
                    && left.validator_set == right.validator_set
                    && left.validator_set_pops == right.validator_set_pops
            }
            (None, Some(_)) | (Some(_), None) => false,
        }
    }

    /// Return whether two lane QCs certify one Prepare/Commit decision while
    /// differing only in signer selection and aggregate signature bytes.
    fn canonical_autonomous_lane_replica_qcs_certify_same_decision(
        left: &LaneBlockQcV1,
        right: &LaneBlockQcV1,
    ) -> bool {
        left.body == right.body
            && left.validator_set_hash_version == right.validator_set_hash_version
            && left.validator_set_hash == right.validator_set_hash
            && left.validator_set == right.validator_set
            && Self::canonical_autonomous_lane_replica_ready_qcs_certify_same_subject(
                left.payload_availability_qc.as_ref(),
                right.payload_availability_qc.as_ref(),
            )
    }

    /// Return whether two fully validated replicas describe the same durable
    /// carrier, payload, execution input, and lane decision.
    ///
    /// Quorum signer bitmaps, aggregate signatures, and the corresponding
    /// projected signer-PoP maps are deliberately excluded. They are proof
    /// variants for one decision, not part of the replicated lane-block
    /// identity. Callers must validate both records before using this
    /// comparison as an idempotence shortcut.
    fn canonical_autonomous_lane_replicas_certify_same_decision(
        left: &CanonicalAutonomousLaneReplicaV1,
        right: &CanonicalAutonomousLaneReplicaV1,
    ) -> bool {
        left.version == right.version
            && left.carrier_height == right.carrier_height
            && left.carrier_block_hash == right.carrier_block_hash
            && left.carrier_finality_hash == right.carrier_finality_hash
            && left.carrier_execution_commitment == right.carrier_execution_commitment
            && left.input == right.input
            && left.bundle.version == right.bundle.version
            && left.bundle.autonomous.format == right.bundle.autonomous.format
            && left.bundle.autonomous.executable_payload
                == right.bundle.autonomous.executable_payload
            && left.bundle.autonomous.view_checkpoint == right.bundle.autonomous.view_checkpoint
            && left.bundle.autonomous.new_view_certificates
                == right.bundle.autonomous.new_view_certificates
            && left.bundle.certified.format == right.bundle.certified.format
            && left.bundle.certified.proposal == right.bundle.certified.proposal
            && Self::canonical_autonomous_lane_replica_qcs_certify_same_decision(
                &left.bundle.certified.prepare_qc,
                &right.bundle.certified.prepare_qc,
            )
            && Self::canonical_autonomous_lane_replica_qcs_certify_same_decision(
                &left.bundle.certified.commit_qc,
                &right.bundle.certified.commit_qc,
            )
            && match (
                left.bundle.autonomous.availability_certificate.as_ref(),
                right.bundle.autonomous.availability_certificate.as_ref(),
            ) {
                (None, None) => true,
                (Some(left), Some(right)) => {
                    Self::canonical_autonomous_lane_replica_qcs_certify_same_decision(
                        &left.certificate,
                        &right.certificate,
                    )
                }
                (None, Some(_)) | (Some(_), None) => false,
            }
    }

    /// Derive the unique replica record from a fully validated lane
    /// certificate and the exact Kura-canonical, globally finalized carrier.
    ///
    /// The caller holds `prune_lock` and `canonical_chain_lock`, in that order.
    fn canonical_autonomous_lane_replica_from_certified_under_prune_and_canonical_guards(
        &self,
        certified: &CertifiedLaneBlockArtifact,
    ) -> Result<CanonicalAutonomousLaneReplicaV1> {
        Self::validate_certified_lane_block_artifact(certified).map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message)
        })?;
        let availability = certified
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical autonomous replica certificate has no READY aggregate",
                )
            })?;
        let expected_network_id = availability.body.network_id;
        let expected_epoch = availability.body.epoch;
        let descriptor = &certified.proposal.descriptor;
        let carrier_height = descriptor.proposal_height;
        let height = usize::try_from(carrier_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical autonomous replica carrier height is not representable",
                )
            })?;
        let (retained_header, finality, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier_height)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical autonomous replica carrier has no verified durable finality",
                )
            })?;
        let block = self
            .get_block_without_merge_sidecar(height)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical autonomous replica carrier body is not durably readable",
                )
            })?;
        let executed_block_wire_hash = block.executed_block_wire_hash().map_err(|error| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                format!("canonical autonomous replica carrier wire is invalid: {error}"),
            )
        })?;
        let executed_block_wire_len = u64::try_from(block.encode_wire()?.len())?;
        let execution_commitment = finality.commit_qc.execution_commitment;
        if block.header() != retained_header
            || block.header().height().get() != carrier_height
            || block.hash() != finality.block_hash
            || finality.height != carrier_height
            || finality.height_context.height != carrier_height
            || finality.height_context.network_id != expected_network_id
            || finality.height_context.epoch != expected_epoch
            || execution_commitment.executed_block_wire_len != executed_block_wire_len
            || execution_commitment.executed_block_wire_hash != executed_block_wire_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica carrier differs from finality, context, or executed wire",
            ));
        }
        let carrier_hint = iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
            proposal_height: carrier_height,
            proposal_view: block.header().view_change_index(),
            proposal_block_hash: block.hash(),
        };
        if certified
            .proposal
            .payload_block_hint
            .is_some_and(|hint| hint != carrier_hint)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica certificate carries a conflicting global hint",
            ));
        }
        let context = block.execution_context().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica carrier has no execution context",
            )
        })?;
        if block.header().execution_context_hash() != Some(HashOf::new(context)) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica carrier execution context is not header-bound",
            ));
        }
        let ordinary_matches = context
            .lane_payload_ownerships
            .iter()
            .filter(|ownership| {
                Self::canonical_autonomous_lane_replica_ownership_matches(
                    ownership,
                    descriptor,
                    carrier_hint.proposal_view,
                )
            })
            .count();
        if ordinary_matches != 0 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica carrier also anchors the proposal as ordinary work",
            ));
        }
        let mut exact_payload = None;
        for envelope in &context.autonomous_lane_payloads {
            let decoded = decode_autonomous_lane_payload_envelope(
                envelope,
                expected_network_id,
                expected_epoch,
            )
            .and_then(|payload| {
                payload.attach_global_hint_exact(carrier_hint, expected_network_id, expected_epoch)
            })
            .map_err(|error| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    format!(
                        "canonical autonomous replica carrier contains an invalid envelope: {error}"
                    ),
                )
            })?;
            if decoded
                .origin_proposal
                .same_consensus_identity(&certified.proposal)
            {
                if exact_payload.replace(decoded).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "canonical autonomous replica carrier repeats the certified payload",
                    ));
                }
            }
        }
        let payload = exact_payload.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica carrier lacks the certified payload",
            )
        })?;
        let mut canonical_certified = certified.clone();
        canonical_certified.proposal = payload.origin_proposal.clone();
        Self::validate_certified_lane_block_artifact(&canonical_certified).map_err(|message| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                format!("canonicalized autonomous replica certificate is invalid: {message}"),
            )
        })?;
        let availability_certificate = DurableLanePayloadAvailabilityCertificateV1 {
            certificate: canonical_certified.prepare_qc.clone(),
        };
        crate::lane_consensus::validate_lane_payload_availability_certificate(
            &availability_certificate,
            &payload,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|error| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                format!("canonical autonomous replica READY aggregate is invalid: {error}"),
            )
        })?;
        let autonomous = AutonomousLaneBlockArtifact {
            format: AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload.clone(),
            availability_certificate: Some(availability_certificate),
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        };
        let bundle = AutonomousLaneMergeBundleV1 {
            version: AutonomousLaneMergeBundleV1::VERSION,
            autonomous,
            certified: canonical_certified,
        };
        let input = Self::autonomous_lane_block_execution_input_candidate(
            &payload,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|availability| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                format!(
                    "canonical autonomous replica cannot reconstruct execution input: {availability:?}"
                ),
            )
        })?;
        let record = CanonicalAutonomousLaneReplicaV1 {
            version: CANONICAL_AUTONOMOUS_LANE_REPLICA_VERSION_V1,
            carrier_height,
            carrier_block_hash: block.hash(),
            carrier_finality_hash: HashOf::new(&finality),
            carrier_execution_commitment: execution_commitment,
            bundle,
            input,
        };
        Self::validate_canonical_autonomous_lane_replica_structure(&record).map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message)
        })?;
        Ok(record)
    }

    /// Revalidate a structural replica against the exact currently canonical
    /// block and verified finality while the outer prune/canonical corridor is
    /// still held.
    fn validate_canonical_autonomous_lane_replica_against_kura_under_prune_and_canonical_guards(
        &self,
        record: &CanonicalAutonomousLaneReplicaV1,
    ) -> Result<()> {
        Self::validate_canonical_autonomous_lane_replica_structure(record).map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message)
        })?;
        let expected = self
            .canonical_autonomous_lane_replica_from_certified_under_prune_and_canonical_guards(
                &record.bundle.certified,
            )?;
        if expected != *record {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous lane replica differs from current canonical finality",
            ));
        }
        Ok(())
    }

    fn validate_canonical_autonomous_lane_replica_pair_layout_locked(
        &self,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<(SidecarIndexLayout, BTreeSet<u64>), &'static str> {
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("canonical autonomous replica pair changed before validation");
        }
        let index_len = bound
            .index
            .metadata()
            .map_err(|_| "canonical autonomous replica index metadata is unreadable")?
            .len();
        let layout = SidecarIndexLayout::read_from(&mut bound.index, index_len)
            .map_err(|_| "canonical autonomous replica index is malformed")?;
        if layout.aligned_len != index_len {
            return Err("canonical autonomous replica index has trailing or partial bytes");
        }
        if usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
            > self.canonical_autonomous_lane_replica_pair_entry_limit()
        {
            return Err("canonical autonomous replica index exceeds its bounded entry count");
        }
        let data_len = bound
            .data
            .metadata()
            .map_err(|_| "canonical autonomous replica data metadata is unreadable")?
            .len();
        if data_len
            > u64::try_from(self.canonical_autonomous_lane_replica_pair_byte_limit())
                .unwrap_or(u64::MAX)
        {
            return Err("canonical autonomous replica data exceeds its aggregate byte budget");
        }
        bound
            .index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|_| "canonical autonomous replica index entries are unreadable")?;
        let mut heights = BTreeSet::new();
        let mut indexed_end = 0_u64;
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for offset in 0..layout.entry_count {
            bound
                .index
                .read_exact(&mut entry_bytes)
                .map_err(|_| "canonical autonomous replica index entry is unreadable")?;
            let entry = SidecarIndexEntry::from_bytes(entry_bytes);
            if entry.len == 0 {
                if entry.offset != 0 {
                    return Err(
                        "empty canonical autonomous replica index entry has a non-zero offset",
                    );
                }
                continue;
            }
            if entry.len
                > u64::try_from(MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_BYTES).unwrap_or(u64::MAX)
            {
                return Err("canonical autonomous replica entry exceeds its hard byte limit");
            }
            if entry.offset != indexed_end {
                return Err(
                    "canonical autonomous replica data ranges are overlapping, gapped, or reordered",
                );
            }
            indexed_end = entry
                .offset
                .checked_add(entry.len)
                .ok_or("canonical autonomous replica data range overflows")?;
            if indexed_end > data_len {
                return Err("canonical autonomous replica entry extends beyond its data file");
            }
            let height = layout
                .base_height
                .checked_add(offset)
                .ok_or("canonical autonomous replica index height overflows")?;
            heights.insert(height);
        }
        if indexed_end != data_len {
            return Err("canonical autonomous replica data has an unindexed suffix");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("canonical autonomous replica pair changed during validation");
        }
        Ok((layout, heights))
    }

    fn read_canonical_autonomous_lane_replica_from_bound_locked(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<Option<(CanonicalAutonomousLaneReplicaV1, Vec<u8>)>, &'static str>
    {
        let (layout, populated_heights) =
            self.validate_canonical_autonomous_lane_replica_pair_layout_locked(bound)?;
        if !populated_heights.contains(&lane_block_height) {
            return Ok(None);
        }
        let entry_position = layout
            .entry_position(lane_block_height)
            .ok_or("canonical autonomous replica index lost a populated height")?;
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        bound
            .index
            .seek(SeekFrom::Start(entry_position))
            .and_then(|_| bound.index.read_exact(&mut entry_bytes))
            .map_err(|_| "canonical autonomous replica index entry is unreadable")?;
        let entry = SidecarIndexEntry::from_bytes(entry_bytes);
        let payload_len = usize::try_from(entry.len)
            .map_err(|_| "canonical autonomous replica entry length is not representable")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(payload_len)
            .map_err(|_| "canonical autonomous replica allocation exceeds process limits")?;
        bytes.resize(payload_len, 0);
        bound
            .data
            .seek(SeekFrom::Start(entry.offset))
            .and_then(|_| bound.data.read_exact(&mut bytes))
            .map_err(|_| "canonical autonomous replica entry is unreadable")?;
        let record = norito::decode_canonical::<CanonicalAutonomousLaneReplicaV1>(&bytes)
            .map_err(|_| "canonical autonomous replica entry is not canonical framed Norito")?;
        Self::validate_canonical_autonomous_lane_replica_structure(&record)
            .map_err(|_| "canonical autonomous replica entry is structurally invalid")?;
        let descriptor = &record.bundle.certified.proposal.descriptor;
        if descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height {
            return Err("canonical autonomous replica entry names another lane slot");
        }
        if record
            .encode_framed()
            .map_err(|_| "canonical autonomous replica entry cannot be re-encoded")?
            != bytes
        {
            return Err("canonical autonomous replica entry is not canonically stable");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("canonical autonomous replica pair changed during exact lookup");
        }
        Ok(Some((record, bytes)))
    }

    /// Persist one non-owning replica derived solely from the exact finalized
    /// global carrier and a fully aggregate-validated lane certificate.
    ///
    /// No autonomous attempt, lifecycle cursor, READY/NewView sidecar, Queue
    /// reservation, entrypoint claim, or retirement record is read or written.
    /// Exact replay and a different fully validated quorum proof for the same
    /// decision are idempotent. A different carrier, payload, READY subject,
    /// or Prepare/Commit decision for the same active lane slot fails closed.
    pub(crate) fn persist_canonical_autonomous_lane_replica(
        &self,
        certified: &CertifiedLaneBlockArtifact,
    ) -> Result<DurableAutonomousLaneMergeSource> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        self.durable_mutation_authorized()?;
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let record = self
            .canonical_autonomous_lane_replica_from_certified_under_prune_and_canonical_guards(
                certified,
            )?;
        let descriptor = &record.bundle.certified.proposal.descriptor;
        let lane_block_height = descriptor.lane_block_height;
        if lane_block_height == 0 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica lane height must be non-zero",
            ));
        }
        let encoded = record.encode_framed()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let (data_path, index_path) =
            Self::canonical_autonomous_lane_replica_paths_for_entry(&entry, &self.store_root);
        let directory = data_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "canonical autonomous replica path has no parent directory",
            )
        })?;
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::MkDir(error, directory.clone()))?;
        let _sidecar_guard = self.sidecar_lock.lock();
        if !self.recover_bound_progress_sidecar_artifacts(
            &data_path,
            &index_path,
            CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
        ) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "canonical autonomous replica pair recovery did not reach a durable fixed point",
            ));
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        let mut pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let layout = match &mut pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => Some(
                self.validate_canonical_autonomous_lane_replica_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(data_path.clone(), message)
                    })?
                    .0,
            ),
        };
        if let BoundProgressPair::Present(bound) = &mut pair
            && let Some((existing, existing_bytes)) = self
                .read_canonical_autonomous_lane_replica_from_bound_locked(
                    descriptor.lane_id,
                    lane_block_height,
                    bound,
                )
                .map_err(|message| Self::invalid_lane_artifact_error(data_path.clone(), message))?
        {
            self.validate_canonical_autonomous_lane_replica_against_kura_under_prune_and_canonical_guards(
                &existing,
            )?;
            let exact_replay = existing == record && existing_bytes == encoded;
            if !exact_replay
                && !Self::canonical_autonomous_lane_replicas_certify_same_decision(
                    &existing, &record,
                )
            {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "active canonical autonomous replica slot contains a conflicting decision",
                ));
            }
            if !self
                .sync_bound_progress_sidecar(bound, CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL)
            {
                return Err(Error::IO(
                    std::io::Error::other(
                        "failed to make existing canonical autonomous replica durable",
                    ),
                    data_path,
                ));
            }
            return Self::canonical_autonomous_lane_replica_source(&existing);
        }
        drop(pair);
        let projected_entry_count = match layout {
            None | Some(SidecarIndexLayout { entry_count: 0, .. }) => 1_u64,
            Some(layout) if lane_block_height < layout.base_height => layout
                .entry_count
                .checked_add(layout.base_height - lane_block_height)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.clone(),
                        "canonical autonomous replica index growth overflows",
                    )
                })?,
            Some(layout) => layout.entry_count.max(
                lane_block_height
                    .checked_sub(layout.base_height)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            index_path.clone(),
                            "canonical autonomous replica index height overflows",
                        )
                    })?,
            ),
        };
        if usize::try_from(projected_entry_count).unwrap_or(usize::MAX)
            > self.canonical_autonomous_lane_replica_pair_entry_limit()
        {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "canonical autonomous replica index would exceed its bounded entry count",
            ));
        }
        let projected_data_len = Self::file_len_or_zero(&data_path)?
            .checked_add(u64::try_from(encoded.len())?)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "canonical autonomous replica data growth overflows",
                )
            })?;
        if projected_data_len
            > u64::try_from(self.canonical_autonomous_lane_replica_pair_byte_limit())
                .unwrap_or(u64::MAX)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "canonical autonomous replica data would exceed its aggregate byte budget",
            ));
        }
        let transient_bytes = u64::try_from(encoded.len())?
            .checked_add(Self::maximum_index_growth_for_unresolved_sidecar_write(
                lane_block_height,
            ))
            .and_then(|bytes| {
                bytes.checked_add(u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES).ok()?)
            })
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "canonical autonomous replica publication peak overflows",
                )
            })?;
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            transient_bytes,
            false,
            false,
            &data_path,
        )?;
        let before_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        if !Self::append_indexed_progress_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &encoded,
            CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
            None,
            &namespace,
        ) {
            return Err(Error::IO(
                std::io::Error::other("failed to persist canonical autonomous replica"),
                data_path,
            ));
        }
        let mut readback_pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let readback = match &mut readback_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => self
                .read_canonical_autonomous_lane_replica_from_bound_locked(
                    descriptor.lane_id,
                    lane_block_height,
                    bound,
                )
                .map_err(|message| Self::invalid_lane_artifact_error(data_path.clone(), message))?,
        }
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "canonical autonomous replica disappeared after strict publication",
            )
        })?;
        if readback.0 != record || readback.1 != encoded {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "canonical autonomous replica changed before durable readback",
            ));
        }
        self.validate_canonical_autonomous_lane_replica_against_kura_under_prune_and_canonical_guards(
            &readback.0,
        )?;
        let after_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        accounting_mutation.finish();
        self.note_committed_lane_status_change();
        Self::canonical_autonomous_lane_replica_source(&record)
    }

    /// Read and revalidate one merge-only canonical autonomous replica.
    ///
    /// `Ok(None)` means the exact slot is absent. Malformed, stale,
    /// non-canonical, partially published, or conflicting evidence is an error.
    pub(crate) fn durable_canonical_autonomous_lane_replica(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<Option<DurableAutonomousLaneMergeSource>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        self.durable_canonical_autonomous_lane_replica_under_prune_and_canonical_guards(
            lane_id,
            lane_block_height,
            expected_network_id,
            Some(expected_epoch),
        )
    }

    /// Read one merge-only replica when the caller cannot know the historical
    /// epoch until it has recovered the certified proposal.
    ///
    /// The embedded epoch is still authenticated against the exact verified
    /// global finality context before this method returns it.
    pub(crate) fn durable_canonical_autonomous_lane_replica_for_network(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
    ) -> Result<Option<DurableAutonomousLaneMergeSource>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        self.durable_canonical_autonomous_lane_replica_under_prune_and_canonical_guards(
            lane_id,
            lane_block_height,
            expected_network_id,
            None,
        )
    }

    /// Return a bounded newest suffix of fully revalidated, non-owning
    /// autonomous replicas accepted by `accept`.
    ///
    /// This is a repair-disabled diagnostics/recovery read. An unresolved
    /// append, malformed entry, stale carrier binding, or caller request above
    /// the explicit result bound is returned as an error rather than skipped.
    /// Results are ordered by ascending lane-local height.
    pub(crate) fn latest_canonical_autonomous_lane_replicas_matching<F>(
        &self,
        lane_id: LaneId,
        limit: usize,
        mut accept: F,
    ) -> Result<Vec<DurableAutonomousLaneMergeSource>>
    where
        F: FnMut(&DurableAutonomousLaneMergeSource) -> bool,
    {
        if limit > MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_MATCH_RESULTS {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical autonomous replica result limit exceeds its explicit bound",
            ));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let (data_path, index_path) =
            Self::canonical_autonomous_lane_replica_paths_for_entry(&entry, &self.store_root);
        let sidecar_guard = self.sidecar_lock.lock();
        if self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
            return Ok(Vec::new());
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            &data_path,
            &index_path,
            CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
        )?;
        let mut pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let scan_budget = limit
            .checked_mul(8)
            .unwrap_or(usize::MAX)
            .max(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET)
            .min(MAX_CANONICAL_AUTONOMOUS_LANE_REPLICA_MATCH_SCAN);
        let candidates = match &mut pair {
            BoundProgressPair::Absent(namespace) => {
                if !self.bound_progress_namespace_unchanged(namespace) {
                    return Err(Self::invalid_lane_artifact_error(
                        data_path,
                        "canonical autonomous replica absence changed during bounded read",
                    ));
                }
                Vec::new()
            }
            BoundProgressPair::Present(bound) => {
                let (_, heights) = self
                    .validate_canonical_autonomous_lane_replica_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(data_path.clone(), message)
                    })?;
                let mut candidates = Vec::new();
                for lane_block_height in heights.into_iter().rev().take(scan_budget) {
                    let (record, _) = self
                        .read_canonical_autonomous_lane_replica_from_bound_locked(
                            lane_id,
                            lane_block_height,
                            bound,
                        )
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(data_path.clone(), message)
                        })?
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                data_path.clone(),
                                "canonical autonomous replica disappeared during bounded read",
                            )
                        })?;
                    self.require_active_lane_artifact(
                        &entry,
                        &record.bundle.certified.proposal.descriptor,
                    )?;
                    self.validate_canonical_autonomous_lane_replica_against_kura_under_prune_and_canonical_guards(
                        &record,
                    )?;
                    candidates.push(Self::canonical_autonomous_lane_replica_source(&record)?);
                }
                if !self.bound_progress_sidecar_unchanged(bound) {
                    return Err(Self::invalid_lane_artifact_error(
                        data_path,
                        "canonical autonomous replica pair changed during bounded read",
                    ));
                }
                candidates
            }
        };
        drop(pair);
        drop(sidecar_guard);
        drop(geometry_guard);
        drop(canonical_chain_guard);
        drop(prune_guard);
        let mut accepted = candidates
            .into_iter()
            .filter(|source| accept(source))
            .take(limit)
            .collect::<Vec<_>>();
        accepted.reverse();
        Ok(accepted)
    }

    /// Read one exact replica while the caller holds the complete
    /// prune/canonical/geometry/sidecar lock corridor.
    ///
    /// Keeping this reader lock-free is important for canonical terminal
    /// reconciliation, which must join the replica to its merge receipt before
    /// Queue receives any cleanup authority. The optional chain context is
    /// used only by terminal completion, where the authenticated payload is
    /// itself the source of the network and epoch.
    fn canonical_autonomous_lane_replica_record_from_paths_locked(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        data_path: &Path,
        index_path: &Path,
        expected_network_id: Option<iroha_data_model::NetworkId>,
        expected_epoch: Option<u64>,
    ) -> Result<Option<CanonicalAutonomousLaneReplicaV1>> {
        if lane_block_height == 0 {
            return Ok(None);
        }
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        if self.bound_progress_sidecar_directory_is_absent(data_path, index_path)? {
            return Ok(None);
        }
        let namespace = self.open_bound_progress_namespace(data_path, index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            data_path,
            index_path,
            CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
        )?;
        let mut pair = self.open_bound_progress_pair(data_path, index_path)?;
        let record = match &mut pair {
            BoundProgressPair::Absent(namespace) => {
                if !self.bound_progress_namespace_unchanged(namespace) {
                    return Err(Self::invalid_lane_artifact_error(
                        data_path.to_path_buf(),
                        "canonical autonomous replica absence changed during read",
                    ));
                }
                return Ok(None);
            }
            BoundProgressPair::Present(bound) => self
                .read_canonical_autonomous_lane_replica_from_bound_locked(
                    lane_id,
                    lane_block_height,
                    bound,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(data_path.to_path_buf(), message)
                })?,
        };
        let Some((record, _)) = record else {
            return Ok(None);
        };
        let payload = record.bundle.executable_payload();
        if expected_network_id.is_some_and(|expected| payload.network_id != expected)
            || expected_epoch.is_some_and(|expected| payload.epoch != expected)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                "canonical autonomous replica has the wrong network or epoch",
            ));
        }
        self.validate_canonical_autonomous_lane_replica_against_kura_under_prune_and_canonical_guards(
            &record,
        )?;
        if let BoundProgressPair::Present(bound) = &pair
            && !self.bound_progress_sidecar_unchanged(bound)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                "canonical autonomous replica pair changed during canonical validation",
            ));
        }
        Ok(Some(record))
    }

    fn canonical_autonomous_lane_replica_record_locked(
        &self,
        entry: &LaneConfigEntry,
        lane_block_height: u64,
        expected_network_id: Option<iroha_data_model::NetworkId>,
        expected_epoch: Option<u64>,
    ) -> Result<Option<CanonicalAutonomousLaneReplicaV1>> {
        let (data_path, index_path) =
            Self::canonical_autonomous_lane_replica_paths_for_entry(entry, &self.store_root);
        let record = self.canonical_autonomous_lane_replica_record_from_paths_locked(
            entry.lane_id,
            lane_block_height,
            &data_path,
            &index_path,
            expected_network_id,
            expected_epoch,
        )?;
        if let Some(record) = record.as_ref() {
            self.require_active_lane_artifact(entry, &record.bundle.certified.proposal.descriptor)?;
        }
        Ok(record)
    }

    /// Guard-aware reader used by startup verification sessions.
    fn durable_canonical_autonomous_lane_replica_under_prune_and_canonical_guards(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: Option<u64>,
    ) -> Result<Option<DurableAutonomousLaneMergeSource>> {
        if lane_block_height == 0 {
            return Ok(None);
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        self.canonical_autonomous_lane_replica_record_locked(
            &entry,
            lane_block_height,
            Some(expected_network_id),
            expected_epoch,
        )?
        .as_ref()
        .map(Self::canonical_autonomous_lane_replica_source)
        .transpose()
    }

    /// Recover interrupted replica pair appends before runtime/startup readers
    /// are allowed to interpret their contents.
    fn recover_canonical_autonomous_lane_replica_pairs_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entries = self
            .lane_storage_entries
            .lock()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let _sidecar_guard = self.sidecar_lock.lock();
        for entry in entries {
            let (data_path, index_path) =
                Self::canonical_autonomous_lane_replica_paths_for_entry(&entry, &self.store_root);
            if self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
                continue;
            }
            if !self.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "canonical autonomous replica pair failed startup recovery",
                ));
            }
        }
        Ok(())
    }
}
