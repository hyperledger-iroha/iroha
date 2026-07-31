
    #[test]
    fn outer_ingress_batch_services_completions_and_runtime_before_every_ingress() {
        assert_eq!(
            outer_ingress_turns(3).collect::<Vec<_>>(),
            vec![
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
            ]
        );
        assert_eq!(
            outer_ingress_turns(0).collect::<Vec<_>>(),
            vec![
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
            ],
            "a zero-sized batch still owes completion and runtime service opportunities"
        );
    }

    #[test]
    fn terminal_ingress_discards_commit_discovery_and_losing_current_body_requests() {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let body = b"terminal ingress exact body".to_vec();
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"terminal ingress block")),
            payload_hash: Hash::new(&body),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"terminal ingress parent state"),
                Hash::new(b"terminal ingress post state"),
                Hash::new(b"terminal ingress writes"),
                1,
                Hash::new(b"terminal ingress executed block"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"terminal ingress commit request",
                )),
                certificate: certificate.clone(),
                responder: PeerId::new(keys[0].public_key().clone()),
                signature: vec![1],
            },
        );
        assert!(v2_payload_is_terminal_reducer_control(&response));

        let manifest = wire::PayloadManifest::derive(
            &context,
            round,
            subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            std::slice::from_ref(&body),
        )
        .expect("terminal body manifest");
        assert!(!v2_payload_is_terminal_reducer_control(
            &wire::ConsensusMessageV2Payload::PayloadManifest(manifest)
        ));

        let exact_request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: certificate.clone(),
            requester: PeerId::new(keys[1].public_key().clone()),
            signature: vec![1],
        };
        assert!(!certified_body_request_is_superseded_after_decision(
            &exact_request,
            Some(subject),
            context.height,
        ));

        let losing_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"losing terminal block")),
            ..subject
        };
        let mut losing_request = exact_request.clone();
        losing_request.subject = losing_subject;
        losing_request.certificate.subject = losing_subject;
        assert!(certified_body_request_is_superseded_after_decision(
            &losing_request,
            Some(subject),
            context.height,
        ));

        losing_request.round.height = context.height.saturating_sub(1);
        losing_request.certificate.round.height = losing_request.round.height;
        losing_request.certificate.proposal_round.height = losing_request.round.height;
        assert!(!certified_body_request_is_superseded_after_decision(
            &losing_request,
            Some(subject),
            context.height,
        ));
    }

    #[test]
    fn finalized_rollover_closes_ingress_before_successor_replay() {
        let ready = AtomicBool::new(true);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        close_ingress_for_rollover(&ready, &ingress);
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
    }

    #[test]
    fn synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff() {
        // This narrow rollover contract starts from a synthesized, internally
        // consistent Kura receipt/finality artifact. It does not exercise the
        // QC -> body recovery -> store -> validation -> application pipeline or
        // claim end-to-end catch-up coverage.
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let context = super::super::v2_worker::tests::production_output_handoff_with_dead_target();
        publish_applied_runner_status(&context);

        let predecessor = test_predecessor(&context, b"dead target rollover");
        let construction =
            PendingSuccessorConstruction::begin(predecessor).expect("begin successor handoff");
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure successor ingress");
        let mut successor_context = context.clone();
        successor_context.height = successor_context.height.saturating_add(1);
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let activation = construction
            .bind(test_successor_authority(
                predecessor,
                successor.height_context_id,
            ))
            .expect("bind exact predecessor authority");
        let output_guard = ConsensusOutputGuard::isolated();

        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("dead-target durable handoff permits successor activation");

        assert!(ready.load(Ordering::Acquire));
        let active = super::super::status::v2_status().expect("active successor status");
        assert_eq!(active.height, successor.height);
        assert_eq!(active.last_committed_height, context.height);
        assert!(matches!(
            active.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_activation_is_published_only_after_ingress_is_open() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();
        publish_applied_runner_status(&context);
        let predecessor = test_predecessor(&context, b"live ingress rollover");
        let construction =
            PendingSuccessorConstruction::begin(predecessor).expect("begin successor handoff");
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        let before = super::super::status::v2_status().expect("predecessor status");
        assert_eq!(before.height, context.height);
        assert_eq!(
            before.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            before
                .liveness
                .last_progress
                .expect("application marker")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied
        );
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
                Err(FairV2IngressPushError::Closed(_))
            ),
            "closed ingress must precede activation publication"
        );

        let mut successor_context = context.clone();
        successor_context.height += 1;
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let activation = construction
            .bind(test_successor_authority(
                predecessor,
                successor.height_context_id,
            ))
            .expect("bind exact predecessor authority");
        let output_guard = ConsensusOutputGuard::isolated();
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("open ingress and publish one activation");

        assert!(ready.load(Ordering::Acquire));
        ingress
            .try_push(InboundBlockMessage::new(valid_ingress_probe(), None))
            .expect("activation publication follows open ingress");
        let active = super::super::status::v2_status().expect("active successor status");
        assert_eq!(active.height, successor.height);
        let marker = active
            .liveness
            .last_progress
            .expect("successor activation marker");
        assert_eq!(
            marker.transition,
            wire::SumeragiV2ProgressTransition::SuccessorHeightActivated
        );
        assert_eq!(marker.generation, successor.liveness.generation);
        assert_eq!(marker.round.context_id, successor.height_context_id);
        assert_eq!(marker.round.height, successor.height);
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();

        publish_applied_runner_status(&context);
        let predecessor = test_predecessor(&context, b"foreign successor context");
        let construction = PendingSuccessorConstruction::begin(predecessor)
            .expect("begin mismatched-context handoff");
        let foreign_context_id =
            wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
                Hash::new(b"foreign successor context"),
            ));
        let activation = construction
            .bind(test_successor_authority(predecessor, foreign_context_id))
            .expect("bind the exact predecessor but foreign successor context");
        let rejected_ready = AtomicBool::new(false);
        let rejected_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        rejected_ingress
            .configure_roster(std::iter::empty())
            .expect("configure rejected test lane");
        assert!(
            open_ingress_for_active_height(
                output_guard.as_ref(),
                &rejected_ready,
                &rejected_ingress,
                Some((activation, successor)),
            )
            .is_err(),
            "an activation token cannot authorize another successor context"
        );
        assert!(!rejected_ready.load(Ordering::Acquire));
        assert!(
            matches!(
                rejected_ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
                Err(FairV2IngressPushError::Closed(_))
            ),
            "foreign-context rejection must close ingress again"
        );
        let predecessor = super::super::status::v2_status()
            .expect("foreign-context rejection retains the predecessor");
        assert_eq!(predecessor.height, context.height);
        assert_eq!(
            predecessor.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            predecessor
                .liveness
                .last_progress
                .expect("application remains authoritative")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn complete_tip_recovery_uses_the_same_live_successor_boundary() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (parent_context, _) = context();
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");

        let mut successor_context = parent_context.clone();
        successor_context.height += 1;
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = parent_context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let output_guard = ConsensusOutputGuard::isolated();
        let foreign_context_id =
            wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
                Hash::new(b"foreign recovered successor context"),
            ));
        let predecessor = test_predecessor(&parent_context, b"complete tip recovery");
        let foreign_activation = PendingSuccessorActivation::recovered(
            RecoveredSuccessorActivationAuthority::CompleteTip(test_successor_authority(
                predecessor,
                foreign_context_id,
            )),
        )
        .expect("authenticate complete-tip retry lifecycle");
        assert!(
            open_ingress_for_active_height(
                output_guard.as_ref(),
                &ready,
                &ingress,
                Some((foreign_activation, successor.clone())),
            )
            .is_err(),
            "recovery cannot authorize a same-height snapshot from another context"
        );
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            super::super::status::v2_status().is_none(),
            "rejected recovery must not publish a successor"
        );

        let activation = PendingSuccessorActivation::recovered(
            RecoveredSuccessorActivationAuthority::CompleteTip(test_successor_authority(
                predecessor,
                successor.height_context_id,
            )),
        )
        .expect("authenticate complete-tip retry lifecycle");
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("open recovered successor");

        assert!(ready.load(Ordering::Acquire));
        let active = super::super::status::v2_status().expect("recovered successor status");
        assert_eq!(active.height, successor.height);
        assert_eq!(active.last_committed_height, parent_context.height);
        assert!(matches!(
            active.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_construction_rejects_foreign_same_height_predecessor_authority() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();
        publish_applied_runner_status(&context);
        let expected = test_predecessor(&context, b"expected predecessor");
        let foreign = test_predecessor(&context, b"foreign same-height predecessor");
        assert_eq!(expected.height(), foreign.height());
        assert_ne!(expected, foreign);

        let construction =
            PendingSuccessorConstruction::begin(expected).expect("begin exact predecessor handoff");
        let mut successor_context = context.clone();
        successor_context.height += 1;
        let error = construction
            .bind(test_successor_authority(foreign, successor_context.id()))
            .expect_err("same-height foreign predecessor must not bind activation");
        assert!(matches!(
            error,
            V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                expected: actual_expected,
                actual,
            } if actual_expected == expected && actual == foreign
        ));
        let predecessor = super::super::status::v2_status().expect("predecessor remains visible");
        assert_eq!(
            predecessor.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_startup_failure_stays_running_and_fails_closed_without_activation() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, keys) = context();
        publish_applied_runner_status(&context);
        let activation = PendingSuccessorConstruction::begin(test_predecessor(
            &context,
            b"failed successor startup",
        ))
        .expect("begin successor handoff");
        let ready = Arc::new(AtomicBool::new(false));
        let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        let output_guard = ConsensusOutputGuard::isolated();

        // Force the real adapter constructor to fail on an existing directory
        // where it requires a WAL file. Runtime, service, and later startup
        // failures return through the same armed token/runner-guard boundary.
        let failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = super::super::v2::VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified constructor context");
        let directory = TempDir::new().expect("temporary directory");
        let constructor = SumeragiV2Adapter::open_deferred_status(
            directory.path(),
            verified,
            None,
            Generation::new(context.height),
            [0xA7; 32],
            AdapterFingerprints {
                node: Hash::new(b"failed constructor node"),
                build: Hash::new(b"failed constructor build"),
                config: Hash::new(b"failed constructor config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        );
        assert!(
            constructor.is_err(),
            "a directory cannot be opened as a WAL"
        );
        drop(activation);
        drop(failure_guard);

        assert!(output_guard.restart_required());
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
        let stalled = super::super::status::v2_status().expect("stalled predecessor status");
        assert_eq!(stalled.height, context.height);
        assert_eq!(
            stalled.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            stalled
                .liveness
                .last_progress
                .expect("application remains the final progress marker")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied,
            "dropping an incomplete activation token must not claim successor activation"
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn status_guard_retains_failure_snapshot_and_clears_clean_shutdown() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();

        let failure_status_guard = V2StatusClearGuard::new();
        publish_applied_runner_status(&context);
        super::super::status::mark_v2_restart_required();
        drop(failure_status_guard);
        let retained = super::super::status::v2_status().expect("retained failure snapshot");
        assert_eq!(retained.height, context.height);
        assert!(retained.restart_required);

        let mut clean_status_guard = V2StatusClearGuard::new();
        publish_applied_runner_status(&context);
        clean_status_guard.clear_on_drop();
        drop(clean_status_guard);
        assert!(super::super::status::v2_status().is_none());
    }

    #[test]
    fn ingress_capacity_error_preserves_message_and_byte_units() {
        let (context, _) = context();
        let validators = context
            .roster
            .iter()
            .take(2)
            .map(|validator| validator.validator.clone())
            .collect::<Vec<_>>();

        let count_error = FairV2Ingress::new(8, 3 * 1024, 1024, 0, 0)
            .configure_roster(validators.clone())
            .expect_err("two validators require ten protected message slots");
        assert!(matches!(
            ingress_capacity_error(count_error),
            V2RunnerError::IngressCapacity {
                configured: 8,
                required: 10,
            }
        ));

        let byte_error = FairV2Ingress::new(10, 2 * 1024, 1024, 0, 0)
            .configure_roster(validators)
            .expect_err("two validators and untrusted traffic require three byte partitions");
        assert!(matches!(
            ingress_capacity_error(byte_error),
            V2RunnerError::IngressByteCapacity {
                configured: 2048,
                required: 3072,
            }
        ));
    }

    #[test]
    fn ingress_guard_fails_closed_during_unwind() {
        let ready = Arc::new(AtomicBool::new(true));
        let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
            let ready = Arc::clone(&ready);
            let ingress = Arc::clone(&ingress);
            move || {
                let _guard = V2IngressClearGuard::new(Arc::clone(&ready), Arc::clone(&ingress));
                ingress.open().expect("reopen inside guarded runner");
                ready.store(true, Ordering::Release);
                panic!("model runner panic");
            }
        }));
        assert!(unwind.is_err());
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
    }

    #[test]
    fn runner_failure_guard_latches_restart_required_during_unwind() {
        let output_guard = ConsensusOutputGuard::isolated();
        let admitted_output = output_guard.acquire().expect("admit earlier output");
        let unwind = std::panic::catch_unwind({
            let output_guard = Arc::clone(&output_guard);
            move || {
                let _failure_guard = V2RunnerFailureGuard::new(output_guard);
                panic!("model runner panic before production services start");
            }
        });

        assert!(unwind.is_err(), "runner panic must continue unwinding");
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        drop(admitted_output);
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn clean_runner_completion_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
        failure_guard.disarm();
        drop(failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn prelatched_historical_serve_invokes_no_signer_cache_or_network() {
        let output_guard = ConsensusOutputGuard::isolated();
        output_guard.activate_restart_required();
        let signer_calls = Cell::new(0_u8);
        let cache_writes = Cell::new(0_u8);
        let network_posts = Cell::new(0_u8);

        let result = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || {
                signer_calls.set(signer_calls.get().saturating_add(1));
                cache_writes.set(cache_writes.get().saturating_add(1));
                Ok(Some(()))
            },
            |(), _permit| {
                network_posts.set(network_posts.get().saturating_add(1));
                Ok(())
            },
        );

        assert!(matches!(result, Err(V2BlockSyncError::RestartRequired)));
        assert_eq!(signer_calls.get(), 0);
        assert_eq!(cache_writes.get(), 0);
        assert_eq!(network_posts.get(), 0);
    }
