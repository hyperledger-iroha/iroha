#[test]
fn production_exact_output_observes_finality_only_after_state_commit() {
    let (mut service, keys) = fixture();
    let mut valid =
        crate::block::ValidBlock::new_dummy_and_modify_header(keys[0].private_key(), |header| {
            header.set_height(NonZeroU64::new(service.context.height).expect("non-zero height"));
            header.set_prev_block_hash(None);
            header.creation_time_ms = service.context.height;
            header.merkle_root = None;
        });
    valid
        .as_mut()
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("attach empty block result metadata");
    let block = valid.commit_unchecked().unpack(|_| {});
    service
        .kura
        .store_block(block.clone())
        .expect("persist canonical block before finality");
    let subject = wire::BlockSubject {
        parent_block_hash: block.as_ref().header().prev_block_hash(),
        block_hash: block.as_ref().hash(),
        payload_hash: block
            .as_ref()
            .canonical_proposal_wire_hash()
            .expect("canonical proposal wire"),
    };
    let mut execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"worker applied-finality parent state"),
        Hash::new(b"worker applied-finality post state"),
        Hash::new(b"worker applied-finality writes"),
        1,
        Hash::new(b"worker applied-finality executed block"),
    );
    execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .as_ref()
            .encode_wire()
            .expect("canonical executed block wire")
            .len(),
    )
    .expect("executed block wire length fits u64");
    execution_commitment.executed_block_wire_hash = block
        .as_ref()
        .executed_block_wire_hash()
        .expect("canonical executed block wire hash");
    let artifact = signed_worker_finality_artifact(
        &service.context,
        &keys,
        0,
        subject,
        execution_commitment,
        [
            "aggregate applied-finality CommitQC",
            "applied-finality validator PoP",
            "valid applied-finality artifact",
        ],
    );
    let _ = service
        .kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist exact Kura finality before State commit");
    service.set_exact_output_admission_hook(|post, ticket| {
        assert!(ticket.is_none());
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: None,
            rank: 1,
        })
    });
    let vote = routing_vote(&service, 0, wire::GlobalPhase::Commit);
    assert_eq!(
        service
            .broadcast_consensus(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(vote),
            ))
            .expect("retain pre-State-commit GlobalV2 output"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect pre-State-commit exact output");
        assert!(pending.is_pending());
        assert!(pending.applied_height_finality.is_none());
    }
    assert!(
        service
            .retry_pending_exact_output()
            .expect("Kura finality alone cannot supersede exact output")
    );
    {
        let topology = crate::sumeragi::network_topology::Topology::new(
            service
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone()),
        );
        let mut state_block = service.state.block(block.as_ref().header());
        let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
        state_block.commit().expect("commit synthetic State block");
    }
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("State commit unlocks exact durable-finality supersession")
    );
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect post-State-commit exact output");
    assert!(!pending.is_pending());
    assert_eq!(pending.applied_height_finality.as_ref(), Some(&artifact));
}

#[test]
fn applied_height_finality_releases_only_ticketless_global_topology_target() {
    let (service, keys) = fixture();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let stranded = service.context.roster[1].validator.clone();
    let responsive = service.context.roster[2].validator.clone();
    let vote = routing_vote(&service, 0, wire::GlobalPhase::Commit);
    let message = ProductionV2Services::preencode_v2_network_message(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
    )
    .expect("encode exact GlobalV2 vote");
    let mut pending = PendingExactOutput::new(2, 1, 2, &[stranded.clone(), responsive.clone()])
        .expect("two frozen validator targets fit");
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::claimed(
                    vec![message.clone()],
                    vec![stranded.clone(), responsive.clone()],
                    ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
                )
                .expect("valid GlobalV2 fanout")
                .expect("non-empty GlobalV2 fanout"),
            )
            .expect("retain both frozen targets"),
        ExactFanoutOwnership::Owned
    );

    let mut responsive_admissions = 0usize;
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            if post.peer_id == stranded {
                return Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket: None,
                    rank: 1,
                });
            }
            assert_eq!(post.peer_id, responsive);
            responsive_admissions += 1;
            Ok(ExactOutputAttemptOutcome::Admitted)
        }),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 1 })
    );
    assert_eq!(responsive_admissions, 1);
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
    assert_eq!(pending.fanouts[0].targets[1].message_index, 1);

    pending.applied_height_finality = Some(artifact.clone());
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert_eq!(post.peer_id, stranded);
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 1,
            })
        }),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert!(!pending.is_pending());

    let mut manual =
        PendingExactOutput::new(1, 1, 1, &[stranded.clone()]).expect("one manual target fits");
    manual.applied_height_finality = Some(artifact.clone());
    manual
        .enqueue(
            PendingExactFanout::new(vec![message.clone()], vec![stranded.clone()]).expect("fanout"),
        )
        .expect("retain manual fanout");
    assert_eq!(
        manual.drive_with_budget_ack(usize::MAX, |post, _ticket, _route, _timeout_attempt| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 7,
            })
        }),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 7 })
    );
    assert!(manual.is_pending(), "manual output retains exact authority");

    let mut ticketed =
        PendingExactOutput::new(1, 1, 1, &[stranded.clone()]).expect("one ticketed target fits");
    ticketed.applied_height_finality = Some(artifact);
    ticketed
        .enqueue(
            PendingExactFanout::claimed(
                vec![message],
                vec![stranded],
                ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
            )
            .expect("valid ticketed GlobalV2 fanout")
            .expect("non-empty ticketed GlobalV2 fanout"),
        )
        .expect("retain ticketed GlobalV2 fanout");
    let mut ticket_fixture = None;
    assert_eq!(
        ticketed.drive_with_budget_ack(usize::MAX, |post, ticket, _route, _timeout_attempt| {
            assert!(ticket.is_none());
            let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
            ticket_fixture = Some(fixture);
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: Some(ticket),
                rank: 1,
            })
        }),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 1 })
    );
    assert!(ticketed.is_pending(), "ticketed GlobalV2 stays owned");
    assert_eq!(
        ticket_fixture
            .as_ref()
            .expect("retain genuine actor ticket fixture")
            .waiter_count(),
        1
    );
}

#[test]
fn applied_height_finality_releases_only_covered_ticketless_payload_chunks() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let stranded = service.context.roster[1].validator.clone();
    let (_, payload, _) = proposal_body_and_payload(&service.context, &keys);
    let manifest = payload.manifest().clone();
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("sign and retain exact payload chunks");
    let messages = service
        .outbound_chunks
        .get(&HashOf::new(&manifest))
        .expect("registered payload owns its exact manifest")
        .messages
        .clone();
    let chunk_count = messages.len();

    let mut ticketless =
        PendingExactOutput::new(1, chunk_count, 1, std::slice::from_ref(&stranded))
            .expect("one ticketless payload-chunk target fits");
    ticketless.applied_height_finality = Some(artifact.clone());
    ticketless
        .enqueue(
            PendingExactFanout::claimed(
                messages.clone(),
                vec![stranded.clone()],
                ExactOutputRolloverClaim::PayloadChunks {
                    scope: service.exact_output_scope(),
                    manifest: manifest.clone(),
                },
            )
            .expect("valid ticketless payload-chunk fanout")
            .expect("non-empty ticketless payload-chunk fanout"),
        )
        .expect("retain ticketless payload chunks");
    let mut ticketless_attempts = 0usize;
    assert_eq!(
        ticketless.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert_eq!(post.peer_id, stranded);
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            ticketless_attempts += 1;
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 1,
            })
        }),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert_eq!(ticketless_attempts, chunk_count);
    assert!(!ticketless.is_pending());

    let mut ticketed = PendingExactOutput::new(1, chunk_count, 1, std::slice::from_ref(&stranded))
        .expect("one ticketed payload-chunk target fits");
    ticketed.applied_height_finality = Some(artifact.clone());
    ticketed
        .enqueue(
            PendingExactFanout::claimed(
                messages.clone(),
                vec![stranded.clone()],
                ExactOutputRolloverClaim::PayloadChunks {
                    scope: service.exact_output_scope(),
                    manifest: manifest.clone(),
                },
            )
            .expect("valid ticketed payload-chunk fanout")
            .expect("non-empty ticketed payload-chunk fanout"),
        )
        .expect("retain ticketed payload chunks");
    let mut ticket_fixture = None;
    assert_eq!(
        ticketed.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
            ticket_fixture = Some(fixture);
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: Some(ticket),
                rank: 2,
            })
        }),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 2 })
    );
    assert!(ticketed.is_pending(), "ticketed payload chunks stay owned");
    assert_eq!(
        ticket_fixture
            .as_ref()
            .expect("retain genuine actor ticket fixture")
            .waiter_count(),
        1
    );

    let mut unrelated_scope = service.exact_output_scope();
    unrelated_scope.height = unrelated_scope
        .height
        .checked_add(1)
        .expect("fixture height has a successor");
    let mut uncovered = PendingExactOutput::new(1, chunk_count, 1, std::slice::from_ref(&stranded))
        .expect("one uncovered payload-chunk target fits");
    uncovered.applied_height_finality = Some(artifact);
    uncovered
        .enqueue(
            PendingExactFanout::claimed(
                messages,
                vec![stranded],
                ExactOutputRolloverClaim::PayloadChunks {
                    scope: unrelated_scope,
                    manifest,
                },
            )
            .expect("structurally valid uncovered payload-chunk fanout")
            .expect("non-empty uncovered payload-chunk fanout"),
        )
        .expect("retain uncovered payload chunks");
    assert_eq!(
        uncovered.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 3,
            })
        }),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 3 })
    );
    assert!(
        uncovered.is_pending(),
        "an applied artifact cannot release another creation scope"
    );
}

#[test]
fn terminal_retry_revalidates_only_ticketless_exact_kura_advert() {
    // Binding is process-lifetime, so probe one fresh durable fixture per
    // CommitQC signer until the deterministic keeper is found.
    let signer_indices = durable_history_fixture().artifact.commit_qc.signers.clone();
    let mut selected_history = None;
    for local_validator in signer_indices {
        let candidate = durable_history_fixture();
        let local_index = usize::try_from(local_validator)
            .expect("history-fixture signer index fits this platform");
        let local_key = candidate
            .validators
            .get(local_index)
            .expect("history-fixture signer belongs to its roster")
            .clone();
        candidate
            .kura
            .bind_local_peer_id(PeerId::new(local_key.public_key().clone()))
            .expect("bind one candidate history-fixture keeper");
        let source = candidate
            .kura
            .probe_kura_replica_advert_source(candidate.artifact.height, &local_key)
            .expect("probe exact history-fixture keeper authority");
        if let Some(source) = source {
            selected_history = Some((candidate, local_validator, source));
            break;
        }
    }
    let (history, local_validator, source) =
        selected_history.expect("history fixture has a deterministic CommitQC keeper");
    let new_service = || {
        service_for_history_context_with_local_validator(
            Arc::clone(&history.kura),
            history.artifact.height_context.clone(),
            &history.validators,
            local_validator,
        )
    };
    let commit_history_to_state = |service: &ProductionV2Services| {
        let height = NonZeroUsize::new(
            usize::try_from(history.artifact.height).expect("history height fits usize"),
        )
        .expect("history height is non-zero");
        let block = service
            .kura
            .get_block(height)
            .expect("durable history retains its canonical block");
        let topology = crate::sumeragi::network_topology::Topology::new(
            service
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone()),
        );
        let mut state_block = service.state.block(block.as_ref().header());
        let committed =
            crate::block::ValidBlock::committed_from_replay_signed_block(block.as_ref().clone());
        let _events = state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
        state_block
            .commit()
            .expect("commit the durable history block to State");
    };
    let source_height = history.artifact.height;
    let now = Instant::now();

    // Reproduce topology-ticket cancellation first. The retry becomes
    // ticketless while State is still below the source height and must remain
    // owned; the same occurrence may retire only after State-applied finality.
    let mut ticketless = new_service();
    let canceled_membership = Arc::new(AtomicBool::new(false));
    let canceled_membership_for_hook = Arc::clone(&canceled_membership);
    let ticket_fixtures = Arc::new(Mutex::new(Vec::new()));
    let ticket_fixtures_for_hook = Arc::clone(&ticket_fixtures);
    ticketless.set_exact_output_admission_hook(move |post, ticket| {
        if canceled_membership_for_hook.load(Ordering::Acquire) {
            assert!(
                ticket
                    .as_ref()
                    .and_then(NetworkActorAdmissionTicket::rank)
                    .is_none(),
                "topology cancellation must invalidate the retained actor ticket"
            );
            drop(ticket);
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 1,
            });
        }
        assert!(ticket.is_none());
        let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
        ticket_fixtures_for_hook
            .lock()
            .expect("retain advert actor-ticket fixtures")
            .push(fixture);
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: Some(ticket),
            rank: 1,
        })
    });
    let initial_refresh = ticketless
        .service_kura_replica_advert_refresh_turn(now)
        .expect("publish a Kura-backed advert into exact output");
    assert!(initial_refresh.fanout_attempted);
    assert!(
        ticketless
            .has_pending_exact_output()
            .expect("inspect ticketed advert output")
    );
    {
        let fixtures = ticket_fixtures
            .lock()
            .expect("inspect advert actor-ticket fixtures");
        assert_eq!(fixtures.len(), ticketless.remote_voters().len());
        for fixture in fixtures.iter() {
            assert_eq!(fixture.waiter_count(), 1);
            assert_eq!(fixture.cancel_topology_membership(), 1);
            assert_eq!(fixture.waiter_count(), 0);
        }
    }
    canceled_membership.store(true, Ordering::Release);
    assert!(
        ticketless
            .retry_pending_exact_output()
            .expect("ticketless advert remains before State-applied finality"),
        "Kura evidence alone must not release the advert"
    );
    {
        let pending = ticketless
            .lock_pending_exact_output()
            .expect("inspect pre-finality advert ownership");
        assert!(pending.is_pending());
        assert!(pending.applied_height_finality.is_none());
    }
    assert!(
        ticketless
            .kura_replica_advert_refresh
            .lock_state()
            .expect("inspect pre-finality advert refresh owner")
            .urgent_heights
            .is_empty()
    );

    commit_history_to_state(&ticketless);
    assert!(
        !ticketless
            .retry_pending_exact_output()
            .expect("terminal retry revalidates the ticketless advert from Kura")
    );
    assert!(
        !ticketless
            .has_pending_exact_output()
            .expect("terminal retry clears the revalidated advert")
    );
    assert!(
        ticketless
            .kura_replica_advert_refresh
            .lock_state()
            .expect("inspect scheduled advert retry")
            .urgent_heights
            .contains(&source_height),
        "release must schedule the durable source before losing the occurrence"
    );

    // State-applied finality cannot supersede an occurrence while its live
    // actor ticket still owns progress for the frozen topology membership.
    let mut ticketed = new_service();
    commit_history_to_state(&ticketed);
    let live_ticket_fixtures = Arc::new(Mutex::new(Vec::new()));
    let live_ticket_fixtures_for_hook = Arc::clone(&live_ticket_fixtures);
    ticketed.set_exact_output_admission_hook(move |post, ticket| {
        if let Some(ticket) = ticket {
            assert_eq!(ticket.rank(), Some(1));
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: Some(ticket),
                rank: 1,
            });
        }
        let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
        live_ticket_fixtures_for_hook
            .lock()
            .expect("retain live advert actor-ticket fixtures")
            .push(fixture);
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: Some(ticket),
            rank: 1,
        })
    });
    let ticketed_refresh = ticketed
        .service_kura_replica_advert_refresh_turn(now)
        .expect("publish a live-ticket Kura advert");
    assert!(ticketed_refresh.fanout_attempted);
    assert!(
        ticketed
            .retry_pending_exact_output()
            .expect("live actor tickets retain the advert")
    );
    assert!(
        ticketed
            .has_pending_exact_output()
            .expect("inspect live-ticket advert")
    );
    assert!(
        live_ticket_fixtures
            .lock()
            .expect("inspect live advert actor-ticket fixtures")
            .iter()
            .all(|fixture| fixture.waiter_count() == 1)
    );
    assert!(
        ticketed
            .kura_replica_advert_refresh
            .lock_state()
            .expect("inspect live-ticket advert refresh owner")
            .urgent_heights
            .is_empty()
    );

    // A syntactically valid, keeper-signed claim still stays owned when its
    // body hash no longer matches the independently readable Kura source.
    let mut mismatched = new_service();
    commit_history_to_state(&mismatched);
    let durable_tip = mismatched
        .kura
        .exact_kura_replica_advert_tip()
        .expect("read the exact durable tip");
    mismatched
        .kura_replica_advert_refresh
        .note_durable_tip(durable_tip, now)
        .expect("bound the refresh owner to the durable window");
    let mut advert = mismatched
        .kura
        .build_signed_kura_replica_advert_from_source(&source, &mismatched.key_pair)
        .expect("build the exact durable advert before tampering");
    advert.executed_block_wire_hash = Hash::new(b"tampered advert executed block wire");
    advert.signature = Signature::new(
        mismatched.key_pair.private_key(),
        &advert.signature_preimage(),
    )
    .payload()
    .to_vec();
    let rollover_claim = ExactOutputRolloverClaim::DurableKuraReplicaAdvert {
        scope: mismatched.exact_output_scope(),
        source_height,
        advert_hash: HashOf::new(&advert),
    };
    let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::KuraReplicaAdvert(advert)))
        .expect("encode the signed but Kura-mismatched advert");
    mismatched.set_exact_output_admission_hook(|post, ticket| {
        assert!(ticket.is_none());
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: None,
            rank: 1,
        })
    });
    let output_guard = Arc::clone(&mismatched.output_guard);
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open the mismatched advert output operation");
    assert_eq!(
        mismatched.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
            mismatched.remote_voters(),
            rollover_claim,
            operation.permit(),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    operation.complete();
    assert!(
        mismatched
            .retry_pending_exact_output()
            .expect("Kura-mismatched advert stays owned")
    );
    assert!(
        mismatched
            .has_pending_exact_output()
            .expect("inspect Kura-mismatched advert")
    );
    assert!(
        mismatched
            .kura_replica_advert_refresh
            .lock_state()
            .expect("inspect Kura-mismatched advert refresh owner")
            .urgent_heights
            .is_empty()
    );
}

#[test]
fn terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission() {
    let history = durable_history_fixture();
    let leader_index = usize::try_from(history.artifact.height_context.leader(0))
        .expect("QueuePlan leader index fits usize");
    let local_validator = (0..history.validators.len())
        .find(|index| *index != leader_index)
        .expect("QueuePlan fixture has a remote leader");
    let local_validator = wire::ValidatorIndex::try_from(local_validator)
        .expect("QueuePlan local validator index fits the wire type");
    let new_service = || {
        service_for_history_context_with_local_validator(
            Arc::clone(&history.kura),
            history.artifact.height_context.clone(),
            &history.validators,
            local_validator,
        )
    };
    let commit_history_to_state = |service: &ProductionV2Services| {
        let height = NonZeroUsize::new(
            usize::try_from(history.artifact.height).expect("history height fits usize"),
        )
        .expect("history height is non-zero");
        let block = service
            .kura
            .get_block(height)
            .expect("durable history retains its canonical block");
        let topology = crate::sumeragi::network_topology::Topology::new(
            service
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone()),
        );
        let mut state_block = service.state.block(block.as_ref().header());
        let committed =
            crate::block::ValidBlock::committed_from_replay_signed_block(block.as_ref().clone());
        let _events = state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
        state_block
            .commit()
            .expect("commit the durable history block to State");
    };
    let certificate = Arc::new(vec![0x51, 0x50, 0x41, 0x52]);
    let certificate_hash = Hash::new(certificate.as_slice());

    // Begin with an exact Kura source and a live actor ticket, then reproduce
    // topology-ticket cancellation before State has applied the durable height.
    let mut ticketless = new_service();
    ticketless
        .kura
        .persist_pending_queue_plan_admission_certificate(&certificate)
        .expect("persist exact QueuePlan reconstruction source");
    let (target, rollover_claim, messages) =
        queue_plan_rollover_fixture(&ticketless, Arc::clone(&certificate));
    let canceled_membership = Arc::new(AtomicBool::new(false));
    let canceled_membership_for_hook = Arc::clone(&canceled_membership);
    let ticket_fixtures = Arc::new(Mutex::new(Vec::new()));
    let ticket_fixtures_for_hook = Arc::clone(&ticket_fixtures);
    ticketless.set_exact_output_admission_hook(move |post, ticket| {
        if canceled_membership_for_hook.load(Ordering::Acquire) {
            assert!(
                ticket
                    .as_ref()
                    .and_then(NetworkActorAdmissionTicket::rank)
                    .is_none(),
                "topology cancellation must invalidate the QueuePlan actor ticket"
            );
            drop(ticket);
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 1,
            });
        }
        assert!(ticket.is_none());
        let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
        ticket_fixtures_for_hook
            .lock()
            .expect("retain QueuePlan actor-ticket fixture")
            .push(fixture);
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: Some(ticket),
            rank: 1,
        })
    });
    let output_guard = Arc::clone(&ticketless.output_guard);
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open QueuePlan exact-output operation");
    assert_eq!(
        ticketless.enqueue_exact_fanout_while_guarded(
            messages,
            vec![target],
            rollover_claim,
            operation.permit(),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    operation.complete();
    {
        let fixtures = ticket_fixtures
            .lock()
            .expect("inspect QueuePlan actor-ticket fixture");
        assert_eq!(fixtures.len(), 1);
        assert_eq!(fixtures[0].waiter_count(), 1);
        assert_eq!(fixtures[0].cancel_topology_membership(), 1);
        assert_eq!(fixtures[0].waiter_count(), 0);
    }
    canceled_membership.store(true, Ordering::Release);
    assert!(
        ticketless
            .retry_pending_exact_output()
            .expect("ticketless QueuePlan remains before State-applied finality")
    );

    commit_history_to_state(&ticketless);
    ticketless
        .kura
        .remove_pending_queue_plan_admission_certificate(certificate_hash)
        .expect("remove the exact QueuePlan source");
    assert!(
        ticketless
            .retry_pending_exact_output()
            .expect("missing QueuePlan source retains ticketless output"),
        "State-applied finality cannot replace absent Kura bytes"
    );
    ticketless
        .kura
        .persist_pending_queue_plan_admission_certificate(b"different QueuePlan source")
        .expect("persist non-matching QueuePlan source");
    assert!(
        ticketless
            .retry_pending_exact_output()
            .expect("non-matching QueuePlan source retains ticketless output"),
        "another hash-addressed certificate cannot replace the exact Kura bytes"
    );
    ticketless
        .kura
        .persist_pending_queue_plan_admission_certificate(&certificate)
        .expect("restore exact QueuePlan reconstruction source");
    assert!(
        !ticketless
            .retry_pending_exact_output()
            .expect("terminal retry revalidates QueuePlan output from Kura")
    );
    assert!(
        !ticketless
            .has_pending_exact_output()
            .expect("revalidated QueuePlan output leaves exact ownership")
    );
    assert!(
        ticketless
            .kura_replica_advert_refresh
            .lock_state()
            .expect("inspect unrelated advert refresh owner")
            .urgent_heights
            .is_empty(),
        "QueuePlan release must not schedule a Kura advert refresh"
    );

    // State-applied finality and exact Kura bytes do not supersede a live
    // actor ticket for the frozen leader.
    let mut ticketed = new_service();
    commit_history_to_state(&ticketed);
    let (target, rollover_claim, messages) =
        queue_plan_rollover_fixture(&ticketed, Arc::clone(&certificate));
    let live_ticket_fixtures = Arc::new(Mutex::new(Vec::new()));
    let live_ticket_fixtures_for_hook = Arc::clone(&live_ticket_fixtures);
    ticketed.set_exact_output_admission_hook(move |post, ticket| {
        if let Some(ticket) = ticket {
            assert_eq!(ticket.rank(), Some(1));
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: Some(ticket),
                rank: 1,
            });
        }
        let (fixture, ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
        live_ticket_fixtures_for_hook
            .lock()
            .expect("retain live QueuePlan actor-ticket fixture")
            .push(fixture);
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket: Some(ticket),
            rank: 1,
        })
    });
    let output_guard = Arc::clone(&ticketed.output_guard);
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open live-ticket QueuePlan exact-output operation");
    assert_eq!(
        ticketed.enqueue_exact_fanout_while_guarded(
            messages,
            vec![target],
            rollover_claim,
            operation.permit(),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    operation.complete();
    assert!(
        ticketed
            .retry_pending_exact_output()
            .expect("live actor ticket retains QueuePlan output")
    );
    assert!(
        ticketed
            .has_pending_exact_output()
            .expect("inspect live-ticket QueuePlan output")
    );
    let fixtures = live_ticket_fixtures
        .lock()
        .expect("inspect live QueuePlan actor-ticket fixture");
    assert_eq!(fixtures.len(), 1);
    assert_eq!(fixtures[0].waiter_count(), 1);
}

#[test]
fn closed_flush_racing_final_receiver_retirement_is_nonfatal() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"closed reply retirement race");
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let mut pending =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one retirement-race reply attempt fits");
    pending
        .enqueue(
            PendingExactFanout::new_with_routes(
                vec![message],
                vec![peer],
                vec![ExactTargetRoute::Reply(route.clone())],
            )
            .expect("one retirement-race reply fanout"),
        )
        .expect("retain the retirement-race reply");
    let mut control = None;
    pending
        .drive_with_budget_ack(usize::MAX, |post, _ticket, attempted, _timeout_attempt| {
            let ExactTargetRoute::Reply(attempted) = attempted else {
                panic!("retirement-race response changed route kind")
            };
            let (writer, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, attempted);
            control = Some(writer);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("queue the retirement-race writer flush");
    assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
    assert!(control.as_mut().expect("writer controller").close());
    assert!(routes.retire(&route));
    pending
        .poll_reply_flushes()
        .expect("final receiver retirement after Closed must not fail stop output");
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 0);
    assert!(target.current.is_none());
    assert!(target.pending_flush.is_none());
    assert!(target.parked);
    assert!(!pending.source_fifo_owners.is_empty());
}
