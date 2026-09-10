// Crash after the real Commit WAL append, before any current Decision effect
// can replace or settle an already durable Prepare body owner.
fn active_prepare_body_survives_decision_crash_fixture(
    cut: LifecycleWorkClass,
    timeout_before_commit: bool,
) {
    let mut fixture = ready_body_fixture();
    let original_tag = fixture.transport.executor.current_tag();
    assert_eq!(original_tag.generation(), Generation::INITIAL);
    let mut ordinal = fixture.ordinal;
    for next in [LifecycleWorkClass::Store, LifecycleWorkClass::Validate] {
        if cut == LifecycleWorkClass::Fetch
            || (cut == LifecycleWorkClass::Store && next == LifecycleWorkClass::Validate)
        {
            break;
        }
        let advanced = fixture
            .owner
            .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
            .expect("advance the real Ready Prepare owner to the requested crash stage");
        let ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child_ordinal,
            child,
        } = advanced
        else {
            panic!("the exact existing body owner must advance normally")
        };
        assert_eq!(parent_ordinal, ordinal);
        assert_eq!(child, next);
        ordinal = child_ordinal;
    }
    let snapshot = fixture
        .owner
        .active_body_owner_before_decision_cold_for_test(ordinal, cut);
    let ledger_path = fixture
        ._owner_directory
        .path()
        .join("ledger/lifecycle-ledger-v1.norito");
    let ledger_before = std::fs::read(&ledger_path).expect("real durable Prepare body ledger");
    let wal_path = fixture
        .transport
        ._directory
        .path()
        .join("transport-regression-safety.wal");
    let wal_before = std::fs::read(&wal_path).expect("actual pre-Decision safety WAL");
    if timeout_before_commit {
        assert_eq!(cut, LifecycleWorkClass::Validate);
        let before_tag = fixture.transport.executor.current_tag();
        let timeout = signed_timeout_certificate(&fixture, true);
        let driver = fixture.transport.executor.runtime.driver_mut_for_test();
        let authenticated = driver
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ))
            .expect("authenticate the actual current timeout quorum");
        let pending_timeout = driver
            .receive_authenticated(authenticated)
            .expect("fsync EnterView before the retained Validate is serviced");
        let durable_tag = driver.current_tag();
        assert!(
            durable_tag.strictly_advances(before_tag),
            "the fsynced timeout must advance the actual adapter owner"
        );
        assert_eq!(
            fixture.transport.executor.current_tag(),
            before_tag,
            "the crash precedes dispatching EnterView into the runtime clocks"
        );
        assert_eq!(
            std::fs::read(&ledger_path).expect("ledger at the timeout crash cut"),
            ledger_before,
            "the old physical validation owner must still exist at this crash cut",
        );
        assert_eq!(
            fixture
                .planner_io
                .lifecycle_validate_io_snapshot()
                .physical_admissions(),
            0,
            "the old Validate has no physical result at the TC crash cut",
        );
        fixture.planner_io.assert_owned_durable_body_for_test(
            &fixture.transport.executor.durable_bodies
                [&(fixture.transport.round, fixture.transport.subject)],
            &fixture.transport.body,
        );
        drop(pending_timeout);
    }
    let commit = fixture.transport.quorum_certificate(
        wire::GlobalPhase::Commit,
        fixture.transport.canonical_commitment,
    );
    let expected_decision = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let driver = fixture.transport.executor.runtime.driver_mut_for_test();
    let authenticated = driver
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(commit),
        ))
        .expect("authenticate the actual matching Commit quorum");
    let pending_decision = driver
        .receive_authenticated(authenticated)
        .expect("fsync the actual Commit Decision before the crash");
    assert_eq!(
        fixture
            .transport
            .executor
            .runtime
            .decided_body()
            .expect("read durable Decision"),
        expected_decision
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("ledger at the WAL crash cut"),
        ledger_before
    );
    let wal_after = std::fs::read(&wal_path).expect("actual fsynced Decision safety WAL");
    assert!(
        wal_after.starts_with(&wal_before) && wal_after.len() > wal_before.len(),
        "the test must crash after a real append, not a synthesized in-memory Decision"
    );
    // Losing process-local output at a crash must not erase the older durable
    // executable owner; cold startup authenticates both durable stores itself.
    drop(pending_decision);
    if timeout_before_commit {
        recover_stale_prepare_decision_crash_fixture(fixture, snapshot, ordinal, expected_decision);
        return;
    }
    let (mut fixture, _gate) =
        reopen_body_owner_fixture(fixture, FixtureValidationReplay::NoMarkers);
    assert_eq!(
        fixture
            .transport
            .executor
            .runtime
            .decided_body()
            .expect("recover same Decision"),
        expected_decision
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("ledger after owner-preserving cold open"),
        ledger_before
    );
    assert_eq!(fixture.transport.executor.current_tag(), original_tag);
    fixture
        .owner
        .assert_active_body_owner_after_decision_cold_for_test(&snapshot, cut);
    assert_eq!(
        fixture
            .planner_io
            .lifecycle_validate_io_snapshot()
            .physical_admissions(),
        0
    );
    assert!(fixture.owner.apply_ordinals_for_retry_test().is_empty());
    let now = Instant::now();
    fixture
        .transport
        .executor
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            now,
        )
        .expect("activate the actual recovered runtime");
    assert!(fixture.transport.executor.protected_decision.is_none());
    assert!(
        fixture
            .transport
            .executor
            .pending_runner_decision_cleanup
            .is_none()
    );
    let tag = fixture.transport.executor.current_tag();
    let local = usize::try_from(
        fixture
            .transport
            .executor
            .local_validator
            .expect("same local validator"),
    )
    .expect("local index");
    crate::sumeragi::v2_worker::tests::install_active_tag_for_test(&mut fixture.services, tag);
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
        &mut fixture.services,
        &fixture.transport.validator_keys[local],
    );
    for next in [LifecycleWorkClass::Store, LifecycleWorkClass::Validate] {
        if cut == LifecycleWorkClass::Validate
            || (cut == LifecycleWorkClass::Store && next == LifecycleWorkClass::Store)
        {
            continue;
        }
        let advanced = fixture
            .owner
            .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
            .unwrap_or_else(|error| panic!(
                "continue the preserved {cut:?} crash owner at ordinal {ordinal} to {next:?} under actual Commit: {error:?}"
            ));
        let ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child_ordinal,
            child,
        } = advanced
        else {
            panic!("cold body recovery must publish the next real owner: {advanced:?}")
        };
        assert_eq!(parent_ordinal, ordinal);
        assert_eq!(child, next);
        ordinal = child_ordinal;
    }
    finish_current_decision_validate_and_reopen(fixture, ordinal);
}

fn finish_current_decision_validate_and_reopen(mut fixture: ReadyBodyFixture, ordinal: u128) {
    assert_eq!(fixture.owner.dispatch_completion_for_test(
        &mut fixture.services, &mut fixture.transport.executor, 0,
    ).expect("queue the single real recovered Validate"),
        ProductionCompletionDispatchV1::ValidateQueued { ordinal });
    fixture.planner_io.activate_one_lifecycle_validate();
    assert_eq!(
        fixture.planner_io.execute_held_lifecycle_validate_fixture(
            fixture.transport.canonical_commitment,
            Arc::clone(&fixture.transport.executor.output_guard),
        ),
        1,
        "execute the original body exactly once after recovery"
    );
    let completion = match fixture
        .services
        .take_next_lifecycle_completion()
        .expect("the actual recovered physical result")
    {
        LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("the only worker completion must be Validate"),
    };
    let successor = fixture
        .owner
        .publish_validate_successor_for_retry_test(completion, ordinal, false);
    let published = fixture
        .owner
        .dispatch_ready_validate_successor_for_test(
            &mut fixture.services,
            &mut fixture.transport.executor,
            successor,
            0,
        )
        .expect("publish the actual current Decision Apply from the completed old body owner");
    let crate::sumeragi::v2_lifecycle_coordinator::ReadyValidateSuccessorDispatchV1::Resolved(
        ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child_ordinal,
            child: LifecycleWorkClass::Apply,
        },
    ) = published
    else {
        panic!("a current Commit must own the linked Apply after real validation")
    };
    assert_eq!(parent_ordinal, ordinal);
    assert_eq!(
        fixture.owner.apply_ordinals_for_retry_test(),
        vec![child_ordinal]
    );
    assert_active_prepare_linked_apply_cold_reopens(fixture, child_ordinal);
}

// Reopen at two real crash boundaries before executing the fresh Decision's
// signed-response persistence, then use the ordinary physical Validate tail.
fn recover_stale_prepare_decision_crash_fixture(
    fixture: ReadyBodyFixture,
    previous: crate::sumeragi::v2_lifecycle_coordinator::BodyOwnerSnapshotForTest,
    previous_ordinal: u128,
    expected_decision: Option<(
        wire::ConsensusRound,
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
) {
    use crate::sumeragi::v2_lifecycle_coordinator::{
        LaunchedProductionLifecycleV1, ProductionLifecycleIngressSelectionV1,
        ProductionLifecycleIngressTurnV1,
    };
    use crate::sumeragi::v2_runner::{
        LifecycleRunnerRankTarget, with_lifecycle_current_runner_turn_for_test,
    };

    let ReadyBodyFixture {
        mut transport,
        owner,
        planner_io,
        mut services,
        _owner_directory: directory,
        certificate,
        ..
    } = fixture;
    let validator = transport
        .executor
        .local_validator
        .expect("same local validator");
    let local = usize::try_from(validator).expect("frozen local index");
    let context = transport.context.clone();
    let proofs = transport
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen validator PoP")
        })
        .collect::<Vec<_>>();
    let wal_path = transport
        ._directory
        .path()
        .join("transport-regression-safety.wal");
    let ledger_path = directory.path().join("ledger/lifecycle-ledger-v1.norito");
    planner_io.detach(&mut services);
    drop(services);
    drop(owner);
    drop(transport.executor);
    let reopen = || {
        SumeragiV2Adapter::reopen_cancelled_body_owner_for_test(
            &wal_path,
            directory.path(),
            VerifiedHeightContext::genesis(context.clone(), proofs.clone())
                .expect("verify the exact cold four-validator context"),
            validator,
            &transport.validator_keys[local],
            AdapterFingerprints {
                node: Hash::new(b"production transport node"),
                build: Hash::new(b"production transport build"),
                config: Hash::new(b"production transport config"),
            },
            [0x63; 32],
            |_| panic!("a Ready Validate has no successful marker to fabricate during recovery"),
        )
    };
    let mut first = reopen();
    let fetch = first.assert_stale_body_owner_retired_for_test(&previous, directory.path());
    assert_eq!(
        first.recovered_decision_fetch_row_summary_for_test(),
        Some((fetch, fetch))
    );
    let current =
        first.active_body_owner_before_decision_cold_for_test(fetch, LifecycleWorkClass::Fetch);
    let after_retirement =
        std::fs::read(&ledger_path).expect("published cancellation and fresh Decision Fetch");
    drop(first);
    let owner = reopen();
    owner
        .assert_active_body_owner_after_decision_cold_for_test(&current, LifecycleWorkClass::Fetch);
    assert_eq!(
        owner.assert_stale_body_owner_retired_for_test(&previous, directory.path()),
        fetch
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("second cold Fetch ledger"),
        after_retirement,
        "restarting after retirement must neither recreate the old owner nor append another Decision owner"
    );
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let source_bytes = iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let ordinary_bytes = iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES
        .get()
        .checked_add(crate::sumeragi::BODY_ENVELOPE_HEADROOM_BYTES)
        .expect("default ordinary response partition fits usize");
    let completion_bytes = source_bytes
        .checked_sub(crate::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES)
        .and_then(|bytes| bytes.checked_sub(crate::sumeragi::TIMEOUT_VOTE_RESERVE_BYTES))
        .and_then(|bytes| bytes.checked_sub(ordinary_bytes))
        .expect("default response source partitions are disjoint");
    let global_plaintext = iroha_p2p::frame_plaintext_cap(
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
    );
    let ingress = Arc::new(
        crate::sumeragi::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            32,
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get(),
            source_bytes,
            crate::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            crate::sumeragi::TIMEOUT_VOTE_RESERVE_BYTES,
            completion_bytes,
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get()),
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES
                .get(),
            None,
        ),
    );
    ingress
        .configure_roster_for_context(
            context.roster.iter().map(|entry| entry.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .expect("bind the complete four-validator response ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    crate::sumeragi::v2_worker::tests::install_lifecycle_ingress_for_test(
        &mut services,
        Arc::clone(&ingress),
    );
    let output_guard = ConsensusOutputGuard::isolated();
    let now = Instant::now();
    let (mut launched, mut planner_io) =
        LaunchedProductionLifecycleV1::recovered_decision_services_for_restart_test(
            Box::new(owner),
            Box::new(services),
            &wal_path,
            now,
            validator,
            Arc::clone(&output_guard),
            Arc::clone(&ingress),
        );
    let request_hash =
        launched.with_proposal_restart_fixture_for_test(|owner, executor, services| {
            assert_eq!(
                executor
                    .runtime
                    .decided_body()
                    .expect("actual cold Decision"),
                expected_decision
            );
            executor
                .arm_live_clocks(
                    ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                    now,
                )
                .expect("activate exact recovered Decision runtime");
            assert!(executor.protected_decision.is_none());
            assert!(executor.pending_runner_decision_cleanup.is_none());
            crate::sumeragi::v2_worker::tests::install_active_tag_for_test(
                services,
                executor.current_tag(),
            );
            crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
                services,
                &transport.validator_keys[local],
            );
            services
                .set_exact_output_shared_unit_capacity_for_test(64)
                .expect("bind exact request fanout to the restored four-validator service context");
            assert_eq!(
                owner
                    .dispatch_completion_for_test(services, executor, 0)
                    .expect("dispatch actual signed current Decision Fetch"),
                ProductionCompletionDispatchV1::FetchDispatched { ordinal: fetch }
            );
            let (key, request_hash) = executor
                .recovered_decision_fetch_owner_for_test()
                .expect("retain the exact canonical Fetch request");
            assert_eq!(key.lifecycle_ordinal(), fetch);
            request_hash
        });
    let mut response = wire::CertifiedBodyResponse {
        request_hash,
        manifest: transport.manifest.clone(),
        body: transport.body.clone(),
        responder: context.roster[0].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        transport.validator_keys[0].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
            )),
            context.roster[0].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    with_lifecycle_current_runner_turn_for_test(
        &context,
        LifecycleRunnerRankTarget::Ingress,
        |runner| {
            assert!(matches!(
                launched.drive_ingress_turn(runner),
                ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued,
                )
            ));
        },
    );
    let queued_fetch = planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(
        queued_fetch.physical_admissions(),
        1,
        "the real response persistence owns one physical slot"
    );
    assert_eq!(queued_fetch.command_depth(), 1);
    assert_eq!(
        queued_fetch.queued(),
        0,
        "no validation has been submitted before response persistence"
    );
    assert_eq!(queued_fetch.active(), 0);
    assert_eq!(queued_fetch.completion_pending(), 0);
    planner_io.execute_one_recovered_decision_fetch_for_test(Arc::clone(&output_guard));
    launched.settle_decision_fetch_worker_for_test();
    assert_eq!(
        ingress.len(),
        0,
        "the actual response occurrence was durably consumed"
    );
    let (owner, executor, services) = (*launched).into_settled_body_fixture_for_test();
    transport.executor = executor;
    let mut fixture = ReadyBodyFixture {
        transport,
        owner,
        planner_io: *planner_io,
        services,
        _owner_directory: directory,
        certificate,
        ordinal: fetch,
    };
    let ProductionCompletionDispatchV1::BodyStageAdvanced {
        parent_ordinal: store,
        child_ordinal: validate,
        child: LifecycleWorkClass::Validate,
    } = fixture
        .owner
        .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
        .expect("the real persisted current response Store publishes Validate")
    else {
        panic!("actual current Decision Store must advance to one physical Validate")
    };
    assert!(store > fetch && validate > store);
    fixture
        .owner
        .body_recovery_snapshot_for_test(previous_ordinal, validate);
    fixture.planner_io.assert_owned_durable_body_for_test(
        &fixture.transport.executor.durable_bodies
            [&(fixture.transport.round, fixture.transport.subject)],
        &fixture.transport.body,
    );
    finish_current_decision_validate_and_reopen(fixture, validate);
}

// The second crash exercises the real linked Apply producer/consumer join,
// not only initial Ready-body admission under a recovered Decision.
fn assert_active_prepare_linked_apply_cold_reopens(fixture: ReadyBodyFixture, apply: u128) {
    let snapshot = fixture
        .owner
        .active_body_owner_before_decision_cold_for_test(apply, LifecycleWorkClass::Apply);
    let ReadyBodyFixture {
        transport,
        owner,
        planner_io,
        mut services,
        _owner_directory: directory,
        ..
    } = fixture;
    let ledger_path = directory.path().join("ledger/lifecycle-ledger-v1.norito");
    let before = std::fs::read(&ledger_path).expect("real linked Apply ledger");
    let validator = transport
        .executor
        .local_validator
        .expect("same local validator");
    let verified = VerifiedHeightContext::genesis(
        transport.context.clone(),
        transport
            .validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen validator PoP")
            })
            .collect(),
    )
    .expect("authenticate unchanged cold context");
    let wal_path = transport
        ._directory
        .path()
        .join("transport-regression-safety.wal");
    planner_io.detach(&mut services);
    drop(services);
    drop(owner);
    drop(transport.executor);
    let mut revalidated = 0;
    let mut reopened = SumeragiV2Adapter::reopen_cancelled_body_owner_for_test(
        &wal_path,
        directory.path(),
        verified,
        validator,
        &transport.validator_keys[usize::try_from(validator).expect("validator index")],
        AdapterFingerprints {
            node: Hash::new(b"production transport node"),
            build: Hash::new(b"production transport build"),
            config: Hash::new(b"production transport config"),
        },
        [0x63; 32],
        |body| {
            assert_eq!(
                body.encode_wire().expect("canonical recovered body"),
                transport.body
            );
            revalidated += 1;
            Ok(transport.canonical_commitment)
        },
    );
    assert_eq!(
        revalidated, 1,
        "semantically replay the single successful marker"
    );
    assert!(reopened.exact_recovered_body_pipeline_join_for_test());
    reopened.assert_active_body_owner_after_decision_cold_for_test(
        &snapshot,
        LifecycleWorkClass::Apply,
    );
    assert_eq!(reopened.apply_ordinals_for_retry_test(), vec![apply]);
    assert_eq!(
        std::fs::read(&ledger_path).expect("linked Apply ledger after second recovery"),
        before
    );
}

#[test]
fn active_prepare_body_owners_cold_reopen_under_durable_commit() {
    let result = crate::sumeragi::sumeragi_thread_builder("active-prepare-decision-cold")
        .spawn(|| {
            for cut in [
                LifecycleWorkClass::Fetch,
                LifecycleWorkClass::Store,
                LifecycleWorkClass::Validate,
            ] {
                active_prepare_body_survives_decision_crash_fixture(cut, false);
            }
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn active_prepare_validate_cold_reopen_after_timeout_and_durable_commit() {
    let result = crate::sumeragi::sumeragi_thread_builder("active-prepare-tc-decision-cold")
        .spawn(|| {
            active_prepare_body_survives_decision_crash_fixture(LifecycleWorkClass::Validate, true);
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}
