//! Storage maintenance that precedes one route's retirement evidence snapshot.

use super::super::AuthenticatedLaneHistoryRetention;
use super::{
    BoundProgressDirectory, Error, ErrorKind, Kura, LaneHistoryCompactionOutcome, LaneId,
    LaneRetirementIdentity, LaneStorageEntry, Result,
};
use std::{collections::BTreeSet, path::Path};

impl Kura {
    /// Only a committed terminal rewrite needs the retained-history recovery owner.
    /// Ordinary frontier repair preserves its own admission and error ordering.
    fn certified_history_has_committed_rewrite_locked(
        &self,
        entry: &LaneStorageEntry,
    ) -> Result<bool> {
        for (data, index) in [
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root),
            Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root),
        ] {
            if self.bound_progress_sidecar_directory_is_absent(&data, &index)? {
                continue;
            }
            let namespace = self.open_bound_progress_namespace(&data, &index)?;
            if self
                .open_optional_bound_progress_file(&namespace, &index.with_extension("index.tmp"))?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Repair and compact one route before binding its immutable scan directory.
    ///
    /// The caller retains prune -> canonical-chain -> geometry -> sidecar guards.
    /// This method acquires none of those locks and preserves the original order:
    /// certified frontier recovery, authenticated merge-frontier compaction, then
    /// recovery of all seven fixed progress pairs. Capacity-blocked compaction
    /// leaves its history intact for the existing atomic retirement archive.
    ///
    /// This is mutating local storage maintenance, not candidate admission or a
    /// durable authorization. The returned directory binds the subsequent scan;
    /// it does not prove that the scan's remaining evidence is admissible.
    // TODO: run maintenance before candidate reservation once the remaining
    // evidence attestations and competing artifact writers have explicit owners.
    pub(super) fn maintain_lane_retirement_route_locked(
        &self,
        pending_canonical_bytes: u64,
        storage_lane_id: LaneId,
        entry: &LaneStorageEntry,
        retiring: &BTreeSet<LaneRetirementIdentity>,
        fixed_progress_pairs: &[(&Path, &Path, &str); 7],
    ) -> Result<BoundProgressDirectory> {
        let lane_artifacts = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let merge_application_frontier =
            Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root);
        if let Some(frontier_read) =
            self.read_latest_certified_lane_block_frontier_locked(entry, true)?
        {
            if self.certified_history_has_committed_rewrite_locked(entry)? {
                let retention = self
                    .decode_lane_merge_application_frontier(entry, &merge_application_frontier)?
                    .map(|frontier| {
                        if self
                            .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(&frontier)
                            .is_none()
                        {
                            return Err(self.geometry_error(
                                ErrorKind::InvalidData,
                                "lane retirement rewrite has no authenticated merge carrier",
                            ));
                        }
                        Ok(AuthenticatedLaneHistoryRetention {
                            entry: entry.clone(),
                            first_retained_height: frontier.lane_block_height
                                .saturating_sub(self.lane_history_retention.get() as u64)
                                .saturating_add(1),
                            frontier,
                        })
                    })
                    .transpose()?;
                self.recover_certified_bundle_history_rewrites_locked(
                    entry,
                    retention.as_ref(),
                    Some(&frontier_read.frontier.artifact),
                )?;
            }
            self.recover_certified_lane_block_pair_from_frontier_locked(
                entry,
                &frontier_read.frontier.artifact,
                None,
                None,
            )
            .map_err(|error| match error {
                Error::IO(source, _) if source.kind() == ErrorKind::WouldBlock => self
                    .geometry_error(
                        ErrorKind::WouldBlock,
                        "lane retirement certified lane block durability attestation failed",
                    ),
                error => error,
            })?;
            self.confirm_latest_certified_lane_block_frontier_read_locked(
                entry,
                &frontier_read.snapshot,
            )?;
            self.note_certified_frontier_artifact_validation(
                storage_lane_id,
                &frontier_read.frontier,
                &frontier_read.snapshot,
            );
        }
        if let Some(frontier) =
            self.decode_lane_merge_application_frontier(entry, &merge_application_frontier)?
        {
            if self
                .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                    &frontier,
                )
                .is_none()
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement merge application frontier has no authenticated carrier",
                ));
            }
            let frontier_identity = LaneRetirementIdentity {
                lane_id: frontier.lane_id,
                dataspace_id: frontier.dataspace_id,
                lane_incarnation: frontier.lane_incarnation,
            };
            if retiring.contains(&frontier_identity) {
                match self.compact_lane_histories_through_merge_frontier_locked(
                    pending_canonical_bytes,
                    entry,
                    &frontier,
                )? {
                    LaneHistoryCompactionOutcome::Complete => {}
                    LaneHistoryCompactionOutcome::CapacityBlocked => {
                        iroha_logger::debug!(
                            lane = %entry.lane_id.as_u32(),
                            "lane retirement retained uncompacted history in its atomic archive because configured temporary capacity is unavailable"
                        );
                    }
                }
            }
        }
        self.recover_geometry_progress_pairs_before_snapshot(
            &lane_artifacts,
            fixed_progress_pairs,
            "first-release lane retirement",
        )
    }
}
