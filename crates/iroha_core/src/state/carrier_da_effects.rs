//! Retained DA visibility and policy projections from the original candidate.
//!
//! Publication consumes these inputs without consulting a newer catalog or a
//! disposable cursor journal for incarnation authority. Index insertion still
//! allocates: aggregate admission must fund those writes before this component
//! can be part of the sole carrier publisher.

use super::*;
use crate::execution_attempt::ExecutionDeferred;
use mv::allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError, PrepaidBufferError,
};
use std::alloc::Layout;

/// Original bundle and its fully projected visibility, cursor and receipt inputs.
pub(super) struct PreparedDaCommitmentEffects {
    pending: PendingDaCommitmentBundle,
    active: ChargedBuffer<usize>,
    query_visible: ChargedBuffer<DaCommitmentKey>,
    identity_visible: ChargedBuffer<DaCommitmentKey>,
    confidential: ChargedBuffer<usize>,
}

/// Disposable journal persistence after the State generation has completed.
pub(super) struct DaCommitmentPostPublication {
    persist: bool,
    snapshot: Option<DaShardCursorJournal>,
    captured: bool,
}

impl PreparedDaCommitmentEffects {
    /// Capture only from the validated candidate's post-lifecycle projection.
    /// Retired-lane identities remain reserved, while a recreated lane hides
    /// evidence at or before its authenticated incarnation activation height.
    pub(super) fn try_prepare(
        pending: PendingDaCommitmentBundle,
        nexus: &iroha_config::parameters::actual::Nexus,
        runtime: &SnapshotNexusRuntime,
        budget: &AllocationBudget,
    ) -> Result<Self, (PendingDaCommitmentBundle, ExecutionDeferred)> {
        // All four descriptor arrays are admitted before classification. They
        // refer only to original bundle positions: no record, signature, policy
        // graph or full lane configuration is cloned during preparation.
        let (mut query_visible, mut identity_visible, mut active, mut confidential) =
            match reserve_projection(pending.bundle.commitments.len(), budget) {
                Ok(indexes) => indexes,
                Err(error) => return Err((pending, error)),
            };
        let policy_context = crate::da::ActiveLaneProofPolicyContext::new(nexus);
        let height = pending.block_height;
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
                identity_visible.push_reserved(key);
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
                    identity_visible.push_reserved(key);
                    query_visible.push_reserved(key);
                    active.push_reserved(index);
                    if policy.is_some() && crate::da::da_bundle_location_index(index).is_some() {
                        confidential.push_reserved(index);
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
        sort_unique(&mut query_visible);
        sort_unique(&mut identity_visible);
        Ok(Self {
            pending,
            active,
            query_visible,
            identity_visible,
            confidential,
        })
    }

    /// Consume under the enclosing State generation, borrowing the same retained
    /// candidate configuration used by preparation. The publisher owns that
    /// immutable snapshot through cursor capture; current live config is never read.
    pub(super) fn publish(
        self,
        state: &State,
        indexes: &mut effect_publication::StateEffectLocks<'_>,
        lane_config: &iroha_config::parameters::actual::LaneConfig,
        _publication: &StateViewGenerationWriteGuard<'_>,
        persist_cursor_journal: bool,
    ) -> DaCommitmentPostPublication {
        let Self {
            pending,
            active,
            query_visible,
            identity_visible,
            confidential,
        } = self;
        let height = pending.block_height;
        let cursor_result = state.advance_da_shard_cursors_into(
            indexes
                .da_shard_cursors
                .as_mut()
                .expect("prepared DA shard cursors"),
            lane_config,
            height,
            active
                .as_slice()
                .iter()
                .map(|&index| &pending.bundle.commitments[index]),
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
            active
                .as_slice()
                .iter()
                .map(|&index| &pending.bundle.commitments[index]),
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
            for &index in confidential.as_slice() {
                let record = &pending.bundle.commitments[index];
                let policy = lane_config
                    .confidential_compute_policy(record.lane_id)
                    .expect(
                        "the original frozen candidate retains its admitted confidential policy",
                    );
                let location = DaCommitmentLocation {
                    block_height: height,
                    index_in_bundle: crate::da::da_bundle_location_index(index)
                        .expect("prepared original bundle location"),
                };
                store.insert(record, location, policy);
            }
        }
        indexes
            .da_commitments
            .as_mut()
            .expect("prepared DA commitments")
            .insert_bundle_with_visibility_filter(
                height,
                pending.bundle,
                |record| {
                    identity_visible
                        .as_slice()
                        .binary_search(&DaCommitmentKey::from_record(record))
                        .is_ok()
                },
                |record| {
                    query_visible
                        .as_slice()
                        .binary_search(&DaCommitmentKey::from_record(record))
                        .is_ok()
                },
            );
        DaCommitmentPostPublication {
            persist,
            snapshot: None,
            captured: false,
        }
    }
}

// Each descriptor array has at most one entry per original bundle row. All
// exact backing layouts are admitted together before any allocation is made.
fn reserve_projection(
    count: usize,
    budget: &AllocationBudget,
) -> Result<
    (
        ChargedBuffer<DaCommitmentKey>,
        ChargedBuffer<DaCommitmentKey>,
        ChargedBuffer<usize>,
        ChargedBuffer<usize>,
    ),
    ExecutionDeferred,
> {
    let keys = Layout::array::<DaCommitmentKey>(count)
        .map_err(|_| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
    let positions = Layout::array::<usize>(count)
        .map_err(|_| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
    let mut reservation = budget
        .try_reserve_layouts([keys, keys, positions, positions])
        .map_err(ExecutionDeferred::from)?;
    let query = allocate_projection(count, &mut reservation)?;
    let identity = allocate_projection(count, &mut reservation)?;
    let active = allocate_projection(count, &mut reservation)?;
    let confidential = allocate_projection(count, &mut reservation)?;
    Ok((query, identity, active, confidential))
}
fn allocate_projection<T>(
    count: usize,
    reservation: &mut mv::allocation::AllocationReservation,
) -> Result<ChargedBuffer<T>, ExecutionDeferred> {
    ChargedBuffer::from_reservation(count, reservation).map_err(|error| match error {
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(refusal)) => refusal.into(),
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. }) => {
            ivm::error::ExecutionDeferral::AllocationUnavailable.into()
        }
        PrepaidBufferError::Reservation(_) => {
            unreachable!("all exact DA projection layouts were admitted together")
        }
    })
}

fn sort_unique(keys: &mut ChargedBuffer<DaCommitmentKey>) {
    let values = keys.as_mut_slice();
    values.sort_unstable();
    let mut unique = 0;
    for index in 0..values.len() {
        if unique == 0 || values[index] != values[unique - 1] {
            values[unique] = values[index];
            unique += 1;
        }
    }
    keys.truncate(unique);
}

impl DaCommitmentPostPublication {
    /// Retain the final same-carrier cursor image through the already acquired
    /// original writer. This must follow every lifecycle and DA cursor update.
    pub(super) fn capture_snapshot(
        &mut self,
        state: &State,
        lane_config: &iroha_config::parameters::actual::LaneConfig,
        cursors: &DaShardCursorIndex,
    ) {
        assert!(!self.captured, "original DA cursor snapshot captured once");
        self.captured = true;
        if !self.persist {
            return;
        }
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
