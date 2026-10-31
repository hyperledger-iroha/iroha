//! Immutable privacy projections physically owned by the original execution pool.

use std::{alloc::Layout, fmt};

use iroha_allocation::{
    AllocationBudget, AllocationRefusal, AllocatorRefusal, ChargedBuffer, ChargedBufferError,
    ChargedShared, PrepaidBufferError, PrepaidSharedError,
};
use iroha_crypto::privacy::{LaneCommitmentId, LanePrivacyCommitment, PrivacyWitness};
use iroha_model_base::topology::{DataSpaceId, LaneId};

use super::LanePrivacyRegistryError;
use crate::{
    execution_attempt::ExecutionDeferred,
    governance::manifest::{LaneManifestRegistry, LaneManifestStatus},
};

/// Immutable privacy registry retaining every physical allocation through its last reader.
///
/// An empty registry allocates nothing. Populated registries share their original
/// pool-owned shell and buffers; cloning never copies or reallocates the graph.
#[derive(Clone, Default)]
pub struct LanePrivacyRegistry {
    owner: Option<ChargedShared<PrivacyEntries>>,
}

struct PrivacyEntries {
    lanes: ChargedBuffer<LanePrivacyLane>,
}

/// Per-lane commitments borrowed from an immutable admitted registry.
pub struct LanePrivacyLane {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    commitments: ChargedBuffer<LanePrivacyCommitment>,
}

fn buffer_error(error: ChargedBufferError) -> ExecutionDeferred {
    match error {
        ChargedBufferError::Admission(original) => original.into(),
        original @ ChargedBufferError::Allocator { .. } => AllocatorRefusal::try_from(original)
            .expect("matched original allocator failure")
            .into(),
    }
}

fn prepaid_buffer_error(error: PrepaidBufferError) -> ExecutionDeferred {
    match error {
        PrepaidBufferError::Allocation(original) => buffer_error(original),
        // The complete checked layout sum owns this reservation. A shortage is
        // a local construction invariant, never a codec error or protocol cap.
        PrepaidBufferError::Reservation(_) => {
            ivm::error::ExecutionDeferral::LocalInvariantViolation.into()
        }
    }
}

fn shared_error(error: PrepaidSharedError) -> ExecutionDeferred {
    match error {
        original @ PrepaidSharedError::Allocator { .. } => AllocatorRefusal::try_from(original)
            .expect("matched original shared allocator failure")
            .into(),
        PrepaidSharedError::Reservation(_) => {
            ivm::error::ExecutionDeferral::LocalInvariantViolation.into()
        }
    }
}

fn array_layout<T>(capacity: usize) -> Result<Layout, ExecutionDeferred> {
    Layout::array::<T>(capacity).map_err(|_| AllocationRefusal::DemandOverflow.into())
}

impl LanePrivacyRegistry {
    /// Construct an empty registry without allocating a shell or selecting a pool.
    pub const fn empty() -> Self {
        Self { owner: None }
    }

    /// Whether no lane advertises privacy commitments.
    pub fn is_empty(&self) -> bool {
        self.owner.is_none()
    }

    /// Build one immutable projection from borrowed statuses and the original pool.
    ///
    /// Empty statuses are skipped. The last nonempty status for a lane wins;
    /// within it, the last commitment with a given ID wins. Lookup and iteration
    /// retain ascending ID order. All temporary and retained buffers are exact
    /// fallible physical allocations; source manifest graphs remain borrowed.
    ///
    /// # Errors
    /// Returns the original local allocation refusal before publishing any owner.
    /// Callers holding physical guards must defer this pool's refund callbacks
    /// through the complete guard lifetime, including failed partial construction.
    pub fn try_from_statuses<'a, I>(
        statuses: I,
        budget: &AllocationBudget,
    ) -> Result<Self, ExecutionDeferred>
    where
        I: IntoIterator<Item = &'a LaneManifestStatus>,
        I::IntoIter: Clone,
    {
        let statuses = statuses.into_iter();
        let count = statuses.clone().try_fold(0_usize, |count, status| {
            if status.privacy_commitments.is_empty() {
                Ok(count)
            } else {
                count
                    .checked_add(1)
                    .ok_or(AllocationRefusal::DemandOverflow)
            }
        })?;
        if count == 0 {
            return Ok(Self::empty());
        }
        let mut selected = ChargedBuffer::new(count, budget).map_err(buffer_error)?;
        for (ordinal, status) in statuses.enumerate() {
            if !status.privacy_commitments.is_empty() {
                selected.push_reserved((ordinal, status));
            }
        }
        selected
            .as_mut_slice()
            .sort_unstable_by_key(|(ordinal, status)| (status.lane, *ordinal));
        // Compact only Copy borrowed pointers, retaining the original scratch
        // capacity charge until this exact allocation is dropped.
        let mut unique = 0;
        for index in 0..count {
            let is_last = index + 1 == count
                || selected.as_slice()[index].1.lane != selected.as_slice()[index + 1].1.lane;
            if is_last {
                let winner = selected.as_slice()[index];
                selected.as_mut_slice()[unique] = winner;
                unique += 1;
            }
        }
        selected.truncate(unique);
        let mut bytes = ChargedShared::<PrivacyEntries>::allocation_layout().size();
        bytes = bytes
            .checked_add(array_layout::<LanePrivacyLane>(unique)?.size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        for (_, status) in selected.as_slice() {
            bytes = bytes
                .checked_add(
                    array_layout::<LanePrivacyCommitment>(status.privacy_commitments.len())?.size(),
                )
                .ok_or(AllocationRefusal::DemandOverflow)?;
        }
        let mut reservation = budget.try_reserve_bytes(bytes)?;
        let shell = ChargedShared::<PrivacyEntries>::reserve_from(&mut reservation)
            .map_err(shared_error)?;
        let mut lanes = ChargedBuffer::from_reservation(unique, &mut reservation)
            .map_err(prepaid_buffer_error)?;
        for (_, status) in selected.as_slice() {
            let mut commitments =
                ChargedBuffer::from_reservation(status.privacy_commitments.len(), &mut reservation)
                    .map_err(prepaid_buffer_error)?;
            for commitment in &status.privacy_commitments {
                match commitments
                    .as_slice()
                    .binary_search_by_key(&commitment.id(), LanePrivacyCommitment::id)
                {
                    Ok(index) => commitments.as_mut_slice()[index] = *commitment,
                    Err(index) => {
                        commitments.push_reserved(*commitment);
                        commitments.as_mut_slice()[index..].rotate_right(1);
                    }
                }
            }
            lanes.push_reserved(LanePrivacyLane {
                lane_id: status.lane,
                dataspace_id: status.dataspace,
                commitments,
            });
        }
        Ok(Self {
            owner: Some(shell.initialize(PrivacyEntries { lanes })),
        })
    }

    /// Derive an admitted projection from this exact immutable manifest registry.
    ///
    /// # Errors
    /// Returns the original local allocation refusal, preserving its source owner.
    pub fn try_from_manifest_registry(
        registry: &LaneManifestRegistry,
        budget: &AllocationBudget,
    ) -> Result<Self, ExecutionDeferred> {
        Self::try_from_statuses(registry.iter_statuses(), budget)
    }

    /// Whether every allocation belongs to this original pool; empty owns no allocation.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        // Private construction splits every child from the shell's same original
        // reservation; no API can replace or detach those children. The shell is
        // therefore a constant-time proof for the entire closed graph.
        self.owner
            .as_ref()
            .is_none_or(|owner| owner.belongs_to(budget))
    }

    /// Whether both values share exactly the same original graph or are both empty.
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (&left.owner, &right.owner) {
            (None, None) => true,
            (Some(left), Some(right)) => ChargedShared::ptr_eq(left, right),
            _ => false,
        }
    }

    fn lanes(&self) -> &[LanePrivacyLane] {
        self.owner
            .as_ref()
            .map_or(&[], |owner| owner.lanes.as_slice())
    }

    /// Borrow the recorded lane without allocating or detaching its custody.
    pub fn lane(&self, lane_id: LaneId) -> Option<&LanePrivacyLane> {
        let lanes = self.lanes();
        lanes
            .binary_search_by_key(&lane_id, |lane| lane.lane_id)
            .ok()
            .map(|index| &lanes[index])
    }

    /// Verify a witness against this immutable projection.
    ///
    /// # Errors
    /// Returns a missing-lane/commitment or the original cryptographic proof error.
    pub fn verify(
        &self,
        lane_id: LaneId,
        commitment_id: LaneCommitmentId,
        witness: PrivacyWitness,
    ) -> Result<(), LanePrivacyRegistryError> {
        let lane = self
            .lane(lane_id)
            .ok_or(LanePrivacyRegistryError::LaneMissing { lane_id })?;
        let commitment =
            lane.get(commitment_id)
                .ok_or(LanePrivacyRegistryError::UnknownCommitment {
                    lane_id,
                    commitment_id,
                })?;
        commitment
            .verify(witness)
            .map_err(|source| LanePrivacyRegistryError::Verification {
                lane_id,
                commitment_id,
                source,
            })
    }
}

impl LanePrivacyLane {
    /// Lane identifier associated with this entry.
    pub const fn lane_id(&self) -> LaneId {
        self.lane_id
    }
    /// Data space identifier advertised by its exact manifest status.
    pub const fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }
    /// Iterate commitments in ascending ID order without allocation.
    pub fn commitments(&self) -> impl Iterator<Item = LanePrivacyCommitment> + '_ {
        self.commitments.as_slice().iter().copied()
    }
    /// Look up a commitment without allocating or copying its backing owner.
    pub fn get(&self, id: LaneCommitmentId) -> Option<LanePrivacyCommitment> {
        let commitments = self.commitments.as_slice();
        commitments
            .binary_search_by_key(&id, LanePrivacyCommitment::id)
            .ok()
            .map(|index| commitments[index])
    }
    /// Whether no commitment is retained by this lane.
    pub fn is_empty(&self) -> bool {
        self.commitments.as_slice().is_empty()
    }
}

impl PartialEq for LanePrivacyLane {
    fn eq(&self, other: &Self) -> bool {
        self.lane_id == other.lane_id
            && self.dataspace_id == other.dataspace_id
            && self.commitments.as_slice() == other.commitments.as_slice()
    }
}
impl Eq for LanePrivacyLane {}
impl PartialEq for LanePrivacyRegistry {
    fn eq(&self, other: &Self) -> bool {
        Self::ptr_eq(self, other) || self.lanes() == other.lanes()
    }
}
impl Eq for LanePrivacyRegistry {}
impl fmt::Debug for LanePrivacyLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanePrivacyLane")
            .field("lane_id", &self.lane_id)
            .field("dataspace_id", &self.dataspace_id)
            .field("commitments", &self.commitments.as_slice())
            .finish()
    }
}
impl fmt::Debug for LanePrivacyRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanePrivacyRegistry")
            .field("entries", &self.lanes())
            .finish()
    }
}

#[cfg(test)]
mod tests;
