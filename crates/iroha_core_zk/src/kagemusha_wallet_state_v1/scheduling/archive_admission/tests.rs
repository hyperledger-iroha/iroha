//! Explicit scheduler orderings over private synthetic source facts; no monetary qualification.

use super::*;

fn target() -> ArchiveTargetV1 {
    ArchiveTargetV1 {
        request_id: [1; 32],
        source: ArchiveHeadV1 {
            sequence: 7,
            head: [2; 32],
            capsule: [3; 32],
        },
    }
}

fn successor() -> ArchiveHeadV1 {
    ArchiveHeadV1 {
        sequence: 8,
        head: [4; 32],
        capsule: [5; 32],
    }
}

fn reserve(scheduler: &Scheduler, turn: &ArchiveTurnV1) -> ArchiveDispatchPermit {
    scheduler
        .reserve_archive(turn, target(), 16, false, 0)
        .unwrap()
}

#[test]
fn delayed_speculation_cannot_take_priority_from_an_archive_covering_fold() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    let speculation = super::super::Speculation::<()>::new();
    let job = speculation.job();
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert!(scheduler.0.lock().archive.blocks_speculation());
    scheduler
        .settle_archive(
            reserve(&scheduler, &turn),
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Unfolded,
            },
        )
        .unwrap();
    drop(turn);
    assert!(scheduler.speculate(&job).is_none());
    assert!(!scheduler.speculating_for_test());
    let fold = scheduler.start().unwrap();
    assert!(scheduler.speculate(&job).is_none());
    fold.token.check().unwrap();
    drop(fold);
    let observation = scheduler.reconciling_archive().unwrap();
    scheduler
        .reconcile_archive(
            &observation,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Covered,
            },
        )
        .unwrap();
    let run = scheduler.speculate(&job).unwrap();
    run.finish(());
    assert_eq!(speculation.take(), Some(()));
}

#[test]
fn maintenance_wake_hint_never_grants_priority_or_clears_an_unresolved_attempt() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    drop(reserve(&scheduler, &turn));
    drop(turn);
    scheduler
        .wait_for_maintenance_change(std::time::Duration::ZERO)
        .unwrap();
    assert_eq!(scheduler.0.lock().payments, 0);
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
    scheduler.close();
    scheduler.set_activity(true, true);
    assert_eq!(
        scheduler.wait_for_maintenance_change(std::time::Duration::ZERO),
        Err(ArchiveAdmissionErrorV1::Closed)
    );
}

#[test]
fn turn_validation_authenticates_owner_and_lifetime_without_granting_dispatch() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    turn.validate_for(&scheduler).unwrap();
    assert_eq!(
        turn.validate_for(&Scheduler::new()),
        Err(ArchiveAdmissionErrorV1::Foreign)
    );
    // A live turn remains valid for bounded recovery after its final dispatch reservation.
    let permit = reserve(&scheduler, &turn);
    turn.validate_for(&scheduler).unwrap();
    drop(permit);
    turn.validate_for(&scheduler).unwrap();
    scheduler.0.lock().archive.turn = None;
    assert_eq!(
        turn.validate_for(&scheduler),
        Err(ArchiveAdmissionErrorV1::Stale)
    );
    scheduler.close();
    assert_eq!(
        turn.validate_for(&scheduler),
        Err(ArchiveAdmissionErrorV1::Closed)
    );
}

#[test]
fn foreground_alone_does_not_grant_charging_and_every_turn_rechecks_alternatives() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 15, false, 0),
        Err(ArchiveAdmissionErrorV1::NotDue)
    ));
    scheduler.set_activity(false, true);
    let permit = scheduler
        .reserve_archive(&turn, target(), 1, false, 0)
        .unwrap();
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::Unselected {
                source: target().source,
            },
        )
        .unwrap();
    drop(turn);
    // A new turn cannot inherit charging from the preceding dispatch.
    scheduler.set_activity(true, false);
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 15, false, 0),
        Err(ArchiveAdmissionErrorV1::NotDue)
    ));
    let permit = scheduler
        .reserve_archive(&turn, target(), 16, false, 0)
        .unwrap();
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::Unselected {
                source: target().source,
            },
        )
        .unwrap();
    drop(turn);
    let turn = scheduler.prepare_archive_turn().unwrap();
    // Count fell to15; a fresh positive Native age is an independent alternative.
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 15, false, 0),
        Err(ArchiveAdmissionErrorV1::NotDue)
    ));
    let permit = scheduler
        .reserve_archive(&turn, target(), 15, true, 0)
        .unwrap();
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::Unselected {
                source: target().source,
            },
        )
        .unwrap();
    drop(turn);
    let turn = scheduler.prepare_archive_turn().unwrap();
    // Native reports unavailable/regressed/not-due age as false: never latch the prior due.
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 15, false, 0),
        Err(ArchiveAdmissionErrorV1::NotDue)
    ));
}

#[test]
fn empty_queue_or_unfolded_head_refuses_even_when_all_thresholds_are_satisfied() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, true);
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 0, true, 0),
        Err(ArchiveAdmissionErrorV1::NotDue)
    ));
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 16, true, 1),
        Err(ArchiveAdmissionErrorV1::FoldBacklog)
    ));
    assert!(matches!(scheduler.0.lock().archive.phase, Phase::Clear));
}

#[test]
fn exchange_before_prepare_or_final_reservation_wins_without_dispatch() {
    let scheduler = Scheduler::new();
    let lease = scheduler.begin_exchange().unwrap();
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::ExchangeOpen)
    ));
    assert_eq!(scheduler.0.lock().payments, 0);
    drop(lease);
    let turn = scheduler.prepare_archive_turn().unwrap();
    let lease = scheduler.begin_exchange().unwrap();
    let child = lease.enter_call().unwrap();
    assert_eq!(lease.end().unwrap(), super::super::ExchangeEndV1::Ending);
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 16, true, 0),
        Err(ArchiveAdmissionErrorV1::ExchangeOpen)
    ));
    drop(child);
    let permit = reserve(&scheduler, &turn);
    permit
        .validate_for(&scheduler, &target().request_id, target().source)
        .unwrap();
}

#[test]
fn late_exchange_retains_occupancy_while_archive_and_covering_fold_finish() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    let turn = scheduler.prepare_archive_turn().unwrap();
    let permit = reserve(&scheduler, &turn);
    let lease = scheduler.begin_exchange().unwrap();
    let call = lease.enter_call().unwrap();
    permit
        .validate_for(&scheduler, &target().request_id, target().source)
        .unwrap();
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Unfolded,
            },
        )
        .unwrap();
    drop(turn);
    assert_eq!(scheduler.0.lock().payments, 0);
    let fold = scheduler
        .start()
        .expect("late exchange holds no Payment priority");
    call.validate_for(&scheduler).unwrap();
    fold.token.check().unwrap();
    drop(fold);
    let observation = scheduler.reconciling_archive().unwrap();
    assert_eq!(observation.head, Some(successor()));
    scheduler
        .reconcile_archive(
            &observation,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Covered,
            },
        )
        .unwrap();
    scheduler.require_archive_clear().unwrap();
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::ExchangeOpen)
    ));
    drop(call);
    drop(lease);
    scheduler.prepare_archive_turn().unwrap();
}

#[test]
fn dropped_dispatched_permit_keeps_uncertainty_until_exact_nonselection() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    let permit = reserve(&scheduler, &turn);
    drop(permit);
    drop(turn);
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::Busy)
    ));
    let observed = scheduler.reconciling_archive().unwrap();
    assert_eq!(observed.target(), target());
    assert_eq!(observed.attempt.selected, None);
    assert_eq!(
        scheduler.reconcile_archive(
            &observed,
            ArchiveResolutionV1::Unselected {
                source: successor()
            }
        ),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
    scheduler
        .reconcile_archive(
            &observed,
            ArchiveResolutionV1::Unselected {
                source: target().source,
            },
        )
        .unwrap();
    scheduler.require_archive_clear().unwrap();
    let next = scheduler.prepare_archive_turn().unwrap();
    let _permit = reserve(&scheduler, &next);
    assert_eq!(
        scheduler.reconcile_archive(
            &observed,
            ArchiveResolutionV1::Unselected {
                source: target().source
            }
        ),
        Err(ArchiveAdmissionErrorV1::Stale)
    );
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
}

#[test]
fn selected_pending_requires_its_exact_capsule_then_exact_covering_head() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    let permit = reserve(&scheduler, &turn);
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::SelectedPending {
                capsule: successor().capsule,
            },
        )
        .unwrap();
    drop(turn);
    let observed = scheduler.reconciling_archive().unwrap();
    assert_eq!(observed.attempt.selected, Some(successor().capsule));
    assert_eq!(
        scheduler.reconcile_archive(
            &observed,
            ArchiveResolutionV1::Unselected {
                source: target().source
            }
        ),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    assert_eq!(
        scheduler.reconcile_archive(
            &observed,
            ArchiveResolutionV1::SelectedPending { capsule: [9; 32] }
        ),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    scheduler
        .reconcile_archive(
            &observed,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Unfolded,
            },
        )
        .unwrap();
    assert_eq!(
        scheduler.reconcile_archive(
            &observed,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Covered
            }
        ),
        Err(ArchiveAdmissionErrorV1::Stale)
    );
    let folded = scheduler.reconciling_archive().unwrap();
    let mut wrong = successor();
    wrong.head = [8; 32];
    assert_eq!(
        scheduler.reconcile_archive(
            &folded,
            ArchiveResolutionV1::SelectedReleased {
                head: wrong,
                coverage: ArchiveCoverageV1::Covered
            }
        ),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    assert_eq!(
        scheduler.reconcile_archive(
            &folded,
            ArchiveResolutionV1::SelectedPending {
                capsule: successor().capsule
            }
        ),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    scheduler
        .reconcile_archive(
            &folded,
            ArchiveResolutionV1::SelectedReleased {
                head: successor(),
                coverage: ArchiveCoverageV1::Covered,
            },
        )
        .unwrap();
    scheduler.require_archive_clear().unwrap();
}

#[test]
fn permit_cannot_authorize_another_request_source_or_scheduler() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    let permit = reserve(&scheduler, &turn);
    assert_eq!(permit.target(), target());
    assert_eq!(
        permit.validate_for(&scheduler, &[9; 32], target().source),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    assert_eq!(
        permit.validate_for(&scheduler, &target().request_id, successor()),
        Err(ArchiveAdmissionErrorV1::InvalidResolution)
    );
    let other = Scheduler::new();
    assert_eq!(
        permit.validate_for(&other, &target().request_id, target().source),
        Err(ArchiveAdmissionErrorV1::Foreign)
    );
    assert!(matches!(
        other.reserve_archive(&turn, target(), 16, true, 0),
        Err(ArchiveAdmissionErrorV1::Foreign)
    ));
    other.require_archive_clear().unwrap();
    drop(permit);
    let observed = scheduler.reconciling_archive().unwrap();
    assert_eq!(
        other.reconcile_archive(
            &observed,
            ArchiveResolutionV1::Unselected {
                source: target().source
            }
        ),
        Err(ArchiveAdmissionErrorV1::Foreign)
    );
    other.require_archive_clear().unwrap();
}

#[test]
fn dropping_predispatch_turn_releases_priority_but_dropping_dispatched_turn_does_not_clear() {
    let scheduler = Scheduler::new();
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert_eq!(scheduler.0.lock().payments, 1);
    drop(turn);
    assert_eq!(scheduler.0.lock().payments, 0);
    scheduler.require_archive_clear().unwrap();
    let turn = scheduler.prepare_archive_turn().unwrap();
    let permit = reserve(&scheduler, &turn);
    drop(turn);
    assert_eq!(scheduler.0.lock().payments, 0);
    assert_eq!(
        permit.validate_for(&scheduler, &target().request_id, target().source),
        Err(ArchiveAdmissionErrorV1::Stale)
    );
    assert_eq!(
        scheduler.require_archive_clear(),
        Err(ArchiveAdmissionErrorV1::Busy)
    );
    drop(permit);
    assert!(scheduler.reconciling_archive().is_some());
}

#[test]
fn close_during_preparation_or_execution_is_terminal_even_after_activity_callbacks() {
    for dispatched in [false, true] {
        let scheduler = Scheduler::new();
        scheduler.set_activity(true, true);
        let turn = scheduler.prepare_archive_turn().unwrap();
        let permit = dispatched.then(|| reserve(&scheduler, &turn));
        scheduler.close();
        scheduler.set_activity(true, true);
        assert!(!scheduler.0.lock().charging);
        assert_eq!(
            scheduler.require_archive_clear(),
            Err(ArchiveAdmissionErrorV1::Closed)
        );
        assert!(matches!(
            scheduler.reserve_archive(&turn, target(), 16, true, 0),
            Err(ArchiveAdmissionErrorV1::Closed)
        ));
        if let Some(permit) = permit {
            assert_eq!(
                scheduler.settle_archive(
                    permit,
                    ArchiveResolutionV1::Unselected {
                        source: target().source
                    }
                ),
                Err(ArchiveAdmissionErrorV1::Closed)
            );
        }
        drop(turn);
        assert_eq!(scheduler.0.lock().payments, 0);
        assert!(matches!(
            scheduler.prepare_archive_turn(),
            Err(ArchiveAdmissionErrorV1::Closed)
        ));
    }
}

#[test]
fn competing_payment_turn_and_exhausted_ids_refuse_without_reusing_an_attempt() {
    let scheduler = Scheduler::new();
    let payment = scheduler.payment();
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::Busy)
    ));
    drop(payment);
    scheduler.0.lock().archive.next = u64::MAX - 1;
    let turn = scheduler.prepare_archive_turn().unwrap();
    assert_eq!(turn.id, u64::MAX);
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::Busy)
    ));
    let payment = scheduler.payment();
    assert!(matches!(
        scheduler.reserve_archive(&turn, target(), 16, false, 0),
        Err(ArchiveAdmissionErrorV1::Busy)
    ));
    drop(payment);
    let permit = reserve(&scheduler, &turn);
    scheduler
        .settle_archive(
            permit,
            ArchiveResolutionV1::Unselected {
                source: target().source,
            },
        )
        .unwrap();
    drop(turn);
    assert!(matches!(
        scheduler.prepare_archive_turn(),
        Err(ArchiveAdmissionErrorV1::Exhausted)
    ));
    assert_eq!(scheduler.0.lock().payments, 0);
}

#[test]
fn contradictory_selected_resolution_never_releases_the_dispatched_barrier() {
    for mutation in 0..4 {
        let scheduler = Scheduler::new();
        let turn = scheduler.prepare_archive_turn().unwrap();
        let permit = reserve(&scheduler, &turn);
        let mut wrong = successor();
        match mutation {
            0 => wrong.sequence += 1,
            1 => wrong.head = [0; 32],
            2 => wrong.capsule = target().source.capsule,
            _ => wrong.head = target().source.head,
        }
        assert_eq!(
            scheduler.settle_archive(
                permit,
                ArchiveResolutionV1::SelectedReleased {
                    head: wrong,
                    coverage: ArchiveCoverageV1::Covered
                }
            ),
            Err(ArchiveAdmissionErrorV1::InvalidResolution)
        );
        drop(turn);
        assert_eq!(
            scheduler.require_archive_clear(),
            Err(ArchiveAdmissionErrorV1::Busy)
        );
        assert!(scheduler.reconciling_archive().is_some());
    }
}
