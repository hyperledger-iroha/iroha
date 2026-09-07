//! Production witness identity and exact-state authentication regressions.

use super::*;

#[test]
fn queue_plan_selection_mints_current_source_bound_witness() {
    let binding_a = CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_CANONICAL_PAYLOAD,
        [0x61; 32],
    );
    let before = ProductionInFlightFirstReleaseStateProjection {
        validator_count: 1,
        producer: 1,
        producer_selected_owner: 1,
        replicated_carrier_owners: 0,
        payload_binding_a: 1,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT,
            selected_count: 0,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection::default(),
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: 1,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection::default(),
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    let mut after = before;
    after.queue.plan_state = IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED;
    after.queue.selected_count = 2;
    after.history.ever_queue_plan_v1 = true;
    let projection = ProductionInFlightFirstReleaseTransitionProjection {
        action: IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V1,
        actor: 0,
        target: 0,
        before,
        after,
    };

    let checked = check_production_in_flight_first_release_transition(projection)
        .expect("QueuePlan selection must pass the source-bound production wrapper");
    let witness = *checked
        .first_release_witness()
        .expect("production wrapper must attach a first-release witness");
    assert_eq!(
        witness.source_identity,
        production_in_flight_first_release_source_identity_body!()
    );
    assert!(
        authenticate_production_in_flight_first_release_transition_witness_v1(projection, witness,)
    );
    let model_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla"
    ));
    let model_digest = iroha_crypto::sha256(model_bytes);
    let source_words = [
        witness.source_identity.word0,
        witness.source_identity.word1,
        witness.source_identity.word2,
        witness.source_identity.word3,
    ];
    for (word, bytes) in source_words.into_iter().zip(model_digest.chunks_exact(8)) {
        assert_eq!(word.to_be_bytes().as_slice(), bytes);
    }

    // Every digest word is authenticated, including state digests which the
    // dependency-free structural witness kernel deliberately does not hash.
    for digest_index in 0..3 {
        for word_index in 0..4 {
            let mut changed = witness;
            let digest = match digest_index {
                0 => &mut changed.source_identity,
                1 => &mut changed.before_state_digest,
                _ => &mut changed.after_state_digest,
            };
            let word = match word_index {
                0 => &mut digest.word0,
                1 => &mut digest.word1,
                2 => &mut digest.word2,
                _ => &mut digest.word3,
            };
            *word ^= 1;
            assert_ne!(changed, witness);
            assert!(
                !authenticate_production_in_flight_first_release_transition_witness_v1(
                    projection, changed,
                ),
                "modified digest {digest_index} word {word_index} must fail authentication",
            );
        }
    }
    for parameter in 0..4 {
        let mut changed = witness;
        match parameter {
            0 => changed.schema_version ^= 1,
            1 => changed.action ^= 1,
            2 => changed.actor ^= 1,
            _ => changed.target ^= 1,
        }
        assert_ne!(changed, witness);
        assert!(
            !authenticate_production_in_flight_first_release_transition_witness_v1(
                projection, changed,
            ),
            "modified witness parameter {parameter} must fail authentication",
        );
    }
    let mut other_selection = projection;
    other_selection.after.queue.selected_count += 1;
    assert!(refinement::production_in_flight_first_release_transition_kernel(other_selection));
    assert!(
        !authenticate_production_in_flight_first_release_transition_witness_v1(
            other_selection,
            witness,
        ),
        "a witness must not authorize another valid QueuePlan selection",
    );
}

#[test]
fn replica_queue_observation_mints_exact_source_bound_witness() {
    let binding_a = CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_CANONICAL_PAYLOAD,
        [0x73; 32],
    );
    let before = ProductionInFlightFirstReleaseStateProjection {
        validator_count: 4,
        producer: 1,
        producer_selected_owner: 1,
        replicated_carrier_owners: 14,
        payload_binding_a: 3,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: 2,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: 3,
            ..ProductionInFlightFirstReleaseCarrierProjection::default()
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: 3,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v1: true,
            ever_reservation_v1: true,
            pending_high_water: 2,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection {
            release_owner: 2,
            release_scope: binding_a,
            ..ProductionInFlightFirstReleaseDecisionProjection::default()
        },
        release: ProductionInFlightFirstReleaseReleaseProjection {
            kura_retired: true,
            pending_prefix: 2,
            ..ProductionInFlightFirstReleaseReleaseProjection::default()
        },
    };
    assert!(production_in_flight_first_release_state_kernel(before));
    for exact_ordinary_fifo_preserved in [false, true] {
        let checked =
            check_production_in_flight_first_release_observe_replica_queue_release_transition(
                before,
                exact_ordinary_fifo_preserved,
            )
            .expect("an exact replica observation must pass the production wrapper");
        let witness = *checked
            .first_release_witness()
            .expect("replica observation must carry the source-bound production witness");
        let projection = checked.into_projection();
        assert_eq!(
            projection.action,
            refinement::IN_FLIGHT_FIRST_RELEASE_ACTION_OBSERVE_REPLICA_QUEUE_RELEASE,
        );
        assert_eq!(projection.actor, 0);
        assert_eq!(projection.target, u128::from(exact_ordinary_fifo_preserved));
        assert_eq!(projection.before, before);
        let mut expected_after = before;
        expected_after.queue.reservation_state = if exact_ordinary_fifo_preserved {
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED
        } else {
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT
        };
        assert_eq!(projection.after, expected_after);
        assert_eq!(
            witness.source_identity,
            production_in_flight_first_release_source_identity_body!(),
        );
        assert!(
            authenticate_production_in_flight_first_release_transition_witness_v1(
                projection, witness,
            )
        );
        for digest_index in 0..3 {
            for word_index in 0..4 {
                let mut changed = witness;
                let digest = match digest_index {
                    0 => &mut changed.source_identity,
                    1 => &mut changed.before_state_digest,
                    _ => &mut changed.after_state_digest,
                };
                let word = match word_index {
                    0 => &mut digest.word0,
                    1 => &mut digest.word1,
                    2 => &mut digest.word2,
                    _ => &mut digest.word3,
                };
                *word ^= 1;
                assert_ne!(changed, witness);
                assert!(
                    !authenticate_production_in_flight_first_release_transition_witness_v1(
                        projection, changed,
                    )
                );
            }
        }
        let alternate =
            check_production_in_flight_first_release_observe_replica_queue_release_transition(
                before,
                !exact_ordinary_fifo_preserved,
            )
            .expect("the other exact Queue disposition is independently valid")
            .into_projection();
        assert_ne!(alternate, projection);
        assert!(
            !authenticate_production_in_flight_first_release_transition_witness_v1(
                alternate, witness,
            )
        );
        let mut different_before = before;
        different_before.queue.selected_count += 1;
        different_before.release.pending_prefix += 1;
        different_before.history.pending_high_water += 1;
        let changed =
            check_production_in_flight_first_release_observe_replica_queue_release_transition(
                different_before,
                exact_ordinary_fifo_preserved,
            )
            .expect("a different complete claim prefix is independently valid")
            .into_projection();
        assert_ne!(changed.before, projection.before);
        assert_ne!(changed.after, projection.after);
        assert!(
            !authenticate_production_in_flight_first_release_transition_witness_v1(
                changed, witness,
            )
        );
    }
    let mut producer = before;
    producer.decision.release_owner = producer.producer;
    for exact_ordinary_fifo_preserved in [false, true] {
        assert!(
            check_production_in_flight_first_release_observe_replica_queue_release_transition(
                producer,
                exact_ordinary_fifo_preserved,
            )
            .is_none()
        );
    }
}
