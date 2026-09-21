//! Original-session reservation for the two live low-digit scalar vectors.
//!
//! The group and prepared plane each contain exactly 16,384 T256 scalars.
//! This reservation covers their payloads and this new control owner only.
//! Existing source/spool buffers, emitted chunks, earlier owners, MSM scratch,
//! allocator bookkeeping, stack/RSS and aggregate arithmetic work remain open.
use super::*;
use crate::vega::zk_ams::mkhe::rns_native_resource_budget::{
    RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1, RnsNativeResourceReservationV1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) enum LowDigitWorkspaceErrorV1
{
    Capacity,
    Source,
    Resource,
}

/// Opaque, move-only admission retained by the real consuming source driver.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct LowDigitWorkspaceV1
{
    reservation: RnsNativeResourceReservationV1,
}
impl LowDigitWorkspaceV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) const fn bytes_v1()
    -> u64 {
        (2 * RADIX_COEFFICIENTS_PER_GROUP_V2 * core::mem::size_of::<VegaT256ScalarV1>()
            + core::mem::size_of::<Self>()) as u64
    }

    /// Only the original retained source phase supplies its existing ledger.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn admit_v1(
        budget: &mut RnsNativeProofResourceBudgetV1,
    ) -> Result<Self, LowDigitWorkspaceErrorV1> {
        let reservation = budget
            .reserve_workspace_v1(Self::bytes_v1(), 0)
            .map_err(|error| match error {
                RnsNativeResourceErrorV1::WorkspaceLimit => LowDigitWorkspaceErrorV1::Capacity,
                _ => LowDigitWorkspaceErrorV1::Resource,
            })?;
        Ok(Self { reservation })
    }

    #[cfg(test)]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn belongs_to_v1(
        &self,
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> bool {
        self.reservation.belongs_to_v1(budget)
    }
}

/// Only a local capacity refusal preserves the unchanged original source.
#[must_use = "retry the same source after original-ledger capacity is released"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct LowDigitPreparationRefusalV1
<R, K, P> {
    pub(super) reason: LowDigitWorkspaceErrorV1,
    pub(super) source: Option<Phase23RadixWitnessMaterializedV2<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> LowDigitPreparationRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> LowDigitWorkspaceErrorV1 {
        self.reason
    }

    #[allow(
        clippy::result_large_err,
        reason = "pre-allocation refusal owns the unchanged original source without allocating a box"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
    ) -> Result<LowDigitPreparationV1<R, K, P>, Self> {
        let Some(source) = self.source.take() else {
            return Err(self);
        };
        source.into_low_digit_preparation_v1()
    }
}
