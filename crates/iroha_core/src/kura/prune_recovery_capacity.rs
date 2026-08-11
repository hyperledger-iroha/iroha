// Canonical prune recovery stages that consume the capacity admission sealed in
// `KuraPruneIntentV2`.

impl Kura {
    fn truncate_pipeline_sidecars_for_prune(&self, intent: &KuraPruneIntentV2) -> Result<()> {
        let target_height = intent.target_height;
        let remaining = self.reconcile_and_project_prune_sidecar_rewrites_locked(target_height)?;
        if !intent.sidecar_rewrite.authorizes(remaining) {
            return Err(Error::PruneIntentConflict(
                "remaining canonical sidecar rewrite exceeds its authenticated prune projection"
                    .to_owned(),
            ));
        }
        let remaining_roster = self
            .roster_log
            .read()
            .project_truncate_to_height(target_height)
            .map_err(|error| {
                Error::PruneIntentConflict(format!(
                    "failed to re-project commit-roster prune capacity: {error}"
                ))
            })?;
        if !intent.capacity.roster.authorizes(remaining_roster) {
            return Err(Error::PruneIntentConflict(
                "remaining commit-roster state exceeds its authenticated prune projection"
                    .to_owned(),
            ));
        }
        self.validate_recovered_prune_capacity(intent, remaining_roster, remaining)?;
        let directory = self.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
        for (data_file, index_file, kind, expected) in [
            (
                PIPELINE_SIDECARS_DATA_FILE,
                PIPELINE_SIDECARS_INDEX_FILE,
                "pipeline recovery sidecar",
                remaining.pipeline,
            ),
            (
                ROSTER_SIDECARS_DATA_FILE,
                ROSTER_SIDECARS_INDEX_FILE,
                "roster metadata sidecar",
                remaining.roster,
            ),
        ] {
            self.truncate_indexed_sidecar_to_height(
                &directory.join(data_file),
                &directory.join(index_file),
                target_height,
                kind,
                expected,
            )?;
        }
        if directory.exists() {
            for entry in
                std::fs::read_dir(&directory).map_err(|err| Error::IO(err, directory.clone()))?
            {
                let entry = entry.map_err(|err| Error::IO(err, directory.clone()))?;
                let path = entry.path();
                let Some(height) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_prefix("block_"))
                    .and_then(|name| name.strip_suffix(".json"))
                    .and_then(|height| height.parse::<u64>().ok())
                else {
                    continue;
                };
                if height > target_height {
                    let file_type = entry
                        .file_type()
                        .map_err(|err| Error::IO(err, path.clone()))?;
                    if !file_type.is_file() && !file_type.is_symlink() {
                        return Err(Error::PruneIntentConflict(format!(
                            "pipeline JSON suffix entry {} is not removable as a file",
                            path.display()
                        )));
                    }
                    std::fs::remove_file(&path).map_err(|err| Error::IO(err, path))?;
                }
            }
            sync_dir(&directory).map_err(|err| Error::IO(err, directory))?;
        }
        self.pipeline_sidecar_queue
            .lock()
            .retain(|sidecar| sidecar.height <= target_height);
        self.fastpq_proof_queue
            .lock()
            .retain(|snapshot| snapshot.snapshot.height <= target_height);
        Ok(())
    }

    fn validate_pipeline_sidecars_for_prune(
        &self,
        max_height: u64,
        require_compact: bool,
    ) -> Result<()> {
        let directory = self.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
        for (data_file, index_file, kind) in [
            (
                PIPELINE_SIDECARS_DATA_FILE,
                PIPELINE_SIDECARS_INDEX_FILE,
                "pipeline recovery sidecar",
            ),
            (
                ROSTER_SIDECARS_DATA_FILE,
                ROSTER_SIDECARS_INDEX_FILE,
                "roster metadata sidecar",
            ),
        ] {
            Self::validate_indexed_sidecar_pair(
                &directory.join(data_file),
                &directory.join(index_file),
                max_height,
                kind,
                require_compact,
                true,
            )?;
        }
        if directory.exists() {
            for entry in
                std::fs::read_dir(&directory).map_err(|err| Error::IO(err, directory.clone()))?
            {
                let entry = entry.map_err(|err| Error::IO(err, directory.clone()))?;
                let path = entry.path();
                let future_json = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_prefix("block_"))
                    .and_then(|name| name.strip_suffix(".json"))
                    .and_then(|height| height.parse::<u64>().ok())
                    .is_some_and(|height| height > max_height);
                if future_json {
                    return Err(Error::PruneIntentConflict(format!(
                        "pipeline JSON sidecar extends above canonical height {max_height}: {}",
                        path.display()
                    )));
                }
            }
        }
        Ok(())
    }

    fn truncate_roster_for_prune(
        &self,
        intent: &KuraPruneIntentV2,
        remaining_sidecar: KuraPruneSidecarRewriteProjectionV2,
    ) -> Result<()> {
        let height = intent.target_height;
        let before = self.roster_journal_tracked_bytes()?;
        {
            // Keep the shared in-memory fence unchanged until the replacement
            // generation is durable. A post-intent failure is fail-stop, but
            // readers must still not observe a journal state never published.
            let mut roster_log = self.roster_log.write();
            let remaining_roster =
                roster_log
                    .project_truncate_to_height(height)
                    .map_err(|err| {
                        Error::PruneIntentConflict(format!(
                            "failed to project commit-roster journal at height {height}: {err}"
                        ))
                    })?;
            if !intent.capacity.roster.authorizes(remaining_roster) {
                return Err(Error::PruneIntentConflict(
                    "remaining commit-roster state exceeds its authenticated prune projection"
                        .to_owned(),
                ));
            }
            self.validate_recovered_prune_capacity(intent, remaining_roster, remaining_sidecar)?;
            let mut candidate = roster_log.clone();
            candidate
                .truncate_to_height_with_projection(height, intent.capacity.roster)
                .map_err(|err| {
                    Error::PruneIntentConflict(format!(
                        "failed to truncate commit-roster journal to height {height}: {err}"
                    ))
                })?;
            *roster_log = candidate;
        }
        let after = self.roster_journal_tracked_bytes()?;
        self.update_disk_usage_delta(before, after);
        Ok(())
    }

    fn preflight_recovered_prune_capacity_before_mutation(
        &self,
        intent: &KuraPruneIntentV2,
    ) -> Result<KuraPruneSidecarRewriteProjectionV2> {
        // Normalize at most one non-growing sequential temp stage, authenticate
        // the exact remaining projection, and reject insufficient configured
        // physical capacity before any new retained-pair allocation or other
        // forward-recovery mutation.
        let _guard = self.sidecar_lock.lock();
        let remaining =
            self.reconcile_and_project_prune_sidecar_rewrites_locked(intent.target_height)?;
        if !intent.sidecar_rewrite.authorizes(remaining) {
            return Err(Error::PruneIntentConflict(
                "startup sidecar rewrite exceeds its authenticated prune projection".to_owned(),
            ));
        }
        let remaining_roster = self
            .roster_log
            .read()
            .project_truncate_to_height(intent.target_height)
            .map_err(|error| {
                Error::PruneIntentConflict(format!(
                    "failed to recover commit-roster prune projection: {error}"
                ))
            })?;
        if !intent.capacity.roster.authorizes(remaining_roster) {
            return Err(Error::PruneIntentConflict(
                "startup commit-roster state exceeds its authenticated prune projection".to_owned(),
            ));
        }
        self.validate_recovered_prune_capacity(intent, remaining_roster, remaining)?;
        Ok(remaining)
    }

    fn complete_recovered_prune_intent(&self, intent: &KuraPruneIntentV2) -> Result<()> {
        let remaining_sidecar = self.preflight_recovered_prune_capacity_before_mutation(intent)?;
        // Merge reconciliation runs before this method and uses the durable
        // block height to remove future carriers and their merge-log suffix.
        let blocks_dir = self.active_blocks_dir.lock().clone();
        self.prune_retained_block_records_from(
            &blocks_dir,
            intent.target_height.saturating_add(1),
        )?;
        self.truncate_roster_for_prune(intent, remaining_sidecar)?;
        {
            let _guard = self.sidecar_lock.lock();
            let wsv_dir = self.wsv_checkpoint_dir();
            Self::prune_wsv_checkpoints_above_in_dir(&wsv_dir, intent.target_height)?;
            let manifest_dir = self.commit_manifest_dir();
            Self::prune_commit_manifests_above_in_dir(&manifest_dir, intent.target_height)?;
            let finality_dir = self.v2_finality_artifact_dir();
            Self::prune_v2_finality_artifacts_above_in_dir(&finality_dir, intent.target_height)?;
            for directory in Self::kagemusha_finality_sidecar_dirs_for(&blocks_dir) {
                Self::prune_commit_manifests_above_in_dir(&directory, intent.target_height)?;
            }
            self.truncate_pipeline_sidecars_for_prune(intent)?;
        }
        self.validate_completed_prune_intent(intent)?;
        self.finish_prune_intent()
    }
}
