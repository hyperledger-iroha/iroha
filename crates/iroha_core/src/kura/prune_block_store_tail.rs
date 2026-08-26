// Included at `kura` module scope; keep the extracted methods on `BlockStore`.
impl BlockStore {
    /// Prune the block storage to the given height
    ///
    /// Removes block entries higher than the given height from
    /// the data file, index file, and hashes file.
    ///
    /// This function **does not** fail if the data in files is behind
    /// the given height.
    ///
    /// # Errors
    ///
    /// - If files do not exist (call [`Self::create_files_if_they_do_not_exist`])
    /// - Other IO errors
    pub(crate) fn prune(&mut self, height: u64) -> Result<()> {
        self.prune_with_failpoint(height, 0)
    }
    fn maybe_fail_prune_after_stage(fail_stage: usize, stage: usize) {
        #[cfg(test)]
        if fail_stage == stage {
            panic!("injected block-store prune crash after stage {stage}");
        }
        #[cfg(not(test))]
        let _ = (fail_stage, stage);
    }
    fn prune_with_failpoint(&mut self, height: u64, fail_stage: usize) -> Result<()> {
        self.recover_canonical_storage_stages()?;
        self.prune_durable(height, fail_stage)
    }
    fn prune_durable(&mut self, height: u64, fail_stage: usize) -> Result<()> {
        self.invalidate_data_mmap();
        let logical_count = self.read_index_count_from_len()?;
        let durable_count = self.read_durable_index_count()?;
        let pruned_index_count = height.min(logical_count).min(durable_count);
        // The current prune marker is the sole forward-recovery authority and
        // is published before any destructive work.
        self.publish_commit_marker(pruned_index_count)?;
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_BLOCK_MARKER);
        {
            let mut file =
                FileWrap::open_read_write(self.path_to_blockchain.join(INDEX_FILE_NAME))?;
            let len = file.try_io(|file| file.metadata().map(|metadata| metadata.len()))?;
            let new_len = (BlockIndex::SIZE * pruned_index_count).min(len);
            file.try_io(|file| {
                file.set_len(new_len)?;
                file.sync_data()
            })?;
        }
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_BLOCK_INDEX);
        {
            let mut file =
                FileWrap::open_read_write(self.path_to_blockchain.join(HASHES_FILE_NAME))?;
            let len = file.try_io(|file| file.metadata().map(|metadata| metadata.len()))?;
            let new_len = (SIZE_OF_BLOCK_HASH * pruned_index_count).min(len);
            file.try_io(|file| {
                file.set_len(new_len)?;
                file.sync_data()
            })?;
        }
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_BLOCK_HASHES);
        {
            let mut file = FileWrap::open_read_write(self.path_to_blockchain.join(DATA_FILE_NAME))?;
            let len = file.try_io(|file| file.metadata().map(|metadata| metadata.len()))?;
            let new_len = self.data_end_for_index_prefix(pruned_index_count)?.min(len);
            file.try_io(|file| {
                file.set_len(new_len)?;
                file.sync_data()
            })?;
        }
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_BLOCK_DATA);
        self.prune_da_block_files_above(pruned_index_count)?;
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_DA_SIDECARS);
        self.commit_marker_pending = None;
        self.commit_marker_count = pruned_index_count;
        if self
            .read_verified_snapshot_tail_marker()?
            .is_some_and(|marker| pruned_index_count < marker.snapshot_height)
        {
            self.remove_verified_snapshot_tail_marker()?;
        }
        Ok(())
    }
}
