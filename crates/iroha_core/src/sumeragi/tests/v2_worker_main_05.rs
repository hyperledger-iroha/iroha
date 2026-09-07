#[test]
fn finalized_cleanup_without_exact_output_seal_latches_restart() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    let output_guard = Arc::clone(&service.output_guard);
    service.clean_teardown = false;
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);
    assert!(output_guard.restart_required());
    assert!(
        outcome.warnings().iter().any(|warning| {
            warning.target() == PostFinalityCleanupTarget::CleanupWorker
                && warning.reason().contains("was not sealed")
        }),
        "finalized cleanup must diagnose an unsealed exact-output owner"
    );
}
#[test]
fn zero_cleanup_deadline_polls_an_already_buffered_completion() {
    let (command_tx, _command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    assert!(
        completion_tx
            .try_send(V2IoCompletion::AuxiliaryNoop)
            .is_ok(),
        "buffer cleanup completion before the zero-deadline poll"
    );
    let io = V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    };

    assert!(matches!(
        recv_cleanup_completion(&io, Instant::now()),
        Ok(V2IoCompletion::AuxiliaryNoop)
    ));
    assert!(matches!(
        recv_cleanup_completion(&io, Instant::now()),
        Err(CleanupCompletionWaitError::DeadlineElapsed)
    ));
}
#[test]
fn finalized_cleanup_without_context_worker_reports_unavailability() {
    let (service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);
    assert_eq!(outcome.warnings().len(), 1);
    assert!(outcome.warnings()[0].reason().contains("unavailable"));
}
#[test]
fn finalized_cleanup_reports_disconnected_worker_without_failing_rollover() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    drop(command_rx);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);
    assert_eq!(outcome.warnings().len(), 1);
    assert_eq!(
        outcome.warnings()[0].target(),
        PostFinalityCleanupTarget::CleanupWorker
    );
    assert!(outcome.warnings()[0].reason().contains("disconnected"));
}
#[test]
fn prelatched_finalized_cleanup_does_not_mutate_the_io_queue() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    service.output_guard.activate_restart_required();
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);
    assert!(command_rx.try_recv().is_err());
    assert_eq!(outcome.warnings().len(), 1);
    assert!(outcome.warnings()[0].reason().contains("restart"));
}
#[test]
fn finalized_cleanup_does_not_wait_for_post_retire_completion() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(2);
    let join = thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(join),
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);
    assert!(outcome.warnings().is_empty());
}
#[test]
fn finalized_cleanup_releases_rollover_after_retire_enqueue() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let (accepted_tx, accepted_rx) = mpsc::sync_channel(1);
    let join = thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
        accepted_tx
            .send(())
            .expect("announce accepted retirement request");
        // Deliberately withhold a completion. Closing the command channel
        // at the deadline must still give this worker a supervised exit.
        assert!(command_rx.recv().is_err());
        drop(completion_tx);
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(join),
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let mut supervisor = V2CleanupSupervisor::default();
    let started = Instant::now();
    let outcome = service.finish_height(receipt, Duration::from_millis(10), &mut supervisor);
    accepted_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("worker accepted the queued Retire request");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "a silent post-finality worker must not hold successor rollover"
    );
    assert!(outcome.warnings().is_empty());
}
#[test]
fn finalized_cleanup_full_queue_timeout_allows_normal_worker_disconnect() {
    let (mut service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    seal_empty_exact_output_for_cleanup_test(&service);
    let output_guard = Arc::clone(&service.output_guard);
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
    let worker_allow_finalized_disconnect = Arc::clone(&allow_finalized_disconnect);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let queued_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued cleanup block")),
        payload_hash: Hash::new(b"queued cleanup payload"),
    };
    assert!(
        command_tx
            .try_send(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(0),
                subject: queued_subject,
            })
            .is_ok(),
        "fill ordered I/O queue before Retire enqueue"
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let worker_output_guard = Arc::clone(&output_guard);
    let join = thread::spawn(move || {
        let _worker_failure_guard =
            V2IoWorkerFailureGuard::new(worker_output_guard, worker_allow_finalized_disconnect);
        release_rx
            .recv()
            .expect("release full-queue cleanup worker");
        assert!(matches!(
            command_rx.recv(),
            Ok(V2IoCommand::LoadCandidate { .. })
        ));
        assert!(command_rx.recv().is_err());
        drop(completion_tx);
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(join),
        allow_finalized_disconnect: Arc::clone(&allow_finalized_disconnect),
        admission,
    });
    let mut supervisor = V2CleanupSupervisor::default();
    let outcome = service.finish_height(receipt, Duration::from_millis(10), &mut supervisor);
    assert!(
        allow_finalized_disconnect.load(AtomicOrdering::Acquire),
        "typed-finality timeout must authorize the ensuing normal disconnect"
    );
    assert_eq!(outcome.warnings().len(), 1);
    assert!(outcome.warnings()[0].reason().contains("enqueue exceeded"));
    assert!(!output_guard.restart_required());
    release_tx.send(()).expect("release cleanup worker");
    assert!(!output_guard.restart_required());
    assert!(output_guard.acquire().is_some());
}
#[test]
fn cleanup_diagnostics_retain_height_context_and_block_hash() {
    let (service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    let identity = CleanupWorkerIdentity::from_receipt(&receipt);
    assert_eq!(identity.height, receipt.height());
    assert_eq!(identity.context_id, receipt.context_id());
    assert_eq!(identity.block_hash, receipt.block_hash());
}
fn cleanup_job_fixture(
    service: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    body_root: &Path,
) -> PostFinalityCleanupJob {
    let bodies = V2BodyStore::open(body_root, service.context.clone())
        .expect("open cleanup body fixture")
        .into_retirement_job(receipt)
        .expect("authorize exact cleanup fixture");
    PostFinalityCleanupJob {
        identity: CleanupWorkerIdentity::from_receipt(receipt),
        bodies,
    }
}
#[test]
fn cleanup_submission_is_bounded_and_never_waits_for_capacity() {
    let (service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    let first_root = TempDir::new().expect("first cleanup body root");
    let second_root = TempDir::new().expect("second cleanup body root");
    let (sender, _receiver) = mpsc::sync_channel(1);
    let submission = V2CleanupSubmission { sender };
    submission
        .try_submit(cleanup_job_fixture(&service, &receipt, first_root.path()))
        .expect("first cleanup fills the bounded queue");
    let started = Instant::now();
    let error = submission
        .try_submit(cleanup_job_fixture(&service, &receipt, second_root.path()))
        .expect_err("second cleanup cannot exceed queue capacity");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(error.contains("queue is full"));
}
#[test]
fn cleanup_worker_job_removes_bodies_off_the_consensus_path() {
    let (service, keys) = fixture();
    let receipt = durable_receipt(&service, &keys);
    let root = TempDir::new().expect("cleanup execution root");
    let job = cleanup_job_fixture(&service, &receipt, root.path());
    let context_directory = root
        .path()
        .join(hex::encode(service.context.id().0.as_ref()));
    assert!(context_directory.is_dir());
    execute_post_finality_cleanup(job);
    assert!(!context_directory.exists());
}
fn merge_sidecar_reference(label: &[u8]) -> CertifiedMergeLedgerReference {
    CertifiedMergeLedgerReference {
        version: 1,
        entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(label)),
        encoded_len: 512,
        epoch_id: 9,
        execution_batch_hash: None,
        entrypoint_count: None,
        entrypoint_merkle_root: None,
        result_merkle_root: None,
        base_state_height: None,
        base_state_hash: None,
        merge_qc: MergeQuorumCertificate::new(
            2,
            9,
            1,
            HashOf::from_untyped_unchecked(Hash::new(b"merge parent")),
            crate::sumeragi::synthetic_network_id("v2-worker-merge-sidecar"),
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Hash::new(b"merge certificate message"),
        ),
    }
}
fn chunk(
    manifest_hash: HashOf<wire::PayloadManifest>,
    index: u32,
    bytes: &[u8],
    sender: wire::ValidatorIndex,
) -> wire::PayloadChunk {
    wire::PayloadChunk {
        manifest_hash,
        index,
        bytes: bytes.to_vec(),
        sender,
        signature: vec![0xA5],
    }
}
fn fair_ingress_route_owner(
    message: BlockMessage,
    semantic_origin: PeerId,
    authenticated_via: PeerId,
    route: NetworkReplyRoute,
) -> (NetworkReplyRoutes, FairV2IngressOwnershipEvidence) {
    let roster = vec![semantic_origin.clone()];
    let mut admitted = fair_v2_ingress_admit_with_roster_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            message,
            semantic_origin,
            authenticated_via,
            route,
        )
        .expect("test route binds fair-ingress ownership"),
        roster,
    );
    let ownership = admitted
        .take_ingress_ownership()
        .expect("fair ingress attaches exact ownership");
    let (_, _, routes) = admitted.into_message_sender_and_reply_routes();
    (
        routes.expect("authenticated test ingress retains its reply route"),
        ownership,
    )
}
#[test]
fn exact_output_coalescing_preserves_distinct_fair_ingress_admissions() {
    let (service, _) = fixture();
    let requester = service.context.roster[1].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(requester.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(requester.clone(), hub_b.clone());
    let request = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(chunk(
            manifest_hash(b"fair output request"),
            0,
            b"owned",
            0,
        )),
    ));
    let (routes_a, ownership_a) =
        fair_ingress_route_owner(request.clone(), requester.clone(), hub_a, route_a.clone());
    let (routes_b, ownership_b) =
        fair_ingress_route_owner(request.clone(), requester.clone(), hub_b, route_b.clone());
    let response = lane_commit_qc_message(service.local_peer.clone());
    let mut retained = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
        vec![response.clone()],
        requester.clone(),
        routes_a,
        Some(ownership_a),
        ExactOutputRolloverClaim::Exact,
    )
    .expect("source A ownership is exact")
    .expect("source A response fanout");
    let candidate = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
        vec![response.clone()],
        requester.clone(),
        routes_b.clone(),
        Some(ownership_b),
        ExactOutputRolloverClaim::Exact,
    )
    .expect("source B ownership is exact")
    .expect("source B response fanout");
    assert!(retained.coalesce_retry(&candidate).expect("lossless merge"));
    assert_eq!(retained.targets.len(), 2);
    let ownership = retained
        .ingress_ownership
        .as_ref()
        .expect("coalesced response retains fair ownership");
    assert!(ownership.validate_exact());
    assert_eq!(ownership.admission_count, 2);
    assert!(ownership.matches_reply_routes(retained.reply_routes.as_ref()));
    assert!(retained.targets.iter().any(|target| {
        matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&route_a))
    }));
    assert!(retained.targets.iter().any(|target| {
        matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&route_b))
    }));
    let source_a_index = retained
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&route_a))
            })
            .expect("source A target");
    retained
        .mark_admitted(source_a_index)
        .expect("source A advances independently");
    let ownership = retained
        .ingress_ownership
        .as_ref()
        .expect("admission retains fair ownership");
    let source_a_cursor = ownership
        .attempts
        .iter()
        .find(|attempt| attempt.route.same_source(&route_a))
        .expect("source A cursor");
    let source_b_cursor = ownership
        .attempts
        .iter()
        .find(|attempt| attempt.route.same_source(&route_b))
        .expect("source B cursor");
    assert_eq!(source_a_cursor.message_cursor, 1);
    assert_eq!(source_b_cursor.message_cursor, 0);
    {
        let owned_fanout = |authenticated_via: PeerId, route: NetworkReplyRoute| {
            let (reply_routes, ownership) = fair_ingress_route_owner(
                request.clone(),
                requester.clone(),
                authenticated_via,
                route,
            );
            PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
                vec![response.clone()],
                requester.clone(),
                reply_routes,
                Some(ownership),
                ExactOutputRolloverClaim::Exact,
            )
            .expect("race source ownership is exact")
            .expect("race source response fanout")
        };
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let hub_d = PeerId::new(KeyPair::random().public_key().clone());
        let route_c = route_fixture.mint_via(requester.clone(), hub_c.clone());
        let route_d = route_fixture.mint_via(requester.clone(), hub_d.clone());
        let mut retained_race = owned_fanout(hub_c, route_c.clone());
        let candidate_race = owned_fanout(hub_d, route_d.clone());
        let plan = retained_race
            .reply_target_merge_plan_after_route_merge(&candidate_race, || {
                assert!(
                    route_fixture.retire(&route_c),
                    "retained source retires after the initial route merge"
                );
            })
            .expect("candidate source survives retained-source retirement");
        assert_eq!(
            plan.targets,
            vec![ReplyTargetMerge::Append { candidate_index: 0 }]
        );
        assert_eq!(
            plan.reply_routes.len(),
            2,
            "a disconnect after reconciliation is deferred to the next bounded snapshot"
        );
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_d))
        );
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_c))
        );
        let ownership = plan
            .ingress_ownership
            .as_ref()
            .expect("candidate source retains fair-ingress ownership");
        assert!(ownership.validate_exact());
        assert!(ownership.matches_reply_routes(Some(&plan.reply_routes)));
        assert_eq!(ownership.admission_count, 2);
        assert_eq!(ownership.attempts.len(), 2);
        assert!(ownership.attempts.iter().any(|attempt| {
            attempt.route.same_delivery(&route_d)
                && attempt.message_cursor == 0
                && attempt.chunk_cursor == 0
        }));
        let preview = retained_race
            .preview_coalesce_plan(&candidate_race, &plan)
            .expect("snapshot-coherent race plan has valid target geometry");
        retained_race.commit_coalesce_plan(&candidate_race, &plan, preview.current_source_targets);
        assert_eq!(
            retained_race
                .retain_active_unowned_reply_targets()
                .expect("the next service snapshot prunes only retired source C"),
            1
        );
        let retained_routes = retained_race
            .reply_routes
            .as_ref()
            .expect("source D retains route history");
        assert_eq!(retained_routes.len(), 1);
        assert!(
            retained_routes
                .iter()
                .any(|route| route.same_delivery(&route_d))
        );
        assert!(
            retained_race
                .ingress_ownership
                .as_ref()
                .is_some_and(|ownership| ownership.validate_exact()
                    && ownership.matches_reply_routes(Some(retained_routes)))
        );
        let hub_e = PeerId::new(KeyPair::random().public_key().clone());
        let hub_f = PeerId::new(KeyPair::random().public_key().clone());
        let route_e = route_fixture.mint_via(requester.clone(), hub_e.clone());
        let route_f = route_fixture.mint_via(requester.clone(), hub_f.clone());
        let mut retained_race = owned_fanout(hub_e, route_e.clone());
        let candidate_race = owned_fanout(hub_f, route_f.clone());
        let plan = retained_race
            .reply_target_merge_plan_after_route_merge(&candidate_race, || {
                assert!(
                    route_fixture.retire(&route_f),
                    "candidate source retires after the initial route merge"
                );
            })
            .expect("retained source survives candidate-source retirement");
        assert_eq!(
            plan.targets,
            vec![ReplyTargetMerge::Append { candidate_index: 0 }]
        );
        assert_eq!(plan.reply_routes.len(), 2);
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_e))
        );
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_f))
        );
        let ownership = plan
            .ingress_ownership
            .as_ref()
            .expect("retained source keeps fair-ingress ownership");
        assert!(ownership.validate_exact());
        assert!(ownership.matches_reply_routes(Some(&plan.reply_routes)));
        assert_eq!(ownership.admission_count, 2);
        assert_eq!(ownership.attempts.len(), 2);
        let preview = retained_race
            .preview_coalesce_plan(&candidate_race, &plan)
            .expect("candidate-retirement plan remains snapshot coherent");
        retained_race.commit_coalesce_plan(&candidate_race, &plan, preview.current_source_targets);
        assert_eq!(
            retained_race
                .retain_active_unowned_reply_targets()
                .expect("the next service snapshot prunes only retired source F"),
            1
        );
        let retained_routes = retained_race
            .reply_routes
            .as_ref()
            .expect("source E retains route history");
        assert_eq!(retained_routes.len(), 1);
        assert!(
            retained_routes
                .iter()
                .any(|route| route.same_delivery(&route_e))
        );
        assert!(
            retained_race
                .ingress_ownership
                .as_ref()
                .is_some_and(|ownership| ownership.validate_exact()
                    && ownership.matches_reply_routes(Some(retained_routes)))
        );
        let hub_g = PeerId::new(KeyPair::random().public_key().clone());
        let hub_h = PeerId::new(KeyPair::random().public_key().clone());
        let route_g = route_fixture.mint_via(requester.clone(), hub_g.clone());
        let route_h = route_fixture.mint_via(requester.clone(), hub_h.clone());
        let (routes_g, ownership_g) = fair_ingress_route_owner(
            request.clone(),
            requester.clone(),
            hub_g.clone(),
            route_g.clone(),
        );
        let (routes_h, ownership_h) =
            fair_ingress_route_owner(request.clone(), requester.clone(), hub_h, route_h.clone());
        let repeated_responses = vec![response.clone(), response.clone()];
        let mut retained_cursor =
            PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
                repeated_responses.clone(),
                requester.clone(),
                routes_g,
                Some(ownership_g),
                ExactOutputRolloverClaim::Exact,
            )
            .expect("source G cursor ownership is exact")
            .expect("source G response fanout");
        retained_cursor
            .mark_admitted(0)
            .expect("source G advances to its second immutable response");
        let candidate_h = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
            repeated_responses.clone(),
            requester.clone(),
            routes_h,
            Some(ownership_h),
            ExactOutputRolloverClaim::Exact,
        )
        .expect("source H cursor ownership is exact")
        .expect("source H response fanout");
        assert!(
            route_fixture.retire(&route_g),
            "source G retires before the authoritative strict-merge snapshot"
        );
        let plan = retained_cursor
            .reply_target_merge_plan(&candidate_h)
            .expect("source H progresses while retired source G stays owned");
        assert_eq!(
            plan.targets,
            vec![
                ReplyTargetMerge::Park { prior_index: 0 },
                ReplyTargetMerge::Append { candidate_index: 0 },
            ]
        );
        let preview = retained_cursor
            .preview_coalesce_plan(&candidate_h, &plan)
            .expect("parked-source merge preserves bounded geometry");
        retained_cursor.commit_coalesce_plan(&candidate_h, &plan, preview.current_source_targets);
        assert!(retained_cursor.targets[0].parked);
        assert_eq!(retained_cursor.targets[0].message_index, 1);
        let parked_cursor = retained_cursor
            .ingress_ownership
            .as_ref()
            .expect("parked source retains fair ownership")
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_g))
            .expect("parked source G retains its cursor");
        assert_eq!(parked_cursor.message_cursor, 1);
        assert_eq!(parked_cursor.chunk_cursor, 0);
        assert!(retained_cursor.targets.iter().any(|target| {
            matches!(&target.route, ExactTargetRoute::Reply(route)
                    if route.same_delivery(&route_h) && !target.parked)
        }));
        let reconnect_g = route_fixture.mint_via(requester.clone(), hub_g.clone());
        let (reconnect_routes, reconnect_ownership) = fair_ingress_route_owner(
            request.clone(),
            requester.clone(),
            hub_g,
            reconnect_g.clone(),
        );
        let reconnect_candidate =
            PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
                repeated_responses,
                requester.clone(),
                reconnect_routes,
                Some(reconnect_ownership),
                ExactOutputRolloverClaim::Exact,
            )
            .expect("reconnect ownership is exact")
            .expect("reconnect response fanout");
        let reconnect_plan = retained_cursor
            .reply_target_merge_plan(&reconnect_candidate)
            .expect("reconnect reuses the parked source owner");
        assert_eq!(
            reconnect_plan.targets,
            vec![ReplyTargetMerge::Update {
                prior_index: 0,
                candidate_index: 0,
                update: NetworkReplyRouteSourceUpdate::Reconnected,
            }]
        );
        let reconnect_preview = retained_cursor
            .preview_coalesce_plan(&reconnect_candidate, &reconnect_plan)
            .expect("reconnect preview preserves the current item");
        retained_cursor.commit_coalesce_plan(
            &reconnect_candidate,
            &reconnect_plan,
            reconnect_preview.current_source_targets,
        );
        assert!(!retained_cursor.targets[0].parked);
        assert_eq!(retained_cursor.targets[0].message_index, 1);
        assert!(matches!(
            &retained_cursor.targets[0].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&reconnect_g)
        ));
        let resumed_cursor = retained_cursor
            .ingress_ownership
            .as_ref()
            .expect("reconnected source retains fair ownership")
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&reconnect_g))
            .expect("reconnected source G retains its cursor");
        assert_eq!(resumed_cursor.message_cursor, 1);
        assert_eq!(resumed_cursor.chunk_cursor, 0);
    }
    let missing = PendingExactFanout::claimed_with_reply_routes(
        vec![response],
        requester,
        routes_b,
        ExactOutputRolloverClaim::Exact,
    )
    .expect("shape-only candidate")
    .expect("shape-only response fanout");
    assert!(retained.coalesce_retry(&missing).is_err());
}
#[test]
fn orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 4;
    let sender = service.context.roster[0].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(sender.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(sender.clone(), hub_b.clone());
    let payload_chunk = chunk(manifest_hash(b"fair buffered chunk"), 0, b"a", 0);
    let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(payload_chunk.clone()),
    ));
    let (_, ownership_a) =
        fair_ingress_route_owner(message.clone(), sender.clone(), hub_a, route_a.clone());
    let (_, ownership_b) =
        fair_ingress_route_owner(message, sender.clone(), hub_b, route_b.clone());
    assert_eq!(
        service.buffer_orphan_payload_chunk_owned(
            sender.clone(),
            payload_chunk.clone(),
            ownership_a,
        ),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk_owned(sender, payload_chunk.clone(), ownership_b),
        PayloadChunkDisposition::Duplicate
    );
    let ownership = service
        .orphan_chunks
        .get(&payload_chunk.manifest_hash)
        .and_then(|chunks| chunks.front())
        .and_then(|chunk| chunk.ingress_ownership.as_ref())
        .expect("buffered duplicate retains fair ownership");
    assert_eq!(ownership.admission_count, 2);
    let routes = ownership
        .current_reply_routes()
        .expect("both authenticated routes remain available");
    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|route| route.same_delivery(&route_a)));
    assert!(routes.iter().any(|route| route.same_delivery(&route_b)));
}
#[test]
fn manifest_bound_duplicate_promotes_proofless_orphan_to_runtime_owner() {
    let (mut service, keys) = fixture_with_block_payload();
    service.max_orphan_chunks = 1;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let directory = TempDir::new().expect("temporary promoted-orphan gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &directory);
    let (_, _, proposal, payload_chunk, sender) = productive_chunk_at_view(&service, &keys, 0);
    let chunk_bytes = u64::try_from(payload_chunk.bytes.len()).expect("fixture chunk fits u64");
    let chunk_message = || {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
            payload_chunk.clone(),
        ))
    };
    let proofless = admit_productive_orphan_runtime(&ingress, chunk_message(), sender.clone());
    assert!(proofless.leader_wire_runtime_receipt().is_none());
    assert_eq!(
        service
            .buffer_orphan_payload_chunk_owned(sender.clone(), payload_chunk.clone(), proofless,),
        PayloadChunkDisposition::Buffered
    );
    admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
    let productive = admit_productive_orphan_runtime(&ingress, chunk_message(), sender.clone());
    assert!(
        productive.leader_wire_runtime_receipt().is_some(),
        "the signed Proposal must bind the exact retransmitted chunk to real runtime ownership"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk_owned(sender, payload_chunk.clone(), productive),
        PayloadChunkDisposition::Duplicate
    );
    let promoted = service
        .orphan_chunks
        .get(&payload_chunk.manifest_hash)
        .and_then(|chunks| chunks.front())
        .and_then(|buffered| buffered.ingress_ownership.as_ref())
        .expect("one promoted orphan remains retained");
    assert!(promoted.leader_wire_runtime_receipt().is_some());
    assert!(
        !service.evict_one_proofless_orphan_chunk(),
        "proofless eviction cannot discard the promoted runtime owner"
    );
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, chunk_bytes);
}

fn bind_productive_orphan_test_ingress(
    service: &mut ProductionV2Services,
    directory: &TempDir,
) -> Arc<FairV2Ingress> {
    let roster = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    ingress
        .configure_roster_for_context(
            roster.clone(),
            &service.context.network_id,
            service.context.da_layout,
        )
        .expect("configure productive-orphan ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            service.context.da_layout.max_chunk_count,
        )
        .expect("derive productive-orphan lifecycle capacity");
    let owner = [0xE2; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            service.context.id(),
            service.context.height,
            owner,
            0,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &directory.path().join("productive-orphan-tail.wal"),
            service.context.id(),
            service.context.height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            service.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open productive-orphan lifecycle gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(64),
            service.context.id(),
            service.context.height,
        )
        .expect("bind productive-orphan lifecycle gate");
    ingress.open().expect("open productive-orphan ingress");
    service.leader_wire_recovery_authority = recovery_authority;
    service.leader_wire_ingress = Arc::clone(&ingress);
    ingress
}
fn admit_productive_orphan_runtime(
    ingress: &FairV2Ingress,
    message: wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message),
            sender,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let mut admitted = ingress.try_recv().expect("drain productive-orphan ingress");
    let mut ownership = admitted
        .take_ingress_ownership()
        .expect("productive orphan retains fair-ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind productive-orphan runtime receipt");
    ownership
}
fn buffer_productive_orphan_for_replay(
    service: &mut ProductionV2Services,
    ingress: &FairV2Ingress,
    sender: PeerId,
    chunk: wire::PayloadChunk,
) -> super::super::FairV2IngressLeaderWireToken {
    let ownership = admit_productive_orphan_runtime(
        ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    let token = ownership
        .leader_wire_token()
        .expect("productive orphan has a leader-wire token")
        .clone();
    assert_eq!(
        service.buffer_orphan_payload_chunk_owned(sender, chunk, ownership),
        PayloadChunkDisposition::Buffered
    );
    token
}
fn productive_chunk_at_view(
    service: &ProductionV2Services,
    keys: &[KeyPair],
    view: u64,
) -> (
    Vec<u8>,
    wire::PayloadManifest,
    wire::Proposal,
    wire::PayloadChunk,
    PeerId,
) {
    let (canonical_wire, payload) = proposal_body_and_payload_at_view(&service.context, keys, view);
    let (manifest, chunks) = payload.into_parts();
    assert!(
        !chunks.is_empty(),
        "fixture body must have an exact data chunk"
    );
    let proposer = service.context.leader(view);
    let proposer_index = usize::try_from(proposer).expect("small proposer index");
    let sender = service.context.roster[proposer_index].validator.clone();
    let mut proposal = wire::Proposal {
        round: manifest.round,
        proposer,
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        keys[proposer_index].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let validated = wire::ValidatedPayloadManifest::new(&service.context, manifest.clone())
        .expect("validate chunk manifest once");
    let mut chunk = wire::PayloadChunk {
        manifest_hash: validated.manifest_hash(),
        index: 0,
        bytes: chunks.into_iter().next().expect("fixture data chunk"),
        sender: proposer,
        signature: Vec::new(),
    };
    chunk.signature = Signature::new(
        keys[proposer_index].private_key(),
        &chunk
            .signature_payload(&validated)
            .expect("chunk signature payload")
            .signature_preimage(),
    )
    .payload()
    .to_vec();
    (canonical_wire, manifest, proposal, chunk, sender)
}
fn admit_and_terminalize_productive_proposal(
    ingress: &FairV2Ingress,
    proposal: wire::Proposal,
    sender: PeerId,
) {
    let ownership = admit_productive_orphan_runtime(
        ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal)),
        sender,
    );
    ingress
        .mark_leader_wire_volatile_terminal(
            ownership
                .leader_wire_runtime_receipt()
                .expect("proposal has productive runtime ownership"),
        )
        .expect("terminalize proposal after binding its manifest coordinates");
}
fn chunk_effect_executor(
    service: &ProductionV2Services,
    recovered: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
) -> V2EffectExecutor<SaturatedCompletionRuntime> {
    chunk_effect_executor_with_exact_ownership(service, recovered, None)
}
fn chunk_effect_executor_with_exact_ownership(
    service: &ProductionV2Services,
    recovered: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    exact_effect_ownership: Option<(AdapterEffect, RuntimeEffectOwnership)>,
) -> V2EffectExecutor<SaturatedCompletionRuntime> {
    let mut runtime = SaturatedCompletionRuntime::admitting_network_ingress(0, 8);
    runtime.exact_effect_ownership = exact_effect_ownership;
    V2EffectExecutor::with_runtime(
        runtime,
        recovered,
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct productive-chunk effect executor")
}
fn chunk_effect_executor_with_remote_proposal(
    service: &ProductionV2Services,
    recovered: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    proposal: wire::Proposal,
) -> V2EffectExecutor<SaturatedCompletionRuntime> {
    let effect = AdapterEffect::FetchBody {
        tag: service.active_tag,
        round: proposal.round,
        subject: proposal.subject,
        manifest: Some(proposal.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            service.active_tag,
            1,
        )],
    )
    .expect("bind authenticated productive-chunk Fetch owner")
    .pop()
    .expect("one productive-chunk Fetch has one exact owner");
    assert!(ownership.bind_authenticated_remote_proposal_replay_for_test(proposal, &effect));
    chunk_effect_executor_with_exact_ownership(service, recovered, Some((effect, ownership)))
}
#[test]
fn productive_chunk_waits_for_exact_fetch_before_runtime_handoff() {
    let (mut service, keys) = fixture_with_block_payload();
    service.max_orphan_chunks = 1;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let gate_directory = TempDir::new().expect("temporary productive-chunk ingress gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let (_, manifest, proposal, chunk, sender) = productive_chunk_at_view(&service, &keys, 0);
    let proposal_replay = proposal.clone();
    admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());

    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),
            )),
            sender.clone(),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let token = {
        let state = ingress.state.lock();
        let record = state
            .leader_wire_lifecycles
            .values()
            .find(|record| record.token.matches_chunk_manifest(chunk.manifest_hash))
            .expect("productive chunk owns one exact ingress lifecycle");
        assert_eq!(
            record.status,
            super::super::FairV2IngressLeaderWireStatus::Ingress
        );
        record.token.clone()
    };
    let physical_cut = ingress.next_physical_admission_ordinal();

    let mut executor =
        chunk_effect_executor_with_remote_proposal(&service, BTreeMap::new(), proposal_replay);
    assert!(
        ingress
            .capture_next_ingress_turn_cut_before(physical_cut, |occurrence| {
                super::super::v2_effects::v2_ingress_head_can_drain(
                    occurrence.inbound(),
                    &executor,
                    None,
                )
            })
            .expect("classify the productive pre-fetch chunk")
            .is_none(),
        "a current productive chunk must retain durable Ingress ownership until its exact fetch exists"
    );
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::Ingress
    );

    let durable = DurableBodyReceipt::for_test(
        service.context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let durable_executor = chunk_effect_executor(
        &service,
        BTreeMap::from([(
            (manifest.round, manifest.subject),
            (manifest.clone(), durable),
        )]),
    );
    let durable_cut = ingress
        .capture_next_ingress_turn_cut_before(physical_cut, |occurrence| {
            super::super::v2_effects::v2_ingress_head_can_drain(
                occurrence.inbound(),
                &durable_executor,
                None,
            )
        })
        .expect("classify the exact durable-body chunk")
        .expect("durable body ownership makes the productive chunk drainable");
    assert_eq!(
        durable_cut.selected_disposition(),
        super::super::FairV2IngressDequeueDisposition::Admit
    );
    drop(durable_cut);

    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: service.active_tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut service,
        )
        .expect("open the exact manifest-bearing chunk fetch");
    let fetch_cut = ingress
        .capture_next_ingress_turn_cut_before(physical_cut, |occurrence| {
            super::super::v2_effects::v2_ingress_head_can_drain(
                occurrence.inbound(),
                &executor,
                None,
            )
        })
        .expect("classify the exact fetch-owned chunk")
        .expect("an exact manifest fetch makes the productive chunk drainable");
    let (mut inbound, disposition) = fetch_cut
        .dequeue_exact_retaining()
        .unwrap_or_else(|_| panic!("dequeue the exact fetch-owned productive chunk"));
    assert_eq!(
        disposition,
        super::super::FairV2IngressDequeueDisposition::Admit
    );
    let ownership = inbound
        .take_ingress_ownership()
        .expect("dequeued productive chunk retains fair-ingress ownership");
    assert!(ownership.leader_wire_runtime_receipt().is_some());
    let (message, routed_sender) = inbound.into_message_and_sender();
    let BlockMessage::V2(message) = message else {
        panic!("productive chunk changed its v2 envelope")
    };
    let wire::ConsensusMessageV2Payload::PayloadChunk(routed_chunk) = message.payload else {
        panic!("productive chunk changed its payload family")
    };
    assert_eq!(routed_sender, sender);
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, routed_sender, routed_chunk, ownership)
            .expect("route the exact fetch-owned productive chunk"),
        PayloadChunkDisposition::Delivered
    );
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn durable_reconstructed_body_terminalizes_late_chunk_across_arrival_order() {
    for durable_before_late_chunk in [false, true] {
        let (mut service, keys) = fixture_with_block_payload();
        service.max_orphan_chunks = 16;
        service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
        let gate_directory = TempDir::new().expect("temporary durable-chunk gate");
        let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
        let (_, manifest, proposal, chunk, sender) = productive_chunk_at_view(&service, &keys, 0);
        admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
        let ownership = admit_productive_orphan_runtime(
            &ingress,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
                chunk.clone(),
            )),
            sender.clone(),
        );
        let token = ownership
            .leader_wire_token()
            .expect("late chunk has a productive token")
            .clone();
        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let recovered = BTreeMap::from([(
            (manifest.round, manifest.subject),
            (manifest.clone(), durable),
        )]);
        let mut executor = chunk_effect_executor(
            &service,
            if durable_before_late_chunk {
                recovered.clone()
            } else {
                BTreeMap::new()
            },
        );
        let disposition = service
            .route_payload_chunk(&mut executor, sender.clone(), chunk, ownership)
            .expect("route late chunk around durable recovery");
        if durable_before_late_chunk {
            assert_eq!(
                disposition,
                PayloadChunkDisposition::Duplicate,
                "pre-existing durable recovery must terminalize the late chunk immediately"
            );
        } else {
            assert_eq!(
                disposition,
                PayloadChunkDisposition::Buffered,
                "the late chunk must remain owned until durable recovery arrives"
            );
            assert_eq!(service.orphan_chunk_count, 1);
            assert_ne!(service.orphan_chunk_bytes, 0);
            executor = chunk_effect_executor(&service, recovered);
            assert_eq!(
                service
                    .replay_buffered_chunks(&mut executor)
                    .expect("durable recovery sweeps the buffered runtime owner"),
                0
            );
        }
        assert!(
            service.orphan_chunks.is_empty(),
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            service.orphan_chunk_count, 0,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            service.orphan_chunk_bytes, 0,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
            super::super::FairV2IngressLeaderWireStatus::Terminal,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        let next_view = (1..=1_024)
            .find(|view| service.context.leader(*view) == service.context.leader(0))
            .expect("bounded view search returns to the same leader");
        let (_, _, next_proposal, next_chunk, next_sender) =
            productive_chunk_at_view(&service, &keys, next_view);
        assert_eq!(
            next_sender, sender,
            "view rotation returns to the same origin"
        );
        admit_and_terminalize_productive_proposal(&ingress, next_proposal, next_sender.clone());
        let next = admit_productive_orphan_runtime(
            &ingress,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
                next_chunk,
            )),
            next_sender,
        );
        let next_token = next.leader_wire_token().expect("next-view token");
        assert_eq!(
            next_token.view(),
            next_view,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert!(
            next.leader_wire_runtime_receipt().is_some(),
            "higher-view chunk must reach Runtime admission"
        );
        assert_eq!(
            ingress.state.lock().leader_wire_lifecycles[&next_token.slot].status,
            super::super::FairV2IngressLeaderWireStatus::Runtime,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
    }
}
#[test]
fn productive_orphan_lifecycle_sweep_bounds_turns_services_completion_and_wraps() {
    let (mut service, keys) = fixture_with_block_payload();
    // Each retained chunk owns its actual (validator, chunk-index) slot.
    // One complete leader rotation yields independent live slots; a second
    // rotation cannot replace those owners merely by changing the view.
    let capacity = service.context.roster.len();
    assert!(capacity > MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY);
    assert!(
        capacity
            <= usize::try_from(service.context.da_layout.max_chunk_count)
                .expect("fixture orphan capacity fits usize")
    );
    service.max_orphan_chunks = capacity;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let gate_directory = TempDir::new().expect("temporary bounded orphan-sweep gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let mut complete_recovered = BTreeMap::new();
    let mut recovered_keys = Vec::with_capacity(capacity);
    let mut tokens = Vec::with_capacity(capacity);
    for view in 0..u64::try_from(capacity).expect("fixture capacity fits u64") {
        let (_, manifest, proposal, chunk, sender) =
            productive_chunk_at_view(&service, &keys, view);
        admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
        let manifest_hash = HashOf::new(&manifest);
        let token = buffer_productive_orphan_for_replay(&mut service, &ingress, sender, chunk);
        assert!(
            tokens.iter().all(
                |(_, previous): &(_, super::super::FairV2IngressLeaderWireToken)| previous.slot
                    != token.slot
            ),
            "each retained owner must have a distinct live slot"
        );
        tokens.push((manifest_hash, token));
        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            manifest.round,
            manifest.subject,
            manifest_hash,
        );
        let key = (manifest.round, manifest.subject);
        recovered_keys.push((manifest_hash, key));
        complete_recovered.insert(key, (manifest, durable));
    }
    assert_eq!(service.orphan_chunk_count, capacity);
    assert_eq!(
        service
            .orphan_chunks
            .values()
            .map(VecDeque::len)
            .sum::<usize>(),
        capacity
    );
    // Keep the last deterministic sweep position live while every other
    // exact owner is already durable. This forces a full cursor cycle and
    // a wrap before the final owner can retire.
    let retained_manifest_hash = *service
        .orphan_chunks
        .keys()
        .next_back()
        .expect("capacity fixture has a final manifest");
    let retained_key = recovered_keys
        .iter()
        .find_map(|(manifest_hash, key)| (*manifest_hash == retained_manifest_hash).then_some(*key))
        .expect("retained manifest has exact recovered coordinates");
    let mut partial_recovered = complete_recovered.clone();
    assert!(partial_recovered.remove(&retained_key).is_some());
    let mut executor = chunk_effect_executor(&service, partial_recovered);
    let (command_tx, _command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    completion_tx
        .try_send(V2IoCompletion::AuxiliaryNoop)
        .expect("queue completion behind the first bounded sweep");
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("first bounded lifecycle sweep remains valid"),
        0
    );
    assert_eq!(service.orphan_chunk_count, capacity.saturating_sub(1));
    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("bounded sweep returns a completion service opportunity"),
        1,
        "a ready service completion must run before the next lifecycle sweep"
    );
    // No worker owns this synthetic channel; remove it before service Drop
    // attempts the production shutdown handshake.
    drop(service.io.take());
    for _ in 1..capacity {
        let before = service.orphan_chunk_count;
        assert_eq!(
            service
                .replay_buffered_chunks(&mut executor)
                .expect("bounded lifecycle sweep remains valid"),
            0,
            "terminal lifecycle sweeping must not report chunk delivery"
        );
        assert!(
            before.saturating_sub(service.orphan_chunk_count) <= 1,
            "one service turn may deeply classify at most one orphan"
        );
    }
    assert_eq!(
        service.orphan_chunk_count, 1,
        "one Retain owner must not starve the durable tail during a complete cursor cycle"
    );
    let retained_token = tokens
        .iter()
        .find_map(|(manifest_hash, token)| {
            (*manifest_hash == retained_manifest_hash).then_some(token)
        })
        .expect("retained manifest has a lifecycle token");
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&retained_token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::Runtime
    );
    let mut complete_executor = chunk_effect_executor(&service, complete_recovered);
    assert_eq!(
        service
            .replay_buffered_chunks(&mut complete_executor)
            .expect("cursor wrap reaches the newly durable retained owner"),
        0
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(tokens.iter().all(|(_, token)| {
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status
            == super::super::FairV2IngressLeaderWireStatus::Terminal
    }));
    assert_eq!(
        service
            .replay_buffered_chunks(&mut complete_executor)
            .expect("an empty lifecycle sweep is idle"),
        0
    );
    assert!(service.orphan_lifecycle_sweep_cursor.is_none());
}
#[test]
fn productive_retry_after_proofless_reconstruction_does_not_become_orphan() {
    let (mut service, keys) = fixture_with_block_payload();
    service.max_orphan_chunks = 16;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let gate_directory = TempDir::new().expect("temporary reconstructed-chunk gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let (_, manifest, proposal, chunk, sender) = productive_chunk_at_view(&service, &keys, 0);
    let proofless = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    assert!(proofless.leader_wire_runtime_receipt().is_none());
    let mut executor =
        chunk_effect_executor_with_remote_proposal(&service, BTreeMap::new(), proposal.clone());
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender.clone(), chunk.clone(), proofless)
            .expect("buffer proofless chunk"),
        PayloadChunkDisposition::Buffered
    );
    admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
    let tag = service.active_tag;
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut service,
        )
        .expect("open proofless reconstruction fetch");
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("reconstruct proofless body"),
        1
    );
    assert_eq!(service.local_completions.len(), 1);
    let productive = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    let token = productive
        .leader_wire_token()
        .expect("retransmit binds productive token")
        .clone();
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender, chunk, productive)
            .expect("queued reconstruction owns the exact bytes"),
        PayloadChunkDisposition::Duplicate
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal
    );
}
#[test]
fn session_changed_terminal_failure_still_retires_productive_orphan_tail() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let gate_directory = TempDir::new().expect("temporary productive-orphan gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let (canonical_wire, manifest, proposal, completing_chunk, completing_sender) =
        productive_chunk_at_view(&service, &keys, 0);
    let proposer = service.context.roster
        [usize::try_from(proposal.proposer).expect("small proposer index")]
    .validator
    .clone();
    let _proposal_ownership = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone())),
        proposer,
    );
    assert_eq!(manifest.layout.data_shards, 1);
    assert_eq!(manifest.layout.parity_shards, 1);
    assert_eq!(manifest.chunk_hashes.len(), 2);
    assert!(manifest.payload_size_bytes <= u64::from(manifest.layout.chunk_size_bytes));
    assert_eq!(completing_chunk.index, 0);
    let sender = service.context.roster[0].validator.clone();
    let current_failure_chunk = chunk(HashOf::new(&manifest), 1, b"current terminal failure", 0);
    let tail_failure_chunk = chunk(HashOf::new(&manifest), 2, b"tail terminal failure", 0);
    let tail_success_chunk = chunk(HashOf::new(&manifest), 3, b"tail terminal success", 0);
    let expected_bytes = [
        &completing_chunk,
        &current_failure_chunk,
        &tail_failure_chunk,
        &tail_success_chunk,
    ]
    .into_iter()
    .map(|chunk| u64::try_from(chunk.bytes.len()).expect("small orphan chunk"))
    .sum::<u64>();
    let _completing_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        completing_sender,
        completing_chunk,
    );
    let current_failure_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        sender.clone(),
        current_failure_chunk,
    );
    let tail_failure_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        sender.clone(),
        tail_failure_chunk,
    );
    let tail_success_token =
        buffer_productive_orphan_for_replay(&mut service, &ingress, sender, tail_success_chunk);
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, expected_bytes);
    {
        let mut state = ingress.state.lock();
        state
            .leader_wire_lifecycles
            .get_mut(&current_failure_token.slot)
            .expect("current faulted productive orphan remains indexed")
            .status = super::super::FairV2IngressLeaderWireStatus::Terminal;
        assert!(
            state
                .leader_wire_lifecycles
                .remove(&tail_failure_token.slot)
                .is_some(),
            "tail fault injection removes only its in-memory terminal target"
        );
    }
    let mut executor =
        chunk_effect_executor_with_remote_proposal(&service, BTreeMap::new(), proposal.clone());
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut service,
        )
        .expect("open productive-orphan fetch session");
    let current_error = "leader-wire volatile terminal changed runtime ownership";
    let tail_error = "leader-wire volatile terminal has no runtime record";
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect_err("current session-changed terminal transfer must fail"),
        format!(
            "{current_error}; additionally failed to retire buffered payload tail: {tail_error}"
        )
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(matches!(
        service.local_completions.front(),
        Some(LocalCompletion::Reconstructed { body, .. })
            if body.as_ref() == canonical_wire.as_slice()
    ));
    let state = ingress.state.lock();
    assert_eq!(
        state
            .leader_wire_lifecycles
            .get(&current_failure_token.slot)
            .expect("current faulted owner remains indexed")
            .status,
        super::super::FairV2IngressLeaderWireStatus::Terminal
    );
    assert!(
        !state
            .leader_wire_lifecycles
            .contains_key(&tail_failure_token.slot),
        "the combined error must come from attempting the missing tail target"
    );
    assert_eq!(
        state
            .leader_wire_lifecycles
            .get(&tail_success_token.slot)
            .expect("last tail owner remains indexed")
            .status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal,
        "tail retirement must continue after retaining its first error"
    );
}
#[test]
fn owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let (canonical_wire, manifest, proposal, payload_chunk, sender) =
        productive_chunk_at_view(&service, &keys, 0);
    assert_eq!(manifest.layout.data_shards, 1);
    assert_eq!(manifest.layout.parity_shards, 1);
    assert_eq!(manifest.chunk_hashes.len(), 2);
    assert!(manifest.payload_size_bytes <= u64::from(manifest.layout.chunk_size_bytes));
    assert_eq!(payload_chunk.index, 0);
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(sender.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(sender.clone(), hub_b.clone());
    let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(payload_chunk.clone()),
    ));
    let (_, mut ownership_a) =
        fair_ingress_route_owner(message.clone(), sender.clone(), hub_a, route_a.clone());
    let (_, ownership_b) =
        fair_ingress_route_owner(message, sender.clone(), hub_b, route_b.clone());
    assert!(ownership_a.advance_reply_cursors(&route_a, 3, 5));
    let mut executor =
        chunk_effect_executor_with_remote_proposal(&service, BTreeMap::new(), proposal.clone());
    assert_eq!(
        service
            .route_payload_chunk(
                &mut executor,
                sender.clone(),
                payload_chunk.clone(),
                ownership_a,
            )
            .expect("buffer owned orphan chunk"),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender, payload_chunk.clone(), ownership_b,)
            .expect("coalesce alternate owned orphan route"),
        PayloadChunkDisposition::Duplicate
    );
    let expected_ownership_projection = {
        let ownership = service
            .orphan_chunks
            .get_mut(&payload_chunk.manifest_hash)
            .and_then(|buffered| buffered.front_mut())
            .and_then(|buffered| buffered.ingress_ownership.as_mut())
            .expect("coalesced orphan retains fair-ingress ownership");
        assert!(ownership.advance_reply_cursors(&route_b, 7, 11));
        assert_eq!(ownership.admission_count, 2);
        let routes = ownership
            .current_reply_routes()
            .expect("both authenticated source routes remain owned");
        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| route.same_delivery(&route_a)));
        assert!(routes.iter().any(|route| route.same_delivery(&route_b)));
        let source_a = ownership
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_a))
            .expect("source A cursor ownership");
        let source_b = ownership
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_b))
            .expect("source B cursor ownership");
        assert_eq!((source_a.message_cursor, source_a.chunk_cursor), (3, 5));
        assert_eq!((source_b.message_cursor, source_b.chunk_cursor), (7, 11));
        ownership.process_local_projection_hash()
    };
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    assert_eq!(
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag,
                    round: manifest.round,
                    subject: manifest.subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut service,
            )
            .expect("open matching live fetch session"),
        1
    );
    assert!(
        service
            .fetch_work_for_manifest(payload_chunk.manifest_hash)
            .is_some()
    );
    let retained = service
        .orphan_chunks
        .get(&payload_chunk.manifest_hash)
        .and_then(|buffered| buffered.front())
        .and_then(|buffered| buffered.ingress_ownership.as_ref())
        .expect("opening the session must not alter orphan ownership");
    assert_eq!(
        retained.process_local_projection_hash(),
        expected_ownership_projection
    );
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("replay exact owned orphan chunk"),
        1
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(matches!(
        service.local_completions.front(),
        Some(LocalCompletion::Reconstructed {
            manifest: completed_manifest,
            body,
            ..
        }) if completed_manifest == &manifest && body.as_ref() == canonical_wire.as_slice()
    ));
    assert!(!service.output_guard.restart_required());
}
#[test]
fn orphan_chunk_bounds_preserve_exact_duplicate_semantics_at_capacity() {
    let (mut service, _) = fixture();
    let hash = manifest_hash(b"manifest-a");
    let sender = service.context.roster[0].validator.clone();
    let first = chunk(hash, 0, b"a", 0);
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), first.clone()),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), first),
        PayloadChunkDisposition::Duplicate,
        "an exact retransmission remains idempotent even when the buffer is full"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), chunk(hash, 0, b"b", 0)),
        PayloadChunkDisposition::Rejected,
        "a conflicting claim cannot replace retained bytes"
    );
    assert_eq!(
        service
            .buffer_orphan_payload_chunk(sender, chunk(manifest_hash(b"manifest-b"), 0, b"c", 0)),
        PayloadChunkDisposition::Rejected,
        "one unknown manifest cannot force storage beyond the global bound"
    );
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
}
#[test]
fn proofless_orphan_eviction_releases_exact_count_and_byte_capacity() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 2;
    service.max_orphan_chunk_bytes = 2;
    let sender = service.context.roster[0].validator.clone();
    let first_hash = manifest_hash(b"proofless-eviction-a");
    let second_hash = manifest_hash(b"proofless-eviction-b");
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), chunk(first_hash, 0, b"a", 0),),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender, chunk(second_hash, 0, b"b", 0)),
        PayloadChunkDisposition::Buffered
    );
    assert!(service.evict_one_proofless_orphan_chunk());
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
    assert_eq!(
        service
            .orphan_chunks
            .values()
            .map(VecDeque::len)
            .sum::<usize>(),
        1
    );
    assert!(service.evict_one_proofless_orphan_chunk());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(service.orphan_chunks.is_empty());
    assert!(!service.evict_one_proofless_orphan_chunk());
}
#[test]
fn authenticated_orphan_flood_stays_inside_frozen_count_and_byte_geometry() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = 4;
    for sender_index in 0..4_u32 {
        let sender_position = usize::try_from(sender_index).expect("test sender index fits usize");
        let sender = service.context.roster[sender_position].validator.clone();
        assert_eq!(
            service.buffer_orphan_payload_chunk(
                sender,
                chunk(
                    manifest_hash(&[0xA0, u8::try_from(sender_index).expect("small index")]),
                    0,
                    &[u8::try_from(sender_index).expect("small index")],
                    sender_index,
                ),
            ),
            PayloadChunkDisposition::Buffered,
            "each authenticated roster source can consume only the shared finite orphan budget"
        );
    }
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, 4);
    let attacker = service.context.roster[0].validator.clone();
    let retained = chunk(manifest_hash(&[0xA0, 0]), 0, &[0], 0);
    assert_eq!(
        service.buffer_orphan_payload_chunk(attacker.clone(), retained),
        PayloadChunkDisposition::Duplicate,
        "the exact retained identity still coalesces at the capacity boundary"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(
            attacker,
            chunk(manifest_hash(b"fifth authenticated orphan"), 1, &[0xFF], 0),
        ),
        PayloadChunkDisposition::Rejected,
        "authenticated junk cannot replenish beyond the frozen global owner universe"
    );
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, 4);
}
#[test]
fn orphan_chunk_cheap_checks_reject_spoofing_and_oversize_without_allocation() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 8;
    let hash = manifest_hash(b"manifest-cheap-checks");
    let validator_zero = service.context.roster[0].validator.clone();
    let validator_one = service.context.roster[1].validator.clone();
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_one, chunk(hash, 0, b"a", 0)),
        PayloadChunkDisposition::Rejected,
        "outer transport identity must match the claimed validator index"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(
            validator_zero.clone(),
            chunk(hash, service.context.da_layout.max_chunk_count, b"a", 0),
        ),
        PayloadChunkDisposition::Rejected
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 0, &[], 0)),
        PayloadChunkDisposition::Rejected
    );
    assert_eq!(
        service
            .buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 0, b"123456789", 0)),
        PayloadChunkDisposition::Rejected
    );
    let mut missing_signature = chunk(hash, 0, b"a", 0);
    missing_signature.signature.clear();
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero.clone(), missing_signature),
        PayloadChunkDisposition::Rejected
    );
    let mut oversized_signature = chunk(hash, 0, b"a", 0);
    oversized_signature.signature = vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES + 1];
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero.clone(), oversized_signature),
        PayloadChunkDisposition::Rejected
    );
    service.max_orphan_chunk_bytes = 1;
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero, chunk(hash, 0, b"ab", 0)),
        PayloadChunkDisposition::Rejected
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
}
#[test]
fn merge_sidecar_validation_deferral_retains_exact_request_idempotently() {
    let (mut service, _) = fixture();
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"merge carrier parent",
        ))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"merge carrier block")),
        payload_hash: Hash::new(b"merge carrier payload"),
    };
    let reference = merge_sidecar_reference(b"merge sidecar");
    let work_id = EffectWorkId::for_test(7);
    service
        .work_deferred_for_merge_sidecar(work_id, round, subject, &reference)
        .expect("retain exact merge-sidecar deferral");
    service
        .work_deferred_for_merge_sidecar(work_id, round, subject, &reference)
        .expect("exact retransmission is idempotent");
    let mut conflicting = reference.clone();
    conflicting.encoded_len += 1;
    assert!(
        service
            .work_deferred_for_merge_sidecar(work_id, round, subject, &conflicting)
            .is_err(),
        "one work ID cannot claim conflicting reference metadata"
    );
    assert_eq!(service.merge_sidecar_deferrals.len(), 1);
    let deferred = service
        .take_merge_sidecar_deferral()
        .expect("retained merge-sidecar deferral");
    assert_eq!(deferred.round(), round);
    assert_eq!(deferred.work_id(), work_id);
    assert_eq!(deferred.subject(), subject);
    assert_eq!(deferred.reference(), &reference);
    assert!(service.take_merge_sidecar_deferral().is_none());
}
#[test]
fn merge_sidecar_validation_deferral_returns_error_at_capacity_without_eviction() {
    let (mut service, _) = fixture();
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 3,
    };
    let first_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"first merge carrier")),
        payload_hash: Hash::new(b"first merge payload"),
    };
    let second_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"second merge carrier")),
        payload_hash: Hash::new(b"second merge payload"),
        ..first_subject
    };
    let first_reference = merge_sidecar_reference(b"first merge sidecar");
    let second_reference = merge_sidecar_reference(b"second merge sidecar");
    service
        .work_deferred_for_merge_sidecar(
            EffectWorkId::for_test(1),
            round,
            first_subject,
            &first_reference,
        )
        .expect("fill bounded deferral queue");
    assert_eq!(service.merge_sidecar_deferrals.len(), 1);
    assert!(
        service
            .work_deferred_for_merge_sidecar(
                EffectWorkId::for_test(2),
                round,
                second_subject,
                &second_reference,
            )
            .is_err(),
        "a different validation cannot displace the retained exact request"
    );
    assert_eq!(service.merge_sidecar_deferrals.len(), 1);
    let retained = service
        .take_merge_sidecar_deferral()
        .expect("original deferral remains retained");
    assert_eq!(retained.subject(), first_subject);
    assert_eq!(retained.reference(), &first_reference);
}
#[test]
fn outbound_payload_registration_is_exactly_idempotent_and_signed() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 8;
    let payload = b"authoritative body";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
        payload_hash: Hash::new(payload),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let encoded = encode_payload(&service.context, round, subject, payload).expect("encode");
    let expected_manifest = encoded.manifest().clone();
    assert_eq!(
        service
            .register_outbound_payload(service.active_tag, encoded.clone())
            .expect("first registration"),
        expected_manifest
    );
    let first_frames = service
        .outbound_chunks
        .get(&HashOf::new(&expected_manifest))
        .expect("first registration retains chunks")
        .messages
        .iter()
        .map(|message| {
            let NetworkMessage::SumeragiBlock(envelope) = message else {
                panic!("retained payload chunk changed network lane")
            };
            Arc::clone(envelope)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        service
            .register_outbound_payload(service.active_tag, encoded)
            .expect("exact retransmission"),
        expected_manifest
    );
    let messages = service
        .outbound_chunks
        .get(&HashOf::new(&expected_manifest))
        .expect("retained chunks");
    assert_eq!(
        messages.messages.len(),
        expected_manifest.chunk_hashes.len()
    );
    assert!(
        messages
            .messages
            .iter()
            .zip(first_frames)
            .all(|(message, first)| matches!(
                message,
                NetworkMessage::SumeragiBlock(envelope) if Arc::ptr_eq(envelope, &first)
            ))
    );
    assert!(messages.messages.iter().all(|message| {
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return false;
        };
        envelope.as_ref().encoded_len().is_some()
            && matches!(
                envelope.as_message(),
                BlockMessage::V2(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::PayloadChunk(chunk)
                            if !chunk.signature.is_empty()
                    )
            )
    }));
}
#[test]
fn decision_retires_candidate_and_outbound_work_but_keeps_exact_sidecar_deferral() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 8;
    service.max_merge_sidecar_deferrals = 2;
    let decision_round = locked_candidate_round(&service, 0);
    let decision_subject = locked_candidate_subject(b"decided candidate");
    let losing_subject = locked_candidate_subject(b"losing candidate");
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    service
        .request_locked_candidate(service.active_tag, decision_round, decision_subject)
        .expect("queue decided candidate acquisition");
    service
        .prepared_candidates
        .push_back(PreparedCandidateBody {
            tag: service.active_tag,
            subject: decision_subject,
        });
    let reference = merge_sidecar_reference(b"decided merge sidecar");
    service
        .merge_sidecar_deferrals
        .push_back(DeferredMergeSidecarWork {
            work_id: EffectWorkId::for_test(91),
            round: decision_round,
            subject: decision_subject,
            reference: reference.clone(),
        });
    service
        .merge_sidecar_deferrals
        .push_back(DeferredMergeSidecarWork {
            work_id: EffectWorkId::for_test(92),
            round: decision_round,
            subject: losing_subject,
            reference,
        });
    let encoded = outbound_payload_at_view(&service, 0);
    service
        .register_outbound_payload(service.active_tag, encoded)
        .expect("retain terminally superseded outbound payload");
    service
        .retire_all_outbound_payloads()
        .expect("retire outbound payloads at Decision");
    service
        .retire_candidate_work_after_decision(decision_round, decision_subject)
        .expect("retire candidate work at Decision");
    assert!(service.proposal_work_retired);
    assert!(service.outbound_chunks.is_empty());
    assert!(service.locked_candidate_acquisition.is_none());
    assert!(service.prepared_candidates.is_empty());
    assert!(matches!(
        service.merge_sidecar_deferrals.as_slices(),
        ([deferred], [])
            if deferred.round() == decision_round
                && deferred.subject() == decision_subject
    ));
    let terminal_payload = outbound_payload_at_view(&service, 0);
    assert!(
        service
            .register_outbound_payload(service.active_tag, terminal_payload)
            .is_err()
    );
    assert!(command_rx.try_iter().next().is_some());
    detach_locked_candidate_io(&mut service);
}
fn outbound_payload_at_view(service: &ProductionV2Services, view: u64) -> EncodedV2Payload {
    let body = view.to_le_bytes();
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            &[b"bounded outbound view", body.as_slice()].concat(),
        )),
        payload_hash: Hash::new(&body),
    };
    encode_payload(
        &service.context,
        wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view,
        },
        subject,
        &body,
    )
    .expect("encode view-owned payload")
}
fn timeout_certificate_at_view(
    service: &ProductionV2Services,
    view: u64,
) -> wire::TimeoutCertificate {
    wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view,
        },
        groups: Vec::new(),
    }
}

#[cfg(feature = "bls")]
struct WorkerWalAuthorityFixture {
    adapter: SumeragiV2Adapter,
    ingress: Arc<FairV2Ingress>,
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
}

#[cfg(feature = "bls")]
fn worker_wal_authority_fixture(
    service: &mut ProductionV2Services,
    directory: &TempDir,
    local_validator: Option<wire::ValidatorIndex>,
) -> WorkerWalAuthorityFixture {
    let context = service.context.clone();
    let wal_path = directory.path().join("worker-authority.wal");
    let fingerprints = AdapterFingerprints {
        node: Hash::new(b"worker actual WAL authority"),
        build: Hash::new(b"worker authority fixture build"),
        config: Hash::new(b"worker authority fixture config"),
    };
    let (adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        VerifiedHeightContext::genesis(context.clone(), service.validator_set_pops.clone())
            .expect("authenticate worker fixture context"),
        local_validator,
        service.active_tag.generation(),
        [0xE2; 32],
        fingerprints,
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open worker authority safety WAL");
    assert!(startup.is_empty());
    assert_eq!(adapter.current_tag(), service.active_tag);
    let authority = adapter
        .leader_wire_recovery_authority()
        .expect("project actual initial WAL consumer");
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<BTreeSet<_>>();
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(
            adapter
                .mint_leader_wire_store_authority(&wal_path)
                .expect("mint the worker's exact safety-WAL sibling"),
            context.id(),
            context.height,
            fingerprints.node.into(),
            roster.clone(),
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("derive worker lifecycle slots"),
            context.da_layout.max_chunk_count,
            authority,
            &[],
            &[],
        )
        .expect("open worker WAL-owned ingress gate");
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::fair_v2_ingress_required_certified_fence_escape_bytes(roster.len()),
            8 * 1024 * 1024,
            super::super::fair_v2_ingress_required_transport_completion_bytes(context.da_layout)
                .max(super::super::MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES),
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    ingress
        .configure_roster_for_context(roster, &context.network_id, context.da_layout)
        .expect("configure worker authority ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            lifecycle_ordinals.clone(),
            context.id(),
            context.height,
        )
        .expect("bind worker authority ingress");
    ingress.open().expect("open worker authority ingress");
    service.leader_wire_recovery_authority = authority;
    service.leader_wire_ingress = Arc::clone(&ingress);
    WorkerWalAuthorityFixture {
        adapter,
        ingress,
        lifecycle_ordinals,
    }
}

#[cfg(feature = "bls")]
fn worker_signed_timeout_certificate(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
    highest_prepare_qc: Option<wire::QuorumCertificate>,
) -> wire::TimeoutCertificate {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let signers = vec![0, 1, 2];
    let shares = signers
        .iter()
        .map(|signer| {
            let vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: highest_prepare_qc.clone(),
                signer: *signer,
                signature: Vec::new(),
            };
            Signature::new(
                keys[*signer as usize].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate actual timeout shares"),
        }],
    }
}

#[cfg(feature = "bls")]
fn worker_signed_prepare_certificate(
    keys: &[KeyPair],
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
) -> wire::QuorumCertificate {
    let signers = vec![0, 1, 2];
    let shares = signers
        .iter()
        .map(|signer| {
            let vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment,
                signer: *signer,
                signature: Vec::new(),
            };
            Signature::new(
                keys[*signer as usize].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate actual Prepare shares"),
    }
}

#[cfg(feature = "bls")]
fn worker_authority_commitment() -> wire::ExecutionCommitment {
    wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"worker authority parent state"),
        Hash::new(b"worker authority post state"),
        Hash::new(b"worker authority writes"),
        1,
        Hash::new(b"worker authority executed wire"),
    )
}

#[cfg(feature = "bls")]
fn persist_worker_commit_intent(
    adapter: &mut SumeragiV2Adapter,
    keys: &[KeyPair],
    directory: &TempDir,
) -> wire::Vote {
    let prepare = prepare_worker_commit_intent(adapter, keys, directory);
    let authenticated = adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(prepare),
        ))
        .expect("authenticate actual PrepareQC");
    let effects = adapter
        .receive_authenticated(authenticated)
        .expect("persist actual LockAndCommit")
        .into_effects();
    effects
        .into_iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if vote.phase == wire::GlobalPhase::Commit => Some(vote),
            _ => None,
        })
        .expect("durable LockAndCommit exposes its exact Commit statement")
}

#[cfg(feature = "bls")]
fn prepare_worker_commit_intent(
    adapter: &mut SumeragiV2Adapter,
    keys: &[KeyPair],
    directory: &TempDir,
) -> wire::QuorumCertificate {
    let context = adapter.wire_context().clone();
    let (bytes, payload) = proposal_body_and_payload_at_view(&context, keys, 0);
    let mut body_store = V2BodyStore::open_with_policy(
        directory.path().join("commit-body"),
        context.clone(),
        BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open canonical Commit fixture body store");
    let durable = body_store
        .store(payload.manifest().clone(), bytes)
        .expect("persist canonical local proposal body");
    let validated = body_store
        .validate(&durable, |_| Ok::<_, String>(worker_authority_commitment()))
        .expect("persist exact local execution commitment");
    let mut effects = adapter
        .local_proposal_ready(
            adapter.current_tag(),
            payload.manifest().clone(),
            &durable,
            &validated,
        )
        .expect("persist local ProposalIntent")
        .into_effects();
    let _proposal_sign_owner = adapter
        .take_live_proposal_intent_wal_sign(&effects)
        .expect("consume the exact post-fsync proposal-sign sidecar")
        .expect("a live ProposalIntent owns its exact Sign continuation");
    for expected_phase in [None, Some(wire::GlobalPhase::Prepare)] {
        let (tag, request) = effects
            .into_iter()
            .find_map(|effect| match effect {
                AdapterEffect::Sign { tag, request } => Some((tag, request)),
                _ => None,
            })
            .expect("durable local intent emits exact Sign");
        let signer = match &request {
            SignRequest::Proposal(proposal) => {
                assert_eq!(expected_phase, None);
                proposal.proposer
            }
            SignRequest::Vote(vote) => {
                assert_eq!(expected_phase, Some(vote.phase));
                vote.signer
            }
            SignRequest::TimeoutVote(_) => panic!("proposal preparation cannot sign Timeout"),
        };
        let signature = Signature::new(
            keys[signer as usize].private_key(),
            &request.signature_preimage(),
        )
        .payload()
        .to_vec();
        effects = adapter
            .signature_completed(tag, signature)
            .expect("complete exact local durable signature")
            .into_effects();
    }
    let prepare = worker_signed_prepare_certificate(
        keys,
        payload.manifest().round,
        payload.manifest().subject,
        validated.execution_commitment(),
    );
    prepare
}

#[cfg(feature = "bls")]
fn enter_worker_view_from_wal(
    service: &mut ProductionV2Services,
    adapter: &mut SumeragiV2Adapter,
    certificate: wire::TimeoutCertificate,
) -> EventTag {
    let authenticated = adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ))
        .expect("authenticate actual view certificate");
    let effects = adapter
        .receive_authenticated(authenticated)
        .expect("persist actual view transition")
        .into_effects();
    let (tag, certificate, protected_lock) = effects
        .into_iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                certificate,
                protected_lock,
            } => Some((tag, certificate, protected_lock)),
            _ => None,
        })
        .expect("actual WAL transition emits EnterView");
    service
        .finish_runtime_step_reconciliation(
            None,
            Some(
                adapter
                    .leader_wire_recovery_authority()
                    .expect("post-WAL authority"),
            ),
        )
        .expect("publish actual WAL authority before EnterView");
    service
        .entered_view(
            tag,
            certificate,
            protected_lock
                .as_ref()
                .map(|locked| (locked.proposal_round, locked.subject)),
        )
        .expect("consume exact published EnterView");
    tag
}
#[cfg(feature = "bls")]
fn deliver_worker_executor_wire(
    executor: &mut V2EffectExecutor,
    service: &mut ProductionV2Services,
    ingress: &FairV2Ingress,
    now: Instant,
    payload: wire::ConsensusMessageV2Payload,
    sender: wire::ValidatorIndex,
) -> super::super::serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt {
    let message = wire::ConsensusMessageV2::new(payload);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message.clone()),
            service.context.roster[sender as usize].validator.clone(),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut admitted = ingress.try_recv().expect("dequeue actual worker ingress");
    let ownership = admitted
        .take_ingress_ownership()
        .expect("exact worker physical owner");
    let receipt = ownership
        .leader_wire_runtime_receipt()
        .expect("actual WAL-backed runtime receipt")
        .clone();
    executor
        .enqueue_network_with_ingress_ownership(message, ownership)
        .expect("authenticate exact owned worker input");
    assert!(matches!(
        executor
            .step(now, service)
            .expect("dispatch runtime effects through production services"),
        EffectExecutorStep::Advanced { .. }
    ));
    assert_eq!(service.active_tag, executor.current_tag());
    assert!(service.leader_wire_recovery_authority.consumer_tag() == executor.current_tag());
    assert!(!service.output_guard.restart_required());
    receipt
}

#[cfg(feature = "bls")]
#[test]
fn runtime_executor_publishes_actual_wal_consumer_before_same_round_enter_view_and_vote_retry() {
    let (mut service, keys) = fixture_with_block_payload();
    service.local_validator = None;
    let directory = TempDir::new().expect("full runtime consumer regression");
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, None);
    let context = service.context.clone();
    let mut body_store = V2BodyStore::open_with_policy(
        directory.path().join("body"),
        context.clone(),
        BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open actual executor body store");
    let (vote_bytes, vote_payload) = proposal_body_and_payload_at_view(&context, &keys, 1);
    let (origin_bytes, origin_payload) = proposal_body_and_payload_at_view(&context, &keys, 0);
    assert_eq!(
        vote_bytes, origin_bytes,
        "a certified view change retains the exact origin-signed block body"
    );
    assert_eq!(
        vote_payload.manifest().subject,
        origin_payload.manifest().subject
    );
    let durable = body_store
        .store(vote_payload.manifest().clone(), vote_bytes)
        .expect("retain the exact signed body for the remote Prepare vote");
    let validated = body_store
        .validate(&durable, |_| Ok::<_, String>(worker_authority_commitment()))
        .expect("persist its execution commitment before direct vote admission");
    wal.adapter
        .bind_validated_body(vote_payload.manifest(), &validated)
        .expect("bind independently durable execution authority before constructing the runtime");
    let started_at = Instant::now();
    let (runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
        wal.adapter,
        Vec::new(),
        started_at,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        wal.lifecycle_ordinals,
    )
    .expect("construct runtime with the exact gate ordinal source");
    assert!(startup.is_empty());
    let output_guard = ConsensusOutputGuard::isolated();
    let (mut executor, _body_store) = V2EffectExecutor::open_with_body_store(
        runtime, body_store,
        super::super::v2_lifecycle_coordinator::RecoveredDurableValidateRetryCensusV1::empty_for_test(),
        None, context.clone(), service.local_peer.clone(), None, Arc::clone(&output_guard),
        EffectQueueConfig::default(),
    ).expect("open actual production executor");
    service.output_guard = output_guard;
    let _commands = attach_locked_candidate_io(&mut service, 8);
    service.set_exact_output_admission_hook(|_, _| Ok(()));
    executor.arm_live_clocks(
        super::super::v2_lifecycle_coordinator::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
        started_at,
    ).expect("activate actual runtime clocks");
    deliver_worker_executor_wire(
        &mut executor,
        &mut service,
        &wal.ingress,
        started_at,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(worker_signed_timeout_certificate(
            &context, &keys, 0, None,
        )),
        0,
    );
    let first_tag = executor.current_tag();
    assert_eq!(first_tag.view(), 1);
    let vote_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 1,
    };
    let mut vote = wire::Vote {
        round: vote_round,
        proposal_round: vote_round,
        phase: wire::GlobalPhase::Prepare,
        subject: vote_payload.manifest().subject,
        execution_commitment: worker_authority_commitment(),
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
        .payload()
        .to_vec();
    let first_receipt = deliver_worker_executor_wire(
        &mut executor,
        &mut service,
        &wal.ingress,
        started_at,
        wire::ConsensusMessageV2Payload::Vote(vote.clone()),
        0,
    );
    let protected = worker_signed_prepare_certificate(
        &keys,
        wire::ConsensusRound {
            view: 0,
            ..vote_round
        },
        locked_candidate_subject(b"full runtime stronger TC lock"),
        worker_authority_commitment(),
    );
    deliver_worker_executor_wire(
        &mut executor,
        &mut service,
        &wal.ingress,
        started_at,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(worker_signed_timeout_certificate(
            &context,
            &keys,
            0,
            Some(protected.clone()),
        )),
        0,
    );
    let upgraded_tag = executor.current_tag();
    assert_eq!(upgraded_tag.view(), first_tag.view());
    assert!(upgraded_tag.strictly_advances(first_tag));
    assert!(service.leader_wire_recovery_authority.matches_entered_view(
        upgraded_tag,
        Some((protected.proposal_round, protected.subject))
    ));
    let retry_receipt = deliver_worker_executor_wire(
        &mut executor,
        &mut service,
        &wal.ingress,
        started_at,
        wire::ConsensusMessageV2Payload::Vote(vote.clone()),
        0,
    );
    assert_eq!(
        retry_receipt.token(),
        first_receipt.token(),
        "same logical vote retains its immutable service and scheduler ordinals"
    );
    assert_ne!(
        retry_receipt, first_receipt,
        "same view does not identify the replacement volatile consumer"
    );
    assert!(
        wal.ingress
            .mark_leader_wire_volatile_terminal(&first_receipt)
            .is_err(),
        "a late receipt cannot retire a different consumer epoch"
    );
    // Reaching quorum after the retry proves the new reducer pool consumed it.
    for signer in [1, 2] {
        let mut sibling = vote.clone();
        sibling.signer = signer;
        sibling.signature = Signature::new(
            keys[signer as usize].private_key(),
            &sibling.signature_preimage(),
        )
        .payload()
        .to_vec();
        deliver_worker_executor_wire(
            &mut executor,
            &mut service,
            &wal.ingress,
            started_at,
            wire::ConsensusMessageV2Payload::Vote(sibling),
            signer,
        );
    }
    let expected = worker_signed_prepare_certificate(
        &keys,
        vote.round,
        vote.subject,
        vote.execution_commitment,
    );
    assert!(
        executor
            .last_runtime_step_observation_for_test()
            .expect("observe quorum reduction")
            .sole_broadcast_is_exact_prepare_qc(&expected)
    );
    assert_eq!(executor.status().pending_outputs, 1);
    assert_eq!(
        executor.pending_lifecycle_output_admission_census().len(),
        1,
        "the sole exact PrepareQC output remains owned until lifecycle admission services it"
    );
    assert!(!service.output_guard.restart_required());
    detach_locked_candidate_io(&mut service);
}

#[cfg(feature = "bls")]
#[test]
fn entered_view_accepts_same_view_higher_generation_supersession() {
    let (mut service, keys) = fixture();
    let directory = TempDir::new().expect("actual same-round worker WAL");
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, None);
    let initial = service.active_tag;
    let thin = worker_signed_timeout_certificate(&service.context, &keys, initial.view(), None);
    let view_one = enter_worker_view_from_wal(&mut service, &mut wal.adapter, thin.clone());
    let payload = outbound_payload_at_view(&service, view_one.view());
    service
        .register_outbound_payload(view_one, payload)
        .expect("retain work owned by the first view-one generation");
    assert!(
        service.entered_view(view_one, thin, None).is_err(),
        "an equal lifecycle tag is not a supersession"
    );
    let invalid_tag = EventTag::new(
        view_one.height(),
        view_one.view(),
        Generation::new(view_one.generation().get() + 1),
    );
    let wrong_round =
        worker_signed_timeout_certificate(&service.context, &keys, view_one.view(), None);
    assert!(
        service
            .entered_view(invalid_tag, wrong_round, None)
            .is_err(),
        "the certificate must identify the immediate predecessor round"
    );
    let locked_round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: initial.view(),
    };
    let prepare = worker_signed_prepare_certificate(
        &keys,
        locked_round,
        locked_candidate_subject(b"actual same-round promoted lock"),
        worker_authority_commitment(),
    );
    let stronger =
        worker_signed_timeout_certificate(&service.context, &keys, initial.view(), Some(prepare));
    let rebound = enter_worker_view_from_wal(&mut service, &mut wal.adapter, stronger);
    assert_eq!(rebound.view(), view_one.view());
    assert!(rebound.strictly_advances(view_one));
    assert_eq!(service.active_tag, rebound);
    assert!(service.outbound_chunks.is_empty());
    assert!(!service.output_guard.restart_required());
}
#[cfg(feature = "bls")]
#[test]
fn entered_view_advances_live_leader_wire_recovery_cut() {
    let (mut service, keys) = fixture_with_block_payload();
    let gate_directory = TempDir::new().expect("temporary live view-cut gate");
    let mut wal = worker_wal_authority_fixture(&mut service, &gate_directory, None);
    let ingress = Arc::clone(&wal.ingress);
    let initial = service.active_tag;
    let certificate =
        worker_signed_timeout_certificate(&service.context, &keys, initial.view(), None);
    let next = enter_worker_view_from_wal(&mut service, &mut wal.adapter, certificate.clone());
    let (gate, before) = {
        let state = ingress.state.lock();
        (
            Arc::clone(
                state
                    .leader_wire_lifecycle_gate
                    .as_ref()
                    .expect("actual WAL gate"),
            ),
            (
                state.len,
                state.bytes,
                state.last_admission_ordinal,
                state.leader_wire_lifecycles.len(),
                state.pending_wire_owners.len(),
            ),
        )
    };
    let durable_before = gate
        .restore()
        .expect("read actual durable ownership before stale replay");
    let (_, _, stale_proposal, _, stale_sender) =
        productive_chunk_at_view(&service, &keys, initial.view());
    let stale_admission = ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(stale_proposal),
        )),
        stale_sender,
    ));
    assert!(
        matches!(
            &stale_admission,
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ),
        "obsolete exact wire must coalesce without a new owner: {stale_admission:?}"
    );
    let after = {
        let state = ingress.state.lock();
        (
            state.len,
            state.bytes,
            state.last_admission_ordinal,
            state.leader_wire_lifecycles.len(),
            state.pending_wire_owners.len(),
        )
    };
    assert_eq!(
        after, before,
        "obsolete delivery cannot allocate queue bytes or an owner"
    );
    let durable_after = gate
        .restore()
        .expect("read durable ownership after stale replay");
    assert_eq!(
        durable_after.records().len(),
        durable_before.records().len()
    );
    assert_eq!(
        durable_after.last_admission_ordinal(),
        durable_before.last_admission_ordinal()
    );
    assert_eq!(
        durable_after.scheduler_ordinal_high_watermark(),
        durable_before.scheduler_ordinal_high_watermark()
    );
    let (_, _, mut current_proposal, _, current_sender) =
        productive_chunk_at_view(&service, &keys, next.view());
    current_proposal.justification =
        wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: certificate,
            highest_prepare_qc: None,
        });
    current_proposal.signature = Signature::new(
        keys[usize::try_from(current_proposal.proposer).expect("current proposer index")]
            .private_key(),
        &current_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let current_message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(current_proposal));
    wal.adapter
        .authenticate(current_message.clone())
        .expect("current proposal carries the actual entered TC and exact signature");
    let current_admission = ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(current_message),
        current_sender,
    ));
    assert!(
        matches!(
            &current_admission,
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ),
        "the actual new WAL view must admit current work: {current_admission:?}"
    );
    assert_eq!(ingress.len(), before.0 + 1);
}
#[cfg(feature = "bls")]
#[test]
fn entered_view_publishes_the_exact_protected_commit_vote_cut() {
    let (mut service, keys) = fixture_with_block_payload();
    let directory = TempDir::new().expect("actual protected-Commit WAL");
    let local_validator = service.context.leader(0);
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, Some(local_validator));
    let mut commit = persist_worker_commit_intent(&mut wal.adapter, &keys, &directory);
    commit.signature = Signature::new(
        keys[commit.signer as usize].private_key(),
        &commit.signature_preimage(),
    )
    .payload()
    .to_vec();
    let certificate = worker_signed_timeout_certificate(&service.context, &keys, 0, None);
    let next = enter_worker_view_from_wal(&mut service, &mut wal.adapter, certificate);
    assert!(commit.round.view < next.view());
    assert!(
        service
            .leader_wire_recovery_authority
            .matches_entered_view(next, Some((commit.proposal_round, commit.subject)))
    );
    let mut unrelated = commit.clone();
    unrelated.execution_commitment =
        wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"unrelated parent"),
            Hash::new(b"unrelated post"),
            Hash::new(b"unrelated writes"),
            1,
            Hash::new(b"unrelated execution"),
        );
    unrelated.signature = Signature::new(
        keys[unrelated.signer as usize].private_key(),
        &unrelated.signature_preimage(),
    )
    .payload()
    .to_vec();
    let sender = service.context.roster[commit.signer as usize]
        .validator
        .clone();
    let (gate, before) = {
        let state = wal.ingress.state.lock();
        (
            Arc::clone(
                state
                    .leader_wire_lifecycle_gate
                    .as_ref()
                    .expect("actual protected-Commit lifecycle gate"),
            ),
            (
                state.len,
                state.bytes,
                state.last_admission_ordinal,
                state.leader_wire_lifecycles.len(),
                state.pending_wire_owners.len(),
            ),
        )
    };
    let durable_before = gate.restore().expect("read protected-Commit ownership");
    // A changed execution statement is permanently obsolete after this view
    // transition. Coalescing it must not allocate an owner or consume ordinals.
    assert!(matches!(
        wal.ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(unrelated)
                )),
                sender.clone()
            )),
        Ok(super::super::FairV2IngressPushDisposition::Coalesced)
    ));
    let after = {
        let state = wal.ingress.state.lock();
        (
            state.len,
            state.bytes,
            state.last_admission_ordinal,
            state.leader_wire_lifecycles.len(),
            state.pending_wire_owners.len(),
        )
    };
    assert_eq!(
        after, before,
        "unrelated Commit cannot acquire ingress ownership"
    );
    assert_eq!(
        gate.restore()
            .expect("read ownership after unrelated Commit"),
        durable_before,
        "unrelated Commit cannot alter any durable lifecycle record or ordinal"
    );
    assert!(matches!(
        wal.ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(commit)
                )),
                sender
            )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
}

#[test]
fn durable_decision_advances_live_leader_wire_recovery_cut() {
    let (mut service, keys) = fixture_with_block_payload();
    let gate_directory = TempDir::new().expect("temporary live Decision-cut gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let _command_rx = attach_locked_candidate_io(&mut service, 4);
    let decided_subject = locked_candidate_subject(b"live leader-wire Decision cut");
    service
        .finish_runtime_step_reconciliation(
            Some(decided_subject),
            Some(
                service
                    .leader_wire_recovery_authority
                    .with_durable_decision(),
            ),
        )
        .expect("publish Decision and retire obsolete leader-wire identities");
    let census = || {
        let state = ingress.state.lock();
        (
            state.len,
            state.bytes,
            state.last_admission_ordinal,
            state.leader_wire_lifecycles.len(),
            state.pending_wire_owners.len(),
            state.open,
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .expect("bound durable gate")
                .restore()
                .expect("read exact durable restart image"),
        )
    };
    let after_decision = census();
    assert!(
        after_decision.5,
        "Decision retains ingress for auxiliary service"
    );
    for view in [service.active_tag.view(), service.active_tag.view() + 1] {
        let (_, _, proposal, _, sender) = productive_chunk_at_view(&service, &keys, view);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(proposal),
                )),
                sender,
            )),
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert_eq!(
            census(),
            after_decision,
            "obsolete proposals must not allocate or rewrite durable ownership"
        );
        assert!(ingress.try_recv().is_none());
        assert!(!service.output_guard.restart_required());
    }
    detach_locked_candidate_io(&mut service);
}
#[cfg(feature = "bls")]
#[test]
fn outbound_payload_retention_is_constant_across_many_view_changes() {
    let (mut service, keys) = fixture();
    let expected_payload_bytes = outbound_payload_at_view(&service, 0)
        .into_parts()
        .1
        .iter()
        .map(Vec::len)
        .sum::<usize>();
    assert!(expected_payload_bytes >= std::mem::size_of::<u64>());
    let directory = TempDir::new().expect("actual bounded-retention WAL");
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, None);
    let mut max_manifests = 0usize;
    let mut max_payload_bytes = 0usize;
    for view in 0..=1_024 {
        if view != 0 {
            let certificate =
                worker_signed_timeout_certificate(&service.context, &keys, view - 1, None);
            enter_worker_view_from_wal(&mut service, &mut wal.adapter, certificate);
            assert!(
                service.outbound_chunks.is_empty(),
                "view installation must prune the prior payload before publishing ownership"
            );
        }
        let tag = wal.adapter.current_tag();
        assert_eq!(tag.view(), view);
        let encoded = outbound_payload_at_view(&service, view);
        service
            .register_outbound_payload(tag, encoded)
            .expect("register exact active-view payload");
        let payload_bytes = service
            .outbound_chunks
            .values()
            .flat_map(|retained| retained.messages.iter())
            .map(|message| {
                let NetworkMessage::SumeragiBlock(envelope) = message else {
                    return 0;
                };
                let BlockMessage::V2(message) = envelope.as_message() else {
                    return 0;
                };
                match &message.payload {
                    wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => chunk.bytes.len(),
                    _ => 0,
                }
            })
            .sum::<usize>();
        max_manifests = max_manifests.max(service.outbound_chunks.len());
        max_payload_bytes = max_payload_bytes.max(payload_bytes);
        assert_eq!(service.outbound_chunks.len(), 1);
        assert_eq!(payload_bytes, expected_payload_bytes);
    }
    assert_eq!(max_manifests, 1);
    assert_eq!(max_payload_bytes, expected_payload_bytes);
}
#[cfg(feature = "bls")]
#[test]
fn late_stale_proposal_signature_cannot_restore_pruned_outbound_payload() {
    let (mut service, keys) = fixture();
    let directory = TempDir::new().expect("actual stale-signature WAL");
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, None);
    let old_tag = service.active_tag;
    let old_payload = outbound_payload_at_view(&service, old_tag.view());
    service
        .register_outbound_payload(old_tag, old_payload.clone())
        .expect("register old-view proposal payload");
    assert_eq!(service.outbound_chunks.len(), 1);
    let certificate =
        worker_signed_timeout_certificate(&service.context, &keys, old_tag.view(), None);
    let new_tag = enter_worker_view_from_wal(&mut service, &mut wal.adapter, certificate);
    assert!(service.outbound_chunks.is_empty());
    service
        .restore_outbound_payload_after_signature(CompletionDisposition::Stale, Some(old_payload))
        .expect("stale completion is retired without restoring bytes");
    assert!(service.outbound_chunks.is_empty());
    assert_eq!(service.active_tag, new_tag);
    assert!(!service.output_guard.restart_required());
}
#[test]
fn observer_cannot_register_or_disseminate_a_proposal_payload() {
    let (mut service, _) = fixture();
    service.local_validator = None;
    let payload = b"observer payload";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"observer block")),
        payload_hash: Hash::new(payload),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let encoded = encode_payload(&service.context, round, subject, payload).expect("encode");
    assert!(
        service
            .register_outbound_payload(service.active_tag, encoded)
            .is_err()
    );
    assert!(service.outbound_chunks.is_empty());
}
#[test]
fn pipeline_release_tracks_only_successfully_queued_durable_prepare_intent() {
    let (mut service, _) = fixture();
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let tag = EventTag::new(
        service.context.height,
        0,
        Generation::new(service.context.height),
    );
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"prepared block")),
        payload_hash: Hash::new(b"prepared payload"),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let execution_commitment =
        wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"worker prepared parent state"),
            Hash::new(b"worker prepared post state"),
            Hash::new(b"worker prepared ordinary writes"),
            1,
            Hash::new(b"worker prepared executed block wire"),
        );
    let vote = |phase| wire::Vote {
        round,
        proposal_round: round,
        phase,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    service
        .enqueue_consensus_sign(ConsensusSignTask::for_test(
            1,
            tag,
            super::super::v2::SignRequest::Vote(vote(wire::GlobalPhase::Prepare)),
        ))
        .expect("queue Prepare signature");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign {
            restore_outbound_payload: false,
            ..
        })
    ));
    assert_eq!(
        service.take_prepared_candidate(),
        Some(PreparedCandidateBody { tag, subject })
    );
    service
        .enqueue_consensus_sign(ConsensusSignTask::for_test(
            2,
            tag,
            super::super::v2::SignRequest::Vote(vote(wire::GlobalPhase::Commit)),
        ))
        .expect("queue Commit signature");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign {
            restore_outbound_payload: false,
            ..
        })
    ));
    assert_eq!(service.take_prepared_candidate(), None);
    // No worker owns this synthetic channel; remove it before service Drop
    // attempts the production shutdown handshake.
    drop(service.io.take());
}

#[cfg(feature = "bls")]
#[test]
fn current_commit_keeps_exact_owner_until_prepare_qc_publishes_local_lock() {
    use super::super::v2_runtime::LeaderWireRuntimeTerminal;
    let (mut service, keys) = fixture_with_block_payload();
    let directory = TempDir::new().expect("actual current-Commit WAL");
    let local = service.context.leader(0);
    let mut wal = worker_wal_authority_fixture(&mut service, &directory, Some(local));
    let prepare = prepare_worker_commit_intent(&mut wal.adapter, &keys, &directory);
    assert!(
        wal.adapter
            .leader_wire_recovery_authority()
            .expect("actual pre-QC authority")
            .matches_entered_view(wal.adapter.current_tag(), None)
    );
    let signer = (0..u32::try_from(keys.len()).unwrap())
        .find(|index| *index != local)
        .expect("an actual distinct remote validator");
    let sender = wal.adapter.wire_context().roster[signer as usize]
        .validator
        .clone();
    let mut vote = wire::Vote {
        round: prepare.round,
        proposal_round: prepare.proposal_round,
        phase: wire::GlobalPhase::Commit,
        subject: prepare.subject,
        execution_commitment: prepare.execution_commitment,
        signer,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(
        keys[signer as usize].private_key(),
        &vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone()));
    wal.adapter
        .authenticate(message.clone())
        .expect("remote Commit matches actual durable execution authority before PrepareQC");
    let exact_peer_payload = message.payload.clone();
    let ingress = Arc::clone(&wal.ingress);
    let now = Instant::now();
    let (mut runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
        wal.adapter,
        Vec::new(),
        now,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        wal.lifecycle_ordinals,
    )
    .expect("construct actual serialized consumer after local Prepare signing");
    assert!(startup.is_empty());
    ingress
        .advance_leader_wire_recovery_cut(
            runtime
                .driver()
                .leader_wire_recovery_authority()
                .expect("actual prepared WAL frontier"),
        )
        .expect("publish exact prepared consumer");
    let admission = ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(message.clone()),
        sender.clone(),
    ));
    assert!(
        matches!(
            &admission,
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ),
        "current unlocked Commit must own one bounded carrier: {admission:?}"
    );
    let mut inbound = ingress
        .try_recv()
        .expect("dequeue exact current Commit carrier");
    let owner = inbound
        .take_ingress_ownership()
        .expect("exact physical Commit owner");
    let receipt = owner
        .leader_wire_runtime_receipt()
        .expect("durable runtime handoff")
        .clone();
    runtime
        .enqueue_network_with_ingress_ownership(message, owner)
        .expect("transfer exact Commit owner into the real runtime");
    runtime
        .arm_live_clocks(now)
        .expect("arm bounded runtime clocks");
    let wal_before = runtime
        .driver()
        .leader_wire_recovery_authority()
        .expect("pre-admission WAL authority");
    let pending = runtime
        .step(now)
        .expect("current lock-missing Commit remains retryable");
    assert!(
        matches!(&pending, RuntimeStep::Idle),
        "pending Commit does not produce side effects: {pending:?}"
    );
    assert_eq!(
        runtime
            .driver()
            .leader_wire_recovery_authority()
            .expect("unchanged WAL authority"),
        wal_before
    );
    assert_eq!(runtime.queued_commands(), 1);
    runtime
        .take_last_scheduler_ownership()
        .expect("retained Commit scheduler owner")
        .validate_exact()
        .expect("exact bounded FIFO retention");
    assert!(
        runtime
            .take_effect_ownership(0)
            .expect("no pending effects")
            .is_empty()
    );
    assert!(
        runtime.take_leader_wire_runtime_terminals().is_empty(),
        "missing local lock cannot retire the original physical owner"
    );
    let gate = Arc::clone(
        ingress
            .state
            .lock()
            .leader_wire_lifecycle_gate
            .as_ref()
            .expect("actual bounded safety-WAL gate"),
    );
    let before = gate.restore().expect("inspect exact retained Commit slot");
    let mut competing = vote;
    competing.subject.payload_hash = Hash::new(b"competing current Commit subject");
    competing.signature = Signature::new(
        keys[signer as usize].private_key(),
        &competing.signature_preimage(),
    )
    .payload()
    .to_vec();
    let competing = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(competing));
    assert!(
        runtime.driver().authenticate(competing.clone()).is_err(),
        "an incompatible candidate cannot borrow exact local execution authority"
    );
    let competing_admission = ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(competing),
        sender.clone(),
    ));
    assert!(
        matches!(
            &competing_admission,
            Err(super::super::FairV2IngressPushError::Rejected(_))
        ),
        "a competing same-view statement cannot replace the retained owner: {competing_admission:?}"
    );
    assert_eq!(ingress.len(), 0);
    let after = gate
        .restore()
        .expect("inspect bounded slot after competing delivery");
    assert_eq!(after.records().len(), before.records().len());
    assert_eq!(
        after.last_admission_ordinal(),
        before.last_admission_ordinal()
    );
    assert_eq!(
        after.scheduler_ordinal_high_watermark(),
        before.scheduler_ordinal_high_watermark()
    );
    let qc_message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(prepare));
    let qc_admission = ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(qc_message.clone()),
        sender,
    ));
    assert!(
        matches!(
            &qc_admission,
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ),
        "exact PrepareQC remains bounded progress: {qc_admission:?}"
    );
    let mut qc_inbound = ingress.try_recv().expect("dequeue exact PrepareQC");
    let qc_owner = qc_inbound
        .take_ingress_ownership()
        .expect("exact physical QC owner");
    runtime
        .enqueue_network_with_ingress_ownership(qc_message, qc_owner)
        .expect("enqueue the dependency behind the retained Commit");
    let RuntimeStep::Advanced(effects) = runtime
        .step(now)
        .expect("progress class services PrepareQC")
    else {
        panic!("current PrepareQC cannot idle behind a retained Commit");
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("PrepareQC scheduler owner")
        .validate_exact()
        .expect("exact PrepareQC turn");
    let owners = runtime
        .take_effect_ownership(effects.len())
        .expect("exact post-QC effect owners");
    let (tag, commit, sign_owner) = effects
        .into_iter()
        .zip(owners)
        .find_map(|(effect, owner)| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Commit => Some((tag, vote, owner)),
            _ => None,
        })
        .expect("actual PrepareQC persists LockAndCommit and emits its owned Sign");
    assert!(
        runtime
            .driver()
            .leader_wire_recovery_authority()
            .expect("actual post-QC authority")
            .matches_entered_view(
                runtime.round_tag(),
                Some((commit.proposal_round, commit.subject))
            )
    );
    assert!(
        runtime
            .driver()
            .wire_ingress_may_use_progress(&exact_peer_payload),
        "the retained exact peer statement is now eligible Commit progress"
    );
    ingress
        .advance_leader_wire_recovery_cut(
            runtime
                .driver()
                .leader_wire_recovery_authority()
                .expect("actual lock authority"),
        )
        .expect("publish lock without replacing current owner");
    for terminal in runtime.take_leader_wire_runtime_terminals() {
        let LeaderWireRuntimeTerminal::Volatile(retired) = terminal else {
            panic!("PrepareQC has one volatile terminal");
        };
        assert_ne!(retired, receipt, "only PrepareQC was consumed so far");
        ingress
            .mark_leader_wire_volatile_terminal(&retired)
            .expect("release consumed QC");
    }
    let signature = Signature::new(
        keys[commit.signer as usize].private_key(),
        &commit.signature_preimage(),
    )
    .payload()
    .to_vec();
    runtime
        .enqueue_signature_with_owner(tag, signature, &sign_owner)
        .expect("enqueue exact local Commit signer dependency");
    let mut consumed = false;
    let mut signed_local_commit = false;
    for _ in 0..6 {
        let step = runtime
            .step(now)
            .expect("bounded signing and Commit retry progress");
        let RuntimeStep::Advanced(effects) = step else {
            panic!("owned Commit or its signer remains runnable");
        };
        signed_local_commit |= effects.iter().any(|effect| {
            matches!(effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote), ..
            }) if vote.phase == wire::GlobalPhase::Commit && vote.signer == local)
        });
        runtime
            .take_last_scheduler_ownership()
            .expect("signing/retry scheduler owner")
            .validate_exact()
            .expect("exact signing or retry turn");
        runtime
            .take_effect_ownership(effects.len())
            .expect("consume exact resulting effects");
        ingress
            .advance_leader_wire_recovery_cut(
                runtime
                    .driver()
                    .leader_wire_recovery_authority()
                    .expect("actual consumer after signing or retry"),
            )
            .expect("publish resulting consumer");
        for terminal in runtime.take_leader_wire_runtime_terminals() {
            let LeaderWireRuntimeTerminal::Volatile(retired) = terminal else {
                panic!("peer Commit has an ordinary terminal");
            };
            if retired == receipt {
                assert!(
                    signed_local_commit,
                    "the exact signer dependency completes before peer Commit consumption"
                );
                assert!(!consumed, "the original Commit terminal is unique");
                consumed = true;
            }
            ingress
                .mark_leader_wire_volatile_terminal(&retired)
                .expect("release consumed wire");
        }
        if consumed {
            break;
        }
    }
    assert!(
        consumed,
        "original peer Commit must retry successfully after exact lock/signature"
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!service.output_guard.restart_required());
}
