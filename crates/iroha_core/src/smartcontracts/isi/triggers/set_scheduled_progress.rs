//! Autonomous carrier deadlines must agree with the actual native Time matcher.

use super::*;
use iroha_data_model::events::time::Schedule;
use std::time::Duration;

fn action(start: u64, period: Option<u64>, repeats: Repeats) -> SpecializedAction<TimeEventFilter> {
    let mut action = SpecializedAction::new(
        Executable::Instructions(vec![Log::new(Level::INFO, "due".to_owned()).into()].into()),
        repeats,
        crate::smartcontracts::isi::sccp::test_support::authority(31),
        TimeEventFilter(ExecutionTime::Schedule(Schedule {
            start_ms: start,
            period_ms: period,
        })),
    )
    .unwrap();
    action.metadata.insert(
        "__registered_block_height".parse().unwrap(),
        Json::new(1_u64),
    );
    action.metadata.insert(
        super::super::progress::DECLARATION_KEY.parse().unwrap(),
        Json::new(1_024_u32),
    );
    super::super::progress::initialize(&mut action.metadata, 1_024);
    action
}

fn install(set: &Set, id: &str, action: SpecializedAction<TimeEventFilter>) {
    let mut block = set.block();
    let mut tx = block.transaction();
    assert!(
        tx.add_time_trigger(SpecializedTrigger::new(id.parse().unwrap(), action))
            .unwrap()
    );
    tx.apply();
    block.commit();
}

#[test]
fn scheduled_deadline_matches_native_right_open_intervals_and_periodic_cursor() {
    for period in [None, Some(1), Some(7), Some(100)] {
        let set = Set::default();
        install(&set, "scheduled", action(100, period, Repeats::Exactly(20)));
        let view = set.view();
        let id: TriggerId = "scheduled".parse().unwrap();
        let loaded = view.time_triggers().get(&id).unwrap();
        for since in [0, 99, 100, 101, 199, 200, 201, 800] {
            for to in since..since + 110 {
                let event = TimeEvent {
                    interval: TimeInterval::new(
                        Duration::from_millis(since),
                        Duration::from_millis(to - since),
                    ),
                };
                assert_eq!(
                    view.next_scheduled_progress_at_ms(since, 2)
                        .is_some_and(|due| due <= to),
                    time_trigger_action_is_due(loaded, &event, 2, to),
                    "period={period:?} since={since} to={to}"
                );
            }
        }
    }
}

#[test]
fn failed_unretried_oneshot_does_not_create_more_carriers() {
    let set = Set::default();
    install(&set, "once", action(100, None, Repeats::Exactly(1)));
    assert_eq!(set.view().next_scheduled_progress_at_ms(99, 2), Some(101));
    // Failed callbacks without retry retain their action, but the consumed interval is past.
    assert_eq!(set.view().next_scheduled_progress_at_ms(101, 3), None);
}

#[test]
fn finite_attempts_stop_periodic_carriers_even_if_success_repeats_never_decrease() {
    let set = Set::default();
    let mut recurring = action(100, Some(10), Repeats::Indefinitely);
    super::super::progress::initialize(&mut recurring.metadata, 2);
    install(&set, "bounded", recurring);
    for (since, expected_deadline) in [(99, 101), (101, 111)] {
        assert_eq!(
            set.view().next_scheduled_progress_at_ms(since, 2),
            Some(expected_deadline)
        );
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.inspect_by_id_mut(&"bounded".parse().unwrap(), |action| {
            assert!(super::super::progress::consume(action.metadata_mut()));
        })
        .unwrap();
        tx.apply();
        block.commit();
    }
    assert_eq!(set.view().next_scheduled_progress_at_ms(111, 3), None);
    assert_eq!(
        set.view()
            .time_triggers()
            .get(&"bounded".parse().unwrap())
            .unwrap()
            .repeats,
        Repeats::Indefinitely
    );
}

#[test]
fn existing_transaction_driven_schedules_do_not_implicitly_authorize_carriers() {
    let set = Set::default();
    let mut ordinary = action(100, Some(10), Repeats::Exactly(50));
    ordinary
        .metadata
        .remove(super::super::progress::REMAINING_KEY);
    ordinary
        .metadata
        .remove(super::super::progress::DECLARATION_KEY);
    install(&set, "ordinary", ordinary);
    assert_eq!(set.view().next_scheduled_progress_at_ms(0, 2), None);
}

#[test]
fn retry_deadline_is_inclusive_and_uses_original_incarnation() {
    let set = Set::default();
    install(&set, "retry", action(100, None, Repeats::Exactly(1)));
    let mut block = set.block();
    let mut tx = block.transaction();
    let loaded = tx.time_triggers.get_mut(&"retry".parse().unwrap()).unwrap();
    loaded.retry_state = Some(TimeTriggerRetryState {
        retries_used: 1,
        next_retry_at_ms: 500,
    });
    tx.apply();
    block.commit();
    assert_eq!(set.view().next_scheduled_progress_at_ms(101, 3), Some(500));
}

#[test]
fn precommit_disabled_depleted_and_new_incarnations_cannot_wake() {
    let set = Set::default();
    let mut precommit = action(0, None, Repeats::Indefinitely);
    precommit.filter = TimeEventFilter(ExecutionTime::PreCommit);
    install(&set, "precommit", precommit);
    let mut disabled = action(0, None, Repeats::Exactly(1));
    disabled
        .metadata
        .insert("__enabled".parse().unwrap(), Json::new(false));
    install(&set, "disabled", disabled);
    install(&set, "depleted", action(0, None, Repeats::Exactly(0)));
    let mut fresh = action(0, None, Repeats::Exactly(1));
    fresh.metadata.insert(
        "__registered_block_height".parse().unwrap(),
        Json::new(2_u64),
    );
    install(&set, "new", fresh);
    assert_eq!(set.view().next_scheduled_progress_at_ms(0, 2), None);
    assert_eq!(set.view().next_scheduled_progress_at_ms(0, 3), Some(1));
}

#[test]
fn parent_deadline_ignores_current_registration_removal_and_transaction_replacement() {
    let set = Set::default();
    install(
        &set,
        "original",
        action(100, Some(100), Repeats::Exactly(10)),
    );
    let mut block = set.block();
    let mut tx = block.transaction();
    assert!(tx.remove(&"original".parse().unwrap()));
    assert!(
        tx.add_time_trigger(SpecializedTrigger::new(
            "original".parse().unwrap(),
            action(10, Some(10), Repeats::Exactly(10))
        ))
        .unwrap()
    );
    assert_eq!(tx.next_scheduled_progress_at_ms(0, 2), Some(11));
    assert_eq!(tx.scheduled_progress_before_block_ms(0, 2), Some(101));
    tx.apply();
    assert_eq!(block.next_scheduled_progress_at_ms(0, 2), Some(11));
    assert_eq!(block.scheduled_progress_before_block_ms(0, 2), Some(101));
}

#[test]
fn time_registration_cap_rejects_overflow_without_silencing_admitted_work() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for i in 0..MAX_TIME_TRIGGERS_TOTAL {
        assert!(
            tx.add_time_trigger(SpecializedTrigger::new(
                format!("time{i}").parse().unwrap(),
                action(100, None, Repeats::Exactly(1))
            ))
            .unwrap()
        );
    }
    assert!(matches!(
        tx.add_time_trigger(SpecializedTrigger::new(
            "overflow".parse().unwrap(),
            action(100, None, Repeats::Exactly(1))
        )),
        Err(Error::TimeTriggerCapacity)
    ));
    assert_eq!(tx.next_scheduled_progress_at_ms(0, 2), Some(101));
    assert_eq!(tx.time_triggers.len(), MAX_TIME_TRIGGERS_TOTAL);
    assert!(
        !tx.add_time_trigger(SpecializedTrigger::new(
            "time0".parse().unwrap(),
            action(1, None, Repeats::Exactly(1))
        ))
        .unwrap()
    );
}

#[test]
fn overflowing_and_corrupt_schedules_do_not_wrap_into_due_work() {
    for (start, period, since) in [
        (u64::MAX, None, 0),
        (u64::MAX - 10, Some(11), u64::MAX),
        (0, Some(0), 1),
    ] {
        let set = Set::default();
        install(&set, "overflow", action(start, period, Repeats::Exactly(1)));
        assert_eq!(set.view().next_scheduled_progress_at_ms(since, 2), None);
    }
}
