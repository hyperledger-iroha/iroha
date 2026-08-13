#[cfg(feature = "bls")]
#[test]
fn nonzero_view_proposal_intent_replays_through_production_services() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let context = service.context.clone();
    let target_view = (1_u64
        ..=u64::try_from(context.roster.len()).expect("fixture roster length fits u64"))
        .find(|view| context.leader(*view) == 0)
        .expect("round-robin leader rotation returns to genesis authority");
    let local_validator = context.leader(target_view);
    let local_index = usize::try_from(local_validator).expect("fixture leader index");
    assert_eq!(local_index, 0);
    service.local_validator = Some(local_validator);
    service.local_peer = context.roster[local_index].validator.clone();
    service.key_pair = keys[local_index].clone();
    let signature_policy =
        BlockSignaturePolicy::GenesisAuthority(keys[local_index].public_key().clone());
    let proofs_of_possession = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture proof of possession")
        })
        .collect::<Vec<_>>();
    let fingerprints = AdapterFingerprints {
        node: Hash::new(b"nonzero-view-restart-node"),
        build: Hash::new(b"nonzero-view-restart-build"),
        config: Hash::new(b"nonzero-view-restart-config"),
    };
    let consensus_key_hash = [0xA6; 32];
    let directory = TempDir::new().expect("restart storage root");
    let wal_path = directory
        .path()
        .join("wal")
        .join("00000000000000000001.wal");
    let body_root = directory.path().join("bodies");
    std::fs::create_dir_all(wal_path.parent().expect("WAL parent directory"))
        .expect("create WAL parent directory");
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs_of_possession.clone())
        .expect("verify restart context");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        wal_path.clone(),
        verified,
        Some(local_validator),
        Generation::new(context.height),
        consensus_key_hash,
        fingerprints,
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open pre-crash adapter");
    assert!(startup.is_empty());
    let timeout_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: target_view - 1,
    };
    let timeout_signers = vec![0, 1, 2];
    let timeout_shares = timeout_signers
        .iter()
        .map(|signer| {
            let vote = wire::TimeoutVote {
                round: timeout_round,
                highest_prepare_qc: None,
                signer: *signer,
                signature: Vec::new(),
            };
            Signature::new(
                keys[usize::try_from(*signer).expect("fixture timeout signer")].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_share_refs = timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let timeout_certificate = wire::TimeoutCertificate {
        round: timeout_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: timeout_signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&timeout_share_refs)
                .expect("aggregate fixture timeout certificate"),
        }],
    };
    let authenticated_timeout = adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate.clone()),
        ))
        .expect("authenticate timeout certificate");
    let view_effects = adapter
        .receive_authenticated(authenticated_timeout)
        .expect("durably install timeout certificate")
        .into_effects();
    let pre_crash_tag = view_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView { tag, .. } => Some(*tag),
            _ => None,
        })
        .expect("timeout certificate enters its successor view");
    assert_eq!(pre_crash_tag.view(), target_view);
    let directive = adapter
        .local_proposal_directive()
        .expect("read post-timeout proposal directive");
    assert_eq!(directive.tag(), pre_crash_tag);
    assert_eq!(directive.leader(), local_validator);
    let (canonical_wire, payload) = proposal_body_and_payload_at_view(&context, &keys, target_view);
    let proposal_round = payload.manifest().round;
    let proposal_subject = payload.manifest().subject;
    let mut body_store =
        V2BodyStore::open_with_policy(&body_root, context.clone(), signature_policy.clone())
            .expect("open pre-crash body store");
    let durable = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("persist exact nonzero-view body");
    let validation_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"restart parent state"),
        Hash::new(b"restart post state"),
        Hash::new(b"restart ordinary writes"),
        1,
        Hash::new(b"restart executed block wire"),
    );
    let validated = body_store
        .validate(&durable, |_| Ok::<_, &'static str>(validation_commitment))
        .expect("persist exact nonzero-view validation marker");
    let signing = adapter
        .local_proposal_ready(
            directive.tag(),
            payload.manifest().clone(),
            &durable,
            &validated,
        )
        .expect("persist nonzero-view proposal intent")
        .into_effects();
    assert!(matches!(
        signing.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        }] if *tag == pre_crash_tag
            && proposal.round == proposal_round
            && proposal.subject == proposal_subject
            && matches!(
                &proposal.justification,
                wire::ProposalJustification::Timeout(timeout)
                    if timeout.timeout_certificate == timeout_certificate
            )
    ));
    drop(adapter);
    drop(body_store);
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs_of_possession)
        .expect("reverify restart context");
    let (adapter, startup_effects) = SumeragiV2Adapter::open(
        wal_path,
        verified,
        Some(local_validator),
        Generation::new(context.height),
        consensus_key_hash,
        fingerprints,
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("reopen adapter from safety WAL");
    let replayed_tag = match startup_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ] => {
            assert_eq!(proposal.round, proposal_round);
            assert_eq!(proposal.subject, proposal_subject);
            assert!(matches!(
                &proposal.justification,
                wire::ProposalJustification::Timeout(timeout)
                    if timeout.timeout_certificate == timeout_certificate
            ));
            *tag
        }
        effects => panic!("unexpected nonzero-view startup effects: {effects:?}"),
    };
    let expected_replayed_tag =
        EventTag::new(context.height, target_view, Generation::new(context.height));
    assert_eq!(replayed_tag, expected_replayed_tag);
    let started_at = Instant::now();
    let (runtime, startup_effects) = SerializedV2Runtime::new(
        adapter,
        startup_effects,
        started_at,
        Duration::from_secs(2),
        RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("construct replay runtime");
    let output_guard = ConsensusOutputGuard::isolated();
    let mut reopened_body_store =
        V2BodyStore::open_with_policy(&body_root, context.clone(), signature_policy)
            .expect("reopen exact body store for semantic replay");
    reopened_body_store
        .revalidate_recovered_markers(|_| Ok::<_, String>(validation_commitment))
        .expect("semantically replay the recovered validation marker");
    let (mut executor, reopened_body_store) = V2EffectExecutor::open_with_body_store(
        runtime,
        reopened_body_store,
        context.clone(),
        service.local_peer.clone(),
        Some(local_validator),
        Arc::clone(&output_guard),
        EffectQueueConfig::default(),
    )
    .expect("reopen exact-body executor");
    assert_eq!(executor.current_tag(), replayed_tag);
    assert!(
        reopened_body_store
            .recovered(proposal_round, proposal_subject)
            .expect("read recovered proposal body")
            .is_some()
    );
    let (command_tx, command_rx, admission) = test_io_command_channel(4);
    let (completion_tx, completion_rx) = mpsc::sync_channel(4);
    service.active_tag = replayed_tag;
    service.output_guard = Arc::clone(&output_guard);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let expected_targets = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    let expected_chunk_targets = {
        let committee = service
            .committee_for_round(proposal_round)
            .expect("project replayed proposal committee");
        service
            .remote_voters_for_indices(committee.set_a())
            .expect("resolve Set A peers")
            .into_iter()
            .collect::<BTreeSet<_>>()
    };
    let admitted_posts = Arc::new(Mutex::new(Vec::new()));
    let admitted_posts_for_hook = Arc::clone(&admitted_posts);
    service.set_exact_output_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        admitted_posts_for_hook
            .lock()
            .expect("lock admitted replay outputs")
            .push(post);
        Ok(())
    });
    executor
        .consume_effects(startup_effects, &mut service)
        .expect("dispatch replayed proposal signature");
    assert_eq!(executor.status().pending_signatures, 1);
    let (proposal_work_id, proposal_completion) = match command_rx.try_recv() {
        Ok(V2IoCommand::Sign {
            task,
            restore_outbound_payload,
        }) => {
            assert!(restore_outbound_payload);
            assert_eq!(task.tag(), replayed_tag);
            assert!(matches!(
                task.request(),
                SignRequest::Proposal(proposal)
                    if proposal.round == proposal_round
                        && proposal.subject == proposal_subject
            ));
            let work_id = task.id();
            let completion = sign_consensus_task(
                &reopened_body_store,
                &context,
                &service.key_pair,
                task,
                restore_outbound_payload,
            )
            .expect("sign replayed production proposal");
            (work_id, completion)
        }
        _ => panic!("expected replayed production proposal signature"),
    };
    command_rx.complete_work(proposal_work_id);
    completion_tx
        .try_send(proposal_completion)
        .expect("return production signature completion");
    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("restore replayed outbound chunks"),
        1
    );
    let retained = service
        .outbound_chunks
        .get(&HashOf::new(payload.manifest()))
        .expect("replayed proposal restores exact outbound chunks before broadcast");
    assert_eq!(retained.owner, replayed_tag);
    assert_eq!(retained.round, proposal_round);
    assert_eq!(retained.subject, proposal_subject);
    executor
        .arm_live_clocks(started_at)
        .expect("arm post-recovery pacemaker");
    assert_eq!(
        executor
            .step(started_at, &mut service)
            .expect("broadcast replayed proposal and continue consensus"),
        EffectExecutorStep::Advanced { effects: 2 }
    );
    let prepare = match command_rx.try_recv() {
        Ok(V2IoCommand::Sign {
            task,
            restore_outbound_payload: false,
        }) => task,
        _ => panic!("proposal broadcast must re-enter progress with a Prepare vote"),
    };
    assert_eq!(prepare.tag(), replayed_tag);
    assert!(matches!(
        prepare.request(),
        SignRequest::Vote(vote)
            if vote.phase == wire::GlobalPhase::Prepare
                && vote.round == proposal_round
                && vote.subject == proposal_subject
    ));
    assert_eq!(executor.current_tag(), replayed_tag);
    assert_eq!(service.active_tag, replayed_tag);
    assert_eq!(executor.status().pending_signatures, 1);
    let admitted_posts = admitted_posts
        .lock()
        .expect("inspect admitted replay outputs");
    let mut proposal_targets = BTreeSet::new();
    let mut chunk_targets = BTreeSet::new();
    for post in admitted_posts.iter() {
        let NetworkMessage::SumeragiBlock(envelope) = &post.data else {
            panic!("replayed proposal emitted a non-Sumeragi message");
        };
        let BlockMessage::V2(message) = envelope.as_message() else {
            panic!("replayed proposal emitted a lane message");
        };
        match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                assert_eq!(proposal.round, proposal_round);
                assert_eq!(proposal.subject, proposal_subject);
                assert!(proposal_targets.insert(post.peer_id.clone()));
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {
                assert_eq!(chunk.manifest_hash, HashOf::new(payload.manifest()));
                chunk_targets.insert(post.peer_id.clone());
            }
            payload => panic!("unexpected replay output payload: {payload:?}"),
        }
    }
    assert_eq!(proposal_targets, expected_targets);
    assert_eq!(chunk_targets, expected_chunk_targets);
    drop(service.io.take());
}
