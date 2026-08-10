impl MergeLedgerLog {
    fn snapshot(&self) -> Vec<MergeLedgerEntry> {
        self.entries.clone()
    }

    fn latest_snapshot(&self, limit: usize) -> Vec<MergeLedgerEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }

    fn contains_hash(&mut self, hash: HashOf<MergeLedgerEntry>) -> bool {
        #[cfg(test)]
        {
            self.indexed_membership_checks = self.indexed_membership_checks.saturating_add(1);
        }
        self.frames_by_hash.contains_key(&hash)
    }

    /// Provisional snapshot authentication must not open, create, or repair the
    /// unauthenticated durable merge log.
    fn startup(path: &Path, cache_capacity: usize, provisional: bool) -> Result<Self> {
        if provisional {
            Ok(Self::in_memory(cache_capacity))
        } else {
            Self::open_at(path, cache_capacity)
        }
    }

    fn validate_execution_entry_index_update(
        entries: &BTreeMap<(LaneId, DataSpaceId, Hash), (u64, HashOf<MergeLedgerEntry>)>,
        entry: &MergeLedgerEntry,
    ) -> Result<()> {
        let Some(batch) = entry.execution_batch.as_ref() else {
            return Ok(());
        };
        let mut routes = BTreeSet::new();
        for execution in &batch.lanes {
            let descriptor = &execution.proposal.descriptor;
            let route = (
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
            );
            if !routes.insert(route) {
                return Err(Error::MergeCarrierConflict(
                    "merge execution batch duplicates one route/incarnation".to_owned(),
                ));
            }
            if let Some((current_height, current_entry_hash)) = entries.get(&route)
                && descriptor.lane_block_height <= *current_height
            {
                return Err(Error::MergeCarrierConflict(format!(
                    "merge execution index is non-monotonic for route/incarnation: current height {current_height} entry {current_entry_hash}, next height {} entry {}",
                    descriptor.lane_block_height,
                    entry.canonical_hash(),
                )));
            }
        }
        Ok(())
    }

    fn record_execution_entry_unchecked(
        entries: &mut BTreeMap<(LaneId, DataSpaceId, Hash), (u64, HashOf<MergeLedgerEntry>)>,
        entry: &MergeLedgerEntry,
    ) {
        let Some(batch) = entry.execution_batch.as_ref() else {
            return;
        };
        let entry_hash = entry.canonical_hash();
        for execution in &batch.lanes {
            let descriptor = &execution.proposal.descriptor;
            let height = descriptor.lane_block_height;
            entries.insert(
                (
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                ),
                (height, entry_hash),
            );
        }
    }

    /// Source-bound reconstruction guard shared by durable log load and in-memory
    /// truncation. `preflight_append` invokes the same validator before any frame
    /// bytes are written, so an equal-height fork can never be silently selected.
    fn record_execution_entry(
        entries: &mut BTreeMap<(LaneId, DataSpaceId, Hash), (u64, HashOf<MergeLedgerEntry>)>,
        entry: &MergeLedgerEntry,
    ) -> Result<()> {
        Self::validate_execution_entry_index_update(entries, entry)?;
        Self::record_execution_entry_unchecked(entries, entry);
        Ok(())
    }

    fn execution_index_for_entries(
        ordered_entries: &[MergeLedgerEntry],
    ) -> Result<BTreeMap<(LaneId, DataSpaceId, Hash), (u64, HashOf<MergeLedgerEntry>)>> {
        let mut index = BTreeMap::new();
        for entry in ordered_entries {
            Self::record_execution_entry(&mut index, entry)?;
        }
        Ok(index)
    }

    #[cfg(test)]
    fn latest_execution_heights(&self) -> BTreeMap<(LaneId, DataSpaceId, Hash), u64> {
        self.latest_execution_entries
            .iter()
            .map(|(route, (height, _))| (*route, *height))
            .collect()
    }

    fn latest_execution_height(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> Option<u64> {
        self.latest_execution_entries
            .get(&(lane_id, dataspace_id, lane_incarnation))
            .map(|(height, _)| *height)
    }

    fn latest_execution_entry(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> Option<(u64, HashOf<MergeLedgerEntry>)> {
        self.latest_execution_entries
            .get(&(lane_id, dataspace_id, lane_incarnation))
            .copied()
    }

    /// Reconstruct a bounded exact identity-to-carrier map in chronological order.
    ///
    /// The route-latest index remains the constant-time tip path. Startup calls
    /// this forward pass only for older incomplete lifecycle identities that have
    /// neither a local terminal outcome nor a durability-attested receipt. Memory
    /// is therefore bounded by the caller-supplied identity set, not merge-history
    /// length.
    fn execution_entries_for_bounded_identities(
        &mut self,
        identities: &BTreeSet<(LaneId, DataSpaceId, Hash, u64, u64)>,
    ) -> Result<BTreeMap<(LaneId, DataSpaceId, Hash, u64, u64), HashOf<MergeLedgerEntry>>> {
        if identities.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(Error::MergeCarrierConflict(
                "exact merge execution reconstruction exceeds its identity bound".to_owned(),
            ));
        }
        self.recover_failed_append_tail()?;
        #[cfg(test)]
        {
            self.complete_execution_scans = self.complete_execution_scans.saturating_add(1);
        }
        let mut found = BTreeMap::new();
        for index in 1..=self.total_entries {
            let epoch = u64::try_from(index)?;
            let frame = *self.frames_by_epoch.get(&epoch).ok_or_else(|| {
                Error::MergeCarrierConflict(
                    "exact merge execution reconstruction lost an indexed epoch".to_owned(),
                )
            })?;
            let entry = if let Some(file) = self.file.as_mut() {
                Self::read_indexed_frame(file, frame)?
            } else {
                self.in_memory_entries
                    .get(&frame.entry_hash)
                    .cloned()
                    .ok_or_else(|| {
                        Error::MergeCarrierConflict(
                            "exact merge execution reconstruction lost an in-memory frame"
                                .to_owned(),
                        )
                    })?
            };
            if let Some(batch) = entry.execution_batch.as_ref() {
                for execution in &batch.lanes {
                    let descriptor = &execution.proposal.descriptor;
                    let identity = (
                        descriptor.lane_id,
                        descriptor.dataspace_id,
                        descriptor.lane_incarnation,
                        descriptor.lane_block_height,
                        descriptor.proposal_height,
                    );
                    if identities.contains(&identity)
                        && found.insert(identity, frame.entry_hash).is_some()
                    {
                        return Err(Error::MergeCarrierConflict(
                            "exact merge execution reconstruction found a duplicate identity"
                                .to_owned(),
                        ));
                    }
                }
            }
            if found.len() == identities.len() {
                break;
            }
        }
        if let Some(file) = self.file.as_mut() {
            file.try_io(|inner| inner.seek(SeekFrom::End(0)))?;
        }
        Ok(found)
    }

    fn has_execution_for_route(&self, lane_id: LaneId, dataspace_id: DataSpaceId) -> bool {
        self.latest_execution_entries
            .keys()
            .any(|(candidate_lane, candidate_dataspace, _)| {
                *candidate_lane == lane_id && *candidate_dataspace == dataspace_id
            })
    }

    fn entry_by_hash(
        &mut self,
        hash: HashOf<MergeLedgerEntry>,
    ) -> Result<Option<MergeLedgerEntry>> {
        self.entry_by_hash_with_append_repair_policy(hash, true)
    }

    fn entry_by_hash_with_append_repair_policy(
        &mut self,
        hash: HashOf<MergeLedgerEntry>,
        repair_append_tail: bool,
    ) -> Result<Option<MergeLedgerEntry>> {
        if repair_append_tail {
            self.recover_failed_append_tail()?;
        } else if self.append_recovery_offset.is_some() {
            return Err(Error::MergeCarrierConflict(
                "merge ledger has an unresolved append tail".to_owned(),
            ));
        }
        #[cfg(test)]
        {
            self.indexed_lookups = self.indexed_lookups.saturating_add(1);
        }
        let Some(frame) = self.frames_by_hash.get(&hash).copied() else {
            return Ok(None);
        };
        let Some(file) = self.file.as_mut() else {
            return self
                .in_memory_entries
                .get(&hash)
                .cloned()
                .map(Some)
                .ok_or_else(|| {
                    Error::MergeCarrierConflict(
                        "in-memory merge frame index references a missing entry".to_owned(),
                    )
                });
        };
        let result = Self::read_indexed_frame(file, frame).map(Some);
        let restore = file.try_io(|inner| inner.seek(SeekFrom::End(0)));
        restore?;
        result
    }
}
