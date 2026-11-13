//! Native obligation scopes preserve the recorded implementation and the operation's own step.

use iroha_data_model::{
    ValidationFail,
    isi::{InstructionBox, error::InstructionExecutionError},
    runtime::definition::RootSelectionV1,
};

use super::NativeExecution;

use crate::{
    runtime::{
        change_set::PublishedDefinitions, native_amx_capture, native_escrow_capture,
        native_game_capture, obligations::Step,
    },
    state::StateTransaction,
};

/// The exact previous scope survives a nested call, a rejection and an unwind.
struct NativeScope<'transaction, 'block, 'world> {
    transaction: &'transaction mut StateTransaction<'block, 'world>,
    previous_execution: Option<NativeExecution>,
    previous_selection: Option<RootSelectionV1>,
}

impl Drop for NativeScope<'_, '_, '_> {
    fn drop(&mut self) {
        self.transaction.native_execution = self.previous_execution.take();
        self.transaction
            .world
            .runtime_selection
            .set_executing(self.previous_selection.take());
    }
}

fn refused(reason: impl core::fmt::Display) -> ValidationFail {
    ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(
        format!("native obligation execution: {reason}").into(),
    ))
}

/// Select the authoritative native context before permissions or a registered terminal effect.
/// A root without the selected capture service follows its original execution path.
pub(crate) fn with_native_obligation<T>(
    transaction: &mut StateTransaction<'_, '_>,
    instruction: &InstructionBox,
    execute: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<T, ValidationFail>,
) -> Result<T, ValidationFail> {
    let native = native_escrow_capture::prepare_execution(transaction, instruction)
        .map_err(ValidationFail::InstructionFailed)?;
    let native = match native {
        Some(native) => Some(native),
        None => native_amx_capture::prepare_execution(transaction, instruction)
            .map_err(ValidationFail::InstructionFailed)?,
    };
    let native = match native {
        Some(native) => Some(native),
        None => native_game_capture::prepare_execution(transaction, instruction)
            .map_err(ValidationFail::InstructionFailed)?,
    };
    match native {
        Some(native) => enter_native(transaction, native, execute),
        None => execute(transaction),
    }
}

fn enter_native<T>(
    transaction: &mut StateTransaction<'_, '_>,
    native: NativeExecution,
    execute: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<T, ValidationFail>,
) -> Result<T, ValidationFail> {
    let selection = native
        .capture
        .execution_selection(
            transaction.world.runtime_selection.canonical(),
            &PublishedDefinitions(&transaction.world.runtime_definitions),
        )
        .map_err(refused)?;
    if (transaction.completing_obligation.is_some() || transaction.native_execution.is_some())
        && transaction.world.runtime_selection.get() != &selection
    {
        return Err(refused(
            "enclosing captured execution disagrees with the native obligation's original closure",
        ));
    }
    let previous_selection = transaction
        .world
        .runtime_selection
        .replace_executing(Some(selection));
    let previous_execution = transaction.native_execution.replace(native);
    let scope = NativeScope {
        transaction,
        previous_execution,
        previous_selection,
    };
    execute(&mut *scope.transaction)
}

/// Admission authority follows the native operation, including `Continue` for acceptance.
/// An enclosing deferred body never borrows this scope's restriction exemption.
pub(crate) fn native_step(
    transaction: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Option<Step> {
    if !transaction.deferred_expectations.is_empty() {
        return None;
    }
    let native = transaction.native_execution.as_ref()?;
    super::capture_contains_binding(transaction, &native.capture, instruction)
        .then_some(native.step)
}

#[cfg(test)]
mod tests;
