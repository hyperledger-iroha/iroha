//! Semantic projections tied to the original immutable Musubi World borrow.
//!
//! The source-work validator retains static rejections and prepays sequential
//! NFC scratch. Concrete codec and borrowed verifier/helper controls cover the
//! three registered semantic readers. This token proves completed predicates on
//! the borrowed cut and keeps its original pool; temporary validation scratch
//! is released before tree capture. It cannot establish State finality.

use super::super::*;
use crate::state::authority_registry::world::{
    musubi_availability_policy::MusubiAvailabilityAuthorityV1,
    musubi_universal_policy::{MusubiDirectoryAuthorityV1, MusubiResolverAuthorityV1},
};
use crate::state::deserialize::musubi_source_read::{MusubiSourceReadOnly, StateMusubiSourceCut};
use iroha_allocation::AllocationBudget;

// The public WorldReadOnly trait alone cannot promise immutable observations:
// outside implementations may replace rows through interior mutability.
mod sealed {
    /// Closed implementation marker for native immutable observation carriers.
    pub trait NativeCut {}
    impl NativeCut for super::StateMusubiSourceCut<'_> {}
    impl NativeCut for crate::state::WorldView<'_> {}
    impl NativeCut for crate::state::WorldBlock<'_> {}
    impl NativeCut for crate::state::WorldTransaction<'_, '_> {}
    impl NativeCut for Box<crate::state::WorldTransaction<'_, '_>> {}
}

/// Native immutable borrow carriers admitted by this semantic observation owner.
/// The private supertrait prevents third-party WorldReadOnly implementations.
pub(in crate::state) trait MusubiObservationCut:
    MusubiSourceReadOnly + sealed::NativeCut
{
}
impl MusubiObservationCut for WorldView<'_> {}
impl MusubiObservationCut for StateMusubiSourceCut<'_> {}
impl MusubiObservationCut for WorldBlock<'_> {}
impl MusubiObservationCut for WorldTransaction<'_, '_> {}
impl MusubiObservationCut for Box<WorldTransaction<'_, '_>> {}

/// A completed semantic check of one still-borrowed cut, never a funding permit.
pub(in crate::state) struct ValidatedMusubiSource<'cut, W: MusubiObservationCut> {
    world: &'cut W,
    // Keep the caller's original pool identity, without reserving fictitious
    // crypto bytes or extending charges for scratch already released.
    execution_budget: &'cut AllocationBudget,
}

impl<W: MusubiObservationCut> std::fmt::Debug for ValidatedMusubiSource<'_, W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedMusubiSource")
            .finish_non_exhaustive()
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "row-only observation helpers are currently exercised by source-custody controls"
    )
)]
impl<'cut, W: MusubiObservationCut> ValidatedMusubiSource<'cut, W> {
    /// Only the parent validator constructs this after all semantic passes succeed.
    pub(super) fn new_validated(world: &'cut W, execution_budget: &'cut AllocationBudget) -> Self {
        Self {
            world,
            execution_budget,
        }
    }

    /// Original pool for a future real allocation, not evidence of existing funding.
    pub(in crate::state) fn execution_budget(&self) -> &AllocationBudget {
        self.execution_budget
    }

    /// Borrow the exact validated availability keys and copy only authority fields.
    pub(in crate::state) fn availability_rows(
        &self,
    ) -> impl Iterator<Item = (&ArchiveId, MusubiAvailabilityAuthorityV1)> {
        self.world
            .source_musubi_archive_availability()
            .iter()
            .map(|(key, row)| (key, MusubiAvailabilityAuthorityV1::from_record(row)))
    }

    /// Borrow exact validated release keys and each independently authoritative revision.
    pub(in crate::state) fn resolver_rows(
        &self,
    ) -> impl Iterator<Item = (&MusubiReleaseIdV1, MusubiResolverAuthorityV1)> {
        self.world
            .source_musubi_resolver_index()
            .iter()
            .map(|(key, row)| (key, MusubiResolverAuthorityV1::from_record(row)))
    }

    /// Borrow exact validated selector keys and each independently authoritative revision.
    pub(in crate::state) fn directory_rows(
        &self,
    ) -> impl Iterator<Item = (&MusubiPackageSelectorV1, MusubiDirectoryAuthorityV1)> {
        self.world
            .source_musubi_public_directory()
            .iter()
            .map(|(key, row)| (key, MusubiDirectoryAuthorityV1::from_record(row)))
    }
}

#[cfg(test)]
#[path = "deserialize_world_musubi_source_observation_tests.rs"]
mod tests;

#[path = "deserialize_world_musubi_capture_candidate.rs"]
mod capture_candidate;
pub(in crate::state) use capture_candidate::MusubiSemanticTable;
