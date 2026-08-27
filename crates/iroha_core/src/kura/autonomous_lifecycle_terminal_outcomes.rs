/// Custody-fenced completion permit for one exact, re-observed bootstrap authority.
enum AutonomousLifecycleBootstrapCompletionFence<'queue> {
    ProducerQueue(AutonomousLaneKuraActivationAuthorization<'queue>),
    DurablePayloadCustody,
    #[cfg(test)]
    Test,
}
/// Exact bootstrap revalidation used only by custody-fenced completion.
#[must_use = "the bootstrap completion revalidation must be handled"]
struct AutonomousLifecycleBootstrapCompletionRevalidation {
    /// Exact bootstrap authority refreshed at its current durable crash boundary.
    authority: AutonomousLifecycleBootstrapRecoveryAuthority,
    /// Whether the exact application receipt was observed during this refresh.
    receipt_terminal: bool,
}
/// Authorization mode for the low-level autonomous payload writer.
#[derive(Clone, Copy)]
enum LaneExecutablePayloadPersistenceMode<'bootstrap> {
    /// Ordinary writers stop before mutation once an exact receipt is durable.
    #[cfg(test)]
    Ordinary,
    /// A pre-existing signed bootstrap may roll its exact payload forward to
    /// Live so canonical terminal reconciliation retains a complete lifecycle unit.
    SignedBootstrap(&'bootstrap AutonomousLifecycleBootstrapRecoveryAuthority),
}
#[must_use = "the authenticated bootstrap permit must be completed or deliberately dropped"]
pub(crate) struct AutonomousLifecycleBootstrapCompletionPermit<'queue> {
    authority: AutonomousLifecycleBootstrapRecoveryAuthority,
    fence: AutonomousLifecycleBootstrapCompletionFence<'queue>,
}
/// Exact post-bootstrap cursor observation; a newer process must take over through Crash/Recover.
#[must_use = "the post-bootstrap lifecycle lease must be consumed or deliberately dropped"]
pub(crate) struct AutonomousLifecycleBootstrapCompletion {
    cursor_read: AutonomousLifecycleCursorRead,
    takeover_required: bool,
}
/// Result of completing a custody-fenced lifecycle bootstrap.
#[must_use = "the bootstrap completion outcome must be handled"]
pub(crate) enum AutonomousLifecycleBootstrapCompletionOutcome {
    /// The payload and Live cursor crossed their durability boundaries.
    Completed(AutonomousLifecycleBootstrapCompletion),
    /// The exact proposal was already durably applied. Its pre-existing signed
    /// bootstrap was rolled forward to an exact Live lifecycle unit without
    /// re-entering volatile consensus or releasing Queue ownership.
    AlreadyTerminal,
}
impl AutonomousLifecycleBootstrapCompletion {
    /// Whether the pre-signed historical Live owner must be taken over by this process generation.
    #[must_use]
    pub(crate) const fn takeover_required(&self) -> bool {
        self.takeover_required
    }
    /// Borrow the exact Live cursor durably read after bootstrap deletion.
    #[must_use]
    pub(crate) fn cursor(&self) -> &AutonomousLifecycleCursorV1 {
        self.cursor_read
            .cursor()
            .expect("completed bootstrap always returns its exact Live cursor")
    }
    /// Consume the completion into the ordinary move-only cursor read and CAS lease.
    #[must_use]
    pub(crate) fn into_cursor_read(self) -> AutonomousLifecycleCursorRead {
        self.cursor_read
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutonomousLaneBlockViewStateReadMode {
    MainOnly,
    LatestReadOnly,
    Recover { pending_canonical_bytes: u64 },
}
/// Result of a lane auxiliary-artifact persistence attempt serialized with
/// application-receipt publication and merge-frontier compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneBlockAuxiliaryPersistenceOutcome {
    /// The requested auxiliary artifact crossed its durability boundary.
    Persisted,
    /// The exact lane proposal is already durably applied, so auxiliary
    /// payload/input state is terminal and must not be recreated.
    AlreadyTerminal,
}
/// Result of appending one authenticated autonomous NewView certificate while
/// serialized with exact lane-application receipt publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LaneBlockNewViewPersistenceOutcome {
    /// The certificate crossed its durability boundary and advanced this cursor.
    Persisted(LaneBlockProposalV1),
    /// The exact immutable origin proposal is already durably applied, so no
    /// later view evidence may be written for its retained lifecycle attempt.
    AlreadyTerminal,
}
#[derive(Clone, Debug)]
struct AutonomousLaneAttemptInventoryBudget {
    attempts_at_height: usize,
    lifecycle_identities: BTreeSet<(u64, u64)>,
    terminal_outcome_identities: BTreeSet<(u64, u64)>,
    complete_terminal_outcome_identities: BTreeSet<(u64, u64)>,
    conceptual_files: usize,
    conceptual_bytes: u64,
}
impl AutonomousLaneAttemptInventoryBudget {
    fn empty() -> Self {
        Self {
            attempts_at_height: 0,
            lifecycle_identities: BTreeSet::new(),
            terminal_outcome_identities: BTreeSet::new(),
            complete_terminal_outcome_identities: BTreeSet::new(),
            conceptual_files: 0,
            conceptual_bytes: 0,
        }
    }
    fn has_reserved_terminal_outcome(&self, identity: (u64, u64)) -> bool {
        self.lifecycle_identities.contains(&identity)
            && !self.terminal_outcome_identities.contains(&identity)
    }
    fn needs_terminal_reservation_for_new_identity(&self, identity: (u64, u64)) -> bool {
        !self.lifecycle_identities.contains(&identity)
            && !self.terminal_outcome_identities.contains(&identity)
    }
}
struct AutonomousLifecycleTerminalPendingPublicationPlan {
    entry: LaneConfigEntry,
    identity: (u64, u64),
    path: PathBuf,
    outcome: AutonomousLifecycleTerminalOutcomeV1,
    pending_bytes: Option<Vec<u8>>,
}
impl Kura {
    fn active_autonomous_lifecycle_attempt_inventory_for_process_record(
        &self,
        process_record: &AutonomousLifecycleProcessGenerationRecordV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_local_peer_id: &PeerId,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        planner_covered_pending_groups: &[LaneQueueReservationGroupBindingV1],
    ) -> Result<Vec<AutonomousLifecycleAttemptInventoryEntry>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let (active_incarnation, _) = self.active_lane_incarnation_marker(&entry)?;
        if entry.dataspace_id != dataspace_id
            || active_incarnation != lane_incarnation
            || process_record.body.network_id != expected_network_id
            || &process_record.body.local_peer_id != expected_local_peer_id
        {
            return Err(Self::invalid_lane_artifact_error(
                Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)),
                "autonomous lifecycle inventory targets a stale route or process identity",
            ));
        }
        if planner_covered_pending_groups.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(Self::invalid_lane_artifact_error(
                Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)),
                "planner-covered lifecycle Pending group set exceeds its hard bound",
            ));
        }
        let mut planner_covered = BTreeMap::new();
        for group in planner_covered_pending_groups {
            if group.identity.lane_id != lane_id
                || group.identity.dataspace_id != dataspace_id
                || group.identity.lane_incarnation != lane_incarnation
                || group.identity.proposal_height == 0
                || group.identity.lane_block_height == 0
                || group.reservation_count == 0
                || group
                    .reservation_group_hash
                    .as_ref()
                    .iter()
                    .all(|byte| *byte == 0)
                || planner_covered
                    .insert(group.reservation_group_hash, *group)
                    .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)),
                    "planner-covered lifecycle Pending group set is stale, malformed, or duplicated",
                ));
            }
        }
        let mut consumed_planner_covered = BTreeSet::new();
        let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let _sidecar_guard = self.sidecar_lock.lock();
        let _namespace_budget = self.autonomous_lane_attempt_inventory_counts_locked(&entry, 1)?;
        let directory_entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                self.ensure_autonomous_lifecycle_process_generation_record_unchanged(
                    process_record,
                )?;
                if !planner_covered.is_empty() {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "planner-covered lifecycle Pending group has no active artifact namespace",
                    ));
                }
                return Ok(Vec::new());
            }
            Err(error) => return Err(Error::IO(error, directory)),
        };
        let mut related_files = 0_usize;
        let mut related_bytes = 0_u64;
        let mut attempts = BTreeMap::<(u64, u64), LaneExecutablePayloadV1>::new();
        let mut attempts_per_height = BTreeMap::<u64, usize>::new();
        let mut cursors = BTreeMap::<(u64, u64), AutonomousLifecycleCursorV1>::new();
        let mut terminal_outcomes =
            BTreeMap::<(u64, u64), (PathBuf, AutonomousLifecycleTerminalOutcomeV1)>::new();
        let mut bootstrap_stages =
            BTreeMap::<(u64, u64), AutonomousLifecycleBootstrapRecoveryStage>::new();
        for directory_entry in directory_entries {
            let directory_entry =
                directory_entry.map_err(|error| Error::IO(error, directory.clone()))?;
            let path = directory_entry.path();
            let name = directory_entry.file_name().into_string().map_err(|_| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle inventory contains a non-UTF-8 artifact",
                )
            })?;
            if Self::is_unresolved_autonomous_publication_temporary_name(
                &name,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle inventory found a bootstrap atomic temporary",
                ));
            }
            if !name.starts_with("autonomous_") && !name.starts_with(".kura-sidecar-") {
                continue;
            }
            related_files = related_files.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous lifecycle inventory file count overflows",
                )
            })?;
            if related_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "autonomous lifecycle inventory exceeds its hard file-count limit",
                ));
            }
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle inventory contains a non-regular, linked, or symlinked artifact",
                ));
            }
            related_bytes = related_bytes.checked_add(metadata.len()).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous lifecycle inventory byte count overflows",
                )
            })?;
            if related_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "autonomous lifecycle inventory exceeds the shared sidecar aggregate byte budget",
                ));
            }
            if name.starts_with(".kura-sidecar-") {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle inventory found an unresolved atomic temporary",
                ));
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lane_block_attempt_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle payload disappeared during bounded inventory",
                        )
                    })?;
                let artifact = norito::decode_canonical::<AutonomousLaneBlockArtifact>(&bytes)
                    .map_err(Error::NoritoFrame)?;
                let pointer =
                    AutonomousLaneBlockLatestAttemptV1::from_payload(&artifact.executable_payload);
                let descriptor = &artifact.executable_payload.origin_proposal.descriptor;
                if pointer.network_id != expected_network_id
                    || pointer.lane_id != lane_id
                    || pointer.dataspace_id != dataspace_id
                    || pointer.lane_incarnation != lane_incarnation
                    || pointer.lane_block_height != lane_block_height
                    || pointer.proposal_height != proposal_height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle payload has a stale or conflicting route identity",
                    ));
                }
                let record = self
                    .read_autonomous_lane_block_attempt_record_locked(
                        &entry,
                        lane_id,
                        lane_block_height,
                        proposal_height,
                        pointer.network_id,
                        pointer.epoch,
                        None,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle payload disappeared after validation",
                        )
                    })?;
                if descriptor
                    .validator_set
                    .iter()
                    .all(|peer| peer != expected_local_peer_id)
                {
                    continue;
                }
                let retained = attempts_per_height.entry(lane_block_height).or_default();
                *retained = retained.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "autonomous lifecycle per-height retention count overflows",
                    )
                })?;
                if *retained > self.lane_history_retention.get() {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle attempts exceed the per-height retention bound",
                    ));
                }
                if attempts
                    .insert(
                        (lane_block_height, proposal_height),
                        record.artifact.executable_payload,
                    )
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "autonomous lifecycle inventory contains duplicate payload identities",
                    ));
                }
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_cursor_coordinates(&name) {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle cursor disappeared during bounded inventory",
                        )
                    })?;
                let cursor = Self::decode_autonomous_lifecycle_cursor(&path, &bytes)?;
                Self::validate_autonomous_lifecycle_cursor_process_generation(
                    &process_record,
                    &cursor,
                )
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                if cursors.insert(identity, cursor).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle inventory contains duplicate cursor identities",
                    ));
                }
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_bootstrap_coordinates(&name) {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle bootstrap disappeared during active inventory",
                        )
                    })?;
                let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&path, &bytes)?;
                Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                    &process_record,
                    &bootstrap,
                )
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
                if identity != (descriptor.lane_block_height, descriptor.proposal_height)
                    || descriptor.lane_id != lane_id
                    || descriptor.dataspace_id != dataspace_id
                    || descriptor.lane_incarnation != lane_incarnation
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle bootstrap conflicts with active inventory route",
                    ));
                }
                let stage =
                    self.classify_autonomous_lifecycle_bootstrap_locked(&entry, &bootstrap)?;
                if bootstrap_stages.insert(identity, stage).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle inventory contains duplicate bootstrap identities",
                    ));
                }
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_terminal_outcome_coordinates(&name) {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle terminal outcome disappeared during active inventory",
                        )
                    })?;
                let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
                if terminal_outcomes
                    .insert(identity, (path.clone(), outcome))
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle inventory contains duplicate terminal outcomes",
                    ));
                }
                continue;
            }
            if Self::autonomous_lane_block_attempt_view_temp_coordinates(&name).is_some() {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle inventory found an unresolved named temporary",
                ));
            }
            if Self::autonomous_two_height_coordinates(
                &name,
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
            )
            .is_some()
                || Self::autonomous_one_height_coordinate(
                    &name,
                    AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
                )
                .is_some()
                || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
            {
                continue;
            }
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle inventory found an unexpected or obsolete artifact",
            ));
        }
        if cursors
            .keys()
            .any(|identity| !attempts.contains_key(identity))
        {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous lifecycle inventory contains an orphan local cursor",
            ));
        }
        if terminal_outcomes
            .keys()
            .any(|identity| !attempts.contains_key(identity) || !cursors.contains_key(identity))
        {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous lifecycle inventory contains an orphan terminal outcome",
            ));
        }
        if terminal_outcomes
            .keys()
            .any(|identity| bootstrap_stages.contains_key(identity))
        {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous lifecycle terminal outcome overlaps an unfinished signed bootstrap",
            ));
        }
        if cursors.iter().any(|(identity, cursor)| {
            cursor.sequence() == 1
                && cursor.phase_kind() == AutonomousLifecycleCursorPhaseKindV1::Prepared
                && !bootstrap_stages.contains_key(identity)
        }) {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "initial Prepared lifecycle cursor is orphaned from its signed bootstrap",
            ));
        }
        if attempts.keys().any(|identity| {
            !cursors.contains_key(identity)
                && bootstrap_stages.get(identity)
                    != Some(&AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable)
        }) {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous payload attempt lacks its exact lifecycle cursor or signed payload-durable bootstrap",
            ));
        }
        let mut inventory = Vec::new();
        inventory.try_reserve_exact(attempts.len())?;
        for (identity, executable_payload) in attempts {
            let cursor = cursors.remove(&identity);
            if let Some(cursor) = cursor.as_ref() {
                cursor
                    .validate_for_payload(&executable_payload)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                    })?;
            }
            if let Some((path, outcome)) = terminal_outcomes.remove(&identity) {
                outcome
                    .validate_for_payload(&executable_payload)
                    .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                if cursor.as_ref().map(AutonomousLifecycleCursorV1::binding)
                    != Some(outcome.binding())
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle terminal outcome differs from its signed cursor binding",
                    ));
                }
                match outcome.source() {
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
                        ..
                    } => {
                        self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                            &executable_payload,
                            source,
                        )?;
                    }
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
                        ..
                    } => {
                        let record = self
                            .read_autonomous_lane_block_attempt_record_locked(
                                &entry,
                                lane_id,
                                identity.0,
                                identity.1,
                                executable_payload.network_id,
                                executable_payload.epoch,
                                None,
                            )?
                            .ok_or_else(|| {
                                Self::invalid_lane_artifact_error(
                                    path.clone(),
                                    "autonomous lifecycle release outcome lost its attempt",
                                )
                            })?;
                        self.autonomous_lifecycle_terminal_source_matches_release_locked(
                            Some(pending_canonical_bytes),
                            &entry,
                            &executable_payload,
                            record.retirement.as_ref(),
                            source,
                        )?;
                    }
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                        ..
                    } => {
                        let record = self
                            .read_autonomous_lane_block_attempt_record_locked(
                                &entry,
                                lane_id,
                                identity.0,
                                identity.1,
                                executable_payload.network_id,
                                executable_payload.epoch,
                                None,
                            )?
                            .ok_or_else(|| {
                                Self::invalid_lane_artifact_error(
                                    path.clone(),
                                    "autonomous lifecycle replica outcome lost its attempt",
                                )
                            })?;
                        let queue_disposition = self
                            .autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                                Some(pending_canonical_bytes),
                                &entry,
                                &executable_payload,
                                record.retirement.as_ref(),
                                source,
                            )?;
                        if outcome.is_complete() {
                            let retirement = record.retirement.as_ref().ok_or_else(|| {
                                Self::invalid_lane_artifact_error(
                                    path.clone(),
                                    "Complete replica Queue disposition lost its exact retirement",
                                )
                            })?;
                            self.complete_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                                pending_canonical_bytes,
                                Some(pending_canonical_bytes),
                                &executable_payload,
                                retirement,
                                queue_disposition,
                                &outcome,
                            )?;
                        }
                    }
                }
                if outcome.is_complete() {
                    // Queue ownership has already reached one exact terminal
                    // owner. Do not feed this attempt back into Crash/Recover
                    // lifecycle planning on startup.
                    continue;
                }
                let group = outcome.binding().reservation_group_binding();
                if planner_covered.get(&group.reservation_group_hash) == Some(&group) {
                    if !consumed_planner_covered.insert(group.reservation_group_hash) {
                        return Err(Self::invalid_lane_artifact_error(
                            path,
                            "planner-covered lifecycle Pending group aliases multiple attempts",
                        ));
                    }
                } else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::WouldBlock,
                            "autonomous lifecycle Pending terminal outcome must be reconciled before lifecycle attempt inventory",
                        ),
                        path,
                    ));
                }
            }
            inventory.push(AutonomousLifecycleAttemptInventoryEntry {
                executable_payload,
                cursor,
            });
        }
        if consumed_planner_covered.len() != planner_covered.len() {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "planner-covered lifecycle Pending group was not an exact source-validated active attempt",
            ));
        }
        self.ensure_autonomous_lifecycle_process_generation_record_unchanged(process_record)?;
        Ok(inventory)
    }
    fn autonomous_two_height_coordinates(name: &str, prefix: &str) -> Option<(u64, u64)> {
        let raw = name
            .strip_prefix(prefix)?
            .strip_prefix('_')?
            .strip_suffix(".norito")?;
        let (lane_block_height, proposal_height) = raw.split_once('_')?;
        let lane_block_height = lane_block_height.parse::<u64>().ok()?;
        let proposal_height = proposal_height.parse::<u64>().ok()?;
        (lane_block_height != 0
            && proposal_height != 0
            && name == format!("{prefix}_{lane_block_height:020}_{proposal_height:020}.norito"))
        .then_some((lane_block_height, proposal_height))
    }
    fn autonomous_lane_block_attempt_coordinates(name: &str) -> Option<(u64, u64)> {
        Self::autonomous_two_height_coordinates(name, AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX)
    }
    fn autonomous_lifecycle_cursor_coordinates(name: &str) -> Option<(u64, u64)> {
        Self::autonomous_two_height_coordinates(name, AUTONOMOUS_LIFECYCLE_CURSOR_PREFIX)
    }
    fn autonomous_lifecycle_bootstrap_coordinates(name: &str) -> Option<(u64, u64)> {
        Self::autonomous_two_height_coordinates(name, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_PREFIX)
    }
    fn autonomous_lane_block_attempt_view_temp_coordinates(name: &str) -> Option<(u64, u64)> {
        let stable_name = name.strip_suffix(".tmp")?;
        Self::autonomous_two_height_coordinates(
            stable_name,
            AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
        )
    }
    fn autonomous_lifecycle_terminal_outcome_coordinates(name: &str) -> Option<(u64, u64)> {
        Self::autonomous_two_height_coordinates(name, AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_PREFIX)
    }
    fn autonomous_lifecycle_terminal_outcome_path_for_entry(
        entry: &LaneConfigEntry,
        store_root: &Path,
        lane_block_height: u64,
        proposal_height: u64,
    ) -> PathBuf {
        Self::lane_artifact_dir(&entry.blocks_dir(store_root)).join(format!(
            "{AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_PREFIX}_{lane_block_height:020}_{proposal_height:020}.norito"
        ))
    }
    #[cfg(test)]
    pub(crate) fn autonomous_lifecycle_terminal_outcome_path_for_test(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_height: u64,
    ) -> Result<PathBuf> {
        let entry = self.lane_storage_entry(lane_id)?;
        Ok(Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
            &entry,
            &self.store_root,
            lane_block_height,
            proposal_height,
        ))
    }
    fn decode_autonomous_lifecycle_terminal_outcome(
        path: &Path,
        bytes: &[u8],
    ) -> Result<AutonomousLifecycleTerminalOutcomeV1> {
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle terminal outcome exceeds its hard byte limit",
            ));
        }
        let outcome = norito::decode_canonical::<AutonomousLifecycleTerminalOutcomeV1>(bytes)
            .map_err(|error| match error {
                norito::Error::NonCanonicalEncoding => Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous lifecycle terminal outcome is not canonical Norito",
                ),
                other => Error::NoritoFrame(other),
            })?;
        outcome
            .validate_structure()
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        if outcome.encode_framed().map_err(Error::NoritoFrame)? != bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle terminal outcome does not round-trip canonically",
            ));
        }
        let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle terminal outcome path has no UTF-8 filename",
            ));
        };
        if Self::autonomous_lifecycle_terminal_outcome_coordinates(name)
            != Some((
                outcome.binding().lane_block_height,
                outcome.binding().proposal_height,
            ))
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle terminal outcome identity does not match its exact attempt path",
            ));
        }
        Ok(outcome)
    }
    fn validate_autonomous_lifecycle_terminal_outcome_budget(
        related_files: usize,
        related_bytes: u64,
        previous_len: u64,
        next_len: u64,
        replacing_existing: bool,
    ) -> std::result::Result<(), &'static str> {
        if replacing_existing != (previous_len != 0) {
            return Err(
                "autonomous lifecycle terminal outcome replacement accounting is inconsistent",
            );
        }
        if next_len > AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64 {
            return Err("autonomous lifecycle terminal outcome exceeds its hard byte limit");
        }
        // A create consumes the conceptual file/byte slot reserved when the
        // lifecycle identity was first admitted. Replacement already owns the
        // exact stable slot, so it substitutes only its framed bytes.
        let resulting_files = related_files;
        if resulting_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err("autonomous lifecycle terminal outcome exceeds the namespace file bound");
        }
        let replaced_len = if replacing_existing {
            previous_len
        } else {
            AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64
        };
        let resulting_bytes = related_bytes
            .checked_sub(replaced_len)
            .and_then(|bytes| bytes.checked_add(next_len))
            .ok_or(
                "autonomous lifecycle terminal outcome byte accounting or reservation underflowed",
            )?;
        if resulting_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err("autonomous lifecycle terminal outcome exceeds the shared byte budget");
        }
        let peak_files = related_files
            .checked_add(1)
            .ok_or("autonomous lifecycle terminal outcome temporary file accounting overflowed")?;
        if peak_files > MAX_AUTONOMOUS_LIFECYCLE_CURSOR_CAS_PEAK_FILES {
            return Err("autonomous lifecycle terminal outcome exceeds the temporary file bound");
        }
        let peak_bytes = related_bytes
            .checked_add(next_len)
            .ok_or("autonomous lifecycle terminal outcome temporary byte accounting overflowed")?;
        if peak_bytes > AUTONOMOUS_LIFECYCLE_CURSOR_CAS_PEAK_BYTES as u64 {
            return Err("autonomous lifecycle terminal outcome exceeds the temporary byte bound");
        }
        Ok(())
    }
    fn autonomous_lifecycle_terminal_source_from_merge_receipt(
        receipt: &LaneBlockApplicationReceiptArtifact,
    ) -> std::result::Result<AutonomousLifecycleTerminalOutcomeSourceV1, &'static str> {
        if receipt.format != LaneBlockApplicationReceiptArtifactFormat::MergeExecution {
            return Err("autonomous lifecycle canonical outcome requires a merge receipt");
        }
        let merge_epoch_id = receipt
            .merge_epoch_id
            .ok_or("autonomous lifecycle merge receipt lacks its epoch")?;
        let merge_entry_hash = receipt
            .merge_entry_hash
            .ok_or("autonomous lifecycle merge receipt lacks its entry hash")?;
        let carrier_block_height = receipt
            .merge_carrier_block_height
            .ok_or("autonomous lifecycle merge receipt lacks its carrier height")?;
        let carrier_block_hash = receipt
            .merge_carrier_block_hash
            .ok_or("autonomous lifecycle merge receipt lacks its carrier hash")?;
        let source = AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
            merge_epoch_id,
            merge_entry_hash,
            carrier_block_height,
            carrier_block_hash,
            application_receipt_hash: HashOf::new(receipt),
        };
        source.validate_structure()?;
        Ok(source)
    }
    fn autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
        &self,
        payload: &LaneExecutablePayloadV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<LaneBlockApplicationReceiptArtifact> {
        let descriptor = &payload.origin_proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let (receipt_data_path, receipt_index_path) =
            Self::lane_block_application_receipt_paths_for_entry(&entry, &self.store_root);
        self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_from_receipt_paths_locked(
            payload,
            source,
            &receipt_data_path,
            &receipt_index_path,
        )
    }
    /// Revalidate a canonical terminal source against both its committed
    /// merge carrier and the exact durability-attested receipt pair at the
    /// caller-selected active or archived namespace.
    ///
    /// The caller holds the prune, canonical-chain, geometry, and sidecar
    /// guards. This deliberately uses the lock-free inner receipt reader so a
    /// startup or retirement sweep cannot recurse into those locks.
    fn autonomous_lifecycle_terminal_source_matches_canonical_carrier_from_receipt_paths_locked(
        &self,
        payload: &LaneExecutablePayloadV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
        receipt_data_path: &Path,
        receipt_index_path: &Path,
    ) -> Result<LaneBlockApplicationReceiptArtifact> {
        let AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
            merge_epoch_id,
            merge_entry_hash,
            carrier_block_height,
            carrier_block_hash,
            application_receipt_hash,
        } = source
        else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle terminal source is not a canonical carrier",
            ));
        };
        let entry = self
            .merge_log
            .lock()
            .entry_by_hash(merge_entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lifecycle canonical terminal source lost its merge entry",
                )
            })?;
        if entry.epoch_id != merge_epoch_id {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal source changed merge epoch",
            ));
        }
        let expected_carrier = MergeLedgerCarrierRecord {
            version: 1,
            entry_hash: merge_entry_hash,
            epoch_id: merge_epoch_id,
            block_height: carrier_block_height,
            block_hash: carrier_block_hash,
        };
        if self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(merge_entry_hash)?
            .as_ref()
            != Some(&expected_carrier)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal source lost its canonical carrier",
            ));
        }
        let batch = entry.execution_batch.as_ref().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal source has no execution batch",
            )
        })?;
        let mut matching = batch.lanes.iter().filter_map(|execution| {
            let bundle = Self::decode_autonomous_lane_merge_bundle(
                &execution.source_bundle,
                execution.autonomous_network_id,
                execution.autonomous_epoch,
            )
            .ok()?;
            (bundle.executable_payload() == payload
                && bundle.bundle_hash().ok() == Some(execution.source_bundle_hash)
                && execution.origin_proposal == payload.origin_proposal
                && execution.autonomous_network_id == payload.network_id
                && execution.autonomous_epoch == payload.epoch
                && execution.autonomous_payload_hash == payload.payload_hash)
                .then_some((execution, bundle))
        });
        let Some((execution, bundle)) = matching.next() else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal source has no exact payload execution",
            ));
        };
        if matching.next().is_some()
            || bundle.certified.proposal != execution.proposal
            || bundle.certified.prepare_qc != execution.prepare_qc
            || bundle.certified.commit_qc != execution.commit_qc
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal source has ambiguous execution evidence",
            ));
        }
        let expected = LaneBlockApplicationReceiptArtifact::new_merge_execution(
            &entry,
            batch,
            execution,
            Self::merge_lane_block_execution_source(execution),
            carrier_block_height,
            carrier_block_hash,
        );
        if HashOf::new(&expected) != application_receipt_hash {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle canonical terminal receipt hash changed",
            ));
        }
        self.require_exact_autonomous_lifecycle_terminal_application_receipt_locked(
            &expected,
            receipt_data_path,
            receipt_index_path,
        )?;
        Ok(expected)
    }
    fn require_exact_autonomous_lifecycle_terminal_application_receipt_locked(
        &self,
        expected: &LaneBlockApplicationReceiptArtifact,
        receipt_data_path: &Path,
        receipt_index_path: &Path,
    ) -> Result<()> {
        let descriptor = &expected.proposal.descriptor;
        let durable = self
            .read_lane_block_application_receipt_from_paths_durability_attested_locked(
                descriptor.lane_id,
                descriptor.lane_block_height,
                receipt_data_path,
                receipt_index_path,
                false,
            )
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    receipt_data_path.to_path_buf(),
                    "autonomous lifecycle canonical terminal source lost its exact durability-attested application receipt",
                )
            })?;
        if durable != *expected {
            return Err(Self::invalid_lane_artifact_error(
                receipt_data_path.to_path_buf(),
                "autonomous lifecycle canonical terminal source receipt differs from its reconstructed merge application",
            ));
        }
        Ok(())
    }
    #[cfg(test)]
    fn require_exact_autonomous_lifecycle_terminal_application_receipt_for_tests(
        &self,
        expected: &LaneBlockApplicationReceiptArtifact,
        receipt_data_path: &Path,
        receipt_index_path: &Path,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        self.require_exact_autonomous_lifecycle_terminal_application_receipt_locked(
            expected,
            receipt_data_path,
            receipt_index_path,
        )
    }
    fn autonomous_lifecycle_terminal_source_matches_release_locked(
        &self,
        pending_canonical_bytes: Option<u64>,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        retirement: Option<&AutonomousLaneSlotRetirementV1>,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<()> {
        let AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease { retirement_hash } = source
        else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle terminal source is not a retired release",
            ));
        };
        let retirement = retirement.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle release terminal source lost its durable retirement",
            )
        })?;
        if !retirement.matches_payload(payload) || retirement.digest()? != retirement_hash {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle release terminal source changed retirement identity",
            ));
        }
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                entry,
                payload.origin_proposal.descriptor.lane_block_height,
                pending_canonical_bytes,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lifecycle release terminal source has no current lane attempt",
                )
            })?;
        if current.artifact.executable_payload == *payload {
            self.require_autonomous_lane_entrypoint_claims_released_locked(payload, retirement)
        } else {
            self.require_autonomous_lane_release_completed_or_superseded_locked(
                entry, payload, retirement,
            )
        }
    }
    fn autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
        &self,
        pending_canonical_bytes: Option<u64>,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        retirement: Option<&AutonomousLaneSlotRetirementV1>,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<AutonomousLifecycleReplicaQueueDispositionV1> {
        let AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
            retirement_hash,
            queue_disposition,
        } = source
        else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle terminal source is not a replica Queue disposition",
            ));
        };
        let retirement = retirement.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle replica terminal source lost its durable retirement",
            )
        })?;
        if !retirement.matches_payload(payload) || retirement.digest()? != retirement_hash {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lifecycle replica terminal source changed retirement identity",
            ));
        }
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(entry, payload)?;
        let (_, local_actor) = cursor.binding().local_validator_identity();
        if local_actor == cursor.binding().producer_actor_projection() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "producer lifecycle cursor cannot claim a replica Queue disposition",
            ));
        }
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                entry,
                payload.origin_proposal.descriptor.lane_block_height,
                pending_canonical_bytes,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lifecycle replica terminal source has no current lane attempt",
                )
            })?;
        if current.artifact.executable_payload == *payload {
            self.require_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                payload,
                retirement,
                queue_disposition,
            )?;
        } else {
            self.require_autonomous_lane_replica_release_completed_or_superseded_locked(
                entry,
                payload,
                retirement,
                queue_disposition,
            )?;
        }
        Ok(queue_disposition)
    }
    fn autonomous_lifecycle_replica_terminal_outcome_is_complete_locked(
        &self,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
        queue_disposition: AutonomousLifecycleReplicaQueueDispositionV1,
    ) -> Result<Option<Hash>> {
        if !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica Complete outcome retirement differs from its payload",
            ));
        }
        let retirement_hash = retirement.digest()?;
        let descriptor = &payload.origin_proposal.descriptor;
        let path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
            entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "replica Complete outcome path has no parent directory",
            )
        })?;
        let Some(bytes) = self.read_regular_sidecar_bytes(
            &path,
            parent,
            AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
        )?
        else {
            return Ok(None);
        };
        let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
        let expected_source =
            AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                retirement_hash,
                queue_disposition,
            };
        if outcome.source() != expected_source {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "replica Complete outcome changed its Queue disposition",
            ));
        }
        outcome
            .validate_for_payload(payload)
            .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(entry, payload)?;
        if cursor.binding() != outcome.binding() {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "replica Complete outcome changed its signed cursor binding",
            ));
        }
        Ok(outcome.is_complete().then_some(outcome.outcome_hash))
    }
    fn read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(
        &self,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
    ) -> Result<AutonomousLifecycleCursorV1> {
        let descriptor = &payload.origin_proposal.descriptor;
        let path = Self::autonomous_lifecycle_cursor_path_for_entry(
            entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "autonomous lifecycle terminal outcome cursor has no parent directory",
            )
        })?;
        let bytes = self
            .read_regular_sidecar_bytes(&path, parent, AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal outcome lacks its signed cursor",
                )
            })?;
        let cursor = Self::decode_autonomous_lifecycle_cursor(&path, &bytes)?;
        cursor
            .validate_for_payload(payload)
            .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
        let process_record = self
            .read_autonomous_lifecycle_process_generation_record()?
            .map(|(record, _)| record)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal outcome lacks a process generation",
                )
            })?;
        Self::validate_autonomous_lifecycle_cursor_process_generation(&process_record, &cursor)
            .map_err(|message| Self::invalid_lane_artifact_error(path, message))?;
        Ok(cursor)
    }
    fn prepare_autonomous_lifecycle_terminal_outcome_pending_locked(
        &self,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<AutonomousLifecycleTerminalPendingPublicationPlan> {
        let descriptor = &payload.origin_proposal.descriptor;
        let bootstrap_path = Self::autonomous_lifecycle_bootstrap_path_for_entry(
            entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let bootstrap_parent = bootstrap_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                bootstrap_path.clone(),
                "autonomous lifecycle bootstrap path has no parent directory",
            )
        })?;
        if self
            .regular_sidecar_metadata(&bootstrap_path, bootstrap_parent)?
            .is_some()
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::WouldBlock,
                    "terminal outcome waits for signed lifecycle bootstrap completion",
                ),
                bootstrap_path,
            ));
        }
        let attempt = self
            .read_autonomous_lane_block_attempt_record_locked(
                entry,
                descriptor.lane_id,
                descriptor.lane_block_height,
                descriptor.proposal_height,
                payload.network_id,
                payload.epoch,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lifecycle terminal outcome lacks its exact payload attempt",
                )
            })?;
        if attempt.artifact.executable_payload != *payload {
            return Err(Self::invalid_lane_artifact_error(
                attempt.view_state_path,
                "autonomous lifecycle terminal outcome payload attempt changed",
            ));
        }
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(entry, payload)?;
        let pending =
            AutonomousLifecycleTerminalOutcomeV1::pending(cursor.binding().clone(), source)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
        let path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
            entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "autonomous lifecycle terminal outcome path has no parent directory",
            )
        })?;
        let current_bytes = self.read_regular_sidecar_bytes(
            &path,
            parent,
            AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
        )?;
        if let Some(bytes) = current_bytes.as_deref() {
            let current = Self::decode_autonomous_lifecycle_terminal_outcome(&path, bytes)?;
            if current.binding() != pending.binding() || current.source() != source {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle terminal outcome conflicts with its durable pending source",
                ));
            }
            current
                .validate_for_payload(payload)
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
            return Ok(AutonomousLifecycleTerminalPendingPublicationPlan {
                entry: entry.clone(),
                identity: (descriptor.lane_block_height, descriptor.proposal_height),
                path,
                outcome: current,
                pending_bytes: None,
            });
        }
        let bytes = pending.encode_framed().map_err(Error::NoritoFrame)?;
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle Pending terminal outcome exceeds its hard byte limit",
            ));
        }
        Ok(AutonomousLifecycleTerminalPendingPublicationPlan {
            entry: entry.clone(),
            identity: (descriptor.lane_block_height, descriptor.proposal_height),
            path,
            outcome: pending,
            pending_bytes: Some(bytes),
        })
    }
    /// Validate the complete terminal Pending write set, including every lane
    /// namespace reservation and the exact configured Kura disk peak, before
    /// the first file is materialized.
    fn preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(
        &self,
        pending_canonical_bytes: u64,
        plans: &[AutonomousLifecycleTerminalPendingPublicationPlan],
    ) -> Result<()> {
        let mut inventories = BTreeMap::<PathBuf, AutonomousLaneAttemptInventoryBudget>::new();
        let mut pending_paths = BTreeSet::new();
        let mut additional_disk_bytes = 0_u64;
        let mut maximum_atomic_bytes = 0_u64;
        for plan in plans {
            let directory = Self::lane_artifact_dir(&plan.entry.blocks_dir(&self.store_root));
            if !inventories.contains_key(&directory) {
                let inventory = self.autonomous_lane_attempt_inventory_counts_locked(
                    &plan.entry,
                    plan.identity.0,
                )?;
                inventories.insert(directory.clone(), inventory);
            }
            let Some(bytes) = plan.pending_bytes.as_deref() else {
                continue;
            };
            if !pending_paths.insert(plan.path.clone()) {
                return Err(Self::invalid_lane_artifact_error(
                    plan.path.clone(),
                    "autonomous lifecycle Pending publication aliases another carrier member",
                ));
            }
            let inventory = inventories
                .get_mut(&directory)
                .expect("lane inventory was inserted above");
            if !inventory.has_reserved_terminal_outcome(plan.identity) {
                return Err(Self::invalid_lane_artifact_error(
                    plan.path.clone(),
                    "autonomous lifecycle Pending publication lacks its admitted terminal-outcome reservation",
                ));
            }
            let next_len = u64::try_from(bytes.len())?;
            Self::validate_autonomous_lifecycle_terminal_outcome_budget(
                inventory.conceptual_files,
                inventory.conceptual_bytes,
                0,
                next_len,
                false,
            )
            .map_err(|message| Self::invalid_lane_artifact_error(plan.path.clone(), message))?;
            inventory.terminal_outcome_identities.insert(plan.identity);
            inventory.conceptual_bytes = inventory
                .conceptual_bytes
                .checked_sub(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64)
                .and_then(|bytes| bytes.checked_add(next_len))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        plan.path.clone(),
                        "autonomous lifecycle Pending reservation consumption overflows",
                    )
                })?;
            additional_disk_bytes =
                additional_disk_bytes.checked_add(next_len).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous lifecycle Pending write-set disk accounting overflows",
                    )
                })?;
            maximum_atomic_bytes = maximum_atomic_bytes.max(next_len);
        }
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            maximum_atomic_bytes,
            false,
            true,
            &self.store_root,
        )?;
        if additional_disk_bytes != 0 {
            self.kura_total_disk_usage_bytes()?
                .checked_add(additional_disk_bytes)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous lifecycle Pending total-disk accounting overflows",
                    )
                })?;
        }
        Ok(())
    }
    #[cfg(test)]
    fn autonomous_lifecycle_terminal_reservation_budget_for_tests(
        &self,
        lane_id: LaneId,
        identity: (u64, u64),
    ) -> Result<(bool, usize, u64)> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let inventory = self.autonomous_lane_attempt_inventory_counts_locked(&entry, identity.0)?;
        Ok((
            inventory.has_reserved_terminal_outcome(identity),
            inventory.conceptual_files,
            inventory.conceptual_bytes,
        ))
    }
    fn publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(
        &self,
        pending_canonical_bytes: u64,
        plan: &AutonomousLifecycleTerminalPendingPublicationPlan,
    ) -> Result<()> {
        if let Some(bytes) = plan.pending_bytes.as_deref() {
            let next_len = u64::try_from(bytes.len())?;
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            if !self.write_atomic_synced_noclobber(&plan.path, bytes)? {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "autonomous lifecycle terminal outcome appeared during pending publication",
                    ),
                    plan.path.clone(),
                ));
            }
            self.update_disk_usage_delta(0, next_len);
            self.update_total_disk_usage_delta(0, next_len);
            accounting_mutation.finish();
        }
        self.reconcile_post_wsv_lane_artifact_budget_for_terminal_outcome_locked(
            pending_canonical_bytes,
            &plan.outcome,
        )?;
        Ok(())
    }
    fn persist_autonomous_lifecycle_terminal_outcome_pending_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<AutonomousLifecycleTerminalOutcomeV1> {
        let plan = self
            .prepare_autonomous_lifecycle_terminal_outcome_pending_locked(entry, payload, source)?;
        self.preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(
            pending_canonical_bytes,
            std::slice::from_ref(&plan),
        )?;
        self.publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(
            pending_canonical_bytes,
            &plan,
        )?;
        Ok(plan.outcome)
    }
    /// Publish one replica terminal outcome directly as `Complete` while the
    /// caller retains Queue's move-only per-hash disposition fence.
    ///
    /// A missing file is created atomically in its final state, so no new
    /// crash-visible Pending observer outcome exists. A matching defensive
    /// Pending record is completed with the same fixed-length replacement;
    /// an exact Complete retry is a stutter.
    fn persist_autonomous_lifecycle_replica_terminal_outcome_complete_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        payload: &LaneExecutablePayloadV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
        terminal: ProductionInFlightFirstReleaseStateProjection,
    ) -> Result<AutonomousLifecycleTerminalOutcomeV1> {
        if !matches!(
            source,
            AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition { .. }
        ) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "direct Complete publication requires a replica Queue terminal source",
            ));
        }
        let mut plan = self
            .prepare_autonomous_lifecycle_terminal_outcome_pending_locked(entry, payload, source)?;
        if let Some(existing) = plan
            .outcome
            .terminal_projection()
            .map_err(|message| Self::invalid_lane_artifact_error(plan.path.clone(), message))?
        {
            if existing == terminal {
                return Ok(plan.outcome);
            }
            return Err(Self::invalid_lane_artifact_error(
                plan.path,
                "replica lifecycle terminal outcome conflicts with its existing Complete state",
            ));
        }
        let complete = plan
            .outcome
            .complete(terminal)
            .map_err(|message| Self::invalid_lane_artifact_error(plan.path.clone(), message))?;
        let next_bytes = complete.encode_framed().map_err(Error::NoritoFrame)?;
        // Preflight every raw->sealed claim replacement before publishing
        // Complete. The terminal path retains its own bounded CAS check below;
        // a missing path is already represented by its admitted terminal-file
        // reservation, while an existing Pending file is already baseline.
        // Keeping this projection claim-only also avoids misclassifying the
        // cumulative group growth as one shared terminal CAS transient.
        let capacity_path = self.store_root.join("blocks");
        let mut capacity = AutonomousClaimMutationPeak::default();
        let AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
            retirement_hash,
            queue_disposition,
        } = source
        else {
            unreachable!("replica source was checked above");
        };
        for entrypoint_hash in &payload.entrypoint_hashes {
            let claim_path = Self::autonomous_lane_entrypoint_claim_path(
                &self.store_root,
                &payload.network_id,
                entrypoint_hash,
            );
            let existing =
                Self::decode_autonomous_lane_entrypoint_claim(&claim_path).map_err(|message| {
                    Self::invalid_lane_artifact_error(claim_path.clone(), message)
                })?;
            let released = AutonomousLaneEntrypointClaimV1::replica_released_for_payload(
                payload,
                *entrypoint_hash,
                retirement_hash,
                queue_disposition,
            );
            if existing != released
                || !self.autonomous_lane_entrypoint_claim_path_matches(&existing, &claim_path)
            {
                return Err(Self::invalid_lane_artifact_error(
                    claim_path,
                    "replica Complete preflight requires the exact raw released claim group",
                ));
            }
            let sealed = AutonomousLaneEntrypointClaimV1::replica_released_complete_for_payload(
                payload,
                *entrypoint_hash,
                retirement_hash,
                queue_disposition,
                complete.outcome_hash,
            );
            let sealed_bytes = norito::encode_canonical(&sealed).map_err(Error::NoritoFrame)?;
            if sealed_bytes.is_empty()
                || sealed_bytes.len() > AUTONOMOUS_LANE_ENTRYPOINT_CLAIM_MAX_BYTES
            {
                return Err(Self::invalid_lane_artifact_error(
                    claim_path,
                    "sealed replica claim exceeds its hard byte limit",
                ));
            }
            capacity
                .atomic_replace(
                    Self::file_len_or_zero(&claim_path)?,
                    u64::try_from(sealed_bytes.len())?,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(capacity_path.clone(), message)
                })?;
        }
        let additional_peak_bytes = capacity
            .additional_peak_bytes()
            .map_err(|message| Self::invalid_lane_artifact_error(capacity_path.clone(), message))?;
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            additional_peak_bytes,
            false,
            false,
            &capacity_path,
        )?;
        if plan.pending_bytes.is_some() {
            plan.outcome = complete.clone();
            plan.pending_bytes = Some(next_bytes);
            self.preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(
                pending_canonical_bytes,
                std::slice::from_ref(&plan),
            )?;
            self.publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(
                pending_canonical_bytes,
                &plan,
            )?;
            return Ok(complete);
        }
        let parent = plan.path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                plan.path.clone(),
                "replica terminal completion path has no parent directory",
            )
        })?;
        let current_bytes = self
            .read_regular_sidecar_bytes(
                &plan.path,
                parent,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    plan.path.clone(),
                    "replica terminal Pending outcome disappeared before completion",
                )
            })?;
        let current =
            Self::decode_autonomous_lifecycle_terminal_outcome(&plan.path, &current_bytes)?;
        if current != plan.outcome || next_bytes.len() != current_bytes.len() {
            return Err(Self::invalid_lane_artifact_error(
                plan.path,
                "replica terminal Pending outcome changed before fixed-length completion",
            ));
        }
        let inventory = self.autonomous_lane_attempt_inventory_counts_locked(
            entry,
            payload.origin_proposal.descriptor.lane_block_height,
        )?;
        let previous_len = u64::try_from(current_bytes.len())?;
        let next_len = u64::try_from(next_bytes.len())?;
        Self::validate_autonomous_lifecycle_terminal_outcome_budget(
            inventory.conceptual_files,
            inventory.conceptual_bytes,
            previous_len,
            next_len,
            true,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(plan.path.clone(), message))?;
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            next_len,
            false,
            true,
            &plan.path,
        )?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        self.write_atomic_synced_replace(&plan.path, &next_bytes)?;
        self.update_disk_usage_delta(previous_len, next_len);
        self.update_total_disk_usage_delta(previous_len, next_len);
        accounting_mutation.finish();
        Ok(complete)
    }
    /// Materialize and revalidate the complete source-outcome set for one
    /// canonical merge carrier while all Kura ordering locks are held.
    ///
    /// Missing members are durably published as Pending before this function
    /// returns any Queue authority. Existing source-equivalent Complete
    /// members are retained and reported separately; callers choose whether
    /// their Queue authorization vector includes those idempotent members.
    fn canonical_carrier_source_outcome_set_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &MergeLedgerEntry,
        include_complete_authorizations: bool,
    ) -> Result<(
        Vec<(
            LaneQueueReservationGroupBindingV1,
            AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
        )>,
        Vec<LaneQueueReservationGroupBindingV1>,
        MergeLedgerCarrierRecord,
        iroha_data_model::NetworkId,
    )> {
        self.durable_mutation_authorized()?;
        let entry_hash = crate::merge::merge_ledger_entry_hash(entry);
        if self.merge_log.lock().entry_by_hash(entry_hash)?.as_ref() != Some(entry) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle source-outcome set names an uncommitted merge entry",
            ));
        }
        let batch = entry.execution_batch.as_ref().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle source-outcome set has no execution batch",
            )
        })?;
        if batch.lanes.is_empty() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle source-outcome set has no execution lanes",
            ));
        }
        let carrier = self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome set lost its exact carrier",
                )
            })?;
        if carrier.version != 1 || carrier.epoch_id != entry.epoch_id {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle source-outcome carrier has a stale version or epoch",
            ));
        }
        self.ensure_post_wsv_lane_artifact_budget_reservation_locked(
            pending_canonical_bytes,
            entry,
            carrier.block_height,
            carrier.block_hash,
        )?;
        let mut queue_authorizations = Vec::new();
        queue_authorizations.try_reserve_exact(batch.lanes.len())?;
        let mut complete_reservation_groups = Vec::new();
        complete_reservation_groups.try_reserve_exact(batch.lanes.len())?;
        let mut terminal_publication_plans = Vec::new();
        terminal_publication_plans.try_reserve_exact(batch.lanes.len())?;
        let mut seen_groups = BTreeSet::new();
        let mut expected_network_id = None;
        for execution in &batch.lanes {
            let bundle = Self::decode_autonomous_lane_merge_bundle(
                &execution.source_bundle,
                execution.autonomous_network_id,
                execution.autonomous_epoch,
            )
            .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?;
            let payload = bundle.executable_payload();
            if expected_network_id
                .replace(payload.network_id)
                .is_some_and(|expected| expected != payload.network_id)
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome set mixes chain identities",
                ));
            }
            let descriptor = &payload.origin_proposal.descriptor;
            let lane_entry = self.lane_storage_entry(descriptor.lane_id)?;
            self.require_active_lane_artifact(&lane_entry, descriptor)?;
            let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
                entry,
                batch,
                execution,
                Self::merge_lane_block_execution_source(execution),
                carrier.block_height,
                carrier.block_hash,
            );
            let source = Self::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
            let durable_receipt = self
                .autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                    payload, source,
                )?;
            if durable_receipt != receipt {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome receipt changed during full-set validation",
                ));
            }
            let publication_plan = self
                .prepare_autonomous_lifecycle_terminal_outcome_pending_locked(
                    &lane_entry,
                    payload,
                    source,
                )?;
            let outcome = &publication_plan.outcome;
            outcome.validate_for_payload(payload).map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?;
            if outcome.source() != source {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome member changed its exact source",
                ));
            }
            let cursor = self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(
                &lane_entry,
                payload,
            )?;
            if cursor.binding() != outcome.binding() {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome member changed its signed cursor binding",
                ));
            }
            let reservation_group = outcome.binding().reservation_group_binding();
            if lane_queue_reservation_group_binding_from_ordered_keys(
                payload.reservation_keys.iter(),
            )
            .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })? != reservation_group
                || !seen_groups.insert(reservation_group.reservation_group_hash)
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome set has a duplicate or changed reservation group",
                ));
            }
            let is_complete = outcome.is_complete();
            if is_complete {
                complete_reservation_groups.push(reservation_group);
            }
            if !is_complete || include_complete_authorizations {
                queue_authorizations.push((
                    reservation_group,
                    AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization {
                        reservation_group,
                        ordered_keys: payload.reservation_keys.clone(),
                        source_outcome_hash: outcome.outcome_hash,
                    },
                ));
            }
            terminal_publication_plans.push(publication_plan);
        }
        let expected_network_id = expected_network_id.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle source-outcome set has no network identity",
            )
        })?;
        self.preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(
            pending_canonical_bytes,
            &terminal_publication_plans,
        )?;
        for publication_plan in &terminal_publication_plans {
            self.publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(
                pending_canonical_bytes,
                publication_plan,
            )?;
        }
        Ok((
            queue_authorizations,
            complete_reservation_groups,
            carrier,
            expected_network_id,
        ))
    }
    /// Persist and source-authenticate the complete durable source-outcome set
    /// for an exact committed merge entry.
    ///
    /// The returned move-only publication covers every execution lane in
    /// canonical batch order. A first attempt binds Pending record hashes; an
    /// idempotent retry binds the current source-equivalent Complete hashes.
    /// Queue must receive this whole set before mutating any reservation group.
    pub(crate) fn persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(
        &self,
        entry: &MergeLedgerEntry,
    ) -> Result<Option<AutonomousLifecycleCanonicalCarrierSourceOutcomePublication>> {
        let entry_hash = crate::merge::merge_ledger_entry_hash(entry);
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        if entry.execution_batch.is_none() {
            if self.merge_log.lock().entry_by_hash(entry_hash)?.as_ref() != Some(entry) {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle source-outcome entry is not committed",
                ));
            }
            return Ok(None);
        }
        let expected_count = entry
            .execution_batch
            .as_ref()
            .map_or(0, |batch| batch.lanes.len());
        let (queue_authorizations, complete_reservation_groups, _, _) =
            self.canonical_carrier_source_outcome_set_locked(pending_canonical_bytes, entry, true)?;
        if queue_authorizations.len() != expected_count
            || complete_reservation_groups.len() > expected_count
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle live publication does not cover the complete carrier set",
            ));
        }
        Ok(Some(
            AutonomousLifecycleCanonicalCarrierSourceOutcomePublication {
                entry_hash,
                queue_authorizations,
            },
        ))
    }
    /// Reconstruct a complete canonical carrier source-outcome set from one
    /// exact committed reservation group when startup has no outcome-file seed.
    ///
    /// The group selects only the durability-attested receipt and committed
    /// merge entry. The returned publication still covers every carrier lane;
    /// callers deduplicate publications by entry hash and authenticate the
    /// complete set before any Queue mutation.
    pub(crate) fn reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(
        &self,
        reservation_group: &LaneQueueReservationGroupBindingV1,
    ) -> Result<AutonomousLifecycleCanonicalCarrierSourceOutcomePublication> {
        let identity = reservation_group.identity;
        let receipt = self
            .read_lane_block_application_receipt_without_sidecar_repair(
                identity.lane_id,
                identity.lane_block_height,
            )
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle startup reconstruction lacks a durable application receipt",
                )
            })?;
        let descriptor = &receipt.proposal.descriptor;
        if descriptor.lane_id != identity.lane_id
            || descriptor.dataspace_id != identity.dataspace_id
            || descriptor.lane_incarnation != identity.lane_incarnation
            || descriptor.proposal_height != identity.proposal_height
            || descriptor.lane_block_height != identity.lane_block_height
            || descriptor.lane_block_view != identity.lane_block_view
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle startup receipt names another route or attempt",
            ));
        }
        let source = Self::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
            .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?;
        let AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
            merge_entry_hash, ..
        } = source
        else {
            unreachable!("merge receipt always constructs a canonical terminal source")
        };
        let canonical_entry = self
            .merge_log
            .lock()
            .entry_by_hash(merge_entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical lifecycle startup reconstruction lost its merge entry",
                )
            })?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        let expected_count = canonical_entry
            .execution_batch
            .as_ref()
            .map_or(0, |batch| batch.lanes.len());
        let (queue_authorizations, complete_reservation_groups, _, _) = self
            .canonical_carrier_source_outcome_set_locked(
                pending_canonical_bytes,
                &canonical_entry,
                true,
            )?;
        if queue_authorizations.len() != expected_count
            || complete_reservation_groups.len() > expected_count
            || !queue_authorizations
                .iter()
                .any(|(group, _)| group == reservation_group)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "canonical lifecycle startup reconstruction does not cover the selecting group and full carrier",
            ));
        }
        Ok(
            AutonomousLifecycleCanonicalCarrierSourceOutcomePublication {
                entry_hash: merge_entry_hash,
                queue_authorizations,
            },
        )
    }
    /// Persist a release Pending outcome after exact claims are Released and
    /// before Queue publishes FIFO ownership and forgets its barrier.
    pub(crate) fn persist_autonomous_lifecycle_release_terminal_outcome_pending(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization> {
        if retirement.network_id != expected_network_id || retirement.epoch != expected_epoch {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "release lifecycle pending outcome has the wrong chain context",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(retirement.lane_id)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                retirement.lane_id,
                retirement.lane_block_height,
                retirement.proposal_height,
                expected_network_id,
                expected_epoch,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "release lifecycle pending outcome lacks its exact attempt",
                )
            })?;
        if record.retirement.as_ref() != Some(retirement) {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "release lifecycle pending outcome differs from its durable retirement",
            ));
        }
        let payload = &record.artifact.executable_payload;
        let source = AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
            retirement_hash: retirement.digest()?,
        };
        self.autonomous_lifecycle_terminal_source_matches_release_locked(
            Some(pending_canonical_bytes),
            &entry,
            payload,
            record.retirement.as_ref(),
            source,
        )?;
        let outcome = self.persist_autonomous_lifecycle_terminal_outcome_pending_locked(
            pending_canonical_bytes,
            &entry,
            payload,
            source,
        )?;
        let barrier = retirement.queue_release_barrier()?;
        if outcome.binding().reservation_group_binding()
            != lane_queue_reservation_group_binding_from_ordered_keys(barrier.ordered_keys.iter())
                .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "release lifecycle source-outcome changed its exact reservation group",
            ));
        }
        Ok(AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization {
            barrier,
            source_outcome_hash: outcome.outcome_hash,
        })
    }
    /// Directly prove that every caller-expected terminal outcome still exists
    /// at its exact durable path, revalidates against its payload/cursor/source,
    /// and is either Pending or Complete.
    ///
    /// This bounded batch deliberately does not reconstruct missing canonical
    /// members. Startup uses it on both sides of remaining Pending completion,
    /// so deletion cannot be mistaken for an already-Complete outcome.
    pub(crate) fn verify_expected_autonomous_lifecycle_terminal_outcome_stages(
        &self,
        expected_network_id: iroha_data_model::NetworkId,
        expected_groups: &[AutonomousLifecyclePendingReservationGroupObservation],
    ) -> Result<Vec<AutonomousLifecycleTerminalOutcomeStageObservation>> {
        if expected_network_id.as_bytes().iter().all(|byte| *byte == 0)
            || expected_groups.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "expected autonomous lifecycle terminal outcome batch has a zero chain or exceeds its hard bound",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let mut seen_group_hashes = BTreeMap::new();
        let mut seen_group_identities = BTreeMap::new();
        let mut seen_transaction_hashes = BTreeMap::new();
        let mut seen_entrypoint_hashes = BTreeSet::new();
        let mut preflighted = Vec::new();
        preflighted.try_reserve_exact(expected_groups.len())?;
        for expected in expected_groups {
            let expected_group = expected.binding();
            let expected_keys = expected.ordered_keys();
            let recomputed_group =
                lane_queue_reservation_group_binding_from_ordered_keys(expected_keys.iter())
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                    })?;
            if recomputed_group != expected_group
                || seen_group_hashes
                    .insert(expected_group.reservation_group_hash, expected_group)
                    .is_some()
                || seen_group_identities
                    .insert(expected_group.identity, expected_group)
                    .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "expected autonomous lifecycle terminal outcome batch is malformed, duplicated, or colliding",
                ));
            }
            for key in expected_keys {
                if seen_transaction_hashes
                    .insert(key.entrypoint_hash, *key)
                    .is_some()
                    || !seen_entrypoint_hashes.insert(key.entrypoint_hash.clone())
                {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "expected autonomous lifecycle terminal outcomes overlap one transaction or entrypoint identity",
                    ));
                }
            }
            let identity = expected_group.identity;
            let entry = self.lane_storage_entry(identity.lane_id)?;
            let (active_incarnation, activation_height) =
                self.active_lane_incarnation_marker(&entry)?;
            let path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
                &entry,
                &self.store_root,
                identity.lane_block_height,
                identity.proposal_height,
            );
            if entry.dataspace_id != identity.dataspace_id
                || active_incarnation != identity.lane_incarnation
                || identity.proposal_height <= activation_height
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "expected autonomous lifecycle terminal outcome targets stale lane geometry",
                ));
            }
            preflighted.push((expected, expected_group, entry, path));
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        let mut verified = Vec::new();
        verified.try_reserve_exact(preflighted.len())?;
        for (expected, expected_group, entry, path) in preflighted {
            let identity = expected_group.identity;
            let parent = path.parent().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "expected autonomous lifecycle terminal outcome path has no parent directory",
                )
            })?;
            let bytes = self
                .read_regular_sidecar_bytes(
                    &path,
                    parent,
                    AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                )?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "expected autonomous lifecycle terminal outcome is missing",
                    )
                })?;
            let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
            let binding = outcome.binding();
            if binding.network_id != expected_network_id
                || binding.reservation_group_binding() != expected_group
                || binding.route_identity()
                    != (
                        identity.lane_id,
                        identity.dataspace_id,
                        identity.lane_incarnation,
                    )
                || binding.attempt_coordinates()
                    != (
                        identity.proposal_height,
                        identity.lane_block_height,
                        identity.lane_block_view,
                    )
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "expected autonomous lifecycle terminal outcome changed its exact binding",
                ));
            }
            let record = self
                .read_autonomous_lane_block_attempt_record_locked(
                    &entry,
                    identity.lane_id,
                    identity.lane_block_height,
                    identity.proposal_height,
                    binding.network_id,
                    binding.epoch,
                    None,
                )?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "expected autonomous lifecycle terminal outcome lost its payload attempt",
                    )
                })?;
            let payload = &record.artifact.executable_payload;
            if payload.reservation_keys.as_slice() != expected.ordered_keys() {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "expected autonomous lifecycle terminal outcome changed its ordered reservation bytes",
                ));
            }
            outcome
                .validate_for_payload(payload)
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
            let cursor =
                self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(&entry, payload)?;
            if cursor.binding() != binding {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "expected autonomous lifecycle terminal outcome changed its signed cursor binding",
                ));
            }
            let source_kind = match outcome.source() {
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier { .. } => {
                    self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                        payload, source,
                    )?;
                    AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
                }
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease { .. } => {
                    self.autonomous_lifecycle_terminal_source_matches_release_locked(
                        Some(pending_canonical_bytes),
                        &entry,
                        payload,
                        record.retirement.as_ref(),
                        source,
                    )?;
                    AutonomousLifecycleTerminalOutcomeSourceKind::RetiredRelease
                }
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                    ..
                } => {
                    self.autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                        Some(pending_canonical_bytes),
                        &entry,
                        payload,
                        record.retirement.as_ref(),
                        source,
                    )?;
                    AutonomousLifecycleTerminalOutcomeSourceKind::RetiredReplicaQueueDisposition
                }
            };
            let stage = if outcome.is_complete() {
                AutonomousLifecycleTerminalOutcomeDurableStage::Complete
            } else {
                AutonomousLifecycleTerminalOutcomeDurableStage::Pending
            };
            verified.push(AutonomousLifecycleTerminalOutcomeStageObservation {
                binding: expected_group,
                source_kind,
                stage,
            });
        }
        Ok(verified)
    }
    /// Return every source-revalidated Pending outcome across active lane
    /// segments in deterministic carrier/route order.
    ///
    /// Any observed canonical member causes the complete carrier set to be
    /// reconstructed and durably materialized before this function returns.
    /// Canonical recovery therefore contains every actual Pending member plus
    /// the disjoint Complete members that startup validates without mutation.
    pub(crate) fn pending_autonomous_lifecycle_terminal_outcome_inventory(
        &self,
    ) -> Result<Vec<AutonomousLifecyclePendingTerminalOutcomeRecovery>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let mut entries = self
            .lane_storage_entries
            .lock()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            Self::lane_artifact_dir(&left.blocks_dir(&self.store_root)).cmp(
                &Self::lane_artifact_dir(&right.blocks_dir(&self.store_root)),
            )
        });
        let _sidecar_guard = self.sidecar_lock.lock();
        let mut canonical_entries = BTreeMap::new();
        let mut release_recoveries = Vec::new();
        let mut validated_release_groups = Vec::new();
        let mut observed_groups = BTreeMap::new();
        let mut outcomes_seen = 0_usize;
        for entry in entries {
            let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
            let directory_entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::IO(error, directory)),
            };
            // Reuse the complete autonomous namespace validator so malformed,
            // temporary, linked, oversized, or unexpected siblings cannot be
            // hidden by scanning only the new prefix.
            let _ = self.autonomous_lane_attempt_inventory_counts_locked(&entry, 1)?;
            let mut outcomes = BTreeMap::new();
            for directory_entry in directory_entries {
                let directory_entry =
                    directory_entry.map_err(|error| Error::IO(error, directory.clone()))?;
                let path = directory_entry.path();
                let name = directory_entry.file_name().into_string().map_err(|_| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "autonomous lifecycle terminal inventory contains a non-UTF-8 artifact",
                    )
                })?;
                let Some(identity) = Self::autonomous_lifecycle_terminal_outcome_coordinates(&name)
                else {
                    if name.starts_with("autonomous_lifecycle_terminal_outcome") {
                        return Err(Self::invalid_lane_artifact_error(
                            path,
                            "autonomous lifecycle terminal inventory found a malformed or legacy path",
                        ));
                    }
                    continue;
                };
                outcomes_seen = outcomes_seen.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        directory.clone(),
                        "autonomous lifecycle terminal inventory count overflows",
                    )
                })?;
                if outcomes_seen > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "autonomous lifecycle terminal inventory exceeds its global startup bound",
                    ));
                }
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle terminal outcome disappeared during inventory",
                        )
                    })?;
                let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
                if outcomes.insert(identity, (path, outcome)).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "autonomous lifecycle terminal inventory has duplicate coordinates",
                    ));
                }
            }
            for ((lane_block_height, proposal_height), (path, outcome)) in outcomes {
                let binding = outcome.binding();
                let (active_incarnation, activation_height) =
                    self.active_lane_incarnation_marker(&entry)?;
                if binding.lane_id != entry.lane_id
                    || binding.dataspace_id != entry.dataspace_id
                    || binding.lane_incarnation != active_incarnation
                    || binding.proposal_height <= activation_height
                    || binding.lane_block_height != lane_block_height
                    || binding.proposal_height != proposal_height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle terminal inventory targets stale lane geometry",
                    ));
                }
                let record = self
                    .read_autonomous_lane_block_attempt_record_locked(
                        &entry,
                        binding.lane_id,
                        binding.lane_block_height,
                        binding.proposal_height,
                        binding.network_id,
                        binding.epoch,
                        None,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle terminal outcome is orphaned from its payload attempt",
                        )
                    })?;
                let payload = &record.artifact.executable_payload;
                outcome
                    .validate_for_payload(payload)
                    .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                let cursor = self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(
                    &entry, payload,
                )?;
                if cursor.binding() != binding {
                    return Err(Self::invalid_lane_artifact_error(
                        path.clone(),
                        "autonomous lifecycle terminal inventory cursor binding changed",
                    ));
                }
                match outcome.source() {
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
                        ..
                    } => {
                        self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                            payload, source,
                        )?;
                    }
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
                        ..
                    } => {
                        self.autonomous_lifecycle_terminal_source_matches_release_locked(
                            Some(pending_canonical_bytes),
                            &entry,
                            payload,
                            record.retirement.as_ref(),
                            source,
                        )?;
                    }
                    source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                        ..
                    } => {
                        let queue_disposition = self
                            .autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                                Some(pending_canonical_bytes),
                                &entry,
                                payload,
                                record.retirement.as_ref(),
                                source,
                            )?;
                        if outcome.is_complete() {
                            let retirement = record.retirement.as_ref().ok_or_else(|| {
                                Self::invalid_lane_artifact_error(
                                    path.clone(),
                                    "Complete replica Queue disposition lost its exact retirement",
                                )
                            })?;
                            self.complete_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                                pending_canonical_bytes,
                                Some(pending_canonical_bytes),
                                payload,
                                retirement,
                                queue_disposition,
                                &outcome,
                            )?;
                        }
                    }
                }
                let group = binding.reservation_group_binding();
                if observed_groups
                    .insert(group.reservation_group_hash, (group, path.clone()))
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path.clone(),
                        "autonomous lifecycle terminal inventory has duplicate reservation groups",
                    ));
                }
                match outcome.source() {
                    AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
                        merge_entry_hash,
                        ..
                    } => {
                        let observed = canonical_entries
                            .entry(merge_entry_hash)
                            .or_insert_with(|| (path.clone(), 0_usize));
                        observed.1 = observed.1.checked_add(1).ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                path.clone(),
                                "canonical lifecycle terminal carrier member count overflows",
                            )
                        })?;
                    }
                    AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease { .. } => {
                        validated_release_groups.push((group, path.clone()));
                        if outcome.is_complete() {
                            continue;
                        }
                        let retirement = record.retirement.as_ref().ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                path.clone(),
                                "Pending release outcome lost its exact retirement",
                            )
                        })?;
                        let barrier = retirement.queue_release_barrier()?;
                        let finalization = AutonomousLaneReleaseProjectionContext::from_payload(
                            self, payload, retirement,
                        )
                        .and_then(|context| {
                            context.queue_finalization_authorization(retirement, &barrier)
                        })
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(path.clone(), message)
                        })?;
                        release_recoveries.try_reserve(1)?;
                        release_recoveries.push(
                            AutonomousLifecyclePendingTerminalOutcomeRecovery::RetiredRelease {
                                barrier: barrier.clone(),
                                finalization,
                                source_outcome_authorization:
                                    AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization {
                                        barrier,
                                        source_outcome_hash: outcome.outcome_hash,
                                    },
                            },
                        );
                    }
                    AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                        ..
                    } => {
                        validated_release_groups.push((group, path.clone()));
                        if outcome.is_complete() {
                            continue;
                        }
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::WouldBlock,
                                "Pending replica Queue disposition requires a freshly reacquired exact Queue fence",
                            ),
                            path,
                        ));
                    }
                }
            }
        }
        // Rebuild every complete carrier set before yielding any Queue
        // authority. This closes a crash after only a strict subset of its
        // per-lane Pending files reached durable storage.
        let recovery_capacity = canonical_entries
            .len()
            .checked_add(release_recoveries.len())
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lifecycle recovery unit count overflows",
                )
            })?;
        let mut recovered = Vec::new();
        recovered.try_reserve_exact(recovery_capacity)?;
        let mut validated_groups = BTreeMap::new();
        for (group, path) in validated_release_groups {
            if validated_groups
                .insert(group.reservation_group_hash, group)
                .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle terminal inventory aliases a release reservation group",
                ));
            }
        }
        let mut expanded_outcomes_seen = outcomes_seen;
        let mut canonical_carriers = Vec::new();
        canonical_carriers.try_reserve_exact(canonical_entries.len())?;
        for (merge_entry_hash, (path, observed_member_count)) in canonical_entries {
            let canonical_entry = self
                .merge_log
                .lock()
                .entry_by_hash(merge_entry_hash)?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "canonical terminal outcome lost its exact merge entry",
                    )
                })?;
            let expected_member_count = canonical_entry
                .execution_batch
                .as_ref()
                .map_or(0, |batch| batch.lanes.len());
            let missing_member_count = expected_member_count
                .checked_sub(observed_member_count)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "canonical lifecycle terminal carrier has more observed outcomes than execution members",
                    )
                })?;
            expanded_outcomes_seen = expanded_outcomes_seen
                .checked_add(missing_member_count)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "expanded autonomous lifecycle terminal inventory count overflows",
                    )
                })?;
            if expanded_outcomes_seen > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(Self::invalid_lane_artifact_error(
                    path.clone(),
                    "expanded autonomous lifecycle terminal inventory exceeds its global startup bound",
                ));
            }
            canonical_carriers.push((path, canonical_entry));
        }
        for (path, canonical_entry) in canonical_carriers {
            let (
                pending_queue_authorizations,
                complete_reservation_groups,
                carrier,
                expected_network_id,
            ) = self.canonical_carrier_source_outcome_set_locked(
                pending_canonical_bytes,
                &canonical_entry,
                false,
            )?;
            for group in pending_queue_authorizations
                .iter()
                .map(|(group, _)| group)
                .chain(complete_reservation_groups.iter())
            {
                if validated_groups
                    .insert(group.reservation_group_hash, *group)
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path.clone(),
                        "canonical carrier source-outcome set aliases another reservation group",
                    ));
                }
            }
            if !pending_queue_authorizations.is_empty() {
                recovered.push(
                    AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(
                        AutonomousLifecyclePendingCanonicalCarrierRecovery {
                            pending_queue_authorizations,
                            complete_reservation_groups,
                            reference: CertifiedMergeLedgerReference::new(&canonical_entry),
                            entry: canonical_entry,
                            carrier_block_height: carrier.block_height,
                            carrier_block_hash: carrier.block_hash,
                            expected_network_id,
                        },
                    ),
                );
            }
        }
        recovered.extend(release_recoveries);
        Ok(recovered)
    }
    fn complete_autonomous_lifecycle_terminal_outcome(
        &self,
        reservation_group: LaneQueueReservationGroupBindingV1,
        terminal: ProductionInFlightFirstReleaseStateProjection,
        canonical: bool,
        expected_source_outcome_hash: Hash,
    ) -> Result<()> {
        let identity = reservation_group.identity;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(identity.lane_id)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
            &entry,
            &self.store_root,
            identity.lane_block_height,
            identity.proposal_height,
        );
        let parent = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "autonomous lifecycle terminal completion path has no parent directory",
            )
        })?;
        let current_bytes = self
            .read_regular_sidecar_bytes(
                &path,
                parent,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal completion has no durable source outcome",
                )
            })?;
        let current = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &current_bytes)?;
        if expected_source_outcome_hash
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
            || current.outcome_hash != expected_source_outcome_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle terminal completion no longer names the exact current source outcome",
            ));
        }
        let binding = current.binding();
        if binding.reservation_group_binding() != reservation_group
            || binding.route_identity()
                != (
                    identity.lane_id,
                    identity.dataspace_id,
                    identity.lane_incarnation,
                )
            || binding.attempt_coordinates()
                != (
                    identity.proposal_height,
                    identity.lane_block_height,
                    identity.lane_block_view,
                )
            || current.source().is_canonical_carrier() != canonical
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle terminal Queue evidence names another source outcome",
            ));
        }
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                identity.lane_id,
                identity.lane_block_height,
                identity.proposal_height,
                binding.network_id,
                binding.epoch,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal completion lost its exact payload attempt",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        current
            .validate_for_payload(payload)
            .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(&entry, payload)?;
        if cursor.binding() != binding {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle terminal completion cursor binding changed",
            ));
        }
        if canonical {
            self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                payload,
                current.source(),
            )?;
        } else {
            self.autonomous_lifecycle_terminal_source_matches_release_locked(
                Some(pending_canonical_bytes),
                &entry,
                payload,
                record.retirement.as_ref(),
                current.source(),
            )?;
        }
        if let Some(existing) = current
            .terminal_projection()
            .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?
        {
            if existing == terminal {
                if canonical {
                    self.reconcile_post_wsv_lane_artifact_budget_for_terminal_outcome_locked(
                        pending_canonical_bytes,
                        &current,
                    )?;
                }
                return Ok(());
            }
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle terminal completion conflicts with existing terminal state",
            ));
        }
        let complete = current
            .complete(terminal)
            .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
        let next_bytes = complete.encode_framed().map_err(Error::NoritoFrame)?;
        if next_bytes.len() != current_bytes.len() {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "autonomous lifecycle terminal stage changed its fixed framed length",
            ));
        }
        let inventory = self
            .autonomous_lane_attempt_inventory_counts_locked(&entry, identity.lane_block_height)?;
        let previous_len = u64::try_from(current_bytes.len())?;
        let next_len = u64::try_from(next_bytes.len())?;
        Self::validate_autonomous_lifecycle_terminal_outcome_budget(
            inventory.conceptual_files,
            inventory.conceptual_bytes,
            previous_len,
            next_len,
            true,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            next_len,
            false,
            true,
            &path,
        )?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        self.write_atomic_synced_replace(&path, &next_bytes)?;
        self.update_disk_usage_delta(previous_len, next_len);
        self.update_total_disk_usage_delta(previous_len, next_len);
        accounting_mutation.finish();
        if canonical {
            self.reconcile_post_wsv_lane_artifact_budget_for_terminal_outcome_locked(
                pending_canonical_bytes,
                &complete,
            )?;
        }
        Ok(())
    }
    /// Join canonical Queue terminal ownership to the exact current durable
    /// merge-carrier source outcome.
    pub(crate) fn complete_autonomous_lifecycle_canonical_terminal_outcome(
        &self,
        evidence: AutonomousLaneCanonicalQueueTerminalEvidence,
    ) -> Result<()> {
        let (group, terminal, expected_source_outcome_hash) =
            evidence.consume_for_kura().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "canonical Queue terminal evidence is malformed",
                )
            })?;
        self.complete_autonomous_lifecycle_terminal_outcome(
            group,
            terminal,
            true,
            expected_source_outcome_hash,
        )
    }
    /// Join restored-FIFO Queue terminal ownership to the exact current
    /// retired-release source outcome.
    pub(crate) fn complete_autonomous_lifecycle_release_terminal_outcome(
        &self,
        evidence: AutonomousLaneReleaseQueueTerminalEvidence,
    ) -> Result<()> {
        let (group, terminal, expected_source_outcome_hash) =
            evidence.consume_for_kura().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "release Queue terminal evidence is malformed",
                )
            })?;
        self.complete_autonomous_lifecycle_terminal_outcome(
            group,
            terminal,
            false,
            expected_source_outcome_hash,
        )
    }
    /// Decode one retained terminal outcome while auditing archived lifecycle evidence.
    fn audit_autonomous_lifecycle_terminal_outcome_locked(
        &self,
        path: &Path,
        parent: &Path,
    ) -> Result<()> {
        let bytes = self
            .read_regular_sidecar_bytes(
                path,
                parent,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "retained autonomous lifecycle terminal outcome disappeared during audit",
                )
            })?;
        let _ = Self::decode_autonomous_lifecycle_terminal_outcome(path, &bytes)?;
        Ok(())
    }
    /// Collect one startup terminal outcome, if `name` belongs to that namespace.
    fn collect_autonomous_lifecycle_terminal_outcome_for_startup_locked(
        &self,
        entry: &LaneConfigEntry,
        directory: &Path,
        path: &Path,
        name: &str,
        outcomes: &mut BTreeMap<(u64, u64), (PathBuf, AutonomousLifecycleTerminalOutcomeV1)>,
    ) -> Result<bool> {
        let Some((lane_block_height, proposal_height)) =
            Self::autonomous_lifecycle_terminal_outcome_coordinates(name)
        else {
            return Ok(false);
        };
        let bytes = self
            .read_regular_sidecar_bytes(
                path,
                directory,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous lifecycle terminal outcome disappeared during startup reconstruction",
                )
            })?;
        let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(path, &bytes)?;
        let binding = outcome.binding();
        let (active_incarnation, activation_height) = self.active_lane_incarnation_marker(entry)?;
        if binding.lane_id != entry.lane_id
            || binding.dataspace_id != entry.dataspace_id
            || binding.lane_incarnation != active_incarnation
            || binding.proposal_height <= activation_height
            || binding.lane_block_height != lane_block_height
            || binding.proposal_height != proposal_height
            || outcomes
                .insert(
                    (lane_block_height, proposal_height),
                    (path.to_path_buf(), outcome),
                )
                .is_some()
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle terminal outcome has a stale, duplicate, or namespace-conflicting identity",
            ));
        }
        Ok(true)
    }
    /// Seal every source-authenticated Complete replica outcome before startup
    /// performs any unrelated capacity-consuming repair.
    pub(crate) fn seal_completed_autonomous_lifecycle_replica_claims_on_startup(
        &self,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entries = self
            .lane_storage_entries
            .lock()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let _sidecar_guard = self.sidecar_lock.lock();
        self.seal_completed_autonomous_lifecycle_replica_claims_on_startup_locked(
            pending_canonical_bytes,
            &entries,
        )
    }

    /// Perform the seal-only startup pass while the Kura mutation locks are held.
    ///
    /// The terminal outcome is durable before its entrypoint claims are sealed,
    /// so a crash can leave `ReplicaReleased` claims behind a Complete outcome.
    /// Scan every active lane first: otherwise an earlier lane's pointer or view
    /// repair could consume the capacity needed to make those claims
    /// archive-independent.
    fn seal_completed_autonomous_lifecycle_replica_claims_on_startup_locked(
        &self,
        pending_canonical_bytes: u64,
        entries: &[LaneConfigEntry],
    ) -> Result<()> {
        let mut outcomes_seen = 0_usize;
        for entry in entries {
            let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
            let directory_entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::IO(error, directory)),
            };
            for directory_entry in directory_entries {
                let directory_entry =
                    directory_entry.map_err(|error| Error::IO(error, directory.clone()))?;
                let path = directory_entry.path();
                let name = directory_entry.file_name().into_string().map_err(|_| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "replica Complete startup seal found a non-UTF-8 artifact",
                    )
                })?;
                let Some(identity) = Self::autonomous_lifecycle_terminal_outcome_coordinates(&name)
                else {
                    if name.starts_with(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_PREFIX) {
                        return Err(Self::invalid_lane_artifact_error(
                            path,
                            "replica Complete startup seal found a malformed terminal outcome path",
                        ));
                    }
                    continue;
                };
                outcomes_seen = outcomes_seen.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        directory.clone(),
                        "replica Complete startup seal inventory count overflows",
                    )
                })?;
                if outcomes_seen > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                    return Err(Self::invalid_lane_artifact_error(
                        directory,
                        "replica Complete startup seal inventory exceeds its global bound",
                    ));
                }
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "replica Complete terminal outcome disappeared during startup seal",
                        )
                    })?;
                let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
                if !outcome.is_complete()
                    || !matches!(
                        outcome.source(),
                        AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                            ..
                        }
                    )
                {
                    continue;
                }
                let binding = outcome.binding();
                let (active_incarnation, activation_height) =
                    self.active_lane_incarnation_marker(entry)?;
                if binding.lane_id != entry.lane_id
                    || binding.dataspace_id != entry.dataspace_id
                    || binding.lane_incarnation != active_incarnation
                    || binding.proposal_height <= activation_height
                    || binding.lane_block_height != identity.0
                    || binding.proposal_height != identity.1
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "replica Complete startup seal targets stale lane geometry",
                    ));
                }
                let record = self
                    .read_autonomous_lane_block_attempt_record_locked(
                        entry,
                        binding.lane_id,
                        binding.lane_block_height,
                        binding.proposal_height,
                        binding.network_id,
                        binding.epoch,
                        None,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "replica Complete startup seal lost its payload attempt",
                        )
                    })?;
                let payload = &record.artifact.executable_payload;
                outcome
                    .validate_for_payload(payload)
                    .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                let cursor = self
                    .read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(entry, payload)?;
                if cursor.binding() != binding {
                    return Err(Self::invalid_lane_artifact_error(
                        path.clone(),
                        "replica Complete startup seal differs from its signed cursor binding",
                    ));
                }
                let queue_disposition = self
                    .autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                        // This is deliberately a seal-only pre-sweep. A recoverable
                        // view-state temporary must wait for the ordered startup
                        // repair corridor; consuming capacity here could strand a
                        // later Complete replica group's raw claims.
                        None,
                        entry,
                        payload,
                        record.retirement.as_ref(),
                        outcome.source(),
                    )?;
                let retirement = record.retirement.as_ref().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path,
                        "replica Complete startup seal lost its exact retirement",
                    )
                })?;
                self.complete_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                    pending_canonical_bytes,
                    None,
                    payload,
                    retirement,
                    queue_disposition,
                    &outcome,
                )?;
            }
        }
        Ok(())
    }
    /// Revalidate every collected startup outcome against its payload, cursor, and source.
    fn validate_autonomous_lifecycle_terminal_outcomes_on_startup_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        attempts: &BTreeMap<
            u64,
            Vec<(
                AutonomousLaneBlockLatestAttemptV1,
                AutonomousLaneBlockDurableRecord,
            )>,
        >,
        cursors: &BTreeMap<(u64, u64), AutonomousLifecycleCursorV1>,
        outcomes: &BTreeMap<(u64, u64), (PathBuf, AutonomousLifecycleTerminalOutcomeV1)>,
    ) -> Result<()> {
        for (identity, (path, outcome)) in outcomes {
            let record = attempts.get(&identity.0).and_then(|attempts_at_height| {
                attempts_at_height.iter().find_map(|(pointer, record)| {
                    (pointer.proposal_height == identity.1).then_some(record)
                })
            });
            let record = record.ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal outcome is orphaned from its payload attempt",
                )
            })?;
            let payload = &record.artifact.executable_payload;
            outcome
                .validate_for_payload(payload)
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
            if cursors
                .get(identity)
                .map(AutonomousLifecycleCursorV1::binding)
                != Some(outcome.binding())
            {
                return Err(Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous lifecycle terminal outcome differs from its signed cursor binding",
                ));
            }
            match outcome.source() {
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier { .. } => {
                    self.autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked(
                        payload, source,
                    )?;
                }
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease { .. } => {
                    self.autonomous_lifecycle_terminal_source_matches_release_locked(
                        Some(pending_canonical_bytes),
                        entry,
                        payload,
                        record.retirement.as_ref(),
                        source,
                    )?;
                }
                source @ AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
                    ..
                } => {
                    let queue_disposition = self
                        .autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
                        Some(pending_canonical_bytes),
                        entry,
                        payload,
                        record.retirement.as_ref(),
                        source,
                    )?;
                    if outcome.is_complete() {
                        let retirement = record.retirement.as_ref().ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                path.clone(),
                                "Complete replica Queue disposition lost its exact retirement",
                            )
                        })?;
                        self.complete_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                            pending_canonical_bytes,
                            Some(pending_canonical_bytes),
                            payload,
                            retirement,
                            queue_disposition,
                            outcome,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}
impl Kura {
    fn validate_autonomous_lifecycle_bootstrap_authority_identity_locked(
        &self,
        process_generation: &AutonomousLifecycleProcessGenerationClaim,
        entry: &LaneConfigEntry,
        path: &Path,
        bootstrap: &AutonomousLifecycleBootstrapV1,
    ) -> Result<()> {
        let process_record =
            self.validate_autonomous_lifecycle_process_generation_claim(process_generation)?;
        Self::validate_autonomous_lifecycle_bootstrap_process_generation(
            &process_record,
            bootstrap,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
        let (active_incarnation, activation_height) = self.active_lane_incarnation_marker(entry)?;
        let expected_path = Self::autonomous_lifecycle_bootstrap_path_for_entry(
            entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        if entry.lane_id != descriptor.lane_id
            || entry.dataspace_id != descriptor.dataspace_id
            || active_incarnation != descriptor.lane_incarnation
            || descriptor.proposal_height <= activation_height
            || path != expected_path.as_path()
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous lifecycle bootstrap targets a stale route or incarnation",
            ));
        }
        Ok(())
    }
    fn revalidate_autonomous_lifecycle_bootstrap_for_completion(
        &self,
        authority: AutonomousLifecycleBootstrapRecoveryAuthority,
    ) -> Result<AutonomousLifecycleBootstrapCompletionRevalidation> {
        if authority.store_root != self.store_root {
            return Err(Self::invalid_lane_artifact_error(
                authority.path,
                "autonomous lifecycle bootstrap authority belongs to another Kura root",
            ));
        }
        self.validate_autonomous_lifecycle_process_generation_claim(&authority.process_generation)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let proposal = &authority.bootstrap.body.executable_payload.origin_proposal;
        let already_terminal = self
            .lane_block_application_receipt_available_under_prune_and_canonical_guards(proposal);
        let _geometry_guard = self.lane_geometry_lock.lock();
        let descriptor = &proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        let parent = authority.path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                authority.path.clone(),
                "autonomous lifecycle bootstrap authority path has no parent",
            )
        })?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let _namespace_budget = self.autonomous_lane_attempt_inventory_counts_locked(
            &entry,
            descriptor.lane_block_height,
        )?;
        let bytes = self
            .read_regular_sidecar_bytes(
                &authority.path,
                parent,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    authority.path.clone(),
                    "autonomous lifecycle bootstrap disappeared before completion",
                )
            })?;
        if bytes != authority.expected_bytes || Hash::new(&bytes) != authority.expected_bytes_hash {
            return Err(Self::invalid_lane_artifact_error(
                authority.path,
                "autonomous lifecycle bootstrap changed after recovery authority was minted",
            ));
        }
        let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&authority.path, &bytes)?;
        if bootstrap != authority.bootstrap {
            return Err(Self::invalid_lane_artifact_error(
                authority.path,
                "autonomous lifecycle bootstrap identity changed during recovery",
            ));
        }
        let authority = self.autonomous_lifecycle_bootstrap_authority_locked(
            &authority.process_generation,
            &entry,
            authority.path,
            bytes,
            bootstrap,
        )?;
        Ok(AutonomousLifecycleBootstrapCompletionRevalidation {
            authority,
            receipt_terminal: already_terminal,
        })
    }
    fn publish_autonomous_lifecycle_bootstrap_cursor_stage(
        &self,
        authority: &AutonomousLifecycleBootstrapRecoveryAuthority,
        target: AutonomousLifecycleBootstrapRecoveryStage,
    ) -> Result<LaneBlockAuxiliaryPersistenceOutcome> {
        self.durable_mutation_authorized()?;
        if authority.store_root != self.store_root {
            return Err(Self::invalid_lane_artifact_error(
                authority.path.clone(),
                "autonomous lifecycle bootstrap cursor authority belongs to another Kura root",
            ));
        }
        self.validate_autonomous_lifecycle_process_generation_claim(&authority.process_generation)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let proposal = &authority.bootstrap.body.executable_payload.origin_proposal;
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let descriptor = &proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let current_stage = self.validate_signed_bootstrap_payload_persistence_locked(
            authority,
            &entry,
            &authority.bootstrap.body.executable_payload,
        )?;
        let (next, replacing_existing) = match (target, current_stage) {
            (
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
                AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable,
            ) => (&authority.bootstrap.body.prepared_activate, false),
            (
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable
                | AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
            ) => return Ok(LaneBlockAuxiliaryPersistenceOutcome::Persisted),
            (
                AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
            ) => (&authority.bootstrap.body.live_activate, true),
            (
                AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
                AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
            ) => return Ok(LaneBlockAuxiliaryPersistenceOutcome::Persisted),
            (AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly, _)
            | (AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable, _)
            | (AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable, _)
            | (AutonomousLifecycleBootstrapRecoveryStage::LiveDurable, _) => {
                return Err(Self::invalid_lane_artifact_error(
                    authority.path.clone(),
                    "autonomous lifecycle bootstrap cursor stages are not contiguous",
                ));
            }
        };
        let cursor_path = Self::autonomous_lifecycle_cursor_path_for_entry(
            &entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let cursor_parent = cursor_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                cursor_path.clone(),
                "autonomous lifecycle bootstrap cursor path has no parent",
            )
        })?;
        let current_bytes = self.read_regular_sidecar_bytes(
            &cursor_path,
            cursor_parent,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
        )?;
        let prepared_bytes = authority
            .bootstrap
            .body
            .prepared_activate
            .encode_framed()
            .map_err(Error::NoritoFrame)?;
        if replacing_existing && current_bytes.as_deref() != Some(prepared_bytes.as_slice())
            || !replacing_existing && current_bytes.is_some()
        {
            return Err(Self::invalid_lane_artifact_error(
                cursor_path,
                "autonomous lifecycle bootstrap cursor head changed before publication",
            ));
        }
        let next_bytes = next.encode_framed().map_err(Error::NoritoFrame)?;
        let previous_len = current_bytes
            .as_ref()
            .map_or(Ok(0), |bytes| u64::try_from(bytes.len()))?;
        let next_len = u64::try_from(next_bytes.len())?;
        let inventory = self.autonomous_lane_attempt_inventory_counts_locked(
            &entry,
            descriptor.lane_block_height,
        )?;
        Self::validate_autonomous_lifecycle_cursor_cas_budget(
            inventory.conceptual_files,
            inventory.conceptual_bytes,
            previous_len,
            next_len,
            replacing_existing,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(cursor_path.clone(), message))?;
        // The atomic writer materializes the complete successor beside the old
        // cursor (for replace) or empty stable path (for create). Preflight that
        // exact transient exposure against both accounting domains before any
        // mutation; only the enforced domain has a configured capacity bound.
        let creates_lifecycle_identity = !replacing_existing
            && inventory.needs_terminal_reservation_for_new_identity((
                descriptor.lane_block_height,
                descriptor.proposal_height,
            ));
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            next_len,
            creates_lifecycle_identity,
            false,
            &cursor_path,
        )?;
        self.kura_total_disk_usage_bytes()?
            .checked_add(next_len)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    cursor_path.clone(),
                    "autonomous lifecycle bootstrap cursor atomic peak total-disk accounting overflows",
                )
            })?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        if replacing_existing {
            self.write_atomic_synced_replace(&cursor_path, &next_bytes)?;
        } else if !self.write_atomic_synced_noclobber(&cursor_path, &next_bytes)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "autonomous lifecycle bootstrap cursor appeared during publication",
                ),
                cursor_path,
            ));
        }
        self.update_disk_usage_delta(previous_len, next_len);
        self.update_total_disk_usage_delta(previous_len, next_len);
        accounting_mutation.finish();
        let readback = self
            .read_regular_sidecar_bytes(
                &cursor_path,
                cursor_parent,
                AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    cursor_path.clone(),
                    "autonomous lifecycle bootstrap cursor disappeared after publication",
                )
            })?;
        if Self::decode_autonomous_lifecycle_cursor(&cursor_path, &readback)? != *next {
            return Err(Self::invalid_lane_artifact_error(
                cursor_path,
                "autonomous lifecycle bootstrap cursor readback differs from the signed target",
            ));
        }
        Ok(LaneBlockAuxiliaryPersistenceOutcome::Persisted)
    }
}
