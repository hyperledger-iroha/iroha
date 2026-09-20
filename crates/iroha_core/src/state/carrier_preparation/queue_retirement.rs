//! Retain the original service Queue cut for the exact captured retiring routes.
//!
//! Queue emptiness is only one local publication obligation. The captured
//! lifecycle, certified frontier and original Kura retirement checks continue
//! to authorize the actual namespace transition independently.

use crate::{
    queue::{QueueLaneRetirementCut, QueueLaneRetirementUnavailable},
    state::{
        BlockHashOwner, LaneLifecycleError, State,
        carrier_geometry_preparation::PreparedCarrierGeometry,
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::BlockHeader,
    nexus::{DataSpaceId, LaneId},
};
use std::sync::Arc;

/// Local retirement refusal; no variant is a consensus-invalidity verdict.
#[derive(Debug)]
pub(in crate::state) enum CarrierQueueRetirementError {
    /// No original service Queue capability was supplied for an actual retirement.
    Missing,
    /// The service capability belongs to another original State.
    ForeignState,
    /// The original Queue is physically held by independent work.
    Busy {
        field: &'static str,
        wait: mv::ReleaseWait,
    },
    /// The exact old route still owns work; retry only after its real release.
    Pending {
        lane: LaneId,
        dataspace: DataSpaceId,
        incarnation: Hash,
        wait: mv::ReleaseWait,
    },
    /// Ambiguous Queue durability cannot be treated as an empty route.
    Unavailable(QueueLaneRetirementUnavailable),
    /// The captured retiring route lost its canonical predecessor identity.
    Geometry(LaneLifecycleError),
}

/// Move-only custody retained until all authoritative State components are visible.
/// Drop later component/State guards before this cut; never carry it across await.
pub(in crate::state) struct CarrierQueueRetirement<'queue> {
    state_owner: Arc<BlockHashOwner>,
    header: BlockHeader,
    routes: Vec<(LaneId, DataSpaceId, Hash)>,
    _cut: QueueLaneRetirementCut<'queue>,
}

impl<'queue> CarrierQueueRetirement<'queue> {
    /// Observe every exact predecessor route under the shared Queue predicate.
    /// Aggregate installation admission must cover this vector and Queue scan.
    pub(in crate::state) fn try_new(
        target: &State,
        geometry: &PreparedCarrierGeometry,
        header: BlockHeader,
        source: &crate::sumeragi::v2_apply::carrier_queue_retirement::OriginalCarrierQueue<'queue>,
        cut: QueueLaneRetirementCut<'queue>,
    ) -> Result<Self, CarrierQueueRetirementError> {
        if !source.belongs_to(target)
            || !source.owns_cut(&cut)
            || !geometry.matches_publication_target(target, header)
        {
            return Err(CarrierQueueRetirementError::ForeignState);
        }
        let mut routes = Vec::new();
        geometry
            .for_each_retirement_route(|lane, dataspace, incarnation| {
                routes.push((lane, dataspace, incarnation));
                Ok(())
            })
            .map_err(CarrierQueueRetirementError::Geometry)?;
        for &(lane, dataspace, incarnation) in &routes {
            match cut.lane_pending_work_release(lane, dataspace, incarnation) {
                Ok(None) => {}
                Ok(Some(wait)) => {
                    return Err(CarrierQueueRetirementError::Pending {
                        lane,
                        dataspace,
                        incarnation,
                        wait,
                    });
                }
                Err(error) => return Err(CarrierQueueRetirementError::Unavailable(error)),
            }
        }
        Ok(Self {
            state_owner: Arc::clone(&target.block_hashes.owner),
            header,
            routes,
            _cut: cut,
        })
    }

    /// A sticky recovery fault may latch independently of the retained mutation locks.
    pub(in crate::state) fn ensure_available(&self) -> Result<(), CarrierQueueRetirementError> {
        if self._cut.durability_faulted() {
            Err(CarrierQueueRetirementError::Unavailable(
                QueueLaneRetirementUnavailable::DurabilityFault,
            ))
        } else {
            Ok(())
        }
    }

    /// Check the retained exact route set without reacquiring locks or allocating.
    pub(in crate::state) fn authenticates(
        &self,
        target: &State,
        geometry: &PreparedCarrierGeometry,
        header: BlockHeader,
    ) -> bool {
        if self.ensure_available().is_err()
            || !Arc::ptr_eq(&self.state_owner, &target.block_hashes.owner)
            || self.header != header
            || !geometry.matches_publication_target(target, header)
        {
            return false;
        }
        let mut index = 0;
        let result = geometry.for_each_retirement_route(|lane, dataspace, incarnation| {
            if self.routes.get(index) != Some(&(lane, dataspace, incarnation)) {
                return Err(LaneLifecycleError::Storage(
                    "retained Queue cut differs from captured retiring routes".to_owned(),
                ));
            }
            index += 1;
            Ok(())
        });
        result.is_ok() && index == self.routes.len()
    }
}
