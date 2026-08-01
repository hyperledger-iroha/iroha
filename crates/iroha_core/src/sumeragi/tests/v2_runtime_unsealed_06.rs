
    #[test]
    fn unbound_direct_prepare_and_commit_votes_are_recoverable_after_validation() {
        for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
            let directory = TempDir::new().expect("temporary unbound-vote directory");
            let (mut runtime, context, keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let manifest = runtime_manifest(&context, 0xD7);
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(&manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable);
            let signed_vote = signed_runtime_vote(
                &keys,
                manifest.round,
                phase,
                manifest.subject,
                validated.execution_commitment(),
            );

            let far_future_round = wire::ConsensusRound {
                view: u64::MAX,
                ..manifest.round
            };
            let signed_far_future = signed_runtime_vote(
                &keys,
                far_future_round,
                phase,
                manifest.subject,
                validated.execution_commitment(),
            );
            assert!(
                runtime.can_admit_network_message(&signed_far_future),
                "a structurally valid far-future {phase:?} vote must drain without certified local view authority"
            );
            assert!(matches!(
                runtime.enqueue_network(signed_far_future),
                Err(NetworkIngressError::Authentication(
                    AdapterError::MissingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "rejecting a far-future unbound {phase:?} vote must not poison the runtime"
            );

            let mut malformed_future = signed_vote.clone();
            let wire::ConsensusMessageV2Payload::Vote(malformed_vote) =
                &mut malformed_future.payload
            else {
                unreachable!("fixture is a direct vote");
            };
            malformed_vote.round.view = u64::MAX;
            malformed_vote.proposal_round.view = u64::MAX;
            malformed_vote.signature.clear();
            assert!(
                runtime.can_admit_network_message(&malformed_future),
                "a structurally invalid far-future {phase:?} vote must drain for normal rejection"
            );
            assert!(matches!(
                runtime.enqueue_network(malformed_future),
                Err(NetworkIngressError::Authentication(_))
            ));
            assert_eq!(runtime.queued_commands(), 0);

            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "an early {phase:?} vote must remain fair-ingress owned until its proposal is validated"
            );
            // The mutating seam still rejects a caller that bypasses the
            // non-mutating fair-ingress gate.
            assert!(matches!(
                runtime.enqueue_network(signed_vote.clone()),
                Err(NetworkIngressError::Authentication(
                    AdapterError::MissingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "recoverable {phase:?} authentication rejection must not poison the runtime"
            );

            let proposer = context.leader(manifest.round.view);
            let mut proposal = wire::Proposal {
                round: manifest.round,
                proposer,
                subject: manifest.subject,
                manifest: manifest.clone(),
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: Vec::new(),
            };
            proposal.signature = Signature::new(
                keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
                &proposal.signature_preimage(),
            )
            .payload()
            .to_vec();
            runtime
                .enqueue_network(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(proposal),
                ))
                .expect("matching proposal establishes a pending body pipeline");
            assert_eq!(runtime.queued_commands(), 1);
            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "the {phase:?} vote remains a recoverable fair-ingress prerequisite while validation is pending"
            );
            runtime
                .arm_live_clocks(Instant::now())
                .expect("arm fixture clocks before dispatch");
            runtime
                .step_and_take_scheduler_ownership_for_test(Instant::now())
                .expect("dispatch matching proposal");
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "the registered manifest keeps the {phase:?} vote deferred while validation is pending"
            );
            assert!(!runtime.fail_closed);

            runtime
                .recover_validated_body(&manifest, &validated)
                .expect("local validation establishes canonical commitment authority");
            assert!(
                runtime.can_admit_network_message(&signed_vote),
                "the retained fair-ingress {phase:?} vote becomes drainable after validation"
            );

            let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"conflicting early vote parent state"),
                Hash::new(b"conflicting early vote post state"),
                Hash::new(b"conflicting early vote ordinary writes"),
                1,
                Hash::new(b"conflicting early vote executed block"),
            );
            assert_ne!(
                conflicting_commitment,
                validated.execution_commitment(),
                "the conflict fixture must differ from canonical validation"
            );
            let conflicting_vote = signed_runtime_vote(
                &keys,
                manifest.round,
                phase,
                manifest.subject,
                conflicting_commitment,
            );
            assert!(
                runtime.can_admit_network_message(&conflicting_vote),
                "a conflicting bound {phase:?} vote must drain for authenticated rejection"
            );
            assert!(matches!(
                runtime.enqueue_network(conflicting_vote),
                Err(NetworkIngressError::Authentication(
                    AdapterError::ConflictingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "conflicting {phase:?} vote rejection must not poison the runtime"
            );

            runtime
                .enqueue_network(signed_vote)
                .expect("the same signed canonical vote becomes admissible after validation");
            assert_eq!(runtime.queued_commands(), 1);
            assert!(!runtime.fail_closed);

            let stale_directory = TempDir::new().expect("temporary stale-vote directory");
            let (mut stale_runtime, stale_context, stale_keys) =
                authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
            let stale_manifest = runtime_manifest(&stale_context, 0xD9);
            let stale_durable = DurableBodyReceipt::for_test(
                stale_context.id(),
                stale_manifest.round,
                stale_manifest.subject,
                HashOf::new(&stale_manifest),
            );
            let stale_validated = ValidatedBodyReceipt::for_test(stale_durable);
            let stale_message = signed_runtime_vote(
                &stale_keys,
                stale_manifest.round,
                phase,
                stale_manifest.subject,
                stale_validated.execution_commitment(),
            );
            assert!(
                !stale_runtime.can_admit_network_message(&stale_message),
                "an unbound {phase:?} vote is retained while its view remains active"
            );
            let initial = stale_runtime.round_tag();
            let next = EventTag::new(
                initial.height(),
                initial.view() + 1,
                Generation::new(initial.generation().get() + 1),
            );
            observe_enter_view_for_test(&mut stale_runtime, initial, next, &stale_manifest);
            assert!(
                stale_runtime.can_admit_network_message(&stale_message),
                "view change releases an unmatched stale {phase:?} vote for bounded rejection"
            );
        }
    }

    #[test]
    fn exact_authenticated_network_retransmission_obeys_runtime_boundaries() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let original = signed_runtime_proposal(&context, &keys, 1);
        let second = signed_runtime_proposal(&context, &keys, 2);
        let third = signed_runtime_proposal(&context, &keys, 3);
        let transport = match &original.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadManifest(proposal.manifest.clone()),
            ),
            _ => unreachable!("fixture is a proposal"),
        };

        let owner_tag = runtime
            .enqueue_network(original.clone())
            .expect("first authenticated proposal owns one normal slot");
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces below the normal boundary"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);

        let mut invalid = third.clone();
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut invalid.payload else {
            unreachable!("fixture is a proposal")
        };
        proposal.signature[0] ^= 0x80;
        assert!(matches!(
            runtime.enqueue_network(invalid),
            Err(NetworkIngressError::Authentication(_))
        ));
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .enqueue_network(second.clone())
            .expect("non-identical authenticated proposal uses ordinary capacity");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces at reserved capacity"),
            owner_tag
        );
        assert!(matches!(
            runtime.enqueue_network(third.clone()),
            Err(NetworkIngressError::Backpressure(
                EnqueueError::ReservedCapacity
            ))
        ));

        let cursor_before = runtime.ingress.next_class;
        let tags_before = runtime
            .ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![4]),
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![5]),
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert!(runtime.can_admit_network_message(&original));
        assert!(!runtime.can_admit_network_message(&third));
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact authenticated duplicate coalesces at full capacity"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(runtime.ingress.next_class, cursor_before);
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .take(tags_before.len())
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before
        );
        assert!(matches!(
            runtime.enqueue_network(third),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));

        runtime.fail_closed = true;
        assert!(matches!(
            runtime.enqueue_network(original.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            runtime.enqueue_network(transport.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        runtime.fail_closed = false;
        assert!(matches!(
            runtime.enqueue_network(transport),
            Err(NetworkIngressError::TransportPayload)
        ));
    }

    #[test]
    fn commit_certificate_response_waits_for_embedded_qc_progress_capacity() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"response-capacity-block")),
            payload_hash: Hash::new(b"response-capacity-payload"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"response capacity parent state"),
                Hash::new(b"response capacity post state"),
                Hash::new(b"response capacity ordinary writes"),
                1,
                Hash::new(b"response capacity executed block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let response = |certificate| {
            wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"response capacity request",
                        )),
                        certificate,
                        responder: PeerId::new(keys[0].public_key().clone()),
                        signature: vec![1],
                    },
                ),
            )
        };
        let exact_response = response(certificate.clone());
        let mut distinct_certificate = certificate.clone();
        distinct_certificate.aggregate_signature = vec![2];
        let distinct_response = response(distinct_certificate);
        let owner_tag = runtime.round_tag();

        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![3]),
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![4]),
        );
        runtime
            .ingress
            .enqueue_authenticated(
                owner_tag,
                CommandClass::Progress,
                AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                )),
            )
            .expect("authenticated CommitQC fills the Progress prefix");
        assert_eq!(runtime.queued_commands(), 3);

        assert!(
            !runtime.can_admit_network_message(&distinct_response),
            "a distinct response remains in outer ingress while inner Progress is full"
        );
        assert!(
            runtime.can_admit_network_message(&exact_response),
            "an exact embedded CommitQC can coalesce with its queued owner"
        );

        let released = runtime
            .ingress
            .pop_next()
            .expect("release one shared-capacity owner");
        assert_eq!(released.class, CommandClass::Completion);
        assert!(
            runtime.can_admit_network_message(&distinct_response),
            "the retained response can drain after Progress capacity returns"
        );
    }

    #[test]
    fn commit_certificate_response_coalesces_with_exact_busy_deferred_qc() {
        let directory = TempDir::new().expect("temporary deferred-QC runtime directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
            Some(0),
        );
        let owner_tag = runtime.round_tag();
        let exact_certificate = signed_runtime_quorum_certificate(&context, &keys, 0xE1);
        let distinct_certificate = signed_runtime_quorum_certificate(&context, &keys, 0xE2);
        let exact_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(exact_certificate.clone()),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm the production runtime before dispatch");

        let timeout = runtime
            .driver
            .timeout_elapsed(owner_tag)
            .expect("open a signer fence before CommitQC dispatch");
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
        runtime
            .enqueue_network_with_ingress_ownership(
                exact_message.clone(),
                fair_network_ownership(&exact_message, PeerId::new(keys[0].public_key().clone())),
            )
            .expect("enqueue the authenticated CommitQC before the fence is observed");
        assert!(matches!(
            runtime
                .step(now)
                .expect("move the Busy CommitQC into adapter ownership"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&exact_message)
                .map(|(tag, _)| tag),
            Some(owner_tag)
        );
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&exact_certificate),
            Some(owner_tag),
            "the exact canonical QC retains its Busy-deferred owner"
        );
        let distinct_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(distinct_certificate.clone()),
        );
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&distinct_message)
                .map(|(tag, _)| tag),
            None
        );
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&distinct_certificate),
            None
        );
        let mut reordered_signers = exact_certificate.clone();
        reordered_signers.signers.reverse();
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&reordered_signers),
            None,
            "canonical signer order is part of the deferred QC identity"
        );
        let mut altered_aggregate = exact_certificate.clone();
        altered_aggregate.aggregate_signature.push(0xFF);
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&altered_aggregate),
            None,
            "the aggregate signature is part of the deferred QC identity"
        );
        let mut altered_proposal_round = exact_certificate.clone();
        altered_proposal_round.proposal_round.view =
            altered_proposal_round.proposal_round.view.saturating_add(1);
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&altered_proposal_round),
            None,
            "the proposal round is part of the deferred QC identity"
        );

        for signature in [vec![3], vec![4], vec![5]] {
            runtime
                .enqueue_signature(owner_tag, signature)
                .expect("completion traffic saturates the shared Progress prefix");
        }
        assert_eq!(runtime.queued_commands(), 3);

        let response = |certificate| {
            wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"deferred-QC coalescing request",
                        )),
                        certificate,
                        responder: PeerId::new(keys[0].public_key().clone()),
                        signature: vec![1],
                    },
                ),
            )
        };
        assert!(
            runtime.can_admit_network_message(&response(exact_certificate.clone())),
            "an exact response can reach authentication through its Busy-deferred owner"
        );
        assert!(
            !runtime.can_admit_network_message(&response(distinct_certificate.clone())),
            "a distinct response remains blocked while the Progress prefix is saturated"
        );

        let queued_before = runtime.queued_commands();
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    exact_message.clone(),
                    fair_network_ownership(
                        &exact_message,
                        PeerId::new(keys[1].public_key().clone()),
                    ),
                )
                .expect("an exact QC from another source coalesces with adapter ownership"),
            owner_tag
        );
        let exact_response = response(exact_certificate.clone());
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    exact_message.clone(),
                    fair_network_ownership(
                        &exact_response,
                        PeerId::new(keys[2].public_key().clone()),
                    ),
                )
                .expect("the authenticated discovery response coalesces with adapter ownership"),
            owner_tag
        );
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "authenticated coalescing must not create a runtime-queued duplicate"
        );
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(exact_certificate.clone(),),
                ))
                .map(|(tag, _)| tag),
            Some(owner_tag),
            "request completion leaves the sole Busy-deferred owner intact"
        );
        let retained = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy-deferred QC retains its ingress carriers");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);
        assert_eq!(retained.commit_certificate_response.len(), 1);
        assert!(!runtime.fail_closed);

        for _ in 2..MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
            let source = PeerId::from(KeyPair::random().public_key().clone());
            let candidate = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                &exact_message,
                fair_network_ownership(&exact_message, source),
            )
            .expect("independent Busy-deferred carrier is exact");
            runtime
                .deferred_ingress_ownership
                .values_mut()
                .next()
                .expect("the Busy-deferred QC retains its ingress carriers")
                .merge_downstream(candidate)
                .expect("every protocol-bounded Busy-deferred carrier remains exact");
        }
        let deferred_owner_before = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy-deferred carrier set is full")
            .clone();
        let excess_source = PeerId::from(KeyPair::random().public_key().clone());
        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(
                exact_message.clone(),
                fair_network_ownership(&exact_message, excess_source),
            ),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));
        assert_eq!(
            runtime
                .deferred_ingress_ownership
                .values()
                .next()
                .expect("backpressure preserves the full Busy-deferred carrier set"),
            &deferred_owner_before
        );
        assert!(!runtime.fail_closed);
        assert!(runtime.fail_closed_reason.is_none());
        assert!(matches!(
            runtime.enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(distinct_certificate),
            )),
            Err(NetworkIngressError::Backpressure(
                EnqueueError::ReservedCapacity
            ))
        ));
        assert_eq!(runtime.queued_commands(), queued_before);
    }

    #[test]
    fn exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot()
    {
        let directory = TempDir::new().expect("temporary multi-source TC directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
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
        assert!(matches!(
            timeout_effects.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));

        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ));
        let first_source = PeerId::new(keys[1].public_key().clone());
        let second_source = PeerId::new(keys[2].public_key().clone());
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, first_source),
                )
                .expect("the first authenticated TC carrier owns the runtime command"),
            round_tag
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, second_source),
                )
                .expect("the same TC from another source coalesces"),
            round_tag
        );
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one exact aggregate TC must retain every bounded source carrier"
        );
        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the coalesced TC retains exact ingress ownership");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);

        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("the Busy TC dispatch retains its exact runtime owner");
        assert!(selected.validate_exact().is_ok());
        let deferred = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy TC retains the coalesced source carriers");
        assert!(deferred.validate_exact());
        assert_eq!(deferred.direct.len(), 2);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn admitted_progress_runs_after_its_frozen_prefix_before_later_normal_churn() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 0..3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }
        for value in 100..140 {
            assert_eq!(
                enqueue_fake(
                    &mut runtime,
                    initial,
                    CommandClass::Normal,
                    FakeCommand::record(value)
                ),
                Err(EnqueueError::ReservedCapacity)
            );
        }
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(200),
        )
        .expect("CommitQC/progress reserve remains available");

        let initial_queue = runtime.queue_snapshot(start);
        assert_eq!(initial_queue.normal.depth, 3);
        assert_eq!(initial_queue.progress.depth, 1);

        for (expected, replacement) in [(0, 3), (1, 4), (2, 5)] {
            runtime
                .step_and_take_scheduler_ownership_for_test(start)
                .expect("one frozen normal predecessor drains");
            assert_eq!(runtime.driver.delivered.last(), Some(&(initial, expected)));
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(replacement),
            )
            .expect("later normal churn may refill only the vacated normal slot");
        }
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("the admitted progress owner runs after its finite frozen prefix");
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 0), (initial, 1), (initial, 2), (initial, 200)]
        );
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.capacity, 3);
        assert_eq!(queue.normal.max_service_debt, 1);
        assert_eq!(queue.progress.depth, 0);
        assert_eq!(queue.completion.depth, 0);
    }

    #[test]
    fn periodic_retransmit_cannot_starve_admitted_work_when_every_step_arrives_late() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 1..=2 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }

        for seconds in [2, 4, 6, 8] {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(seconds));
        }

        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);

        // Drain a periodic episode and the one-shot timeout before admitting
        // a new target. Every later runner entry is again exactly one whole
        // retransmit interval late. The drained timer's dormant semantic key
        // must not resurrect its old physical ordinal on each entry.
        let mut post_timeout = self::runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
            .expect("drain the first periodic episode");
        assert_eq!(post_timeout.driver.retransmits, vec![initial]);
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("emit the one-shot absolute timeout");
        assert_eq!(post_timeout.driver.timeouts, vec![initial]);
        enqueue_fake(
            &mut post_timeout,
            initial,
            CommandClass::Normal,
            FakeCommand::record(9),
        )
        .expect("admit work after the old periodic owner drained");

        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(12))
            .expect("the admitted target precedes the fresh periodic episode");
        assert_eq!(post_timeout.driver.delivered, vec![(initial, 9)]);
        assert_eq!(
            post_timeout.driver.retransmits,
            vec![initial],
            "a drained timer cannot reacquire its old position ahead of the target"
        );
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(14))
            .expect("the freshly positioned periodic episode follows the target");
        assert_eq!(post_timeout.driver.retransmits, vec![initial, initial]);
    }

    #[test]
    fn frozen_lifecycle_order_precedes_timeout_priority() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(7),
        )
        .unwrap();

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
            .expect("the older admitted FIFO lifecycle dispatches first");
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.retransmits.is_empty());
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(10))
            .expect("the earlier frozen periodic lifecycle dispatches next");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
        assert_eq!(runtime.driver.retransmits, vec![initial]);
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(12))
            .expect("the later absolute-timeout lifecycle dispatches last");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("absolute timeout publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Timeout
        );
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert_eq!(
            runtime.driver.retransmits,
            vec![initial],
            "the absolute deadline cannot replenish the drained periodic owner"
        );
    }

    #[test]
    fn due_timeout_becomes_older_than_replenished_exact_serve_tickets() {
        let start = Instant::now();
        let initial = tag(0);
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with the shared Serve source")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm shared-source runtime");

        let first_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve first exact Serve occurrence");
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    first_barrier,
                )
                .expect("first barrier freezes the due timeout"),
            "a clock first frozen behind this ticket cannot overtake it"
        );

        let second_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve a distinct retransmission occurrence");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    second_barrier,
                )
                .expect("replenished barrier validates against the same source"),
            "the frozen timeout must predate every later exact ticket"
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("one bounded predecessor episode dispatches the timeout");
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn restored_serve_high_watermark_precedes_startup_runtime_owner() {
        let start = Instant::now();
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(41);
        let (mut runtime, startup) = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(tag(0)),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            vec![FakeEffect::other()],
            lifecycle_ordinals.clone(),
        )
        .expect("construct restarted runtime after durable Serve waiter");
        let ownership = runtime
            .take_effect_ownership(startup.len())
            .expect("startup owner retains exact lifecycle sidecar");
        assert_eq!(ownership.len(), 1);
        assert_eq!(ownership[0].owner().lifecycle_ordinal(), 42);
        assert_eq!(
            lifecycle_ordinals
                .reserve_one()
                .expect("later exact Serve ticket follows startup recovery"),
            43
        );
    }

    #[test]
    fn full_runtime_churn_cannot_cross_an_exact_serve_ordinal() {
        let start = Instant::now();
        let initial = tag(0);
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with shared admission order")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm shared-source runtime");
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("admit the frozen predecessor");
        let barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve exact Serve position");
        for value in 2..=3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("fill only the later normal prefix");
        }

        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(start, barrier)
                .expect("compare the full runtime prefix")
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("one bounded predecessor transition runs");
        assert_eq!(runtime.driver.delivered, vec![(initial, 1)]);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(start, barrier)
                .expect("later churn remains behind the exact ticket")
        );
    }

    #[test]
    fn network_admission_uses_exact_normal_and_progress_reservations() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"runtime-test-context",
            ))),
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime-test-block")),
            payload_hash: Hash::new(b"runtime-test-payload"),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"runtime parent state"),
            Hash::new(b"runtime post state"),
            Hash::new(b"runtime ordinary writes"),
            1,
            Hash::new(b"runtime executed block wire"),
        );
        let vote = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: vec![1],
        });
        let locked_commit_vote = match &vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.phase = wire::GlobalPhase::Commit;
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        runtime.driver.protected_commit = Some((round, subject, execution_commitment));
        let mismatched_commit_vote = match &locked_commit_vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.subject.payload_hash = Hash::new(b"mismatched runtime commit vote");
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let commit_qc = wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone());
        let timeout_vote = wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![1],
        });
        let commit_response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime commit request")),
                certificate,
                responder: PeerId::new(KeyPair::random().public_key().clone()),
                signature: vec![1],
            },
        );
        assert_eq!(network_command_class(&vote), Some(CommandClass::Normal));
        assert_eq!(
            network_command_class(&commit_qc),
            Some(CommandClass::Progress)
        );
        assert_eq!(
            network_command_class(&timeout_vote),
            Some(CommandClass::Progress),
            "authenticated TimeoutVote traffic owns the protected progress prefix"
        );
        assert_eq!(network_command_class(&commit_response), None);
        assert_eq!(
            network_admission_class(&commit_response),
            Some(CommandClass::Progress)
        );
        assert!(runtime.can_admit_network_payload(&vote));
        assert!(runtime.can_admit_network_payload(&commit_qc));
        assert!(runtime.can_admit_network_payload(&timeout_vote));
        assert!(runtime.can_admit_network_payload(&commit_response));

        for value in [1, 2] {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("fill the normal prefix");
        }
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(
            !runtime.can_admit_network_payload(&mismatched_commit_vote),
            "a merely Commit-shaped vote must stop at pre-authentication backpressure"
        );
        assert!(
            runtime.can_admit_network_payload(&locked_commit_vote),
            "the exact locked Commit vote can reach authentication through the progress reserve"
        );
        assert!(
            runtime.can_admit_network_payload(&commit_qc),
            "CommitQC can use the reserved progress slot"
        );
        assert!(
            runtime.can_admit_network_payload(&timeout_vote),
            "TimeoutVote can use the reserved progress slot"
        );
        assert!(runtime.can_admit_network_payload(&commit_response));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("fill the progress prefix");
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(!runtime.can_admit_network_payload(&mismatched_commit_vote));
        assert!(!runtime.can_admit_network_payload(&locked_commit_vote));
        assert!(!runtime.can_admit_network_payload(&commit_qc));
        assert!(!runtime.can_admit_network_payload(&timeout_vote));
        assert!(!runtime.can_admit_network_payload(&commit_response));

        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
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
            chunk_hashes: vec![Hash::new([0_u8])],
            chunk_root: Hash::new(b"runtime transport root"),
        });
        assert!(runtime.can_admit_network_payload(&transport));
    }

    #[test]
    fn stale_completion_retains_tag_and_precedes_a_later_due_retransmit() {
        let start = Instant::now();
        let current = tag(4);
        let stale = tag(2);
        let mut runtime = runtime(
            FakeDriver::new(current),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            stale,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .unwrap();
        runtime
            .step(start + Duration::from_secs(2))
            .expect("the older admitted completion owns the first turn");
        assert_eq!(runtime.driver.delivered, vec![(stale, 9)]);
        assert!(runtime.driver.retransmits.is_empty());
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the completion publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        runtime
            .take_effect_ownership(1)
            .expect("consume the completion effect owner before the next turn");

        // The retransmit lifecycle was frozen when it first became due, so it
        // owns the next turn after the older completion drains.
        runtime
            .step(start + Duration::from_secs(4))
            .expect("the frozen retransmit owns the next turn");
        assert_eq!(runtime.driver.retransmits, vec![current]);
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
    }

    #[test]
    fn only_enter_view_effect_restarts_both_clocks() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1));
        assert_eq!(runtime.round_tag(), initial);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::enter_view(next),
        )
        .unwrap();
        // The TC-like progress command was admitted before the next runner
        // freeze, so it precedes the newly positioned old-view retransmit
        // episode. EnterView then resets both clocks and retires that stale
        // periodic owner before it can dispatch.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.round_tag(), next);
        assert!(runtime.driver.retransmits.is_empty());
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
            Ok(RuntimeStep::Idle)
        ));
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Idle)
        ));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(11));
        assert_eq!(runtime.driver.retransmits, vec![next]);
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(19));
        assert!(runtime.driver.timeouts.is_empty());
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(29));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn startup_enter_view_effect_restarts_clocks_and_is_returned_unchanged() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let (mut runtime, effects) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![FakeEffect::enter_view(next), FakeEffect::other()],
        )
        .unwrap();
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(
            effects,
            vec![FakeEffect::enter_view(next), FakeEffect::other()]
        );
        assert!(matches!(
            runtime.step(start + Duration::from_secs(100)),
            Err(RuntimeError::ClocksNotArmed)
        ));
        runtime
            .arm_live_clocks(start + Duration::from_secs(100))
            .expect("arm after startup effects are dispatched");
        assert_eq!(
            runtime.arm_live_clocks(start + Duration::from_secs(101)),
            Err(RuntimeClockError::AlreadyArmed)
        );
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(119)),
            Ok(RuntimeStep::Advanced(_)) | Ok(RuntimeStep::Idle)
        ));
        assert!(runtime.driver.timeouts.is_empty());
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(120));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn interrupted_tip_recovery_drains_ingress_without_arming_live_timers() {
        let start = Instant::now();
        let initial = tag(0);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("queue local recovery completion");

        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(
                start + Duration::from_secs(1_000)
            ),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());
        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(
                start + Duration::from_secs(2_000)
            ),
            Ok(RuntimeStep::Idle)
        ));
    }

    #[test]
    fn interrupted_tip_recovery_is_rejected_after_live_clock_arm() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        assert!(matches!(
            runtime.step_recovery(start),
            Err(RuntimeError::RecoveryAfterClocksArmed)
        ));
    }

    #[test]
    fn adapter_failure_closes_runtime_permanently() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::fail(),
        )
        .unwrap();
        assert!(matches!(
            runtime.step(start),
            Err(RuntimeError::Driver(FakeError))
        ));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("runtime driver rejected a serialized transition: fake driver failure")
        );
        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("runtime driver rejected a serialized transition: fake driver failure"),
            "the generic closed guard cannot replace the driver root cause"
        );
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        let start = Instant::now();
        let initial = tag(0);
        let result = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::ZERO,
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::<FakeEffect>::new(),
        );
        assert!(matches!(
            result,
            Err(RuntimeConfigError::InvalidRoundTimeout)
        ));

        let invalid_queue = RuntimeQueueConfig::new(2, 1, 1).validate();
        assert_eq!(
            invalid_queue,
            Err(RuntimeConfigError::InvalidQueueAllocation)
        );
    }
