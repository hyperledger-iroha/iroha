//! Exact comparator/signed tail owned by the original admitted proof session.
//!
//! The retained scalar and inventory point are copied only after admission. No
//! public constructor accepts a replacement point, blinding or source identity.
use super::*;
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::comparator_signed_coordinate_v1;

const PLANE_COUNT_V1: u16 = 9_288;
const TAIL_BYTES_V1: u64 = 16_384;
const TAIL_PREFIX_BYTES_V1: usize = 32 + 33;

fn tail_plane_ordinal_v1(
    coordinate: GlobalLookupCommitmentCoordinateV1,
) -> Result<u16, ZkAmsMkheErrorV1> {
    let first = match coordinate.purpose {
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop => 0_u32,
        GlobalLookupCommitmentPurposeV1::ComparatorSumTop => 344,
        GlobalLookupCommitmentPurposeV1::ComparatorBorrow => 688,
        GlobalLookupCommitmentPurposeV1::ComparatorMixedTop => 6_880,
        GlobalLookupCommitmentPurposeV1::SmallSigned => 7_224,
        GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude => 8_256,
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    let ordinal = first
        .checked_add(coordinate.purpose_ordinal)
        .and_then(|ordinal| u16::try_from(ordinal).ok())
        .filter(|ordinal| *ordinal < PLANE_COUNT_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if comparator_signed_coordinate_v1(u32::from(ordinal))? != coordinate {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(ordinal)
}

/// Move-only canonical tail minted from one newly admitted original opening.
#[must_use = "the tail must be emitted after its 32 values before source handoff"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedPlaneOpeningTailV1
{
    ordinal: u16,
    blinding: ZeroizingT256ScalarCopyV1,
    point_wire: [u8; 33],
    #[cfg(test)]
    drop_probe: TailDropProbeV1,
}

impl PreparedPlaneOpeningTailV1 {
    // Only this original session and its commitment-producing descendants can
    // mint the tail. The retained rho is selected internally after its append.
    pub(super) fn from_admitted_v1<R>(
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        coordinate: GlobalLookupCommitmentCoordinateV1,
        retained_blinding: &Scalar,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let ordinal = tail_plane_ordinal_v1(coordinate)?;
        let next = commitment_coordinate_v1(
            coordinate
                .global_ordinal
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )?;
        if session.pending_source.is_some()
            || session.next_global_ordinal != next.global_ordinal
            || session.next_purpose != next.purpose
            || session.next_purpose_ordinal != next.purpose_ordinal
            || retained_blinding.is_zero()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let admitted = session
            .inventory
            .slots
            .get(coordinate.global_ordinal as usize)
            .and_then(Option::as_ref)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if admitted.coordinate != coordinate {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Point::from_non_identity_wire_bytes_exact(&admitted.point_wire)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        Ok(Self {
            ordinal,
            blinding: ZeroizingT256ScalarCopyV1::new(*retained_blinding),
            point_wire: admitted.point_wire,
            #[cfg(test)]
            drop_probe: TailDropProbeV1,
        })
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn require_ordinal_v1(
        &self,
        expected: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.ordinal != expected || expected >= PLANE_COUNT_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn into_chunk_v1(
        self,
        expected: u16,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        self.require_ordinal_v1(expected)?;
        if self.blinding.as_ref().is_zero() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Point::from_non_identity_wire_bytes_exact(&self.point_wire)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(TAIL_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let encoded: &mut [u8; 32] = (&mut chunk.as_mut_slice_v1()[..32])
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.blinding.as_ref().write_le_bytes_ref(encoded);
        encoded.reverse();
        chunk.as_mut_slice_v1()[32..TAIL_PREFIX_BYTES_V1].copy_from_slice(&self.point_wire);
        Ok(chunk)
    }
}

#[cfg(test)]
std::thread_local! {
    static TAIL_SCALAR_OWNER_DROPS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

// Field drop order runs this observer after the existing scalar erasure guard.
// It observes disposal of that guard, not compiler/register-wide erasure.
#[cfg(test)]
struct TailDropProbeV1;

#[cfg(test)]
impl Drop for TailDropProbeV1 {
    fn drop(&mut self) {
        TAIL_SCALAR_OWNER_DROPS_V1.with(|count| count.set(count.get() + 1));
    }
}

#[cfg(test)]
impl PreparedPlaneOpeningTailV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn test_wire_fixture_v1(
        ordinal: u16,
    ) -> Self {
        tests::admitted_tail_fixture_v1(ordinal)
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn test_scalar_owner_drop_count_v1()
    -> usize {
        TAIL_SCALAR_OWNER_DROPS_V1.with(core::cell::Cell::get)
    }
}

#[cfg(test)]
#[path = "prepared_opening_tail_v1_tests.rs"]
mod tests;
