//! Exact selected artifact execution through the original VM and ordered host effect owner.

use iroha_data_model::{
    ValidationFail, account::AccountId, isi::InstructionBox,
    runtime::definition::ImplementationCodeV1,
};
use ivm::{IVM, IvmProfileId, VMError, error::VmTrap};

use crate::{
    execution_attempt::{ExecutionAttemptError, ExecutionDeferred, vm_attempt_error},
    smartcontracts::ivm::{
        host::{
            CoreHostImpl, NoQueryState,
            generic_carrier::{carrier_error, copy_account},
        },
        map_vm_error_with_context_to_validation,
    },
    state::{StateReadOnly, StateTransaction, StorageReadOnly},
};

/// Work remains chargeable after deterministic VM or effect rejection.
pub(crate) struct CarrierExecutionReport {
    pub(crate) gas_used: u64,
    pub(crate) verdict: Result<(), ValidationFail>,
}

/// An unfinished local attempt, preserving the actual owner and its publication guard.
#[derive(Debug)]
pub(crate) enum CarrierExecutionRefusal {
    /// This boundary still owns the original unlatched VM/codec refusal.
    Execution(ExecutionDeferred),
    /// The original execution refusal is already retained by this same State transaction.
    ExecutionRetained,
    /// The existing State storage owner retains the original MV refusal and release source.
    Storage(crate::state::StateStorageAdmissionError),
}

fn denied() -> ValidationFail {
    ValidationFail::NotPermitted("requires an ordinary selected original carrier invocation".into())
}

fn vm_failure(error: VMError) -> ExecutionAttemptError<ValidationFail> {
    vm_attempt_error(error, |error| {
        ValidationFail::NotPermitted(error.to_string())
    })
}

/// Execute the source-bound original instruction selected by the signed transaction owner.
/// Local refusals escape with their original owner; rejected runs never apply queued writes.
pub(crate) fn execute(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
    gas_limit: u64,
) -> Result<CarrierExecutionReport, CarrierExecutionRefusal> {
    if state.is_execution_deferred() {
        return Err(CarrierExecutionRefusal::ExecutionRetained);
    }
    state
        .require_storage_admission()
        .map_err(CarrierExecutionRefusal::Storage)?;
    let mut gas_used = 0;
    let result = execute_attempt(state, authority, instruction, gas_limit, &mut gas_used);
    // Root-scope reads and artifact application may already retain the original owner. Leave it
    // there for the actual output boundary rather than exporting then re-recording it in State.
    if state.is_execution_deferred() {
        return Err(CarrierExecutionRefusal::ExecutionRetained);
    }
    state
        .require_storage_admission()
        .map_err(CarrierExecutionRefusal::Storage)?;
    let verdict = match result {
        Ok(()) => Ok(()),
        Err(ExecutionAttemptError::Rejected(error)) => Err(error),
        Err(ExecutionAttemptError::Deferred(original)) => {
            return Err(CarrierExecutionRefusal::Execution(original));
        }
    };
    Ok(CarrierExecutionReport { gas_used, verdict })
}

fn execute_attempt(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
    gas_limit: u64,
    gas_used: &mut u64,
) -> Result<(), ExecutionAttemptError<ValidationFail>> {
    if state.execution_callback_active()
        || state.completing_obligation.is_some()
        || !state.deferred_expectations.is_empty()
        || state.native_execution.is_some()
        || state.world.runtime_selection.get() != state.world.runtime_selection.canonical()
        || state.genesis_execution_scope.is_some()
    {
        return Err(denied().into());
    }
    let catalog = crate::runtime::release_catalog().map_err(|_| denied())?;
    let (code_hash, maximum_gas) = {
        let published =
            crate::runtime::change_set::PublishedDefinitions(&state.world.runtime_definitions);
        let decoded = super::decode_selected(
            catalog,
            state.world.runtime_selection.get(),
            &published,
            state.block_height(),
            instruction,
        )
        .map_err(carrier_error)
        .map_err(vm_failure)?
        .ok_or_else(denied)?;
        let ImplementationCodeV1::Ivm {
            code_hash,
            entrypoint,
        } = &decoded.implementation.code
        else {
            return Err(denied().into());
        };
        if entrypoint != "main" {
            return Err(denied().into());
        }
        (*code_hash, decoded.implementation.limits.max_gas)
    };
    let artifact_id = crate::executor::root_scope::captured_artifact_id(
        state,
        iroha_crypto::Hash::prehashed(code_hash),
    )?;
    let limit = gas_limit.min(maximum_gas);
    let budget = state.execution_budget();
    let mut runtime = IVM::try_new_with_memory_budget(limit, &budget).map_err(vm_failure)?;
    runtime
        .set_heap_max_limit(state.world.parameters.get().smart_contract().memory().get())
        .map_err(vm_failure)?;
    {
        let artifact = state
            .world
            .runtime_generic_artifacts
            .get(&artifact_id)
            .ok_or_else(denied)?;
        if artifact.len() > iroha_data_model::isi::runtime_carrier::MAX_CARRIER_ARTIFACT_BYTES_V1
            || <[u8; 32]>::from(ivm::contract_code_hash(artifact)) != code_hash
        {
            return Err(denied().into());
        }
        runtime
            .load_program_for_profile(artifact, IvmProfileId::C10_GENERIC_CARRIER)
            .map_err(vm_failure)?;
    }
    let published =
        crate::runtime::change_set::PublishedDefinitions(&state.world.runtime_definitions);
    let decoded = super::decode_selected(
        catalog,
        state.world.runtime_selection.get(),
        &published,
        state.block_height(),
        instruction,
    )
    .map_err(carrier_error)
    .map_err(vm_failure)?
    .ok_or_else(denied)?;
    // Charge both the temporary envelope and authority clone before materializing them. The VM
    // separately retains its own INPUT backing from this same original allocation pool.
    let mut bytes = decoded.frame.len().checked_add(39).ok_or_else(denied)?;
    crate::executor::add_account_clone_bytes(&mut bytes, authority).map_err(vm_failure)?;
    let _input_backing = budget
        .try_reserve_bytes(bytes)
        .map_err(|original| ExecutionAttemptError::Deferred(original.into()))?;
    let input = ivm_abi::pointer_abi::encode_tlv_with_allocator_custody(
        ivm::pointer_abi::PointerType::NoritoBytes,
        decoded.frame,
    )
    .map_err(vm_failure)?;
    let pointer = runtime.alloc_input_tlv(&input).map_err(vm_failure)?;
    runtime.set_register(10, pointer);
    let mut host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
        copy_account(authority).map_err(vm_failure)?,
        state,
        instruction,
    )
    .map_err(vm_failure)?;
    let result = runtime.run_with_host(&mut host);
    *gas_used = limit.saturating_sub(runtime.remaining_gas());
    result.map_err(|error| map_vm_error_with_context_to_validation(&runtime, error))?;
    if runtime.register(10) != 0 {
        return Err(map_vm_error_with_context_to_validation(
            &runtime,
            VMError::Trap(VmTrap::PermissionDenied),
        ));
    }
    let artifacts = host.into_execution_artifacts(None)?;
    artifacts.apply_to_transaction(state, authority)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
