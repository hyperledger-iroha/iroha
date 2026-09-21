//! Original-session source/packing mask derivation, without proof authority.
//!
//! The retained owner fixes 344 reconstructed D openings followed by 1,032
//! signed openings. It samples no entropy, adopts no point and never advances
//! the next QMaskDigit coordinate. The native40 authenticated-context join and
//! final derived-mask provider remain unavailable.

use super::*;
use crate::vega::zk_ams::mkhe::rns_native_source_packing_same_opening::{
    DIFFERENCE_GROUPS_V1, OWNERS_V1, PLANES_PER_SIGNED_ROLE_V1, RADIX_LOW_DIGITS_V1, RECORDS_V1,
    RnsNativeSignedSourceRoleV1, RnsNativeSourcePackingCommitmentViewV1,
    RnsNativeSourcePackingSameOpeningErrorV1, SIGNED_OWNERS_V1, SIGNED_ROLES_V1,
    source_packing_point_root_v1,
};

const DERIVED_MASK_BYTES_V1: usize = OWNERS_V1 * core::mem::size_of::<Scalar>();
const _: () = {
    assert!(OWNERS_V1 == 1_376);
    assert!(DERIVED_MASK_BYTES_V1 == 44_032);
    assert!(RADIX_LOW_DIGITS_V1 == EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1);
    assert!(DIFFERENCE_GROUPS_V1 == EXISTING_RADIX_CANDIDATE_GROUPS_V1);
    assert!(SIGNED_OWNERS_V1 == 1_032);
};

/// Secret derived material retained inside its original completed session.
/// There is deliberately no public constructor, getter or detached handoff.
pub(super) struct PreparedSourcePackingOpeningsV1 {
    masks: ZeroizingT256ScalarVecV1,
    point_root: [u8; 32],
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeSmallSignedCommitmentsV1<R> {
    /// Derive the sole original source/packing mask material exactly once.
    pub(in crate::vega::zk_ams::mkhe) fn prepare_source_packing_openings_v1(
        mut self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Taking the original owner first makes every error/unwind consuming.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_small_signed_progress_v1(&live)?;
        if live.next_plane != SIGNED_AFTER_PLANE_V1 || live.packing_openings.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let view = OriginalPackingCommitmentViewV1 { live: &live };
        let mut masks = ZeroizingT256ScalarVecV1::try_with_exact_capacity(OWNERS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for group in 0..DIFFERENCE_GROUPS_V1 {
            let mask = derive_difference_mask_v1(&live, group)?;
            masks.push(mask.get());
        }
        for signed in 0..SIGNED_OWNERS_V1 {
            let (record, role, plane) = signed_owner_coordinate_v1(signed)?;
            let coordinate = signed_inventory_coordinate_v1(record, role, plane)?;
            // Revalidate the exact original ticket before selecting its rho.
            view.point_v1(coordinate)?;
            masks.push(
                *live
                    .blindings
                    .as_slice()
                    .get(signed)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            );
        }
        if masks.len() != OWNERS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let point_root = source_packing_point_root_v1(&view)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.packing_openings = Some(PreparedSourcePackingOpeningsV1 { masks, point_root });
        // The original next coordinate is still QMaskDigit/0 at 27,176.
        validate_small_signed_progress_v1(&live)?;
        self.live = Some(live);
        Ok(self)
    }
}

fn difference_low_coordinate_v1(
    group: usize,
    digit: usize,
) -> Result<ExistingRadixCandidateCoordinateV1, ZkAmsMkheErrorV1> {
    if group >= DIFFERENCE_GROUPS_V1 || digit >= RADIX_LOW_DIGITS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    // Retained low masks are group-major D[17],S[17]; the sole inventory is
    // purpose-major. Reuse its canonical coordinate instead of assuming offsets.
    let wire = group * EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1 + digit;
    let coordinate = existing_radix_candidate_coordinate_v1(wire as u32)?;
    if coordinate.role != RnsNativeExistingRadixCandidateRoleV1::DifferenceLow
        || coordinate.group as usize != group
        || coordinate.column as usize != digit
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coordinate)
}

fn signed_owner_coordinate_v1(
    ordinal: usize,
) -> Result<(usize, RnsNativeSignedSourceRoleV1, usize), ZkAmsMkheErrorV1> {
    if ordinal >= SIGNED_OWNERS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let record = ordinal / (SIGNED_ROLES_V1 * PLANES_PER_SIGNED_ROLE_V1);
    let role = match (ordinal / PLANES_PER_SIGNED_ROLE_V1) % SIGNED_ROLES_V1 {
        0 => RnsNativeSignedSourceRoleV1::R,
        1 => RnsNativeSignedSourceRoleV1::E0,
        2 => RnsNativeSignedSourceRoleV1::E1,
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    Ok((record, role, ordinal % PLANES_PER_SIGNED_ROLE_V1))
}

fn signed_inventory_coordinate_v1(
    record: usize,
    role: RnsNativeSignedSourceRoleV1,
    plane: usize,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    if record >= RECORDS_V1 || plane >= PLANES_PER_SIGNED_ROLE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let signed = (record * SIGNED_ROLES_V1 + role as usize) * PLANES_PER_SIGNED_ROLE_V1 + plane;
    signed_commitment_coordinate_v1(SIGNED_FIRST_PLANE_V1 + signed as u16)
}

fn derive_difference_mask_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &SmallSignedCommitmentsLiveV1<R>,
    group: usize,
) -> Result<ZeroizingT256ScalarCopyV1, ZkAmsMkheErrorV1> {
    let view = OriginalPackingCommitmentViewV1 { live };
    let top = &live.continuation.difference.top;
    let mut mask = ZeroizingT256ScalarCopyV1::new(Scalar::zero());
    let mut weight = Scalar::one();
    let radix = Scalar::from_u64(1 << 15);
    for digit in 0..RADIX_LOW_DIGITS_V1 {
        let coordinate = difference_low_coordinate_v1(group, digit)?;
        view.point_v1(commitment_coordinate_v1(coordinate.inventory_ordinal)?)?;
        let rho = top
            .retained_low
            .blindings
            .as_slice()
            .get(coordinate.wire_ordinal as usize)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        mask.add_product_assign(&weight, rho);
        weight *= radix;
    }
    // This is bD, never the separately retained bS or Csrc source-order mask.
    view.point_v1(top_coordinate_v1(group as u16)?)?;
    let rho = top
        .blindings
        .as_slice()
        .get(group)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    mask.add_product_assign(&weight, rho);
    Ok(mask)
}

struct OriginalPackingCommitmentViewV1<'owner, R> {
    live: &'owner SmallSignedCommitmentsLiveV1<R>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> OriginalPackingCommitmentViewV1<'_, R> {
    fn point_v1(
        &self,
        coordinate: GlobalLookupCommitmentCoordinateV1,
    ) -> Result<Point, ZkAmsMkheErrorV1> {
        let session = self
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let expected = commitment_coordinate_v1(coordinate.global_ordinal)?;
        let ticket = session
            .inventory
            .slots
            .get(coordinate.global_ordinal as usize)
            .and_then(Option::as_ref)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if coordinate != expected || ticket.coordinate != expected {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Point::from_non_identity_wire_bytes_exact(&ticket.point_wire)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RnsNativeSourcePackingCommitmentViewV1
    for OriginalPackingCommitmentViewV1<'_, R>
{
    fn packing_difference_low_v1(
        &self,
        group: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        let coordinate = difference_low_coordinate_v1(group, digit)
            .and_then(|coordinate| commitment_coordinate_v1(coordinate.inventory_ordinal))
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
        self.point_v1(coordinate)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)
    }
    fn packing_difference_top_v1(
        &self,
        group: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        if group >= DIFFERENCE_GROUPS_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let coordinate = top_coordinate_v1(group as u16)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
        self.point_v1(coordinate)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)
    }
    fn packing_signed_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        let coordinate = signed_inventory_coordinate_v1(record, role, plane)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
        self.point_v1(coordinate)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)
    }
}

// TODO: consume these retained masks only through the genuine native40 source
// context and later same-opening prover owner. No raw digest adapter or final
// RnsNativeSourcePackingDerivedMaskSourceV1 implementation is authorized here.
#[cfg(test)]
#[path = "prepared_source_packing_openings_v1_tests.rs"]
mod tests;
