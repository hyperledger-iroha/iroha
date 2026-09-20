//! Original-session beta/m admission after all source-bound delta commitments.
//!
//! Sealed comparator preparation resumes at logical plane 688 only after the
//! exact delta-complete stage. All prior openings and the original entropy move
//! together while physical slots 18,576..25,112 receive actual commitments.
use super::*;

const CONTINUATION_FIRST_PLANE_V1: u16 = 688;
const CONTINUATION_AFTER_PLANE_V1: u16 = 7_224;
const CONTINUATION_PLANE_COUNT_V1: usize = 6_536;
const CONTINUATION_AFTER_INVENTORY_V1: u32 = 25_112;
const CONTINUATION_RETAINED_BLINDING_BYTES_V1: usize = CONTINUATION_PLANE_COUNT_V1 * 32;
const _: () = {
    assert!(CONTINUATION_PLANE_COUNT_V1 == EXISTING_RADIX_CANDIDATE_GROUPS_V1 * 19);
    assert!(CONTINUATION_RETAINED_BLINDING_BYTES_V1 == 209_152);
    assert!(CONTINUATION_RETAINED_BLINDING_BYTES_V1 as u64 <= INVENTORY_BLINDING_BYTES_V1);
};

fn continuation_coordinate_v1(
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    if !(CONTINUATION_FIRST_PLANE_V1..CONTINUATION_AFTER_PLANE_V1).contains(&ordinal) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    let (purpose, purpose_ordinal) = if ordinal < 6_880 {
        (
            GlobalLookupCommitmentPurposeV1::ComparatorBorrow,
            u32::from(ordinal - CONTINUATION_FIRST_PLANE_V1),
        )
    } else {
        (
            GlobalLookupCommitmentPurposeV1::ComparatorMixedTop,
            u32::from(ordinal - 6_880),
        )
    };
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != purpose
        || coordinate.purpose_ordinal != purpose_ordinal
        || coordinate.global_ordinal
            != DIFFERENCE_AFTER_ORDINAL_V1 + u32::from(ordinal - CONTINUATION_FIRST_PLANE_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

struct ComparatorContinuationLiveV1<R> {
    difference: DifferenceCommitmentsLiveV1<R>,
    blindings: ZeroizingT256ScalarVecV1,
    next_plane: u16,
}

/// Sole session with retained original source, D/S, delta and comparator openings.
#[must_use = "dropping comparator continuation closes the original session and all blindings"]
pub(in super::super::super::super) struct RnsNativeComparatorContinuationV1<R> {
    live: Option<ComparatorContinuationLiveV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeComparatorContinuationV1<R> {
    pub(in super::super::super::super) fn validate_start_v1(
        owner: &RnsNativeDifferenceCommitmentsV1<R>,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if ordinal != CONTINUATION_FIRST_PLANE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        owner.require_complete_v1()?;
        let live = owner
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let session = live
            .top
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = continuation_coordinate_v1(ordinal)?;
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

    pub(in super::super::super::super) fn begin_v1(
        mut owner: RnsNativeDifferenceCommitmentsV1<R>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::validate_start_v1(&owner, CONTINUATION_FIRST_PLANE_V1)?;
        let difference = owner
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let blindings =
            ZeroizingT256ScalarVecV1::try_with_exact_capacity(CONTINUATION_PLANE_COUNT_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self {
            live: Some(ComparatorContinuationLiveV1 {
                difference,
                blindings,
                next_plane: CONTINUATION_FIRST_PLANE_V1,
            }),
        })
    }

    pub(in super::super::super::super) fn require_position_v1(
        &self,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        require_continuation_position_v1(live, ordinal).map(|_| ())
    }

    pub(in super::super::super::super) fn validate_completed_source_prefix_v1(
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
        validate_continuation_progress_v1(live)?;
        live.difference
            .top
            .session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }

    pub(in super::super::super::super) fn commit_prepared_v1(
        mut self,
        statement: &PreparedComparatorStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Any validation, entropy, MSM or adoption failure consumes all prior
        // openings. No point, scalar, replacement session or ordinal is supplied.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        statement.require_ordinal_v1(live.next_plane)?;
        let coordinate = require_continuation_position_v1(&live, live.next_plane)?;
        let session = live
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
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
        validate_continuation_progress_v1(&live)?;
        self.live = Some(live);
        Ok(self)
    }
}

fn validate_continuation_progress_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &ComparatorContinuationLiveV1<R>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if !(CONTINUATION_FIRST_PLANE_V1..=CONTINUATION_AFTER_PLANE_V1).contains(&live.next_plane)
        || live.blindings.len() != usize::from(live.next_plane - CONTINUATION_FIRST_PLANE_V1)
        || live.difference.next_plane != DIFFERENCE_PLANE_COUNT_V1
        || live.difference.blindings.len() != DIFFERENCE_PLANE_COUNT_V1 as usize
        || live.difference.top.next_plane != TOP_PLANE_COUNT_V1
        || live.difference.top.blindings.len() != TOP_PLANE_COUNT_V1 as usize
        || live.difference.top.retained_low.blindings.len()
            != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let session = live
        .difference
        .top
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    validate_existing_radix_source_axes_v1(session)?;
    let expected = if live.next_plane < CONTINUATION_AFTER_PLANE_V1 {
        continuation_coordinate_v1(live.next_plane)?
    } else {
        let coordinate = commitment_coordinate_v1(CONTINUATION_AFTER_INVENTORY_V1)?;
        if coordinate.purpose != GlobalLookupCommitmentPurposeV1::SmallSigned
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

fn require_continuation_position_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &ComparatorContinuationLiveV1<R>,
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let coordinate = continuation_coordinate_v1(ordinal)?;
    validate_continuation_progress_v1(live)?;
    let session = live
        .difference
        .top
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if live.next_plane != ordinal
        || session
            .inventory
            .slots
            .get(coordinate.global_ordinal as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .is_some()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

#[path = "prepared_small_signed_commitment_v1.rs"]
mod prepared_small_signed_commitment_v1;
pub(in super::super::super::super) use prepared_small_signed_commitment_v1::RnsNativeSmallSignedCommitmentsV1;

// TODO: join actual value chunks to the retained-opening tail writer. The
// signed child continues this owner without minting composite proof authority.
#[cfg(test)]
#[path = "prepared_comparator_continuation_v1_tests.rs"]
mod tests;
