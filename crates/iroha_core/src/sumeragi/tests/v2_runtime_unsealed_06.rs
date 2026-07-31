
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
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"runtime parent state"),
            Hash::new(b"runtime post state"),
            Hash::new(b"runtime ordinary writes"),
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
        // Service the retransmission tick due at t=9, then the queued TC-like
        // progress command at the same monotonic instant.
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9));
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Idle)
        ));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(11));
        assert_eq!(runtime.driver.retransmits, vec![initial, next]);
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
