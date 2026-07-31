
    #[test]
    fn applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation() {
        const PHASE_INVENTORY: [&str; 1] = ["local_proposal_ready"];

        let directory = TempDir::new().expect("temporary local-proposal phase directory");
        let (fixture_context, _) = authenticated_runtime_context();
        let leader = fixture_context.leader(0);
        let (mut runtime, context, _keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(leader),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for local proposal dispatch");
        let tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9C);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        runtime
            .enqueue_local_proposal(tag, manifest.clone(), durable.clone(), validated.clone())
            .expect("enqueue exact local proposal completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("persist the exact proposal intent"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
        ));

        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_local_proposal(tag, manifest, durable, validated)
            .expect("the durable proposal intent suppresses its exact callback retry");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert_eq!(["local_proposal_ready"], PHASE_INVENTORY);
    }

    #[test]
    fn drained_internal_ignore_uses_exact_durable_tombstone_before_readmission() {
        const PHASE_INVENTORY: [&str; 2] = ["terminal_ignore", "restart_tombstone"];

        let directory = TempDir::new().expect("temporary runtime tombstone directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9D);
        let ordinal_before_first = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("the first ownerless completion reaches its terminal reducer discard");
        assert_eq!(runtime.queued_commands(), 1);
        assert_ne!(runtime.ingress.next_admission_ordinal, ordinal_before_first);
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(Instant::now())
                .expect("drain the first ownerless completion"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));

        let next_ordinal = runtime.ingress.next_admission_ordinal;
        for _ in 0..3 {
            runtime
                .enqueue_body_available(tag, manifest.clone())
                .expect("the exact terminal lifecycle coalesces in-process");
        }
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        let mut suppressed_phases = vec!["terminal_ignore"];
        drop(runtime);

        let (mut restarted, restarted_context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        assert_eq!(restarted_context.id(), context.id());
        let restarted_tag = restarted.round_tag();
        let next_ordinal = restarted.ingress.next_admission_ordinal;
        for _ in 0..3 {
            restarted
                .enqueue_body_available(restarted_tag, manifest.clone())
                .expect("the exact terminal lifecycle coalesces after restart");
        }
        assert_eq!(restarted.queued_commands(), 0);
        assert_eq!(restarted.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("restart_tombstone");
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
    }

    #[test]
    fn stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal() {
        let stale_directory = TempDir::new().expect("temporary stale internal-callback directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
        let current = runtime.round_tag();
        let stale = EventTag::new(
            current.height(),
            current.view(),
            Generation::new(current.generation().get().saturating_sub(1)),
        );
        let manifest = runtime_manifest(&context, 0x9E);
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(stale, manifest.clone())
            .expect("valid stale internal callback is discarded before admission");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        drop(runtime);

        let (mut restarted, restarted_context, _keys) =
            authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
        assert_eq!(restarted_context.id(), context.id());
        let next_ordinal = restarted.ingress.next_admission_ordinal;
        restarted
            .enqueue_body_available(restarted.round_tag(), manifest)
            .expect("stale discard did not create a current-incarnation tombstone");
        assert_eq!(restarted.queued_commands(), 1);
        assert_ne!(restarted.ingress.next_admission_ordinal, next_ordinal);

        let malformed_directory =
            TempDir::new().expect("temporary malformed internal-callback directory");
        let (mut malformed_runtime, malformed_context, _keys) =
            authenticated_network_runtime(&malformed_directory, RuntimeQueueConfig::new(8, 1, 1));
        let mut malformed_manifest = runtime_manifest(&malformed_context, 0x9F);
        let mut foreign_context = malformed_context.clone();
        foreign_context.chain_id = "foreign-runtime-preflight".into();
        malformed_manifest.round.context_id = foreign_context.id();
        let next_ordinal = malformed_runtime.ingress.next_admission_ordinal;
        assert_eq!(
            malformed_runtime
                .enqueue_body_available(malformed_runtime.round_tag(), malformed_manifest),
            Err(EnqueueError::FailClosed)
        );
        assert_eq!(malformed_runtime.queued_commands(), 0);
        assert_eq!(
            malformed_runtime.ingress.next_admission_ordinal,
            next_ordinal
        );
        assert!(malformed_runtime.fail_closed);
    }

    #[test]
    fn body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates() {
        let directory = TempDir::new().expect("temporary body-pipeline retirement directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };
        let three_stages = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 1,
            local_proposal: 1,
        };
        let validation_only = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 0,
            validation: 1,
            local_proposal: 0,
        };

        let ingress_manifest = runtime_manifest(&context, 0xA1);
        let (durable, validated) = receipts(&ingress_manifest);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: durable.clone(),
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: validated.clone(),
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: ingress_manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_manifest.round,
                    ingress_manifest.subject,
                )
                .expect("retire ingress body pipeline"),
            three_stages
        );

        let ingress_failure_manifest = runtime_manifest(&context, 0xA2);
        runtime
            .enqueue_validation_failed(
                owner_tag,
                ingress_failure_manifest.round,
                ingress_failure_manifest.subject,
            )
            .expect("enqueue ingress validation-failure owner");
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_failure_manifest.round,
                    ingress_failure_manifest.subject,
                )
                .expect("retire ingress validation failure"),
            validation_only
        );

        let deferred_manifest = runtime_manifest(&context, 0xB1);
        for stage in [
            DeferredBodyPipelineStageForTest::BodyStored,
            DeferredBodyPipelineStageForTest::ValidationSucceeded,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        ] {
            runtime
                .driver
                .defer_body_pipeline_stage_for_test(owner_tag, &deferred_manifest, stage)
                .expect("stage Busy-deferred body completion");
        }
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_manifest.round,
                    deferred_manifest.subject,
                )
                .expect("retire Busy-deferred body pipeline"),
            three_stages
        );

        let deferred_failure_manifest = runtime_manifest(&context, 0xB2);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_failure_manifest,
                DeferredBodyPipelineStageForTest::ValidationFailed,
            )
            .expect("stage Busy-deferred validation failure");
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_failure_manifest.round,
                    deferred_failure_manifest.subject,
                )
                .expect("retire Busy-deferred validation failure"),
            validation_only
        );

        let duplicate_body_stored = runtime_manifest(&context, 0xC1);
        let (durable, _) = receipts(&duplicate_body_stored);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: duplicate_body_stored.round,
                subject: duplicate_body_stored.subject,
                receipt: durable,
            },
        );
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &duplicate_body_stored,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage duplicate deferred BodyStored owner");
        let stored_only = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 0,
            local_proposal: 0,
        };
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime.ingress.body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only
        );
        assert_eq!(
            runtime.driver.deferred_body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    duplicate_body_stored.round,
                    duplicate_body_stored.subject,
                )
                .expect_err("duplicate BodyStored ownership must fail"),
            "Sumeragi v2 body pipeline has duplicate exact serialized completion stages"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime.ingress.body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only,
            "preflight must retain the ingress owner"
        );
        assert_eq!(
            runtime.driver.deferred_body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only,
            "preflight must retain the Busy-deferred owner"
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    duplicate_body_stored.round,
                    duplicate_body_stored.subject,
                )
                .expect_err("fail-closed runtime must reject a second pipeline retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, duplicate_body_stored.subject,),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    #[test]
    fn decision_retirement_releases_queued_leader_wire_runtime_owner() {
        let directory = TempDir::new().expect("temporary leader-wire Decision directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let fixture = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC1,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
            unreachable!("leader-wire fixture carries Proposal")
        };
        runtime
            .enqueue_network_with_ingress_ownership(
                fixture.message.clone(),
                fixture.ownership.clone(),
            )
            .expect("enqueue proposal with durable leader-wire runtime ownership");
        let ordinal = fixture.receipt.owner().admission_ordinal();
        assert_eq!(
            runtime.leader_wire_runtime_receipts.get(&ordinal),
            Some(&fixture.receipt)
        );

        let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"leader-wire Decision state root"),
            Hash::new(b"leader-wire Decision event root"),
            Hash::new(b"leader-wire Decision reject root"),
            1,
            Hash::new(b"leader-wire Decision fee root"),
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(proposal.round, proposal.subject, commitment,)
                .expect("Decision retires queued proposal ownership"),
            DecisionProposalRetirement::default()
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
        let terminals = runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
            panic!("Decision retirement must emit one volatile leader-wire terminal")
        };
        assert_volatile_leader_wire_release(&fixture, receipt);
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime after consuming Decision terminal");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn lock_retirement_releases_busy_deferred_leader_wire_runtime_owner() {
        let directory = TempDir::new().expect("temporary leader-wire lock directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let fixture = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC2,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
            unreachable!("leader-wire fixture carries Proposal")
        };
        let ingress_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &fixture.message,
            fixture.ownership.clone(),
        )
        .expect("project exact leader-wire ownership into runtime");
        let tagged = TaggedCommand::with_ingress_ownership(
            runtime.round_tag(),
            CommandClass::Normal,
            AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
                fixture.message.clone(),
            )),
            Instant::now(),
            ingress_ownership.clone(),
        );
        let lifecycle_ordinal = tagged
            .lifecycle_ordinal
            .expect("leader-wire command carries its scheduler ordinal");
        let lifecycle_owner =
            RuntimeLifecycleOwner::new(tagged.causal_origin.clone(), lifecycle_ordinal)
                .expect("construct exact deferred lifecycle owner");
        let owner_tag = runtime.round_tag();
        runtime
            .driver
            .defer_authenticated_proposal_for_test(owner_tag, proposal)
            .expect("stage Busy-deferred proposal");
        let (_, deferred_ordinal) = runtime
            .driver
            .deferred_authenticated_message_owner(&fixture.message)
            .expect("deferred proposal exposes its adapter ordinal");
        assert!(
            runtime
                .deferred_ingress_ownership
                .insert(deferred_ordinal, ingress_ownership.clone())
                .is_none()
        );
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(deferred_ordinal, lifecycle_owner)
                .is_none()
        );
        runtime
            .register_leader_wire_runtime_receipt(&ingress_ownership)
            .expect("register deferred leader-wire receipt");
        let ordinal = fixture.receipt.owner().admission_ordinal();
        assert_eq!(
            runtime.leader_wire_runtime_receipts.get(&ordinal),
            Some(&fixture.receipt)
        );

        let locked_subject = runtime_manifest(&context, 0xC3).subject;
        assert_ne!(locked_subject, proposal.subject);
        assert_eq!(
            runtime
                .retire_unsafe_proposals_for_lock(proposal.round, locked_subject)
                .expect("lock retires unsafe Busy-deferred proposal"),
            1
        );
        assert!(
            runtime
                .driver
                .authenticated_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
        let terminals = runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
            panic!("lock retirement must emit one volatile leader-wire terminal")
        };
        assert_volatile_leader_wire_release(&fixture, receipt);
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime after consuming lock terminal");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn production_authenticated_preflight_is_never_semantic_only_coalesce() {
        let directory = TempDir::new().expect("temporary authenticated-preflight directory");
        let (runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let message = signed_runtime_proposal(&context, &keys, 0xC4);
        let authenticated = runtime
            .driver
            .authenticate(message)
            .expect("authenticate the production Proposal command");
        let command = AdapterCommand::Authenticated(authenticated);

        assert_eq!(
            runtime
                .driver
                .preflight_runtime_command_admission(runtime.round_tag(), &command),
            RuntimeCommandAdmissionPreflight::Admit
        );
    }

    #[test]
    fn semantic_only_authenticated_coalesce_fails_before_receipt_registration() {
        let directory = TempDir::new().expect("temporary coalesce-defense directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let existing = signed_runtime_proposal(&context, &keys, 0xC5);
        runtime
            .enqueue_network(existing)
            .expect("retain an existing authenticated semantic owner");
        let queued_before = runtime.queued_commands();

        let candidate = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC6,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let candidate_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &candidate.message,
            candidate.ownership.clone(),
        )
        .expect("project the fresh leader-wire runtime receipt");
        assert!(
            candidate_ownership
                .leader_wire_runtime_receipt()
                .expect("inspect exact candidate receipt")
                .is_some()
        );
        assert!(runtime.leader_wire_runtime_receipts.is_empty());

        assert!(matches!(
            runtime.reject_authenticated_preflight_coalescence(
                RuntimeCommandAdmissionPreflight::Coalesce,
            ),
            Err(NetworkIngressError::FailClosed)
        ));
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "defensive rejection must not delete the existing semantic owner"
        );
        assert!(
            runtime.leader_wire_runtime_receipts.is_empty(),
            "semantic-only coalescence cannot register an ownerless runtime receipt"
        );
        assert!(runtime.pending_leader_wire_terminals.is_empty());
        assert!(runtime.fail_closed);
    }

    #[test]
    fn decision_retires_proposal_owners_but_preserves_body_and_application_completions() {
        let directory = TempDir::new().expect("temporary decision-retirement directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(12, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };

        let decision_manifest = runtime_manifest(&context, 0xD0);
        let (decision_durable, decision_validated) = receipts(&decision_manifest);
        let decision_commitment = decision_validated.execution_commitment();
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xD1))
            .expect("enqueue authenticated proposal at decided height");
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: decision_manifest.clone(),
                durable_receipt: decision_durable.clone(),
                validated_receipt: decision_validated,
            },
        );
        let other_local_manifest = runtime_manifest(&context, 0xD2);
        let (other_durable, other_validated) = receipts(&other_local_manifest);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: other_local_manifest.clone(),
                durable_receipt: other_durable,
                validated_receipt: other_validated,
            },
        );
        runtime
            .enqueue_body_available(owner_tag, decision_manifest.clone())
            .expect("enqueue body-recovery completion");
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: decision_manifest.round,
                subject: decision_manifest.subject,
                receipt: decision_durable,
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::ApplicationCompleted(decision_manifest.subject),
        );

        let deferred_proposal = match signed_runtime_proposal(&context, &keys, 0xD3).payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal,
            _ => unreachable!("fixture is a proposal"),
        };
        runtime
            .driver
            .defer_authenticated_proposal_for_test(owner_tag, &deferred_proposal)
            .expect("stage Busy-deferred authenticated proposal");
        let deferred_local_manifest = runtime_manifest(&context, 0xD4);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_local_manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage Busy-deferred LocalProposalReady");
        let deferred_body_manifest = runtime_manifest(&context, 0xD5);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_body_manifest,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage Busy-deferred body-store completion");
        assert_eq!(
            runtime
                .driver
                .status()
                .expect("status before decision retirement")
                .liveness
                .work
                .candidate,
            wire::SumeragiV2LocalWorkStage::Complete
        );

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    decision_manifest.round,
                    decision_manifest.subject,
                    decision_commitment,
                )
                .expect("retire proposal work after decision"),
            DecisionProposalRetirement::new(Some(owner_tag), 0),
            "the exact current-tag LocalProposalReady owner must remain queued"
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert!(runtime.ingress.commands.iter().all(|queued| !matches!(
            &queued.command,
            AdapterCommand::Authenticated(authenticated)
                if matches!(
                    authenticated.payload(),
                    wire::ConsensusMessageV2Payload::Proposal(_)
                )
        )));
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::LocalProposalReady { manifest, .. }
                if manifest == &decision_manifest
        )));
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::BodyAvailable { .. }))
        );
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::BodyStored { .. }))
        );
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::ApplicationCompleted(_)))
        );
        assert_eq!(
            runtime
                .driver
                .status()
                .expect("status after decision retirement")
                .liveness
                .work
                .candidate,
            wire::SumeragiV2LocalWorkStage::Idle,
            "decision retirement clears stale active proposal state"
        );
        let deferred_local_commitment = receipts(&deferred_local_manifest).1.execution_commitment();
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    deferred_local_manifest.round,
                    deferred_local_manifest.subject,
                    deferred_local_commitment,
                )
                .merge(runtime.driver.deferred_decided_local_proposal_counts(
                    owner_tag,
                    deferred_local_manifest.round,
                    deferred_local_manifest.subject,
                    deferred_local_commitment,
                )),
            DecisionLocalProposalCounts::default(),
            "all nonmatching local proposal completions were retired"
        );

        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    decision_manifest.round,
                    decision_manifest.subject,
                )
                .expect("body recovery remains queued after decision"),
            RetiredBodyPipelineCompletions {
                body_available: 1,
                body_stored: 1,
                validation: 0,
                local_proposal: 1,
            }
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_body_manifest.round,
                    deferred_body_manifest.subject,
                )
                .expect("Busy-deferred body store remains queued after decision"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front().map(|queued| &queued.command),
            Some(AdapterCommand::ApplicationCompleted(subject))
                if *subject == decision_manifest.subject
        ));

        let duplicate_manifest = runtime_manifest(&context, 0xD6);
        let (duplicate_durable, duplicate_validated) = receipts(&duplicate_manifest);
        let duplicate_commitment = duplicate_validated.execution_commitment();
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: duplicate_manifest.clone(),
                durable_receipt: duplicate_durable,
                validated_receipt: duplicate_validated,
            },
        );
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &duplicate_manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage duplicate exact local completion in Busy-deferred lane");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .expect_err("duplicate exact local completion ownership must fail"),
            "Sumeragi v2 decided local proposal completion has duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            2,
            "preflight must retain the application and ingress proposal owners"
        );
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
            "preflight must retain the Busy-deferred proposal owner"
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .expect_err("fail-closed runtime must reject a second proposal retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_signature(owner_tag, vec![0xD6]),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    #[test]
    fn decision_retires_stale_local_completion_for_durable_recovery() {
        let directory = TempDir::new().expect("temporary stale-decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let stale_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD7);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let commitment = validated.execution_commitment();
        stage_completion_for_queue_test(
            &mut runtime,
            stale_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

        runtime.round_tag = EventTag::new(
            stale_tag.height(),
            stale_tag.view().saturating_add(1),
            Generation::new(stale_tag.generation().get().saturating_add(1)),
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("retire stale exact completion after certified view change"),
            DecisionProposalRetirement::new(None, 1)
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.fail_closed);
        runtime
            .enqueue_body_available(runtime.round_tag(), manifest)
            .expect("durable reconstruction can claim the current reducer tag");
    }

    #[test]
    fn progress_cursor_decision_preserves_outer_ingress_completion_until_apply() {
        const PHASE_INVENTORY: [&str; 2] =
            ["decided_local_proposal_ready", "application_completed"];

        let directory = TempDir::new().expect("temporary Decision-race directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD9);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let commitment = validated.execution_commitment();
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable.clone(),
                validated_receipt: validated.clone(),
            },
        );
        runtime
            .enqueue_local_proposal(
                owner_tag,
                manifest.clone(),
                durable.clone(),
                validated.clone(),
            )
            .expect("an exact trusted retry coalesces with its existing owner");
        assert_eq!(runtime.queued_commands(), 1);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: manifest.subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD9; 96],
        };
        runtime
            .ingress
            .enqueue_authenticated(
                owner_tag,
                CommandClass::Progress,
                AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
                )),
            )
            .expect("enqueue the CommitQC progress item");
        runtime.ingress.next_class = CommandClass::Progress;
        let now = Instant::now();
        runtime.arm_live_clocks(now).expect("arm runtime clocks");

        let RuntimeStep::Advanced(decision_effects) = runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("Progress cursor installs Decision")
        else {
            panic!("queued CommitQC must advance the reducer")
        };
        assert!(matches!(
            decision_effects.as_slice(),
            [AdapterEffect::FetchBody {
                subject,
                certificate: Some(certificate),
                ..
            }] if *subject == manifest.subject && certificate == &decision
        ));
        assert_eq!(runtime.queued_commands(), 1);

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("Decision cleanup preserves the exact completion"),
            DecisionProposalRetirement::new(Some(owner_tag), 0)
        );
        let RuntimeStep::Advanced(completion_effects) = runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("fair completion service reaches the reducer")
        else {
            panic!("retained completion must advance the reducer")
        };
        assert!(matches!(
            completion_effects.as_slice(),
            [AdapterEffect::Apply {
                subject,
                certificate,
                ..
            }] if *subject == manifest.subject && certificate == &decision
        ));
        assert!(!completion_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::FetchBody { .. } | AdapterEffect::StoreBody { .. }
        )));
        assert_eq!(runtime.queued_commands(), 0);

        let mut suppressed_phases = Vec::new();
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
            .expect("the decided validated body suppresses a drained local completion retry");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("decided_local_proposal_ready");

        runtime
            .enqueue_application_completed(owner_tag, manifest.subject)
            .expect("enqueue exact Apply acknowledgement");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch exact Apply acknowledgement"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        for _ in 0..3 {
            runtime
                .enqueue_application_completed(owner_tag, manifest.subject)
                .expect("an applied-height acknowledgement retry is a monotone stutter");
        }
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("application_completed");
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
    }

    #[test]
    fn decision_cleanup_preserves_unique_busy_deferred_completion() {
        let directory = TempDir::new().expect("temporary Busy-deferred Decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xDA);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage exact Busy-deferred completion");

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("retain exact Busy-deferred completion"),
            DecisionProposalRetirement::new(Some(owner_tag), 0)
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    manifest.round,
                    manifest.subject,
                    commitment,
                )
                .retainable(),
            1
        );
    }

    #[test]
    fn decision_commitment_mismatch_fails_closed_before_retirement() {
        let directory = TempDir::new().expect("temporary mismatched-decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD8);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"decision mismatch parent state"),
            Hash::new(b"decision mismatch post state"),
            Hash::new(b"decision mismatch ordinary writes"),
            1,
            Hash::new(b"decision mismatch executed block"),
        );
        assert_ne!(validated.execution_commitment(), conflicting_commitment);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    manifest.round,
                    manifest.subject,
                    conflicting_commitment,
                )
                .expect_err("Decision commitment drift must fail closed"),
            "Sumeragi v2 decided local proposal evidence conflicts with the durable Decision"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            1,
            "conflict preflight must preserve the original evidence for diagnosis"
        );
        assert!(matches!(
            runtime.ingress.commands.front().map(|queued| &queued.command),
            Some(AdapterCommand::LocalProposalReady {
                manifest: queued,
                ..
            }) if queued == &manifest
        ));
    }
