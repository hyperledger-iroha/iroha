//! Node-owned witness classification for resumed replica FIFO release proofs.

use super::*;

#[test]
fn resumed_replica_fifo_release_requires_explicit_witness_stutter() {
    let before = refinement::replica_fifo_state(0);
    let mut live = before;
    live.queue.reservation_state = IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE;
    live.release.fifo_restored = false;
    let direct = ProductionInFlightFirstReleaseTransitionProjection {
        action: IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
        actor: 2,
        target: 0,
        before: live,
        after: before,
    };
    let after = refinement::replica_fifo_state(1);
    let resumed_fifo_proof = ProductionInFlightFirstReleaseTransitionProjection {
        action: IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
        actor: 2,
        target: 0,
        before: after,
        after,
    };
    assert!(
        check_production_in_flight_first_release_replay_step_v1(
            resumed_fifo_proof,
            ProductionInFlightFirstReleaseReplayStepV1::ComposedNext,
        )
        .is_none(),
        "an unchanged replica FIFO proof must not masquerade as a state-changing step",
    );
    let checked = check_production_in_flight_first_release_replay_step_v1(
        resumed_fifo_proof,
        ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter,
    )
    .expect(
        "an exact already-proved nonproducer FIFO release must pass its explicit stutter class",
    );
    let witness = *checked
        .first_release_witness()
        .expect("node stutter carries a witness");
    assert!(
        authenticate_production_in_flight_first_release_transition_witness_v1(
            resumed_fifo_proof,
            witness
        )
    );
    let inferred = check_production_in_flight_first_release_transition(resumed_fifo_proof)
        .expect("the production wrapper must classify a resumed replica FIFO proof");
    assert_eq!(inferred.first_release_witness(), Some(&witness));
    assert!(
        check_production_in_flight_first_release_replay_step_v1(
            direct,
            ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter,
        )
        .is_none(),
        "a state-changing first FIFO proof must not pass the stutter class",
    );
}
