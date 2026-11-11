//! The kernel's obligation registry (`specs/live_runtime_upgrades.md` §7.2): registration,
//! completion, and what a restriction or a change set must leave to a registered obligation.
//!
//! `world.runtime_obligations` holds the one [`ObligationRecordV1`] each owner holds: an account
//! that registered for itself, or the subject account of a contract whose lifecycle controller
//! registered for it. A registration commits to the exact instructions that complete the
//! obligation and captures their completion closure from the selection the root executes.
//!
//! - **Completion.** When the owner issues one of the committed instructions, the kernel marks
//!   it performed and executes it as a completion of the obligation: admission closure and
//!   completion-only on the definitions behind it, or behind what it invokes, do not stop it
//!   ([`performing`], [`super::Step`]). Each committed instruction lends that capability once.
//!   A contract has no kernel call under the first-release ABI, and needs none: the committed
//!   instructions it queues under its subject account complete the obligation registered for
//!   it in the same way.
//! - **Activation.** While an obligation is live, a change set that stops executing a captured
//!   definition or rebinds an interface its closure reaches lapses ([`closure_gap`]).
//!
//! A release answers a wire ID from the selection it executes, so a completion runs the code the
//! root binds now, and keeping a registered completion as it was means keeping its bindings.
//! TODO(§7.2): run a captured implementation beside its replacement, so a change set may rebind
//! what a live obligation reaches. That needs one handler per implementation and wire ID, and an
//! execution context every selection-dependent switch (services, authorization, storage
//! contracts) reads instead of the root's current selection.
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
/// that owns an obligation ends it, because the contract itself cannot. Anyone ends an
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
/// terminal effect of the live obligation `authority` holds and is still to be performed, mark
/// it performed and execute it as a completion of that obligation (§7.2).
///
/// A failing instruction fails its transaction, whose rollback restores the effect as pending.
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
    if state_transaction.completing_obligation
        || !perform(state_transaction, authority, instruction)
    {
        return execute(state_transaction);
    }
    state_transaction.completing_obligation = true;
    let result = execute(state_transaction);
    state_transaction.completing_obligation = false;
    result
}

/// Mark `instruction` performed if it is a pending terminal effect of the live obligation
/// `authority` holds.
fn perform(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> bool {
    if state_transaction
        .world
        .runtime_obligations
        .iter()
        .next()
        .is_none()
    {
        return false;
    }
    let Ok((id, Some(mut record))) = held(state_transaction, authority) else {
        return false;
    };
    if record.owner != *authority || !record.is_live(block_ms(state_transaction)) {
        return false;
    }
    let Some(index) = terminal_effect_id(instruction)
        .ok()
        .and_then(|envelope| record.pending_effect(&envelope))
    else {
        return false;
    };
    record.terminal_effects[index].performed = true;
    state_transaction
        .world
        .runtime_obligations
        .insert(id, record);
    true
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

/// Why `successor` would strand a live registered obligation, if it would: it stops executing a
/// captured definition that `predecessor` executes, or rebinds an interface a captured closure
/// reaches ([`CompletionCaptureV1::broken_by`]).
pub(super) fn closure_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl DefinitionLookup,
    captures: &[(CompletionCaptureV1, usize)],
) -> Option<String> {
    captures.iter().find_map(|(capture, count)| {
        let reason = match capture.broken_by(predecessor, successor, published) {
            Ok(reason) => reason,
            Err(error) => Some(error.to_string()),
        }?;
        Some(format!("{count} live registered obligations: {reason}"))
    })
}

#[cfg(test)]
mod tests;
