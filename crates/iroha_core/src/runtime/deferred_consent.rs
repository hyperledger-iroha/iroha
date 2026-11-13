//! Selected deferred-consent service: immutable captures and execution-time enforcement.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    isi::{InstructionBox, error::InstructionExecutionError as Error},
    runtime::{
        deferred::{
            DeferredConsentKeyV1 as Key, DeferredConsentRecordV1 as Record,
            DeferredConsentV1 as Consent, SERVICE, SERVICE_CLOSURE,
        },
        definition::RootSelectionV1,
        lifecycle::{ExpectationV1, RuntimeRejectionV1},
    },
    trigger::TriggerId,
};
use mv::storage::StorageReadOnly as _;

use crate::{
    smartcontracts::isi::triggers::set::{ExecutableRef, SetReadOnly as _},
    state::{StateTransaction, WorldReadOnly},
};

/// Whether the exact compiled service is selected; an unsupported profile fails closed.
pub(crate) fn enabled(selection: &RootSelectionV1) -> Result<bool, String> {
    super::release_catalog()
        .map_err(|error| error.to_string())?
        .service_enabled(
            selection.services_profile,
            SERVICE,
            Hash::new(SERVICE_CLOSURE).into(),
        )
        .map_err(|error| error.to_string())
}

/// Reject an explicit opt-in before its handler can mutate an inactive root.
pub(crate) fn require_enabled(tx: &StateTransaction<'_, '_>) -> Result<(), Error> {
    if enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        require_current_execution(tx)
    } else {
        Err(refused("current-binding consent service is inactive"))
    }
}

// Explicit current-binding consent cannot inherit a different implementation from an
// enclosing obligation's retained completion context. Restrictions remain ordinary policy.
fn require_current_execution(tx: &StateTransaction<'_, '_>) -> Result<(), Error> {
    let canonical = tx.world.runtime_selection.canonical();
    let executing = tx.world.runtime_selection.get();
    if core::ptr::eq(canonical, executing) {
        return Ok(());
    }
    let host_profiles =
        |selection: &RootSelectionV1| {
            selection
                .active
                .iter()
                .copied()
                .filter(|id| {
                    matches!(
            tx.world.runtime_definitions.get(id),
            Some(iroha_data_model::runtime::definition::DefinitionDescriptorV1::HostProfile(_))
        )
                })
                .collect::<Vec<_>>()
        };
    if (
        canonical.kernel,
        canonical.commitment_profile,
        canonical.services_profile,
    ) != (
        executing.kernel,
        executing.commitment_profile,
        executing.services_profile,
    ) || canonical.bindings != executing.bindings
        || host_profiles(canonical) != host_profiles(executing)
    {
        return Err(missing());
    }
    Ok(())
}

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("deferred consent: {reason}").into())
}

fn missing() -> Error {
    RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.into()
}

/// The immutable native occurrence key, without a dependency from the data model onto the executor.
pub(crate) fn multisig_key(owner: &AccountId, proposal: &[u8; 32]) -> Key {
    Key::Multisig {
        owner: owner.clone(),
        proposal: *proposal,
    }
}

fn capture(
    tx: &StateTransaction<'_, '_>,
    wires: Vec<String>,
    opaque: bool,
) -> Result<Consent, Error> {
    let expectation = ExpectationV1::capture(
        tx.world.runtime_selection.canonical(),
        &super::change_set::PublishedDefinitions(&tx.world.runtime_definitions),
        &wires,
        opaque,
    )
    .map_err(refused)?;
    expectation
        .check_at(tx.world.runtime_selection.canonical(), tx.block_height())
        .map_err(Error::from)?;
    expectation
        .check_at(tx.world.runtime_selection.get(), tx.block_height())
        .map_err(Error::from)?;
    Ok(Consent::Captured(expectation))
}

fn instruction_wires(instructions: &[InstructionBox]) -> Result<Vec<String>, Error> {
    instructions
        .iter()
        .map(|instruction| {
            iroha_data_model::isi::instruction_wire_id(instruction)
                .map(str::to_owned)
                .ok_or_else(|| refused("deferred instruction has no wire identity"))
        })
        .collect()
}

/// Prepare consent before either proposal or consent State is mutated.
pub(crate) fn prepare_multisig(
    tx: &StateTransaction<'_, '_>,
    owner: &AccountId,
    instructions: &[InstructionBox],
    current_bindings: bool,
) -> Result<Option<Record>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    let consent = if current_bindings {
        require_current_execution(tx)?;
        Consent::CurrentBindings
    } else {
        capture(tx, instruction_wires(instructions)?, false)?
    };
    let executable = *HashOf::new(&instructions.to_vec()).as_ref();
    Ok(Some(Record {
        authority: owner.clone(),
        executable,
        consent,
    }))
}

fn executable_identity(executable: &ExecutableRef) -> Result<([u8; 32], Vec<String>, bool), Error> {
    let (tag, bytes, wires, opaque) = match executable {
        ExecutableRef::Instructions(instructions) => (
            0u8,
            norito::encode_canonical(instructions).map_err(refused)?,
            instruction_wires(instructions)?,
            false,
        ),
        ExecutableRef::Ivm(hash) => (
            1,
            norito::encode_canonical(hash).map_err(refused)?,
            Vec::new(),
            true,
        ),
        ExecutableRef::ContractCall(call) => (
            2,
            norito::encode_canonical(call).map_err(refused)?,
            Vec::new(),
            true,
        ),
        ExecutableRef::Batch(items) => {
            let mut instructions = Vec::new();
            let mut opaque = false;
            for item in items.iter() {
                match item {
                    iroha_data_model::transaction::executable::ExecutableBatchItem::Instruction(instruction) => instructions.push(instruction.clone()),
                    iroha_data_model::transaction::executable::ExecutableBatchItem::ContractCall(_) => opaque = true,
                }
            }
            (
                3,
                norito::encode_canonical(items).map_err(refused)?,
                instruction_wires(&instructions)?,
                opaque,
            )
        }
    };
    Ok((
        Hash::new_from_chunks(&[b"iroha:runtime:deferred-executable:v1\0", &[tag], &bytes]).into(),
        wires,
        opaque,
    ))
}

/// Prepare the exact loaded executable identity before registering any trigger State.
pub(crate) fn prepare_trigger(
    tx: &StateTransaction<'_, '_>,
    trigger: &iroha_data_model::trigger::Trigger,
    current_bindings: bool,
) -> Result<Option<Record>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    use iroha_data_model::transaction::Executable;
    let executable = match trigger.action().executable() {
        Executable::Instructions(instructions) => ExecutableRef::Instructions(instructions.clone()),
        Executable::Ivm(bytecode) => ExecutableRef::Ivm(HashOf::new(bytecode)),
        Executable::ContractCall(call) => ExecutableRef::ContractCall(call.clone()),
        Executable::Batch(items) => ExecutableRef::Batch(items.clone()),
        Executable::IvmProved(_) => return Err(refused("proof-backed triggers are unavailable")),
    };
    let (executable, wires, opaque) = executable_identity(&executable)?;
    let consent = if current_bindings {
        require_current_execution(tx)?;
        Consent::CurrentBindings
    } else {
        capture(tx, wires, opaque)?
    };
    Ok(Some(Record {
        authority: trigger.action().authority().clone(),
        executable,
        consent,
    }))
}

fn checked_record(
    world: &impl WorldReadOnly,
    selection: &RootSelectionV1,
    key: &Key,
    authority: &AccountId,
    executable: [u8; 32],
    height: u64,
) -> Result<Option<ExpectationV1>, Error> {
    if !enabled(selection).map_err(refused)? {
        return Ok(None);
    }
    let record = world
        .runtime_deferred_consents()
        .get(key)
        .ok_or_else(missing)?;
    if &record.authority != authority || record.executable != executable {
        return Err(missing());
    }
    match &record.consent {
        Consent::CurrentBindings => Ok(None),
        Consent::Captured(expectation) => {
            expectation
                .check_at(selection, height)
                .map_err(Error::from)?;
            Ok(Some(expectation.clone()))
        }
    }
}

fn checked_transaction_record(
    tx: &StateTransaction<'_, '_>,
    key: &Key,
    authority: &AccountId,
    executable: [u8; 32],
) -> Result<Option<ExpectationV1>, Error> {
    let canonical = tx.world.runtime_selection.canonical();
    let expectation = checked_record(
        &*tx.world,
        canonical,
        key,
        authority,
        executable,
        tx.block_height(),
    )?;
    if let Some(capture) = &expectation
        && !core::ptr::eq(canonical, tx.world.runtime_selection.get())
    {
        capture
            .check_at(tx.world.runtime_selection.get(), tx.block_height())
            .map_err(Error::from)?;
    } else if enabled(canonical).map_err(refused)? {
        require_current_execution(tx)?;
    }
    Ok(expectation)
}

/// Check every approval before adding its signer; expired occurrences remain cancellable.
pub(crate) fn check_multisig(
    tx: &StateTransaction<'_, '_>,
    owner: &AccountId,
    proposal: &[u8; 32],
    instructions: &[InstructionBox],
) -> Result<Option<ExpectationV1>, Error> {
    checked_transaction_record(
        tx,
        &multisig_key(owner, proposal),
        owner,
        *HashOf::new(&instructions.to_vec()).as_ref(),
    )
}

/// Whether the current trigger incarnation is eligible to fire; mismatches suspend it.
pub(crate) fn trigger_ready(world: &impl WorldReadOnly, id: &TriggerId, height: u64) -> bool {
    if matches!(enabled(world.runtime_selection()), Ok(false)) {
        return true;
    }
    world
        .triggers()
        .inspect_by_id(id, |action| {
            executable_identity(action.executable())
                .and_then(|(digest, _, _)| {
                    checked_record(
                        world,
                        world.runtime_selection(),
                        &Key::Trigger(id.clone()),
                        action.authority(),
                        digest,
                        height,
                    )
                })
                .is_ok()
        })
        .unwrap_or(false)
}

/// A data/by-call candidate must also match canonical consent inside a completion context.
pub(crate) fn trigger_ready_in_transaction(tx: &StateTransaction<'_, '_>, id: &TriggerId) -> bool {
    if matches!(enabled(tx.world.runtime_selection.canonical()), Ok(false)) {
        return true;
    }
    tx.world
        .triggers
        .inspect_by_id(id, |action| {
            trigger_expectation(tx, id, action.authority(), action.executable()).is_ok()
        })
        .unwrap_or(false)
}

/// Check the trigger again immediately before its body and acquire its dynamic-call expectation.
pub(crate) fn trigger_expectation(
    tx: &StateTransaction<'_, '_>,
    id: &TriggerId,
    authority: &AccountId,
    executable: &ExecutableRef,
) -> Result<Option<ExpectationV1>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    let (digest, _, _) = executable_identity(executable)?;
    checked_transaction_record(tx, &Key::Trigger(id.clone()), authority, digest)
}

/// Every enclosing capture constrains every dynamic call, including nested deferred work.
pub(crate) fn check_call(
    tx: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<(), Error> {
    if tx.deferred_expectations.iter().any(Option::is_none) {
        require_current_execution(tx)?;
    }
    let Some(wire) = iroha_data_model::isi::instruction_wire_id(instruction) else {
        return Ok(());
    };
    for expectation in tx.deferred_expectations.iter().flatten() {
        expectation
            .check_call(
                tx.world.runtime_selection.canonical(),
                wire,
                tx.block_height(),
            )
            .map_err(Error::from)?;
        if !core::ptr::eq(
            tx.world.runtime_selection.canonical(),
            tx.world.runtime_selection.get(),
        ) {
            expectation
                .check_call(tx.world.runtime_selection.get(), wire, tx.block_height())
                .map_err(Error::from)?;
        }
    }
    Ok(())
}

/// Hold every selected deferred scope, including explicit current-binding execution. Its
/// marker prevents borrowing an enclosing obligation's completion exemption and unwinds on
/// errors or caught panics. Inactive roots retain their original execution behavior.
pub(crate) fn with_scope<T>(
    tx: &mut StateTransaction<'_, '_>,
    expectation: Option<ExpectationV1>,
    execute: impl FnOnce(&mut StateTransaction<'_, '_>) -> T,
) -> T {
    struct Scope<'a, 'block, 'world> {
        tx: &'a mut StateTransaction<'block, 'world>,
        depth: usize,
    }
    impl Drop for Scope<'_, '_, '_> {
        fn drop(&mut self) {
            self.tx.deferred_expectations.truncate(self.depth);
        }
    }
    let depth = tx.deferred_expectations.len();
    if matches!(enabled(tx.world.runtime_selection.canonical()), Ok(true)) {
        tx.deferred_expectations.push(expectation);
    }
    let scope = Scope { tx, depth };
    execute(&mut *scope.tx)
}

/// No activation infers consent for existing deferred work, and retirement cannot discard it.
pub(crate) fn check_transition(
    world: &impl WorldReadOnly,
    before: &RootSelectionV1,
    after: &RootSelectionV1,
) -> Result<(), String> {
    let before_enabled = enabled(before)?;
    let after_enabled = enabled(after)?;
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let storage = catalog
        .additions
        .iter()
        .find(|addition| addition.module == super::catalog::additions::DEFERRED_CONSENT.module)
        .and_then(|addition| addition.storage.get("world.runtime_deferred_consents"));
    let Some(storage) = storage else {
        return if before_enabled || after_enabled {
            Err("deferred-consent service has no compiled storage definition".into())
        } else {
            Ok(())
        };
    };
    let installed = after.installed_storage.binary_search(storage).is_ok();
    if after_enabled != installed {
        return Err(
            "deferred-consent service and its kernel-owned table must be selected together".into(),
        );
    }
    if before_enabled == after_enabled {
        return Ok(());
    }
    if world.triggers().ids().iter().next().is_some()
        || world
            .smart_contract_state()
            .iter()
            .any(|(key, _)| key.as_ref().starts_with("multisig/proposal/"))
    {
        return Err("deferred-consent service transition requires every trigger and pending multisig occurrence to drain".into());
    }
    Ok(())
}

/// Remove the consent of an ended occurrence; inactive roots never touch the new table.
pub(crate) fn remove(tx: &mut StateTransaction<'_, '_>, key: Key) {
    if matches!(enabled(tx.world.runtime_selection.canonical()), Ok(true)) {
        tx.world.runtime_deferred_consents.remove(key);
    }
}

#[cfg(test)]
mod tests;
