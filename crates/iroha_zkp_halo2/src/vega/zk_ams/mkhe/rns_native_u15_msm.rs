//! Original-ledger admission for the fixed canonical T256 fifteen-bit kernel.
//!
//! Only the consuming stored-source transition binds this local table to the
//! original source session. Its real memory reservations cover the named new
//! owners; prior allocations, shared arithmetic work units and RSS remain open.
#![allow(
    dead_code,
    reason = "the private original-S producer remains closed pending complete native40 source and proof-role integration"
)]
// TODO: complete the original-S producer before any production proof admission.
use super::rns_native_resource_budget::{
    RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1, RnsNativeResourceReservationV1,
};
use crate::{
    generalized_bulletproof::{
        GeneralizedBulletproofErrorV1, SecretPoint,
        secret_u15_msm_v1::{CanonicalU15PublicTableV1, SECRET_U15_PLANE_LEN_V1, SecretU15PlaneV1},
    },
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::ZkAmsT256BulletproofSuiteV1 as Suite,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeU15MsmErrorV1 {
    Capacity,
    Source,
    Allocation,
    Ledger,
}
fn ledger_error_v1(error: RnsNativeResourceErrorV1) -> RnsNativeU15MsmErrorV1 {
    match error {
        RnsNativeResourceErrorV1::WorkspaceLimit => RnsNativeU15MsmErrorV1::Capacity,
        _ => RnsNativeU15MsmErrorV1::Ledger,
    }
}
fn arithmetic_error_v1(error: GeneralizedBulletproofErrorV1) -> RnsNativeU15MsmErrorV1 {
    match error {
        GeneralizedBulletproofErrorV1::ResourceOverflow => RnsNativeU15MsmErrorV1::Allocation,
        _ => RnsNativeU15MsmErrorV1::Source,
    }
}

/// The immutable public table drops before its actual retained reservation.
pub(super) struct RnsNativeU15MsmTableV1 {
    table: CanonicalU15PublicTableV1<Suite>,
    reservation: RnsNativeResourceReservationV1,
}
impl RnsNativeU15MsmTableV1 {
    pub(super) const fn retained_bytes_v1() -> usize {
        CanonicalU15PublicTableV1::<Suite>::heap_bytes_v1() + core::mem::size_of::<Self>()
    }

    /// The original session supplies its existing ledger; no budget is created.
    pub(super) fn new_v1(
        budget: &mut RnsNativeProofResourceBudgetV1,
    ) -> Result<Self, RnsNativeU15MsmErrorV1> {
        let mut reservation = budget
            .reserve_workspace_v1(
                Self::retained_bytes_v1() as u64,
                CanonicalU15PublicTableV1::<Suite>::construction_scratch_bytes_v1() as u64,
            )
            .map_err(ledger_error_v1)?;
        let table = CanonicalU15PublicTableV1::<Suite>::new_v1().map_err(arithmetic_error_v1)?;
        // Construction's exact temporary row and generator view have dropped.
        reservation.release_scratch();
        Ok(Self { table, reservation })
    }

    pub(super) fn require_original_budget_v1(
        &self,
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<(), RnsNativeU15MsmErrorV1> {
        if !self.reservation.belongs_to_v1(budget) {
            return Err(RnsNativeU15MsmErrorV1::Source);
        }
        Ok(())
    }

    /// Reserve one actual evaluation lifetime before rho or digit preparation.
    /// The resulting move-only admission is tied to this original ledger.
    pub(super) fn admit_evaluation_v1(
        &self,
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<RnsNativeU15EvaluationAdmissionV1, RnsNativeU15MsmErrorV1> {
        self.require_original_budget_v1(budget)?;
        let reservation = self
            .reservation
            .reserve_child_workspace_v1(
                core::mem::size_of::<RnsNativeU15CommitmentV1>() as u64,
                CanonicalU15PublicTableV1::<Suite>::evaluation_scratch_bytes_v1() as u64,
            )
            .map_err(ledger_error_v1)?;
        Ok(RnsNativeU15EvaluationAdmissionV1 { reservation })
    }

    /// Reserve scratch and the returned point before reading any digit. The
    /// original arithmetic controls use the same consuming evaluator below.
    pub(super) fn commitment_v1(
        &mut self,
        budget: &RnsNativeProofResourceBudgetV1,
        exact_len: usize,
        source: impl FnMut(usize) -> Result<u16, GeneralizedBulletproofErrorV1>,
        blinding: &Scalar,
    ) -> Result<RnsNativeU15CommitmentV1, RnsNativeU15MsmErrorV1> {
        self.require_original_budget_v1(budget)?;
        if exact_len != SECRET_U15_PLANE_LEN_V1 || blinding.is_zero() {
            return Err(RnsNativeU15MsmErrorV1::Source);
        }
        let admission = self.admit_evaluation_v1(budget)?;
        self.commitment_with_admission_v1(budget, admission, exact_len, source, blinding)
    }

    /// Consume a previously funded evaluation; do not reserve the same buffers
    /// again after the original producer has already sampled its rho.
    pub(super) fn commitment_with_admission_v1(
        &mut self,
        budget: &RnsNativeProofResourceBudgetV1,
        admission: RnsNativeU15EvaluationAdmissionV1,
        exact_len: usize,
        source: impl FnMut(usize) -> Result<u16, GeneralizedBulletproofErrorV1>,
        blinding: &Scalar,
    ) -> Result<RnsNativeU15CommitmentV1, RnsNativeU15MsmErrorV1> {
        self.require_original_budget_v1(budget)?;
        if !admission.reservation.belongs_to_v1(budget)
            || exact_len != SECRET_U15_PLANE_LEN_V1
            || blinding.is_zero()
        {
            return Err(RnsNativeU15MsmErrorV1::Source);
        }
        let mut reservation = admission.reservation;
        let plane =
            SecretU15PlaneV1::from_source_v1(exact_len, source).map_err(arithmetic_error_v1)?;
        let point = self
            .table
            .commitment_v1(plane, blinding)
            .map_err(arithmetic_error_v1)?;
        // Digits, full-width rho builder and all construction scratch are gone.
        reservation.release_scratch();
        Ok(RnsNativeU15CommitmentV1 { point, reservation })
    }
}

/// Exact evaluation reservation only. No caller can manufacture a token, reset
/// its ledger or substitute a different accounting owner.
pub(super) struct RnsNativeU15EvaluationAdmissionV1 {
    reservation: RnsNativeResourceReservationV1,
}

/// Arithmetic result only: no inventory ticket, source authority or proof seal.
/// The point is erased before its retained memory credit is released.
pub(super) struct RnsNativeU15CommitmentV1 {
    point: SecretPoint<Point>,
    reservation: RnsNativeResourceReservationV1,
}
impl RnsNativeU15CommitmentV1 {
    pub(super) fn expose_ref_v1(&self) -> &Point {
        self.point.expose_ref()
    }

    pub(super) fn require_original_budget_v1(
        &self,
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<(), RnsNativeU15MsmErrorV1> {
        if !self.reservation.belongs_to_v1(budget) {
            return Err(RnsNativeU15MsmErrorV1::Source);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "rns_native_u15_msm_tests.rs"]
mod tests;
