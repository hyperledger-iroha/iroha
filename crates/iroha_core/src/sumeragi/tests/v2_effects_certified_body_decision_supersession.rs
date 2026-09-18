// Authenticated Decision exclusion consumes one real disk completion without a Ready child.

#[test]
fn admitted_certified_persistence_cancels_after_competing_durable_decision() {
    let result = crate::sumeragi::sumeragi_thread_builder(
        "admitted_certified_persistence_cancels_after_competing_durable_decision",
    )
    .spawn(|| {
        let mut pending = pending_body_fixture();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let fixture = &mut pending.ready;
            let old_key = (fixture.transport.round, fixture.transport.subject);
            let old_work = *fixture.transport.executor.pending_fetches.keys().next().unwrap();
            let old_pending = fixture.transport.executor.pending_fetches[&old_work].clone();
            let old_pipeline = fixture.transport.executor.body_pipeline_owners[&old_key];
            let mut selected_subject = fixture.transport.subject;
            selected_subject.block_hash = HashOf::from_untyped_unchecked(Hash::new(
                b"Decision excludes an already admitted persistence task",
            ));
            let certificate = fixture.transport.quorum_certificate_for(
                fixture.transport.round,
                selected_subject,
                wire::GlobalPhase::Commit,
                fixture.transport.canonical_commitment,
            );
            let now = Instant::now();
            let executor = &mut fixture.transport.executor;
            executor.arm_live_clocks(
                ProductionLifecycleLiveClockActivationPermitV1::for_test(), now,
            ).expect("arm actual serialized clocks");
            executor.enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
            )).expect("enqueue the signed competing Commit quorum");
            for _ in 0..32 {
                executor.step(now, fixture.services.as_mut())
                    .expect("install the actual durable Decision while disk custody remains");
                if executor.runtime.decided_body().unwrap().is_some() { break; }
            }
            let decision = executor.runtime.decided_body().unwrap().expect("signed Commit decides");
            assert_eq!(decision.2, selected_subject);
            assert_eq!(executor.protected_decision, Some(decision));
            assert!(!executor.decision_body_drained);
            assert_eq!(executor.pending_fetches.get(&old_work), Some(&old_pending));
            assert_eq!(executor.body_pipeline_owners.get(&old_key), Some(&old_pipeline));
            assert_eq!(executor.decision_persistence_readiness(fixture.services.as_ref()).unwrap(),
                DecisionPersistenceReadinessV1::Waiting(BTreeSet::from([old_work])));
            // The terminal cleanup guard must reject before any owner is retired.
            assert!(matches!(executor.reconcile_decision_work(decision, true, fixture.services.as_mut()),
                Err(EffectExecutorError::Contract(reason)) if reason.contains("admitted certified-body persistence")));
            assert_eq!(executor.pending_fetches.get(&old_work), Some(&old_pending));
            assert!(!executor.decision_body_drained);

            fixture.planner_io.execute_one_certified_fetch(Arc::clone(&executor.output_guard));
            let completion = match fixture.services.take_next_lifecycle_completion().unwrap() {
                LifecycleCompletionTakeV1::CertifiedFetch(completion) => completion,
                _ => panic!("the exact real worker result must remain selected"),
            };
            let settled = fixture.owner.complete_certified_fetch_for_test(
                executor, fixture.services.as_mut(), &pending.ingress, completion,
            );
            if let Err(error) = settled {
                match error {
                    crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(error) =>
                        panic!("Decision cancellation retained a retry: {}: {}", error.reason(), error.detail()),
                    _ => panic!("authenticated Decision cancellation failed closed"),
                }
            }
            assert!(matches!(fixture.owner.fetch_wait_projection_for_test(fixture.ordinal, pending.source),
                (Some(LifecycleState::Terminal(crate::sumeragi::v2_lifecycle_coordinator::TerminalOutcome::Cancelled)), _, None, false)));
            assert!(!executor.pending_fetches.contains_key(&old_work));
            assert!(!executor.body_pipeline_owners.contains_key(&old_key));
            assert!(!executor.durable_bodies.contains_key(&old_key));
            assert!(!executor.ready_bodies.contains_key(&old_key));
            assert!(fixture.services.certified_fetch_persistence_work().is_empty());
            assert_eq!(executor.decision_persistence_readiness(fixture.services.as_ref()).unwrap(),
                DecisionPersistenceReadinessV1::Ready);
            assert!(!executor.output_guard.restart_required());
            executor.reconcile_decision_work(decision, true, fixture.services.as_mut())
                .expect("strict terminal cleanup succeeds after the physical result settles");
            assert!(executor.decision_body_drained);
            assert_eq!(executor.pending_work(), 0);
            assert!(executor.certified_work.is_empty());
            assert!(executor.outstanding_requests.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
        }));
        pending.ready.planner_io.detach(pending.ready.services.as_mut());
        if let Err(payload) = result { std::panic::resume_unwind(payload); }
    })
    .expect("spawn on the production consensus stack")
    .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn admitted_certified_persistence_retains_production_completion_claim_until_ack() {
    let result = crate::sumeragi::sumeragi_thread_builder(
        "admitted_certified_persistence_retains_production_completion_claim_until_ack",
    )
    .spawn(|| {
        let mut pending = pending_body_fixture();
        let fixture = &mut pending.ready;
        let assert_claimed = |services: &ProductionV2Services| {
            // This is the exact fresh predicate used by the launched owner's
            // producer_claim_projection to exclude Ready dispatch and Ingress.
            assert_eq!(
                services.has_unleased_lifecycle_completion_work(),
                Some(true)
            );
            assert_eq!(services.certified_fetch_persistence_work().len(), 1);
        };
        assert_claimed(fixture.services.as_ref());
        fixture
            .planner_io
            .execute_one_certified_fetch_with_active_observer(
                Arc::clone(&fixture.transport.executor.output_guard),
                || assert_claimed(fixture.services.as_ref()),
            );
        assert_claimed(fixture.services.as_ref());
        let completion = match fixture.services.take_next_lifecycle_completion().unwrap() {
            LifecycleCompletionTakeV1::CertifiedFetch(completion) => completion,
            _ => panic!("the real disk result remains next"),
        };
        assert_claimed(fixture.services.as_ref());
        let mut settled = settle_pending_body_fixture(pending, completion);
        assert_eq!(
            settled.services.has_unleased_lifecycle_completion_work(),
            Some(false)
        );
        settled.planner_io.detach(settled.services.as_mut());
    })
    .expect("spawn on production consensus stack")
    .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}
