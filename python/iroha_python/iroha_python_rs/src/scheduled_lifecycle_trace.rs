//! Consume complete compact lifecycle journals without inferring effects from guest code.
//!
//! The caller has authenticated finality and the registration's runtime selection.
//! Every block is structurally validated with one cumulative range budget. Only
//! the original direct Insert and exact native scheduled transitions are admitted
//! for the selected trigger; unrelated opaque execution needs no guessed effect list.

use super::*;
use iroha_data_model::block::execution_output::{
    TriggerLifecycleActionFrameBudgetV1, TriggerLifecycleActionFrameLimitsV1,
    TriggerLifecycleActionSemanticsV1, TriggerLifecycleActionValidatorV1,
    TriggerLifecycleChangeV1 as Change, TriggerLifecycleEffectV1 as Effect,
    TriggerLifecycleOriginV1 as Origin, TriggerLifecyclePolicyV1 as Policy,
    TriggerLifecycleRetryStateV1, TriggerLifecycleStateKindV1, TriggerLifecycleStateV1,
    validate_execution_outputs_with_lifecycle_budget_v1,
};

/// The selected successor uses fixed-size persistent hash references. Historical
/// full Action frames are not accepted as an implicit alternate activation profile.
struct CompactOnly;
impl TriggerLifecycleActionValidatorV1 for CompactOnly {
    fn validate_canonical_action(
        &mut self,
        _frame: &[u8],
        _payload: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, String> {
        Err("scheduled lifecycle requires the selected compact Action codec".into())
    }
}

pub(super) fn frame_budget() -> TriggerLifecycleActionFrameBudgetV1 {
    TriggerLifecycleActionFrameBudgetV1::new(TriggerLifecycleActionFrameLimitsV1 {
        max_frame_bytes: iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES
            as u64,
        max_total_frame_bytes: MAX_PROOF_BYTES as u64,
        max_frames: 262_144,
    })
}

fn registration_index(original: &SignedTransaction, selected: &Trigger) -> PyResult<u32> {
    let matches = |instruction: &InstructionBox| {
        matches!(instruction.as_any().downcast_ref::<RegisterBox>(),
            Some(RegisterBox::Trigger(registration)) if &registration.object == selected)
    };
    let position = match original.instructions() {
        Executable::Instructions(items) => items.iter().position(matches),
        Executable::Batch(items) => items.iter().position(|item| {
            matches!(item, ExecutableBatchItem::Instruction(instruction) if matches(instruction))
        }),
        _ => None,
    };
    u32::try_from(position.ok_or_else(|| refusal("original direct registration is absent"))?)
        .map_err(refusal)
}

fn state_matches(
    state: &TriggerLifecycleStateV1,
    selected: &Trigger,
    registered: (u64, u64),
    retry: Option<(u32, u64)>,
    repeats: u32,
    progress: Option<u32>,
) -> PyResult<()> {
    let hash = action_hash_with_progress(
        selected.action(),
        registered.0,
        registered.1,
        retry,
        Repeats::Exactly(repeats),
        true,
        progress,
    )?;
    let expected_retry = retry.map(|(attempt, next_time_ms)| TriggerLifecycleRetryStateV1 {
        attempt,
        next_time_ms,
    });
    if state.kind != TriggerLifecycleStateKindV1::Time
        || state.registered_at_height != Some(registered.0)
        || state.action_hash != Some(hash)
        || state.action.persistent_hash() != Some(hash)
        || state.retry_state != expected_retry
    {
        return Err(refusal(
            "selected lifecycle state differs from its original action and native retry chain",
        ));
    }
    Ok(())
}

pub(super) fn check_block(
    block: &SignedBlock,
    original_hash: HashOf<TransactionEntrypoint>,
    original: &SignedTransaction,
    selected: &Trigger,
    registered: (u64, u64),
    budget: &mut TriggerLifecycleActionFrameBudgetV1,
) -> PyResult<usize> {
    validate_execution_outputs_with_lifecycle_budget_v1(
        block.execution_outputs(),
        block.hash(),
        block.header().height().get(),
        block,
        budget,
        &mut CompactOnly,
    )
    .map_err(refusal)?;
    let mut registrations = 0;
    for output in block.execution_outputs() {
        let selected_time =
            time_row(output).is_some_and(|row| row.invocation.trigger.trigger_id == *selected.id());
        for effect in output
            .require_trigger_lifecycle()
            .map_err(refusal)?
            .effects()
        {
            if &effect.trigger_id != selected.id() || selected_time {
                continue;
            }
            let original_root = output.network().is_some_and(|row| {
                block
                    .network_entrypoint_at(row.input_index as usize)
                    .is_some_and(|input| input.hash() == original_hash)
            });
            if original_root
                && output.result().is_ok()
                && effect.origin
                    == (Origin::DirectNetworkInstruction {
                        item_index: registration_index(original, selected)?,
                    })
                && let Change::Insert { after } = &effect.change
            {
                state_matches(
                    after,
                    selected,
                    registered,
                    None,
                    1,
                    native_progress_declaration(selected.action())?,
                )?;
                registrations += 1;
            } else {
                return Err(refusal(
                    "selected trigger lifecycle changed outside its original registration or scheduled policy",
                ));
            }
        }
    }
    Ok(registrations)
}

fn policy(effect: &Effect, expected: Policy) -> PyResult<()> {
    if effect.origin
        != (Origin::InvocationPolicy {
            callback_index: 0,
            policy: expected,
        })
    {
        return Err(refusal(
            "selected lifecycle transition has a foreign invocation policy",
        ));
    }
    Ok(())
}

fn update(
    effect: &Effect,
    expected_policy: Policy,
    selected: &Trigger,
    registered: (u64, u64),
    before: (Option<(u32, u64)>, u32, Option<u32>),
    after: (Option<(u32, u64)>, u32, Option<u32>),
) -> PyResult<()> {
    policy(effect, expected_policy)?;
    let Change::Update {
        before: old,
        after: new,
    } = &effect.change
    else {
        return Err(refusal(
            "native lifecycle policy is not the required Update",
        ));
    };
    state_matches(old, selected, registered, before.0, before.1, before.2)?;
    state_matches(new, selected, registered, after.0, after.1, after.2)
}

fn remove(
    effect: &Effect,
    expected_policy: Policy,
    selected: &Trigger,
    registered: (u64, u64),
    retry: Option<(u32, u64)>,
    repeats: u32,
    progress: Option<u32>,
) -> PyResult<()> {
    policy(effect, expected_policy)?;
    let Change::Remove { before } = &effect.change else {
        return Err(refusal(
            "native lifecycle policy is not the required Remove",
        ));
    };
    state_matches(before, selected, registered, retry, repeats, progress)
}

pub(super) fn check_occurrence(
    output: &ExecutionOutputV1,
    selected: &Trigger,
    registered_height: u64,
    registered_ms: u64,
    retry: Option<(u32, u64)>,
    time_ms: u64,
    disposition: &str,
    progress: Option<u32>,
) -> PyResult<()> {
    let registered = (registered_height, registered_ms);
    let after_progress = progress.map(|remaining| remaining.saturating_sub(1));
    let mut effects = output
        .require_trigger_lifecycle()
        .map_err(refusal)?
        .effects()
        .iter()
        .filter(|effect| &effect.trigger_id == selected.id());
    let missing = || refusal("scheduled output omits a required selected lifecycle effect");
    match disposition {
        "applied" => {
            let effect = effects.next().ok_or_else(missing)?;
            let Change::Invoke {
                state,
                callback_index: 0,
            } = &effect.change
            else {
                return Err(refusal("successful Time root has no original Invoke"));
            };
            if effect.origin
                != (Origin::Derived {
                    callback_index: Some(0),
                })
            {
                return Err(refusal("Time Invoke has a foreign callback origin"));
            }
            state_matches(state, selected, registered, retry, 1, progress)?;
            if progress.is_some_and(|remaining| remaining > 0) {
                update(
                    effects.next().ok_or_else(missing)?,
                    Policy::ConsumeTimeProgress,
                    selected,
                    registered,
                    (retry, 1, progress),
                    (retry, 1, after_progress),
                )?;
            }
            if retry.is_some() {
                update(
                    effects.next().ok_or_else(missing)?,
                    Policy::ClearTimeRetry,
                    selected,
                    registered,
                    (retry, 1, after_progress),
                    (None, 1, after_progress),
                )?;
            }
            update(
                effects.next().ok_or_else(missing)?,
                Policy::ConsumeRepeat,
                selected,
                registered,
                (None, 1, after_progress),
                (None, 0, after_progress),
            )?;
            remove(
                effects.next().ok_or_else(missing)?,
                Policy::RemoveDepleted,
                selected,
                registered,
                None,
                0,
                after_progress,
            )?;
        }
        "rejected" => {
            let used = retry.map_or(0_u32, |(used, _)| used).saturating_add(1);
            let retry_policy = selected
                .action()
                .retry_policy()
                .expect("validated original policy");
            let effect = effects.next().ok_or_else(missing)?;
            if used > retry_policy.max_retries.get() {
                remove(
                    effect,
                    Policy::ExhaustTimeRetry,
                    selected,
                    registered,
                    retry,
                    1,
                    progress,
                )?;
            } else {
                let next = time_ms.saturating_add(retry_policy.retry_after_ms.get());
                update(
                    effect,
                    Policy::AdvanceTimeRetry,
                    selected,
                    registered,
                    (retry, 1, progress),
                    (Some((used, next)), 1, after_progress),
                )?;
            }
        }
        "output_limit" => {
            if progress.is_some_and(|remaining| remaining > 0) {
                update(
                    effects.next().ok_or_else(missing)?,
                    Policy::ConsumeTimeProgress,
                    selected,
                    registered,
                    (retry, 1, progress),
                    (retry, 1, after_progress),
                )?;
            }
        }
        _ => return Err(refusal("unknown native scheduled disposition")),
    }
    if effects.next().is_some() {
        return Err(refusal(
            "scheduled output contains an additional selected-trigger touch",
        ));
    }
    Ok(())
}
