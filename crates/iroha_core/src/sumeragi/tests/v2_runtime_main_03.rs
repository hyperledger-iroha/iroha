#[test]
fn class_cursor_advances_from_the_served_class_after_empty_classes() {
    let admitted_at = Instant::now();
    let initial = tag(0);
    let queued =
        |class, value| TaggedCommand::new(initial, class, FakeCommand::record(value), admitted_at);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
    ingress
        .enqueue(queued(CommandClass::Normal, 1))
        .expect("normal command fits the bounded ingress");
    let first = ingress.pop_next().expect("normal class is reachable");
    assert_eq!(first.command.record, Some(1));
    assert_eq!(ingress.next_class, CommandClass::Completion);
    let lifecycle_ordinal = ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve one causal lifecycle for both ready classes");
    let root = FakeCommand::record(2);
    let mut causal_origin =
        RuntimeCandidateCausalOrigin::mint(initial, CommandClass::Normal, &root, None);
    assert!(causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal));
    let causal = |class, command| {
        TaggedCommand::with_causal_origin(
            initial,
            class,
            command,
            admitted_at,
            causal_origin.clone(),
            lifecycle_ordinal,
        )
        .expect("construct an exact same-lifecycle class sibling")
    };
    ingress
        .enqueue(causal(CommandClass::Normal, root))
        .expect("second normal command fits the bounded ingress");
    ingress
        .enqueue(causal(CommandClass::Completion, FakeCommand::record(3)))
        .expect("completion reserve remains available");
    let second = ingress.pop_next().expect("completion class is selected");
    assert_eq!(second.command.record, Some(3));
    assert_eq!(ingress.next_class, CommandClass::Progress);
    let third = ingress
        .pop_next()
        .expect("empty progress class is skipped to normal");
    assert_eq!(third.command.record, Some(2));
    assert_eq!(ingress.next_class, CommandClass::Completion);
}
#[test]
fn production_ingress_pop_uses_shared_selector_for_every_ready_mask() {
    let admitted_at = Instant::now();
    let initial = tag(0);
    for cursor in [
        CommandClass::Completion,
        CommandClass::Progress,
        CommandClass::Normal,
    ] {
        for ready_mask in 0u8..8 {
            let completion_ready = ready_mask & 0b001 != 0;
            let progress_ready = ready_mask & 0b010 != 0;
            let normal_ready = ready_mask & 0b100 != 0;
            let expected = select_bounded_service_class(
                cursor.service_code(),
                completion_ready,
                progress_ready,
                normal_ready,
            );
            let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
            ingress.next_class = cursor;
            let lifecycle_ordinal = ingress
                .lifecycle_ordinals
                .reserve_one()
                .expect("reserve one lifecycle shared by the ready mask");
            let root = FakeCommand::record(0);
            let mut causal_origin =
                RuntimeCandidateCausalOrigin::mint(initial, CommandClass::Normal, &root, None);
            assert!(causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal));
            for (class, ready) in [
                (CommandClass::Normal, normal_ready),
                (CommandClass::Progress, progress_ready),
                (CommandClass::Completion, completion_ready),
            ] {
                if ready {
                    ingress
                        .enqueue(
                            TaggedCommand::with_causal_origin(
                                initial,
                                class,
                                FakeCommand::record(class.service_code()),
                                admitted_at,
                                causal_origin.clone(),
                                lifecycle_ordinal,
                            )
                            .expect("construct one exact same-lifecycle ready class"),
                        )
                        .expect("one command per ready class fits reserved ingress");
                }
            }
            let selected = ingress.pop_next();
            assert_eq!(
                selected.as_ref().and_then(|queued| queued.command.record),
                (expected.selected != SERVICE_CLASS_NONE).then_some(expected.selected),
            );
            assert_eq!(ingress.next_class.service_code(), expected.next);
        }
    }
}
#[test]
fn healthy_same_class_fifo_depth_does_not_accrue_service_debt() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(9, 2, 2),
    );
    for id in 0..4 {
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(id),
        )
        .expect("enqueue same-class work");
    }
    let _ = runtime.step(start);
    let queue = runtime.queue_snapshot(start);
    assert_eq!(queue.normal.depth, 3);
    assert_eq!(queue.normal.max_service_debt, 0);
}
#[test]
fn canonical_body_completion_prunes_only_conflicting_queued_proposals() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"queued-body-context",
        ))),
        height: 7,
        view: 2,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued-body-block")),
        payload_hash: Hash::new(b"queued-body-payload"),
    };
    let layout = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 2,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 1,
        max_chunk_count: 2,
    };
    let canonical = wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout,
        chunk_hashes: vec![Hash::new(b"canonical chunk"); 2],
        chunk_root: Hash::new(b"canonical root"),
    };
    let conflicting = wire::PayloadManifest {
        chunk_hashes: vec![Hash::new(b"conflicting chunk"); 2],
        chunk_root: Hash::new(b"conflicting root"),
        ..canonical.clone()
    };
    let other_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"other queued block")),
        payload_hash: Hash::new(b"other queued payload"),
        ..subject
    };
    let other = wire::PayloadManifest {
        subject: other_subject,
        ..conflicting.clone()
    };
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 1, 1));
    for (command_tag, manifest) in [
        (tag(0), conflicting.clone()),
        (tag(1), canonical.clone()),
        (tag(2), other.clone()),
    ] {
        ingress
            .enqueue_authenticated(
                command_tag,
                CommandClass::Normal,
                authenticated_proposal_for_test(manifest),
            )
            .expect("queue authenticated proposal");
    }
    ingress
        .enqueue_canonical_body_available(tag(3), canonical.clone())
        .expect("trusted completion prunes its conflicting proposal and appends in FIFO order");
    assert_eq!(ingress.len(), 3);
    assert!(
        ingress
            .conflicts_with_pending_body_available(&authenticated_proposal_for_test(conflicting))
    );
    assert!(
        !ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(canonical))
    );
    assert!(
        !ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(other))
    );
    let retained_tags = ingress
        .commands
        .iter()
        .map(|queued| queued.tag)
        .collect::<Vec<_>>();
    assert_eq!(retained_tags, vec![tag(1), tag(2), tag(3)]);
    let committed = ingress
        .commands
        .back()
        .expect("canonical completion remains at the queue tail");
    assert_eq!(committed.tag, tag(3));
    assert_eq!(committed.class, CommandClass::Completion);
    assert_eq!(
        committed.admission_ordinal,
        Some(4),
        "pruning cannot reuse a retired actor-global admission ordinal"
    );
    assert!(matches!(
        ingress.commands.back().map(|queued| &queued.command),
        Some(AdapterCommand::BodyAvailable { manifest }) if manifest.subject == subject
    ));
}
#[test]
fn unpublished_body_completion_reservation_fences_conflicting_proposals() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"reserved-body-context",
        ))),
        height: 8,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"reserved-body-block")),
        payload_hash: Hash::new(b"reserved-body-payload"),
    };
    let canonical = wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 2,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1,
            max_chunk_count: 2,
        },
        chunk_hashes: vec![Hash::new(b"reserved canonical chunk"); 2],
        chunk_root: Hash::new(b"reserved canonical root"),
    };
    let conflicting = wire::PayloadManifest {
        chunk_hashes: vec![Hash::new(b"reserved conflicting chunk"); 2],
        chunk_root: Hash::new(b"reserved conflicting root"),
        ..canonical.clone()
    };
    let canonical_proposal = authenticated_proposal_for_test(canonical.clone());
    let conflicting_proposal = authenticated_proposal_for_test(conflicting);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(2, 0, 0));
    let reservation = ingress
        .reserve_canonical_body_available(tag(0), canonical)
        .expect("the unpublished completion atomically claims capacity and an ordinal");
    assert_eq!(ingress.len(), 0, "reservation is not reducer-visible");
    assert_eq!(ingress.remaining_capacity(), 0);
    assert_eq!(reservation.admission_ordinal, Some(1));
    assert!(
        ingress.conflicts_with_pending_body_available(&conflicting_proposal),
        "the unpublished canonical manifest must already fence a conflicting proposal"
    );
    assert!(
        !ingress.conflicts_with_pending_body_available(&canonical_proposal),
        "an exact proposal does not conflict with its reserved completion"
    );
    let mut mismatched = reservation.clone();
    mismatched.tag = tag(1);
    assert_eq!(
        ingress.commit_canonical_body_available(mismatched),
        Err(EnqueueError::FailClosed),
        "a stale or mismatched token must not silently lose the completion"
    );
    assert_eq!(ingress.len(), 0);
    assert_eq!(
        ingress.reserved_body_available.as_ref(),
        Some(&reservation),
        "a rejected token preserves the exact unpublished owner"
    );
    ingress
        .commit_canonical_body_available(reservation)
        .expect("the exact reservation token publishes its completion");
    let completion = ingress
        .commands
        .front()
        .expect("commit publishes the already-owned completion slot");
    assert_eq!(completion.admission_ordinal, Some(1));
    assert_eq!(completion.lifecycle_ordinal, Some(1));
    assert!(ingress.conflicts_with_pending_body_available(&conflicting_proposal));
}
#[test]
fn aborted_body_completion_retry_reclaims_the_entire_token_without_reminting() {
    let directory = TempDir::new().expect("temporary body retry directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xB1);
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0xB3))
        .expect("ordinary ingress occupies its sole unreserved slot");
    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ),
        ))
        .expect("certified progress occupies the isolated certified-fence slot");
    assert_eq!(runtime.remaining_completion_capacity(), 2);
    let reservation = runtime
        .reserve_body_available(owner_tag, manifest.clone())
        .expect("reserve one unpublished exact completion");
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    let source_after_reserve = runtime
        .ingress
        .lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect source after body reservation");
    let mut mismatched_abort = reservation.clone();
    mismatched_abort.tag = tag(1);
    runtime.abort_body_available(mismatched_abort);
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&reservation),
        "a mismatched abort has no authority to clear the exact token",
    );
    runtime.abort_body_available(reservation.clone());
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&reservation),
        "abort retains the exact token instead of orphaning its ordinal",
    );
    let retry = runtime
        .reserve_body_available(owner_tag, manifest.clone())
        .expect("exact retry reclaims the unpublished token");
    assert_eq!(retry, reservation);
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after exact retry"),
        source_after_reserve,
        "exact retry cannot mint a second physical ordinal",
    );
    let competing_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("advance the shared source through another actor owner");
    let source_before_materialization = runtime
        .ingress
        .lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect actual shared source before materialization");
    assert_eq!(Some(competing_ordinal), source_after_reserve);
    runtime
        .commit_body_available(retry)
        .expect("materialize the exact retained reservation");
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("materialization preserves the shared source"),
        source_before_materialization,
        "materialization observes but never advances the current source",
    );
}
#[test]
fn conflicting_body_completion_retry_latches_without_replacing_the_exact_token() {
    let directory = TempDir::new().expect("temporary body conflict directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xB4);
    let reservation = runtime
        .reserve_body_available(owner_tag, manifest.clone())
        .expect("reserve one unpublished exact completion");
    let source_after_reserve = runtime
        .ingress
        .lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect source after body reservation");
    let conflicting = wire::PayloadManifest {
        chunk_root: Hash::new(b"conflicting retained body root"),
        chunk_hashes: vec![Hash::new(b"conflicting retained body chunk")],
        ..manifest
    };
    assert_eq!(
        runtime.reserve_body_available(owner_tag, conflicting),
        Err(EnqueueError::DuplicateCompletionOwnership),
        "same logical slot with different evidence cannot replace the retained token",
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&reservation),
    );
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after rejected conflict"),
        source_after_reserve,
        "conflicting evidence cannot burn a fresh physical ordinal",
    );
}
#[test]
fn restored_pre_store_body_available_spends_one_fresh_completion_slot() {
    let directory = TempDir::new().expect("temporary restored body directory");
    let (_runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = tag(0);
    let manifest = runtime_manifest(&context, 0xB3);
    let lifecycle_key = Hash::new(b"restored pre-store body lifecycle");
    let body_command = AdapterCommand::BodyAvailable {
        manifest: manifest.clone(),
    };
    let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
        owner_tag,
        CommandClass::Completion,
        &body_command,
        None,
        lifecycle_key,
        1,
    )
    .expect("restore exact stage-7 producer lifecycle");
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let mut ingress =
        BoundedIngress::with_lifecycle_ordinals(RuntimeQueueConfig::new(4, 1, 1), source.clone());
    assert!(ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(ingress.remaining_capacity(), 3);
    let reservation = ingress
        .reserve_canonical_body_available_internal(
            owner_tag,
            manifest.clone(),
            Some(&owner),
            None,
            Some(RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE),
        )
        .expect("restored pre-store body spends a fresh physical slot");
    assert_eq!(reservation.lifecycle_ordinal, Some(1));
    assert_eq!(reservation.admission_ordinal, Some(2));
    assert_eq!(reservation.dormant_replacement, None);
    assert_eq!(ingress.len(), 0);
    assert_eq!(ingress.remaining_capacity(), 2);
    let source_after_reserve = source
        .next_ordinal_for_test()
        .expect("inspect source after pre-store body reservation");
    ingress.abort_canonical_body_available(reservation.clone());
    let retry = ingress
        .reserve_canonical_body_available_internal(
            owner_tag,
            manifest,
            Some(&owner),
            None,
            Some(RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE),
        )
        .expect("exact stage-7 retry reuses the unpublished token");
    assert_eq!(retry, reservation);
    assert_eq!(ingress.len(), 0);
    assert_eq!(ingress.remaining_capacity(), 2);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after exact stage-7 retry"),
        source_after_reserve,
        "an exact retry cannot mint a second physical admission"
    );
    ingress
        .commit_canonical_body_available(retry)
        .expect("stage-7 body materializes under its old logical lifecycle");
    assert!(ingress.reserved_body_available.is_none());
    assert!(ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(ingress.len(), 1);
    assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
    assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
    assert_eq!(
        ingress.commands[0].restored_producer_stage,
        Some(RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE)
    );
    assert_eq!(ingress.remaining_capacity(), 2);
}
#[test]
fn dormant_body_reservation_aliases_full_capacity_across_abort_retry_and_commit() {
    let directory = TempDir::new().expect("temporary dormant body retry directory");
    let (_runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = tag(0);
    let manifest = runtime_manifest(&context, 0xB2);
    let lifecycle_key = Hash::new(b"dormant body completion lifecycle");
    let body_command = AdapterCommand::BodyAvailable {
        manifest: manifest.clone(),
    };
    let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
        owner_tag,
        CommandClass::Completion,
        &body_command,
        None,
        lifecycle_key,
        1,
    )
    .expect("restore exact dormant body owner");
    let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8);
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let mut ingress =
        BoundedIngress::with_lifecycle_ordinals(RuntimeQueueConfig::new(2, 0, 0), source.clone());
    ingress
        .install_dormant_local_fifo_reservations(vec![dormant])
        .expect("install the full-capacity dormant completion owner");
    assert_eq!(ingress.remaining_capacity(), 0);
    let reservation = ingress
        .reserve_canonical_body_available_internal(
            owner_tag,
            manifest.clone(),
            Some(&owner),
            None,
            Some(8),
        )
        .expect("unpublished token aliases the dormant capacity owner");
    assert_eq!(reservation.lifecycle_ordinal, Some(1));
    assert_eq!(reservation.admission_ordinal, Some(2));
    assert_eq!(reservation.dormant_replacement, Some(dormant));
    assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
    assert_eq!(ingress.remaining_capacity(), 0);
    let source_after_reserve = source
        .next_ordinal_for_test()
        .expect("inspect source after dormant reservation");
    ingress.abort_canonical_body_available(reservation.clone());
    let retry = ingress
        .reserve_canonical_body_available_internal(owner_tag, manifest, Some(&owner), None, Some(8))
        .expect("exact dormant retry reclaims the whole token");
    assert_eq!(retry, reservation);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after dormant retry"),
        source_after_reserve,
    );
    assert_eq!(
        ingress.reserve_canonical_body_available_internal(
            owner_tag,
            retry.manifest().clone(),
            Some(&owner),
            None,
            Some(9),
        ),
        Err(EnqueueError::FailClosed),
        "retry cannot replace the exact dormant stage",
    );
    assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));
    let source_before_failed_commit = source
        .next_ordinal_for_test()
        .expect("inspect source before rejected dormant commit");
    let mut mismatched_commit = retry.clone();
    mismatched_commit.tag = tag(1);
    assert_eq!(
        ingress.commit_canonical_body_available(mismatched_commit),
        Err(EnqueueError::FailClosed),
    );
    assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));
    assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("rejected dormant commit preserves the source"),
        source_before_failed_commit,
    );
    ingress
        .commit_canonical_body_available(retry)
        .expect("materialization atomically replaces token and dormant backing");
    assert!(ingress.reserved_body_available.is_none());
    assert!(ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(ingress.len(), 1);
    assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
    assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("materialization preserves the source"),
        source_after_reserve,
    );
}
#[test]
fn mismatched_body_completion_commit_fails_closed_without_losing_reservation() {
    let directory = TempDir::new().expect("temporary body reservation directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xA4);
    let reservation = runtime
        .reserve_body_available(owner_tag, manifest)
        .expect("reserve the exact unpublished completion");
    let exact = reservation.clone();
    let mut mismatched = reservation;
    mismatched.tag = tag(1);
    assert_eq!(
        runtime.commit_body_available(mismatched),
        Err(EnqueueError::FailClosed)
    );
    assert!(runtime.fail_closed);
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&exact),
        "the invalid token cannot consume the exact reserved owner"
    );
}
#[test]
fn retiring_exact_body_completion_releases_a_capacity_one_ingress_slot() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"retired-body-context",
        ))),
        height: 11,
        view: 4,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retired-body-block")),
        payload_hash: Hash::new(b"retired-body-payload"),
    };
    let layout = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 2,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 1,
        max_chunk_count: 2,
    };
    let original = wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout,
        chunk_hashes: vec![Hash::new(b"retired chunk"); 2],
        chunk_root: Hash::new(b"retired root"),
    };
    let replacement = wire::PayloadManifest {
        round: wire::ConsensusRound {
            view: round.view + 1,
            ..round
        },
        chunk_hashes: vec![Hash::new(b"replacement chunk"); 2],
        chunk_root: Hash::new(b"replacement root"),
        ..original.clone()
    };
    let original_tag = tag(4);
    let replacement_tag = tag(5);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(2, 0, 0));
    ingress
        .enqueue_canonical_body_available(original_tag, original.clone())
        .expect("the original completion claims the sole slot");
    assert_eq!(
        ingress.enqueue_canonical_body_available(replacement_tag, replacement.clone()),
        Err(EnqueueError::Full)
    );
    assert_eq!(
        ingress.retire_canonical_body_available(original_tag, &original),
        1
    );
    assert_eq!(ingress.remaining_capacity(), 1);
    ingress
        .enqueue_canonical_body_available(replacement_tag, replacement.clone())
        .expect("retirement releases the sole completion slot");
    assert_eq!(ingress.len(), 1);
    assert!(matches!(
        ingress.commands.front(),
        Some(TaggedCommand {
            tag,
            command: AdapterCommand::BodyAvailable { manifest },
            ..
        }) if *tag == replacement_tag && manifest == &replacement
    ));
}
#[test]
fn exact_authenticated_progress_retransmission_is_queue_coalesced() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"coalesced-progress-context",
        ))),
        height: 7,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-progress-block")),
        payload_hash: Hash::new(b"coalesced-progress-payload"),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"coalesced parent state"),
        Hash::new(b"coalesced post state"),
        Hash::new(b"coalesced ordinary writes"),
        1,
        Hash::new(b"coalesced executed block wire"),
    );
    let payload = wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    });
    let authenticated =
        || AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload.clone()));
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(0), CommandClass::Progress, authenticated())
            .expect("first authenticated CommitQC owns one queue slot"),
        tag(0)
    );
    let admitted_origin = ingress.commands[0].causal_origin.clone();
    let admitted_lifecycle_ordinal = ingress.commands[0].lifecycle_ordinal;
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(1), CommandClass::Progress, authenticated())
            .expect("equal authenticated retransmission is coalesced"),
        tag(0),
        "a coalesced retransmission returns the original queue owner's tag"
    );
    assert_eq!(ingress.len(), 1);
    assert_eq!(ingress.commands[0].causal_origin, admitted_origin);
    assert_eq!(
        ingress.commands[0].lifecycle_ordinal, admitted_lifecycle_ordinal,
        "an exact transport retry retains the first lifecycle owner"
    );
    let dispatched = ingress
        .pop_next()
        .expect("the sole queued CommitQC is dispatchable");
    assert_eq!(dispatched.class, CommandClass::Progress);
    assert!(matches!(
        dispatched.command,
        AdapterCommand::Authenticated(_)
    ));
    assert_eq!(ingress.len(), 0);
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated())
            .expect("a later retransmission starts a new ownership interval"),
        tag(2)
    );
    assert_eq!(ingress.len(), 1);
    assert!(
        !ingress.commands[0]
            .causal_origin
            .same_lifecycle(&admitted_origin),
        "a later interval is not spliced into the drained causal root"
    );
}
#[test]
fn runtime_merges_alternate_sources_for_one_semantic_request() {
    let directory = TempDir::new().expect("temporary alternate-source runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x76);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let source_a = PeerId::new(keys[1].public_key().clone());
    let source_b = PeerId::new(keys[2].public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(source_a.clone(), 2);
    let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
    let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
    let ownership_a = fair_runtime_ownership_with_reply_route(
        &message,
        semantic_origin.clone(),
        source_a,
        route_a.clone(),
    );
    let ownership_b = fair_runtime_ownership_with_reply_route(
        &message,
        semantic_origin,
        source_b,
        route_b.clone(),
    );
    let owner_tag = runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first source admits the semantic request");
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership_b)
            .expect("alternate source attaches to the retained request"),
        owner_tag
    );
    assert_eq!(runtime.queued_commands(), 1);
    let ownership = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("coalesced runtime command retains exact source ownership");
    assert!(ownership.validate_exact());
    let projection_hash = ownership.projection_hash;
    let direct = ownership
        .direct
        .first()
        .expect("proposal retains direct fair-ingress ownership");
    assert_eq!(
        direct
            .current_reply_routes()
            .expect("route-aware fair ownership")
            .len(),
        2
    );
    assert!(routes.retire(&route_a));
    let ownership = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("queued ownership survives a normal source disconnect");
    assert!(ownership.validate_exact());
    assert_eq!(
        ownership.projection_hash, projection_hash,
        "connection liveness is not part of immutable runtime ownership identity"
    );
    assert!(
        ownership
            .direct
            .first()
            .and_then(FairV2IngressOwnershipEvidence::current_reply_routes)
            .is_some_and(|owned| {
                owned.iter().any(|route| route.same_delivery(&route_a))
                    && owned.iter().any(|route| route.same_delivery(&route_b))
            }),
        "retirement is applied only by an authoritative prune receipt"
    );
    assert!(!runtime.fail_closed);
}
#[test]
fn later_same_semantic_fair_retry_retains_runtime_lifecycle_root() {
    let directory = TempDir::new().expect("temporary lifecycle-retry runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0xD1);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let authenticated_via = PeerId::new(keys[1].public_key().clone());
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let retained_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint first fair lifecycle");
    let retry_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint later fair retry lifecycle");
    let retained = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(&message, semantic_origin.clone(), authenticated_via.clone()),
        retained_ordinal,
    );
    let retry = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(&message, semantic_origin, authenticated_via),
        retry_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), retained)
        .expect("first fair lifecycle enters runtime");
    let physical_ordinal = runtime.ingress.commands[0]
        .admission_ordinal
        .expect("runtime admission owns one physical position");
    let next_before_retry = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect shared source before coalescing retry");
    runtime
        .enqueue_network_with_ingress_ownership(message, retry)
        .expect("later same-semantic retry coalesces");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source after coalescing retry"),
        next_before_retry,
        "runtime coalescence cannot mint a second physical FIFO position"
    );
    let queued = &runtime.ingress.commands[0];
    assert_eq!(queued.admission_ordinal, Some(physical_ordinal));
    assert_eq!(queued.lifecycle_ordinal, Some(retained_ordinal));
    assert_eq!(
        queued.causal_origin.root_lifecycle_ordinal,
        Some(retained_ordinal)
    );
    let ownership = queued
        .ingress_ownership
        .as_ref()
        .expect("coalesced command retains exact fair ownership");
    assert_eq!(
        ownership.earliest_lifecycle_ordinal(),
        Ok(Some(retained_ordinal))
    );
    let carrier = ownership
        .direct
        .first()
        .expect("same semantic retry remains one bounded carrier");
    assert_eq!(carrier.admission_count, 2);
    assert_eq!(carrier.first.lifecycle_ordinal, Some(retained_ordinal));
    assert_eq!(carrier.latest.lifecycle_ordinal, Some(retained_ordinal));
    assert!(ownership.validate_exact());
    assert!(!runtime.fail_closed);
}
#[test]
fn ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it() {
    let directory = TempDir::new().expect("temporary fair-to-runtime predecessor directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0xD6);
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let fair_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint ordinary fair-ingress predecessor lifecycle");
    let ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        fair_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, ownership)
        .expect("transfer ordinary fair predecessor into serialized runtime");
    let serve_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint exact Serve target behind the transferred predecessor");
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for exact predecessor comparison");
    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("transferred Fair owner participates in runtime minimum"),
        "the exact Serve target cannot prepare past the transferred predecessor"
    );
    let (_, consumed) = runtime
        .ingress
        .pop_next_with_ownership()
        .expect("runtime predecessor selection remains exact")
        .expect("ordinary Fair predecessor is ready");
    assert_eq!(consumed.lifecycle_ordinal, fair_ordinal);
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("recompute minimum after consuming the predecessor"),
        "Serve becomes eligible only after the transferred lifecycle drains"
    );
    assert!(!runtime.fail_closed);
}
#[test]
fn older_frozen_aggregate_carrier_rebases_queued_runtime_minimum() {
    let directory = TempDir::new().expect("temporary aggregate-rebase runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0xD2),
        ));
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let older_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint frozen older aggregate lifecycle");
    let newer_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint later independently admissible aggregate lifecycle");
    let newer = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
        newer_ordinal,
    );
    let older = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
        older_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), newer)
        .expect("newer admissible aggregate enters runtime first");
    assert_eq!(
        runtime.ingress.commands[0].lifecycle_ordinal,
        Some(newer_ordinal)
    );
    let physical_ordinal = runtime.ingress.commands[0].admission_ordinal;
    let next_before_older = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect shared source before older carrier transfer");
    let mut unfrozen_older = older.clone();
    unfrozen_older.runtime_physical_cut = None;
    assert!(unfrozen_older.validate_exact());
    let unfrozen_projection =
        RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, unfrozen_older)
            .expect("pre-dequeue aggregate identity remains exact");
    assert!(!unfrozen_projection.validate_frozen_physical());
    let retained_projection = runtime.ingress.commands[0]
        .ingress_ownership
        .as_ref()
        .expect("newer aggregate retains checked ingress ownership");
    let mut mixed_preview = retained_projection.clone();
    mixed_preview
        .merge_downstream(unfrozen_projection)
        .expect("capacity probe can preview a frozen/unfrozen aggregate merge");
    assert!(mixed_preview.validate_exact());
    assert!(
        !mixed_preview.validate_frozen_physical(),
        "only checked dequeue may promote the preview to mutable runtime ownership"
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, older)
        .expect("older frozen aggregate carrier joins the queued envelope");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source after aggregate reconciliation"),
        next_before_older,
        "carrier reconciliation cannot mint another physical command"
    );
    let queued = &runtime.ingress.commands[0];
    assert_eq!(queued.admission_ordinal, physical_ordinal);
    assert_eq!(queued.lifecycle_ordinal, Some(older_ordinal));
    assert_eq!(
        queued.causal_origin.root_lifecycle_ordinal,
        Some(older_ordinal)
    );
    let ownership = queued
        .ingress_ownership
        .as_ref()
        .expect("aggregate command retains both fair carriers");
    assert_eq!(ownership.direct.len(), 2);
    assert_eq!(
        ownership.earliest_lifecycle_ordinal(),
        Ok(Some(older_ordinal))
    );
    assert!(ownership.validate_exact());
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before exact Serve comparison");
    let serve_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint exact Serve barrier after both aggregate carriers");
    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("compare reconciled aggregate minimum"),
        "the later-transferred frozen carrier must become the active minimum"
    );
    assert!(!runtime.fail_closed);
}
#[test]
fn network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals() {
    let unminted_directory = TempDir::new().expect("temporary unminted-fair runtime directory");
    let (mut unminted_runtime, context, keys) =
        authenticated_network_runtime(&unminted_directory, RuntimeQueueConfig::new(8, 2, 2));
    let source = unminted_runtime.ingress.lifecycle_ordinals.clone();
    let unminted_ordinal = source
        .next_ordinal_for_test()
        .expect("inspect unminted source position")
        .expect("fresh source has a first ordinal");
    let first_message = signed_runtime_proposal(&context, &keys, 0xD3);
    let first_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &first_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        unminted_ordinal,
    );
    assert!(matches!(
        unminted_runtime.enqueue_network_with_ingress_ownership(first_message, first_ownership),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(unminted_runtime.fail_closed);
    assert_eq!(unminted_runtime.queued_commands(), 0);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("unminted rejection preserves the source"),
        Some(unminted_ordinal)
    );
    let collision_directory = TempDir::new().expect("temporary fair-collision runtime directory");
    let (mut collision_runtime, context, keys) =
        authenticated_network_runtime(&collision_directory, RuntimeQueueConfig::new(8, 2, 2));
    let source = collision_runtime.ingress.lifecycle_ordinals.clone();
    let shared_ordinal = source.reserve_one().expect("mint one exact fair lifecycle");
    let admitted_message = signed_runtime_proposal(&context, &keys, 0xD4);
    let conflicting_message = signed_runtime_proposal(&context, &keys, 0xD5);
    let admitted_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &admitted_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        shared_ordinal,
    );
    let conflicting_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &conflicting_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        shared_ordinal,
    );
    collision_runtime
        .enqueue_network_with_ingress_ownership(admitted_message, admitted_ownership)
        .expect("first exact fair lifecycle enters runtime");
    let next_before_collision = source
        .next_ordinal_for_test()
        .expect("inspect source before unrelated collision");
    assert!(matches!(
        collision_runtime
            .enqueue_network_with_ingress_ownership(conflicting_message, conflicting_ownership,),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(collision_runtime.fail_closed);
    assert_eq!(collision_runtime.queued_commands(), 1);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("collision rejection preserves the physical source"),
        next_before_collision,
        "unrelated ordinal collision must fail before a FIFO position is minted"
    );
}
#[test]
fn runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent() {
    let directory = TempDir::new().expect("temporary distinct-origin runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x77);
    let origin_a = PeerId::new(keys[0].public_key().clone());
    let origin_b = PeerId::new(keys[1].public_key().clone());
    let source = PeerId::new(keys[2].public_key().clone());
    let ownership_a = fair_runtime_ownership(&message, origin_a, source.clone());
    let ownership_b = fair_runtime_ownership(&message, origin_b, source);
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first semantic origin owns one runtime occurrence");
    runtime
        .enqueue_network_with_ingress_ownership(message, ownership_b)
        .expect("distinct semantic origin retains an independent occurrence");
    assert_eq!(runtime.queued_commands(), 2);
    assert!(runtime.ingress.commands.iter().all(|queued| {
        queued
            .ingress_ownership
            .as_ref()
            .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
    }));
    let mut commands = runtime.ingress.commands.iter();
    let first = commands.next().expect("first semantic root is retained");
    let second = commands.next().expect("second semantic root is retained");
    assert!(
        !first.causal_origin.same_lifecycle(&second.causal_origin),
        "identical wire bytes from unrelated semantic origins cannot coalesce"
    );
    assert!(!runtime.fail_closed);
}
#[test]
fn busy_deferred_request_merges_alternate_source_and_services_exact_carrier() {
    let directory = TempDir::new().expect("temporary Busy-deferred ownership directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before authenticated ingress");
    let round_tag = runtime.round_tag();
    let message = signed_runtime_proposal(&context, &keys, 0x78);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let request_lifecycle_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("preown the authenticated request before timeout signing");
    let ownership_a = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &message,
            semantic_origin.clone(),
            PeerId::new(keys[1].public_key().clone()),
        ),
        request_lifecycle_ordinal,
    );
    let deadline = now + runtime.round_timeout();
    let timeout_step = runtime
        .step(deadline)
        .expect("install a runtime-owned local signing fence");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout dispatch retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("timeout dispatch unexpectedly idled")
    };
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout Sign retains its lifecycle owner");
    assert_eq!(timeout_effect_ownership.len(), 1);
    let (signature_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending timeout signer owner");
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first source enters runtime ingress");
    assert!(matches!(
        runtime.step(deadline),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let queued_owner = runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains its exact queue owner");
    assert!(queued_owner.validate_exact().is_ok());
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    let admission_ordinal = *runtime
        .deferred_ingress_ownership
        .keys()
        .next()
        .expect("authenticated Busy owner has an actor-global ordinal");
    let projection_before_alternate =
        runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash;
    let ownership_b = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &message,
            semantic_origin,
            PeerId::new(keys[2].public_key().clone()),
        ),
        request_lifecycle_ordinal,
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership_b)
            .expect("alternate source attaches to the Busy owner"),
        round_tag
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert_ne!(
        runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash,
        projection_before_alternate,
        "alternate ownership history must change the exact runtime projection"
    );
    let replay_origin = &runtime.deferred_remote_proposal_replay[&admission_ordinal];
    assert_eq!(
        replay_origin.ingress,
        runtime.deferred_ingress_ownership[&admission_ordinal],
        "the Proposal replay sidecar must atomically adopt merged ingress ownership"
    );
    assert!(
        replay_origin
            .ingress
            .exactly_matches_authenticated(&replay_origin.authenticated)
    );
    let signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(signature_tag, signature, &timeout_effect_ownership[0])
        .expect("enqueue the exact signing completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the pending signer after completion enqueue");

    assert!(matches!(
        runtime.step(deadline),
        Ok(RuntimeStep::Advanced(ref effects))
            if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("the exact signer completion retains fence scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::FenceCompletion
    );
    runtime
        .take_effect_ownership(1)
        .expect("the executor consumes the TimeoutVote broadcast owner");

    let deferred_effects = match runtime.step(deadline) {
        Ok(RuntimeStep::Advanced(effects)) => effects,
        other => panic!("deferred owner did not receive its service turn: {other:?}"),
    };
    assert!(
        deferred_effects.is_empty()
            || matches!(
                deferred_effects.as_slice(),
                [AdapterEffect::FetchBody { .. }]
            ),
        "the timeout intent may obsolete the proposal, but no unrelated effect may replace it: {deferred_effects:?}"
    );
    let deferred_owner = runtime
        .take_last_scheduler_ownership()
        .expect("deferred service hands off its exact owner");
    runtime
        .take_effect_ownership(deferred_effects.len())
        .expect("the executor consumes the deferred proposal effect owner");
    let RuntimeSelectedCandidateOwnership::ExactDeferred(deferred) = &deferred_owner.candidate
    else {
        panic!("expected exact deferred scheduler ownership")
    };
    assert!(
        deferred
            .ingress_ownership
            .as_ref()
            .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
    );
    assert!(runtime.deferred_ingress_ownership.is_empty());

    let periodic_effects = match runtime.step(deadline) {
        Ok(RuntimeStep::Advanced(effects)) => effects,
        other => panic!("the frozen retransmit did not receive its bounded turn: {other:?}"),
    };
    assert!(
        matches!(periodic_effects.as_slice(), [AdapterEffect::Broadcast(_)]),
        "the post-signature retransmit repeats the durable TimeoutVote: {periodic_effects:?}"
    );
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("the frozen retransmit retains exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer
    );
    runtime
        .take_effect_ownership(periodic_effects.len())
        .expect("the executor consumes the frozen retransmit effect owner");
    assert!(!runtime.fail_closed);
}
#[test]
fn busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation() {
    let directory = TempDir::new().expect("temporary Busy-deferred rebase directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before Busy-deferred aggregate ingress");
    let owner_tag = runtime.round_tag();
    let timeout = runtime
        .driver
        .timeout_elapsed(owner_tag)
        .expect("install a signer fence before aggregate dispatch");
    assert!(
        matches!(
            timeout.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ),
        "unexpected timeout effects: {:?}",
        timeout.effects()
    );
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(
                &context,
                &keys,
                0x79,
                wire::GlobalPhase::Prepare,
            ),
        ));
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let mutation_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the oldest identity-mutation carrier");
    let older_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the older delayed aggregate carrier");
    let newer_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the newer aggregate carrier admitted first");
    let newer = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
        newer_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), newer)
        .expect("newer aggregate carrier enters runtime before the frozen predecessor");
    assert!(matches!(
        runtime.step(now),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains the exact queued owner");
    assert!(selected.validate_exact().is_ok());
    let deferred_ordinals = runtime
        .deferred_ingress_ownership
        .keys()
        .copied()
        .collect::<Vec<_>>();
    let [deferred_ordinal] = deferred_ordinals.as_slice() else {
        panic!("aggregate dispatch must retain exactly one Busy-deferred owner")
    };
    let deferred_ordinal = *deferred_ordinal;
    assert_eq!(
        runtime.deferred_ingress_ownership[&deferred_ordinal].earliest_lifecycle_ordinal(),
        Ok(Some(newer_ordinal))
    );
    assert_eq!(
        runtime.deferred_lifecycle_ownership[&deferred_ordinal].lifecycle_ordinal(),
        newer_ordinal
    );
    let frozen_physical_cut = runtime.deferred_lifecycle_ownership[&deferred_ordinal].physical_cut;
    let frozen_source_physical_ordinal =
        runtime.deferred_lifecycle_ownership[&deferred_ordinal].source_physical_ordinal;
    let frozen_runtime_seal = runtime.deferred_lifecycle_ownership[&deferred_ordinal]
        .runtime_seal
        .clone();
    assert_ne!(frozen_physical_cut, 0);
    assert!(frozen_source_physical_ordinal.is_some());
    let older = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
        older_ordinal,
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), older)
            .expect("older frozen carrier joins the exact Busy-deferred aggregate"),
        owner_tag
    );
    let merged = runtime
        .deferred_ingress_ownership
        .get(&deferred_ordinal)
        .expect("Busy-deferred aggregate retains the merged carrier set");
    assert_eq!(merged.direct.len(), 2);
    assert_eq!(merged.earliest_lifecycle_ordinal(), Ok(Some(older_ordinal)));
    assert!(merged.validate_exact());
    let rebased_owner = runtime
        .deferred_lifecycle_ownership
        .get(&deferred_ordinal)
        .expect("Busy-deferred aggregate retains its rebased lifecycle owner");
    assert_eq!(rebased_owner.lifecycle_ordinal(), older_ordinal);
    assert_eq!(
        rebased_owner.physical_cut, frozen_physical_cut,
        "logical owner replacement cannot refresh the continuation's physical cut"
    );
    assert_eq!(
        rebased_owner.source_physical_ordinal, frozen_source_physical_ordinal,
        "logical owner replacement cannot replace the source occurrence"
    );
    assert_eq!(
        rebased_owner.runtime_seal, frozen_runtime_seal,
        "logical owner replacement cannot replace the admitted occurrence capability"
    );
    assert_eq!(
        rebased_owner.causal_origin().root_lifecycle_ordinal,
        Some(older_ordinal)
    );
    assert_eq!(
        rebased_owner.causal_origin().root_ingress_identity,
        Some(runtime_ingress_causal_origin_projection_hash(merged))
    );
    assert!(rebased_owner.validate_exact());
    let healthy_owner = rebased_owner.clone();
    let mutation = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &message,
        fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[0].public_key().clone())),
            mutation_ordinal,
        ),
    )
    .expect("oldest aggregate carrier has exact runtime ownership");
    assert_eq!(
        mutation.earliest_lifecycle_ordinal(),
        Ok(Some(mutation_ordinal))
    );
    let mut identity_mutated_lifecycle_owner = healthy_owner.owner.clone();
    identity_mutated_lifecycle_owner
        .causal_origin
        .root_ingress_identity = Some(Hash::new(b"mutated Busy-deferred ingress identity"));
    identity_mutated_lifecycle_owner.causal_origin.lifecycle_key =
        runtime_candidate_causal_origin_lifecycle_key(
            &identity_mutated_lifecycle_owner.causal_origin,
        );
    identity_mutated_lifecycle_owner
        .causal_origin
        .projection_hash = runtime_candidate_causal_origin_projection_hash(
        &identity_mutated_lifecycle_owner.causal_origin,
    );
    identity_mutated_lifecycle_owner.projection_hash =
        runtime_lifecycle_owner_projection_hash(&identity_mutated_lifecycle_owner);
    assert!(
        matches!(
            RuntimeDeferredLifecycleOwnership::new(
                identity_mutated_lifecycle_owner,
                healthy_owner.deferred_admission_ordinal,
                healthy_owner.current_ingress,
                healthy_owner.source_physical_ordinal,
                healthy_owner.physical_cut,
                healthy_owner.runtime_seal.clone(),
            ),
            Err(EnqueueError::FailClosed)
        ),
        "the adapter-private seal rejects a coherently rehashed causal identity substitution"
    );
    runtime
        .reconcile_deferred_ingress_ownership(Some((deferred_ordinal, mutation)))
        .expect("the same earlier carrier rebases after restoring the exact identity");
    let final_ingress = &runtime.deferred_ingress_ownership[&deferred_ordinal];
    assert_eq!(final_ingress.direct.len(), 3);
    assert_eq!(
        final_ingress.earliest_lifecycle_ordinal(),
        Ok(Some(mutation_ordinal))
    );
    let final_owner = &runtime.deferred_lifecycle_ownership[&deferred_ordinal];
    assert_eq!(final_owner.lifecycle_ordinal(), mutation_ordinal);
    assert_eq!(final_owner.physical_cut, frozen_physical_cut);
    assert_eq!(
        final_owner.source_physical_ordinal,
        frozen_source_physical_ordinal
    );
    assert_eq!(
        final_owner.runtime_seal, frozen_runtime_seal,
        "repeated aggregate rebasing retains the first admitted occurrence capability"
    );
    assert_eq!(
        final_owner.causal_origin().root_lifecycle_ordinal,
        Some(mutation_ordinal)
    );
    assert_eq!(
        final_owner.causal_origin().root_ingress_identity,
        Some(runtime_ingress_causal_origin_projection_hash(final_ingress))
    );
    assert!(final_owner.validate_exact());
    assert!(!runtime.fail_closed);
}
#[test]
fn distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner() {
    let directory = TempDir::new().expect("temporary pre-runtime leader-wire directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before Busy-deferred aggregate ingress");
    let owner_tag = runtime.round_tag();
    let timeout = runtime
        .driver
        .timeout_elapsed(owner_tag)
        .expect("install a signer fence before aggregate dispatch");
    assert!(matches!(
        timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(
                &context,
                &keys,
                0x7A,
                wire::GlobalPhase::Prepare,
            ),
        ));
    let first_source = context.roster[2].validator.clone();
    let second_source = context.roster[1].validator.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), first_source)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let [first_ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("fixture creates one exact runtime-owned carrier");
    let mut same_token_pre_runtime = first_ownership.clone();
    same_token_pre_runtime.runtime_physical_cut = None;
    same_token_pre_runtime.leader_wire_runtime_receipt = None;
    assert!(same_token_pre_runtime.validate_exact());
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), first_ownership)
        .expect("first leader-wire carrier enters the runtime");
    assert!(matches!(
        runtime.step(now),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains the first exact carrier");
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert!(
        runtime
            .can_admit_network_message_with_ingress_ownership(&message, &same_token_pre_runtime,),
        "an exact pre-runtime retry may merge into its existing Busy owner",
    );
    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(second_source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let selected = leader_wire_ingress.try_recv_if(|inbound| {
        let BlockMessage::V2(candidate) = inbound.message() else {
            return true;
        };
        let ownership = inbound
            .ingress_ownership()
            .expect("productive fair ingress attaches exact ownership");
        runtime.can_admit_network_message_with_ingress_ownership(candidate, ownership)
    });
    assert!(
        selected.is_none(),
        "a distinct productive leader-wire token must remain physically queued behind the Busy owner"
    );
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert!(!runtime.fail_closed);
}
#[test]
fn restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner() {
    let directory = TempDir::new().expect("temporary restored-TC runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let started_at = Instant::now();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime before freezing its clock owner");
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
            signed_runtime_timeout_certificate(&context, &keys),
        ));
    let source = context.roster[1].validator.clone();
    let post_snapshot_source = context.roster[2].validator.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), source)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("publish the pre-timeout carrier high-water mark");
    let [runtime_owned]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("fixture creates one exact durable Runtime owner");
    let restored_receipt = runtime_owned
        .leader_wire_runtime_receipt()
        .expect("pre-crash TC owns one exact runtime receipt")
        .clone();
    // The runner publishes the receiver high-water mark immediately
    // before its serialized turn. A producer can then atomically reserve
    // and publish a distinct valid TC carrier before that turn freezes the
    // timeout owner. Preserve that real interleaving instead of fabricating
    // a post-cut physical ordinal in the ownership evidence.
    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(post_snapshot_source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    // Restart normalizes Runtime/VolatileTerminal to Dormant.  Its exact
    // retransmission regains an Ingress carrier with the old immutable
    // token, but the new process has not yet frozen the Runtime receipt or
    // receiver physical cut.  This is the precise pre-dequeue projection
    // observed on Taira after restart.
    let mut restored_pre_runtime = runtime_owned.clone();
    restored_pre_runtime.runtime_physical_cut = None;
    restored_pre_runtime.leader_wire_runtime_receipt = None;
    assert!(restored_pre_runtime.validate_exact());
    assert_eq!(
        restored_pre_runtime.leader_wire_token(),
        Some(restored_receipt.token())
    );
    let original_physical_ordinal = restored_pre_runtime
        .physical_admission_ordinal()
        .expect("the first delivery owns one physical occurrence");
    assert_eq!(
        restored_receipt.token().admission_ordinal(),
        original_physical_ordinal,
        "the first physical delivery is not a replay"
    );
    let deadline = started_at + runtime.round_timeout();
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(deadline)
        .expect("freeze the new process's absolute-timeout owner");
    let timeout_physical_cut = runtime
        .timeout_owner_physical_cut
        .expect("the frozen timeout owns one immutable receiver cut");
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("refresh the live receiver high-water mark after freezing the timeout cut");
    assert!(
        restored_receipt.owner().admission_ordinal() < timeout_owner.lifecycle_ordinal(),
        "the durable replay must retain its pre-restart scheduler position"
    );
    assert!(
        leader_wire_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.ingress_ownership().is_some_and(|ownership| {
                    runtime.can_admit_network_message_with_ingress_ownership(candidate, ownership)
                })
            })
            .is_none(),
        "the real post-snapshot carrier must remain queued behind the frozen timeout"
    );
    let mut fresh_inbound = leader_wire_ingress
        .try_recv()
        .expect("force the real post-snapshot carrier across the test-only dequeue seam");
    let fresh_runtime = fresh_inbound
        .take_ingress_ownership()
        .expect("checked dequeue retains the real post-snapshot ownership");
    let fresh_token = fresh_runtime
        .leader_wire_token()
        .expect("the second TC owns one exact productive token");
    let fresh_physical_ordinal = fresh_runtime
        .physical_admission_ordinal()
        .expect("the second TC owns one exact physical carrier");
    assert_eq!(fresh_token.admission_ordinal(), fresh_physical_ordinal);
    assert!(u128::from(fresh_physical_ordinal) >= timeout_physical_cut);
    assert!(fresh_token.scheduler_ordinal() < timeout_owner.lifecycle_ordinal());
    assert_eq!(
        runtime.clock_owner_reservation_blocks_occurrence(
            fresh_token.scheduler_ordinal(),
            fresh_physical_ordinal,
        ),
        Ok(true),
        "the fresh carrier must exercise the active timeout reservation"
    );
    assert!(
        !runtime.can_admit_network_message_with_ingress_ownership(&message, &fresh_runtime,),
        "a fresh certified post-cut carrier cannot impersonate a durable replay"
    );
    assert!(fresh_runtime.validate_exact());
    let fresh_runtime_projection =
        RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fresh_runtime.clone())
            .expect("fresh carrier projects exact runtime ingress ownership");
    assert_eq!(
        fresh_runtime_projection.is_physical_leader_wire_replay(),
        Ok(false),
        "equal token and carrier ordinals identify a fresh delivery"
    );
    let queued_before_fresh = runtime.queued_commands();
    let receipts_before_fresh = runtime.leader_wire_runtime_receipts.len();
    let terminals_before_fresh = runtime.pending_leader_wire_terminals.len();
    assert!(matches!(
        runtime.enqueue_network_with_ingress_ownership(message.clone(), fresh_runtime),
        Err(NetworkIngressError::Backpressure(EnqueueError::Full))
    ));
    assert_eq!(runtime.queued_commands(), queued_before_fresh);
    assert_eq!(
        runtime.leader_wire_runtime_receipts.len(),
        receipts_before_fresh
    );
    assert_eq!(
        runtime.pending_leader_wire_terminals.len(),
        terminals_before_fresh
    );
    assert!(
        !runtime.fail_closed,
        "rejecting the fresh carrier is retryable backpressure"
    );
    // The retransmitted carrier is physically new even though its durable
    // leader-wire token is logically older.  Recreate the dequeue-time
    // projection on the far side of the frozen timeout cut.
    let timeout_cut_ordinal = u64::try_from(timeout_physical_cut)
        .expect("the small test receiver cut fits its physical ordinal");
    let restored_physical_ordinal = timeout_cut_ordinal
        .max(restored_receipt.token().admission_ordinal())
        .checked_add(1)
        .expect("the small replay ordinal has a successor");
    runtime
        .set_ingress_physical_cut(
            u128::from(restored_physical_ordinal)
                .checked_add(1)
                .expect("the modeled restored carrier has a receiver successor"),
        )
        .expect("publish the modeled restored carrier high-water mark");
    restored_pre_runtime.first.physical_admission_ordinal = restored_physical_ordinal;
    restored_pre_runtime.latest.physical_admission_ordinal = restored_physical_ordinal;
    assert!(restored_pre_runtime.validate_exact());
    assert!(
        restored_receipt.token().admission_ordinal() < restored_physical_ordinal,
        "the admitted carrier must be an exact physical replay"
    );
    assert_eq!(
        runtime.clock_owner_reservation_blocks_occurrence(
            restored_receipt.token().scheduler_ordinal(),
            restored_physical_ordinal,
        ),
        Ok(true),
        "the restored carrier must exercise the narrow replay exception"
    );
    assert!(
        runtime.can_admit_network_message_with_ingress_ownership(&message, &restored_pre_runtime,),
        "a later clock reservation cannot pin an older durable replay at fair ingress"
    );
    // Model the checked dequeue's atomic Ingress -> Runtime handoff.  The
    // mutating seam must agree with the read-only predicate and still
    // authenticate the TC before it owns reducer capacity.
    let mut restored_runtime = restored_pre_runtime;
    restored_runtime.runtime_physical_cut = u128::from(restored_physical_ordinal).checked_add(1);
    restored_runtime.leader_wire_runtime_receipt = Some(restored_receipt);
    assert!(restored_runtime.validate_exact());
    let restored_runtime_projection =
        RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, restored_runtime.clone())
            .expect("restored carrier projects exact runtime ingress ownership");
    assert_eq!(
        restored_runtime_projection.is_physical_leader_wire_replay(),
        Ok(true),
        "a strictly later carrier identifies the retained physical replay"
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, restored_runtime)
        .expect("authenticate and enqueue the restored TC under its old owner");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.leader_wire_runtime_receipts.len(), 1);
    assert!(
        runtime
            .ingress
            .commands
            .front()
            .is_some_and(|queued| queued.restored_producer_stage.is_none()),
        "authenticated replay must use the ordinary Admit path"
    );
    let timeout_step = runtime
        .step(deadline)
        .expect("the absolute timeout retains its already-frozen turn");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("frozen timeout unexpectedly idled")
    };
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("persisted TimeoutIntent transfers one signer owner");
    assert_eq!(timeout_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending timeout signer owner");
    let tc_step = runtime
        .try_step_pacemaker_escape(deadline)
        .expect("restored authenticated TC remains a certified escape")
        .expect("TC runs after the one-shot timeout persistence turn");
    let RuntimeStep::Advanced(tc_effects) = tc_step else {
        panic!("restored TC unexpectedly idled")
    };
    assert!(matches!(
        tc_effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == 1
    ));
    assert_eq!(runtime.round_tag().view(), 1);
    runtime
        .take_last_scheduler_ownership()
        .expect("TC publishes exact scheduler ownership");
    runtime
        .take_effect_ownership(tc_effects.len())
        .expect("consume the TC EnterView ownership");
    let terminals = runtime.take_leader_wire_runtime_terminals();
    assert_eq!(
        terminals.len(),
        1,
        "the restored TC terminalizes exactly once"
    );
    assert!(!runtime.fail_closed);
}
