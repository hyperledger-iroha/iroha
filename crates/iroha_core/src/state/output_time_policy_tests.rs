//! Real disposable failure-policy overlays, without a lifecycle publication claim.

use super::*;
use iroha_data_model::events::time::{TimeEvent, TimeInterval};
use std::time::Duration;

use super::super::time_test_support as support;

#[test]
fn lifecycle_failure_policy_captures_only_retained_retry_or_exhaustion() {
    use crate::smartcontracts::isi::triggers::set::lifecycle_journal::{
        LifecycleChange, LifecycleJournalLimits,
    };
    use iroha_data_model::block::execution_output::{
        TriggerLifecycleOriginV1, TriggerLifecyclePolicyV1,
    };
    for (previous, expected_policy) in [
        (None, TriggerLifecyclePolicyV1::AdvanceTimeRetry),
        (Some((3, 20)), TriggerLifecyclePolicyV1::ExhaustTimeRetry),
    ] {
        let (state, id) = support::state_fixture(previous, true);
        let mut block = state.block(support::header());
        let invocation = invocation(&block, &id);
        let pool = iroha_allocation::AllocationBudget::new(2_000_000);
        let policy = invocation
            .prepare_failure_policy_with(&mut block, 2, 42, |tx| {
                tx.world
                    .triggers
                    .enable_lifecycle_journal(
                        pool.clone(),
                        LifecycleJournalLimits {
                            effects: 8,
                            total_frame_bytes: 131_072,
                            scratch_bytes: 262_144,
                            encode: support::limits(),
                        },
                    )
                    .unwrap();
            })
            .unwrap()
            .unwrap();
        let effects = policy
            .transaction
            .as_ref()
            .unwrap()
            .world
            .triggers
            .lifecycle_effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].trigger_id.id, id);
        assert_eq!(
            effects[0].origin,
            TriggerLifecycleOriginV1::InvocationPolicy {
                callback_index: 0,
                policy: expected_policy
            }
        );
        assert!(match (&effects[0].change, expected_policy) {
            (LifecycleChange::Update(_, after), TriggerLifecyclePolicyV1::AdvanceTimeRetry) =>
                after.retry_state.unwrap().attempt == 1,
            (LifecycleChange::Remove(_), TriggerLifecyclePolicyV1::ExhaustTimeRetry) => true,
            _ => false,
        });
        drop(policy);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::retry(&block, &id), previous);
    }
}

#[test]
fn lifecycle_pipeline_quarantine_is_a_separate_disposable_policy_effect() {
    use crate::smartcontracts::isi::triggers::{
        set::lifecycle_journal::{LifecycleChange, LifecycleJournalLimits},
        specialized::{SpecializedAction, SpecializedTrigger},
    };
    use iroha_data_model::block::execution_output::{
        PipelineEventPositionV1, TriggerLifecycleOriginV1, TriggerLifecyclePolicyV1,
    };
    use iroha_data_model::prelude::*;
    let (state, _) = support::state_fixture(None, false);
    let mut block = state.block(support::header());
    let id: TriggerId = "quarantine_journal".parse().unwrap();
    {
        let mut tx = block.transaction();
        let mut action = SpecializedAction::new(
            Executable::Instructions(Vec::<InstructionBox>::new().into()),
            Repeats::Exactly(1),
            iroha_test_samples::ALICE_ID.clone(),
            PipelineEventFilterBox::Transaction(
                iroha_data_model::events::pipeline::TransactionEventFilter::new(),
            ),
        )
        .unwrap();
        action.metadata.insert(
            "__registered_block_height".parse().unwrap(),
            Json::from(1_u64),
        );
        tx.world
            .triggers
            .add_pipeline_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap();
        tx.apply();
    }
    let invocation = InternalInvocation::Pipeline(PipelineInvocationV1 {
        event: PipelineEventPositionV1::BlockApproved,
        candidate_index: 0,
        trigger: pipeline_trigger_use_v1(&block.world.triggers, &id, 2).unwrap(),
    });
    let pool = iroha_allocation::AllocationBudget::new(2_000_000);
    let policy = invocation
        .prepare_failure_policy_with(&mut block, 2, 42, |tx| {
            tx.world
                .triggers
                .enable_lifecycle_journal(
                    pool.clone(),
                    LifecycleJournalLimits {
                        effects: 8,
                        total_frame_bytes: 131_072,
                        scratch_bytes: 262_144,
                        encode: support::limits(),
                    },
                )
                .unwrap();
        })
        .unwrap()
        .unwrap();
    let effects = policy
        .transaction
        .as_ref()
        .unwrap()
        .world
        .triggers
        .lifecycle_effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].origin,
        TriggerLifecycleOriginV1::InvocationPolicy {
            callback_index: 0,
            policy: TriggerLifecyclePolicyV1::QuarantinePipeline
        }
    );
    let LifecycleChange::Update(before, after) = &effects[0].change else {
        panic!("quarantine update");
    };
    assert_ne!(before.action_hash, after.action_hash);
    assert_eq!(
        after.action.as_bytes(),
        iroha_data_model::block::execution_output::lifecycle_hash_frame(after.action_hash.unwrap())
            .unwrap()
            .as_slice()
    );
    assert_eq!(
        policy
            .transaction
            .as_ref()
            .unwrap()
            .world
            .triggers
            .pipeline_triggers()
            .get(&id)
            .unwrap()
            .metadata
            .get("__enabled"),
        Some(&Json::from(false))
    );
    drop(policy);
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(
        block
            .world
            .triggers
            .pipeline_triggers()
            .get(&id)
            .unwrap()
            .metadata
            .get("__enabled")
            .is_none()
    );
}

fn invocation(
    block: &StateBlock<'_>,
    id: &iroha_data_model::trigger::TriggerId,
) -> InternalInvocation {
    InternalInvocation::Time(TimeInvocationV1 {
        schedule_index: 0,
        event: TimeEvent {
            interval: TimeInterval::new(Duration::from_millis(10), Duration::from_millis(32)),
        },
        trigger: time_trigger_use_v1(&block.world.triggers, id, 2).unwrap(),
    })
}

#[test]
fn time_policy_prepared_retry_is_disposable_and_legacy_apply_still_commits() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    let invocation = invocation(&block, &id);
    let policy = invocation
        .prepare_failure_policy(&mut block, 2, 42)
        .unwrap()
        .unwrap();
    let retry = policy
        .transaction
        .as_ref()
        .unwrap()
        .world
        .triggers
        .time_triggers()
        .get(&id)
        .unwrap()
        .retry_state
        .unwrap();
    assert_eq!((retry.retries_used, retry.next_retry_at_ms), (1, 47));
    drop(policy);
    assert_eq!(support::retry(&block, &id), None);
    assert_eq!(support::original_action(&block, &id), original);
    invocation.apply_failure_policy(&mut block, 2, 42).unwrap();
    assert_eq!(support::retry(&block, &id), Some((1, 47)));
    assert_eq!(support::original_action(&block, &id), original);
}

#[test]
fn time_policy_reads_parent_retry_only_after_failed_body_overlay_is_dropped() {
    let (state, id) = support::state_fixture(Some((1, 20)), true);
    let mut block = state.block(support::header());
    let invocation = invocation(&block, &id);
    {
        let mut body = OutputTransaction::new(&mut block).unwrap();
        assert!(
            body.transaction
                .as_mut()
                .unwrap()
                .world
                .triggers
                .set_time_trigger_retry_state(
                    &id,
                    Some(TimeTriggerRetryState {
                        retries_used: 3,
                        next_retry_at_ms: 999
                    }),
                )
        );
        // Actual body rollback precedes the independent policy observation.
        drop(body);
    }
    let policy = invocation
        .prepare_failure_policy(&mut block, 2, 42)
        .unwrap()
        .unwrap();
    let retry = policy
        .transaction
        .as_ref()
        .unwrap()
        .world
        .triggers
        .time_triggers()
        .get(&id)
        .unwrap()
        .retry_state
        .unwrap();
    assert_eq!((retry.retries_used, retry.next_retry_at_ms), (2, 47));
    drop(policy);
    assert_eq!(support::retry(&block, &id), Some((1, 20)));
}

#[test]
fn time_policy_exhaustion_drop_restores_action_and_apply_removes_it() {
    let (state, id) = support::state_fixture(Some((3, 20)), true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    let invocation = invocation(&block, &id);
    let policy = invocation
        .prepare_failure_policy(&mut block, 2, 42)
        .unwrap()
        .unwrap();
    assert!(
        policy
            .transaction
            .as_ref()
            .unwrap()
            .world
            .triggers
            .time_triggers()
            .get(&id)
            .is_none()
    );
    drop(policy);
    assert_eq!(support::retry(&block, &id), Some((3, 20)));
    assert_eq!(support::original_action(&block, &id), original);
    invocation
        .prepare_failure_policy(&mut block, 2, 42)
        .unwrap()
        .unwrap()
        .apply()
        .unwrap();
    assert!(block.world.triggers.time_triggers().get(&id).is_none());
}

#[test]
fn time_policy_no_policy_and_stale_original_do_not_prepare_mutations() {
    let (state, id) = support::state_fixture(None, false);
    let mut block = state.block(support::header());
    let invocation = invocation(&block, &id);
    assert!(
        invocation
            .prepare_failure_policy(&mut block, 2, 42)
            .unwrap()
            .is_none()
    );
    assert_eq!(support::retry(&block, &id), None);
    let mut changed = OutputTransaction::new(&mut block).unwrap();
    assert!(
        changed
            .transaction
            .as_mut()
            .unwrap()
            .world
            .triggers
            .set_time_trigger_retry_state(
                &id,
                Some(TimeTriggerRetryState {
                    retries_used: 1,
                    next_retry_at_ms: 44
                }),
            )
    );
    changed.apply().unwrap();
    assert!(
        invocation
            .prepare_failure_policy(&mut block, 2, 42)
            .is_err()
    );
    assert_eq!(support::retry(&block, &id), Some((1, 44)));
}
