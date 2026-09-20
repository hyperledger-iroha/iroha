//! Original-session commitments for the actual prepared bD/bS value planes.
//!
//! Completed D/S material is consumed once and retained while the same session
//! samples 344 bD and 344 bS blindings. The next delta purpose is mandatory; this
//! component cannot skip it to beta. No external point or entropy is accepted.
use super::*;
use crate::vega::zk_ams::mkhe::{
    collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedComparatorStatementV1,
    global_lookup_statement_v1::{comparator_signed_coordinate_v1, first_commitment_ordinal_v1},
};

const TOP_PLANE_COUNT_V1: u16 = 2 * EXISTING_RADIX_CANDIDATE_GROUPS_V1 as u16;
const TOP_RETAINED_BLINDING_BYTES_V1: usize = TOP_PLANE_COUNT_V1 as usize * 32;
const _: () = {
    assert!(TOP_PLANE_COUNT_V1 == 688);
    assert!(TOP_RETAINED_BLINDING_BYTES_V1 == 22_016);
    assert!(TOP_RETAINED_BLINDING_BYTES_V1 as u64 <= INVENTORY_BLINDING_BYTES_V1);
};

fn top_coordinate_v1(ordinal: u16) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    if ordinal >= TOP_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    let groups = EXISTING_RADIX_CANDIDATE_GROUPS_V1 as u16;
    let purpose = if ordinal < groups {
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop
    } else {
        GlobalLookupCommitmentPurposeV1::ComparatorSumTop
    };
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != purpose
        || coordinate.purpose_ordinal != u32::from(ordinal % groups)
        || coordinate.global_ordinal
            != EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1 + u32::from(ordinal)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

// Move the actual completed material, never reconstruct it from bare roots.
// Later proof/transport owners still need its blindings and one append permit.
struct RetainedLowOpeningMaterialV1 {
    blindings: ZeroizingT256ScalarVecV1,
    candidate_root: [u8; 32],
    blinding_root: [u8; 32],
    owner_binding_digest: [u8; 32],
    append_permit: Option<ExistingRadixCandidateAppendPermitV1>,
}

struct ComparatorTopLiveV1<R> {
    session: GlobalLookupCommitmentSessionV1<R, ExistingRadixCandidateCompleteStageV1>,
    retained_low: RetainedLowOpeningMaterialV1,
    blindings: ZeroizingT256ScalarVecV1,
    next_plane: u16,
}

/// Sole progressing session and actual retained source, D/S and top openings.
#[must_use = "dropping top commitments closes the original session and all blindings"]
pub(in super::super) struct RnsNativeComparatorTopCommitmentsV1<R> {
    live: Option<ComparatorTopLiveV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeComparatorTopCommitmentsV1<R> {
    pub(in super::super) fn validate_start_v1(
        owner: &RnsNativeExistingRadixCandidateOwnerV1<R>,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if ordinal != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        top_coordinate_v1(ordinal)?;
        // Keep the original exact 12040 predicate, never an arbitrary >= cursor.
        owner.validate_v1()
    }

    pub(in super::super) fn begin_v1(
        owner: RnsNativeExistingRadixCandidateOwnerV1<R>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::validate_start_v1(&owner, 0)?;
        let blindings =
            ZeroizingT256ScalarVecV1::try_with_exact_capacity(TOP_PLANE_COUNT_V1 as usize)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let RnsNativeExistingRadixCandidateOwnerV1 {
            session,
            blindings: low_blindings,
            candidate_root,
            blinding_root,
            owner_binding_digest,
            append_permit,
        } = owner;
        Ok(Self {
            live: Some(ComparatorTopLiveV1 {
                session,
                retained_low: RetainedLowOpeningMaterialV1 {
                    blindings: low_blindings,
                    candidate_root,
                    blinding_root,
                    owner_binding_digest,
                    append_permit,
                },
                blindings,
                next_plane: 0,
            }),
        })
    }

    pub(in super::super) fn require_position_v1(
        &self,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        require_top_position_v1(live, ordinal).map(|_| ())
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
        validate_top_progress_v1(live)?;
        live.session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }

    pub(in super::super) fn commit_prepared_v1(
        mut self,
        statement: &PreparedComparatorStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Take before order validation, entropy, secret MSM or inventory mutation.
        // The sampled scalar and computed point never leave this consuming call.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        statement.require_ordinal_v1(live.next_plane)?;
        let coordinate = require_top_position_v1(&live, live.next_plane)?;
        let session = live
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
        let slot = session
            .inventory
            .slots
            .get_mut(coordinate.global_ordinal as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if slot.is_some() || live.blindings.len() >= TOP_PLANE_COUNT_V1 as usize {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        *slot = Some(GlobalLookupCommitmentTicketV1 {
            coordinate,
            point_wire,
        });
        live.blindings.push(scalar.get());
        live.next_plane = live
            .next_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let next = commitment_coordinate_v1(
            coordinate
                .global_ordinal
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        session.next_global_ordinal = next.global_ordinal;
        session.next_purpose = next.purpose;
        session.next_purpose_ordinal = next.purpose_ordinal;
        validate_top_progress_v1(&live)?;
        self.live = Some(live);
        Ok(self)
    }
}

fn validate_top_progress_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &ComparatorTopLiveV1<R>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if live.next_plane > TOP_PLANE_COUNT_V1 || live.blindings.len() != usize::from(live.next_plane)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let session = live
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    validate_existing_radix_source_axes_v1(session)?;
    let expected = if live.next_plane < TOP_PLANE_COUNT_V1 {
        top_coordinate_v1(live.next_plane)?
    } else {
        let ordinal =
            first_commitment_ordinal_v1(GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit);
        let coordinate = commitment_coordinate_v1(ordinal)?;
        if coordinate.purpose != GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit
            || coordinate.purpose_ordinal != 0
            || coordinate.global_ordinal
                != EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1
                    + u32::from(TOP_PLANE_COUNT_V1)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        coordinate
    };
    if session.pending_source.is_some()
        || session.next_global_ordinal != expected.global_ordinal
        || session.next_purpose != expected.purpose
        || session.next_purpose_ordinal != expected.purpose_ordinal
        || live.retained_low.blindings.len() != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn require_top_position_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &ComparatorTopLiveV1<R>,
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let coordinate = top_coordinate_v1(ordinal)?;
    validate_top_progress_v1(live)?;
    let session = live
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

#[cfg(test)]
#[path = "prepared_comparator_commitment_v1_tests.rs"]
mod tests;

#[path = "prepared_difference_commitment_v1.rs"]
mod prepared_difference_commitment_v1;
pub(in super::super) use prepared_difference_commitment_v1::{
    RnsNativeComparatorContinuationV1, RnsNativeDifferenceCommitmentsV1,
    RnsNativeSmallSignedCommitmentsV1,
};
