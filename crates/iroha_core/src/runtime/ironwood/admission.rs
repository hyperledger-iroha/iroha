//! Selection-aware predecessor payload grammar checks before inclusion or durable proposals.

use iroha_data_model::{
    isi::{InstructionBox, Register, RegisterBox, privacy::*},
    runtime::definition::RootSelectionV1,
    transaction::{Executable, ExecutableBatchItem, SignedTransaction},
    trigger::Trigger,
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use mv::storage::StorageReadOnly as _;

use crate::execution_attempt::{ExecutionAttemptError as Attempt, canonical_decode_attempt_error};

/// Dedicated additive wire identities; frozen baseline instructions cannot carry Ironwood.
pub(crate) fn direct(instruction: &InstructionBox) -> bool {
    let any = instruction.as_any();
    any.is::<BootstrapPrivacyIronwoodPoolV1>()
        || any.is::<SubmitPrivacyIronwoodProofV1>()
        || any.is::<SchedulePrivacyIronwoodLimitsUpdateV1>()
}

/// Whether a typed binary carrier uses the successor grammar. Opaque Custom JSON is not
/// decoded here: the predecessor admitted those bytes and classified them during execution.
pub(crate) fn instruction_requires_ironwood(value: &InstructionBox) -> bool {
    direct(value)
        || value.as_any().downcast_ref::<RegisterBox>().is_some_and(|register| {
            matches!(register, RegisterBox::Trigger(register) if executable_requires_ironwood(register.object.action().executable()))
        })
        || value.as_any().downcast_ref::<Register<Trigger>>().is_some_and(|register| {
            executable_requires_ironwood(register.object.action().executable())
        })
}

/// Inspect only typed instruction carriers, retaining opaque bytecode and Custom JSON boundaries.
pub(crate) fn executable_requires_ironwood(value: &Executable) -> bool {
    match value {
        Executable::Instructions(values) => values.iter().any(instruction_requires_ironwood),
        Executable::IvmProved(proved) => proved.overlay.iter().any(instruction_requires_ironwood),
        Executable::Batch(values) => values.iter().any(|value| match value {
            ExecutableBatchItem::Instruction(value) => instruction_requires_ironwood(value),
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
    }
}

/// Refuse the same typed bytes that the predecessor codec cannot interpret before inclusion.
pub(crate) fn instruction(
    value: &InstructionBox,
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    if instruction_requires_ironwood(value) && !super::enabled(selection) {
        return Err(Attempt::Rejected(format!(
            "inactive Ironwood payload grammar: predecessor decoder refuses {} (additive Ironwood instruction wire ID)",
            iroha_data_model::isi::instruction_wire_id(value).unwrap_or("native instruction")
        )));
    }
    Ok(())
}

/// Classify an already parsed opaque native instruction using the selected JSON grammar.
/// A predecessor parser would not recognize a proposal containing newly typed instructions.
/// Nested Custom JSON stays opaque until its own native execution/classification boundary.
pub(crate) fn multisig_for_selection(
    value: MultisigInstructionBox,
    selection: &RootSelectionV1,
) -> Result<MultisigInstructionBox, norito::Error> {
    if !super::enabled(selection)
        && matches!(&value, MultisigInstructionBox::Propose(proposal)
            if proposal.instructions.iter().any(instruction_requires_ironwood))
    {
        return Err(norito::Error::Message(
            "unknown instruction grammar in multisig proposal".into(),
        ));
    }
    Ok(value)
}

/// Decode opaque native JSON with the selected grammar, retaining original decoder errors.
pub(crate) fn multisig_instruction(
    value: &InstructionBox,
    selection: &RootSelectionV1,
) -> Result<MultisigInstructionBox, norito::Error> {
    multisig_for_selection(MultisigInstructionBox::try_from(value)?, selection)
}

/// Validate an ordered instruction carrier, retaining its original local retry owner.
pub(crate) fn instructions(
    values: &[InstructionBox],
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    values
        .iter()
        .try_for_each(|value| instruction(value, selection))
}

/// Check all typed instruction carriers, including proved overlays and stored trigger bodies.
pub(crate) fn executable(
    value: &Executable,
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    match value {
        Executable::Instructions(values) => instructions(values, selection),
        Executable::IvmProved(proved) => instructions(&proved.overlay, selection),
        Executable::Batch(values) => values.iter().try_for_each(|value| match value {
            ExecutableBatchItem::Instruction(value) => instruction(value, selection),
            ExecutableBatchItem::ContractCall(_) => Ok(()),
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => Ok(()),
    }
}

/// Check the signed transaction before queue insertion and before block execution/results.
pub(crate) fn transaction(
    value: &SignedTransaction,
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    executable(value.instructions(), selection)
}

/// Inspect native proposal carriers without reinterpreting unrelated or malformed opaque bytes.
/// Only a successful typed decode can reveal newly inadmissible instructions; a local decode
/// refusal retains its original retry owner instead of becoming a semantic rejection.
pub(crate) fn proposal_state_row(
    key: &iroha_model_base::state_path::StatePath,
    bytes: &[u8],
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    use iroha_executor_data_model::isi::multisig::{
        MultisigProposalState, MultisigProposalTerminalExecutionStateV1,
        MultisigProposalTerminalState,
    };
    if super::enabled(selection) {
        return Ok(());
    }
    macro_rules! inspect {
        ($ty:ty, $value:ident, $instructions:expr) => {
            match norito::core::classify_decode_attempt(|| norito::decode_from_bytes::<$ty>(bytes))
                .map_err(|error| canonical_decode_attempt_error(error, |_| ()))
            {
                Ok($value) => instructions($instructions, selection),
                Err(Attempt::Deferred(original)) => Err(Attempt::Deferred(original)),
                Err(Attempt::Rejected(())) => Ok(()),
            }
        };
    }
    if key.as_ref().starts_with("multisig/proposal/") {
        inspect!(MultisigProposalState, value, &value.instructions)
    } else if key.as_ref().starts_with("multisig/proposal-terminal/") {
        inspect!(
            MultisigProposalTerminalState,
            value,
            &value.proposal.instructions
        )
    } else if key
        .as_ref()
        .starts_with("multisig/proposal-terminal-execution/")
    {
        inspect!(
            MultisigProposalTerminalExecutionStateV1,
            value,
            &value.terminal.proposal.instructions
        )
    } else {
        Ok(())
    }
}

/// Validate the complete retained durable overlay before any queued effect or state write.
/// Contract-scoped paths remain opaque; only exact native proposal namespaces are inspected.
pub(crate) fn durable_state_overlay(
    values: &std::collections::BTreeMap<iroha_model_base::state_path::StatePath, Option<Vec<u8>>>,
    selection: &RootSelectionV1,
) -> Result<(), Attempt<String>> {
    if super::enabled(selection) {
        return Ok(());
    }
    values.iter().try_for_each(|(key, value)| match value {
        Some(bytes) => proposal_state_row(key, bytes, selection),
        None => Ok(()),
    })
}

/// Validate original current and retained undo proposal bytes before publishing a restored World.
/// Native proposal keys are authoritative; arbitrary unrelated byte strings are never sniffed.
pub(crate) fn world(value: &mut crate::state::World) -> Result<(), Attempt<String>> {
    let selection = value.runtime_selection.view().get().clone();
    super::validate_cutover(
        &selection,
        &selection,
        &crate::runtime::change_set::PublishedDefinitions(&value.runtime_definitions.view()),
    )
    .map_err(Attempt::Rejected)?;
    if super::enabled(&selection) {
        return Ok(());
    }
    value
        .triggers
        .validate_privacy_payload_grammar(&selection)?;
    let history = value.smart_contract_state.history();
    for (key, bytes) in history
        .current()
        .iter()
        .map(|(key, value)| (key, Some(value)))
        .chain(
            history
                .revert_map()
                .iter()
                .map(|(key, value)| (key, value.as_ref())),
        )
    {
        if let Some(bytes) = bytes {
            proposal_state_row(key, bytes, &selection)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    include!("admission_tests.rs");
}
