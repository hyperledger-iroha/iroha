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
    pub fn prune(&mut self, height: u64) -> Result<()> {
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
        self.prune_durable(height, false, fail_stage)
    }

    fn validate_rollback_prefix(
        &mut self,
        intent: &KuraRollbackIntent,
        intent_path: &Path,
    ) -> Result<()> {
        let invalid = |reason: String| Error::RollbackIntentInvalid {
            path: intent_path.to_path_buf(),
            reason,
        };
        let index_count = self.read_index_count()?;
        let hashes_count = self.read_hashes_count()?;
        if index_count < intent.target_height || hashes_count < intent.target_height {
            return Err(invalid(format!(
                "canonical prefix is shorter than rollback target: index={index_count}, hashes={hashes_count}, target={}",
                intent.target_height
            )));
        }
        if index_count > intent.from_height || hashes_count > intent.from_height {
            return Err(invalid(format!(
                "canonical files advanced beyond rollback source: index={index_count}, hashes={hashes_count}, source={}",
                intent.from_height
            )));
        }
        if intent.target_height > 0 {
            let actual = self
                .read_block_hashes(intent.target_height.saturating_sub(1), 1)?
                .first()
                .copied()
                .ok_or_else(|| invalid("rollback target hash is missing".to_owned()))?;
            if Some(actual) != intent.target_block_hash {
                return Err(invalid(format!(
                    "rollback target hash mismatch: expected {:?}, actual {actual}",
                    intent.target_block_hash
                )));
            }
        }
        let data_len = self.data_file_len()?;
        for index in 0..intent.target_height {
            let entry = self.read_block_index(index)?;
            if entry.is_evicted() {
                continue;
            }
            let end = entry
                .start
                .checked_add(entry.length)
                .ok_or_else(|| invalid(format!("block data range overflows at index {index}")))?;
            if entry.length == 0 || end > data_len {
                return Err(invalid(format!(
                    "block data does not contain rollback target prefix at index {index}"
                )));
            }
        }
        Ok(())
    }

    fn prune_for_rollback(
        &mut self,
        intent: &KuraRollbackIntent,
        intent_path: &Path,
    ) -> Result<()> {
        self.validate_rollback_prefix(intent, intent_path)?;
        self.prune_durable(intent.target_height, true, 0)?;
        self.verify_rollback_boundary(intent, intent_path)
    }

    fn verify_rollback_boundary(
        &mut self,
        intent: &KuraRollbackIntent,
        intent_path: &Path,
    ) -> Result<()> {
        let index_count = self.read_index_count()?;
        let hashes_count = self.read_hashes_count()?;
        if index_count != intent.target_height || hashes_count != intent.target_height {
            return Err(Error::RollbackIntentInvalid {
                path: intent_path.to_path_buf(),
                reason: format!(
                    "rollback block boundary verification failed: index={index_count}, hashes={hashes_count}, target={}",
                    intent.target_height
                ),
            });
        }
        let expected_data_len = self.data_end_for_index_prefix(intent.target_height)?;
        let actual_data_len = self.data_file_len()?;
        if actual_data_len != expected_data_len {
            return Err(Error::RollbackIntentInvalid {
                path: intent_path.to_path_buf(),
                reason: format!(
                    "rollback data boundary verification failed: data={actual_data_len}, expected={expected_data_len}"
                ),
            });
        }
        let marker_path = self.commit_marker_path();
        let marker_bytes = Self::read_required_bounded_commit_marker_bytes(
            &marker_path,
            "rollback block marker is missing",
        )?;
        let marker =
            norito::decode_canonical::<BlockStoreCommitMarker>(&marker_bytes).map_err(|err| {
                Error::RollbackIntentInvalid {
                    path: intent_path.to_path_buf(),
                    reason: format!("rollback commit marker is not decodable: {err}"),
                }
            })?;
        if marker.version != BlockStoreCommitMarker::VERSION || marker.count != intent.target_height
        {
            return Err(Error::RollbackIntentInvalid {
                path: intent_path.to_path_buf(),
                reason: format!(
                    "rollback commit marker boundary mismatch: version={}, count={}, target={}",
                    marker.version, marker.count, intent.target_height
                ),
            });
        }
        if self.da_blocks_dir.exists() {
            for entry in std::fs::read_dir(&self.da_blocks_dir)
                .map_err(|err| Error::IO(err, self.da_blocks_dir.clone()))?
            {
                let entry = entry.map_err(|err| Error::IO(err, self.da_blocks_dir.clone()))?;
                let path = entry.path();
                if !entry
                    .file_type()
                    .map_err(|err| Error::IO(err, path.clone()))?
                    .is_file()
                {
                    continue;
                }
                let Some(height) = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(|stem| stem.parse::<u64>().ok())
                else {
                    continue;
                };
                if height > intent.target_height {
                    return Err(Error::RollbackIntentInvalid {
                        path: intent_path.to_path_buf(),
                        reason: format!(
                            "DA block artifact remains above rollback target at height {height}"
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    fn prune_durable(&mut self, height: u64, rollback: bool, fail_stage: usize) -> Result<()> {
        self.invalidate_data_mmap();
        let logical_count = self.read_index_count_from_len()?;
        let durable_count = self.read_durable_index_count()?;
        let pruned_index_count = height.min(logical_count).min(durable_count);
        if rollback && pruned_index_count != height {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "rollback target exceeds the durable canonical prefix",
                ),
                self.path_to_blockchain.clone(),
            ));
        }

        // The prune marker is a forward-recovery authority and is published
        // before destructive work. Legacy rollback intents instead retain the
        // source marker until every canonical file reaches the target.
        if !rollback {
            self.publish_commit_marker(pruned_index_count)?;
            Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_BLOCK_MARKER);
        }

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
        if rollback {
            rollback_fault_point(RollbackFaultPoint::BlockIndexSynced)?;
        }

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
        if rollback {
            rollback_fault_point(RollbackFaultPoint::BlockHashesSynced)?;
        }

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
        if rollback {
            rollback_fault_point(RollbackFaultPoint::BlockDataSynced)?;
        }

        self.prune_da_block_files_above(pruned_index_count)?;
        Self::maybe_fail_prune_after_stage(fail_stage, PRUNE_STAGE_DA_SIDECARS);
        if rollback {
            rollback_fault_point(RollbackFaultPoint::DaPruned)?;
        }

        self.commit_marker_pending = None;
        self.commit_marker_count = pruned_index_count;
        if rollback {
            self.write_commit_marker(pruned_index_count)?;
            rollback_fault_point(RollbackFaultPoint::CommitMarkerPublished)?;
        }
        if self
            .read_verified_snapshot_tail_marker()?
            .is_some_and(|marker| pruned_index_count < marker.snapshot_height)
        {
            self.remove_verified_snapshot_tail_marker()?;
        }

        Ok(())
    }
}
