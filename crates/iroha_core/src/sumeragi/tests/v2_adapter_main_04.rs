#[test]
fn full_normal_deferred_lane_cannot_drop_absolute_timeout() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    // Leave the reducer waiting for a Prepare signature, then model a
    // saturated untrusted deferred lane. The absolute timeout is delivered
    // while that signature fence is active, exactly where it used to be
    // classified as normal traffic and silently discarded.
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xD2);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, proposed_subject))
        .expect("accept proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let round = manifest.round;
    adapter
        .body_available(tag, manifest)
        .expect("body available");
    let receipt = durable_body_receipt(&adapter, round, proposed_subject);
    adapter
        .body_stored(tag, round, proposed_subject, &receipt)
        .expect("body stored");
    let validated = ValidatedBodyReceipt::for_test(receipt);
    let sign = adapter
        .validation_succeeded(tag, round, proposed_subject, &validated)
        .expect("body valid")
        .into_effects();
    let sign_tag = match sign.as_slice() {
        [AdapterEffect::Sign { tag, .. }] => *tag,
        effects => panic!("unexpected validation effects: {effects:?}"),
    };

    let normal_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xD3),
        execution_commitment: execution_commitment(0xD3),
        signer: 1,
        signature: vec![0xD3],
    };
    let deferred_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(normal_vote.clone()),
        ))
        .expect("defer normal authenticated vote");
    assert_eq!(
        deferred_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let filler = adapter
        .deferred_inputs
        .front()
        .expect("normal vote is queued")
        .clone();
    assert_eq!(filler.priority, DeferredPriority::Normal);
    let mut saturated_inputs = VecDeque::from([filler.clone()]);
    for _ in 1..MAX_DEFERRED_INPUTS {
        let admission_capability = adapter
            .deferred_admission_ordinals
            .mint(filler.admission_capability.origin)
            .expect("each saturated fixture owns a distinct adapter admission");
        let mut distinct_filler = filler.clone();
        distinct_filler.admission_ordinal = admission_capability.ordinal;
        distinct_filler.admission_capability = admission_capability;
        saturated_inputs.push_back(distinct_filler);
    }
    adapter.deferred_inputs = saturated_inputs;

    let mut backpressured_vote = normal_vote;
    backpressured_vote.signer = 2;
    backpressured_vote.signature = vec![0xD4];
    let backpressured_key = IngressSemanticKey::Vote {
        round,
        phase: wire::GlobalPhase::Prepare,
        signer: 2,
    };
    let backpressured = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote.clone()),
        ))
        .expect("apply normal-lane backpressure");
    assert_eq!(
        backpressured.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        adapter
            .ingress_equivocations
            .contains_key(&backpressured_key)
    );
    assert!(
        !adapter.ingress_deliveries.contains_key(&backpressured_key),
        "admission without queue ownership must remain retryable"
    );

    adapter.deferred_inputs.pop_back();
    let retried = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote),
        ))
        .expect("retry after reserved ownership becomes available");
    assert_eq!(
        retried.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(adapter.ingress_deliveries.contains_key(&backpressured_key));
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);

    // Saturate the ordinary semantic table as well. TimeoutVote owns an
    // independent signer-bounded semantic slot, so it must still reach the
    // protected Busy-deferred partition instead of being rejected before
    // the reducer boundary.
    saturate_ordinary_semantic_history(&mut adapter, round);

    let timeout_vote = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xD5],
    };
    let timeout_key = IngressSemanticKey::TimeoutVote { round, signer: 1 };
    let deferred_timeout_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
        ))
        .expect("defer TimeoutVote through its protected class");
    assert_eq!(
        deferred_timeout_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(
        adapter
            .ingress_equivocations
            .get(&timeout_key)
            .is_some_and(|record| record.capacity_bypass),
        "current-view TimeoutVote must bypass saturated ordinary semantic capacity"
    );
    assert!(adapter.ingress_deliveries.contains_key(&timeout_key));
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { .. },
            priority: DeferredPriority::Progress,
            protected_progress: false,
            ..
        })
    ));
    assert_eq!(
        deferred_progress_class(
            adapter
                .deferred_progress_inputs
                .back()
                .expect("deferred TimeoutVote owns the progress lane")
        ),
        Some(DeferredProgressClass::TimeoutVote)
    );

    let timeout = adapter
        .timeout_elapsed(sign_tag)
        .expect("defer trusted absolute timeout");
    assert_eq!(
        timeout.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutElapsed { .. },
            priority: DeferredPriority::Completion,
            ..
        })
    ));

    let completed = adapter
        .signature_completed(sign_tag, vec![0xD2; 96])
        .expect("complete outstanding Prepare signature")
        .into_effects();
    assert!(completed.iter().all(|effect| !matches!(
        effect,
        AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }
    )));
    let timeout_effects = adapter
        .drain_deferred()
        .expect("service the absolute timeout as one deferred macro-step");
    let timeout_sign_tag = timeout_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            } => Some(*tag),
            _ => None,
        })
        .expect("absolute timeout starts the durable local TimeoutVote signature");
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the remote TimeoutVote remains owned while the local TimeoutVote signature fences the reducer"
    );

    adapter
        .signature_completed(timeout_sign_tag, vec![0xD6; 96])
        .expect("complete the local TimeoutVote signature");
    adapter
        .drain_deferred()
        .expect("service protected progress in its own macro-step");
    assert!(adapter.deferred_progress_inputs.is_empty());
}

#[test]
fn failed_ingress_conversion_rolls_back_registry_and_admission() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xE0);
    let valid = proposal(&adapter.wire_context, proposer, proposed_subject);
    let mut malformed = valid.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut malformed.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    proposal.justification = wire::ProposalJustification::Timeout(wire::TimeoutJustification {
        timeout_certificate: wire::TimeoutCertificate {
            round: proposal.round,
            groups: Vec::new(),
        },
        highest_prepare_qc: None,
    });

    let subject_count = adapter.registry.subjects.len();
    let manifest_count = adapter.registry.manifests.len();
    assert!(adapter.receive_verified(malformed).is_err());
    assert_eq!(adapter.registry.subjects.len(), subject_count);
    assert_eq!(adapter.registry.manifests.len(), manifest_count);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.active_subject.is_none());

    // The failed conversion did not poison the semantic key; the valid
    // proposal for the same leader and round is still admitted.
    assert!(matches!(
        adapter
            .receive_verified(valid)
            .expect("valid retry")
            .effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
}

#[cfg(feature = "bls")]
#[test]
fn authentication_rejects_valid_commitment_conflicts_without_mutating_adapter() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified = VerifiedHeightContext::genesis(context.clone(), pops).expect("verified context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("commitment-auth-safety.wal"),
        verified,
        None,
        reducer::Generation::new(1),
        [0x83; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let locally_validated_subject = subject(0x87);
    let locally_validated_payload = [0x87, 2];
    let locally_validated_manifest = encode_payload(
        &context,
        round,
        locally_validated_subject,
        &locally_validated_payload,
    )
    .expect("encode locally validated payload")
    .manifest()
    .clone();
    let (_, locally_validated_receipt) =
        validated_receipts_for_manifest(&context, &locally_validated_manifest);
    let locally_validated_commitment = locally_validated_receipt.execution_commitment();
    let wrong_unbound_commitment = execution_commitment(0x87);
    assert_ne!(wrong_unbound_commitment, locally_validated_commitment);
    let signed_vote = |execution_commitment| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: locally_validated_subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    let wrong_unbound_vote = signed_vote(wrong_unbound_commitment);
    let canonical_unbound_vote = signed_vote(locally_validated_commitment);
    let registry_before_unbound_votes = adapter.registry.clone();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert_registry_eq(&adapter.registry, &registry_before_unbound_votes);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    adapter
        .recover_validated_body(&locally_validated_manifest, &locally_validated_receipt)
        .expect("local deterministic validation establishes canonical commitment authority");
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote),
        ))
        .expect("the same signed canonical vote is admissible after local validation");
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    let bound_subject = subject(0x83);
    let canonical_commitment = execution_commitment(0x83);
    let conflicting_commitment = execution_commitment(0x84);
    let core_subject = adapter
        .registry
        .register_subject(bound_subject)
        .expect("register canonical subject");
    adapter
        .registry
        .register_execution_commitment(
            reducer::Round::new(round.height, round.view),
            core_subject,
            canonical_commitment,
        )
        .expect("bind canonical validated execution result");
    let retained_registry = adapter.registry.clone();
    let retained_equivocations = adapter.ingress_equivocations.clone();
    let retained_deliveries = adapter.ingress_deliveries.clone();
    let retained_queue_lengths = (
        adapter.deferred_completions.len(),
        adapter.deferred_progress_inputs.len(),
        adapter.deferred_inputs.len(),
    );

    let mut conflicting_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_vote.signer).expect("small signer")].private_key(),
        &conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(conflicting_vote.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let mut conflicting_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut conflicting_qc, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(conflicting_qc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let later_round = wire::ConsensusRound { view: 1, ..round };
    let mut cross_round_conflicting_vote = wire::Vote {
        round: later_round,
        proposal_round: later_round,
        signature: Vec::new(),
        ..conflicting_vote
    };
    cross_round_conflicting_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_conflicting_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_conflicting_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_conflicting_vote.clone());
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_conflicting_payload),
        None,
        "a same-subject cross-round conflict must drain instead of retaining fair-ingress ownership"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            cross_round_conflicting_payload,
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let mut cross_round_canonical_vote = wire::Vote {
        execution_commitment: canonical_commitment,
        signature: Vec::new(),
        ..cross_round_conflicting_vote
    };
    cross_round_canonical_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_canonical_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_canonical_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_canonical_vote);
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_canonical_payload),
        Some((later_round, bound_subject)),
        "the same commitment on another round remains unbound until exact-round validation"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(cross_round_canonical_payload,)),
        Err(AdapterError::MissingExecutionCommitment)
    ));

    let mut cross_round_conflict = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_conflict, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let mut cross_round_canonical = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        execution_commitment: canonical_commitment,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_canonical, &keys);
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_canonical),
        ))
        .expect("an unchanged re-proposal authenticates the same deterministic execution");

    let timeout_round = wire::ConsensusRound { view: 1, ..round };
    let timeout_preimage = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &timeout_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate valid timeout signatures");
    let mut conflicting_timeout_vote = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_timeout_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_timeout_vote.signer).expect("small signer")].private_key(),
        &conflicting_timeout_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(conflicting_timeout_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let conflicting_tc = wire::TimeoutCertificate {
        round: timeout_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(conflicting_qc.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: timeout_signature,
        }],
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(conflicting_tc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let proposal_round = wire::ConsensusRound { view: 2, ..round };
    let proposal_subject = bound_subject;
    let proposal_body = vec![0x83, 2];
    let proposal_manifest =
        encode_payload(&context, proposal_round, proposal_subject, &proposal_body)
            .expect("encode later-view proposal payload")
            .manifest()
            .clone();
    let proposer = context.leader(proposal_round.view);
    let mut conflicting_proposal = wire::Proposal {
        round: proposal_round,
        proposer,
        subject: proposal_subject,
        manifest: proposal_manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: conflicting_tc,
            highest_prepare_qc: Some(conflicting_qc.clone()),
        }),
        signature: Vec::new(),
    };
    conflicting_proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &conflicting_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let conflicting_proposal_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal),
    );
    // Exercise the read-only embedded-certificate compatibility walk
    // directly, then confirm ordinary ingress rejects the same
    // structurally valid proposal for its conflicting deterministic
    // execution result.
    let authenticated_conflicting_proposal =
        AuthenticatedConsensusMessage::for_test(conflicting_proposal_message.clone());
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(
            &authenticated_conflicting_proposal,
        ),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(conflicting_proposal_message),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let unbound_subject = subject(0x85);
    let mut unbound_qc_a = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: unbound_subject,
        execution_commitment: execution_commitment(0x85),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut unbound_qc_a, &keys);
    let mut unbound_qc_b = wire::QuorumCertificate {
        execution_commitment: execution_commitment(0x86),
        ..unbound_qc_a.clone()
    };
    authenticate_qc(&mut unbound_qc_b, &keys);
    let timeout_group = |highest_prepare_qc: wire::QuorumCertificate,
                         signers: Vec<wire::ValidatorIndex>| {
        let preimage = wire::TimeoutVote {
            round: timeout_round,
            highest_prepare_qc: Some(highest_prepare_qc.clone()),
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(highest_prepare_qc),
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate valid disjoint timeout group"),
        }
    };
    let mut conflicting_groups = vec![
        timeout_group(unbound_qc_a, vec![0, 1]),
        timeout_group(unbound_qc_b, vec![2, 3]),
    ];
    conflicting_groups.sort_by_key(|group| {
        group
            .highest_prepare_qc
            .as_ref()
            .map(wire::QuorumCertificate::as_ref)
    });
    let within_envelope_conflict = wire::TimeoutCertificate {
        round: timeout_round,
        groups: conflicting_groups,
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(within_envelope_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(
        !adapter
            .registry
            .execution_commitments
            .keys()
            .any(|(_, registered_subject)| *registered_subject
                == reducer::Subject::new(Hash::new(unbound_subject.encode()).into())),
        "within-envelope checking cannot bind either attacker commitment"
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    // Transport adapters authenticate their outer request/response
    // identities separately. The same read-only compatibility walk still
    // covers every embedded certificate before a transport payload is
    // unwrapped into reducer ingress.
    let certified_request = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
            round,
            subject: bound_subject,
            certificate: conflicting_qc.clone(),
            requester: context.roster[0].validator.clone(),
            signature: vec![0x83; 96],
        }),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&certified_request),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let commit_response = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"commitment-conflict-request",
                )),
                certificate: wire::QuorumCertificate {
                    phase: wire::GlobalPhase::Commit,
                    ..conflicting_qc
                },
                responder: context.roster[1].validator.clone(),
                signature: vec![0x84; 96],
            },
        ),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&commit_response),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    assert_registry_eq(&adapter.registry, &retained_registry);
    assert_eq!(adapter.ingress_equivocations, retained_equivocations);
    assert_eq!(adapter.ingress_deliveries, retained_deliveries);
    assert_eq!(
        (
            adapter.deferred_completions.len(),
            adapter.deferred_progress_inputs.len(),
            adapter.deferred_inputs.len(),
        ),
        retained_queue_lengths
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    let mut canonical_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: canonical_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    canonical_vote.signature = Signature::new(
        keys[usize::try_from(canonical_vote.signer).expect("small signer")].private_key(),
        &canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_vote),
        ))
        .expect("the exact canonical commitment remains authentically admissible");
    assert!(adapter.ingress_ready());
}

#[cfg(feature = "bls")]
#[test]
fn authenticated_ingress_verifies_individual_and_aggregate_bls() {
    let (context, keys, pops) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(12);
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
        .payload()
        .to_vec();
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
        &pops,
    )
    .expect("verify individual vote");

    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&refs)
            .expect("aggregate BLS votes"),
    };
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            certificate,
        )),
        &pops,
    )
    .expect("verify aggregate QC");
}

#[cfg(feature = "bls")]
#[test]
fn timeout_vote_installs_embedded_qc_before_forming_tc() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified_context =
        VerifiedHeightContext::genesis(context.clone(), pops).expect("verify context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("timeout-safety.wal"),
        verified_context,
        None,
        reducer::Generation::new(1),
        [0x33; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(13);
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let prepare_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &prepare_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate PrepareQC"),
    };
    let protected_payload = [13, 2];
    let manifest = encode_payload(&context, round, subject, &protected_payload)
        .expect("encode protected-body payload")
        .manifest()
        .clone();
    let core_manifest = adapter
        .registry
        .manifest_to_core(&manifest, &context)
        .expect("register protected-body manifest");
    let core_round = reducer::Round::new(round.height, round.view);
    let core_subject = core_manifest.subject();
    let original_tag = adapter.current_tag();

    let mut all_effects = Vec::new();
    for signer in 0_u32..3 {
        if signer == 2 {
            adapter.deferred_completions.push_back(DeferredInput {
                admission_ordinal: 1,
                admission_capability: DeferredAdmissionCapability::for_test(1),
                event: reducer::Event::BodyAvailable {
                    tag: original_tag,
                    round: core_round,
                    subject: core_subject,
                },
                completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: manifest.clone(),
                }),
                retag_authenticated_ingress: false,
                priority: DeferredPriority::Completion,
                protected_progress: false,
                admission: None,
                authenticated_wire_identity: None,
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        }
        let mut timeout = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer,
            signature: Vec::new(),
        };
        timeout.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer")].private_key(),
            &timeout.signature_preimage(),
        )
        .payload()
        .to_vec();
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
            ))
            .expect("authenticate self-contained timeout vote");
        all_effects.push(
            adapter
                .receive_authenticated(authenticated)
                .expect("ingest timeout vote")
                .into_effects(),
        );
    }
    let final_effects = all_effects.pop().expect("three timeout outcomes");

    assert_eq!(adapter.reducer.durable_state().current_view(), 1);
    assert!(adapter.reducer.durable_state().highest_prepare().is_some());
    assert!(final_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
            ..
        })
    )));
    assert!(
        final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::EnterView { .. }))
    );
    assert!(
        !final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::StoreBody { .. })),
        "old-generation BodyAvailable must not cross EnterView before executor rebinding"
    );
    let (rebound_tag, protected_lock) = final_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_lock,
                ..
            } => Some((*tag, protected_lock.as_ref())),
            _ => None,
        })
        .expect("view installation effect");
    assert_eq!(protected_lock, Some(&prepare));
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, round, subject },
            ..
        }) if *tag == original_tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        adapter.rebind_deferred_body_available(original_tag, rebound_tag, &manifest),
        1
    );
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, .. },
            ..
        }) if *tag == rebound_tag
    ));
    assert_eq!(
        adapter
            .retire_deferred_body_available(rebound_tag, &manifest)
            .expect("persist rebound completion retirement"),
        1
    );
    assert!(adapter.deferred_completions.is_empty());
}
