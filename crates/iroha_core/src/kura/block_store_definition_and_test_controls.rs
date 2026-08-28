#[cfg(test)]
impl Kura {
    /// Inject a semantically valid selected-keeper observation in focused
    /// storage tests which do not own the keeper private key.  Production
    /// ingress can only use [`Self::admit_kura_replica_advert`].
    pub(crate) fn record_block_replica_advert(
        &self,
        peer: PeerId,
        height: u64,
        block_hash: HashOf<BlockHeader>,
        executed_block_wire_len: u64,
    ) {
        if height == 0 || executed_block_wire_len == 0 {
            return;
        }
        let blocks_dir = self.active_blocks_dir.lock().clone();
        let Ok(Some(authority)) =
            self.verified_kura_replica_authority_for_eviction(&blocks_dir, height, block_hash)
        else {
            return;
        };
        if authority.key.executed_block_wire_len != executed_block_wire_len {
            return;
        }
        let Some((keeper_index, _)) = authority
            .selected_keepers
            .iter()
            .find(|(_, keeper)| keeper == &peer)
        else {
            return;
        };
        self.replica_registry
            .lock()
            .entry(authority.key)
            .or_default()
            .insert(
                peer,
                BlockReplicaAdvert {
                    keeper_index: *keeper_index,
                    observed_at: Instant::now(),
                },
            );
    }
}
#[cfg(any(test, feature = "bench", feature = "iroha-core-tests"))]
impl Kura {
    /// Persist a benchmark block directly into the canonical block store.
    ///
    /// # Errors
    /// Returns an error if the block cannot be appended or the tracked block-store byte usage
    /// cannot be measured.
    pub fn persist_block_immediate_for_bench(&self, block: &Arc<SignedBlock>) -> Result<()> {
        self.durable_mutation_authorized()?;
        let _write_guard = self.block_store_write_lock.lock();
        self.ensure_no_retired_rollback_intents()?;
        let mut store = self.block_store.lock();
        let before_bytes = Self::block_store_tracked_bytes(&mut store)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        store.append_block_to_chain(block.as_ref())?;
        let after_bytes = Self::block_store_tracked_bytes(&mut store)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        let persisted_count = usize::try_from(block.header().height().get())?;
        self.publish_durable_budget_snapshot(persisted_count, 0);
        accounting_mutation.finish();
        Ok(())
    }
    /// Append an in-memory pending block for storage-budget benchmark scenarios.
    pub fn append_pending_block_for_bench(&self, block: Arc<SignedBlock>) {
        if self.durable_mutation_authorized().is_err() {
            return;
        }
        let hash = block.hash();
        self.block_data.lock().push((hash, Some(block)));
        self.invalidate_pending_budget_cache();
    }
    /// Run storage-budget accounting without storing a block.
    pub fn check_storage_budget_for_bench(&self, block: &SignedBlock) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resolve_canonical_storage_before_mutation()?;
        self.check_storage_budget(block, None)
    }
    /// Advertise enough matching remote replicas for the block at `height`.
    #[must_use]
    pub fn advertise_required_replicas_for_bench(&self, height: NonZeroUsize) -> Option<u64> {
        let height_u64 = u64::try_from(height.get()).ok()?;
        let (block_hash, payload_len, blocks_dir) = {
            let index = u64::try_from(height.get().saturating_sub(1)).ok()?;
            let mut store = self.block_store.lock();
            let payload_len = store.read_block_index(index).ok()?.length;
            let block_hash = store.read_block_hashes(index, 1).ok()?.first().copied()?;
            (block_hash, payload_len, store.path_to_blockchain.clone())
        };
        if payload_len == 0 {
            return None;
        }
        let authority = self
            .verified_kura_replica_authority_for_eviction(&blocks_dir, height_u64, block_hash)
            .ok()??;
        if authority.selected_keepers.is_empty() {
            return None;
        }
        if self.local_peer_id.get().is_none() {
            let mut local = checked_peer_id();
            while authority
                .selected_keepers
                .iter()
                .any(|(_, keeper)| keeper == &local)
            {
                local = checked_peer_id();
            }
            self.bind_local_peer_id(local).ok()?;
        }
        let now = Instant::now();
        let mut registry = self.replica_registry.lock();
        let peers = registry.entry(authority.key).or_default();
        for (keeper_index, keeper) in &authority.selected_keepers {
            peers.insert(
                keeper.clone(),
                BlockReplicaAdvert {
                    keeper_index: *keeper_index,
                    observed_at: now,
                },
            );
        }
        Some(payload_len)
    }
    /// Evict persisted block bodies for benchmark scenarios.
    ///
    /// # Errors
    /// Returns an error if Kura cannot read, rewrite, or atomically replace block-store files.
    pub fn evict_block_bodies_for_bench(&self, bytes_needed: u64) -> Result<u64> {
        self.evict_block_bodies(bytes_needed)
    }
}
#[cfg(any(test, feature = "iroha-core-tests"))]
impl Kura {
    /// Remove only the reverse carrier record while retaining its committed full entry.
    ///
    /// This models the finalized block-first crash seam repaired at startup.
    #[cfg(test)]
    pub(crate) fn remove_merge_carrier_record_for_testing(
        &self,
        block: &SignedBlock,
        entry: &MergeLedgerEntry,
    ) -> Result<()> {
        let record = Self::carrier_record_for_block_entry(block, entry)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _carrier_guard = self.merge_carrier_lock.lock();
        self.remove_merge_carrier_record_unlocked(record)
    }
    /// Remove the local DA cache only after a canonical body was genuinely evicted.
    ///
    /// This test-only hook models a remote-only historical block so downstream
    /// proof-serving regressions cannot accidentally succeed by re-decoding the
    /// complete local body instead of the immutable retained record.
    ///
    /// # Errors
    ///
    /// Returns an error when `height` is absent, still inline, or its local
    /// sidecar cannot be removed.
    pub fn remove_evicted_block_sidecar_for_testing(&self, height: NonZeroUsize) -> Result<()> {
        let index = u64::try_from(height.get().saturating_sub(1))?;
        let accounting_mutation = {
            let mut store = self.block_store.lock();
            let block_index = store.read_block_index(index)?;
            if !block_index.is_evicted() {
                let path = store.da_block_path(u64::try_from(height.get())?);
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "cannot remove a DA sidecar for a block whose canonical body is still inline",
                    ),
                    path,
                ));
            }
            let height = u64::try_from(height.get())?;
            let path = store.da_block_path(height);
            let before_bytes = Self::file_len_or_zero(&path)?;
            if before_bytes == 0 {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "cannot remove an absent evicted-block DA sidecar",
                    ),
                    path,
                ));
            }
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            store.remove_da_block_file(height)?;
            self.update_total_disk_usage_delta(before_bytes, 0);
            accounting_mutation
        };
        if let Some((_, cached)) = self
            .block_data
            .lock()
            .get_mut(height.get().saturating_sub(1))
        {
            *cached = None;
        }
        accounting_mutation.finish();
        Ok(())
    }
    /// Remove only the newest exact Native AMX application manifest record.
    ///
    /// This test-only hook creates the crash shape where a durable receipt and
    /// latest-route pointer outlive their QC-authenticated manifest. Combined
    /// with an evicted remote-only carrier, startup repair must fetch the
    /// canonical body from a CommitQC signer before it can recreate the exact
    /// manifest and revalidate the receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the route/incarnation/application identity does
    /// not match the active newest manifest or the standalone removal fails.
    pub fn remove_latest_native_amx_participant_manifest_for_testing(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        participant_height: u64,
        application_block_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        if participant_height == 0 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "cannot remove a zero-height Native AMX manifest",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        if entry.dataspace_id != dataspace_id {
            return Err(Self::invalid_lane_artifact_error(
                entry.blocks_dir(&self.store_root),
                "Native AMX manifest removal targets another dataspace",
            ));
        }
        let (active_incarnation, _) = self.active_lane_incarnation_marker(&entry)?;
        if active_incarnation != lane_incarnation {
            return Err(Self::invalid_lane_artifact_error(
                entry.blocks_dir(&self.store_root),
                "Native AMX manifest removal targets an inactive lane incarnation",
            ));
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        let namespace = self.native_amx_evidence_namespace_for_entry(&entry)?;
        self.complete_native_amx_evidence_prune_intent_locked(&entry, &namespace)?;
        self.recover_native_amx_evidence_publication_temp_locked(
            &entry,
            &namespace,
            NativeAmxEvidenceRecoveryPhase::Startup,
        )?;
        let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, false)?;
        let path = Self::native_amx_application_manifest_path_for_entry(
            &entry,
            &self.store_root,
            participant_height,
        );
        let artifact = self
            .read_native_amx_participant_application_manifest_from_paths_locked(
                &entry,
                participant_height,
                &path,
                &namespace,
            )
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "exact Native AMX manifest to remove is unavailable",
                )
            })?;
        let leaf = &artifact.leaf;
        if leaf.lane_incarnation != lane_incarnation
            || leaf.application_block_hash != application_block_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native AMX manifest removal identity mismatch",
            ));
        }
        if inventory
            .manifests
            .last_key_value()
            .map(|(height, _)| *height)
            != Some(participant_height)
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native AMX manifest removal is restricted to the newest record",
            ));
        }
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let before_bytes = self.native_amx_evidence_tracked_bytes_locked(&namespace)?;
        Self::remove_bound_progress_temp_if_present(&namespace, &path)
            .map_err(|error| Error::IO(error, path.clone()))?;
        self.sync_native_amx_evidence_namespace(
            &namespace,
            NativeAmxParticipantApplicationManifestArtifactV1::FORMAT_LABEL,
        )?;
        let after_bytes = self.native_amx_evidence_tracked_bytes_locked(&namespace)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        accounting_mutation.finish();
        Ok(())
    }
}
#[cfg(test)]
impl Kura {
    /// Persist one canonical test block and its exact retained SCCP archive.
    pub(crate) fn persist_block_with_retained_archive_for_tests(
        &self,
        block: &Arc<SignedBlock>,
    ) -> Result<()> {
        self.store_block(Arc::clone(block))?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let height = block.header().height().get();
        let canonical_hash = block.hash();
        self.ensure_durable_block_at_height(height, canonical_hash)?;
        let blocks_dir = self.active_blocks_dir.lock().clone();
        self.persist_retained_block_record(&blocks_dir, canonical_hash, block.as_ref())
    }
    pub(crate) fn persist_block_immediate_for_tests(&self, block: &Arc<SignedBlock>) {
        let _write_guard = self.block_store_write_lock.lock();
        let mut store = self.block_store.lock();
        let before_bytes = Self::block_store_tracked_bytes(&mut store)
            .expect("measure block store bytes before test append");
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        store
            .append_block_to_chain(block.as_ref())
            .expect("persist block for tests");
        let after_bytes = Self::block_store_tracked_bytes(&mut store)
            .expect("measure block store bytes after test append");
        self.update_disk_usage_delta(before_bytes, after_bytes);
        match usize::try_from(block.header().height().get()) {
            Ok(persisted_count) => self.publish_durable_budget_snapshot(persisted_count, 0),
            Err(_) => self.invalidate_durable_budget_snapshot(),
        }
        accounting_mutation.finish();
    }
    fn pause_next_store_after_pending_merge_stage_for_tests(&self) {
        self.store_paused_after_pending_merge_stage
            .store(false, Ordering::Release);
        self.pause_store_after_pending_merge_stage
            .store(true, Ordering::Release);
    }
    fn store_paused_after_pending_merge_stage_for_tests(&self) -> bool {
        self.store_paused_after_pending_merge_stage
            .load(Ordering::Acquire)
    }
    fn resume_store_after_pending_merge_stage_for_tests(&self) {
        self.store_paused_after_pending_merge_stage
            .store(false, Ordering::Release);
    }
    fn pause_next_eviction_after_snapshot_for_tests(&self) {
        self.eviction_paused_after_snapshot
            .store(false, Ordering::Release);
        self.pause_eviction_after_snapshot
            .store(true, Ordering::Release);
    }
    fn eviction_paused_after_snapshot_for_tests(&self) -> bool {
        self.eviction_paused_after_snapshot.load(Ordering::Acquire)
    }
    fn resume_eviction_after_snapshot_for_tests(&self) {
        self.eviction_paused_after_snapshot
            .store(false, Ordering::Release);
    }
    fn pause_next_eviction_before_stage_publication_for_tests(&self) {
        self.eviction_paused_before_stage_publication
            .store(false, Ordering::Release);
        self.pause_eviction_before_stage_publication
            .store(true, Ordering::Release);
    }
    fn eviction_paused_before_stage_publication_for_tests(&self) -> bool {
        self.eviction_paused_before_stage_publication
            .load(Ordering::Acquire)
    }
    fn resume_eviction_before_stage_publication_for_tests(&self) {
        self.eviction_paused_before_stage_publication
            .store(false, Ordering::Release);
    }
    fn pause_next_block_read_before_cache_recheck_for_tests(&self) {
        self.block_read_paused_before_cache_recheck
            .store(false, Ordering::Release);
        self.pause_block_read_before_cache_recheck
            .store(true, Ordering::Release);
    }
    fn block_read_paused_before_cache_recheck_for_tests(&self) -> bool {
        self.block_read_paused_before_cache_recheck
            .load(Ordering::Acquire)
    }
    fn resume_block_read_before_cache_recheck_for_tests(&self) {
        self.block_read_paused_before_cache_recheck
            .store(false, Ordering::Release);
    }
    fn force_next_durable_blocks_count_fallback_for_tests(&self) {
        self.durable_blocks_count_fallback_reached
            .store(false, Ordering::Release);
        self.force_durable_blocks_count_fallback
            .store(true, Ordering::Release);
    }
    fn durable_blocks_count_fallback_reached_for_tests(&self) -> bool {
        self.durable_blocks_count_fallback_reached
            .load(Ordering::Acquire)
    }
    fn pause_next_hash_only_extension_before_store_for_tests(&self) {
        self.hash_only_extension_paused_before_store
            .store(false, Ordering::Release);
        self.pause_hash_only_extension_before_store
            .store(true, Ordering::Release);
    }
    fn hash_only_extension_paused_before_store_for_tests(&self) -> bool {
        self.hash_only_extension_paused_before_store
            .load(Ordering::Acquire)
    }
    fn resume_hash_only_extension_before_store_for_tests(&self) {
        self.hash_only_extension_paused_before_store
            .store(false, Ordering::Release);
    }
    fn pause_next_total_disk_usage_scan_after_scan_for_tests(&self) {
        self.total_disk_usage_scan_paused
            .store(false, Ordering::Release);
        self.pause_total_disk_usage_scan_after_scan
            .store(true, Ordering::Release);
    }
    fn total_disk_usage_scan_paused_for_tests(&self) -> bool {
        self.total_disk_usage_scan_paused.load(Ordering::Acquire)
    }
    fn resume_total_disk_usage_scan_for_tests(&self) {
        self.total_disk_usage_scan_paused
            .store(false, Ordering::Release);
    }
    fn fail_retained_rewrite_discard_after_for_tests(&self, removed_index: usize) {
        self.fail_retained_rewrite_discard_after
            .store(removed_index, Ordering::Release);
    }
    fn fail_next_retained_rewrite_recovery_for_tests(&self) {
        self.fail_next_retained_rewrite_recovery
            .store(true, Ordering::Release);
    }
    /// Return raw cache state together with independent exact scans without refreshing caches.
    pub(crate) fn disk_usage_accounting_snapshot_for_tests(
        &self,
    ) -> Result<DiskUsageAccountingSnapshotForTesting> {
        Ok(DiskUsageAccountingSnapshotForTesting {
            enforced_initialized: self.disk_usage_initialized.load(Ordering::Acquire),
            total_initialized: self.disk_usage_total_initialized.load(Ordering::Acquire),
            cached_enforced_bytes: self.disk_usage.load(Ordering::Relaxed),
            cached_total_bytes: self.disk_usage_total.load(Ordering::Relaxed),
            exact_enforced_bytes: self.kura_disk_usage_bytes()?,
            exact_total_bytes: self.kura_total_disk_usage_bytes()?,
        })
    }
    fn fail_next_retired_tree_purge_after_one_removal_for_tests(&self) {
        self.fail_next_retired_tree_purge_after_one_removal
            .store(true, Ordering::Release);
    }
    pub(crate) fn fail_next_store_for_tests(&self) {
        self.fail_next_block_write.store(true, Ordering::Relaxed);
    }
    pub(crate) fn fail_next_block_write_for_tests(&self) {
        self.fail_next_block_write.store(true, Ordering::Relaxed);
    }
    #[cfg(test)]
    pub(crate) fn poison_canonical_storage_for_tests(&self) {
        self.poison_canonical_storage(
            "injected preexisting canonical-storage poison",
            &Error::CanonicalStoragePoisoned,
        );
    }
    #[cfg(test)]
    pub(crate) fn overwrite_commit_marker_for_tests(&self, bytes: &[u8]) -> Result<()> {
        let store = self.block_store.lock();
        let path = store.commit_marker_path();
        std::fs::write(&path, bytes).map_err(|error| Error::IO(error, path))
    }
    #[cfg(test)]
    pub(crate) fn publish_exact_commit_marker_for_tests(&self) -> Result<()> {
        let mut store = self.block_store.lock();
        let index_len = store.index_file_len()?;
        let hashes_len = store.hashes_file_len()?;
        if index_len % BlockIndex::SIZE != 0 || hashes_len % SIZE_OF_BLOCK_HASH != 0 {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "cannot publish a test marker for a partial canonical journal",
                ),
                store.path_to_blockchain.clone(),
            ));
        }
        let index_count = index_len / BlockIndex::SIZE;
        let hashes_count = hashes_len / SIZE_OF_BLOCK_HASH;
        if index_count != hashes_count {
            return Err(Error::HashesFileHeightMismatch);
        }
        store.write_commit_marker(index_count)?;
        let marker = store.read_commit_marker()?.ok_or_else(|| {
            Error::IO(
                std::io::Error::new(ErrorKind::NotFound, "published commit marker is missing"),
                store.commit_marker_path(),
            )
        })?;
        store.validate_commit_marker_tip(&marker, hashes_count)?;
        if marker.count != index_count {
            return Err(Error::HashesFileHeightMismatch);
        }
        store.commit_marker_count = index_count;
        Ok(())
    }
    pub(crate) fn fail_next_wsv_checkpoint_write_for_tests(&self) {
        self.fail_next_wsv_checkpoint_write
            .store(true, Ordering::Relaxed);
    }
    pub(crate) fn fail_next_commit_manifest_write_for_tests(&self) {
        self.fail_next_commit_manifest_write
            .store(true, Ordering::Relaxed);
    }
    pub(crate) fn fail_prune_after_stage_for_tests(&self, stage: usize) {
        self.fail_prune_after_stage.store(stage, Ordering::Relaxed);
    }
    pub(crate) fn fail_prune_sidecar_promotion_for_tests(&self, stage: usize) {
        self.fail_prune_sidecar_promotion_stage
            .store(stage, Ordering::Relaxed);
    }
    pub(crate) fn fail_next_v2_finality_write_for_tests(&self) {
        self.fail_next_v2_finality_write
            .store(true, Ordering::Relaxed);
    }
    pub(crate) fn fail_next_native_amx_prepublication_for_tests(&self) {
        self.fail_next_native_amx_prepublication
            .store(true, Ordering::Relaxed);
    }
    #[cfg(test)]
    pub(crate) fn fail_progress_sidecar_ancestor_sync_attempts_for_tests(
        &self,
        ancestor_index: usize,
        failures: usize,
    ) {
        fail_progress_sidecar_ancestor_sync_for_tests(ancestor_index, failures);
    }
    /// Replace manifest bytes without updating the checkpoint digest, for corruption tests.
    #[cfg(test)]
    pub(crate) fn overwrite_commit_manifest_without_binding_for_tests(
        &self,
        manifest: &CommitManifest,
    ) -> Result<()> {
        self.ensure_durable_block_at_height(manifest.height, manifest.block_hash)?;
        let path = self.commit_manifest_path(manifest.height);
        let dir = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::other("manifest path has no parent"),
                path.clone(),
            )
        })?;
        std::fs::create_dir_all(dir).map_err(|err| Error::IO(err, dir.to_path_buf()))?;
        std::fs::write(&path, manifest.encode()).map_err(|err| Error::IO(err, path))
    }
    /// Remove manifest bytes without updating the checkpoint digest, for corruption tests.
    #[cfg(test)]
    pub(crate) fn remove_commit_manifest_without_binding_for_tests(
        &self,
        height: u64,
    ) -> Result<()> {
        let path = self.commit_manifest_path(height);
        std::fs::remove_file(&path).map_err(|err| Error::IO(err, path))
    }
    /// Remove checkpoint bytes without changing any companion sidecar, for corruption tests.
    #[cfg(test)]
    pub(crate) fn remove_wsv_checkpoint_without_binding_for_tests(
        &self,
        height: u64,
    ) -> Result<()> {
        let path = self.wsv_checkpoint_path(height);
        std::fs::remove_file(&path).map_err(|err| Error::IO(err, path))
    }
    /// Replace checkpoint state and optional manifest binding without validating either value.
    ///
    /// This deliberately bypasses the production publication protocol so replay tests can model
    /// independently corrupted and mutually correlated sidecars.
    #[cfg(test)]
    pub(crate) fn overwrite_wsv_checkpoint_without_validation_for_tests(
        &self,
        height: u64,
        state_hash: Hash,
        manifest: Option<&CommitManifest>,
    ) -> Result<()> {
        let path = self.wsv_checkpoint_path(height);
        let Some(mut checkpoint) = Self::decode_wsv_checkpoint_at(&path)? else {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::NotFound, "WSV checkpoint is missing"),
                path,
            ));
        };
        checkpoint.state_hash = state_hash;
        checkpoint.commit_manifest_hash = manifest.map(CommitManifest::encoded_hash);
        std::fs::write(&path, checkpoint.encode()).map_err(|err| Error::IO(err, path))
    }
    /// Remove v2 finality bytes without changing the durable block or manifest, for tests.
    #[cfg(test)]
    pub(crate) fn remove_v2_finality_without_binding_for_tests(&self, height: u64) -> Result<()> {
        let path = self.v2_finality_artifact_path(height);
        std::fs::remove_file(&path).map_err(|err| Error::IO(err, path))
    }
    /// Replace durable v2-finality bytes without decoding them, for corruption tests.
    #[cfg(test)]
    pub(crate) fn overwrite_v2_finality_bytes_for_tests(
        &self,
        height: u64,
        bytes: &[u8],
    ) -> Result<()> {
        let path = self.v2_finality_artifact_path(height);
        std::fs::write(&path, bytes).map_err(|err| Error::IO(err, path))
    }
    /// Replace the artifact inside an existing finality envelope without validation, for tests.
    #[cfg(test)]
    pub(crate) fn overwrite_v2_finality_without_validation_for_tests(
        &self,
        height: u64,
        artifact: V2FinalityArtifact,
    ) -> Result<()> {
        let path = self.v2_finality_artifact_path(height);
        let dir = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::other("v2 finality path has no parent"),
                path.clone(),
            )
        })?;
        let Some((mut record, _)) = self.decode_v2_finality_record_at(&path, dir)? else {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::NotFound, "v2 finality sidecar is missing"),
                path,
            ));
        };
        record.artifact = artifact;
        std::fs::write(&path, record.encode()).map_err(|err| Error::IO(err, path))
    }
}
/// Loaded block count
#[derive(Clone, Copy, Debug)]
pub struct BlockCount(pub usize);
/// Low-level filesystem block store used internally by [`Kura`].
///
/// Its public mutation surface is intentionally limited to initializing and appending an offline
/// store for tooling such as Kagami. A running node must mutate canonical storage through [`Kura`]
/// so authentication, poisoning, recovery, and lock-order checks remain enforced.
pub struct BlockStore {
    path_to_blockchain: PathBuf,
    da_blocks_dir: PathBuf,
    read_only: bool,
    data_file: Option<FileWrap>,
    index_file: Option<FileWrap>,
    hashes_file: Option<FileWrap>,
    fsync: FsyncState,
    fsync_telemetry: FsyncTelemetry,
    encode_scratch: Vec<u8>,
    read_scratch: Vec<u8>,
    data_mmap: Option<MemoryMirror>,
    data_mmap_len: u64,
    /// Canonical inline-body bytes read through either block-store read path.
    #[cfg(test)]
    body_bytes_read: AtomicU64,
    /// Durable prefix validated read-only before emergency Fast recovery.
    fast_prevalidated_count: Option<u64>,
    commit_marker_count: u64,
    commit_marker_pending: Option<u64>,
    /// Committed DA rewrite whose body promotion must be retried before the next mutation.
    deferred_da_recovery_fault: Option<String>,
    /// Test hook for failing after a DA rewrite is staged and journal files are written, but before
    /// its commit marker is published.
    #[cfg(test)]
    fail_next_da_rewrite_before_marker: AtomicBool,
    /// Test hook for failing after a DA rewrite marker is durable but before body promotion.
    #[cfg(test)]
    fail_next_da_rewrite_after_marker: AtomicBool,
    /// Test hook for failing the immediate staged recovery attempted after marker publication.
    #[cfg(test)]
    fail_next_da_rewrite_recovery: AtomicBool,
    /// Test-only abrupt-stop boundary after journal writes and before marker publication.
    #[cfg(test)]
    crash_next_da_rewrite_before_marker: AtomicBool,
    /// Test-only abrupt-stop boundary after marker publication and before body promotion.
    #[cfg(test)]
    crash_next_da_rewrite_after_marker: AtomicBool,
    /// Test hook for failing before the next atomic commit-marker write.
    #[cfg(test)]
    fail_next_commit_marker_write: AtomicBool,
    /// Test hook for stopping after the deterministic marker temp is synced.
    #[cfg(test)]
    fail_next_commit_marker_after_temp_sync: AtomicBool,
    /// Test hook for failing the next commit-marker readback.
    #[cfg(test)]
    fail_next_commit_marker_read: AtomicBool,
    /// Test hook for failing acknowledgement after a marker was atomically persisted and synced.
    #[cfg(test)]
    fail_next_commit_marker_ack_after_persist: AtomicBool,
    /// Test hook for a pre-persist marker failure followed by an unreadable marker state.
    #[cfg(test)]
    fail_next_commit_marker_write_and_readback: AtomicBool,
    /// Test hook for a persisted new marker followed by acknowledgement/readback failure.
    #[cfg(test)]
    fail_next_commit_marker_ack_and_readback: AtomicBool,
    /// Test-only abrupt-stop boundary after an eviction compaction stage is durable.
    #[cfg(test)]
    crash_next_eviction_after_stage: AtomicBool,
    /// Test-only abrupt-stop boundary after replacement data is promoted.
    #[cfg(test)]
    crash_next_eviction_after_data_promotion: AtomicBool,
    /// Test-only abrupt-stop boundary after both replacement files are promoted.
    #[cfg(test)]
    crash_next_eviction_after_index_promotion: AtomicBool,
    /// Test-only count of stage durability acknowledgements to fail.
    #[cfg(test)]
    fail_eviction_stage_syncs_remaining: AtomicUsize,
}
impl Debug for BlockStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockStore")
            .field("path_to_blockchain", &self.path_to_blockchain)
            .field("da_blocks_dir", &self.da_blocks_dir)
            .field("read_only", &self.read_only)
            .field("data_file_open", &self.data_file.is_some())
            .field("index_file_open", &self.index_file.is_some())
            .field("hashes_file_open", &self.hashes_file.is_some())
            .field("fsync_mode", &self.fsync.mode)
            .field("fsync_pending", &self.fsync.pending_since.is_some())
            .field("fsync_telemetry", &self.fsync_telemetry)
            .field("encode_scratch_len", &self.encode_scratch.len())
            .field("read_scratch_len", &self.read_scratch.len())
            .field(
                "mirror_kind",
                &self.data_mmap.as_ref().map(MemoryMirror::kind),
            )
            .field("mmap_len", &self.data_mmap_len)
            .field("commit_marker_count", &self.commit_marker_count)
            .field("commit_marker_pending", &self.commit_marker_pending)
            .finish()
    }
}
impl BlockStore {
    fn read_required_bounded_commit_marker_bytes(
        path: &Path,
        missing_reason: &'static str,
    ) -> Result<Vec<u8>> {
        Self::read_bounded_commit_marker_bytes(path)?.ok_or_else(|| {
            Error::IO(
                std::io::Error::new(ErrorKind::NotFound, missing_reason),
                path.to_path_buf(),
            )
        })
    }
    fn maybe_fail_commit_marker_after_temp_sync(&self, temporary_path: &Path) -> Result<()> {
        #[cfg(test)]
        if self
            .fail_next_commit_marker_after_temp_sync
            .swap(false, Ordering::AcqRel)
        {
            return Err(Error::IO(
                std::io::Error::other("injected crash after deterministic commit-marker temp sync"),
                temporary_path.to_path_buf(),
            ));
        }
        #[cfg(not(test))]
        let _ = temporary_path;
        Ok(())
    }
}
