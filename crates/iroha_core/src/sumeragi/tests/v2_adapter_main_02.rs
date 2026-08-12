#[test]
fn direct_certified_body_busy_wait_observes_monotone_reducer_fence() {
    let directory = TempDir::new().expect("temporary direct-fence directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0xA2);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, subject))
        .expect("accept proposal")
        .into_effects();
    let (fetch_tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };

    let timeout_tag = adapter.current_tag();
    let sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("persist timeout intent")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(_),
        }] if *tag == timeout_tag
    ));
    let blocked_generation = adapter.reducer_fence_generation();
    let DirectCertifiedBodyAvailablePreparation::Blocked(wait) = adapter
        .prepare_direct_certified_body_available(fetch_tag, &manifest)
        .expect("classify persistence/signature-fenced body completion")
    else {
        panic!("active signature work must return an explicit reducer-fence wait")
    };
    assert_eq!(wait.context_id(), manifest.round.context_id);
    assert_eq!(wait.generation(), blocked_generation);
    drop(wait);

    adapter
        .signature_completed(timeout_tag, vec![0xA2; 96])
        .expect("complete exact timeout signature");
    assert!(adapter.reducer_fence_generation() > blocked_generation);
    assert!(matches!(
        adapter
            .prepare_direct_certified_body_available(fetch_tag, &manifest)
            .expect("retry after the observed fence advances"),
        DirectCertifiedBodyAvailablePreparation::Applied(_)
    ));
}

#[test]
fn reducer_fence_generation_reserves_max_for_coordinator_overflow_detection() {
    let directory = TempDir::new().expect("temporary reducer-fence-overflow directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    adapter.reducer_fence_generation = u64::MAX - 1;

    assert!(matches!(
        adapter.timeout_elapsed(adapter.current_tag()),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer_fence_generation, u64::MAX - 1);
    assert!(adapter.fail_closed);
}

#[test]
fn pacemaker_certificate_stays_queued_until_exact_wal_acknowledgement() {
    use super::super::v2_runtime::{
        RuntimeQueueConfig, RuntimeSelectedCandidateOwnership, RuntimeSelectedOwnerKind,
        RuntimeStep, SerializedV2Runtime,
    };

    let directory = TempDir::new().expect("temporary pending-WAL directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let pending = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        })
        .expect("stage one real TimeoutIntent persistence fence");
    assert!(matches!(
        pending.effects(),
        [reducer::Effect::Persist { .. }]
    ));
    assert!(adapter.pacemaker_escape_is_parked());
    assert!(!adapter.signature_fence_is_active());

    let wire_context = adapter.wire_context.clone();
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic pending-WAL key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert!(
        keys.iter()
            .zip(&wire_context.roster)
            .all(|(key, validator)| key.public_key() == validator.validator.public_key())
    );
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let signers = vec![0, 1, 2];
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate pending-WAL timeout certificate"),
            }],
        }),
    );

    let now = Instant::now();
    let (mut runtime, startup) = SerializedV2Runtime::new(
        adapter,
        startup,
        now,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(4, 1, 1),
    )
    .expect("construct runtime across the pending persistence cut");
    assert!(startup.is_empty());
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime while persistence owns dispatch");
    runtime
        .enqueue_network(certificate)
        .expect("admit the authenticated TC behind the WAL fence");
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        runtime
            .try_step_pacemaker_escape(now)
            .expect("parked pacemaker observation remains valid")
            .is_none(),
        "certified progress cannot cross an unacknowledged safety write"
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(runtime.last_scheduler_ownership().is_none());

    let post_persist = runtime
        .driver_mut_for_test()
        .drive_effects(pending.into_effects())
        .expect("append, fsync, and acknowledge the exact TimeoutIntent");
    assert!(matches!(
        post_persist.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    runtime
        .observe_effects_with_test_ownership(now, &post_persist)
        .expect("retain the signer effect's runtime owner");
    assert!(!runtime.driver().pacemaker_escape_is_parked());
    assert!(runtime.driver().signature_fence_is_active());

    let escaped = runtime
        .try_step_pacemaker_escape(now)
        .expect("post-ack pacemaker selection remains exact")
        .expect("the queued TC advances after its WAL predecessor");
    let RuntimeStep::Advanced(effects) = escaped else {
        panic!("the post-ack TC unexpectedly idled")
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == 1
    ));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("post-ack TC retains one exact scheduler owner");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert!(matches!(
        evidence.candidate,
        RuntimeSelectedCandidateOwnership::Exact(_)
    ));
    assert_eq!(evidence.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(effects.len())
        .expect("consume the post-ack EnterView ownership");
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.driver().fail_closed);
}

#[test]
fn tc_promoted_lock_requires_same_subject_reproposal_before_commit() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let subject = subject(0x97);
    let payload = [0x97, 2];
    let manifest = encode_payload(&adapter.wire_context, round, subject, &payload)
        .expect("encode certified-body payload")
        .manifest()
        .clone();
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let execution_commitment = validated.execution_commitment();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers: vec![1, 2, 3],
        aggregate_signature: vec![0x97; 96],
    };

    let timeout_tag = adapter.current_tag();
    let timeout_sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("persist a local timeout without the remote PrepareQC")
        .into_effects();
    assert!(matches!(
        timeout_sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        }] if *tag == timeout_tag && vote.highest_prepare_qc.is_none()
    ));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    adapter
        .signature_completed(timeout_tag, vec![0xA7; 96])
        .expect("complete the timeout vote before installing the remote TC");

    let timeout = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare.clone()),
            signers: vec![1, 2, 3],
            aggregate_signature: vec![0xB7; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install the TC carrying a PrepareQC missed by this validator")
        .into_effects();
    assert_eq!(adapter.wal.recovered_records().len(), 2);
    assert!(
        installed
            .iter()
            .all(|effect| !matches!(effect, AdapterEffect::Sign { .. })),
        "the TC cannot expose Commit signing before local body validation"
    );
    let fetch_tag = match installed.as_slice() {
        [
            AdapterEffect::EnterView {
                tag: enter_tag,
                protected_lock: Some(protected_lock),
                ..
            },
            AdapterEffect::FetchBody {
                tag,
                round: fetched_round,
                subject: fetched_subject,
                certificate: Some(certificate),
                ..
            },
        ] if enter_tag == tag
            && protected_lock == &prepare
            && *fetched_round == round
            && *fetched_subject == subject
            && certificate.as_ref() == prepare.as_ref() =>
        {
            *tag
        }
        effects => panic!(
            "TC acknowledgement must expose EnterView before its exact body fetch: {effects:?}"
        ),
    };

    assert!(matches!(
        adapter
            .body_available(fetch_tag, manifest)
            .expect("recover the TC-protected body")
            .effects(),
        [AdapterEffect::StoreBody {
            tag,
            round: stored_round,
            subject: stored_subject,
        }] if *tag == fetch_tag
            && *stored_round == round
            && *stored_subject == subject
    ));
    assert!(matches!(
        adapter
            .body_stored(fetch_tag, round, subject, &durable)
            .expect("store the TC-protected body")
            .effects(),
        [AdapterEffect::ValidateBody {
            tag,
            round: validated_round,
            subject: validated_subject,
        }] if *tag == fetch_tag
            && *validated_round == round
            && *validated_subject == subject
    ));
    let validation = adapter
        .validation_succeeded(fetch_tag, round, subject, &validated)
        .expect("validate the TC-protected body without relabelling its origin")
        .into_effects();
    let current_round = wire::ConsensusRound {
        view: fetch_tag.view(),
        ..round
    };
    assert_eq!(
        current_round.view,
        round.view + 1,
        "the TC installs the successor proposal view"
    );
    assert!(
        validation.is_empty(),
        "validating an old-round lock cannot mint a split-round Commit vote: {validation:?}"
    );
    assert_eq!(
        adapter.wal.recovered_records().len(),
        2,
        "validation must not append LockAndCommit until the immutable body is re-proposed"
    );
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
    let core_current_round = reducer::Round::new(current_round.height, current_round.view);
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .commit_intent(core_current_round),
        None,
        "only a new same-round PrepareQC may authorize Commit in the successor view"
    );
    let status = adapter.status().expect("protected reproposal status");
    assert!(status.liveness.outbound_intents.iter().all(|intent| {
        !matches!(
            intent.kind,
            wire::SumeragiV2OutboundIntentKind::CommitVote
                | wire::SumeragiV2OutboundIntentKind::CommitQc
        )
    }));
}

#[test]
fn leader_without_owned_candidate_work_reports_missing_proposal_state() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader adapter");
    assert!(startup.is_empty());
    let status = adapter.status().expect("fresh leader status");
    let local = adapter
        .registry
        .validator_index(
            adapter
                .reducer
                .local_validator()
                .expect("fixture has a local validator"),
        )
        .expect("map local validator");
    assert_eq!(status.leader, local, "fixture local validator is leader");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Idle,
        "leadership alone is not ownership of candidate construction"
    );
    assert_eq!(status.phase, wire::SumeragiV2StatusPhase::AwaitingProposal);
}

#[test]
fn one_round_and_subject_cannot_change_its_registered_manifest() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3D);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, subject))
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
    adapter
        .body_available(tag, manifest.clone())
        .expect("register exact manifest");
    let alternate_body = b"other";
    let alternate_chunks =
        wire::encode_payload_chunks(adapter.wire_context.da_layout, alternate_body)
            .expect("encode complete canonical alternate-body chunks");
    // Deliberately bind the complete canonical alternate body to the
    // original subject so this remains a manifest-conflict negative.
    let conflicting = wire::PayloadManifest::derive(
        &adapter.wire_context,
        manifest.round,
        manifest.subject,
        u64::try_from(alternate_body.len()).expect("alternate body length fits u64"),
        &alternate_chunks,
    )
    .expect("structurally valid conflicting manifest");

    assert!(matches!(
        adapter.body_available(tag, conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
}

#[test]
fn authenticated_proposal_cannot_conflict_with_registered_canonical_manifest() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let context = adapter.wire_context.clone();
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3E);
    let canonical = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload else {
        panic!("fixture is a proposal")
    };
    adapter
        .registry
        .manifest_to_core(&canonical_proposal.manifest, &context)
        .expect("register canonical body manifest before proposal arrival");

    let canonical = AuthenticatedConsensusMessage::for_test(canonical);
    adapter
        .ensure_authenticated_manifest_compatible(&canonical)
        .expect("the exact registered manifest remains admissible");

    let mut conflicting = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) = &mut conflicting.payload
    else {
        panic!("fixture is a proposal")
    };
    let alternate_body = b"other";
    let alternate_chunks = wire::encode_payload_chunks(context.da_layout, alternate_body)
        .expect("encode complete canonical alternate-body chunks");
    // Deliberately bind the complete canonical alternate body to the
    // original subject so this remains a manifest-conflict negative.
    conflicting_proposal.manifest = wire::PayloadManifest::derive(
        &context,
        conflicting_proposal.round,
        conflicting_proposal.subject,
        u64::try_from(alternate_body.len()).expect("alternate body length fits u64"),
        &alternate_chunks,
    )
    .expect("structurally valid alternate manifest");
    let conflicting = AuthenticatedConsensusMessage::for_test(conflicting);
    assert!(matches!(
        adapter.ensure_authenticated_manifest_compatible(&conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
    assert!(!adapter.fail_closed);
}
