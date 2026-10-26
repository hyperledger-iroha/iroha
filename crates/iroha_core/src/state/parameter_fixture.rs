//! Explicit test-only authoring into the one immutable admitted parameter cell.
//!
//! This helper does not authenticate genesis or bypass production instruction
//! checks. World seeding is for a caller-owned pre-chain component fixture;
//! transaction seeding preserves ordinary before-image/apply/rollback semantics.

use iroha_data_model::parameter::Parameter;
use mv::cell::CellInitialization;

use super::{World, WorldBlock, WorldTransaction};
use crate::pipeline_gas_policy::{
    ParameterIngressError, ParameterRetentionError, RetainedStateParameters,
};

/// Exact refusal before an explicit component parameter fixture can be installed.
#[derive(Debug, thiserror::Error)]
pub enum ParameterFixtureError {
    /// Original canonical parsing and complete projection admission refusal.
    #[error(transparent)]
    Ingress(#[from] ParameterIngressError),
    /// Original canonical validation or graph ownership refusal.
    #[error(transparent)]
    Parameter(#[from] ParameterRetentionError),
    /// Original capacity refusal before any fixture writer is acquired.
    #[error(transparent)]
    Admission(#[from] iroha_allocation::AllocationRefusal),
    /// Original exact current/undo physical backing refusal.
    #[error(transparent)]
    Generations(#[from] mv::cell::CellGenerationBackingError),
    /// Original exact publication identity backing refusal.
    #[error(transparent)]
    Successor(#[from] mv::cell::CellPublicationSuccessorError),
    /// Original nonblocking Cell writer acquisition refusal.
    #[error(transparent)]
    Acquisition(#[from] mv::storage::AdmittedStorageError),
    /// Original physical admission for a new pre-chain fixture cell.
    #[error(transparent)]
    Cell(#[from] mv::cell::CellInitializationError),
}

impl World {
    /// Author one complete consensus family in an explicit pre-chain fixture.
    /// This grants no signed-genesis authority and preserves original-pool refusal.
    ///
    /// # Errors
    /// Returns admission or full Cell initialization refusal before replacement.
    pub fn set_sumeragi_for_testing(
        &mut self,
        consensus: iroha_data_model::parameter::SumeragiParameters,
    ) -> Result<(), ParameterFixtureError> {
        let budget = self.sccp_committee_generations.allocation_budget().clone();
        let original = self.parameters.view();
        let prepared = original.get().prepare_sumeragi(&consensus, &budget)?;
        let controls = CellInitialization::<RetainedStateParameters>::try_reserve(&budget)?;
        let successor = prepared
            .into_successor(original.get(), &budget)
            .map_err(|(_prepared, error)| error)?;
        drop(original);
        self.parameters = controls.initialize(successor, None);
        Ok(())
    }
    /// Seed an explicit parameter in a caller-owned pre-chain component fixture.
    ///
    /// The complete successor and new current/undo controls are funded before
    /// the old fixture cell is replaced. This starts an explicit fixture cut with
    /// no prior block; use the transaction helper to exercise retained undo.
    /// It does not create a signed-genesis capability or insert policy defaults.
    ///
    /// # Errors
    /// Returns original validation, finite-pool or physical allocation refusal.
    pub fn set_parameter_for_testing(
        &mut self,
        parameter: Parameter,
    ) -> Result<(), ParameterFixtureError> {
        let budget = self.sccp_committee_generations.allocation_budget().clone();
        let original = self.parameters.view();
        let prepared = original
            .get()
            .prepare_fixture_parameter(parameter, &budget)?;
        let controls = CellInitialization::<RetainedStateParameters>::try_reserve(&budget)?;
        let successor = prepared
            .into_successor(original.get(), &budget)
            .map_err(|(_prepared, error)| error)?;
        drop(original);
        self.parameters = controls.initialize(successor, None);
        Ok(())
    }
}
impl WorldTransaction<'_, '_> {
    /// Seed one explicit test parameter through complete successor preparation.
    ///
    /// The exact predecessor is checked before first mutable cell access. Applied
    /// siblings, rollback, retained original policy and late reader lifetime use
    /// the same immutable graph and MV before-images as ordinary publication.
    /// This never bypasses authorization in production because this module is
    /// compiled only for explicit component test support.
    ///
    /// # Errors
    /// Returns original validation or ownership refusal without touching the Cell.
    pub fn set_parameter_for_testing(
        &mut self,
        parameter: Parameter,
    ) -> Result<(), ParameterFixtureError> {
        let budget = self.sccp_committee_generations.allocation_budget().clone();
        let prepared = self
            .parameters
            .get()
            .prepare_fixture_parameter(parameter, &budget)?;
        prepared.check_source(self.parameters.get(), &budget)?;
        prepared
            .install(self.parameters.get_mut(), &budget)
            .map_err(|(_prepared, error)| error)?;
        Ok(())
    }
}

impl WorldBlock<'_> {
    /// Author an explicit component parameter through this real MV block cut.
    /// All successor backing precedes mutable access; ordinary undo is retained.
    ///
    /// # Errors
    /// Returns original parsing, admission or source-identity refusal.
    pub fn set_parameter_for_testing(
        &mut self,
        parameter: Parameter,
    ) -> Result<(), ParameterFixtureError> {
        let pool = self.sccp_committee_generations.allocation_budget();
        let prepared = self.parameters.get().prepare_parameter(&parameter, pool)?;
        let next = prepared
            .into_successor(self.parameters.get(), pool)
            .map_err(|(_prepared, original)| original)?;
        *self.parameters.get_mut() = next;
        Ok(())
    }

    /// Author one complete consensus family while preserving the real block undo.
    /// This does not authenticate a signed-genesis or protocol instruction.
    ///
    /// # Errors
    /// Returns original parsing, admission or source-identity refusal.
    pub fn set_sumeragi_for_testing(
        &mut self,
        consensus: iroha_data_model::parameter::SumeragiParameters,
    ) -> Result<(), ParameterFixtureError> {
        let pool = self.sccp_committee_generations.allocation_budget();
        let prepared = self.parameters.get().prepare_sumeragi(&consensus, pool)?;
        let next = prepared
            .into_successor(self.parameters.get(), pool)
            .map_err(|(_prepared, original)| original)?;
        *self.parameters.get_mut() = next;
        Ok(())
    }
}

impl WorldTransaction<'_, '_> {
    /// Author one complete family through actual transaction rollback/apply.
    ///
    /// # Errors
    /// Returns original preparation or source refusal before mutation.
    pub fn set_sumeragi_for_testing(
        &mut self,
        consensus: iroha_data_model::parameter::SumeragiParameters,
    ) -> Result<(), ParameterFixtureError> {
        let pool = self.sccp_committee_generations.allocation_budget();
        let prepared = self.parameters.get().prepare_sumeragi(&consensus, pool)?;
        let next = prepared
            .into_successor(self.parameters.get(), pool)
            .map_err(|(_prepared, original)| original)?;
        *self.parameters.get_mut() = next;
        Ok(())
    }
}

/// Admit the exact original parameter-only fixture publication before acquiring
/// either writer. Every other World cell keeps its existing current/undo cut.
///
/// Both EBR shells and the next publication identity use the original World pool;
/// the retained graph handle is only shared after those physical owners exist.
/// This is explicit component fixture construction, not protocol authorization.
///
/// # Errors
/// Preserves the original finite-pool, physical backing or writer refusal.
pub fn parameter_cell_block_for_testing(
    world: &World,
) -> Result<
    mv::cell::Block<'_, RetainedStateParameters, iroha_allocation::AllocationCharge>,
    ParameterFixtureError,
> {
    parameter_cell_for_testing(world, mv::BlockMode::Ordinary)
}

/// Acquire only the original parameter Cell in replacement mode with funded backing.
/// The same retained current/undo versions are selected by MV's canonical replacement
/// kernel; every other World cell keeps its current and original cut unchanged.
///
/// # Errors
/// Preserves original finite-pool, physical backing, and writer refusals.
pub fn parameter_cell_block_and_revert_for_testing(
    world: &World,
) -> Result<
    mv::cell::Block<'_, RetainedStateParameters, iroha_allocation::AllocationCharge>,
    ParameterFixtureError,
> {
    parameter_cell_for_testing(world, mv::BlockMode::Replace)
}

fn parameter_cell_for_testing(
    world: &World,
    mode: mv::BlockMode,
) -> Result<
    mv::cell::Block<'_, RetainedStateParameters, iroha_allocation::AllocationCharge>,
    ParameterFixtureError,
> {
    use super::parameter_acquisition::AcquisitionError;
    super::parameter_acquisition::original_block(world, mode).map_err(|original| match original {
        AcquisitionError::Admission(error) => ParameterFixtureError::Admission(error),
        AcquisitionError::Generations(error) => ParameterFixtureError::Generations(error),
        AcquisitionError::Successor(error) => ParameterFixtureError::Successor(error),
        AcquisitionError::Writer(error) => ParameterFixtureError::Acquisition(error),
        AcquisitionError::ForeignPool => ParameterRetentionError::ForeignPool.into(),
    })
}

/// Update only an explicit fixture parameter Cell block, preserving every other
/// World's current/undo cut. The caller commits this same Cell block normally.
/// No genesis capability or production authorization is granted.
///
/// # Errors
/// Returns the original refusal before first mutable access to the Cell.
pub fn set_parameter_cell_for_testing(
    cell: &mut mv::cell::Block<'_, RetainedStateParameters, iroha_allocation::AllocationCharge>,
    parameter: Parameter,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), ParameterFixtureError> {
    let prepared = cell.get().prepare_parameter(&parameter, budget)?;
    let next = prepared
        .into_successor(cell.get(), budget)
        .map_err(|(_prepared, original)| original)?;
    *cell.get_mut() = next;
    Ok(())
}

/// Replace one explicit fixture consensus family through parameter-only MV.
///
/// # Errors
/// Returns the original refusal before changing the current/undo graph handles.
pub fn set_sumeragi_cell_for_testing(
    cell: &mut mv::cell::Block<'_, RetainedStateParameters, iroha_allocation::AllocationCharge>,
    consensus: iroha_data_model::parameter::SumeragiParameters,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), ParameterFixtureError> {
    let prepared = cell.get().prepare_sumeragi(&consensus, budget)?;
    let next = prepared
        .into_successor(cell.get(), budget)
        .map_err(|(_prepared, original)| original)?;
    *cell.get_mut() = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::parameter::{SumeragiParameter, SumeragiParameters};

    #[test]
    fn parameter_only_fixtures_refuse_before_touch_and_keep_the_exact_undo() {
        let world = World::default();
        let budget = world.sccp_committee_generations.allocation_budget();
        let before = world.parameters.view().get().clone();
        let mut block = parameter_cell_block_for_testing(&world).unwrap();
        let held = budget.reserved_bytes();
        let original_limit = budget.limit_bytes();
        budget.set_limit_bytes(held);
        assert!(
            set_parameter_cell_for_testing(
                &mut block,
                Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(71)),
                budget
            )
            .is_err()
        );
        assert!(before.same_version(block.get()));
        assert_eq!(budget.reserved_bytes(), held);
        budget.set_limit_bytes(original_limit);
        set_parameter_cell_for_testing(
            &mut block,
            Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(71)),
            budget,
        )
        .unwrap();
        assert!(before.same_version(block.get_before_block()));
        block.commit();
        let mut next = parameter_cell_block_for_testing(&world).unwrap();
        let first = next.get().clone();
        let mut consensus = SumeragiParameters::default();
        consensus.max_clock_drift_ms = 97;
        set_sumeragi_cell_for_testing(&mut next, consensus, budget).unwrap();
        assert!(first.same_version(next.get_before_block()));
        assert_eq!(next.get().sumeragi.max_clock_drift_ms, 97);
        drop(next);
        assert!(first.same_version(world.parameters.view().get()));
    }
    #[test]
    fn parameter_only_replacement_retains_exact_undo_and_refuses_before_writer() {
        let world = World::default();
        let budget = world.sccp_committee_generations.allocation_budget();
        let original = world.parameters.view().get().clone();
        let mut first = parameter_cell_block_for_testing(&world).unwrap();
        set_parameter_cell_for_testing(
            &mut first,
            Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(71)),
            budget,
        )
        .unwrap();
        first.commit();
        let current = world.parameters.view().get().clone();
        assert!(!current.same_version(&original));
        let held = budget.reserved_bytes();
        let limit = budget.limit_bytes();
        budget.set_limit_bytes(held);
        assert!(matches!(
            parameter_cell_block_and_revert_for_testing(&world),
            Err(ParameterFixtureError::Admission(_))
        ));
        assert_eq!(budget.reserved_bytes(), held);
        assert!(current.same_version(world.parameters.view().get()));
        assert!(original.same_version(world.parameters.predecessor_view().get().as_ref().unwrap()));
        budget.set_limit_bytes(limit);
        let mut replacement = parameter_cell_block_and_revert_for_testing(&world).unwrap();
        assert!(original.same_version(replacement.get()));
        assert!(original.same_version(replacement.get_before_block()));
        set_parameter_cell_for_testing(
            &mut replacement,
            Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(97)),
            budget,
        )
        .unwrap();
        drop(replacement);
        assert!(current.same_version(world.parameters.view().get()));
        assert!(original.same_version(world.parameters.predecessor_view().get().as_ref().unwrap()));
        let ordinary = parameter_cell_block_for_testing(&world).unwrap();
        assert!(current.same_version(ordinary.get()));
        drop(ordinary);
        let replacement = parameter_cell_block_and_revert_for_testing(&world).unwrap();
        assert!(original.same_version(replacement.get()));
        drop(replacement);
    }

    #[test]
    fn parameter_only_acquisition_refuses_original_backing_before_any_writer() {
        use crate::test_allocations::refuse_layout_occurrence_during;
        use mv::cell::{Cell, CellPublicationSuccessor};
        let world = World::default();
        let budget = world.sccp_committee_generations.allocation_budget();
        let before = world.parameters.view().get().clone();
        let held = budget.reserved_bytes();
        let limit = budget.limit_bytes();
        budget.set_limit_bytes(held);
        assert!(matches!(
            parameter_cell_block_for_testing(&world),
            Err(ParameterFixtureError::Admission(_))
        ));
        assert_eq!(budget.reserved_bytes(), held);
        assert!(before.same_version(world.parameters.view().get()));
        budget.set_limit_bytes(limit);
        let [current, undo] =
            Cell::<RetainedStateParameters, iroha_allocation::AllocationCharge>::allocation_layouts(
            );
        let layouts = [current, undo, CellPublicationSuccessor::allocation_layout()];
        for (index, layout) in layouts.into_iter().enumerate() {
            let occurrence = layouts[..index]
                .iter()
                .filter(|earlier| **earlier == layout)
                .count();
            let (result, refused) = refuse_layout_occurrence_during(layout, occurrence, || {
                parameter_cell_block_for_testing(&world)
            });
            assert!(
                refused,
                "actual selected backing allocation must be refused"
            );
            let error = result
                .err()
                .expect("refused original backing cannot acquire a block");
            let original_layout = match error {
                ParameterFixtureError::Generations(
                    mv::cell::CellGenerationBackingError::Allocator { layout },
                ) => layout,
                ParameterFixtureError::Successor(
                    mv::cell::CellPublicationSuccessorError::Allocator { layout },
                ) => layout,
                other => panic!("unexpected refusal {other}"),
            };
            assert_eq!(original_layout, layout);
            assert_eq!(budget.reserved_bytes(), held);
            assert!(before.same_version(world.parameters.view().get()));
            let retry = parameter_cell_block_for_testing(&world).unwrap();
            assert!(before.same_version(retry.get()));
            drop(retry);
            assert_eq!(budget.reserved_bytes(), held);
        }
    }
}
