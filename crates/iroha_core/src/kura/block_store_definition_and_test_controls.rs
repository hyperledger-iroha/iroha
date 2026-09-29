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
}
#[cfg(test)]
impl Kura {
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
    pub(crate) fn fail_next_store_for_tests(&self) {
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
    read_scratch: Vec<u8>,
    data_mmap: Option<MemoryMirror>,
    data_mmap_len: u64,
    /// Canonical inline-body bytes read through either block-store read path.
    #[cfg(test)]
    body_bytes_read: AtomicU64,
    #[cfg(test)]
    body_read_calls: AtomicUsize,
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
