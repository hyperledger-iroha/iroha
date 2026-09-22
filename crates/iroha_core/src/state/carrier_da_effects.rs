//! Retained DA visibility and policy projections from the original candidate.
//!
//! Publication consumes these inputs without consulting a newer catalog or a
//! disposable cursor journal for incarnation authority. Index insertion still
//! allocates: aggregate admission must fund those writes before this component
//! can be part of the sole carrier publisher.

use super::*;
use iroha_data_model::da::confidential_compute::ConfidentialComputePolicy;

/// Original bundle and its fully projected visibility, cursor and receipt inputs.
pub(super) struct PreparedDaCommitmentEffects {
    pending: PendingDaCommitmentBundle,
    lane_config: iroha_config::parameters::actual::LaneConfig,
    active: Vec<DaCommitmentRecord>,
    query_visible: BTreeSet<DaCommitmentKey>,
    identity_visible: BTreeSet<DaCommitmentKey>,
    confidential: Vec<(
        DaCommitmentRecord,
        DaCommitmentLocation,
        ConfidentialComputePolicy,
    )>,
}

/// Disposable journal persistence after the State generation has completed.
pub(super) struct DaCommitmentPostPublication {
    lane_config: Option<iroha_config::parameters::actual::LaneConfig>,
    snapshot: Option<DaShardCursorJournal>,
    captured: bool,
}

impl PreparedDaCommitmentEffects {
    /// Capture only from the validated candidate's post-lifecycle projection.
    /// Retired-lane identities remain reserved, while a recreated lane hides
    /// evidence at or before its authenticated incarnation activation height.
    pub(super) fn prepare(
        pending: PendingDaCommitmentBundle,
        nexus: &iroha_config::parameters::actual::Nexus,
        runtime: &SnapshotNexusRuntime,
    ) -> Self {
        let policy_context = crate::da::ActiveLaneProofPolicyContext::new(nexus);
        let height = pending.block_height;
        let mut active = Vec::new();
        let mut query_visible = BTreeSet::new();
        let mut identity_visible = BTreeSet::new();
        let mut confidential = Vec::new();
        for (index, record) in pending.bundle.commitments.iter().enumerate() {
            // This sorted lineage belongs to the original canonical MV record,
            // including retired lanes. A local journal cannot suppress a record.
            let visible_incarnation = runtime
                .lane_incarnation_lineage
                .binary_search_by_key(&record.lane_id, |entry| entry.lane_id)
                .ok()
                .is_none_or(|index| {
                    let activation = runtime.lane_incarnation_lineage[index].activation_height;
                    activation == 0 || height > activation
                });
            if !visible_incarnation {
                continue;
            }
            let key = DaCommitmentKey::from_record(record);
            if nexus.lane_config.entry(record.lane_id).is_none() {
                // The canonical bundle keeps its original positions and bytes.
                // Retirement hides query rows, but does not free identities.
                identity_visible.insert(key);
                continue;
            }
            let policy = policy_context
                .enforce_commitment_at_height(record, height)
                .map_err(crate::da::DaCommitmentValidationError::from)
                .and_then(|()| {
                    crate::da::validate_confidential_compute_record(&nexus.lane_config, record)
                        .map_err(crate::da::DaCommitmentValidationError::from)
                });
            match policy {
                Ok(policy) => {
                    identity_visible.insert(key);
                    query_visible.insert(key);
                    active.push(record.clone());
                    if let Some(policy) = policy
                        && let Some(index_in_bundle) = crate::da::da_bundle_location_index(index)
                    {
                        confidential.push((
                            record.clone(),
                            DaCommitmentLocation {
                                block_height: height,
                                index_in_bundle,
                            },
                            policy,
                        ));
                    }
                }
                Err(error) => {
                    warn!(
                        ?error,
                        height,
                        lane = record.lane_id.as_u32(),
                        "omitting DA query projection incompatible with the accepted lifecycle"
                    );
                }
            }
        }
        Self {
            pending,
            lane_config: nexus.lane_config.clone(),
            active,
            query_visible,
            identity_visible,
            confidential,
        }
    }

    /// Consume under the enclosing State generation; no catalog or policy is rebuilt.
    pub(super) fn publish(
        self,
        state: &State,
        indexes: &mut effect_publication::StateEffectLocks<'_>,
        _publication: &StateViewGenerationWriteGuard<'_>,
        persist_cursor_journal: bool,
    ) -> DaCommitmentPostPublication {
        let Self {
            pending,
            lane_config,
            active,
            query_visible,
            identity_visible,
            confidential,
        } = self;
        let height = pending.block_height;
        indexes
            .da_commitments
            .as_mut()
            .expect("prepared DA commitments")
            .insert_bundle_with_visibility_filter(
                height,
                pending.bundle,
                |record| identity_visible.contains(&DaCommitmentKey::from_record(record)),
                |record| query_visible.contains(&DaCommitmentKey::from_record(record)),
            );
        let cursor_result = state.advance_da_shard_cursors_into(
            indexes
                .da_shard_cursors
                .as_mut()
                .expect("prepared DA shard cursors"),
            &lane_config,
            height,
            &active,
        );
        let persist = match cursor_result {
            Ok(()) => persist_cursor_journal,
            Err(error) => {
                warn!(
                    ?error,
                    height, "failed to advance DA shard cursor index during block commit"
                );
                false
            }
        };
        if let Err(error) = state.advance_da_receipt_cursors_into(
            indexes
                .da_receipt_cursors
                .as_mut()
                .expect("prepared DA receipt cursors"),
            height,
            &active,
        ) {
            warn!(
                ?error,
                height, "failed to advance DA receipt cursor index during block commit"
            );
        }
        {
            let store = indexes
                .da_confidential_compute
                .as_mut()
                .expect("prepared confidential compute");
            for (record, location, policy) in confidential {
                store.insert(&record, location, &policy);
            }
        }
        DaCommitmentPostPublication {
            lane_config: persist.then_some(lane_config),
            snapshot: None,
            captured: false,
        }
    }
}

impl DaCommitmentPostPublication {
    /// Retain the final same-carrier cursor image through the already acquired
    /// original writer. This must follow every lifecycle and DA cursor update.
    pub(super) fn capture_snapshot(&mut self, state: &State, cursors: &DaShardCursorIndex) {
        assert!(!self.captured, "original DA cursor snapshot captured once");
        self.captured = true;
        let Some(lane_config) = self.lane_config.as_ref() else {
            return;
        };
        let path = state.da_shard_cursor_journal_path();
        if !path.as_os_str().is_empty() {
            self.snapshot = Some(DaShardCursorJournal::from_index(
                lane_config,
                cursors,
                &path,
            ));
        }
    }

    /// Schedule only after generation publication, with the original lane mapping.
    /// The complete carrier publisher must retain its physical fences until this
    /// final cursor projection is captured, then retain any unfinished completion.
    pub(super) fn publish(self, state: &State) {
        assert!(
            self.captured,
            "original DA cursor snapshot precedes release"
        );
        let Some(snapshot) = self.snapshot else {
            return;
        };
        state.da_shard_cursor_persistor.schedule(snapshot);
    }
}

#[cfg(test)]
#[path = "carrier_da_effects_tests.rs"]
mod tests;
