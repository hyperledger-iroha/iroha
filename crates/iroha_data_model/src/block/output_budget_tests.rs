//! Capacity arithmetic and exact canonical row tests; no State execution claims.

use super::*;
use crate::{
    block::execution_output::{
        NetworkExecutionOutputV1, PipelineEventPositionV1, PipelineInvocationV1, TimeInvocationV1,
        TriggerUseV1,
    },
    events::time::{TimeEvent, TimeInterval},
    transaction::{
        error::{TransactionLimitError, TransactionRejectionReason},
        signed::TransactionResult,
    },
};
use iroha_crypto::Hash;

fn terminal(index: u32) -> ExecutionOutputV1 {
    ExecutionOutputV1::network_output_limit_rejection(index)
}

fn attempted(index: u32, message_bytes: usize) -> ExecutionOutputV1 {
    ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: index,
        result: TransactionResult::new(Err(TransactionRejectionReason::LimitCheck(
            TransactionLimitError {
                reason: "x".repeat(message_bytes),
            },
        ))),
        completions: Vec::new(),
    })
}

fn trigger() -> TriggerUseV1 {
    TriggerUseV1 {
        trigger_id: "budget-root".parse().unwrap(),
        registered_at_height: 3,
        action_hash: Hash::new(b"test-only untrusted action description"),
    }
}

fn pipeline() -> ExecutionOutputV1 {
    ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
        event: PipelineEventPositionV1::BlockApproved,
        candidate_index: 0,
        trigger: trigger(),
    })
}

fn time() -> ExecutionOutputV1 {
    ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
        schedule_index: 0,
        event: TimeEvent {
            interval: TimeInterval {
                since_ms: 1,
                length_ms: 2,
            },
        },
        trigger: trigger(),
    })
}

fn limits(count: u32, row: u64, total: u64) -> ExecutionOutputLimits {
    ExecutionOutputLimits {
        max_outputs: count,
        max_output_bytes: row,
        max_total_output_bytes: total,
        max_executed_wire_bytes: total + 4096,
    }
}

fn phase(count: u32, bytes: u64) -> ExecutionOutputPhaseReservation {
    ExecutionOutputPhaseReservation {
        count,
        terminal_bytes_per_output: bytes,
    }
}

fn empty() -> ExecutionOutputPhaseReservation {
    phase(0, 0)
}

#[test]
fn explicit_output_limits_reject_zero_and_inverted_policy() {
    let valid = limits(2, 100, 200);
    valid.validate().unwrap();
    for invalid in [
        ExecutionOutputLimits {
            max_outputs: 0,
            ..valid
        },
        ExecutionOutputLimits {
            max_output_bytes: 0,
            ..valid
        },
        ExecutionOutputLimits {
            max_total_output_bytes: 0,
            ..valid
        },
        ExecutionOutputLimits {
            max_executed_wire_bytes: 0,
            ..valid
        },
        ExecutionOutputLimits {
            max_output_bytes: 201,
            ..valid
        },
        ExecutionOutputLimits {
            max_executed_wire_bytes: 199,
            ..valid
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn row_limits_use_exact_canonical_frames_for_count_row_and_aggregate() {
    let rows = [terminal(0), terminal(1)];
    let sizes = rows
        .each_ref()
        .map(|row| u64::try_from(norito::encode_canonical(row).unwrap().len()).unwrap());
    assert_eq!(sizes[0], output_bytes(&rows[0]).unwrap());
    let total = sizes.iter().sum();
    let row = *sizes.iter().max().unwrap();
    limits(2, row, total).validate_outputs(&rows).unwrap();
    assert!(limits(1, row, total).validate_outputs(&rows).is_err());
    assert!(limits(2, row - 1, total).validate_outputs(&rows).is_err());
    assert!(limits(2, row, total - 1).validate_outputs(&rows).is_err());
    let _ambient = norito::core::DecodeFlagsGuard::enter(0);
    limits(2, row, total).validate_outputs(&rows).unwrap();
    assert_eq!(output_bytes(&rows[0]).unwrap(), sizes[0]);
}

#[test]
fn complete_terminal_plan_rejects_infeasible_counts_bytes_and_overflow_before_work() {
    let unit = output_bytes(&terminal(0)).unwrap();
    let policy = limits(2, unit, unit * 2);
    ExecutionOutputBudget::new(policy, [phase(2, unit), empty(), empty()]).unwrap();
    assert!(ExecutionOutputBudget::new(policy, [phase(3, unit), empty(), empty()]).is_err());
    assert!(ExecutionOutputBudget::new(policy, [phase(1, unit + 1), empty(), empty()]).is_err());
    assert!(ExecutionOutputBudget::new(policy, [phase(1, 0), empty(), empty()]).is_err());
    assert!(ExecutionOutputBudget::new(policy, [phase(0, 1), empty(), empty()]).is_err());
    let reduced = limits(2, unit, unit * 2 - 1);
    assert!(ExecutionOutputBudget::new(reduced, [phase(2, unit), empty(), empty()]).is_err());
    let huge = ExecutionOutputLimits {
        max_outputs: u32::MAX,
        max_output_bytes: u64::MAX,
        max_total_output_bytes: u64::MAX,
        max_executed_wire_bytes: u64::MAX,
    };
    assert!(ExecutionOutputBudget::new(huge, [phase(2, u64::MAX), empty(), empty()]).is_err());
    assert!(ExecutionOutputBudget::new(huge, [phase(u32::MAX, 1), phase(1, 1), empty()]).is_err());
    assert!(ExecutionOutputBudget::new(huge, [phase(1, u64::MAX), phase(1, 1), empty()]).is_err());
}

#[test]
fn fitting_output_spends_only_surplus_and_leaves_later_terminal_owned() {
    let first = attempted(0, 1024);
    let first_bytes = output_bytes(&first).unwrap();
    let terminal_bytes = output_bytes(&terminal(1)).unwrap();
    let policy = limits(2, first_bytes, first_bytes + terminal_bytes);
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, terminal_bytes), empty(), empty()]).unwrap();
    assert!(matches!(
        owner.begin(terminal(0)).unwrap().finish(first).unwrap(),
        ReservedExecutionOutput::Accepted(_)
    ));
    assert!(owner.finish().is_err(), "later input remains an obligation");

    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, terminal_bytes), empty(), empty()]).unwrap();
    owner
        .begin(terminal(0))
        .unwrap()
        .finish(attempted(0, 1024))
        .unwrap();
    owner
        .begin(terminal(1))
        .unwrap()
        .finish(terminal(1))
        .unwrap();
    assert_eq!(owner.finish().unwrap(), (2, first_bytes + terminal_bytes));
}

#[test]
fn oversized_row_uses_exact_reserved_failure_without_allocating_a_new_diagnostic() {
    let fallback = terminal(0);
    let bytes = output_bytes(&fallback).unwrap();
    let policy = limits(1, bytes, bytes);
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(1, bytes), empty(), empty()]).unwrap();
    let outcome = owner
        .begin(fallback.clone())
        .unwrap()
        .finish(attempted(0, 32_000))
        .unwrap();
    let ReservedExecutionOutput::OutputLimit(retained) = outcome else {
        panic!("oversized row accepted")
    };
    assert_eq!(retained, fallback);
    assert_eq!(owner.finish().unwrap(), (1, bytes));
}

#[test]
fn aggregate_exhaustion_cannot_spend_another_invocations_terminal() {
    let fallback_bytes = output_bytes(&terminal(0)).unwrap();
    let attempted = attempted(0, 1024);
    let actual_bytes = output_bytes(&attempted).unwrap();
    let total = actual_bytes + fallback_bytes - 1;
    let policy = limits(2, actual_bytes, total);
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, fallback_bytes), empty(), empty()]).unwrap();
    assert!(matches!(
        owner.begin(terminal(0)).unwrap().finish(attempted).unwrap(),
        ReservedExecutionOutput::OutputLimit(_)
    ));
    owner
        .begin(terminal(1))
        .unwrap()
        .finish(terminal(1))
        .unwrap();
    assert_eq!(owner.finish().unwrap(), (2, fallback_bytes * 2));
}

#[test]
fn phases_and_proven_callback_skips_preserve_network_coverage() {
    let n = output_bytes(&terminal(0)).unwrap();
    let p = output_bytes(&pipeline()).unwrap();
    let t = output_bytes(&time()).unwrap();
    let mut owner = ExecutionOutputBudget::new(
        limits(4, n.max(p).max(t), n + p * 2 + t),
        [phase(1, n), phase(2, p), phase(1, t)],
    )
    .unwrap();
    assert!(owner.begin(pipeline()).is_err());
    assert!(
        owner
            .skip_uninvoked(ExecutionOutputPhase::Network, 1)
            .is_err()
    );
    owner
        .begin(terminal(0))
        .unwrap()
        .finish(terminal(0))
        .unwrap();
    assert!(
        owner
            .skip_uninvoked(ExecutionOutputPhase::Pipeline, 3)
            .is_err()
    );
    owner
        .skip_uninvoked(ExecutionOutputPhase::Pipeline, 1)
        .unwrap();
    assert!(owner.begin(time()).is_err());
    owner.begin(pipeline()).unwrap().finish(pipeline()).unwrap();
    owner.begin(time()).unwrap().finish(time()).unwrap();
    assert!(owner.begin(pipeline()).is_err());
    assert_eq!(owner.finish().unwrap(), (3, n + p + t));
}

#[test]
fn reservation_rejects_false_terminal_bound_and_nonterminal_before_consuming_slot() {
    let bytes = output_bytes(&terminal(0)).unwrap();
    let policy = limits(1, bytes, bytes);
    let mut short =
        ExecutionOutputBudget::new(policy, [phase(1, bytes - 1), empty(), empty()]).unwrap();
    assert!(short.begin(terminal(0)).is_err());
    assert!(short.finish().is_err());
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(1, bytes), empty(), empty()]).unwrap();
    assert!(owner.begin(attempted(0, 1)).is_err());
    owner
        .begin(terminal(0))
        .unwrap()
        .finish(terminal(0))
        .unwrap();
    assert!(owner.begin(terminal(0)).is_err());
    assert_eq!(owner.finish().unwrap(), (1, bytes));
}

#[test]
fn abandoned_or_substituted_invocation_permanently_refuses_carrier_sealing() {
    let bytes = output_bytes(&terminal(0)).unwrap();
    let policy = limits(2, bytes, bytes * 2);
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, bytes), empty(), empty()]).unwrap();
    drop(owner.begin(terminal(0)).unwrap());
    assert!(owner.begin(terminal(1)).is_err());
    assert!(
        owner
            .skip_uninvoked(ExecutionOutputPhase::Pipeline, 0)
            .is_err()
    );
    assert!(owner.finish().is_err());
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, bytes), empty(), empty()]).unwrap();
    assert!(
        owner
            .begin(terminal(0))
            .unwrap()
            .finish(terminal(1))
            .is_err()
    );
    assert!(owner.finish().is_err());
}

#[test]
fn forgotten_reservation_cannot_disappear_from_owner_or_allow_another_begin() {
    let bytes = output_bytes(&terminal(0)).unwrap();
    let policy = limits(2, bytes, bytes * 2);
    let mut owner =
        ExecutionOutputBudget::new(policy, [phase(2, bytes), empty(), empty()]).unwrap();
    core::mem::forget(owner.begin(terminal(0)).unwrap());
    assert!(owner.begin(terminal(1)).is_err());
    assert!(owner.finish().is_err());

    let mut last = ExecutionOutputBudget::new(policy, [phase(1, bytes), empty(), empty()]).unwrap();
    core::mem::forget(last.begin(terminal(0)).unwrap());
    assert!(
        last.finish().is_err(),
        "the last obligation cannot vanish either"
    );
}

#[test]
fn reservation_binds_complete_internal_origin_not_just_row_kind() {
    for original in [pipeline(), time()] {
        let bytes = output_bytes(&original).unwrap();
        let phase_index = ExecutionOutputPhase::of(&original).index();
        let mut plan = [empty(); 3];
        plan[phase_index] = phase(1, bytes);
        let mut owner = ExecutionOutputBudget::new(limits(1, bytes, bytes), plan).unwrap();
        let mut substituted = original.clone();
        match &mut substituted {
            ExecutionOutputV1::Pipeline(output) => output.invocation.candidate_index += 1,
            ExecutionOutputV1::Time(output) => {
                output.invocation.trigger.action_hash = Hash::new(b"another action")
            }
            ExecutionOutputV1::Network(_) => unreachable!(),
        }
        assert!(owner.begin(original).unwrap().finish(substituted).is_err());
        assert!(owner.finish().is_err());
    }
}

#[test]
fn envelope_counts_all_network_events_block_approved_and_time_before_selection() {
    let n = output_bytes(&terminal(u32::MAX)).unwrap();
    let p = output_bytes(&pipeline()).unwrap();
    let t = output_bytes(&time()).unwrap();
    let envelope = ExecutionOutputEnvelope {
        pipeline_candidates_per_event: 3,
        max_time_invocations: 2,
        terminal_bytes: [n, p, t],
    };
    let policy = limits(17, n.max(p).max(t), 3 * n + 12 * p + 2 * t);
    assert_eq!(envelope.maximum_network_inputs(&policy).unwrap(), 3);
    assert_eq!(
        envelope.reservations(3, &policy).unwrap(),
        [phase(3, n), phase(12, p), phase(2, t)]
    );
    assert!(envelope.reservations(4, &policy).is_err());
    let smaller = ExecutionOutputLimits {
        max_total_output_bytes: policy.max_total_output_bytes - 1,
        ..policy
    };
    assert_eq!(envelope.maximum_network_inputs(&smaller).unwrap(), 2);
    assert!(envelope.reservations(3, &smaller).is_err());
    let count_limited = ExecutionOutputLimits {
        max_outputs: 16,
        ..policy
    };
    assert_eq!(envelope.maximum_network_inputs(&count_limited).unwrap(), 2);
}

#[test]
fn envelope_zero_capacity_is_not_a_perpetual_retry_or_an_internal_only_bypass() {
    let n = output_bytes(&terminal(u32::MAX)).unwrap();
    let p = output_bytes(&pipeline()).unwrap();
    let envelope = ExecutionOutputEnvelope {
        pipeline_candidates_per_event: 2,
        max_time_invocations: 0,
        terminal_bytes: [n, p, 0],
    };
    let internal_only = limits(2, n.max(p), 2 * p);
    assert_eq!(envelope.maximum_network_inputs(&internal_only).unwrap(), 0);
    envelope.reservations(0, &internal_only).unwrap();
    assert!(envelope.reservations(1, &internal_only).is_err());
    let cannot_fit_base = limits(1, n.max(p), 2 * p);
    assert_eq!(
        envelope.maximum_network_inputs(&cannot_fit_base).unwrap(),
        0
    );
    assert!(envelope.reservations(0, &cannot_fit_base).is_err());
}

#[test]
fn envelope_maximum_matches_exhaustive_small_terminal_plans() {
    for pipeline_count in 0..5 {
        for time_count in 0..5 {
            let envelope = ExecutionOutputEnvelope {
                pipeline_candidates_per_event: pipeline_count,
                max_time_invocations: time_count,
                terminal_bytes: [
                    5,
                    if pipeline_count == 0 { 0 } else { 7 },
                    if time_count == 0 { 0 } else { 11 },
                ],
            };
            for maximum_count in 1..25 {
                let policy = limits(maximum_count, 11, 100);
                let maximum = envelope.maximum_network_inputs(&policy).unwrap();
                for network in 1..=25 {
                    let pipeline = (network + 1) * pipeline_count;
                    let plan = [
                        phase(network, 5),
                        if pipeline == 0 {
                            empty()
                        } else {
                            phase(pipeline, 7)
                        },
                        if time_count == 0 {
                            empty()
                        } else {
                            phase(time_count, 11)
                        },
                    ];
                    assert_eq!(
                        ExecutionOutputBudget::new(policy, plan).is_ok(),
                        network <= maximum
                    );
                    assert_eq!(
                        envelope.reservations(network, &policy).is_ok(),
                        network <= maximum
                    );
                }
            }
        }
    }
}

#[test]
fn actual_network_rejection_uses_distinct_bounded_diagnostic_and_roundtrips() {
    use crate::block::execution_output::NETWORK_REJECTION_DIAGNOSTIC_OMITTED;
    for index in [0, u32::MAX] {
        let unit = output_bytes(&terminal(index)).unwrap();
        for payload in [1, 32_768] {
            let actual = attempted(index, payload);
            let mut budget = ExecutionOutputBudget::new(
                limits(1, unit, unit),
                [phase(1, unit), empty(), empty()],
            )
            .unwrap();
            let row = budget
                .begin(terminal(index))
                .unwrap()
                .finish_network_rejection(actual.clone())
                .unwrap();
            assert!(
                !row.is_output_limit_rejection(),
                "actual rejection is not healthy rollback"
            );
            if payload == 1 {
                assert_eq!(row, actual);
            } else {
                assert!(
                    matches!(row.result().as_ref(), Err(TransactionRejectionReason::LimitCheck(error))
                    if error.reason == NETWORK_REJECTION_DIAGNOSTIC_OMITTED)
                );
                assert!(output_bytes(&row).unwrap() <= unit);
            }
            let bytes = norito::encode_canonical(&row).unwrap();
            assert_eq!(
                norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
                row
            );
            let json = norito::json::to_json(&row).unwrap();
            assert_eq!(
                norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
                row
            );
            assert_eq!(budget.finish().unwrap(), (1, output_bytes(&row).unwrap()));
        }
    }
}

#[test]
fn rejection_diagnostic_exact_fit_and_shared_surplus_preserve_later_terminals() {
    let unit = output_bytes(&terminal(0)).unwrap();
    let actual = attempted(0, 1024);
    let exact = output_bytes(&actual).unwrap();
    for room in [exact, exact - 1] {
        let mut budget = ExecutionOutputBudget::new(
            limits(2, exact, room + unit),
            [phase(2, unit), empty(), empty()],
        )
        .unwrap();
        let row = budget
            .begin(terminal(0))
            .unwrap()
            .finish_network_rejection(actual.clone())
            .unwrap();
        assert_eq!(row == actual, room == exact);
        assert!(!row.is_output_limit_rejection());
        let later = match budget
            .begin(terminal(1))
            .unwrap()
            .finish(terminal(1))
            .unwrap()
        {
            ReservedExecutionOutput::Accepted(row) => row,
            ReservedExecutionOutput::OutputLimit(_) => panic!("later terminal was reserved"),
        };
        assert_eq!(
            budget.finish().unwrap(),
            (
                2,
                output_bytes(&row).unwrap() + output_bytes(&later).unwrap()
            )
        );
    }
}

#[test]
fn rejection_projection_refuses_success_foreign_origin_and_internal_rows() {
    let mut success = attempted(0, 1);
    let ExecutionOutputV1::Network(row) = &mut success else {
        unreachable!()
    };
    row.result = TransactionResult::new(Ok(Vec::new()));
    let unit = output_bytes(&terminal(0)).unwrap();
    for invalid in [success, attempted(1, 1), time()] {
        let mut budget =
            ExecutionOutputBudget::new(limits(1, unit, unit), [phase(1, unit), empty(), empty()])
                .unwrap();
        assert!(
            budget
                .begin(terminal(0))
                .unwrap()
                .finish_network_rejection(invalid)
                .is_err()
        );
        assert!(
            budget.finish().is_err(),
            "refused rejection still owns an unresolved slot"
        );
    }
}

fn assert_rejected_side_channel_poisoned(actual: ExecutionOutputV1) {
    let terminal_bytes = output_bytes(&terminal(0)).unwrap();
    let actual_bytes = output_bytes(&actual).unwrap();
    // Test both a roomy reservation and a bounded fallback corridor: overflow
    // must not launder invalid applied receipts or callback completions.
    for row_bytes in [terminal_bytes.max(actual_bytes), terminal_bytes] {
        let mut budget = ExecutionOutputBudget::new(
            limits(2, row_bytes, row_bytes + terminal_bytes),
            [phase(2, terminal_bytes), empty(), empty()],
        )
        .unwrap();
        let error = budget
            .begin(terminal(0))
            .unwrap()
            .finish_network_rejection(actual.clone())
            .unwrap_err();
        assert_eq!(
            error,
            "rejection settlement retains successful business output"
        );
        assert!(
            budget.begin(terminal(1)).is_err(),
            "a refused earlier slot poisons later admission"
        );
        assert!(
            budget.finish().is_err(),
            "invalid rejection cannot become a retained bounded terminal"
        );
    }
}

#[test]
fn network_rejection_refuses_batch_receipts_before_fitting_or_fallback() {
    use crate::events::data::prelude::{
        AssetBatchTransferLegStatus, AssetBatchTransferOutcome, AssetBatchTransferRejection,
        AssetBatchTransferRejectionCode,
    };
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x53; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let authority = crate::account::AccountId::new(key.public_key().clone());
    let receipt = AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "rolled-back-business-leg".into(),
        asset: crate::asset::AssetId::new(
            crate::asset::AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("rejection-test", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            authority.clone(),
        ),
        destination: authority,
        amount: iroha_primitives::numeric::Quantity::from(1_u32),
        status: AssetBatchTransferLegStatus::Applied,
    };
    for status in [
        AssetBatchTransferLegStatus::Applied,
        AssetBatchTransferLegStatus::Rejected(AssetBatchTransferRejection {
            code: AssetBatchTransferRejectionCode::InsufficientFunds,
            message: "rolled-back independent leg".into(),
        }),
    ] {
        for diagnostic_bytes in [1, 32_768] {
            let mut actual = attempted(0, diagnostic_bytes);
            let ExecutionOutputV1::Network(row) = &mut actual else {
                unreachable!()
            };
            let mut receipt = receipt.clone();
            receipt.status = status.clone();
            row.result.set_batch_transfer_outcomes(vec![receipt]);
            assert_rejected_side_channel_poisoned(actual);
        }
    }
}

#[test]
fn network_rejection_refuses_success_and_failure_completions_before_fitting_or_fallback() {
    use crate::{
        block::execution_output::InvocationCompletionV1,
        events::trigger_completed::TriggerCompletedOutcome,
    };
    for outcome in [
        TriggerCompletedOutcome::Success,
        TriggerCompletedOutcome::Failure("actual nested failure".into()),
    ] {
        for diagnostic_bytes in [1, 32_768] {
            let mut actual = attempted(0, diagnostic_bytes);
            let ExecutionOutputV1::Network(row) = &mut actual else {
                unreachable!()
            };
            row.completions.push(InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: "rolled-back-callback".parse().unwrap(),
                outcome: outcome.clone(),
            });
            assert_rejected_side_channel_poisoned(actual);
        }
    }
}

mod internal_rejection {
    use super::*;
    use crate::{
        ValidationFail,
        block::execution_output::{InvocationCompletionV1, TriggerFailureRootV1},
        events::trigger_completed::TriggerCompletedOutcome,
        transaction::ExecutionStep,
    };

    fn parts_mut(
        row: &mut ExecutionOutputV1,
    ) -> (
        &mut TransactionResult,
        &mut Option<TriggerFailureRootV1>,
        &mut Vec<InvocationCompletionV1>,
    ) {
        match row {
            ExecutionOutputV1::Pipeline(row) => {
                (&mut row.result, &mut row.failure_root, &mut row.completions)
            }
            ExecutionOutputV1::Time(row) => {
                (&mut row.result, &mut row.failure_root, &mut row.completions)
            }
            ExecutionOutputV1::Network(_) => panic!("internal fixture"),
        }
    }

    fn actual(terminal: &ExecutionOutputV1, bytes: usize, returned: bool) -> ExecutionOutputV1 {
        let mut row = terminal.clone();
        let (result, root, completions) = parts_mut(&mut row);
        *result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("e".repeat(bytes)),
        )));
        let step = ExecutionStep(
            vec![crate::isi::Log::new(crate::prelude::Level::INFO, "r".repeat(bytes)).into()]
                .into(),
        );
        *root = Some(if returned {
            TriggerFailureRootV1::ReturnedBeforeRollback(step)
        } else {
            TriggerFailureRootV1::DeclaredInstructionProjection(step)
        });
        completions[0].outcome = TriggerCompletedOutcome::Failure("actual callback failure".into());
        row
    }

    fn budget(
        terminal: &ExecutionOutputV1,
        count: u32,
        row: u64,
        total: u64,
    ) -> ExecutionOutputBudget {
        let mut phases = [empty(); 3];
        phases[ExecutionOutputPhase::of(terminal).index()] =
            phase(count, output_bytes(terminal).unwrap());
        ExecutionOutputBudget::new(limits(count, row, total), phases).unwrap()
    }

    fn diagnostic_pointers(row: &ExecutionOutputV1) -> (*const u8, *const u8) {
        let Err(TransactionRejectionReason::LimitCheck(error)) = row.result().as_ref() else {
            panic!("bounded diagnostic")
        };
        let TriggerCompletedOutcome::Failure(completion) = &row.completions()[0].outcome else {
            panic!("root failure")
        };
        (error.reason.as_ptr(), completion.as_ptr())
    }

    #[test]
    fn both_internal_rejections_preserve_full_rows_or_reuse_exact_terminal_allocations() {
        for terminal in [pipeline(), time()] {
            let unit = output_bytes(&terminal).unwrap();
            for returned in [false, true] {
                let actual = actual(&terminal, 1024, returned);
                let exact = output_bytes(&actual).unwrap();
                assert!(exact > unit);
                for row_limit in [exact, exact - 1, unit] {
                    let reserved = terminal.clone();
                    let pointers = diagnostic_pointers(&reserved);
                    let mut owner = budget(&reserved, 1, row_limit, row_limit);
                    let row = owner
                        .begin(reserved)
                        .unwrap()
                        .finish_internal_rejection(actual.clone())
                        .unwrap();
                    assert!(!row.is_output_limit_rejection());
                    if row_limit == exact {
                        assert_eq!(row, actual);
                        assert!(!row.is_internal_rejection_diagnostic_omitted());
                    } else {
                        assert!(row.is_internal_rejection_diagnostic_omitted());
                        assert_eq!(
                            diagnostic_pointers(&row),
                            pointers,
                            "both fallback strings retain their preowned allocations"
                        );
                        assert!(output_bytes(&row).unwrap() <= unit);
                    }
                    let encoded = norito::encode_canonical(&row).unwrap();
                    assert_eq!(
                        norito::decode_canonical::<ExecutionOutputV1>(&encoded).unwrap(),
                        row
                    );
                    let json = norito::json::to_json(&row).unwrap();
                    assert_eq!(
                        norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
                        row
                    );
                    assert_eq!(owner.finish().unwrap(), (1, output_bytes(&row).unwrap()));
                }
            }
        }
    }

    #[test]
    fn shared_internal_rejection_surplus_preserves_the_later_terminal() {
        for terminal in [pipeline(), time()] {
            let unit = output_bytes(&terminal).unwrap();
            let actual = actual(&terminal, 1024, false);
            let exact = output_bytes(&actual).unwrap();
            let mut later_terminal = terminal.clone();
            match &mut later_terminal {
                ExecutionOutputV1::Pipeline(row) => row.invocation.candidate_index += 1,
                ExecutionOutputV1::Time(row) => row.invocation.schedule_index += 1,
                ExecutionOutputV1::Network(_) => unreachable!(),
            }
            assert_eq!(output_bytes(&later_terminal).unwrap(), unit);
            for available in [exact, exact - 1] {
                let mut owner = budget(&terminal, 2, exact, available + unit);
                let first = owner
                    .begin(terminal.clone())
                    .unwrap()
                    .finish_internal_rejection(actual.clone())
                    .unwrap();
                assert_eq!(first == actual, available == exact);
                assert_eq!(
                    first.is_internal_rejection_diagnostic_omitted(),
                    available != exact
                );
                let ReservedExecutionOutput::Accepted(later) = owner
                    .begin(later_terminal.clone())
                    .unwrap()
                    .finish(later_terminal.clone())
                    .unwrap()
                else {
                    panic!("later healthy terminal must retain its reservation")
                };
                assert!(later.is_output_limit_rejection());
                assert_eq!(
                    owner.finish().unwrap(),
                    (
                        2,
                        output_bytes(&first).unwrap() + output_bytes(&later).unwrap()
                    )
                );
            }
        }
    }

    fn assert_refused(terminal: &ExecutionOutputV1, invalid: &ExecutionOutputV1) {
        let unit = output_bytes(terminal).unwrap();
        let roomy = output_bytes(invalid).unwrap().max(unit);
        for row_limit in [unit, roomy] {
            let mut owner = budget(terminal, 2, row_limit, row_limit + unit);
            assert!(
                owner
                    .begin(terminal.clone())
                    .unwrap()
                    .finish_internal_rejection(invalid.clone())
                    .is_err()
            );
            assert!(
                owner.begin(terminal.clone()).is_err(),
                "refusal poisons later work"
            );
            assert!(
                owner.finish().is_err(),
                "invalid rows cannot escape through fallback"
            );
        }
    }

    #[test]
    fn internal_rejection_refuses_every_foreign_origin_and_successful_owner() {
        for terminal in [pipeline(), time()] {
            assert_refused(&terminal, &attempted(0, 32_768));
            let other = if matches!(terminal, ExecutionOutputV1::Pipeline(_)) {
                time()
            } else {
                pipeline()
            };
            assert_refused(&terminal, &actual(&other, 32_768, false));
            for fault in 0..6 {
                let mut invalid = actual(&terminal, 32_768, false);
                match &mut invalid {
                    ExecutionOutputV1::Pipeline(row) => match fault {
                        0 => row.invocation.candidate_index += 1,
                        1 => row.invocation.event = PipelineEventPositionV1::Network(0),
                        2 => row.invocation.trigger.registered_at_height += 1,
                        3 => row.invocation.trigger.action_hash = Hash::new(b"foreign action"),
                        4 => row.invocation.trigger.trigger_id = "foreign-root".parse().unwrap(),
                        _ => row.result = TransactionResult::new(Ok(Vec::new())),
                    },
                    ExecutionOutputV1::Time(row) => match fault {
                        0 => row.invocation.schedule_index += 1,
                        1 => row.invocation.event.interval.length_ms += 1,
                        2 => row.invocation.trigger.registered_at_height += 1,
                        3 => row.invocation.trigger.action_hash = Hash::new(b"foreign action"),
                        4 => row.invocation.trigger.trigger_id = "foreign-root".parse().unwrap(),
                        _ => row.result = TransactionResult::new(Ok(Vec::new())),
                    },
                    ExecutionOutputV1::Network(_) => unreachable!(),
                }
                assert_refused(&terminal, &invalid);
            }
        }
    }

    #[test]
    fn internal_rejection_refuses_missing_foreign_or_rolled_back_completion_roots() {
        for terminal in [pipeline(), time()] {
            assert_refused(&terminal, &terminal); // Healthy overflow is never actual rejection.
            for fault in 0..8 {
                let mut invalid = actual(&terminal, 32_768, false);
                let (_, root, completions) = parts_mut(&mut invalid);
                match fault {
                    0 => *root = None,
                    1 => *root = Some(TriggerFailureRootV1::OmittedByOutputLimit),
                    2 => *root = Some(TriggerFailureRootV1::OmittedAfterRejection),
                    3 => completions.clear(),
                    4 => completions[0].outcome = TriggerCompletedOutcome::Success,
                    5 => completions[0].trigger_id = "foreign-root".parse().unwrap(),
                    6 => completions[0].callback_index = 1,
                    _ => completions.push(completions[0].clone()),
                }
                assert_refused(&terminal, &invalid);
            }
        }
    }

    #[test]
    fn internal_rejection_refuses_both_applied_and_rejected_business_receipts() {
        use crate::events::data::prelude::{
            AssetBatchTransferLegStatus, AssetBatchTransferOutcome, AssetBatchTransferRejection,
            AssetBatchTransferRejectionCode,
        };
        let key =
            iroha_crypto::KeyPair::from_seed(vec![0x64; 32], iroha_crypto::Algorithm::Ed25519);
        let account = crate::account::AccountId::new(key.public_key().clone());
        for terminal in [pipeline(), time()] {
            for status in [
                AssetBatchTransferLegStatus::Applied,
                AssetBatchTransferLegStatus::Rejected(AssetBatchTransferRejection {
                    code: AssetBatchTransferRejectionCode::InsufficientFunds,
                    message: "rejected business leg".into(),
                }),
            ] {
                let mut invalid = actual(&terminal, 32_768, true);
                parts_mut(&mut invalid).0.set_batch_transfer_outcomes(vec![
                    AssetBatchTransferOutcome {
                        leg_index: 0,
                        leg_id: "rolled-back-leg".into(),
                        asset: crate::asset::AssetId::of(
                            crate::asset::AssetDefinitionId::derive_from_components(
                                iroha_model_base::domain::DomainId::try_new(
                                    "internal-rejection",
                                    "universal",
                                )
                                .unwrap(),
                                "coin".parse().unwrap(),
                            ),
                            account.clone(),
                        ),
                        destination: account.clone(),
                        amount: iroha_primitives::numeric::Quantity::from(1_u32),
                        status,
                    },
                ]);
                assert_refused(&terminal, &invalid);
            }
        }
    }

    #[test]
    fn internal_rejection_accounting_contradiction_abandons_its_owner() {
        for terminal in [pipeline(), time()] {
            let unit = output_bytes(&terminal).unwrap();
            let mut owner = budget(&terminal, 1, unit, unit);
            let mut reservation = owner.begin(terminal.clone()).unwrap();
            // Private adversarial corruption proves a failed size check is not
            // silently converted into a canonical rejection or a sealed owner.
            reservation.terminal_bytes = 0;
            assert_eq!(
                reservation
                    .finish_internal_rejection(actual(&terminal, 1024, false))
                    .unwrap_err(),
                "internal rejection exceeds its preowned terminal bytes"
            );
            assert!(owner.finish().is_err());
        }
    }
}
