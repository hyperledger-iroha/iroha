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

        let mut reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen recovery body store");
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<_, String>(validated_receipt.execution_commitment())
            })
            .expect("semantically replay recovered validation marker");
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
                equivocation(&fixture, 1),
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

    #[test]
    fn production_transport_adversarial_matrix_still_finalizes_three_of_four() {
        let mut fixture = ProductionTransportFixture::new_validator();
        let started = Instant::now();
        fixture
            .executor
            .arm_live_clocks(started)
            .expect("arm the production serialized runtime");
        let mut services = FakeServices::default();

        let conflicting_body = b"delayed-GST equivocation payload".to_vec();
        let conflicting_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"delayed-GST equivocation block",
            )),
            payload_hash: Hash::new(&conflicting_body),
            ..fixture.subject
        };
        let conflicting_manifest = canonical_payload_manifest(
            &fixture.context,
            fixture.round,
            conflicting_subject,
            &conflicting_body,
        );
        let conflicting_durable = DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.round,
            conflicting_subject,
            HashOf::new(&conflicting_manifest),
        );
        let conflicting_validated = ValidatedBodyReceipt::for_test(conflicting_durable);
        let conflicting_vote_commitment = conflicting_validated.execution_commitment();
        fixture
            .executor
            .runtime
            .bind_validated_body(&conflicting_manifest, &conflicting_validated)
            .expect("bind a second locally validated equivocation subject");

        let signed_vote =
            |subject: wire::BlockSubject,
             execution_commitment: wire::ExecutionCommitment,
             signer: wire::ValidatorIndex| {
                let mut vote = wire::Vote {
                    round: fixture.round,
                    proposal_round: fixture.round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signer,
                    signature: Vec::new(),
                };
                vote.signature = Signature::new(
                    fixture.validator_keys
                        [usize::try_from(signer).expect("small fixture signer")]
                    .private_key(),
                    &vote.signature_preimage(),
                )
                .payload()
                .to_vec();
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote))
            };
        let canonical_share = signed_vote(fixture.subject, fixture.canonical_commitment, 3);
        let wire::ConsensusMessageV2Payload::Vote(expected_first) = canonical_share.payload.clone()
        else {
            unreachable!("phase-vote fixture")
        };
        let conflicting_share =
            signed_vote(conflicting_subject, conflicting_vote_commitment, 3);
        let wire::ConsensusMessageV2Payload::Vote(expected_second) =
            conflicting_share.payload.clone()
        else {
            unreachable!("phase-vote fixture")
        };
        fixture
            .executor
            .enqueue_network(canonical_share.clone())
            .expect("authenticate the withheld validator's first Prepare share");
        fixture
            .executor
            .enqueue_network(canonical_share)
            .expect("an exact duplicate coalesces without another reducer owner");
        fixture
            .executor
            .enqueue_network(conflicting_share)
            .expect("authenticate conflicting signed evidence for reducer reporting");
        for _ in 0..16 {
            if matches!(
                fixture
                    .executor
                    .step(started, &mut services)
                    .expect("drain duplicate/equivocation ingress"),
                EffectExecutorStep::Idle
            ) {
                break;
            }
        }
        assert_eq!(services.equivocations.len(), 1);
        let wire::SumeragiV2Equivocation::PhaseVote { first, second } =
            &services.equivocations[0]
        else {
            panic!("expected exact phase-vote equivocation evidence")
        };
        assert_eq!(first, &expected_first);
        assert_eq!(second, &expected_second);
        assert!(
            fixture
                .executor
                .runtime
                .replayed_decision_key()
                .expect("inspect durable decision before quorum")
                .is_none()
        );

        let canonical_prepare = fixture
            .quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
        assert_eq!(canonical_prepare.signers, vec![0, 1, 2]);
        let invalid_certificates = {
            let resign = |certificate: &mut wire::QuorumCertificate| {
                let preimage = wire::Vote {
                    round: certificate.round,
                    proposal_round: certificate.proposal_round,
                    phase: certificate.phase,
                    subject: certificate.subject,
                    execution_commitment: certificate.execution_commitment,
                    signer: certificate.signers[0],
                    signature: Vec::new(),
                }
                .signature_preimage();
                let shares = certificate
                    .signers
                    .iter()
                    .map(|signer| {
                        Signature::new(
                            fixture.validator_keys
                                [usize::try_from(*signer).expect("small QC signer")]
                            .private_key(),
                            &preimage,
                        )
                        .payload()
                        .to_vec()
                    })
                    .collect::<Vec<_>>();
                let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
                certificate.aggregate_signature =
                    iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                        .expect("re-sign adversarial certificate");
            };
            let mut foreign_context = fixture.context.clone();
            foreign_context.network_id =
                crate::sumeragi::synthetic_network_id("delayed-gst-foreign-context");

            let mut wrong_context = canonical_prepare.clone();
            wrong_context.round.context_id = foreign_context.id();
            wrong_context.proposal_round.context_id = foreign_context.id();
            resign(&mut wrong_context);

            let mut wrong_height = canonical_prepare.clone();
            wrong_height.round.height += 1;
            wrong_height.proposal_round.height += 1;
            resign(&mut wrong_height);

            let mut wrong_view = canonical_prepare.clone();
            wrong_view.proposal_round.view += 1;
            resign(&mut wrong_view);

            let mut wrong_signature = canonical_prepare.clone();
            wrong_signature.aggregate_signature[0] ^= 0x80;

            let wrong_commitment = fixture.quorum_certificate(
                wire::GlobalPhase::Prepare,
                fixture.conflicting_commitment,
            );
            [
                ("context", wrong_context),
                ("height", wrong_height),
                ("view", wrong_view),
                ("signature", wrong_signature),
                ("commitment", wrong_commitment),
            ]
        };
        for (kind, certificate) in invalid_certificates {
            let result = fixture.executor.enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
            ));
            assert!(result.is_err(), "wrong {kind} certificate was admitted");
            assert!(fixture.executor.runtime.driver().ingress_ready());
            assert!(
                fixture
                    .executor
                    .runtime
                    .replayed_decision_key()
                    .expect("invalid input cannot corrupt durable decision inspection")
                    .is_none()
            );
            assert!(!fixture.executor.status().fail_closed);
        }

        let prepare_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(canonical_prepare.clone()),
        );
        fixture
            .executor
            .enqueue_network(prepare_message.clone())
            .expect("the responsive 3-of-4 PrepareQC enters production ingress");
        fixture
            .executor
            .enqueue_network(prepare_message)
            .expect("the exact PrepareQC duplicate coalesces");
        for _ in 0..16 {
            if matches!(
                fixture
                    .executor
                    .step(started, &mut services)
                    .expect("drain the responsive PrepareQC"),
                EffectExecutorStep::Idle
            ) {
                break;
            }
        }
        assert!(
            fixture
                .executor
                .runtime
                .replayed_decision_key()
                .expect("PrepareQC is not a decision")
                .is_none()
        );

        let canonical_commit = fixture
            .quorum_certificate(wire::GlobalPhase::Commit, fixture.canonical_commitment);
        assert_eq!(canonical_commit.signers, vec![0, 1, 2]);
        let commit_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(canonical_commit.clone()),
        );
        fixture
            .executor
            .enqueue_network(commit_message.clone())
            .expect("the responsive 3-of-4 CommitQC enters production ingress");
        fixture
            .executor
            .enqueue_network(commit_message)
            .expect("the exact CommitQC duplicate coalesces");
        for _ in 0..32 {
            let _ = fixture
                .executor
                .step(started, &mut services)
                .expect("drive the 3-of-4 CommitQC to durable finality");
            if fixture
                .executor
                .runtime
                .replayed_decision_key()
                .expect("inspect durable 3-of-4 decision")
                .is_some()
            {
                break;
            }
        }
        assert_eq!(
            fixture
                .executor
                .runtime
                .replayed_decision_key()
                .expect("read durable finality key"),
            Some((
                canonical_commit.round,
                canonical_commit.proposal_round,
                canonical_commit.subject,
                canonical_commit.execution_commitment,
            ))
        );
        for _ in 0..16 {
            if matches!(
                fixture
                    .executor
                    .step(started, &mut services)
                    .expect("drain post-decision effects without duplicate application"),
                EffectExecutorStep::Idle
            ) {
                break;
            }
        }
        assert!(
            services.apply_tasks.len() <= 1,
            "an exact CommitQC duplicate must not transfer application twice"
        );
        assert!(
            services
                .apply_tasks
                .iter()
                .all(|task| task.subject() == fixture.subject)
        );
        assert!(!fixture.executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }
