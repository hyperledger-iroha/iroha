// Crash-safe, capacity-bounded lane-history compaction.
#[derive(Clone, Copy)]
enum LaneHistoryTerminalEvidenceRole {
    Unpinned,
    CanonicalReplica,
    ApplicationReceipt,
}

impl Kura {
    fn compact_lane_histories_through_merge_frontier_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        frontier: &LaneMergeApplicationFrontierV1,
    ) -> Result<LaneHistoryCompactionOutcome> {
        if self
            .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                frontier,
            )
            .is_none()
        {
            return Err(Self::invalid_lane_artifact_error(
                Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root),
                "lane merge application frontier does not match its merge entry and carrier",
            ));
        }
        let retention = self.lane_history_retention;
        // Terminal files are independent crash-safe records. Collect their
        // references before recovering any pair temp so an older, otherwise
        // well-formed rewrite cannot be promoted after omitting newly durable
        // evidence. Both Pending and Complete stages retain dependencies.
        let terminal_references =
            self.active_autonomous_terminal_evidence_references_locked(entry)?;
        let empty_required_heights = BTreeSet::new();
        let pairs = [
            (
                Self::lane_artifact_paths_for_entry(entry, &self.store_root),
                LaneBlockArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::lane_block_execution_input_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionInputArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::lane_block_execution_preflight_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionPreflightArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::certified_lane_block_paths_for_entry(entry, &self.store_root),
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root),
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::canonical_autonomous_lane_replica_paths_for_entry(entry, &self.store_root),
                CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica,
            ),
            (
                Self::lane_block_application_receipt_paths_for_entry(entry, &self.store_root),
                LaneBlockApplicationReceiptArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt,
            ),
        ];
        // Finish every already-durable rewrite before optional capacity
        // preflight or fresh pruning. Required heights are checked against the
        // recovery candidate itself, including the data-promoted/index-temp
        // crash boundary.
        for ((data_path, index_path), kind, role) in &pairs {
            let required_heights = match role {
                LaneHistoryTerminalEvidenceRole::Unpinned => &empty_required_heights,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica => {
                    &terminal_references.replica_heights
                }
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt => {
                    &terminal_references.receipt_heights
                }
            };
            // Complete any already-durable rewrite before deciding whether a
            // fresh optional compaction can afford another temporary pair.
            // Otherwise the crash temp is counted once in physical usage and
            // again as projected headroom, so a tight-cap startup can refuse
            // the very promotion needed to make retirement drainable.
            let before_recovery = Self::sidecar_tracked_bytes(data_path, index_path)?;
            let recovery_accounting = self.begin_total_disk_usage_mutation();
            if !Self::recover_indexed_sidecar_artifacts_with_required_heights(
                data_path,
                index_path,
                required_heights,
                kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    format!("{kind} terminal-frontier recovery failed"),
                ));
            }
            let before = Self::sidecar_tracked_bytes(data_path, index_path)?;
            self.update_disk_usage_delta(before_recovery, before);
            recovery_accounting.finish();
        }
        self.validate_active_autonomous_terminal_evidence_references_locked(
            entry,
            &terminal_references,
        )?;

        // Rewrites publish one data/index temp pair at a time. Preflight the
        // largest pair against the unchanged pre-compaction state so a
        // configured-capacity refusal is byte-exact and cannot leave an early
        // pair compacted while a later pair is rejected.
        if self.max_disk_usage_bytes != 0 {
            let temp_peak = pairs.iter().try_fold(0_u64, |peak, ((data, index), _, _)| {
                Self::sidecar_tracked_bytes(data, index).map(|bytes| peak.max(bytes))
            })?;
            let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
            let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;
            let terminal_reservations =
                self.autonomous_global_terminal_outcome_reserved_bytes_locked()?;
            let required = self
                .kura_disk_usage_bytes()?
                .checked_add(pending_canonical_bytes)
                .and_then(|bytes| bytes.checked_add(terminal_reservations))
                .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
                .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
                .and_then(|bytes| {
                    bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
                })
                .and_then(|bytes| bytes.checked_add(temp_peak))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)),
                        "lane history compaction configured accounting overflowed",
                    )
                })?;
            if required > self.max_disk_usage_bytes {
                iroha_logger::debug!(
                    lane = %entry.lane_id.as_u32(),
                    required,
                    limit = self.max_disk_usage_bytes,
                    "skipping bounded lane-history compaction without configured temp headroom"
                );
                return Ok(LaneHistoryCompactionOutcome::CapacityBlocked);
            }
        }

        for ((data_path, index_path), kind, role) in pairs {
            let required_heights = match role {
                LaneHistoryTerminalEvidenceRole::Unpinned => &empty_required_heights,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica => {
                    &terminal_references.replica_heights
                }
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt => {
                    &terminal_references.receipt_heights
                }
            };
            let before = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            if !Self::prune_indexed_sidecars_through_terminal_frontier_with_required_heights(
                &data_path,
                &index_path,
                frontier.lane_block_height,
                retention,
                required_heights,
                kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    format!("{kind} terminal-frontier compaction failed"),
                ));
            }
            let after = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
            self.update_disk_usage_delta(before, after);
            accounting_mutation.finish();
        }
        self.validate_active_autonomous_terminal_evidence_references_locked(
            entry,
            &terminal_references,
        )?;
        // Autonomous attempt/cursor/outcome files form one crash-sensitive
        // evidence unit. Even though this maintenance runs only after the
        // receipt frontier (and, on the live carrier path, after terminal
        // completion), it must not unlink only the payload/view half of a
        // lifecycle unit. Keep the bounded namespace intact; lane
        // archive/removal moves the whole directory atomically after terminal
        // validation. A future compactor may remove complete units only behind
        // its own durable intent.
        Ok(LaneHistoryCompactionOutcome::Complete)
    }
}
