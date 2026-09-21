//! Private authenticated compact-source access for signed commitment production.
use super::*;
use crate::vega::zk_ams::mkhe::{
    collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedSmallSignedStatementV1,
    global_lookup_statement_v1::{
        GlobalLookupCommitmentPhaseV1, GlobalLookupCommitmentPurposeV1,
        comparator_signed_coordinate_v1,
    },
};

fn signed_source_coordinate_v1(ordinal: u16) -> Result<CompactPlaneCoordinateV1, ZkAmsMkheErrorV1> {
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || !matches!(
            coordinate.purpose,
            GlobalLookupCommitmentPurposeV1::SmallSigned
                | GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude
        )
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    compact_plane_coordinate_v1(coordinate.purpose_ordinal as usize)
}

fn read_signed_compact_slot_v1(
    snapshot: &mut ConfidentialSpoolSnapshotV1,
    ordinal: u16,
    context: [u8; 32],
) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
    let coordinate = signed_source_coordinate_v1(ordinal)?;
    snapshot
        .read_slot_v1(u64::from(coordinate.slot), context)
        .map_err(map_leaf_error_v1)
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P>
    Phase23GlobalLookupSourceReplayEvidenceV1<R, K, P>
{
    /// Read only the next original compact slot after lineage and stage checks.
    /// The consuming radix owner drops this evidence on any error or unwind.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn read_small_signed_plane_v1(
        &mut self,
        replay_record_digest: [u8; 32],
        source_receipt_digest: [u8; 32],
        ordinal: u16,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        self.validate_radix_materialization_source_v1(replay_record_digest, source_receipt_digest)?;
        self.openings.require_small_signed_position_v1(ordinal)?;
        read_signed_compact_slot_v1(
            &mut self.snapshot,
            ordinal,
            self.record.spool_context_digest,
        )
    }

    /// Admit the statement only through the same source's retained original session.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn commit_prepared_small_signed_v1(
        &mut self,
        statement: &PreparedSmallSignedStatementV1<'_>,
    ) -> Result<PreparedPlaneOpeningTailV1, ZkAmsMkheErrorV1> {
        validate_replay_evidence_v1(self)?;
        statement
            .validate_origin_v1(self.record.record_digest, self.record.source_receipt_digest)?;
        self.openings.commit_prepared_small_signed_v1(statement)
    }
}

#[cfg(test)]
#[path = "prepared_small_signed_source_v1_tests.rs"]
mod tests;
