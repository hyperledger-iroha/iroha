//! Actual prepared D/S values drive the original session's sampled commitment.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedLowDigitStatementV1;

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeExistingRadixCandidateAssemblyV1<R> {
    pub(in super::super) fn commit_prepared_values_v1(
        mut self,
        statement: &PreparedLowDigitStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        statement.require_ordinal_v1(live.next_wire_ordinal)?;
        let coordinate = existing_radix_candidate_coordinate_v1(live.next_wire_ordinal)?;
        let blinding = self.sample_next_blinding_v1(
            usize::from(coordinate.group),
            coordinate.role,
            usize::from(coordinate.column),
        )?;
        // The point comes only from these exact privately prepared values and
        // this session's sampled, purpose-bound token. No point is an input.
        let commitment = statement.commitment_v1(blinding.scalar_v1())?;
        self.adopt_next_commitment_v1(blinding, commitment.expose_ref())?;
        Ok(self)
    }

    pub(in super::super) fn all_prepared_values_committed_v1(
        &self,
    ) -> Result<bool, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.pending.is_some()
            || live.next_wire_ordinal > EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32
            || live.blindings.len() != live.next_wire_ordinal as usize
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(live.next_wire_ordinal == EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32)
    }

    pub(in super::super) fn validate_completed_source_prefix_v1(
        &self,
        record: [u8; 32],
        context: [u8; 32],
        points: [u8; 32],
        blindings: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeExistingRadixCandidateOwnerV1<R> {
    pub(in super::super) fn validate_completed_source_prefix_v1(
        &self,
        record: [u8; 32],
        context: [u8; 32],
        points: [u8; 32],
        blindings: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }
}

#[cfg(test)]
#[path = "prepared_commitment_v1_tests.rs"]
mod tests;
