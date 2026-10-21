//! Deterministic exchange ownership and ordering probes; no wallet/proof qualification.

use super::*;

fn occupancy(scheduler: &Scheduler) -> usize {
    scheduler.0.lock().exchanges.live.len()
}

#[test]
fn requested_exchange_is_immediate_and_never_takes_payment_or_cancels_a_fold() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    let fold = scheduler.start().unwrap();
    let lease = scheduler.begin_exchange().unwrap();
    assert_eq!(occupancy(&scheduler), 1);
    assert_eq!(scheduler.0.lock().payments, 0);
    fold.token.check().unwrap();
    let call = lease.enter_call().unwrap();
    call.validate_for(&scheduler.clone()).unwrap();
    fold.token.check().unwrap();
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ending);
    drop(fold);
    let covering_fold = scheduler.start().unwrap();
    covering_fold.token.check().unwrap();
    drop(call);
    assert_eq!(occupancy(&scheduler), 0);
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::AlreadyEnded);
}

#[test]
fn ending_fences_new_children_and_retains_every_previously_accepted_child() {
    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    let first = lease.enter_call().unwrap();
    let second = lease.enter_call().unwrap();
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ending);
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ending);
    assert!(matches!(lease.enter_call(), Err(ExchangeErrorV1::Ended)));
    first.validate_for(&scheduler).unwrap();
    second.validate_for(&scheduler).unwrap();
    drop(first);
    assert_eq!(occupancy(&scheduler), 1);
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ending);
    drop(second);
    assert_eq!(occupancy(&scheduler), 0);
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::AlreadyEnded);
}

#[test]
fn child_accepted_before_worker_queue_survives_end_without_reopening_the_lease() {
    use std::sync::mpsc;

    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    let child = lease.enter_call().unwrap();
    let (start, queued) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    let worker_scheduler = scheduler.clone();
    let worker = std::thread::spawn(move || {
        queued.recv().unwrap();
        child.validate_for(&worker_scheduler).unwrap();
        assert_eq!(occupancy(&worker_scheduler), 1);
        drop(child);
        finished.send(()).unwrap();
    });
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ending);
    assert_eq!(occupancy(&scheduler), 1);
    assert!(matches!(lease.enter_call(), Err(ExchangeErrorV1::Ended)));
    start.send(()).unwrap();
    done.recv().unwrap();
    worker.join().unwrap();
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::AlreadyEnded);
    assert_eq!(occupancy(&scheduler), 0);
}

#[test]
fn dropping_lease_waits_only_in_accounting_and_old_end_never_releases_a_new_lease() {
    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    let child = lease.enter_call().unwrap();
    drop(lease);
    assert_eq!(occupancy(&scheduler), 1);
    child.validate_for(&scheduler).unwrap();
    drop(child);
    let old = scheduler.begin_exchange().unwrap();
    assert_eq!(old.end().unwrap(), ExchangeEndV1::Ended);
    let new = scheduler.begin_exchange().unwrap();
    assert_ne!(old.id, new.id);
    assert_eq!(old.end().unwrap(), ExchangeEndV1::AlreadyEnded);
    drop(old);
    assert_eq!(occupancy(&scheduler), 1);
    drop(new);
    assert_eq!(occupancy(&scheduler), 0);
}

#[test]
fn close_invalidates_all_capabilities_without_waiting_for_user_holders() {
    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    let child = lease.enter_call().unwrap();
    scheduler.close();
    assert_eq!(occupancy(&scheduler), 0);
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Closed)
    ));
    assert!(matches!(lease.enter_call(), Err(ExchangeErrorV1::Closed)));
    assert_eq!(lease.end(), Err(ExchangeErrorV1::Closed));
    assert_eq!(child.validate_for(&scheduler), Err(ExchangeErrorV1::Closed));
    scheduler.set_activity(true, true);
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Closed)
    ));
    let reopened = Scheduler::new();
    let replacement = reopened.begin_exchange().unwrap();
    assert_eq!(
        lease.id, replacement.id,
        "IDs are local; owner identity is required"
    );
    assert_eq!(child.validate_for(&reopened), Err(ExchangeErrorV1::Foreign));
    drop(child);
    drop(lease);
    assert_eq!(occupancy(&reopened), 1);
    replacement
        .enter_call()
        .unwrap()
        .validate_for(&reopened)
        .unwrap();
}

#[test]
fn foreign_open_owner_is_rejected_and_shared_scheduler_is_accepted() {
    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    let child = lease.enter_call().unwrap();
    let other = Scheduler::new();
    assert_eq!(child.validate_for(&other), Err(ExchangeErrorV1::Foreign));
    assert_eq!(occupancy(&other), 0);
    child.validate_for(&scheduler.clone()).unwrap();
    assert_eq!(occupancy(&scheduler), 1);
}

#[test]
fn capacity_counts_ending_leases_until_their_last_accepted_child_finishes() {
    let scheduler = Scheduler::new();
    let leases: Vec<_> = (0..MAX_EXCHANGES)
        .map(|_| scheduler.begin_exchange().unwrap())
        .collect();
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Capacity)
    ));
    let child = leases[0].enter_call().unwrap();
    assert_eq!(leases[0].end().unwrap(), ExchangeEndV1::Ending);
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Capacity)
    ));
    drop(child);
    let next = scheduler.begin_exchange().unwrap();
    assert!(next.id > leases.last().unwrap().id);
    assert_eq!(occupancy(&scheduler), MAX_EXCHANGES);
    drop(leases);
    assert_eq!(occupancy(&scheduler), 1);
}

#[test]
fn exhausted_identity_and_call_counters_refuse_without_reuse_or_lost_occupancy() {
    let scheduler = Scheduler::new();
    scheduler.0.lock().exchanges.next = u64::MAX - 1;
    let lease = scheduler.begin_exchange().unwrap();
    assert_eq!(lease.id, u64::MAX);
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Exhausted)
    ));
    scheduler
        .0
        .lock()
        .exchanges
        .live
        .get_mut(&lease.id)
        .unwrap()
        .calls = usize::MAX;
    assert!(matches!(lease.enter_call(), Err(ExchangeErrorV1::Capacity)));
    assert_eq!(occupancy(&scheduler), 1);
    assert_eq!(
        scheduler.0.lock().exchanges.live[&lease.id].calls,
        usize::MAX
    );
    // The synthetic maximum represents no actual guards; restore it before ending.
    scheduler
        .0
        .lock()
        .exchanges
        .live
        .get_mut(&lease.id)
        .unwrap()
        .calls = 0;
    assert_eq!(lease.end().unwrap(), ExchangeEndV1::Ended);
    assert!(matches!(
        scheduler.begin_exchange(),
        Err(ExchangeErrorV1::Exhausted)
    ));
    assert_eq!(occupancy(&scheduler), 0);
}
