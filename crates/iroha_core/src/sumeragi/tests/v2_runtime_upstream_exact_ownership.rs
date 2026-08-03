// Upstream exact-ownership and scheduler regressions retained through the merge.

#[test]
fn adapter_effect_binding_is_exact_route_neutral_and_three_bounded() {
    let (context, keys) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x6A);
    let tag = EventTag::new(context.height, 0, Generation::new(1));
    let store = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    assert_eq!(
        production_adapter_effect_kind(&store),
        RUNTIME_EFFECT_KIND_STORE_BODY
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&store)
            .expect("StoreBody is a causal candidate")
            .0,
        RUNTIME_CANDIDATE_KIND_STORE_BODY
    );

    let owner = RuntimeEffectOwnership::fresh_for_test(tag, 71);
    let bound = bind_adapter_effect_batch_ownership(&[store.clone()], vec![owner])
        .expect("one exact StoreBody candidate is within the bound");
    assert!(bound[0].validate_bound_exact());
    let first_owner_projection = production_adapter_effect_candidate_trace_projection(
        &store, &bound[0], 1, 1, 1, 1, 0, 1, true,
    )
    .expect("recompute lossless first-owner projection");
    assert!(check_production_effect_to_candidate_transition(first_owner_projection).is_some());
    assert!(first_owner_projection.candidate_owner_admitted);
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&store, 0, 1),
        Ok(RuntimeCandidateAdmissionDisposition::FirstAdmission)
    );

    let retry_owner = RuntimeEffectOwnership::fresh_for_test(tag, 71);
    let retry = bind_adapter_effect_batch_ownership(&[store.clone()], vec![retry_owner])
        .expect("same exact producer retry remains bindable");
    assert_eq!(bound[0].candidate_identity(), retry[0].candidate_identity());
    let retry_projection = production_adapter_effect_candidate_trace_projection(
        &store, &retry[0], 1, 1, 1, 1, 1, 1, true,
    )
    .expect("recompute coalesced retry projection");
    assert!(check_production_effect_to_candidate_transition(retry_projection).is_some());
    assert!(!retry_projection.candidate_owner_admitted);
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&store, 1, 1),
        Ok(RuntimeCandidateAdmissionDisposition::CoalescedRetry)
    );

    let diagnostic = AdapterEffect::ReportInvalidCertifiedBody {
        subject: manifest.subject,
        certificate: signed_runtime_quorum_certificate(&context, &keys, 0x6B),
    };
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&diagnostic, 0, 0),
        Ok(RuntimeCandidateAdmissionDisposition::NonCandidate)
    );
    for invalid in [(0, 0), (1, 0), (0, 2), (2, 1)] {
        assert!(
                production_adapter_effect_candidate_admission_disposition(
                    &store, invalid.0, invalid.1,
                )
                .is_err(),
                "candidate count mutation {invalid:?} must fail closed"
            );
    }
    assert!(
        production_adapter_effect_candidate_admission_disposition(&diagnostic, 0, 1).is_err(),
        "a non-candidate cannot mint an owner"
    );

    let changed_tag = EventTag::new(context.height, 0, Generation::new(2));
    let changed = AdapterEffect::StoreBody {
        tag: changed_tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    assert_ne!(
        production_adapter_effect_semantic_identity(&store),
        production_adapter_effect_semantic_identity(&changed)
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&store),
        production_adapter_effect_candidate_semantic_identity(&changed),
        "process-local generation is absent from abstract candidate identity"
    );

    let changed_subject = AdapterEffect::StoreBody {
        tag: changed_tag,
        round: manifest.round,
        subject: wire::BlockSubject {
            payload_hash: Hash::new(b"changed candidate payload"),
            ..manifest.subject
        },
    };
    assert_ne!(
        production_adapter_effect_candidate_semantic_identity(&store),
        production_adapter_effect_candidate_semantic_identity(&changed_subject),
        "the immutable subject remains part of abstract candidate identity"
    );

    let sources = keys[..2]
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut reversed_sources = sources.clone();
    reversed_sources.reverse();
    let fetch = |certified_sources| AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources,
        certificate: None,
    };
    let first_route = fetch(sources);
    let second_route = fetch(reversed_sources);
    assert_ne!(
        production_adapter_effect_semantic_identity(&first_route),
        production_adapter_effect_semantic_identity(&second_route),
        "the exact transport effect includes ordered destinations"
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&first_route),
        production_adapter_effect_candidate_semantic_identity(&second_route),
        "transport retries retain one route-neutral abstract candidate"
    );

    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0x73);
    let apply = AdapterEffect::Apply {
        tag,
        subject: certificate.subject,
        certificate: certificate.clone(),
    };
    let mut alternate_carrier = certificate.clone();
    alternate_carrier.signers.reverse();
    alternate_carrier.aggregate_signature = vec![0xA7; 96];
    let alternate_apply = AdapterEffect::Apply {
        tag: changed_tag,
        subject: alternate_carrier.subject,
        certificate: alternate_carrier,
    };
    assert_ne!(
        production_adapter_effect_semantic_identity(&apply),
        production_adapter_effect_semantic_identity(&alternate_apply),
        "concrete effect identity retains signer and aggregate carriers"
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&apply),
        production_adapter_effect_candidate_semantic_identity(&alternate_apply),
        "candidate identity excludes aggregate, signer, and local-incarnation carriers"
    );

    let mut changed_statement = certificate;
    changed_statement.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"changed candidate parent state"),
            Hash::new(b"changed candidate post state"),
            Hash::new(b"changed candidate ordinary writes"),
            1,
            Hash::new(b"changed candidate block wire"),
        );
    let changed_apply = AdapterEffect::Apply {
        tag,
        subject: changed_statement.subject,
        certificate: changed_statement,
    };
    assert_ne!(
        production_adapter_effect_candidate_semantic_identity(&apply),
        production_adapter_effect_candidate_semantic_identity(&changed_apply),
        "execution commitment remains part of the normalized statement"
    );

    let three_candidates = vec![store.clone(), first_route.clone(), apply.clone()];
    let three_owners = (1_u128..=3)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag, 90 + ordinal))
        .collect();
    let three_bound = bind_adapter_effect_batch_ownership(&three_candidates, three_owners)
        .expect("exactly three causal successors remain within the bound");
    assert_eq!(three_bound.len(), 3);
    for (index, (effect, ownership)) in three_candidates.iter().zip(&three_bound).enumerate() {
        let position = u8::try_from(index + 1).expect("three positions fit in u8");
        assert!(ownership.validate_bound_exact());
        let projection = production_adapter_effect_candidate_trace_projection(
            effect, ownership, position, 3, position, 3, 0, 1, true,
        )
        .expect("recompute one of three exact first-admission projections");
        assert!(check_production_effect_to_candidate_transition(projection).is_some());
        assert!(projection.candidate_owner_admitted);
    }

    let four_candidates = vec![store.clone(), store.clone(), store.clone(), store.clone()];
    let four_owners = (1_u128..=4)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag, 100 + ordinal))
        .collect();
    assert!(
        bind_adapter_effect_batch_ownership(&four_candidates, four_owners).is_err(),
        "a fourth causal successor must fail before retention"
    );

    let mut forged = bound[0].clone();
    forged
        .binding
        .as_mut()
        .expect("bound ownership has positional evidence")
        .effect_position = 2;
    assert!(!forged.validate_exact());
    assert!(
        production_adapter_effect_candidate_trace_projection(
            &store, &forged, 1, 1, 1, 1, 0, 1, true,
        )
        .is_err(),
        "positional binding mutation must fail before projection"
    );
}

#[test]
fn body_pipeline_acquires_commit_authority_monotonically_under_one_owner() {
    let (context, keys) = authenticated_runtime_context();
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x75);
    let tag = EventTag::new(context.height, commit.round.view, Generation::new(4));
    let store = AdapterEffect::StoreBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let apply = AdapterEffect::Apply {
        tag,
        subject: commit.subject,
        certificate: commit.clone(),
    };

    // A local proposal or ordinary body fetch has no quorum authority at
    // Store/Validate time. A late durable Decision supplies Commit
    // authority without replacing the body's immutable local owner.
    let local_store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_101)],
    )
    .expect("local Store binds an uncertified body statement")
    .pop()
    .expect("one local Store owner");
    let local_validate = local_store
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("local Validate retains the uncertified statement");
    let local_apply = local_validate
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("late Decision refines local validation to Commit authority");
    assert_eq!(local_store.owner(), local_apply.owner());
    assert_eq!(
        local_validate
            .candidate_semantic_statement()
            .expect("local Validate carries its typed statement")
            .phase,
        None
    );
    assert_eq!(
        local_apply
            .candidate_semantic_statement()
            .expect("local Apply carries its acquired authority")
            .phase,
        Some(wire::GlobalPhase::Commit)
    );

    // A Prepare-certified reconstruction has already frozen the
    // commitment. The matching CommitQC may promote only the phase.
    let mut prepare = commit.clone();
    prepare.phase = wire::GlobalPhase::Prepare;
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(prepare),
    };
    let prepare_fetch = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_102)],
    )
    .expect("Prepare Fetch binds its certified statement")
    .pop()
    .expect("one Prepare Fetch owner");
    let prepare_store = prepare_fetch
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Store retains Prepare authority");
    let prepare_validate = prepare_store
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Validate retains Prepare authority");
    let prepare_apply = prepare_validate
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("matching Decision promotes Prepare to Commit");
    assert_eq!(prepare_fetch.owner(), prepare_apply.owner());
    assert_eq!(
        prepare_validate
            .candidate_semantic_statement()
            .expect("Prepare Validate carries its statement")
            .phase,
        Some(wire::GlobalPhase::Prepare)
    );
    assert_eq!(
        prepare_apply.candidate_semantic_statement(),
        production_adapter_effect_candidate_statement(&apply).map(|(_, statement)| statement)
    );

    let rejects =
        |certificate: wire::QuorumCertificate, subject: wire::BlockSubject, mutation: &str| {
            let changed = AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            };
            assert!(
                prepare_validate
                    .rebind_as_inherited_adapter_effect(&changed)
                    .is_err(),
                "{mutation} must be rejected before candidate refinement"
            );
        };

    let mut changed_subject = commit.clone();
    changed_subject.subject = wire::BlockSubject {
        payload_hash: Hash::new(b"foreign Apply subject"),
        ..changed_subject.subject
    };
    rejects(
        changed_subject.clone(),
        changed_subject.subject,
        "subject drift",
    );
    rejects(
        changed_subject,
        commit.subject,
        "certificate/effect subject disagreement",
    );

    let mut changed_proposal_round = commit.clone();
    changed_proposal_round.proposal_round.view += 1;
    rejects(
        changed_proposal_round,
        commit.subject,
        "proposal-round drift",
    );

    let mut changed_round = commit.clone();
    changed_round.round.view += 1;
    rejects(changed_round, commit.subject, "round drift");

    let foreign_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign Apply height context",
    )));
    let mut changed_context = commit.clone();
    changed_context.round.context_id = foreign_context;
    changed_context.proposal_round.context_id = foreign_context;
    rejects(changed_context, commit.subject, "context drift");

    let mut changed_commitment = commit;
    changed_commitment.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign refinement parent state"),
            Hash::new(b"foreign refinement post state"),
            Hash::new(b"foreign refinement ordinary writes"),
            1,
            Hash::new(b"foreign refinement executed block"),
        );
    let changed_commitment_subject = changed_commitment.subject;
    rejects(
        changed_commitment,
        changed_commitment_subject,
        "commitment drift",
    );
}

#[test]
fn certified_body_pipeline_retains_statement_and_owner_across_stage_kinds() {
    let (context, keys) = authenticated_runtime_context();
    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0x74);
    let tag = EventTag::new(context.height, certificate.round.view, Generation::new(3));
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(certificate.clone()),
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_001)],
    )
    .expect("certified Fetch binds its complete statement")
    .pop()
    .expect("one Fetch owner");
    let statement = fetch_ownership
        .candidate_semantic_statement()
        .expect("production Fetch carries typed statement evidence");
    assert_eq!(statement.phase, Some(wire::GlobalPhase::Commit));
    assert_eq!(
        statement.execution_commitment,
        Some(certificate.execution_commitment)
    );

    let store = AdapterEffect::StoreBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
    };
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Store inherits the certified Fetch statement");
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
    };
    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Validate inherits the certified Store statement");
    let apply = AdapterEffect::Apply {
        tag,
        subject: certificate.subject,
        certificate: certificate.clone(),
    };
    let apply_ownership = validate_ownership
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("Apply retains the exact certified body authority");

    for ownership in [&store_ownership, &validate_ownership, &apply_ownership] {
        assert_eq!(ownership.owner(), fetch_ownership.owner());
        assert_eq!(ownership.candidate_semantic_statement(), Some(statement));
    }
    let stage_identities = [
        fetch_ownership.candidate_semantic_identity(),
        store_ownership.candidate_semantic_identity(),
        validate_ownership.candidate_semantic_identity(),
        apply_ownership.candidate_semantic_identity(),
    ];
    assert!(stage_identities.iter().all(Option::is_some));
    assert_eq!(
        stage_identities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        stage_identities.len(),
        "the outer work kind distinguishes stage occurrences without replacing the owner"
    );

    let mut lost_phase_and_commitment = store_ownership.clone();
    let fresh_store = production_adapter_effect_candidate_statement(&store)
        .expect("Store is a candidate")
        .1;
    lost_phase_and_commitment
        .binding
        .as_mut()
        .expect("Store has exact binding")
        .candidate_statement = Some(fresh_store);
    assert!(
        !lost_phase_and_commitment.validate_exact(),
        "dropping inherited phase and commitment invalidates the sealed binding"
    );

    let wrong_round = wire::ConsensusRound {
        view: certificate.proposal_round.view + 1,
        ..certificate.proposal_round
    };
    let wrong_store = AdapterEffect::StoreBody {
        tag,
        round: wrong_round,
        subject: certificate.subject,
    };
    assert!(
        fetch_ownership
            .rebind_as_inherited_adapter_effect(&wrong_store)
            .is_err(),
        "a causal Store cannot drop or replace the frozen proposal round"
    );

    let mut wrong_certificate = certificate;
    wrong_certificate.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign pipeline parent state"),
            Hash::new(b"foreign pipeline post state"),
            Hash::new(b"foreign pipeline ordinary writes"),
            1,
            Hash::new(b"foreign pipeline executed block"),
        );
    let wrong_apply = AdapterEffect::Apply {
        tag,
        subject: wrong_certificate.subject,
        certificate: wrong_certificate,
    };
    assert!(
        validate_ownership
            .rebind_as_inherited_adapter_effect(&wrong_apply)
            .is_err(),
        "Apply cannot replace the inherited execution commitment"
    );
}

#[test]
fn fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers() {
    let directory = TempDir::new().expect("temporary real-adapter ordering directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let start = Instant::now();
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime after adapter startup");

    // Service one complete periodic episode before the signer becomes
    // busy. A later tick must mint a new lifecycle ordinal rather than
    // resurrecting this drained episode at its old position.
    let before_timeout = start + runtime.retransmit_interval();
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("service pre-fence retransmission"),
        RuntimeStep::Advanced(_)
    ));

    let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
    runtime
        .enqueue_network(proposal)
        .expect("enqueue authenticated proposal");
    let proposal_effects = match runtime
        .step_and_take_scheduler_ownership_for_test(before_timeout)
        .expect("dispatch authenticated proposal")
    {
        RuntimeStep::Advanced(effects) => effects,
        RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
    };
    let (tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };

    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue reconstructed body");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch reconstructed body"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
    ));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
        .expect("enqueue durable-body completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch durable-body completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
    ));
    runtime
        .enqueue_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
        )
        .expect("enqueue validated-body completion");
    let validation_step = runtime
        .step(before_timeout)
        .expect("dispatch validated-body completion");
    runtime
        .take_last_scheduler_ownership()
        .expect("validation macro-step retains exact scheduler ownership");
    let RuntimeStep::Advanced(validation_effects) = validation_step else {
        panic!("validation dispatch unexpectedly idled")
    };
    let prepare_effect_ownership = runtime
        .take_effect_ownership(validation_effects.len())
        .expect("Prepare signature request retains its lifecycle owner");
    let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare
            && vote.round == manifest.round
            && vote.subject == manifest.subject =>
        {
            (*tag, vote.signature_preimage())
        }
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    assert_eq!(prepare_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
        .expect("publish the pending Prepare signer owner");

    // The second periodic episode is still before the absolute deadline,
    // but it is frozen only at this serialized runner entry. The pending
    // Prepare signer already owns an older lifecycle position, so the new
    // episode waits without entering the adapter or creating fence debt.
    let second_retransmission = before_timeout + runtime.retransmit_interval();
    assert!(second_retransmission < start + runtime.round_timeout());
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(second_retransmission)
            .expect("freeze the pre-deadline second retransmission"),
        RuntimeStep::Idle
    ));
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty(),
        "a younger periodic owner cannot enter the adapter ahead of the signer"
    );
    assert!(
        runtime.retransmit_owner.is_some(),
        "the fresh periodic episode remains frozen at its later lifecycle position"
    );

    let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(
            prepare_sign_tag,
            prepare_signature,
            &prepare_effect_ownership[0],
        )
        .expect("enqueue exact Prepare signature completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the pending Prepare signer owner after completion enqueue");
    assert_eq!(runtime.queued_commands(), 1);

    let prepare_broadcast = runtime
        .step(second_retransmission)
        .expect("owned Prepare completion precedes the younger retransmission");
    let prepare_completion = runtime
        .take_last_scheduler_ownership()
        .expect("Prepare completion retains exact scheduler ownership");
    assert_eq!(prepare_completion.selected, RuntimeSelectedOwnerKind::Fifo);
    assert!(!prepare_completion.fence_completion_bypass);
    assert!(
        prepare_completion
            .fence_predecessor_lifecycle_ordinal
            .is_none()
    );
    assert!(prepare_completion.validate_exact().is_ok());
    let RuntimeStep::Advanced(prepare_broadcasts) = prepare_broadcast else {
        panic!("Prepare completion unexpectedly idled")
    };
    assert!(matches!(
        prepare_broadcasts.as_slice(),
        [AdapterEffect::Broadcast(message)]
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::Vote(vote)
                    if vote.phase == wire::GlobalPhase::Prepare
                        && vote.round == manifest.round
                        && vote.subject == manifest.subject
            )
    ));
    runtime
        .take_effect_ownership(prepare_broadcasts.len())
        .expect("test executor consumes Prepare broadcast ownership");
    assert!(
        runtime.retransmit_owner.is_some(),
        "the younger periodic episode remains frozen until its own turn"
    );
    assert_eq!(runtime.queued_commands(), 0);

    // Once the older completion drains, the retained fresh episode runs
    // and rebroadcasts the newly published Prepare vote.
    let retransmit_retry = runtime
        .step_and_take_scheduler_ownership_for_test(second_retransmission)
        .expect("service younger pre-deadline retransmission episode");
    assert!(matches!(
        retransmit_retry,
        RuntimeStep::Advanced(ref effects)
            if effects.iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::Vote(vote)
                            if vote.phase == wire::GlobalPhase::Prepare
                                && vote.round == manifest.round
                    )
            ))
    ));
    assert_eq!(
        prepare_completion.validate_exact(),
        Ok(()),
        "immutable completion evidence remains valid after the younger owner runs"
    );
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.retransmit_owner.is_none());

    // Absolute timeout remains one-shot after the pre-deadline episode
    // drains. Its signing lifecycle likewise predates the next periodic
    // episode.
    let deadline = start + runtime.round_timeout();
    let timeout_macro_step = runtime
        .step(deadline)
        .expect("deliver the absolute timeout through the real adapter");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout macro-step retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_macro_step else {
        panic!("absolute timeout unexpectedly idled")
    };
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout signature request retains its lifecycle owner");
    let (timeout_sign_tag, timeout_signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] if vote.round == manifest.round => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    assert_eq!(timeout_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending TimeoutVote signer owner");

    // A fresh retransmission episode becomes due while TimeoutVote signing
    // is active. Its new ordinal follows the signer, so it remains at the
    // runtime boundary instead of entering the adapter as Busy debt.
    let post_timeout_retransmission = deadline + runtime.retransmit_interval();
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
            .expect("freeze post-timeout retransmission behind signing"),
        RuntimeStep::Idle
    ));
    assert!(
        runtime.retransmit_owner.is_some(),
        "post-timeout retransmission retains its fresh runtime owner while blocked"
    );
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );

    let timeout_signature = Signature::new(keys[0].private_key(), &timeout_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(
            timeout_sign_tag,
            timeout_signature,
            &timeout_effect_ownership[0],
        )
        .expect("enqueue exact TimeoutVote signature completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the pending TimeoutVote signer owner after completion enqueue");
    let first_timeout_vote = runtime
        .step(post_timeout_retransmission)
        .expect("owned TimeoutVote completion precedes the younger retransmission");
    let timeout_completion = runtime
        .take_last_scheduler_ownership()
        .expect("TimeoutVote completion retains exact scheduler ownership");
    assert_eq!(timeout_completion.selected, RuntimeSelectedOwnerKind::Fifo);
    assert!(!timeout_completion.fence_completion_bypass);
    assert!(
        timeout_completion
            .fence_predecessor_lifecycle_ordinal
            .is_none()
    );
    assert!(timeout_completion.validate_exact().is_ok());
    let RuntimeStep::Advanced(first_timeout_vote_effects) = first_timeout_vote else {
        panic!("TimeoutVote completion unexpectedly idled")
    };
    assert!(first_timeout_vote_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                    if vote.round == manifest.round
            )
    )));
    runtime
        .take_effect_ownership(first_timeout_vote_effects.len())
        .expect("test executor consumes first TimeoutVote ownership");
    assert!(
        runtime.retransmit_owner.is_some(),
        "the younger post-timeout episode remains frozen until its own turn"
    );

    // Treat the first TimeoutVote broadcast as lost. The retained younger
    // periodic episode now owns the next serialized turn and rebroadcasts
    // the published vote.
    let timeout_vote_retry = runtime
        .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
        .expect("rebroadcast a lost first TimeoutVote");
    assert!(matches!(
        timeout_vote_retry,
        RuntimeStep::Advanced(ref effects)
            if effects.iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                            if vote.round == manifest.round
                    )
            ))
    ));
    assert_eq!(runtime.queued_commands(), 0);
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.retransmit_owner.is_none());

    // A later periodic tick remains armed after the one-shot timeout and
    // continues broadcasting the published TimeoutVote.
    let later_post_timeout_tick = post_timeout_retransmission + runtime.retransmit_interval();
    let later_retry = runtime
        .step(later_post_timeout_tick)
        .expect("service a later post-timeout periodic tick");
    let later_retry_owner = runtime
        .take_last_scheduler_ownership()
        .expect("later periodic tick retains scheduler ownership");
    assert_eq!(
        later_retry_owner.selected,
        RuntimeSelectedOwnerKind::PeriodicTimer
    );
    assert!(later_retry_owner.validate_exact().is_ok());
    let RuntimeStep::Advanced(later_retry_effects) = later_retry else {
        panic!("later post-timeout periodic tick unexpectedly idled")
    };
    assert!(later_retry_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                    if vote.round == manifest.round
            )
    )));
    runtime
        .take_effect_ownership(later_retry_effects.len())
        .expect("test executor consumes later TimeoutVote retry ownership");
    assert_eq!(runtime.queued_commands(), 0);
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
}

#[test]
fn missing_nonempty_effect_ownership_latches_runtime_fail_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );

    assert_eq!(
        runtime.take_effect_ownership(1),
        Err("Sumeragi v2 effect batch omitted its lifecycle ownership".to_owned()),
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        ),
        Err(EnqueueError::FailClosed),
        "missing runtime ownership permanently closes later ingress",
    );
}

#[test]
fn queued_body_completion_coalesces_only_its_incumbent_owner() {
    let directory = TempDir::new().expect("temporary queued-owner directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xA7);
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue one exact body completion owner");
    let incumbent = RuntimeEffectOwnership::inherited(
        runtime.ingress.commands[0]
            .lifecycle_owner()
            .expect("queued body completion has one exact owner"),
    );
    let next_ordinal = runtime.ingress.next_admission_ordinal;

    let exact = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &incumbent)
        .expect("same-owner queued retry coalesces");
    assert!(!exact.owns_new_slot());
    assert_eq!(exact.lifecycle_owner().as_ref(), Some(incumbent.owner()));
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);

    let foreign = RuntimeEffectOwnership::fresh_for_test(
        tag,
        incumbent
            .owner()
            .lifecycle_ordinal()
            .checked_add(1)
            .expect("test lifecycle ordinal remains finite"),
    );
    assert_eq!(
        runtime.reserve_body_available_with_owner(tag, manifest, &foreign),
        Err(EnqueueError::FailClosed),
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(runtime.fail_closed);
}

#[test]
fn replayed_proposal_fanout_consumes_the_live_producer_reservation() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xAA);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct replay runtime");
    let replay_owner = runtime
        .mint_fresh_lifecycle_owner(
            initial,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"replayed-proposal-signature",
        )
        .expect("mint exact startup recovery owner");
    let replay_ownership =
        RuntimeEffectOwnership::fresh(replay_owner, RuntimeFreshRootKind::StartupRecovery);
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve live producer after replay work was restored");
    runtime
        .arm_live_clocks(start)
        .expect("arm clocks after replay restoration");

    runtime
        .complete_active_view_producer_after_proposal_fanout(proposal.round, &replay_ownership)
        .expect("replayed original Proposal fanout consumes the live reservation");
    assert!(runtime.active_view_producer.is_none());
    assert!(!runtime.fail_closed);
}

#[test]
fn retransmitted_proposal_fanout_preserves_the_live_producer_reservation() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xAB);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct retransmit runtime");
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve live producer");
    let retransmit_owner = runtime
        .mint_fresh_lifecycle_owner(
            initial,
            CommandClass::Progress,
            RuntimeFreshRootKind::Retransmit,
            b"periodic-retransmit",
        )
        .expect("mint exact retransmit owner");
    let retransmit_ownership =
        RuntimeEffectOwnership::fresh(retransmit_owner, RuntimeFreshRootKind::Retransmit);
    runtime
        .arm_live_clocks(start)
        .expect("arm clocks after producer reservation");

    runtime
        .complete_active_view_producer_after_proposal_fanout(proposal.round, &retransmit_ownership)
        .expect("periodic Proposal fanout is not the live producer terminal");
    assert!(runtime.active_view_producer.is_some());
    assert!(!runtime.fail_closed);
}

#[test]
fn scheduler_minimum_uses_cached_admission_but_dispatch_revalidates_ingress() {
    let directory = TempDir::new().expect("temporary cached-admission directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0xA6),
        ));
    let source = PeerId::new(keys[0].public_key().clone());

    runtime
        .enqueue_network_with_ingress_ownership(
            message.clone(),
            fair_network_ownership(&message, source),
        )
        .expect("deeply validated authenticated command enters the runtime FIFO");
    let lifecycle_ordinal = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("published command owns a lifecycle ordinal");

    // Model corruption after publication to prove the two validation
    // boundaries are distinct. A rank scan consumes only the immutable
    // cached admission certificate; dispatch still validates the full
    // ingress carrier and therefore must fail closed before removal.
    let queued = runtime
        .ingress
        .commands
        .front_mut()
        .expect("published command remains queued");
    queued
        .ingress_ownership
        .as_mut()
        .expect("authenticated command retains ingress ownership")
        .projection_hash = Hash::new(b"invalid retained ingress projection");
    assert!(!queued.validate_admission_identity());
    assert_eq!(
        runtime.ingress.oldest_lifecycle_ordinal(),
        Ok(Some(lifecycle_ordinal)),
        "scheduler rank scans must not repeat deep envelope validation"
    );
    assert!(matches!(
        runtime.ingress.pop_next_with_ownership(),
        Err(EnqueueError::FailClosed)
    ));
    assert_eq!(
        runtime.ingress.commands.len(),
        1,
        "dispatch rejects corrupted ingress before consuming the cached owner"
    );
}
