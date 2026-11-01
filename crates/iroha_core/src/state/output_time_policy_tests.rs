//! Real disposable failure-policy overlays, without a lifecycle publication claim.

use super::*;
use iroha_data_model::events::time::{TimeEvent, TimeInterval};
use std::time::Duration;

mod support {
    //! Real-State fixtures for the Time failure-policy seam.
    //! Typed insertion supplies component data; it grants no genesis or registration authority.

    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::triggers::{
            set::SetReadOnly,
            specialized::{SpecializedAction, SpecializedTrigger, TimeTriggerRetryState},
        },
        state::{State, StateBlock, World},
    };
    use iroha_data_model::{
        block::BlockHeader,
        events::time::{ExecutionTime, Schedule},
        prelude::*,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    use std::{
        num::{NonZeroU32, NonZeroU64},
        time::Duration,
    };

    pub(super) fn state_fixture(retries: Option<(u32, u64)>, policy: bool) -> (State, TriggerId) {
        let mut world = World::default();
        world
            .set_parameter_for_testing(
                crate::sumeragi::test_chain::crypto_fixture_policy()
                    .into_parameter()
                    .unwrap(),
            )
            .unwrap();
        let id: TriggerId = "time_policy_retry".parse().unwrap();
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            let mut action = SpecializedAction::new(
                Executable::Instructions(Vec::<InstructionBox>::new().into()),
                Repeats::Exactly(2),
                ALICE_ID.clone(),
                TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
                    Duration::from_millis(10),
                ))),
            )
            .unwrap();
            action.metadata.insert(
                "__registered_block_height".parse().unwrap(),
                Json::from(1_u64),
            );
            action
                .metadata
                .insert("__registered_at_ms".parse().unwrap(), Json::from(10_u64));
            action.retry_policy = policy.then_some(TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::new(3).unwrap(),
                retry_after_ms: NonZeroU64::new(5).unwrap(),
            });
            assert!(
                tx.add_time_trigger(SpecializedTrigger::new(id.clone(), action))
                    .unwrap()
            );
            if let Some((retries_used, next_retry_at_ms)) = retries {
                assert!(tx.set_time_trigger_retry_state(
                    &id,
                    Some(TimeTriggerRetryState {
                        retries_used,
                        next_retry_at_ms,
                    })
                ));
            }
            tx.apply();
            block.commit();
        }
        (
            State::new(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            ),
            id,
        )
    }

    pub(super) fn header() -> BlockHeader {
        BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 42, 0)
    }

    pub(super) fn retry(block: &StateBlock<'_>, id: &TriggerId) -> Option<(u32, u64)> {
        block
            .world
            .triggers
            .time_triggers()
            .get(id)
            .unwrap()
            .retry_state
            .map(|state| (state.retries_used, state.next_retry_at_ms))
    }

    pub(super) fn original_action(block: &StateBlock<'_>, id: &TriggerId) -> Vec<u8> {
        let trigger = block
            .world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == id)
            .unwrap();
        norito::encode_canonical(trigger.action()).unwrap()
    }
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
