    #[test]
    fn tc_body_rebind_cancels_fetch_superseded_by_a_higher_different_qc() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
        let original = fixture.qc(wire::GlobalPhase::Prepare);
        let original_sources = certified_sources(&fixture, &original);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer_tag(0),
                    round: original.round,
                    subject: original.subject,
                    manifest: None,
                    certified_sources: original_sources,
                    certificate: Some(original.clone()),
                }],
                &mut services,
            )
            .expect("begin original protected fetch");
        let original_id = services.fetch_tasks[0].id();

        let mut first_timeout = timeout_at_view(&fixture, 0);
        first_timeout.groups[0].highest_prepare_qc = Some(original.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(1),
                    certificate: first_timeout,
                    protected_body: Some((original.round, original.subject)),
                }],
                &mut services,
            )
            .expect("retain original exact high-QC fetch");

        let replacement_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replacement high-QC block")),
            payload_hash: Hash::new(b"replacement high-QC payload"),
            ..original.subject
        };
        let replacement_round = round(&fixture.context, 1);
        let replacement = wire::QuorumCertificate {
            round: replacement_round,
            proposal_round: replacement_round,
            phase: wire::GlobalPhase::Prepare,
            subject: replacement_subject,
            execution_commitment: fixture_execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let mut replacement_timeout = timeout_at_view(&fixture, 1);
        replacement_timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(2),
                    certificate: replacement_timeout,
                    protected_body: Some((replacement.round, replacement.subject)),
                }],
                &mut services,
            )
            .expect("higher different QC supersedes old acquisition");

        assert!(executor.pending_fetches.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.cancelled_fetches, vec![original_id]);

        let replacement_sources = certified_sources(&fixture, &replacement);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer_tag(2),
                    round: replacement.round,
                    subject: replacement.subject,
                    manifest: None,
                    certified_sources: replacement_sources,
                    certificate: Some(replacement),
                }],
                &mut services,
            )
            .expect("replacement high-QC fetch claims the released bounded slot");
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_work(), 1);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_view_churn_cancels_stale_fetches_and_releases_capacity() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        for view in 0..6 {
            let manifest = manifest_at_view(&fixture, view);
            let certificate = wire::QuorumCertificate {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: wire::GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: fixture_execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            };
            let sources = certified_sources(&fixture, &certificate);
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: EventTag::new(1, view, Generation::new(7 + view)),
                        round: manifest.round,
                        subject: manifest.subject,
                        manifest: None,
                        certified_sources: sources,
                        certificate: Some(certificate),
                    }],
                    &mut services,
                )
                .expect("begin view fetch");
            assert_eq!(executor.pending_fetches.len(), 1);
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, view + 1, Generation::new(8 + view)),
                        certificate: timeout_at_view(&fixture, view),
                        protected_body: None,
                    }],
                    &mut services,
                )
                .expect("install next view");
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.outstanding_requests.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
        }
        assert_eq!(services.cancelled_fetches.len(), 6);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_view_churn_cancels_stale_signing_and_releases_capacity() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let mut stale_ids = Vec::new();
        for view in 0..6 {
            let manifest = manifest_at_view(&fixture, view);
            persist_fsynced_validation_marker(
                &mut executor,
                &mut services,
                &fixture,
                manifest.clone(),
            );
            executor
                .consume_effects(
                    vec![AdapterEffect::Sign {
                        tag: EventTag::new(1, view, Generation::new(7 + view)),
                        request: SignRequest::Vote(wire::Vote {
                            round: manifest.round,
                            proposal_round: manifest.round,
                            phase: wire::GlobalPhase::Prepare,
                            subject: manifest.subject,
                            execution_commitment: fixture_execution_commitment(),
                            signer: 0,
                            signature: Vec::new(),
                        }),
                    }],
                    &mut services,
                )
                .expect("begin view signing");
            stale_ids.push(services.sign_tasks.last().expect("sign task").id());
            assert_eq!(executor.pending_signatures.len(), 1);
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, view + 1, Generation::new(8 + view)),
                        certificate: timeout_at_view(&fixture, view),
                        protected_body: None,
                    }],
                    &mut services,
                )
                .expect("install next view");
            assert!(executor.pending_signatures.is_empty());
        }
        assert_eq!(services.cancelled_signatures, stale_ids);
        let late_signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            b"late completion is never admitted",
        )
        .payload()
        .to_vec();
        assert_eq!(
            executor
                .complete_consensus_signature(stale_ids[0], late_signature, &mut services)
                .expect("late signature is stale"),
            CompletionDisposition::Stale
        );
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_sources_must_exactly_match_canonical_frozen_roster() {
        let fixture = Fixture::new();
        let certificate = fixture.qc(wire::GlobalPhase::Prepare);
        let canonical = certified_sources(&fixture, &certificate);
        let signer_only = certificate
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        let mut duplicate = canonical.clone();
        duplicate[3] = duplicate[0].clone();
        let mut reordered = canonical.clone();
        reordered.swap(0, 1);
        for bad_sources in [signer_only, duplicate, reordered] {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            assert!(matches!(
                executor.consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: None,
                        certified_sources: bad_sources,
                        certificate: Some(certificate.clone()),
                    }],
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(_))
            ));
            assert!(services.fetch_tasks.is_empty());
            assert!(executor.status().fail_closed);
        }
    }

    #[test]
    fn reopened_durable_receipt_satisfies_fetch_without_network() {
        let fixture = Fixture::new();
        let directory = TempDir::new().expect("recovery directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open body store");
        let task = BodyStoreTask::for_test(
            91,
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
        );
        let durable = store
            .execute_store_task(&task)
            .expect("persist body before crash");
        let receipt = durable.receipt().clone();
        drop(store);
        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen body store");
        let catalog = reopened.recovery_catalog().expect("recovery catalog");
        let mut executor = V2EffectExecutor::with_runtime(
            FakeRuntime::default(),
            catalog,
            fixture.context.clone(),
            PeerId::new(fixture.requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::default(),
        )
        .expect("recovered executor");
        let mut services = FakeServices {
            _body_directory: Some(directory),
            body_store: Some(reopened),
            requester_key: Some(fixture.requester_key.clone()),
            ..FakeServices::default()
        };
        let recovered_fetch = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        executor
            .consume_effects(vec![recovered_fetch.clone()], &mut services)
            .expect("recover local durable body");
        assert!(services.fetch_tasks.is_empty());
        assert_eq!(executor.runtime.queued_commands(), 1);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
        executor
            .consume_effects(vec![recovered_fetch], &mut services)
            .expect("retransmitted recovery fetch remains idempotent");
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
            .expect("acknowledge recovered durability");
        assert_eq!(
            executor
                .durable_bodies
                .get(&(fixture.manifest.round, fixture.manifest.subject)),
            Some(&receipt)
        );
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("queue recovered validation");
        let validation_id = services.validation_tasks[0].id();
        let completion = services.execute_validation(validation_id);
        executor
            .complete_body_validation(completion, &mut services)
            .expect("validate reopened exact body");
    }

    #[test]
    fn delayed_pending_tip_recovery_allows_only_local_apply_pipeline() {
        let fixture = Fixture::new();
        let directory = TempDir::new().expect("recovery directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open body store");
        let durable = store
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist exact decided body");
        let validated_receipt = store
            .validate(&durable, |_| {
                Ok::<_, &'static str>(fixture_execution_commitment())
            })
            .expect("persist exact deterministic-validation marker");
        drop(store);

        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen recovery body store");
        let recovered = reopened.recovery_catalog().expect("recovery catalog");
        let recovered_validations = reopened.validated_recovery_catalog();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let expected = PendingKuraApply::for_test(
            fixture.context.id(),
            fixture.context.height,
            fixture.block.hash(),
        );
        let decision = Some((
            commit.round,
            commit.proposal_round,
            commit.subject,
            validated_receipt.execution_commitment(),
        ));
        let (_, recovery_evidence) = verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &recovered_validations,
            expected,
            tag(0),
            tag(0),
            commit.clone(),
            Some(&fixture.manifest),
        )
        .expect("authenticate delayed pending-tip recovery evidence");
        let mut runtime = FakeRuntime {
            round_tag: Some(tag(0)),
            ..FakeRuntime::default()
        };
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources: certified_sources(&fixture, &commit),
                certificate: Some(commit.clone()),
            }])));
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }])));
        runtime.steps.push_back(Ok(RuntimeStep::Advanced(vec![
            AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
        ])));
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: commit,
            }])));

        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            recovered,
            fixture.context.clone(),
            PeerId::new(fixture.requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::default(),
        )
        .expect("recovered executor");
        executor.validated_bodies = recovered_validations;
        executor.pending_tip_recovery = Some(recovery_evidence);
        let mut services = FakeServices {
            _body_directory: Some(directory),
            body_store: Some(reopened),
            requester_key: Some(fixture.requester_key.clone()),
            ..FakeServices::default()
        };

        for _ in 0..4 {
            assert!(matches!(
                executor
                    .step_pending_tip_recovery(Instant::now(), &mut services)
                    .expect("advance local-only recovery"),
                EffectExecutorStep::Advanced { effects: 1 }
            ));
        }
        assert_eq!(executor.pending_tip_recovery_attempts(), 4);
        assert_eq!(
            executor.status().pending_tip_recovery_stage,
            Some(PendingKuraApplyRecoveryStage::ApplicationDispatched)
        );
        assert_eq!(
            executor.status().pending_tip_recovery_last_result,
            Some(PendingTipRecoveryAttemptResult::Advanced)
        );
        assert_eq!(services.apply_tasks.len(), 1);
        assert!(services.fetch_tasks.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.broadcasts.is_empty());
        assert!(services.entered_views.is_empty());
        assert!(services.equivocations.is_empty());
        assert!(services.invalid_bodies.is_empty());

        // Model a slow WSV/checkpoint/fsync completion. Repeated idle polling must remain silent,
        // and an accidental reducer broadcast is rejected before reaching the network adapter.
        for _ in 0..3 {
            assert_eq!(
                executor
                    .step_pending_tip_recovery(Instant::now(), &mut services)
                    .expect("wait for delayed local Apply"),
                EffectExecutorStep::Idle
            );
        }
        assert_eq!(executor.pending_tip_recovery_attempts(), 7);
        assert_eq!(
            executor.status().pending_tip_recovery_last_result,
            Some(PendingTipRecoveryAttemptResult::Waiting)
        );
        executor
            .record_pending_tip_recovery_deadline_exceeded(&mut services)
            .expect("publish terminal recovery deadline observation");
        assert_eq!(
            services
                .statuses
                .last()
                .expect("deadline status")
                .pending_tip_recovery_last_result,
            Some(PendingTipRecoveryAttemptResult::DeadlineExceeded)
        );
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Broadcast(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    fixture.qc(wire::GlobalPhase::Commit),
                )),
            )])));
        assert!(matches!(
            executor.step_pending_tip_recovery(Instant::now(), &mut services),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("non-local consensus effect")
        ));
        assert!(services.broadcasts.is_empty());
    }

    #[test]
    fn runtime_step_dispatches_entire_effect_batch_before_returning() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let message = wire::ConsensusMessageV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            payload: wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                signature: vec![1],
                ..vote(&fixture)
            }),
        };
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![
                AdapterEffect::Broadcast(message.clone()),
                AdapterEffect::ReportEquivocation {
                    offender: fixture.context.roster[1].validator.clone(),
                    round: fixture.manifest.round,
                    kind: EquivocationKind::Vote,
                },
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.manifest.subject,
                    certificate: fixture.qc(wire::GlobalPhase::Prepare),
                },
            ])));

        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("dispatch complete effect batch"),
            EffectExecutorStep::Advanced { effects: 3 }
        );
        assert_eq!(
            services.effect_service_order,
            vec!["broadcast", "equivocation", "invalid-body"]
        );
        assert_eq!(services.broadcasts, vec![message]);
        assert_eq!(services.equivocations.len(), 1);
        assert_eq!(services.invalid_bodies, vec![fixture.manifest.subject]);
        assert!(
            executor.runtime.steps.is_empty(),
            "the emitted effect batch must have no pending tail"
        );
        assert!(executor.runtime.completions.is_empty());
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("runtime is idle after complete batch dispatch"),
            EffectExecutorStep::Idle
        );
    }

    #[test]
    fn runtime_step_consumes_effect_batch_and_idle_publishes_status() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        persist_fsynced_validation_marker(
            &mut executor,
            &mut services,
            &fixture,
            fixture.manifest.clone(),
        );
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }])));
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("advanced step"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("idle step"),
            EffectExecutorStep::Idle
        );
        assert_eq!(services.sign_tasks.len(), 1);
        assert_eq!(services.statuses.len(), 2);
    }
