#[derive(Clone, Copy, PartialEq, Eq)]
enum ValidateRetryOriginForTest {
    Live,
    Recovered,
    Published,
    ColdTerminal,
    UnprotectedTerminal,
    UnprotectedPublishedTerminal,
}

fn published_validate_retry_fixture() -> (ReadyBodyFixture, u128) {
    let mut fixture = ready_body_fixture();
    let mut parent = fixture.ordinal;
    for expected in [LifecycleWorkClass::Store, LifecycleWorkClass::Validate] {
        let advanced = fixture
            .owner
            .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
            .expect("publish the real Fetch/Store body successor before EnterView");
        let ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child_ordinal,
            child,
        } = advanced
        else {
            panic!("the exact body pipeline must publish its next owner")
        };
        assert_eq!(parent_ordinal, parent);
        assert_eq!(child, expected);
        parent = child_ordinal;
    }
    let key = (fixture.transport.round, fixture.transport.subject);
    assert!(
        fixture
            .transport
            .executor
            .durable_validate_retry_seals
            .is_empty()
    );
    assert_eq!(
        fixture
            .transport
            .executor
            .published_lifecycle_validate_retry_markers
            .len(),
        1
    );
    assert_eq!(
        fixture
            .transport
            .executor
            .published_lifecycle_validate_retry_markers[&key]
            .lifecycle_ordinal,
        Some(parent)
    );
    (fixture, parent)
}

// A real physical Validate terminal must remain a usable result source after EnterView.
fn live_validate_retry_fixture() -> (ReadyBodyFixture, u128) {
    let mut fixture = ready_body_fixture();
    let advanced = fixture
        .owner
        .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
        .expect("advance the authenticated Fetch to its real Store");
    let ProductionCompletionDispatchV1::BodyStageAdvanced {
        child_ordinal,
        child: LifecycleWorkClass::Store,
        ..
    } = advanced
    else {
        panic!("the authenticated Fetch must create Store")
    };
    fixture.ordinal = child_ordinal;
    let mut current_services = FakeServices::default();
    let started = Instant::now();
    install_timeout(&mut fixture, true, &mut current_services, started);
    let new_tag = fixture.transport.executor.current_tag();
    assert_eq!(fixture.owner.dispatch_completion_for_test(
        &mut fixture.services, &mut fixture.transport.executor, 0,
    ).expect("retire the obsolete Store with its exact owner"),
        ProductionCompletionDispatchV1::BodyStageCancelled { ordinal: child_ordinal, stage: LifecycleWorkClass::Store });
    fixture
        .transport
        .executor
        .step(started, &mut current_services)
        .expect("service the retained TC and current protected Fetch");
    let ordinal = drive_current_body_to_validate(
        &mut fixture,
        &mut current_services,
        started,
        new_tag,
        false,
    );
    let key = (fixture.transport.round, fixture.transport.subject);
    assert!(matches!(
        fixture.transport.executor.durable_validate_retry_seals[&key],
        DurableValidateRetrySealV1::Live { .. }
    ));
    (fixture, ordinal)
}

#[derive(Clone, Copy)]
enum FixtureValidationReplay {
    NoMarkers,
    Validated,
    Rejected,
}

fn reopen_body_owner_fixture(
    fixture: ReadyBodyFixture,
    replay: FixtureValidationReplay,
) -> (
    ReadyBodyFixture,
    Arc<crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
) {
    let ReadyBodyFixture {
        mut transport,
        owner,
        planner_io,
        mut services,
        _owner_directory: directory,
        certificate,
        ordinal,
    } = fixture;
    let validator = transport
        .executor
        .local_validator
        .expect("the restart retains the actual local committee member");
    let verified = VerifiedHeightContext::genesis(
        transport.context.clone(),
        transport
            .validator_keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen PoP"))
            .collect(),
    )
    .expect("authenticate the exact recovered context");
    let wal_path = transport
        ._directory
        .path()
        .join("transport-regression-safety.wal");
    planner_io.detach(&mut services);
    drop(services);
    drop(owner);
    drop(transport.executor);
    let expected_body = transport.body.clone();
    let expected_commitment = transport.canonical_commitment;
    let mut replayed = 0usize;
    let mut owner = SumeragiV2Adapter::reopen_cancelled_body_owner_for_test(
        &wal_path,
        directory.path(),
        verified,
        validator,
        &transport.validator_keys[usize::try_from(validator).expect("local index")],
        AdapterFingerprints {
            node: Hash::new(b"production transport node"),
            build: Hash::new(b"production transport build"),
            config: Hash::new(b"production transport config"),
        },
        [0x63; 32],
        |body| {
            assert_eq!(
                body.encode_wire().expect("canonical replayed fixture body"),
                expected_body,
                "semantic replay must execute the exact originally validated body"
            );
            replayed += 1;
            match replay {
                FixtureValidationReplay::NoMarkers => {
                    panic!("an active Validate fixture has no completed marker to replay")
                }
                FixtureValidationReplay::Validated => Ok(expected_commitment),
                FixtureValidationReplay::Rejected => {
                    Err("deterministic terminal Validate regression rejection".to_owned())
                }
            }
        },
    );
    assert_eq!(
        replayed,
        usize::from(!matches!(replay, FixtureValidationReplay::NoMarkers)),
        "cold startup must replay each terminal outcome exactly once"
    );
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let (executor, planner_io, leader_wire_gate, ordinals) = owner
        .bind_recovered_cancelled_body_executor_for_test(
            &wal_path,
            &mut services,
            ConsensusOutputGuard::isolated(),
            validator,
        );
    transport.executor = executor;
    transport._lifecycle_ordinals = ordinals;
    (
        ReadyBodyFixture {
            transport,
            owner,
            planner_io,
            services,
            _owner_directory: directory,
            certificate,
            ordinal,
        },
        leader_wire_gate,
    )
}

/// Exercise the actual ReleasedTerminal startup consumer while the Apply is
/// still Ready; a generic body-fixture binder cannot consume this pending Apply.
fn assert_released_apply_owner_cold_reopens(
    fixture: ReadyBodyFixture,
    terminal: &crate::sumeragi::v2_lifecycle_coordinator::ResolvedValidateOwnerSnapshotForTest,
) {
    let ReadyBodyFixture {
        transport,
        owner,
        planner_io,
        mut services,
        _owner_directory: directory,
        ..
    } = fixture;
    let ledger_root = directory.path().join("ledger");
    let snapshot = owner.released_apply_owner_snapshot_for_test(&ledger_root);
    let validator = transport
        .executor
        .local_validator
        .expect("same local validator");
    let verified = VerifiedHeightContext::genesis(
        transport.context.clone(),
        transport
            .validator_keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen PoP"))
            .collect(),
    )
    .expect("authenticate the exact Decision height context");
    let wal_path = transport
        ._directory
        .path()
        .join("transport-regression-safety.wal");
    planner_io.detach(&mut services);
    drop(services);
    drop(owner);
    drop(transport.executor);
    let mut replayed = 0usize;
    let reopened = SumeragiV2Adapter::reopen_cancelled_body_owner_for_test(
        &wal_path,
        directory.path(),
        verified,
        validator,
        &transport.validator_keys[usize::try_from(validator).expect("local index")],
        AdapterFingerprints {
            node: Hash::new(b"production transport node"),
            build: Hash::new(b"production transport build"),
            config: Hash::new(b"production transport config"),
        },
        [0x63; 32],
        |body| {
            assert_eq!(
                body.encode_wire().expect("canonical Decision body"),
                transport.body,
                "cold Apply must semantically replay the exact originally validated body",
            );
            replayed += 1;
            Ok(transport.canonical_commitment)
        },
    );
    assert_eq!(
        replayed, 1,
        "the retained success requires one semantic replay"
    );
    reopened.resolved_validate_cold_snapshot_for_test(terminal, &ledger_root);
    reopened.assert_released_apply_owner_cold_for_test(&snapshot, &ledger_root);
}

fn reopen_validate_retry_fixture(
    fixture: ReadyBodyFixture,
    validate_ordinal: u128,
) -> (
    ReadyBodyFixture,
    Arc<crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
) {
    let expected = fixture
        .owner
        .body_recovery_snapshot_for_test(fixture.ordinal, validate_ordinal);
    let (fixture, gate) = reopen_body_owner_fixture(fixture, FixtureValidationReplay::NoMarkers);
    fixture
        .owner
        .assert_body_recovery_snapshot_for_test(&expected);
    let key = (fixture.transport.round, fixture.transport.subject);
    assert!(matches!(
        fixture.transport.executor.durable_validate_retry_seals[&key],
        DurableValidateRetrySealV1::Recovered { .. }
    ));
    (fixture, gate)
}

fn physically_resolved_validate_retry_fixture(
    origin: ValidateRetryOriginForTest,
    rejected: bool,
    busy: bool,
) -> (
    ReadyBodyFixture,
    u128,
    crate::sumeragi::v2_lifecycle_coordinator::LifecycleDigest,
    Option<Arc<crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleStoreGate>>,
    FakeServices,
    Instant,
) {
    let (fixture, validate_ordinal) = if matches!(
        origin,
        ValidateRetryOriginForTest::Published
            | ValidateRetryOriginForTest::UnprotectedPublishedTerminal
    ) {
        published_validate_retry_fixture()
    } else {
        live_validate_retry_fixture()
    };
    let (mut fixture, leader_wire_gate) = if origin == ValidateRetryOriginForTest::Recovered {
        let (fixture, gate) = reopen_validate_retry_fixture(fixture, validate_ordinal);
        (fixture, Some(gate))
    } else {
        (fixture, None)
    };
    let key = (fixture.transport.round, fixture.transport.subject);
    let old_tag = fixture.transport.executor.current_tag();
    crate::sumeragi::v2_worker::tests::install_active_tag_for_test(&mut fixture.services, old_tag);
    let pending_digest = fixture
        .owner
        .validate_slot_digest_for_retry_test(validate_ordinal);
    let local = usize::try_from(
        fixture
            .transport
            .executor
            .local_validator
            .expect("local validator"),
    )
    .expect("local index");
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
        &mut fixture.services,
        &fixture.transport.validator_keys[local],
    );
    assert_eq!(fixture.owner.dispatch_completion_for_test(
        &mut fixture.services, &mut fixture.transport.executor, 0,
    ).expect("queue the genuine inherited Validate"), ProductionCompletionDispatchV1::ValidateQueued { ordinal: validate_ordinal });
    fixture.planner_io.activate_one_lifecycle_validate();
    let mut current_services = FakeServices::default();
    let now = Instant::now();
    if busy {
        let sign = fixture
            .transport
            .executor
            .runtime
            .driver_mut_for_test()
            .timeout_elapsed(old_tag)
            .expect("persist the exact current TimeoutIntent before physical completion");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));
    } else {
        install_timeout(&mut fixture, true, &mut current_services, now);
    }
    let callbacks = if rejected {
        fixture
            .planner_io
            .execute_held_lifecycle_validate_rejection_fixture(Arc::clone(
                &fixture.transport.executor.output_guard,
            ))
    } else {
        fixture.planner_io.execute_held_lifecycle_validate_fixture(
            fixture.transport.canonical_commitment,
            Arc::clone(&fixture.transport.executor.output_guard),
        )
    };
    assert_eq!(callbacks, 1, "one actual deterministic validation callback");
    let completion = match fixture
        .services
        .take_next_lifecycle_completion()
        .expect("actual physical Validate result")
    {
        LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("the held Validate must return its exact completion"),
    };
    let mut successor = fixture.owner.publish_validate_successor_for_retry_test(
        completion,
        validate_ordinal,
        rejected,
    );
    if busy {
        let observed = fixture
            .owner
            .dispatch_ready_validate_successor_for_test(
                &mut fixture.services,
                &mut fixture.transport.executor,
                successor,
                0,
            )
            .expect("Busy must retain the exact physical result");
        let crate::sumeragi::v2_lifecycle_coordinator::ReadyValidateSuccessorDispatchV1::ReducerFencePending {
            successor: retained, wait,
        } = observed else { panic!("the actual TimeoutIntent fence must park the completion") };
        successor = retained;
        assert_eq!(
            fixture
                .transport
                .executor
                .validate_retry_lifecycle_ordinal_for_test(key),
            Some(Some(validate_ordinal))
        );
        assert!(fixture.owner.apply_ordinals_for_retry_test().is_empty());
        let again = fixture
            .owner
            .dispatch_ready_validate_successor_for_test(
                &mut fixture.services,
                &mut fixture.transport.executor,
                successor,
                0,
            )
            .expect("unchanged Busy fence must preserve the same result owner");
        let crate::sumeragi::v2_lifecycle_coordinator::ReadyValidateSuccessorDispatchV1::ReducerFencePending {
            successor: retained, wait: repeated,
        } = again else { panic!("an unchanged Busy fence cannot consume the result") };
        assert_eq!(wait, repeated);
        successor = retained;
        install_timeout(&mut fixture, true, &mut current_services, now);
    }
    assert!(
        fixture
            .transport
            .executor
            .current_tag()
            .strictly_advances(old_tag)
    );
    assert!(
        matches!(fixture.owner.dispatch_ready_validate_successor_for_test(
        &mut fixture.services, &mut fixture.transport.executor, successor, 0,
    ).expect("publish the current reducer's genuine NoSuccessor cut"),
        crate::sumeragi::v2_lifecycle_coordinator::ReadyValidateSuccessorDispatchV1::Resolved(
            ProductionCompletionDispatchV1::ValidateNoSuccessor { ordinal }
        ) if ordinal == validate_ordinal)
    );
    (
        fixture,
        validate_ordinal,
        pending_digest,
        leader_wire_gate,
        current_services,
        now,
    )
}

fn resolved_validate_owner_retries_commit_fixture(
    origin: ValidateRetryOriginForTest,
    corrupt_terminal: bool,
    prepare_first: bool,
    busy: bool,
) {
    let (
        mut fixture,
        validate_ordinal,
        pending_digest,
        mut _leader_wire_gate,
        mut current_services,
        mut now,
    ) = physically_resolved_validate_retry_fixture(origin, false, busy);
    let key = (fixture.transport.round, fixture.transport.subject);
    let ledger_root = fixture._owner_directory.path().join("ledger");
    let mut terminal = fixture.owner.resolved_validate_owner_snapshot_for_test(
        validate_ordinal,
        pending_digest,
        &ledger_root,
    );
    if origin == ValidateRetryOriginForTest::ColdTerminal {
        let (reopened, gate) =
            reopen_body_owner_fixture(fixture, FixtureValidationReplay::Validated);
        fixture = reopened;
        _leader_wire_gate = Some(gate);
        terminal = fixture
            .owner
            .resolved_validate_cold_snapshot_for_test(&terminal, &ledger_root);
        current_services = FakeServices::default();
        now = Instant::now();
        fixture
            .transport
            .executor
            .arm_live_clocks(
                ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                now,
            )
            .expect("activate the ordinary runtime after authenticated terminal recovery");
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .command_depth(),
            0
        );
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .physical_admissions(),
            0
        );
        assert_eq!(
            fixture
                .transport
                .executor
                .validate_retry_lifecycle_ordinal_for_test(key),
            Some(None)
        );
    }
    if matches!(
        origin,
        ValidateRetryOriginForTest::UnprotectedTerminal
            | ValidateRetryOriginForTest::UnprotectedPublishedTerminal
    ) {
        fixture
            .transport
            .executor
            .step(now, &mut current_services)
            .expect("consume only the already-owned EnterView and protected Fetch");
        assert!(
            fixture
                .transport
                .executor
                .pending_durable_validate_admissions
                .is_empty()
        );
        let prior = fixture.transport.executor.current_tag();
        install_timeout(
            &mut fixture,
            false,
            &mut current_services,
            now + Duration::from_millis(1),
        );
        // The split runtime step may first service an older deferred owner.
        // Drive the already-enqueued TC through ordinary scheduling instead
        // of treating any Advanced macro-step as its view installation.
        let expected_view = prior.view().checked_add(1).expect("next fixture view");
        for turn in 0..32_u64 {
            let executor = &mut fixture.transport.executor;
            if executor.current_tag().view() == expected_view {
                break;
            }
            executor
                .step(now + Duration::from_millis(2 + turn), &mut current_services)
                .expect("service the queued unprotected TC after older owners");
            let _settlement = executor
                .settle_pending_lifecycle_output_admissions(
                    &mut fixture.owner,
                    &mut current_services,
                )
                .expect("settle preceding control output before TC installation");
        }
        assert_eq!(
            fixture.transport.executor.current_tag().view(),
            expected_view
        );
        assert!(
            fixture
                .transport
                .executor
                .current_tag()
                .strictly_advances(prior)
        );
        assert_eq!(
            fixture
                .transport
                .executor
                .validate_retry_lifecycle_ordinal_for_test(key),
            Some(None),
            "an unprotected view cannot discard the immutable completed result while its terminal key survives"
        );
        fixture
            .owner
            .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
    }
    let current_tag = fixture.transport.executor.current_tag();
    assert_eq!(
        fixture
            .transport
            .executor
            .validate_retry_lifecycle_ordinal_for_test(key),
        Some(None)
    );
    fixture
        .owner
        .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);

    if prepare_first {
        let mut settled = 0;
        for turn in 0..24_u64 {
            let executor = &mut fixture.transport.executor;
            executor
                .step(now + Duration::from_millis(turn), &mut current_services)
                .expect("reconstruct the current historical Prepare body");
            let _settlement = executor
                .settle_pending_lifecycle_output_admissions(
                    &mut fixture.owner,
                    &mut current_services,
                )
                .expect("publish exact current Prepare/TC output");
            executor
                .settle_pending_durable_validate_admissions(
                    &mut fixture.owner,
                    &mut current_services,
                )
                .expect("settle exact current-body registry admission");
            settled += executor
                .settle_pending_released_validate_apply_publication(
                    &mut fixture.owner,
                    &mut current_services,
                )
                .expect("replay the actual cached historical Prepare result");
            if executor.runtime.driver().body_state_for_test(key.0, key.1)
                == crate::sumeragi::v2_core::BodyState::Validated
            {
                break;
            }
        }
        assert_eq!(settled, 1, "one cached Prepare replay repairs the body");
        assert_eq!(
            fixture
                .transport
                .executor
                .runtime
                .driver()
                .body_state_for_test(key.0, key.1),
            crate::sumeragi::v2_core::BodyState::Validated
        );
        assert_eq!(fixture.transport.executor.current_tag(), current_tag);
        assert!(key.0.view < current_tag.view());
        assert!(
            !fixture
                .transport
                .executor
                .has_pending_live_wal_sign_admission(),
            "a historical Prepare cannot sign a new vote for the closed view"
        );
        assert!(current_services.sign_tasks.is_empty());
        assert!(current_services.apply_tasks.is_empty());
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .command_depth(),
            0
        );
        fixture
            .owner
            .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
    }

    let io_before_commit = fixture.planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(io_before_commit.physical_admissions(), 0);
    if corrupt_terminal {
        fixture
            .owner
            .substitute_resolved_validate_digest_for_test(&terminal, true);
    }

    let commit = fixture.transport.quorum_certificate(
        wire::GlobalPhase::Commit,
        fixture.transport.canonical_commitment,
    );
    fixture
        .owner
        .assert_resolved_validate_decision_identity_for_test(&terminal, current_tag, &commit);
    let message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
    );
    fixture
        .transport
        .executor
        .enqueue_network(message.clone())
        .expect("queue the real three-validator CommitQC");
    for turn in 0..24_u64 {
        let executor = &mut fixture.transport.executor;
        executor
            .step(
                now + Duration::from_millis(64 + turn),
                &mut current_services,
            )
            .expect("drive the exact retained body and Decision owners");
        executor
            .reconcile_pending_runner_decision_cleanup(&mut current_services)
            .expect("restore the actual durable Decision protection");
        // There is no local proposal lease or lane adapter in this fixture.
        executor
            .acknowledge_runner_decision_cleanup(executor.current_tag(), Some(key.1))
            .expect("acknowledge the empty process-local Decision handoff");
        let _settlement = executor
            .settle_pending_lifecycle_output_admissions(&mut fixture.owner, &mut current_services)
            .expect("settle unrelated exact TC/QC output ownership first");
        executor
            .settle_pending_durable_validate_admissions(&mut fixture.owner, &mut current_services)
            .expect("settle real registry admission before the cached result publication");
        let ledger_before_publication =
            std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
                .expect("read the complete ledger immediately before Apply publication");
        let result = executor.settle_pending_released_validate_apply_publication(
            &mut fixture.owner,
            &mut current_services,
        );
        if corrupt_terminal && result.is_err() {
            assert!(
                matches!(&result, Err(EffectExecutorError::Contract(reason))
                if reason == "terminal Validate replay changed its authenticated outcome or current authority"),
                "the corruption must be rejected at the terminal-proof publication boundary: {result:?}"
            );
            assert!(fixture.transport.executor.output_guard.restart_required());
            assert!(fixture.owner.apply_ordinals_for_retry_test().is_empty());
            assert_eq!(
                std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
                    .expect("ledger after rejected terminal proof"),
                ledger_before_publication
            );
            fixture
                .owner
                .substitute_resolved_validate_digest_for_test(&terminal, false);
            fixture
                .owner
                .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
            assert_eq!(
                fixture.planner_io.lifecycle_validate_io_snapshot(),
                io_before_commit
            );
            fixture.planner_io.detach(&mut fixture.services);
            return;
        }
        result.expect("the actual terminal result must reach one durable continuation");
        if !fixture.owner.apply_ordinals_for_retry_test().is_empty()
            || (prepare_first && !current_services.apply_tasks.is_empty())
        {
            break;
        }
    }
    assert!(
        !corrupt_terminal,
        "a different physical outcome digest must fail before any Apply publication"
    );
    assert_eq!(
        fixture.planner_io.lifecycle_validate_io_snapshot(),
        io_before_commit,
        "a current Commit must reuse the exact terminal result without physical revalidation"
    );
    let expected_typed_applies = usize::from(!prepare_first);
    let applies = fixture.owner.apply_ordinals_for_retry_test();
    assert_eq!(applies.len(), expected_typed_applies);
    if prepare_first {
        // The repaired body is already Validated. The real reducer emits a
        // direct ordinary Apply with its current Decision owner, so no new
        // Validate callback or typed Validate successor is appropriate.
        assert_eq!(current_services.apply_tasks.len(), 1);
        let task = &current_services.apply_tasks[0];
        let effect = AdapterEffect::Apply {
            tag: current_tag,
            subject: key.1,
            certificate: commit.clone(),
        };
        let pending = fixture
            .transport
            .executor
            .pending_applications
            .get(&task.id())
            .expect("the real ordinary Apply retains its current owner");
        assert_eq!(task.authorized_owner_tag(), current_tag);
        assert_eq!(task.tag(), current_tag);
        assert_eq!(task.subject(), key.1);
        assert_eq!(task.certificate(), &commit);
        assert_ne!(task.lifecycle_ordinal(), validate_ordinal);
        assert_eq!(task.lifecycle_ordinal(), pending.task.lifecycle_ordinal());
        assert!(pending.ownership.exactly_binds_adapter_effect(&effect));
        assert!(pending.ownership.binds_durable_decision_authority(
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        ));
        assert_eq!(
            Some(task.validated_receipt()),
            fixture.transport.executor.validated_bodies.get(&key)
        );
        assert_eq!(fixture.transport.executor.pending_applications.len(), 1);
        assert!(
            fixture
                .transport
                .executor
                .live_lifecycle_decision_apply
                .is_none()
        );
    } else {
        assert_ne!(applies[0], validate_ordinal);
        assert!(
            current_services.apply_tasks.is_empty(),
            "cached Validate replay must publish through its real lifecycle owner"
        );
    }
    fixture
        .owner
        .assert_resolved_validate_owner_retained_for_test(
            &terminal,
            &ledger_root,
            expected_typed_applies,
        );
    assert!(
        fixture
            .transport
            .executor
            .pending_durable_validate_admissions
            .is_empty()
    );
    let ledger_before_retry = std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
        .expect("published ledger bytes");
    fixture
        .transport
        .executor
        .enqueue_network(message)
        .expect("queue exact duplicate Commit");
    fixture
        .transport
        .executor
        .step(now + Duration::from_millis(104), &mut current_services)
        .expect("duplicate Commit remains bounded");
    assert_eq!(
        fixture
            .transport
            .executor
            .settle_pending_released_validate_apply_publication(
                &mut fixture.owner,
                &mut current_services
            )
            .expect("duplicate Apply publication is inert"),
        0
    );
    fixture
        .owner
        .assert_resolved_validate_owner_retained_for_test(
            &terminal,
            &ledger_root,
            expected_typed_applies,
        );
    assert_eq!(
        current_services.apply_tasks.len(),
        usize::from(prepare_first)
    );
    assert_eq!(
        fixture.planner_io.lifecycle_validate_io_snapshot(),
        io_before_commit
    );
    assert_eq!(
        std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
            .expect("ledger after duplicate"),
        ledger_before_retry
    );
    assert!(!fixture.transport.executor.output_guard.restart_required());
    assert!(!fixture.transport.executor.status().fail_closed);
    assert!(current_services.closed.is_empty());
    if expected_typed_applies == 1 {
        drop(_leader_wire_gate.take());
        assert_released_apply_owner_cold_reopens(fixture, &terminal);
    } else {
        fixture.planner_io.detach(&mut fixture.services);
    }
}

#[test]
fn resolved_live_validate_retained_terminal_publishes_one_current_commit_apply() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-live-validate-terminal")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Live,
                false,
                false,
                false,
            )
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn resolved_recovered_validate_retained_terminal_publishes_one_current_commit_apply() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-recovered-validate-terminal")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Recovered,
                false,
                false,
                false,
            )
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn resolved_validate_retained_terminal_rejects_changed_outcome_digest_before_apply() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-validate-terminal-digest")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Live,
                true,
                false,
                false,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Recovered,
                true,
                false,
                false,
            );
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn resolved_validate_historical_prepare_repair_then_same_tag_commit_publishes_once() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-validate-prepare-commit")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Live,
                false,
                true,
                false,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Recovered,
                false,
                true,
                false,
            );
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn physical_validate_busy_retains_exact_result_until_timeout_quorum_then_commit() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-validate-busy-commit")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Live,
                false,
                false,
                true,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Recovered,
                false,
                false,
                true,
            );
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn resolved_rejected_validate_replays_exact_report_fixture(
    origin: ValidateRetryOriginForTest,
    busy: bool,
    cold_after_report: bool,
) {
    let (
        mut fixture,
        validate_ordinal,
        pending_digest,
        mut _leader_wire_gate,
        mut current_services,
        mut now,
    ) = physically_resolved_validate_retry_fixture(origin, true, busy);
    let key = (fixture.transport.round, fixture.transport.subject);
    let ledger_root = fixture._owner_directory.path().join("ledger");
    let mut terminal = fixture.owner.resolved_validate_owner_snapshot_for_test(
        validate_ordinal,
        pending_digest,
        &ledger_root,
    );
    if origin == ValidateRetryOriginForTest::ColdTerminal {
        let (reopened, gate) =
            reopen_body_owner_fixture(fixture, FixtureValidationReplay::Rejected);
        fixture = reopened;
        _leader_wire_gate = Some(gate);
        terminal = fixture
            .owner
            .resolved_validate_cold_snapshot_for_test(&terminal, &ledger_root);
        current_services = FakeServices::default();
        now = Instant::now();
        fixture
            .transport
            .executor
            .arm_live_clocks(
                ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                now,
            )
            .expect("activate the ordinary runtime after authenticated terminal recovery");
        // Cold startup has no process-local TC output left to retry the body.
        // Drive the actual periodic occurrence at its configured deadline.
        now += fixture.transport.executor.runtime.retransmit_interval();
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .command_depth(),
            0
        );
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .physical_admissions(),
            0
        );
        assert_eq!(
            fixture
                .transport
                .executor
                .validate_retry_lifecycle_ordinal_for_test(key),
            Some(None)
        );
    }
    assert!(
        !fixture
            .transport
            .executor
            .validated_bodies
            .contains_key(&key)
    );
    let io_before = fixture.planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(io_before.command_depth(), 0);
    assert_eq!(io_before.active(), 0);
    let mut settled = 0;
    for turn in 0..24_u64 {
        let executor = &mut fixture.transport.executor;
        executor
            .step(now + Duration::from_millis(turn), &mut current_services)
            .expect("retry the exact protected body after its rejected terminal cut");
        let settlement = executor
            .settle_pending_lifecycle_output_admissions(&mut fixture.owner, &mut current_services)
            .expect("settle current Prepare/TC output ownership");
        if settlement.requires_outer_executor_yield() {
            continue;
        }
        executor
            .settle_pending_durable_validate_admissions(&mut fixture.owner, &mut current_services)
            .expect("settle the actual registry handoff");
        settled += executor
            .settle_pending_released_validate_apply_publication(
                &mut fixture.owner,
                &mut current_services,
            )
            .expect("replay the durable deterministic rejection");
        if executor.runtime.driver().body_state_for_test(key.0, key.1)
            == crate::sumeragi::v2_core::BodyState::Invalid
        {
            break;
        }
    }
    assert_eq!(settled, 1);
    assert_eq!(
        fixture
            .transport
            .executor
            .runtime
            .driver()
            .body_state_for_test(key.0, key.1),
        crate::sumeragi::v2_core::BodyState::Invalid
    );
    assert!(
        !fixture
            .transport
            .executor
            .validated_bodies
            .contains_key(&key)
    );
    assert!(current_services.apply_tasks.is_empty());
    assert_eq!(
        fixture.planner_io.lifecycle_validate_io_snapshot(),
        io_before,
        "cached rejection cannot run another validation callback or acquire worker capacity"
    );
    fixture
        .owner
        .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
    let reports = fixture.owner.invalid_body_report_ordinals_for_retry_test();
    assert_eq!(
        reports.len(),
        1,
        "the actual registry must own the deterministic invalid-body report"
    );
    let ledger_before = std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
        .expect("durable rejected result and report");
    assert_eq!(
        fixture
            .transport
            .executor
            .settle_pending_released_validate_apply_publication(
                &mut fixture.owner,
                &mut current_services
            )
            .expect("a repeated cached rejection settlement is inert"),
        0
    );
    assert_eq!(
        fixture.owner.invalid_body_report_ordinals_for_retry_test(),
        reports
    );
    assert_eq!(
        std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
            .expect("ledger after repeated report publication"),
        ledger_before
    );
    // A later real TC clears the reducer's Invalid state but retains the exact
    // protected PrepareQC. Its ordinary retransmit must replay the cached result
    // into Invalid again while reusing the already durable Report owner.
    let original_report = fixture
        .owner
        .invalid_body_report_snapshot_for_retry_test(reports[0], &ledger_root);
    let reported_tag = fixture.transport.executor.current_tag();
    now += Duration::from_millis(100);
    install_timeout(&mut fixture, true, &mut current_services, now);
    let mut repeated_settled = 0;
    for turn in 0..48_u64 {
        let executor = &mut fixture.transport.executor;
        executor
            .step(now + Duration::from_millis(turn), &mut current_services)
            .expect("consume the later TC and normally retransmit its protected body");
        let settlement = executor
            .settle_pending_lifecycle_output_admissions(&mut fixture.owner, &mut current_services)
            .expect("settle only the actual later timeout output");
        if settlement.requires_outer_executor_yield() {
            continue;
        }
        executor
            .settle_pending_durable_validate_admissions(&mut fixture.owner, &mut current_services)
            .expect("retain the current protected validation occurrence");
        let before_reuse = std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
            .expect("ledger before current rejection replay");
        repeated_settled += executor
            .settle_pending_released_validate_apply_publication(
                &mut fixture.owner,
                &mut current_services,
            )
            .expect("the same rejected terminal reuses its original Report after EnterView");
        assert_eq!(
            std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
                .expect("ledger after current rejection replay"),
            before_reuse,
            "the current reducer occurrence cannot append or rewrite its existing Report",
        );
        if executor.current_tag().strictly_advances(reported_tag)
            && executor.runtime.driver().body_state_for_test(key.0, key.1)
                == crate::sumeragi::v2_core::BodyState::Invalid
        {
            break;
        }
    }
    assert!(
        fixture
            .transport
            .executor
            .current_tag()
            .strictly_advances(reported_tag)
    );
    assert_eq!(
        repeated_settled, 1,
        "one actual current rejected-result replay"
    );
    assert_eq!(
        fixture.owner.invalid_body_report_ordinals_for_retry_test(),
        reports
    );
    fixture
        .owner
        .assert_invalid_body_report_retained_for_retry_test(&original_report, &ledger_root);
    fixture
        .owner
        .assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
    assert_eq!(
        fixture.planner_io.lifecycle_validate_io_snapshot(),
        io_before
    );
    assert!(current_services.apply_tasks.is_empty());
    if cold_after_report {
        let report = fixture
            .owner
            .invalid_body_report_snapshot_for_retry_test(reports[0], &ledger_root);
        drop(_leader_wire_gate.take());
        let (reopened, gate) =
            reopen_body_owner_fixture(fixture, FixtureValidationReplay::Rejected);
        fixture = reopened;
        _leader_wire_gate = Some(gate);
        fixture
            .owner
            .resolved_validate_cold_snapshot_for_test(&terminal, &ledger_root);
        fixture
            .owner
            .assert_invalid_body_report_recovered_for_retry_test(&report, &ledger_root);
        assert_eq!(
            fixture.owner.invalid_body_report_ordinals_for_retry_test(),
            reports,
            "recovery must retain the sole report rather than admit a duplicate"
        );
        assert!(fixture.owner.apply_ordinals_for_retry_test().is_empty());
        let recovered_io = fixture.planner_io.lifecycle_validate_io_snapshot();
        assert_eq!(recovered_io.command_depth(), 0);
        assert_eq!(recovered_io.active(), 0);
        assert_eq!(recovered_io.physical_admissions(), 0);
        assert!(
            !fixture
                .transport
                .executor
                .validated_bodies
                .contains_key(&key)
        );
        let ledger_after_reopen = std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
            .expect("reopened terminal and report ledger");
        let mut recovered_services = FakeServices::default();
        assert_eq!(
            fixture
                .transport
                .executor
                .settle_pending_released_validate_apply_publication(
                    &mut fixture.owner,
                    &mut recovered_services,
                )
                .expect("recovery cannot replay another callback or duplicate report admission"),
            0
        );
        assert!(recovered_services.apply_tasks.is_empty());
        assert_eq!(
            fixture.planner_io.lifecycle_validate_io_snapshot(),
            recovered_io
        );
        assert_eq!(
            std::fs::read(ledger_root.join("lifecycle-ledger-v1.norito"))
                .expect("ledger after inert recovered replay settlement"),
            ledger_after_reopen
        );
        fixture
            .owner
            .assert_invalid_body_report_recovered_for_retry_test(&report, &ledger_root);
    }
    assert!(!fixture.transport.executor.output_guard.restart_required());
    assert!(!fixture.transport.executor.status().fail_closed);
    fixture.planner_io.detach(&mut fixture.services);
}

#[test]
fn resolved_rejected_validate_replays_report_once_without_revalidation_or_apply() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-validate-rejected-report")
        .spawn(|| {
            for origin in [
                ValidateRetryOriginForTest::Live,
                ValidateRetryOriginForTest::Recovered,
                ValidateRetryOriginForTest::Published,
            ] {
                for busy in [false, true] {
                    resolved_rejected_validate_replays_exact_report_fixture(origin, busy, false);
                }
            }
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn resolved_published_validate_retained_terminal_publishes_one_current_commit_apply() {
    let result = crate::sumeragi::sumeragi_thread_builder("resolved-published-validate-terminal")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Published,
                false,
                false,
                false,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Published,
                false,
                true,
                false,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Published,
                false,
                false,
                true,
            );
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::Published,
                true,
                false,
                false,
            );
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn already_terminal_validate_cold_reopen_preserves_success_and_rejection() {
    let result = crate::sumeragi::sumeragi_thread_builder("terminal-validate-cold-reopen")
        .spawn(|| {
            resolved_validate_owner_retries_commit_fixture(
                ValidateRetryOriginForTest::ColdTerminal,
                false,
                false,
                false,
            );
            resolved_rejected_validate_replays_exact_report_fixture(
                ValidateRetryOriginForTest::ColdTerminal,
                false,
                false,
            );
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn resolved_validate_survives_unprotected_view_until_current_commit() {
    let result = crate::sumeragi::sumeragi_thread_builder("terminal-validate-unprotected-view")
        .spawn(|| {
            for origin in [
                ValidateRetryOriginForTest::UnprotectedTerminal,
                ValidateRetryOriginForTest::UnprotectedPublishedTerminal,
            ] {
                resolved_validate_owner_retries_commit_fixture(origin, false, false, false);
            }
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn rejected_terminal_and_published_report_cold_reopen_preserves_one_output_owner() {
    let result = crate::sumeragi::sumeragi_thread_builder("terminal-report-cold-reopen")
        .spawn(|| {
            for origin in [
                ValidateRetryOriginForTest::Live,
                ValidateRetryOriginForTest::ColdTerminal,
            ] {
                resolved_rejected_validate_replays_exact_report_fixture(origin, false, true);
            }
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

include!("v2_effects_active_prepare_decision_cold_cases.rs");
