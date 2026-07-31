
    #[test]
    fn causal_lifecycle_key_ignores_only_process_generation() {
        let first_tag = EventTag::new(9, 4, Generation::new(1));
        let replay_tag = EventTag::new(9, 4, Generation::new(7));
        let different_view = EventTag::new(9, 5, Generation::new(7));
        let command = FakeCommand::record(0xA5);

        let first =
            RuntimeCandidateCausalOrigin::mint(first_tag, CommandClass::Progress, &command, None);
        let replay =
            RuntimeCandidateCausalOrigin::mint(replay_tag, CommandClass::Progress, &command, None);
        let other_view = RuntimeCandidateCausalOrigin::mint(
            different_view,
            CommandClass::Progress,
            &command,
            None,
        );

        assert!(first.same_lifecycle(&replay));
        assert_eq!(first.lifecycle_key, replay.lifecycle_key);
        assert_ne!(
            first.projection_hash, replay.projection_hash,
            "the full diagnostic carrier still records process generation"
        );
        assert!(!first.same_lifecycle(&other_view));
        assert_ne!(first.lifecycle_key, other_view.lifecycle_key);
    }

    #[test]
    fn aggregate_certificate_causal_roots_ignore_signer_carrier_replacement() {
        let (context, keys) = authenticated_runtime_context();
        let owner_tag = tag(0);
        let source_a = PeerId::new(keys[0].public_key().clone());
        let source_b = PeerId::new(keys[1].public_key().clone());
        let tagged_origin = |message: wire::ConsensusMessageV2, source: PeerId| {
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                &message,
                fair_runtime_ownership(&message, source.clone(), source),
            )
            .expect("fair ingress yields exact runtime ownership");
            let authenticated = AuthenticatedConsensusMessage::for_test(message);
            assert_eq!(
                authenticated.exact_runtime_command_identity(),
                AdapterCommand::Authenticated(authenticated.clone())
                    .exact_runtime_command_identity(),
                "the authenticated token and adapter wrapper share one exact identity"
            );
            TaggedCommand::with_ingress_ownership(
                owner_tag,
                CommandClass::Progress,
                authenticated,
                Instant::now(),
                ownership,
            )
            .causal_origin
        };

        let qc_a = signed_runtime_quorum_certificate(&context, &keys, 0xD1);
        let mut qc_b = qc_a.clone();
        qc_b.signers.rotate_left(1);
        qc_b.aggregate_signature = vec![0xB2; qc_b.aggregate_signature.len()];
        let qc_origin_a = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_a)),
            source_a.clone(),
        );
        let qc_origin_b = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_b)),
            source_b.clone(),
        );
        assert!(qc_origin_a.same_lifecycle(&qc_origin_b));

        let tc_a = signed_runtime_timeout_certificate(&context, &keys);
        let mut tc_b = tc_a.clone();
        tc_b.groups[0].signers.rotate_left(1);
        tc_b.groups[0].aggregate_signature = vec![0xC3; tc_b.groups[0].aggregate_signature.len()];
        let tc_message_a = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_a.clone()),
        );
        let tc_message_b = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_b),
        );
        let exact_tc_a = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_a.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        let exact_tc_b = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_b.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        assert_ne!(
            exact_tc_a, exact_tc_b,
            "deep command identity still distinguishes replaceable certificate carriers"
        );
        let tc_origin_a = tagged_origin(tc_message_a, source_a);
        let tc_origin_b = tagged_origin(tc_message_b, source_b.clone());
        assert!(tc_origin_a.same_lifecycle(&tc_origin_b));

        let mut other_round = tc_a;
        other_round.round.view = other_round.round.view.saturating_add(1);
        let other_round_origin = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                other_round,
            )),
            source_b,
        );
        assert!(
            !tc_origin_a.same_lifecycle(&other_round_origin),
            "transition-relevant certified round cannot collide with carrier normalization"
        );
    }

    #[test]
    fn class_cursor_advances_from_the_served_class_after_empty_classes() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        let queued = |class, value| {
            TaggedCommand::new(initial, class, FakeCommand::record(value), admitted_at)
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));

        ingress
            .enqueue(queued(CommandClass::Normal, 1))
            .expect("normal command fits the bounded ingress");
        let first = ingress.pop_next().expect("normal class is reachable");
        assert_eq!(first.command.record, Some(1));
        assert_eq!(ingress.next_class, CommandClass::Completion);

        ingress
            .enqueue(queued(CommandClass::Normal, 2))
            .expect("second normal command fits the bounded ingress");
        ingress
            .enqueue(queued(CommandClass::Completion, 3))
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
                for (class, ready) in [
                    (CommandClass::Normal, normal_ready),
                    (CommandClass::Progress, progress_ready),
                    (CommandClass::Completion, completion_ready),
                ] {
                    if ready {
                        ingress
                            .enqueue(TaggedCommand::new(
                                initial,
                                class,
                                FakeCommand::record(class.service_code()),
                                admitted_at,
                            ))
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
            RuntimeQueueConfig::new(8, 2, 2),
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
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let canonical = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"canonical chunk")],
            chunk_root: Hash::new(b"canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"conflicting chunk")],
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
                .enqueue(TaggedCommand::new(
                    command_tag,
                    CommandClass::Normal,
                    AdapterCommand::Authenticated(authenticated_proposal_for_test(manifest)),
                    Instant::now(),
                ))
                .expect("queue authenticated proposal");
        }

        ingress
            .enqueue_canonical_body_available(tag(3), canonical.clone())
            .expect("trusted completion prunes its conflicting proposal and appends in FIFO order");
        assert_eq!(ingress.len(), 3);
        assert!(
            ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(
                conflicting
            ))
        );
        assert!(
            !ingress
                .conflicts_with_pending_body_available(&authenticated_proposal_for_test(canonical))
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
        assert_eq!(committed.admission_ordinal, Some(3));
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
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new(b"reserved canonical chunk")],
            chunk_root: Hash::new(b"reserved canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"reserved conflicting chunk")],
            chunk_root: Hash::new(b"reserved conflicting root"),
            ..canonical.clone()
        };
        let canonical_proposal = authenticated_proposal_for_test(canonical.clone());
        let conflicting_proposal = authenticated_proposal_for_test(conflicting);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

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
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let original = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"retired chunk")],
            chunk_root: Hash::new(b"retired root"),
        };
        let replacement = wire::PayloadManifest {
            round: wire::ConsensusRound {
                view: round.view + 1,
                ..round
            },
            chunk_hashes: vec![Hash::new(b"replacement chunk")],
            chunk_root: Hash::new(b"replacement root"),
            ..original.clone()
        };
        let original_tag = tag(4);
        let replacement_tag = tag(5);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

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
        let execution_commitment = wire::ExecutionCommitment::without_topups(
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
        let authenticated = || {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload.clone()))
        };
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

        let collision_directory =
            TempDir::new().expect("temporary fair-collision runtime directory");
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
            collision_runtime.enqueue_network_with_ingress_ownership(
                conflicting_message,
                conflicting_ownership,
            ),
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
        let timeout_effects = runtime
            .driver
            .timeout_elapsed(round_tag)
            .expect("install a local signing fence")
            .into_effects();
        let (signature_tag, signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };

        let message = signed_runtime_proposal(&context, &keys, 0x78);
        let semantic_origin = PeerId::new(keys[0].public_key().clone());
        let ownership_a = fair_runtime_ownership(
            &message,
            semantic_origin.clone(),
            PeerId::new(keys[1].public_key().clone()),
        );
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
            .expect("first source enters runtime ingress");
        assert!(matches!(
            runtime.step(now),
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

        let ownership_b = fair_runtime_ownership(
            &message,
            semantic_origin,
            PeerId::new(keys[2].public_key().clone()),
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

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(signature_tag, signature)
            .expect("enqueue the exact signing completion");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects))
                if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
        ));
        assert!(runtime.take_last_scheduler_ownership().is_some());

        let deferred_effects = match runtime.step(now) {
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
        assert!(!runtime.fail_closed);
    }

