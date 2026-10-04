//! Immutable execution scope at actual instruction boundaries, including VM-produced effects.

use iroha_data_model::{
    ValidationFail,
    block::consensus::SumeragiRootScope,
    isi::{InstructionBox, Log, SetParameter},
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use mv::storage::StorageReadOnly as _;

use crate::{
    state::{RootScopeDecodeRefusal, StateStorageAdmissionError, StateTransaction},
    sumeragi::lanes::routing::read_committed_root_scope,
};

/// Read execution authority from the original genesis capability or immutable committed metadata.
/// A header-shaped bootstrap overlay has no authority on its own.
pub(crate) fn execution_root_scope(
    state: &mut StateTransaction<'_, '_>,
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
    match read_committed_root_scope(&state.world) {
        Ok(Some(scope)) => Ok(scope),
        Ok(None) => Err(denied(
            "instruction execution requires immutable root scope",
        )),
        Err(error) => {
            let refusal = match error {
                norito::json::Error::DecodeResourceLimit => RootScopeDecodeRefusal::Budget,
                norito::json::Error::AllocationFailed => RootScopeDecodeRefusal::Allocator,
                // Intrinsic malformed/depth errors are not a local retry policy.
                _ => {
                    return Err(denied(
                        "instruction execution requires immutable root scope",
                    ));
                }
            };
            if cfg!(all(test, sumeragi_core_mutation = "HC27")) {
                return Err(ValidationFail::InternalError(
                    "local execution-root read did not complete".into(),
                ));
            }
            state.arm_local_storage_refusal(StateStorageAdmissionError::RootScopeDecode(refusal));
            Err(state.defer_execution(match refusal {
                RootScopeDecodeRefusal::Budget => {
                    ivm::error::ExecutionDeferral::ActiveMemoryCapacity
                }
                RootScopeDecodeRefusal::Allocator => {
                    ivm::error::ExecutionDeferral::AllocationUnavailable
                }
            }))
        }
    }
}

/// Bind an unbound code hash to the actual source's already-captured execution dataspace.
pub(crate) fn captured_artifact_id(
    state: &mut StateTransaction<'_, '_>,
    code_hash: iroha_crypto::Hash,
) -> Result<iroha_data_model::smart_contract::ContractArtifactId, ValidationFail> {
    Ok(iroha_data_model::smart_contract::ContractArtifactId::new(
        captured_dataspace(state)?,
        code_hash,
    ))
}

/// Exact source-owned native execution dataspace, without an implicit universal default.
pub(crate) fn captured_dataspace(
    state: &mut StateTransaction<'_, '_>,
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
    state: &mut StateTransaction<'_, '_>,
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
    state: &mut StateTransaction<'_, '_>,
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
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    use crate::execution_attempt::ExecutionAttemptError;
    let scope = match read_committed_root_scope(world) {
        Ok(Some(scope)) => scope,
        Err(error) if !cfg!(all(test, sumeragi_core_mutation = "HC30")) => {
            let reason = match error {
                norito::json::Error::DecodeResourceLimit => {
                    ivm::error::ExecutionDeferral::ActiveMemoryCapacity
                }
                norito::json::Error::AllocationFailed => {
                    ivm::error::ExecutionDeferral::AllocationUnavailable
                }
                _ => return Err(denied("contract execution requires immutable root scope").into()),
            };
            return Err(ExecutionAttemptError::Deferred(reason.into()));
        }
        Ok(None) | Err(_) => {
            return Err(denied("contract execution requires immutable root scope").into());
        }
    };
    ensure_address_scope(scope, address).map_err(Into::into)
}

/// Require registry reads to belong to the immutable root's exact dataspace.
/// Global roots may read any explicitly addressed artifact; scope is never inferred.
pub(crate) fn ensure_committed_artifact_scope(
    world: &impl crate::state::WorldReadOnly,
    artifact: &iroha_data_model::smart_contract::ContractArtifactId,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    use crate::execution_attempt::ExecutionAttemptError;
    let scope = match read_committed_root_scope(world) {
        Ok(Some(scope)) => scope,
        Err(error)
            if !cfg!(all(
                test,
                any(
                    sumeragi_core_mutation = "HC28",
                    sumeragi_core_mutation = "HC30"
                )
            )) =>
        {
            let reason = match error {
                norito::json::Error::DecodeResourceLimit => {
                    ivm::error::ExecutionDeferral::ActiveMemoryCapacity
                }
                norito::json::Error::AllocationFailed => {
                    ivm::error::ExecutionDeferral::AllocationUnavailable
                }
                _ => return Err(denied("artifact access requires immutable root scope").into()),
            };
            return Err(ExecutionAttemptError::Deferred(reason.into()));
        }
        Ok(None) | Err(_) => {
            return Err(denied("artifact access requires immutable root scope").into());
        }
    };
    if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope
        && artifact.dataspace_id != dataspace_id
    {
        return Err(denied("private root cannot access a foreign artifact dataspace").into());
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
    state: &mut StateTransaction<'_, '_>,
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
    state: &mut StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
    depth: usize,
) -> Result<(), ValidationFail> {
    if depth >= 64 {
        return Err(denied(
            "private instruction nesting exceeds the bounded scope review",
        ));
    }
    // These are the global coordinator's operations, including at genesis. The
    // private bootstrap exception below grants local initialization authority only.
    if !cfg!(all(test, sumeragi_core_mutation = "HC26"))
        && is_global_amx_coordinator_instruction(instruction)
    {
        return Err(denied(
            "AMX coordinator requires the authenticated global root",
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
    .map_err(|error| match error {
        crate::queue::RoutingResolveError::Deferred(reason) => state.defer_execution(reason),
        _ => denied("private instruction has no exact native dataspace target"),
    })?;
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
    let multisig = match MultisigInstructionBox::try_from(instruction) {
        Ok(multisig) => Some(multisig),
        Err(error) => {
            match crate::smartcontracts::isi::multisig::multisig_instruction_decode_attempt(
                error,
                |_| (),
            ) {
                crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                    return Err(state.defer_execution(reason));
                }
                crate::execution_attempt::ExecutionAttemptError::Rejected(()) => None,
            }
        }
    };
    if let Some(multisig) = multisig {
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
                    .map_err(|error| state.attempt_error_to_validation_fail(error))?
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
    // A recipient's routing scope cannot establish ownership of a token naming another
    // contract. Review this exact mutation even when native routing already found a local target.
    if ensure_private_entrypoint_permission_scope(instruction, state, dataspace)? {
        return Ok(());
    }
    if ensure_private_holding_limit_scope(instruction, state, dataspace)? {
        return Ok(());
    }
    if ensure_private_account_metadata_scope(instruction, state, dataspace)? {
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

/// Account metadata mutations own one actual retained account, including an account with no
/// routing label. The authenticated private World supplies its resource scope; this grants no
/// metadata permission and does not replace native reserved-key or value-size validation.
fn ensure_private_account_metadata_scope(
    instruction: &InstructionBox,
    state: &StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<bool, ValidationFail> {
    use iroha_data_model::isi::{RemoveKeyValueBox, SetKeyValueBox};

    let account = if let Some(SetKeyValueBox::Account(set)) =
        instruction.as_any().downcast_ref::<SetKeyValueBox>()
    {
        set.object()
    } else if let Some(RemoveKeyValueBox::Account(remove)) =
        instruction.as_any().downcast_ref::<RemoveKeyValueBox>()
    {
        remove.object()
    } else {
        return Ok(false);
    };
    if state
        .world
        .contract_subject_addresses
        .get(account)
        .is_some_and(|address| &address.subject_id() != account)
    {
        return Err(denied(
            "private account metadata requires its original contract subject index",
        ));
    }
    // Reuse the existing retained account/directory/contract-subject scope guard. Unlabelled
    // accounts belong to this authenticated private World; no caller route or new fallback
    // invents an account binding, and an explicit foreign directory entry remains a refusal.
    ensure_private_permission_account_scope(state, account, dataspace)?;
    Ok(true)
}

/// A holding limit mutates one actual account and asset definition, including when the
/// instruction has no routing hint. This establishes only resource scope: the native handler
/// still requires the asset owner or the exact account-and-asset holding-limit permission.
fn ensure_private_holding_limit_scope(
    instruction: &InstructionBox,
    state: &mut StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<bool, ValidationFail> {
    let Some(limit) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::SetAssetHoldingLimit>()
    else {
        return Ok(false);
    };
    review_private_holding_limit_scope(limit, state, dataspace)
        .map_err(|error| state.attempt_error_to_validation_fail(error))?;
    Ok(true)
}

fn review_private_holding_limit_scope(
    limit: &iroha_data_model::isi::SetAssetHoldingLimit,
    state: &StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    use crate::state::WorldReadOnly as _;
    use iroha_data_model::asset::AssetBalancePolicy;

    ensure_private_permission_account_scope(state, &limit.account_id, dataspace)?;
    // Borrow the original retained definition and indexes; scope admission must not clone
    // arbitrary asset metadata before its resource owner has been established.
    let definition = state
        .world
        .asset_definitions()
        .get(&limit.asset_definition_id)
        .ok_or_else(|| denied("private holding limit requires an existing asset definition"))?;
    if definition.id != limit.asset_definition_id
        || definition.balance_scope_policy() != AssetBalancePolicy::DataspaceRestricted
    {
        return Err(denied("private holding limit requires its original restricted asset").into());
    }
    let domain = definition
        .owning_domain()
        .as_ref()
        .ok_or_else(|| denied("private holding limit requires an immutable owning domain"))?;
    if state
        .world
        .asset_definition_domains()
        .get(&limit.asset_definition_id)
        != Some(domain)
        || state.world.domains().get(domain).is_none()
    {
        return Err(
            denied("private holding limit requires its original asset-domain binding").into(),
        );
    }
    ensure_private_permission_account_scope(state, definition.owned_by(), dataspace)?;
    ensure_private_holding_limit_dataspace(state, domain.dataspace().as_ref(), dataspace)?;

    let binding = state
        .world
        .asset_definition_alias_bindings()
        .get(&limit.asset_definition_id);
    match binding {
        Some(binding) => {
            if binding.is_grace_expired_at(state.block_unix_timestamp_ms())
                || state.world.asset_definition_aliases().get(&binding.alias)
                    != Some(&limit.asset_definition_id)
                || definition
                    .alias()
                    .as_ref()
                    .is_some_and(|alias| alias != &binding.alias)
            {
                return Err(denied(
                    "private holding limit requires its retained asset-alias binding",
                )
                .into());
            }
            ensure_private_holding_limit_dataspace(
                state,
                binding.alias.dataspace_segment(),
                dataspace,
            )?;
        }
        None if definition.alias().is_some() => {
            return Err(denied("private holding limit cannot use an unbound asset alias").into());
        }
        None => {}
    }
    Ok(())
}

fn ensure_private_holding_limit_dataspace(
    state: &StateTransaction<'_, '_>,
    alias: &str,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let resolved = crate::sns::resolve_active_dataspace_id_by_alias(
        &state.world,
        &state.nexus.dataspace_catalog,
        alias,
        state.block_unix_timestamp_ms(),
    )
    .map_err(|error| {
        error.into_attempt_error(|_| {
            denied("private holding limit requires an exact active asset dataspace binding")
        })
    })?;
    if resolved != dataspace {
        return Err(denied("private holding limit cannot target a foreign asset dataspace").into());
    }
    Ok(())
}

/// An exact invocation token is owned by its actual retained contract, never by the caller's
/// default route. This scope check grants no delegation authority: the native lifecycle-owner
/// permission boundary still runs for every owned, borrowed and contract-emitted mutation.
fn ensure_private_entrypoint_permission_scope(
    instruction: &InstructionBox,
    state: &mut StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<bool, ValidationFail> {
    use iroha_data_model::smart_contract::ContractLifecycleOwnerV1;

    let Some(super::PermissionOrRoleMutation::AccountPermission {
        permission,
        destination,
        ..
    }) = super::extract_permission_or_role_mutation(instruction)
    else {
        return Ok(false);
    };
    if permission.name() != "CanInvokeContractEntrypoint" {
        return Ok(false);
    }
    #[derive(crate::json_macros::JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct ExactEntrypointScope {
        contract: iroha_data_model::smart_contract::ContractAddress,
        entrypoint: String,
    }
    let token = super::read_permission_payload::<ExactEntrypointScope>(permission)
        .map_err(|error| state.attempt_error_to_validation_fail(error))?
        .ok_or_else(|| denied("private entrypoint permission requires its exact typed payload"))?;
    if token.entrypoint.is_empty() || token.entrypoint.trim() != token.entrypoint {
        return Err(denied(
            "private entrypoint permission requires a canonical non-empty selector",
        ));
    }
    if token.contract.dataspace_id().ok() != Some(dataspace) {
        return Err(denied(
            "private entrypoint permission cannot name a foreign contract dataspace",
        ));
    }
    let (subject, lifecycle) =
        crate::smartcontracts::code::fetch_contract_lifecycle(&state.world, &token.contract)
            .map_err(ValidationFail::NotPermitted)?
            .ok_or_else(|| {
                denied("private entrypoint permission requires a deployed lifecycle owner")
            })?;
    if state.world.contract_subject_addresses.get(&subject) != Some(&token.contract) {
        return Err(denied(
            "private entrypoint permission requires its original contract subject index",
        ));
    }
    ensure_private_permission_account_scope(state, destination, dataspace)?;
    ensure_private_permission_account_scope(state, &subject, dataspace)?;
    for owner in [Some(&lifecycle.owner), lifecycle.pending_owner.as_ref()]
        .into_iter()
        .flatten()
    {
        if let ContractLifecycleOwnerV1::Account(account) = owner {
            ensure_private_permission_account_scope(state, account, dataspace)?;
        }
    }
    Ok(true)
}

fn ensure_private_permission_account_scope(
    state: &StateTransaction<'_, '_>,
    account: &iroha_data_model::account::AccountId,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<(), ValidationFail> {
    use crate::state::WorldReadOnly as _;
    use iroha_model_base::topology::DataSpaceId;

    state
        .world
        .account(account)
        .map_err(|_| denied("private entrypoint permission requires an existing local account"))?;
    if crate::smartcontracts::code::historical_contract_for_subject(&state.world, account)
        .is_some_and(|address| address.dataspace_id().ok() != Some(dataspace))
    {
        return Err(denied(
            "private entrypoint permission cannot target a foreign contract subject",
        ));
    }
    // Unlabelled accounts retained by this private World have only the universal read fallback.
    // An explicit application scope must still belong to this signed root; it cannot narrow a
    // foreign or shared account into the token's contract dataspace.
    if state
        .world
        .account_scope_directory()
        .get(account)
        .is_some_and(|entry| {
            entry
                .iter()
                .any(|(scope, _)| *scope != DataSpaceId::UNIVERSAL && *scope != dataspace)
        })
    {
        return Err(denied(
            "private entrypoint permission cannot target a foreign account scope",
        ));
    }
    Ok(())
}

fn is_global_amx_coordinator_instruction(instruction: &InstructionBox) -> bool {
    use iroha_data_model::isi::sumeragi_amx::{
        BeginAmxV1, RegisterAmxDataspaceV1, RelayAmxHandoffV1, RelayAmxPreparedV1,
    };
    let instruction = instruction.as_any();
    instruction.is::<RegisterAmxDataspaceV1>()
        || instruction.is::<BeginAmxV1>()
        || instruction.is::<RelayAmxPreparedV1>()
        || instruction.is::<RelayAmxHandoffV1>()
}

fn is_reviewed_root_local_instruction(instruction: &InstructionBox) -> bool {
    instruction.as_any().is::<Log>()
}

fn denied(message: &'static str) -> ValidationFail {
    ValidationFail::NotPermitted(message.into())
}

#[cfg(test)]
mod tests;
