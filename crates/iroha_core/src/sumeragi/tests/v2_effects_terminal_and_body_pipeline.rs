    #[test]
    fn effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains() {
        let fixture = Fixture::new();
        let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 97);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor
            .runtime
            .leader_wire_terminal_batches
            .push_back(Vec::new());
        executor
            .runtime
            .leader_wire_terminal_batches
            .push_back(vec![terminal.clone()]);
        let mut services = fixture.services();
        assert_eq!(
            executor
                .consume_effects(
                    vec![AdapterEffect::ReportEquivocation {
                        offender: fixture.context.roster[1].validator.clone(),
                        round: fixture.manifest.round,
                        kind: EquivocationKind::Vote,
                    }],
                    &mut services,
                )
                .expect("dispatch batch and consume its late terminal"),
            1
        );
        assert_eq!(services.leader_wire_terminals, vec![terminal]);
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("a consumed terminal cannot fail-close the next scheduler turn"),
            EffectExecutorStep::Idle
        );
    }

    #[test]
    fn lock_reconciliation_consumes_retirement_terminal_before_the_next_turn() {
        let fixture = Fixture::new();
        let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 96);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.runtime.leader_wire_terminal_after_lock = Some(terminal.clone());
        let mut services = fixture.services();

        executor
            .reconcile_locked_body_for_recovery(
                tag(fixture.manifest.round.view),
                (fixture.manifest.round, fixture.manifest.subject),
                &mut services,
            )
            .expect("lock retirement transfers its terminal in the same synchronous call");

        assert_eq!(services.leader_wire_terminals, vec![terminal]);
        assert!(executor.runtime.leader_wire_terminal_after_lock.is_none());
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("the next scheduler turn cannot overtake a lock-retirement terminal"),
            EffectExecutorStep::Idle
        );
    }

    #[test]
    fn leader_wire_terminal_batch_attempts_every_owner_after_one_transfer_fails() {
        let fixture = Fixture::new();
        let (_first_directory, first) = leader_wire_runtime_terminal_fixture(&fixture, 95);
        let (_second_directory, second) = leader_wire_runtime_terminal_fixture(&fixture, 96);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor
            .runtime
            .leader_wire_terminal_batches
            .push_back(vec![first, second.clone()]);
        let mut services = fixture.services();
        services.fail_on = Some("leader-wire-terminal");

        assert!(
            executor.consume_effects(Vec::new(), &mut services).is_err(),
            "the first injected terminal-transfer failure must fail closed"
        );
        assert_eq!(
            services.leader_wire_terminals,
            vec![second],
            "a failed first transfer cannot drop later independent runtime owners"
        );
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn retained_live_retry_consumes_decision_retirement_terminal_same_cycle() {
        let fixture = Fixture::new();
        let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 98);
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
        let mut services = fixture.services();
        assert_eq!(
            executor
                .consume_effects(
                    vec![timeout_sign(&fixture, 0), timeout_sign(&fixture, 1)],
                    &mut services,
                )
                .expect("retain the second timeout-sign effect at pending-work capacity"),
            1
        );
        assert!(executor.retained_effect_batch.is_some());

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body = Some((
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        ));
        executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());

        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("Decision retires the retained suffix and transfers its runtime terminal"),
            EffectExecutorStep::Idle
        );
        assert_eq!(services.leader_wire_terminals, vec![terminal]);
        assert!(
            executor
                .runtime
                .leader_wire_terminal_after_decision
                .is_none()
        );
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert!(executor.retained_effect_batch.is_none());
        assert!(executor.pending_signatures.is_empty());
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn retained_drain_failure_transfers_decision_terminal_before_fail_close() {
        let fixture = Fixture::new();
        let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 100);
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
        let mut services = fixture.services();
        assert_eq!(
            executor
                .consume_effects(
                    vec![timeout_sign(&fixture, 0), timeout_sign(&fixture, 1)],
                    &mut services,
                )
                .expect("retain the second timeout-sign effect at pending-work capacity"),
            1
        );
        assert!(executor.retained_effect_batch.is_some());

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body = Some((
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        ));
        executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());
        services.fail_on = Some("cancel-sign");

        assert!(
            executor.step(Instant::now(), &mut services).is_err(),
            "the injected retained-suffix cancellation failure must fail closed"
        );
        assert_eq!(
            services.leader_wire_terminals,
            vec![terminal],
            "the earlier Decision terminal must cross its gate before fail-close teardown"
        );
        assert!(
            executor
                .runtime
                .leader_wire_terminal_after_decision
                .is_none()
        );
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle() {
        let fixture = Fixture::new();
        let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 99);
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
        let mut services = fixture.services();
        let fetch = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        assert_eq!(
            executor
                .consume_effects(vec![timeout_sign(&fixture, 0), fetch], &mut services)
                .expect("retain the exact recovery fetch at pending-work capacity"),
            1
        );
        assert!(executor.retained_effect_batch.is_some());

        let durable = DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.manifest.round,
            fixture.manifest.subject,
            HashOf::new(&fixture.manifest),
        );
        executor.recovered_bodies.insert(
            (fixture.manifest.round, fixture.manifest.subject),
            (fixture.manifest.clone(), durable),
        );
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body = Some((
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        ));
        executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());

        assert_eq!(
            executor
                .step_pending_tip_recovery(Instant::now(), &mut services)
                .expect("recovery retry consumes the Decision retirement terminal"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(services.leader_wire_terminals, vec![terminal]);
        assert!(
            executor
                .runtime
                .leader_wire_terminal_after_decision
                .is_none()
        );
        assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
        assert!(executor.retained_effect_batch.is_none());
        assert!(executor.pending_signatures.is_empty());
        assert_eq!(
            executor.status().pending_tip_recovery_last_result,
            Some(PendingTipRecoveryAttemptResult::Advanced)
        );
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn ready_body_backpressure_retains_exact_ingress_until_capacity_retry() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, 1, 4));
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("fetch");
        let fetch_task = services.fetch_tasks[0].clone();
        assert!(matches!(
            executor.complete_body_reconstruction(
                &fetch_task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            ),
            Err(EffectTransportError::Backpressure)
        ));
        assert!(!executor.status().fail_closed);
        let mut wrong = fixture.body.clone();
        wrong[0] ^= 1;
        assert!(matches!(
            executor.complete_body_reconstruction(
                &fetch_task,
                fixture.manifest.clone(),
                wrong,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch(_))
        ));

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("upgrade the retained fetch with certified authority");
        let request = services
            .fetch_tasks
            .last()
            .and_then(BodyFetchTask::certified_request)
            .expect("signed certified request");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(request),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: 0,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();

        let responder = fixture.context.roster[0].validator.clone();
        let (_leader_wire_directory, leader_wire_ingress, leader_wire_gate, ingress_ownership) =
            certified_response_runtime_ingress_ownership(&fixture, &response, responder.clone());
        let expected_terminal = LeaderWireRuntimeTerminal::Volatile(
            ingress_ownership
                .leader_wire_runtime_receipt()
                .expect("production response owns a runtime receipt")
                .clone(),
        );
        let response_envelope = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response.clone()),
        );
        assert!(matches!(
            executor.accept_certified_body_response_with_ingress_ownership(
                response,
                &responder,
                &ingress_ownership,
                &mut services,
            ),
            Err(EffectTransportError::Backpressure)
        ));
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.certified_work.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
        assert!(executor.has_retained_certified_body_response());
        let retained_scheduler_ordinal = executor
            .retained_certified_body_response_scheduler_ordinal()
            .expect("read retained response ordinal")
            .expect("retained response owns one scheduler position");
        assert!(!executor.can_admit_local_proposal());
        assert!(
            !executor.retained_dispatch_allows_network_ingress(&response_envelope.payload),
            "later ingress remains behind the exact retained completion"
        );
        assert!(
            executor.outstanding_requests.response_claim_count() == 0,
            "capacity rejection precedes response-occurrence acquisition"
        );
        assert!(services.leader_wire_terminals.is_empty());
        assert!(
            leader_wire_gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read live response gate")
                .is_some(),
            "capacity backpressure must not tombstone the runtime owner"
        );
        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(response_envelope.clone()),
                Some(responder.clone()),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            leader_wire_ingress.try_recv().is_none(),
            "an exact retransmission coalesces into the retained runtime owner"
        );
        assert!(!executor.status().fail_closed);

        executor.config.max_ready_body_bytes =
            u64::try_from(fixture.body.len()).expect("body length");
        services.retry_certified_fetch_once = true;
        assert_eq!(
            executor.retry_retained_certified_body_response(&mut services),
            Err(EffectTransportError::Backpressure),
            "the typed retryable service boundary retains the production carrier",
        );
        assert!(executor.has_retained_certified_body_response());
        assert_eq!(
            executor
                .retained_certified_body_response_scheduler_ordinal()
                .expect("read typed-retry response ordinal"),
            Some(retained_scheduler_ordinal),
            "typed retry preserves the original leader-wire ticket",
        );
        assert_eq!(executor.outstanding_requests.response_claim_count(), 1);
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.certified_work.len(), 1);
        let retained_runtime_token = executor
            .body_ownership_projection()
            .runtime_body_reservation
            .expect("typed retry retains the successfully reserved runtime token");
        assert_eq!(retained_runtime_token.tag(), tag(0));
        assert_eq!(retained_runtime_token.manifest(), &fixture.manifest);
        assert!(services.completed_certified_fetches.is_empty());
        assert!(services.leader_wire_terminals.is_empty());
        assert!(
            leader_wire_gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read typed-retry response gate")
                .is_some(),
            "typed retry cannot tombstone the retained leader-wire ticket",
        );
        assert!(!executor.status().fail_closed);

        assert_eq!(
            executor
                .retry_retained_certified_body_response(&mut services)
                .expect("same response succeeds after capacity is available"),
            Some(CompletionDisposition::Accepted)
        );
        assert!(!executor.has_retained_certified_body_response());
        assert_eq!(
            services.leader_wire_terminals,
            vec![expected_terminal.clone()]
        );
        let LeaderWireRuntimeTerminal::Volatile(runtime) = &expected_terminal else {
            panic!("certified response completion must publish a volatile terminal");
        };
        leader_wire_ingress
            .mark_leader_wire_volatile_terminal(runtime)
            .expect("production terminal hook retires the response runtime owner");
        assert_eq!(
            leader_wire_gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read retired response gate"),
            None
        );
        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(response_envelope),
                Some(responder),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            leader_wire_ingress.try_recv().is_none(),
            "the volatile tombstone cannot resurrect the drained response stage"
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![fetch_task.id()]);
    }

    #[test]
    fn body_fetch_authority_upgrades_monotonically_in_both_orders() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);

        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("proposal starts ordinary acquisition");
        let work_id = services.fetch_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("PrepareQC adds certified authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        assert_eq!(upgraded.id(), work_id);
        assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
        assert_eq!(
            upgraded
                .certified_request()
                .map(|request| &request.certificate),
            Some(&prepare)
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);

        let first_request = upgraded
            .certified_request()
            .expect("first certified authority")
            .clone();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(commit),
                }],
                &mut services,
            )
            .expect("later same-subject QC retransmits first authority");
        assert_eq!(
            services
                .fetch_tasks
                .last()
                .and_then(BodyFetchTask::certified_request),
            Some(&first_request)
        );
        assert_eq!(executor.outstanding_requests.len(), 1);

        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: None,
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("PrepareQC starts certified acquisition");
        let work_id = services.fetch_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("proposal adds manifest authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        assert_eq!(upgraded.id(), work_id);
        assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
        assert_eq!(
            upgraded
                .certified_request()
                .map(|request| &request.certificate),
            Some(&prepare)
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
    }

    #[test]
    fn hybrid_reconstruction_wins_and_retires_certified_request() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("start hybrid acquisition");
        let task = services.fetch_tasks[0].clone();

        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &task,
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("authenticated reconstruction wins"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert!(services.completed_certified_fetches.is_empty());
    }

    #[test]
    fn authenticated_genesis_satisfies_later_view_fetch_through_normal_body_pipeline() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .install_authenticated_genesis_body(&fixture.block)
            .expect("retain authenticated staged genesis");

        let manifest = manifest_at_view(&fixture, 5);
        let round = manifest.round;
        let subject = manifest.subject;
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(5),
                    round,
                    subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("derive the later-view manifest from authenticated genesis");

        assert!(services.fetch_tasks.is_empty());
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.ready_bodies[&(round, subject)].manifest, manifest);
        assert_eq!(
            executor.ready_bodies[&(round, subject)].bytes.as_ref(),
            fixture.body.as_slice()
        );
        assert!(executor.durable_bodies.is_empty());
        assert!(executor.validated_bodies.is_empty());
        assert_eq!(
            executor.runtime.completions,
            vec![RuntimeCompletion::BodyAvailable(tag(5), manifest.clone())]
        );

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(5),
                    round,
                    subject,
                }],
                &mut services,
            )
            .expect("enter the ordinary durable-store stage");
        assert_eq!(services.store_tasks.len(), 1);
        assert_eq!(services.store_tasks[0].manifest(), &manifest);
        assert_eq!(
            services.store_tasks[0].canonical_wire(),
            fixture.body.as_slice()
        );
        let store_id = services.store_tasks[0].id();
        let store_completion = services.execute_store(store_id);
        executor
            .complete_body_store(store_completion, &mut services)
            .expect("complete the current-round durable store");
        assert_eq!(executor.durable_bodies[&(round, subject)].round(), round);

        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(5),
                    round,
                    subject,
                }],
                &mut services,
            )
            .expect("enter ordinary deterministic validation");
        assert_eq!(services.validation_tasks.len(), 1);
        assert_eq!(services.validation_tasks[0].round(), round);
        assert_eq!(services.validation_tasks[0].subject(), subject);
        assert!(executor.validated_bodies.is_empty());
    }

    #[test]
    fn authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .install_authenticated_genesis_body(&fixture.block)
            .expect("retain authenticated staged genesis");

        let certificate = fixture.qc(wire::GlobalPhase::Commit);
        let sources = certified_sources(&fixture, &certificate);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: None,
                    certified_sources: sources,
                    certificate: Some(certificate),
                }],
                &mut services,
            )
            .expect("consume certified Decision from authenticated local genesis");

        assert!(services.fetch_tasks.is_empty());
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(
            executor.ready_bodies[&(fixture.manifest.round, fixture.manifest.subject)].manifest,
            fixture.manifest
        );
        assert_eq!(
            executor.runtime.completions,
            vec![RuntimeCompletion::BodyAvailable(
                tag(0),
                fixture.manifest.clone()
            )]
        );
    }

    #[test]
    fn authenticated_genesis_cache_does_not_satisfy_a_different_subject() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .install_authenticated_genesis_body(&fixture.block)
            .expect("retain authenticated staged genesis");

        let proposal_round = round(&fixture.context, 4);
        let (subject, body) = distinct_body(&fixture);
        let manifest = wire::PayloadManifest::derive(
            &fixture.context,
            proposal_round,
            subject,
            u64::try_from(body.len()).expect("distinct body length"),
            std::slice::from_ref(&body),
        )
        .expect("distinct current-round manifest");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(4),
                    round: proposal_round,
                    subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("unrelated proposal uses network acquisition");

        assert_eq!(services.fetch_tasks.len(), 1);
        assert_eq!(services.fetch_tasks[0].manifest(), Some(&manifest));
        assert_eq!(executor.pending_fetches.len(), 1);
        assert!(executor.ready_bodies.is_empty());
        assert!(executor.runtime.completions.is_empty());
    }

    #[test]
    fn retained_exact_body_pipeline_prevents_reacquisition_at_every_stage() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let fetch = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("start one exact acquisition");
        let task = services.fetch_tasks[0].clone();
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain reconstructed body");
        assert_eq!(executor.runtime.queued_commands(), 1);

        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("ready body makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 1);

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("advance body into exact store ownership");
        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("pending store makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert_eq!(executor.pending_stores.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 1);

        let store_id = services.store_tasks[0].id();
        let completion = services.execute_store(store_id);
        executor
            .complete_body_store(completion, &mut services)
            .expect("advance body into durable ownership");
        assert_eq!(executor.runtime.queued_commands(), 2);
        executor
            .consume_effects(vec![fetch], &mut services)
            .expect("durable receipt makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.durable_bodies.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 2);

        let mut conflicting_manifest = fixture.manifest.clone();
        conflicting_manifest.payload_size_bytes = conflicting_manifest
            .payload_size_bytes
            .checked_add(1)
            .expect("small fixture body");
        let conflicting_result = executor.consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(conflicting_manifest),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        );
        assert!(
            matches!(conflicting_result, Err(EffectExecutorError::Contract(_))),
            "conflicting retained manifest must fail closed: {conflicting_result:?}"
        );
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.status().fail_closed);
    }
    #[test]
    fn uncertified_fetch_rejects_spurious_certified_sources() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: vec![fixture.context.roster[0].validator.clone()],
                    certificate: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert!(services.fetch_tasks.is_empty());
        assert!(executor.status().fail_closed);
    }
