//! Retain one clearing FRI input per lane across commitment and query opening.

use super::*;

/// The only retained originals; consumed lanes leave empty, clearing slots.
pub(super) struct MainRetainedFriInputsV1 {
    lanes: ZeroizingExtensionChunksV1,
    rows: usize,
}

impl MainRetainedFriInputsV1 {
    /// Adopt before validating so malformed shapes also erase initialized cells.
    pub(super) fn new_v1(lanes: Vec<Vec<E>>, rows: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let lanes = ZeroizingExtensionChunksV1::new(lanes, zeroize_extension_chunks_v1);
        if rows == 0
            || !rows.is_power_of_two()
            || lanes.len() != SECURITY_LANES
            || lanes.iter().any(|lane| lane.len() != rows)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self { lanes, rows })
    }

    /// Add the existing independently committed masks to the original inputs.
    pub(super) fn lanes_mut_v1(&mut self) -> impl Iterator<Item = &mut Vec<E>> {
        self.lanes.iter_mut()
    }

    /// Count every retained vector capacity, including pointer-bearing slots.
    pub(super) fn allocated_payload_bytes_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        self.lanes.iter().try_fold(
            self.lanes
                .capacity()
                .checked_mul(core::mem::size_of::<Vec<E>>())
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            |bytes, lane| {
                lane.capacity()
                    .checked_mul(core::mem::size_of::<E>())
                    .and_then(|payload| bytes.checked_add(payload))
                    .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
            },
        )
    }

    /// Allocate under a forecast, recheck actual capacity before copying, and
    /// keep the outgoing copy clearing-owned until the FRI builder adopts it.
    pub(super) fn copy_lane_v1(
        &self,
        lane: usize,
        mut admit: impl FnMut(usize) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<ZeroizingExtensionColumnV1, ZkX509StarkErrorV1> {
        let original = self
            .lanes
            .get(lane)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if original.len() != self.rows {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        admit(self.rows)?;
        let mut copy = ZeroizingExtensionColumnV1(Vec::new());
        copy.0
            .try_reserve_exact(self.rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        admit(copy.0.capacity())?;
        copy.0.extend_from_slice(original);
        Ok(copy)
    }

    /// Transfer exactly once into the already clearing streaming-opening owner.
    pub(super) fn take_lane_v1(
        &mut self,
        lane: usize,
    ) -> Result<ZeroizingExtensionColumnV1, ZkX509StarkErrorV1> {
        let original = self
            .lanes
            .get_mut(lane)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if original.len() != self.rows {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(ZeroizingExtensionColumnV1(core::mem::take(original)))
    }
}

/// The already retained masks keep their actual evaluation and owner capacities.
pub(super) fn mask_evaluation_payload_v1(
    masks: &Vec<aggregate::AggregateFriMaskOracleMaterialV1>,
) -> Result<usize, ZkX509StarkErrorV1> {
    if masks.len() != SECURITY_LANES {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    masks.iter().try_fold(
        masks
            .capacity()
            .checked_mul(core::mem::size_of::<
                aggregate::AggregateFriMaskOracleMaterialV1,
            >())
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
        |bytes, mask| {
            mask.evaluations
                .capacity()
                .checked_mul(core::mem::size_of::<E>())
                .and_then(|payload| bytes.checked_add(payload))
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
        },
    )
}

#[cfg(test)]
#[path = "main_fri_retention_tests.rs"]
mod tests;
