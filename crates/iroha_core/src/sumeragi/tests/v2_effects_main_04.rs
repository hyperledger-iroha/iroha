#[cfg(feature = "bls")]
#[test]
fn live_local_proposal_sign_enters_lifecycle_before_generic_sign_or_broadcast() {
    let fixture = ProductionTransportFixture::new();
    let tag = tag(0);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.round,
        fixture.subject,
        HashOf::new(&fixture.manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verify live ProposalIntent context");
    let directory = TempDir::new().expect("temporary live ProposalIntent WAL");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        directory.path().join("live-proposal-intent.wal"),
        verified,
        Some(fixture.context.leader(0)),
        tag.generation(),
        [0xD7; 32],
        AdapterFingerprints {
            node: Hash::new(b"live ProposalIntent node"),
            build: Hash::new(b"live ProposalIntent build"),
            config: Hash::new(b"live ProposalIntent config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open live ProposalIntent adapter");
    assert!(startup.is_empty());
    assert_eq!(adapter.current_tag(), tag);
    let effects = adapter
        .local_proposal_ready(tag, fixture.manifest.clone(), &durable, &validated)
        .expect("drive LocalProposalReady into one fsynced ProposalIntent")
        .into_effects();
    let [sign_effect] = effects.as_slice() else {
        panic!("local ProposalReady must emit one Proposal Sign: {effects:?}")
    };
    assert!(matches!(
        sign_effect,
        AdapterEffect::Sign {
            request: SignRequest::Proposal(proposal),
            ..
        } if proposal.signature.is_empty()
    ));
    let handoff = adapter
        .take_live_proposal_intent_wal_sign(&effects)
        .expect("consume exact live ProposalIntent sidecar")
        .expect("live ProposalIntent must retain one WAL Sign sidecar");

    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let store_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&store_effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                9_701,
                b"live local proposal lineage",
            ),
        ],
    )
    .expect("bind exact local Store owner")
    .pop()
    .expect("one local Store owner");
    let local =
        LocalProposalEffectOwnership::for_test(store_ownership, &store_effect, &fixture.manifest)
            .expect("seal local Store replay lineage");
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let validate_ownership = local
        .exact_store_task_ownership(&store_effect, &fixture.manifest)
        .expect("retain exact local Store scheduling owner")
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("project exact local Validate owner");
    let validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(&validate_effect)
        .expect("project exact local Validate pending owner");
    let command_identity = LocalProposalReadyCommandIdentity::from_exact_pending_handoff(
        tag,
        &fixture.manifest,
        &durable,
        &validated,
        &validate_pending,
    )
    .expect("derive exact LocalProposalReady command identity");
    let ready = local
        .project_exact_validate(
            &store_effect,
            &fixture.manifest,
            &durable,
            &validate_effect,
            &validate_ownership,
        )
        .unwrap_or_else(|_| panic!("project exact local Validate replay"))
        .complete_local_proposal(
            &validate_effect,
            &fixture.manifest,
            validated,
            command_identity,
            validate_ownership.owner().lifecycle_ordinal(),
        )
        .unwrap_or_else(|_| panic!("complete exact LocalProposalReady replay"));
    let sign_ownership = validate_ownership
        .rebind_as_inherited_adapter_effect(sign_effect)
        .expect("project exact ProposalIntent effect owner");
    let mut executor = V2EffectExecutor::with_runtime(
        FakeRuntime {
            steps: VecDeque::from([Ok(RuntimeStep::Advanced(effects.clone()))]),
            round_tag: Some(tag),
            next_lifecycle_ordinal: 9_702,
            exact_effect_ownership: Some((sign_effect.clone(), sign_ownership)),
            live_proposal_intent_wal_sign: Some((sign_effect.clone(), handoff)),
            ..FakeRuntime::default()
        },
        BTreeMap::new(),
        fixture.context.clone(),
        PeerId::new(fixture.requester_key.public_key().clone()),
        Some(fixture.context.leader(0)),
        EffectQueueConfig::default(),
    )
    .expect("construct live ProposalIntent executor");
    assert!(
        executor
            .local_proposal_ready_replay
            .insert(command_identity, ready)
            .is_none()
    );
    let mut services = FakeServices::default();
    assert!(matches!(
        executor.step(Instant::now(), &mut services),
        Ok(EffectExecutorStep::Advanced { effects: 1 })
    ));
    assert!(executor.pending_live_wal_sign_admission.is_some());
    assert_eq!(executor.status().pending_signatures, 1);
    assert_eq!(executor.pending_work(), 1);
    assert!(executor.pending_signatures.is_empty());
    assert!(executor.pending_lifecycle_output_admissions.is_empty());
    assert!(executor.local_proposal_ready_replay.is_empty());
    assert!(executor.local_proposal_intent_replay.is_empty());
    assert!(services.sign_tasks.is_empty());
    assert!(services.broadcast_attempts.is_empty());
    assert!(services.broadcasts.is_empty());
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    live_proposal_intent_sign_is_joined_before_generic_effect_dispatch
);

#[test]
fn live_lifecycle_validation_marker_promotes_idempotently_and_authorizes_vote_signing() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let durable = services
        .body_store
        .as_mut()
        .expect("body store service")
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist exact body fixture");
    let validated =
        validate_durable_body_fixture(&mut services, &fixture.manifest, durable.clone());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );
    assert!(executor.validated_bodies.is_empty());

    executor
        .record_lifecycle_validated_body(ReadyValidatedExecutorCatalogAuthorityV1::for_test(
            validated.clone(),
        ))
        .expect("promote one live fsynced validation marker");
    executor
        .record_lifecycle_validated_body(ReadyValidatedExecutorCatalogAuthorityV1::for_test(
            validated.clone(),
        ))
        .expect("exact live marker retry is idempotent");
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));

    let conflicting = ValidatedBodyReceipt::for_test_with_commitment(
        durable,
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting live marker parent state"),
            Hash::new(b"conflicting live marker post state"),
            Hash::new(b"conflicting live marker writes"),
            1,
            Hash::new(b"conflicting live marker executed block"),
        ),
    );
    assert!(matches!(
        executor.record_lifecycle_validated_body(
            ReadyValidatedExecutorCatalogAuthorityV1::for_test(conflicting),
        ),
        Err(EffectExecutorError::BodyStore(reason))
            if reason.contains("conflicting validation receipts")
    ));
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));

    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }],
            &mut services,
        )
        .expect("live validation marker authorizes exact Vote signing");
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(!executor.status().fail_closed);
}

#[test]
fn vote_signing_requires_the_exact_fsynced_execution_commitment() {
    let fixture = Fixture::new();
    let mut missing = fixture.executor(EffectQueueConfig::default());
    let mut missing_services = fixture.services();
    assert!(matches!(
        missing.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }],
            &mut missing_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("fsynced validation marker")
    ));
    assert!(missing.status().fail_closed);
    assert!(missing_services.sign_tasks.is_empty());
    let mut drift = fixture.executor(EffectQueueConfig::default());
    let mut drift_services = fixture.services();
    install_fsynced_validation_fixture(
        &mut drift,
        &mut drift_services,
        &fixture,
        fixture.manifest.clone(),
    );
    let mut drifted_vote = vote(&fixture);
    drifted_vote.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"drifted effects fixture parent state"),
        Hash::new(b"drifted effects fixture post state"),
        Hash::new(b"drifted effects fixture ordinary writes"),
        1,
        Hash::new(b"drifted effects fixture executed block wire"),
    );
    assert!(matches!(
        drift.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(drifted_vote),
            }],
            &mut drift_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("differs from the durable validation marker")
    ));
    assert!(drift.status().fail_closed);
    assert!(drift_services.sign_tasks.is_empty());
}
#[test]
fn split_round_commit_signing_is_rejected_before_service_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let mut commit = vote(&fixture);
    commit.round = round(&fixture.context, fixture.manifest.round.view + 2);
    commit.proposal_round = fixture.manifest.round;
    commit.phase = wire::GlobalPhase::Commit;
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(commit.round.view),
                request: SignRequest::Vote(commit),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("same-round proposal authority")
    ));
    assert!(services.sign_tasks.is_empty());
}
#[test]
fn reproposal_commit_signing_uses_its_same_round_validation_marker() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let reproposal_round = round(&fixture.context, fixture.manifest.round.view + 2);
    let reproposal_manifest = canonical_payload_manifest(
        &fixture.context,
        reproposal_round,
        fixture.manifest.subject,
        &fixture.body,
    );
    install_fsynced_validation_fixture(&mut executor, &mut services, &fixture, reproposal_manifest);
    let mut commit = vote(&fixture);
    commit.round = reproposal_round;
    commit.proposal_round = reproposal_round;
    commit.phase = wire::GlobalPhase::Commit;
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(reproposal_round.view),
                request: SignRequest::Vote(commit.clone()),
            }],
            &mut services,
        )
        .expect("same-round reproposal Commit owns its exact validation marker");
    assert!(matches!(
        services.sign_tasks.as_slice(),
        [task]
            if matches!(task.request(), SignRequest::Vote(vote) if vote == &commit)
    ));
    assert!(!executor.status().fail_closed);
}
#[test]
fn sign_effect_verifies_signature_and_preserves_original_tag() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let request = SignRequest::Vote(vote(&fixture));
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: request.clone(),
            }],
            &mut services,
        )
        .expect("consume sign");
    let task = services.sign_tasks[0].clone();
    let preimage = match task.request() {
        SignRequest::Vote(vote) => vote.signature_preimage(),
        _ => panic!("vote task expected"),
    };
    let signature = Signature::new(fixture.validator_keys[0].private_key(), &preimage)
        .payload()
        .to_vec();
    assert_eq!(
        executor
            .complete_consensus_signature(task.id(), signature.clone(), &mut services)
            .expect("complete signature"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        &executor.runtime.completions[0],
        RuntimeCompletion::Signature(completion_tag, completion)
            if *completion_tag == tag(0) && completion == &signature
    ));
    assert_eq!(
        executor
            .complete_consensus_signature(task.id(), signature, &mut services)
            .expect("stale completion"),
        CompletionDisposition::Stale
    );
}
#[test]
fn invalid_signer_completion_fails_closed_without_runtime_input() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }],
            &mut services,
        )
        .expect("consume sign");
    let id = services.sign_tasks[0].id();
    let wrong = Signature::new(fixture.validator_keys[1].private_key(), b"wrong")
        .payload()
        .to_vec();
    assert!(matches!(
        executor.complete_consensus_signature(id, wrong, &mut services),
        Err(EffectExecutorError::InvalidConsensusSignature(_))
    ));
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn broadcast_view_and_evidence_effects_reach_exact_hooks() {
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
    executor.runtime.round_tag = Some(tag(1));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_certificate(&fixture),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("consume the immediate view transition");
    executor
        .consume_effects(
            vec![
                AdapterEffect::Broadcast(message),
                AdapterEffect::ReportEquivocation {
                    evidence: vote_equivocation_evidence(&fixture, 1),
                },
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.manifest.subject,
                    certificate: fixture.qc(wire::GlobalPhase::Prepare),
                },
            ],
            &mut services,
        )
        .expect("transfer exact outputs into lifecycle admission");
    assert_eq!(services.entered_views, vec![tag(1)]);
    assert!(services.broadcasts.is_empty());
    assert!(services.equivocations.is_empty());
    assert!(services.invalid_bodies.is_empty());
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 3);
}
#[test]
fn equivocation_reporting_rejects_a_mutated_non_conflicting_pair() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (first, mut second) = vote_equivocation_evidence(&fixture, 1)
        .into_vote_pair_for_test()
        .expect("vote evidence helper returns vote evidence");
    second.round = first.round;
    second.proposal_round = first.proposal_round;
    second.phase = first.phase;
    second.subject = first.subject;
    second.execution_commitment = first.execution_commitment;
    let effect = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(first, second),
    };
    let ownership = bound_test_effect_ownership(&effect, tag(0), 1);
    executor.runtime.exact_effect_ownership = Some((effect.clone(), ownership.clone()));
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("executor transfers evidence validation to the lifecycle output owner");
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(services.equivocations.is_empty());
    assert!(!executor.status().fail_closed);
    let key = *executor
        .pending_lifecycle_output_admissions
        .keys()
        .next()
        .expect("one parked equivocation owner");
    let _pending = executor
        .pending_lifecycle_output_admissions
        .remove(&key)
        .expect("transfer malformed evidence into lifecycle service settlement");
    assert!(matches!(
        executor.execute_lifecycle_output_service(&effect, &ownership, &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("ReportEquivocation carried invalid evidence")
    ));
    assert!(services.equivocations.is_empty());
}

fn authenticated_proposal_fetch_ownership(
    fixture: &Fixture,
    effect: &AdapterEffect,
    lifecycle_ordinal: u128,
) -> RuntimeEffectOwnership {
    let AdapterEffect::FetchBody {
        tag: replay_tag,
        round,
        subject,
        manifest: Some(manifest),
        certificate: None,
        ..
    } = effect
    else {
        panic!("authenticated Proposal replay requires one ordinary manifest Fetch")
    };
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) = proposal(fixture).payload else {
        unreachable!("Proposal fixture has one Proposal payload")
    };
    proposal.round = *round;
    proposal.proposer = fixture.context.leader(round.view);
    proposal.subject = *subject;
    proposal.manifest.clone_from(manifest);
    let mut ownership = bound_test_effect_ownership(effect, *replay_tag, lifecycle_ordinal);
    assert!(ownership.bind_authenticated_remote_proposal_replay_for_test(proposal, effect));
    ownership
}

#[test]
fn authenticated_chunk_reconstruction_rejection_retries_exact_proposal_fetch_nonfatally() {
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
    let ownership = authenticated_proposal_fetch_ownership(&fixture, &fetch, 9_018);
    executor.runtime.exact_effect_ownership = Some((fetch.clone(), ownership));
    executor
        .consume_effects(vec![fetch.clone()], &mut services)
        .expect("begin fetch");
    let fetch_task = services.fetch_tasks[0].clone();
    let work_id = fetch_task.id();
    services.reject_authenticated_chunks = true;
    let mut chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&fixture.manifest),
        index: 0,
        bytes: fixture.encoded_chunks[0].clone(),
        sender: 0,
        signature: Vec::new(),
    };
    chunk.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &chunk
            .signature_preimage(&fixture.context, &fixture.manifest)
            .expect("chunk preimage"),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        executor.accept_payload_chunk(
            work_id,
            chunk,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::BodyMismatch(
            "authenticated chunks reconstructed invalid or noncanonical body data"
        ))
    ));
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_fetches[&work_id].task, fetch_task);
    assert!(
        executor
            .body_pipeline_owners
            .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
    );
    assert!(matches!(
        executor
            .remote_proposal_replay
            .get(&(fixture.manifest.round, fixture.manifest.subject)),
        Some(RemoteProposalReplayStageV1::Fetch {
            work_id: replay_work_id,
            ..
        }) if *replay_work_id == work_id
    ));
    assert_eq!(
        services.fetch_tasks,
        vec![fetch_task.clone(), fetch_task.clone()]
    );
    assert_eq!(services.chunks, vec![work_id]);
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);

    let periodic_ownership = bound_test_effect_ownership(&fetch, tag(0), 9_019);
    assert!(
        periodic_ownership
            .exact_remote_proposal_fetch_replay(&fetch)
            .is_none()
    );
    executor.runtime.exact_effect_ownership = Some((fetch.clone(), periodic_ownership));
    assert_eq!(
        executor
            .consume_effects(vec![fetch], &mut services)
            .expect("periodic rediscovery retries the retained authenticated Fetch owner"),
        1
    );
    assert_eq!(services.fetch_tasks, vec![fetch_task.clone(); 3]);
    assert_eq!(executor.pending_fetches[&work_id].task, fetch_task);
    assert!(!executor.status().fail_closed);
}
#[test]
fn failed_view_cleanup_keeps_stale_fetch_and_requires_restart() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("admit prior-view body recovery");
    let before = executor.body_ownership_projection();
    services.fail_on = Some("cancel-fetch");
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::FailClosed(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources,
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("admit certified prior-view recovery");
    let request_hash = *executor
        .certified_work
        .keys()
        .next()
        .expect("certified request index");
    assert!(executor.outstanding_requests.cancel(request_hash));
    let before = executor.body_ownership_projection();
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: Some(prepare),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.protected_lock, None);
    assert!(services.cancelled_fetches.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn view_cleanup_second_cancellation_failure_commits_no_fetch_retirement() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let first_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let first_sources = certified_sources(&fixture, &first_prepare);
    let (second_subject, second_body) = distinct_body(&fixture);
    let second_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        second_subject,
        &second_body,
    );
    let mut second_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    second_prepare.subject = second_manifest.subject;
    let second_sources = certified_sources(&fixture, &second_prepare);
    executor
        .consume_effects(
            vec![
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: first_sources,
                    certificate: Some(first_prepare),
                },
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: second_manifest.round,
                    subject: second_manifest.subject,
                    manifest: Some(second_manifest),
                    certified_sources: second_sources,
                    certificate: Some(second_prepare),
                },
            ],
            &mut services,
        )
        .expect("admit two stale certified recoveries");
    assert_eq!(executor.pending_fetches.len(), 2);
    let first_work_id = services.fetch_tasks[0].id();
    let before = executor.body_ownership_projection();
    services.fail_on_call = Some(("cancel-fetch", 2));
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason))
            if reason.contains("cancel-fetch call 2 failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.cancelled_fetches, vec![first_work_id]);
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let chunk = signed_payload_chunk(&fixture);
    let sender = fixture.context.roster[0].validator.clone();
    let unknown = EffectWorkId::for_test(999);
    let exact_ownership = payload_chunk_ingress_ownership(&chunk, sender.clone());
    assert_eq!(
        executor.accept_payload_chunk_with_ingress_ownership(
            unknown,
            chunk.clone(),
            &sender,
            &exact_ownership,
            &mut services,
        ),
        Err(EffectTransportError::UnknownWork(unknown))
    );
    assert!(!executor.status().fail_closed);
    assert!(services.chunks.is_empty());
    let foreign_origin = fixture.context.roster[1].validator.clone();
    let swapped_ownership = payload_chunk_ingress_ownership(&chunk, foreign_origin);
    assert!(matches!(
        executor.accept_payload_chunk_with_ingress_ownership(
            unknown,
            chunk,
            &sender,
            &swapped_ownership,
            &mut services,
        ),
        Err(EffectTransportError::FailClosed(reason))
            if reason.contains("fair-ingress ownership")
    ));
    assert!(services.chunks.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn proposal_reconstruction_rejection_retries_and_preserves_hybrid_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let mut alternate_chunk = fixture.body.clone();
    alternate_chunk[0] ^= 1;
    let alternate_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        &alternate_chunk,
    );
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(alternate_manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ordinary_ownership =
        authenticated_proposal_fetch_ownership(&fixture, &ordinary_fetch, 9_020);
    executor.runtime.exact_effect_ownership = Some((ordinary_fetch.clone(), ordinary_ownership));
    executor
        .consume_effects(vec![ordinary_fetch], &mut services)
        .expect("proposal starts body acquisition");
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(alternate_manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    executor
        .consume_effects(vec![certified_fetch.clone()], &mut services)
        .expect("Prepare authority upgrades the authenticated Proposal fetch");
    let fetch_task = services
        .fetch_tasks
        .last()
        .expect("hybrid Fetch reaches the service")
        .clone();
    let work_id = fetch_task.id();
    let request_hash = HashOf::new(
        fetch_task
            .certified_request()
            .expect("hybrid Fetch owns one certified request"),
    );
    assert_eq!(
        executor
            .complete_body_reconstruction(
                &fetch_task,
                alternate_manifest,
                fixture.body.clone(),
                &mut services,
            )
            .expect("noncanonical proposal data is a recoverable remote rejection"),
        CompletionDisposition::Rejected
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_fetches[&work_id].task, fetch_task);
    assert_eq!(executor.certified_work.get(&request_hash), Some(&work_id));
    assert!(executor.outstanding_requests.contains(request_hash));
    assert!(executor.ready_bodies.is_empty());
    assert!(
        executor
            .body_pipeline_owners
            .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
    );
    assert!(matches!(
        executor
            .remote_proposal_replay
            .get(&(fixture.manifest.round, fixture.manifest.subject)),
        Some(RemoteProposalReplayStageV1::Fetch {
            work_id: replay_work_id,
            ..
        }) if *replay_work_id == work_id
    ));
    assert!(executor.runtime.completions.is_empty());
    assert_eq!(services.fetch_tasks.last(), Some(&fetch_task));
    assert_eq!(services.completed_reconstruction_fetches, vec![work_id]);
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);

    executor
        .consume_effects(vec![certified_fetch], &mut services)
        .expect("periodic certified rediscovery retries the same hybrid owner");
    assert_eq!(executor.pending_fetches[&work_id].task, fetch_task);
    assert_eq!(executor.certified_work.get(&request_hash), Some(&work_id));
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert!(!executor.status().fail_closed);
}

#[test]
fn certificate_only_fetch_rejects_chunks_and_retains_typed_response_path() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(&fixture, &prepare),
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("start certificate-only fetch with manifest authority");
    let task = services.fetch_tasks[0].clone();
    let work_id = task.id();
    let request_hash = HashOf::new(
        task.certified_request()
            .expect("certificate-only fetch owns one response request"),
    );
    let ownership_before = executor.body_ownership_projection();

    assert_eq!(
        executor.accept_payload_chunk(
            work_id,
            signed_payload_chunk(&fixture),
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::WrongFetchKind),
        "chunks without Proposal replay cannot manufacture certified lifecycle authority",
    );
    assert!(services.chunks.is_empty());
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(executor.certified_work.get(&request_hash), Some(&work_id));
    assert!(executor.outstanding_requests.contains(request_hash));

    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let responder = fixture.context.roster[0].validator.clone();
    assert!(matches!(
        executor
            .probe_certified_response_priority(&response, &responder)
            .expect("the untouched request still accepts its typed response"),
        CertifiedResponsePriorityProbe::PreflightRequired(candidate)
            if candidate.work_id() == work_id
                && candidate.request_hash() == request_hash
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn certified_body_response_carrier_swap_is_rejected_before_fetch_mutation() {
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
        .expect("certified fetch");
    let task = services.fetch_tasks[0].clone();
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let mut other = response.clone();
    other.body.push(0xFF);
    let responder = fixture.context.roster[0].validator.clone();
    let swapped_ownership = certified_response_ingress_ownership(&other, responder.clone());
    let response_envelope = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
    );
    let pending_before = executor.pending_fetches.clone();
    let certified_before = executor.certified_work.clone();
    let outstanding_before = executor.outstanding_requests.hashes();
    assert!(swapped_ownership.validate_exact());
    assert!(!swapped_ownership.matches_message(&BlockMessage::V2(response_envelope.clone())));
    assert!(
        !executor.can_admit_network_message_with_ingress_ownership(
            &response_envelope,
            &swapped_ownership,
        )
    );
    assert_eq!(executor.pending_fetches, pending_before);
    assert_eq!(executor.certified_work, certified_before);
    assert_eq!(executor.outstanding_requests.hashes(), outstanding_before);
    assert!(!executor.status().fail_closed);
}
#[test]
fn certified_response_priority_probe_is_read_only_and_detects_revalidation_drift() {
    let fixture = Fixture::new();
    assert!(fixture.body.len() > 1);
    let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, 1, 4));
    assert_eq!(executor.validated_certified_request_presence(), Ok(false));
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(&fixture, &prepare),
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("hybrid fetch");
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    executor
        .validate_lifecycle_ingress_selector_authority()
        .expect("healthy executor can classify an exact queue cut");
    let task = services.fetch_tasks[0].clone();
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let responder = fixture.context.roster[0].validator.clone();
    let ownership_before = executor.body_ownership_projection();
    let claims_before = executor.outstanding_requests.response_claim_count();
    let candidate = match executor
        .probe_certified_response_priority(&response, &responder)
        .expect("exact response authentication succeeds")
    {
        CertifiedResponsePriorityProbe::PreflightRequired(candidate) => candidate,
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(reason) => {
            panic!("exact outstanding response was classified non-priority: {reason:?}")
        }
        CertifiedResponsePriorityProbe::RecoveredPreflightRequired(_) => {
            panic!("ordinary outstanding response was classified as recovered")
        }
    };
    assert_eq!(candidate.context_id(), fixture.context.id());
    assert_eq!(candidate.height(), fixture.context.height);
    assert_eq!(candidate.work_id(), task.id());
    assert_eq!(candidate.fetch_tag(), tag(0));
    assert_eq!(candidate.request_hash(), response.request_hash);
    assert_eq!(candidate.response_hash(), HashOf::new(&response));
    assert_eq!(candidate.round(), fixture.manifest.round);
    assert_eq!(candidate.subject(), fixture.manifest.subject);
    assert_eq!(
        candidate.proposal_manifest_hash(),
        Some(HashOf::new(&fixture.manifest))
    );
    assert!(
        candidate
            .pending_effect_binding()
            .exactly_binds_adapter_effect(&task.adapter_effect())
    );
    assert_eq!(
        candidate.canonical_manifest_hash(),
        HashOf::new(&fixture.manifest)
    );
    assert_eq!(candidate.body_payload_hash(), Hash::new(&fixture.body));
    assert_eq!(
        candidate.claim_preflight(),
        &CertifiedBodyResponseClaimPreflight::Vacant
    );
    assert!(candidate.matches_authenticated_response(&response, &responder));
    assert_eq!(
        executor
            .revalidate_certified_response_priority_candidate(&candidate, &response, &responder,),
        Ok(true),
        "an unchanged executor must reproduce the exact opaque candidate"
    );
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before,
        "the read-only probe cannot acquire the response family"
    );
    let mut unsolicited = response.clone();
    unsolicited.request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"unsolicited certified response priority probe"));
    assert!(matches!(
        executor
            .probe_certified_response_priority(&unsolicited, &responder)
            .expect("unsolicited classification is not an authentication capability"),
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(
            CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
        ) if request_hash == unsolicited.request_hash
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    let mut invalid = response.clone();
    invalid.signature[0] ^= 1;
    assert!(matches!(
        executor.probe_certified_response_priority(&invalid, &responder),
        Err(EffectTransportError::Authentication(
            V2TransportError::InvalidSignature { .. }
        ))
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    assert!(executor.runtime.completions.is_empty());
    assert!(!executor.status().fail_closed);
    let authenticated = executor
        .outstanding_requests
        .authenticate_response(&fixture.context, response.clone(), &responder)
        .expect("authenticate exact setup claim");
    assert_eq!(
        executor
            .outstanding_requests
            .prepare_authenticated_response_claim(&authenticated)
            .expect("prepare exact setup claim")
            .commit(),
        super::super::v2_transport::CertifiedBodyResponseClaimDisposition::Acquired
    );
    assert_eq!(
        executor
            .revalidate_certified_response_priority_candidate(&candidate, &response, &responder,),
        Ok(false),
        "Vacant-to-retransmission family drift must change the opaque candidate"
    );
    executor.output_guard.activate_restart_required();
    assert!(matches!(
        executor.validate_lifecycle_ingress_selector_authority(),
        Err(EffectTransportError::FailClosed(_))
    ));
}
#[test]
fn recovered_decision_fetch_fences_later_ordinary_body_coordinates() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let requester = PeerId::new(fixture.requester_key.public_key().clone());
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let sources = certified_sources(&fixture, &certificate);
    let mut request = wire::CertifiedBodyRequest {
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        certificate: certificate.clone(),
        requester: requester.clone(),
        signature: Vec::new(),
    };
    request.signature = Signature::new(
        fixture.requester_key.private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let authenticated = executor
        .authenticate_certified_body_request(request, &requester)
        .expect("authenticate the recovered request fixture");
    let key = crate::sumeragi::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1::for_height_context_test(
        &fixture.context,
        41,
        0xD1,
    );
    let owner =
        RecoveredDecisionFetchRequestOwnerV1::for_test(key, tag(0), sources.clone(), authenticated);
    assert!(owner.validates_exact_executor_context(&fixture.context, &requester));
    assert_eq!(
        executor.recovered_decision_fetch_registration_available(&owner),
        Ok(true),
        "an exact vacant recovered owner reports physical executor capacity"
    );
    let request_hash = owner.request_hash();
    assert!(
        executor
            .recovered_decision_fetches
            .insert(key, owner)
            .is_none()
    );
    assert!(
        executor
            .recovered_decision_fetch_by_request
            .insert(request_hash, key)
            .is_none()
    );
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    assert_eq!(
        executor.recovered_decision_fetch_registration_available(
            executor
                .recovered_decision_fetches
                .get(&key)
                .expect("installed recovered owner remains indexed"),
        ),
        Err(RecoveredDecisionFetchRequestRegistrationErrorV1::Occupied),
        "an existing dedicated owner is corruption/ownership, not capacity backpressure"
    );

    let ingress =
        crate::sumeragi::FairV2Ingress::new(32, 5 * 512 * 1024, 512 * 1024, 0, 512 * 1024);
    ingress
        .configure_roster(
            fixture
                .context
                .roster
                .iter()
                .map(|power| power.validator.clone()),
        )
        .expect("fixture roster fits the recovered selector ingress");
    ingress.state.lock().leader_wire_context = Some((fixture.context.id(), fixture.context.height));
    ingress.open().expect("open recovered selector ingress");
    let recovered_response = |responder_index: wire::ValidatorIndex| {
        let responder_index = usize::try_from(responder_index).expect("small responder index");
        let mut response = wire::CertifiedBodyResponse {
            request_hash,
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: fixture.context.roster[responder_index].validator.clone(),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[responder_index].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let mut ordinary_response = recovered_response(2);
    ordinary_response.request_hash = HashOf::from_untyped_unchecked(Hash::new(
        b"ordinary response ahead of recovered Decision Fetch",
    ));
    ordinary_response.signature = Signature::new(
        fixture.validator_keys[2].private_key(),
        &ordinary_response.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(ordinary_response.clone()),
            )),
            fixture.context.roster[2].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let ordinary_ordinal = ingress.state.lock().last_admission_ordinal;
    let mut recovered_ordinals = Vec::new();
    for responder in [0, 1] {
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(recovered_response(responder)),
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message,
                fixture.context.roster[usize::try_from(responder).expect("small responder index")]
                    .validator
                    .clone(),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        recovered_ordinals.push(ingress.state.lock().last_admission_ordinal);
    }
    let queue_depth_before_selector = ingress.len();
    let physical_cut_before_selector = ingress.next_physical_admission_ordinal();
    let recovered_keys_before_selector = executor
        .recovered_decision_fetches
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let recovered_index_before_selector = executor.recovered_decision_fetch_by_request.clone();
    let ownership_before_selector = executor.body_ownership_projection();
    let claims_before_selector = executor.outstanding_requests.response_claim_count();
    assert_eq!(
        executor
            .classify_selected_certified_response_priority_for_test(&ingress, ordinary_ordinal)
            .expect("classify an unrelated response family without mutation"),
        SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority,
    );
    assert_eq!(ingress.len(), queue_depth_before_selector);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before_selector,
    );
    assert_eq!(
        executor
            .recovered_decision_fetches
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        recovered_keys_before_selector,
    );
    assert_eq!(
        executor.recovered_decision_fetch_by_request,
        recovered_index_before_selector,
    );
    assert_eq!(
        executor.body_ownership_projection(),
        ownership_before_selector
    );
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before_selector,
    );
    assert!(
        executor
            .prepare_next_recovered_decision_fetch_ingress_selector(&ingress)
            .expect("ordinary fair head is a non-fatal lifecycle pass-through")
            .is_none(),
        "a later recovered response cannot leapfrog the ordinary fair winner",
    );
    assert_eq!(ingress.len(), queue_depth_before_selector);
    let drained_ordinary = ingress
        .try_recv_if_checked(|_| true)
        .expect("ordinary checked dequeue remains available")
        .expect("ordinary winner remains queued after lifecycle pass-through");
    assert_eq!(
        drained_ordinary
            .ingress_ownership()
            .expect("ordinary response retains physical ownership")
            .first
            .physical_admission_ordinal,
        ordinary_ordinal,
    );
    assert!(matches!(
        drained_ordinary.message(),
        BlockMessage::V2(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                    if response.request_hash == ordinary_response.request_hash
            )
    ));
    let [first_recovered_ordinal, second_recovered_ordinal] = recovered_ordinals.as_slice() else {
        panic!("two recovered response occurrences must be enqueued");
    };
    let first_recovered_ordinal = *first_recovered_ordinal;
    let second_recovered_ordinal = *second_recovered_ordinal;
    let recovered_depth_before_priority = ingress.len();
    let recovered_cut_before_priority = ingress.next_physical_admission_ordinal();
    assert_eq!(
        executor
            .classify_selected_certified_response_priority_for_test(
                &ingress,
                first_recovered_ordinal,
            )
            .expect("classify the exact recovered claimed response winner"),
        SelectedCertifiedResponsePriorityV1::RecoveredClaimed,
    );
    assert_eq!(
        executor
            .classify_selected_certified_response_priority_for_test(
                &ingress,
                second_recovered_ordinal,
            )
            .expect("classify the later recovered same-family duplicate"),
        SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority,
        "only the lowest recovered response occurrence may own the family",
    );
    assert_eq!(ingress.len(), recovered_depth_before_priority);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        recovered_cut_before_priority,
    );
    assert_eq!(
        executor
            .recovered_decision_fetches
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        recovered_keys_before_selector,
    );
    assert_eq!(
        executor.recovered_decision_fetch_by_request,
        recovered_index_before_selector,
    );
    assert_eq!(
        executor.body_ownership_projection(),
        ownership_before_selector
    );
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before_selector,
        "read-only recovered classification cannot acquire its response-family claim",
    );
    assert!(!executor.status().fail_closed);
    let mut selected = executor
        .prepare_next_recovered_decision_fetch_ingress_selector(&ingress)
        .expect("queue-owned recovered response selection remains exact")
        .expect("one recovered response family is selected");
    assert_eq!(
        selected.selected_cut_for_test().2,
        first_recovered_ordinal,
        "the queue-owned selector chooses the next fair exact family occurrence",
    );
    let target = selected
        .take_lifecycle_io_target()
        .expect("the selected target remains a recovered Fetch persistence carrier");
    assert_eq!(
        target.kind(),
        crate::sumeragi::v2_lifecycle_coordinator::LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
    );
    assert!(target.matches_recovered_decision_fetch_key(key));
    drop(selected);
    assert_eq!(ingress.len(), queue_depth_before_selector - 1);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before_selector,
        "queue-owned selector discovery cannot dequeue or renumber ingress",
    );

    let uncertified_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let uncertified_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&uncertified_effect),
        vec![executor.runtime.test_effect_ownership(&uncertified_effect)],
    )
    .expect("bind exact uncertified Fetch owner")
    .pop()
    .expect("one uncertified Fetch has one exact owner");
    let certified_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(certificate.clone()),
    };
    let certified_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_effect),
        vec![executor.runtime.test_effect_ownership(&certified_effect)],
    )
    .expect("bind exact certified Fetch owner")
    .pop()
    .expect("one certified Fetch has one exact owner");
    let ownership_before = executor.body_ownership_projection();
    let service_fetches_before = services.fetch_tasks.clone();
    let uncertified = executor.begin_fetch(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        Some(fixture.manifest.clone()),
        Vec::new(),
        None,
        uncertified_ownership.clone(),
        None,
        &mut services,
    );
    assert!(matches!(
        uncertified,
        Err(EffectExecutorError::Contract(reason))
            if reason == "body-fetch coordinates already have a recovered Decision Fetch owner"
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(services.fetch_tasks, service_fetches_before);
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    let certified = executor.begin_fetch(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        None,
        sources,
        Some(certificate),
        certified_ownership,
        None,
        &mut services,
    );
    assert!(matches!(
        certified,
        Err(EffectExecutorError::Contract(reason))
            if reason == "body-fetch coordinates already have a recovered Decision Fetch owner"
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(services.fetch_tasks, service_fetches_before);
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    let collision_id = EffectWorkId::for_test(73);
    executor.pending_fetches.insert(
        collision_id,
        PendingFetch {
            task: BodyFetchTask {
                id: collision_id,
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                sources: Vec::new(),
                certified_request: None,
                ownership: uncertified_ownership,
            },
            request_hash: None,
        },
    );
    assert!(matches!(
        executor.validated_certified_request_presence(),
        Err(EffectTransportError::Authentication(
            V2TransportError::InconsistentRequestIndex(hash)
        )) if hash == request_hash
    ));
}
#[test]
fn selected_certified_response_priority_routes_only_physical_family_winners_read_only() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind production Fetch ownership for read-only classification");
    fixture
        .executor
        .consume_effects(effects, &mut services)
        .expect("hybrid fetch establishes one exact ordinary response family");
    let task = services.fetch_tasks[0].clone();
    let response = |responder_index: wire::ValidatorIndex| {
        let responder_index = usize::try_from(responder_index).expect("small responder index");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(
                task.certified_request()
                    .expect("classified Fetch owns its signed request"),
            ),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: fixture.context.roster[responder_index].validator.clone(),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[responder_index].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let ingress =
        crate::sumeragi::FairV2Ingress::new(32, 5 * 512 * 1024, 512 * 1024, 0, 512 * 1024);
    ingress
        .configure_roster(
            fixture
                .context
                .roster
                .iter()
                .map(|power| power.validator.clone()),
        )
        .expect("fixture roster fits the classifier ingress");
    ingress.state.lock().leader_wire_context = Some((fixture.context.id(), fixture.context.height));
    ingress.open().expect("open classifier ingress");
    let mut ordinals = Vec::new();
    for responder in [0, 1] {
        let response = response(responder);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                )),
                fixture.context.roster[usize::try_from(responder).expect("small responder index")]
                    .validator
                    .clone(),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        ordinals.push(ingress.state.lock().last_admission_ordinal);
    }
    let [first_ordinal, second_ordinal] = ordinals.as_slice() else {
        panic!("two exact ordinary response occurrences must be queued");
    };
    let queue_depth_before = ingress.len();
    let physical_cut_before = ingress.next_physical_admission_ordinal();
    let pending_before = fixture.executor.pending_fetches.clone();
    let certified_before = fixture.executor.certified_work.clone();
    let outstanding_before = fixture.executor.outstanding_requests.hashes();
    let claims_before = fixture.executor.outstanding_requests.response_claim_count();
    assert_eq!(
        fixture
            .executor
            .classify_selected_certified_response_priority_for_test(&ingress, *first_ordinal)
            .expect("classify the lowest exact ordinary response occurrence"),
        SelectedCertifiedResponsePriorityV1::OrdinaryClaimed,
    );
    assert_eq!(
        fixture
            .executor
            .classify_selected_certified_response_priority_for_test(&ingress, *second_ordinal)
            .expect("classify the later exact ordinary response duplicate"),
        SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority,
    );
    assert_eq!(ingress.len(), queue_depth_before);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before,
    );
    assert_eq!(fixture.executor.pending_fetches, pending_before);
    assert_eq!(fixture.executor.certified_work, certified_before);
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        outstanding_before,
    );
    assert_eq!(
        fixture.executor.outstanding_requests.response_claim_count(),
        claims_before,
    );
    assert!(!fixture.executor.status().fail_closed);
}
#[test]
#[allow(clippy::too_many_lines)]
fn certified_fetch_capacity_release_after_timeout_cleanup_retries_without_restart() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut effect_services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: fixture.executor.current_tag(),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind production Fetch ownership before timeout cleanup");
    fixture
        .executor
        .consume_effects(effects, &mut effect_services)
        .expect("admit one certified Fetch before timeout cleanup");
    let task = effect_services.fetch_tasks[0].clone();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(
            task.certified_request()
                .expect("the selected Fetch owns its signed request"),
        ),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: fixture.context.roster[0].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let (_ingress_directory, ingress, _ingress_gate) = fixture.bound_certified_response_ingress();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
            )),
            fixture.context.roster[0].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let response_ordinal = ingress.state.lock().last_admission_ordinal;
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, response_ordinal)
        .expect("prepare the exact response before its Fetch becomes stale");
    let (_, _, _, _, _, _, wake_source) = prepared
        .certified_fetch_ready_authority_for_test()
        .expect("derive the selected response's sealed Fetch wake authority");

    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified owner context");
    let owner_directory = TempDir::new().expect("temporary lifecycle owner storage");
    let mut owner = crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::empty_owner_for_ingress_test(
        verified,
        &fixture.validator_keys[0],
        owner_directory.path(),
    );
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
        1,
    );
    production_services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    V2EffectServices::enqueue_body_fetch(&mut production_services, task.clone())
        .expect("install the exact certified-Fetch service owner");
    planner_io.saturate_consensus_prefix(&production_services);
    let registry_before_wait = owner.fetch_registry_snapshot_for_test();
    let wait = match owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    ) {
        Ok(ProductionIngressTurnPreparation::CapacityWait(wait)) => wait,
        Ok(ProductionIngressTurnPreparation::Queued(_)) => {
            panic!("a saturated Consensus prefix cannot admit Fetch persistence")
        }
        Err(_) => panic!("saturation must retain the selector in a capacity wait"),
    };
    assert_eq!(
        owner.fetch_wait_projection_for_test(1, wake_source),
        (None, None, None, false),
        "ordinary capacity waiting must not create durable Fetch admission",
    );
    assert_eq!(
        owner.fetch_registry_snapshot_for_test(),
        registry_before_wait
    );

    let timeout_applied_at = Instant::now();
    fixture
        .executor
        .arm_live_clocks(
            crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            timeout_applied_at,
        )
        .expect("arm pacemaker before applying the authenticated timeout certificate");
    let mode_before_timeout = fixture.executor.lifecycle_mode_rank_snapshot();
    let timeout_signers = vec![0, 1, 2];
    let timeout_preimage = wire::TimeoutVote {
        round: fixture.round,
        highest_prepare_qc: None,
        signer: timeout_signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = timeout_signers
        .iter()
        .map(|signer| {
            Signature::new(
                fixture.validator_keys[usize::try_from(*signer).expect("small timeout signer")]
                    .private_key(),
                &timeout_preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_share_refs = timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let timeout = wire::TimeoutCertificate {
        round: fixture.round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: timeout_signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&timeout_share_refs)
                .expect("aggregate authenticated timeout certificate"),
        }],
    };
    fixture
        .executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("admit the externally authenticated timeout certificate");
    assert!(matches!(
        fixture
            .executor
            .step(timeout_applied_at, &mut effect_services)
            .expect("the timeout certificate advances the view and retires stale Fetch"),
        EffectExecutorStep::Advanced { .. }
    ));
    assert!(fixture.executor.pending_fetches.is_empty());
    assert!(fixture.executor.certified_work.is_empty());
    assert!(fixture.executor.outstanding_requests.hashes().is_empty());
    assert_eq!(
        fixture.executor.lifecycle_mode_rank_snapshot(),
        mode_before_timeout,
        "a view-only transition does not change finality-completion debt",
    );

    planner_io.release_one_predecessor();
    let released_selector = match wait.retry(&production_services, &fixture.executor) {
        ProductionIngressCapacityRetry::Released(selector) => selector,
        ProductionIngressCapacityRetry::Pending(_) => {
            panic!("the advanced service generation must release the retained selector")
        }
        ProductionIngressCapacityRetry::RestartRequired => {
            panic!("view-only cleanup cannot turn an ordinary capacity wait into restart")
        }
    };
    planner_io.release_one_predecessor();
    let ingress_len_before_retry = ingress.len();
    let physical_cut_before_retry = ingress.next_physical_admission_ordinal();
    let retry = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        released_selector,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        retry,
        Err(ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionPreparation { .. })
    ));
    drop(retry);
    assert_eq!(
        owner.fetch_wait_projection_for_test(1, wake_source),
        (None, None, None, false),
        "stale retry cannot leave a durable Fetch row or wake owner",
    );
    assert_eq!(
        owner.fetch_registry_snapshot_for_test(),
        registry_before_wait
    );
    assert_eq!(ingress.len(), ingress_len_before_retry);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before_retry,
    );
    assert!(!fixture.executor.output_guard.restart_required());
    assert!(!fixture.executor.status().fail_closed);
    planner_io.detach(&mut production_services);
}
#[test]
#[allow(clippy::too_many_lines)]
fn lifecycle_selector_capture_censuses_competing_response_family_exactly_once() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind production Fetch ownership for selector capture");
    fixture
        .executor
        .consume_effects(effects, &mut services)
        .expect("hybrid fetch establishes one exact response family");
    let task = services.fetch_tasks[0].clone();
    let response = |responder_index: wire::ValidatorIndex| {
        let responder_index = usize::try_from(responder_index).expect("small responder index");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(
                task.certified_request()
                    .expect("selector Fetch owns its signed request"),
            ),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: fixture.context.roster[responder_index].validator.clone(),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[responder_index].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let first_response = response(0);
    let second_response = response(1);
    assert_eq!(first_response.request_hash, second_response.request_hash);
    assert_ne!(HashOf::new(&first_response), HashOf::new(&second_response));
    let (_ingress_directory, ingress, ingress_gate) = fixture.bound_certified_response_ingress();
    let first_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(first_response.clone()),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            first_message,
            fixture.context.roster[0].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let first_ordinal = ingress.state.lock().last_admission_ordinal;
    let first_leader_wire_token =
        queued_leader_wire_ingress_token(&ingress, &ingress_gate, first_ordinal);
    let second_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(second_response.clone()),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            second_message,
            fixture.context.roster[1].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let second_ordinal = ingress.state.lock().last_admission_ordinal;
    let second_leader_wire_token =
        queued_leader_wire_ingress_token(&ingress, &ingress_gate, second_ordinal);
    assert!(first_ordinal < second_ordinal);
    assert_ne!(first_leader_wire_token.slot, second_leader_wire_token.slot);
    let queue_depth_before_priority = ingress.len();
    let physical_cut_before_priority = ingress.next_physical_admission_ordinal();
    let pending_before_priority = fixture.executor.pending_fetches.clone();
    let certified_before_priority = fixture.executor.certified_work.clone();
    let outstanding_before_priority = fixture.executor.outstanding_requests.hashes();
    let claims_before_priority = fixture.executor.outstanding_requests.response_claim_count();
    assert_eq!(
        fixture
            .executor
            .classify_selected_certified_response_priority_for_test(&ingress, first_ordinal)
            .expect("classify the exact ordinary claimed response winner"),
        SelectedCertifiedResponsePriorityV1::OrdinaryClaimed,
    );
    assert_eq!(
        fixture
            .executor
            .classify_selected_certified_response_priority_for_test(&ingress, second_ordinal)
            .expect("classify the later ordinary same-family duplicate"),
        SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority,
        "only the lowest physical occurrence may own the authenticated family",
    );
    assert_eq!(ingress.len(), queue_depth_before_priority);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before_priority,
    );
    assert_eq!(fixture.executor.pending_fetches, pending_before_priority);
    assert_eq!(fixture.executor.certified_work, certified_before_priority);
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        outstanding_before_priority,
    );
    assert_eq!(
        fixture.executor.outstanding_requests.response_claim_count(),
        claims_before_priority,
        "read-only ordinary classification cannot claim its response family",
    );
    assert!(!fixture.executor.status().fail_closed);
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, second_ordinal)
        .expect("complete selector preparation authenticates both occurrences");
    let (
        context,
        positions,
        selected_ordinal,
        physical_cut,
        selected_is_embedded,
        request_fence_active,
    ) = prepared.selected_cut_for_test();
    assert_eq!(context.height(), fixture.context.height);
    assert_eq!(selected_ordinal, second_ordinal);
    assert!(positions.into_iter().all(|position| position > 0));
    assert!(physical_cut > u128::from(second_ordinal));
    assert!(selected_is_embedded);
    assert!(request_fence_active);
    assert_eq!(prepared.verdict_count_for_test(), 2);
    assert_eq!(
        prepared.priority_owners_for_test(),
        &BTreeSet::from([first_ordinal]),
        "the leader-wire barrier leaves only the first physical family winner drainable",
    );
    assert_eq!(
        prepared.selector_debt_for_test(),
        1,
        "the blocked later duplicate owns no selector debt",
    );
    assert_eq!(
        prepared.claimed_family_winners_for_test(),
        BTreeMap::from([(first_response.request_hash, first_ordinal)]),
        "the lowest exact physical response wins the request family",
    );
    assert_eq!(
        prepared.certified_fetch_ready_authority_for_test(),
        Err(CertifiedFetchReadyPublicationError::InvalidSelectedOccurrence),
        "a blocked later retransmission cannot borrow the drainable winner's family authority",
    );
    drop(prepared);
    let winning_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact family winner remains an authenticated prepared target");
    let (
        wake_context,
        wake_ordinal,
        wake_physical_digest,
        wake_request_hash,
        wake_key,
        _causal_root,
        wake_source,
    ) = winning_prepared
        .certified_fetch_ready_authority_for_test()
        .expect("the selected family winner derives one sealed Fetch wake authority");
    let signed_request = task
        .certified_request()
        .expect("the selected family retains its exact signed request");
    assert_eq!(wake_context, context);
    assert_eq!(wake_ordinal, first_ordinal);
    assert_ne!(wake_physical_digest, LifecycleDigest::new([0; 32]));
    assert_eq!(wake_request_hash, first_response.request_hash);
    assert_eq!(wake_key.phase(), LifecyclePhase::Fetch);
    assert_eq!(
        wake_key.round().height(),
        signed_request.certificate.round.height
    );
    assert_eq!(
        wake_key.round().view(),
        signed_request.certificate.round.view
    );
    assert_eq!(
        wake_key
            .proposal_round()
            .map(|round| (round.height(), round.view())),
        Some((signed_request.round.height, signed_request.round.view))
    );
    assert!(matches!(wake_source, WaitSource::External(_)));
    let incumbent_effect = task.adapter_effect();
    let incumbent_pending = task
        .ownership()
        .exact_pending_adapter_effect_binding(&incumbent_effect)
        .expect("the real Fetch task mints its exact ordinal-free registry binding");
    let queue_depth_before = ingress.len();
    let next_physical_ordinal_before = ingress.next_physical_admission_ordinal();
    let (incumbent_digest, replacement_digest) = winning_prepared
        .certified_fetch_registry_preflight_for_test(incumbent_effect, incumbent_pending)
        .expect("the real selector winner crosses the sealed registry preflight");
    assert_ne!(incumbent_digest, replacement_digest);
    assert_eq!(replacement_digest, wake_physical_digest);
    assert_eq!(ingress.len(), queue_depth_before);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        next_physical_ordinal_before,
        "registry preflight cannot dequeue, append, or renumber ingress",
    );
    let reprobed_winner = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact queued winner remains selectable after registry preflight");
    assert_eq!(
        reprobed_winner.selected_cut_for_test(),
        winning_prepared.selected_cut_for_test(),
        "registry preflight leaves the complete queue-minted cut unchanged",
    );
    drop(reprobed_winner);
    assert_eq!(
        winning_prepared.selected_cut_for_test().2,
        first_ordinal,
        "deriving readiness borrows and preserves the complete prepared token",
    );
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified owner context");
    let owner_directory = TempDir::new().expect("temporary lifecycle owner storage");
    let mut owner =
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::empty_owner_for_ingress_test(
            verified,
            &fixture.validator_keys[0],
            owner_directory.path(),
        );
    let lifecycle_ordinal = 1;
    let lifecycle_source = wake_source;
    assert!(first_leader_wire_token.scheduler_ordinal() > lifecycle_ordinal);
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let before_foreign_sign_cursor =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    assert!(matches!(
        owner.dispatch_recovered_lifecycle_sign(
            &production_services,
            &fixture.executor,
            crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context,),
        ),
        Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_foreign_sign_cursor,
        "a non-Completion runner cursor cannot claim or mutate a recovered Sign owner",
    );
    let before_unbound = owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let unbound_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        winning_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        unbound_result,
        Err(ProductionIngressSchedulerInputsError::BodyStoreNotBound)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_unbound,
        "an owner retaining its startup store cannot plan against an independent service",
    );
    let foreign_output_guard = ConsensusOutputGuard::isolated();
    let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&foreign_output_guard),
        1,
    );
    let guard_mismatch_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact winner remains selectable for the guard mismatch");
    let before_guard_mismatch =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let guard_mismatch = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        guard_mismatch_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        guard_mismatch,
        Err(ProductionIngressSchedulerInputsError::ForeignOutputGuard)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_guard_mismatch,
        "a foreign service guard cannot advance the coordinator or claim a lease",
    );
    assert!(
        !fixture.executor.output_guard.restart_required(),
        "guard mismatch rejection leaves the executor's canonical output open",
    );
    assert!(
        !foreign_output_guard.restart_required(),
        "pre-capture mismatch rejection leaves the foreign service guard open",
    );
    planner_io.install_output_guard_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
    );
    production_services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    V2EffectServices::enqueue_body_fetch(&mut production_services, task.clone())
        .expect("install the exact certified-Fetch service owner for Phase B");
    planner_io.saturate_consensus_prefix(&production_services);
    let waiting_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact winner remains selectable for a capacity wait");
    let before_capacity_wait =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let registry_before_capacity_wait = owner.fetch_registry_snapshot_for_test();
    assert_eq!(
        before_capacity_wait,
        (None, None, None, false),
        "an empty owner has no durable Fetch admission before capacity capture",
    );
    let capacity_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        waiting_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    let capacity_wait = match capacity_result {
        Ok(ProductionIngressTurnPreparation::CapacityWait(wait)) => wait,
        Ok(ProductionIngressTurnPreparation::Queued(_)) => {
            panic!("a saturated Consensus prefix cannot admit Fetch persistence")
        }
        Err(_) => panic!("saturation must return the opaque capacity wait"),
    };
    assert_eq!(
        capacity_wait.capacity_status(&production_services),
        ProductionIngressCapacityStatus::Pending
    );
    let capacity_wait = match capacity_wait.retry(&production_services, &fixture.executor) {
        ProductionIngressCapacityRetry::Pending(wait) => wait,
        ProductionIngressCapacityRetry::Released(_) => {
            panic!("the unchanged saturated generation cannot release capacity")
        }
        ProductionIngressCapacityRetry::RestartRequired => {
            panic!("the exact unchanged service/executor owners cannot require restart")
        }
    };
    planner_io.release_one_predecessor();
    assert_eq!(
        capacity_wait.capacity_status(&production_services),
        ProductionIngressCapacityStatus::Released
    );
    let released_selector = match capacity_wait.retry(&production_services, &fixture.executor) {
        ProductionIngressCapacityRetry::Released(selector) => selector,
        ProductionIngressCapacityRetry::Pending(_) => {
            panic!("an advanced service generation must release the retained selector")
        }
        ProductionIngressCapacityRetry::RestartRequired => {
            panic!("an exact generation release cannot require restart")
        }
    };
    planner_io.release_one_predecessor();
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_capacity_wait,
        "capacity waiting cannot create or advance a durable Fetch admission",
    );
    assert_eq!(
        owner.fetch_registry_snapshot_for_test(),
        registry_before_capacity_wait,
        "capacity waiting cannot install the concrete Fetch carrier",
    );
    let mode = fixture.executor.lifecycle_mode_rank_snapshot();
    let runner =
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context);
    let planned = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        mode,
        released_selector,
        runner,
    );
    let queued = match planned {
        Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
        Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
            panic!("available exact capacity must not produce a capacity wait")
        }
        Err(_) => panic!("the exact locked Fetch transaction must publish its command"),
    };
    assert_eq!(queued.ordinal(), lifecycle_ordinal);
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (
            Some(LifecycleState::Waiting(wait)),
            Some(1),
            None,
            false,
        ) if wait.source() == lifecycle_source && wait.observed_generation() == 1
    ));
    assert_eq!(planner_io.queued_certified_fetch_count(), 1);
    let repeated = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the queued physical winner remains selectable before Phase B");
    let before_repeat = owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let repeated_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        repeated,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        repeated_result,
        Err(ProductionIngressSchedulerInputsError::InFlightSelectedWork { .. })
    ));
    drop(repeated_result);
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_repeat,
        "an in-flight exact command must reject before advancing Fetch generation",
    );
    assert_eq!(planner_io.queued_certified_fetch_count(), 1);
    assert!(!fixture.executor.output_guard.restart_required());
    let queue_depth_before_completion = ingress.len();
    let selected_strong_count_before_worker = ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.admission_ordinal == first_ordinal)
        .map(|entry| Arc::strong_count(&entry.inbound))
        .expect("the selected response remains queued before worker execution");
    assert_eq!(
        selected_strong_count_before_worker, 1,
        "all selector probes must release the selected response before worker execution",
    );
    planner_io.execute_one_certified_fetch(Arc::clone(&fixture.executor.output_guard));
    let completion = match production_services
        .take_next_lifecycle_completion()
        .expect("the persisted Fetch retains its exact physical completion owner")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::CertifiedFetch(completion) => {
            completion
        }
        _ => panic!("the persisted ordinary Fetch must classify as CertifiedFetch"),
    };
    owner
        .complete_certified_fetch_for_test(
            &mut fixture.executor,
            &mut production_services,
            &ingress,
            completion,
        )
        .unwrap_or_else(|error| match error {
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
                error,
            ) => panic!(
                "Phase B must publish Ready and retire the exact physical response: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
                error,
            ) => panic!(
                "Phase B lost its productive ingress before persistence: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                error,
            ) => panic!(
                "Phase B reached a restart-only persistence failure: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
                error,
            ) => panic!("Phase B lost its exact Runtime handoff after dequeue: {error}"),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                error,
            ) => panic!("Phase B failed after its persistence commit: {error}"),
        });
    assert_eq!(
        ingress.len(),
        queue_depth_before_completion - 1,
        "Phase B dequeues only the selected response occurrence",
    );
    assert_leader_wire_body_terminal(&ingress_gate, &first_leader_wire_token);
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (Some(LifecycleState::Ready), Some(2), None, false)
    ));
    assert!(matches!(
        production_services
            .take_next_lifecycle_completion()
            .expect("the completion FIFO remains valid after exact acknowledgement"),
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::None
    ));
    let body_key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = fixture
        .executor
        .durable_bodies
        .get(&body_key)
        .expect("Phase B joins the durable receipt into the executor catalog");
    assert_eq!(durable.manifest_hash(), HashOf::new(&fixture.manifest));
    assert!(
        fixture
            .executor
            .recovered_bodies
            .get(&body_key)
            .is_some_and(
                |(manifest, recovered)| manifest == &fixture.manifest && recovered == durable
            )
    );
    assert!(!fixture.executor.output_guard.restart_required());
    assert_eq!(
        queued_leader_wire_ingress_token(&ingress, &ingress_gate, second_ordinal),
        second_leader_wire_token,
        "winner terminalization cannot change the losing physical Ingress owner",
    );
    assert_leader_wire_body_terminal(&ingress_gate, &first_leader_wire_token);
    assert_eq!(
        fixture
            .executor
            .classify_selected_certified_response_priority_for_test(&ingress, second_ordinal)
            .expect("classify the stale losing response after its request owner is gone"),
        SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority,
        "a stale response with no outstanding family must route through ordinary dequeue",
    );
    assert_eq!(
        queued_leader_wire_ingress_token(&ingress, &ingress_gate, second_ordinal),
        second_leader_wire_token,
        "read-only stale classification cannot mutate the losing Ingress owner",
    );
    let (mut losing_response, disposition) = ingress
        .try_recv_if_checked_retiring_obsolete(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ..
                }) if response == &second_response
            )
        })
        .expect("ordinary selector preserves the losing response handoff")
        .expect("the losing exact response remains physically queued");
    assert_eq!(
        disposition,
        crate::sumeragi::FairV2IngressDequeueDisposition::Admit
    );
    let mut missing_ownership = losing_response.clone();
    assert!(missing_ownership.take_ingress_ownership().is_some());
    assert_eq!(
        certified_fetch_preledger_productive_ingress_token(&missing_ownership),
        Err(CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingOwnership),
    );
    assert_eq!(
        certified_fetch_postdequeue_runtime_receipt(&missing_ownership, &second_leader_wire_token,),
        Err(CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MissingOwnership),
    );
    let mut invalid_ownership = losing_response.clone();
    invalid_ownership.sender = fixture.context.roster[0].validator.clone();
    assert_eq!(
        certified_fetch_preledger_productive_ingress_token(&invalid_ownership),
        Err(CertifiedFetchPreLedgerProductiveIngressErrorV1::InvalidOwnership),
    );
    assert_eq!(
        certified_fetch_postdequeue_runtime_receipt(&invalid_ownership, &second_leader_wire_token,),
        Err(CertifiedFetchPostDequeueRuntimeHandoffErrorV1::InvalidOwnership),
    );
    assert_eq!(
        certified_fetch_preledger_productive_ingress_token(&losing_response),
        Err(CertifiedFetchPreLedgerProductiveIngressErrorV1::RuntimeAlreadyBound),
        "a canonically dequeued carrier cannot re-enter the pre-ledger productive stage",
    );
    let validated_losing_receipt =
        certified_fetch_postdequeue_runtime_receipt(&losing_response, &second_leader_wire_token)
            .expect("canonical dequeue installs the exact selected Runtime receipt");
    assert_eq!(validated_losing_receipt.token(), &second_leader_wire_token);
    assert_eq!(
        certified_fetch_postdequeue_runtime_receipt(&losing_response, &first_leader_wire_token,),
        Err(CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MismatchedRuntimeReceipt,),
        "a valid Runtime carrier cannot satisfy another physical token",
    );
    let ungated_dequeued = crate::sumeragi::fair_v2_ingress_admit_for_test(
        InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(second_response.clone()),
            )),
            fixture.context.roster[1].validator.clone(),
        ),
    );
    assert_eq!(
        certified_fetch_postdequeue_runtime_receipt(&ungated_dequeued, &second_leader_wire_token,),
        Err(CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MissingRuntimeReceipt),
        "ungated dequeue cannot fabricate a durable Runtime receipt",
    );
    let losing_ownership = losing_response
        .take_ingress_ownership()
        .expect("losing response retains its exact fair-ingress ownership");
    assert!(losing_ownership.validate_exact());
    assert_eq!(
        losing_ownership.leader_wire_token(),
        Some(&second_leader_wire_token)
    );
    let losing_runtime = losing_ownership
        .leader_wire_runtime_receipt()
        .expect("ordinary losing response crosses the canonical Runtime handoff");
    assert_eq!(losing_runtime.token(), &second_leader_wire_token);
    ingress
        .mark_leader_wire_volatile_terminal(losing_runtime)
        .expect("ordinary response retirement publishes one volatile terminal");
    assert_eq!(ingress.len(), 0);
    assert!(ingress_gate.exact_record_is_volatile_terminal_for_test(&second_leader_wire_token));
    assert_leader_wire_body_terminal(&ingress_gate, &first_leader_wire_token);
    planner_io.detach(&mut production_services);
}
