// Durable block publication and atomic sidecar persistence primitives.

impl Kura {
    fn blocks_root_debug_file_bytes(root: &Path) -> Result<u64> {
        let path = root.join("blocks.jsonl");
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(Error::IO(error, path)),
        };
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || !Self::sidecar_is_single_link(&metadata)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "debug block dump is not a single-link regular file",
                ),
                path,
            ));
        }
        Ok(metadata.len())
    }

    fn read_durable_hash_at_height(
        block_store: &mut BlockStore,
        height: u64,
    ) -> Result<Option<HashOf<BlockHeader>>> {
        let durable_count = block_store.read_durable_index_count()?;
        if durable_count < height {
            return Ok(None);
        }
        Ok(block_store
            .read_block_hashes(height.saturating_sub(1), 1)?
            .first()
            .copied())
    }

    fn ensure_durable_block_at_height(&self, height: u64, hash: HashOf<BlockHeader>) -> Result<()> {
        let mut block_store = self.block_store.lock();
        match Self::read_durable_hash_at_height(&mut block_store, height)? {
            Some(existing) if existing == hash => Ok(()),
            Some(expected) => Err(Error::BlockHeightConflict {
                height,
                expected,
                actual: hash,
            }),
            None => {
                let expected_next_height =
                    block_store.read_durable_index_count()?.saturating_add(1);
                Err(Error::BlockHeightGap {
                    expected_next_height,
                    actual_height: height,
                })
            }
        }
    }

    /// Append a debug dump while the caller holds prune and canonical-chain
    /// locks. Pending canonical bytes are captured before the lower-order
    /// geometry/sidecar capacity locks.
    fn append_debug_block_dump(&self, block: &Arc<SignedBlock>) {
        let path_guard = self.block_plain_text_path.lock();
        let Some(path) = path_guard.clone() else {
            return;
        };
        let mut bytes = Vec::new();
        if let Err(error) = norito::json::to_writer(&mut bytes, block.as_ref()) {
            warn!(?error, path = %path.display(), "Failed to encode debug block dump");
            return;
        }
        bytes.push(b'\n');
        let pending_canonical_bytes = match self
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
        {
            Ok(bytes) => bytes,
            Err(error) => {
                debug!(
                    ?error,
                    path = %path.display(),
                    "skipping debug block dump because pending canonical capacity is unavailable"
                );
                return;
            }
        };
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        if let Err(error) = self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            false,
            false,
            &path,
        ) {
            debug!(
                ?error,
                path = %path.display(),
                "skipping debug block dump to preserve reserved Kura capacity"
            );
            return;
        }
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        match self.append_bound_debug_block_dump(&path, &bytes) {
            Ok((before, after)) => {
                self.update_disk_usage_delta(before, after);
                accounting_mutation.finish();
            }
            Err(error) => warn!(
                ?error,
                path = %path.display(),
                "Failed to append debug block dump"
            ),
        }
    }

    fn append_bound_debug_block_dump(&self, path: &Path, bytes: &[u8]) -> Result<(u64, u64)> {
        let parent = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, "debug block dump has no parent"),
                path.to_path_buf(),
            )
        })?;
        let directory = Self::open_bound_progress_directory(&self.store_root, parent)?;
        let namespace = BoundProgressNamespace {
            data_path: path.to_path_buf(),
            index_path: path.to_path_buf(),
            directories: vec![directory],
        };
        let Some(expected) = Self::regular_sidecar_metadata_for(&self.store_root, path, parent)?
        else {
            if !Self::progress_mutation_namespace_unchanged(&namespace)
                || !self.write_atomic_synced_noclobber(path, bytes)?
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "debug block dump appeared while proving no-clobber creation",
                    ),
                    path.to_path_buf(),
                ));
            }
            let created = Self::regular_sidecar_metadata_for(&self.store_root, path, parent)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "debug block dump disappeared after no-clobber creation",
                        ),
                        path.to_path_buf(),
                    )
                })?;
            if created.file.len() != u64::try_from(bytes.len())?
                || !Self::progress_mutation_namespace_unchanged(&namespace)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "debug block dump identity changed after no-clobber creation",
                    ),
                    path.to_path_buf(),
                ));
            }
            return Ok((0, created.file.len()));
        };
        let before = expected.file.len();
        let mut file = Self::open_bound_progress_file(&namespace, path, &expected)?;
        file.seek(SeekFrom::End(0))
            .and_then(|_| file.write_all(bytes))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let after = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let current = Self::regular_sidecar_metadata_for(&self.store_root, path, parent)?
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "debug block dump disappeared after bound append",
                    ),
                    path.to_path_buf(),
                )
            })?;
        if !Self::sidecar_metadata_same_object(&expected.file, &after)
            || !Self::sidecar_metadata_same_object(&after, &current.file)
            || !Self::sidecar_is_single_link(&after)
            || !Self::progress_mutation_namespace_unchanged(&namespace)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "debug block dump identity changed during bound append",
                ),
                path.to_path_buf(),
            ));
        }
        Ok((before, after.len()))
    }

    #[cfg(test)]
    fn persist_block_at_height(&self, block: &Arc<SignedBlock>, height: u64) -> Result<()> {
        let write_guard = self.lock_block_store_for_write();
        self.persist_block_at_height_while_locked(block, height, &write_guard)
    }

    fn persist_block_at_height_while_locked(
        &self,
        block: &Arc<SignedBlock>,
        height: u64,
        _write_guard: &parking_lot::MutexGuard<'_, ()>,
    ) -> Result<()> {
        self.ensure_canonical_storage_not_poisoned()?;
        #[cfg(test)]
        if self.fail_next_block_write.swap(false, Ordering::Relaxed) {
            return Err(Error::IO(
                std::io::Error::other("kura store_block injected failure"),
                PathBuf::from("block_store_test_fail"),
            ));
        }

        let start_height = height.saturating_sub(1);
        self.ensure_no_pending_rollback()?;
        let mut block_store = self.block_store.lock();
        let block_store_before = match Self::block_store_tracked_bytes(&mut block_store) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                warn!(?err, "failed to measure block store bytes before append");
                None
            }
        };
        let total_initialized = self.disk_usage_total_initialized.load(Ordering::Relaxed);
        let da_before = if total_initialized {
            match Self::da_payload_bytes_for_range(&block_store, start_height, 1) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    warn!(?err, "failed to measure DA payload bytes before append");
                    None
                }
            }
        } else {
            None
        };
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let mut accounting_complete =
            block_store_before.is_some() && (!total_initialized || da_before.is_some());
        if let Err(error) = block_store.append_block_batch_at(
            start_height,
            std::slice::from_ref(block),
            self.max_disk_usage_bytes,
        ) {
            if matches!(error, Error::DaBlockRewriteCommitStateUnknown { .. }) {
                self.poison_canonical_storage("ambiguous DA block rewrite publication", &error);
            }
            return Err(error);
        }
        if let Some(message) = block_store.take_deferred_da_recovery_fault() {
            let recovery_error = Error::IO(
                std::io::Error::other(message),
                block_store.da_block_rewrite_stage_path(),
            );
            self.poison_canonical_storage("committed DA block rewrite recovery", &recovery_error);
            return Err(Error::CanonicalBlockCommittedRecoveryRequired {
                detail: format!(
                    "DA rewrite marker committed but body promotion could not complete: {recovery_error}"
                ),
            });
        }
        if let Err(error) = block_store.flush_pending_fsync(true) {
            let publication_is_ambiguous =
                matches!(error, Error::DaBlockRewriteCommitStateUnknown { .. });
            drop(block_store);
            if publication_is_ambiguous {
                self.poison_canonical_storage("ambiguous block append publication", &error);
            }
            return Err(error);
        }
        let rewritten_index = block_store.read_block_index(start_height)?;
        if !rewritten_index.is_evicted()
            && let Err(error) = block_store.remove_da_block_file(height)
        {
            warn!(
                ?error,
                height, "canonical inline rewrite is durable but stale DA-sidecar cleanup failed"
            );
            self.record_writer_fault("stale DA-sidecar cleanup", &error);
        }

        if let Some(block_store_before) = block_store_before {
            match Self::block_store_tracked_bytes(&mut block_store) {
                Ok(after_bytes) => self.update_disk_usage_delta(block_store_before, after_bytes),
                Err(err) => {
                    accounting_complete = false;
                    warn!(?err, "failed to measure block store bytes after append");
                }
            }
        }
        if let Some(da_before) = da_before {
            match Self::da_payload_bytes_for_range(&block_store, start_height, 1) {
                Ok(da_after) => self.update_total_disk_usage_delta(da_before, da_after),
                Err(err) => {
                    accounting_complete = false;
                    warn!(?err, "failed to measure DA payload bytes after append");
                }
            }
        }
        match usize::try_from(height) {
            Ok(persisted_count) => self.publish_durable_budget_snapshot(persisted_count, 0),
            Err(err) => {
                warn!(?err, height, "failed to cache Kura durable budget metadata");
                self.invalidate_durable_budget_snapshot();
            }
        }
        if accounting_complete {
            accounting_mutation.finish();
        }
        Ok(())
    }

    fn store_block_durable(
        &self,
        block: &Arc<SignedBlock>,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resolve_canonical_storage_before_mutation()?;
        let blocks_dir = self.active_blocks_dir.lock().clone();
        self.resolve_retained_block_rewrite_stage_before_canonical_mutation(&blocks_dir)?;
        let block_hash = block.hash();
        let actual_height = block.header().height().get();
        let actual_height_usize = usize::try_from(actual_height)?;
        match (Self::block_merge_reference(block), merge_entry) {
            (Some(reference), Some(entry)) if reference.matches_entry(entry) => {}
            (Some(reference), None) => {
                return Err(Error::MissingCertifiedMergeSidecar {
                    entry_hash: reference.entry_hash,
                });
            }
            (Some(_), Some(_)) => {
                return Err(Error::MergeReferenceMismatch(
                    "block compact reference does not match supplied merge entry".to_owned(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::MergeReferenceMismatch(
                    "merge entry supplied for a block without a compact reference".to_owned(),
                ));
            }
            (None, None) => {}
        }
        if let Some(entry) = merge_entry {
            Self::validate_merge_transaction_uniqueness(block, entry)?;
        }

        {
            let block_data = self.block_data.lock();
            self.ensure_prune_recovery_not_required()?;
            Self::validate_next_or_existing_block(
                block_data.as_slice(),
                actual_height,
                actual_height_usize,
                block_hash,
            )?;
            if actual_height <= u64::try_from(block_data.len())? {
                let chain_len = block_data.len();
                drop(block_data);
                self.ensure_existing_block_wire_matches(block, actual_height, block_hash)?;
                if let Some(entry) = merge_entry {
                    self.preflight_committed_merge_entry_for_block(block, entry)?;
                    let associated = self.associated_merge_entry_for_block(block)?;
                    if associated.as_ref() != Some(entry) {
                        self.persist_pending_certified_merge_entry(entry)?;
                    }
                }
                self.persist_lane_payload_ownership_artifacts_for_block(block)?;
                self.set_block_height_index_entry(actual_height_usize, block_hash);
                if let Some(entry) = merge_entry {
                    self.set_transaction_entrypoint_index_entry_with_merge(
                        actual_height_usize,
                        block,
                        None,
                        chain_len,
                        false,
                    );
                    self.append_committed_merge_entry_for_block_if_missing(block, entry)?;
                    self.ensure_post_wsv_lane_artifact_budget_reservation_under_prune_and_canonical_guards(
                        entry,
                        actual_height,
                        block_hash,
                    )
                    .map_err(|error| {
                        self.committed_recovery_failure(
                            "post-WSV lane artifact budget reservation",
                            &error,
                        )
                    })?;
                } else {
                    self.set_transaction_entrypoint_index_entry(
                        actual_height_usize,
                        block,
                        chain_len,
                        None,
                    );
                }
                debug!(
                    height = actual_height,
                    ?block_hash,
                    "block already durably stored in Kura"
                );
                return Ok(());
            }
        }

        if let Some(entry) = merge_entry {
            self.preflight_committed_merge_entry_for_block(block, entry)?;
        }
        self.check_storage_budget(block, merge_entry)?;
        if let Some(entry) = merge_entry {
            // The exact full entry is the recovery source for every crash after
            // the canonical block commit point, including direct callers that
            // did not arrive through the pending sidecar transport.
            self.persist_pending_certified_merge_entry(entry)?;
            #[cfg(test)]
            self.maybe_pause_store_after_pending_merge_stage_for_tests();
        }
        let mut lane_artifacts = self.stage_lane_payload_ownership_artifacts_for_block(
            block,
            LaneBlockArtifactConflictPolicy::PreserveCanonical,
        )?;

        // Lane-artifact staging, when present, already owns `sidecar_lock`. Canonical mutation
        // therefore follows one global order: sidecar -> block-store write -> block_data.
        let write_guard = self.lock_block_store_for_write();
        let mut block_data = self.block_data.lock();
        self.ensure_prune_recovery_not_required()?;
        Self::validate_next_or_existing_block(
            block_data.as_slice(),
            actual_height,
            actual_height_usize,
            block_hash,
        )?;
        if actual_height <= u64::try_from(block_data.len())? {
            let chain_len = block_data.len();
            drop(block_data);
            self.ensure_existing_block_wire_matches(block, actual_height, block_hash)?;
            if let Some(entry) = merge_entry {
                self.preflight_committed_merge_entry_for_block(block, entry)?;
            }
            if let Some(batch) = lane_artifacts.take() {
                batch.commit();
            }
            self.set_block_height_index_entry(actual_height_usize, block_hash);
            if let Some(entry) = merge_entry {
                self.set_transaction_entrypoint_index_entry_with_merge(
                    actual_height_usize,
                    block,
                    None,
                    chain_len,
                    false,
                );
                self.append_committed_merge_entry_for_block_if_missing(block, entry)?;
                self.ensure_post_wsv_lane_artifact_budget_reservation_under_prune_and_canonical_guards(
                    entry,
                    actual_height,
                    block_hash,
                )
                .map_err(|error| {
                    self.committed_recovery_failure(
                        "post-WSV lane artifact budget reservation",
                        &error,
                    )
                })?;
            } else {
                self.set_transaction_entrypoint_index_entry(
                    actual_height_usize,
                    block,
                    chain_len,
                    None,
                );
            }
            debug!(
                height = actual_height,
                ?block_hash,
                "block was durably stored in Kura while waiting to append"
            );
            return Ok(());
        }

        if let Some(entry) = merge_entry {
            // Recheck after all fallible staging and while the canonical height
            // is still exclusively reserved. Deterministic binding conflicts
            // must fail before the block becomes irrevocable.
            self.preflight_committed_merge_entry_for_block(block, entry)?;
        }

        self.write_canonical_association_stage(block, merge_entry)?;
        if let Err(err) =
            self.persist_block_at_height_while_locked(block, actual_height, &write_guard)
        {
            if matches!(
                err,
                Error::DaBlockRewriteCommitStateUnknown { .. }
                    | Error::CanonicalBlockCommittedRecoveryRequired { .. }
                    | Error::CanonicalStoragePoisoned
            ) {
                // Startup resolves the marker first, then applies or discards the durable
                // association stage against the selected canonical block hash.
                return Err(err);
            }
            if let Some(mut batch) = lane_artifacts.take()
                && let Err(rollback_err) = batch.rollback()
            {
                error!(
                    ?rollback_err,
                    ?block_hash,
                    "Failed to rollback lane artifacts after block write failure"
                );
            }
            self.remove_canonical_association_stage()?;
            return Err(err);
        }

        if let Some(batch) = lane_artifacts.take() {
            batch.commit();
        }
        block_data.push((block_hash, Some(Arc::clone(block))));
        Self::drop_persisted_blocks(
            &mut block_data,
            actual_height_usize,
            self.blocks_in_memory.get(),
        );
        let new_len = block_data.len();
        self.set_block_height_index_entry(actual_height_usize, block_hash);
        // The canonical block is now durable, but its compact merge reference
        // is not query-complete until the full entry, sparse carrier record,
        // and exact finality projection are all durable. Passing no entry
        // records that prepublication frontier.
        self.set_transaction_entrypoint_index_entry(actual_height_usize, block, new_len, None);
        drop(block_data);
        // Apply associations only after block_data and the durable marker agree. The durable
        // stage remains authoritative across any post-commit association failure.
        if let Err(association_error) = self.recover_canonical_association_stage() {
            return Err(self.committed_recovery_failure(
                "committed canonical association recovery",
                &association_error,
            ));
        }
        if let Some(entry) = merge_entry {
            self.ensure_post_wsv_lane_artifact_budget_reservation_under_prune_and_canonical_guards(
                entry,
                actual_height,
                block_hash,
            )
            .map_err(|error| {
                self.committed_recovery_failure("post-WSV lane artifact budget reservation", &error)
            })?;
        }
        self.append_debug_block_dump(block);

        if let Some(entry) = merge_entry {
            // The block fsync above is the Kura commit point. From here on all
            // repair is monotonic: never truncate the block, lane artifacts, or
            // a successfully appended merge frame when a later write fails.
            if let Err(err) = self.append_committed_merge_entry_for_block_if_missing(block, entry) {
                error!(
                    ?err,
                    ?block_hash,
                    entry_epoch = entry.epoch_id,
                    "Failed to publish merge-ledger association after canonical block commit"
                );
                return Err(self.committed_recovery_failure(
                    "committed merge-ledger association publication",
                    &err,
                ));
            }
            // Canonical-association recovery already attempted the
            // post-commit cleanup. Preserve its redundant repair sidecar when
            // that best-effort removal failed; an idempotent store retry uses
            // the existing-block branch above to remove it.
        }

        debug!(
            height = actual_height,
            new_len,
            ?block_hash,
            "stored block durably in Kura"
        );

        Ok(())
    }

    fn validate_next_or_existing_block(
        block_data: &[(HashOf<BlockHeader>, Option<Arc<SignedBlock>>)],
        actual_height: u64,
        actual_height_usize: usize,
        block_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        let expected_next_height = u64::try_from(block_data.len())?.saturating_add(1);

        if actual_height < expected_next_height {
            let index = actual_height_usize.saturating_sub(1);
            if let Some((expected, _)) = block_data.get(index) {
                if *expected == block_hash {
                    return Ok(());
                }
                return Err(Error::BlockHeightConflict {
                    height: actual_height,
                    expected: *expected,
                    actual: block_hash,
                });
            }
        }

        if actual_height > expected_next_height {
            return Err(Error::BlockHeightGap {
                expected_next_height,
                actual_height,
            });
        }

        Ok(())
    }

    fn file_len_or_zero(path: &Path) -> Result<u64> {
        if path.as_os_str().is_empty() {
            return Ok(0);
        }
        match std::fs::metadata(path) {
            Ok(meta) => Ok(meta.len()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(0),
            Err(err) => Err(Error::IO(err, path.to_path_buf())),
        }
    }

    fn read_block_data_from_file(
        file: &mut FileWrap,
        start_location_in_data_file: u64,
        dest_buffer: &mut [u8],
    ) -> Result<()> {
        file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            file.read_exact(dest_buffer)
        })
    }

    fn write_atomic_synced_replace(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        self.write_atomic_synced_impl(path, bytes, true).map(|_| ())
    }

    fn write_atomic_synced_noclobber(&self, path: &Path, bytes: &[u8]) -> Result<bool> {
        self.write_atomic_synced_impl(path, bytes, false)
    }

    fn write_atomic_synced_impl(
        &self,
        path: &Path,
        bytes: &[u8],
        allow_replace: bool,
    ) -> Result<bool> {
        self.write_atomic_synced_impl_with_prefix(path, bytes, allow_replace, ".kura-sidecar-")
    }

    fn write_atomic_synced_impl_with_prefix(
        &self,
        path: &Path,
        bytes: &[u8],
        allow_replace: bool,
        temporary_prefix: &str,
    ) -> Result<bool> {
        self.durable_mutation_authorized()?;
        let parent = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "atomic sidecar path has no parent directory",
                ),
                path.to_path_buf(),
            )
        })?;
        let (_, directory_before) = self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "atomic sidecar directory does not exist",
                ),
                parent.to_path_buf(),
            )
        })?;
        let mut temporary = tempfile::Builder::new()
            .prefix(temporary_prefix)
            .tempfile_in(parent)
            .map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        let (_, directory_after_create) =
            self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "atomic sidecar directory disappeared",
                    ),
                    parent.to_path_buf(),
                )
            })?;
        if !Self::sidecar_metadata_same_object(&directory_before, &directory_after_create) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "atomic sidecar directory changed while creating the temporary file",
                ),
                parent.to_path_buf(),
            ));
        }
        let temporary_metadata = temporary
            .as_file()
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !temporary_metadata.is_file() || !Self::sidecar_is_single_link(&temporary_metadata) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "atomic sidecar temporary path is not a single-link regular file",
                ),
                path.to_path_buf(),
            ));
        }
        temporary
            .as_file_mut()
            .write_all(bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        temporary
            .as_file_mut()
            .flush()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        #[cfg(test)]
        if self
            .fail_next_atomic_write_after_temporary_sync
            .swap(false, Ordering::Relaxed)
        {
            let (_temporary_file, temporary_path) = temporary
                .keep()
                .map_err(|error| Error::IO(error.error, path.to_path_buf()))?;
            return Err(Error::IO(
                std::io::Error::other(
                    "injected atomic publication failure after temporary fsync and before rename",
                ),
                temporary_path,
            ));
        }
        let (_, directory_before_persist) =
            self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "atomic sidecar directory disappeared before rename",
                    ),
                    parent.to_path_buf(),
                )
            })?;
        if !Self::sidecar_metadata_same_object(&directory_before, &directory_before_persist) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "atomic sidecar directory changed before rename",
                ),
                parent.to_path_buf(),
            ));
        }
        let persisted = if allow_replace {
            temporary
                .persist(path)
                .map_err(|error| Error::IO(error.error, path.to_path_buf()))?
        } else {
            match temporary.persist_noclobber(path) {
                Ok(file) => file,
                Err(error) if error.error.kind() == ErrorKind::AlreadyExists => return Ok(false),
                Err(error) => return Err(Error::IO(error.error, path.to_path_buf())),
            }
        };
        persisted
            .sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let persisted_metadata = persisted
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let path_metadata = std::fs::symlink_metadata(path)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let (_, directory_after_persist) =
            self.canonical_sidecar_directory(parent)?.ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "atomic sidecar directory disappeared after rename",
                    ),
                    parent.to_path_buf(),
                )
            })?;
        if !Self::sidecar_metadata_same_object(&directory_before, &directory_after_persist)
            || path_metadata.file_type().is_symlink()
            || !path_metadata.is_file()
            || !Self::sidecar_file_metadata_unchanged(&persisted_metadata, &path_metadata)
            || persisted_metadata.len() != u64::try_from(bytes.len())?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "atomic sidecar changed during durable rename",
                ),
                path.to_path_buf(),
            ));
        }
        sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        Ok(true)
    }

    #[cfg(test)]
    fn fail_next_atomic_write_after_temporary_sync_for_test(&self) {
        self.fail_next_atomic_write_after_temporary_sync
            .store(true, Ordering::Relaxed);
    }
}
