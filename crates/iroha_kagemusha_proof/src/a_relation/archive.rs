//! Archive's hard own authorization and evidence-owner boundary.
//!
//! Invalid incoming evidence may select the adjusted-lineage no-op branch.
//! It never excuses a malformed own object, missing C4 verification, changed
//! retained Payment or a different own sigma. TODO: compose all seven fixed
//! owners and qualify genuine Q/A/W chains before admitting an Archive key.

use iroha_plonk::frontend::Error;
use iroha_plonk_recursion::obligation::ledger::Variant;

use super::{context::ContextPlan, schedule::OperationTask};

pub mod authorization;
pub mod evidence;
pub mod incoming;
pub mod maps;
pub mod proofs;
pub mod results;
pub mod retained;
pub mod stage;

#[cfg(test)]
mod tests;

pub(super) fn require_variant(variant: Variant) -> Result<(), Error> {
    if matches!(variant, Variant::ArchiveReceive | Variant::ArchiveStatus) {
        Ok(())
    } else {
        Err(Error::Synthesis)
    }
}

pub(super) fn require_task(
    plan: &ContextPlan,
    stage: u32,
    task: OperationTask,
) -> Result<(), Error> {
    require_variant(plan.operation().frame().variant())?;
    if !plan
        .operation_tasks(usize::try_from(stage).map_err(|_| Error::BoundsFailure)?)
        .is_some_and(|tasks| tasks.contains(&task))
    {
        return Err(Error::Synthesis);
    }
    Ok(())
}
