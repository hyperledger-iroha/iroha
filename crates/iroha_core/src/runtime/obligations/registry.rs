//! The kernel's obligation registry (`specs/live_runtime_upgrades.md` §7.2): registration,
//! completion, and what a restriction or a change set must leave to a registered obligation.
//!
//! `world.runtime_obligations` holds the one [`ObligationRecordV1`] each owner holds: an account
//! that registered for itself, or a contract subject registered by its lifecycle controller or
//! by its authenticated named-profile invocation. A registration commits to the exact instructions that complete the
//! obligation and captures their completion closure from the selection the root executes.
//!
//! - **Completion.** When the owner issues one of the committed instructions, the kernel
//!   executes it as a completion and marks it performed on success: admission closure and
//!   completion-only on the definitions behind it, or behind what it invokes, do not stop it
//!   ([`performing`], [`super::Step`]). Each committed instruction lends that capability once.
//!   The baseline ABI has no registration kernel call: committed instructions a contract queues
//!   under its subject account complete the obligation registered for it. The independently
//!   selected contract lifecycle host profile also lets it register and end its own obligation.
//! - **Activation.** A new binding may coexist with a captured implementation. The completion
//!   runs under its captured context, including authorization and host selection, while normal
//!   instructions run under the new binding. Removing or disabling a required definition, or
//!   changing root-wide kernel, commitment or services semantics, still lapses ([`closure_gap`]).
//!
//! The registry is storage of the `iroha.runtime.lifecycle.v1` addition. A root that has not
//! activated it holds no record, so nothing here changes what that root executes.

use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{
        InstructionBox,
        error::InstructionExecutionError as Error,
        runtime_obligation::{CompleteRuntimeObligationV1, RegisterRuntimeObligationV1},
    },
    runtime::{
        definition::{DefinitionLookup, RootSelectionV1},
        obligation::{
            CompletionCaptureV1, ObligationId, ObligationRecordV1, check_registration,
            terminal_effect_id, terminal_effects,
        },
    },
    smart_contract::{ContractAddress, ContractLifecycleOwnerV1},
};
use mv::storage::StorageReadOnly as _;

use super::super::change_set::{PublishedDefinitions, block_ms};
use crate::state::StateTransaction;

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("runtime obligation: {reason}").into())
}

/// Release the temporary authority even when an outer execution owner catches an unwind.
struct CompletionScope<'transaction, 'block, 'world> {
    transaction: &'transaction mut StateTransaction<'block, 'world>,
    previous_selection: Option<RootSelectionV1>,
}

impl Drop for CompletionScope<'_, '_, '_> {
    fn drop(&mut self) {
        self.transaction.completing_obligation = None;
        self.transaction
            .world
            .runtime_selection
            .set_executing(self.previous_selection.take());
    }
}

/// The account that controls the lifecycle of `contract` and the contract's subject account.
///
/// # Errors
/// The contract has no lifecycle binding, or Parliament alone controls its lifecycle.
fn contract_owner(
    world: &impl crate::state::WorldReadOnly,
    contract: &ContractAddress,
) -> Result<(AccountId, AccountId), Error> {
    let binding = crate::smartcontracts::code::fetch_contract_lifecycle_binding(world, contract)
        .map_err(refused)?
        .ok_or_else(|| refused(format_args!("{contract} is not a deployed contract")))?;
    match &binding.lifecycle.owner {
        ContractLifecycleOwnerV1::Account(controller) => {
            Ok((controller.clone(), binding.subject.clone()))
        }
        ContractLifecycleOwnerV1::Parliament => Err(refused(format_args!(
            "Parliament controls the lifecycle of {contract}; no account registers for it"
        ))),
    }
}

/// The obligation `owner` holds, with its identity.
fn held(
    state_transaction: &StateTransaction<'_, '_>,
    owner: &AccountId,
) -> Result<(ObligationId, Option<ObligationRecordV1>), Error> {
    let id = ObligationId::derive(owner).map_err(refused)?;
    Ok((
        id,
        state_transaction
            .world
            .runtime_obligations
            .get(&id)
            .cloned(),
    ))
}

/// Register the obligation of an owner with the kernel, or replace it (§7.2).
///
/// The owner is `authority`, or the subject account of the contract the instruction names, whose
/// lifecycle `authority` must control. The completion closure is captured from the selection the
/// root executes now, and registering is new work: a restricted terminal effect refuses it. An
/// owner holds one obligation; registering again replaces it.
///
/// # Errors
/// The registration is malformed, `authority` does not control the named contract, or the
/// closure cannot be captured.
pub(crate) fn register(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &RegisterRuntimeObligationV1,
) -> Result<(), Error> {
    let now_ms = block_ms(state_transaction);
    let height = state_transaction._curr_block.height().get();
    check_registration(instruction.kind, instruction.expires_ms, now_ms).map_err(refused)?;
    let (committed, wire_ids) = terminal_effects(&instruction.terminal_effects).map_err(refused)?;
    let owner = match &instruction.contract {
        None => authority.clone(),
        Some(contract) => {
            let (controller, subject) = contract_owner(&state_transaction.world, contract)?;
            if controller != *authority {
                return Err(refused(format_args!(
                    "only the account that controls the lifecycle of {contract} registers for it"
                )));
            }
            subject
        }
    };
    let capture = CompletionCaptureV1::capture(
        state_transaction.world.runtime_selection.get(),
        &PublishedDefinitions(&state_transaction.world.runtime_definitions),
        &wire_ids,
        height,
    )
    .map_err(refused)?;
    crate::smartcontracts::ivm::host_profile::ensure_new_capture(
        state_transaction.world(),
        &capture,
        height,
    )?;
    let (id, _) = held(state_transaction, &owner)?;
    state_transaction.world.runtime_obligations.insert(
        id,
        ObligationRecordV1 {
            owner,
            contract: instruction.contract.clone(),
            kind: instruction.kind,
            created_at: height,
            expires_ms: instruction.expires_ms,
            capture,
            terminal_effects: committed,
        },
    );
    Ok(())
}

/// End the obligation an owner holds and release its completion closure (§7.2).
///
/// The owner ends its own obligation. The account that controls the lifecycle of the contract
/// that owns an obligation can also end it. A selected contract kernel call uses the authenticated
/// contract subject directly. Anyone ends an
/// obligation that is no longer live, so a record no capability remains behind does not stay
/// in State.
///
/// # Errors
/// The owner holds no obligation, or `authority` may not end it.
pub(crate) fn complete(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &CompleteRuntimeObligationV1,
) -> Result<(), Error> {
    let (id, record) = held(state_transaction, &instruction.owner)?;
    let record =
        record.ok_or_else(|| refused(format_args!("{} holds no obligation", instruction.owner)))?;
    let entitled = record.owner == *authority
        || !record.is_live(block_ms(state_transaction))
        || record.contract.as_ref().is_some_and(|contract| {
            contract_owner(&state_transaction.world, contract)
                .is_ok_and(|(controller, _)| controller == *authority)
        });
    if !entitled {
        return Err(refused("only its owner ends a live obligation"));
    }
    state_transaction.world.runtime_obligations.remove(id);
    Ok(())
}

/// Run `execute` for `instruction`, which `authority` issues. If the instruction is a committed
/// terminal effect of the live obligation `authority` holds and is still to be performed,
/// execute it as a completion and consume the capability only on success (§7.2).
///
/// A caught instruction rejection cannot consume the pending effect. The record remains live
/// during execution, including checks against executor replacement. If the callback explicitly
/// replaces or ends the registration, completing the old effect never resurrects it.
/// A root that holds no obligation pays one emptiness check.
///
/// # Errors
/// What `execute` returns.
pub(crate) fn performing<T>(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
    execute: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<T, ValidationFail>,
) -> Result<T, ValidationFail> {
    if state_transaction.completing_obligation.is_some() {
        return execute(state_transaction);
    }
    let Some((id, mut record, effect)) = pending_effect(state_transaction, authority, instruction)
    else {
        return execute(state_transaction);
    };
    let selection = record
        .capture
        .execution_selection(
            state_transaction.world.runtime_selection.canonical(),
            &PublishedDefinitions(&state_transaction.world.runtime_definitions),
        )
        .map_err(|error| ValidationFail::InstructionFailed(refused(error)))?;
    // A user registration cannot replace a native obligation's authoritative custody context.
    // Matching consent may finish the registered effect, but native progression keeps
    // its own Step (acceptance never becomes completion through this registration).
    if state_transaction.native_execution.is_some()
        && state_transaction.world.runtime_selection.get() != &selection
    {
        return Err(ValidationFail::InstructionFailed(refused(
            "registered terminal context disagrees with native obligation custody",
        )));
    }
    let previous_selection = state_transaction
        .world
        .runtime_selection
        .replace_executing(Some(selection));
    state_transaction.completing_obligation = Some(record.capture.clone());
    let result = {
        let scope = CompletionScope {
            transaction: state_transaction,
            previous_selection,
        };
        execute(&mut *scope.transaction)
    };
    if result.is_ok() && state_transaction.world.runtime_obligations.get(&id) == Some(&record) {
        record.terminal_effects[effect].performed = true;
        state_transaction
            .world
            .runtime_obligations
            .insert(id, record);
    }
    result
}

/// Whether the executing obligation captured the exact binding of `instruction`.
///
/// A completion capability covers its captured call graph, not every instruction a callback
/// might derive. An interface introduced outside that graph follows ordinary admission even
/// while a committed terminal effect is executing.
pub(crate) fn captures_executing(
    state_transaction: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> bool {
    // Deferred execution follows its own consent and ordinary restrictions. Neither captured
    // nor current-binding consent borrows an enclosing completion's admission exemption.
    if !state_transaction.deferred_expectations.is_empty()
        || state_transaction.native_execution.is_some()
    {
        return false;
    }
    let Some(capture) = &state_transaction.completing_obligation else {
        return false;
    };
    crate::runtime::execution::capture_contains_binding(state_transaction, capture, instruction)
}

/// Find the original live registration and pending effect without consuming it before execution.
fn pending_effect(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> Option<(ObligationId, ObligationRecordV1, usize)> {
    if state_transaction
        .world
        .runtime_obligations
        .iter()
        .next()
        .is_none()
    {
        return None;
    }
    let Ok((id, Some(record))) = held(state_transaction, authority) else {
        return None;
    };
    if record.owner != *authority || !record.is_live(block_ms(state_transaction)) {
        return None;
    }
    let Some(index) = terminal_effect_id(instruction)
        .ok()
        .and_then(|envelope| record.pending_effect(&envelope))
    else {
        return None;
    };
    Some((id, record, index))
}

/// The distinct completion closures of the obligations of `world` that are live at committed
/// time `now_ms`, with how many obligations share each.
pub(super) fn live_captures(
    world: &crate::state::WorldTransaction<'_, '_>,
    now_ms: u64,
) -> Vec<(CompletionCaptureV1, usize)> {
    let mut captures: Vec<(CompletionCaptureV1, usize)> = Vec::new();
    for (_, record) in world.runtime_obligations.iter() {
        if !record.is_live(now_ms) {
            continue;
        }
        match captures
            .iter_mut()
            .find(|(capture, _)| *capture == record.capture)
        {
            Some((_, count)) => *count += 1,
            None => captures.push((record.capture.clone(), 1)),
        }
    }
    captures
}

/// Why `successor` cannot retain the original execution context of a registered obligation.
/// Binding replacement is allowed while the root retains the captured definitions and profiles.
pub(super) fn closure_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl DefinitionLookup,
    captures: &[(CompletionCaptureV1, usize)],
) -> Option<String> {
    captures.iter().find_map(|(capture, count)| {
        let context = match capture.execution_selection(successor, published) {
            Ok(context) => context,
            Err(error) => return Some(format!("{count} live registered obligations: {error}")),
        };
        let reason = match capture.broken_by(predecessor, &context, published) {
            Ok(reason) => reason,
            Err(error) => Some(error.to_string()),
        }?;
        Some(format!("{count} live registered obligations: {reason}"))
    })
}

#[cfg(test)]
mod tests;

/// Register new work owned by the exact active deployed contract of an opaque invocation.
/// No caller-owned address, controller substitution or transaction-authority fallback is used.
pub(crate) fn register_contract(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1,
    context: Option<&crate::executor::ContractRuntimeExecutionContext>,
) -> Result<(), Error> {
    use crate::{smartcontracts::code, state::WorldReadOnly as _};
    use iroha_data_model::smart_contract::ContractArtifactId;
    let context = context.ok_or_else(|| {
        refused("contract registration requires authenticated invocation context")
    })?;
    let binding = code::fetch_contract_lifecycle_binding(&tx.world, &context.contract_address)
        .map_err(refused)?
        .ok_or_else(|| refused("contract has no lifecycle binding"))?;
    if context.contract_subject != *authority
        || binding.subject != *authority
        || context.contract_address.subject_id() != *authority
        || tx.world.contract_subject_addresses().get(authority) != Some(&context.contract_address)
    {
        return Err(refused(
            "contract invocation subject disagrees with the active binding",
        ));
    }
    let code_hash = binding
        .lifecycle
        .active_code_hash
        .ok_or_else(|| refused("only an active deployed contract registers an obligation"))?;
    if code::pending_contract_lifecycle(&tx.world, &context.contract_address)
        .map_err(refused)?
        .is_some()
    {
        return Err(refused(
            "pending lifecycle hooks cannot register contract obligations",
        ));
    }
    let artifact =
        ContractArtifactId::for_address(&context.contract_address, code_hash).map_err(refused)?;
    let expected = iroha_crypto::Hash::prehashed(ivm::syscalls::compute_abi_hash(
        ivm::SyscallPolicy::AbiV1RuntimeLifecycle,
    ));
    let manifest = tx
        .world
        .contract_manifests()
        .get(&artifact)
        .ok_or_else(|| refused("active contract manifest is absent"))?;
    if manifest.code_hash != Some(code_hash) || manifest.abi_hash != Some(expected) {
        return Err(refused(
            "active contract manifest does not name the contract lifecycle host profile",
        ));
    }
    let now_ms = block_ms(tx);
    let height = tx._curr_block.height().get();
    let canonical = tx.world.runtime_selection.canonical();
    if tx.world.runtime_selection.get() != canonical {
        return Err(refused(
            "contract registration is new work and cannot borrow a retained execution scope",
        ));
    }
    let wire =
        iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1::WIRE_ID;
    let binding = canonical
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .ok_or_else(|| refused("contract registration is not selected"))?;
    for id in [binding.interface, binding.implementation] {
        if super::super::blocking_restriction(canonical, &id, height).is_some() {
            return Err(refused("contract registration requires ordinary admission"));
        }
    }
    check_registration(instruction.kind, instruction.expires_ms, now_ms).map_err(refused)?;
    let (committed, wire_ids) = terminal_effects(&instruction.terminal_effects).map_err(refused)?;
    let lookup = PublishedDefinitions(&tx.world.runtime_definitions);
    let canonical = tx.world.runtime_selection.canonical();
    let capture =
        CompletionCaptureV1::capture(canonical, &lookup, &wire_ids, height).map_err(refused)?;
    crate::smartcontracts::ivm::host_profile::ensure_new_capture(tx.world(), &capture, height)?;
    let expected_execution = capture
        .execution_selection(canonical, &lookup)
        .map_err(refused)?;
    if (
        expected_execution.kernel,
        expected_execution.commitment_profile,
        expected_execution.services_profile,
    ) != (
        tx.world.runtime_selection.get().kernel,
        tx.world.runtime_selection.get().commitment_profile,
        tx.world.runtime_selection.get().services_profile,
    ) || capture
        .broken_by(
            &expected_execution,
            tx.world.runtime_selection.get(),
            &lookup,
        )
        .map_err(refused)?
        .is_some()
    {
        return Err(refused(
            "new contract work must execute the canonical root's captured bindings",
        ));
    }
    let (id, _) = held(tx, authority)?;
    tx.world.runtime_obligations.insert(
        id,
        ObligationRecordV1 {
            owner: authority.clone(),
            contract: Some(context.contract_address.clone()),
            kind: instruction.kind,
            created_at: height,
            expires_ms: instruction.expires_ms,
            capture,
            terminal_effects: committed,
        },
    );
    Ok(())
}

#[cfg(test)]
mod contract_tests;
