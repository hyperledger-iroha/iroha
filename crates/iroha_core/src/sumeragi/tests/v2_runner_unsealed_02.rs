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
fn locked_body_reproposal_rearms_only_for_transient_executor_capacity() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 5, Generation::new(19));
    let subject = proposal_subject(b"capacity-blocked locked-body reproposal");
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
    let blocked_plan = locked_body_recovery_plan(directive, leader, None, false);
    assert!(locked_body_reproposal_is_capacity_blocked(
        blocked_plan,
        directive,
        leader,
        None,
        false,
    ));
    assert!(
        !locked_body_reproposal_is_capacity_blocked(blocked_plan, directive, leader, None, true),
        "a consumed runtime producer reservation must not hot-rearm"
    );
    assert!(!locked_body_reproposal_is_capacity_blocked(
        blocked_plan,
        directive,
        nonleader,
        None,
        false,
    ));
    assert!(!locked_body_reproposal_is_capacity_blocked(
        blocked_plan,
        directive,
        leader,
        Some(owner),
        false,
    ));
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
    let (manifest, chunks) = encoded.into_parts();
    let mut session = super::super::v2_chunks::V2ChunkSession::open(&context, manifest)
        .expect("open exact reproposal chunk session");
    for (index, chunk) in chunks.iter().enumerate() {
        session
            .admit_bytes(
                u32::try_from(index).expect("fixture chunk index fits u32"),
                chunk,
            )
            .expect("admit exact reproposal chunk");
    }
    assert_eq!(
        session
            .reconstruct()
            .expect("reconstruct exact reproposal body")
            .expect("complete exact reproposal body"),
        canonical_wire
    );
    let foreign_subject = proposal_subject(b"foreign locked subject");
    assert!(matches!(
        encode_exact_local_body(&context, tag, Some(foreign_subject), &canonical_wire,),
        Err(V2RunnerError::LockedBodyMismatch)
    ));
}
#[test]
fn recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 3, Generation::new(9));
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replayed proposal block")),
        payload_hash: Hash::new(b"replayed proposal payload"),
    };

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
    let recovered =
        super::super::v2::RecoveredLifecycleLocalProposalAttemptV1::for_test(tag, round, subject);
    assert!(recovered.exactly_matches_directive(unlocked));
    assert_eq!(
        LocalProposalState::from_recovered_lifecycle_attempt(true, unlocked).attempted,
        Some(LocalProposalOwner::from(unlocked))
    );
    assert!(
        LocalProposalState::from_recovered_lifecycle_attempt(false, unlocked)
            .attempted
            .is_none()
    );
    let mut setup = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    assert!(setup.bind_recovered_local_proposal(unlocked));
    assert!(
        !setup.bind_recovered_local_proposal(unlocked),
        "a second bind must reject the already-owned runner state"
    );
    assert!(setup.already_attempted(unlocked));
    let exact_lock = directive(Some(subject), None);
    assert!(recovered.exactly_matches_directive(exact_lock));

    let upgraded_lock = directive(Some(proposal_subject(b"upgraded replay lock")), None);
    assert!(
        recovered.exactly_matches_directive(upgraded_lock),
        "a same-view lock upgrade cannot reopen that view's one proposal slot"
    );
    let mismatched_round = super::super::v2::RecoveredLifecycleLocalProposalAttemptV1::for_test(
        tag,
        wire::ConsensusRound { view: 2, ..round },
        subject,
    );
    assert!(!mismatched_round.exactly_matches_directive(unlocked));

    let decided = directive(Some(subject), Some(subject));
    assert!(!recovered.exactly_matches_directive(decided));
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
    assert!(!v2_payload_is_terminal_reducer_control(
        &wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"terminal ingress orphan chunk",
            )),
            index: 0,
            bytes: Vec::new(),
            sender: 0,
            signature: vec![1],
        })
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
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        )),
        Err(FairV2IngressPushError::Closed(_))
    ));
}
#[test]
fn lifecycle_preactivation_recovery_aperture_borrows_exact_future_activation() {
    let _status_guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let configured_ingress = || {
        let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure preactivation recovery ingress");
        ingress
    };

    let ready = Arc::new(AtomicBool::new(false));
    let ingress = configured_ingress();
    let mut activation = ProductionLifecycleRunnerActivationV1::current_height_for_test(
        Arc::clone(&ready),
        Arc::clone(&ingress),
    );
    let aperture = activation
        .open_canonical_recovery_ingress(&ingress)
        .expect("borrow exact ordinary activation ingress");
    assert!(ready.load(Ordering::Acquire));
    assert!(ingress.state.lock().open);
    assert!(std::ptr::eq(aperture.ingress(), ingress.as_ref()));
    assert!(aperture.close_and_verify());
    assert!(!ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    assert!(super::super::status::v2_status().is_none());

    let complete_tip_ready = Arc::new(AtomicBool::new(false));
    let complete_tip_ingress = configured_ingress();
    let mut complete_tip = ProductionLifecycleCompleteTipRunnerActivationV1::for_test(
        Arc::clone(&complete_tip_ready),
        Arc::clone(&complete_tip_ingress),
    );
    {
        let aperture = complete_tip
            .open_canonical_recovery_ingress(&complete_tip_ingress)
            .expect("borrow exact CompleteTip activation ingress");
        assert!(complete_tip_ready.load(Ordering::Acquire));
        assert!(aperture.ingress().state.lock().open);
    }
    assert!(!complete_tip_ready.load(Ordering::Acquire));
    assert!(!complete_tip_ingress.state.lock().open);
    assert!(super::super::status::v2_status().is_none());
    complete_tip
        .retire_unpublished(&complete_tip_ingress)
        .expect("retire unpublished CompleteTip activation");

    let pending_ready = Arc::new(AtomicBool::new(false));
    let pending_ingress = configured_ingress();
    let mut pending = ProductionLifecyclePendingKuraRunnerActivationV1::for_test(
        Arc::clone(&pending_ready),
        Arc::clone(&pending_ingress),
    );
    {
        let aperture = pending
            .open_canonical_recovery_ingress(&pending_ingress)
            .expect("borrow exact pending-Kura activation ingress");
        assert!(pending_ready.load(Ordering::Acquire));
        assert!(aperture.ingress().state.lock().open);
    }
    assert!(!pending_ready.load(Ordering::Acquire));
    assert!(!pending_ingress.state.lock().open);
    assert!(super::super::status::v2_status().is_none());
    pending
        .retire_unpublished(&pending_ingress)
        .expect("retire unpublished pending-Kura activation");

    let foreign = configured_ingress();
    assert!(matches!(
        activation.open_canonical_recovery_ingress(&foreign),
        Err(V2RunnerError::LifecycleActivationIngressMismatch)
    ));
    assert!(!ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    activation
        .retire_unpublished(&ingress)
        .expect("retire exact unpublished ordinary activation after rejection");
}

#[test]
fn pending_kura_runner_activation_publishes_current_height_without_successor_authority() {
    let _status_guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let (context, _) = context();
    let ready = Arc::new(AtomicBool::new(false));
    let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
    ingress
        .configure_roster(std::iter::empty())
        .expect("configure pending-Kura activation ingress");
    let activation = ProductionLifecyclePendingKuraRunnerActivationV1::for_test(
        Arc::clone(&ready),
        Arc::clone(&ingress),
    );
    let expected = runner_status(&context);
    let activated = activation
        .open_and_publish_recovered_height(&ingress, expected.clone())
        .expect("publish recovered current-height status without successor authority");
    assert!(ready.load(Ordering::Acquire));
    assert!(ingress.state.lock().open);
    assert_eq!(super::super::status::v2_status(), Some(expected));

    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(ingress.len(), 1);
    activated
        .close_ingress(&ingress)
        .expect("close pending-Kura ingress for a finite finalized drain");
    assert!(!ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    assert_eq!(ingress.len(), 1, "closing preserves the admitted prefix");
    assert!(matches!(
        ingress.ensure_closed_drained_cut(),
        Err(reason) if reason.contains("retained physical ownership")
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        )),
        Err(FairV2IngressPushError::Closed(_))
    ));
    assert!(
        ingress
            .try_recv_if_checked(|_| true)
            .expect("drain the closed pending-Kura ingress")
            .is_some()
    );
    assert!(
        ingress
            .try_recv_if_checked(|_| true)
            .expect("observe the finite pending-Kura ingress cut")
            .is_none()
    );
    ingress
        .ensure_closed_drained_cut()
        .expect("authenticate the empty closed pending-Kura ingress cut");

    drop(activated);
    assert!(!ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    super::super::status::clear_v2_status();
}

#[test]
fn pending_kura_finalization_closes_and_drains_before_rollover() {
    let source = include_str!("../v2_runner/lifecycle_pending_kura.rs");
    let start = source
        .find("let finalization_ready = activated.ready_for_finalized_rollover")
        .expect("pending-Kura finalization preflight remains explicit");
    let end = source[start..]
        .find("let prepared_successor = {")
        .map(|offset| start + offset)
        .expect("pending-Kura successor construction follows rollover");
    let finalization = &source[start..end];
    let mut cursor = 0;
    for token in [
        "let finalization_ready = activated.ready_for_finalized_rollover",
        "let rollover_ready = if finalization_ready",
        "if !rollover_ready",
        "close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)",
        "loop {",
        "drain_decided_lane_recovery_ingress(",
        "drain_finalized_lane_relay_prefix(",
        "if !drained_terminal_ingress && !drained_terminal_relay",
        "break;",
        "ensure_closed_drained_cut()",
        "activated.into_finalized_rollover(&mut active_runner)",
    ] {
        let offset = finalization[cursor..]
            .find(token)
            .unwrap_or_else(|| panic!("pending-Kura finalized drain lost `{token}`"));
        cursor = cursor.saturating_add(offset).saturating_add(token.len());
    }
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
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                valid_ingress_probe(),
                authenticated_peer_for_test(),
            )),
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
        .try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        ))
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
            rejected_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                valid_ingress_probe(),
                authenticated_peer_for_test(),
            )),
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
fn complete_tip_recovery_requires_authenticated_predecessor_retirement() {
    let _guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let (parent_context, keys) = context();
    let ready = AtomicBool::new(false);
    let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
    ingress
        .configure_roster(std::iter::empty())
        .expect("configure untrusted test lane");
    let directory = TempDir::new().expect("temporary unretired CompleteTip lifecycle root");
    let kura = Kura::blank_kura_for_testing();
    let mut successor_context = parent_context.clone();
    successor_context.height += 1;
    let error = PendingSuccessorActivation::recovered(
        RecoveredSuccessorActivationAuthority::CompleteTip(test_recovered_complete_tip_authority(
            &parent_context,
            successor_context.id(),
            b"exact recovered successor context",
            directory.path(),
        )),
        kura.as_ref(),
        &keys[0],
    )
    .expect_err("CompleteTip cannot form activation before exact predecessor retirement");
    assert!(matches!(
        error,
        V2RunnerError::CompleteTipPredecessorStorage(_)
    ));
    assert!(!ready.load(Ordering::Acquire));
    assert!(
        super::super::status::v2_status().is_none(),
        "unretired CompleteTip recovery must not publish successor status"
    );
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                valid_ingress_probe(),
                authenticated_peer_for_test(),
            )),
            Err(FairV2IngressPushError::Closed(_))
        ),
        "unretired CompleteTip recovery must leave ingress closed"
    );
    super::super::status::clear_v2_status();
    #[cfg(feature = "bls")]
    {
        let successor_status = |context: &wire::HeightContext| {
            let mut status = runner_status(context);
            status.liveness.generation = context.height;
            status.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
                generation: status.liveness.generation,
                round: wire::ConsensusRound {
                    context_id: status.height_context_id,
                    height: status.height,
                    view: status.view,
                },
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                age_ms: 0,
            });
            status
        };
        let (_kura, _predecessor_root, exact_context, retirement) =
            super::super::v2_first_release_recovery::complete_tip_restart_activation_fixture();
        let activation = PendingSuccessorActivation::RecoveredCompleteTip {
            authority: retirement,
        };
        activation
            .preflight_recovered_startup()
            .expect("exact retired CompleteTip reauthenticates its H+1 ledger");
        let exact_ready = AtomicBool::new(false);
        let exact_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        exact_ingress
            .configure_roster(std::iter::empty())
            .expect("configure exact restart ingress");
        let exact_output_guard = ConsensusOutputGuard::isolated();
        open_ingress_for_active_height(
            exact_output_guard.as_ref(),
            &exact_ready,
            &exact_ingress,
            Some((activation, successor_status(&exact_context))),
        )
        .expect("exact retired CompleteTip publishes its authenticated successor");
        assert!(exact_ready.load(Ordering::Acquire));
        assert!(!exact_output_guard.restart_required());
        let published = super::super::status::v2_status()
            .expect("exact CompleteTip restart publishes H+1 status");
        assert_eq!(published.height_context_id, exact_context.id());
        assert_eq!(published.height, exact_context.height);
        assert_eq!(published.last_committed_height + 1, published.height);
        close_ingress_for_rollover(&exact_ready, &exact_ingress);
        super::super::status::clear_v2_status();
        let (_kura, _predecessor_root, typed_context, retirement) =
            super::super::v2_lifecycle_coordinator::complete_tip_restart_activation_fixture();
        let typed_ready = Arc::new(AtomicBool::new(false));
        let typed_ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        typed_ingress
            .configure_roster(std::iter::empty())
            .expect("configure typed CompleteTip activation ingress");
        let typed_activation = ProductionLifecycleCompleteTipRunnerActivationV1::for_test(
            Arc::clone(&typed_ready),
            Arc::clone(&typed_ingress),
        );
        let activated = typed_activation
            .open_and_publish(&typed_ingress, retirement, successor_status(&typed_context))
            .expect("typed CompleteTip activation retains retirement through publication");
        assert!(typed_ready.load(Ordering::Acquire));
        assert_eq!(
            super::super::status::v2_status()
                .expect("typed CompleteTip activation publishes H+1")
                .height_context_id,
            typed_context.id()
        );
        drop(activated);
        assert!(!typed_ready.load(Ordering::Acquire));
        assert!(!typed_ingress.state.lock().open);
        super::super::status::clear_v2_status();
        let (_kura, _predecessor_root, invalid_context, retirement) =
            super::super::v2_lifecycle_coordinator::complete_tip_restart_activation_fixture();
        let invalid_ready = Arc::new(AtomicBool::new(true));
        let invalid_ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        invalid_ingress
            .configure_roster(std::iter::empty())
            .expect("configure invalid CompleteTip activation ingress");
        invalid_ingress
            .open()
            .expect("open stale CompleteTip activation ingress");
        let invalid_activation = ProductionLifecycleCompleteTipRunnerActivationV1::for_test(
            Arc::clone(&invalid_ready),
            Arc::clone(&invalid_ingress),
        );
        let mut invalid_status = successor_status(&invalid_context);
        invalid_status.last_committed_height = invalid_status.height;
        assert!(matches!(
            invalid_activation.open_and_publish(&invalid_ingress, retirement, invalid_status),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        assert!(!invalid_ready.load(Ordering::Acquire));
        assert!(!invalid_ingress.state.lock().open);
        let (drift_kura, _predecessor_root, drift_context, retirement) =
            super::super::v2_first_release_recovery::complete_tip_restart_activation_fixture();
        let successor_ledger = drift_kura
            .sumeragi_v2_storage_root()
            .join("lifecycle-v1")
            .join(hex::encode(drift_context.id().0.as_ref()))
            .join("lifecycle-ledger-v1.norito");
        std::fs::write(&successor_ledger, b"replaced successor frame")
            .expect("replace the successor frame after retirement authentication");
        let drift_activation = PendingSuccessorActivation::RecoveredCompleteTip {
            authority: retirement,
        };
        assert!(matches!(
            drift_activation.preflight_recovered_startup(),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        let drift_ready = AtomicBool::new(false);
        let drift_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        drift_ingress
            .configure_roster(std::iter::empty())
            .expect("configure drifted restart ingress");
        let drift_output_guard = ConsensusOutputGuard::isolated();
        assert!(matches!(
            open_ingress_for_active_height(
                drift_output_guard.as_ref(),
                &drift_ready,
                &drift_ingress,
                Some((drift_activation, successor_status(&drift_context))),
            ),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        assert!(!drift_ready.load(Ordering::Acquire));
        assert!(drift_output_guard.restart_required());
        assert!(drift_output_guard.acquire().is_none());
        assert!(super::super::status::v2_status().is_none());
        let (predecessor_kura, predecessor_root, predecessor_context, retirement) =
            super::super::v2_first_release_recovery::complete_tip_restart_activation_fixture();
        let predecessor_ledger = predecessor_root.join("lifecycle-ledger-v1.norito");
        std::fs::write(&predecessor_ledger, b"replaced predecessor frame")
            .expect("replace the predecessor frame after retirement authentication");
        let predecessor_activation = PendingSuccessorActivation::RecoveredCompleteTip {
            authority: retirement,
        };
        assert!(matches!(
            predecessor_activation.preflight_recovered_startup(),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        let predecessor_ready = AtomicBool::new(false);
        let predecessor_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        predecessor_ingress
            .configure_roster(std::iter::empty())
            .expect("configure predecessor-drift restart ingress");
        let predecessor_output_guard = ConsensusOutputGuard::isolated();
        assert!(matches!(
            open_ingress_for_active_height(
                predecessor_output_guard.as_ref(),
                &predecessor_ready,
                &predecessor_ingress,
                Some((
                    predecessor_activation,
                    successor_status(&predecessor_context),
                )),
            ),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        assert!(!predecessor_ready.load(Ordering::Acquire));
        assert!(predecessor_output_guard.restart_required());
        assert!(predecessor_output_guard.acquire().is_none());
        assert!(super::super::status::v2_status().is_none());
        drop(predecessor_kura);
        let (_foreign_kura, _predecessor_root, foreign_context, retirement) =
            super::super::v2_first_release_recovery::complete_tip_restart_activation_fixture();
        let mut foreign_status = successor_status(&foreign_context);
        let foreign_context_id =
            wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
                Hash::new(b"foreign CompleteTip restart successor"),
            ));
        foreign_status.height_context_id = foreign_context_id;
        foreign_status
            .liveness
            .last_progress
            .as_mut()
            .expect("successor activation marker")
            .round
            .context_id = foreign_context_id;
        let foreign_ready = AtomicBool::new(false);
        let foreign_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        foreign_ingress
            .configure_roster(std::iter::empty())
            .expect("configure foreign-context restart ingress");
        let foreign_output_guard = ConsensusOutputGuard::isolated();
        assert!(matches!(
            open_ingress_for_active_height(
                foreign_output_guard.as_ref(),
                &foreign_ready,
                &foreign_ingress,
                Some((
                    PendingSuccessorActivation::RecoveredCompleteTip {
                        authority: retirement,
                    },
                    foreign_status,
                )),
            ),
            Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid { .. })
        ));
        assert!(!foreign_ready.load(Ordering::Acquire));
        assert!(foreign_output_guard.restart_required());
        assert!(foreign_output_guard.acquire().is_none());
        assert!(super::super::status::v2_status().is_none());
    }
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
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        )),
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
    let byte_error = FairV2Ingress::new(10, 2 * 1024 - 1, 1024, 0, 0)
        .configure_roster(validators)
        .expect_err("two validators require two exact byte partitions");
    assert!(matches!(
        ingress_capacity_error(byte_error),
        V2RunnerError::IngressByteCapacity {
            configured: 2047,
            required: 2048,
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
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            valid_ingress_probe(),
            authenticated_peer_for_test(),
        )),
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
