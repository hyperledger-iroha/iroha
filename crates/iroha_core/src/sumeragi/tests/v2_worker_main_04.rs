#[test]
fn restored_valid_qc_without_local_retention_authority_is_negative() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    for (case, local_validator, signers) in [
        ("observer", None, vec![0, 1, 2, 3]),
        ("non-signer", Some(0), vec![1, 2, 3]),
    ] {
        let (request, validator_pops) = production_authenticated_serve_request(
            &context,
            &keys,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
            &signers,
        );
        let body_root = TempDir::new().expect("nonowner startup body root");
        let serve_root = TempDir::new().expect("nonowner startup Serve root");
        let body_store = V2BodyStore::open(body_root.path(), context.clone())
            .expect("open empty nonowner body store");
        let lifecycle_id = persist_unsealed_serve_fixture(
            serve_root.path(),
            &context,
            &request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            Some(3),
        );
        let (command_tx, _command_rx, _admission) = production_persistent_test_io_command_channel(
            4,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            local_validator,
            None,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .unwrap_or_else(|error| panic!("{case} startup discharge failed: {error}"));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&lifecycle_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Rejected(
                CertifiedServeNegativeOutcome::LocalRetentionAuthorityAbsent
            )),
            "{case} must deterministically negative-terminalize the lifecycle"
        );
    }
}
#[test]
fn restored_negative_outcome_tags_must_match_reconstructed_authority() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let (valid, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let invalid = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let foreign_decision = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"foreign durable Decision tag mutation",
        )),
        ..proposal.subject
    };
    let other_decision = wire::BlockSubject {
        payload_hash: Hash::new(b"other durable Decision tag mutation"),
        ..foreign_decision
    };
    let body_root = TempDir::new().expect("negative-tag body root");
    let body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open empty body store");
    for (case, request, outcome, durable_decided_subject) in [
        (
            "invalid certificate tagged as missing authority",
            &invalid,
            CertifiedServeNegativeOutcome::LocalRetentionAuthorityAbsent,
            None,
        ),
        (
            "valid certificate tagged as invalid",
            &valid,
            CertifiedServeNegativeOutcome::InvalidCertificate,
            None,
        ),
        (
            "local signer tagged as missing authority",
            &valid,
            CertifiedServeNegativeOutcome::LocalRetentionAuthorityAbsent,
            None,
        ),
        (
            "supersession tag without Decision",
            &valid,
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(foreign_decision),
            None,
        ),
        (
            "supersession tag equal to request subject",
            &valid,
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(proposal.subject),
            Some(proposal.subject),
        ),
        (
            "supersession tag names another Decision",
            &valid,
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(foreign_decision),
            Some(other_decision),
        ),
    ] {
        let serve_root = TempDir::new().expect("negative-tag Serve root");
        persist_negative_serve_fixture(
            serve_root.path(),
            &context,
            request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            outcome,
        );
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let before = fs::read(&state_path).expect("read mutated negative snapshot");
        let result = production_persistent_test_io_command_channel(
            4,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            durable_decided_subject,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        );
        let Err(error) = result else {
            panic!("{case} must fail source-bound negative validation")
        };
        assert!(
            error.contains(
                "durable negative Serve tombstone does not match its reconstructed authority"
            ),
            "{case} produced an unexpected error: {error}"
        );
        assert_eq!(
            fs::read(&state_path).expect("reload rejected negative tag"),
            before,
            "{case} cannot mutate durable Serve state"
        );
    }
}
#[test]
fn strict_higher_view_replaces_negative_without_resurrecting_exact_request() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, _, proposal) = proposal_body_and_payload(&context, &keys);
    let invalid = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let higher_round = wire::ConsensusRound {
        view: proposal.round.view + 1,
        ..proposal.round
    };
    let (higher, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        higher_round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let _ = fully_authenticate_persisted_certified_serve_request(
        &context,
        higher.request().clone(),
        &validator_pops,
    )
    .expect("strict higher fixture has a valid production QC");
    let body_root = TempDir::new().expect("negative successor body root");
    let serve_root = TempDir::new().expect("negative successor Serve root");
    let (higher_manifest, _) =
        encode_payload(&context, higher_round, proposal.subject, &canonical_wire)
            .expect("encode strict higher canonical body")
            .into_parts();
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let _ = body_store
        .store(higher_manifest.clone(), canonical_wire.clone())
        .expect("retain strict higher canonical body");
    let response = certified_serve_response(&higher, higher_manifest, canonical_wire, &keys[0]);
    let negative_id = persist_unsealed_serve_fixture(
        serve_root.path(),
        &context,
        &invalid,
        CertifiedServeOwnerKey::Roster(invalid.request().requester.clone()),
        1,
        Some(4),
    );
    let (command_tx, command_rx, _admission) = production_persistent_test_io_command_channel(
        4,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    )
    .expect("install initial invalid-QC negative");
    assert_eq!(
        command_tx
            .queue
            .lock()
            .serves
            .get(&negative_id)
            .map(|tracked| tracked.state),
        Some(V2IoServeState::Rejected(
            CertifiedServeNegativeOutcome::InvalidCertificate
        ))
    );
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            invalid.request(),
            context.roster[3].validator.clone(),
        )),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    assert_eq!(
        command_tx
            .queue
            .lock()
            .next_serve_ingress_reservation_ordinal,
        4
    );
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            higher.request(),
            context.roster[3].validator.clone(),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect strict negative successor")
        .expect("strict higher view owns one fresh occurrence");
    assert_eq!(barrier.scheduler_ordinal(), 5);
    assert_eq!(barrier.lifecycle_id().admission_ordinal, 2);
    assert_ne!(barrier.lifecycle_id(), negative_id);
    {
        let state = command_tx.queue.lock();
        assert!(!state.serves.contains_key(&negative_id));
        assert_eq!(
            state
                .serve_by_family
                .get(&CertifiedServeFamilyKey {
                    requester: higher.request().requester.clone(),
                    phase: higher.request().certificate.phase,
                })
                .copied(),
            Some(barrier.lifecycle_id())
        );
    }
    let (admission, committed) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(higher.request().requester.clone()),
        &higher,
    );
    assert_eq!(admission.lifecycle_id, barrier.lifecycle_id());
    assert!(matches!(committed, CertifiedServeCommit::Queued));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == admission.lifecycle_id
    ));
    command_rx
        .complete_serve_response(admission.lifecycle_id, &response)
        .expect("seal strict higher response");
    command_tx
        .acknowledge_serve_completion(
            admission.lifecycle_id,
            V2IoServeTerminal::Response(response),
        )
        .expect("acknowledge strict higher response");
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire strict negative-successor gate");
    drop(ingress);
    drop(command_rx);
    drop(command_tx);
    let (restart_tx, _restart_rx, _admission) = production_persistent_test_io_command_channel(
        4,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    )
    .expect("restart validates the strict higher terminal");
    assert_eq!(
        restart_tx
            .queue
            .lock()
            .serves
            .get(&barrier.lifecycle_id())
            .map(|tracked| tracked.state),
        Some(V2IoServeState::Terminal)
    );
    let (restart_ingress, restart_gate) = gated_fair_ingress(&context, &restart_tx);
    let actor_ordinal_before_old_retry =
        restart_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    assert!(matches!(
        restart_ingress.try_push(certified_serve_inbound(
            invalid.request(),
            context.roster[3].validator.clone(),
        )),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    assert_eq!(
        restart_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_before_old_retry,
        "the displaced lower negative cannot resurrect after successor restart"
    );
    assert!(matches!(
        restart_ingress.try_push(certified_serve_inbound(
            higher.request(),
            context.roster[3].validator.clone(),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        restart_tx
            .serve_barrier()
            .expect("inspect restarted higher replay")
            .expect("higher terminal replay owns one fresh occurrence")
            .lifecycle_id(),
        barrier.lifecycle_id()
    );
    restart_ingress.close();
    restart_ingress
        .unbind_certified_serve_gate(&restart_gate)
        .expect("retire restarted strict higher gate");
}
#[test]
fn terminal_response_and_raw_restart_are_superseded_by_durable_decision() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let _ = fully_authenticate_persisted_certified_serve_request(
        &context,
        request.request().clone(),
        &validator_pops,
    )
    .expect("supersession fixture request has a valid production QC");
    let decided_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"durable Decision superseding retained Serve response",
        )),
        payload_hash: Hash::new(b"durable Decision exact payload"),
        ..proposal.subject
    };
    assert_ne!(decided_subject, proposal.subject);
    let body_root = TempDir::new().expect("superseded response body root");
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire.clone())
        .expect("retain canonical body for response reconstruction");
    let response = certified_serve_response(
        &request,
        payload.manifest().clone(),
        canonical_wire,
        &keys[0],
    );
    let live_serve_root = TempDir::new().expect("live supersession Serve root");
    let live_lifecycle_id = {
        let (command_tx, command_rx, _admission) =
            persistent_test_io_command_channel(4, live_serve_root.path(), &context, &body_store)
                .expect("open live supersession queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let via = context.roster[3].validator.clone();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            &request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == admission.lifecycle_id
        ));
        command_rx
            .complete_serve_response(admission.lifecycle_id, &response)
            .expect("seal the exact certified-body response");
        command_tx
            .acknowledge_serve_completion(
                admission.lifecycle_id,
                V2IoServeTerminal::Response(response.clone()),
            )
            .expect("acknowledge the exact certified-body response");
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&admission.lifecycle_id)
                .expect("terminal response remains indexed");
            assert_eq!(tracked.state, V2IoServeState::Terminal);
            assert!(tracked.terminal.is_some());
            assert!(tracked.reply_routes.is_some());
            assert!(tracked.ingress_ownership.is_some());
        }
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let superseding_occurrence = command_tx
            .serve_barrier()
            .expect("inspect live supersession barrier")
            .expect("terminal replay owns one physical occurrence");
        assert_eq!(
            superseding_occurrence.lifecycle_id(),
            admission.lifecycle_id
        );
        let actor_ordinal_before_decision =
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        let (ingress_highwater_before_decision, lifecycle_highwater_before_decision) = {
            let state = command_tx.queue.lock();
            (
                state.next_serve_ingress_reservation_ordinal,
                state.next_serve_admission_ordinal,
            )
        };
        command_tx
            .begin_decision_serve_reconciliation()
            .expect("fence Serve admission before the durable Decision step");
        command_tx
            .finish_decision_serve_reconciliation(Some(decided_subject))
            .expect("publish the durable Decision under the Serve queue lock");
        {
            let state = command_tx.queue.lock();
            let retained = state
                .serves
                .get(&admission.lifecycle_id)
                .expect("pre-fence terminal replay retains its logical lifecycle");
            assert_eq!(retained.state, V2IoServeState::Terminal);
            assert_eq!(
                retained.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone()))
            );
            assert_eq!(
                state
                    .serve_ingress_reservation
                    .as_ref()
                    .map(|reservation| reservation.lifecycle_id),
                Some(admission.lifecycle_id),
                "Decision publication must not detach or eagerly rewrite a pre-fence carrier"
            );
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before_decision
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before_decision
            );
        }
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before_decision
        );
        let fair_before_supersession = fair_ingress_accounting_snapshot(&ingress);
        let state_path = live_serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let durable_before_supersession =
            fs::read(&state_path).expect("read terminal response before supersession");
        let temporary_state = state_path.with_extension("norito.tmp");
        let error = ingress
            .try_recv_if_checked(|inbound| {
                let selected = matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload:
                            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                        ..
                    }) if HashOf::new(candidate) == request.request_hash()
                );
                if selected {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("stage Decision supersession before fault injection");
                    fs::create_dir(&temporary_state)
                        .expect("block atomic supersession publication");
                }
                selected
            })
            .expect_err("failed supersession publication retains the terminal replay");
        assert!(
            error.contains("failed to create Sumeragi v2 Serve temporary state"),
            "unexpected supersession publication error: {error}"
        );
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            fair_before_supersession,
            "failed supersession publication retains the exact fair carrier"
        );
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&admission.lifecycle_id)
                .expect("failed supersession retains the response lifecycle");
            assert_eq!(tracked.state, V2IoServeState::Terminal);
            assert_eq!(
                tracked.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone()))
            );
            assert!(
                tracked.reply_routes.is_some() && tracked.ingress_ownership.is_some(),
                "failed supersession restores response transport ownership"
            );
            assert_eq!(
                state
                    .serve_ingress_reservation
                    .as_ref()
                    .map(|reservation| reservation.state),
                Some(
                    CertifiedServeIngressReservationState::DeterministicallyRejected(
                        CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject,),
                    ),
                )
            );
        }
        assert_eq!(
            fs::read(&state_path).expect("reload failed supersession snapshot"),
            durable_before_supersession,
            "failed supersession cannot alter the terminal durable snapshot"
        );
        fs::remove_dir(&temporary_state).expect("unblock supersession publication");
        let drained = ingress
            .try_recv_if_checked(|inbound| {
                let selected = matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload:
                            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                        ..
                    }) if HashOf::new(candidate) == request.request_hash()
                );
                if selected {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("stage the fully authenticated Decision supersession");
                }
                selected
            })
            .expect("publish live Decision supersession")
            .expect("drain live superseded replay");
        drop(drained);
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&admission.lifecycle_id)
                .expect("negative terminal remains indexed");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject,),
                )
            );
            assert!(tracked.terminal.is_none());
            assert!(
                tracked.reply_routes.is_none() && tracked.ingress_ownership.is_none(),
                "Decision supersession retires stale response transport ownership"
            );
            assert!(state.serve_ingress_reservation.is_none());
        }
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("live supersession retains a durable store")
            .load(&context)
            .expect("reload live supersession");
        assert!(persisted.terminal_tombstones.is_empty());
        assert_eq!(
            persisted
                .negative_tombstones
                .iter()
                .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                .collect::<Vec<_>>(),
            vec![(
                admission.lifecycle_id,
                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
            )]
        );
        let actor_ordinal_after_negative =
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_after_negative
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire live supersession gate");
        admission.lifecycle_id
    };
    let (live_restart_tx, _live_restart_rx, _admission) =
        production_persistent_test_io_command_channel(
            4,
            live_serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(decided_subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("restart validates the live supersession outcome");
    assert_eq!(
        live_restart_tx
            .queue
            .lock()
            .serves
            .get(&live_lifecycle_id)
            .map(|tracked| tracked.state),
        Some(V2IoServeState::Rejected(
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
        ))
    );
    let terminal_serve_root = TempDir::new().expect("startup terminal supersession root");
    let terminal_lifecycle_id = persist_terminal_serve_fixture(
        terminal_serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        &response,
    );
    let (terminal_restart_tx, _terminal_restart_rx, _admission) =
        production_persistent_test_io_command_channel(
            4,
            terminal_serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(decided_subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("startup converts a superseded response before producer exposure");
    assert_eq!(
        terminal_restart_tx
            .queue
            .lock()
            .serves
            .get(&terminal_lifecycle_id)
            .map(|tracked| tracked.state),
        Some(V2IoServeState::Rejected(
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
        ))
    );
    let raw_serve_root = TempDir::new().expect("post-Decision raw admission root");
    let raw_lifecycle_id = persist_unsealed_serve_fixture(
        raw_serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        Some(9),
    );
    let (raw_restart_tx, _raw_restart_rx, _admission) =
        production_persistent_test_io_command_channel(
            4,
            raw_serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(decided_subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("post-Decision raw admission is terminalized during startup");
    {
        let state = raw_restart_tx.queue.lock();
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.serve_ingress_reservation.is_none());
        assert_eq!(
            state
                .serves
                .get(&raw_lifecycle_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Rejected(
                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
            ))
        );
    }
}
#[test]
fn missing_or_retargeted_reply_capability_is_rejected_before_serve_admission() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let request = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = context.roster[0].validator.clone();
    let body_root = TempDir::new().expect("route-less Serve body root");
    let serve_root = TempDir::new().expect("route-less Serve state root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
            .expect("open route-less Serve queue");
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.request().clone()),
    ));
    for _ in 0..2 {
        let without_route = InboundBlockMessage::from_transport(
            message.clone(),
            request.request().requester.clone(),
            via.clone(),
        );
        assert!(matches!(
            ingress.try_push(without_route),
            Err(FairV2IngressPushError::Rejected(_))
        ));
    }
    let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
    let wrong_target = context.roster[2].validator.clone();
    let wrong_route = routes.mint_via(wrong_target, via.clone());
    let retargeted = InboundBlockMessage {
        message,
        sender: Some(request.request().requester.clone()),
        via: Some(via),
        reply_routes: Some(
            NetworkReplyRoutes::try_from_route(wrong_route)
                .expect("wrong-target fixture still forms a bounded route set"),
        ),
        ingress_ownership: None,
    };
    assert!(matches!(
        ingress.try_push(retargeted),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    {
        let state = command_tx.queue.lock();
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
        assert_eq!(state.next_serve_admission_ordinal, 0);
        assert!(state.serves.is_empty());
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.serve_ingress_reservation.is_none());
    }
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_before,
        "missing or retargeted reply capabilities are rejected before the actor-global ordinal source"
    );
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("route-less fixture has durable Serve store")
        .load(&context)
        .expect("reload route-less snapshot");
    assert!(persisted.ingress_waiters.is_empty());
    assert!(persisted.unsealed_lifecycles.is_empty());
    assert!(persisted.negative_tombstones.is_empty());
    assert!(persisted.terminal_tombstones.is_empty());
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire route-less Serve gate");
}
#[test]
fn invalid_requester_signed_qc_quarantines_one_family_without_consuming_honest_capacity() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let invalid = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let mut honest_request = authenticated_serve_request(
        &context,
        &keys[2],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Commit,
    )
    .request()
    .clone();
    let validator_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator proof of possession")
        })
        .collect::<Vec<_>>();
    let honest_signers = (0..context.roster.len())
        .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
        .collect::<Vec<_>>();
    let honest_vote_preimage = wire::Vote {
        round: honest_request.certificate.round,
        proposal_round: honest_request.certificate.proposal_round,
        phase: honest_request.certificate.phase,
        subject: honest_request.certificate.subject,
        execution_commitment: honest_request.certificate.execution_commitment,
        signer: honest_signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let honest_signature_shares = honest_signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("fixture signer index fits usize")]
                    .private_key(),
                &honest_vote_preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let honest_signature_share_refs = honest_signature_shares
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    honest_request.certificate.signers = honest_signers;
    honest_request.certificate.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&honest_signature_share_refs)
            .expect("aggregate honest Serve certificate");
    honest_request.signature =
        Signature::new(keys[2].private_key(), &honest_request.signature_preimage())
            .payload()
            .to_vec();
    let honest = authenticate_certified_body_request(
        &context,
        honest_request,
        &PeerId::new(keys[2].public_key().clone()),
        |context, certificate| {
            wire::finality::verify_quorum_certificate_with_validator_pops(
                context,
                certificate,
                &validator_pops,
            )
        },
    )
    .expect("production verification accepts the distinct honest Serve QC");
    assert!(
        authenticate_certified_body_request(
            &context,
            invalid.request().clone(),
            &invalid.request().requester,
            |context, certificate| {
                wire::finality::verify_quorum_certificate_with_validator_pops(
                    context,
                    certificate,
                    &validator_pops,
                )
            },
        )
        .is_err(),
        "the requester signature is valid, but production QC verification must reject the fixture certificate"
    );
    let via = context.roster[0].validator.clone();
    let retry_via = context.roster[3].validator.clone();
    let body_root = TempDir::new().expect("invalid-QC quarantine body root");
    let serve_root = TempDir::new().expect("invalid-QC quarantine Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let invalid_lifecycle = {
        let (command_tx, command_rx, _admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("open invalid-QC quarantine queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(invalid.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let invalid_lifecycle = command_tx
            .serve_barrier()
            .expect("inspect invalid-QC barrier")
            .expect("raw invalid-QC admission owns one physical occurrence")
            .lifecycle_id();
        assert_eq!(invalid_lifecycle.admission_ordinal, 1);
        let mut reject_invalid_qc = |inbound: &InboundBlockMessage| {
            let BlockMessage::V2(message) = inbound.message() else {
                panic!("invalid-QC fixture changed transport message class")
            };
            let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload
            else {
                panic!("invalid-QC fixture changed v2 payload class")
            };
            assert_eq!(request, invalid.request());
            let requester = inbound
                .sender()
                .expect("invalid-QC fixture retains its authenticated requester");
            assert_eq!(requester, &request.requester);
            assert!(
                authenticate_certified_body_request(
                    &context,
                    request.clone(),
                    requester,
                    |context, certificate| {
                        wire::finality::verify_quorum_certificate_with_validator_pops(
                            context,
                            certificate,
                            &validator_pops,
                        )
                    },
                )
                .is_err(),
                "checked dequeue must observe production QC rejection before draining"
            );
            command_tx
                .stage_selected_serve_rejection(
                    invalid.request_hash(),
                    CertifiedServeNegativeOutcome::InvalidCertificate,
                )
                .expect("stage invalid-QC outcome before checked physical drain");
            true
        };
        let rejected = ingress
            .try_recv_if_checked(&mut reject_invalid_qc)
            .expect("publish runner-side invalid-QC rejection")
            .expect("drain the rejected invalid-QC carrier");
        drop(rejected);
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&invalid_lifecycle)
                .expect("invalid family retains one durable quarantine owner");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(CertifiedServeNegativeOutcome::InvalidCertificate)
            );
            assert!(tracked.reply_routes.is_none());
            assert!(tracked.ingress_ownership.is_none());
            assert!(tracked.terminal.is_none());
            assert!(state.serve_barrier.is_none());
            assert!(state.serve_barrier_predecessors.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(state.pending_serve_requests.is_empty());
            assert!(state.serve_replacements.is_empty());
            assert!(state.commands.is_empty());
            assert_eq!(state.serves.len(), 1);
            assert_eq!(state.serve_by_request.len(), 1);
            assert_eq!(state.serve_by_family.len(), 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
        }
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        for _ in 0..2 {
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(
                    invalid.request(),
                    retry_via.clone()
                )),
                Err(FairV2IngressPushError::Rejected(_))
            ));
            assert_eq!(
                command_tx
                    .queue
                    .lock()
                    .next_serve_ingress_reservation_ordinal,
                1,
                "negative retries cannot mint another physical occurrence"
            );
        }
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire invalid-QC quarantine gate");
        invalid_lifecycle
    };
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
            .expect("restart invalid-QC quarantine queue");
    {
        let state = command_tx.queue.lock();
        let tracked = state
            .serves
            .get(&invalid_lifecycle)
            .expect("restart retains the exact invalid family owner");
        assert_eq!(
            tracked.state,
            V2IoServeState::Rejected(CertifiedServeNegativeOutcome::InvalidCertificate)
        );
        assert!(tracked.reply_routes.is_none());
        assert!(tracked.ingress_ownership.is_none());
        assert!(tracked.terminal.is_none());
        assert_eq!(state.serves.len(), 1);
        assert!(state.serve_barrier.is_none());
        assert!(state.serve_ingress_reservation.is_none());
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.pending_serve_requests.is_empty());
        assert!(state.commands.is_empty());
        assert_eq!(state.next_serve_admission_ordinal, 1);
    }
    assert!(matches!(
        command_tx.try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(8_002),
                subject: proposal.subject,
            },
        ),
        Ok(())
    ));
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(honest.request(), via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let honest_lifecycle = command_tx
        .serve_barrier()
        .expect("inspect distinct honest Serve barrier")
        .expect("distinct honest family retains physical Serve capacity")
        .lifecycle_id();
    assert_ne!(honest_lifecycle, invalid_lifecycle);
    {
        let state = command_tx.queue.lock();
        assert_eq!(state.serves.len(), 2);
        assert_eq!(state.serve_by_request.len(), 2);
        assert_eq!(state.serve_by_family.len(), 2);
        assert_eq!(state.next_serve_admission_ordinal, 2);
        assert_eq!(
            state
                .serves
                .get(&invalid_lifecycle)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Rejected(
                CertifiedServeNegativeOutcome::InvalidCertificate
            ))
        );
        assert_eq!(
            state
                .serves
                .get(&honest_lifecycle)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::AwaitingRetry)
        );
        assert!(
            state.commands.iter().any(|command| matches!(
                command,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(8_002),
                    subject,
                } if *subject == proposal.subject
            )),
            "the quarantined Byzantine family cannot consume ordinary honest I/O capacity"
        );
    }
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire honest-capacity quarantine gate");
}
#[test]
fn durable_raw_admission_restart_locally_seals_before_later_producers() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let higher = authenticated_serve_request(
        &context,
        &keys[1],
        wire::ConsensusRound {
            view: proposal.round.view + 1,
            ..proposal.round
        },
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let requester = request.request().requester.clone();
    let via = context.roster[0].validator.clone();
    let replay_via = context.roster[3].validator.clone();
    let body_root = TempDir::new().expect("raw-admission restart body root");
    let serve_root = TempDir::new().expect("raw-admission restart Serve root");
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("retain raw-admission restart body");
    let (scheduler_ordinal, lifecycle_id) = {
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("open pre-crash raw-admission queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let barrier = command_tx
            .serve_barrier()
            .expect("inspect raw-admission barrier")
            .expect("raw exact admission owns its barrier");
        assert_eq!(barrier.scheduler_ordinal(), 1);
        assert_eq!(barrier.lifecycle_id().admission_ordinal, 1);
        let before_higher = fair_ingress_accounting_snapshot(&ingress);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(higher.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            before_higher,
            "a later family view cannot replace an admitted exact raw target"
        );
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
            assert_eq!(
                state
                    .serves
                    .get(&barrier.lifecycle_id())
                    .map(|tracked| tracked.state),
                Some(V2IoServeState::AwaitingRetry)
            );
        }
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("fixture retains durable Serve state")
            .load(&context)
            .expect("reload raw-admission snapshot");
        assert_eq!(
            persisted
                .ingress_waiters
                .iter()
                .map(|waiter| (waiter.ingress_ordinal, waiter.lifecycle_id))
                .collect::<Vec<_>>(),
            vec![(barrier.scheduler_ordinal(), barrier.lifecycle_id())]
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("detach pre-crash raw carrier");
        (barrier.scheduler_ordinal(), barrier.lifecycle_id())
    };
    let (command_tx, command_rx, _admission) = production_persistent_test_io_command_channel(
        2,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    )
    .expect("restart locally discharges raw admission");
    {
        let state = command_tx.queue.lock();
        assert!(
            state.serve_ingress_waiters.is_empty(),
            "startup discharge retires every requester-dependent scheduler ticket"
        );
        assert_eq!(
            state.next_serve_ingress_reservation_ordinal,
            scheduler_ordinal
        );
        assert_eq!(
            state.next_serve_admission_ordinal,
            lifecycle_id.admission_ordinal
        );
        assert_eq!(
            state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
    }
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert_eq!(
        gate.dormant_ingress_scheduler_ordinal()
            .expect("inspect discharged durable admission debt"),
        None
    );
    drop(
        command_tx
            .try_begin_producer_episode()
            .expect("inspect producer admission after startup discharge")
            .expect("startup discharge exposes later finite producers"),
    );
    assert!(command_tx.queue.can_enqueue_as(V2IoAdmissionClass::Control));
    assert!(matches!(
        command_tx.try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(8_001),
                subject: proposal.subject,
            },
        ),
        Ok(())
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(8_001),
            ..
        })
    ));
    let unrelated = InboundBlockMessage::from_transport(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                wire::CommitCertificateRequest {
                    protocol_version: wire::PROTOCOL_VERSION,
                    network_id: context.network_id,
                    context_id: context.id(),
                    height: context.height.saturating_add(1),
                    requester: replay_via.clone(),
                    signature: vec![0x5A],
                },
            ),
        )),
        replay_via.clone(),
        replay_via.clone(),
    );
    assert!(matches!(
        ingress.try_push(unrelated),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(
        ingress.try_recv_if(|_| true).is_some(),
        "unrelated traffic drains after local startup service"
    );
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            request.request(),
            replay_via.clone()
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let restored_barrier = command_tx
        .serve_barrier()
        .expect("inspect restored raw barrier")
        .expect("exact retransmission owns a fresh replay occurrence");
    assert!(restored_barrier.scheduler_ordinal() > scheduler_ordinal);
    assert_eq!(restored_barrier.lifecycle_id(), lifecycle_id);
    assert_eq!(
        gate.dormant_ingress_scheduler_ordinal()
            .expect("exact head retry clears physical admission debt"),
        None
    );
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            request.request(),
            replay_via.clone()
        )),
        Ok(FairV2IngressPushDisposition::Coalesced)
    ));
    assert_eq!(
        command_tx
            .serve_barrier()
            .expect("inspect coalesced restored barrier"),
        Some(restored_barrier)
    );
    let (admission, committed) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester),
        &request,
    );
    assert_eq!(admission.lifecycle_id, lifecycle_id);
    assert!(matches!(committed, CertifiedServeCommit::Replay { .. }));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), replay_via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let post_drain = command_tx
        .serve_barrier()
        .expect("inspect post-drain exact replay")
        .expect("post-drain replay owns a fresh scheduler ticket");
    assert!(post_drain.scheduler_ordinal() > scheduler_ordinal);
    assert_eq!(post_drain.lifecycle_id(), lifecycle_id);
    assert_eq!(
        command_tx.queue.lock().next_serve_admission_ordinal,
        lifecycle_id.admission_ordinal,
        "physical replay cannot mint another logical Serve lifecycle"
    );
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire raw-admission restart fixture gate");
}
#[test]
fn restored_observer_waiter_is_locally_serviced_without_requester_fairness() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let observer = KeyPair::random();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &observer,
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let authenticated_source = PeerId::new(KeyPair::random().public_key().clone());
    let body_root = TempDir::new().expect("observer-waiter body root");
    let serve_root = TempDir::new().expect("observer-waiter Serve root");
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("retain observer startup-discharge body");
    let (scheduler_ordinal, lifecycle_id) = {
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("open pre-crash observer queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                request.request(),
                authenticated_source.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let barrier = command_tx
            .serve_barrier()
            .expect("inspect observer raw barrier")
            .expect("observer raw admission owns a scheduler ticket");
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("fixture retains durable Serve state")
            .load(&context)
            .expect("reload observer raw-admission snapshot");
        assert_eq!(persisted.ingress_waiters.len(), 1);
        assert!(matches!(
            &persisted.ingress_waiters[0].owner,
            CertifiedServeOwnerKey::AuthenticatedSource(source)
                if source == &authenticated_source
        ));
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("detach pre-crash observer carrier");
        (barrier.scheduler_ordinal(), barrier.lifecycle_id())
    };
    let (command_tx, command_rx, _admission) = production_persistent_test_io_command_channel(
        2,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    )
    .expect("restore and locally service observer raw admission");
    {
        let state = command_tx.queue.lock();
        assert!(
            state.serve_ingress_waiters.is_empty(),
            "a non-responsive observer cannot retain global scheduler debt after restore"
        );
        assert_eq!(
            state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal),
            "observer scheduler retirement must locally finish the exact lifecycle"
        );
        assert_eq!(
            state.next_serve_ingress_reservation_ordinal, scheduler_ordinal,
            "retired observer ordinals remain consumed"
        );
    }
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("restored fixture retains durable Serve state")
        .load(&context)
        .expect("reload locally retired observer snapshot");
    assert!(
        persisted.ingress_waiters.is_empty(),
        "observer scheduler retirement is durable across another crash"
    );
    assert!(
        persisted.unsealed_lifecycles.is_empty()
            && persisted
                .terminal_tombstones
                .iter()
                .any(|lifecycle| lifecycle.lifecycle_id == lifecycle_id)
    );
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert_eq!(
        gate.dormant_ingress_scheduler_ordinal()
            .expect("inspect restored observer scheduler debt"),
        None
    );
    let producer_episode = command_tx
        .try_begin_producer_episode()
        .expect("inspect producer admission after observer restore")
        .expect("external restored debt cannot block local producers");
    drop(producer_episode);
    assert!(command_tx.queue.can_enqueue_as(V2IoAdmissionClass::Control));
    command_tx
        .try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(8_101),
                subject: proposal.subject,
            },
        )
        .expect("local control work proceeds without observer retransmission");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(8_101),
            ..
        })
    ));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            request.request(),
            authenticated_source.clone(),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let retry = command_tx
        .serve_barrier()
        .expect("inspect post-restore observer retry")
        .expect("exact observer retry takes a new scheduler position");
    assert_eq!(retry.lifecycle_id(), lifecycle_id);
    assert!(retry.scheduler_ordinal() > scheduler_ordinal);
    let (admission, replay) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::AuthenticatedSource(authenticated_source),
        &request,
    );
    assert_eq!(admission.lifecycle_id, lifecycle_id);
    assert!(matches!(replay, CertifiedServeCommit::Replay { .. }));
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire observer restore fixture gate");
}
#[test]
fn durable_raw_waiter_rejects_mutated_logical_lineage() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let request = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = context.roster[0].validator.clone();
    let body_root = TempDir::new().expect("raw-lineage mutation body root");
    let serve_root = TempDir::new().expect("raw-lineage mutation Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    {
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("open raw-lineage queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("detach raw-lineage carrier");
    }
    let family_capacity = certified_serve_family_capacity(context.roster.len(), 2, 2)
        .expect("raw-lineage family capacity");
    let (store, mut persisted) =
        CertifiedServeStateStore::open(serve_root.path(), &context, family_capacity)
            .expect("open raw-lineage durable state");
    let waiter = persisted
        .ingress_waiters
        .first_mut()
        .expect("raw admission persists one waiter");
    waiter.lifecycle_id = CertifiedServeLifecycleId {
        admission_ordinal: 2,
        request_hash: request.request_hash(),
    };
    persisted.next_lifecycle_admission_ordinal = 2;
    store
        .persist(&persisted)
        .expect("publish canonically checksummed lineage mutation");
    let error =
        match persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store) {
            Ok(_) => panic!("mutated raw lifecycle lineage must fail closed"),
            Err(error) => error,
        };
    assert!(
        error.contains("atomically admitted logical lifecycle"),
        "unexpected raw-lineage rejection: {error}"
    );
}
#[test]
fn certified_serve_abort_mismatch_preserves_logical_and_physical_handoff() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let requester = request.request().requester.clone();
    let via = service.context.roster[0].validator.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
    let route = routes.mint_via(requester.clone(), via);
    let (command_tx, _command_rx, _admission) = test_io_command_channel(2);
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            service.context.roster[0].validator.clone(),
            route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let mut admission = None;
    let inbound = ingress
        .try_recv_if_checked(|_| {
            admission = Some(
                command_tx
                    .prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(requester.clone()),
                        request.clone(),
                    )
                    .expect("prepare abort-mismatch handoff"),
            );
            true
        })
        .expect("publish abort-mismatch physical drain")
        .expect("drain abort-mismatch carrier");
    drop(inbound);
    let admission = admission.expect("retain abort-mismatch admission");
    let reservation_id = admission
        .ingress_reservation_id
        .expect("gated admission retains its physical reservation");
    let snapshot = {
        let state = command_tx.queue.lock();
        (
            state.serve_barrier,
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id, reservation.state)),
            state
                .serves
                .get(&admission.lifecycle_id)
                .map(|serve| serve.state),
            state
                .commands
                .iter()
                .map(V2IoCommand::serve_lifecycle_id)
                .collect::<Vec<_>>(),
            state.next_serve_admission_ordinal,
            state.next_serve_ingress_reservation_ordinal,
        )
    };
    let mismatched = CertifiedServeAdmission {
        lifecycle_id: admission.lifecycle_id,
        kind: admission.kind,
        request: admission.request.clone(),
        ingress_reservation_id: Some(CertifiedServeIngressReservationId(
            reservation_id
                .0
                .checked_add(1)
                .expect("fixture reservation has a successor"),
        )),
    };
    assert!(
        command_tx
            .abort_serve(mismatched)
            .expect_err("mismatched physical abort must fail closed")
            .contains("changed its physically drained ingress handoff")
    );
    let after = {
        let state = command_tx.queue.lock();
        (
            state.serve_barrier,
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id, reservation.state)),
            state
                .serves
                .get(&admission.lifecycle_id)
                .map(|serve| serve.state),
            state
                .commands
                .iter()
                .map(V2IoCommand::serve_lifecycle_id)
                .collect::<Vec<_>>(),
            state.next_serve_admission_ordinal,
            state.next_serve_ingress_reservation_ordinal,
        )
    };
    assert_eq!(
        after, snapshot,
        "abort mismatch cannot mutate either side of the exact handoff"
    );
    command_tx
        .abort_serve(admission)
        .expect("valid abort retires the exact physical handoff");
    {
        let state = command_tx.queue.lock();
        assert!(state.serve_barrier.is_none());
        assert!(state.serve_ingress_reservation.is_none());
        assert!(state.commands.is_empty());
    }
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire abort-mismatch fixture gate");
}
#[test]
fn certified_serve_abort_error_preserves_primary_and_rollback_failures() {
    let primary = "primary Serve failure".to_owned();
    assert_eq!(
        combine_certified_serve_abort_error(primary.clone(), Ok(())),
        primary
    );
    let combined = combine_certified_serve_abort_error(
        primary.clone(),
        Err("durable rollback failure".to_owned()),
    );
    assert!(combined.starts_with(&primary));
    assert!(combined.contains("durable rollback failure"));
}
#[test]
fn raw_admission_persistence_failure_rolls_back_logical_lineage() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let request = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = context.roster[0].validator.clone();
    let body_root = TempDir::new().expect("raw-admission failure body root");
    let serve_root = TempDir::new().expect("raw-admission failure Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
            .expect("open raw-admission failure queue");
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    let temporary_state = serve_root
        .path()
        .join(CERTIFIED_SERVE_STATE_FILE)
        .with_extension("norito.tmp");
    fs::create_dir(&temporary_state).expect("block atomic raw-admission temporary file");
    let before = fair_ingress_accounting_snapshot(&ingress);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Err(FairV2IngressPushError::Closed(_))
    ));
    let after = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(after.last_admission_ordinal, before.last_admission_ordinal);
    assert_eq!(after.len, before.len);
    assert_eq!(after.bytes, before.bytes);
    assert!(!after.open);
    {
        let state = command_tx.queue.lock();
        assert!(!state.sender_open);
        assert!(state.receiver_open);
        assert!(state.serve_ingress_reservation.is_none());
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.serves.is_empty());
        assert!(state.serve_by_request.is_empty());
        assert!(state.serve_by_family.is_empty());
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
        assert_eq!(state.next_serve_admission_ordinal, 0);
        assert_eq!(
            command_tx
                .queue
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect fail-closed shared scheduler source"),
            Some(2),
            "failed persistence may consume a scheduler candidate, but the closed queue cannot reuse it"
        );
    }
    fs::remove_dir(&temporary_state).expect("unblock raw-admission state path");
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("detach failed raw-admission gate");
}
#[test]
fn fair_ingress_gate_overflow_closes_without_partial_admission() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = service.context.roster[0].validator.clone();
    let (command_tx, _command_rx, _admission) = test_io_command_channel(2);
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    {
        let mut state = command_tx.queue.lock();
        state.next_serve_ingress_reservation_ordinal = u128::MAX;
    }
    command_tx.queue.lifecycle_ordinals.exhaust_for_test();
    let before = fair_ingress_accounting_snapshot(&ingress);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
        Err(FairV2IngressPushError::Closed(_))
    ));
    let after = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(after.last_admission_ordinal, before.last_admission_ordinal);
    assert_eq!(after.ready, before.ready);
    assert_eq!(after.pending_wire_owners, before.pending_wire_owners);
    assert_eq!(after.lanes, before.lanes);
    assert_eq!(after.len, before.len);
    assert_eq!(after.bytes, before.bytes);
    assert!(!after.open, "closed Serve owner fail-closes fair ingress");
    {
        let state = command_tx.queue.lock();
        assert!(!state.sender_open);
        assert!(state.receiver_open);
        assert!(state.serve_ingress_reservation.is_none());
        assert_eq!(state.next_serve_ingress_reservation_ordinal, u128::MAX);
        assert_eq!(state.next_serve_admission_ordinal, 0);
    }
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Err(FairV2IngressPushError::Closed(_))
    ));
    let repeated = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(repeated, after);
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("closed empty ingress detaches failed gate");
}
#[test]
fn fair_ingress_classifies_current_historical_future_and_unauthenticated_requests() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let authenticated = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = service.context.roster[0].validator.clone();
    let body_root = TempDir::new().expect("raw-gate body root");
    let serve_root = TempDir::new().expect("raw-gate Serve root");
    let body_store = V2BodyStore::open(body_root.path(), service.context.clone())
        .expect("open raw-gate body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(2, serve_root.path(), &service.context, &body_store)
            .expect("open raw-gate persistent queue");
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    let (mut foreign_service, foreign_keys) = fixture();
    allow_fixture_block_payload(&mut foreign_service.context);
    foreign_service.context.network_id =
        crate::sumeragi::synthetic_network_id("v2-worker-foreign-test");
    foreign_service
        .context
        .validate()
        .expect("valid distinct foreign context");
    let (_, _, foreign_proposal) =
        proposal_body_and_payload(&foreign_service.context, &foreign_keys);
    let foreign = authenticated_serve_request(
        &foreign_service.context,
        &foreign_keys[1],
        foreign_proposal.round,
        foreign_proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(foreign.request(), via.clone(),)),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    let forged_outer = InboundBlockMessage::from_transport(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(authenticated.request().clone()),
        )),
        service.context.roster[3].validator.clone(),
        via.clone(),
    );
    assert!(matches!(
        ingress.try_push(forged_outer),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    let mut future = authenticated.request().clone();
    future.round.height += 1;
    future.certificate.round.height += 1;
    future.certificate.proposal_round.height += 1;
    future.signature = Signature::new(keys[1].private_key(), &future.signature_preimage())
        .payload()
        .to_vec();
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(&future, via.clone())),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    let mut invalid = authenticated.request().clone();
    invalid.signature[0] ^= 0xFF;
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(&invalid, via.clone())),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    {
        let state = command_tx.queue.lock();
        assert!(state.serve_ingress_reservation.is_none());
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.serve_barrier.is_none());
        assert!(state.serves.is_empty());
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
        assert_eq!(state.next_serve_admission_ordinal, 0);
    }
    assert_eq!(ingress.len(), 0);
    assert_eq!(
        ingress.state.lock().last_admission_ordinal,
        0,
        "raw gate rejection precedes both fair and Serve ordinal mutation"
    );
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire rejection fixture gate");
    let mut active_context = service.context.clone();
    active_context.height += 1;
    active_context.snapshot_bootstrap = Some(wire::SnapshotBootstrapAnchor {
        snapshot_height: service.context.height,
        snapshot_block_hash: proposal.subject.block_hash,
        snapshot_block_creation_time_ms: service.context.height,
        snapshot_state_hash: Hash::new(b"historical Serve gate snapshot state"),
    });
    active_context
        .validate()
        .expect("valid snapshot-successor active context");
    let active_body_root = TempDir::new().expect("historical-gate body root");
    let active_serve_root = TempDir::new().expect("historical-gate Serve root");
    let active_body_store = V2BodyStore::open(active_body_root.path(), active_context.clone())
        .expect("open active-height body store");
    let (active_tx, active_rx, _active_admission) = persistent_test_io_command_channel(
        2,
        active_serve_root.path(),
        &active_context,
        &active_body_store,
    )
    .expect("open active-height persistent queue");
    let (historical_ingress, historical_gate) = gated_fair_ingress(&active_context, &active_tx);
    assert!(matches!(
        historical_ingress.try_push(certified_serve_inbound(authenticated.request(), via,)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let active_round = wire::ConsensusRound {
        context_id: active_context.id(),
        height: active_context.height,
        view: proposal.round.view,
    };
    let active = authenticated_serve_request(
        &active_context,
        &keys[2],
        active_round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let active_via = active_context.roster[3].validator.clone();
    assert!(matches!(
        historical_ingress.try_push(certified_serve_inbound(active.request(), active_via,)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        active_tx
            .serve_barrier_request_hash()
            .expect("inspect mixed historical/current Serve target"),
        Some(active.request_hash())
    );
    let admitted = fair_ingress_accounting_snapshot(&historical_ingress);
    assert_eq!(admitted.last_admission_ordinal, 2);
    let ticket_owners = admitted
        .lanes
        .iter()
        .flat_map(|lane| &lane.entries)
        .filter(|entry| entry.owns_certified_serve_ticket)
        .count();
    assert_eq!(
        ticket_owners, 1,
        "only the current-height request charges the active exact-Serve geometry"
    );
    {
        let state = active_tx.queue.lock();
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.projection.request_hash),
            Some(active.request_hash())
        );
        assert!(state.serve_ingress_waiters.is_empty());
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
        assert_eq!(state.next_serve_admission_ordinal, 1);
    }
    let (active_admission, active_commit) = drain_and_commit_gated_serve(
        &historical_ingress,
        &active_tx,
        CertifiedServeOwnerKey::Roster(active.request().requester.clone()),
        &active,
    );
    assert!(matches!(active_commit, CertifiedServeCommit::Queued));
    assert!(matches!(
        active_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == active_admission.lifecycle_id
    ));
    assert!(
        active_tx
            .serve_barrier_request_hash()
            .expect("prepared active target crosses the checked physical cut")
            .is_none()
    );
    let delivered = historical_ingress
        .try_recv_if_checked(|_| true)
        .expect("historical physical drain cannot fail Serve publication")
        .expect("authenticated historical request reaches context-store service");
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(delivered_request),
        ..
    }) = delivered.message()
    else {
        panic!("historical classification preserves the certified request");
    };
    assert_eq!(delivered_request, authenticated.request());
    historical_ingress.close();
    historical_ingress
        .unbind_certified_serve_gate(&historical_gate)
        .expect("retire historical classification gate");
}
include!("v2_worker_serve_unsealed_cases.rs");
include!("v2_worker_serve_decision_restart_cases.rs");
include!("v2_worker_certified_serve_budget_cases.rs");
#[test]
fn abnormal_service_drop_shuts_worker_down_before_blocking_final_drain() {
    let (mut service, _) = fixture();
    service.clean_teardown = false;
    let output_guard = Arc::clone(&service.output_guard);
    let permit_guard = Arc::clone(&output_guard);
    let (permit_ready_tx, permit_ready_rx) = mpsc::sync_channel(1);
    let (release_permit_tx, release_permit_rx) = mpsc::sync_channel(1);
    let permit_holder = thread::spawn(move || {
        let admitted_output = permit_guard.acquire().expect("admit earlier output");
        permit_ready_tx.send(()).expect("publish admitted output");
        release_permit_rx
            .recv()
            .expect("release admitted output after worker shutdown");
        drop(admitted_output);
    });
    permit_ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("earlier output must be admitted before abnormal teardown");
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let (shutdown_seen_tx, shutdown_seen_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Shutdown)));
        shutdown_seen_tx.send(()).expect("publish worker shutdown");
        release_permit_tx
            .send(())
            .expect("release output after worker shutdown");
        drop(completion_tx);
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(worker),
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    drop(service);
    shutdown_seen_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("abnormal teardown must stop the worker before draining admitted output");
    permit_holder.join().expect("join admitted-output holder");
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn recovery_gate_is_cross_thread_and_precedes_fatal_completion() {
    let gate = ConsensusOutputGuard::isolated();
    let admitted_output = gate.acquire().expect("initial output permit");
    let worker_gate = Arc::clone(&gate);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let later_candidate_published = Arc::new(AtomicBool::new(false));
    let worker_candidate_published = Arc::clone(&later_candidate_published);
    let worker = thread::spawn(move || {
        let fatal_operation = worker_gate
            .begin_fail_stop_operation()
            .expect("fatal worker output operation");
        drop(fatal_operation);
        let _ = completion_tx.try_send(V2IoCompletion::RecoveryRequired(
            "committed marker requires restart".to_owned(),
        ));
        assert!(worker_gate.restart_required());
        if worker_gate.acquire().is_some() {
            worker_candidate_published.store(true, Ordering::Release);
        }
    });
    let completion = completion_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("fatal completion must follow recovery admission closure");
    assert!(matches!(
        completion,
        V2IoCompletion::RecoveryRequired(reason)
            if reason == "committed marker requires restart"
    ));
    assert!(
        gate.restart_required(),
        "the guard must close before publishing the fatal completion"
    );
    assert!(
        gate.acquire().is_none(),
        "a second output must not enter while fatal recovery activation drains"
    );
    drop(admitted_output);
    worker.join().expect("join recovery worker");
    assert!(gate.restart_required());
    assert!(gate.acquire().is_none());
    assert!(
        !later_candidate_published.load(Ordering::Acquire),
        "no candidate may be published after the fatal durability transition"
    );
}
#[test]
fn io_command_panic_latches_restart_required_before_unwinding() {
    let output_guard = ConsensusOutputGuard::isolated();
    let unwind = std::panic::catch_unwind({
        let output_guard = Arc::clone(&output_guard);
        move || {
            let _ = execute_fail_stop_io_command(&output_guard, || {
                panic!("model I/O command panic");
            });
        }
    });
    assert!(unwind.is_err());
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn retire_panic_closes_gate_before_inflight_output_drains() {
    let output_guard = ConsensusOutputGuard::isolated();
    let admitted_output = output_guard.acquire().expect("admit earlier output");
    let worker_guard = Arc::clone(&output_guard);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let unwind = std::panic::catch_unwind(move || {
            let _ = execute_retire_io_command(&worker_guard, || {
                entered_tx.send(()).expect("publish Retire entry");
                panic!("model Retire panic");
            });
        });
        assert!(unwind.is_err());
    });
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("Retire operation entered");
    let activation_deadline = Instant::now() + Duration::from_secs(1);
    while !output_guard.restart_required() && Instant::now() < activation_deadline {
        thread::yield_now();
    }
    assert!(
        output_guard.restart_required(),
        "Retire panic must close admission while earlier output still drains"
    );
    assert!(
        output_guard.acquire().is_none(),
        "no later output may cross the gate after the Retire panic"
    );
    drop(admitted_output);
    worker.join().expect("join panicking Retire model");
    assert!(output_guard.acquire().is_none());
}
#[test]
fn retire_failure_is_nonfatal_and_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let mut worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), Arc::new(AtomicBool::new(false)));
    let completion = execute_retire_io_command(&output_guard, || {
        Err("injected post-finality retirement failure".to_owned())
    })
    .expect("open guard admits Retire");
    assert!(matches!(
        completion,
        V2IoCompletion::RetirementFailed(reason)
            if reason == "injected post-finality retirement failure"
    ));
    worker_failure_guard.disarm();
    drop(worker_failure_guard);
    assert!(!output_guard.restart_required());
    assert!(output_guard.acquire().is_some());
}
#[test]
fn io_worker_lifetime_guard_latches_panic_after_success_before_completion_delivery() {
    let output_guard = ConsensusOutputGuard::isolated();
    let unwind = std::panic::catch_unwind({
        let output_guard = Arc::clone(&output_guard);
        move || {
            let _worker_failure_guard = V2IoWorkerFailureGuard::new(
                Arc::clone(&output_guard),
                Arc::new(AtomicBool::new(false)),
            );
            let completion =
                execute_fail_stop_io_command(&output_guard, || Ok(V2IoCompletion::AuxiliaryNoop))
                    .expect("model successful I/O operation");
            assert!(matches!(completion, V2IoCompletion::AuxiliaryNoop));
            panic!("model panic before completion delivery");
        }
    });
    assert!(unwind.is_err());
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn io_worker_explicit_shutdown_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let mut worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), Arc::new(AtomicBool::new(false)));
    worker_failure_guard.disarm();
    drop(worker_failure_guard);
    assert!(!output_guard.restart_required());
    assert!(output_guard.acquire().is_some());
}
#[test]
fn flagged_finalized_disconnect_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
    allow_finalized_disconnect.store(true, AtomicOrdering::Release);
    let worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);
    drop(worker_failure_guard);
    assert!(!output_guard.restart_required());
    assert!(output_guard.acquire().is_some());
}
#[test]
fn flagged_worker_panic_closes_gate_before_inflight_output_drains() {
    let output_guard = ConsensusOutputGuard::isolated();
    let admitted_output = output_guard.acquire().expect("admit earlier output");
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
    let worker_output_guard = Arc::clone(&output_guard);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let _worker_failure_guard =
            V2IoWorkerFailureGuard::new(worker_output_guard, allow_finalized_disconnect);
        entered_tx.send(()).expect("publish worker entry");
        panic!("model flagged finalized-cleanup worker panic");
    });
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("flagged worker entered");
    let activation_deadline = Instant::now() + Duration::from_secs(1);
    while !output_guard.restart_required() && Instant::now() < activation_deadline {
        thread::yield_now();
    }
    assert!(output_guard.restart_required());
    assert!(
        output_guard.acquire().is_none(),
        "the finalized-disconnect flag must never suppress panic closure"
    );
    drop(admitted_output);
    assert!(worker.join().is_err());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn flagged_worker_fail_stop_error_still_latches_restart_required() {
    let output_guard = ConsensusOutputGuard::isolated();
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
    let worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);
    assert!(
        execute_fail_stop_io_command(&output_guard, || {
            Err("injected fail-stop I/O error".to_owned())
        })
        .is_err()
    );
    drop(worker_failure_guard);
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn recovery_gate_rejects_service_outputs_and_candidate_delivery() {
    let (mut service, _) = fixture();
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let encoded = encode_payload(
        &service.context,
        wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        },
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked block")),
            payload_hash: Hash::new(b"blocked body"),
        },
        b"blocked body",
    )
    .expect("encode bounded payload");
    service
        .prepared_candidates
        .push_back(PreparedCandidateBody {
            tag: EventTag::new(1, 0, Generation::new(1)),
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked candidate")),
                payload_hash: Hash::new(b"blocked payload"),
            },
        });
    service.output_guard.activate_restart_required();
    assert!(service.take_prepared_candidate().is_none());
    let blocked_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked load block")),
        payload_hash: Hash::new(b"blocked load payload"),
    };
    assert!(
        service
            .request_locked_candidate(
                EventTag::new(1, 0, Generation::new(1)),
                locked_candidate_round(&service, 0),
                blocked_subject,
            )
            .is_err()
    );
    assert!(service.locked_candidate_acquisition.is_none());
    assert!(
        command_rx.try_recv().is_err(),
        "post-latch service work must not mutate the ordered I/O queue"
    );
    assert!(
        service
            .register_outbound_payload(service.active_tag, encoded)
            .is_err(),
        "recovery must reject new proposal material before publication"
    );
    assert!(service.output_permit().is_err());
    drop(completion_tx);
}
fn manifest_hash(label: &[u8]) -> HashOf<wire::PayloadManifest> {
    HashOf::from_untyped_unchecked(Hash::new(label))
}
/// Build exact finality and Kura-receipt authority for sibling rollover tests.
pub(in crate::sumeragi) fn durable_finality_fixture(
    service: &ProductionV2Services,
    keys: &[KeyPair],
) -> (KuraV2CommitReceipt, wire::finality::V2FinalityArtifact) {
    let subject = wire::BlockSubject {
        parent_block_hash: service
            .context
            .parent_commit_qc
            .as_ref()
            .map(|parent| parent.subject.block_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"finalized worker block")),
        payload_hash: Hash::new(b"finalized worker payload"),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"worker parent state"),
        Hash::new(b"worker post state"),
        Hash::new(b"worker ordinary writes"),
        1,
        Hash::new(b"worker executed block wire"),
    );
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signature_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signature_shares
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate valid worker CommitQC"),
    };
    let artifact = wire::finality::V2FinalityArtifact::new(
        service.context.clone(),
        subject,
        certificate,
        keys.iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("worker fixture validator PoP")
            })
            .collect(),
    );
    artifact.validate().expect("valid worker finality artifact");
    (KuraV2CommitReceipt::for_test(&artifact), artifact)
}
fn durable_receipt(service: &ProductionV2Services, keys: &[KeyPair]) -> KuraV2CommitReceipt {
    durable_finality_fixture(service, keys).0
}
fn seal_empty_exact_output_for_cleanup_test(service: &ProductionV2Services) {
    let pending = service
        .pending_exact_output
        .lock()
        .expect("cleanup fixture exact-output corridor");
    assert!(
        !pending.is_pending(),
        "cleanup fixture must not bypass pending exact output"
    );
    service
        .exact_output_handoff_owner
        .seal()
        .expect("seal the cleanup fixture's empty exact-output corridor");
}
/// Rebind closed-network production services to an exact durable context.
pub(in crate::sumeragi) fn service_for_history_context(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
) -> ProductionV2Services {
    service_for_history_context_with_local_validator(kura, context, validators, 0)
}
/// Rebind a closed-network service to an explicit paired handoff owner.
pub(in crate::sumeragi) fn service_for_history_context_with_handoff_owner(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
) -> ProductionV2Services {
    let mut service = service_for_history_context(kura, context, validators);
    service.exact_output_handoff_owner = exact_output_handoff_owner;
    service
}
/// Rebind one explicit validator and pair the service with its exact handoff owner.
pub(in crate::sumeragi) fn service_for_history_context_with_local_validator_and_handoff_owner(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
) -> ProductionV2Services {
    let mut service = service_for_history_context_with_local_validator(
        kura,
        context,
        validators,
        local_validator,
    );
    service.exact_output_handoff_owner = exact_output_handoff_owner;
    service
}
/// Rebind closed-network production services to one validator in an exact durable context.
pub(in crate::sumeragi) fn service_for_history_context_with_local_validator(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
) -> ProductionV2Services {
    let (mut service, _) = fixture();
    context.validate().expect("valid history-fixture successor");
    let local_index = usize::try_from(local_validator)
        .expect("history-fixture validator index fits this platform");
    let local_key = validators
        .get(local_index)
        .expect("history-fixture validator index belongs to its key roster")
        .clone();
    let local_peer = PeerId::new(local_key.public_key().clone());
    assert_eq!(
        context
            .roster
            .get(local_index)
            .map(|entry| &entry.validator),
        Some(&local_peer),
        "history-fixture key roster must match its durable context"
    );
    service.validator_set_pops = validators
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("history-fixture validator PoP")
        })
        .collect();
    let business_chain_id = service.state.chain_id.clone();
    service.state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        business_chain_id,
        context.network_id,
    ));
    service.context = context;
    service.local_peer = local_peer;
    service.local_validator = Some(local_validator);
    service.key_pair = local_key;
    service.kura = kura;
    service.active_tag = EventTag::new(
        service.context.height,
        0,
        Generation::new(service.context.height),
    );
    service
}
fn successor_service_for_history(
    kura: Arc<Kura>,
    parent: &wire::finality::V2FinalityArtifact,
    validators: &[KeyPair],
) -> ProductionV2Services {
    successor_service_for_history_as(kura, parent, validators, 0)
}
fn successor_service_for_history_as(
    kura: Arc<Kura>,
    parent: &wire::finality::V2FinalityArtifact,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
) -> ProductionV2Services {
    let mut context = parent.height_context.clone();
    context.height = parent.height.saturating_add(1);
    context.parent_commit_qc = Some(parent.commit_qc.clone());
    service_for_history_context_with_local_validator(kura, context, validators, local_validator)
}
