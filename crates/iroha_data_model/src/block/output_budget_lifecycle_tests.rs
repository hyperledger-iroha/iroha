//! Source-only budget regression candidate. These model fixtures authenticate
//! neither execution nor action digests; actual native compilation is pending.

use super::*;
use crate::{
    ValidationFail,
    account::AccountId,
    block::execution_output::{
        INTERNAL_REJECTION_DIAGNOSTIC_OMITTED, InvocationCompletionV1, PipelineEventPositionV1,
        PipelineInvocationV1, TimeExecutionOutputV1, TimeExecutionOutputV2, TimeInvocationV1,
        TriggerFailureRootV1, TriggerLifecycleActionCodecV1, TriggerLifecycleActionFrameV1,
        TriggerLifecycleChangeV1, TriggerLifecycleEffectV1, TriggerLifecycleEffectsV1,
        TriggerLifecycleOriginV1, TriggerLifecyclePolicyV1, TriggerLifecycleRetryStateV1,
        TriggerLifecycleStateKindV1, TriggerLifecycleStateV1, TriggerUseV1,
    },
    events::{
        EventFilterBox,
        time::{ExecutionTime, Schedule, TimeEvent, TimeEventFilter, TimeInterval},
        trigger_completed::TriggerCompletedOutcome,
    },
    transaction::{
        Executable,
        error::{TransactionLimitError, TransactionRejectionReason},
        signed::{ExecutionStep, TransactionResult},
    },
    trigger::{
        DataTriggerStep,
        action::{Action, Repeats, TimeTriggerRetryPolicy},
    },
};
use iroha_crypto::{Algorithm, Hash, KeyPair};

fn trigger() -> TriggerUseV1 {
    TriggerUseV1 {
        trigger_id: "budget-lifecycle-root".parse().unwrap(),
        registered_at_height: 3,
        action_hash: Hash::new(b"budget fixture; not an authenticated action digest"),
    }
}

fn time_invocation() -> TimeInvocationV1 {
    TimeInvocationV1 {
        schedule_index: 2,
        event: TimeEvent {
            interval: TimeInterval {
                since_ms: 100,
                length_ms: 20,
            },
        },
        trigger: trigger(),
    }
}

fn pipeline_invocation() -> PipelineInvocationV1 {
    PipelineInvocationV1 {
        event: PipelineEventPositionV1::BlockApproved,
        candidate_index: 4,
        trigger: trigger(),
    }
}

fn empty_step() -> ExecutionStep {
    ExecutionStep(iroha_primitives::const_vec::ConstVec::new_empty())
}

fn time_state() -> TriggerLifecycleStateV1 {
    let key = KeyPair::from_seed(vec![0x63; 32], Algorithm::Ed25519);
    let action = Action::new(
        Executable::Instructions(iroha_primitives::const_vec::ConstVec::new_empty()),
        Repeats::Exactly(1),
        AccountId::new(key.public_key().clone()),
        EventFilterBox::Time(TimeEventFilter::new(ExecutionTime::Schedule(
            Schedule::starting_at(std::time::Duration::from_millis(100)),
        ))),
    )
    .unwrap()
    .with_retry_policy(TimeTriggerRetryPolicy {
        max_retries: std::num::NonZeroU32::new(2).unwrap(),
        retry_after_ms: std::num::NonZeroU64::new(10).unwrap(),
    })
    .unwrap();
    TriggerLifecycleStateV1 {
        kind: TriggerLifecycleStateKindV1::Time,
        registered_at_height: Some(trigger().registered_at_height),
        action_hash: Some(trigger().action_hash),
        action: TriggerLifecycleActionFrameV1 {
            codec: TriggerLifecycleActionCodecV1::NoritoCanonicalV1,
            bytes: norito::encode_canonical(&action).unwrap(),
        },
        retry_state: None,
    }
}

fn genuine_disposition_fixture(diagnostic_len: usize) -> ExecutionOutputV1 {
    let before = time_state();
    let mut after = before.clone();
    after.retry_state = Some(TriggerLifecycleRetryStateV1 {
        attempt: 1,
        next_time_ms: 130,
    });
    after.action_hash = Some(Hash::new(b"budget fixture next retry digest"));
    let row = ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
        execution: TimeExecutionOutputV1 {
            invocation: time_invocation(),
            result: TransactionResult::new(Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("r".repeat(diagnostic_len)),
            ))),
            failure_root: Some(TriggerFailureRootV1::DeclaredInstructionProjection(
                empty_step(),
            )),
            completions: vec![InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: trigger().trigger_id,
                outcome: TriggerCompletedOutcome::Failure("r".repeat(diagnostic_len)),
            }],
        },
        trigger_lifecycle: TriggerLifecycleEffectsV1::Complete {
            effects: vec![TriggerLifecycleEffectV1 {
                ordinal: 0,
                trigger_id: trigger().trigger_id,
                origin: TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy: TriggerLifecyclePolicyV1::AdvanceTimeRetry,
                },
                change: TriggerLifecycleChangeV1::Update { before, after },
            }],
        },
    });
    row.validate_structure(8, &[]).unwrap();
    assert!(!row.is_output_limit_rejection());
    row
}

fn expected_omitted(actual: &ExecutionOutputV1) -> ExecutionOutputV1 {
    let mut expected = actual.clone();
    let ExecutionOutputV1::TimeWithLifecycle(row) = &mut expected else {
        panic!("Time lifecycle fixture required")
    };
    row.execution.result = TransactionResult::new(Err(TransactionRejectionReason::LimitCheck(
        TransactionLimitError {
            reason: INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.into(),
        },
    )));
    row.execution.failure_root = Some(TriggerFailureRootV1::OmittedAfterRejection);
    row.execution.completions[0].outcome =
        TriggerCompletedOutcome::Failure(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.into());
    expected.validate_structure(8, &[]).unwrap();
    assert!(expected.is_internal_rejection_diagnostic_omitted());
    assert!(!expected.is_output_limit_rejection());
    expected
}

fn healthy_success() -> ExecutionOutputV1 {
    let row = ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
        execution: TimeExecutionOutputV1 {
            invocation: time_invocation(),
            result: TransactionResult::new(Ok(vec![DataTriggerStep {
                id: trigger().trigger_id,
                instructions: empty_step(),
            }])),
            failure_root: None,
            completions: vec![InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: trigger().trigger_id,
                outcome: TriggerCompletedOutcome::Success,
            }],
        },
        trigger_lifecycle: TriggerLifecycleEffectsV1::Complete {
            effects: vec![TriggerLifecycleEffectV1 {
                ordinal: 0,
                trigger_id: trigger().trigger_id,
                origin: TriggerLifecycleOriginV1::Derived {
                    callback_index: Some(0),
                },
                change: TriggerLifecycleChangeV1::Invoke {
                    state: time_state(),
                    callback_index: 0,
                },
            }],
        },
    });
    row.validate_structure(8, &[]).unwrap();
    row
}

fn budget(
    phase: ExecutionOutputPhase,
    count: u32,
    ceiling: u64,
    row_limit: u64,
    total_limit: u64,
) -> ExecutionOutputBudget {
    let mut phases = [ExecutionOutputPhaseReservation {
        count: 0,
        terminal_bytes_per_output: 0,
    }; 3];
    phases[phase.index()] = ExecutionOutputPhaseReservation {
        count,
        terminal_bytes_per_output: ceiling,
    };
    ExecutionOutputBudget::new(
        ExecutionOutputLimits {
            max_outputs: count,
            max_output_bytes: row_limit,
            max_total_output_bytes: total_limit,
            max_executed_wire_bytes: total_limit,
        },
        phases,
    )
    .unwrap()
}

#[test]
fn lifecycle_phases_preserve_exact_carrier_version_and_source() {
    let pairs = [
        (
            ExecutionOutputPhase::Network,
            ExecutionOutputV1::network_output_limit_rejection(0),
            ExecutionOutputV1::network_lifecycle_output_limit_rejection(0),
        ),
        (
            ExecutionOutputPhase::Pipeline,
            ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation()),
            ExecutionOutputV1::pipeline_lifecycle_output_limit_rejection(pipeline_invocation()),
        ),
        (
            ExecutionOutputPhase::Time,
            ExecutionOutputV1::time_output_limit_rejection(time_invocation()),
            ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation()),
        ),
    ];
    for (phase, old, new) in pairs {
        assert_eq!(ExecutionOutputPhase::of(&old), phase);
        assert_eq!(ExecutionOutputPhase::of(&new), phase);
        assert!(same_origin(&old, &old));
        assert!(same_origin(&new, &new));
        assert!(!same_origin(&old, &new));
        assert!(!same_origin(&new, &old));
        let mut foreign = new.clone();
        match &mut foreign {
            ExecutionOutputV1::NetworkWithLifecycle(row) => row.execution.input_index += 1,
            ExecutionOutputV1::PipelineWithLifecycle(row) => {
                row.execution.invocation.candidate_index += 1
            }
            ExecutionOutputV1::TimeWithLifecycle(row) => {
                row.execution.invocation.schedule_index += 1
            }
            _ => unreachable!(),
        }
        assert!(!same_origin(&new, &foreign));
        for (terminal, actual) in [(old.clone(), new.clone()), (new, old)] {
            let ceiling = output_bytes(&terminal)
                .unwrap()
                .max(output_bytes(&actual).unwrap());
            let mut owner = budget(phase, 1, ceiling, ceiling, ceiling);
            assert!(owner.begin(terminal).unwrap().finish(actual).is_err());
            assert_eq!(owner.committed_outputs, 0);
            assert!(owner.abandoned);
            assert!(owner.finish().is_err());
        }
    }
}

#[test]
fn fitting_lifecycle_rejection_retains_the_exact_actual_row() {
    let actual = genuine_disposition_fixture(64);
    let expected = expected_omitted(&actual);
    let actual_bytes = output_bytes(&actual).unwrap();
    let ceiling = output_bytes(&expected).unwrap();
    let cap = ceiling.max(actual_bytes);
    let mut owner = budget(ExecutionOutputPhase::Time, 1, ceiling, cap, cap);
    let retained = owner
        .begin(ExecutionOutputV1::time_lifecycle_output_limit_rejection(
            time_invocation(),
        ))
        .unwrap()
        .finish_internal_rejection(actual.clone())
        .unwrap();
    assert_eq!(retained, actual);
    assert_eq!(owner.finish().unwrap(), (1, actual_bytes));
}

#[test]
fn oversized_time_rejection_preserves_full_policy_trace_within_preowned_ceiling() {
    let actual = genuine_disposition_fixture(16_384);
    let expected = expected_omitted(&actual);
    let terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    let bytes = output_bytes(&expected).unwrap();
    assert!(bytes > output_bytes(&terminal).unwrap());
    assert!(output_bytes(&actual).unwrap() > bytes);
    let mut owner = budget(ExecutionOutputPhase::Time, 1, bytes, bytes, bytes);
    let retained = owner
        .begin(terminal)
        .unwrap()
        .finish_internal_rejection(actual)
        .unwrap();
    assert_eq!(retained, expected);
    let trace = retained.require_trigger_lifecycle().unwrap();
    assert_eq!(trace.effects().len(), 1);
    let TriggerLifecycleChangeV1::Update { before, after } = &trace.effects()[0].change else {
        panic!("the actual complete retry transition must survive diagnostic omission")
    };
    assert_eq!(before.retry_state, None);
    assert_eq!(
        after.retry_state,
        Some(TriggerLifecycleRetryStateV1 {
            attempt: 1,
            next_time_ms: 130,
        })
    );
    assert_eq!(before.action, after.action);
    assert!(retained.is_internal_rejection_diagnostic_omitted());
    assert!(!retained.is_output_limit_rejection());
    assert_eq!(owner.finish().unwrap(), (1, bytes));
}

#[test]
fn one_byte_short_policy_ceiling_refuses_and_abandons_without_healthy_terminal() {
    let actual = genuine_disposition_fixture(16_384);
    let expected = expected_omitted(&actual);
    let terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    let bytes = output_bytes(&expected).unwrap();
    let ceiling = bytes - 1;
    assert!(output_bytes(&terminal).unwrap() <= ceiling);
    let mut owner = budget(ExecutionOutputPhase::Time, 1, ceiling, bytes, bytes);
    assert!(
        owner
            .begin(terminal)
            .unwrap()
            .finish_internal_rejection(actual)
            .is_err()
    );
    assert!(owner.abandoned);
    assert_eq!(owner.committed_outputs, 0);
    assert_eq!(owner.committed_bytes, 0);
    assert!(owner.finish().is_err());
}

#[test]
fn aggregate_pressure_uses_policy_fallback_and_preserves_the_next_reservation() {
    let actual = genuine_disposition_fixture(16_384);
    let expected = expected_omitted(&actual);
    let fallback_bytes = output_bytes(&expected).unwrap();
    let actual_bytes = output_bytes(&actual).unwrap();
    assert!(actual_bytes > fallback_bytes * 2);
    let terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    let terminal_bytes = output_bytes(&terminal).unwrap();
    let mut owner = budget(
        ExecutionOutputPhase::Time,
        2,
        fallback_bytes,
        actual_bytes,
        actual_bytes,
    );
    let retained = owner
        .begin(terminal.clone())
        .unwrap()
        .finish_internal_rejection(actual)
        .unwrap();
    assert_eq!(retained, expected);
    assert_eq!(owner.committed_outputs, 1);
    assert_eq!(owner.committed_bytes, fallback_bytes);
    assert_eq!(owner.remaining_terminal_bytes, fallback_bytes);
    let next = owner
        .begin(terminal.clone())
        .unwrap()
        .finish(terminal)
        .unwrap();
    assert!(matches!(next, ReservedExecutionOutput::Accepted(_)));
    assert_eq!(
        owner.finish().unwrap(),
        (2, fallback_bytes + terminal_bytes)
    );
}

#[test]
fn healthy_oversized_success_keeps_empty_trace_and_typed_output_limit() {
    let actual = healthy_success();
    let terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    let bytes = output_bytes(&terminal).unwrap();
    assert!(output_bytes(&actual).unwrap() > bytes);
    let mut owner = budget(ExecutionOutputPhase::Time, 1, bytes, bytes, bytes);
    let decision = owner
        .begin(terminal.clone())
        .unwrap()
        .finish(actual)
        .unwrap();
    let ReservedExecutionOutput::OutputLimit(retained) = decision else {
        panic!("oversized healthy work must roll back into the reserved capacity terminal")
    };
    assert_eq!(retained, terminal);
    assert!(
        retained
            .require_trigger_lifecycle()
            .unwrap()
            .effects()
            .is_empty()
    );
    assert!(retained.is_output_limit_rejection());
    assert!(!retained.is_internal_rejection_diagnostic_omitted());
    retained.validate_structure(8, &[]).unwrap();
    assert_eq!(owner.finish().unwrap(), (1, bytes));
}

#[test]
fn generic_finish_cannot_erase_real_internal_lifecycle_rejection_policy() {
    let actual = genuine_disposition_fixture(16_384);
    let terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    let actual_bytes = output_bytes(&actual).unwrap();
    let terminal_bytes = output_bytes(&terminal).unwrap();
    assert!(actual_bytes > terminal_bytes * 2);

    // The refusal belongs only to the oversized path: a fitting genuine row
    // remains exactly retained through this same public method.
    let mut fitting = budget(
        ExecutionOutputPhase::Time,
        1,
        terminal_bytes,
        actual_bytes,
        actual_bytes,
    );
    let retained = fitting
        .begin(terminal.clone())
        .unwrap()
        .finish(actual.clone())
        .unwrap();
    let ReservedExecutionOutput::Accepted(retained) = retained else {
        panic!("a fitting genuine rejection must remain exactly retained")
    };
    assert_eq!(retained, actual);
    assert_eq!(fitting.finish().unwrap(), (1, actual_bytes));

    for aggregate_only in [false, true] {
        let (count, row_limit, total_limit) = if aggregate_only {
            // The row itself fits, but not while preserving the second slot.
            (2, actual_bytes, actual_bytes)
        } else {
            (1, terminal_bytes, terminal_bytes)
        };
        let mut owner = budget(
            ExecutionOutputPhase::Time,
            count,
            terminal_bytes,
            row_limit,
            total_limit,
        );
        assert!(
            owner
                .begin(terminal.clone())
                .unwrap()
                .finish(actual.clone())
                .is_err(),
            "generic finish must refuse lossy policy projection; aggregate_only={aggregate_only}",
        );
        assert!(owner.abandoned);
        assert_eq!(owner.committed_outputs, 0);
        assert_eq!(owner.committed_bytes, 0);
        assert!(owner.finish().is_err());
    }
}
