// Crash-safe, capacity-bounded lane-history compaction.
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
        let retention = self.roster_sidecar_retention;
        let pairs = [
            (
                Self::lane_artifact_paths_for_entry(entry, &self.store_root),
                LaneBlockArtifact::FORMAT_LABEL,
            ),
            (
                Self::lane_block_execution_input_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionInputArtifact::FORMAT_LABEL,
            ),
            (
                Self::lane_block_execution_preflight_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionPreflightArtifact::FORMAT_LABEL,
            ),
            (
                Self::certified_lane_block_paths_for_entry(entry, &self.store_root),
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
            ),
            (
                Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root),
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            ),
            (
                Self::lane_block_application_receipt_paths_for_entry(entry, &self.store_root),
                LaneBlockApplicationReceiptArtifact::FORMAT_LABEL,
            ),
        ];
        for ((data_path, index_path), kind) in pairs {
            // Complete any already-durable rewrite before deciding whether a
            // fresh optional compaction can afford another temporary pair.
            // Otherwise the crash temp is counted once in physical usage and
            // again as projected headroom, so a tight-cap startup can refuse
            // the very promotion needed to make retirement drainable.
            let before_recovery = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
            let recovery_accounting = self.begin_total_disk_usage_mutation();
            if !Self::recover_indexed_sidecar_artifacts(&data_path, &index_path, kind) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    format!("{kind} terminal-frontier recovery failed"),
                ));
            }
            let before = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
            self.update_disk_usage_delta(before_recovery, before);
            recovery_accounting.finish();
            // Rewrites publish one data/index temp pair at a time. The temp
            // payload cannot exceed the current pair; allow one additional
            // based-index header for a legacy-to-based retained prefix. This
            // maintenance is optional and must never consume Queue terminal
            // authority or spin the startup caller when configured headroom is
            // unavailable.
            if self.max_disk_usage_bytes != 0 {
                let temp_peak = before
                    .checked_add(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64)
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.clone(),
                            "lane history compaction temporary accounting overflowed",
                        )
                    })?;
                let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
                let certified_bundle_reservations =
                    self.certified_bundle_capacity_reserved_bytes()?;
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
                            data_path.clone(),
                            "lane history compaction configured accounting overflowed",
                        )
                    })?;
                if required > self.max_disk_usage_bytes {
                    iroha_logger::debug!(
                        lane = %entry.lane_id.as_u32(),
                        kind,
                        required,
                        limit = self.max_disk_usage_bytes,
                        "skipping bounded lane-history compaction without configured temp headroom"
                    );
                    return Ok(LaneHistoryCompactionOutcome::CapacityBlocked);
                }
            }
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            if !Self::prune_indexed_sidecars_through_terminal_frontier(
                &data_path,
                &index_path,
                frontier.lane_block_height,
                retention,
                kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    format!("{kind} terminal-frontier compaction failed"),
                ));
            }
            let after = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
            self.update_disk_usage_delta(before, after);
            accounting_mutation.finish();
        }
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
