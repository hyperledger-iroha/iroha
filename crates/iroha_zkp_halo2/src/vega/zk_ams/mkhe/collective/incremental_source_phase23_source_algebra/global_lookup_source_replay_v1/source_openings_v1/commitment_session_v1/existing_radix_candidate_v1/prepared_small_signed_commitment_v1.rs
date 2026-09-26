//! Original-session admission of signed sources and their negative magnitudes.
//!
//! This child consumes the exact completed beta/m owner and retains all earlier
//! openings. The positive point is derived from the two admitted points; it has
//! neither a separate inventory slot nor freshly sampled randomness.
use super::*;
use crate::vega::zk_ams::mkhe::rns_native_u15_msm::{
    RnsNativeU15MsmErrorV1, RnsNativeU15MsmTableV1,
};

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
    packing_openings: Option<prepared_source_packing_openings_v1::PreparedSourcePackingOpeningsV1>,
}

/// Original proof session with all prior, signed and negative-magnitude openings.
#[must_use = "dropping signed commitments closes the sole original session and all masks"]
pub(in super::super::super::super::super) struct RnsNativeSmallSignedCommitmentsV1<R> {
    live: Option<SmallSignedCommitmentsLiveV1<R>>,
}

/// The same completed inventory and rhos, consumed once for immutable replay.
/// Only original prefix inspection and exact per-tail linkage are exposed.
/// TODO: future Q-mask continuation must consume the enclosing fully verified
/// stored source; it must never recover a parallel mutable session from here.
#[must_use = "retains the sole original inventory, entropy and zeroizing scalars"]
pub(in super::super::super::super::super) struct RnsNativeStoredPlaneReplayV1<R> {
    live: SmallSignedCommitmentsLiveV1<R>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeStoredPlaneReplayV1<R> {
    /// Borrow only the original ledger retained under this source/inventory.
    pub(in super::super::super::super::super) fn original_budget_mut_v1(
        &mut self,
    ) -> Result<
        &mut crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1,
        ZkAmsMkheErrorV1,
    > {
        Ok(&mut self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .proof_resources)
    }

    pub(in super::super::super::super::super) fn admit_next_q_mask_s_block_v1(
        &mut self,
        table: &RnsNativeU15MsmTableV1,
        stream: &QMaskSOpeningStreamV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::WrittenQMaskSBlockFileV1,
    ) -> Result<QMaskSBlockAdmissionV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        QMaskSBlockAdmissionV1::next_v1(session, table, stream, file)
    }
    pub(in super::super::super::super::super) fn continue_q_mask_s_block_v1(
        &mut self,
        table: &mut RnsNativeU15MsmTableV1,
        stream: QMaskSOpeningStreamV1,
        file: &mut crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFileV1,
        admission: QMaskSBlockAdmissionV1,
    ) -> Result<QMaskSOpeningStreamV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        stream.continue_v1(session, table, file, admission)
    }
    pub(in super::super::super::super::super) fn finish_q_mask_s_openings_v1(
        &mut self,
        table: &RnsNativeU15MsmTableV1,
        stream: QMaskSOpeningStreamV1,
    ) -> Result<CompleteQMaskSOpeningsV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        stream.finish_v1(session, table)
    }
    pub(in super::super::super::super::super) fn begin_q_mask_complements_v1(
        &mut self,
        table: &RnsNativeU15MsmTableV1,
        source: &CompleteQMaskSOpeningsV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
    ) -> Result<QMaskComplementOpeningsV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        QMaskComplementOpeningsV1::new_v1(session, table, source, file)
    }
    pub(in super::super::super::super::super) fn produce_q_mask_complement_block_v1(
        &mut self,
        table: &mut RnsNativeU15MsmTableV1,
        source: &mut CompleteQMaskSOpeningsV1,
        file: &mut crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
        complements: &mut QMaskComplementOpeningsV1,
    ) -> Result<(), QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        complements.produce_next_v1(session, table, source, file)
    }
    pub(in super::super::super::super::super) fn finish_q_mask_complements_v1(
        &mut self,
        table: &RnsNativeU15MsmTableV1,
        source: &CompleteQMaskSOpeningsV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
        complements: &QMaskComplementOpeningsV1,
    ) -> Result<(), QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        complements.require_complete_v1(session, table, source, file)
    }
    pub(in super::super::super::super::super) fn admit_first_q_mask_openings_v1(
        &mut self,
        table: &RnsNativeU15MsmTableV1,
        block: &SampledQMaskSBlockV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::WrittenQMaskSBlockFileV1,
    ) -> Result<QMaskSBlockAdmissionV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        QMaskSBlockAdmissionV1::new_v1(session, table, block, file)
    }
    pub(in super::super::super::super::super) fn produce_first_q_mask_openings_v1(
        &mut self,
        table: &mut RnsNativeU15MsmTableV1,
        block: SampledQMaskSBlockV1,
        admission: QMaskSBlockAdmissionV1,
    ) -> Result<QMaskSOpeningStreamV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        QMaskSOpeningStreamV1::produce_v1(session, table, block, admission)
    }

    pub(in super::super::super::super::super) fn reserve_q_mask_first_memory_v1(
        &mut self,
        plan: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFilePlanV1,
    ) -> Result<
        (
            QMaskFirstBlockMemoryV1,
            crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFileMemoryV1,
        ),
        QMaskSErrorV1,
    > {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        QMaskFirstBlockMemoryV1::new_v1(session, plan)
    }
    pub(in super::super::super::super::super) fn sample_q_mask_first_block_v1(
        &mut self,
        memory: QMaskFirstBlockMemoryV1,
    ) -> Result<SampledQMaskSBlockV1, QMaskSErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(QMaskSErrorV1::Source)?;
        SampledQMaskSBlockV1::sample_v1(session, memory)
    }

    #[cfg(test)]
    pub(in super::super::super::super::super) fn test_u15_workspace_v1(
        &self,
        limit: Option<u64>,
    ) -> u64 {
        let budget = &self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_ref()
            .unwrap()
            .proof_resources;
        if let Some(limit) = limit {
            budget.set_test_workspace_limit_v1(limit);
        }
        budget.live_bytes().unwrap()
    }

    #[cfg(test)]
    pub(in super::super::super::super::super) fn test_u15_deny_next_entropy_v1(
        &mut self,
    ) -> [u32; 3] {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap();
        let GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy) = &mut session.entropy
        else {
            panic!("isolated inventory fixture expected");
        };
        entropy.fault = TestEntropyFaultV1::PanicAt(session.next_global_ordinal);
        [
            session.next_global_ordinal,
            session.next_purpose_ordinal,
            session
                .inventory
                .slots
                .iter()
                .filter(|slot| slot.is_some())
                .count() as u32,
        ]
    }

    // These private arithmetic/resource operations are reachable only from
    // the consuming verified-stored-source transition. They expose no mutable
    // ledger, inventory, entropy or source extraction API.
    pub(in super::super::super::super::super) fn admit_u15_table_v1(
        &mut self,
    ) -> Result<RnsNativeU15MsmTableV1, RnsNativeU15MsmErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .ok_or(RnsNativeU15MsmErrorV1::Source)?;
        RnsNativeU15MsmTableV1::new_v1(&mut session.proof_resources)
    }

    pub(in super::super::super::super::super) fn validate_stored_plane_tail_v1(
        &self,
        ordinal: u16,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        // Admission already consumed and validated this immutable owner. Keep
        // each exact original ticket/rho check, without rehashing the prefix.
        validate_original_stored_tail_v1(&self.live, ordinal, bytes)
    }

    pub(in super::super::super::super::super) fn validate_completed_source_prefix_v1(
        &self,
        record: [u8; 32],
        context: [u8; 32],
        points: [u8; 32],
        blindings: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.live
            .continuation
            .difference
            .top
            .session
            .validate_completed_source_prefix_v1(record, context, points, blindings)
    }
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
                packing_openings: None,
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

    /// Compare authenticated stored tail bytes with the sole admitted ticket/rho.
    /// Consume the complete original inventory into an immutable replay stage.
    /// No session, mask, entropy source or caller-supplied identity is copied.
    pub(in super::super::super::super::super) fn into_stored_plane_replay_v1(
        mut self,
    ) -> Result<RnsNativeStoredPlaneReplayV1<R>, ZkAmsMkheErrorV1> {
        self.require_complete_v1()?;
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        Ok(RnsNativeStoredPlaneReplayV1 { live })
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
    ) -> Result<(Self, PreparedPlaneOpeningTailV1), ZkAmsMkheErrorV1> {
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
        let tail = PreparedPlaneOpeningTailV1::from_admitted_v1(
            live.continuation
                .difference
                .top
                .session
                .live
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            coordinate,
            live.blindings
                .as_slice()
                .last()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )?;
        self.live = Some(live);
        Ok((self, tail))
    }
}

#[cfg(test)]
impl RnsNativeSmallSignedCommitmentsV1<core::convert::Infallible> {
    /// Synthetic completed inventory for actual retained replay dispatch tests.
    /// It is not an authenticated source or a proof-provider authority.
    pub(in super::super::super::super::super) fn test_completed_signed_v1() -> Self {
        let previous = tests::continuation_fixture_before_v1(CONTINUATION_AFTER_PLANE_V1);
        let mut owner = Self::begin_v1(previous).unwrap();
        tests::seed_signed_before_v1(&mut owner, SIGNED_AFTER_PLANE_V1);
        owner
    }

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
// genuine proof-role consumers of the retained stored pair. The local ordered
// writer/replay handoff is not sign/range proof or composite admission.
#[cfg(test)]
#[path = "prepared_small_signed_commitment_v1_tests.rs"]
mod tests;

#[path = "prepared_source_packing_openings_v1.rs"]
mod prepared_source_packing_openings_v1;

// Physical ticket order contains the delta gap, while retained rho vectors are
// logical top[688], continuation[6536], signed[2064]. Never decode an ordinal
// from the file or adopt a caller-supplied point as its expected value.
fn validate_original_stored_tail_v1<R>(
    live: &SmallSignedCommitmentsLiveV1<R>,
    ordinal: u16,
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if ordinal >= SIGNED_AFTER_PLANE_V1
        || bytes.len() != 16_384
        || bytes[65..].iter().any(|byte| *byte != 0)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let ticket = session
        .inventory
        .slots
        .get(coordinate.global_ordinal as usize)
        .and_then(Option::as_ref)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if ticket.coordinate != coordinate || bytes[32..65] != ticket.point_wire {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Point::from_non_identity_wire_bytes_exact(&ticket.point_wire)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let expected = if ordinal < TOP_PLANE_COUNT_V1 {
        live.continuation
            .difference
            .top
            .blindings
            .as_slice()
            .get(usize::from(ordinal))
    } else if ordinal < SIGNED_FIRST_PLANE_V1 {
        live.continuation
            .blindings
            .as_slice()
            .get(usize::from(ordinal - TOP_PLANE_COUNT_V1))
    } else {
        live.blindings
            .as_slice()
            .get(usize::from(ordinal - SIGNED_FIRST_PLANE_V1))
    }
    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let rho = ZeroizingT256ScalarCopyV1::new(
        Scalar::from_be_bytes_exact_ref(
            bytes[..32]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
    );
    if expected.is_zero() || rho.as_ref() != expected {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

#[cfg(test)]
#[path = "prepared_stored_tail_linkage_v1_tests.rs"]
mod stored_tail_linkage_tests;
