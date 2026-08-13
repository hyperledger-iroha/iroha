#[test]
fn proposal_signing_restores_chunks_only_when_outbound_payload_is_absent() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (command_tx, command_rx, admission) = test_io_command_channel(2);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(2);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );

    service
        .enqueue_consensus_sign(ConsensusSignTask::for_test(
            9,
            tag,
            super::super::v2::SignRequest::Proposal(proposal.clone()),
        ))
        .expect("queue replayed proposal signature");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign {
            restore_outbound_payload: true,
            ..
        })
    ));

    service
        .register_outbound_payload(tag, payload)
        .expect("register live proposal payload");
    service
        .enqueue_consensus_sign(ConsensusSignTask::for_test(
            10,
            tag,
            super::super::v2::SignRequest::Proposal(proposal),
        ))
        .expect("queue live proposal signature");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign {
            restore_outbound_payload: false,
            ..
        })
    ));

    // No worker owns this synthetic channel; remove it before service Drop
    // attempts the production shutdown handshake.
    drop(service.io.take());
}

#[test]
fn completion_sources_alternate_under_simultaneous_bursts() {
    let (mut service, _) = fixture();
    let (command_tx, _command_rx, admission) = test_io_command_channel(2);
    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    completion_tx
        .try_send(V2IoCompletion::AuxiliaryNoop)
        .expect("first I/O completion");
    completion_tx
        .try_send(V2IoCompletion::AuxiliaryNoop)
        .expect("second I/O completion");

    let payload = b"completion fairness body";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"completion fairness block",
        )),
        payload_hash: Hash::new(payload),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let manifest = encode_payload(&service.context, round, subject, payload)
        .expect("encode completion fairness body")
        .manifest()
        .clone();
    let completion_tag = EventTag::new(
        service.context.height,
        round.view,
        Generation::new(service.context.height),
    );
    for id in 1..=2 {
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: BodyFetchTask::ordinary_for_test(id, completion_tag, manifest.clone()),
                manifest: manifest.clone(),
                body: payload.to_vec().into(),
            });
    }

    assert!(matches!(
        service.take_next_completion(true),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io { .. }),
            ..
        }
    ));
    let first_local = service.take_next_completion(true);
    let IoCompletionTake {
        completion:
            Some(PendingServiceCompletion::Local(LocalCompletion::Reconstructed {
                task: first_task,
                ..
            })),
        ..
    } = first_local
    else {
        panic!("the local source must follow the first I/O completion");
    };
    service
        .complete_body_reconstruction_fetch(&first_task)
        .expect("successful reducer admission retires the exact local owner");
    assert!(matches!(
        service.take_next_completion(true),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io { .. }),
            ..
        }
    ));
    let second_local = service.take_next_completion(true);
    let IoCompletionTake {
        completion:
            Some(PendingServiceCompletion::Local(LocalCompletion::Reconstructed {
                task: second_task,
                ..
            })),
        ..
    } = second_local
    else {
        panic!("the local source must follow the second I/O completion");
    };
    service
        .complete_body_reconstruction_fetch(&second_task)
        .expect("successful reducer admission retires the exact local owner");
    assert!(matches!(
        service.take_next_completion(true),
        IoCompletionTake {
            completion: None,
            retained_runtime: false
        }
    ));

    drop(service.io.take());
}

#[test]
fn exact_serve_predecessor_episode_services_older_local_without_admitting_later_io() {
    let (mut service, _) = fixture();
    let (command_tx, _command_rx, admission) = test_io_command_channel(2);
    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    let later_ordinal = 70_u128;
    let later_work_id = EffectWorkId::for_test(70);
    try_send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id: later_work_id,
            signature: vec![0x70],
            outbound_payload: None,
        },
        Some(later_ordinal),
    )
    .expect("retain a completion created after the exact Serve ticket");
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    let payload = b"older local reconstruction";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"older local reconstruction block",
        )),
        payload_hash: Hash::new(payload),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let manifest = encode_payload(&service.context, round, subject, payload)
        .expect("encode older local reconstruction")
        .manifest()
        .clone();
    let tag = EventTag::new(
        service.context.height,
        round.view,
        Generation::new(service.context.height),
    );
    let older_task = BodyFetchTask::ordinary_for_test(7, tag, manifest.clone());
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: older_task.clone(),
            manifest,
            body: payload.to_vec().into(),
        });

    let first_ticket_ordinal = 50;
    assert_eq!(
        service
            .certified_serve_predecessor_completion_evidence(true, first_ticket_ordinal,)
            .expect("project the completed predecessor without consuming it")
            .map(ExactServePredecessorCompletionEvidence::lifecycle_ordinal),
        Some(older_task.lifecycle_ordinal()),
        "the least strict local predecessor must reopen the exact Serve episode"
    );
    assert!(
        service
            .certified_serve_predecessor_completion_evidence(false, first_ticket_ordinal,)
            .expect("project the capacity-blocked predecessor")
            .is_none(),
        "a completion that needs runtime capacity cannot reopen before capacity exists"
    );
    let IoCompletionTake {
        completion:
            Some(PendingServiceCompletion::Local(LocalCompletion::Reconstructed { task, .. })),
        retained_runtime: false,
    } = service.take_exact_serve_predecessor_completion(true, first_ticket_ordinal)
    else {
        panic!("the strictly older local reconstruction must own the bounded episode");
    };
    assert_eq!(task, older_task);
    service
        .complete_body_reconstruction_fetch(&task)
        .expect("successful predecessor admission retires the exact local owner");
    assert!(service.local_completions.is_empty());
    assert!(service.held_io_completion.is_none());
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion corridor")
            .completion_snapshot(Instant::now())
            .depth,
        1,
        "the later I/O completion must remain outside runtime"
    );

    for fresh_ticket_ordinal in first_ticket_ordinal..=later_ordinal {
        assert!(
            service
                .certified_serve_predecessor_completion_evidence(true, fresh_ticket_ordinal,)
                .expect("project the equal-or-later I/O completion")
                .is_none(),
            "an equal-or-later completion cannot reopen this ticket"
        );
        assert!(matches!(
            service.take_exact_serve_predecessor_completion(true, fresh_ticket_ordinal),
            IoCompletionTake {
                completion: None,
                retained_runtime: false,
            }
        ));
        assert!(service.held_io_completion.is_none());
    }
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion corridor")
            .completion_snapshot(Instant::now())
            .depth,
        1,
        "retransmission churn cannot let an equal-or-later completion overtake its ticket"
    );

    assert_eq!(
        service
            .certified_serve_predecessor_completion_evidence(true, later_ordinal + 1)
            .expect("project the newly strict I/O predecessor")
            .map(ExactServePredecessorCompletionEvidence::lifecycle_ordinal),
        Some(later_ordinal),
        "the exact I/O completion becomes eligible only below a later ticket"
    );
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion corridor")
            .completion_snapshot(Instant::now())
            .depth,
        1,
        "completion evidence must not consume the projected I/O result"
    );

    assert!(matches!(
        service.take_exact_serve_predecessor_completion(true, later_ordinal + 1),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io {
                completion: V2IoCompletion::Signature { work_id, .. },
                ownership_position: 0,
            }),
            retained_runtime: false,
        } if work_id == later_work_id
    ));
    service
        .io
        .as_ref()
        .expect("attached completion corridor")
        .admission
        .acknowledge_completion_at(0);
    drop(service.io.take());
}

#[test]
fn timeout_recovery_completion_prefix_includes_cut_and_excludes_successor() {
    let (mut service, _) = fixture();
    let (command_tx, _command_rx, admission) = test_io_command_channel(2);
    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    let timeout_ordinal = 50_u128;
    let timeout_work_id = EffectWorkId::for_test(50);
    let successor_work_id = EffectWorkId::for_test(51);
    for (work_id, ordinal) in [
        (timeout_work_id, timeout_ordinal),
        (successor_work_id, timeout_ordinal + 1),
    ] {
        try_send_tracked_completion_with_lifecycle_ordinal(
            &completion_tx,
            &admission,
            V2IoCompletion::Signature {
                work_id,
                signature: vec![u8::try_from(ordinal).expect("small ordinal")],
                outbound_payload: None,
            },
            Some(ordinal),
        )
        .expect("retain exact timeout-boundary completion");
    }
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    assert!(matches!(
        service.take_timeout_recovery_prefix_completion(true, timeout_ordinal),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io {
                completion: V2IoCompletion::Signature { work_id, .. },
                ownership_position: 0,
            }),
            retained_runtime: false,
        } if work_id == timeout_work_id
    ));
    service
        .io
        .as_ref()
        .expect("attached completion corridor")
        .admission
        .acknowledge_completion_at(0);
    assert!(matches!(
        service.take_timeout_recovery_prefix_completion(true, timeout_ordinal),
        IoCompletionTake {
            completion: None,
            retained_runtime: false,
        }
    ));
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion corridor")
            .completion_snapshot(Instant::now())
            .depth,
        1,
        "the T+1 producer stays outside the inclusive T prefix"
    );
    assert!(matches!(
        service.take_timeout_recovery_prefix_completion(true, timeout_ordinal + 1),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io {
                completion: V2IoCompletion::Signature { work_id, .. },
                ownership_position: 0,
            }),
            retained_runtime: false,
        } if work_id == successor_work_id
    ));
    service
        .io
        .as_ref()
        .expect("attached completion corridor")
        .admission
        .acknowledge_completion_at(0);
    drop(service.io.take());
}

#[test]
fn repeated_exact_serve_claims_close_all_older_sources_before_later_io() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = service.context.roster[0].validator.clone();
    let (command_tx, _command_rx, admission) = test_io_command_channel(4);
    for expected in 1_u128..=4 {
        assert_eq!(
            command_tx
                .queue
                .lifecycle_ordinals
                .reserve_one()
                .expect("reserve pre-ticket lifecycle ordinal"),
            expected
        );
    }
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect repeated-claim barrier")
        .expect("admitted exact request owns a barrier");
    assert_eq!(barrier.scheduler_ordinal(), 5);
    assert_eq!(
        command_tx
            .queue
            .lifecycle_ordinals
            .reserve_one()
            .expect("reserve adversarial post-ticket lifecycle ordinal"),
        6
    );

    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    let older_io_work_id = EffectWorkId::for_test(1);
    try_send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id: older_io_work_id,
            signature: vec![0x01],
            outbound_payload: None,
        },
        Some(1),
    )
    .expect("retain first strictly older I/O completion");
    let later_io_work_id = EffectWorkId::for_test(6);
    try_send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id: later_io_work_id,
            signature: vec![0x06],
            outbound_payload: None,
        },
        Some(6),
    )
    .expect("retain later-rank I/O completion");
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    let payload = b"same-rank replacement source";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"same-rank replacement source block",
        )),
        payload_hash: Hash::new(payload),
    };
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let manifest = encode_payload(&service.context, round, subject, payload)
        .expect("encode same-rank replacement source")
        .manifest()
        .clone();
    let tag = EventTag::new(
        service.context.height,
        round.view,
        Generation::new(service.context.height),
    );
    let older_local_task = BodyFetchTask::ordinary_for_test(1, tag, manifest.clone());
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: older_local_task.clone(),
            manifest,
            body: payload.to_vec().into(),
        });

    assert!(
        service
            .claim_certified_serve_runtime_episode(barrier)
            .expect("claim first bounded predecessor turn")
    );
    assert!(matches!(
        service.take_exact_serve_predecessor_completion(
            true,
            barrier.scheduler_ordinal()
        ),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Io {
                completion: V2IoCompletion::Signature { work_id, .. },
                ownership_position: 0,
            }),
            retained_runtime: false,
        } if work_id == older_io_work_id
    ));
    service
        .io
        .as_ref()
        .expect("attached completion corridor")
        .admission
        .acknowledge_completion_at(0);
    service
        .finish_certified_serve_runtime_episode_turn(barrier, true)
        .expect("remaining same-rank local owner reopens the episode");

    assert!(
        service
            .claim_certified_serve_runtime_episode(barrier)
            .expect("claim second bounded predecessor turn")
    );
    let IoCompletionTake {
        completion:
            Some(PendingServiceCompletion::Local(LocalCompletion::Reconstructed { task, .. })),
        retained_runtime: false,
    } = service.take_exact_serve_predecessor_completion(true, barrier.scheduler_ordinal())
    else {
        panic!("the second turn must select the same-rank replacement owner");
    };
    assert_eq!(task, older_local_task);
    service
        .complete_body_reconstruction_fetch(&task)
        .expect("retire second strictly older source");
    assert!(matches!(
        service.take_exact_serve_predecessor_completion(true, barrier.scheduler_ordinal()),
        IoCompletionTake {
            completion: None,
            retained_runtime: false,
        }
    ));
    service
        .finish_certified_serve_runtime_episode_turn(barrier, false)
        .expect("later-rank I/O cannot keep the older-owner episode open");
    assert!(
        !service
            .claim_certified_serve_runtime_episode(barrier)
            .expect("completed episode cannot be reclaimed")
    );
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion corridor")
            .completion_snapshot(Instant::now())
            .depth,
        1,
        "later-rank completion remains outside runtime until after target ingress"
    );
    assert!(matches!(
        service.take_exact_serve_predecessor_completion(true, barrier.scheduler_ordinal()),
        IoCompletionTake {
            completion: None,
            retained_runtime: false,
        }
    ));

    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire repeated-claim gate");
    drop(service.io.take());
}

#[test]
fn completed_exact_serve_episode_reopens_once_for_new_runtime_witness() {
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
    let (command_tx, command_rx, _admission) = test_io_command_channel(4);
    for expected in 1_u128..=2 {
        assert_eq!(
            command_tx
                .queue
                .lifecycle_ordinals
                .reserve_one()
                .expect("reserve predecessor lifecycle ordinal"),
            expected
        );
    }
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect predecessor-witness barrier")
        .expect("admitted exact request owns a barrier");
    assert_eq!(barrier.scheduler_ordinal(), 3);

    let first = ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 1, 1);
    assert!(
        command_tx
            .claim_serve_runtime_episode(barrier)
            .expect("claim initial predecessor turn")
    );
    assert!(
        !command_tx
            .observe_serve_predecessor_episode_witness(barrier, first)
            .expect("record initial runtime witness")
    );
    command_tx
        .finish_serve_runtime_episode_turn(barrier, false)
        .expect("seal exhausted initial predecessor turn");
    assert!(
        !command_tx
            .observe_serve_predecessor_episode_witness(barrier, first)
            .expect("same physical episode must coalesce")
    );
    assert!(
        !command_tx
            .claim_serve_runtime_episode(barrier)
            .expect("same witness cannot reopen a completed turn")
    );

    let conflicting =
        ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 1);
    assert!(
        command_tx
            .observe_serve_predecessor_episode_witness(barrier, conflicting)
            .is_err(),
        "one episode cannot change its exact predecessor evidence"
    );
    let skipped = ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 3);
    assert!(
        command_tx
            .observe_serve_predecessor_episode_witness(barrier, skipped)
            .is_err(),
        "the consumer cannot skip a predecessor episode ordinal"
    );

    let replenished =
        ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 2);
    assert!(
        command_tx
            .observe_serve_predecessor_episode_witness(barrier, replenished)
            .expect("strictly newer runtime witness reopens the target")
    );
    assert!(
        command_tx
            .claim_serve_runtime_episode(barrier)
            .expect("claim exactly one replenished predecessor turn")
    );
    assert!(
        !command_tx
            .observe_serve_predecessor_episode_witness(barrier, replenished)
            .expect("repeated replenishment witness must stutter")
    );
    command_tx
        .finish_serve_runtime_episode_turn(barrier, false)
        .expect("seal replenished predecessor turn");
    assert!(
        command_tx
            .observe_serve_predecessor_episode_witness(barrier, first)
            .is_err(),
        "a consumed predecessor episode cannot regress"
    );

    let (admission, committed) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester),
        &request,
    );
    assert!(matches!(committed, CertifiedServeCommit::Queued));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == admission.lifecycle_id
    ));
    let producer_episode = command_tx
        .try_begin_producer_episode()
        .expect("consume the post-Serve producer handoff")
        .expect("final target retirement owes one producer episode");
    drop(producer_episode);

    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire predecessor-witness gate");
}

#[test]
fn exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission() {
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
    let (command_tx, command_rx, _admission) = test_io_command_channel(1);
    assert_eq!(
        command_tx
            .queue
            .lifecycle_ordinals
            .reserve_one()
            .expect("reserve the causal predecessor ordinal"),
        1
    );
    command_tx
        .try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(90),
                subject: proposal.subject,
            },
        )
        .expect("fill the sole physical slot with a frozen Control predecessor");
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect the exact barrier")
        .expect("the admitted target owns a barrier");
    assert_eq!(barrier.scheduler_ordinal(), 2);
    assert!(
        command_tx
            .claim_serve_runtime_episode(barrier)
            .expect("claim the bounded causal predecessor turn")
    );
    assert!(
        !command_tx
            .serve_runtime_predecessor_capacity_available(barrier)
            .expect("inspect the full frozen prefix"),
        "the runner must wait instead of dispatching a retained effect into a full queue"
    );

    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let sign = |ordinal| V2IoCommand::Sign {
        task: ConsensusSignTask::for_test(
            ordinal,
            tag,
            super::super::v2::SignRequest::Proposal(proposal.clone()),
        ),
        restore_outbound_payload: false,
    };
    assert!(matches!(
        command_tx.try_send(sign(1)),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(matches!(
        command_tx.try_send(sign(2)),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(matches!(
        command_tx.try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(91),
                subject: proposal.subject,
            },
        ),
        Err(V2IoTrySendError::Full(_))
    ));

    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(90),
            ..
        })
    ));
    assert!(
        command_tx
            .serve_runtime_predecessor_capacity_available(barrier)
            .expect("the drained Control prefix releases causal capacity")
    );
    command_tx
        .try_send(sign(1))
        .expect("the claimed turn admits its strictly older causal owner");
    {
        let state = command_tx.queue.lock();
        assert!(matches!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.runtime_episode),
            Some(CertifiedServeRuntimeEpisodeState::Claimed {
                predecessor_ordinal: Some(1)
            })
        ));
    }
    assert!(matches!(
        command_tx.try_send(sign(2)),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(matches!(
        command_tx.try_send(sign(3)),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(matches!(
        command_tx.try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(92),
                subject: proposal.subject,
            },
        ),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign { task, .. }) if task.id() == EffectWorkId::for_test(1)
    ));
    command_rx.complete_work(EffectWorkId::for_test(1));
    command_tx.acknowledge_completion(EffectWorkId::for_test(1));

    let _prepared = command_tx
        .prepare_reserved_serve(
            CertifiedServeOwnerKey::Roster(requester.clone()),
            request.clone(),
        )
        .expect("materialize the uncommitted exact target");
    {
        let state = command_tx.queue.lock();
        assert!(matches!(
            state.commands.front(),
            Some(V2IoCommand::Serve { .. })
        ));
    }
    assert!(
        command_tx
            .serve_runtime_predecessor_capacity_available(barrier)
            .expect("the older owner can borrow the uncommitted target unit")
    );
    command_tx
        .try_send(sign(1))
        .expect("same-rank causal replacement precedes the materialized target");
    {
        let state = command_tx.queue.lock();
        assert!(matches!(
            state.commands.front(),
            Some(V2IoCommand::Sign { task, .. })
                if task.id() == EffectWorkId::for_test(1)
        ));
        let lifecycle_id = state
            .serve_barrier
            .expect("the exact logical barrier remains installed");
        assert_eq!(
            state.serves.get(&lifecycle_id).map(|serve| serve.state),
            Some(V2IoServeState::PendingCapacity),
            "only the physical target unit transfers to the same-rank predecessor"
        );
    }
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign { task, .. }) if task.id() == EffectWorkId::for_test(1)
    ));
    {
        let state = command_tx.queue.lock();
        assert!(matches!(
            state.commands.front(),
            Some(V2IoCommand::Serve { .. })
        ));
    }
    command_rx.complete_work(EffectWorkId::for_test(1));
    command_tx.acknowledge_completion(EffectWorkId::for_test(1));
    command_tx
        .finish_serve_runtime_episode_turn(barrier, false)
        .expect("the mandatory full recheck seals the exhausted causal episode");
    assert!(
        !command_tx
            .claim_serve_runtime_episode(barrier)
            .expect("sealed causal ownership cannot be resurrected")
    );

    let (admission, committed) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester),
        &request,
    );
    assert!(matches!(committed, CertifiedServeCommit::Queued));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. }) if lifecycle_id == admission.lifecycle_id
    ));
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire the full Control-prefix fixture gate");
}

#[test]
fn worker_completion_is_retained_behind_a_full_runtime_fifo() {
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let channel_capacity = admission.capacity();
    let (command_tx, _command_rx) = v2_io_command_channel(
        channel_capacity,
        channel_capacity.max(1),
        channel_capacity.max(1),
        channel_capacity.max(1),
        Arc::clone(&admission),
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(channel_capacity);
    try_send_tracked_completion(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id: EffectWorkId::for_test(76),
            signature: vec![0x4b],
            outbound_payload: None,
        },
    )
    .expect("retain one completed worker result");
    let snapshot_at = Instant::now() + Duration::from_millis(250);
    let io = V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    };

    assert!(
        !io.record_completion_service_attempt(1),
        "a free runtime slot must not accrue completion service debt"
    );
    for expected_debt in 1..=3 {
        assert!(
            io.record_completion_service_attempt(0),
            "the full runtime FIFO must retain the oldest worker completion"
        );
        let snapshot = io.completion_snapshot(snapshot_at);
        assert_eq!(snapshot.depth, 1);
        assert_eq!(snapshot.capacity, channel_capacity + 2);
        assert!(
            snapshot
                .oldest_age
                .is_some_and(|age| age >= Duration::from_millis(250))
        );
        assert_eq!(snapshot.max_service_debt, expected_debt);
    }

    assert!(matches!(
        io.try_recv_completion_unacknowledged(),
        Ok(V2IoCompletion::Signature { work_id, .. })
            if work_id == EffectWorkId::for_test(76)
    ));
    io.admission.acknowledge_completion_at(0);
    let drained = io.completion_snapshot(snapshot_at + Duration::from_millis(250));
    assert_eq!(drained.depth, 0);
    assert_eq!(drained.capacity, channel_capacity + 2);
    assert_eq!(drained.oldest_age, None);
    assert_eq!(drained.max_service_debt, 0);
}

#[test]
fn production_drain_publishes_worker_completion_behind_full_runtime_fifo() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let mut executor = V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(1, 1),
        BTreeMap::new(),
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct saturated effect executor");
    assert_eq!(executor.remaining_completion_capacity(), 0);

    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let channel_capacity = admission.capacity();
    let (command_tx, command_rx) = v2_io_command_channel(
        channel_capacity,
        channel_capacity.max(1),
        channel_capacity.max(1),
        channel_capacity.max(1),
        Arc::clone(&admission),
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(channel_capacity);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let work_id = EffectWorkId::for_test(77);
    let later_work_id = EffectWorkId::for_test(78);
    command_tx
        .try_send(V2IoCommand::Sign {
            task: ConsensusSignTask::for_test(
                work_id.get(),
                tag,
                super::super::v2::SignRequest::Proposal(proposal.clone()),
            ),
            restore_outbound_payload: false,
        })
        .expect("queue runtime-producing work");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign { .. })
    ));
    command_rx.complete_work(work_id);
    command_tx
        .try_send(V2IoCommand::Sign {
            task: ConsensusSignTask::for_test(
                later_work_id.get(),
                tag,
                super::super::v2::SignRequest::Proposal(proposal),
            ),
            restore_outbound_payload: false,
        })
        .expect("queue later runtime-producing work");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign { .. })
    ));
    command_rx.complete_work(later_work_id);
    try_send_tracked_completion(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id,
            signature: vec![0x5a],
            outbound_payload: None,
        },
    )
    .expect("retain runtime-producing completion");
    try_send_tracked_completion(&completion_tx, &admission, V2IoCompletion::AuxiliaryNoop)
        .expect("retain auxiliary completion behind runtime work");
    try_send_tracked_completion(
        &completion_tx,
        &admission,
        V2IoCompletion::Signature {
            work_id: later_work_id,
            signature: vec![0x6b],
            outbound_payload: None,
        },
    )
    .expect("retain later runtime completion behind auxiliary work");
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("full runtime still services auxiliary completion"),
        1
    );
    let first = service
        .last_status
        .as_ref()
        .expect("backpressure publishes effect status");
    assert_eq!(first.queued_runtime_completions, 1);
    assert_eq!(first.effect_completion_queue.depth, 2);
    assert_eq!(first.effect_completion_queue.capacity, channel_capacity + 2);
    assert!(first.effect_completion_queue.oldest_age.is_some());
    assert_eq!(first.effect_completion_queue.max_service_debt, 1);
    assert!(matches!(
        service.held_io_completion.as_ref(),
        Some(V2IoCompletion::Signature { work_id: held, .. }) if *held == work_id
    ));
    assert_eq!(
        service
            .io
            .as_ref()
            .expect("attached completion owner")
            .completion_requires_runtime_capacity_at(1),
        Some(true),
        "the later runtime result must remain in the worker FIFO"
    );
    assert_eq!(
        command_rx
            .queue
            .lock()
            .work
            .get(&work_id)
            .map(|work| work.state),
        Some(V2IoWorkState::CompletionPending),
        "the held runtime result must remain unacknowledged"
    );
    assert_eq!(
        command_rx
            .queue
            .lock()
            .work
            .get(&later_work_id)
            .map(|work| work.state),
        Some(V2IoWorkState::CompletionPending),
        "the later runtime result must not be popped or acknowledged"
    );

    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("repeated full runtime retains worker result"),
        0
    );
    let second = service
        .last_status
        .as_ref()
        .expect("repeated backpressure republishes effect status");
    assert_eq!(second.effect_completion_queue.depth, 2);
    assert_eq!(second.effect_completion_queue.max_service_debt, 2);
    service.retire_held_io_completion();
    let drained = service
        .io
        .as_ref()
        .expect("attached completion owner")
        .completion_snapshot(Instant::now());
    assert_eq!(drained.depth, 1);
    assert!(
        !command_rx.queue.lock().work.contains_key(&work_id),
        "retiring the consumed held result acknowledges exact work ownership"
    );
    assert_eq!(
        command_rx
            .queue
            .lock()
            .work
            .get(&later_work_id)
            .map(|work| work.state),
        Some(V2IoWorkState::CompletionPending)
    );
    assert!(matches!(
        service
            .io
            .as_ref()
            .expect("attached completion owner")
            .try_recv_completion(),
        Ok(V2IoCompletion::Signature { work_id, .. }) if work_id == later_work_id
    ));
    let drained = service
        .io
        .as_ref()
        .expect("attached completion owner")
        .completion_snapshot(Instant::now());
    assert_eq!(drained.depth, 0);
    assert_eq!(drained.oldest_age, None);
    assert_eq!(drained.max_service_debt, 0);
    drop(service.io.take());
}

#[test]
fn successful_auxiliary_drain_republishes_cleared_completion_ownership() {
    let (mut service, _) = fixture();
    let mut executor = V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(0, 1),
        BTreeMap::new(),
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct effect executor");
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let channel_capacity = admission.capacity();
    let (command_tx, _command_rx) = v2_io_command_channel(
        channel_capacity,
        channel_capacity.max(1),
        channel_capacity.max(1),
        channel_capacity.max(1),
        Arc::clone(&admission),
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(channel_capacity);
    try_send_tracked_completion(&completion_tx, &admission, V2IoCompletion::AuxiliaryNoop)
        .expect("retain auxiliary completion");
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    assert!(service.last_status.is_none());
    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("service auxiliary completion"),
        1
    );
    let published = service
        .last_status
        .as_ref()
        .expect("successful drain republishes service-owned state");
    assert_eq!(published.effect_completion_queue.depth, 0);
    assert_eq!(published.effect_completion_queue.oldest_age, None);
    assert_eq!(published.effect_completion_queue.max_service_debt, 0);
    drop(service.io.take());
}

#[test]
fn auxiliary_completion_drain_is_batch_bounded() {
    let (mut service, _) = fixture();
    let mut executor = V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(0, 1),
        BTreeMap::new(),
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct effect executor");
    let admission = Arc::new(
        V2IoAdmission::new(MAX_COMPLETION_DRAIN_BATCH + 1, 1).expect("bounded I/O admission"),
    );
    let channel_capacity = admission.capacity();
    let (command_tx, _command_rx) = v2_io_command_channel(
        channel_capacity,
        channel_capacity.max(1),
        channel_capacity.max(1),
        channel_capacity.max(1),
        Arc::clone(&admission),
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(channel_capacity);
    for _ in 0..=MAX_COMPLETION_DRAIN_BATCH {
        try_send_tracked_completion(&completion_tx, &admission, V2IoCompletion::AuxiliaryNoop)
            .expect("retain bounded auxiliary burst");
    }
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });

    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("drain one bounded batch"),
        MAX_COMPLETION_DRAIN_BATCH
    );
    assert_eq!(
        service
            .last_status
            .as_ref()
            .expect("batch drain republishes status")
            .effect_completion_queue
            .depth,
        1
    );
    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("drain remaining auxiliary result"),
        1
    );
    assert_eq!(
        service
            .last_status
            .as_ref()
            .expect("final drain republishes status")
            .effect_completion_queue
            .depth,
        0
    );
    drop(service.io.take());
}

#[test]
fn cancelling_fetch_consumes_queued_reconstruction_owner() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(41, tag, payload.manifest().clone());
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: payload.manifest().clone(),
            body: canonical_wire.into(),
        });

    service
        .cancel_body_fetch(&task)
        .expect("queued reconstruction owns the cancelled fetch");

    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());
}

#[test]
fn fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(47, tag, payload.manifest().clone());
    let rebound_tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height + 1),
    );
    let rebound = task
        .rebind_consumer(rebound_tag)
        .expect("later same-view generation rebinds immutable fetch work");
    service
        .enqueue_body_fetch(task.clone())
        .expect("open exact live reconstruction session");
    service
        .rebind_body_fetch(&task, rebound.clone())
        .expect("rebind live reconstruction consumer");
    assert_eq!(service.fetches[&task.id()].task, rebound);
    assert_eq!(
        service
            .fetch_by_manifest
            .get(&HashOf::new(payload.manifest())),
        Some(&task.id())
    );
    assert!(!service.output_guard.restart_required());

    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(48, tag, payload.manifest().clone());
    let rebound_tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height + 1),
    );
    let rebound = task
        .rebind_consumer(rebound_tag)
        .expect("later same-view generation rebinds queued reconstruction");
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: payload.manifest().clone(),
            body: canonical_wire.into(),
        });
    service
        .rebind_body_fetch(&task, rebound.clone())
        .expect("rebind queued reconstruction consumer");
    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert!(matches!(
        service.take_next_completion(true),
        IoCompletionTake {
            completion: Some(PendingServiceCompletion::Local(
                LocalCompletion::Reconstructed { task, manifest, .. }
            )),
            ..
        } if task == rebound && manifest == *payload.manifest()
    ));
    assert!(!service.output_guard.restart_required());
}

#[test]
fn invalid_fetch_consumer_rebind_fails_closed_without_consuming_owner() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(49, tag, payload.manifest().clone());
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: payload.manifest().clone(),
            body: canonical_wire.into(),
        });

    let error = service
        .rebind_body_fetch(&task, task.clone())
        .expect_err("same-view consumer rebind must fail closed");

    assert!(error.contains("invalid consumer rebind"));
    assert_eq!(service.local_completions.len(), 1);
    assert!(service.output_guard.restart_required());
}

#[test]
fn retransmitting_fetch_with_queued_reconstruction_is_idempotent() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(45, tag, payload.manifest().clone());
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: payload.manifest().clone(),
            body: canonical_wire.into(),
        });

    service
        .enqueue_body_fetch(task)
        .expect("queued reconstruction makes retransmission idempotent");

    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert_eq!(service.local_completions.len(), 1);
    assert!(!service.output_guard.restart_required());
}

#[test]
fn retransmitting_fetch_with_conflicting_queued_manifest_fails_closed() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(46, tag, payload.manifest().clone());
    let mut conflicting_manifest = payload.manifest().clone();
    conflicting_manifest.payload_size_bytes = conflicting_manifest
        .payload_size_bytes
        .checked_add(1)
        .expect("small fixture payload size");
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: conflicting_manifest,
            body: canonical_wire.into(),
        });

    let error = service
        .enqueue_body_fetch(task)
        .expect_err("conflicting queued result must fail closed");

    assert!(error.contains("inconsistent manifest ownership"));
    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert_eq!(service.local_completions.len(), 1);
    assert!(service.output_guard.restart_required());
}

#[test]
fn cancelling_fetch_consumes_live_session_and_manifest_owner() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(42, tag, payload.manifest().clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("open exact live reconstruction session");

    service
        .cancel_body_fetch(&task)
        .expect("live reconstruction session owns the cancelled fetch");

    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());
}

#[test]
fn invalid_reconstruction_waits_for_reducer_authorized_retirement() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let mut invalid_body = body.clone();
    invalid_body[0] ^= 1;
    let invalid_chunks = wire::encode_payload_chunks(service.context.da_layout, &invalid_body)
        .expect("canonically encode the alternate reconstruction body");
    // Deliberate negative data: the alternate bytes use complete RS16
    // geometry, but the manifest remains bound to the original proposal
    // subject so reconstruction reaches the semantic payload-hash check.
    let invalid_manifest = wire::PayloadManifest::derive(
        &service.context,
        proposal.round,
        proposal.subject,
        u64::try_from(invalid_body.len()).expect("body length"),
        &invalid_chunks,
    )
    .expect("structurally valid invalid manifest");
    assert_ne!(invalid_manifest, *payload.manifest());
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(61, tag, invalid_manifest.clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("open invalid remote reconstruction session");
    let mut chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&invalid_manifest),
        index: 0,
        bytes: invalid_chunks[0].clone(),
        sender: 0,
        signature: Vec::new(),
    };
    chunk.signature = Signature::new(
        keys[0].private_key(),
        &chunk
            .signature_preimage(&service.context, &invalid_manifest)
            .expect("chunk preimage"),
    )
    .payload()
    .to_vec();
    let sender = service.context.roster[0].validator.clone();
    let authenticated =
        authenticate_payload_chunk(&service.context, &invalid_manifest, chunk, &sender)
            .expect("authenticate chunk committed by invalid manifest");

    assert_eq!(
        service
            .accept_authenticated_chunk(&task, authenticated)
            .expect("invalid remote reconstruction is not a local service failure"),
        AuthenticatedChunkDisposition::Rejected
    );
    assert_eq!(service.fetches[&task.id()].task, task);
    assert_eq!(
        service.fetch_by_manifest[&HashOf::new(&invalid_manifest)],
        task.id()
    );
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());

    service
        .complete_body_reconstruction_fetch(&task)
        .expect("the reducer retires the exact rejected reconstruction owner");
    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());
    assert!(!service.output_guard.restart_required());
}

#[test]
fn cancelling_unowned_fetch_fails_closed() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(43, tag, payload.manifest().clone());

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("missing service ownership must fail closed");

    assert!(error.contains("has no service owner"));
    assert!(service.output_guard.restart_required());
}

#[test]
fn cancelling_fetch_with_overlapping_owners_fails_closed() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(44, tag, payload.manifest().clone());
    let work_id = task.id();
    let manifest_hash = HashOf::new(payload.manifest());
    service
        .enqueue_body_fetch(task.clone())
        .expect("open exact live reconstruction session");
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: task.clone(),
            manifest: payload.manifest().clone(),
            body: canonical_wire.into(),
        });

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("overlapping service ownership must fail closed");

    assert!(error.contains("has conflicting service owners"));
    assert!(service.fetches.contains_key(&work_id));
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&work_id)
    );
    assert_eq!(service.local_completions.len(), 1);
    assert!(service.output_guard.restart_required());
}

#[test]
fn service_monotonically_upgrades_body_fetch_authority_in_both_orders() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let ordinary = BodyFetchTask::ordinary_for_test(51, tag, payload.manifest().clone());
    let hybrid = certified_fetch_task(
        &service,
        51,
        tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    service
        .enqueue_body_fetch(ordinary)
        .expect("start manifest acquisition");
    service
        .enqueue_body_fetch(hybrid.clone())
        .expect("add certified authority");
    let live = service.fetches.get(&hybrid.id()).expect("hybrid owner");
    assert_eq!(live.task, hybrid);
    assert!(live.chunks.is_some());
    assert_eq!(
        service
            .fetch_by_manifest
            .get(&HashOf::new(payload.manifest())),
        Some(&hybrid.id())
    );

    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let certified = certified_fetch_task(&service, 52, tag, None, proposal.round, proposal.subject);
    let hybrid = certified_fetch_task(
        &service,
        52,
        tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    service
        .enqueue_body_fetch(certified)
        .expect("start certified acquisition");
    assert!(service.fetch_by_manifest.is_empty());
    service
        .enqueue_body_fetch(hybrid.clone())
        .expect("add manifest authority");
    let live = service.fetches.get(&hybrid.id()).expect("hybrid owner");
    assert_eq!(live.task, hybrid);
    assert!(live.chunks.is_some());
    assert_eq!(
        service
            .fetch_by_manifest
            .get(&HashOf::new(payload.manifest())),
        Some(&hybrid.id())
    );
}

#[test]
fn certified_completion_retires_exact_live_or_reconstructed_owner() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let live_task = certified_fetch_task(
        &service,
        53,
        tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    service
        .enqueue_body_fetch(live_task.clone())
        .expect("start hybrid fetch");
    let manifest_hash = HashOf::new(payload.manifest());
    let prepared = service
        .prepare_certified_body_fetch_owner_removal(&live_task)
        .expect("exact live owner prepares without mutation");
    drop(prepared);
    assert_eq!(
        service
            .fetches
            .get(&live_task.id())
            .map(|fetch| &fetch.task),
        Some(&live_task)
    );
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&live_task.id())
    );
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());

    let foreign_guard = ConsensusOutputGuard::isolated();
    let foreign_permit = foreign_guard.acquire().expect("foreign output permit");
    let prepared = service
        .prepare_certified_body_fetch_owner_removal(&live_task)
        .expect("exact live owner prepares for permit-binding negative");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = prepared.commit(&foreign_permit);
    }));
    assert!(unwind.is_err());
    assert_eq!(
        service
            .fetches
            .get(&live_task.id())
            .map(|fetch| &fetch.task),
        Some(&live_task)
    );
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&live_task.id())
    );
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());
    drop(foreign_permit);
    service
        .complete_certified_body_fetch(&live_task)
        .expect("certified response retires live owner");
    assert!(service.fetches.is_empty());
    assert!(service.fetch_by_manifest.is_empty());

    let queued_ordinary = BodyFetchTask::ordinary_for_test(54, tag, payload.manifest().clone());
    let queued_task = certified_fetch_task(
        &service,
        54,
        tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    service
        .local_completions
        .push_back(LocalCompletion::Reconstructed {
            task: queued_ordinary,
            manifest: payload.manifest().clone(),
            body: body.into(),
        });
    service
        .enqueue_body_fetch(queued_task.clone())
        .expect("queued reconstruction accepts certified upgrade");
    service
        .complete_certified_body_fetch(&queued_task)
        .expect("certified response retires queued reconstruction");
    assert!(service.local_completions.is_empty());
    assert!(!service.output_guard.restart_required());
}

#[test]
fn certified_completion_preflight_rejects_mismatched_task_without_owner_mutation() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let live_task = certified_fetch_task(
        &service,
        55,
        tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    service
        .enqueue_body_fetch(live_task.clone())
        .expect("start exact hybrid fetch");
    let mismatched_tag = EventTag::new(
        tag.height(),
        tag.view(),
        Generation::new(tag.generation().get().saturating_add(1)),
    );
    let mismatched = certified_fetch_task(
        &service,
        55,
        mismatched_tag,
        Some(payload.manifest().clone()),
        proposal.round,
        proposal.subject,
    );
    let manifest_hash = HashOf::new(payload.manifest());

    let error = service
        .complete_certified_body_fetch(&mismatched)
        .expect_err("a different executor task cannot retire the live service owner");

    assert!(error.contains("differs from executor ownership"));
    assert_eq!(
        service
            .fetches
            .get(&live_task.id())
            .map(|fetch| &fetch.task),
        Some(&live_task),
    );
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&live_task.id()),
    );
    assert!(service.local_completions.is_empty());
    assert!(
        !service.output_guard.restart_required(),
        "preflight rejection occurs before the guarded mutation boundary",
    );
}

#[test]
fn cancellation_rejects_a_different_task_without_consuming_exact_owner() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(55, tag, payload.manifest().clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("start exact fetch");
    let wrong = BodyFetchTask::ordinary_for_test(
        55,
        EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height + 1),
        ),
        payload.manifest().clone(),
    );

    let error = service
        .cancel_body_fetch(&wrong)
        .expect_err("different task identity must fail closed");

    assert!(error.contains("differs from executor ownership"));
    assert!(service.fetches.contains_key(&task.id()));
    assert_eq!(
        service
            .fetch_by_manifest
            .get(&HashOf::new(payload.manifest())),
        Some(&task.id())
    );
    assert!(service.output_guard.restart_required());
}

#[test]
fn corrupt_manifest_index_is_preserved_and_fails_closed_before_cancellation() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(56, tag, payload.manifest().clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("start exact fetch");
    let manifest_hash = HashOf::new(payload.manifest());
    let innocent_owner = EffectWorkId::for_test(999);
    service
        .fetch_by_manifest
        .insert(manifest_hash, innocent_owner);

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("corrupt manifest ownership must fail closed");

    assert!(error.contains("mismatched manifest owner"));
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&innocent_owner)
    );
    assert!(service.fetches.contains_key(&task.id()));
    assert!(service.output_guard.restart_required());
}

#[test]
fn duplicate_queued_fetch_owners_fail_closed_without_consumption() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(57, tag, payload.manifest().clone());
    for _ in 0..2 {
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: task.clone(),
                manifest: payload.manifest().clone(),
                body: body.clone().into(),
            });
    }

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("duplicate queue ownership must fail closed");

    assert!(error.contains("duplicate queued reconstruction owners"));
    assert_eq!(service.local_completions.len(), 2);
    assert!(service.output_guard.restart_required());
}

#[test]
fn missing_orphan_and_wrong_manifest_indices_fail_closed_without_consumption() {
    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(58, tag, payload.manifest().clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("start exact fetch");
    let manifest_hash = HashOf::new(payload.manifest());
    assert_eq!(
        service.fetch_by_manifest.remove(&manifest_hash),
        Some(task.id())
    );

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("missing manifest index must fail closed");
    assert!(error.contains("mismatched manifest owner"));
    assert!(service.fetch_by_manifest.is_empty());
    assert!(service.fetches.contains_key(&task.id()));
    assert!(service.output_guard.restart_required());

    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(59, tag, payload.manifest().clone());
    let manifest_hash = HashOf::new(payload.manifest());
    service.fetch_by_manifest.insert(manifest_hash, task.id());

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("orphan manifest index must fail closed");
    assert!(error.contains("orphaned manifest owner"));
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&task.id())
    );
    assert!(service.fetches.is_empty());
    assert!(service.output_guard.restart_required());

    let (mut service, keys) = fixture();
    let _chunk_root = install_temporary_chunk_root(&mut service);
    allow_fixture_block_payload(&mut service.context);
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyFetchTask::ordinary_for_test(60, tag, payload.manifest().clone());
    service
        .enqueue_body_fetch(task.clone())
        .expect("start exact fetch");
    let manifest_hash = HashOf::new(payload.manifest());
    let wrong_owner = EffectWorkId::for_test(1_000);
    service.fetch_by_manifest.insert(manifest_hash, wrong_owner);

    let error = service
        .cancel_body_fetch(&task)
        .expect_err("wrong manifest index must fail closed");
    assert!(error.contains("mismatched manifest owner"));
    assert_eq!(
        service.fetch_by_manifest.get(&manifest_hash),
        Some(&wrong_owner)
    );
    assert!(service.fetches.contains_key(&task.id()));
    assert!(service.output_guard.restart_required());
}

#[test]
fn io_queue_cancellation_frees_capacity_without_reordering_retained_work() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let sign = |id| V2IoCommand::Sign {
        task: ConsensusSignTask::for_test(
            id,
            tag,
            super::super::v2::SignRequest::Proposal(proposal.clone()),
        ),
        restore_outbound_payload: false,
    };
    let (command_tx, command_rx, admission) = test_io_command_channel(2);

    command_tx
        .try_send(sign(1))
        .expect("queue first signing task");
    command_tx
        .try_send(sign(2))
        .expect("queue retained signing task");
    assert!(matches!(
        command_tx.try_send(sign(3)),
        Err(V2IoTrySendError::Full(_))
    ));
    assert!(
        command_tx
            .cancel(EffectWorkId::for_test(1), V2IoCancellableKind::Sign)
            .expect("cancel queued signing task")
    );
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    command_tx
        .try_send(sign(3))
        .expect("reclaimed slot accepts current-view work");

    for expected in [2, 3] {
        let command = command_rx.try_recv().expect("retained queued command");
        let work_id = command.work_id().expect("signing work identifier");
        assert_eq!(work_id, EffectWorkId::for_test(expected));
        command_rx.complete_work(work_id);
        command_tx.acknowledge_completion(work_id);
    }
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);

    let durable = DurableBodyReceipt::for_test(
        service.context.id(),
        proposal.round,
        proposal.subject,
        HashOf::new(&proposal.manifest),
    );
    let validation = BodyValidationTask::for_test(4, durable);
    let validation_id = validation.id();
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission: Arc::clone(&admission),
    });
    service
        .io()
        .expect("synthetic validation queue")
        .enqueue(V2IoCommand::Validate(validation))
        .expect("queue stale validation");
    service
        .cancel_body_validation(validation_id)
        .expect("production callback cancels queued validation");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    drop(service.io.take());
}

#[test]
fn io_queue_reports_active_store_cancellation_as_retained() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = BodyStoreTask::for_test(5, tag, payload.manifest().clone(), body);
    let work_id = task.id();
    let (command_tx, command_rx, admission) = test_io_command_channel(1);

    command_tx
        .try_send(V2IoCommand::Store(task))
        .expect("queue body store");
    let active = command_rx.try_recv().expect("activate body store");
    assert_eq!(active.work_id(), Some(work_id));
    assert!(
        !command_tx
            .cancel(work_id, V2IoCancellableKind::Store)
            .expect("active body store remains owned")
    );
    assert_eq!(
        command_tx.queue.lock().work[&work_id].state,
        V2IoWorkState::Active
    );

    command_rx.complete_work(work_id);
    assert!(
        !command_tx
            .cancel(work_id, V2IoCancellableKind::Store)
            .expect("completion-pending body store remains owned")
    );
    assert_eq!(
        command_tx.queue.lock().work[&work_id].state,
        V2IoWorkState::CompletionPending
    );
    command_tx.acknowledge_completion(work_id);
    assert!(command_tx.queue.lock().work.is_empty());
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
}

#[test]
fn io_queue_rejects_cancellation_without_tracked_ownership() {
    let (command_tx, _command_rx, _admission) = test_io_command_channel(1);
    let missing = EffectWorkId::for_test(6);

    let error = command_tx
        .cancel(missing, V2IoCancellableKind::Store)
        .expect_err("missing work ownership must not look active");

    assert!(error.contains("has no tracked owner"));
    assert!(command_tx.queue.lock().work.is_empty());
}

#[test]
fn io_queue_validation_identity_is_only_work_id_and_durable_receipt() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let durable = DurableBodyReceipt::for_test(
        service.context.id(),
        proposal.round,
        proposal.subject,
        HashOf::new(&proposal.manifest),
    );
    let exact = BodyValidationTask::for_test(8, durable.clone());
    let conflicting = BodyValidationTask::for_test(
        8,
        DurableBodyReceipt::for_test(
            service.context.id(),
            proposal.round,
            proposal.subject,
            HashOf::from_untyped_unchecked(Hash::new(b"conflicting durable manifest")),
        ),
    );
    let (command_tx, command_rx, admission) = test_io_command_channel(1);

    command_tx
        .try_send(V2IoCommand::Validate(exact.clone()))
        .expect("queue immutable validation");
    command_tx
        .try_send(V2IoCommand::Validate(exact.clone()))
        .expect("coalesce exact immutable retransmission");
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    assert!(matches!(
        command_tx.try_send(V2IoCommand::Validate(conflicting)),
        Err(V2IoTrySendError::ConflictingWorkId { work_id, .. })
            if work_id == EffectWorkId::for_test(8)
    ));

    let command = command_rx.try_recv().expect("single validation command");
    let work_id = command.work_id().expect("validation work identifier");
    assert_eq!(work_id, EffectWorkId::for_test(8));
    command_tx
        .try_send(V2IoCommand::Validate(exact))
        .expect("coalesce immutable validation while active");
    command_rx.complete_work(work_id);
    command_tx.acknowledge_completion(work_id);
    assert!(command_tx.queue.lock().work.is_empty());
}

#[test]
fn io_queue_duplicate_apply_coalesces_and_conflicting_work_id_fails_closed() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let durable = DurableBodyReceipt::for_test(
        service.context.id(),
        proposal.round,
        proposal.subject,
        HashOf::new(&proposal.manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable);
    let certificate = wire::QuorumCertificate {
        round: proposal.round,
        proposal_round: proposal.round,
        phase: wire::GlobalPhase::Commit,
        subject: proposal.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let task = ApplyTask::for_test(
        7,
        tag,
        proposal.subject,
        certificate.clone(),
        validated.clone(),
    );
    let conflicting = ApplyTask::for_test(
        7,
        EventTag::new(
            service.context.height,
            proposal.round.view + 1,
            Generation::new(service.context.height),
        ),
        proposal.subject,
        certificate,
        validated,
    );
    let (command_tx, command_rx, admission) = test_io_command_channel(1);

    command_tx
        .try_send(V2IoCommand::Apply(task.clone()))
        .expect("queue exact apply");
    command_tx
        .try_send(V2IoCommand::Apply(task.clone()))
        .expect("coalesce queued exact apply retransmission");
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    assert!(matches!(
        command_tx.try_send(V2IoCommand::Apply(conflicting)),
        Err(V2IoTrySendError::ConflictingWorkId { work_id, .. })
            if work_id == EffectWorkId::for_test(7)
    ));

    let command = command_rx.try_recv().expect("single coalesced apply");
    let work_id = command.work_id().expect("apply work identifier");
    assert_eq!(work_id, EffectWorkId::for_test(7));
    assert!(matches!(command, V2IoCommand::Apply(_)));
    command_tx
        .try_send(V2IoCommand::Apply(task.clone()))
        .expect("coalesce exact retransmission while apply is active");
    command_rx.complete_work(work_id);
    command_tx
        .try_send(V2IoCommand::Apply(task.clone()))
        .expect("coalesce exact retransmission while completion is pending");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(command_rx);
    command_tx.acknowledge_completion(work_id);
    assert!(command_tx.queue.lock().work.is_empty());
}

#[test]
fn selected_serve_target_waits_for_complete_physical_prefix_and_excludes_later_ingress() {
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
    let later_source = service.context.roster[3].validator.clone();
    let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
    let route = route_fixture.mint_via(requester.clone(), via.clone());
    let (command_tx, _command_rx, _admission) = test_io_command_channel(4);
    let (ingress, _gate) = gated_fair_ingress(&service.context, &command_tx);
    let ordinary = |height, source: &PeerId| {
        InboundBlockMessage::from_transport(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                    wire::CommitCertificateRequest {
                        protocol_version: wire::PROTOCOL_VERSION,
                        network_id: service.context.network_id,
                        context_id: service.context.id(),
                        height,
                        requester: source.clone(),
                        signature: vec![u8::try_from(height).unwrap_or(u8::MAX)],
                    },
                ),
            )),
            source.clone(),
            source.clone(),
        )
    };
    let predecessor_height = service.context.height.saturating_add(1);
    let later_height = service.context.height.saturating_add(2);

    assert!(matches!(
        ingress.try_push(ordinary(predecessor_height, &via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via.clone(),
            route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(ordinary(later_height, &later_source)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect selected Serve carrier")
        .expect("exact request owns the selected barrier");
    assert_eq!(barrier.carrier_ordinal(), 2);
    assert!(
        ingress
            .try_recv_if(|inbound| {
                matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload:
                            wire::ConsensusMessageV2Payload::CommitCertificateRequest(request),
                        ..
                    }) if request.height == later_height
                )
            })
            .is_none(),
        "a post-target occurrence cannot enter the selected Serve union"
    );
    assert!(
        ingress
            .try_recv_if(|inbound| {
                matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload:
                            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                        ..
                    }) if HashOf::new(candidate) == request.request_hash()
                )
            })
            .is_none(),
        "a predicate which rejects the predecessor cannot select the exact target"
    );
    let predecessor = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload:
                        wire::ConsensusMessageV2Payload::CommitCertificateRequest(candidate),
                    ..
                }) if candidate.height == predecessor_height
            )
        })
        .expect("the finite frozen predecessor remains separately serviceable");
    assert!(matches!(
        predecessor.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(candidate),
            ..
        }) if candidate.height == predecessor_height
    ));

    let mut prepared = None;
    let mut target = ingress
        .try_recv_if(|inbound| {
            let is_target = matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                    ..
                }) if HashOf::new(candidate) == request.request_hash()
            );
            if is_target {
                prepared = Some(
                    command_tx
                        .prepare_reserved_serve(
                            CertifiedServeOwnerKey::Roster(requester.clone()),
                            request.clone(),
                        )
                        .expect("prepare exact selected Serve target"),
                );
            }
            is_target
        })
        .expect("the exact target becomes eligible after its frozen prefix drains");
    let admission = prepared.expect("target predicate prepared its exact reservation");
    let ownership = target
        .take_ingress_ownership()
        .expect("selected target retains fair ownership");
    let (_, _, reply_routes) = target.into_message_sender_and_reply_routes();
    assert!(matches!(
        command_tx
            .commit_serve(
                &admission,
                reply_routes.expect("selected target retains its reply route"),
                ownership,
            )
            .expect("commit selected Serve target"),
        CertifiedServeCommit::Queued
    ));
    assert_eq!(ingress.len(), 1);
}

#[test]
fn selected_serve_request_unblocks_matching_body_dependent_control() {
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
    let voter = service.context.roster[0].validator.clone();
    let vote = wire::Vote {
        round: proposal.round,
        proposal_round: proposal.round,
        phase: wire::GlobalPhase::Prepare,
        subject: proposal.subject,
        execution_commitment: request.request().certificate.execution_commitment,
        signer: 0,
        signature: vec![0xA5; 48],
    };
    let vote_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(vote),
    ));
    let qc_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(request.request().certificate.clone()),
    ));
    let proposal_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
    ));

    for (case, control_message) in [
        ("proposal", proposal_message),
        ("vote", vote_message),
        ("prepare-qc", qc_message),
    ] {
        let (command_tx, command_rx, _admission) = test_io_command_channel(4);
        let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            128,
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
        );
        let roster = service
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        ingress
            .configure_roster_for_context(
                roster.iter().cloned(),
                &service.context.network_id,
                service.context.da_layout,
            )
            .expect("configure production-shaped combined ingress");
        ingress.require_certified_serve_gate();
        ingress.require_leader_wire_lifecycle_gate();

        let directory = TempDir::new().expect("temporary combined ingress gate");
        let wal_path = directory
            .path()
            .join(format!("{case}-serve-body-recovery.wal"));
        let owner = [0xAA; 32];
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                service.context.da_layout.max_chunk_count,
            )
            .expect("derive finite leader lifecycle capacity");
        let recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                service.context.id(),
                service.context.height,
                owner,
                0,
                false,
            );
        let (leader_gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &wal_path,
                service.context.id(),
                service.context.height,
                owner,
                roster,
                capacity,
                service.context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open production-shaped leader lifecycle gate");
        let serve_gate = CertifiedServeIngressGate {
            queue: Arc::clone(&command_tx.queue),
        };
        ingress
            .bind_certified_serve_gate(serve_gate.clone())
            .expect("bind exact Serve ingress gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&leader_gate),
                restore,
                command_tx.queue.lifecycle_ordinals.clone(),
                service.context.id(),
                service.context.height,
            )
            .expect("bind leader gate to the same actor-global ordinal source");
        ingress.open().expect("open combined production ingress");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                control_message,
                Some(voter.clone()),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), voter.clone())),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let serve_barrier = serve_gate
            .selected_barrier()
            .expect("inspect the bound Serve gate")
            .expect("the body-recovery request owns the selected Serve barrier");
        assert_eq!(serve_barrier.scheduler_ordinal(), 2, "case={case}");
        assert_eq!(serve_barrier.carrier_ordinal(), 2, "case={case}");
        assert_eq!(
            leader_gate
                .earliest_ingress_scheduler_ordinal()
                .expect("inspect the retained control owner"),
            Some(1),
            "case={case}"
        );

        let mut prepared = None;
        let mut body_request = ingress
            .try_recv_if_checked(|inbound| {
                let is_matching_request = matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                        ..
                    }) if candidate.round == proposal.round
                        && candidate.subject == proposal.subject
                        && HashOf::new(candidate) == request.request_hash()
                );
                if is_matching_request {
                    prepared = Some(
                        command_tx
                            .prepare_reserved_serve(
                                CertifiedServeOwnerKey::Roster(requester.clone()),
                                request.clone(),
                            )
                            .expect("prepare the exact selected body-recovery request"),
                    );
                }
                is_matching_request
            })
            .expect("checked selector preserves both durable gates")
            .unwrap_or_else(|| {
                panic!("selected Serve request crosses blocked matching control: {case}")
            });
        let admission = prepared.expect("predicate prepared the exact Serve reservation");
        let ingress_ownership = body_request
            .take_ingress_ownership()
            .expect("body-recovery request retains fair-ingress ownership");
        let (_, _, reply_routes) = body_request.into_message_sender_and_reply_routes();
        assert!(matches!(
            command_tx
                .commit_serve(
                    &admission,
                    reply_routes.expect("body-recovery request retains its reply route"),
                    ingress_ownership,
                )
                .expect("commit the exact body-recovery Serve request"),
            CertifiedServeCommit::Queued
        ));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == admission.lifecycle_id
        ));
        assert_eq!(
            ingress.len(),
            1,
            "retained control is not displaced: {case}"
        );
        assert_eq!(
            leader_gate
                .earliest_ingress_scheduler_ordinal()
                .expect("control remains the durable leader-wire owner"),
            Some(1),
            "case={case}"
        );
    }
}
