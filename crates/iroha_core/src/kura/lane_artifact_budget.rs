#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PostWsvLaneArtifactExecutionIdentity {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    lane_block_descriptor_hash: Hash,
    proposal_hash: Hash,
    receipt_hash: HashOf<LaneBlockApplicationReceiptArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PostWsvLaneArtifactStableComponentId {
    Receipt(PostWsvLaneArtifactExecutionIdentity),
    Frontier(PostWsvLaneArtifactExecutionIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostWsvLaneArtifactExecutionPlan {
    identity: PostWsvLaneArtifactExecutionIdentity,
    receipt: LaneBlockApplicationReceiptArtifact,
    frontier: LaneMergeApplicationFrontierV1,
    terminal_source: AutonomousLifecycleTerminalOutcomeSourceV1,
    executable_payload: LaneExecutablePayloadV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostWsvLaneArtifactBudgetPlan {
    entry_hash: HashOf<MergeLedgerEntry>,
    carrier_height: u64,
    carrier_hash: HashOf<BlockHeader>,
    stable_components: BTreeMap<PostWsvLaneArtifactStableComponentId, u64>,
    executions: BTreeMap<PostWsvLaneArtifactExecutionIdentity, PostWsvLaneArtifactExecutionPlan>,
    shared_transient_bytes: u64,
}

impl PostWsvLaneArtifactBudgetPlan {
    fn initial_reserved_bytes(&self) -> Option<u64> {
        self.stable_components
            .values()
            .try_fold(self.shared_transient_bytes, |total, bytes| {
                total.checked_add(*bytes)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostWsvLaneArtifactBudgetReservation {
    plan: PostWsvLaneArtifactBudgetPlan,
    outstanding_components: BTreeSet<PostWsvLaneArtifactStableComponentId>,
    incomplete_terminal_outcomes: BTreeSet<PostWsvLaneArtifactExecutionIdentity>,
}

impl PostWsvLaneArtifactBudgetReservation {
    fn new(plan: PostWsvLaneArtifactBudgetPlan) -> Self {
        Self {
            outstanding_components: plan.stable_components.keys().copied().collect(),
            incomplete_terminal_outcomes: plan.executions.keys().copied().collect(),
            plan,
        }
    }

    fn reserved_bytes(&self) -> Option<u64> {
        self.outstanding_components
            .iter()
            .try_fold(self.plan.shared_transient_bytes, |total, component| {
                total.checked_add(*self.plan.stable_components.get(component)?)
            })
    }
}

impl Kura {
    fn post_wsv_lane_artifact_budget_plan(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<Option<PostWsvLaneArtifactBudgetPlan>> {
        let Some(batch) = entry.execution_batch.as_ref() else {
            return Ok(None);
        };
        if batch.lanes.is_empty() {
            return Err(Self::invalid_lane_artifact_error(
                PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                "post-WSV artifact plan has an empty execution batch",
            ));
        }
        let entry_hash = crate::merge::merge_ledger_entry_hash(entry);
        let mut stable_components = BTreeMap::new();
        let mut executions = BTreeMap::new();
        let mut shared_transient_bytes = u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)?;
        for execution in &batch.lanes {
            let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
                entry,
                batch,
                execution,
                Self::merge_lane_block_artifact(execution),
                carrier_height,
                carrier_hash,
            );
            Self::validate_lane_block_application_receipt_artifact(&receipt).map_err(
                |message| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                        format!("cannot account merge application receipt: {message}"),
                    )
                },
            )?;
            let receipt_bytes = receipt.encode_framed()?;
            let receipt_len = u64::try_from(receipt_bytes.len())?;
            if receipt_len == 0 || receipt_len > STRICT_INIT_MAX_BLOCK_BYTES {
                return Err(Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    "merge application receipt exceeds the strict progress-sidecar payload bound",
                ));
            }
            let frontier =
                LaneMergeApplicationFrontierV1::from_receipt(&receipt).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_MERGE_APPLICATION_FRONTIER_FILE),
                        "merge application receipt cannot produce its durable frontier",
                    )
                })?;
            let frontier_bytes = norito::encode_canonical(&frontier).map_err(Error::NoritoFrame)?;
            if frontier_bytes.is_empty()
                || frontier_bytes.len() > LANE_MERGE_APPLICATION_FRONTIER_MAX_BYTES
            {
                return Err(Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_MERGE_APPLICATION_FRONTIER_FILE),
                    "merge application frontier exceeds its hard byte limit during accounting",
                ));
            }
            let frontier_len = u64::try_from(frontier_bytes.len())?;
            shared_transient_bytes = shared_transient_bytes.max(frontier_len);
            let descriptor = &execution.proposal.descriptor;
            let identity = PostWsvLaneArtifactExecutionIdentity {
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                proposal_height: descriptor.proposal_height,
                lane_block_height: descriptor.lane_block_height,
                lane_block_descriptor_hash: descriptor.descriptor_hash,
                proposal_hash: execution.proposal.proposal_hash,
                receipt_hash: HashOf::new(&receipt),
            };
            let receipt_component_bytes = receipt_len
                .checked_add(Self::maximum_index_growth_for_unresolved_sidecar_write(
                    descriptor.lane_block_height,
                ))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                        "merge receipt component byte accounting overflowed",
                    )
                })?;
            if stable_components
                .insert(
                    PostWsvLaneArtifactStableComponentId::Receipt(identity),
                    receipt_component_bytes,
                )
                .is_some()
                || stable_components
                    .insert(
                        PostWsvLaneArtifactStableComponentId::Frontier(identity),
                        frontier_len,
                    )
                    .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    "post-WSV artifact plan aliases a domain-separated stable component",
                ));
            }
            let bundle = Self::decode_autonomous_lane_merge_bundle(
                &execution.source_bundle,
                execution.autonomous_chain_id_hash,
                execution.autonomous_epoch,
            )
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    format!("cannot account autonomous execution payload: {message}"),
                )
            })?;
            let executable_payload = bundle.executable_payload().clone();
            let terminal_source = Self::autonomous_lifecycle_terminal_source_from_merge_receipt(
                &receipt,
            )
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    message,
                )
            })?;
            if executions
                .insert(
                    identity,
                    PostWsvLaneArtifactExecutionPlan {
                        identity,
                        receipt,
                        frontier,
                        terminal_source,
                        executable_payload,
                    },
                )
                .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    "post-WSV artifact plan duplicates one execution identity",
                ));
            }
        }
        Ok(Some(PostWsvLaneArtifactBudgetPlan {
            entry_hash,
            carrier_height,
            carrier_hash,
            stable_components,
            executions,
            shared_transient_bytes,
        }))
    }

    fn merge_lane_application_artifact_required_bytes_for_carrier(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<u64> {
        self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?
            .map_or(Ok(0), |plan| {
                plan.initial_reserved_bytes().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                        "merge application artifact byte accounting overflowed",
                    )
                })
            })
    }

    fn merge_lane_application_artifact_required_bytes_for_block(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<u64> {
        let Some(reference) = Self::block_merge_reference(block) else {
            if merge_entry.is_some() {
                return Err(Error::MergeReferenceMismatch(
                    "merge entry supplied while accounting a block without a compact reference"
                        .to_owned(),
                ));
            }
            return Ok(0);
        };
        let entry = merge_entry.ok_or(Error::MissingCertifiedMergeSidecar {
            entry_hash: reference.entry_hash,
        })?;
        if !reference.matches_entry(entry) {
            return Err(Error::MergeReferenceMismatch(
                "block compact reference differs from the merge entry used for artifact accounting"
                    .to_owned(),
            ));
        }
        self.merge_lane_application_artifact_required_bytes_for_carrier(
            entry,
            block.header().height().get(),
            block.hash(),
        )
    }

    /// Read exact active evidence for one immutable carrier plan while the
    /// caller holds lane geometry and sidecar locks. A complete terminal record
    /// is an authenticated tombstone for receipt/frontier bytes that later
    /// bounded compaction may have removed.
    fn post_wsv_lane_artifact_durable_evidence_locked(
        &self,
        plan: &PostWsvLaneArtifactBudgetPlan,
    ) -> Result<
        Option<(
            BTreeSet<PostWsvLaneArtifactStableComponentId>,
            BTreeSet<PostWsvLaneArtifactExecutionIdentity>,
        )>,
    > {
        let mut consumed = BTreeSet::new();
        let mut complete = BTreeSet::new();
        let mut active = 0_usize;
        for execution in plan.executions.values() {
            let identity = execution.identity;
            let Ok(entry) = self.lane_storage_entry(identity.lane_id) else {
                continue;
            };
            if self
                .require_active_lane_artifact(&entry, &execution.receipt.proposal.descriptor)
                .is_err()
            {
                continue;
            }
            active = active.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "post-WSV active execution count overflowed",
                )
            })?;

            let terminal_path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
                &entry,
                &self.store_root,
                identity.lane_block_height,
                identity.proposal_height,
            );
            let terminal_parent = terminal_path.parent().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    terminal_path.clone(),
                    "post-WSV terminal evidence path has no parent",
                )
            })?;
            if let Some(bytes) = self.read_regular_sidecar_bytes(
                &terminal_path,
                terminal_parent,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            )? {
                let outcome =
                    Self::decode_autonomous_lifecycle_terminal_outcome(&terminal_path, &bytes)?;
                outcome
                    .validate_for_payload(&execution.executable_payload)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(terminal_path.clone(), message)
                    })?;
                if outcome.source() != execution.terminal_source {
                    return Err(Self::invalid_lane_artifact_error(
                        terminal_path,
                        "post-WSV terminal evidence names another carrier component plan",
                    ));
                }
                if outcome.is_complete() {
                    complete.insert(identity);
                    consumed.insert(PostWsvLaneArtifactStableComponentId::Receipt(identity));
                    consumed.insert(PostWsvLaneArtifactStableComponentId::Frontier(identity));
                    continue;
                }
            }

            let (receipt_data_path, receipt_index_path) =
                Self::lane_block_application_receipt_paths_for_entry(&entry, &self.store_root);
            if let Some(receipt) = self
                .read_lane_block_application_receipt_from_paths_durability_attested_locked(
                    identity.lane_id,
                    identity.lane_block_height,
                    &receipt_data_path,
                    &receipt_index_path,
                    false,
                )
            {
                if receipt != execution.receipt {
                    return Err(Self::invalid_lane_artifact_error(
                        receipt_data_path,
                        "post-WSV receipt evidence conflicts with its exact carrier plan",
                    ));
                }
                consumed.insert(PostWsvLaneArtifactStableComponentId::Receipt(identity));
            }

            let frontier_path =
                Self::lane_merge_application_frontier_path_for_entry(&entry, &self.store_root);
            if let Some(frontier) =
                self.decode_lane_merge_application_frontier(&entry, &frontier_path)?
            {
                if frontier == execution.frontier {
                    consumed.insert(PostWsvLaneArtifactStableComponentId::Frontier(identity));
                } else if frontier.lane_id != identity.lane_id
                    || frontier.dataspace_id != identity.dataspace_id
                    || frontier.lane_incarnation != identity.lane_incarnation
                    || frontier.lane_block_height <= identity.lane_block_height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        frontier_path,
                        "post-WSV frontier evidence conflicts with its carrier plan",
                    ));
                }
            }
        }
        if active == 0 {
            return Ok(None);
        }
        if active != plan.executions.len() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "post-WSV carrier plan mixes active and historical lane incarnations",
            ));
        }
        Ok(Some((consumed, complete)))
    }

    fn ensure_post_wsv_lane_artifact_budget_plan_locked(
        &self,
        pending_canonical_bytes: u64,
        plan: PostWsvLaneArtifactBudgetPlan,
    ) -> Result<u64> {
        let Some((consumed, complete)) =
            self.post_wsv_lane_artifact_durable_evidence_locked(&plan)?
        else {
            let reservations = self.post_wsv_lane_artifact_budget_reservations.lock();
            if reservations.contains_key(&plan.entry_hash) {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "an outstanding post-WSV reservation became historical before release",
                ));
            }
            return Ok(0);
        };
        let mut reservations = self.post_wsv_lane_artifact_budget_reservations.lock();
        if let Some(reservation) = reservations.get_mut(&plan.entry_hash) {
            if reservation.plan != plan {
                return Err(Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    "post-WSV lane artifact reservation conflicts for one merge entry",
                ));
            }
            reservation
                .outstanding_components
                .retain(|component| !consumed.contains(component));
            reservation
                .incomplete_terminal_outcomes
                .retain(|identity| !complete.contains(identity));
            return reservation.reserved_bytes().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                    "post-WSV lane artifact remaining-byte accounting overflowed",
                )
            });
        }

        let mut reservation = PostWsvLaneArtifactBudgetReservation::new(plan);
        reservation
            .outstanding_components
            .retain(|component| !consumed.contains(component));
        reservation
            .incomplete_terminal_outcomes
            .retain(|identity| !complete.contains(identity));
        // All-Complete history is its own exact durable tombstone. Repeated
        // startup inventory/readback and store retries must stutter without
        // reinstalling a process-local envelope.
        if reservation.outstanding_components.is_empty()
            && reservation.incomplete_terminal_outcomes.is_empty()
        {
            return Ok(0);
        }
        let reserved_bytes = reservation.reserved_bytes().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                "post-WSV lane artifact remaining-byte accounting overflowed",
            )
        })?;
        if self.max_disk_usage_bytes != 0 && !self.store_root.as_os_str().is_empty() {
            let existing_reservations =
                reservations.values().try_fold(0_u64, |total, existing| {
                    let bytes = existing.reserved_bytes().ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "existing post-WSV reservation byte accounting overflowed",
                        )
                    })?;
                    total.checked_add(bytes).ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "existing post-WSV reservations overflow configured accounting",
                        )
                    })
                })?;
            let used = self.kura_disk_usage_bytes()?;
            let terminal_reservations =
                self.autonomous_global_terminal_outcome_reserved_bytes_locked()?;
            let certified_bundle_reservations =
                self.certified_bundle_capacity_reserved_bytes()?;
            let required = used
                .checked_add(pending_canonical_bytes)
                .and_then(|bytes| bytes.checked_add(terminal_reservations))
                .and_then(|bytes| bytes.checked_add(existing_reservations))
                .and_then(|bytes| bytes.checked_add(reserved_bytes))
                .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
                .and_then(|bytes| {
                    bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
                })
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "new post-WSV reservation configured accounting overflowed",
                    )
                })?;
            if required > self.max_disk_usage_bytes {
                return Err(Error::StorageBudgetExceeded {
                    limit: self.max_disk_usage_bytes,
                    used,
                    required,
                });
            }
        }
        reservations.insert(reservation.plan.entry_hash, reservation);
        Ok(reserved_bytes)
    }

    fn authenticate_post_wsv_lane_artifact_carrier_under_prune_and_canonical_guards(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        let entry_hash = crate::merge::merge_ledger_entry_hash(entry);
        if self.merge_log.lock().entry_by_hash(entry_hash)?.as_ref() != Some(entry) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "post-WSV reservation entry is absent from the committed merge log",
            ));
        }
        let carrier = self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "post-WSV reservation lost its exact canonical carrier",
                )
            })?;
        if carrier.block_height != carrier_height
            || carrier.block_hash != carrier_hash
            || carrier.epoch_id != entry.epoch_id
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "post-WSV reservation names a stale or conflicting canonical carrier",
            ));
        }
        Ok(())
    }

    /// Authenticate an exact committed carrier while the caller holds prune
    /// and canonical-chain guards, then acquire geometry and sidecar in the
    /// sole permitted order before installing or reconciling its envelope.
    fn ensure_post_wsv_lane_artifact_budget_reservation_under_prune_and_canonical_guards(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<u64> {
        self.authenticate_post_wsv_lane_artifact_carrier_under_prune_and_canonical_guards(
            entry,
            carrier_height,
            carrier_hash,
        )?;
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        let Some(plan) =
            self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?
        else {
            return Ok(0);
        };
        self.ensure_post_wsv_lane_artifact_budget_plan_locked(pending_canonical_bytes, plan)
    }

    fn ensure_post_wsv_lane_artifact_budget_reservation_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<u64> {
        self.authenticate_post_wsv_lane_artifact_carrier_under_prune_and_canonical_guards(
            entry,
            carrier_height,
            carrier_hash,
        )?;
        let Some(plan) =
            self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?
        else {
            return Ok(0);
        };
        self.ensure_post_wsv_lane_artifact_budget_plan_locked(pending_canonical_bytes, plan)
    }

    fn ensure_post_wsv_lane_artifact_budget_reservation(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<u64> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_post_wsv_lane_artifact_budget_reservation_under_prune_and_canonical_guards(
            entry,
            carrier_height,
            carrier_hash,
        )
    }

    fn reconcile_post_wsv_lane_artifact_budget_for_receipt_locked(
        &self,
        pending_canonical_bytes: u64,
        receipt: &LaneBlockApplicationReceiptArtifact,
    ) -> Result<()> {
        if receipt.format != LaneBlockApplicationReceiptArtifactFormat::MergeExecution {
            return Ok(());
        }
        let entry_hash = receipt.merge_entry_hash.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "merge receipt budget reconciliation lacks its entry hash",
            )
        })?;
        let carrier_height = receipt.merge_carrier_block_height.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "merge receipt budget reconciliation lacks its carrier height",
            )
        })?;
        let carrier_hash = receipt.merge_carrier_block_hash.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "merge receipt budget reconciliation lacks its carrier hash",
            )
        })?;
        let entry = self
            .merge_log
            .lock()
            .entry_by_hash(entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "merge receipt budget reconciliation lost its exact entry",
                )
            })?;
        let carrier = self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "merge receipt budget reconciliation lost its exact carrier",
                )
            })?;
        if carrier.block_height != carrier_height
            || carrier.block_hash != carrier_hash
            || carrier.epoch_id != entry.epoch_id
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "merge receipt budget reconciliation changed carrier identity",
            ));
        }
        self.ensure_post_wsv_lane_artifact_budget_reservation_locked(
            pending_canonical_bytes,
            &entry,
            carrier_height,
            carrier_hash,
        )?;
        Ok(())
    }

    fn reconcile_post_wsv_lane_artifact_budget_for_terminal_outcome_locked(
        &self,
        pending_canonical_bytes: u64,
        outcome: &AutonomousLifecycleTerminalOutcomeV1,
    ) -> Result<()> {
        let AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
            merge_epoch_id,
            merge_entry_hash,
            carrier_block_height,
            carrier_block_hash,
            ..
        } = outcome.source()
        else {
            return Ok(());
        };
        let entry = self
            .merge_log
            .lock()
            .entry_by_hash(merge_entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "terminal budget reconciliation lost its exact merge entry",
                )
            })?;
        let carrier = self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(merge_entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "terminal budget reconciliation lost its exact carrier",
                )
            })?;
        if entry.epoch_id != merge_epoch_id
            || carrier.epoch_id != merge_epoch_id
            || carrier.block_height != carrier_block_height
            || carrier.block_hash != carrier_block_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "terminal budget reconciliation changed carrier identity",
            ));
        }
        self.ensure_post_wsv_lane_artifact_budget_reservation_locked(
            pending_canonical_bytes,
            &entry,
            carrier_block_height,
            carrier_block_hash,
        )?;
        Ok(())
    }

    fn post_wsv_lane_artifact_budget_reserved_bytes(&self) -> Result<u64> {
        self.post_wsv_lane_artifact_budget_reservations
            .lock()
            .values()
            .try_fold(0_u64, |total, reservation| {
                let remaining = reservation.reserved_bytes().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                        "post-WSV lane artifact reservation overflows",
                    )
                })?;
                total.checked_add(remaining).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                        "post-WSV lane artifact reservations overflow",
                    )
                })
            })
    }

    pub(crate) fn release_post_wsv_lane_artifact_budget_reservation(
        &self,
        entry: &MergeLedgerEntry,
        carrier_height: u64,
        carrier_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        let Some(plan) =
            self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?
        else {
            return Ok(());
        };
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let committed = self
            .merge_log
            .lock()
            .entry_by_hash(plan.entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "post-WSV budget release lost its exact committed entry",
                )
            })?;
        let carrier = self
            .merge_carrier_for_entry_under_prune_and_canonical_guards(plan.entry_hash)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "post-WSV budget release lost its exact carrier",
                )
            })?;
        if committed != *entry
            || carrier.block_height != carrier_height
            || carrier.block_hash != carrier_hash
            || carrier.epoch_id != entry.epoch_id
        {
            return Err(Self::invalid_lane_artifact_error(
                PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                "post-WSV lane artifact budget release names another canonical carrier",
            ));
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        self.ensure_post_wsv_lane_artifact_budget_plan_locked(
            pending_canonical_bytes,
            plan.clone(),
        )?;
        let mut reservations = self.post_wsv_lane_artifact_budget_reservations.lock();
        let Some(reservation) = reservations.get(&plan.entry_hash) else {
            // Exact durable Complete evidence made a restart/retry reservation
            // unnecessary; this is the all-Complete stutter case.
            return Ok(());
        };
        if reservation.plan != plan
            || !reservation.outstanding_components.is_empty()
            || !reservation.incomplete_terminal_outcomes.is_empty()
        {
            return Err(Self::invalid_lane_artifact_error(
                PathBuf::from(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE),
                "post-WSV lane artifact budget release has outstanding stable or terminal components",
            ));
        }
        reservations.remove(&plan.entry_hash);
        Ok(())
    }

    /// Rebuild process-local envelopes from the bounded set of active,
    /// incomplete lifecycle identities. The route/incarnation latest-execution
    /// index handles tips; one bounded chronological reconstruction maps older
    /// exact identities. Recovery never reverse-scans carrier blocks or keeps
    /// an unbounded execution-height index.
    fn rebuild_post_wsv_lane_artifact_budget_reservations_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
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
        let mut incomplete_seen = 0_usize;
        let mut carrier_hashes = BTreeSet::new();
        let mut historical_execution_identities = BTreeSet::new();
        for lane_entry in entries {
            let inventory = self.autonomous_lane_attempt_inventory_counts_locked(&lane_entry, 1)?;
            for identity in inventory
                .lifecycle_identities
                .difference(&inventory.complete_terminal_outcome_identities)
                .copied()
            {
                incomplete_seen = incomplete_seen.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup incomplete lifecycle identity count overflowed",
                    )
                })?;
                if incomplete_seen > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup incomplete lifecycle identities exceed the bounded recovery inventory",
                    ));
                }
                let terminal_path = Self::autonomous_lifecycle_terminal_outcome_path_for_entry(
                    &lane_entry,
                    &self.store_root,
                    identity.0,
                    identity.1,
                );
                let terminal_parent = terminal_path.parent().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        terminal_path.clone(),
                        "startup terminal outcome path has no parent",
                    )
                })?;
                let terminal_entry_hash = self
                    .read_regular_sidecar_bytes(
                        &terminal_path,
                        terminal_parent,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .map(|bytes| {
                        Self::decode_autonomous_lifecycle_terminal_outcome(&terminal_path, &bytes)
                    })
                    .transpose()?
                    .and_then(|outcome| match outcome.source() {
                        AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
                            merge_entry_hash,
                            ..
                        } => Some(merge_entry_hash),
                        AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease { .. } => None,
                    });
                if let Some(entry_hash) = terminal_entry_hash {
                    carrier_hashes.insert(entry_hash);
                    continue;
                }

                let (receipt_data_path, receipt_index_path) =
                    Self::lane_block_application_receipt_paths_for_entry(
                        &lane_entry,
                        &self.store_root,
                    );
                if let Some(receipt) = self
                    .read_lane_block_application_receipt_from_paths_durability_attested_locked(
                        lane_entry.lane_id,
                        identity.0,
                        &receipt_data_path,
                        &receipt_index_path,
                        false,
                    )
                    .filter(|receipt| {
                        receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution
                            && receipt.proposal.descriptor.proposal_height == identity.1
                    })
                    && let Some(entry_hash) = receipt.merge_entry_hash
                {
                    carrier_hashes.insert(entry_hash);
                    continue;
                }

                let (incarnation, _) = self.active_lane_incarnation_marker(&lane_entry)?;
                let latest = self.merge_log.lock().latest_execution_entry(
                    lane_entry.lane_id,
                    lane_entry.dataspace_id,
                    incarnation,
                );
                let execution_identity = (
                    lane_entry.lane_id,
                    lane_entry.dataspace_id,
                    incarnation,
                    identity.0,
                    identity.1,
                );
                let Some((latest_height, entry_hash)) = latest else {
                    historical_execution_identities.insert(execution_identity);
                    continue;
                };
                if latest_height != identity.0 {
                    historical_execution_identities.insert(execution_identity);
                    continue;
                }
                let entry = self
                    .merge_log
                    .lock()
                    .entry_by_hash(entry_hash)?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "startup latest-execution index names a missing merge entry",
                        )
                    })?;
                let exact_member = entry.execution_batch.as_ref().is_some_and(|batch| {
                    batch.lanes.iter().any(|execution| {
                        let descriptor = &execution.proposal.descriptor;
                        descriptor.lane_id == lane_entry.lane_id
                            && descriptor.dataspace_id == lane_entry.dataspace_id
                            && descriptor.lane_incarnation == incarnation
                            && descriptor.lane_block_height == identity.0
                            && descriptor.proposal_height == identity.1
                    })
                });
                if exact_member {
                    carrier_hashes.insert(entry_hash);
                }
            }
        }
        if !historical_execution_identities.is_empty() {
            let historical = self
                .merge_log
                .lock()
                .execution_entries_for_bounded_identities(&historical_execution_identities)?;
            carrier_hashes.extend(historical.into_values());
        }
        if carrier_hashes.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "startup post-WSV carrier set exceeds the bounded lifecycle inventory",
            ));
        }
        for entry_hash in carrier_hashes {
            let entry = self
                .merge_log
                .lock()
                .entry_by_hash(entry_hash)?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup post-WSV carrier set names a missing merge entry",
                    )
                })?;
            let carrier = self
                .merge_carrier_for_entry_under_prune_and_canonical_guards(entry_hash)?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup post-WSV carrier set names a missing carrier",
                    )
                })?;
            self.ensure_post_wsv_lane_artifact_budget_reservation_locked(
                pending_canonical_bytes,
                &entry,
                carrier.block_height,
                carrier.block_hash,
            )?;
        }
        Ok(())
    }

    fn lane_artifact_required_bytes_for_block(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<u64> {
        let block_hash = block.hash();
        let mut total =
            self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?;
        if let Some(bundle) = block.execution_context() {
            for ownership in bundle
                .lane_payload_ownerships
                .iter()
                .filter(|ownership| Self::lane_payload_ownership_is_durable(ownership))
            {
                let artifact = LaneBlockArtifact::new(block_hash, ownership.clone());
                let encoded_len = u64::try_from(artifact.encode_framed()?.len())?;
                total = total.saturating_add(encoded_len).saturating_add(
                    Self::maximum_index_growth_for_unresolved_sidecar_write(
                        ownership.lane_block_height,
                    ),
                );
            }
        }

        let native_manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            block,
            merge_entry,
        )
        .map_err(|error| {
            Self::invalid_lane_artifact_error(
                PathBuf::from(NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX),
                format!("cannot account Native AMX application manifest: {error}"),
            )
        })?;
        let native_artifacts = native_amx_participant_application_artifacts(
            &native_manifest,
            native_amx_participant_application_finality_placeholder_hash(),
        )
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                PathBuf::from(NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX),
                "cannot account missing Native AMX manifest proof",
            )
        })?;
        let mut native_prune_intent_routes = BTreeSet::new();
        for (manifest_artifact, receipt) in native_artifacts {
            let (manifest_bytes, receipt_bytes) =
                native_amx_participant_application_pair_framed_bytes(&manifest_artifact, &receipt)?;
            let manifest_len = u64::try_from(manifest_bytes.len())?;
            let receipt_len = u64::try_from(receipt_bytes.len())?;
            let latest_len = u64::try_from(
                norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                    &receipt,
                ))?
                .len(),
            )?;
            total = total
                .saturating_add(manifest_len)
                .saturating_add(receipt_len)
                .saturating_add(latest_len);
            if native_prune_intent_routes.insert((
                manifest_artifact.leaf.lane_id,
                manifest_artifact.leaf.dataspace_id,
                manifest_artifact.leaf.lane_incarnation,
            )) {
                total = total.saturating_add(u64::try_from(
                    self.native_amx_evidence_prune_intent_max_bytes(),
                )?);
            }
        }
        Ok(total)
    }

    fn native_amx_manifest_for_committed_block(
        &self,
        block: &SignedBlock,
        merge_association: NativeAmxMergeAssociation<'_>,
        finality: &V2FinalityArtifact,
    ) -> Result<crate::sumeragi::exec::NativeAmxApplicationManifestV1> {
        let committed_merge_entry = self.associated_merge_entry_for_block(block)?;
        let planned_merge_entry = match merge_association {
            NativeAmxMergeAssociation::Live(staged)
            | NativeAmxMergeAssociation::Startup(staged) => staged,
            NativeAmxMergeAssociation::CommittedOnly => None,
        };
        if let Some(planned) = planned_merge_entry {
            let record = Self::carrier_record_for_block_entry(block, planned)?;
            Self::validate_merge_carrier_finality_projection(
                record,
                planned,
                block.header(),
                finality,
            )?;
            if committed_merge_entry
                .as_ref()
                .is_some_and(|committed| committed != planned)
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "Native AMX planned merge entry differs from its committed association",
                ));
            }
        }
        let merge_entry = match merge_association {
            NativeAmxMergeAssociation::Live(staged)
                if Self::block_merge_reference(block).is_some() =>
            {
                let staged = staged.ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "live Native AMX merge publication lacks its staged association witness",
                    )
                })?;
                let committed = committed_merge_entry.as_ref().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "live Native AMX merge publication lacks its committed association",
                    )
                })?;
                if committed != staged {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "live Native AMX staged merge entry differs from its committed association",
                    ));
                }
                Some(committed)
            }
            NativeAmxMergeAssociation::Live(_) | NativeAmxMergeAssociation::CommittedOnly => {
                committed_merge_entry.as_ref()
            }
            NativeAmxMergeAssociation::Startup(planned) => {
                committed_merge_entry.as_ref().or(planned)
            }
        };
        if Self::block_merge_reference(block).is_some() && merge_entry.is_none() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "Native AMX application block lacks its committed merge association",
            ));
        }
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            block,
            merge_entry,
        )
        .map_err(|error| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                format!("Native AMX application manifest construction failed: {error}"),
            )
        })
    }
}
