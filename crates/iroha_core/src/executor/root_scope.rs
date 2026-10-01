//! Immutable execution scope at actual instruction boundaries, including VM-produced effects.

use iroha_data_model::{
    ValidationFail,
    block::consensus::SumeragiRootScope,
    isi::{InstructionBox, Log, SetParameter},
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;

use crate::{state::StateTransaction, sumeragi::lanes::routing::committed_root_scope};

/// Read execution authority from the original genesis capability or immutable committed metadata.
/// A header-shaped bootstrap overlay has no authority on its own.
pub(crate) fn execution_root_scope(
    state: &StateTransaction<'_, '_>,
) -> Result<SumeragiRootScope, ValidationFail> {
    if let Some(genesis) = state.genesis_execution_scope.as_ref() {
        return genesis
            .for_transaction(state)
            .ok_or_else(|| denied("genesis instruction scope lost its original source"));
    }
    if super::is_initial_genesis_context(state) {
        return Err(denied(
            "genesis instruction execution requires its authenticated source capability",
        ));
    }
    committed_root_scope(&state.world)
        .ok_or_else(|| denied("instruction execution requires immutable root scope"))
}

/// Bind an unbound code hash to the actual source's already-captured execution dataspace.
pub(crate) fn captured_artifact_id(
    state: &StateTransaction<'_, '_>,
    code_hash: iroha_crypto::Hash,
) -> Result<iroha_data_model::smart_contract::ContractArtifactId, ValidationFail> {
    Ok(iroha_data_model::smart_contract::ContractArtifactId::new(
        captured_dataspace(state)?,
        code_hash,
    ))
}

/// Exact source-owned native execution dataspace, without an implicit universal default.
pub(crate) fn captured_dataspace(
    state: &StateTransaction<'_, '_>,
) -> Result<iroha_model_base::topology::DataSpaceId, ValidationFail> {
    let scope = execution_root_scope(state)?;
    let dataspace = state
        .current_dataspace_id
        .filter(|dataspace| state.world.current_dataspace_id == Some(*dataspace))
        .ok_or_else(|| denied("artifact lookup requires its captured execution dataspace"))?;
    if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope
        && dataspace != dataspace_id
    {
        return Err(denied(
            "artifact lookup differs from its signed root dataspace",
        ));
    }
    Ok(dataspace)
}

/// Private contracts execute through exact on-chain addresses, including atomic batches.
/// Unbound raw IVM/proved overlays cannot acquire a root-local durable-state namespace.
pub(crate) fn ensure_executable_scope(
    state: &StateTransaction<'_, '_>,
    executable: &iroha_data_model::transaction::Executable,
) -> Result<(), ValidationFail> {
    if matches!(
        execution_root_scope(state)?,
        SumeragiRootScope::Dataspace { .. }
    ) && matches!(
        executable,
        iroha_data_model::transaction::Executable::Ivm(_)
            | iroha_data_model::transaction::Executable::IvmProved(_)
    ) {
        return Err(denied(
            "private roots require address-bound contract execution",
        ));
    }
    Ok(())
}

/// Require an address-bound call to stay in the signed private root, including its native route.
pub(crate) fn ensure_contract_scope(
    state: &StateTransaction<'_, '_>,
    address: &iroha_data_model::smart_contract::ContractAddress,
) -> Result<(), ValidationFail> {
    let scope = execution_root_scope(state)?;
    ensure_address_scope(scope, address)?;
    if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope
        && (state.current_dataspace_id != Some(dataspace_id)
            || state.world.current_dataspace_id != Some(dataspace_id))
    {
        return Err(denied(
            "private contract invocation differs from its signed root dataspace",
        ));
    }
    Ok(())
}

/// Check a live nested lookup or retained durable-effect authorization against immutable World.
pub(crate) fn ensure_committed_contract_scope(
    world: &impl crate::state::WorldReadOnly,
    address: &iroha_data_model::smart_contract::ContractAddress,
) -> Result<(), ValidationFail> {
    let scope = committed_root_scope(world)
        .ok_or_else(|| denied("contract execution requires immutable root scope"))?;
    ensure_address_scope(scope, address)
}

/// Require registry reads to belong to the immutable root's exact dataspace.
/// Global roots may read any explicitly addressed artifact; scope is never inferred.
pub(crate) fn ensure_committed_artifact_scope(
    world: &impl crate::state::WorldReadOnly,
    artifact: &iroha_data_model::smart_contract::ContractArtifactId,
) -> Result<(), ValidationFail> {
    let scope = committed_root_scope(world)
        .ok_or_else(|| denied("artifact access requires immutable root scope"))?;
    if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope
        && artifact.dataspace_id != dataspace_id
    {
        return Err(denied(
            "private root cannot access a foreign artifact dataspace",
        ));
    }
    Ok(())
}

fn ensure_address_scope(
    scope: SumeragiRootScope,
    address: &iroha_data_model::smart_contract::ContractAddress,
) -> Result<(), ValidationFail> {
    if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope
        && address.dataspace_id().ok() != Some(dataspace_id)
    {
        return Err(denied(
            "private root cannot invoke a foreign contract dataspace",
        ));
    }
    Ok(())
}

/// Enforce the signed root's authority on each actual instruction before native or custom execution.
/// Static transaction routing is insufficient for generated instructions and deferred multisig.
pub(crate) fn ensure_instruction_scope(
    instruction: &InstructionBox,
    state: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    let scope = execution_root_scope(state)?;
    let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope else {
        return Ok(());
    };
    if state.current_dataspace_id != Some(dataspace_id)
        || state.world.current_dataspace_id != Some(dataspace_id)
    {
        return Err(denied(
            "private instruction execution differs from its signed root dataspace",
        ));
    }
    ensure_private_instruction(instruction, state, dataspace_id, 0)
}

fn ensure_private_instruction(
    instruction: &InstructionBox,
    state: &StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
    depth: usize,
) -> Result<(), ValidationFail> {
    if depth >= 64 {
        return Err(denied(
            "private instruction nesting exceeds the bounded scope review",
        ));
    }
    let bootstrap = state.genesis_execution_scope.is_some();
    let resolve = if bootstrap {
        crate::queue::private_genesis_instruction_target
    } else {
        crate::queue::native_instruction_execution_target
    };
    let target = resolve(
        &**instruction,
        &state.nexus.dataspace_catalog,
        &state.world,
        state.block_unix_timestamp_ms(),
    )
    .map_err(|_| denied("private instruction has no exact native dataspace target"))?;
    let parameter = instruction.as_any().is::<SetParameter>();
    if target.dataspace.is_some_and(|target| target != dataspace)
        || target.global && !(bootstrap && parameter)
        || parameter && !bootstrap
    {
        return Err(denied(
            "private root cannot execute foreign or global-control instructions",
        ));
    }
    if bootstrap {
        // The opaque capability authenticates this original genesis input. Private
        // bootstrap initializes its own alias registry, never a foreign dataspace or parent registry.
        return Ok(());
    }
    if let Ok(multisig) = MultisigInstructionBox::try_from(instruction) {
        match multisig {
            MultisigInstructionBox::Propose(propose) => {
                for nested in &propose.instructions {
                    ensure_private_instruction(nested, state, dataspace, depth + 1)?;
                }
            }
            MultisigInstructionBox::Approve(approve) => {
                if let Some((_, instructions)) =
                    crate::smartcontracts::isi::multisig::live_proposal_instructions_for_approval(
                        state, &approve,
                    )
                {
                    for nested in &instructions {
                        ensure_private_instruction(nested, state, dataspace, depth + 1)?;
                    }
                }
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => {}
        }
        return Ok(());
    }
    if target.dataspace == Some(dataspace) || is_reviewed_root_local_instruction(instruction) {
        return Ok(());
    }
    // TODO: Extend this typed private-root profile only with an explicit scope owner
    // and execution tests. Opaque Custom/native operations, governance and beacon
    // controls must not acquire local authority merely because routing returns None.
    Err(denied(
        "instruction has no reviewed private-root scope owner",
    ))
}

fn is_reviewed_root_local_instruction(instruction: &InstructionBox) -> bool {
    instruction.as_any().is::<Log>()
}

fn denied(message: &'static str) -> ValidationFail {
    ValidationFail::NotPermitted(message.into())
}

#[cfg(test)]
mod tests;
