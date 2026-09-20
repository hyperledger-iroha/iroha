//! Actual internal rejection policy after root/DFS rollback and diagnostic fitting.
//! These exercise private execution; publication and complete admission remain gated.

use super::*;
use iroha_data_model::{
    ValidationFail,
    block::execution_output::INTERNAL_REJECTION_DIAGNOSTIC_OMITTED,
    events::{data::prelude::AccountEventFilter, trigger_completed::TriggerCompletedOutcome},
    transaction::error::TransactionRejectionReason,
};

fn assert_missing_unregister(reason: &TransactionRejectionReason, missing: &TriggerId) {
    // The initial executor authenticates ownership before dispatching Unregister.
    // An absent trigger therefore fails capability validation before the ISI runs.
    let TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message)) = reason
    else {
        panic!("expected actual missing-trigger unregister rejection, got {reason:?}");
    };
    assert_eq!(
        message,
        &format!("permission references unknown trigger `{missing}`")
    );
}

#[test]
fn pipeline_successful_root_then_failed_data_child_rolls_back_both_before_quarantine() {
    let _guard = witness::exec_witness_guard();
    let root: TriggerId = "pipeline_dfs_root".parse().unwrap();
    let child: TriggerId = "pipeline_data_child".parse().unwrap();
    let missing: TriggerId = "pipeline_dfs_absent".parse().unwrap();
    let root_body = vec![write("dfs_root_rolled_back")];
    // Register through the actual ISI: the own-account filter has authentic
    // authority scope. The root's real metadata event selects this child.
    let child_action = Action::new(
        vec![
            write("dfs_child_rolled_back"),
            Unregister::trigger(missing.clone()).into(),
        ],
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        AccountEventFilter::new().for_account(ALICE_ID.clone()),
    )
    .unwrap();
    let (state, source) = pipeline_fixture(
        65_536,
        vec![
            callback("pipeline_dfs_root", root_body.clone(), block_filter(), 1),
            Trigger::new(child.clone(), child_action),
        ],
    );
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute_all(&mut block, &source);
    let rows = &retained(&block).rows;
    assert_eq!(rows.len(), 2, "Network plus one whole Pipeline invocation");
    let ExecutionOutputV1::Pipeline(output) = &rows[1] else {
        panic!("actual Pipeline row")
    };
    assert_missing_unregister(output.result.as_ref().unwrap_err(), &missing);
    assert_eq!(
        output.failure_root,
        Some(TriggerFailureRootV1::ReturnedBeforeRollback(ExecutionStep(
            root_body.into()
        ),))
    );
    assert!(!rows[1].is_output_limit_rejection());
    assert!(!rows[1].is_internal_rejection_diagnostic_omitted());
    assert!(output.result.batch_transfer_outcomes().is_empty());
    assert_eq!(output.completions.len(), 1);
    let completion = &output.completions[0];
    assert_eq!(completion.callback_index, 0);
    assert_eq!(completion.trigger_id, root);
    assert!(matches!(
        completion.outcome,
        TriggerCompletedOutcome::Failure(_)
    ));
    let completed: Vec<_> = block
        .world
        .external_event_buf
        .iter()
        .filter_map(|event| {
            if let EventBox::TriggerCompleted(event) = event {
                Some(event)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        completed.len(),
        1,
        "rolled-back child callbacks are not applied events"
    );
    assert_eq!(completed[0].trigger_id(), &root);
    assert_eq!(*completed[0].step_index(), 0);
    assert_eq!(
        completed[0].trigger_execution_hash(),
        &HashOf::from_untyped_unchecked(
            output
                .invocation
                .execution_call_hash(source.hash())
                .unwrap()
        )
    );
    assert_eq!(completed[0].outcome(), &completion.outcome);
    for key in ["dfs_root_rolled_back", "dfs_child_rolled_back"] {
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(key)
                .is_none()
        );
    }
    let root_action = block.world.triggers.pipeline_triggers().get(&root).unwrap();
    assert_eq!(
        root_action.repeats,
        Repeats::Exactly(1),
        "pre-DFS debit rolled back"
    );
    assert!(!crate::smartcontracts::isi::triggers::trigger_is_enabled(
        &root_action.metadata
    ));
    let child_action = block.world.triggers.data_triggers().get(&child).unwrap();
    assert_eq!(child_action.repeats, Repeats::Exactly(2));
    assert!(crate::smartcontracts::isi::triggers::trigger_is_enabled(
        &child_action.metadata
    ));
    assert_eq!(
        block.committed_fragment_count(),
        fragments + 2,
        "Network and quarantine only"
    );
    assert!(block.gas_used_in_block > 0);
    assert!(block.batch_transfer_outcomes.is_empty());
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
    let view = state.view();
    let original = view.world.triggers.pipeline_triggers().get(&root).unwrap();
    assert!(crate::smartcontracts::isi::triggers::trigger_is_enabled(
        &original.metadata
    ));
    assert_eq!(original.repeats, Repeats::Exactly(1));
}

#[test]
fn oversized_real_pipeline_rejection_omits_diagnostic_but_quarantines_and_keeps_sibling() {
    let _guard = witness::exec_witness_guard();
    let bad: TriggerId = "a_large_failure".parse().unwrap();
    let sibling: TriggerId = "b_healthy_sibling".parse().unwrap();
    let missing: TriggerId = "large_failure_absent".parse().unwrap();
    // Same real body in both cases. The roomy control proves the failure is
    // missing-trigger unregister, not gas admission or a fabricated rejection.
    for (bytes, omitted) in [(65_536, false), (4_096, true)] {
        let body = vec![
            write("large_failure_rolled_back"),
            Log::new(Level::DEBUG, "x".repeat(16_384)).into(),
            Unregister::trigger(missing.clone()).into(),
        ];
        let (state, source) = pipeline_fixture(
            bytes,
            vec![
                callback("a_large_failure", body, block_filter(), 1),
                callback(
                    "b_healthy_sibling",
                    vec![write("healthy_sibling_effect")],
                    block_filter(),
                    1,
                ),
            ],
        );
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute_all(&mut block, &source);
        let rows = &retained(&block).rows;
        assert_eq!(rows.len(), 3);
        let ExecutionOutputV1::Pipeline(failed) = &rows[1] else {
            panic!("failed Pipeline row")
        };
        let ExecutionOutputV1::Pipeline(healthy) = &rows[2] else {
            panic!("healthy sibling row")
        };
        assert_eq!(failed.invocation.trigger.trigger_id, bad);
        assert_eq!(healthy.invocation.trigger.trigger_id, sibling);
        assert_eq!(failed.invocation.candidate_index, 0);
        assert_eq!(healthy.invocation.candidate_index, 1);
        assert!(healthy.result.is_ok());
        assert!(!rows[1].is_output_limit_rejection());
        assert_eq!(rows[1].is_internal_rejection_diagnostic_omitted(), omitted);
        assert!(u64::try_from(norito::canonical_frame_len(&rows[1]).unwrap()).unwrap() <= bytes);
        if omitted {
            assert_eq!(
                failed.failure_root,
                Some(TriggerFailureRootV1::OmittedAfterRejection)
            );
            let TransactionRejectionReason::LimitCheck(error) = failed.result.as_ref().unwrap_err()
            else {
                panic!("bounded actual rejection diagnostic")
            };
            assert_eq!(error.reason, INTERNAL_REJECTION_DIAGNOSTIC_OMITTED);
            assert_eq!(
                failed.completions[0].outcome,
                TriggerCompletedOutcome::Failure(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.to_owned())
            );
        } else {
            assert_missing_unregister(failed.result.as_ref().unwrap_err(), &missing);
            assert!(matches!(
                failed.failure_root,
                Some(TriggerFailureRootV1::DeclaredInstructionProjection(_))
            ));
        }
        assert_eq!(failed.completions.len(), 1);
        assert_eq!(failed.completions[0].callback_index, 0);
        assert_eq!(failed.completions[0].trigger_id, bad);
        assert!(failed.result.batch_transfer_outcomes().is_empty());
        let completed: Vec<_> = block
            .world
            .external_event_buf
            .iter()
            .filter_map(|event| {
                if let EventBox::TriggerCompleted(event) = event {
                    Some(event)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].trigger_id(), &bad);
        assert_eq!(completed[0].outcome(), &failed.completions[0].outcome);
        assert_eq!(
            completed[0].trigger_execution_hash(),
            &HashOf::from_untyped_unchecked(
                failed
                    .invocation
                    .execution_call_hash(source.hash())
                    .unwrap()
            )
        );
        assert_eq!(completed[1].trigger_id(), &sibling);
        assert_eq!(completed[1].outcome(), &TriggerCompletedOutcome::Success);
        let action = block.world.triggers.pipeline_triggers().get(&bad).unwrap();
        assert_eq!(action.repeats, Repeats::Exactly(1));
        assert!(!crate::smartcontracts::isi::triggers::trigger_is_enabled(
            &action.metadata
        ));
        assert!(
            block
                .world
                .triggers
                .pipeline_triggers()
                .get(&sibling)
                .is_none()
        );
        let account = block.world.account(&ALICE_ID).unwrap();
        assert!(
            account
                .metadata()
                .get("large_failure_rolled_back")
                .is_none()
        );
        assert_eq!(
            account.metadata().get("healthy_sibling_effect"),
            Some(&Json::new(1))
        );
        assert_eq!(block.committed_fragment_count(), fragments + 3);
        assert!(block.gas_used_in_block > 0);
        assert!(matches!(
            block.commit().unwrap_err(),
            TransactionsBlockError::ExecutionOutputCapacity
        ));
        let view = state.view();
        assert!(
            view.world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("healthy_sibling_effect")
                .is_none()
        );
        assert!(crate::smartcontracts::isi::triggers::trigger_is_enabled(
            &view
                .world
                .triggers
                .pipeline_triggers()
                .get(&bad)
                .unwrap()
                .metadata
        ));
    }
}
