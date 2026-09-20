//! Complete retirement census with explicit storage effects.
//!
//! The production maintenance path and its read-only test controls share the
//! same semantic scan. Tests retain selected decoded evidence for assertions;
//! production consumes the complete evidence during admission. Directory and
//! progress-file handles remain bounded to one route at a time.

use super::autonomous_observation::ObservedAutonomousAttemptNamespace;
use super::historical_evidence::ObservedHistoricalRecoveryEvidence;
use super::native_evidence::ObservedNativeAmxEvidence;
use super::*;

/// Successful complete cross-route policy check, with test-only evidence snapshots.
/// This result cannot publish, retire, or attest any storage object.
#[derive(Default)]
pub(in crate::kura) struct LaneRetirementCensus {
    #[cfg(test)]
    pub(super) routes: BTreeMap<LaneId, LaneStorageIdentity>,
    #[cfg(test)]
    pub(super) retiring: BTreeSet<LaneRetirementIdentity>,
    #[cfg(test)]
    pub(super) certified_retirements: BTreeSet<LaneRetirementIdentity>,
    #[cfg(test)]
    pub(super) autonomous:
        BTreeMap<(LaneId, u64), (AutonomousLaneBlockArtifact, LaneBlockProposalV1, bool)>,
    #[cfg(test)]
    pub(super) inputs: BTreeMap<(LaneId, u64), LaneBlockExecutionInputArtifact>,
    #[cfg(test)]
    pub(super) certified: BTreeMap<(LaneId, u64), crate::kura::CertifiedLaneBlockArtifact>,
    #[cfg(test)]
    pub(super) receipts: BTreeMap<(LaneId, u64), LaneBlockApplicationReceiptArtifact>,
    #[cfg(test)]
    pub(super) work_items_seen: usize,
}

/// Internal storage-effect choice; it conveys no source or finality authority.
#[derive(Clone, Copy)]
pub(super) enum RetirementScanEffects {
    /// Read authenticated current evidence, refusing any unfinished repair.
    #[cfg(test)]
    Observe,
    /// Preserve the existing explicit maintenance and durability barriers.
    MaintainAndAttest { pending_canonical_bytes: u64 },
}

impl RetirementScanEffects {
    pub(super) fn receipt_matches_merge_log(
        self,
        kura: &Kura,
        receipt: &LaneBlockApplicationReceiptArtifact,
    ) -> bool {
        match self {
            #[cfg(test)]
            Self::Observe => kura.lane_block_application_receipt_matches_merge_log_without_sidecar_repair_under_prune_and_canonical_guards(receipt),
            Self::MaintainAndAttest { .. } => kura.lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(receipt),
        }
    }

    pub(super) fn autonomous_receipt_applies(
        self,
        kura: &Kura,
        receipt: &LaneBlockApplicationReceiptArtifact,
        payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    ) -> bool {
        kura.lane_retirement_merge_receipt_applies_autonomous_payload_with_append_repair_policy(
            receipt,
            payload,
            matches!(self, Self::MaintainAndAttest { .. }),
        )
    }

    pub(super) fn prepare_route(
        self,
        kura: &Kura,
        lane: LaneId,
        entry: &LaneStorageEntry,
        retiring: &BTreeSet<LaneRetirementIdentity>,
        pairs: &[(&Path, &Path, &str); 7],
    ) -> Result<BoundProgressDirectory> {
        match self {
            Self::MaintainAndAttest {
                pending_canonical_bytes,
            } => kura.maintain_lane_retirement_route_locked(
                pending_canonical_bytes,
                lane,
                entry,
                retiring,
                pairs,
            ),
            #[cfg(test)]
            Self::Observe => kura.observe_lane_retirement_route_locked(entry),
        }
    }

    pub(super) fn progress(
        self,
        kura: &Kura,
        bound: &crate::kura::BoundProgressSidecar,
        kind: &str,
    ) -> bool {
        match self {
            Self::MaintainAndAttest { .. } => kura.sync_bound_progress_sidecar(bound, kind),
            #[cfg(test)]
            Self::Observe => {
                !kura.emergency_fast_startup_enabled()
                    && kura.bound_progress_sidecar_unchanged(bound)
            }
        }
    }

    pub(super) fn absence(
        self,
        kura: &Kura,
        pair: &BoundProgressPair,
        data: &Path,
        index: &Path,
    ) -> Result<()> {
        if matches!(self, Self::MaintainAndAttest { .. }) {
            return kura.ensure_absent_geometry_progress_sidecar_remains_absent(pair, data, index);
        }
        let BoundProgressPair::Absent(namespace) = pair else {
            return Ok(());
        };
        let directory = data.parent().ok_or_else(|| {
            kura.geometry_error(
                ErrorKind::InvalidData,
                "observed progress pair has no directory",
            )
        })?;
        if namespace.data_path != data
            || namespace.index_path != index
            || index.parent() != Some(directory)
            || kura.emergency_fast_startup_enabled()
            || !kura.bound_progress_namespace_unchanged(namespace)
            || Kura::regular_sidecar_metadata_for(&kura.store_root, data, directory)?.is_some()
            || Kura::regular_sidecar_metadata_for(&kura.store_root, index, directory)?.is_some()
            || !kura.bound_progress_namespace_unchanged(namespace)
        {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "observed progress-sidecar absence changed during retirement scan",
            ));
        }
        Ok(())
    }

    pub(super) fn historical(
        self,
        observation: ObservedHistoricalRecoveryEvidence<'_>,
    ) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {
        match self {
            #[cfg(test)]
            Self::Observe => observation.into_observed(),
            Self::MaintainAndAttest { .. } => observation.attest(),
        }
    }

    pub(super) fn native(
        self,
        observation: ObservedNativeAmxEvidence<'_>,
    ) -> Result<(
        BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1>,
        BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact>,
    )> {
        match self {
            #[cfg(test)]
            Self::Observe => observation.into_observed(),
            Self::MaintainAndAttest { .. } => observation.attest(),
        }
    }

    pub(super) fn autonomous(
        self,
        observation: ObservedAutonomousAttemptNamespace<'_>,
    ) -> Result<BTreeMap<u64, (AutonomousLaneBlockArtifact, LaneBlockProposalV1, bool)>> {
        match self {
            #[cfg(test)]
            Self::Observe => observation.into_observed(),
            Self::MaintainAndAttest { .. } => observation.attest(),
        }
    }
}

impl Kura {
    /// Authenticate the same frontier inputs as maintenance, without repairing them.
    /// Missing or incomplete progress remains a local maintenance condition.
    #[cfg(test)]
    fn observe_lane_retirement_route_locked(
        &self,
        entry: &LaneStorageEntry,
    ) -> Result<BoundProgressDirectory> {
        if !super::super::sumeragi_v2_validator_storage_supported()
            || self.emergency_fast_startup_enabled()
        {
            return Err(self.geometry_error(
                ErrorKind::Unsupported,
                "retirement observation requires authenticated validator storage",
            ));
        }
        let lane_artifacts = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let directory = Self::open_bound_progress_directory(&self.store_root, &lane_artifacts)?;
        if let Some(frontier) =
            self.read_latest_certified_lane_block_frontier_locked(entry, false)?
        {
            let (data, index) = Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
            let mut pair = self.open_geometry_bound_progress_sidecar(&data, &index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &pair,
                &directory,
                &data,
                &index,
                "observed certified frontier",
            )?;
            let Some(bound) = pair.sidecar_mut() else {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "certified retirement frontier requires progress-pair maintenance",
                ));
            };
            let height = frontier
                .frontier
                .artifact
                .proposal
                .descriptor
                .lane_block_height;
            let heights = self.bound_indexed_sidecar_payload_heights(
                bound,
                "observed certified frontier",
                MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
            )?;
            if heights.last().copied() != Some(height)
                || self
                    .read_certified_lane_block_artifact_from_bound_locked(
                        entry.lane_id,
                        height,
                        bound,
                    )
                    .as_ref()
                    != Some(&frontier.frontier.artifact)
                || !self.bound_progress_sidecar_unchanged(bound)
            {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "certified retirement frontier differs from its observed progress pair",
                ));
            }
            self.confirm_latest_certified_lane_block_frontier_read_locked(
                entry,
                &frontier.snapshot,
            )?;
        }
        let frontier_path =
            Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root);
        if let Some(frontier) =
            self.decode_lane_merge_application_frontier(entry, &frontier_path)?
            && self
                .lane_merge_application_frontier_expected_receipt_with_append_repair_policy_under_prune_and_canonical_guards(
                    &frontier,
                    false,
                )
                .is_none()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "observed retirement merge frontier has no authenticated carrier",
            ));
        }
        if !self.geometry_bound_progress_directory_unchanged(&directory) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retirement route changed while observing its frontiers",
            ));
        }
        Ok(directory)
    }
}
