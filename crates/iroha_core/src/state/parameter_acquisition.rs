//! Exact physical acquisition shared by explicit parameter fixture constructors.
//!
//! This acquires only the original parameter Cell. It neither changes other World
//! current/undo cuts nor grants instruction, genesis, or protocol authorization.

use super::World;
use crate::pipeline_gas_policy::RetainedStateParameters;
use iroha_allocation::{AllocationCharge, AllocationRefusal};
use mv::cell::{Cell, CellAllocationCharges, CellGenerationBacking, CellPublicationSuccessor};

/// Original refusal before an explicit fixture parameter Cell block exists.
#[derive(Debug, thiserror::Error)]
pub(super) enum AcquisitionError {
    /// Complete original finite-pool reservation did not fit.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// Original current/undo backing refused.
    #[error(transparent)]
    Generations(#[from] mv::cell::CellGenerationBackingError),
    /// Original successor identity backing refused.
    #[error(transparent)]
    Successor(#[from] mv::cell::CellPublicationSuccessorError),
    /// Original nonblocking writer probe refused.
    #[error(transparent)]
    Writer(#[from] mv::storage::AdmittedStorageError),
    /// An internal owner did not retain its original pool.
    #[error("parameter Cell acquisition belongs to another original pool")]
    ForeignPool,
}

/// Reserve and retain all three original physical allocations before any writer.
pub(super) fn original_block(
    world: &World,
    mode: mv::BlockMode,
) -> Result<mv::cell::Block<'_, RetainedStateParameters, AllocationCharge>, AcquisitionError> {
    let budget = world.sccp_committee_generations.allocation_budget();
    let [current, undo] = Cell::<RetainedStateParameters, AllocationCharge>::allocation_layouts();
    let identity = CellPublicationSuccessor::allocation_layout();
    let mut reservation = budget.try_reserve_layouts([current, undo, identity])?;
    let charges = CellAllocationCharges::new(
        reservation
            .try_split(current)
            .expect("complete original current layout reservation"),
        reservation
            .try_split(undo)
            .expect("complete original undo layout reservation"),
    );
    let backing = CellGenerationBacking::try_from_charges(budget, charges)
        .map_err(|(_charges, error)| error)?;
    let successor = CellPublicationSuccessor::try_from_charge(
        budget,
        reservation
            .try_split(identity)
            .expect("complete original identity layout reservation"),
    )
    .map_err(|(_charge, error)| error)?;
    let mut acquisition = world
        .parameters
        .try_block_acquisition_with_backing(backing, successor, budget)
        .map_err(|(_backing, _successor)| AcquisitionError::ForeignPool)?;
    acquisition.try_initialize(mode)?;
    Ok(mv::BlockAcquisition::into_block(acquisition))
}
