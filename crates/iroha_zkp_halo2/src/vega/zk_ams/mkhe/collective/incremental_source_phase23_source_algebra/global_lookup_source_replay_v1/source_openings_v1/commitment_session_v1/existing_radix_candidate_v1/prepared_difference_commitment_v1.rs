//! Original-session adoption of source-bound centering-difference commitments.
//!
//! This child consumes the exact complete top owner. It preserves the original
//! source/D/S/top openings and entropy, and fills only the 5,848 delta slots.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedDifferenceDigitStatementV1;

const DIFFERENCE_PLANE_COUNT_V1: u16 = 5_848;
const DIFFERENCE_FIRST_ORDINAL_V1: u32 = 12_728;
const DIFFERENCE_AFTER_ORDINAL_V1: u32 = 18_576;
const DIFFERENCE_RETAINED_BLINDING_BYTES_V1: usize = DIFFERENCE_PLANE_COUNT_V1 as usize * 32;
const _: () = {
    assert!(DIFFERENCE_PLANE_COUNT_V1 as usize == EXISTING_RADIX_CANDIDATE_GROUPS_V1 * 17);
    assert!(DIFFERENCE_RETAINED_BLINDING_BYTES_V1 == 187_136);
    assert!(DIFFERENCE_RETAINED_BLINDING_BYTES_V1 as u64 <= INVENTORY_BLINDING_BYTES_V1);
};

fn difference_coordinate_v1(
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    if ordinal >= DIFFERENCE_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let first =
        first_commitment_ordinal_v1(GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit);
    let coordinate = commitment_coordinate_v1(first + u32::from(ordinal))?;
    if first != DIFFERENCE_FIRST_ORDINAL_V1
        || coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit
        || coordinate.purpose_ordinal != u32::from(ordinal)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

struct DifferenceCommitmentsLiveV1<R> {
    top: ComparatorTopLiveV1<R>,
    blindings: ZeroizingT256ScalarVecV1,
    next_plane: u16,
}

/// Sole original session, retained prior openings and actual delta openings.
#[must_use = "dropping delta commitments closes the original session and all blindings"]
pub(in super::super::super) struct RnsNativeDifferenceCommitmentsV1<R> {
    live: Option<DifferenceCommitmentsLiveV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeDifferenceCommitmentsV1<R> {
    pub(in super::super::super) fn validate_start_v1(
        owner: &RnsNativeComparatorTopCommitmentsV1<R>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let top = owner
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_top_progress_v1(top)?;
        if top.next_plane != TOP_PLANE_COUNT_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let session = top
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = difference_coordinate_v1(0)?;
        if session
            .inventory
            .slots
            .get(coordinate.global_ordinal as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in super::super::super) fn begin_v1(
        mut owner: RnsNativeComparatorTopCommitmentsV1<R>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::validate_start_v1(&owner)?;
        let top = owner
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let blindings =
            ZeroizingT256ScalarVecV1::try_with_exact_capacity(DIFFERENCE_PLANE_COUNT_V1 as usize)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self {
            live: Some(DifferenceCommitmentsLiveV1 {
                top,
                blindings,
                next_plane: 0,
            }),
        })
    }

    pub(in super::super::super) fn require_complete_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_difference_progress_v1(live)?;
        if live.next_plane != DIFFERENCE_PLANE_COUNT_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in super::super::super) fn validate_completed_source_prefix_v1(
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
        validate_difference_progress_v1(live)?;
        live.top
            .session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }

    pub(in super::super::super) fn commit_prepared_v1(
        mut self,
        statement: &PreparedDifferenceDigitStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        statement.require_ordinal_v1(live.next_plane)?;
        validate_difference_progress_v1(&live)?;
        let coordinate = difference_coordinate_v1(live.next_plane)?;
        let session = live
            .top
            .session
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let slot = session
            .inventory
            .slots
            .get(coordinate.global_ordinal as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if slot.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let (_chunk, scalar) = sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal)?;
        let commitment = statement.commitment_v1(scalar.as_ref())?;
        let point_wire = commitment
            .expose_ref()
            .to_non_identity_wire_bytes()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        session.inventory.slots[coordinate.global_ordinal as usize] =
            Some(GlobalLookupCommitmentTicketV1 {
                coordinate,
                point_wire,
            });
        live.blindings.push(scalar.get());
        live.next_plane = live
            .next_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let next = commitment_coordinate_v1(coordinate.global_ordinal + 1)?;
        session.next_global_ordinal = next.global_ordinal;
        session.next_purpose = next.purpose;
        session.next_purpose_ordinal = next.purpose_ordinal;
        validate_difference_progress_v1(&live)?;
        self.live = Some(live);
        Ok(self)
    }
}

fn validate_difference_progress_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &DifferenceCommitmentsLiveV1<R>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if live.next_plane > DIFFERENCE_PLANE_COUNT_V1
        || live.blindings.len() != usize::from(live.next_plane)
        || live.top.next_plane != TOP_PLANE_COUNT_V1
        || live.top.blindings.len() != TOP_PLANE_COUNT_V1 as usize
        || live.top.retained_low.blindings.len() != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let session = live
        .top
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    validate_existing_radix_source_axes_v1(session)?;
    let expected = if live.next_plane < DIFFERENCE_PLANE_COUNT_V1 {
        difference_coordinate_v1(live.next_plane)?
    } else {
        let coordinate = commitment_coordinate_v1(DIFFERENCE_AFTER_ORDINAL_V1)?;
        if coordinate.purpose != GlobalLookupCommitmentPurposeV1::ComparatorBorrow
            || coordinate.purpose_ordinal != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        coordinate
    };
    if session.pending_source.is_some()
        || session.next_global_ordinal != expected.global_ordinal
        || session.next_purpose != expected.purpose
        || session.next_purpose_ordinal != expected.purpose_ordinal
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

#[cfg(test)]
#[path = "prepared_difference_commitment_v1_tests.rs"]
mod tests;

#[path = "prepared_comparator_continuation_v1.rs"]
mod prepared_comparator_continuation_v1;
pub(in super::super::super) use prepared_comparator_continuation_v1::{
    RnsNativeComparatorContinuationV1, RnsNativeSmallSignedCommitmentsV1,
    RnsNativeStoredPlaneReplayV1,
};
