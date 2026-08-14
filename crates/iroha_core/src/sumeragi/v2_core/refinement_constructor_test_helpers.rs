// Test-only constructor invariants included at `refinement` module scope.
#[cfg(test)]
fn assert_in_flight_first_release_actor_target_tampering_fails(
    accepted: ProductionInFlightFirstReleaseTransitionProjection,
    wrong_actor: u128,
    wrong_target: u128,
) {
    let mut tampered_actor = accepted;
    tampered_actor.actor = wrong_actor;
    assert!(
        check_production_in_flight_first_release_transition(tampered_actor).is_none(),
        "checked evidence must reject actor substitution"
    );
    let mut tampered_target = accepted;
    tampered_target.target = wrong_target;
    assert!(
        check_production_in_flight_first_release_transition(tampered_target).is_none(),
        "checked evidence must reject target substitution"
    );
}
#[cfg(test)]
fn assert_in_flight_first_release_transport_constructors_fail_closed(
    reserved: ProductionInFlightFirstReleaseStateProjection,
) {
    for invalid_replica in [0, reserved.producer, 3, 8] {
        assert!(
            check_production_in_flight_first_release_fanout_from_producer_transition(
                reserved,
                invalid_replica,
            )
            .is_none(),
            "fanout must reject invalid replica bitmap {invalid_replica:#x}"
        );
    }
    let mut missing_producer_custody = reserved;
    missing_producer_custody.session.bodies &= !reserved.producer;
    assert!(production_in_flight_first_release_state_kernel(
        missing_producer_custody
    ));
    assert!(
        check_production_in_flight_first_release_fanout_from_producer_transition(
            missing_producer_custody,
            2,
        )
        .is_none(),
        "fanout must not fabricate absent producer custody"
    );
    let fanout =
        check_production_in_flight_first_release_fanout_from_producer_transition(reserved, 2)
            .expect("valid producer fanout must mint checked evidence")
            .into_projection();
    assert_eq!(
        fanout.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER
    );
    assert_eq!(fanout.actor, 2);
    assert_eq!(fanout.target, 0);
    let mut expected_fanout = reserved;
    expected_fanout.session.bodies |= 2;
    assert_eq!(fanout.after, expected_fanout);
    let duplicate_fanout =
        check_production_in_flight_first_release_fanout_from_producer_transition(fanout.after, 2)
            .expect("an exact duplicate fanout is a valid idempotent stutter")
            .into_projection();
    assert_eq!(duplicate_fanout.before, duplicate_fanout.after);
    for (source, target) in [(4, 1), (2, 2), (2, 0), (3, 4)] {
        assert!(
            check_production_in_flight_first_release_serve_late_body_transition(
                fanout.after,
                source,
                target,
            )
            .is_none(),
            "late-body service must reject source/target pair {source:#x}/{target:#x}"
        );
    }
    let crashed_target = check_production_in_flight_first_release_crash_transition(fanout.after, 4)
        .expect("a committee target can crash before body service")
        .into_projection()
        .after;
    assert!(
        check_production_in_flight_first_release_serve_late_body_transition(crashed_target, 2, 4,)
            .is_none(),
        "late-body service must reject a crashed target"
    );
    let served =
        check_production_in_flight_first_release_serve_late_body_transition(fanout.after, 2, 4)
            .expect("authenticated source custody must serve one live target")
            .into_projection();
    assert_eq!(
        served.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY
    );
    assert_eq!((served.actor, served.target), (2, 4));
    let mut expected_serve = fanout.after;
    expected_serve.session.bodies |= 4;
    assert_eq!(served.after, expected_serve);
    let duplicate_serve =
        check_production_in_flight_first_release_serve_late_body_transition(served.after, 2, 4)
            .expect("an exact duplicate late-body service is a valid idempotent stutter")
            .into_projection();
    assert_eq!(duplicate_serve.before, duplicate_serve.after);
    for accepted in [fanout, served] {
        assert_in_flight_first_release_actor_target_tampering_fails(accepted, 0, 1);
    }
}
#[cfg(test)]
fn assert_in_flight_first_release_crash_recovery_constructors_fail_closed(
    ready: ProductionInFlightFirstReleaseStateProjection,
) {
    for invalid_actor in [0, 3, 8] {
        assert!(
            check_production_in_flight_first_release_crash_transition(ready, invalid_actor)
                .is_none(),
            "crash must reject invalid actor bitmap {invalid_actor:#x}"
        );
    }
    assert!(
        check_production_in_flight_first_release_recover_transition(ready, 4).is_none(),
        "recovery must reject a validator that is not crashed"
    );
    let crash = check_production_in_flight_first_release_crash_transition(ready, 4)
        .expect("one live validator must have an exact checked crash")
        .into_projection();
    assert_eq!(crash.action, IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH);
    assert_eq!((crash.actor, crash.target), (4, 0));
    let mut expected_crash = ready;
    expected_crash.session.crashed |= 4;
    expected_crash.session.bodies &= !4;
    expected_crash.session.ready_authorized &= !4;
    assert_eq!(crash.after, expected_crash);
    assert!(
        check_production_in_flight_first_release_crash_transition(crash.after, 4).is_none(),
        "a duplicate crash must not masquerade as progress"
    );
    for invalid_actor in [0, 3, 8] {
        assert!(
            check_production_in_flight_first_release_recover_transition(
                crash.after,
                invalid_actor,
            )
            .is_none(),
            "recovery must reject invalid actor bitmap {invalid_actor:#x}"
        );
    }
    let recovery = check_production_in_flight_first_release_recover_transition(crash.after, 4)
        .expect("one crashed validator must have an exact checked recovery")
        .into_projection();
    assert_eq!(recovery.action, IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER);
    assert_eq!((recovery.actor, recovery.target), (4, 0));
    let mut expected_recovery = crash.after;
    expected_recovery.session.crashed &= !4;
    assert_eq!(recovery.after, expected_recovery);
    assert!(
        check_production_in_flight_first_release_recover_transition(recovery.after, 4).is_none(),
        "a duplicate recovery must not masquerade as progress"
    );
    for accepted in [crash, recovery] {
        assert_in_flight_first_release_actor_target_tampering_fails(accepted, 0, 1);
    }
    let producer_crash =
        check_production_in_flight_first_release_crash_transition(ready, ready.producer)
            .expect("the selected producer has an exact checked crash")
            .into_projection();
    assert!(!producer_crash.after.session.producer_alive);
    let producer_recovery = check_production_in_flight_first_release_recover_transition(
        producer_crash.after,
        ready.producer,
    )
    .expect("the crashed producer can rejoin without volatile custody")
    .into_projection();
    assert!(!producer_recovery.after.session.producer_alive);
    assert_eq!(producer_recovery.after.session.bodies & ready.producer, 0);
}
#[cfg(test)]
fn assert_in_flight_first_release_stutter_constructors_are_exact(
    reserved: ProductionInFlightFirstReleaseStateProjection,
    applied: ProductionInFlightFirstReleaseStateProjection,
) {
    let snapshot =
        check_production_in_flight_first_release_recover_reservation_snapshot_transition(reserved)
            .expect("a valid V5 snapshot replay must mint checked stutter evidence")
            .into_projection();
    assert_eq!(
        snapshot.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
    );
    assert_eq!(snapshot.before, snapshot.after);
    assert_eq!(snapshot.actor, 0);
    assert_eq!(snapshot.target, 0);
    let duplicate_snapshot =
        check_production_in_flight_first_release_recover_reservation_snapshot_transition(
            snapshot.after,
        )
        .expect("an exact duplicate snapshot replay remains a checked stutter")
        .into_projection();
    assert_eq!(duplicate_snapshot, snapshot);
    assert_in_flight_first_release_actor_target_tampering_fails(snapshot, 1, 1);
    let mut changed_snapshot = reserved;
    changed_snapshot.session.bodies |= 2;
    assert!(production_in_flight_first_release_state_kernel(
        changed_snapshot
    ));
    assert!(
        check_production_in_flight_first_release_transition(
            ProductionInFlightFirstReleaseTransitionProjection {
                action: IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
                actor: 0,
                target: 0,
                before: reserved,
                after: changed_snapshot,
            }
        )
        .is_none(),
        "snapshot replay must reject a non-stutter after-state"
    );
    assert!(
        check_production_in_flight_first_release_repair_post_carrier_evidence_transition(reserved)
            .is_none(),
        "post-carrier repair requires canonical WSV application"
    );
    let repair =
        check_production_in_flight_first_release_repair_post_carrier_evidence_transition(applied)
            .expect("a post-application evidence repair must mint checked stutter evidence")
            .into_projection();
    assert_eq!(
        repair.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER
    );
    assert_eq!((repair.actor, repair.target), (0, 0));
    assert_eq!(repair.before, repair.after);
    let duplicate_repair =
        check_production_in_flight_first_release_repair_post_carrier_evidence_transition(
            repair.after,
        )
        .expect("an exact duplicate evidence repair remains a checked stutter")
        .into_projection();
    assert_eq!(duplicate_repair, repair);
    assert_in_flight_first_release_actor_target_tampering_fails(repair, 1, 1);
    let changed_repair = check_production_in_flight_first_release_crash_transition(applied, 4)
        .expect("valid crash supplies an independently valid non-stutter target")
        .into_projection()
        .after;
    assert!(
        check_production_in_flight_first_release_transition(
            ProductionInFlightFirstReleaseTransitionProjection {
                action: IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
                actor: 0,
                target: 0,
                before: applied,
                after: changed_repair,
            }
        )
        .is_none(),
        "post-carrier repair must reject a non-stutter after-state"
    );
}
