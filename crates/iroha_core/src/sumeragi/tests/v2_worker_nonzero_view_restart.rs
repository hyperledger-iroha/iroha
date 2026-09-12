/// Persisted four-validator evidence retained between default-stack fixture phases.
#[cfg(feature = "bls")]
struct NonzeroViewProposalRestartFixtureV1 {
    service: Box<ProductionV2Services>,
    keys: Vec<KeyPair>,
    context: wire::HeightContext,
    target_view: u64,
    local_validator: wire::ValidatorIndex,
    local_index: usize,
    signature_policy: BlockSignaturePolicy,
    proofs_of_possession: Vec<Vec<u8>>,
    fingerprints: AdapterFingerprints,
    consensus_key_hash: [u8; 32],
    directory: TempDir,
    wal_path: std::path::PathBuf,
    body_root: std::path::PathBuf,
    timeout_certificate: wire::TimeoutCertificate,
    payload: EncodedV2Payload,
    proposal_round: wire::ConsensusRound,
    proposal_subject: wire::BlockSubject,
    validation_commitment: wire::ExecutionCommitment,
}

/// Keep the by-value service constructor in its own bounded fixture frame.
#[cfg(feature = "bls")]
#[inline(never)]
fn boxed_nonzero_view_worker_service_fixture() -> (Box<ProductionV2Services>, Vec<KeyPair>) {
    let (service, keys) = fixture();
    (Box::new(service), keys)
}

/// Check exact raw replay without retaining a second adapter beside service launch.
#[cfg(feature = "bls")]
#[inline(never)]
fn assert_nonzero_view_proposal_raw_replay(
    fixture: &NonzeroViewProposalRestartFixtureV1,
) -> EventTag {
    let verified = VerifiedHeightContext::genesis(
        fixture.context.clone(),
        fixture.proofs_of_possession.clone(),
    )
    .expect("reverify restart context");
    let (adapter, startup_effects) = SumeragiV2Adapter::open(
        fixture.wal_path.clone(),
        verified,
        Some(fixture.local_validator),
        Generation::new(fixture.context.height),
        fixture.consensus_key_hash,
        fixture.fingerprints,
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
            assert_eq!(proposal.round, fixture.proposal_round);
            assert_eq!(proposal.subject, fixture.proposal_subject);
            assert!(
                matches!(&proposal.justification, wire::ProposalJustification::Timeout(timeout)
                if timeout.timeout_certificate == fixture.timeout_certificate)
            );
            *tag
        }
        effects => panic!("unexpected nonzero-view startup effects: {effects:?}"),
    };
    assert_eq!(
        replayed_tag,
        EventTag::new(
            fixture.context.height,
            fixture.target_view,
            Generation::INITIAL
        )
    );
    // The observer must release the genuine WAL before the consuming recovery opens it.
    drop(startup_effects);
    drop(adapter);
    replayed_tag
}

/// Replay the original validation marker and consume the actual WAL/body custody.
#[cfg(feature = "bls")]
#[inline(never)]
fn recover_nonzero_view_proposal_owner(
    fixture: &NonzeroViewProposalRestartFixtureV1,
) -> Box<crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1> {
    let verified = VerifiedHeightContext::genesis(
        fixture.context.clone(),
        fixture.proofs_of_possession.clone(),
    )
    .expect("reverify the exact consuming recovery context");
    let mut body_store = V2BodyStore::open_with_policy(
        &fixture.body_root,
        fixture.context.clone(),
        fixture.signature_policy.clone(),
    )
    .expect("reopen exact body store for semantic replay");
    body_store
        .revalidate_recovered_markers(|_| Ok::<_, String>(fixture.validation_commitment))
        .expect("semantically replay the recovered validation marker");
    assert!(
        body_store
            .recovered(fixture.proposal_round, fixture.proposal_subject)
            .expect("read recovered proposal body")
            .is_some()
    );
    SumeragiV2Adapter::reopen_proposal_owner_for_worker_restart_test(
        &fixture.wal_path,
        verified,
        fixture.local_validator,
        Generation::new(fixture.context.height),
        fixture.consensus_key_hash,
        fixture.fingerprints,
        body_store
            .into_revalidated_startup()
            .expect("seal the exact semantically replayed body store"),
        &fixture.directory.path().join("output-owner"),
        &fixture.service.key_pair,
    )
}

/// Persist the exact nonzero-view ProposalIntent and body before the restart phases.
#[cfg(feature = "bls")]
#[inline(never)]
fn persist_nonzero_view_proposal_fixture() -> Box<NonzeroViewProposalRestartFixtureV1> {
    let (mut service, keys) = boxed_nonzero_view_worker_service_fixture();
    allow_fixture_block_payload(&mut service.context);
    let context = service.context.clone();
    let target_view = (1_u64
        ..=u64::try_from(context.roster.len()).expect("fixture roster length fits u64"))
        .find(|view| context.leader(*view) == 0)
        .expect("round-robin leader rotation returns to genesis authority");
    let local_validator = context.leader(target_view);
    let local_index = usize::try_from(local_validator).expect("fixture leader index");
    assert_eq!(local_index, 0);
    assert_ne!(
        context.leader(0),
        local_validator,
        "fixed genesis authority must remain distinct from the original rotating leader"
    );
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
    let (canonical_wire, payload) =
        proposal_body_and_payload_at_view_signed_by(&context, target_view, 0, &keys[local_index]);
    let proposal_round = payload.manifest().round;
    let proposal_subject = payload.manifest().subject;
    let mut body_store =
        V2BodyStore::open_with_policy(&body_root, context.clone(), signature_policy.clone())
            .expect("open pre-crash body store");
    let durable = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("persist exact nonzero-view body");
    let validation_commitment =
        wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
    Box::new(NonzeroViewProposalRestartFixtureV1 {
        service,
        keys,
        context,
        target_view,
        local_validator,
        local_index,
        signature_policy,
        proofs_of_possession,
        fingerprints,
        consensus_key_hash,
        directory,
        wal_path,
        body_root,
        timeout_certificate,
        payload,
        proposal_round,
        proposal_subject,
        validation_commitment,
    })
}

#[cfg(feature = "bls")]
#[test]
fn nonzero_view_proposal_intent_replays_through_production_services() {
    let persisted = persist_nonzero_view_proposal_fixture();
    let replayed_tag = assert_nonzero_view_proposal_raw_replay(&persisted);
    let mut lifecycle_owner = recover_nonzero_view_proposal_owner(&persisted);
    let NonzeroViewProposalRestartFixtureV1 {
        mut service,
        keys,
        context,
        local_validator,
        local_index,
        directory: _directory,
        wal_path,
        timeout_certificate,
        payload,
        proposal_round,
        proposal_subject,
        ..
    } = *persisted;
    let started_at = Instant::now();
    let output_guard = ConsensusOutputGuard::isolated();
    let (_, proposal_ordinal) = lifecycle_owner
        .recovered_control_row_summary_for_test()
        .expect("the recovered Proposal has its genuine Ready carrier");
    let local_peer = &context.roster[local_index].validator;
    assert_eq!(&service.local_peer, local_peer);
    assert_eq!(context.roster.len(), 4);
    let expected_targets = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .filter(|peer| peer != local_peer)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_targets.len(), 3);
    // Cold WAL recovery replays both control and chunks to every remote voter.
    // Only the first live fast-path send limits its chunks to Set A.
    let expected_chunk_targets = expected_targets.clone();
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
    service
        .set_exact_output_shared_unit_capacity_for_test(64)
        .expect("bind the exact Proposal and chunk output capacity to the full roster");
    let source_bytes = iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let ordinary_bytes = iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES
        .get()
        .checked_add(super::super::BODY_ENVELOPE_HEADROOM_BYTES)
        .expect("default ordinary ingress partition fits usize");
    let completion_bytes = source_bytes
        .checked_sub(super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES)
        .and_then(|bytes| bytes.checked_sub(super::super::TIMEOUT_VOTE_RESERVE_BYTES))
        .and_then(|bytes| bytes.checked_sub(ordinary_bytes))
        .expect("default ingress source partitions are disjoint");
    let global_plaintext = iroha_p2p::frame_plaintext_cap(
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
    );
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            128,
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get(),
            source_bytes,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            super::super::TIMEOUT_VOTE_RESERVE_BYTES,
            completion_bytes,
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get()),
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES
                .get(),
            None,
        ),
    );
    ingress
        .configure_roster_for_context(
            context.roster.iter().map(|entry| entry.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .expect("configure the full four-validator recovered ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    service.leader_wire_ingress = Arc::clone(&ingress);
    let (mut launched, planner_io) =
        crate::sumeragi::v2_lifecycle_coordinator::LaunchedProductionLifecycleV1::recovered_proposal_services_for_restart_test(
            lifecycle_owner, service, &wal_path, started_at, local_validator,
            Arc::clone(&output_guard), ingress,
        );
    launched.with_proposal_restart_fixture_for_test(|owner, executor, service| {
        assert_eq!(executor.current_tag(), replayed_tag);
        assert_eq!(service.active_tag, replayed_tag);
        assert_eq!(
            owner
                .dispatch_completion_for_test(service, executor, 0)
                .expect("dispatch the recovered Proposal through its authenticated registry"),
            crate::sumeragi::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::SignQueued {
                ordinal: proposal_ordinal,
            }
        );
        let task = match planner_io.command_rx.try_recv() {
            Ok(V2IoCommand::RecoveredLifecycleSign(task)) => task,
            _ => {
                panic!("the genuine registry must dispatch the recovered Proposal signing command")
            }
        };
        assert_eq!(task.tag, replayed_tag);
        assert!(matches!(&task.request, SignRequest::Proposal(proposal)
            if proposal.round == proposal_round && proposal.subject == proposal_subject
            && matches!(&proposal.justification, wire::ProposalJustification::Timeout(timeout)
                if timeout.timeout_certificate == timeout_certificate)));
        let key = task.dispatch_key();
        assert_eq!(key.lifecycle_ordinal(), proposal_ordinal);
        let result = sign_recovered_lifecycle_task(
            &planner_io.body_store,
            &context,
            &service.key_pair,
            task,
        )
        .expect("sign the recovered Proposal using the production worker");
        assert!(result.is_exact());
        assert_eq!(
            result.outbound_payload.as_ref(),
            Some(&payload),
            "the worker must restore the complete exact canonical body and chunks"
        );
        planner_io
            .command_rx
            .complete_recovered_lifecycle_sign(key, &result)
            .expect("retain the exact recovered Proposal worker result");
        try_send_tracked_completion_with_lifecycle_ordinal(
            &planner_io.completion_tx,
            &planner_io.admission,
            V2IoCompletion::RecoveredLifecycleSign(Box::new(
                GuardedRecoveredLifecycleSignWorkerResultV1::new(result, Arc::clone(&output_guard)),
            )),
            Some(key.lifecycle_ordinal()),
        )
        .expect("publish the tracked production Proposal signature completion");
    });
    assert!(
        launched
            .retain_recovered_lifecycle_sign_completion()
            .expect("retain the exact recovered Proposal completion")
    );
    assert!(
        admitted_posts
            .lock()
            .expect("inspect prepublication outputs")
            .is_empty(),
        "the signed Proposal and chunks must wait for durable lifecycle admission"
    );
    assert_eq!(launched.settle_recovered_lifecycle_proposal_prepare_wal(),
        crate::sumeragi::v2_lifecycle_coordinator::ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied);
    assert!(
        launched
            .has_pending_exact_output_for_ready_sign_test()
            .expect("durable Proposal publication owns its control and chunk fanouts")
    );
    let mut output_pending = true;
    for _ in 0..256 {
        output_pending = launched
            .retry_exact_output_for_ready_sign_test()
            .expect("deliver the recovered Proposal and its exact chunks");
        if !output_pending {
            break;
        }
    }
    assert!(
        !output_pending,
        "the recovered Proposal output must drain boundedly"
    );
    assert!(!output_guard.restart_required());
    launched.with_proposal_restart_fixture_for_test(|owner, executor, service| {
        assert_eq!(executor.current_tag(), replayed_tag);
        assert_eq!(service.active_tag, replayed_tag);
        assert!(!executor.has_pending_lifecycle_output_admissions());
        assert!(matches!(owner.dispatch_completion_for_test(service, executor, 0)
            .expect("dispatch the WAL-backed Prepare successor"),
            crate::sumeragi::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::SignQueued { .. }));
        let task = match planner_io.command_rx.try_recv() {
            Ok(V2IoCommand::RecoveredLifecycleSign(task)) => task,
            _ => panic!("Proposal publication must continue with its genuine Prepare Sign carrier"),
        };
        assert_eq!(task.tag, replayed_tag);
        assert!(matches!(&task.request, SignRequest::Vote(vote)
            if vote.phase == wire::GlobalPhase::Prepare
                && vote.round == proposal_round && vote.subject == proposal_subject));
        let key = task.dispatch_key();
        assert!(key.lifecycle_ordinal() > proposal_ordinal);
        let result = sign_recovered_lifecycle_task(
            &planner_io.body_store, &context, &service.key_pair, task,
        ).expect("sign the actual next Prepare vote");
        planner_io.command_rx.complete_recovered_lifecycle_sign(key, &result)
            .expect("retain the exact Prepare worker result");
        try_send_tracked_completion_with_lifecycle_ordinal(
            &planner_io.completion_tx, &planner_io.admission,
            V2IoCompletion::RecoveredLifecycleSign(Box::new(
                GuardedRecoveredLifecycleSignWorkerResultV1::new(result, Arc::clone(&output_guard)),
            )), Some(key.lifecycle_ordinal()),
        ).expect("return the actual next Prepare completion");
    });
    assert!(
        launched
            .retain_recovered_lifecycle_sign_completion()
            .expect("retain the next Prepare completion")
    );
    assert_eq!(launched.settle_recovered_lifecycle_sign_broadcast(),
        crate::sumeragi::v2_lifecycle_coordinator::ProductionRecoveredLifecycleSignBroadcastSettlementV1::Applied);
    let admitted_posts = admitted_posts
        .lock()
        .expect("inspect admitted replay outputs");
    let mut proposal_targets = BTreeSet::new();
    let mut chunk_targets = BTreeSet::new();
    let mut chunk_indices_by_target = BTreeMap::<_, BTreeSet<_>>::new();
    let validated_manifest =
        wire::ValidatedPayloadManifest::new(&context, payload.manifest().clone())
            .expect("authenticate the exact recovered chunk manifest");
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
                assert!(
                    matches!(&proposal.justification, wire::ProposalJustification::Timeout(timeout)
                    if timeout.timeout_certificate == timeout_certificate)
                );
                Signature::try_from_bytes(&proposal.signature)
                    .expect("canonical recovered Proposal signature")
                    .verify(
                        keys[local_index].public_key(),
                        &proposal.signature_preimage(),
                    )
                    .expect("the recovered Proposal is signed by the exact nonzero-view leader");
                assert!(proposal_targets.insert(post.peer_id.clone()));
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {
                assert_eq!(chunk.manifest_hash, HashOf::new(payload.manifest()));
                let authenticated_chunk = chunk
                    .validate_for_authentication(&validated_manifest)
                    .expect("the recovered chunk belongs to the exact canonical manifest");
                Signature::try_from_bytes(&chunk.signature)
                    .expect("canonical recovered chunk signature")
                    .verify(
                        keys[local_index].public_key(),
                        &authenticated_chunk.signature_preimage(),
                    )
                    .expect("the recovered chunk is signed by the exact nonzero-view leader");
                assert!(
                    chunk_indices_by_target
                        .entry(post.peer_id.clone())
                        .or_default()
                        .insert(chunk.index),
                    "each canonical chunk reaches each recovered remote target exactly once"
                );
                chunk_targets.insert(post.peer_id.clone());
            }
            payload => panic!("unexpected replay output payload: {payload:?}"),
        }
    }
    assert_eq!(proposal_targets, expected_targets);
    assert_eq!(chunk_targets, expected_chunk_targets);
    for indices in chunk_indices_by_target.values() {
        assert_eq!(
            indices.len(),
            payload.manifest().chunk_hashes.len(),
            "every remote voter receives the complete recovered canonical chunk set"
        );
    }
    drop(admitted_posts);
    launched.detach_ready_sign_planner_for_test(*planner_io);
    drop(launched);
    assert!(!output_guard.restart_required());
}
