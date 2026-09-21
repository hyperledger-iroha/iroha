//! One consuming sequence for 32 value chunks and its admitted original tail.
use super::super::super::source_algebra::PreparedPlaneOpeningTailV1;
use super::*;

struct PreparedPlaneOpeningLiveV1 {
    ordinal: u16,
    values: Option<PreparedRadixValuesV1>,
    tail: Option<PreparedPlaneOpeningTailV1>,
}

/// Keeps exact values paired with their original opening until tail emission.
#[must_use = "all 33 canonical slots must be consumed before source handoff"]
pub(in super::super) struct PreparedPlaneOpeningV1 {
    live: Option<PreparedPlaneOpeningLiveV1>,
}

impl PreparedPlaneOpeningV1 {
    pub(in super::super) fn from_committed_v1(
        values: PreparedRadixValuesV1,
        tail: PreparedPlaneOpeningTailV1,
        ordinal: u16,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        tail.require_ordinal_v1(ordinal)?;
        let value_state = values
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if value_state.next_chunk != 0
            || value_state.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self {
            live: Some(PreparedPlaneOpeningLiveV1 {
                ordinal,
                values: Some(values),
                tail: Some(tail),
            }),
        })
    }

    pub(in super::super) fn emit_next_value_chunk_v1(
        &mut self,
        expected_chunk: u8,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.tail.is_none() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let chunk = live
            .values
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .emit_next_v1(expected_chunk)?;
        self.live = Some(live);
        Ok(chunk)
    }

    pub(in super::super) fn emit_tail_v1(
        &mut self,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        // This consumes and erases all 16,384 scalars before allocating the tail
        // chunk. Early/repeated emission poisons both the values and their rho.
        live.values
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .finish_v1()?;
        let tail = live
            .tail
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .into_chunk_v1(live.ordinal)?;
        self.live = Some(live);
        Ok(tail)
    }

    // The writer is lent only by the surrounding original materialized source.
    // Its cursor is checked before extracting the first zeroizing chunk.
    pub(in super::super) fn store_v1(
        &mut self,
        writer: &mut crate::vega::zk_ams::mkhe::global_lookup_statement_v1::OrderedPlaneSpoolWriterV1,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.ordinal != ordinal {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let first = u64::from(ordinal) * 33;
        writer
            .require_next_slot_v1(first)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        for index in 0..32 {
            let chunk = live
                .values
                .as_mut()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
                .emit_next_v1(index)?;
            writer
                .write_slot_v1(first + u64::from(index), chunk)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        }
        live.values
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .finish_v1()?;
        let tail = live
            .tail
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .into_chunk_v1(ordinal)?;
        writer
            .write_slot_v1(first + 32, tail)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        writer
            .require_next_slot_v1(first + 33)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.live = Some(live);
        Ok(())
    }

    pub(in super::super) fn finish_v1(mut self) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.values.is_some() || live.tail.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "prepared_plane_opening_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prepared_plane_storage_v1_tests.rs"]
mod storage_tests;
