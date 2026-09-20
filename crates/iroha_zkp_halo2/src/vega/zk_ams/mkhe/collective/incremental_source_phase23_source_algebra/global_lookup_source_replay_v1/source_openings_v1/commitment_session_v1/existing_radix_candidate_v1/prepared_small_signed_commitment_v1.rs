//! Original-session admission of signed sources and their negative magnitudes.
//!
//! This child consumes the exact completed beta/m owner and retains all earlier
//! openings. The positive point is derived from the two admitted points; it has
//! neither a separate inventory slot nor freshly sampled randomness.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedSmallSignedStatementV1;

const SIGNED_FIRST_PLANE_V1: u16 = 7_224;
const NEGATIVE_FIRST_PLANE_V1: u16 = 8_256;
const SIGNED_AFTER_PLANE_V1: u16 = 9_288;
const SIGNED_PLANE_COUNT_V1: usize = 2_064;
const SIGNED_FIRST_INVENTORY_V1: u32 = 25_112;
const SIGNED_AFTER_INVENTORY_V1: u32 = 27_176;
const SIGNED_RETAINED_BLINDING_BYTES_V1: usize = SIGNED_PLANE_COUNT_V1 * 32;
const _: () = {
    assert!(SIGNED_PLANE_COUNT_V1 == 2 * 43 * 3 * 8);
    assert!(SIGNED_RETAINED_BLINDING_BYTES_V1 == 66_048);
    assert!(SIGNED_RETAINED_BLINDING_BYTES_V1 as u64 <= INVENTORY_BLINDING_BYTES_V1);
};

fn signed_commitment_coordinate_v1(
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    if !(SIGNED_FIRST_PLANE_V1..SIGNED_AFTER_PLANE_V1).contains(&ordinal) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    let (purpose, first) = if ordinal < NEGATIVE_FIRST_PLANE_V1 {
        (
            GlobalLookupCommitmentPurposeV1::SmallSigned,
            SIGNED_FIRST_PLANE_V1,
        )
    } else {
        (
            GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude,
            NEGATIVE_FIRST_PLANE_V1,
        )
    };
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != purpose
        || coordinate.purpose_ordinal != u32::from(ordinal - first)
        || coordinate.global_ordinal
            != SIGNED_FIRST_INVENTORY_V1 + u32::from(ordinal - SIGNED_FIRST_PLANE_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

struct SmallSignedCommitmentsLiveV1<R> {
    continuation: ComparatorContinuationLiveV1<R>,
    blindings: ZeroizingT256ScalarVecV1,
    next_plane: u16,
}

/// Original proof session with all prior, signed and negative-magnitude openings.
#[must_use = "dropping signed commitments closes the sole original session and all masks"]
pub(in super::super::super::super::super) struct RnsNativeSmallSignedCommitmentsV1<R> {
    live: Option<SmallSignedCommitmentsLiveV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeSmallSignedCommitmentsV1<R> {
    pub(in super::super::super::super::super) fn validate_start_v1(
        owner: &RnsNativeComparatorContinuationV1<R>,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if ordinal != SIGNED_FIRST_PLANE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let live = owner
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_continuation_progress_v1(live)?;
        if live.next_plane != CONTINUATION_AFTER_PLANE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let session = live
            .difference
            .top
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if session
            .inventory
            .slots
            .get(SIGNED_FIRST_INVENTORY_V1 as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in super::super::super::super::super) fn begin_v1(
        mut owner: RnsNativeComparatorContinuationV1<R>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::validate_start_v1(&owner, SIGNED_FIRST_PLANE_V1)?;
        let continuation = owner
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let blindings = ZeroizingT256ScalarVecV1::try_with_exact_capacity(SIGNED_PLANE_COUNT_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self {
            live: Some(SmallSignedCommitmentsLiveV1 {
                continuation,
                blindings,
                next_plane: SIGNED_FIRST_PLANE_V1,
            }),
        })
    }

    pub(in super::super::super::super::super) fn require_position_v1(
        &self,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        require_small_signed_position_v1(live, ordinal).map(|_| ())
    }

    pub(in super::super::super::super::super) fn require_complete_v1(
        &self,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_small_signed_progress_v1(live)?;
        if live.next_plane != SIGNED_AFTER_PLANE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in super::super::super::super::super) fn validate_completed_source_prefix_v1(
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
        validate_small_signed_progress_v1(live)?;
        live.continuation
            .difference
            .top
            .session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }

    pub(in super::super::super::super::super) fn commit_prepared_v1(
        mut self,
        statement: &PreparedSmallSignedStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Take the sole owner before ordinal, entropy, MSM and derived-point
        // checks. Failure or unwind cannot return a usable earlier stage.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        statement.require_ordinal_v1(live.next_plane)?;
        let coordinate = require_small_signed_position_v1(&live, live.next_plane)?;
        let session = live
            .continuation
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
        if coordinate.purpose == GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude {
            let signed_coordinate = signed_commitment_coordinate_v1(
                SIGNED_FIRST_PLANE_V1
                    + u16::try_from(coordinate.purpose_ordinal)
                        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?;
            let signed = session
                .inventory
                .slots
                .get(signed_coordinate.global_ordinal as usize)
                .and_then(Option::as_ref)
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            if signed.coordinate != signed_coordinate {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            require_nonidentity_derived_positive_v1(&signed.point_wire, commitment.expose_ref())?;
        }
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
        validate_small_signed_progress_v1(&live)?;
        self.live = Some(live);
        Ok(self)
    }
}

#[cfg(test)]
impl RnsNativeSmallSignedCommitmentsV1<core::convert::Infallible> {
    /// Synthetic earlier inventory for testing the real retained dispatch only.
    pub(in super::super::super::super::super) fn test_completed_continuation_v1()
    -> RnsNativeComparatorContinuationV1<core::convert::Infallible> {
        tests::continuation_fixture_before_v1(CONTINUATION_AFTER_PLANE_V1)
    }
}

fn require_nonidentity_derived_positive_v1(
    signed_wire: &[u8; 33],
    negative: &Point,
) -> Result<(), ZkAmsMkheErrorV1> {
    let signed = Point::from_non_identity_wire_bytes_exact(signed_wire)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    // Its opening is x+n with rho_x+rho_n. A rare identity outcome fails the
    // whole owner; no replacement rho or independently supplied Cplus exists.
    if (signed + *negative).is_identity() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_small_signed_progress_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &SmallSignedCommitmentsLiveV1<R>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if !(SIGNED_FIRST_PLANE_V1..=SIGNED_AFTER_PLANE_V1).contains(&live.next_plane)
        || live.blindings.len() != usize::from(live.next_plane - SIGNED_FIRST_PLANE_V1)
        || live.continuation.next_plane != CONTINUATION_AFTER_PLANE_V1
        || live.continuation.blindings.len() != CONTINUATION_PLANE_COUNT_V1
        || live.continuation.difference.next_plane != DIFFERENCE_PLANE_COUNT_V1
        || live.continuation.difference.blindings.len() != DIFFERENCE_PLANE_COUNT_V1 as usize
        || live.continuation.difference.top.next_plane != TOP_PLANE_COUNT_V1
        || live.continuation.difference.top.blindings.len() != TOP_PLANE_COUNT_V1 as usize
        || live
            .continuation
            .difference
            .top
            .retained_low
            .blindings
            .len()
            != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    validate_existing_radix_source_axes_v1(session)?;
    let expected = if live.next_plane < SIGNED_AFTER_PLANE_V1 {
        signed_commitment_coordinate_v1(live.next_plane)?
    } else {
        let next = commitment_coordinate_v1(SIGNED_AFTER_INVENTORY_V1)?;
        if next.purpose != GlobalLookupCommitmentPurposeV1::QMaskDigit || next.purpose_ordinal != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        next
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

fn require_small_signed_position_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &SmallSignedCommitmentsLiveV1<R>,
    ordinal: u16,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let coordinate = signed_commitment_coordinate_v1(ordinal)?;
    validate_small_signed_progress_v1(live)?;
    let session = live
        .continuation
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

// TODO: consume this completed original owner into Q-mask construction and the
// shared stored-opening writer. Sign/range proofs and composite admission remain open.
#[cfg(test)]
#[path = "prepared_small_signed_commitment_v1_tests.rs"]
mod tests;
