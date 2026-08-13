fn pending_validate_binding_for_test(
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    certificate: Option<wire::QuorumCertificate>,
    owner_marker: u128,
) -> (AdapterEffect, PendingRuntimeEffectBinding) {
    let store = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let store_binding = if let Some(certificate) = certificate {
        let fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: None,
            certified_sources: Vec::new(),
            certificate: Some(certificate),
        };
        let fetch_binding = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, owner_marker)],
        )
        .expect("bind one certified Fetch fixture")
        .pop()
        .expect("one certified Fetch fixture owner")
        .pending_adapter_effect_binding(&fetch)
        .expect("certified Fetch fixture mints one pending binding");
        fetch_binding
            .project_certified_fetch_store_successor(&fetch, &store)
            .expect("certified Fetch fixture derives Store")
    } else {
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&store),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, owner_marker)],
        )
        .expect("bind one ordinary Store fixture")
        .pop()
        .expect("one ordinary Store fixture owner")
        .pending_adapter_effect_binding(&store)
        .expect("ordinary Store fixture mints one pending binding")
    };
    let validate_binding = store_binding
        .project_store_validate_successor(&store, &validate)
        .expect("Store fixture derives Validate");
    (validate, validate_binding)
}

#[test]
fn live_wal_payload_free_pending_roots_bind_all_five_stages_and_exact_frames() {
    let (context, keys) = authenticated_runtime_context();
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        signed_runtime_proposal(&context, &keys, 0x68).payload
    else {
        unreachable!("runtime proposal fixture")
    };
    proposal.signature.clear();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x69,
        wire::GlobalPhase::Prepare,
    );
    let tag = EventTag::new(context.height, 0, Generation::new(9));
    let prepare_vote = wire::Vote {
        round: prepare.round,
        proposal_round: prepare.proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: prepare.subject,
        execution_commitment: prepare.execution_commitment,
        signer: prepare.signers[0],
        signature: Vec::new(),
    };
    let commit_vote = wire::Vote {
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let timeout_vote = wire::TimeoutVote {
        round: prepare.round,
        highest_prepare_qc: Some(prepare),
        signer: 0,
        signature: Vec::new(),
    };
    let enter = wire::TimeoutCertificate {
        round: timeout_vote.round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: timeout_vote.highest_prepare_qc.clone(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x69; 96],
        }],
    };
    let effects = [
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(prepare_vote),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(commit_vote),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(timeout_vote),
        },
        AdapterEffect::EnterView {
            tag,
            certificate: enter,
            protected_lock: None,
        },
    ];
    let mut roots = Vec::new();
    for (index, effect) in effects.iter().enumerate() {
        let sequence = u64::try_from(index).expect("small stage index");
        let frame_hash = if index == 0 {
            [0; 32]
        } else {
            [u8::try_from(index).expect("small stage marker"); 32]
        };
        let identity = LiveWalFrameIdentity::for_test(
            sequence,
            sequence.checked_add(1).expect("bounded persistence id"),
            frame_hash,
        );
        let pending = PendingRuntimeEffectBinding::from_exact_live_wal_append(&identity, effect)
            .expect("exact payload-free live WAL effect derives one pending owner");
        assert!(pending.exactly_binds_adapter_effect(effect));
        roots.push(*pending.causal_lifecycle_key());
    }
    assert_eq!(roots.iter().collect::<BTreeSet<_>>().len(), effects.len());

    let first = LiveWalFrameIdentity::for_test(9, 10, [0; 32]);
    let second = LiveWalFrameIdentity::for_test(10, 11, [0; 32]);
    let first_pending =
        PendingRuntimeEffectBinding::from_exact_live_wal_append(&first, &effects[0])
            .expect("zero-valued digest remains structurally valid");
    let second_pending =
        PendingRuntimeEffectBinding::from_exact_live_wal_append(&second, &effects[0])
            .expect("second exact locator derives a pending owner");
    assert_ne!(
        first_pending.causal_lifecycle_key(),
        second_pending.causal_lifecycle_key(),
        "identical effects from different exact WAL frames cannot share causal authority"
    );
}

#[test]
fn pending_validate_projects_exact_prepare_commit_and_report_successors() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6B,
        wire::GlobalPhase::Prepare,
    );
    let tag = EventTag::new(context.height, prepare.round.view, Generation::new(4));
    let (validate, ordinary_validate) =
        pending_validate_binding_for_test(tag, prepare.proposal_round, prepare.subject, None, 74);
    let (_, prepare_validate) = pending_validate_binding_for_test(
        tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        75,
    );

    let prepare_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let prepare_sign_binding = ordinary_validate
        .project_validate_sign_prepare_successor(&validate, &prepare_sign)
        .expect("ordinary Validate acquires exact Prepare-vote authority");
    assert!(prepare_sign_binding.exactly_binds_adapter_effect(&prepare_sign));
    assert_eq!(
        prepare_sign_binding.causal_lifecycle_key(),
        ordinary_validate.causal_lifecycle_key()
    );
    let prepare_statement = prepare_sign_binding
        .candidate_statement()
        .expect("Prepare Sign carries one candidate statement");
    assert_eq!(prepare_statement.phase(), Some(wire::GlobalPhase::Prepare));
    assert_eq!(
        prepare_statement.execution_commitment(),
        Some(prepare.execution_commitment)
    );

    let commit_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let commit_sign_binding = prepare_validate
        .project_validate_sign_commit_successor(&validate, &commit_sign)
        .expect("Prepare-authorized Validate promotes to exact Commit vote");
    assert!(commit_sign_binding.exactly_binds_adapter_effect(&commit_sign));
    assert_eq!(
        commit_sign_binding.causal_lifecycle_key(),
        prepare_validate.causal_lifecycle_key()
    );
    let commit_statement = commit_sign_binding
        .candidate_statement()
        .expect("Commit Sign carries one candidate statement");
    assert_eq!(commit_statement.phase(), Some(wire::GlobalPhase::Commit));
    assert_eq!(
        commit_statement.execution_commitment(),
        Some(prepare.execution_commitment)
    );

    let report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: prepare,
    };
    let report_binding = prepare_validate
        .project_validate_report_invalid_certified_body_successor(&validate, &report)
        .expect("Prepare-authorized Validate derives its exact invalid-body report");
    assert!(report_binding.exactly_binds_adapter_effect(&report));
    assert_eq!(
        report_binding.causal_lifecycle_key(),
        prepare_validate.causal_lifecycle_key()
    );
    assert_eq!(report_binding.candidate_statement(), None);
    assert_ne!(
        report_binding.exact_effect_identity(),
        prepare_validate.exact_effect_identity()
    );
}

#[test]
fn recovered_commit_retags_monotonically_without_widening_live_projection() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6C,
        wire::GlobalPhase::Prepare,
    );
    let original_tag = EventTag::new(context.height, prepare.round.view, Generation::new(12));
    let current_tag = EventTag::new(
        context.height,
        prepare.round.view + 1,
        original_tag.generation(),
    );
    let (validate, ordinary_validate) = pending_validate_binding_for_test(
        original_tag,
        prepare.proposal_round,
        prepare.subject,
        None,
        76,
    );
    let (_, prepare_validate) = pending_validate_binding_for_test(
        original_tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        77,
    );
    let historical_commit = AdapterEffect::Sign {
        tag: current_tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };

    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &historical_commit)
            .is_none(),
        "the live inherited-Prepare projection retains exact tag equality"
    );
    assert!(
        prepare_validate
            .project_recovered_inherited_validate_commit_successor(
                &validate,
                &historical_commit,
                &prepare,
            )
            .is_some(),
        "sealed recovery may emit the exact old Commit under a later current tag"
    );
    assert!(
        ordinary_validate
            .project_recovered_ordinary_validate_commit_successor(
                &validate,
                &historical_commit,
                &prepare,
            )
            .is_some(),
        "the recovered ordinary-Validate refinement uses the same bounded relation"
    );

    let AdapterEffect::Sign { request, .. } = &historical_commit else {
        unreachable!("historical Commit fixture is a Sign effect")
    };
    let foreign_generation_commit = AdapterEffect::Sign {
        tag: EventTag::new(
            current_tag.height(),
            current_tag.view(),
            Generation::new(current_tag.generation().get() + 1),
        ),
        request: request.clone(),
    };
    assert!(
        prepare_validate
            .project_recovered_inherited_validate_commit_successor(
                &validate,
                &foreign_generation_commit,
                &prepare,
            )
            .is_none(),
        "recovery cannot cross a reducer generation"
    );
}

#[test]
fn pending_validate_successor_projection_rejects_forged_coordinates_and_authority() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6C,
        wire::GlobalPhase::Prepare,
    );
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x6C);
    let tag = EventTag::new(context.height, prepare.round.view, Generation::new(5));
    let (validate, ordinary_validate) =
        pending_validate_binding_for_test(tag, prepare.proposal_round, prepare.subject, None, 76);
    let (_, prepare_validate) = pending_validate_binding_for_test(
        tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        77,
    );
    let store = AdapterEffect::StoreBody {
        tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
    };
    let prepare_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let commit_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: prepare.clone(),
    };

    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&validate, &commit_sign)
            .is_none(),
        "Prepare projection rejects a Commit vote"
    );
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &prepare_sign)
            .is_none(),
        "Commit projection rejects a Prepare vote"
    );
    let commit_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: commit.subject,
        certificate: commit,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&validate, &commit_report,)
            .is_none(),
        "invalid-body reporting requires Prepare rather than Commit authority"
    );

    let foreign = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6D,
        wire::GlobalPhase::Prepare,
    );
    let changed_commitment_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: foreign.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &changed_commitment_sign)
            .is_none(),
        "Commit vote cannot change the registered Prepare commitment"
    );
    let mut changed_commitment_certificate = prepare.clone();
    changed_commitment_certificate.execution_commitment = foreign.execution_commitment;
    let changed_commitment_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: changed_commitment_certificate,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(
                &validate,
                &changed_commitment_report,
            )
            .is_none(),
        "report cannot change the registered Prepare commitment"
    );

    let wrong_tag_prepare = AdapterEffect::Sign {
        tag: EventTag::new(context.height, prepare.round.view, Generation::new(6)),
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&validate, &wrong_tag_prepare)
            .is_none(),
        "Sign successor must retain the complete predecessor tag"
    );
    let wrong_tag_validate = AdapterEffect::ValidateBody {
        tag: EventTag::new(context.height, prepare.round.view, Generation::new(6)),
        round: prepare.proposal_round,
        subject: prepare.subject,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&wrong_tag_validate, &report,)
            .is_none(),
        "report projection requires the exactly bound Validate tag"
    );

    let wrong_subject_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: foreign.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &wrong_subject_sign)
            .is_none(),
        "Sign successor cannot change the validated subject"
    );
    let mut wrong_subject_certificate = prepare.clone();
    wrong_subject_certificate.subject = foreign.subject;
    let wrong_subject_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: foreign.subject,
        certificate: wrong_subject_certificate,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(
                &validate,
                &wrong_subject_report,
            )
            .is_none(),
        "report cannot change the validated subject"
    );

    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&store, &prepare_sign)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&store, &commit_sign)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&store, &report)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        ordinary_validate
            .project_validate_sign_commit_successor(&validate, &commit_sign)
            .is_none(),
        "ordinary Validate needs an opaque concurrent-Prepare refinement capability"
    );
    assert!(
        ordinary_validate
            .project_validate_report_invalid_certified_body_successor(&validate, &report)
            .is_none(),
        "ordinary Validate needs an opaque registered-report carrier capability"
    );
    assert!(
        prepare_validate
            .project_validate_sign_prepare_successor(&validate, &prepare_sign)
            .is_none(),
        "Prepare-authorized Validate cannot regress to the ordinary Prepare-sign branch"
    );
}

#[test]
fn restored_high_watermark_exhaustion_fails_without_erasing_the_source() {
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 1);

    assert!(source.advance_past(u128::MAX).is_err());
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after rejected restored high-watermark"),
        Some(u128::MAX),
        "a rejected restored high-watermark must not turn exhaustion into an empty source"
    );

    let already_exhausted = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX);
    assert!(already_exhausted.advance_past(0).is_err());
    assert_eq!(
        already_exhausted
            .next_ordinal_for_test()
            .expect("inspect an already exhausted restored source"),
        None
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn authenticated_remote_proposal_retains_exact_fetch_store_validate_replay_origin() {
    let directory = TempDir::new().expect("temporary remote Proposal replay directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for authenticated Proposal replay");

    let proposal = signed_runtime_proposal(&context, &keys, 0xA7);
    let mut wrong_signature = proposal.clone();
    let wire::ConsensusMessageV2Payload::Proposal(wrong) = &mut wrong_signature.payload else {
        unreachable!("signed Proposal fixture has one Proposal payload")
    };
    wrong.signature[0] ^= 0xFF;
    assert!(
        runtime.enqueue_network(wrong_signature).is_err(),
        "a substituted signature cannot mint remote Proposal replay authority"
    );
    runtime
        .enqueue_network(proposal)
        .expect("enqueue exact authenticated Proposal");
    let RuntimeStep::Advanced(fetch_effects) = runtime.step(now).expect("dispatch Proposal") else {
        panic!("authenticated Proposal unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("Proposal publishes scheduler ownership");
    let [fetch_effect] = fetch_effects.as_slice() else {
        panic!("Proposal must emit one exact Fetch: {fetch_effects:?}")
    };
    let AdapterEffect::FetchBody {
        tag,
        manifest: Some(manifest),
        certificate: None,
        certified_sources,
        ..
    } = fetch_effect
    else {
        panic!("authenticated ordinary Proposal must emit manifest-bound Fetch")
    };
    assert!(certified_sources.is_empty());
    let tag = *tag;
    let manifest = manifest.clone();
    let fetch_ownership = runtime
        .take_effect_ownership(fetch_effects.len())
        .expect("Fetch retains exact runtime ownership")
        .pop()
        .expect("one Fetch has one exact owner");
    let fetch_pending = fetch_ownership
        .pending_adapter_effect_binding(fetch_effect)
        .expect("Fetch owns one exact pending binding");
    let fetch_replay = fetch_ownership
        .exact_remote_proposal_fetch_replay(fetch_effect)
        .expect("authenticated Proposal attaches its replay origin");
    assert!(fetch_replay.exactly_matches_fetch_pending(fetch_effect, &fetch_pending));
    assert!(fetch_replay.exactly_matches_retry(&fetch_replay.clone(), fetch_effect,));
    let mut foreign_manifest_fetch = fetch_effect.clone();
    let AdapterEffect::FetchBody {
        manifest: Some(foreign_manifest),
        ..
    } = &mut foreign_manifest_fetch
    else {
        unreachable!("ordinary Fetch fixture retains one manifest")
    };
    foreign_manifest.chunk_root = Hash::new(b"foreign remote Proposal manifest root");
    assert!(
        fetch_ownership
            .exact_remote_proposal_fetch_replay(&foreign_manifest_fetch)
            .is_none()
    );
    let mut certified_fetch = fetch_effect.clone();
    let AdapterEffect::FetchBody { certificate, .. } = &mut certified_fetch else {
        unreachable!("fixture remains FetchBody")
    };
    *certificate = Some(signed_runtime_quorum_certificate(&context, &keys, 0xA8));
    assert!(
        fetch_ownership
            .exact_remote_proposal_fetch_replay(&certified_fetch)
            .is_none(),
        "certified Fetch cannot inherit ordinary Proposal replay origin"
    );
    let _proposal_terminals = runtime.take_leader_wire_runtime_terminals();

    let reservation = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &fetch_ownership)
        .expect("reserve exact reconstructed body");
    runtime
        .commit_body_available(reservation)
        .expect("publish exact BodyAvailable successor");
    let RuntimeStep::Advanced(store_effects) = runtime.step(now).expect("dispatch BodyAvailable")
    else {
        panic!("BodyAvailable unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyAvailable publishes scheduler ownership");
    let [store_effect] = store_effects.as_slice() else {
        panic!("BodyAvailable must emit one Store: {store_effects:?}")
    };
    let store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("Store retains exact Fetch causal owner")
        .pop()
        .expect("one Store has one exact owner");
    let store_pending = store_ownership
        .pending_adapter_effect_binding(store_effect)
        .expect("Store owns one exact pending binding");
    assert!(fetch_replay.exactly_projects_store(store_effect, &store_pending));
    let foreign_store_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 98_001)],
    )
    .expect("bind foreign Store root")
    .pop()
    .expect("one foreign Store owner");
    let foreign_store_pending = foreign_store_ownership
        .pending_adapter_effect_binding(store_effect)
        .expect("foreign Store has one binding");
    assert!(
        fetch_replay
            .clone()
            .project_exact_store(store_effect, &foreign_store_pending)
            .is_err(),
        "matching coordinates cannot splice a foreign causal root"
    );
    let Ok(store_replay) = fetch_replay.project_exact_store(store_effect, &store_pending) else {
        panic!("exact Fetch owner projects one Store replay carrier")
    };
    assert!(store_replay.exactly_matches_store_pending(store_effect, &store_pending));

    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.chunk_root = Hash::new(b"foreign durable remote Proposal frame");
    let foreign_durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&foreign_manifest),
    );
    assert!(
        store_replay
            .clone()
            .bind_durable_body(store_effect, &foreign_durable)
            .is_err(),
        "a substituted durable BodyFrame cannot complete Store replay evidence"
    );
    let Ok(stored_replay) = store_replay.bind_durable_body(store_effect, &durable) else {
        panic!("exact durable receipt completes Store replay evidence")
    };
    assert!(stored_replay.exactly_matches_store(store_effect, &durable));

    runtime
        .enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &store_ownership,
        )
        .expect("enqueue exact durable Store completion");
    let RuntimeStep::Advanced(validate_effects) = runtime.step(now).expect("dispatch BodyStored")
    else {
        panic!("BodyStored unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyStored publishes scheduler ownership");
    let [validate_effect] = validate_effects.as_slice() else {
        panic!("BodyStored must emit one Validate: {validate_effects:?}")
    };
    let validate_ownership = runtime
        .take_effect_ownership(validate_effects.len())
        .expect("Validate retains exact Store causal owner")
        .pop()
        .expect("one Validate has one exact owner");
    let validate_pending = validate_ownership
        .pending_adapter_effect_binding(validate_effect)
        .expect("Validate owns one exact pending binding");
    let foreign_validate_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(validate_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 98_002)],
    )
    .expect("bind foreign Validate root")
    .pop()
    .expect("one foreign Validate owner");
    let foreign_validate_pending = foreign_validate_ownership
        .pending_adapter_effect_binding(validate_effect)
        .expect("foreign Validate has one binding");
    assert!(
        stored_replay
            .clone()
            .project_exact_validate(
                store_effect,
                &durable,
                validate_effect,
                &foreign_validate_pending,
            )
            .is_err(),
        "matching Validate coordinates cannot splice a foreign causal root"
    );
    let Ok(validate_replay) = stored_replay.project_exact_validate(
        store_effect,
        &durable,
        validate_effect,
        &validate_pending,
    ) else {
        panic!("exact Store owner projects one Validate replay carrier")
    };
    assert!(validate_replay.exactly_matches_validate_pending(
        validate_effect,
        &durable,
        &validate_pending,
    ));
    let queued_before_drop = runtime.queued_commands();
    drop(validate_replay);
    assert_eq!(runtime.queued_commands(), queued_before_drop);
    assert!(!runtime.fail_closed);
}
