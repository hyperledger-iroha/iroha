//! Authenticate a scheduled action from its original signed registration onward.
//!
//! Time outputs have no ordinary transaction identity. Every occurrence is
//! selected from the complete certified block, and its persistent action digest
//! is rebuilt using the original registration plus preceding authenticated
//! retry results. The public consumer must separately admit the original
//! registration's business intent, Network route and fees.

use super::*;
use iroha_data_model::{
    block::{consensus::SumeragiRootScope, execution_output::ExecutionOutputV1},
    events::EventFilterBox,
    isi::{
        BurnBox, ExecuteTrigger, MintBox, RegisterBox, RemoveKeyValueBox, SetKeyValueBox,
        UnregisterBox,
    },
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, SumeragiFinalityVerifier,
        verify_checkpoint_page,
    },
};
use std::io::{self, Write};

#[path = "scheduled_lifecycle_activation.rs"]
mod activation;
#[path = "scheduled_lifecycle_policy.rs"]
mod activation_policy;
#[path = "scheduled_lifecycle_trace.rs"]
mod lifecycle_trace;

/// Share the exact catalog-derived merged fixture with the Ironwood receipt tests.
#[cfg(test)]
pub(crate) fn merged_selections_for_testing()
-> [iroha_data_model::runtime::definition::RootSelectionV1; 2] {
    activation::merged_selections_for_testing()
}

const MAX_PROOFS: usize = 4096;
const MAX_PROOF_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: usize = 68 * 1024 * 1024;
const MAX_PROJECTION_BYTES: usize = 32 * 1024 * 1024;

/// Compact references can reuse a large original Action many times. Bound the
/// original bytes visited by reconstructing its hashes, independently of trace
/// size; nine hashes cover a successful retry including its native progress debit.
struct LifecycleHashWork {
    source_bytes: usize,
    remaining: usize,
}
impl LifecycleHashWork {
    fn new(original_bytes: usize) -> Self {
        Self {
            // Registration normalizes bounded metadata entries not present in the
            // signed original; this exceeds their fixed canonical overhead.
            source_bytes: original_bytes.saturating_add(1024),
            remaining: 256 * 1024 * 1024,
        }
    }
    fn charge(&mut self, hashes: usize) -> PyResult<()> {
        match self
            .source_bytes
            .checked_mul(hashes)
            .and_then(|bytes| self.remaining.checked_sub(bytes))
        {
            Some(remaining) => {
                self.remaining = remaining;
                Ok(())
            }
            None => {
                self.remaining = 0;
                Err(refusal("scheduled lifecycle hash work exceeds its bound"))
            }
        }
    }
}

fn refusal(message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(message.to_string())
}

fn original_trigger(original: &SignedTransaction, id: &TriggerId) -> PyResult<Trigger> {
    let instructions: Vec<&InstructionBox> = match original.instructions() {
        Executable::Instructions(items) => items.iter().collect(),
        Executable::Batch(items) => items
            .iter()
            .filter_map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                ExecutableBatchItem::ContractCall(_) => None,
            })
            .collect(),
        _ => return Err(refusal("original registration needs native instructions")),
    };
    let selected: Vec<_> = instructions
        .into_iter()
        .filter_map(
            |instruction| match instruction.as_any().downcast_ref::<RegisterBox>()? {
                RegisterBox::Trigger(registration) if registration.object.id() == id => {
                    Some(registration.object.clone())
                }
                _ => None,
            },
        )
        .collect();
    if selected.len() != 1 {
        return Err(refusal(
            "original must register the selected trigger exactly once",
        ));
    }
    let trigger = selected.into_iter().next().expect("one registration");
    let action = trigger.action();
    if action.authority() != original.authority()
        || action.repeats() != Repeats::Exactly(1)
        || action.retry_policy().is_none()
        || !matches!(
            action.filter(),
            EventFilterBox::Time(TimeEventFilter(ExecutionTime::Schedule(_)))
        )
        || matches!(
            action.executable(),
            Executable::Ivm(_) | Executable::IvmProved(_)
        )
    {
        return Err(refusal(
            "scheduled observation requires an own-authority one-success retryable native action",
        ));
    }
    native_progress_declaration(action)?;
    Ok(trigger)
}

/// Only native operations whose complete lifecycle effects are visible here.
/// Arbitrary native dispatch (including multisig execution and executor upgrades)
/// needs an authenticated root-effect trace, just as a ContractCall does.
fn lifecycle_transparent_instruction(instruction: &InstructionBox) -> bool {
    use iroha_data_model::isi::{GrantBox, Log, RevokeBox, TransferBox};
    let any = instruction.as_any();
    any.is::<Log>() || any.is::<RegisterBox>()
        // Account/domain retirement can remove dependent triggers implicitly.
        || matches!(any.downcast_ref::<UnregisterBox>(), Some(UnregisterBox::Trigger(_)))
        || any.is::<SetKeyValueBox>() || any.is::<RemoveKeyValueBox>()
        || any.is::<MintBox>() || any.is::<BurnBox>() || any.is::<TransferBox>()
        || any.is::<GrantBox>() || any.is::<RevokeBox>()
}

fn check_lifecycle_instructions<'a>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
    original_root: bool,
    selected: &Trigger,
) -> PyResult<usize> {
    let id = selected.id();
    let mut registrations = 0;
    for instruction in instructions {
        let any = instruction.as_any();
        if let Some(RegisterBox::Trigger(reg)) = any.downcast_ref::<RegisterBox>()
            && reg.object.id() == id
        {
            if !original_root || &reg.object != selected {
                return Err(refusal(
                    "selected trigger has an unrelated or nested registration",
                ));
            }
            registrations += 1;
        }
        let changed = matches!(any.downcast_ref::<UnregisterBox>(), Some(UnregisterBox::Trigger(op)) if &op.object == id)
            || matches!(any.downcast_ref::<SetKeyValueBox>(), Some(SetKeyValueBox::Trigger(op)) if &op.object == id)
            || matches!(any.downcast_ref::<RemoveKeyValueBox>(), Some(RemoveKeyValueBox::Trigger(op)) if &op.object == id)
            || matches!(any.downcast_ref::<MintBox>(), Some(MintBox::TriggerRepetitions(op)) if &op.destination == id)
            || matches!(any.downcast_ref::<BurnBox>(), Some(BurnBox::TriggerRepetitions(op)) if &op.destination == id)
            || matches!(any.downcast_ref::<ExecuteTrigger>(), Some(op) if &op.trigger == id);
        if changed {
            return Err(refusal(
                "selected trigger lifecycle changed in the authenticated prefix",
            ));
        }
        if !lifecycle_transparent_instruction(instruction) {
            return Err(refusal(
                "scheduled lifecycle requires an authenticated native-effect trace for opaque instructions",
            ));
        }
    }
    Ok(registrations)
}

/// Native Network roots are in the authenticated signed input. Their successful
/// result contains only data callbacks, so even callback zero is never that root.
fn check_network_root_lifecycle(
    executable: &Executable,
    original_root: bool,
    selected: &Trigger,
) -> PyResult<usize> {
    match executable {
        Executable::Instructions(items) => {
            check_lifecycle_instructions(items.iter(), original_root, selected)
        }
        Executable::Batch(items) => {
            let mut registrations = 0;
            for item in items.iter() {
                let ExecutableBatchItem::Instruction(instruction) = item else {
                    return Err(refusal(
                        "scheduled lifecycle requires an authenticated native-effect trace for ContractCall batch items",
                    ));
                };
                registrations += check_lifecycle_instructions(
                    std::iter::once(instruction),
                    original_root,
                    selected,
                )?;
            }
            Ok(registrations)
        }
        _ => Err(refusal(
            "scheduled lifecycle requires an authenticated native-effect trace for opaque Network roots",
        )),
    }
}

/// Count the exact original applied registration and reject every later mutation.
/// Network inputs and successful callback traces have different root ownership.
/// Time/Pipeline results already contain their own root followed by callbacks;
/// none may register the selected trigger as the original Network transaction.
fn check_lifecycle(
    block: &SignedBlock,
    original_hash: HashOf<TransactionEntrypoint>,
    selected: &Trigger,
) -> PyResult<usize> {
    let mut registrations = 0;
    for output in block.execution_outputs() {
        let Ok(steps) = &output.result().0 else {
            continue; // Rejected roots and callbacks rolled back their lifecycle effects.
        };
        if let ExecutionOutputV1::Network(row) = output {
            let input = block
                .network_entrypoints()
                .nth(row.input_index as usize)
                .ok_or_else(|| refusal("applied Network output has no authenticated input"))?;
            let TransactionEntrypoint::External(signed) = input else {
                return Err(refusal(
                    "scheduled lifecycle requires an authenticated native-effect trace for opaque Network inputs",
                ));
            };
            registrations += check_network_root_lifecycle(
                signed.instructions(),
                input.hash() == original_hash,
                selected,
            )?;
        }
        for step in steps.iter() {
            registrations +=
                check_lifecycle_instructions(step.instructions.iter(), false, selected)?;
        }
    }
    Ok(registrations)
}

fn time_row(
    output: &ExecutionOutputV1,
) -> Option<&iroha_data_model::block::execution_output::TimeExecutionOutputV1> {
    match output {
        ExecutionOutputV1::Time(row) => Some(row),
        ExecutionOutputV1::TimeWithLifecycle(row) => Some(&row.execution),
        _ => None,
    }
}

/// The certified model distinguishes a real execution failure from a healthy
/// body rolled back only because its complete output exceeded capacity. Only
/// real failures apply native retry policy, even when their diagnostic is omitted.
fn scheduled_disposition(output: &ExecutionOutputV1) -> PyResult<&'static str> {
    use iroha_data_model::block::execution_output::TriggerFailureRootV1;
    let row =
        time_row(output).ok_or_else(|| refusal("scheduled disposition requires a Time output"))?;
    if row.result.is_ok() {
        if row.failure_root.is_some() {
            return Err(refusal("successful Time output retained a failure root"));
        }
        return Ok("applied");
    }
    match &row.failure_root {
        Some(TriggerFailureRootV1::OmittedByOutputLimit) => {
            if !output.is_output_limit_rejection() {
                return Err(refusal("invalid native Time output-capacity terminal"));
            }
            Ok("output_limit")
        }
        Some(TriggerFailureRootV1::OmittedAfterRejection) => {
            if !output.is_internal_rejection_diagnostic_omitted() {
                return Err(refusal("invalid omitted native Time rejection diagnostic"));
            }
            Ok("rejected")
        }
        Some(TriggerFailureRootV1::ReturnedBeforeRollback(_))
        | Some(TriggerFailureRootV1::DeclaredInstructionProjection(_)) => Ok("rejected"),
        None => Err(refusal(
            "rejected Time output has no native disposition root",
        )),
    }
}

fn frame<T: norito::NoritoSerialize>(value: &T, writer: &mut dyn Write) -> io::Result<()> {
    norito::core::write_canonical_to_writer(value, writer)
        .map_err(|error| io::Error::other(error.to_string()))
}

/// Validate the original registration-only autonomous attempt declaration.
fn native_progress_declaration(action: &TriggerAction) -> PyResult<Option<u32>> {
    if action.metadata.get("__native_progress_remaining").is_some() {
        return Err(refusal(
            "original registration supplies a native progress counter",
        ));
    }
    let Some(value) = action.metadata.get("native_progress_attempts") else {
        return Ok(None);
    };
    let count = value.try_into_any_norito::<u32>().map_err(refusal)?;
    if !(1..=1_024).contains(&count) {
        return Err(refusal(
            "native progress attempts must be between 1 and 1024",
        ));
    }
    Ok(Some(count))
}

/// Match Core's persistent action frame from the original registration and authenticated counters.
fn action_hash_with_progress(
    action: &TriggerAction,
    registration_height: u64,
    registration_ms: u64,
    retry: Option<(u32, u64)>,
    repeats: Repeats,
    lifecycle: bool,
    progress: Option<u32>,
) -> PyResult<Hash> {
    let EventFilterBox::Time(filter) = action.filter() else {
        return Err(refusal("expected Time action"));
    };
    let mut metadata = action.metadata.clone();
    match (native_progress_declaration(action)?, progress) {
        (Some(declared), Some(remaining)) if remaining <= declared => {
            metadata.insert(
                "__native_progress_remaining".parse().unwrap(),
                Json::new(remaining),
            );
        }
        (None, None) => {}
        _ => {
            return Err(refusal(
                "native progress counter differs from its registered allowance",
            ));
        }
    }
    metadata.insert(
        "__registered_block_height".parse().unwrap(),
        Json::from(registration_height),
    );
    metadata.insert(
        "__registered_at_ms".parse().unwrap(),
        Json::from(registration_ms),
    );
    Hash::new_from_writer(|writer| {
        writer.write_all(if lifecycle {
            b"iroha:trigger-lifecycle:action:v1\0"
        } else {
            b"iroha:trigger-use:action:v1\0"
        })?;
        writer.write_all(&[1])?;
        frame(action.authority(), writer)?;
        frame(filter, writer)?;
        frame(&repeats, writer)?;
        frame(&metadata, writer)?;
        frame(&action.retry_policy(), writer)?;
        frame(&retry, writer)?;
        if lifecycle {
            let executable = Hash::new_from_writer(|writer| {
                writer.write_all(b"iroha:trigger-lifecycle:executable:v1\0")?;
                write_selected_executable(action, writer)
            })?;
            frame(&executable, writer)?;
        } else {
            write_selected_executable(action, writer)?;
        }
        Ok(())
    })
    .map_err(refusal)
}

fn write_selected_executable(action: &TriggerAction, writer: &mut dyn io::Write) -> io::Result<()> {
    match action.executable() {
        Executable::Instructions(items) => {
            writer.write_all(&[0])?;
            frame(items, writer)?;
        }
        Executable::ContractCall(call) => {
            writer.write_all(&[1])?;
            frame(call, writer)?;
        }
        Executable::Batch(items) => {
            writer.write_all(&[3])?;
            frame(items, writer)?;
        }
        Executable::Ivm(_) | Executable::IvmProved(_) => {
            return Err(io::Error::other("unsupported IVM trigger"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn authenticate(
    original_bytes: &[u8],
    trigger_id: &str,
    proof_json: &str,
    network: NetworkId,
    chain: &str,
    checkpoint_bytes: &[u8],
) -> PyResult<(String, Vec<u8>)> {
    authenticate_with_activation(
        original_bytes,
        trigger_id,
        proof_json,
        network,
        chain,
        checkpoint_bytes,
        None,
    )
}

#[cfg(test)]
fn authenticate_with_activation(
    original_bytes: &[u8],
    trigger_id: &str,
    proof_json: &str,
    network: NetworkId,
    chain: &str,
    checkpoint_bytes: &[u8],
    activation_json: Option<&str>,
) -> PyResult<(String, Vec<u8>)> {
    authenticate_with_activation_policy(
        original_bytes,
        trigger_id,
        proof_json,
        network,
        chain,
        checkpoint_bytes,
        activation_json,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "independent original proof and optional rollout policy inputs"
)]
fn authenticate_with_activation_policy(
    original_bytes: &[u8],
    trigger_id: &str,
    proof_json: &str,
    network: NetworkId,
    chain: &str,
    checkpoint_bytes: &[u8],
    activation_json: Option<&str>,
    lifecycle_activation_policy_json: Option<&str>,
) -> PyResult<(String, Vec<u8>)> {
    if proof_json.is_empty()
        || proof_json.len() > MAX_PROOF_BYTES
        || checkpoint_bytes.is_empty()
        || checkpoint_bytes.len() > MAX_CHECKPOINT_BYTES
        || chain.is_empty()
        || chain.len() > 1024
    {
        return Err(refusal("scheduled proof inputs exceed their bounds"));
    }
    let activation_policy =
        activation_policy::ActivationPolicy::parse(lifecycle_activation_policy_json, network)?;
    let activation = activation::ActivationWitness::parse(activation_json)?;
    let activation_selection = activation
        .as_ref()
        .map(|witness| witness.selected_identity())
        .transpose()?;
    let original = decode_canonical_signed_transaction_v1(original_bytes)?;
    original.verify_signature().map_err(refusal)?;
    if original.network_id().copied() != Some(network) {
        return Err(refusal("original registration belongs to another network"));
    }
    let id: TriggerId = trigger_id.parse().map_err(refusal)?;
    if id.to_string() != trigger_id {
        return Err(refusal("trigger ID must be canonical"));
    }
    let trigger = original_trigger(&original, &id)?;
    let checkpoint =
        SumeragiFinalityCheckpoint::decode_canonical(checkpoint_bytes).map_err(refusal)?;
    if checkpoint.chain_id() != chain {
        return Err(refusal("checkpoint belongs to another chain"));
    }
    let proofs: Vec<SumeragiFinalityProof> = json::from_json(proof_json).map_err(refusal)?;
    let page = verify_checkpoint_page(network, &checkpoint, &proofs, MAX_PROOFS, MAX_PROOF_BYTES)
        .map_err(refusal)?;
    if page.tip().root_scope() != SumeragiRootScope::Global {
        return Err(refusal(
            "scheduled public execution requires the original Global root",
        ));
    }
    if let Some(policy) = &activation_policy {
        policy.require_tip(page.tip().height())?;
    }
    let original_hash = original.hash_as_entrypoint();
    let action = trigger.action();
    let policy = action.retry_policy().expect("validated retry policy");
    let mut registered = None;
    let mut retry = None;
    let mut progress = native_progress_declaration(action)?;
    let mut terminal = "pending";
    let mut outcomes = Vec::new();
    let mut traced = false;
    let mut registration_proof_index = None;
    let mut cut_verifier = activation
        .as_ref()
        .map(|_| {
            norito::core::with_decode_limits_scope(
                norito::canonical_decode_limits(MAX_PROOF_BYTES),
                || SumeragiFinalityVerifier::from_trusted_checkpoint(&checkpoint, &network, chain),
            )
        })
        .transpose()
        .map_err(refusal)?;
    let mut trace_budget = lifecycle_trace::frame_budget();
    let mut hash_work = LifecycleHashWork::new(original_bytes.len());
    // Every proof wire was authenticated above. Decoding the same complete
    // frames lets the application inspect all occurrences without skipping a
    // failed attempt, selecting a favorable tip, or adopting an unproved gap.
    for (proof_index, proof) in proofs.iter().enumerate() {
        // The full page above already enforced cumulative range bounds. A
        // second linear pass supplies authenticated cuts for the selected
        // native runtime anchor; no row variant substitutes for selection proof.
        let certified_cut = cut_verifier
            .as_mut()
            .map(|verifier| {
                norito::core::with_decode_limits_scope(
                    norito::canonical_decode_limits(MAX_PROOF_BYTES),
                    || {
                        if proof_index == 0 {
                            verifier.verify_same_decision(checkpoint.tip(), proof)
                        } else {
                            verifier.verify(proof)
                        }
                    },
                )
            })
            .transpose()
            .map_err(refusal)?;
        let block = norito::core::with_decode_limits_scope(
            norito::canonical_decode_limits(proof.block_wire.len()),
            || decode_framed_signed_block(&proof.block_wire),
        )
        .map_err(refusal)?;
        let height = block.header().height().get();
        if let Some(policy) = &activation_policy {
            for output in block.execution_outputs() {
                policy.require_row(height, output.trigger_lifecycle().is_some())?;
            }
        }
        let time_ms = u64::try_from(block.header().creation_time().as_millis()).map_err(refusal)?;
        for (index, input) in block.network_entrypoints().enumerate() {
            if input.hash() != original_hash {
                continue;
            }
            if registered.is_some() || input != &TransactionEntrypoint::External(original.clone()) {
                return Err(refusal(
                    "registration replay or substitution in authenticated prefix",
                ));
            }
            let output = block
                .network_output_at(u32::try_from(index).map_err(refusal)?)
                .ok_or_else(|| refusal("registration output missing"))?;
            if !output.1.result.is_ok() {
                return Err(refusal("original trigger registration did not apply"));
            }
            let original_output = &block.execution_outputs()[output.0 as usize];
            traced = original_output.trigger_lifecycle().is_some();
            if traced {
                let witness = activation.as_ref().ok_or_else(|| refusal(
                    "V2 scheduled observation requires an authenticated lifecycle activation witness",
                ))?;
                witness.require_continuation_count(proofs.len() - proof_index - 1)?;
                registration_proof_index = Some(proof_index);
            }
            registered = Some((height, time_ms));
        }
        if traced {
            activation
                .as_ref()
                .expect("V2 witness required above")
                .authenticate(
                    certified_cut
                        .as_ref()
                        .expect("witness cuts authenticated above"),
                    proof_index - registration_proof_index.expect("V2 registration found"),
                    network,
                    activation_selection.expect("V2 typed selection required above"),
                )?;
        }
        if let Some((registered_height, _)) = registered {
            let count = if traced {
                if height == registered_height {
                    hash_work.charge(1)?;
                }
                lifecycle_trace::check_block(
                    &block,
                    original_hash,
                    &original,
                    &trigger,
                    registered.expect("registration found"),
                    &mut trace_budget,
                )?
            } else {
                if block
                    .execution_outputs()
                    .iter()
                    .any(|output| output.trigger_lifecycle().is_some())
                {
                    return Err(refusal(
                        "preactivation registration cannot acquire lifecycle coverage retrospectively",
                    ));
                }
                check_lifecycle(&block, original_hash, &trigger)?
            };
            if count != usize::from(height == registered_height) {
                return Err(refusal(
                    "original activation has no unique applied root registration",
                ));
            }
        }
        for (index, output) in block.execution_outputs().iter().enumerate() {
            let Some(row) = time_row(output) else {
                continue;
            };
            if row.invocation.trigger.trigger_id != id {
                continue;
            }
            let (registered_height, registered_ms) = registered
                .ok_or_else(|| refusal("Time output precedes its original registration"))?;
            if traced {
                hash_work.charge(9)?;
            }
            if terminal == "applied"
                || terminal == "exhausted"
                || registered_height >= height
                || row.invocation.trigger.registered_at_height != registered_height
                || row.invocation.trigger.action_hash
                    != action_hash_with_progress(
                        action,
                        registered_height,
                        registered_ms,
                        retry,
                        action.repeats(),
                        traced,
                        progress,
                    )?
            {
                return Err(refusal(
                    "Time output changed the action, authority, registration or retry chain",
                ));
            }
            if let Some((_, next)) = retry
                && time_ms < next
            {
                return Err(refusal("Time retry precedes its authenticated eligibility"));
            }
            let disposition = scheduled_disposition(output)?;
            if traced {
                lifecycle_trace::check_occurrence(
                    output,
                    &trigger,
                    (registered_height, registered_ms),
                    retry,
                    time_ms,
                    disposition,
                    progress,
                )?;
            }
            progress = progress.map(|remaining| remaining.saturating_sub(1));
            let result_ok = row.result.is_ok();
            let reason = row.result.0.as_ref().err();
            outcomes.push(norito::json!({
                "block_height": height,
                "block_hash": (hex_encode(block.hash().as_ref())),
                "output_index": index,
                "output_hash": (hex_encode(HashOf::new(output).as_ref())),
                "execution_call_hash": (hex_encode(row.invocation.execution_call_hash(block.hash()).map_err(refusal)?.as_ref())),
                "result_ok": result_ok,
                "disposition": disposition,
                "rejection_code": (reason.map(transaction_rejection_code)),
                "rejection_message": (reason.map(ToString::to_string)),
                "contract_rejection": (reason.and_then(transaction_contract_rejection_json)),
                "ivm_fault": (reason.and_then(transaction_ivm_fault_json)),
                "output": (json::to_value(output).map_err(refusal)?),
            }));
            if disposition == "applied" {
                terminal = "applied";
            } else if disposition == "rejected" {
                let used = retry.map_or(0_u32, |(used, _)| used).saturating_add(1);
                if used > policy.max_retries.get() {
                    terminal = "exhausted";
                } else {
                    retry = Some((used, time_ms.saturating_add(policy.retry_after_ms.get())));
                    terminal = "retrying";
                }
            }
        }
    }
    let (registered_height, registered_ms) = registered
        .ok_or_else(|| refusal("prefix must include the exact original registration block"))?;
    let tip = page.tip();
    let projected = norito::json!({
        "proof_kind": "scheduled-time-v1",
        "network_id": (network.to_string()),
        "chain_id": chain,
        "registration_transaction_hash": (hex_encode(original_hash.as_ref())),
        "registered_at_height": registered_height,
        "registered_at_ms": registered_ms,
        "trigger_id": trigger_id,
        "action": (json::to_value(action).map_err(refusal)?),
        "authority": (json::to_value(action.authority()).map_err(refusal)?),
        "execution_route": {"kind": "internal", "root_scope": "Global", "dataspace_id": 0, "lane_id": null},
        "block_height": (tip.height()),
        "block_hash": (hex_encode(tip.block().hash().as_ref())),
        "context_id": (hex_encode(tip.context_id().as_ref())),
        "terminal": terminal,
        "outcomes": outcomes,
    });
    Ok((
        json::to_json_bounded(&projected, MAX_PROJECTION_BYTES).map_err(refusal)?,
        page.checkpoint().encode_canonical().map_err(refusal)?,
    ))
}

#[pyfunction]
#[pyo3(name = "verify_scheduled_trigger_execution_v1", signature = (original_signed_registration, trigger_id, native_finality_proof_chain_json, expected_network_id, expected_chain, trusted_checkpoint, *, expected_chain_discriminant, lifecycle_activation_json=None, lifecycle_activation_policy_json=None))]
#[expect(
    clippy::too_many_arguments,
    reason = "PyO3 exposes the named Python arguments and its interpreter token as separate Rust parameters"
)]
pub(super) fn verify(
    py: Python<'_>,
    original_signed_registration: &Bound<'_, PyBytes>,
    trigger_id: &str,
    native_finality_proof_chain_json: &str,
    expected_network_id: &PyNetworkId,
    expected_chain: &str,
    trusted_checkpoint: &Bound<'_, PyBytes>,
    #[pyo3(from_py_with = exact_chain_discriminant)] expected_chain_discriminant: u16,
    lifecycle_activation_json: Option<&str>,
    lifecycle_activation_policy_json: Option<&str>,
) -> PyResult<(String, Py<PyBytes>)> {
    if !original_signed_registration.is_exact_instance_of::<PyBytes>()
        || !trusted_checkpoint.is_exact_instance_of::<PyBytes>()
    {
        return Err(PyTypeError::new_err(
            "scheduled proof wires must be exact immutable bytes",
        ));
    }
    let _guard = ChainDiscriminantGuard::enter(expected_chain_discriminant);
    let (projection, checkpoint) = authenticate_with_activation_policy(
        original_signed_registration.as_bytes(),
        trigger_id,
        native_finality_proof_chain_json,
        *expected_network_id.as_inner(),
        expected_chain,
        trusted_checkpoint.as_bytes(),
        lifecycle_activation_json,
        lifecycle_activation_policy_json,
    )?;
    Ok((projection, PyBytes::new(py, &checkpoint).unbind()))
}

#[cfg(test)]
#[path = "tests/scheduled_execution_verification_tests.rs"]
mod tests;
