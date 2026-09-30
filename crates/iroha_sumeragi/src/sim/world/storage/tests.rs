//! Storage failure regressions exercise real fake-host barriers and virtual-time recovery.

use super::*;

fn unavailable_storage(backoff: Millis) -> World {
    let mut scenario = Scenario::base("permanent storage failure", 0, 4);
    scenario.duration = 2_000;
    scenario.checks.liveness = false;
    scenario.checks.progress = 0;
    scenario.workload = None;
    scenario.profiles = vec![
        Profile {
            write_fail_ppm: 1_000_000,
            write_min: 1,
            write_max: 1,
            write_retry: backoff,
            ..Profile::default()
        };
        4
    ];
    let mut world = World::new(scenario);
    for r in 0..world.replicas.len() {
        let record = world.replicas[r]
            .records
            .values()
            .next()
            .unwrap()
            .record
            .clone();
        world.apply_action(r, Action::PersistSafety(Box::new(record)), 0);
        world.apply_action(
            r,
            Action::Broadcast {
                to: Vec::new(),
                msg: WireMessage::PayloadRequest(PayloadRequest {
                    instance: world.instances[world.replicas[r].inst].id,
                    height: 1,
                    block_hash: Hash32::ZERO,
                }),
            },
            0,
        );
    }
    world
}

#[test]
fn permanent_write_failure_yields_and_preserves_original_barrier_until_recovery() {
    let mut world = unavailable_storage(5);
    let original: Vec<_> = world
        .replicas
        .iter()
        .map(|rep| (rep.io.pending.front().unwrap().0, rep.records.clone()))
        .collect();
    world.run_until(100);
    assert_eq!(world.now, 100);
    assert!(world.failure.is_none(), "{:?}", world.failure);
    for (rep, (id, records)) in world.replicas.iter().zip(&original) {
        assert_eq!(rep.io.pending.front().unwrap().0, *id);
        assert_eq!(rep.records.len(), records.len());
        for (key, original) in records {
            assert_eq!(rep.records[key].bytes, original.bytes);
        }
        assert_eq!(rep.applied.0, 0);
        assert!(rep.host.held().iter().any(|action| matches!(action,
            Action::Broadcast { to, msg: WireMessage::PayloadRequest(request) }
                if to.is_empty() && request.block_hash == Hash32::ZERO
        )));
    }
    assert!(world.io_completions.iter().all(|count| *count == 0));
    assert_eq!(world.stats.proposals, 0);
    for machine in &mut world.machines {
        machine.profile.write_fail_ppm = 0;
    }
    world.run_until(200);
    assert!(world.failure.is_none(), "{:?}", world.failure);
    assert!(world.io_completions.iter().all(|count| *count > 0));
    for (rep, (id, _)) in world.replicas.iter().zip(&original) {
        assert!(rep.io.pending.iter().all(|(pending, _, _)| pending != id));
        assert!(rep.host.held().is_empty());
    }
}

#[test]
fn zero_retry_backoff_still_allows_virtual_time_to_advance() {
    let mut world = unavailable_storage(0);
    world.run_until(20);
    assert_eq!(world.now, 20);
    assert!(world.failure.is_none(), "{:?}", world.failure);
    assert!(world.io_completions.iter().all(|count| *count == 0));
}

#[test]
fn recovered_head_reschedules_original_successor_after_its_stale_event() {
    let mut world = unavailable_storage(5);
    let first = world.replicas[0].io.pending.front().unwrap().0;
    let record = world.replicas[0]
        .records
        .values()
        .next()
        .unwrap()
        .record
        .clone();
    world.apply_action(0, Action::PersistSafety(Box::new(record)), 0);
    let second = world.replicas[0].io.pending.back().unwrap().0;
    assert!(second > first);
    world.run_until(100);
    assert_eq!(world.replicas[0].io.pending.len(), 2);
    assert_eq!(world.io_completions[0], 0);
    world.machines[0].profile.write_fail_ppm = 0;
    let first_due = world.replicas[0].io.pending[0].1;
    let second_due = world.replicas[0].io.pending[1].1;
    assert!(second_due > first_due);
    world.run_until(first_due);
    assert_eq!(world.io_completions[0], 1);
    assert_eq!(world.replicas[0].io.pending.front().unwrap().0, second);
    world.run_until(second_due);
    assert_eq!(world.io_completions[0], 2);
    assert!(world.replicas[0].io.pending.is_empty());
    world.run_until(second_due + 10);
    assert_eq!(
        world.io_completions[0], 2,
        "stale events cannot complete twice"
    );
    assert!(world.failure.is_none(), "{:?}", world.failure);
}
