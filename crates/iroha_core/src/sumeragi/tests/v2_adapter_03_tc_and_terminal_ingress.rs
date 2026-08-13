#[test]
fn prelock_current_commit_is_readmitted_with_priority_neutral_service_identity() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xBE);
    let locked_execution_commitment = execution_commitment(0xBE);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let remote_commit =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: 1,
            signature: vec![0xBE],
        }));
    let authenticated = AuthenticatedConsensusMessage::for_test(remote_commit.clone());
    assert!(!adapter.authenticated_ingress_is_progress(&authenticated));
    let generation = adapter.current_tag().generation();
    let serviced_before = adapter.serviced_candidate_count_for_test();
    let premature = adapter
        .receive_authenticated(authenticated)
        .expect("deliver the current Commit before its Prepare lock is durable");
    assert_eq!(
        premature.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
    );
    let key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: 1,
    };
    let delivered = adapter
        .ingress_deliveries
        .get(&key)
        .expect("the pre-lock reducer delivery is recorded");
    assert_eq!(delivered.generation, generation);
    assert!(!delivered.locked_commit_progress);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        serviced_before,
        "authenticated policy discards cannot allocate candidate markers"
    );
    assert!(adapter.durable_serviced_candidates.is_empty());
    let wire::ConsensusMessageV2Payload::Vote(normal_vote) = &remote_commit.payload else {
        unreachable!("fixture is a Commit vote")
    };
    let normal_vote = adapter
        .registry
        .vote_to_core(normal_vote, &adapter.wire_context)
        .expect("project the marker-free pre-lock occurrence");
    let normal_candidate = adapter
        .serviced_candidate(
            &reducer::Event::VoteReceived {
                tag: adapter.current_tag(),
                vote: normal_vote,
            },
            DeferredPriority::Normal,
            None,
            None,
        )
        .expect("pre-lock occurrence still has an exact rank identity")
        .0;
    adapter.ingress_deliveries.remove(&key);
    adapter.ingress_equivocations.remove(&key);
    let marker_count = adapter.serviced_candidate_count_for_test();
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                remote_commit.clone(),
            ))
            .expect("marker-free policy discard remains reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
    );
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count,
        "same-class policy replay cannot consume a tombstone"
    );
    let prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAE; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare,
            )),
        ))
        .expect("observe the current PrepareQC");
    assert!(observed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == wire_round
            && *subject == locked_subject
            && certificate.phase == wire::GlobalPhase::Prepare
    )));
    let locked_payload = [0xBE, 2];
    let manifest = encode_payload(
        &adapter.wire_context,
        wire_round,
        locked_subject,
        &locked_payload,
    )
    .expect("encode the certified body payload")
    .manifest()
    .clone();
    let (durable, _) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let validated = ValidatedBodyReceipt::for_test_with_commitment(
        durable.clone(),
        locked_execution_commitment,
    );
    assert!(matches!(
        adapter
            .body_available(adapter.current_tag(), manifest.clone())
            .expect("make the certified body available")
            .effects(),
        [AdapterEffect::StoreBody { .. }]
    ));
    assert!(matches!(
        adapter
            .body_stored(adapter.current_tag(), wire_round, locked_subject, &durable,)
            .expect("acknowledge durable body storage")
            .effects(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let locked = adapter
        .validation_succeeded(
            adapter.current_tag(),
            wire_round,
            locked_subject,
            &validated,
        )
        .expect("persist the exact current LockAndCommit record");
    let commit_sign_tag = locked
        .effects()
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.round == wire_round
                && vote.phase == wire::GlobalPhase::Commit
                && vote.subject == locked_subject =>
            {
                Some(*tag)
            }
            _ => None,
        })
        .expect("durable lock acknowledgement authorizes the local Commit signature");
    assert_eq!(adapter.current_tag().generation(), generation);
    assert!(
        adapter.authenticated_ingress_is_progress(&AuthenticatedConsensusMessage::for_test(
            remote_commit.clone(),
        ))
    );
    let readmitted = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            remote_commit.clone(),
        ))
        .expect("re-admit the exact vote in the new lock consumer epoch");
    assert_eq!(
        readmitted.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    assert!(adapter.deferred_progress_inputs[0].protected_progress);
    let progress_input = &adapter.deferred_progress_inputs[0];
    let progress_candidate = adapter
        .serviced_candidate(
            &progress_input.event,
            progress_input.priority,
            progress_input.completion_evidence.as_ref(),
            progress_input.authenticated_wire_identity.as_deref(),
        )
        .expect("exact-lock Commit has a route-neutral service identity")
        .0;
    assert_eq!(
        progress_candidate.class(),
        ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS
    );
    assert_eq!(
        normal_candidate, progress_candidate,
        "the same authenticated Commit occurrence must coalesce across Normal/Progress routing"
    );
    let delivered = adapter
        .ingress_deliveries
        .get(&key)
        .expect("the exact-lock consumer owns the re-admitted vote");
    assert_eq!(delivered.generation, generation);
    assert!(delivered.locked_commit_progress);
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                remote_commit.clone(),
            ))
            .expect("coalesce behind exact deferred ownership")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    adapter
        .signature_completed(commit_sign_tag, vec![0xBF])
        .expect("self-admit the local Commit");
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    adapter
        .drain_deferred()
        .expect("give the deferred remote owner its serialized runtime turn");
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(
        adapter.serviced_candidates.contains_key(&normal_candidate),
        "the priority-neutral applied Commit remains coalesced for this process generation"
    );
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(
        adapter
            .status()
            .expect("post-lock status")
            .liveness
            .commit_quorums
            .iter()
            .any(|quorum| quorum.round == wire_round
                && quorum.subject == locked_subject
                && quorum.execution_commitment == locked_execution_commitment
                && quorum.signer_count == 2)
    );
    adapter.ingress_deliveries.remove(&key);
    adapter.ingress_equivocations.remove(&key);
    let marker_count = adapter.serviced_candidate_count_for_test();
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(remote_commit))
            .expect("monotone Commit-vote state suppresses replay after ingress reset")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.serviced_candidate_count_for_test(), marker_count);
}
#[test]
#[allow(clippy::too_many_lines)]
fn tc_reset_readmits_exact_locked_commit_once_per_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xD6);
    let locked_execution_commitment = execution_commitment(0xD6);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA6; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the durable PrepareQC");
    let round = prepare.round();
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new(
                core_context.id(),
                round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&lock_entry, &TestAggregator)
        .expect("encode the durable lock");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append the durable lock")
            .sequence(),
        0
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(1),
        [lock_entry],
    )
    .expect("recover the durable locked Commit intent");
    let replay_tag = adapter.reducer.current_tag();
    let replay = adapter
        .reducer
        .step(reducer::Event::ResumeAfterReplay { tag: replay_tag })
        .expect("resume the durable Commit intent");
    assert!(matches!(
        replay.effects(),
        [reducer::Effect::Sign {
            message: reducer::SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == reducer::Phase::Commit
    ));
    adapter
        .signature_completed(replay_tag, vec![0xB6])
        .expect("restore the local locked CommitVote");
    let locked_vote = |signer, marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer,
            signature: vec![marker],
        }))
    };
    let first = AuthenticatedConsensusMessage::for_test(locked_vote(1, 0xB7));
    assert!(adapter.authenticated_ingress_is_progress(&first));
    assert_eq!(
        adapter
            .receive_authenticated(first)
            .expect("deliver the original locked vote")
            .disposition(),
        reducer::StepDisposition::Applied
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("suppress a same-generation duplicate")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    let tag_before_tc = adapter.current_tag();
    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(wire_prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC6; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install the timeout certificate through the adapter");
    assert_eq!(installed.disposition(), reducer::StepDisposition::Applied);
    let installed_effects = installed.into_effects();
    let commit_sign_tag = installed_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.round == wire_round && vote.phase == wire::GlobalPhase::Commit => Some(*tag),
            _ => None,
        })
        .expect("TC installation must reconstruct the exact local locked Commit vote");
    assert!(adapter.current_tag().strictly_advances(tag_before_tc));
    assert_eq!(
        adapter.current_tag().generation(),
        reducer::Generation::INITIAL
    );
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 0);
    assert_eq!(
        adapter.active_subject,
        Some((round, core_subject)),
        "EnterView must restore the durable locked subject"
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("re-admit the exact locked vote after the pool reset")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the reset generation must own exactly one deferred remote vote"
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("coalesce a duplicate behind the reset-generation owner")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    adapter
        .signature_completed(commit_sign_tag, vec![0xB8])
        .expect("complete the reconstructed local vote");
    adapter
        .drain_deferred()
        .expect("drain remote ownership in its own macro-step");
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("suppress a second delivery in the reset generation")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    let liveness = adapter.status().expect("build liveness snapshot");
    liveness
        .validate()
        .expect("adapter liveness snapshot is structurally valid");
    assert_eq!(
        liveness.liveness.generation,
        adapter.current_tag().generation().get()
    );
    assert!(liveness.liveness.outbound_intents.iter().any(|intent| {
        intent.kind == wire::SumeragiV2OutboundIntentKind::CommitVote
            && intent.round == wire_round
            && intent.subject == Some(locked_subject)
            && intent.execution_commitment == Some(locked_execution_commitment)
            && intent.stage == wire::SumeragiV2OutboundIntentStage::Sent
    }));
    assert!(liveness.liveness.commit_quorums.iter().any(|quorum| {
        quorum.round == wire_round
            && quorum.subject == locked_subject
            && quorum.execution_commitment == locked_execution_commitment
            && quorum.signer_count == 2
    }));
    assert_eq!(liveness.liveness.queues.len(), 4);
    assert!(liveness.liveness.ignore_counts.iter().any(|entry| {
        entry.reason == wire::SumeragiV2IgnoreReason::Duplicate && entry.count >= 2
    }));
    let conflict =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0xD7),
            execution_commitment: execution_commitment(0xD7),
            signer: 1,
            signature: vec![0xD7],
        }));
    assert!(
        !adapter.authenticated_ingress_is_progress(&AuthenticatedConsensusMessage::for_test(
            conflict.clone()
        ))
    );
    let evidence = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(conflict.clone()))
        .expect("report the conflicting locked-round vote");
    let [AdapterEffect::ReportEquivocation { evidence }] = evidence.effects() else {
        panic!("conflicting locked-round vote must emit exact evidence")
    };
    let (first, second) = evidence
        .vote_pair()
        .expect("locked-round vote conflict carries a sealed vote pair");
    assert_eq!(first.signer, 1);
    assert_eq!(second.signer, 1);
    assert_eq!(first.round, wire_round);
    assert_eq!(second.round, wire_round);
    assert_ne!(first.signature_preimage(), second.signature_preimage());
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(conflict))
            .expect("cap repeated equivocation evidence")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    let penultimate = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
            2, 0xB8,
        )))
        .expect("deliver another locked vote");
    assert_eq!(penultimate.disposition(), reducer::StepDisposition::Applied);
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
            3, 0xB9,
        )))
        .expect("rebuild the old-round Commit quorum through the adapter");
    assert!(
        penultimate
            .effects()
            .iter()
            .chain(decided.effects())
            .any(|effect| matches!(
                effect,
                AdapterEffect::Apply { subject, certificate, .. }
                    if *subject == locked_subject
                        && certificate.round == wire_round
                        && certificate.phase == wire::GlobalPhase::Commit
            ) || matches!(
                effect,
                AdapterEffect::FetchBody {
                    subject,
                    certificate: Some(certificate),
                    ..
                } if *subject == locked_subject
                    && certificate.round == wire_round
                    && certificate.phase == wire::GlobalPhase::Commit
            ))
    );
    assert!(adapter.reducer.durable_state().decision().is_some());
    assert!(
        adapter.reducer.durable_state().locked().is_some(),
        "the retained Prepare lock makes this a post-decision admission regression"
    );
    // The local signer's vote was reconstructed from WAL rather than
    // admitted through network ingress, so this is a fresh semantic key
    // arriving only after Decision became durable.
    let decided_vote = locked_vote(0, 0xBA);
    let authenticated = AuthenticatedConsensusMessage::for_test(decided_vote.clone());
    assert!(!adapter.wire_ingress_may_use_progress(&decided_vote.payload));
    assert!(!adapter.authenticated_ingress_is_progress(&authenticated));
    let decided_key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: 0,
    };
    let protected_before = adapter
        .deferred_progress_inputs
        .iter()
        .filter(|input| input.protected_progress)
        .count();
    assert_eq!(
        adapter
            .receive_authenticated(authenticated)
            .expect("terminally ignore a new vote after Decision")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::AlreadyDecided)
    );
    for _ in 0..3 {
        // Model successive pool generations retaining the same semantic
        // delivery. Once Decision is durable, the old locked vote must be
        // height-long duplicate history rather than a per-generation retry.
        adapter
            .ingress_deliveries
            .get_mut(&decided_key)
            .expect("terminal AlreadyDecided delivery remains recorded")
            .generation = reducer::Generation::new(1);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    decided_vote.clone(),
                ))
                .expect("suppress the decided vote through the full ingress path")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
            "a durable Decision closes generation-scoped locked-vote retries"
        );
    }
    assert_eq!(
        adapter
            .deferred_progress_inputs
            .iter()
            .filter(|input| input.protected_progress)
            .count(),
        protected_before,
        "decided votes cannot consume protected deferred ownership"
    );
}
#[test]
fn terminal_ignored_ingress_is_recorded_before_duplicate_coalescing() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xBD),
        execution_commitment: execution_commitment(0xBD),
        signer: 1,
        signature: vec![0xBD],
    };
    let payload = wire::ConsensusMessageV2Payload::Vote(vote.clone());
    let (early, admission) = adapter
        .admit_authenticated_payload(&payload)
        .expect("admit a fresh authenticated vote");
    assert!(early.is_none());
    let admission = admission.expect("fresh vote owns semantic admission");
    let context = adapter.wire_context.clone();
    let core_vote = adapter
        .registry
        .vote_to_core(&vote, &context)
        .expect("convert the admitted vote");
    let stale_tag = reducer::EventTag::new(round.height, round.view, reducer::Generation::new(0));
    let ignored = adapter
        .step_authenticated_ingress(
            reducer::Event::VoteReceived {
                tag: stale_tag,
                vote: core_vote,
            },
            Some(admission),
        )
        .expect("terminally ignore the stale-generation delivery");
    assert_eq!(
        ignored.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::StaleGeneration)
    );
    let (duplicate, admission) = adapter
        .admit_authenticated_payload(&payload)
        .expect("coalesce the exact terminal retransmission");
    assert_eq!(
        duplicate
            .expect("terminal delivery is duplicate history")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert!(admission.is_none());
}
#[test]
fn deferred_zero_ordinal_is_exact_single_use_and_never_reminted() {
    let source = DeferredAdmissionOrdinalSource::new(0);
    let tag = reducer::EventTag::new(1, 0, reducer::Generation::new(1));
    let first =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Completion);
    let second =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Completion);
    assert_eq!(first.admission_ordinal, 0);
    assert_eq!(second.admission_ordinal, 1);
    assert!(first.validate_exact());
    assert!(first.belongs_to(&source));
    assert!(first.claim_adapter_service_for_test());
    assert!(!first.claim_adapter_service_for_test());
    assert!(first.claim_runtime_handoff_once());
    assert!(!first.claim_runtime_handoff_once());
    assert!(first.service_handoff_is_complete());
    assert!(second.claim_adapter_service_for_test());
    assert!(second.claim_runtime_handoff_once());
}
#[test]
fn deferred_projection_distinguishes_authenticated_proposal_origins() {
    let context_id = reducer::ContextId::repeat(0xA1);
    let finality_round = reducer::Round::new(7, 4);
    let origin_a = reducer::Round::new(7, 1);
    let origin_b = reducer::Round::new(7, 2);
    let subject = reducer::Subject::repeat(0xA2);
    let signer = reducer::ValidatorId::repeat(0xA3);
    let tag = reducer::EventTag::new(7, 4, reducer::Generation::new(1));
    let signature = reducer::OpaqueSignature::new(vec![0xA4]);
    let project = |event: reducer::Event| {
        let mut projection = Vec::new();
        append_deferred_projection_event(&mut projection, &event);
        projection
    };
    let signed_vote = |proposal_round| {
        reducer::SignedVote::new(
            reducer::Vote::new_with_proposal_round(
                context_id,
                finality_round,
                proposal_round,
                reducer::Phase::Commit,
                subject,
                signer,
            ),
            signature.clone(),
        )
    };
    assert_ne!(
        project(reducer::Event::VoteReceived {
            tag,
            vote: signed_vote(origin_a),
        }),
        project(reducer::Event::VoteReceived {
            tag,
            vote: signed_vote(origin_b),
        })
    );
    let certificate = |proposal_round| {
        reducer::QuorumCertificate::new(
            reducer::CertificateRef::new_with_proposal_round(
                context_id,
                finality_round,
                proposal_round,
                reducer::Phase::Commit,
                subject,
            ),
            vec![reducer::SignatureShare::new(signer, signature.clone())],
        )
    };
    assert_ne!(
        project(reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: certificate(origin_a),
        }),
        project(reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: certificate(origin_b),
        })
    );
    let proposal = |proposal_round| {
        reducer::SignedProposal::new(
            reducer::Proposal::new(
                context_id,
                reducer::Round::new(7, 0),
                signer,
                reducer::PayloadManifest::new(
                    subject,
                    reducer::Digest::repeat(0xA5),
                    reducer::Digest::repeat(0xA6),
                    1,
                    1,
                ),
                reducer::ProposalJustification::ParentCommit(Some(
                    reducer::CertificateRef::new_with_proposal_round(
                        context_id,
                        finality_round,
                        proposal_round,
                        reducer::Phase::Commit,
                        subject,
                    ),
                )),
            ),
            signature.clone(),
        )
    };
    assert_ne!(
        project(reducer::Event::ProposalReceived {
            tag,
            proposal: proposal(origin_a),
        }),
        project(reducer::Event::ProposalReceived {
            tag,
            proposal: proposal(origin_b),
        })
    );
}
#[test]
fn deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    adapter.deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(u128::MAX - 1);
    let context = adapter.wire_context.clone();
    let proposer = context.leader(0);
    let first = proposal(&context, proposer, subject(0xD1));
    let second = proposal(&context, proposer, subject(0xD2));
    let wire::ConsensusMessageV2Payload::Proposal(first) = first.payload else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second) = second.payload else {
        unreachable!("proposal fixture")
    };
    let tag = adapter.current_tag();
    adapter
        .defer_body_available_for_test(tag, &first.manifest)
        .expect("last safely advanceable ordinal is admitted");
    assert_eq!(
        adapter
            .deferred_completions
            .front()
            .expect("first owner remains queued")
            .admission_ordinal,
        u128::MAX - 1
    );
    assert!(matches!(
        adapter.defer_body_available_for_test(tag, &second.manifest),
        Err(AdapterError::DeferredAdmissionOrdinalExhausted)
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert_eq!(
        adapter.deferred_admission_ordinals.next_for_test(),
        u128::MAX,
        "exhaustion cannot wrap the actor source to a stale ordinal"
    );
}
#[test]
fn deferred_actor_source_never_aliases_across_adapter_instances() {
    let first_directory = TempDir::new().expect("first temporary directory");
    let second_directory = TempDir::new().expect("second temporary directory");
    let source = DeferredAdmissionOrdinalSource::new(0);
    let open = |directory: &TempDir, source: DeferredAdmissionOrdinalSource| {
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("shared-ordinal-safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(1),
            [0xD3; 32],
            fingerprints(),
            Box::new(TestAggregator),
            source,
        )
    };
    let (mut first, first_startup) =
        open(&first_directory, source.clone()).expect("open first adapter instance");
    let (mut second, second_startup) =
        open(&second_directory, source.clone()).expect("open second adapter instance");
    assert!(first_startup.is_empty());
    assert!(second_startup.is_empty());
    let first_context = first.wire_context.clone();
    let second_context = second.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(first_proposal) =
        proposal(&first_context, first_context.leader(0), subject(0xD4)).payload
    else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second_proposal) =
        proposal(&second_context, second_context.leader(0), subject(0xD5)).payload
    else {
        unreachable!("proposal fixture")
    };
    let first_tag = first.current_tag();
    let second_tag = second.current_tag();
    first
        .defer_body_available_for_test(first_tag, &first_proposal.manifest)
        .expect("first adapter instance admits owner zero");
    second
        .defer_body_available_for_test(second_tag, &second_proposal.manifest)
        .expect("second adapter instance advances the same actor source");
    let first_owner = first
        .pop_deferred_next()
        .expect("first adapter instance rank remains valid")
        .expect("first adapter instance returns exact owner")
        .evidence;
    let second_owner = second
        .pop_deferred_next()
        .expect("second adapter instance rank remains valid")
        .expect("second adapter instance returns exact owner")
        .evidence;
    assert_eq!(first_owner.admission_ordinal, 0);
    assert_eq!(second_owner.admission_ordinal, 1);
    assert_ne!(
        first_owner.admission_ordinal,
        second_owner.admission_ordinal
    );
    assert!(first_owner.belongs_to(&source));
    assert!(second_owner.belongs_to(&source));
    assert!(first_owner.validate_exact());
    assert!(second_owner.validate_exact());
}
#[test]
fn deferred_occurrence_capability_binds_direct_authenticated_provenance() {
    let directory = TempDir::new().expect("temporary occurrence-capability directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&context, context.leader(0), subject(0xD9)).payload
    else {
        unreachable!("proposal fixture")
    };
    let tag = adapter.current_tag();
    adapter
        .defer_authenticated_proposal_for_test(tag, &proposal)
        .expect("stage one authenticated Busy occurrence");
    let ordinals = adapter
        .all_deferred_admission_ordinals()
        .into_iter()
        .collect::<Vec<_>>();
    let [ordinal] = ordinals.as_slice() else {
        panic!("one exact Busy occurrence is retained")
    };
    let evidence = adapter
        .deferred_occurrence_ownership(*ordinal)
        .expect("snapshot the unclaimed adapter capability");
    assert!(evidence.validate_exact());
    assert_eq!(evidence.admission_ordinal(), *ordinal);
    assert!(evidence.is_authenticated_ingress());
    let mut reclassified = evidence;
    reclassified.authenticated_ingress = false;
    reclassified.projection_hash = deferred_occurrence_ownership_projection_hash(&reclassified);
    assert!(
        !reclassified.validate_exact(),
        "rehashing cannot detach provenance from the private admission capability"
    );
}
#[test]
fn deferred_service_evidence_rejects_every_owner_and_rank_mutation() {
    let source = DeferredAdmissionOrdinalSource::new(0);
    let foreign = DeferredAdmissionOrdinalSource::new(0);
    let tag = reducer::EventTag::new(7, 2, reducer::Generation::new(3));
    let evidence =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Normal);
    assert!(evidence.validate_exact());
    assert!(evidence.belongs_to(&source));
    assert!(!evidence.belongs_to(&foreign));
    let rejected = |mutated: DeferredServiceEvidence| {
        assert!(!mutated.validate_exact());
    };
    let mut mutated = evidence.clone();
    mutated.admission_ordinal = 1;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.priority = DeferredPriority::Progress;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.event_kind = DeferredEventKind::RetransmitElapsed;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.original_tag = reducer::EventTag::new(7, 3, reducer::Generation::new(3));
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.original_event = reducer::Event::RetransmitElapsed { tag };
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.protected_progress = true;
    mutated.projection_hash = deferred_service_projection_hash(&mutated);
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.eligible_skips_after = 1;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.service_cursor_after = DeferredPriority::Normal;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.service_cursor_before = DeferredPriority::Completion;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.queue_lengths_after.completion = 1;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.total_len_before = 2;
    rejected(mutated);
    let mut mutated = evidence.clone();
    mutated.retag = DeferredRetagRelation::AuthenticatedIngress { from: tag, to: tag };
    rejected(mutated);
    let mut mutated = evidence;
    mutated.projection_hash = Hash::new(b"wrong deferred projection");
    rejected(mutated);
}
#[test]
fn deferred_authenticated_retry_retains_exact_original_and_effective_tags() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&context, context.leader(0), subject(0xD6)).payload
    else {
        unreachable!("proposal fixture")
    };
    let authenticated_wire_identity = Arc::<[u8]>::from(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone()))
            .encode(),
    );
    let proposal = adapter
        .registry
        .proposal_to_core(&proposal, &context)
        .expect("convert authenticated proposal");
    let effective_tag = adapter.current_tag();
    let original_tag = reducer::EventTag::new(
        effective_tag.height(),
        effective_tag.view().saturating_add(1),
        effective_tag.generation(),
    );
    let admission_capability = adapter
        .mint_deferred_admission_ordinal(true)
        .expect("mint exact deferred owner");
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: admission_capability.ordinal,
        admission_capability,
        event: reducer::Event::ProposalReceived {
            tag: original_tag,
            proposal,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(authenticated_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    adapter.next_deferred_priority = DeferredPriority::Normal;
    let selection = adapter
        .pop_deferred_next()
        .expect("authenticated retry rank remains valid")
        .expect("select exact authenticated retry");
    assert!(selection.evidence.validate_exact());
    assert_eq!(selection.evidence.original_tag, original_tag);
    assert_eq!(selection.evidence.effective_tag, effective_tag);
    assert_eq!(
        selection.evidence.retag,
        DeferredRetagRelation::AuthenticatedIngress {
            from: original_tag,
            to: effective_tag,
        }
    );
    assert!(
        selection
            .evidence
            .matches_effective_event(&selection.input.event)
    );
    assert!(adapter.deferred_authenticated_event_matches_wire(&selection.evidence));
}
#[test]
fn authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(first) =
        proposal(&context, context.leader(0), subject(0xD7)).payload
    else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(other) =
        proposal(&context, context.leader(0), subject(0xD8)).payload
    else {
        unreachable!("proposal fixture")
    };
    let first = adapter
        .registry
        .proposal_to_core(&first, &context)
        .expect("convert retained authenticated proposal");
    let event = reducer::Event::ProposalReceived {
        tag: adapter.current_tag(),
        proposal: first,
    };
    assert!(matches!(
        adapter.enqueue_deferred(
            event.clone(),
            true,
            DeferredPriority::Normal,
            None,
            None,
            None,
        ),
        Err(AdapterError::RuntimeIngressOwnershipViolation)
    ));
    assert!(adapter.deferred_inputs.is_empty());
    let admission_capability = adapter
        .mint_deferred_admission_ordinal(true)
        .expect("mint exact deferred owner");
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: admission_capability.ordinal,
        admission_capability,
        event,
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::Proposal(other),
        )),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    adapter.next_deferred_priority = DeferredPriority::Normal;
    let durable_before = adapter.reducer.durable_state().clone();
    assert!(matches!(
        adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert_eq!(adapter.reducer.durable_state(), &durable_before);
    assert!(adapter.fail_closed);
}
#[test]
fn deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current = adapter.current_tag();
    let stale = reducer::EventTag::new(
        current.height().saturating_add(1),
        current.view(),
        current.generation(),
    );
    let capability = adapter
        .mint_deferred_admission_ordinal(false)
        .expect("mint exact adapter capability");
    let input = |tag| DeferredInput {
        admission_ordinal: capability.ordinal,
        admission_capability: capability.clone(),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority: DeferredPriority::Completion,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter.deferred_completions.push_back(input(stale));
    let (_, first) = adapter
        .drain_deferred_with_evidence()
        .expect("first exact capability crosses the adapter")
        .expect("first deferred owner is serviceable");
    assert!(first.adapter_service_is_claimed());
    assert!(!first.service_handoff_is_complete());
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::WrongHeight)
            .copied(),
        Some(1)
    );
    adapter
        .deferred_completions
        .push_back(input(reducer::EventTag::new(
            stale.height().saturating_add(1),
            stale.view(),
            stale.generation(),
        )));
    assert!(matches!(
        adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::WrongHeight)
            .copied(),
        Some(1),
        "the replay is rejected before a second reducer transition"
    );
    let foreign_directory = TempDir::new().expect("foreign temporary directory");
    let (mut foreign_adapter, foreign_startup) =
        open_test(&foreign_directory).expect("open foreign adapter");
    assert!(foreign_startup.is_empty());
    let foreign_source = DeferredAdmissionOrdinalSource::new(0);
    let foreign_capability = foreign_source
        .mint(DeferredAdmissionOrigin::LocalOrCausal)
        .expect("mint foreign capability");
    let foreign_tag = reducer::EventTag::new(
        foreign_adapter.current_tag().height().saturating_add(1),
        foreign_adapter.current_tag().view(),
        foreign_adapter.current_tag().generation(),
    );
    foreign_adapter
        .deferred_completions
        .push_back(DeferredInput {
            admission_ordinal: foreign_capability.ordinal,
            admission_capability: foreign_capability,
            event: reducer::Event::TimeoutElapsed { tag: foreign_tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
    assert!(matches!(
        foreign_adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert!(foreign_adapter.ignore_counts.is_empty());
}
#[test]
fn deferred_service_debt_counts_only_oldest_skipped_classes() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut next_ordinal = 1_u128;
    let mut input = |priority: DeferredPriority| {
        let admission_ordinal = next_ordinal;
        next_ordinal = next_ordinal
            .checked_add(1)
            .expect("small deferred fixture ordinal remains representable");
        DeferredInput {
            admission_ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(admission_ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        }
    };
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;
    let selected = adapter
        .pop_deferred_next()
        .expect("deferred service debt remains representable")
        .expect("completion receives its turn");
    assert_eq!(selected.evidence.priority, DeferredPriority::Completion);
    assert!(selected.evidence.validate_exact());
    assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
    assert_eq!(adapter.deferred_progress_inputs[0].eligible_skips, 1);
    assert_eq!(adapter.deferred_progress_inputs[1].eligible_skips, 0);
    assert_eq!(adapter.deferred_inputs[0].eligible_skips, 1);
    assert_eq!(adapter.deferred_inputs[1].eligible_skips, 0);
}
#[test]
fn deferred_selector_services_only_the_runtime_lifecycle_minimum_set() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let input = |ordinal, priority| DeferredInput {
        admission_ordinal: ordinal,
        admission_capability: DeferredAdmissionCapability::for_test(ordinal),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_completions
        .push_back(input(10, DeferredPriority::Completion));
    adapter
        .deferred_inputs
        .push_back(input(11, DeferredPriority::Normal));
    adapter
        .deferred_inputs
        .push_back(input(1, DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;
    let selection = adapter
        .pop_deferred_next_eligible(&BTreeSet::from([1]))
        .expect("lifecycle-filtered deferred selection remains exact")
        .expect("the runtime-minimal deferred owner is present");
    assert_eq!(selection.evidence.admission_ordinal, 1);
    assert_eq!(selection.evidence.priority, DeferredPriority::Normal);
    assert_eq!(
        selection.evidence.queue_lengths_before,
        DeferredQueueLengths {
            completion: 1,
            progress: 0,
            normal: 2,
        }
    );
    assert_eq!(
        selection.evidence.queue_lengths_after,
        DeferredQueueLengths {
            completion: 1,
            progress: 0,
            normal: 1,
        }
    );
    assert_eq!(
        selection.evidence.eligible_queue_lengths_before,
        DeferredQueueLengths {
            completion: 0,
            progress: 0,
            normal: 1,
        }
    );
    assert!(selection.evidence.validate_exact());
    assert!(
        selection
            .evidence
            .matches_eligible_admission_ordinals(&BTreeSet::from([1]))
    );
    assert!(
        !selection
            .evidence
            .matches_eligible_admission_ordinals(&BTreeSet::from([1, 10])),
        "the adapter seal binds the runtime's complete target-relative set"
    );
    let rejected = |mut evidence: DeferredServiceEvidence| {
        evidence.projection_hash = deferred_service_projection_hash(&evidence);
        assert!(
            !evidence.validate_exact(),
            "coherently rehashed eligible-selector weakening must fail"
        );
    };
    let mut wrong_cursor_class = selection.evidence.clone();
    wrong_cursor_class.eligible_queue_lengths_before.completion = 1;
    rejected(wrong_cursor_class);
    let mut missing_selected_owner = selection.evidence.clone();
    missing_selected_owner.eligible_queue_lengths_before.normal = 0;
    rejected(missing_selected_owner);
    let mut exceeds_total_class = selection.evidence.clone();
    exceeds_total_class.eligible_queue_lengths_before.progress = 1;
    rejected(exceeds_total_class);
    assert_eq!(adapter.deferred_completions[0].admission_ordinal, 10);
    assert_eq!(adapter.deferred_inputs[0].admission_ordinal, 11);
    assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
    assert_eq!(adapter.deferred_inputs[0].eligible_skips, 0);
    assert!(selection.evidence.claim_adapter_service_for_test());
    assert!(
        !selection.evidence.claim_adapter_service_for_test(),
        "the exact queue-selection capability crosses the adapter seam once"
    );
}
#[test]
fn deferred_service_debt_overflow_is_typed_and_fail_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let input = |ordinal, priority, eligible_skips| DeferredInput {
        admission_ordinal: ordinal,
        admission_capability: DeferredAdmissionCapability::for_test(ordinal),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips,
    };
    adapter
        .deferred_completions
        .push_back(input(1, DeferredPriority::Completion, 0));
    adapter
        .deferred_progress_inputs
        .push_back(input(2, DeferredPriority::Progress, u64::MAX));
    adapter.next_deferred_priority = DeferredPriority::Completion;
    assert!(matches!(
        adapter.pop_deferred_next(),
        Err(AdapterError::DeferredServiceDebtOverflow)
    ));
    assert!(adapter.fail_closed);
}
#[test]
fn deferred_service_cursor_cycles_nonempty_classes() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut next_ordinal = 1_u128;
    let mut input = |priority: DeferredPriority| {
        let admission_ordinal = next_ordinal;
        next_ordinal = next_ordinal
            .checked_add(1)
            .expect("small deferred fixture ordinal remains representable");
        DeferredInput {
            admission_ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(admission_ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        }
    };
    for priority in [
        DeferredPriority::Completion,
        DeferredPriority::Progress,
        DeferredPriority::Normal,
    ] {
        let queue = match priority {
            DeferredPriority::Completion => &mut adapter.deferred_completions,
            DeferredPriority::Progress => &mut adapter.deferred_progress_inputs,
            DeferredPriority::Normal => &mut adapter.deferred_inputs,
        };
        queue.push_back(input(priority));
        queue.push_back(input(priority));
    }
    adapter.next_deferred_priority = DeferredPriority::Completion;
    let selected = (0..6)
        .map(|_| {
            let selection = adapter
                .pop_deferred_next()
                .expect("deferred service debt remains representable")
                .expect("every nonempty class receives both turns");
            assert!(selection.evidence.validate_exact());
            selection.evidence.priority
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        vec![
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
        ]
    );
    assert!(
        adapter
            .pop_deferred_next()
            .expect("empty rank remains valid")
            .is_none()
    );
}
#[test]
fn deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current = adapter.current_tag();
    let stale = reducer::EventTag::new(
        current.height().saturating_add(1),
        current.view(),
        current.generation(),
    );
    let input = |priority: DeferredPriority| DeferredInput {
        admission_ordinal: priority.code().into(),
        admission_capability: DeferredAdmissionCapability::for_test(priority.code().into()),
        event: reducer::Event::TimeoutElapsed { tag: stale },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;
    for (turn, expected_lengths) in [
        (DeferredPriority::Completion, [0, 1, 1]),
        (DeferredPriority::Progress, [0, 0, 1]),
        (DeferredPriority::Normal, [0, 0, 0]),
    ] {
        assert!(adapter.deferred_work_is_serviceable());
        let before = adapter.deferred_completions.len()
            + adapter.deferred_progress_inputs.len()
            + adapter.deferred_inputs.len();
        assert!(
            adapter
                .drain_deferred()
                .expect("service one stale deferred transition")
                .is_empty()
        );
        let after = adapter.deferred_completions.len()
            + adapter.deferred_progress_inputs.len()
            + adapter.deferred_inputs.len();
        assert_eq!(before - after, 1, "{turn:?} owns exactly one turn");
        assert_eq!(
            [
                adapter.deferred_completions.len(),
                adapter.deferred_progress_inputs.len(),
                adapter.deferred_inputs.len(),
            ],
            expected_lengths,
            "the round-robin cursor selected {turn:?}"
        );
    }
    assert!(!adapter.deferred_work_is_serviceable());
}
#[test]
fn deferred_service_contract_violation_is_terminal() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    assert!(matches!(
        adapter.fail_deferred_service_contract(),
        AdapterError::DeferredServiceContractViolation
    ));
    assert!(adapter.fail_closed);
    assert!(matches!(
        adapter.drain_deferred(),
        Err(AdapterError::FailClosed)
    ));
}
#[test]
#[allow(clippy::too_many_lines)]
fn unowned_busy_prepare_certificate_rolls_back_staged_registry_and_active_subject() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let timeout_sign = adapter
        .timeout_elapsed(tag)
        .expect("start a local timeout signature fence");
    assert!(matches!(
        timeout_sign.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let qc = |phase, marker| wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase,
        subject: subject(marker),
        execution_commitment: execution_commitment(marker),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let deferred_prepare = qc(wire::GlobalPhase::Prepare, 0xE0);
    let deferred_prepare_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::QuorumCertificate(deferred_prepare.clone()),
    );
    let deferred_prepare = adapter
        .registry
        .qc_to_core(&deferred_prepare, &adapter.wire_context)
        .expect("convert PrepareQC lane fixture");
    adapter.deferred_progress_inputs.push_back(DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: deferred_prepare,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Progress,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(deferred_prepare_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    let outcome = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(qc(
                wire::GlobalPhase::Prepare,
                0xE3,
            )),
        ))
        .expect("apply PrepareQC-class backpressure");
    assert_eq!(
        outcome.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_progress_inputs, deferred_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.active_subject, active_subject_before);
}
#[test]
#[allow(clippy::too_many_lines)]
fn unowned_busy_exact_locked_vote_rolls_back_and_remains_retryable() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xE6);
    let locked_execution_commitment = execution_commitment(0xE6);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xE6; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the durable PrepareQC");
    let round = prepare.round();
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new(
                core_context.id(),
                round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&lock_entry, &TestAggregator)
        .expect("encode the durable lock");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append the durable lock")
            .sequence(),
        0
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(1),
        [lock_entry],
    )
    .expect("recover the durable locked Commit intent");
    let replay_tag = adapter.reducer.current_tag();
    let replay = adapter
        .reducer
        .step(reducer::Event::ResumeAfterReplay { tag: replay_tag })
        .expect("resume the durable Commit intent");
    assert!(matches!(
        replay.effects(),
        [reducer::Effect::Sign {
            message: reducer::SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == reducer::Phase::Commit
    ));
    let roster_len = adapter.wire_context.roster.len();
    let mut fillers = VecDeque::with_capacity(roster_len);
    for signer in 0..roster_len {
        let signer = u32::try_from(signer).expect("fixture signer fits u32");
        let wire_filler_vote = wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer,
            signature: vec![0xE7 ^ u8::try_from(signer).expect("fixture signer fits u8")],
        };
        let filler_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::Vote(wire_filler_vote.clone()),
        );
        let filler_vote = adapter
            .registry
            .vote_to_core(&wire_filler_vote, &adapter.wire_context)
            .expect("convert locked-vote capacity fixture");
        fillers.push_back(DeferredInput {
            admission_ordinal: u128::from(signer).saturating_add(1),
            admission_capability: DeferredAdmissionCapability::for_authenticated_test(
                u128::from(signer).saturating_add(1),
            ),
            event: reducer::Event::VoteReceived {
                tag: replay_tag,
                vote: filler_vote,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Progress,
            protected_progress: true,
            admission: None,
            authenticated_wire_identity: Some(filler_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
    }
    adapter.deferred_progress_inputs = fillers;
    let retried_signer = u32::try_from(
        roster_len
            .checked_sub(1)
            .expect("fixture roster is non-empty"),
    )
    .expect("fixture signer fits u32");
    let locked_vote =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: retried_signer,
            signature: vec![0xE8],
        }));
    let key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: retried_signer,
    };
    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    let backpressured = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone()))
        .expect("apply locked-vote-class backpressure");
    assert_eq!(
        backpressured.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        backpressured.requires_runtime_retry(),
        "a full lane retains no adapter owner and must re-expose the exact runtime command"
    );
    assert_eq!(adapter.deferred_progress_inputs, deferred_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.active_subject, active_subject_before);
    assert!(adapter.ingress_equivocations.contains_key(&key));
    assert!(
        !adapter.ingress_deliveries.contains_key(&key),
        "admission without locked-vote queue ownership must remain retryable"
    );
    adapter.deferred_progress_inputs.pop_back();
    let retried = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote))
        .expect("retry after locked-vote ownership becomes available");
    assert_eq!(
        retried.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(!retried.requires_runtime_retry());
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        adapter.wire_context.roster.len()
    );
    assert!(adapter.ingress_deliveries.contains_key(&key));
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::VoteReceived { .. },
            admission: Some(_),
            protected_progress: true,
            ..
        })
    ));
}
#[test]
fn deferred_progress_capacity_matches_partition_geometry() {
    assert_eq!(deferred_progress_capacity(0), 3);
    assert_eq!(deferred_progress_capacity(1), 5);
    assert_eq!(deferred_progress_capacity(4), 11);
    assert_eq!(
        deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT),
        MAX_DEFERRED_PROGRESS_INPUTS
    );
    assert_eq!(
        deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT.saturating_add(1)),
        MAX_DEFERRED_PROGRESS_INPUTS,
        "invalid oversized rosters cannot expand the static adapter bound"
    );
    assert_eq!(semantic_ingress_capacity(0), MAX_INGRESS_SEMANTIC_KEYS);
    assert_eq!(semantic_ingress_capacity(4), MAX_INGRESS_SEMANTIC_KEYS + 12);
    assert_eq!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE, 11);
    assert_eq!(
        BTreeSet::from(ServicedCandidateStage::ALL.map(|stage| stage as u8)).len(),
        SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        "the closed adapter-event projection has eleven distinct classes"
    );
    assert_eq!(
        serviced_candidate_capacity(4),
        (MAX_INGRESS_SEMANTIC_KEYS
            + 12
            + MAX_DEFERRED_INPUTS * 2
            + 11
            + MAX_DEFERRED_INPUTS * 4
            + MAX_DEFERRED_INPUTS
            + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
            + 1)
            * SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        "serviced identities cover active causal/effect/clock owners as well as service queues"
    );
    for roster_len in [0, 1, 4, wire::MAX_VALIDATORS_PER_HEIGHT] {
        assert_eq!(
            serviced_candidate_capacity(roster_len),
            candidate_lifecycle_capacity(roster_len, DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,)
                .saturating_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE),
            "the bound is the complete reviewed lifecycle geometry times the exact stage \
             carrier for roster size {roster_len}"
        );
    }
    let configured = ServicedCandidateCapacityGeometry::new(4_096, 777);
    assert_eq!(
        candidate_lifecycle_capacity(4, configured),
        semantic_ingress_capacity(4)
            + MAX_DEFERRED_INPUTS * 2
            + deferred_progress_capacity(4)
            + 4_096 * 4
            + 777
            + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
            + 1,
        "runtime and effect ownership are derived from the supplied production configuration"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn deferred_progress_partition_owns_every_vote_and_certificate_class() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let roster_len = adapter.wire_context.roster.len();
    let tag = adapter.current_tag();
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    for signer in 0..roster_len {
        let signer = u32::try_from(signer).expect("fixture signer fits u32");
        let marker = u8::try_from(signer).expect("fixture signer fits u8") | 0xA0;
        let locked_subject = subject(marker);
        let locked_commitment = execution_commitment(marker);
        let wire_vote = wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_commitment,
            signer,
            signature: vec![marker],
        };
        let vote_wire_identity =
            authenticated_wire_identity(wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()));
        let vote = adapter
            .registry
            .vote_to_core(&wire_vote, &adapter.wire_context)
            .expect("convert locked Commit capacity fixture");
        let admission = IngressAdmission {
            key: IngressSemanticKey::Vote {
                round: wire_round,
                phase: wire::GlobalPhase::Commit,
                signer,
            },
            fingerprint: IngressFingerprint::Vote(wire_round, locked_subject, locked_commitment),
            generation: tag.generation(),
            inserted_equivocation: false,
            locked_commit_progress: true,
        };
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::VoteReceived { tag, vote },
                    true,
                    DeferredPriority::Progress,
                    Some(admission),
                    None,
                    Some(vote_wire_identity),
                )
                .expect("admit one locked Commit owner per frozen validator")
                .is_some()
        );
        let wire_timeout = wire::TimeoutVote {
            round: wire_round,
            highest_prepare_qc: None,
            signer,
            signature: vec![marker ^ 0x0F],
        };
        let timeout_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire_timeout.clone()),
        );
        let timeout = adapter
            .registry
            .timeout_vote_to_core(&wire_timeout, &adapter.wire_context)
            .expect("convert TimeoutVote capacity fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::TimeoutVoteReceived { tag, vote: timeout },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(timeout_wire_identity),
                )
                .expect("admit one TimeoutVote owner per frozen validator")
                .is_some()
        );
        if signer == 0 {
            let retained = adapter.deferred_progress_inputs.clone();
            let wire_distinct_same_signer = wire::TimeoutVote {
                round: wire::ConsensusRound {
                    view: wire_round.view + 1,
                    ..wire_round
                },
                highest_prepare_qc: None,
                signer,
                signature: vec![marker ^ 0xF0],
            };
            let distinct_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire_distinct_same_signer.clone()),
            );
            let distinct_same_signer = adapter
                .registry
                .timeout_vote_to_core(&wire_distinct_same_signer, &adapter.wire_context)
                .expect("convert distinct same-signer TimeoutVote fixture");
            let distinct_same_signer = reducer::Event::TimeoutVoteReceived {
                tag,
                vote: distinct_same_signer,
            };
            assert!(
                adapter
                    .enqueue_deferred(
                        distinct_same_signer.clone(),
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(Arc::clone(&distinct_wire_identity)),
                    )
                    .expect("same signer cannot consume a second TimeoutVote slot")
                    .is_none(),
                "TimeoutVote ownership must be signer-injective before the class is full"
            );
            assert_eq!(
                adapter.deferred_progress_inputs, retained,
                "later same-signer traffic must not displace admitted progress"
            );
            let core_signer = adapter
                .registry
                .validator_id(signer)
                .expect("fixture signer belongs to the frozen roster");
            let owned_index = adapter
                .deferred_progress_inputs
                .iter()
                .position(|queued| {
                    deferred_progress_owner(queued)
                        == Some(DeferredProgressOwner::TimeoutVote(core_signer))
                })
                .expect("original same-signer TimeoutVote owns one slot");
            adapter.deferred_progress_inputs.remove(owned_index);
            assert!(
                adapter
                    .enqueue_deferred(
                        distinct_same_signer,
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(distinct_wire_identity),
                    )
                    .expect("same signer retries after its prior owner is serviced")
                    .is_some()
            );
        }
    }
    for (phase, marker) in [
        (wire::GlobalPhase::Prepare, 0xB0),
        (wire::GlobalPhase::Commit, 0xB1),
    ] {
        let certificate = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase,
            subject: subject(marker),
            execution_commitment: execution_commitment(marker),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let certificate_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
        );
        let certificate = adapter
            .registry
            .qc_to_core(&certificate, &adapter.wire_context)
            .expect("convert QC capacity fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::QuorumCertificateReceived { tag, certificate },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(certificate_wire_identity),
                )
                .expect("admit the independent QC class owner")
                .is_some()
        );
    }
    let timeout_certificate = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xB2; 96],
        }],
    };
    let timeout_certificate_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate.clone()),
    );
    let timeout_certificate = adapter
        .registry
        .tc_to_core(&timeout_certificate, &adapter.wire_context)
        .expect("convert TC capacity fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutCertificateReceived {
                    tag,
                    certificate: timeout_certificate,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(timeout_certificate_wire_identity),
            )
            .expect("admit the independent TC class owner")
            .is_some()
    );
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        deferred_progress_capacity(roster_len)
    );
    for (class, expected) in [
        (DeferredProgressClass::LockedCommitVote, roster_len),
        (DeferredProgressClass::TimeoutVote, roster_len),
        (DeferredProgressClass::PrepareCertificate, 1),
        (DeferredProgressClass::CommitCertificate, 1),
        (DeferredProgressClass::TimeoutCertificate, 1),
    ] {
        assert_eq!(
            adapter
                .deferred_progress_inputs
                .iter()
                .filter(|input| deferred_progress_class(input) == Some(class))
                .count(),
            expected,
            "each protected Progress class owns its exact partition"
        );
    }
    let retained = adapter.deferred_progress_inputs.clone();
    let later_round = wire::ConsensusRound {
        view: 1,
        ..wire_round
    };
    let overflow = wire::TimeoutVote {
        round: later_round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![0xBF],
    };
    let overflow_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutVote(overflow.clone()),
    );
    let overflow = adapter
        .registry
        .timeout_vote_to_core(&overflow, &adapter.wire_context)
        .expect("convert distinct TimeoutVote overflow fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutVoteReceived {
                    tag,
                    vote: overflow,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(overflow_wire_identity),
            )
            .expect("a full TimeoutVote partition rejects without displacement")
            .is_none()
    );
    assert_eq!(adapter.deferred_progress_inputs, retained);
}
#[test]
fn protected_locked_vote_uses_reserved_capacity_without_evicting_certificate_ownership() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let wire_timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xCA; 96],
        }],
    };
    let timeout_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_timeout.clone()),
    );
    let timeout = adapter
        .registry
        .tc_to_core(&wire_timeout, &adapter.wire_context)
        .expect("convert certificate lane fixture");
    let tag = adapter.current_tag();
    let certificate_input = DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::TimeoutCertificateReceived {
            tag,
            certificate: timeout,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Progress,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(timeout_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_progress_inputs
        .push_back(certificate_input.clone());
    assert!(
        adapter
            .deferred_progress_inputs
            .iter()
            .all(|input| progress_rank(&input.event) > 0)
    );
    let admitted_before = adapter.deferred_progress_inputs.clone();
    let wire_overflow_certificate = wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            view: wire_round.view + 1,
            ..wire_round
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xCB; 96],
        }],
    };
    let overflow_certificate_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_overflow_certificate.clone()),
    );
    let overflow_certificate = adapter
        .registry
        .tc_to_core(&wire_overflow_certificate, &adapter.wire_context)
        .expect("convert distinct certificate overflow fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutCertificateReceived {
                    tag,
                    certificate: overflow_certificate,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(overflow_certificate_wire_identity),
            )
            .expect("ordinary certificate overflow is rejected before admission")
            .is_none()
    );
    assert_eq!(
        adapter.deferred_progress_inputs, admitted_before,
        "equal-rank traffic must never replace already admitted certificate ownership"
    );
    let locked_subject = subject(0xDA);
    let locked_execution_commitment = execution_commitment(0xDA);
    let wire_vote = wire::Vote {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Commit,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signer: 1,
        signature: vec![0xDA],
    };
    let vote_wire_identity =
        authenticated_wire_identity(wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()));
    let vote = adapter
        .registry
        .vote_to_core(&wire_vote, &adapter.wire_context)
        .expect("convert protected locked vote fixture");
    let admission = IngressAdmission {
        key: IngressSemanticKey::Vote {
            round: wire_round,
            phase: wire::GlobalPhase::Commit,
            signer: 1,
        },
        fingerprint: IngressFingerprint::Vote(
            wire_round,
            locked_subject,
            locked_execution_commitment,
        ),
        generation: tag.generation(),
        inserted_equivocation: false,
        locked_commit_progress: true,
    };
    let protected_event = reducer::Event::VoteReceived { tag, vote };
    assert_eq!(progress_rank(&protected_event), 0);
    assert!(
        adapter
            .enqueue_deferred(
                protected_event,
                true,
                DeferredPriority::Progress,
                Some(admission),
                None,
                Some(vote_wire_identity),
            )
            .expect("protected ownership uses its reserved locked-vote capacity")
            .is_some()
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 2);
    assert_eq!(
        adapter
            .deferred_progress_inputs
            .iter()
            .filter(|input| input.protected_progress)
            .count(),
        1
    );
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::VoteReceived { .. },
            admission: Some(_),
            protected_progress: true,
            ..
        })
    ));
}
fn saturate_ordinary_semantic_history(
    adapter: &mut SumeragiV2Adapter,
    round: wire::ConsensusRound,
) {
    let ingress_context = adapter.wire_context.clone();
    for index in 0..MAX_INGRESS_SEMANTIC_KEYS {
        if adapter.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS {
            break;
        }
        let proposer = u32::MAX
            .checked_sub(u32::try_from(index).expect("semantic index fits u32"))
            .expect("fixture proposer remains in range");
        adapter.ingress_equivocations.insert(
            IngressSemanticKey::Proposal { round, proposer },
            IngressEquivocationRecord {
                fingerprint: IngressFingerprint::Proposal(Hash::new(index.to_le_bytes())),
                artifact: synthetic_ingress_proposal(&ingress_context, round, proposer, index),
                equivocation_reported: false,
                capacity_bypass: false,
                admitted_at: Instant::now(),
            },
        );
    }
    assert_eq!(
        adapter.ingress_equivocations.len(),
        MAX_INGRESS_SEMANTIC_KEYS
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current_tag = adapter.current_tag();
    let current_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: current_tag.view(),
    };
    let local_timeout = adapter
        .timeout_elapsed(current_tag)
        .expect("start the local TimeoutVote signature fence");
    assert!(matches!(
        local_timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        },]
    ));
    let timeout_certificate = wire::TimeoutCertificate {
        round: current_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC1; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
        ))
        .expect("authenticated TC bypasses the hung local signature");
    assert_eq!(installed.disposition(), reducer::StepDisposition::Applied);
    assert!(matches!(
        installed.effects(),
        [AdapterEffect::EnterView {
            tag,
            protected_lock: None,
            ..
        }] if tag.view() == current_round.view + 1
    ));
    assert_eq!(adapter.current_tag().view(), current_round.view + 1);
    assert!(adapter.deferred_progress_inputs.is_empty());
    let adjacent_round = wire::ConsensusRound {
        view: current_round
            .view
            .saturating_add(reducer::FUTURE_TIMEOUT_VOTE_LOOKAHEAD),
        ..current_round
    };
    let adjacent_vote = wire::TimeoutVote {
        round: adjacent_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xC3],
    };
    let adjacent_key = IngressSemanticKey::TimeoutVote {
        round: adjacent_round,
        signer: 1,
    };
    let applied = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(adjacent_vote.clone()),
        ))
        .expect("apply the adjacent vote after its view becomes current");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(adapter.ingress_deliveries.contains_key(&adjacent_key));
    let duplicate = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(adjacent_vote),
        ))
        .expect("coalesce the delivered adjacent TimeoutVote");
    assert_eq!(
        duplicate.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn busy_deferred_source_identity_coalesces_across_consumer_view_change() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let first_tag = adapter.current_tag();
    let first_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: first_tag.view(),
    };
    saturate_ordinary_semantic_history(&mut adapter, first_round);
    let first_timeout = adapter
        .timeout_elapsed(first_tag)
        .expect("start the first local TimeoutVote signature fence");
    let _first_sign_tag = match first_timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected first timeout effects: {effects:?}"),
    };
    let old_timeout = wire::TimeoutVote {
        round: first_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xD8],
    };
    let old_key = IngressSemanticKey::TimeoutVote {
        round: first_round,
        signer: 1,
    };
    let deferred_old = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout.clone()),
        ))
        .expect("defer the old-view TimeoutVote behind the signature fence");
    assert_eq!(
        deferred_old.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        adapter
            .ingress_equivocations
            .get(&old_key)
            .is_some_and(|record| record.capacity_bypass)
    );
    assert!(adapter.ingress_deliveries.contains_key(&old_key));
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    let old_input = adapter
        .deferred_progress_inputs
        .back()
        .expect("the old-view TimeoutVote owns the later Busy slot");
    let original_candidate = adapter
        .serviced_candidate(
            &old_input.event,
            old_input.priority,
            old_input.completion_evidence.as_ref(),
            old_input.authenticated_wire_identity.as_deref(),
        )
        .expect("authenticated TimeoutVote has a service identity");
    assert_eq!(original_candidate.1, first_round.view);
    assert_eq!(original_candidate.0.source_view(), first_round.view);
    assert_eq!(
        original_candidate.0.leader(),
        adapter.wire_context.leader(first_round.view)
    );
    let duplicate_old = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout),
        ))
        .expect("coalesce the exact deferred TimeoutVote");
    assert_eq!(
        duplicate_old.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    let timeout_certificate = wire::TimeoutCertificate {
        round: first_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD7; 96],
        }],
    };
    let enter_view = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
        ))
        .expect("authenticated TC bypasses the first signature fence");
    assert_eq!(enter_view.disposition(), reducer::StepDisposition::Applied);
    assert!(enter_view.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView { tag, .. } if tag.view() == 1
    )));
    assert_eq!(adapter.current_tag().view(), 1);
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "EnterView must leave the later old-view TimeoutVote owned until service"
    );
    let old_owner = adapter
        .registry
        .validator_id(1)
        .expect("fixture TimeoutVote signer belongs to the frozen roster");
    assert!(matches!(
        adapter.deferred_progress_inputs.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { vote, .. },
            ..
        }) if vote.vote().round().view() == 0
            && vote.vote().signer() == old_owner
    ));
    let old_input = adapter
        .deferred_progress_inputs
        .front()
        .expect("the old-view TimeoutVote remains owned");
    let retagged_event = old_input
        .event
        .clone()
        .retag_authenticated_ingress(adapter.current_tag());
    let retagged_candidate = adapter
        .serviced_candidate(
            &retagged_event,
            old_input.priority,
            old_input.completion_evidence.as_ref(),
            old_input.authenticated_wire_identity.as_deref(),
        )
        .expect("retagged TimeoutVote retains a service identity");
    assert_eq!(retagged_candidate.0, original_candidate.0);
    assert_eq!(retagged_candidate.0.source_view(), first_round.view);
    assert_eq!(retagged_candidate.1, adapter.current_tag().view());
    assert_ne!(
        retagged_candidate.0.leader(),
        adapter.wire_context.leader(retagged_candidate.1),
        "logical leader ownership derives from source view, not the consumer episode"
    );
    assert_ne!(
        original_candidate.1, retagged_candidate.1,
        "the consumer episode advanced while semantic source identity stayed fixed"
    );
    assert!(
        !adapter.ingress_equivocations.contains_key(&old_key)
            && !adapter.ingress_deliveries.contains_key(&old_key),
        "a capacity-bypass TimeoutVote record must retire when its view is no longer current"
    );
    let second_tag = adapter.current_tag();
    let second_timeout = adapter
        .timeout_elapsed(second_tag)
        .expect("start the current-view TimeoutVote signature fence");
    let second_sign_tag = match second_timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected second timeout effects: {effects:?}"),
    };
    let second_round = wire::ConsensusRound {
        view: second_tag.view(),
        ..first_round
    };
    let current_timeout = wire::TimeoutVote {
        round: second_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xDA],
    };
    let current_key = IngressSemanticKey::TimeoutVote {
        round: second_round,
        signer: 1,
    };
    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    for attempt in 0..2 {
        let blocked = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
            ))
            .expect("same-owner TimeoutVote remains retryable before service");
        assert_eq!(
            blocked.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy),
            "pre-service attempt {attempt} must not be poisoned as a duplicate"
        );
        assert_eq!(adapter.deferred_progress_inputs, deferred_before);
        assert_registry_eq(&adapter.registry, &registry_before);
        assert_eq!(adapter.active_subject, active_subject_before);
        assert!(
            adapter
                .ingress_equivocations
                .get(&current_key)
                .is_some_and(|record| record.capacity_bypass)
        );
        assert!(!adapter.ingress_deliveries.contains_key(&current_key));
    }
    adapter
        .signature_completed(second_sign_tag, vec![0xDB; 96])
        .expect("complete the current-view signature");
    assert!(
        adapter
            .drain_deferred()
            .expect("service the old owner in its own macro-step")
            .is_empty()
    );
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(
        adapter.serviced_candidates.get(&original_candidate.0),
        None,
        "retagged authenticated policy discard remains marker-free"
    );
    let retained_count = adapter.serviced_candidate_count_for_test();
    adapter
        .record_serviced_candidate(Some(retagged_candidate), false, false, None)
        .expect("an exact same-episode source occurrence coalesces");
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        retained_count + 1,
        "a transient same-source projection remains owned until strict episode exit"
    );
    let applied = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
        ))
        .expect("retry the current-view TimeoutVote after service");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(adapter.ingress_deliveries.contains_key(&current_key));
    let duplicate = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout),
        ))
        .expect("coalesce the delivered current-view TimeoutVote");
    assert_eq!(
        duplicate.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
}
