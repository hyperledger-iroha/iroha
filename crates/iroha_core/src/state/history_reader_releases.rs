//! Original reader notices retained through State views and enclosing physical owners.

use super::*;
use concread::release::DeferredReleaseBatch;

impl State {
    /// Original physical history pools whose refunds must outlive an outer fence.
    pub(crate) fn history_allocation_budgets(
        &self,
    ) -> (
        mv::allocation::AllocationBudget,
        mv::allocation::AllocationBudget,
    ) {
        (
            self.block_hashes.budget.clone(),
            self.transactions.budget.clone(),
        )
    }
}

/// Same-source view readers whose actual unlocks notify only after this owner drops.
/// Physical guards never remain held between calls; no caller-selected source is accepted.
pub(crate) struct StateViewReleases<'state> {
    state: &'state State,
    pub(super) lifecycle: LaneLifecycleReleases<'state>,
}

/// Original notices detached from a State borrow before an authenticated State swap.
/// This is only cleanup custody; it grants no read or publication authority.
#[must_use = "retain original read notices through every enclosing physical owner"]
pub(crate) struct StateViewRetirement {
    _indexes: [DeferredReleaseBatch; 12],
    _hashes: Option<DeferredReleaseBatch>,
    _membership: DeferredReleaseBatch,
}

impl<'state> StateViewReleases<'state> {
    /// Bind all view-release sources to this exact State before taking outer fences.
    pub(crate) fn new(state: &'state State) -> Self {
        Self {
            state,
            lifecycle: LaneLifecycleReleases::new(state),
        }
    }

    /// The original State; this reference does not borrow mutable release custody.
    pub(crate) fn state(&self) -> &'state State {
        self.state
    }

    /// Attempt one original-State observation, retaining every actual reader unlock.
    pub(crate) fn try_view_once(
        &mut self,
    ) -> Result<Option<StateView<'state>>, LaneLifecycleError> {
        self.state.try_view_once_with_index_releases(
            &mut self.lifecycle.header,
            &mut self.lifecycle.manifests,
            &mut self.lifecycle.sccp,
            &mut self.lifecycle.hashes,
            &mut self.lifecycle.membership,
        )
    }

    /// Hash the complete current State without delivering notices beneath its writers.
    pub(super) fn lane_execution_state_hash(
        &mut self,
    ) -> Result<HashOf<BlockHeader>, crate::snapshot::SnapshotCaptureError> {
        crate::snapshot::canonical_state_snapshot_hash_with_releases(self)
            .map(HashOf::<BlockHeader>::from_untyped_unchecked)
    }

    /// End read authority while retaining all original notices through later mutation.
    pub(crate) fn into_retirement(self) -> StateViewRetirement {
        let LaneLifecycleReleases {
            hashes,
            membership,
            header,
            sccp,
            merge_admission,
            relays,
            manifests,
            privacy,
            commitments,
            confidential_compute,
            receipt_cursors,
            shard_cursors,
            pin_intents,
            hydrated,
        } = self.lifecycle;
        StateViewRetirement {
            _indexes: [
                header.into_releases(),
                sccp.into_releases(),
                merge_admission.into_releases(),
                relays.into_releases(),
                manifests.into_releases(),
                privacy.into_releases(),
                commitments.into_releases(),
                confidential_compute.into_releases(),
                receipt_cursors.into_releases(),
                shard_cursors.into_releases(),
                pin_intents.into_releases(),
                hydrated.into_releases(),
            ],
            _hashes: hashes,
            _membership: membership,
        }
    }
}

impl BlockHashes {
    /// Only an ordinary original map has an active-reader release source.
    pub(super) fn reader_release_batch(&self) -> Option<DeferredReleaseBatch> {
        self.map().map(BlockHashMap::reader_release_batch)
    }

    /// Read the exact original map while retaining its actual active-lock release.
    pub(super) fn view_retaining(
        &self,
        releases: &mut Option<DeferredReleaseBatch>,
    ) -> BlockHashesView<'_> {
        let inner = match (&self.inner, releases.as_mut()) {
            (BlockHashStorage::Owned(map), Some(releases)) => BlockHashesViewInner::Owned(
                map.read_retaining(releases)
                    .expect("original hash reader source must be healthy"),
            ),
            (BlockHashStorage::EmergencyFastMapped(mapping), None) => {
                BlockHashesViewInner::Mapped(mapped_block_hashes(mapping))
            }
            (BlockHashStorage::EmergencyFastEmpty, None) => BlockHashesViewInner::Mapped(&[]),
            _ => panic!("history reader custody differs from its original storage mode"),
        };
        BlockHashesView { inner }
    }
}

#[cfg(test)]
#[path = "history_reader_release_tests.rs"]
mod tests;
