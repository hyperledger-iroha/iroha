#[test]
fn tag_roundtrip_rejects_another_height() {
    let (context, _) = context();
    let tag = EventTag::new(1, 3, Generation::new(7));
    assert_eq!(round_for_tag(&context, tag).expect("round").view, 3);
    assert!(matches!(
        round_for_tag(&context, EventTag::new(2, 0, Generation::new(7))),
        Err(V2RunnerError::StaleTag)
    ));
}

fn proposal_owner(
    context: &wire::HeightContext,
    tag: EventTag,
    lock: Option<(u64, wire::BlockSubject)>,
    decided_subject: Option<wire::BlockSubject>,
) -> LocalProposalOwner {
    LocalProposalOwner {
        tag,
        locked_body: lock.map(|(view, subject)| {
            (
                wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view,
                },
                subject,
            )
        }),
        decided_subject,
    }
}

fn proposal_subject(label: &[u8]) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        payload_hash: Hash::new(&[label, b" payload"].concat()),
    }
}

#[test]
fn locked_body_recovery_is_independent_of_reproposal_gates() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(18));
    let subject = proposal_subject(b"nonleader locked-body recovery");
    let locked_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 3,
    };
    let leader = context.leader(tag.view());
    let nonleader = if leader == 0 { 1 } else { 0 };
    let directive =
        LocalProposalDirective::for_test(tag, leader, Some(locked_round), Some(subject), None);
    let owner = LocalProposalOwner::from(directive);
    let expected_request = Some((tag, locked_round, subject));

    for (local_validator, attempted, can_admit) in [
        (nonleader, None, true),
        (leader, Some(owner), true),
        (leader, None, false),
    ] {
        let plan = locked_body_recovery_plan(directive, local_validator, attempted, can_admit);
        assert_eq!(plan.request, expected_request);
        assert!(
            !plan.may_repropose,
            "body recovery must survive every local reproposal gate"
        );
    }

    let eligible = locked_body_recovery_plan(directive, leader, None, true);
    assert_eq!(eligible.request, expected_request);
    assert!(eligible.may_repropose);

    let decided = LocalProposalDirective::for_test(
        tag,
        leader,
        Some(locked_round),
        Some(subject),
        Some(subject),
    );
    assert_eq!(
        locked_body_recovery_plan(decided, leader, None, true),
        LockedBodyRecoveryPlan {
            request: None,
            may_repropose: false,
        }
    );
}

#[test]
fn same_tag_higher_lock_retires_all_local_proposal_owners() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(11));
    let subject_a = proposal_subject(b"local owner A");
    let subject_b = proposal_subject(b"local owner B");
    let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
    let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
    let now = Instant::now();
    let mut state = LocalProposalState {
        attempted: Some(owner_a),
        submitted: Some((owner_a, subject_a)),
        non_empty_retry: Some(owner_a),
        candidate_work_wait: Some(CandidateWorkWait {
            owner: owner_a,
            started_at: now,
            next_retry: now,
        }),
        pending_events: Some(PendingLocalEvents {
            owner: owner_a,
            subject: subject_a,
            events: Vec::new(),
        }),
        global_selection: None,
    };

    state.reconcile(owner_b);

    assert!(state.attempted.is_none());
    assert!(state.submitted.is_none());
    assert!(state.non_empty_retry.is_none());
    assert!(state.candidate_work_wait.is_none());
    assert!(state.pending_events.is_none());
}

#[test]
fn deferred_autonomous_work_timeout_arms_only_a_non_empty_retry() {
    let (context, _) = context();
    let owner = proposal_owner(
        &context,
        EventTag::new(context.height, 3, Generation::new(17)),
        None,
        None,
    );
    let started_at = Instant::now();
    let wait_bound = Duration::from_secs(2);
    let mut state = LocalProposalState::default();

    state.defer_candidate_work(owner, started_at, wait_bound);
    assert_eq!(state.non_empty_retry, None);
    assert!(
        state
            .candidate_work_wait
            .is_some_and(|wait| wait.owner == owner && wait.started_at == started_at)
    );

    let expired_at = started_at
        .checked_add(wait_bound)
        .expect("fixture wait deadline is representable");
    state.defer_candidate_work(owner, expired_at, wait_bound);
    assert_eq!(state.non_empty_retry, Some(owner));
    assert!(state.candidate_work_wait.is_none());

    state.defer_candidate_work(owner, expired_at, wait_bound);
    assert_eq!(
        state.non_empty_retry,
        Some(owner),
        "repeated timeout handling must retain the same single retry"
    );
    assert!(state.candidate_work_wait.is_none());

    assert!(state.retire_unsubmitted_non_empty_retry(owner));
    assert_eq!(state.non_empty_retry, None);
    state.defer_candidate_work(owner, expired_at, wait_bound);
    assert!(
        state.candidate_work_wait.is_some_and(|wait| {
            wait.owner == owner && wait.started_at == expired_at && wait.next_retry > expired_at
        }),
        "an unsubmitted retry must cross a fresh bounded observation window"
    );
    assert!(
        !state.retire_unsubmitted_non_empty_retry(owner),
        "a consumed retry cannot be retired twice"
    );
}

#[test]
fn first_same_subject_lock_preserves_pending_local_proposal_events() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(14));
    let subject = proposal_subject(b"first lock keeps local subject");
    let unlocked = proposal_owner(&context, tag, None, None);
    let locked = proposal_owner(&context, tag, Some((5, subject)), None);
    let mut state = LocalProposalState {
        attempted: Some(unlocked),
        submitted: Some((unlocked, subject)),
        pending_events: Some(PendingLocalEvents {
            owner: unlocked,
            subject,
            events: Vec::new(),
        }),
        ..LocalProposalState::default()
    };

    state.reconcile(locked);

    assert_eq!(state.attempted, Some(locked));
    assert_eq!(state.submitted, Some((locked, subject)));
    assert!(
        state
            .pending_events
            .as_ref()
            .is_some_and(|pending| { pending.owner == locked && pending.subject == subject })
    );
}

#[test]
fn higher_same_subject_lock_retires_prior_origin_work() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(15));
    let subject = proposal_subject(b"higher lock retires old origin");
    let lower = proposal_owner(&context, tag, Some((2, subject)), None);
    let higher = proposal_owner(&context, tag, Some((4, subject)), None);
    let mut state = LocalProposalState {
        attempted: Some(lower),
        submitted: Some((lower, subject)),
        pending_events: Some(PendingLocalEvents {
            owner: lower,
            subject,
            events: Vec::new(),
        }),
        ..LocalProposalState::default()
    };

    assert_ne!(lower, higher);
    state.reconcile(higher);

    assert!(state.attempted.is_none());
    assert!(state.submitted.is_none());
    assert!(state.pending_events.is_none());
}

#[test]
fn first_same_subject_lock_from_prior_view_retires_unlocked_work() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(16));
    let subject = proposal_subject(b"old-origin first lock");
    let unlocked = proposal_owner(&context, tag, None, None);
    let locked = proposal_owner(&context, tag, Some((4, subject)), None);
    let mut state = LocalProposalState {
        attempted: Some(unlocked),
        submitted: Some((unlocked, subject)),
        pending_events: Some(PendingLocalEvents {
            owner: unlocked,
            subject,
            events: Vec::new(),
        }),
        ..LocalProposalState::default()
    };

    state.reconcile(locked);

    assert!(state.attempted.is_none());
    assert!(state.submitted.is_none());
    assert!(state.pending_events.is_none());
}

#[test]
fn late_old_rejection_cannot_arm_non_empty_retry_for_replacement_lock() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(12));
    let subject_a = proposal_subject(b"rejected old A");
    let subject_b = proposal_subject(b"current B");
    let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
    let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
    let proposal_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    };
    let mut state = LocalProposalState {
        submitted: Some((owner_a, subject_a)),
        ..LocalProposalState::default()
    };

    assert_eq!(
        state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_a,),
        LocalValidationDisposition::Ignored
    );
    assert_eq!(state.non_empty_retry, None);

    state.submitted = Some((owner_b, subject_b));
    assert_eq!(
        state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
        LocalValidationDisposition::RetryNonEmpty
    );
    assert_eq!(state.non_empty_retry, Some(owner_b));

    state.submitted = Some((owner_b, subject_b));
    assert_eq!(
        state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
        LocalValidationDisposition::FatalNonEmpty
    );
}

#[test]
fn decision_retires_local_work_before_prepared_delivery() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 6, Generation::new(13));
    let subject = proposal_subject(b"decided proposal");
    let active = proposal_owner(&context, tag, Some((4, subject)), None);
    let decided = proposal_owner(&context, tag, Some((4, subject)), Some(subject));
    let mut state = LocalProposalState {
        attempted: Some(active),
        submitted: Some((active, subject)),
        non_empty_retry: None,
        candidate_work_wait: None,
        pending_events: Some(PendingLocalEvents {
            owner: active,
            subject,
            events: Vec::new(),
        }),
        global_selection: None,
    };

    assert!(state.take_prepared_events(decided, tag, subject).is_none());
    assert!(state.attempted.is_none());
    assert!(state.submitted.is_none());
    assert!(state.pending_events.is_none());
}

#[test]
fn height_one_proposal_projects_staged_genesis_to_resultless_wire() {
    let key_pair = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
        .expect("deterministic genesis key");
    let transaction = TransactionBuilder::new(
        crate::sumeragi::synthetic_network_id("height-one-resultless-projection"),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "staged genesis execution".to_owned())])
    .sign(key_pair.private_key());
    let entrypoint = transaction.hash_as_entrypoint();
    let mut staged = SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None);
    staged
        .set_transaction_results(
            Vec::new(),
            &[entrypoint],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach deterministic staged genesis results");
    assert!(staged.has_results());
    assert!(!staged.is_resultless_proposal());
    assert!(staged.header().result_merkle_root().is_some());

    let staged_header_hash = staged.header().hash();
    let staged_hash = staged.hash();
    let staged_signatures = staged.signatures().cloned().collect::<Vec<_>>();
    let staged_result_root = staged.header().result_merkle_root();
    let staged_execution_wire = staged.encode_wire().expect("encode staged execution image");
    let wire =
        canonical_height_one_proposal_wire(&staged).expect("encode canonical height-one proposal");
    let proposal = decode_framed_signed_block(&wire).expect("decode height-one proposal");

    assert!(proposal.is_resultless_proposal());
    assert!(!proposal.has_results());
    assert!(proposal.header().result_merkle_root().is_none());
    assert_eq!(proposal.header().hash(), staged_header_hash);
    assert_eq!(proposal.hash(), staged_hash);
    assert_eq!(
        proposal.signatures().cloned().collect::<Vec<_>>(),
        staged_signatures
    );
    assert_eq!(
        staged.header().result_merkle_root(),
        staged_result_root,
        "proposal projection must not mutate the staged result root"
    );
    assert_eq!(
        staged
            .encode_wire()
            .expect("re-encode staged execution image"),
        staged_execution_wire,
        "proposal projection must not mutate the staged execution image"
    );
    assert_eq!(
        Hash::new(&wire),
        staged
            .canonical_proposal_wire_hash()
            .expect("hash canonical staged-genesis proposal"),
    );
}

#[test]
fn exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift() {
    let (context, _) = context();
    let key_pair = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519)
        .expect("deterministic proposal key");
    let transaction = TransactionBuilder::new(
        crate::sumeragi::synthetic_network_id("locked-reproposal-exact-body"),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "immutable locked body".to_owned())])
    .sign(key_pair.private_key());
    let block = SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None)
        .canonical_resultless_proposal();
    assert!(block.is_resultless_proposal());
    let canonical_wire = block.encode_wire().expect("encode exact proposal body");
    let locked_subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let tag = EventTag::new(context.height, 3, Generation::new(17));

    let encoded = encode_exact_local_body(&context, tag, Some(locked_subject), &canonical_wire)
        .expect("encode unchanged locked body at the reproposal round");
    assert_eq!(
        encoded.manifest().round,
        round_for_tag(&context, tag).unwrap()
    );
    assert_eq!(encoded.manifest().subject, locked_subject);
    let (_, chunks) = encoded.into_parts();
    assert_eq!(chunks.concat(), canonical_wire);

    let foreign_subject = proposal_subject(b"foreign locked subject");
    assert!(matches!(
        encode_exact_local_body(&context, tag, Some(foreign_subject), &canonical_wire,),
        Err(V2RunnerError::LockedBodyMismatch)
    ));
}

#[test]
fn replayed_proposal_sign_reserves_only_the_exact_current_lock_owner() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 3, Generation::new(9));
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    };
    let body = b"replayed proposal payload";
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replayed proposal block")),
        payload_hash: Hash::new(body),
    };
    let manifest = encode_payload(&context, round, subject, body)
        .expect("encode replayed proposal fixture payload")
        .manifest()
        .clone();
    let proposal = wire::Proposal {
        round,
        proposer: context.leader(round.view),
        subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(
            wire::ParentCommitJustification { certificate: None },
        ),
        signature: Vec::new(),
    };
    let effects = [
        AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        )),
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        },
    ];

    let replayed = replayed_proposal_sign(&effects).expect("extract exact replay owner");
    assert_eq!(
        replayed,
        ReplayedProposalSign {
            tag,
            round,
            subject,
        }
    );
    assert_eq!(replayed_proposal_sign(&effects[..1]), None);
    assert_eq!(replayed_proposal_sign(&[]), None);

    let directive = |locked_subject: Option<wire::BlockSubject>,
                     decided_subject: Option<wire::BlockSubject>| {
        LocalProposalDirective::for_test(
            tag,
            context.leader(tag.view()),
            locked_subject.map(|_| wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 1,
            }),
            locked_subject,
            decided_subject,
        )
    };
    let unlocked = directive(None, None);
    assert_eq!(
        LocalProposalState::from_replayed_proposal(Some(replayed), unlocked).attempted,
        Some(LocalProposalOwner::from(unlocked))
    );

    let exact_lock = directive(Some(subject), None);
    assert_eq!(
        LocalProposalState::from_replayed_proposal(Some(replayed), exact_lock).attempted,
        Some(LocalProposalOwner::from(exact_lock)),
        "the exact replayed subject owns current locked-body work"
    );

    let foreign_lock = directive(Some(proposal_subject(b"foreign replay lock")), None);
    assert!(
        LocalProposalState::from_replayed_proposal(Some(replayed), foreign_lock)
            .attempted
            .is_none(),
        "an equal-tag proposal for another subject cannot reserve the current lock owner"
    );

    let mismatched_round = ReplayedProposalSign {
        round: wire::ConsensusRound { view: 2, ..round },
        ..replayed
    };
    assert!(
        LocalProposalState::from_replayed_proposal(Some(mismatched_round), unlocked)
            .attempted
            .is_none(),
        "the replayed proposal round must match its reducer tag"
    );

    let decided = directive(Some(subject), Some(subject));
    assert!(
        LocalProposalState::from_replayed_proposal(Some(replayed), decided)
            .attempted
            .is_none(),
        "a decision retires every replayed proposal reservation"
    );
}

#[test]
fn outer_ingress_batch_services_completions_and_runtime_before_every_ingress() {
    let (context, _) = context();
    let collect = |limit| {
        let mut cursor = outer_ingress_turns(limit, context.id(), context.height);
        let mut turns = Vec::new();
        while let Some(turn) = cursor.next_current() {
            turns.push(turn.turn());
        }
        turns
    };
    assert_eq!(
        collect(3),
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
        collect(0),
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
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
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

    let manifest = encode_payload(&context, round, subject, &body)
        .expect("encode terminal body fixture payload")
        .manifest()
        .clone();
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
    let construction =
        PendingSuccessorConstruction::begin(predecessor).expect("begin mismatched-context handoff");
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
fn bound_block_sync_finalization_retires_only_nonfatal_no_output_paths() {
    let output_guard = ConsensusOutputGuard::isolated();
    let calls = RefCell::new(Vec::new());
    let served = serve_block_sync_while_guarded(
        output_guard.as_ref(),
        || Ok(Some(())),
        |(), _permit| {
            calls.borrow_mut().push("post");
            Ok(())
        },
    );
    let posted = finalize_bound_block_sync_serve(
        served,
        || {
            calls.borrow_mut().push("volatile");
            Ok(())
        },
        |_| calls.borrow_mut().push("remote-rejection"),
    )
    .expect("posted response owns its bound runtime receipt");
    assert_eq!(posted, BoundBlockSyncServeOutcome::Posted);
    assert_eq!(
        calls.borrow().as_slice(),
        ["post"],
        "posted exact output, not VolatileTerminal, owns the runtime receipt"
    );
    calls.borrow_mut().clear();

    let served = serve_block_sync_while_guarded(
        output_guard.as_ref(),
        || Ok::<Option<()>, V2BlockSyncError>(None),
        |(), _permit| {
            calls.borrow_mut().push("unexpected-post");
            Ok(())
        },
    );
    let no_response = finalize_bound_block_sync_serve(
        served,
        || {
            calls.borrow_mut().push("volatile");
            Ok(())
        },
        |_| calls.borrow_mut().push("remote-rejection"),
    )
    .expect("no-response history retires through VolatileTerminal");
    assert_eq!(no_response, BoundBlockSyncServeOutcome::VolatileNoResponse);
    assert_eq!(calls.borrow().as_slice(), ["volatile"]);
    calls.borrow_mut().clear();

    let served = serve_block_sync_while_guarded(
        output_guard.as_ref(),
        || {
            Err::<Option<()>, _>(V2BlockSyncError::Wire(
                wire::ValidationError::WrongHeightContext,
            ))
        },
        |(), _permit| {
            calls.borrow_mut().push("unexpected-post");
            Ok(())
        },
    );
    let remote_rejection = finalize_bound_block_sync_serve(
        served,
        || {
            calls.borrow_mut().push("volatile");
            Ok(())
        },
        |_| calls.borrow_mut().push("remote-rejection"),
    )
    .expect("remote rejection retires through VolatileTerminal");
    assert_eq!(
        remote_rejection,
        BoundBlockSyncServeOutcome::VolatileRemoteRejection
    );
    assert_eq!(
        calls.borrow().as_slice(),
        ["volatile", "remote-rejection"],
        "runtime retirement precedes nonfatal rejection observation"
    );
    calls.borrow_mut().clear();

    let fatal = finalize_bound_block_sync_serve(
        Err(V2BlockSyncError::RestartRequired),
        || {
            calls.borrow_mut().push("volatile");
            Ok(())
        },
        |_| calls.borrow_mut().push("remote-rejection"),
    );
    assert!(matches!(
        fatal,
        Err(V2RunnerError::BlockSync(V2BlockSyncError::RestartRequired))
    ));
    assert!(
        calls.borrow().is_empty(),
        "fatal service failure leaves the durable Runtime owner for restart recovery"
    );
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
