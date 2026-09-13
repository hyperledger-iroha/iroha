// Certified publication receiver checks share State's bounded authentication owner.
#[test]
fn coordinator_publication_authenticates_once_inside_admission_owner() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x71,
    );
    let receiver = binding.admission_context.route_incarnations[0].validator_set[0].clone();
    let state = Arc::new(state);
    let observed_state = state.clone();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0_usize));
    let observed_calls = calls.clone();
    crate::torii_proxy::observe_queue_plan_authentication_for_test(
        move || {
            assert!(
                observed_state
                    .queue_plan_admission_persistence_lock
                    .try_lock()
                    .is_none(),
                "the publication graph must belong to the existing admission owner"
            );
            assert!(
                observed_state.state_commit_lock.try_lock().is_some(),
                "signature work must not hold the State publication fence"
            );
            observed_calls.set(observed_calls.get() + 1);
        },
        || {
            for inserted in [true, false] {
                let outcome = state
                    .persist_classified_queue_plan_admission(
                        &certificate,
                        QueuePlanAdmissionPersistenceScope::CoordinatorPublication(&receiver),
                    )
                    .expect("authorized coordinator publication");
                let PendingQueuePlanAdmissionPersistenceOutcome::Durable {
                    admission,
                    certificate_hash,
                    certificate: retained,
                    inserted: actual,
                    ..
                } = outcome
                else {
                    panic!("publication must have a durable owner")
                };
                assert_eq!(actual, inserted);
                assert_eq!(admission.certificate.binding, binding);
                assert_eq!(certificate_hash, Hash::new(&certificate));
                assert_eq!(retained, certificate);
                assert_eq!(
                    calls.replace(0),
                    1,
                    "receiver authorization and persistence use one authentication"
                );
            }
        },
    );
}

#[test]
fn coordinator_publication_rejects_outsider_before_inventory_or_idempotent_replay() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x72,
    );
    let outsider_key = KeyPair::try_from_seed(vec![0xF3; 32], Algorithm::BlsNormal)
        .expect("deterministic outsider fixture key");
    let outsider = PeerId::new(outsider_key.public_key().clone());
    assert!(
        !binding.admission_context.route_incarnations[0]
            .validator_set
            .contains(&outsider)
    );
    let hash = Hash::new(&certificate);
    for already_retained in [false, true] {
        let scans_before = state
            .kura
            .pending_queue_plan_admission_inventory_scans
            .load(Ordering::Relaxed);
        let error = state
            .persist_classified_queue_plan_admission(
                &certificate,
                QueuePlanAdmissionPersistenceScope::CoordinatorPublication(&outsider),
            )
            .expect_err("an outsider cannot receive a coordinator publication");
        assert!(
            error
                .to_string()
                .contains("not in the certified coordinator roster")
        );
        assert_eq!(
            state
                .kura
                .pending_queue_plan_admission_inventory_scans
                .load(Ordering::Relaxed),
            scans_before,
            "receiver authorization precedes inventory scanning"
        );
        assert_eq!(
            state
                .kura
                .pending_queue_plan_admission_certificate(hash)
                .expect("inspect exact durable bytes"),
            already_retained.then(|| certificate.clone())
        );
        if !already_retained {
            state
                .persist_classified_queue_plan_admission(
                    &certificate,
                    QueuePlanAdmissionPersistenceScope::Admission,
                )
                .expect("the independently authorized ingress can retain its admission");
        }
    }
}

#[test]
fn coordinator_publication_authentication_failure_cannot_change_durable_inventory() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x73,
    );
    let receiver = binding.admission_context.route_incarnations[0].validator_set[0].clone();
    let mut malformed = certificate.clone();
    malformed.truncate(malformed.len() / 2);
    let scans_before = state
        .kura
        .pending_queue_plan_admission_inventory_scans
        .load(Ordering::Relaxed);
    assert!(
        state
            .persist_classified_queue_plan_admission(
                &malformed,
                QueuePlanAdmissionPersistenceScope::CoordinatorPublication(&receiver),
            )
            .is_err()
    );
    assert_eq!(
        state
            .kura
            .pending_queue_plan_admission_inventory_scans
            .load(Ordering::Relaxed),
        scans_before
    );
    assert!(
        state
            .kura
            .pending_queue_plan_admission_certificate(Hash::new(&malformed))
            .expect("inspect rejected malformed certificate")
            .is_none()
    );
    assert!(
        state
            .kura
            .pending_queue_plan_admission_certificate(Hash::new(&certificate))
            .expect("inspect untouched valid certificate")
            .is_none()
    );
}
