// Pipeline recovery metadata and strict indexed-sidecar persistence primitives.
impl Kura {
    /// Enqueue pipeline recovery metadata for asynchronous persistence.
    ///
    /// This avoids consensus-path I/O; the Kura writer thread flushes the queue.
    /// If the queue is full, the sidecar is rejected because pipeline recovery
    /// metadata is best-effort diagnostic state. An active or interrupted canonical
    /// prune also rejects immediately without waiting for disk mutation locks.
    pub fn enqueue_pipeline_metadata(
        &self,
        sidecar: PipelineRecoverySidecar,
    ) -> PipelineSidecarEnqueueResult {
        if self.emergency_fast_startup_enabled() {
            return PipelineSidecarEnqueueResult::RejectedEmergencyFast;
        }
        if self.durable_mutation_authorized().is_err() {
            return PipelineSidecarEnqueueResult::RejectedUnauthorized;
        }
        if self.prune_blocks_sidecar_enqueue() {
            return PipelineSidecarEnqueueResult::RejectedPruneRecovery;
        }
        let cap = self
            .pipeline_sidecar_queue_cap
            .load(Ordering::Relaxed)
            .max(1);
        let (should_notify, queue_depth) = {
            let mut queue = self.pipeline_sidecar_queue.lock();
            if self.prune_blocks_sidecar_enqueue() {
                return PipelineSidecarEnqueueResult::RejectedPruneRecovery;
            }
            if queue.len() >= cap {
                return PipelineSidecarEnqueueResult::RejectedQueueFull { cap };
            }
            let should_notify = queue.is_empty();
            queue.push_back(sidecar);
            (should_notify, queue.len())
        };
        if should_notify {
            self.notify_block_writer(BlockNotify::NewBlock, "pipeline sidecar");
        }
        PipelineSidecarEnqueueResult::Enqueued { queue_depth }
    }
    fn flush_pipeline_sidecars(&self) -> usize {
        if let Err(error) = self.durable_mutation_authorized() {
            iroha_logger::warn!(
                ?error,
                "refusing queued pipeline sidecar mutation while Kura output is unauthorized"
            );
            return 0;
        }
        let sidecars = {
            let mut queue = self.pipeline_sidecar_queue.lock();
            if queue.is_empty() {
                return 0;
            }
            queue.drain(..).collect::<Vec<_>>()
        };
        let count = sidecars.len();
        for sidecar in sidecars {
            self.write_pipeline_metadata_unlocked(&sidecar);
        }
        count
    }
    /// Enqueue a FASTPQ proof attachment for asynchronous persistence in the block sidecar.
    ///
    /// An active or interrupted canonical prune rejects immediately without waiting for disk
    /// mutation locks.
    pub fn enqueue_fastpq_proof_snapshot(
        &self,
        snapshot: FastpqProofSnapshot,
    ) -> FastpqProofEnqueueResult {
        self.enqueue_fastpq_proof_snapshot_unless(snapshot, || false)
    }
    /// Enqueue a FASTPQ proof attachment unless `cancelled` observes shutdown.
    ///
    /// Both admission checks run while holding the proof queue lock. If shutdown
    /// becomes visible after insertion, the just-inserted tail is removed before
    /// the lock is released or the block writer is notified. The predicate must
    /// therefore remain non-blocking and must not call back into Kura.
    pub(crate) fn enqueue_fastpq_proof_snapshot_unless(
        &self,
        snapshot: FastpqProofSnapshot,
        mut cancelled: impl FnMut() -> bool,
    ) -> FastpqProofEnqueueResult {
        if self.emergency_fast_startup_enabled() {
            return FastpqProofEnqueueResult::RejectedEmergencyFast;
        }
        if self.durable_mutation_authorized().is_err() {
            return FastpqProofEnqueueResult::RejectedUnauthorized;
        }
        if self.prune_blocks_sidecar_enqueue() {
            return FastpqProofEnqueueResult::RejectedPruneRecovery;
        }
        let telemetry = FastpqProofSidecarTelemetry;
        let max_bytes = self
            .fastpq_proof_sidecar_max_bytes
            .load(Ordering::Relaxed)
            .max(1);
        let actual = match norito::encode_canonical(&snapshot) {
            Ok(bytes) => bytes.len(),
            Err(err) => {
                telemetry.record_event("rejected_encode");
                iroha_logger::warn!(
                    ?err,
                    "failed to encode FASTPQ proof snapshot before enqueue"
                );
                return FastpqProofEnqueueResult::RejectedEncode {
                    reason: format!("{err:?}"),
                };
            }
        };
        if actual > max_bytes {
            telemetry.record_event("rejected_too_large");
            return FastpqProofEnqueueResult::RejectedTooLarge {
                actual,
                max: max_bytes,
            };
        }
        let cap = self
            .fastpq_proof_sidecar_queue_cap
            .load(Ordering::Relaxed)
            .max(1);
        let (queue_depth, should_notify) = {
            let mut queue = self.fastpq_proof_queue.lock();
            if self.prune_blocks_sidecar_enqueue() {
                return FastpqProofEnqueueResult::RejectedPruneRecovery;
            }
            if queue.len() >= cap {
                telemetry.record_event("rejected_queue_full");
                telemetry.set_queue_depth(queue.len());
                return FastpqProofEnqueueResult::RejectedQueueFull { cap };
            }
            if cancelled() {
                telemetry.record_event("rejected_shutdown");
                telemetry.set_queue_depth(queue.len());
                return FastpqProofEnqueueResult::RejectedShutdown;
            }
            let should_notify = queue.is_empty();
            queue.push_back(QueuedFastpqProofSnapshot {
                snapshot,
                retries: 0,
            });
            if cancelled() {
                let removed = queue.pop_back();
                debug_assert!(
                    removed.is_some(),
                    "inserted FASTPQ proof snapshot disappeared"
                );
                telemetry.record_event("rejected_shutdown");
                telemetry.set_queue_depth(queue.len());
                return FastpqProofEnqueueResult::RejectedShutdown;
            }
            (queue.len(), should_notify)
        };
        telemetry.record_event("enqueued");
        telemetry.set_queue_depth(queue_depth);
        if should_notify {
            self.notify_block_writer(BlockNotify::NewBlock, "FASTPQ proof sidecar");
        }
        FastpqProofEnqueueResult::Enqueued { queue_depth }
    }
    fn flush_fastpq_proof_snapshots(&self) -> usize {
        let telemetry = FastpqProofSidecarTelemetry;
        if let Err(error) = self.durable_mutation_authorized() {
            iroha_logger::warn!(
                ?error,
                "refusing queued FASTPQ sidecar mutation while Kura output is unauthorized"
            );
            telemetry.set_queue_depth(self.fastpq_proof_queue.lock().len());
            return 0;
        }
        let snapshots = {
            let mut queue = self.fastpq_proof_queue.lock();
            if queue.is_empty() {
                telemetry.set_queue_depth(0);
                return 0;
            }
            queue.drain(..).collect::<Vec<_>>()
        };
        let mut groups: Vec<Vec<QueuedFastpqProofSnapshot>> = Vec::new();
        for snapshot in snapshots {
            if let Some(group) = groups.iter_mut().find(|group| {
                group.first().is_some_and(|queued| {
                    queued.snapshot.height == snapshot.snapshot.height
                        && queued.snapshot.block_hash == snapshot.snapshot.block_hash
                })
            }) {
                group.push(snapshot);
            } else {
                groups.push(vec![snapshot]);
            }
        }
        let mut written = 0usize;
        let mut retry = VecDeque::new();
        let max_retries = self
            .fastpq_proof_sidecar_max_retries
            .load(Ordering::Relaxed)
            .max(1);
        for group in groups {
            let snapshots = group
                .iter()
                .map(|queued| &queued.snapshot)
                .collect::<Vec<_>>();
            match self.write_fastpq_proof_snapshots(&snapshots) {
                FastpqProofWriteResult::Written => {
                    telemetry.record_event("written");
                    written = written.saturating_add(group.len());
                }
                FastpqProofWriteResult::Retry => {
                    for mut queued in group {
                        let next_retries = queued.retries.saturating_add(1);
                        if next_retries >= max_retries {
                            telemetry.record_event("dropped");
                            iroha_logger::warn!(
                                height = queued.snapshot.height,
                                retries = next_retries,
                                max_retries,
                                "dropping FASTPQ proof snapshot after retry limit"
                            );
                        } else {
                            queued.retries = next_retries;
                            retry.push_back(queued);
                        }
                    }
                }
                FastpqProofWriteResult::Drop => {
                    for _ in group {
                        telemetry.record_event("dropped");
                    }
                }
            }
        }
        let cap = self
            .fastpq_proof_sidecar_queue_cap
            .load(Ordering::Relaxed)
            .max(1);
        let (queue_depth, requeued, dropped_for_capacity) = {
            let mut queue = self.fastpq_proof_queue.lock();
            let requeued = retry.len().min(cap.saturating_sub(queue.len()));
            queue.extend(retry.drain(..requeued));
            let dropped_for_capacity = retry.len();
            if dropped_for_capacity != 0 {
                retry.clear();
            }
            (queue.len(), requeued, dropped_for_capacity)
        };
        for _ in 0..requeued {
            telemetry.record_event("retried");
        }
        for _ in 0..dropped_for_capacity {
            telemetry.record_event("dropped");
        }
        if dropped_for_capacity != 0 {
            iroha_logger::warn!(
                dropped = dropped_for_capacity,
                cap,
                queue_depth,
                "dropping FASTPQ proof retries because concurrent enqueues filled the queue"
            );
        }
        telemetry.set_queue_depth(queue_depth);
        written
    }
    /// Write per-block pipeline recovery metadata sidecar under the store dir. Best-effort: errors
    /// are logged and ignored.
    pub fn write_pipeline_metadata(&self, sidecar: &PipelineRecoverySidecar) {
        let _prune_guard = self.prune_lock.lock();
        if self.prune_recovery_is_required() {
            warn!(
                height = sidecar.height,
                "refusing pipeline sidecar write until prune recovery completes after restart"
            );
            return;
        }
        if let Err(error) = self.durable_mutation_authorized() {
            iroha_logger::warn!(
                ?error,
                height = sidecar.height,
                "refusing pipeline sidecar mutation while Kura output is unauthorized"
            );
            return;
        }
        self.write_pipeline_metadata_unlocked(sidecar);
    }
    fn write_pipeline_metadata_unlocked(&self, sidecar: &PipelineRecoverySidecar) {
        if let Some(mut dir) = self.store_dir() {
            let _guard = self.sidecar_lock.lock();
            dir.push(PIPELINE_DIR_NAME);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                iroha_logger::warn!(?e, ?dir, "failed to create pipeline dir");
                return;
            }
            let data_path = dir.join(PIPELINE_SIDECARS_DATA_FILE);
            let index_path = dir.join(PIPELINE_SIDECARS_INDEX_FILE);
            let before_bytes = match Self::sidecar_tracked_bytes(&data_path, &index_path) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        ?dir,
                        "failed to measure pipeline sidecar bytes before write"
                    );
                    None
                }
            };
            let fsync_mode = self.sidecar_fsync_mode();
            let accounting_mutation = self.begin_total_disk_usage_mutation().with_resource_paths(
                Self::sidecar_physical_resource_paths(&data_path, &index_path),
            );
            let wrote = match sidecar.encode_framed() {
                Ok(buf) => Self::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    sidecar.height,
                    &buf,
                    "pipeline sidecar",
                    fsync_mode,
                    None,
                ),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        height = sidecar.height,
                        "failed to encode pipeline metadata"
                    );
                    false
                }
            };
            if !wrote {
                self.resource_inventory.invalidate(
                    physical_resource_mask(),
                    resource_inventory::Unavailable::Interrupted,
                );
            }
            let mut accounting_complete = before_bytes.is_some();
            if let Some(before_bytes) = before_bytes {
                match Self::sidecar_tracked_bytes(&data_path, &index_path) {
                    Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                    Err(err) => {
                        accounting_complete = false;
                        iroha_logger::warn!(
                            ?err,
                            ?dir,
                            "failed to measure pipeline sidecar bytes after write"
                        );
                    }
                }
            }
            if accounting_complete {
                accounting_mutation.finish();
            }
        }
    }
    fn write_fastpq_proof_snapshots(
        &self,
        snapshots: &[&FastpqProofSnapshot],
    ) -> FastpqProofWriteResult {
        if let Err(error) = self.durable_mutation_authorized() {
            let retry = matches!(&error, Error::SnapshotBootstrapAuthenticationPending);
            iroha_logger::warn!(
                ?error,
                "refusing FASTPQ proof sidecar mutation while Kura output is unauthorized"
            );
            return if retry {
                FastpqProofWriteResult::Retry
            } else {
                FastpqProofWriteResult::Drop
            };
        }
        let Some(first_snapshot) = snapshots.first().copied() else {
            return FastpqProofWriteResult::Written;
        };
        let height = first_snapshot.height;
        let block_hash = first_snapshot.block_hash;
        if height == 0 {
            iroha_logger::warn!("refusing to store FASTPQ proof snapshot for zero height");
            return FastpqProofWriteResult::Drop;
        }
        let Some(mut dir) = self.store_dir() else {
            iroha_logger::warn!("FASTPQ proof snapshot has no Kura store directory");
            return FastpqProofWriteResult::Drop;
        };
        let _guard = self.sidecar_lock.lock();
        dir.push(PIPELINE_DIR_NAME);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            iroha_logger::warn!(?err, ?dir, "failed to create pipeline dir for FASTPQ proof");
            return FastpqProofWriteResult::Retry;
        }
        let data_path = dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let Some(mut sidecar) = self.read_pipeline_sidecar(
            height,
            PIPELINE_SIDECARS_DATA_FILE,
            PIPELINE_SIDECARS_INDEX_FILE,
            norito::decode_canonical::<PipelineRecoverySidecar>,
            "pipeline sidecar",
        ) else {
            iroha_logger::debug!(
                height,
                "pipeline sidecar not ready for FASTPQ proof attachment"
            );
            return FastpqProofWriteResult::Retry;
        };
        if sidecar.block_hash != block_hash {
            iroha_logger::warn!(
                height,
                expected = %sidecar.block_hash,
                actual = %block_hash,
                "dropping FASTPQ proof snapshot for mismatched block hash"
            );
            return FastpqProofWriteResult::Drop;
        }
        let mut added = 0usize;
        for snapshot in snapshots {
            if snapshot.height != height || snapshot.block_hash != block_hash {
                iroha_logger::warn!(
                    height = snapshot.height,
                    expected_height = height,
                    expected_hash = %block_hash,
                    actual_hash = %snapshot.block_hash,
                    "dropping FASTPQ proof snapshot grouped with a different block"
                );
                continue;
            }
            if sidecar
                .fastpq_proofs
                .iter()
                .any(|existing| existing.same_attachment(snapshot))
            {
                continue;
            }
            sidecar.fastpq_proofs.push((*snapshot).clone());
            added = added.saturating_add(1);
        }
        if added == 0 {
            return FastpqProofWriteResult::Written;
        }
        let accounting_mutation = self.begin_total_disk_usage_mutation().with_resource_paths(
            Self::sidecar_physical_resource_paths(&data_path, &index_path),
        );
        let before_bytes = match Self::sidecar_tracked_bytes(&data_path, &index_path) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?dir,
                    "failed to measure pipeline sidecar bytes before FASTPQ proof write"
                );
                None
            }
        };
        let payload = match sidecar.encode_framed() {
            Ok(payload) => payload,
            Err(err) => {
                iroha_logger::warn!(?err, height, "failed to encode FASTPQ proof sidecar update");
                return FastpqProofWriteResult::Retry;
            }
        };
        let wrote = Self::append_indexed_sidecar(
            &data_path,
            &index_path,
            height,
            &payload,
            "pipeline sidecar",
            self.sidecar_fsync_mode(),
            None,
        );
        if wrote {
            let mut accounting_complete = before_bytes.is_some();
            if let Some(before_bytes) = before_bytes {
                match Self::sidecar_tracked_bytes(&data_path, &index_path) {
                    Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                    Err(err) => {
                        accounting_complete = false;
                        iroha_logger::warn!(
                            ?err,
                            ?dir,
                            "failed to measure pipeline sidecar bytes after FASTPQ proof write"
                        );
                    }
                }
            }
            if accounting_complete {
                accounting_mutation.finish();
            }
            FastpqProofWriteResult::Written
        } else {
            FastpqProofWriteResult::Retry
        }
    }
    /// Decode pipeline recovery metadata without assigning it canonical block authority.
    ///
    /// Callers must validate the returned sidecar against either Kura's canonical block hash or
    /// an explicit candidate block hash before using it. Keeping this helper private prevents an
    /// identity-unchecked sidecar from escaping the storage boundary.
    fn read_pipeline_metadata_payload(&self, height: u64) -> Option<PipelineRecoverySidecar> {
        if self.prune_recovery_is_required() {
            return None;
        }
        let sidecar = {
            let _guard = self.sidecar_lock.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            self.read_pipeline_sidecar(
                height,
                PIPELINE_SIDECARS_DATA_FILE,
                PIPELINE_SIDECARS_INDEX_FILE,
                norito::decode_canonical::<PipelineRecoverySidecar>,
                "pipeline sidecar",
            )
        }?;
        if sidecar.height != height {
            iroha_logger::warn!(
                height,
                sidecar_height = sidecar.height,
                "pipeline sidecar height mismatch"
            );
            return None;
        }
        Some(sidecar)
    }
    /// Read per-block pipeline recovery metadata if present. Returns `None` on errors.
    ///
    /// This canonical reader exposes a sidecar only when its block hash agrees with Kura's
    /// canonical or durable block identity for `height`.
    pub fn read_pipeline_metadata(&self, height: u64) -> Option<PipelineRecoverySidecar> {
        let sidecar = self.read_pipeline_metadata_payload(height)?;
        let expected = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .and_then(|height| {
                self.get_block_hash(height)
                    .or_else(|| self.get_durable_block_hash(height))
            });
        if expected != Some(sidecar.block_hash) {
            iroha_logger::warn!(
                height,
                expected = ?expected,
                actual = %sidecar.block_hash,
                "pipeline sidecar block hash mismatch"
            );
            return None;
        }
        if self.prune_recovery_is_required() {
            return None;
        }
        Some(sidecar)
    }
    /// Read pipeline recovery metadata for an explicitly identified candidate block.
    ///
    /// This is an execution-cache boundary, not a source of canonical block authority. It permits
    /// a speculative executor to reuse metadata that it previously persisted for the same exact
    /// block while rejecting metadata from a competing candidate at the same height.
    #[cfg(test)]
    pub(crate) fn read_pipeline_metadata_for_block(
        &self,
        height: u64,
        expected_block_hash: HashOf<BlockHeader>,
    ) -> Option<PipelineRecoverySidecar> {
        let sidecar = self.read_pipeline_metadata_payload(height)?;
        if sidecar.block_hash != expected_block_hash {
            iroha_logger::debug!(
                height,
                expected = %expected_block_hash,
                actual = %sidecar.block_hash,
                "pipeline sidecar candidate block hash mismatch"
            );
            return None;
        }
        if self.prune_recovery_is_required() {
            return None;
        }
        Some(sidecar)
    }
    /// Read persisted FASTPQ proof snapshots for a committed block.
    #[must_use]
    pub fn fastpq_proofs_for_block(&self, height: u64) -> Vec<FastpqProofSnapshot> {
        self.read_pipeline_metadata(height)
            .map(|sidecar| sidecar.fastpq_proofs)
            .unwrap_or_default()
    }

    #[must_use]
    fn recover_indexed_sidecar_artifacts(data_path: &Path, index_path: &Path, kind: &str) -> bool {
        let required_heights = BTreeSet::new();
        Self::recover_indexed_sidecar_artifacts_with_required_heights(
            data_path,
            index_path,
            &required_heights,
            kind,
        )
    }
    #[must_use]
    fn recover_indexed_sidecar_artifacts_with_required_heights(
        data_path: &Path,
        index_path: &Path,
        required_heights: &BTreeSet<u64>,
        kind: &str,
    ) -> bool {
        let temp_data_path = data_path.with_extension("norito.tmp");
        let temp_index_path = index_path.with_extension("index.tmp");
        let temp_index_exists = temp_index_path.exists();
        let temp_data_exists = temp_data_path.exists();
        if !temp_index_exists {
            if temp_data_exists {
                warn!(
                    ?temp_data_path,
                    kind, "sidecar temp data exists without temp index; failing closed"
                );
                return false;
            }
            if required_heights.is_empty() {
                return true;
            }
            let data_len = match std::fs::metadata(data_path).map(|metadata| metadata.len()) {
                Ok(data_len) => data_len,
                Err(error) => {
                    warn!(
                        ?error,
                        ?data_path,
                        kind,
                        "required terminal evidence has no readable canonical sidecar data"
                    );
                    return false;
                }
            };
            return Self::sidecar_index_contains_required_heights(
                index_path,
                data_len,
                required_heights,
                kind,
                "canonical",
            );
        }
        // A temp index is the durable commit marker for a prune rewrite. When both files remain,
        // validate them as a pair. When only the index remains, the crash happened after data
        // promotion, so validate it against main data. Never publish an index before the payload
        // it references is in its final location.
        let recovery_data_path = if temp_data_exists {
            &temp_data_path
        } else {
            data_path
        };
        let data_len = match std::fs::metadata(recovery_data_path).map(|meta| meta.len()) {
            Ok(data_len) => data_len,
            Err(err) => {
                warn!(
                    ?err,
                    ?temp_index_path,
                    ?recovery_data_path,
                    kind,
                    "failed to read sidecar data length for temp index validation"
                );
                return false;
            }
        };
        if !Self::sidecar_index_sane_with_label(&temp_index_path, data_len, kind, "temp") {
            warn!(
                ?temp_index_path,
                kind, "refusing to promote invalid sidecar temp index"
            );
            return false;
        }
        if !Self::sidecar_index_contains_required_heights(
            &temp_index_path,
            data_len,
            required_heights,
            kind,
            "temp",
        ) {
            warn!(
                ?temp_index_path,
                kind, "refusing to promote a sidecar temp that omits required terminal evidence"
            );
            return false;
        }
        if temp_data_exists && !Self::promote_sidecar_temp(&temp_data_path, data_path, kind, "data")
        {
            warn!(
                ?temp_data_path,
                kind, "sidecar temp data promotion failed; leaving temp index unpublished"
            );
            return false;
        }
        if !Self::promote_sidecar_temp(&temp_index_path, index_path, kind, "index") {
            warn!(
                ?temp_index_path,
                kind,
                "sidecar temp index promotion failed after data promotion; leaving it for recovery"
            );
            return false;
        }
        Self::sidecar_index_contains_required_heights(
            index_path,
            data_len,
            required_heights,
            kind,
            "recovered canonical",
        )
    }
    #[must_use]
    fn promote_sidecar_temp(temp_path: &Path, main_path: &Path, kind: &str, label: &str) -> bool {
        if !temp_path.exists() {
            return false;
        }
        if let Err(err) = std::fs::rename(temp_path, main_path) {
            if main_path.exists() {
                if let Err(remove_err) = std::fs::remove_file(main_path) {
                    warn!(
                        ?remove_err,
                        ?main_path,
                        kind,
                        label,
                        "failed to remove sidecar file before promoting temp"
                    );
                    return false;
                }
                if let Err(err) = std::fs::rename(temp_path, main_path) {
                    warn!(
                        ?err,
                        ?temp_path,
                        ?main_path,
                        kind,
                        label,
                        "failed to promote sidecar temp file after removal"
                    );
                    return false;
                }
            } else {
                warn!(
                    ?err,
                    ?temp_path,
                    ?main_path,
                    kind,
                    label,
                    "failed to promote sidecar temp file"
                );
                return false;
            }
        }
        if let Some(parent) = main_path.parent() {
            if let Err(err) = sync_sidecar_promotion_dir(parent) {
                warn!(
                    ?err,
                    ?parent,
                    kind,
                    label,
                    "failed to sync sidecar parent after temp promotion"
                );
                return false;
            }
        }
        true
    }
    fn sidecar_index_sane_with_label(
        index_path: &Path,
        data_len: u64,
        kind: &str,
        label: &str,
    ) -> bool {
        let mut index = match std::fs::File::open(index_path) {
            Ok(file) => file,
            Err(err) => {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to open sidecar index"
                );
                return false;
            }
        };
        let index_len = match index.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to stat sidecar index"
                );
                return false;
            }
        };
        if index_len == 0 {
            warn!(?index_path, kind, label, "sidecar index is empty");
            return false;
        }
        let layout = match SidecarIndexLayout::read_from(&mut index, index_len) {
            Ok(layout) => layout,
            Err(reason) => {
                warn!(
                    reason,
                    len = index_len,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index layout is malformed"
                );
                return false;
            }
        };
        if index_len != layout.aligned_len {
            warn!(
                len = index_len,
                aligned_len = layout.aligned_len,
                ?index_path,
                kind,
                label,
                "sidecar index length misaligned"
            );
            return false;
        }
        if layout.entry_count == 0 {
            if data_len == 0 {
                return true;
            }
            warn!(
                data_len,
                ?index_path,
                kind,
                label,
                "header-only sidecar index retains unindexed payload bytes"
            );
            return false;
        }
        if index.seek(SeekFrom::Start(layout.entries_offset)).is_err() {
            warn!(
                ?index_path,
                kind, label, "failed to seek to sidecar index entries"
            );
            return false;
        }
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        for _ in 0..layout.entry_count {
            if let Err(err) = index.read_exact(&mut buf) {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to read sidecar index entry"
                );
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(buf);
            if entry.len == 0 {
                continue;
            }
            if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
                warn!(
                    len = entry.len,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry length exceeds limit"
                );
                return false;
            }
            let entry_end = if let Some(end) = entry.offset.checked_add(entry.len) {
                end
            } else {
                warn!(
                    offset = entry.offset,
                    len = entry.len,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry overflows offset"
                );
                return false;
            };
            if entry_end > data_len {
                warn!(
                    offset = entry.offset,
                    len = entry.len,
                    data_len,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry points past data file"
                );
                return false;
            }
        }
        true
    }

    fn sidecar_index_contains_required_heights(
        index_path: &Path,
        data_len: u64,
        required_heights: &BTreeSet<u64>,
        kind: &str,
        label: &str,
    ) -> bool {
        if required_heights.is_empty() {
            return true;
        }
        let mut index = match std::fs::File::open(index_path) {
            Ok(file) => file,
            Err(error) => {
                warn!(
                    ?error,
                    ?index_path,
                    kind,
                    label,
                    "failed to open sidecar index while checking required terminal evidence"
                );
                return false;
            }
        };
        let index_len = match index.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                warn!(
                    ?error,
                    ?index_path,
                    kind,
                    label,
                    "failed to stat sidecar index while checking required terminal evidence"
                );
                return false;
            }
        };
        let layout = match SidecarIndexLayout::read_from(&mut index, index_len) {
            Ok(layout) if index_len == layout.aligned_len => layout,
            Ok(_) => {
                warn!(
                    ?index_path,
                    kind,
                    label,
                    "required terminal evidence index has trailing or misaligned bytes"
                );
                return false;
            }
            Err(reason) => {
                warn!(
                    reason,
                    ?index_path,
                    kind,
                    label,
                    "required terminal evidence index layout is malformed"
                );
                return false;
            }
        };
        for height in required_heights {
            let Some(relative) = height.checked_sub(layout.base_height) else {
                warn!(
                    height,
                    base_height = layout.base_height,
                    ?index_path,
                    kind,
                    label,
                    "required terminal evidence predates the sidecar index"
                );
                return false;
            };
            if relative >= layout.entry_count {
                warn!(
                    height,
                    base_height = layout.base_height,
                    entry_count = layout.entry_count,
                    ?index_path,
                    kind,
                    label,
                    "required terminal evidence is outside the sidecar index"
                );
                return false;
            }
            let Some(offset) = relative
                .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .and_then(|offset| layout.entries_offset.checked_add(offset))
            else {
                return false;
            };
            if index.seek(SeekFrom::Start(offset)).is_err() {
                return false;
            }
            let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
            if index.read_exact(&mut entry_bytes).is_err() {
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(entry_bytes);
            if entry.len == 0
                || entry
                    .offset
                    .checked_add(entry.len)
                    .is_none_or(|end| end > data_len)
            {
                warn!(
                    height,
                    offset = entry.offset,
                    len = entry.len,
                    data_len,
                    ?index_path,
                    kind,
                    label,
                    "required terminal evidence sidecar entry is absent or out of bounds"
                );
                return false;
            }
        }
        true
    }
    #[cfg(test)]
    fn indexed_sidecar_height_range(
        index_path: &Path,
        kind: &str,
    ) -> Option<core::ops::RangeInclusive<u64>> {
        let mut index = match std::fs::File::open(index_path) {
            Ok(index) => index,
            Err(err) => {
                iroha_logger::debug!(?err, ?index_path, kind, "sidecar index is unavailable");
                return None;
            }
        };
        let index_len = match index.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to stat sidecar index");
                return None;
            }
        };
        let layout = match SidecarIndexLayout::read_from(&mut index, index_len) {
            Ok(layout) => layout,
            Err(reason) => {
                iroha_logger::warn!(
                    reason,
                    len = index_len,
                    ?index_path,
                    kind,
                    "refusing malformed sidecar index"
                );
                return None;
            }
        };
        if index_len != layout.aligned_len {
            iroha_logger::warn!(
                len = index_len,
                aligned_len = layout.aligned_len,
                ?index_path,
                kind,
                "sidecar index length misaligned; ignoring trailing bytes"
            );
        }
        layout.height_range()
    }
    fn repair_unindexed_sidecar_tail(
        data: &std::fs::File,
        index: &mut std::fs::File,
        layout: SidecarIndexLayout,
        data_path: &Path,
        index_path: &Path,
        kind: &str,
    ) -> bool {
        let data_len = match data.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                iroha_logger::warn!(?error, ?data_path, kind, "failed to stat sidecar payload");
                return false;
            }
        };
        if index.seek(SeekFrom::Start(layout.entries_offset)).is_err() {
            iroha_logger::warn!(
                ?index_path,
                kind,
                "failed to seek sidecar index for tail repair"
            );
            return false;
        }
        let Ok(entry_capacity) = usize::try_from(layout.entry_count) else {
            iroha_logger::warn!(?index_path, kind, "sidecar index entry count exceeds usize");
            return false;
        };
        let mut ranges = Vec::with_capacity(entry_capacity.min(4096));
        let mut encoded = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for _ in 0..layout.entry_count {
            if let Err(error) = index.read_exact(&mut encoded) {
                iroha_logger::warn!(
                    ?error,
                    ?index_path,
                    kind,
                    "failed to read sidecar index during tail repair"
                );
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(encoded);
            if entry.len == 0 {
                if entry.offset != 0 {
                    iroha_logger::warn!(
                        offset = entry.offset,
                        ?index_path,
                        kind,
                        "zero-length sidecar index entry has a non-zero offset"
                    );
                    return false;
                }
                continue;
            }
            if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
                iroha_logger::warn!(
                    len = entry.len,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    ?index_path,
                    kind,
                    "sidecar index entry exceeds the payload limit during tail repair"
                );
                return false;
            }
            let Some(end) = entry.offset.checked_add(entry.len) else {
                iroha_logger::warn!(
                    offset = entry.offset,
                    len = entry.len,
                    ?index_path,
                    kind,
                    "sidecar index entry overflows during tail repair"
                );
                return false;
            };
            if end > data_len {
                iroha_logger::warn!(
                    offset = entry.offset,
                    len = entry.len,
                    data_len,
                    ?index_path,
                    kind,
                    "sidecar index points past the payload during tail repair"
                );
                return false;
            }
            ranges.push((entry.offset, end));
        }
        ranges.sort_unstable_by_key(|&(start, end)| (start, end));
        if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
            iroha_logger::warn!(
                ?index_path,
                kind,
                "sidecar index contains overlapping active payload ranges"
            );
            return false;
        }
        let indexed_end = ranges.iter().map(|&(_, end)| end).max().unwrap_or(0);
        if data_len == indexed_end {
            return true;
        }
        if let Err(error) = data.set_len(indexed_end) {
            iroha_logger::warn!(
                ?error,
                ?data_path,
                data_len,
                indexed_end,
                kind,
                "failed to truncate unindexed sidecar crash residue"
            );
            return false;
        }
        if let Err(error) = data.sync_data() {
            iroha_logger::warn!(
                ?error,
                ?data_path,
                indexed_end,
                kind,
                "failed to durably repair unindexed sidecar crash residue"
            );
            return false;
        }
        true
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn append_preceding_indexed_sidecar(
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
        should_sync: bool,
        retention: Option<NonZeroUsize>,
        layout: SidecarIndexLayout,
        namespace: Option<&BoundProgressNamespace>,
    ) -> bool {
        debug_assert!(height < layout.base_height);
        let prepend = layout.base_height - height;
        if prepend > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
            iroha_logger::warn!(
                height,
                base_height = layout.base_height,
                prepend,
                limit = MAX_INDEXED_SIDECAR_GAP_ENTRIES,
                ?index_path,
                kind,
                "refusing oversized backward sidecar index gap"
            );
            return false;
        }
        let Some(old_entries_len) = layout
            .entry_count
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
        else {
            iroha_logger::warn!(?index_path, kind, "sidecar entry byte length overflows");
            return false;
        };
        let Some(new_entry_count) = prepend.checked_add(layout.entry_count) else {
            iroha_logger::warn!(?index_path, kind, "sidecar entry count overflows");
            return false;
        };
        let new_entries_offset = INDEXED_SIDECAR_BASE_HEADER_SIZE_U64;
        let Some(projected_index_len) = new_entry_count
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
            .and_then(|entries_len| new_entries_offset.checked_add(entries_len))
        else {
            iroha_logger::warn!(?index_path, kind, "sidecar prepend length overflows");
            return false;
        };
        let data_existed = data_path.exists();
        let mut data =
            match Self::open_direct_sidecar_file_in_namespace(data_path, true, false, namespace) {
                Ok(file) => file,
                Err(err) => {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to open sidecar store");
                    return false;
                }
            };
        let mut repair_index = match Self::open_direct_sidecar_file_in_namespace(
            index_path, false, false, namespace,
        ) {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to open sidecar index");
                return false;
            }
        };
        if !Self::repair_unindexed_sidecar_tail(
            &data,
            &mut repair_index,
            layout,
            data_path,
            index_path,
            kind,
        ) {
            return false;
        }
        drop(repair_index);
        let data_len = match data.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                return false;
            }
        };
        let payload_len = match u64::try_from(payload.len()) {
            Ok(len) => len,
            Err(_) => {
                iroha_logger::warn!(
                    len = payload.len(),
                    kind,
                    "sidecar payload length exceeds u64"
                );
                return false;
            }
        };
        let Some(projected_data_len) = data_len.checked_add(payload_len) else {
            iroha_logger::warn!(data_len, payload_len, kind, "sidecar data length overflows");
            return false;
        };
        let temp_index_path = index_path.with_extension("index.prepend.tmp");
        let remove_temp = || match namespace {
            Some(namespace) => {
                Self::remove_bound_progress_temp_if_present(namespace, &temp_index_path)
            }
            None => match std::fs::remove_file(&temp_index_path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        };
        if let Err(err) = remove_temp() {
            iroha_logger::warn!(
                ?err,
                ?temp_index_path,
                kind,
                "failed to remove stale sidecar prepend temp index"
            );
            return false;
        }
        let mut source_index = match Self::open_direct_sidecar_file_in_namespace(
            index_path, false, false, namespace,
        ) {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to reopen sidecar index");
                return false;
            }
        };
        let mut temp_index = match match namespace {
            Some(namespace) => Self::create_new_bound_progress_temp(namespace, &temp_index_path),
            None => std::fs::OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&temp_index_path),
        } {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?temp_index_path,
                    kind,
                    "failed to create sidecar prepend temp index"
                );
                return false;
            }
        };
        let entry = SidecarIndexEntry {
            offset: data_len,
            len: payload_len,
        };
        let build_result = (|| -> std::io::Result<()> {
            temp_index.write_all(&SidecarIndexLayout::base_header(height))?;
            temp_index.write_all(&entry.to_bytes())?;
            let filler_entries = prepend.saturating_sub(1);
            let filler_len = filler_entries
                .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .and_then(|len| usize::try_from(len).ok())
                .ok_or_else(|| std::io::Error::other("sidecar prepend filler is too large"))?;
            temp_index.write_all(&vec![0_u8; filler_len])?;
            source_index.seek(SeekFrom::Start(layout.entries_offset))?;
            let copied = std::io::copy(
                &mut (&mut source_index).take(old_entries_len),
                &mut temp_index,
            )?;
            if copied != old_entries_len {
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "sidecar source index ended during prepend",
                ));
            }
            temp_index.flush()?;
            if should_sync {
                temp_index.sync_data()?;
            }
            Ok(())
        })();
        if let Err(err) = build_result {
            iroha_logger::warn!(
                ?err,
                ?temp_index_path,
                kind,
                "failed to build sidecar prepend temp index"
            );
            drop(temp_index);
            let _ = remove_temp();
            return false;
        }
        let temp_index_len = temp_index.metadata().map(|meta| meta.len());
        if !matches!(temp_index_len, Ok(len) if len == projected_index_len) {
            iroha_logger::warn!(
                projected_index_len,
                ?temp_index_path,
                kind,
                "sidecar prepend temp index has unexpected length"
            );
            drop(temp_index);
            let _ = remove_temp();
            return false;
        }
        drop(source_index);
        if let Err(err) = data
            .seek(SeekFrom::Start(data_len))
            .and_then(|_| data.write_all(payload))
            .and_then(|_| data.flush())
        {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to append sidecar payload");
            let _ = rollback_unindexed_sidecar_payload(&data, data_len, data_path, kind);
            drop(data);
            if !data_existed && namespace.is_none() {
                let _ = std::fs::remove_file(data_path);
            }
            drop(temp_index);
            let _ = remove_temp();
            return false;
        }
        if should_sync && let Err(err) = sync_indexed_sidecar_initial_data(&data) {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
            let _ = rollback_unindexed_sidecar_payload(&data, data_len, data_path, kind);
            drop(data);
            if !data_existed && namespace.is_none() {
                let _ = std::fs::remove_file(data_path);
            }
            drop(temp_index);
            let _ = remove_temp();
            return false;
        }
        let mut index_was_published = false;
        let promoted = if let Some(namespace) = namespace {
            let temp_layout = temp_index.metadata().ok().and_then(|metadata| {
                SidecarIndexLayout::read_from(&mut temp_index, metadata.len()).ok()
            });
            if temp_layout.is_some_and(|temp_layout| {
                temp_layout.entry_count > 0
                    && temp_layout.aligned_len == projected_index_len
                    && Self::repair_unindexed_sidecar_tail(
                        &data,
                        &mut temp_index,
                        temp_layout,
                        data_path,
                        &temp_index_path,
                        kind,
                    )
            }) {
                match Self::promote_bound_progress_temp(
                    namespace,
                    &temp_index_path,
                    index_path,
                    &temp_index,
                ) {
                    Ok(()) => {
                        index_was_published = true;
                        Self::sync_indexed_sidecar_bound_mutation(
                            &data,
                            &temp_index,
                            namespace,
                            kind,
                        )
                    }
                    Err(error) => {
                        index_was_published = error.published;
                        iroha_logger::warn!(
                            source = ?error.source,
                            published = error.published,
                            ?temp_index_path,
                            ?index_path,
                            kind,
                            "failed to promote bound progress prepend index"
                        );
                        false
                    }
                }
            } else {
                false
            }
        } else {
            Self::sidecar_index_sane_with_label(
                &temp_index_path,
                projected_data_len,
                kind,
                "prepend temp",
            ) && Self::promote_sidecar_temp(&temp_index_path, index_path, kind, "prepend index")
        };
        if !promoted {
            // Once rename publishes the new index, its new entry owns the
            // appended payload even if a later directory barrier fails. Keep
            // that consistent pair intact so an exact retry can reissue the
            // complete barrier sequence; truncating now would leave the main
            // index pointing past EOF.
            if !index_was_published
                && rollback_unindexed_sidecar_payload(&data, data_len, data_path, kind)
                && !data_existed
                && namespace.is_none()
            {
                drop(data);
                let _ = std::fs::remove_file(data_path);
            }
            drop(temp_index);
            let _ = remove_temp();
            return false;
        }
        drop(temp_index);
        drop(data);
        if let Some(retention) = retention
            && !Self::prune_indexed_sidecars(data_path, index_path, retention, kind)
        {
            return false;
        }
        true
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn append_indexed_sidecar(
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
        fsync_mode: FsyncMode,
        retention: Option<NonZeroUsize>,
    ) -> bool {
        Self::append_indexed_sidecar_with_pinned_height(
            data_path, index_path, height, payload, kind, fsync_mode, retention, None, None,
        )
    }
    fn progress_mutation_namespace_unchanged(namespace: &BoundProgressNamespace) -> bool {
        Self::progress_mutation_namespace_classified(namespace).is_ok()
    }
    fn progress_mutation_namespace_classified(
        namespace: &BoundProgressNamespace,
    ) -> std::result::Result<(), BoundProgressRecoveryFailure> {
        for directory in &namespace.directories {
            let opened = secure_file_metadata::from_file(&directory.file)
                .map_err(|error| BoundProgressRecoveryFailure::from_io(&error))?;
            let current = secure_file_metadata::from_path(&directory.expected_path)
                .map_err(|error| BoundProgressRecoveryFailure::from_io(&error))?;
            if !opened.is_dir()
                || !current.is_dir()
                || current.file_type().is_symlink()
                || !Self::sidecar_metadata_same_object(&directory.metadata, &opened)
                || !Self::sidecar_metadata_same_object(&directory.metadata, &current)
            {
                return Err(BoundProgressRecoveryFailure::InvalidData);
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn append_indexed_sidecar_with_pinned_height(
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
        fsync_mode: FsyncMode,
        retention: Option<NonZeroUsize>,
        pinned_height: Option<u64>,
        namespace: Option<&BoundProgressNamespace>,
    ) -> bool {
        // Sidecars are best-effort; only fsync when strict durability is requested.
        let should_sync = matches!(fsync_mode, FsyncMode::Always);
        if height == 0 || height == u64::MAX {
            iroha_logger::warn!(
                height,
                kind,
                "refusing to store sidecar for unrepresentable height"
            );
            return false;
        }
        if namespace.is_none()
            && !Self::recover_indexed_sidecar_artifacts(data_path, index_path, kind)
        {
            return false;
        }
        let data_was_absent = !data_path.exists();
        let index_was_absent = !index_path.exists();
        if data_was_absent != index_was_absent {
            iroha_logger::warn!(
                ?data_path,
                ?index_path,
                kind,
                "sidecar main data and index are only partially present"
            );
            return false;
        }
        let pair_was_absent = data_was_absent;
        let mut index =
            match Self::open_direct_sidecar_file_in_namespace(index_path, true, false, namespace) {
                Ok(file) => file,
                Err(err) => {
                    iroha_logger::warn!(?err, ?index_path, kind, "failed to open sidecar index");
                    return false;
                }
            };
        let mut index_len = match index.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to stat sidecar index");
                return false;
            }
        };
        let layout = if pair_was_absent {
            if index_len != 0 {
                return false;
            }
            let header = SidecarIndexLayout::base_header(height);
            if let Err(err) = index.write_all(&header) {
                iroha_logger::warn!(
                    ?err,
                    height,
                    ?index_path,
                    kind,
                    "failed to initialize sidecar V1 index"
                );
                return false;
            }
            index_len = INDEXED_SIDECAR_BASE_HEADER_SIZE_U64;
            SidecarIndexLayout::based(height, INDEXED_SIDECAR_BASE_HEADER_SIZE_U64)
                .expect("validated sidecar height produces a canonical V1 layout")
        } else {
            match SidecarIndexLayout::read_from(&mut index, index_len) {
                Ok(layout) => layout,
                Err(reason) => {
                    iroha_logger::warn!(
                        reason,
                        len = index_len,
                        ?index_path,
                        kind,
                        "refusing malformed sidecar index"
                    );
                    return false;
                }
            }
        };
        if index_len != layout.aligned_len {
            iroha_logger::warn!(
                len = index_len,
                aligned_len = layout.aligned_len,
                ?index_path,
                kind,
                "sidecar index length misaligned; truncating trailing bytes"
            );
            if let Err(err) = index.set_len(layout.aligned_len) {
                iroha_logger::warn!(
                    ?err,
                    ?index_path,
                    kind,
                    "failed to truncate misaligned sidecar index"
                );
                return false;
            }
        }
        if height < layout.base_height {
            drop(index);
            return Self::append_preceding_indexed_sidecar(
                data_path,
                index_path,
                height,
                payload,
                kind,
                should_sync,
                retention,
                layout,
                namespace,
            );
        }
        let mut data =
            match Self::open_direct_sidecar_file_in_namespace(data_path, true, false, namespace) {
                Ok(file) => file,
                Err(err) => {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to open sidecar store");
                    return false;
                }
            };
        if !Self::repair_unindexed_sidecar_tail(
            &data, &mut index, layout, data_path, index_path, kind,
        ) {
            return false;
        }
        let expected_height = match layout.next_height() {
            Some(height) => height,
            None => {
                iroha_logger::warn!(
                    base_height = layout.base_height,
                    entries = layout.entry_count,
                    ?index_path,
                    kind,
                    "sidecar index height range overflows"
                );
                return false;
            }
        };
        if let Some(entry_pos) = layout.entry_position(height) {
            let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
            if index
                .seek(SeekFrom::Start(entry_pos))
                .and_then(|_| index.read_exact(&mut entry_buf))
                .is_err()
            {
                iroha_logger::warn!(
                    height,
                    ?index_path,
                    kind,
                    "failed to read sidecar index entry for update"
                );
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(entry_buf);
            let mut matches_existing = false;
            if entry.len > 0 {
                if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
                    iroha_logger::warn!(
                        height,
                        len = entry.len,
                        limit = STRICT_INIT_MAX_BLOCK_BYTES,
                        kind,
                        "existing sidecar payload length exceeds limit"
                    );
                    return false;
                }
                let len_usize = if let Ok(len) = usize::try_from(entry.len) {
                    len
                } else {
                    iroha_logger::warn!(
                        len = entry.len,
                        kind,
                        "sidecar payload length exceeds usize"
                    );
                    return false;
                };
                let data_len = match data.metadata() {
                    Ok(meta) => meta.len(),
                    Err(err) => {
                        iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                        return false;
                    }
                };
                if entry
                    .offset
                    .checked_add(entry.len)
                    .is_some_and(|end| end <= data_len)
                {
                    let mut existing = vec![0u8; len_usize];
                    if data
                        .seek(SeekFrom::Start(entry.offset))
                        .and_then(|_| data.read_exact(&mut existing))
                        .is_ok()
                    {
                        matches_existing = existing == payload;
                    } else {
                        iroha_logger::debug!(
                            height,
                            ?data_path,
                            kind,
                            "failed to read existing sidecar payload; overwriting entry"
                        );
                    }
                } else {
                    iroha_logger::debug!(
                        height,
                        offset = entry.offset,
                        len = entry.len,
                        data_len,
                        ?data_path,
                        kind,
                        "sidecar entry points past data file; overwriting entry"
                    );
                }
            }
            if matches_existing {
                iroha_logger::debug!(
                    height,
                    index_entries = layout.entry_count,
                    ?index_path,
                    kind,
                    "sidecar already recorded; revalidating strict durability"
                );
                if should_sync
                    && let Some(namespace) = namespace
                    && !Self::sync_indexed_sidecar_bound_mutation(&data, &index, namespace, kind)
                {
                    return false;
                }
                drop(index);
                drop(data);
                if let Some(retention) = retention {
                    if !Self::prune_indexed_sidecars_with_pinned_height(
                        data_path,
                        index_path,
                        retention,
                        pinned_height,
                        kind,
                    ) {
                        return false;
                    }
                }
                if should_sync
                    && namespace.is_none()
                    && !Self::sync_indexed_sidecar_barriers(data_path, index_path, kind)
                {
                    return false;
                }
                return true;
            }
            let offset = match data.metadata() {
                Ok(meta) => meta.len(),
                Err(err) => {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                    return false;
                }
            };
            let len_u64 = if let Ok(len) = u64::try_from(payload.len()) {
                len
            } else {
                iroha_logger::warn!(
                    len = payload.len(),
                    kind,
                    "sidecar payload length exceeds u64"
                );
                return false;
            };
            if let Err(err) = data
                .seek(SeekFrom::Start(offset))
                .and_then(|_| data.write_all(payload))
            {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to append sidecar payload");
                let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
                return false;
            }
            if should_sync {
                if let Err(err) = sync_indexed_sidecar_initial_data(&data) {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
                    let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
                    return false;
                }
            }
            let new_entry = SidecarIndexEntry {
                offset,
                len: len_u64,
            };
            if let Err(err) = index
                .seek(SeekFrom::Start(entry_pos))
                .and_then(|_| index.write_all(&new_entry.to_bytes()))
            {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to update sidecar index");
                let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
                return false;
            }
            if should_sync
                && let Some(namespace) = namespace
                && !Self::sync_indexed_sidecar_bound_mutation(&data, &index, namespace, kind)
            {
                return false;
            }
            drop(index);
            drop(data);
            if let Some(retention) = retention {
                if !Self::prune_indexed_sidecars_with_pinned_height(
                    data_path,
                    index_path,
                    retention,
                    pinned_height,
                    kind,
                ) {
                    return false;
                }
            }
            if should_sync
                && namespace.is_none()
                && !Self::sync_indexed_sidecar_barriers(data_path, index_path, kind)
            {
                return false;
            }
            return true;
        }
        if height < expected_height {
            iroha_logger::warn!(
                height,
                base_height = layout.base_height,
                expected_height,
                ?index_path,
                kind,
                "sidecar height precedes the compact index base"
            );
            return false;
        }
        let missing = height - expected_height;
        if missing > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
            iroha_logger::warn!(
                height,
                expected_height,
                missing,
                limit = MAX_INDEXED_SIDECAR_GAP_ENTRIES,
                ?index_path,
                kind,
                "refusing oversized sidecar index gap"
            );
            return false;
        }
        let Some(projected_index_len) = missing
            .checked_add(1)
            .and_then(|entries| entries.checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64))
            .and_then(|growth| layout.aligned_len.checked_add(growth))
        else {
            iroha_logger::warn!(
                height,
                expected_height,
                ?index_path,
                kind,
                "sidecar index growth overflows file offsets"
            );
            return false;
        };
        if height > expected_height {
            iroha_logger::warn!(
                height,
                missing,
                kind,
                "sidecar gap detected; filling index placeholders"
            );
            let Some(filler_len_u64) = missing.checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64) else {
                iroha_logger::warn!(
                    height,
                    missing,
                    ?index_path,
                    kind,
                    "sidecar placeholder byte length overflows"
                );
                return false;
            };
            let Ok(filler_len) = usize::try_from(filler_len_u64) else {
                iroha_logger::warn!(
                    height,
                    filler_len = filler_len_u64,
                    ?index_path,
                    kind,
                    "sidecar placeholder byte length exceeds usize"
                );
                return false;
            };
            let filler = vec![0u8; filler_len];
            if let Err(err) = index
                .seek(SeekFrom::Start(layout.aligned_len))
                .and_then(|_| index.write_all(&filler))
            {
                iroha_logger::warn!(
                    ?err,
                    ?index_path,
                    kind,
                    "failed to append placeholder sidecar index entries"
                );
                return false;
            }
        }
        let offset = match data.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                return false;
            }
        };
        let len_u64 = if let Ok(len) = u64::try_from(payload.len()) {
            len
        } else {
            iroha_logger::warn!(
                len = payload.len(),
                kind,
                "sidecar payload length exceeds u64"
            );
            return false;
        };
        if let Err(err) = data
            .seek(SeekFrom::Start(offset))
            .and_then(|_| data.write_all(payload))
        {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to append sidecar payload");
            let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
            return false;
        }
        if should_sync {
            if let Err(err) = sync_indexed_sidecar_initial_data(&data) {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
                let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
                return false;
            }
        }
        let entry = SidecarIndexEntry {
            offset,
            len: len_u64,
        };
        let Some(entry_pos) = projected_index_len.checked_sub(PIPELINE_INDEX_ENTRY_SIZE_U64) else {
            iroha_logger::warn!(
                projected_index_len,
                ?index_path,
                kind,
                "sidecar index entry position underflows"
            );
            let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
            return false;
        };
        if let Err(err) = index
            .seek(SeekFrom::Start(entry_pos))
            .and_then(|_| index.write_all(&entry.to_bytes()))
        {
            iroha_logger::warn!(?err, ?index_path, kind, "failed to append sidecar index");
            let _ = rollback_unindexed_sidecar_payload(&data, offset, data_path, kind);
            return false;
        }
        if should_sync
            && let Some(namespace) = namespace
            && !Self::sync_indexed_sidecar_bound_mutation(&data, &index, namespace, kind)
        {
            return false;
        }
        drop(index);
        drop(data);
        if let Some(retention) = retention {
            if !Self::prune_indexed_sidecars_with_pinned_height(
                data_path,
                index_path,
                retention,
                pinned_height,
                kind,
            ) {
                return false;
            }
        }
        if should_sync
            && namespace.is_none()
            && !Self::sync_indexed_sidecar_barriers(data_path, index_path, kind)
        {
            return false;
        }
        true
    }
    fn sync_indexed_sidecar_bound_mutation(
        data: &std::fs::File,
        index: &std::fs::File,
        namespace: &BoundProgressNamespace,
        kind: &str,
    ) -> bool {
        if let Err(error) = sync_indexed_sidecar_data(data) {
            iroha_logger::warn!(
                ?error,
                kind,
                "failed to sync bound sidecar payload mutation"
            );
            return false;
        }
        if let Err(error) = sync_indexed_sidecar_index(index) {
            iroha_logger::warn!(?error, kind, "failed to sync bound sidecar index mutation");
            return false;
        }
        Self::sync_bound_progress_mutation_directories(namespace, kind)
    }
    fn sync_bound_progress_mutation_directories(
        namespace: &BoundProgressNamespace,
        kind: &str,
    ) -> bool {
        for (position, directory) in namespace.directories.iter().enumerate() {
            let result = if position == 0 {
                sync_indexed_sidecar_dir_handle(&directory.file)
            } else {
                sync_progress_sidecar_ancestor_dir_handle(&directory.file)
            };
            if let Err(error) = result {
                iroha_logger::warn!(
                    ?error,
                    path = ?directory.expected_path,
                    kind,
                    "failed to sync bound sidecar mutation directory"
                );
                return false;
            }
        }
        Self::progress_mutation_namespace_unchanged(namespace)
    }
    /// Reissue the complete strict sidecar durability sequence in dependency order.
    ///
    /// Calling this for an exact existing payload is intentional: a prior attempt may have made
    /// both files readable through the page cache while failing the index or directory barrier.
    fn sync_indexed_sidecar_barriers(data_path: &Path, index_path: &Path, kind: &str) -> bool {
        let data = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(data_path)
        {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?data_path,
                    kind,
                    "failed to open sidecar store for sync"
                );
                return false;
            }
        };
        if let Err(err) = sync_indexed_sidecar_data(&data) {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
            return false;
        }
        let index = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(index_path)
        {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?index_path,
                    kind,
                    "failed to open sidecar index for sync"
                );
                return false;
            }
        };
        if let Err(err) = sync_indexed_sidecar_index(&index) {
            iroha_logger::warn!(?err, ?index_path, kind, "failed to sync sidecar index");
            return false;
        }
        if let Some(parent) = data_path.parent()
            && let Err(err) = sync_indexed_sidecar_dir(parent)
        {
            iroha_logger::warn!(
                ?err,
                ?parent,
                kind,
                "failed to sync sidecar parent directory"
            );
            return false;
        }
        if let Some(parent) = index_path.parent()
            && Some(parent) != data_path.parent()
            && let Err(err) = sync_indexed_sidecar_dir(parent)
        {
            iroha_logger::warn!(
                ?err,
                ?parent,
                kind,
                "failed to sync sidecar index parent directory"
            );
            return false;
        }
        true
    }
    #[allow(clippy::too_many_lines)]
    fn read_pipeline_sidecar<T, F>(
        &self,
        height: u64,
        data_file: &str,
        index_file: &str,
        decoder: F,
        kind: &str,
    ) -> Option<T>
    where
        F: Fn(&[u8]) -> Result<T, norito::Error>,
    {
        let mut dir = self.store_dir()?;
        dir.push(PIPELINE_DIR_NAME);
        let data_path = dir.join(data_file);
        let index_path = dir.join(index_file);
        let entry_byte_limit =
            u64::try_from(MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES).unwrap_or(u64::MAX);
        let recover = !self.emergency_fast_startup_enabled();
        if recover
            && !self.recover_indexed_sidecar_with_physical_resources(&data_path, &index_path, kind)
        {
            return None;
        }
        Self::read_indexed_sidecar_from_paths_with_recovery_and_limit(
            height,
            &data_path,
            &index_path,
            decoder,
            kind,
            false,
            entry_byte_limit,
        )
    }
    #[cfg(test)]
    #[allow(clippy::too_many_lines)]
    fn read_indexed_sidecar_from_paths<T, F>(
        height: u64,
        data_path: &Path,
        index_path: &Path,
        decoder: F,
        kind: &str,
    ) -> Option<T>
    where
        F: Fn(&[u8]) -> Result<T, norito::Error>,
    {
        Self::read_indexed_sidecar_from_paths_with_recovery(
            height, data_path, index_path, decoder, kind, true,
        )
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn read_indexed_sidecar_from_paths_with_recovery_and_limit<T, F>(
        height: u64,
        data_path: &Path,
        index_path: &Path,
        decoder: F,
        kind: &str,
        recover: bool,
        entry_byte_limit: u64,
    ) -> Option<T>
    where
        F: Fn(&[u8]) -> Result<T, norito::Error>,
    {
        if height == 0 {
            return None;
        }
        if recover && !Self::recover_indexed_sidecar_artifacts(data_path, index_path, kind) {
            return None;
        }
        let mut index = open_read_only_regular_file(index_path, "indexed sidecar index").ok()?;
        let mut data = open_read_only_regular_file(data_path, "indexed sidecar data").ok()?;
        Self::read_indexed_sidecar_from_open_files_with_limit(
            height,
            &mut data,
            &mut index,
            data_path,
            index_path,
            decoder,
            kind,
            entry_byte_limit,
        )
    }
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn read_indexed_sidecar_from_open_files_with_limit<T, F>(
        height: u64,
        data: &mut std::fs::File,
        index: &mut std::fs::File,
        data_path: &Path,
        index_path: &Path,
        decoder: F,
        kind: &str,
        entry_byte_limit: u64,
    ) -> Option<T>
    where
        F: Fn(&[u8]) -> Result<T, norito::Error>,
    {
        let index_meta = index.metadata().ok()?;
        let index_len = index_meta.len();
        let layout = match SidecarIndexLayout::read_from(index, index_len) {
            Ok(layout) => layout,
            Err(reason) => {
                iroha_logger::warn!(
                    reason,
                    len = index_len,
                    ?index_path,
                    kind,
                    "refusing malformed sidecar index"
                );
                return None;
            }
        };
        if index_len != layout.aligned_len {
            iroha_logger::warn!(
                len = index_len,
                aligned_len = layout.aligned_len,
                ?index_path,
                kind,
                "sidecar index length misaligned; ignoring trailing bytes"
            );
        }
        let seek_pos = layout.entry_position(height)?;
        let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        if index
            .seek(SeekFrom::Start(seek_pos))
            .and_then(|_| index.read_exact(&mut entry_buf))
            .is_err()
        {
            iroha_logger::warn!(
                height,
                ?index_path,
                kind,
                "failed to read sidecar index entry"
            );
            return None;
        }
        let entry = SidecarIndexEntry::from_bytes(entry_buf);
        if entry.len == 0 {
            iroha_logger::debug!(height, ?index_path, kind, "empty sidecar length; skipping");
            return None;
        }
        if entry.len > entry_byte_limit {
            iroha_logger::warn!(
                height,
                len = entry.len,
                limit = entry_byte_limit,
                ?index_path,
                kind,
                "sidecar length exceeds limit; skipping"
            );
            return None;
        }
        let len_usize = if let Ok(len) = usize::try_from(entry.len) {
            len
        } else {
            iroha_logger::warn!(
                len = entry.len,
                ?index_path,
                kind,
                "sidecar length exceeds usize; skipping"
            );
            return None;
        };
        let data_len = data.metadata().ok()?.len();
        let entry_end = match entry.offset.checked_add(entry.len) {
            Some(end) => end,
            None => {
                iroha_logger::warn!(
                    height,
                    offset = entry.offset,
                    len = entry.len,
                    ?data_path,
                    kind,
                    "sidecar payload range overflows"
                );
                return None;
            }
        };
        if entry_end > data_len {
            iroha_logger::warn!(
                height,
                offset = entry.offset,
                len = entry.len,
                data_len,
                ?data_path,
                kind,
                "sidecar entry points past data file"
            );
            return None;
        }
        if height > layout.base_height {
            let prev_height = height - 1;
            let Some(prev_pos) = layout.entry_position(prev_height) else {
                iroha_logger::warn!(
                    height,
                    prev_height,
                    ?index_path,
                    kind,
                    "sidecar previous index position is unrepresentable"
                );
                return None;
            };
            let mut prev_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
            if index
                .seek(SeekFrom::Start(prev_pos))
                .and_then(|_| index.read_exact(&mut prev_buf))
                .is_err()
            {
                iroha_logger::warn!(
                    height,
                    ?index_path,
                    kind,
                    "failed to read previous sidecar index entry"
                );
                return None;
            }
            let prev = SidecarIndexEntry::from_bytes(prev_buf);
            if prev.len > 0 {
                let Some(prev_end) = prev.offset.checked_add(prev.len) else {
                    iroha_logger::warn!(
                        height,
                        prev_offset = prev.offset,
                        prev_len = prev.len,
                        ?index_path,
                        kind,
                        "previous sidecar payload range overflows"
                    );
                    return None;
                };
                if prev_end <= data_len && entry.offset < prev_end && entry_end > prev.offset {
                    iroha_logger::warn!(
                        height,
                        prev_offset = prev.offset,
                        prev_len = prev.len,
                        offset = entry.offset,
                        len = entry.len,
                        ?index_path,
                        kind,
                        "sidecar index entry overlaps previous payload; skipping"
                    );
                    return None;
                }
            }
        }
        let mut payload = vec![0u8; len_usize];
        if data
            .seek(SeekFrom::Start(entry.offset))
            .and_then(|_| data.read_exact(&mut payload))
            .is_err()
        {
            iroha_logger::warn!(height, ?data_path, kind, "failed to read sidecar payload");
            return None;
        }
        match decoder(&payload) {
            Ok(sidecar) => Some(sidecar),
            Err(err) => {
                iroha_logger::warn!(?err, height, ?data_path, kind, "failed to decode sidecar");
                None
            }
        }
    }
}
