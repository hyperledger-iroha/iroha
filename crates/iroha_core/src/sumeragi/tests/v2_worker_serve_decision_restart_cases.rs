// Certified-Serve decision and restart worker regression tests.
// Included lexically by v2_worker::tests to preserve canonical test names.

    #[test]
    fn prepared_serve_carrier_is_atomically_superseded_by_decision() {
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
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"Decision supersedes a prepared exact Serve carrier",
            )),
            payload_hash: Hash::new(b"prepared exact Serve Decision payload"),
            ..proposal.subject
        };
        assert_ne!(decided_subject, proposal.subject);

        for (saturated, cancel_after_failure) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let body_root = TempDir::new().expect("prepared Decision body root");
            let body_store = V2BodyStore::open(body_root.path(), context.clone())
                .expect("open prepared Decision body store");
            let serve_root = TempDir::new().expect("prepared Decision Serve root");
            let (command_tx, _command_rx, io_admission) =
                persistent_test_io_command_channel(1, serve_root.path(), &context, &body_store)
                    .expect("open prepared Decision Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            if saturated {
                command_tx
                    .try_send_as(
                        V2IoAdmissionClass::Control,
                        V2IoCommand::LoadCandidate {
                            acquisition_id: LockedCandidateAcquisitionId(501),
                            subject: proposal.subject,
                        },
                    )
                    .expect("saturate the physical queue before exact Serve preparation");
            }
            let via = context.roster[3].validator.clone();
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let barrier = command_tx
                .serve_barrier()
                .expect("inspect prepared Decision barrier")
                .expect("exact Serve carrier owns a barrier");
            let mut preparation = None;
            assert!(
                ingress
                    .try_recv_if(|_| {
                        preparation = Some(command_tx.prepare_reserved_serve(
                            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                            request.clone(),
                        ));
                        false
                    })
                    .is_none(),
                "preparing inside the predicate must retain the physical carrier"
            );
            if saturated {
                assert!(matches!(
                    preparation,
                    Some(Err(CertifiedServePrepareError::Backpressure))
                ));
            } else {
                assert!(matches!(preparation, Some(Ok(_))));
            }
            let prepared_state = if saturated {
                V2IoServeState::PendingCapacity
            } else {
                V2IoServeState::Reserved
            };
            {
                let state = command_tx.queue.lock();
                assert_eq!(
                    state
                        .serve_ingress_reservation
                        .as_ref()
                        .map(|reservation| reservation.state),
                    Some(CertifiedServeIngressReservationState::Prepared(
                        barrier.lifecycle_id()
                    ))
                );
                assert_eq!(
                    state
                        .serves
                        .get(&barrier.lifecycle_id())
                        .map(|tracked| tracked.state),
                    Some(prepared_state)
                );
                assert_eq!(
                    state
                        .pending_serve_requests
                        .contains_key(&barrier.lifecycle_id()),
                    saturated
                );
                assert_eq!(
                    state
                        .commands
                        .iter()
                        .filter(|command| {
                            command.serve_lifecycle_id() == Some(barrier.lifecycle_id())
                        })
                        .count(),
                    usize::from(!saturated)
                );
            }
            for invalid_outcome in [
                CertifiedServeNegativeOutcome::InvalidCertificate,
                CertifiedServeNegativeOutcome::LocalRetentionAuthorityAbsent,
            ] {
                assert!(
                    command_tx
                        .stage_selected_serve_rejection(request.request_hash(), invalid_outcome)
                        .is_err(),
                    "a non-Decision negative cannot overwrite a prepared handoff"
                );
            }

            command_tx
                .begin_decision_serve_reconciliation()
                .expect("raise the Decision/Serve fence");
            command_tx
                .finish_decision_serve_reconciliation(Some(decided_subject))
                .expect("publish the superseding Decision");
            let fair_before_failure = fair_ingress_accounting_snapshot(&ingress);
            let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
            let (
                ingress_highwater_before,
                lifecycle_highwater_before,
                serve_by_request_before,
                serve_by_family_before,
                barrier_predecessors_before,
            ) = {
                let state = command_tx.queue.lock();
                (
                    state.next_serve_ingress_reservation_ordinal,
                    state.next_serve_admission_ordinal,
                    state.serve_by_request.clone(),
                    state.serve_by_family.clone(),
                    state.serve_barrier_predecessors.clone(),
                )
            };
            let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
            let durable_before_failure =
                fs::read(&state_path).expect("read prepared Serve durable snapshot");
            let temporary_state = state_path.with_extension("norito.tmp");
            fs::create_dir(&temporary_state)
                .expect("block atomic prepared-carrier Decision publication");
            let error = ingress
                .try_recv_if_checked(|_| {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("stage the exact durable Decision outcome");
                    true
                })
                .expect_err("failed negative publication retains the prepared fair carrier");
            assert!(
                error.contains("failed to create Sumeragi v2 Serve temporary state"),
                "unexpected prepared-carrier persistence error: {error}"
            );
            assert_eq!(
                fair_ingress_accounting_snapshot(&ingress),
                fair_before_failure
            );
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            assert_eq!(
                fs::read(&state_path).expect("reload failed prepared-carrier publication"),
                durable_before_failure
            );
            {
                let state = command_tx.queue.lock();
                assert_eq!(
                    state
                        .serve_ingress_reservation
                        .as_ref()
                        .map(|reservation| reservation.state),
                    Some(
                        CertifiedServeIngressReservationState::DeterministicallyRejected(
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                    ),
                    "the selected carrier retains its staged deterministic outcome"
                );
                assert_eq!(
                    state
                        .serves
                        .get(&barrier.lifecycle_id())
                        .map(|tracked| tracked.state),
                    Some(prepared_state),
                    "failed persistence restores the uncommitted logical owner"
                );
                assert_eq!(state.serve_barrier, Some(barrier.lifecycle_id()));
                assert_eq!(
                    state.serve_barrier_predecessors,
                    barrier_predecessors_before
                );
                assert_eq!(
                    state
                        .pending_serve_requests
                        .contains_key(&barrier.lifecycle_id()),
                    saturated
                );
                assert_eq!(
                    state
                        .commands
                        .iter()
                        .filter(|command| {
                            command.serve_lifecycle_id() == Some(barrier.lifecycle_id())
                        })
                        .count(),
                    usize::from(!saturated)
                );
                assert_eq!(state.serve_by_request, serve_by_request_before);
                assert_eq!(state.serve_by_family, serve_by_family_before);
                assert_eq!(
                    state.next_serve_ingress_reservation_ordinal,
                    ingress_highwater_before
                );
                assert_eq!(
                    state.next_serve_admission_ordinal,
                    lifecycle_highwater_before
                );
            }
            assert_eq!(
                io_admission.queued.load(AtomicOrdering::Acquire),
                1,
                "failed publication retains either the predecessor or reserved Serve admission"
            );

            fs::remove_dir(&temporary_state)
                .expect("unblock prepared-carrier Decision publication");
            if cancel_after_failure {
                ingress.close();
                ingress
                    .unbind_certified_serve_gate(&gate)
                    .expect("drop the failed prepared carrier through its real gate");
                {
                    let state = command_tx.queue.lock();
                    assert_eq!(
                        state
                            .serves
                            .get(&barrier.lifecycle_id())
                            .map(|tracked| tracked.state),
                        Some(V2IoServeState::AwaitingRetry),
                        "carrier cancellation rolls the uncommitted handoff back"
                    );
                    assert!(state.serve_barrier.is_none());
                    assert!(state.serve_barrier_predecessors.is_empty());
                    assert!(
                        !state
                            .pending_serve_requests
                            .contains_key(&barrier.lifecycle_id())
                    );
                    assert!(state.commands.iter().all(|command| {
                        command.serve_lifecycle_id() != Some(barrier.lifecycle_id())
                    }));
                    assert!(state.serve_ingress_reservation.is_none());
                    assert_eq!(state.serve_ingress_waiters.len(), 1);
                    let dormant = state
                        .serve_ingress_waiters
                        .values()
                        .next()
                        .expect("cancelled carrier retains one dormant durable waiter");
                    assert_eq!(dormant.lifecycle_id, barrier.lifecycle_id());
                    assert_eq!(
                        dormant.state,
                        CertifiedServeIngressReservationState::Provisional
                    );
                    assert!(dormant.handed_off.is_none());
                    assert!(dormant.carrier_ordinal.is_none());
                    assert_eq!(state.serve_by_request, serve_by_request_before);
                    assert_eq!(state.serve_by_family, serve_by_family_before);
                    assert_eq!(
                        state.next_serve_ingress_reservation_ordinal,
                        ingress_highwater_before
                    );
                    assert_eq!(
                        state.next_serve_admission_ordinal,
                        lifecycle_highwater_before
                    );
                }
                assert_eq!(
                    io_admission.queued.load(AtomicOrdering::Acquire),
                    usize::from(saturated),
                    "rollback releases only a reserved Serve placeholder"
                );
                assert_eq!(
                    command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                    actor_ordinal_before
                );
                let persisted = command_tx
                    .queue
                    .serve_state_store
                    .as_ref()
                    .expect("cancelled prepared queue has durable state")
                    .load(&context)
                    .expect("reload cancelled prepared-carrier snapshot");
                assert_eq!(persisted.ingress_waiters.len(), 1);
                assert_eq!(persisted.unsealed_lifecycles.len(), 1);
                assert!(persisted.negative_tombstones.is_empty());
                assert!(persisted.terminal_tombstones.is_empty());
                continue;
            }
            let drained = ingress
                .try_recv_if_checked(|_| {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("restage the retained exact Decision outcome");
                    true
                })
                .expect("publish prepared-carrier Decision supersession")
                .expect("drain the superseded exact carrier");
            drop(drained);
            {
                let state = command_tx.queue.lock();
                let tracked = state
                    .serves
                    .get(&barrier.lifecycle_id())
                    .expect("Decision-negative Serve lifecycle remains indexed");
                assert_eq!(
                    tracked.state,
                    V2IoServeState::Rejected(
                        CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                    )
                );
                assert!(tracked.terminal.is_none());
                assert!(tracked.reply_routes.is_none());
                assert!(tracked.ingress_ownership.is_none());
                assert!(state.serve_barrier.is_none());
                assert!(state.serve_barrier_predecessors.is_empty());
                assert!(
                    !state
                        .pending_serve_requests
                        .contains_key(&barrier.lifecycle_id())
                );
                assert!(state
                    .commands
                    .iter()
                    .all(|command| command.serve_lifecycle_id() != Some(barrier.lifecycle_id())));
                assert!(state.serve_ingress_reservation.is_none());
                assert!(state.serve_ingress_waiters.is_empty());
                assert_eq!(state.serve_by_request, serve_by_request_before);
                assert_eq!(state.serve_by_family, serve_by_family_before);
                assert_eq!(
                    state.next_serve_ingress_reservation_ordinal,
                    ingress_highwater_before
                );
                assert_eq!(
                    state.next_serve_admission_ordinal,
                    lifecycle_highwater_before
                );
            }
            assert_eq!(
                io_admission.queued.load(AtomicOrdering::Acquire),
                usize::from(saturated),
                "successful supersession releases only the prepared Serve placeholder"
            );
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("prepared Decision queue has durable state")
                .load(&context)
                .expect("reload prepared-carrier Decision negative");
            assert!(persisted.ingress_waiters.is_empty());
            assert!(persisted.unsealed_lifecycles.is_empty());
            assert!(persisted.terminal_tombstones.is_empty());
            assert_eq!(
                persisted
                    .negative_tombstones
                    .iter()
                    .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                    .collect::<Vec<_>>(),
                vec![(
                    barrier.lifecycle_id(),
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )]
            );
            let fair_before_retry = fair_ingress_accounting_snapshot(&ingress);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via)),
                Err(FairV2IngressPushError::Rejected(_))
            ));
            assert_eq!(
                fair_ingress_accounting_snapshot(&ingress),
                fair_before_retry
            );
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire prepared Decision supersession gate");
        }
    }

    #[test]
    fn established_serve_owner_survives_decision_retry_carrier_retirement() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"Decision supersedes an established Serve retry carrier",
            )),
            payload_hash: Hash::new(b"established Serve retry Decision payload"),
            ..proposal.subject
        };
        assert_ne!(decided_subject, proposal.subject);

        // Cover Queued and Active owners, a response sealed before the retry,
        // and the worker/Decision race where sealing occurs while the retry
        // carrier is still selected.
        for (dequeue_original, seal_before_retry, seal_after_decision) in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let body_root = TempDir::new().expect("established retry body root");
            let body_store = V2BodyStore::open(body_root.path(), context.clone())
                .expect("open established retry body store");
            let serve_root = TempDir::new().expect("established retry Serve root");
            let (command_tx, command_rx, io_admission) =
                persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                    .expect("open established retry Serve queue");
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
            if dequeue_original {
                assert!(matches!(
                    command_rx.try_recv(),
                    Ok(V2IoCommand::Serve { lifecycle_id, .. })
                        if lifecycle_id == admission.lifecycle_id
                ));
            }
            if seal_before_retry {
                assert!(
                    command_rx
                        .complete_serve_response(admission.lifecycle_id, &response)
                        .expect("seal response before admitting its exact retry")
                );
            }

            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let retry_barrier = command_tx
                .serve_barrier()
                .expect("inspect established retry barrier")
                .expect("exact retry owns a selected carrier");
            assert_eq!(retry_barrier.lifecycle_id(), admission.lifecycle_id);
            let mut retry_admission = None;
            assert!(
                ingress
                    .try_recv_if(|_| {
                        retry_admission = Some(
                            command_tx
                                .prepare_reserved_serve(
                                    CertifiedServeOwnerKey::Roster(
                                        request.request().requester.clone(),
                                    ),
                                    request.clone(),
                                )
                                .expect("prepare established exact retry"),
                        );
                        false
                    })
                    .is_none()
            );
            assert!(matches!(
                retry_admission,
                Some(CertifiedServeAdmission {
                    lifecycle_id,
                    kind: CertifiedServeAdmissionKind::Existing,
                    ..
                }) if lifecycle_id == admission.lifecycle_id
            ));
            command_tx
                .begin_decision_serve_reconciliation()
                .expect("raise Decision fence over established retry");
            command_tx
                .finish_decision_serve_reconciliation(Some(decided_subject))
                .expect("publish Decision over established retry");

            if seal_after_decision {
                assert!(
                    command_rx
                        .complete_serve_response(admission.lifecycle_id, &response)
                        .expect("seal response while its Decision-losing retry carrier is live"),
                    "the live carrier keeps worker completion on the established owner"
                );
            }
            let expected_established_state = if !dequeue_original {
                V2IoServeState::Queued
            } else if seal_before_retry || seal_after_decision {
                V2IoServeState::CompletionPending
            } else {
                V2IoServeState::Active
            };
            if expected_established_state == V2IoServeState::CompletionPending {
                let error = command_tx
                    .serve_completion_delivery_ownership(
                        admission.lifecycle_id,
                        response.request_hash,
                    )
                    .expect_err(
                        "delivery cannot overtake the selected Decision-losing retry carrier",
                    );
                assert!(
                    error.contains("before its retry carrier drained"),
                    "unexpected pre-drain delivery error: {error}"
                );
            }
            let fair_before_drain = fair_ingress_accounting_snapshot(&ingress);
            let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
            let (
                ingress_highwater_before,
                lifecycle_highwater_before,
                serve_by_request_before,
                serve_by_family_before,
                reply_routes_before,
                ingress_ownership_before,
            ) = {
                let state = command_tx.queue.lock();
                let tracked = state
                    .serves
                    .get(&admission.lifecycle_id)
                    .expect("established retry lifecycle remains indexed");
                assert_eq!(tracked.state, expected_established_state);
                (
                    state.next_serve_ingress_reservation_ordinal,
                    state.next_serve_admission_ordinal,
                    state.serve_by_request.clone(),
                    state.serve_by_family.clone(),
                    tracked.reply_routes.clone(),
                    tracked
                        .ingress_ownership
                        .as_ref()
                        .map(FairV2IngressOwnershipEvidence::process_local_projection_hash),
                )
            };
            let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
            let durable_before_failure =
                fs::read(&state_path).expect("read established retry durable snapshot");
            let temporary_state = state_path.with_extension("norito.tmp");
            fs::create_dir(&temporary_state)
                .expect("block atomic carrier-only retirement publication");
            let error = ingress
                .try_recv_if_checked(|_| {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("stage established retry Decision loss before forced failure");
                    true
                })
                .expect_err("failed carrier-only publication retains the exact retry");
            assert!(
                error.contains("failed to create Sumeragi v2 Serve temporary state"),
                "unexpected carrier-only persistence error: {error}"
            );
            assert_eq!(
                fair_ingress_accounting_snapshot(&ingress),
                fair_before_drain,
                "failed carrier-only persistence cannot consume or reorder the retry"
            );
            assert_eq!(
                fs::read(&state_path).expect("reload failed carrier-only publication"),
                durable_before_failure
            );
            {
                let state = command_tx.queue.lock();
                let tracked = state
                    .serves
                    .get(&admission.lifecycle_id)
                    .expect("failed carrier-only publication retains logical owner");
                assert_eq!(tracked.state, expected_established_state);
                assert!(
                    tracked
                        .reply_routes
                        .as_ref()
                        .zip(reply_routes_before.as_ref())
                        .is_some_and(|(retained, previous)| {
                            retained.has_same_exact_history(previous)
                        })
                );
                assert_eq!(
                    tracked
                        .ingress_ownership
                        .as_ref()
                        .map(FairV2IngressOwnershipEvidence::process_local_projection_hash),
                    ingress_ownership_before
                );
                assert_eq!(
                    state
                        .serve_ingress_reservation
                        .as_ref()
                        .map(|reservation| (reservation.lifecycle_id, reservation.state)),
                    Some((
                        admission.lifecycle_id,
                        CertifiedServeIngressReservationState::DeterministicallyRejected(
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        ),
                    ))
                );
                assert_eq!(state.serve_by_request, serve_by_request_before);
                assert_eq!(state.serve_by_family, serve_by_family_before);
                assert_eq!(
                    state.next_serve_ingress_reservation_ordinal,
                    ingress_highwater_before
                );
                assert_eq!(
                    state.next_serve_admission_ordinal,
                    lifecycle_highwater_before
                );
            }
            assert_eq!(
                io_admission.queued.load(AtomicOrdering::Acquire),
                1,
                "failed carrier-only publication preserves established admission"
            );
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            fs::remove_dir(&temporary_state)
                .expect("unblock atomic carrier-only retirement publication");
            let drained = ingress
                .try_recv_if_checked(|_| {
                    command_tx
                        .stage_selected_serve_rejection(
                            request.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                                decided_subject,
                            ),
                        )
                        .expect("stage established retry's exact Decision loss");
                    true
                })
                .expect("persist established retry carrier retirement")
                .expect("drain established retry carrier");
            drop(drained);
            assert_ne!(
                fair_ingress_accounting_snapshot(&ingress),
                fair_before_drain,
                "successful physical drain consumes the retry occurrence"
            );
            {
                let state = command_tx.queue.lock();
                let tracked = state
                    .serves
                    .get(&admission.lifecycle_id)
                    .expect("carrier retirement preserves established owner");
                assert_eq!(
                    tracked.state, expected_established_state,
                    "carrier retirement is not logical completion progress"
                );
                assert!(
                    tracked
                        .reply_routes
                        .as_ref()
                        .zip(reply_routes_before.as_ref())
                        .is_some_and(|(retained, previous)| {
                            retained.has_same_exact_history(previous)
                        }),
                    "carrier retirement preserves the established exact reply routes"
                );
                assert_eq!(
                    tracked
                        .ingress_ownership
                        .as_ref()
                        .map(FairV2IngressOwnershipEvidence::process_local_projection_hash),
                    ingress_ownership_before
                );
                assert!(state.serve_ingress_reservation.is_none());
                assert!(state.serve_ingress_waiters.is_empty());
                assert_eq!(state.serve_by_request, serve_by_request_before);
                assert_eq!(state.serve_by_family, serve_by_family_before);
                assert_eq!(
                    state.next_serve_ingress_reservation_ordinal,
                    ingress_highwater_before
                );
                assert_eq!(
                    state.next_serve_admission_ordinal,
                    lifecycle_highwater_before
                );
            }
            assert_eq!(
                io_admission.queued.load(AtomicOrdering::Acquire),
                1,
                "retry retirement preserves the established command/completion admission"
            );
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            let persisted_before_suppression = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("established retry queue has durable state")
                .load(&context)
                .expect("reload carrier-only retirement");
            assert!(persisted_before_suppression.ingress_waiters.is_empty());
            assert!(persisted_before_suppression.negative_tombstones.is_empty());
            match expected_established_state {
                V2IoServeState::Queued | V2IoServeState::Active => {
                    assert_eq!(persisted_before_suppression.unsealed_lifecycles.len(), 1);
                    assert!(persisted_before_suppression.terminal_tombstones.is_empty());
                    if expected_established_state == V2IoServeState::Queued {
                        assert!(matches!(
                            command_rx.try_recv(),
                            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                                if lifecycle_id == admission.lifecycle_id
                        ));
                    }
                    assert!(
                        !command_rx
                            .complete_serve_response(admission.lifecycle_id, &response)
                            .expect("classify post-drain worker completion against Decision")
                    );
                }
                V2IoServeState::CompletionPending => {
                    assert!(persisted_before_suppression.unsealed_lifecycles.is_empty());
                    assert_eq!(persisted_before_suppression.terminal_tombstones.len(), 1);
                    assert!(
                        command_tx
                            .serve_completion_delivery_ownership(
                                admission.lifecycle_id,
                                response.request_hash,
                            )
                            .expect("suppress post-drain response against Decision")
                            .is_none()
                    );
                    command_tx
                        .acknowledge_serve_completion(
                            admission.lifecycle_id,
                            V2IoServeTerminal::Response(response.clone()),
                        )
                        .expect("retire suppressed completion-channel ownership");
                }
                V2IoServeState::AwaitingRetry
                | V2IoServeState::PendingCapacity
                | V2IoServeState::Reserved
                | V2IoServeState::Terminal
                | V2IoServeState::Rejected(_)
                | V2IoServeState::Failed => {
                    unreachable!("established-owner retry state was selected above")
                }
            }
            {
                let state = command_tx.queue.lock();
                let tracked = state
                    .serves
                    .get(&admission.lifecycle_id)
                    .expect("Decision negative remains indexed");
                assert_eq!(
                    tracked.state,
                    V2IoServeState::Rejected(
                        CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                    )
                );
                assert!(tracked.terminal.is_none());
                assert!(tracked.reply_routes.is_none());
                assert!(tracked.ingress_ownership.is_none());
                assert_eq!(
                    state.next_serve_ingress_reservation_ordinal,
                    ingress_highwater_before
                );
                assert_eq!(
                    state.next_serve_admission_ordinal,
                    lifecycle_highwater_before
                );
            }
            assert_eq!(io_admission.queued.load(AtomicOrdering::Acquire), 0);
            assert_eq!(
                command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
                actor_ordinal_before
            );
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("established retry queue has durable state")
                .load(&context)
                .expect("reload established owner Decision negative");
            assert!(persisted.ingress_waiters.is_empty());
            assert!(persisted.unsealed_lifecycles.is_empty());
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
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire established retry Decision gate");
        }
    }

    #[test]
    fn decision_serve_fence_rejects_conflicting_durable_subject_without_ordinals() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let (request, validator_pops) = production_authenticated_serve_request(
            &context,
            &keys,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
            &[0, 1, 2, 3],
        );
        let conflicting_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"conflicting durable Decision subject B",
            )),
            payload_hash: Hash::new(b"conflicting durable Decision payload B"),
            ..proposal.subject
        };
        assert_ne!(conflicting_subject, proposal.subject);

        let body_root = TempDir::new().expect("Decision conflict body root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let serve_root = TempDir::new().expect("Decision conflict Serve root");
        let serve_family_capacity = certified_serve_family_capacity(context.roster.len(), 4, 4)
            .expect("Decision conflict Serve family capacity");
        let (serve_store, empty_persisted) =
            CertifiedServeStateStore::open(serve_root.path(), &context, serve_family_capacity)
                .expect("open empty Decision conflict Serve state");
        serve_store
            .persist(&empty_persisted)
            .expect("persist empty Decision conflict Serve state");
        let (command_tx, _command_rx, _admission) = production_persistent_test_io_command_channel(
            4,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(proposal.subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("open production queue with the first durable Decision");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let fair_before = fair_ingress_accounting_snapshot(&ingress);
        let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let durable_before =
            fs::read(&state_path).expect("read the first durable Decision Serve snapshot");
        let (
            ingress_highwater_before,
            lifecycle_highwater_before,
            serve_by_request_before,
            serve_by_family_before,
            serve_replacement_keys_before,
        ) = {
            let state = command_tx.queue.lock();
            assert_eq!(state.durable_decided_subject, Some(proposal.subject));
            assert!(!state.decision_reconciliation_pending);
            assert!(state.commands.is_empty());
            assert!(state.work.is_empty());
            assert!(state.serves.is_empty());
            assert!(state.serve_barrier.is_none());
            assert!(state.serve_barrier_predecessors.is_empty());
            assert!(state.pending_serve_requests.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(!state.producer_episode_active);
            assert!(state.sender_open && state.receiver_open);
            (
                state.next_serve_ingress_reservation_ordinal,
                state.next_serve_admission_ordinal,
                state.serve_by_request.clone(),
                state.serve_by_family.clone(),
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
            )
        };

        command_tx
            .begin_decision_serve_reconciliation()
            .expect("raise the second pre-WAL Serve admission fence");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                request.request(),
                context.roster[3].validator.clone(),
            )),
            Err(FairV2IngressPushError::Full(_))
        ));
        let error = command_tx
            .finish_decision_serve_reconciliation(Some(conflicting_subject))
            .expect_err("one height cannot replace its durable Decision subject");
        assert!(
            error.contains("two different durable Serve Decision subjects"),
            "unexpected Decision conflict error: {error}"
        );
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                request.request(),
                context.roster[3].validator.clone(),
            )),
            Err(FairV2IngressPushError::Full(_))
        ));

        {
            let state = command_tx.queue.lock();
            assert!(
                state.decision_reconciliation_pending,
                "the conflicting WAL publication must leave the admission fence raised"
            );
            assert_eq!(
                state.durable_decided_subject,
                Some(proposal.subject),
                "the first monotone durable Decision marker must remain authoritative"
            );
            assert!(state.commands.is_empty());
            assert!(state.work.is_empty());
            assert!(state.serves.is_empty());
            assert!(state.serve_barrier.is_none());
            assert!(state.serve_barrier_predecessors.is_empty());
            assert!(state.pending_serve_requests.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(!state.producer_episode_active);
            assert!(state.sender_open && state.receiver_open);
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(
            fs::read(&state_path).expect("reload the retained Decision snapshot"),
            durable_before
        );
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            fair_before,
            "the raised WAL fence blocks before the Fair admission ordinal"
        );
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before,
            "the conflict cannot mint an actor-global logical ordinal"
        );

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire Decision conflict gate");
    }

    #[test]
    fn decision_serve_fence_rolls_back_failed_batch_and_converts_before_ordinals() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let request_b = authenticated_serve_request(
            &context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Commit,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let response_b = certified_serve_response(
            &request_b,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"live Decision fence supersedes terminal Serve A",
            )),
            payload_hash: Hash::new(b"live Decision fence payload B"),
            ..proposal.subject
        };
        assert_ne!(decided_subject, proposal.subject);

        let body_root = TempDir::new().expect("Decision fence body root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let _ = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("retain terminal response body");
        let serve_root = TempDir::new().expect("Decision fence Serve root");
        let lifecycle_id = persist_terminal_serve_fixture(
            serve_root.path(),
            &context,
            &request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            &response,
        );
        let lifecycle_id_b = persist_terminal_serve_fixture(
            serve_root.path(),
            &context,
            &request_b,
            CertifiedServeOwnerKey::Roster(request_b.request().requester.clone()),
            2,
            &response_b,
        );
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("restore terminal response before live Decision publication");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let via = context.roster[3].validator.clone();
        let fair_before = fair_ingress_accounting_snapshot(&ingress);
        let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        let (
            ingress_highwater_before,
            lifecycle_highwater_before,
            serve_by_request_before,
            serve_by_family_before,
            serve_replacement_keys_before,
        ) = {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(
                state
                    .serves
                    .get(&lifecycle_id_b)
                    .map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            (
                state.next_serve_ingress_reservation_ordinal,
                state.next_serve_admission_ordinal,
                state.serve_by_request.clone(),
                state.serve_by_family.clone(),
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
            )
        };

        command_tx
            .begin_decision_serve_reconciliation()
            .expect("raise the pre-WAL Serve admission fence");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            fair_before,
            "the WAL fence blocks before Fair admission"
        );
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before,
            "the WAL fence blocks before the actor-global scheduler source"
        );

        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let durable_before =
            fs::read(&state_path).expect("read the pre-Decision terminal snapshot");
        let temporary_state = state_path.with_extension("norito.tmp");
        fs::create_dir(&temporary_state).expect("block atomic Decision publication");
        let error = command_tx
            .finish_decision_serve_reconciliation(Some(decided_subject))
            .expect_err("failed Decision publication must roll the batch back");
        assert!(
            error.contains("failed to create Sumeragi v2 Serve temporary state"),
            "unexpected Decision publication error: {error}"
        );
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&lifecycle_id)
                .expect("failed Decision publication retains the terminal lifecycle");
            assert_eq!(tracked.state, V2IoServeState::Terminal);
            assert_eq!(
                tracked.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone()))
            );
            assert!(tracked.reply_routes.is_none());
            assert!(tracked.ingress_ownership.is_none());
            let tracked_b = state
                .serves
                .get(&lifecycle_id_b)
                .expect("failed Decision publication retains the second terminal lifecycle");
            assert_eq!(tracked_b.state, V2IoServeState::Terminal);
            assert_eq!(
                tracked_b.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response_b.clone()))
            );
            assert!(tracked_b.reply_routes.is_none());
            assert!(tracked_b.ingress_ownership.is_none());
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert!(state.decision_reconciliation_pending);
            assert_eq!(state.durable_decided_subject, None);
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(
            fs::read(&state_path).expect("reload failed Decision publication"),
            durable_before
        );
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );
        assert_eq!(fair_ingress_accounting_snapshot(&ingress), fair_before);

        fs::remove_dir(&temporary_state).expect("unblock atomic Decision publication");
        command_tx
            .finish_decision_serve_reconciliation(Some(decided_subject))
            .expect("publish the terminal conversion and Decision atomically");
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&lifecycle_id)
                .expect("Decision negative remains indexed");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            );
            assert!(tracked.terminal.is_none());
            assert!(tracked.reply_routes.is_none());
            assert!(tracked.ingress_ownership.is_none());
            let tracked_b = state
                .serves
                .get(&lifecycle_id_b)
                .expect("second Decision negative remains indexed");
            assert_eq!(
                tracked_b.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            );
            assert!(tracked_b.terminal.is_none());
            assert!(tracked_b.reply_routes.is_none());
            assert!(tracked_b.ingress_ownership.is_none());
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert!(
                state.serve_replacements.is_empty(),
                "successful batch publication retires every converted replacement owner"
            );
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            fair_before,
            "post-Decision exact retry is rejected before Fair admission"
        );
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before,
            "Decision publication and exact retry preserve the actor-global source"
        );
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("Decision fence queue has durable state")
            .load(&context)
            .expect("reload the Decision negative");
        assert!(persisted.terminal_tombstones.is_empty());
        assert_eq!(
            persisted
                .negative_tombstones
                .iter()
                .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                .collect::<Vec<_>>(),
            vec![
                (
                    lifecycle_id,
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                ),
                (
                    lifecycle_id_b,
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            ]
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire Decision fence gate");
    }

    #[test]
    fn active_serve_completion_after_decision_publishes_negative_without_response() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"worker Decision supersedes active Serve A",
            )),
            payload_hash: Hash::new(b"worker Decision payload B"),
            ..proposal.subject
        };
        let body_root = TempDir::new().expect("active supersession body root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let serve_root = TempDir::new().expect("active supersession Serve root");
        let (command_tx, command_rx, io_admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("open active supersession queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let via = context.roster[3].validator.clone();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (serve_admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            &request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == serve_admission.lifecycle_id
        ));
        assert_eq!(io_admission.queued.load(AtomicOrdering::Acquire), 1);

        let fair_before = fair_ingress_accounting_snapshot(&ingress);
        let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        let (
            ingress_highwater_before,
            lifecycle_highwater_before,
            serve_by_request_before,
            serve_by_family_before,
            serve_replacement_keys_before,
            serve_barrier_before,
            serve_barrier_predecessors_before,
        ) = {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("active Serve retains its exact logical owner");
            assert_eq!(tracked.state, V2IoServeState::Active);
            assert!(tracked.terminal.is_none());
            assert!(tracked.reply_routes.is_some());
            assert!(tracked.ingress_ownership.is_some());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            (
                state.next_serve_ingress_reservation_ordinal,
                state.next_serve_admission_ordinal,
                state.serve_by_request.clone(),
                state.serve_by_family.clone(),
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                state.serve_barrier,
                state.serve_barrier_predecessors.clone(),
            )
        };
        command_tx
            .begin_decision_serve_reconciliation()
            .expect("fence active Serve against the runtime WAL step");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        command_tx
            .finish_decision_serve_reconciliation(Some(decided_subject))
            .expect("publish Decision while the pre-fence Serve worker is active");
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state
                    .serves
                    .get(&serve_admission.lifecycle_id)
                    .map(|tracked| tracked.state),
                Some(V2IoServeState::Active),
                "Decision publication leaves the pre-fence worker as the sole completion owner"
            );
        }
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        let (reply_routes_before_failure, ingress_ownership_hash_before_failure) = {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("active Serve retains completion transport ownership");
            let reply_routes = tracked
                .reply_routes
                .clone()
                .expect("active Serve retains exact reply routes");
            let ingress_ownership = tracked
                .ingress_ownership
                .as_ref()
                .expect("active Serve retains exact Fair ownership");
            assert!(ingress_ownership.matches_reply_routes(Some(&reply_routes)));
            (
                reply_routes,
                ingress_ownership.process_local_projection_hash(),
            )
        };
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let durable_before_failure =
            fs::read(&state_path).expect("read active Serve before failed supersession");
        let temporary_state = state_path.with_extension("norito.tmp");
        fs::create_dir(&temporary_state).expect("block atomic active-worker Decision supersession");
        let error = command_rx
            .complete_serve_response(serve_admission.lifecycle_id, &response)
            .expect_err("failed negative publication must restore the active completion owner");
        assert!(
            error.contains("failed to create Sumeragi v2 Serve temporary state"),
            "unexpected active-worker supersession error: {error}"
        );
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("failed supersession retains the active Serve lifecycle");
            assert_eq!(tracked.state, V2IoServeState::Active);
            assert!(
                tracked.terminal.is_none(),
                "a failed negative publication cannot expose the response"
            );
            assert!(
                tracked.reply_routes.as_ref().is_some_and(
                    |routes| routes.has_same_exact_history(&reply_routes_before_failure)
                ),
                "failed persistence restores the exact reply-route history"
            );
            assert_eq!(
                tracked
                    .ingress_ownership
                    .as_ref()
                    .map(FairV2IngressOwnershipEvidence::process_local_projection_hash),
                Some(ingress_ownership_hash_before_failure),
                "failed persistence restores the exact Fair ownership carrier"
            );
            assert!(tracked.ingress_ownership.as_ref().is_some_and(|ownership| {
                ownership.matches_reply_routes(tracked.reply_routes.as_ref())
            }));
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert_eq!(state.serve_barrier, serve_barrier_before);
            assert_eq!(
                state.serve_barrier_predecessors,
                serve_barrier_predecessors_before
            );
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(
            fs::read(&state_path).expect("reload failed active-worker supersession"),
            durable_before_failure,
            "failed persistence cannot alter the last durable Serve snapshot"
        );
        assert_eq!(
            io_admission.queued.load(AtomicOrdering::Acquire),
            1,
            "failed suppression retains the active physical completion owner"
        );
        assert_eq!(fair_ingress_accounting_snapshot(&ingress), fair_before);
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );

        fs::remove_dir(&temporary_state).expect("unblock active-worker Decision supersession");
        assert!(
            !command_rx
                .complete_serve_response(serve_admission.lifecycle_id, &response)
                .expect("worker completion deterministically closes as a Decision negative"),
            "superseded active work must not expose a response completion"
        );
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("worker Decision negative remains indexed");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            );
            assert!(tracked.terminal.is_none());
            assert!(tracked.reply_routes.is_none());
            assert!(tracked.ingress_ownership.is_none());
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert_eq!(state.serve_barrier, serve_barrier_before);
            assert_eq!(
                state.serve_barrier_predecessors,
                serve_barrier_predecessors_before
            );
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(
            io_admission.queued.load(AtomicOrdering::Acquire),
            0,
            "typed worker supersession releases the sole physical admission owner"
        );
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            fair_before,
            "fenced and post-Decision retries never enter Fair ingress"
        );
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("active supersession queue has durable state")
            .load(&context)
            .expect("reload active worker Decision negative");
        assert!(persisted.ingress_waiters.is_empty());
        assert!(persisted.unsealed_lifecycles.is_empty());
        assert!(persisted.terminal_tombstones.is_empty());
        assert_eq!(
            persisted.next_ingress_reservation_ordinal,
            ingress_highwater_before
        );
        assert_eq!(
            persisted.next_lifecycle_admission_ordinal,
            lifecycle_highwater_before
        );
        assert_eq!(
            persisted
                .negative_tombstones
                .iter()
                .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                .collect::<Vec<_>>(),
            vec![(
                serve_admission.lifecycle_id,
                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
            )]
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire active supersession gate");
    }

    #[test]
    fn completion_pending_serve_is_suppressed_after_decision_before_delivery() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"Decision suppresses completion-pending Serve A",
            )),
            payload_hash: Hash::new(b"completion-pending Decision payload B"),
            ..proposal.subject
        };
        let body_root = TempDir::new().expect("pending supersession body root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let serve_root = TempDir::new().expect("pending supersession Serve root");
        let (command_tx, command_rx, io_admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("open completion-pending supersession queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let via = context.roster[3].validator.clone();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (serve_admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            &request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == serve_admission.lifecycle_id
        ));
        assert!(
            command_rx
                .complete_serve_response(serve_admission.lifecycle_id, &response)
                .expect("seal the pre-Decision completion"),
            "the response is initially eligible for delivery"
        );
        assert_eq!(io_admission.queued.load(AtomicOrdering::Acquire), 1);

        let fair_before = fair_ingress_accounting_snapshot(&ingress);
        let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        let (
            ingress_highwater_before,
            lifecycle_highwater_before,
            serve_by_request_before,
            serve_by_family_before,
            serve_replacement_keys_before,
            serve_barrier_before,
            serve_barrier_predecessors_before,
            reply_routes_before_failure,
            ingress_ownership_hash_before_failure,
        ) = {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("pending Serve retains its exact logical owner");
            assert_eq!(tracked.state, V2IoServeState::CompletionPending);
            assert_eq!(
                tracked.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone()))
            );
            let reply_routes = tracked
                .reply_routes
                .clone()
                .expect("pending Serve retains exact reply routes");
            let ingress_ownership = tracked
                .ingress_ownership
                .as_ref()
                .expect("pending Serve retains exact Fair ownership");
            assert!(ingress_ownership.matches_reply_routes(Some(&reply_routes)));
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            (
                state.next_serve_ingress_reservation_ordinal,
                state.next_serve_admission_ordinal,
                state.serve_by_request.clone(),
                state.serve_by_family.clone(),
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                state.serve_barrier,
                state.serve_barrier_predecessors.clone(),
                reply_routes,
                ingress_ownership.process_local_projection_hash(),
            )
        };
        command_tx
            .begin_decision_serve_reconciliation()
            .expect("fence completion delivery against the Decision WAL step");
        command_tx
            .finish_decision_serve_reconciliation(Some(decided_subject))
            .expect("publish Decision while response completion is pending");
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state
                    .serves
                    .get(&serve_admission.lifecycle_id)
                    .map(|tracked| tracked.state),
                Some(V2IoServeState::CompletionPending),
                "the completion consumer, not Decision publication, owns pending delivery"
            );
        }
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let durable_before_failure =
            fs::read(&state_path).expect("read pending response before failed supersession");
        let temporary_state = state_path.with_extension("norito.tmp");
        fs::create_dir(&temporary_state)
            .expect("block atomic completion-pending Decision supersession");
        let error = command_tx
            .serve_completion_delivery_ownership(
                serve_admission.lifecycle_id,
                response.request_hash,
            )
            .expect_err("failed negative publication must retain the pending response carrier");
        assert!(
            error.contains("failed to create Sumeragi v2 Serve temporary state"),
            "unexpected completion-pending supersession error: {error}"
        );
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("failed supersession retains the pending Serve lifecycle");
            assert_eq!(tracked.state, V2IoServeState::CompletionPending);
            assert_eq!(
                tracked.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone())),
                "failed persistence restores the undelivered response exactly"
            );
            assert!(
                tracked.reply_routes.as_ref().is_some_and(
                    |routes| routes.has_same_exact_history(&reply_routes_before_failure)
                ),
                "failed persistence restores the exact reply-route history"
            );
            assert_eq!(
                tracked
                    .ingress_ownership
                    .as_ref()
                    .map(FairV2IngressOwnershipEvidence::process_local_projection_hash),
                Some(ingress_ownership_hash_before_failure),
                "failed persistence restores the exact Fair ownership carrier"
            );
            assert!(tracked.ingress_ownership.as_ref().is_some_and(|ownership| {
                ownership.matches_reply_routes(tracked.reply_routes.as_ref())
            }));
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert_eq!(state.serve_barrier, serve_barrier_before);
            assert_eq!(
                state.serve_barrier_predecessors,
                serve_barrier_predecessors_before
            );
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(
            fs::read(&state_path).expect("reload failed pending supersession"),
            durable_before_failure,
            "failed persistence cannot alter the last durable response snapshot"
        );
        assert_eq!(
            io_admission.queued.load(AtomicOrdering::Acquire),
            1,
            "failed suppression retains the undelivered completion owner"
        );
        assert_eq!(fair_ingress_accounting_snapshot(&ingress), fair_before);
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );

        fs::remove_dir(&temporary_state).expect("unblock completion-pending Decision supersession");
        assert!(
            command_tx
                .serve_completion_delivery_ownership(
                    serve_admission.lifecycle_id,
                    response.request_hash,
                )
                .expect("classify the pending response against the durable Decision")
                .is_none(),
            "post-Decision delivery publishes no stale certified-body response"
        );
        command_tx
            .acknowledge_serve_completion(
                serve_admission.lifecycle_id,
                V2IoServeTerminal::Response(response),
            )
            .expect("retire only the suppressed completion-channel owner");
        {
            let state = command_tx.queue.lock();
            let tracked = state
                .serves
                .get(&serve_admission.lifecycle_id)
                .expect("pending-delivery Decision negative remains indexed");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            );
            assert!(tracked.terminal.is_none());
            assert!(tracked.reply_routes.is_none());
            assert!(tracked.ingress_ownership.is_none());
            assert_eq!(state.durable_decided_subject, Some(decided_subject));
            assert!(!state.decision_reconciliation_pending);
            assert_eq!(state.serve_by_request, serve_by_request_before);
            assert_eq!(state.serve_by_family, serve_by_family_before);
            assert_eq!(
                state.serve_replacements.keys().copied().collect::<Vec<_>>(),
                serve_replacement_keys_before
            );
            assert_eq!(state.serve_barrier, serve_barrier_before);
            assert_eq!(
                state.serve_barrier_predecessors,
                serve_barrier_predecessors_before
            );
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                ingress_highwater_before
            );
            assert_eq!(
                state.next_serve_admission_ordinal,
                lifecycle_highwater_before
            );
        }
        assert_eq!(io_admission.queued.load(AtomicOrdering::Acquire), 0);
        assert_eq!(fair_ingress_accounting_snapshot(&ingress), fair_before);
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("pending supersession queue has durable state")
            .load(&context)
            .expect("reload pending-delivery Decision negative");
        assert!(persisted.ingress_waiters.is_empty());
        assert!(persisted.unsealed_lifecycles.is_empty());
        assert!(persisted.terminal_tombstones.is_empty());
        assert_eq!(
            persisted.next_ingress_reservation_ordinal,
            ingress_highwater_before
        );
        assert_eq!(
            persisted.next_lifecycle_admission_ordinal,
            lifecycle_highwater_before
        );
        assert_eq!(
            persisted
                .negative_tombstones
                .iter()
                .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                .collect::<Vec<_>>(),
            vec![(
                serve_admission.lifecycle_id,
                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
            )]
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire completion-pending supersession gate");
    }

    #[test]
    fn production_restart_retires_raw_terminal_replay_waiter_without_resigning() {
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
        let body_root = TempDir::new().expect("terminal replay body root");
        let serve_root = TempDir::new().expect("terminal replay Serve root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let _ = body_store
            .store(payload.manifest().clone(), canonical_wire.clone())
            .expect("retain terminal replay body");
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let lifecycle_id = persist_terminal_serve_fixture(
            serve_root.path(),
            &context,
            &request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            &response,
        );

        {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("restore terminal before raw retry crash");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(
                    request.request(),
                    context.roster[3].validator.clone(),
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("terminal replay queue has durable state")
                .load(&context)
                .expect("reload raw terminal replay");
            assert_eq!(persisted.ingress_waiters.len(), 1);
            assert!(persisted.unsealed_lifecycles.is_empty());
            assert_eq!(persisted.terminal_tombstones.len(), 1);
            assert_eq!(
                persisted.terminal_tombstones[0].response_signature,
                response.signature
            );
            assert_eq!(persisted.next_ingress_reservation_ordinal, 1);
            assert_eq!(persisted.next_lifecycle_admission_ordinal, 1);

            // Crash before checked Fair dequeue: no physical drain, replay
            // preparation, or response post is allowed to run.
            drop(ingress);
            drop(gate);
            drop(command_rx);
            drop(command_tx);
        }

        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (command_tx, _command_rx, _admission) = production_persistent_test_io_command_channel(
            2,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            None,
            lifecycle_ordinals,
        )
        .expect("production restart retires the raw terminal replay waiter locally");
        {
            let state = command_tx.queue.lock();
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(state.commands.is_empty());
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
            let tracked = state
                .serves
                .get(&lifecycle_id)
                .expect("same terminal lifecycle remains indexed");
            assert_eq!(tracked.state, V2IoServeState::Terminal);
            assert_eq!(
                tracked.terminal.as_ref(),
                Some(&V2IoServeTerminal::Response(response.clone()))
            );
        }
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("restarted queue has durable state")
            .load(&context)
            .expect("reload locally retired terminal replay");
        assert!(persisted.ingress_waiters.is_empty());
        assert!(persisted.unsealed_lifecycles.is_empty());
        assert_eq!(persisted.terminal_tombstones.len(), 1);
        assert_eq!(persisted.terminal_tombstones[0].lifecycle_id, lifecycle_id);
        assert_eq!(
            persisted.terminal_tombstones[0].response_signature, response.signature,
            "startup retirement reuses the durable response without signing again"
        );
        assert_eq!(persisted.next_ingress_reservation_ordinal, 1);
        assert_eq!(persisted.next_lifecycle_admission_ordinal, 1);
        assert!(
            command_tx
                .try_begin_producer_episode()
                .expect("inspect producer exposure after local replay retirement")
                .is_some(),
            "all requester-independent terminal replay debt is gone before producers are exposed"
        );
    }

    #[test]
    fn production_restart_atomically_supersedes_raw_terminal_replay_waiter() {
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
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"Decision B supersedes raw terminal replay A",
            )),
            payload_hash: Hash::new(b"Decision B payload"),
            ..proposal.subject
        };
        let body_root = TempDir::new().expect("superseded replay body root");
        let serve_root = TempDir::new().expect("superseded replay Serve root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let _ = body_store
            .store(payload.manifest().clone(), canonical_wire.clone())
            .expect("retain superseded response body");
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let lifecycle_id = persist_terminal_serve_fixture(
            serve_root.path(),
            &context,
            &request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            &response,
        );

        {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("restore terminal before superseding raw retry crash");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(
                    request.request(),
                    context.roster[3].validator.clone(),
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            assert_eq!(
                command_tx
                    .queue
                    .serve_state_store
                    .as_ref()
                    .expect("superseded replay queue has durable state")
                    .load(&context)
                    .expect("reload superseded raw terminal replay")
                    .ingress_waiters
                    .len(),
                1
            );
            drop(ingress);
            drop(gate);
            drop(command_rx);
            drop(command_tx);
        }

        let (command_tx, _command_rx, _admission) = production_persistent_test_io_command_channel(
            2,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(decided_subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("production restart atomically supersedes response and replay waiter");
        {
            let state = command_tx.queue.lock();
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(state.commands.is_empty());
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
            let tracked = state
                .serves
                .get(&lifecycle_id)
                .expect("same lifecycle remains as the Decision outcome");
            assert_eq!(
                tracked.state,
                V2IoServeState::Rejected(
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
                )
            );
            assert!(tracked.terminal.is_none());
        }
        let persisted = command_tx
            .queue
            .serve_state_store
            .as_ref()
            .expect("restarted queue has durable state")
            .load(&context)
            .expect("reload atomic replay supersession");
        assert!(persisted.ingress_waiters.is_empty());
        assert!(persisted.unsealed_lifecycles.is_empty());
        assert!(persisted.terminal_tombstones.is_empty());
        assert_eq!(
            persisted
                .negative_tombstones
                .iter()
                .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
                .collect::<Vec<_>>(),
            vec![(
                lifecycle_id,
                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
            )]
        );
        assert_eq!(persisted.next_ingress_reservation_ordinal, 1);
        assert_eq!(persisted.next_lifecycle_admission_ordinal, 1);
        assert!(
            command_tx
                .try_begin_producer_episode()
                .expect("inspect producer exposure after atomic supersession")
                .is_some()
        );
    }

    #[test]
    fn production_restart_rejects_negative_tombstone_with_physical_retry_waiter() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let (request, validator_pops) = production_authenticated_serve_request(
            &context,
            &keys,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
            &[0, 1, 2, 3],
        );
        let decided_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"negative waiter Decision subject",
            )),
            payload_hash: Hash::new(b"negative waiter Decision payload"),
            ..proposal.subject
        };
        let body_root = TempDir::new().expect("negative waiter body root");
        let serve_root = TempDir::new().expect("negative waiter Serve root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let lifecycle_id = persist_negative_serve_fixture(
            serve_root.path(),
            &context,
            &request,
            CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
            1,
            CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided_subject),
        );
        let family_capacity = certified_serve_family_capacity(context.roster.len(), 8, 8)
            .expect("fixture Serve family capacity");
        let (store, mut persisted) =
            CertifiedServeStateStore::open(serve_root.path(), &context, family_capacity)
                .expect("reopen negative waiter state");
        persisted.next_ingress_reservation_ordinal = 7;
        persisted
            .ingress_waiters
            .push(PersistedCertifiedServeIngressWaiter {
                ingress_ordinal: 7,
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
            });
        store
            .persist(&persisted)
            .expect("persist impossible negative retry crash shape");

        let result = production_persistent_test_io_command_channel(
            2,
            serve_root.path(),
            &context,
            &body_store,
            &keys[0],
            &validator_pops,
            Some(0),
            Some(decided_subject),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        );
        let error = match result {
            Ok(_) => panic!("production restart must reject negative physical retry ownership"),
            Err(error) => error,
        };
        assert!(
            error.contains(
                "durable negative Serve tombstone retained an impossible physical retry waiter"
            ),
            "unexpected negative waiter rejection: {error}"
        );
        let persisted_after = store
            .load(&context)
            .expect("rejected startup leaves source-sealed state unchanged");
        assert_eq!(persisted_after, persisted);
    }

    #[test]
    fn same_height_foreign_context_is_rejected_before_every_serve_ordinal() {
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
        let mut foreign_context = context.clone();
        foreign_context.leader_seed[0] ^= 0xFF;
        let foreign_context_id = foreign_context.id();
        assert_ne!(foreign_context_id, context.id());
        let mut foreign = request.request().clone();
        foreign.round.context_id = foreign_context_id;
        foreign.certificate.round.context_id = foreign_context_id;
        foreign.certificate.proposal_round.context_id = foreign_context_id;
        foreign.signature = Signature::new(keys[1].private_key(), &foreign.signature_preimage())
            .payload()
            .to_vec();

        let body_root = TempDir::new().expect("foreign-context body root");
        let serve_root = TempDir::new().expect("foreign-context Serve root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("open foreign-context Serve queue");
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        let fair_before = fair_ingress_accounting_snapshot(&ingress);
        let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                &foreign,
                context.roster[0].validator.clone(),
            )),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        assert_eq!(fair_ingress_accounting_snapshot(&ingress), fair_before);
        assert_eq!(
            command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
            actor_ordinal_before
        );
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
            assert_eq!(state.next_serve_admission_ordinal, 0);
            assert!(state.serves.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert!(state.sender_open && state.receiver_open);
        }
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire foreign-context Serve gate");
    }

    #[test]
    fn fair_ingress_rollover_retires_ticket_before_old_service_teardown() {
        let (service, keys) = fixture_with_block_payload();
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let first = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let second = authenticated_serve_request(
            &service.context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = service.context.roster[0].validator.clone();
        let (first_tx, first_rx, _first_admission) = test_io_command_channel(2);
        let (ingress, first_gate) = gated_fair_ingress(&service.context, &first_tx);

        assert!(matches!(
            ingress.try_push(certified_serve_inbound(first.request(), via.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(first_tx.queue.lock().serve_ingress_reservation.is_some());
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&first_gate)
            .expect("rollover retires every old-height ticket");
        assert_eq!(ingress.len(), 0);
        {
            let state = first_tx.queue.lock();
            assert!(state.sender_open);
            assert!(state.receiver_open);
            assert!(state.serve_ingress_reservation.is_none());
            assert_eq!(
                state.serve_ingress_waiters.len(),
                1,
                "rollover detaches only the volatile carrier"
            );
            assert_eq!(state.serves.len(), 1);
            assert!(
                state
                    .serves
                    .values()
                    .all(|tracked| tracked.state == V2IoServeState::AwaitingRetry)
            );
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
        }

        let (second_tx, _second_rx, _second_admission) = test_io_command_channel(2);
        let second_gate = CertifiedServeIngressGate {
            queue: Arc::clone(&second_tx.queue),
        };
        ingress
            .configure_roster(
                service
                    .context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone()),
            )
            .expect("reconfigure next-height fair roster");
        ingress
            .bind_certified_serve_gate(second_gate.clone())
            .expect("bind next-height Serve owner");
        ingress.open().expect("open next-height fair ingress");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(second.request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            ingress.state.lock().last_admission_ordinal,
            2,
            "rollover preserves the process-local fair admission high-watermark"
        );
        assert_eq!(
            second_tx
                .queue
                .lock()
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.id),
            Some(CertifiedServeIngressReservationId(1)),
            "the new per-height queue starts its own bounded internal owner space"
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&second_gate)
            .expect("retire next-height fixture ticket");
        assert!(second_tx.queue.lock().serve_ingress_reservation.is_none());
        assert_eq!(second_tx.queue.lock().serve_ingress_waiters.len(), 1);
        drop(first_rx);
    }
