// Certified publication receiver checks share State's bounded authentication owner.
#[test]
fn pending_queue_plan_replay_requires_exact_live_durable_binding() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let route = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, 0x70);
    let (binding, certificate) = queue_plan_admission_certificate_for_entrypoint_state_test(
        &state,
        route.clone(),
        &validators,
        parent.header().height().get(),
        0x70,
        &entrypoint,
    );
    let signed_hash = binding
        .signed_transaction_hash
        .expect("complete signed admission has a signed identity");
    let lookup =
        || state.pending_queue_plan_admission_for_transaction(binding.entrypoint_hash, signed_hash);
    assert!(!lookup().expect("a fresh transaction has no durable custody"));
    assert!(matches!(
        state
            .persist_classified_queue_plan_admission(
                &certificate,
                QueuePlanAdmissionPersistenceScope::Admission,
            )
            .expect("persist exact certified pending input"),
        PendingQueuePlanAdmissionPersistenceOutcome::Durable { inserted: true, .. }
    ));
    assert!(lookup().expect("exact pending certificate authenticates"));
    assert!(
        !state
            .pending_queue_plan_admission_for_transaction(
                HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                    b"other entrypoint"
                )),
                signed_hash,
            )
            .expect("another entrypoint has no pending custody")
    );
    assert!(
        state
            .pending_queue_plan_admission_for_transaction(
                binding.entrypoint_hash,
                HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(
                    b"other signed transaction"
                )),
            )
            .is_err(),
        "a different signed identity cannot claim this entrypoint"
    );

    let alternate = queue_plan_admission_certificate_bytes_for_signer_indices_state_test(
        &entrypoint,
        &binding,
        &validators,
        &[2, 3],
    );
    state
        .kura
        .persist_pending_queue_plan_admission_certificate(&alternate)
        .expect("seed another valid quorum over the same binding");
    assert!(lookup().expect("alternate signer subsets retain one logical admission"));

    let (conflicting_binding, conflicting) =
        queue_plan_admission_certificate_for_entrypoint_state_test(
            &state,
            route,
            &validators,
            parent.header().height().get(),
            0x71,
            &entrypoint,
        );
    assert_eq!(conflicting_binding.entrypoint_hash, binding.entrypoint_hash);
    assert_eq!(
        conflicting_binding.signed_transaction_hash,
        binding.signed_transaction_hash
    );
    assert_ne!(conflicting_binding, binding);
    let conflicting_hash = state
        .kura
        .persist_pending_queue_plan_admission_certificate(&conflicting)
        .expect("seed a conflicting authenticated binding");
    assert!(
        lookup().is_err(),
        "a conflicting pending binding fails closed"
    );
    state
        .remove_pending_queue_plan_admission_certificate(conflicting_hash)
        .expect("remove conflicting test fixture");

    let malformed_hash = state
        .kura
        .persist_pending_queue_plan_admission_certificate(b"malformed")
        .expect("seed corrupt durable evidence");
    assert!(lookup().is_err(), "corrupt pending inventory fails closed");
    state
        .remove_pending_queue_plan_admission_certificate(malformed_hash)
        .expect("remove corrupt test fixture");
    assert!(lookup().expect("exact custody remains live after fixture cleanup"));
}

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

// QueuePlan publication controls use explicit handshakes, never sleep-based schedules.
#[test]
fn pending_queue_plan_busy_kura_releases_state_and_reclassifies_after_publication() {
    for change_incarnation in [false, true] {
        let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
        let (_, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            )),
            &validators,
            parent.header().height().get(),
            0x74,
        );
        let certificate_hash = Hash::new(&certificate);
        let successor = empty_global_block_after(Some(&parent));
        state
            .kura
            .store_block(Arc::new(successor.clone()))
            .expect("durable successor precedes its State publication");
        let state = Arc::new(state);
        let canonical_writer = state.kura.canonical_publication_lease();
        let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let writer_state = Arc::clone(&state);
        let writer = std::thread::spawn(move || {
            let observed = Arc::clone(&writer_state);
            let calls = std::rc::Rc::new(std::cell::Cell::new(0_usize));
            let observed_calls = calls.clone();
            let outcome = crate::torii_proxy::observe_queue_plan_authentication_for_test(
                move || observed_calls.set(observed_calls.get() + 1),
                || {
                    queue_plan_publication_wait_observer::observe(
                        move || {
                            assert!(
                                observed.state_commit_lock.try_lock().is_some(),
                                "a busy Kura writer must not pin the State fence"
                            );
                            assert!(
                                observed
                                    .queue_plan_admission_persistence_lock
                                    .try_lock()
                                    .is_none(),
                                "the admission mutation owner must survive the released interval"
                            );
                            waiting_tx
                                .send(())
                                .expect("announce the released State boundary");
                            resume_rx
                                .recv_timeout(Duration::from_secs(5))
                                .expect("resume after the explicit State publication");
                        },
                        || {
                            writer_state.persist_classified_queue_plan_admission(
                                &certificate,
                                QueuePlanAdmissionPersistenceScope::Admission,
                            )
                        },
                    )
                },
            );
            (outcome, calls.get())
        });
        waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("admission reaches the released boundary behind the held Kura writer");
        {
            let _state_fence = state
                .state_commit_lock
                .try_lock()
                .expect("publication is available while Kura remains exclusively owned");
            // This mutates the block-hash journal that the old StateView owned.
            // Reaching the resume handshake therefore requires its view to be dropped.
            state.append_committed_block_header_for_tests(successor.header());
            if change_incarnation {
                let _ = state.set_lane_incarnation_for_test(
                    LaneId::SINGLE,
                    Hash::new(b"published-incarnation-during-kura-contention"),
                );
            }
        }
        drop(canonical_writer);
        resume_tx
            .send(())
            .expect("release the observer after State publication");
        let (outcome, calls) = writer.join().expect("join the owned admission writer");
        let outcome = outcome.expect("reclassify at the newly published State frontier");
        assert_eq!(calls, 1, "frontier retries reuse immutable authentication");
        assert_eq!(
            u64::try_from(state.committed_height()).expect("height fits u64"),
            successor.header().height().get()
        );
        if change_incarnation {
            assert!(
                matches!(
                    outcome,
                    PendingQueuePlanAdmissionPersistenceOutcome::Rejected {
                        disposition: PendingQueuePlanAdmissionDisposition::Stale,
                        ..
                    }
                ),
                "the old incarnation must not survive the released classification: {outcome:?}"
            );
            assert!(
                state
                    .kura
                    .pending_queue_plan_admission_certificate(certificate_hash)
                    .expect("inspect rejected stale sidecar")
                    .is_none()
            );
        } else {
            assert!(
                matches!(outcome,
                    PendingQueuePlanAdmissionPersistenceOutcome::Durable {
                        certificate_hash: actual, inserted: true, ..
                    } if actual == certificate_hash
                ),
                "unchanged authority becomes durable after exact frontier catch-up"
            );
        }
    }
}

#[test]
fn pending_queue_plan_frontier_mismatch_preserves_all_retirement_candidates() {
    for durable_lead in [-1_i8, 1, 2] {
        let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
        let (binding, first) = queue_plan_admission_certificate_for_state_test(
            &state,
            crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            )),
            &validators,
            parent.header().height().get(),
            0x75,
        );
        let second = queue_plan_admission_certificate_bytes_for_signer_indices_state_test(
            &queue_plan_entrypoint_for_state_test(&state, 0x75),
            &binding,
            &validators,
            &[2, 3],
        );
        let incoming = queue_plan_admission_certificate_bytes_for_signer_indices_state_test(
            &queue_plan_entrypoint_for_state_test(&state, 0x75),
            &binding,
            &validators,
            &[0, 3],
        );
        assert_ne!(first, second);
        assert_ne!(incoming, first);
        assert_ne!(incoming, second);
        // Seed two independently authenticated signer subsets in the raw storage fixture.
        // Classification would retain one and retire the duplicate, but only after preflight.
        for certificate in [&first, &second] {
            state
                .kura
                .persist_pending_queue_plan_admission_certificate(certificate)
                .expect("seed an exact retirement candidate");
        }
        let before = state
            .kura
            .pending_queue_plan_admission_certificates()
            .expect("capture both exact sidecar owners");
        let successor = empty_global_block_after(Some(&parent));
        if durable_lead < 0 {
            state.append_committed_block_header_for_tests(successor.header());
        } else {
            state
                .kura
                .store_block(Arc::new(successor.clone()))
                .expect("advance the durable frontier alone");
            if durable_lead > 1 {
                let later = empty_global_block_after(Some(&successor));
                state
                    .kura
                    .store_block(Arc::new(later))
                    .expect("advance beyond the one-ahead reconciliation case");
            }
        }
        let error = state
            .persist_classified_queue_plan_admission(
                &incoming,
                QueuePlanAdmissionPersistenceScope::Admission,
            )
            .expect_err("every unmatched frontier must refuse retirement and publication");
        assert!(matches!(
            error,
            MergeLedgerCommitError::Persistence(
                crate::kura::Error::QueuePlanAdmissionDurableHeightMismatch { .. }
            )
        ));
        assert_eq!(
            state
                .kura
                .pending_queue_plan_admission_certificates()
                .expect("read untouched retirement candidates"),
            before
        );
        assert!(
            state
                .kura
                .pending_queue_plan_admission_certificate(Hash::new(&incoming))
                .expect("inspect the unpublished incoming certificate")
                .is_none()
        );
    }
}

#[test]
fn pending_queue_plan_checked_publication_deduplicates_and_retires_alternate_subsets() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let (binding, first) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x76,
    );
    let second = queue_plan_admission_certificate_bytes_for_signer_indices_state_test(
        &queue_plan_entrypoint_for_state_test(&state, 0x76),
        &binding,
        &validators,
        &[2, 3],
    );
    let incoming = queue_plan_admission_certificate_bytes_for_signer_indices_state_test(
        &queue_plan_entrypoint_for_state_test(&state, 0x76),
        &binding,
        &validators,
        &[0, 3],
    );
    for certificate in [&first, &second] {
        state
            .kura
            .persist_pending_queue_plan_admission_certificate(certificate)
            .expect("seed alternate signer subsets");
    }
    let inventory = state
        .kura
        .pending_queue_plan_admission_certificates()
        .expect("read deterministic inventory order");
    let (expected_hash, expected_bytes) = inventory[0].clone();
    let outcome = state
        .persist_classified_queue_plan_admission(
            &incoming,
            QueuePlanAdmissionPersistenceScope::Admission,
        )
        .expect("reuse one exact logical owner after guarded retirement");
    assert!(matches!(outcome,
        PendingQueuePlanAdmissionPersistenceOutcome::Durable {
            certificate_hash, certificate, inserted: false, ..
        } if certificate_hash == expected_hash && certificate == expected_bytes
    ));
    assert_eq!(
        state
            .kura
            .pending_queue_plan_admission_certificates()
            .expect("read deduplicated inventory"),
        vec![(expected_hash, expected_bytes.clone())]
    );
    let repeated = state
        .persist_classified_queue_plan_admission(
            &expected_bytes,
            QueuePlanAdmissionPersistenceScope::Admission,
        )
        .expect("exact-hash retry after deduplication");
    assert!(matches!(repeated,
        PendingQueuePlanAdmissionPersistenceOutcome::Durable {
            certificate_hash, inserted: false, ..
        } if certificate_hash == expected_hash
    ));
}

#[test]
fn pending_queue_plan_stale_short_circuit_never_waits_for_kura_publication() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x77,
    );
    let _ = state.set_lane_incarnation_for_test(LaneId::SINGLE, Hash::new(b"stale-before-kura"));
    let _canonical_writer = state.kura.canonical_publication_lease();
    let outcome = queue_plan_publication_wait_observer::observe(
        || panic!("stale classification must return before the Kura wait boundary"),
        || {
            state.persist_classified_queue_plan_admission(
                &certificate,
                QueuePlanAdmissionPersistenceScope::Admission,
            )
        },
    )
    .expect("return the stale disposition with Kura still exclusively owned");
    assert!(matches!(
        outcome,
        PendingQueuePlanAdmissionPersistenceOutcome::Rejected {
            disposition: PendingQueuePlanAdmissionDisposition::Stale,
            ..
        }
    ));
}

#[test]
fn pending_queue_plan_height_mismatch_preserves_stale_conflicting_binding() {
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let routing = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, 0x78);
    let (old_binding, old_certificate) = queue_plan_admission_certificate_for_entrypoint_state_test(
        &state,
        routing.clone(),
        &validators,
        parent.header().height().get(),
        0x78,
        &entrypoint,
    );
    let old_hash = state
        .kura
        .persist_pending_queue_plan_admission_certificate(&old_certificate)
        .expect("seed a certificate that the new State will classify as stale");
    let successor = empty_global_block_after(Some(&parent));
    state.append_committed_block_header_for_tests(successor.header());
    let _ =
        state.set_lane_incarnation_for_test(LaneId::SINGLE, Hash::new(b"replacement-authority"));
    let (new_binding, incoming) = queue_plan_admission_certificate_for_entrypoint_state_test(
        &state,
        routing,
        &validators,
        successor.header().height().get(),
        0x79,
        &entrypoint,
    );
    assert_eq!(old_binding.entrypoint_hash, new_binding.entrypoint_hash);
    assert_ne!(old_binding, new_binding);
    let carrier_height = successor.header().height().get() + 1;
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&old_certificate, carrier_height)
            .expect("classify retained old authority")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&incoming, carrier_height)
            .expect("classify replacement authority")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent
    );
    assert!(matches!(
        state.persist_classified_queue_plan_admission(
            &incoming,
            QueuePlanAdmissionPersistenceScope::Admission,
        ),
        Err(MergeLedgerCommitError::Persistence(
            crate::kura::Error::QueuePlanAdmissionDurableHeightMismatch { .. }
        ))
    ));
    assert_eq!(
        state
            .kura
            .pending_queue_plan_admission_certificates()
            .expect("inspect the unretired conflicting owner"),
        vec![(old_hash, old_certificate)]
    );
    assert!(
        state
            .kura
            .pending_queue_plan_admission_certificate(Hash::new(&incoming))
            .expect("inspect unpublished replacement")
            .is_none()
    );
}
