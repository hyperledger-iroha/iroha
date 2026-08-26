//! Atomic reconstruction and publication of the State DA query projections.

use super::*;

/// Errors surfaced while rebuilding DA indexes from the committed block log.
#[derive(Copy, Clone, Debug, ThisError, PartialEq, Eq)]
pub(crate) enum DaIndexHydrationError {
    /// A committed WSV hash has no corresponding non-hash-only Kura body.
    #[error("committed DA block body at height {height} is missing from Kura")]
    MissingBlock {
        /// One-based committed block height.
        height: NonZeroU64,
    },
    /// A Kura body does not authenticate against the fixed committed WSV hash.
    #[error(
        "DA block body at height {height} does not match committed WSV hash: expected {expected:?}, found {actual:?}"
    )]
    BlockHashMismatch {
        /// One-based committed block height.
        height: NonZeroU64,
        /// Header hash committed in the WSV prefix.
        expected: HashOf<BlockHeader>,
        /// Header hash decoded from the Kura body.
        actual: HashOf<BlockHeader>,
    },
    /// A Kura body's signed header does not declare its committed WSV slot height.
    #[error("DA block body at WSV height {expected} declares header height {actual}")]
    BlockHeightMismatch {
        /// One-based committed WSV slot height.
        expected: NonZeroU64,
        /// Height declared by the signed Kura block header.
        actual: NonZeroU64,
    },
    /// DA shard cursor replay failed.
    #[error("DA shard cursor hydration failed: {0}")]
    ShardCursor(#[from] DaShardCursorError),
    /// DA receipt cursor replay failed.
    #[error("DA receipt cursor hydration failed: {0}")]
    ReceiptCursor(#[from] DaReceiptCursorError),
    /// Confidential-compute receipt replay failed.
    #[error("DA confidential-compute hydration failed: {0}")]
    ConfidentialCompute(#[from] ConfidentialComputeError),
}

/// Complete DA query projection built locally from one committed WSV hash prefix.
struct HydratedDaIndexes {
    commitments: DaCommitmentStore,
    confidential_compute: ConfidentialComputeStore,
    receipt_cursors: DaReceiptCursorIndex,
    shard_cursors: DaShardCursorIndex,
    pin_intents: DaPinStore,
}

impl HydratedDaIndexes {
    fn new(
        lane_config: &iroha_config::parameters::actual::LaneConfig,
        canonical_reset_heights: &BTreeMap<LaneId, u64>,
    ) -> Self {
        let mut shard_cursors = DaShardCursorIndex::new(lane_config);
        shard_cursors.merge_canonical_reset_heights(canonical_reset_heights);
        Self {
            commitments: DaCommitmentStore::default(),
            confidential_compute: ConfidentialComputeStore::default(),
            receipt_cursors: DaReceiptCursorIndex::default(),
            shard_cursors,
            pin_intents: DaPinStore::default(),
        }
    }
}

impl State {
    /// Publish one successfully rebuilt DA projection while every component is write-locked.
    fn publish_hydrated_da_indexes(&self, hydrated: HydratedDaIndexes) {
        let HydratedDaIndexes {
            commitments,
            confidential_compute,
            receipt_cursors,
            shard_cursors,
            pin_intents,
        } = hydrated;
        // Keep this acquisition order aligned with snapshot/test readers that hold more than
        // one DA guard. No published field changes until all five write guards are owned.
        let mut published_commitments = self.da_commitments.write();
        let mut published_confidential_compute = self.da_confidential_compute.write();
        let mut published_receipt_cursors = self.da_receipt_cursors.write();
        let mut published_shard_cursors = self.da_shard_cursors.write();
        let mut published_pin_intents = self.da_pin_intents.write();
        *published_commitments = commitments;
        *published_confidential_compute = confidential_compute;
        *published_receipt_cursors = receipt_cursors;
        *published_shard_cursors = shard_cursors;
        *published_pin_intents = pin_intents;
    }

    pub(crate) fn ensure_da_indexes_hydrated(&self) -> Result<(), DaIndexHydrationError> {
        {
            let guard = self.da_indexes_hydrated.read();
            if let Some(result) = guard.as_ref() {
                return *result;
            }
        }
        let _hydration_guard = self.da_index_hydration_fence.lock();
        if let Some(result) = self.da_indexes_hydrated.read().as_ref() {
            return *result;
        }
        let _state_write_guard = self.state_write_lock.lock();
        let result = self.build_da_indexes_from_kura(None).map(|hydrated| {
            self.publish_hydrated_da_indexes(hydrated);
            self.persist_da_shard_cursor_journal();
        });
        if let Err(err) = &result {
            warn!(?err, "failed to hydrate DA indexes from Kura");
        }
        *self.da_indexes_hydrated.write() = Some(result);
        result
    }

    /// Force a rebuild of DA indexes from the Kura block log, truncating at `target_height` when provided.
    pub(crate) fn rewind_da_indexes_to_height(
        &self,
        target_height: u64,
    ) -> Result<(), DaIndexHydrationError> {
        let _hydration_guard = self.da_index_hydration_fence.lock();
        // Make concurrent accessors join this fenced rebuild instead of observing the
        // previous cached success while the committed projection is being rewound.
        *self.da_indexes_hydrated.write() = None;
        let _state_write_guard = self.state_write_lock.lock();
        let result = self
            .build_da_indexes_from_kura(Some(target_height))
            .map(|hydrated| {
                self.publish_hydrated_da_indexes(hydrated);
                self.persist_da_shard_cursor_journal();
            });
        if let Err(err) = &result {
            warn!(?err, target_height, "failed to rewind DA indexes from Kura");
        }
        *self.da_indexes_hydrated.write() = Some(result);
        result
    }

    #[allow(clippy::too_many_lines)]
    fn build_da_indexes_from_kura(
        &self,
        target_height: Option<u64>,
    ) -> Result<HydratedDaIndexes, DaIndexHydrationError> {
        let nexus = self.nexus_snapshot();
        let lane_config = &nexus.lane_config;
        let incarnation_resets = self
            .lane_incarnation_activation_heights_snapshot()
            .into_iter()
            .filter(|(_, activation_height)| *activation_height > 0)
            .collect::<BTreeMap<_, _>>();
        let mut hydrated = HydratedDaIndexes::new(lane_config, &incarnation_resets);
        let journal_path = self.da_shard_cursor_journal_path();
        let persisted_journal = if journal_path.as_os_str().is_empty() {
            None
        } else {
            match DaShardCursorJournal::load(lane_config, &journal_path) {
                Ok(journal) => Some(journal),
                Err(err) => {
                    warn!(
                        ?err,
                        path = %journal_path.display(),
                        "failed to load persisted DA shard cursor journal; continuing with ledger replay"
                    );
                    None
                }
            }
        };
        // Clone the hash list up front so we do not hold the block-hash lock while
        // loading blocks from Kura (avoids lock-order inversions with Kura writers).
        let mut committed_hash_prefix: Vec<HashOf<BlockHeader>> =
            self.block_hashes.view().iter().copied().collect();
        let replay_len = target_height
            .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX))
            .map_or(committed_hash_prefix.len(), |limit| {
                committed_hash_prefix.len().min(limit)
            });
        committed_hash_prefix.truncate(replay_len);
        let replay_height = u64::try_from(replay_len).unwrap_or(u64::MAX);
        if let Some(journal) = persisted_journal.as_ref() {
            hydrated.shard_cursors.merge_canonical_reset_heights(
                &Self::journal_reset_heights_at_or_below(journal, replay_height),
            );
        }
        if replay_len == 0 {
            if let Some(journal) = persisted_journal.as_ref() {
                self.restore_da_cursors_from_journal(
                    &mut hydrated.shard_cursors,
                    lane_config,
                    journal,
                    0,
                )?;
            }
            return Ok(hydrated);
        }
        let mut saw_da_commitments = false;
        let hash_only_prefix = self.kura.hash_only_unavailable_prefix_len(replay_len);
        if hash_only_prefix > 0 {
            debug!(
                hash_only_prefix,
                replay_len,
                "skipping hash-only hard-fork snapshot blocks while hydrating DA indexes"
            );
        }
        for (idx, expected_hash) in committed_hash_prefix
            .iter()
            .enumerate()
            .skip(hash_only_prefix)
        {
            let height = idx + 1;
            let height_u64 = u64::try_from(height).expect("committed block height must fit u64");
            let height_u64 = NonZeroU64::new(height_u64).expect("block height is non-zero");
            let height_usize = NonZeroUsize::new(height).expect("block height is non-zero");
            let Some(block) = self.kura.get_block(height_usize) else {
                if self.kura.is_hash_only_block_height(height_usize) {
                    debug!(
                        height,
                        "skipping hash-only hard-fork snapshot block while hydrating DA indexes"
                    );
                    continue;
                }
                return Err(DaIndexHydrationError::MissingBlock { height: height_u64 });
            };
            let block_hash = block.hash();
            if block_hash != *expected_hash {
                return Err(DaIndexHydrationError::BlockHashMismatch {
                    height: height_u64,
                    expected: *expected_hash,
                    actual: block_hash,
                });
            }
            if block.header().height() != height_u64 {
                return Err(DaIndexHydrationError::BlockHeightMismatch {
                    expected: height_u64,
                    actual: block.header().height(),
                });
            }
            if let Some(bundle) = block.as_ref().da_commitments() {
                saw_da_commitments = true;
                let policy_context = crate::da::ActiveLaneProofPolicyContext::new(&nexus);
                let active_commitments = bundle
                    .commitments
                    .iter()
                    .filter_map(|record| {
                        let validation = policy_context
                            .enforce_commitment_at_height(record, height_u64.get())
                            .map_err(crate::da::DaCommitmentValidationError::from)
                            .and_then(|()| {
                                crate::da::validate_confidential_compute_record(
                                    lane_config,
                                    record,
                                )
                                .map(|_| ())
                                .map_err(crate::da::DaCommitmentValidationError::from)
                            });
                        match validation {
                            Ok(())
                                if hydrated
                                    .shard_cursors
                                    .canonical_reset_height_for_lane(record.lane_id)
                                    .is_none_or(|reset_height| {
                                        height_u64.get() > reset_height
                                    }) =>
                            {
                                Some(record.clone())
                            }
                            Ok(()) => {
                                warn!(
                                    height,
                                    lane = %record.lane_id.as_u32(),
                                    epoch = record.epoch,
                                    sequence = record.sequence,
                                    "skipping DA commitment index materialization for an earlier lane incarnation"
                                );
                                None
                            }
                            Err(err) => {
                                warn!(
                                    ?err,
                                    height,
                                    lane = %record.lane_id.as_u32(),
                                    epoch = record.epoch,
                                    sequence = record.sequence,
                                    "skipping DA commitment index materialization for inactive lane after lifecycle update"
                                );
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                let query_visible_keys: BTreeSet<_> = active_commitments
                    .iter()
                    .map(DaCommitmentKey::from_record)
                    .collect();
                let identity_visible_keys: BTreeSet<_> = bundle
                    .commitments
                    .iter()
                    .filter(|record| {
                        let key = DaCommitmentKey::from_record(record);
                        hydrated
                            .shard_cursors
                            .canonical_reset_height_for_lane(record.lane_id)
                            .is_none_or(|reset_height| height_u64.get() > reset_height)
                            && (query_visible_keys.contains(&key)
                                || nexus
                                    .lane_catalog
                                    .lanes()
                                    .iter()
                                    .all(|lane| lane.id != record.lane_id))
                    })
                    .map(DaCommitmentKey::from_record)
                    .collect();
                hydrated.commitments.insert_bundle_with_visibility_filter(
                    height_u64.get(),
                    bundle.clone(),
                    |record| identity_visible_keys.contains(&DaCommitmentKey::from_record(record)),
                    |record| query_visible_keys.contains(&DaCommitmentKey::from_record(record)),
                );
                if let Err(err) = self.advance_da_shard_cursors_into(
                    &mut hydrated.shard_cursors,
                    lane_config,
                    height_u64.get(),
                    &active_commitments,
                ) {
                    warn!(
                        ?err,
                        height, "failed to advance shard cursor index while hydrating from Kura"
                    );
                    return Err(DaIndexHydrationError::ShardCursor(err));
                }
                if let Err(err) = self.advance_da_receipt_cursors_into(
                    &mut hydrated.receipt_cursors,
                    height_u64.get(),
                    &active_commitments,
                ) {
                    warn!(
                        ?err,
                        height, "failed to advance receipt cursor index while hydrating from Kura"
                    );
                    return Err(DaIndexHydrationError::ReceiptCursor(err));
                }
                if let Err(err) = Self::record_confidential_compute_into(
                    &mut hydrated.confidential_compute,
                    lane_config,
                    height_u64.get(),
                    &bundle.commitments,
                    |record| query_visible_keys.contains(&DaCommitmentKey::from_record(record)),
                ) {
                    warn!(
                        ?err,
                        height, "failed to hydrate confidential-compute receipts from Kura"
                    );
                    return Err(DaIndexHydrationError::ConfidentialCompute(err));
                }
            }
            if let Some(bundle) = block.as_ref().da_pin_intents() {
                let _ = self.ingest_committed_pin_intents_from_kura_into(
                    &nexus,
                    &mut hydrated.pin_intents,
                    &hydrated.shard_cursors,
                    height_u64.get(),
                    bundle.intents.clone(),
                );
            }
        }
        if let Some(journal) = persisted_journal {
            if !saw_da_commitments || hydrated.shard_cursors.is_empty() {
                self.restore_da_cursors_from_journal(
                    &mut hydrated.shard_cursors,
                    lane_config,
                    &journal,
                    replay_height,
                )?;
            }
            for entry in journal.entries() {
                if replay_height != 0 && entry.last_block_height > replay_height {
                    warn!(
                        lane = %entry.lane_id.as_u32(),
                        shard = %entry.shard_id.as_u32(),
                        cursor_height = entry.last_block_height,
                        replay_height,
                        "persisted DA shard cursor ahead of ledger height; skipping comparison"
                    );
                    continue;
                }
                let shard_id = entry.shard_id.as_u32();
                match hydrated.shard_cursors.get(shard_id, entry.lane_id) {
                    Some(cursor)
                        if (cursor.epoch, cursor.sequence) >= (entry.epoch, entry.sequence) => {}
                    Some(cursor) => {
                        warn!(
                            lane = %entry.lane_id.as_u32(),
                            shard = %shard_id,
                            observed = ?(cursor.epoch, cursor.sequence),
                            persisted = ?(entry.epoch, entry.sequence),
                            "persisted DA shard cursor was ahead of ledger replay; overwriting with ledger state"
                        );
                        #[cfg(feature = "telemetry")]
                        self.telemetry.record_da_shard_cursor_event(
                            "journal_regression",
                            entry.lane_id.as_u32(),
                            shard_id,
                            cursor.last_block_height,
                        );
                    }
                    None => {
                        warn!(
                            lane = %entry.lane_id.as_u32(),
                            shard = %shard_id,
                            persisted_epoch = entry.epoch,
                            persisted_sequence = entry.sequence,
                            "persisted DA shard cursor missing from ledger replay; clearing entry"
                        );
                        #[cfg(feature = "telemetry")]
                        self.telemetry.record_da_shard_cursor_event(
                            "journal_missing",
                            entry.lane_id.as_u32(),
                            shard_id,
                            0,
                        );
                    }
                }
            }
        }
        Ok(hydrated)
    }
}
