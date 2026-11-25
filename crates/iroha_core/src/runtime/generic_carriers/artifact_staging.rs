//! Independently selected immutable code custody, with no grant of runtime execution authority.

use crate::{
    execution_attempt::{
        ExecutionAttemptError, vm_attempt_error_with_rejection as vm_attempt_error,
    },
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, StorageReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::InstructionExecutionError as Error,
        runtime_carrier::{MAX_CARRIER_ARTIFACT_BYTES_V1, StageRuntimeCarrierArtifactV1},
    },
};

fn denied(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}

/// Validate one current or undo artifact using committed owner/scope, without granting execution.
pub(crate) fn validate_stored(
    world: &impl WorldReadOnly,
    id: &iroha_data_model::smart_contract::ContractArtifactId,
    bytes: &[u8],
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), ExecutionAttemptError<String>> {
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    let owner = catalog
        .world_storage
        .get("world.runtime_generic_artifacts")
        .ok_or_else(|| "carrier artifact owner is not compiled".to_owned())?;
    if world
        .runtime_selection()
        .installed_storage
        .binary_search(owner)
        .is_err()
        || world.runtime_definitions().get(owner) != catalog.definitions.get(owner)
    {
        return Err("carrier artifact owner is not exactly installed"
            .to_owned()
            .into());
    }
    crate::executor::root_scope::ensure_committed_artifact_scope(world, id)
        .map_err(|error| error.map_rejection(|error| error.to_string()))?;
    if bytes.len() > MAX_CARRIER_ARTIFACT_BYTES_V1 || ivm::contract_code_hash(bytes) != id.code_hash
    {
        return Err("carrier artifact size or complete-image identity differs"
            .to_owned()
            .into());
    }
    let mut verifier = ivm::IVM::try_new_for_external_host_with_memory_budget(0, budget)
        .map_err(|error| vm_attempt_error(error, |error| error.to_string()))?;
    verifier
        .load_program_for_profile(bytes, ivm::IvmProfileId::C10_GENERIC_CARRIER)
        .map_err(|error| vm_attempt_error(error, |error| error.to_string()))
}

/// Require the compiled staging binding and installed owner even at direct component boundaries.
fn selected(state: &StateTransaction<'_, '_>) -> Result<(), Error> {
    let catalog = crate::runtime::release_catalog().map_err(|error| denied(error.to_string()))?;
    let addition = catalog
        .additions
        .iter()
        .find(|addition| {
            addition.module == crate::runtime::catalog::additions::GENERIC_CARRIER_ARTIFACTS.module
        })
        .ok_or_else(|| denied("carrier artifact staging is not compiled"))?;
    let interface = addition
        .interfaces
        .get(StageRuntimeCarrierArtifactV1::WIRE_ID)
        .ok_or_else(|| denied("carrier staging interface is not compiled"))?;
    let storage = addition
        .storage
        .get("world.runtime_generic_artifacts")
        .ok_or_else(|| denied("carrier artifact owner is not compiled"))?;
    let selection = state.world.runtime_selection.get();
    if selection != state.world.runtime_selection.canonical()
        || !selection.bindings.iter().any(|binding| {
            binding.wire_id == StageRuntimeCarrierArtifactV1::WIRE_ID
                && binding.interface == *interface
                && binding.implementation == addition.implementation
        })
        || selection.installed_storage.binary_search(storage).is_err()
        || [*interface, addition.implementation, *storage]
            .iter()
            .any(|id| {
                crate::runtime::blocking_restriction(selection, id, state.block_height()).is_some()
                    || state.world.runtime_definitions.get(id) != catalog.definitions.get(id)
            })
    {
        return Err(denied(
            "carrier artifact staging requires its ordinary selected binding and storage",
        ));
    }
    Ok(())
}

impl Execute for StageRuntimeCarrierArtifactV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_original(&self, authority, state)
    }
}

/// Borrow the exact native envelope through bounded verification and retained storage admission.
pub(crate) fn execute_original(
    instruction: &StageRuntimeCarrierArtifactV1,
    authority: &AccountId,
    state: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    selected(state)?;
    // Existing artifact creation is available to registered fee-paying accounts. Staging
    // immutable code grants neither a deployment binding nor any right to execute it.
    state.world.account(authority).map_err(Error::from)?;
    let expected =
        crate::executor::root_scope::captured_artifact_id(state, instruction.artifact_id.code_hash)
            .map_err(|error| denied(error.to_string()))?;
    if expected != instruction.artifact_id {
        return Err(denied(
            "carrier artifact differs from its captured source data space",
        ));
    }
    if instruction.code.len() > MAX_CARRIER_ARTIFACT_BYTES_V1 {
        return Err(denied(
            "carrier artifact exceeds its complete-image byte bound",
        ));
    }
    if ivm::contract_code_hash(&instruction.code) != instruction.artifact_id.code_hash {
        return Err(denied(
            "carrier artifact hash differs from its complete bytes",
        ));
    }
    // Use the same explicit profile parser and verifier as invocation, with original VM
    // allocation custody. This accepts no CNTR contract or ordinary-profile fallback.
    let budget = state.execution_budget();
    let mut verifier =
        ivm::IVM::try_new_for_external_host_with_memory_budget(0, &budget).map_err(|error| {
            state.attempt_error_to_instruction_error(vm_attempt_error(error, |error| {
                denied(error.to_string())
            }))
        })?;
    verifier
        .load_program_for_profile(&instruction.code, ivm::IvmProfileId::C10_GENERIC_CARRIER)
        .map_err(|error| {
            state.attempt_error_to_instruction_error(vm_attempt_error(error, |error| {
                denied(error.to_string())
            }))
        })?;
    if let Some(existing) = state
        .world
        .runtime_generic_artifacts
        .get(&instruction.artifact_id)
    {
        return if existing.as_slice() == instruction.code.as_slice() {
            Ok(())
        } else {
            Err(denied(
                "carrier artifact identity already contains different bytes",
            ))
        };
    }
    match super::storage::stage_artifact(state, instruction.artifact_id, &instruction.code) {
        Ok(()) => Ok(()),
        Err(_) if state.is_execution_deferred() || state.require_storage_admission().is_err() => {
            Err(denied(
                "original local carrier artifact admission did not complete",
            ))
        }
        Err(error) => Err(denied(error.to_string())),
    }
}

#[cfg(test)]
mod tests;
