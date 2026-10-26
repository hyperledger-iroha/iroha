//! Shared real-State fixtures for the unactivated retry journal and policy seam.
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
    state_fixture_with_repetitions(retries, policy, 2, false)
}

pub(super) fn state_fixture_with_repetitions(
    retries: Option<(u32, u64)>,
    policy: bool,
    repetitions: u32,
    periodic: bool,
) -> (State, TriggerId) {
    let world = World::default();
    {
        let mut parameters = world.parameters.block();
        parameters
            .set_parameter(
                crate::sumeragi::test_chain::crypto_fixture_policy()
                    .into_parameter()
                    .unwrap(),
            )
            .unwrap();
        parameters.commit();
    }
    let id: TriggerId = "journal_retry".parse().unwrap();
    {
        let mut block = world.triggers.block();
        let mut tx = block.transaction();
        let schedule = Schedule::starting_at(Duration::from_millis(10));
        let schedule = if periodic { schedule.with_period(Duration::from_millis(10)) } else { schedule };
        let mut action = SpecializedAction::new(
            Executable::Instructions(Vec::<InstructionBox>::new().into()),
            Repeats::Exactly(repetitions),
            ALICE_ID.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
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

pub(super) fn limits() -> norito::core::EncodeWorkLimits {
    norito::core::EncodeWorkLimits {
        max_frame_bytes: 65_536,
        max_byte_visits: 10_000_000,
        max_operations: 1_000_000,
        max_depth: 128,
    }
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
