// Canonical prune recovery stages that consume the capacity admission sealed in
// `KuraPruneIntentV3`.
impl Kura {
    fn truncate_pipeline_sidecars_for_prune(&self, intent: &KuraPruneIntentV3) -> Result<()> {
        let target_height = intent.target_height;
        let remaining = self.reconcile_and_project_prune_sidecar_rewrites_locked(target_height)?;
        if !intent.sidecar_rewrite.authorizes(remaining) {
            return Err(Error::PruneIntentConflict(
                "remaining canonical sidecar rewrite exceeds its authenticated prune projection"
                    .to_owned(),
            ));
        }
        self.validate_recovered_prune_capacity(intent, remaining)?;
        let directory = self.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
        self.truncate_indexed_sidecar_to_height(
            &directory.join(PIPELINE_SIDECARS_DATA_FILE),
            &directory.join(PIPELINE_SIDECARS_INDEX_FILE),
            target_height,
            "pipeline recovery sidecar",
            remaining.pipeline,
        )?;
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
        Self::validate_indexed_sidecar_pair(
            &directory.join(PIPELINE_SIDECARS_DATA_FILE),
            &directory.join(PIPELINE_SIDECARS_INDEX_FILE),
            max_height,
            "pipeline recovery sidecar",
            require_compact,
            true,
        )?;
        Ok(())
    }
    fn preflight_recovered_prune_capacity_before_mutation(
        &self,
        intent: &KuraPruneIntentV3,
    ) -> Result<KuraPruneSidecarRewriteProjectionV3> {
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
        self.validate_recovered_prune_capacity(intent, remaining)?;
        Ok(remaining)
    }
    fn complete_recovered_prune_intent(&self, intent: &KuraPruneIntentV3) -> Result<()> {
        // Reconciliation may remove only the interrupted prune suffix. The
        // retained log and carrier index must already be exactly aligned before
        // recovery mutates any remaining canonical sidecars.
        self.validate_committed_merge_carrier_alignment()?;
        self.preflight_recovered_prune_capacity_before_mutation(intent)?;
        // Merge reconciliation runs before this method and uses the durable
        // block height to remove future carriers and their merge-log suffix.
        let blocks_dir = self.active_blocks_dir.lock().clone();
        self.prune_retained_block_records_from(
            &blocks_dir,
            intent.target_height.saturating_add(1),
        )?;
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
