#[test]
fn serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let geometry = ServicedCandidateCapacityGeometry::new(7, 3);
    let (mut adapter, startup) = open_test_with_capacity_geometry(&directory, geometry)
        .expect("open adapter with non-default production geometry");
    assert!(startup.is_empty());
    let capacity =
        serviced_candidate_capacity_with_geometry(adapter.wire_context.roster.len(), geometry);
    assert_eq!(adapter.serviced_candidate_capacity, capacity);
    assert_ne!(
        capacity,
        serviced_candidate_capacity(adapter.wire_context.roster.len()),
        "the configured runtime/effect geometry must replace the fixture default"
    );
    adapter.serviced_candidates.clear();
    for index in 0..capacity {
        let mut evidence = [0_u8; 32];
        evidence[..8].copy_from_slice(
            &u64::try_from(index)
                .expect("bounded capacity index fits u64")
                .to_le_bytes(),
        );
        let source_view = u64::try_from(index).expect("bounded source view fits u64");
        assert_eq!(
            adapter.serviced_candidates.insert(
                ServicedCandidateKey::new(
                    adapter.wire_context.id(),
                    adapter.wire_context.height,
                    adapter.fingerprints.node.into(),
                    adapter.wire_context.leader(source_view),
                    source_view,
                    None,
                    0,
                    DeferredPriority::Normal.code(),
                    u8::MAX,
                    evidence,
                ),
                adapter.current_tag().view(),
            ),
            None
        );
    }
    let retained = adapter.serviced_candidates.clone();
    let reducer_before = adapter.reducer.clone();
    let overflow = unowned_body_event(&adapter, 0x42);
    assert!(matches!(
        adapter.step(overflow),
        Err(AdapterError::ServicedCandidateStore(reason))
            if reason.contains("capacity")
    ));
    assert!(adapter.fail_closed);
    assert_eq!(
        adapter.serviced_candidates, retained,
        "capacity exhaustion cannot evict a prior tombstone"
    );
    assert_eq!(
        adapter.reducer, reducer_before,
        "capacity must be reserved before the consuming reducer transition"
    );
}

#[test]
fn persistence_macro_step_budgets_have_exact_five_effect_maximum() {
    let expected = [
        (
            PersistenceMacroStepClass::ProposalIntent,
            PersistenceMacroStepBudget::new(1, 1),
        ),
        (
            PersistenceMacroStepClass::PrepareIntent,
            PersistenceMacroStepBudget::new(2, 1),
        ),
        (
            PersistenceMacroStepClass::ObservePrepare,
            PersistenceMacroStepBudget::new(4, 1),
        ),
        (
            PersistenceMacroStepClass::LockAndCommit,
            PersistenceMacroStepBudget::new(3, 1),
        ),
        (
            PersistenceMacroStepClass::TimeoutIntent,
            PersistenceMacroStepBudget::new(1, 1),
        ),
        (
            PersistenceMacroStepClass::InstallTimeout,
            PersistenceMacroStepBudget::new(2, 4),
        ),
        (
            PersistenceMacroStepClass::Decision,
            PersistenceMacroStepBudget::new(2, 2),
        ),
    ];
    assert_eq!(
        PersistenceMacroStepClass::ALL,
        expected.map(|(class, _)| class),
        "the exhaustive WAL class inventory must remain source ordered"
    );
    for (class, budget) in expected {
        assert_eq!(class.budget(), budget);
        assert!(budget.initial_effects >= 1);
        assert!(budget.continuation_effects <= reducer::MAX_EFFECTS_PER_STEP);
        assert!(budget.flattened_effects() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    }
    assert_eq!(
        PersistenceMacroStepClass::ALL
            .into_iter()
            .map(|class| class.budget().flattened_effects())
            .max(),
        Some(MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP)
    );
    assert_eq!(
        PersistenceMacroStepClass::InstallTimeout
            .budget()
            .flattened_effects(),
        MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP,
        "local TC formation is the unique five-effect persistence witness"
    );
}

#[test]
fn drive_effects_rejects_oversized_non_persisting_batch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let effect = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let oversized = vec![effect; MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1];

    assert!(matches!(
        adapter.drive_effects(oversized),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects,
            maximum_initial_effects,
            persist_effects: 0,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        }) if initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1
            && maximum_initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn drive_effects_rejects_record_specific_overbudget_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [reducer::Effect::Persist { .. }]
    ));
    let unrelated = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut overbudget = vec![unrelated];
    overbudget.extend(timeout);

    assert!(matches!(
        adapter.drive_effects(overbudget),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 2,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 0,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn drive_effects_rejects_multiple_persist_owners_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    let persist = timeout.pop().expect("one Persist effect");
    assert!(matches!(&persist, reducer::Effect::Persist { .. }));

    assert!(matches!(
        adapter.drive_effects(vec![persist.clone(), persist]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            persist_effects: 2,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn post_wal_oversized_continuation_fails_closed_and_replays_exact_record() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let protected_subject = subject(0x6d);
    let prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: protected_subject,
        execution_commitment: execution_commitment(0x6d),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x6d; 96],
    };
    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x6e; 96],
        }],
    };
    let wire_context = adapter.wire_context.clone();
    let timeout = adapter
        .registry
        .tc_to_core(&timeout, &wire_context)
        .expect("convert the lock-promoting timeout certificate");
    let timeout_tag = adapter.current_tag();
    let pending_timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutCertificateReceived {
            tag: timeout_tag,
            certificate: timeout,
        })
        .expect("stage the real InstallTimeout persistence");
    let mut pending_effects = pending_timeout.into_effects();
    let reducer::Effect::Persist { tag, entry } = pending_effects
        .pop()
        .expect("InstallTimeout has one Persist effect")
    else {
        panic!("InstallTimeout must stage persistence");
    };
    assert!(pending_effects.is_empty());

    // Keep the reducer's real lock-promoting continuation, but classify
    // and encode this adversarial boundary call as the smaller
    // TimeoutIntent class. The substitute is itself a valid first WAL
    // record with the exact pending persistence ID, so the continuation
    // guard is reached only after the append succeeds.
    let timeout_round = reducer::Round::new(wire_round.height, wire_round.view);
    let local_validator = adapter
        .reducer
        .local_validator()
        .expect("test adapter is a validator");
    let forged_entry = reducer::WalEntry::new(
        entry.id(),
        reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
            adapter.reducer.context().id(),
            timeout_round,
            local_validator,
            None,
        )),
    );
    assert!(matches!(
        adapter.drive_effects(vec![reducer::Effect::Persist {
            tag,
            entry: forged_entry,
        }]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 1,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 2,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.wal.recovered_records()[0].sequence, 0);
    drop(adapter);

    let (recovered, first_startup) =
        open_test(&directory).expect("replay the one valid timeout intent");
    assert!(recovered.ingress_ready());
    assert!(!recovered.fail_closed);
    assert_eq!(recovered.wal.recovered_records().len(), 1);
    assert_eq!(recovered.reducer.durable_state().last_id().get(), 1);
    assert!(
        recovered
            .reducer
            .durable_state()
            .timeout_intent(timeout_round)
            .is_some()
    );
    assert!(first_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert!(matches!(
        first_startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round == wire_round
            && vote.highest_prepare_qc.is_none()
            && vote.signer == 0
            && vote.signature.is_empty()
    ));
    drop(recovered);

    let (recovered_again, second_startup) =
        open_test(&directory).expect("repeat deterministic timeout-intent replay");
    assert_eq!(second_startup, first_startup);
    assert!(second_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert_eq!(recovered_again.wal.recovered_records().len(), 1);
    assert!(recovered_again.ingress_ready());
    assert!(!recovered_again.fail_closed);
}

#[test]
fn open_records_exactly_one_recovery_progress_transition() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    assert!(matches!(
        adapter.last_progress,
        Some((
            generation,
            round,
            wire::SumeragiV2ProgressTransition::RecoveryReplayed
        )) if generation == adapter.current_tag().generation()
            && round == reducer::Round::new(adapter.wire_context.height, 0)
    ));
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::Duplicate)
            .copied()
            .unwrap_or_default(),
        0,
        "opening must step ResumeAfterReplay once, not record a duplicate replay"
    );
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "the replay control trigger cannot consume candidate-tombstone capacity"
    );
    for attempt in 0..3 {
        adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("retransmit control attempt {attempt}: {error}"));
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "periodic retransmission triggers remain executable without becoming tombstones"
    );
    let status = adapter.status().expect("status after replay");
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}

#[cfg(feature = "bls")]
#[test]
fn first_recovery_snapshot_tracks_the_durable_locked_body() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xCE);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let (_, keys, _) = authenticated_context();
    let mut wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xCE),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut wire_prepare, &keys);
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register durable PrepareQC");
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
                adapter.reducer.context().id(),
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
        .expect("encode durable lock");
    assert_eq!(
        adapter.wal.append(&encoded).expect("append durable lock"),
        0
    );
    drop(adapter);

    let (mut recovered, startup) = open_test(&directory).expect("recover durable lock");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        }] if vote.phase == wire::GlobalPhase::Commit
            && vote.subject == locked_subject
    ));
    assert_eq!(recovered.active_subject, Some((round, core_subject)));
    let status = recovered.status().expect("first locked recovery snapshot");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Complete
    );
    assert_eq!(
        status.liveness.work.body_recovery,
        wire::SumeragiV2LocalWorkStage::Queued
    );
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}

#[test]
fn persistence_is_fsynced_before_sign_is_exposed() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    assert!(adapter.ingress_ready());
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(7);
    let proposal = proposal(&adapter.wire_context, proposer, subject);
    let fetch = adapter
        .receive_verified(proposal)
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
    let store = adapter
        .body_available(tag, manifest)
        .expect("body available")
        .into_effects();
    assert!(matches!(
        store.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    let receipt = durable_body_receipt(&adapter, round, subject);
    let validate = adapter
        .body_stored(tag, round, subject, &receipt)
        .expect("body stored")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validated = ValidatedBodyReceipt::for_test(receipt.clone());
    let sign = adapter
        .validation_succeeded(tag, round, subject, &validated)
        .expect("valid body")
        .into_effects();
    assert!(matches!(sign.as_slice(), [AdapterEffect::Sign { .. }]));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
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
    let manifest = wire::PayloadManifest::derive(
        &adapter.wire_context,
        round,
        subject,
        5,
        &[b"chunk".to_vec()],
    )
    .expect("valid certified-body manifest");
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
                protected_body: Some((protected_round, protected_subject)),
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
            && *protected_round == round
            && *protected_subject == subject
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
    let conflicting = wire::PayloadManifest::derive(
        &adapter.wire_context,
        manifest.round,
        manifest.subject,
        5,
        &[b"other".to_vec()],
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
    conflicting_proposal.manifest = wire::PayloadManifest::derive(
        &context,
        conflicting_proposal.round,
        conflicting_proposal.subject,
        5,
        &[b"other".to_vec()],
    )
    .expect("structurally valid alternate manifest");
    let conflicting = AuthenticatedConsensusMessage::for_test(conflicting);
    assert!(matches!(
        adapter.ensure_authenticated_manifest_compatible(&conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
    assert!(!adapter.fail_closed);
}

#[test]
fn proposal_registry_preserves_the_first_exact_semantic_envelope() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let wire::ConsensusMessageV2Payload::Proposal(first) =
        proposal(&context, context.leader(0), subject(0x40)).payload
    else {
        unreachable!("proposal fixture")
    };
    let mut later = first.clone();
    later.signature = vec![0x40; 96];

    registry
        .proposal_to_core(&first, &context)
        .expect("register first exact proposal envelope");
    registry
        .proposal_to_core(&later, &context)
        .expect("the same semantic proposal remains convertible");

    let key = (
        reducer::Round::new(first.round.height, first.round.view),
        reducer::Subject::new(Hash::new(first.subject.encode()).into()),
    );
    assert_eq!(
        registry.proposals.get(&key),
        Some(&first),
        "a later exact-envelope alias cannot retarget durable re-signing"
    );
}

#[test]
fn canonical_body_rolls_back_exact_busy_deferred_conflicting_proposal() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let context = adapter.wire_context.clone();
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3F);
    let canonical = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload else {
        panic!("fixture is a proposal")
    };
    let canonical_manifest = canonical_proposal.manifest.clone();
    let round = canonical_manifest.round;

    let mut conflicting = proposal(&context, proposer, subject);
    let conflicting_proposal = {
        let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) =
            &mut conflicting.payload
        else {
            panic!("fixture is a proposal")
        };
        conflicting_proposal.manifest = wire::PayloadManifest::derive(
            &context,
            conflicting_proposal.round,
            conflicting_proposal.subject,
            5,
            &[b"other".to_vec()],
        )
        .expect("structurally valid alternate manifest");
        conflicting_proposal.clone()
    };
    let conflicting_wire_identity = Arc::<[u8]>::from(conflicting.encode());
    let deferred = adapter
        .registry
        .proposal_to_core(&conflicting_proposal, &context)
        .expect("convert authenticated proposal before reducer reports Busy");
    let deferred_tag = adapter.current_tag();
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::ProposalReceived {
            tag: deferred_tag,
            proposal: deferred,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(conflicting_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    let admission_key = IngressSemanticKey::Proposal { round, proposer };
    adapter.ingress_equivocations.insert(
        admission_key,
        IngressEquivocationRecord {
            fingerprint: IngressFingerprint::Proposal(Hash::new(
                conflicting_proposal.signature_preimage(),
            )),
            equivocation_reported: true,
            capacity_bypass: false,
            admitted_at: Instant::now(),
        },
    );
    adapter.ingress_deliveries.insert(
        admission_key,
        IngressDeliveryRecord {
            fingerprint: IngressFingerprint::Proposal(Hash::new(
                conflicting_proposal.signature_preimage(),
            )),
            generation: deferred_tag.generation(),
            locked_commit_progress: false,
        },
    );

    let retained_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(0x3F),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x3F; 96],
    };
    adapter
        .registry
        .qc_to_core(&retained_qc, &context)
        .expect("register independently authenticated QC material");
    let retained_certificates = adapter.registry.certificates.clone();
    let retained_execution_commitments = adapter.registry.execution_commitments.clone();
    assert!(adapter.registry.manifest_conflicts(&canonical_manifest));

    let outcome = adapter
        .body_available(deferred_tag, canonical_manifest.clone())
        .expect("canonical body supersedes only its Busy-deferred proposal authority");
    assert_eq!(
        outcome.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
    assert!(adapter.deferred_inputs.is_empty());
    assert!(!adapter.ingress_equivocations.contains_key(&admission_key));
    assert!(!adapter.ingress_deliveries.contains_key(&admission_key));
    assert!(adapter.registry.proposals.is_empty());
    assert_eq!(
        adapter.registry.manifests.values().next(),
        Some(&canonical_manifest)
    );
    assert_eq!(adapter.registry.certificates, retained_certificates);
    assert_eq!(
        adapter.registry.execution_commitments,
        retained_execution_commitments
    );
    assert!(!adapter.fail_closed);
}

#[test]
fn forged_body_receipt_cannot_cross_the_prepare_durability_boundary() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(31);
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
    let correct = durable_body_receipt(&adapter, round, proposed_subject);
    let forged = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        round,
        subject(32),
        correct.manifest_hash(),
    );
    assert!(matches!(
        adapter.body_stored(tag, round, proposed_subject, &forged),
        Err(AdapterError::DurableBodyMismatch)
    ));
    assert!(matches!(
        adapter
            .body_stored(tag, round, proposed_subject, &correct)
            .expect("the real durable receipt remains usable")
            .effects(),
        [AdapterEffect::ValidateBody { .. }]
    ));
}

#[test]
fn local_proposal_and_prepare_are_each_persisted_before_signing() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let subject = subject(8);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let (durable, validated) =
        validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
    let proposal_tag = adapter.current_tag();
    let sign = adapter
        .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
        .expect("submit local proposal")
        .into_effects();
    let tag = match sign.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ] => {
            assert!(proposal.signature.is_empty());
            *tag
        }
        effects => panic!("unexpected local proposal effects: {effects:?}"),
    };
    assert_eq!(adapter.wal.recovered_records().len(), 1);

    let effects = adapter
        .signature_completed(tag, vec![0xD1; 96])
        .expect("sign local proposal")
        .into_effects();
    assert!(matches!(
        effects.as_slice(),
        [
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            }),
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            }
        ]
    ));
    assert_eq!(adapter.wal.recovered_records().len(), 2);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
}

#[test]
fn local_proposal_commitment_conflict_is_transactional() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let proposed_subject = subject(0x7b);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let conflicting = execution_commitment(0x7c);
    assert_ne!(conflicting, validated.execution_commitment());
    adapter
        .registry
        .register_execution_commitment(round, core_subject, conflicting)
        .expect("pre-bind a conflicting authenticated commitment");

    let subjects_before = adapter.registry.subjects.clone();
    let manifests_before = adapter.registry.manifests.clone();
    let commitments_before = adapter.registry.execution_commitments.clone();
    let active_before = adapter.active_subject;
    let reducer_before = adapter.reducer.clone();
    let wal_len_before = adapter.wal.recovered_records().len();

    assert!(matches!(
        adapter.local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated,),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert_eq!(adapter.registry.subjects, subjects_before);
    assert_eq!(adapter.registry.manifests, manifests_before);
    assert_eq!(adapter.registry.execution_commitments, commitments_before);
    assert_eq!(adapter.active_subject, active_before);
    assert_eq!(adapter.reducer, reducer_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_len_before);
}

#[test]
fn post_decision_selected_lifecycles_cannot_reopen_the_reclaimed_owner_epoch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());

    let decided_subject = subject(0x7c);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7c; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    let reclaimed_snapshot = std::fs::read(adapter.serviced_candidate_store_path_for_test())
        .expect("read reclaimed owner snapshot");

    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision validated body"), 1)
        .expect("bind post-Decision validation lifecycle");
    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("service selected post-Decision validation without a producer owner");
    adapter.clear_selected_producer_lifecycle();
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(applied.producer_handoff().is_none());

    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision application"), 2)
        .expect("bind post-Decision application lifecycle");
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("service selected post-Decision application completion");
    adapter.clear_selected_producer_lifecycle();
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    assert!(completed.producer_handoff().is_none());

    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    assert!(adapter.restored_dormant_producer_continuations.is_empty());
    assert!(adapter.deferred_producer_continuations.is_empty());
    assert!(adapter.pending_producer_handoffs.is_empty());
    assert_eq!(
        std::fs::read(adapter.serviced_candidate_store_path_for_test())
            .expect("reread reclaimed owner snapshot"),
        reclaimed_snapshot,
        "post-Decision service cannot republish or mutate the reclaimed owner epoch"
    );
}

#[test]
fn exact_local_completion_after_decision_reports_body_validated_progress() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let predecision_a = unowned_body_event(&adapter, 0x79);
    adapter
        .step(predecision_a)
        .expect("service pre-Decision candidate A");
    let predecision_b = unowned_body_event(&adapter, 0x7A);
    adapter
        .step(predecision_b)
        .expect("service pre-Decision candidate B");
    assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
    assert_eq!(adapter.durable_serviced_candidates.len(), 2);
    let decided_subject = subject(0x7d);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7d; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "durable Decision reclaims the complete candidate-service epoch, including its triggering occurrence"
    );

    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("transfer trusted local validation to the Decision");
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(matches!(
        adapter.status().expect("liveness snapshot").liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            round,
            transition: wire::SumeragiV2ProgressTransition::BodyValidated,
            ..
        }) if round == decision.round
    ));
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "post-Decision application progress cannot resurrect candidate tombstones"
    );
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("retire the exact Decision application lifecycle");
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    let expected_retransmit = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
    ));
    for attempt in 0..3 {
        let retransmit = adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("post-drain retransmission {attempt}: {error}"));
        assert_eq!(
            retransmit.effects(),
            std::slice::from_ref(&expected_retransmit),
            "a drained exact Decision may retransmit only its exact durable CommitQC control"
        );
    }
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "monotone applied state, not a recycled dormant ordinal or tombstone, suppresses resurrection"
    );
}

#[test]
fn busy_local_completion_during_decision_wal_reaches_apply_once() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7e);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7e; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));

    let busy = adapter
        .local_proposal_ready(decision_tag, manifest, &durable, &validated)
        .expect("Busy boundary retains the trusted local completion");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(adapter.deferred_completions.len(), 1);

    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody {
            subject,
            certificate: Some(certificate),
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("fairly service the Busy-deferred completion");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply {
            subject,
            certificate,
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert!(
        adapter
            .drain_deferred()
            .expect("completion cannot be applied twice")
            .is_empty()
    );
}

#[test]
fn busy_deferred_input_blocks_terminal_readiness_until_serviced() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7f);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7f; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));

    let busy_completion = adapter
        .local_proposal_ready(decision_tag, manifest.clone(), &durable, &validated)
        .expect("retain the trusted completion across the Busy fence");
    assert_eq!(
        busy_completion.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let terminal_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signer: 3,
        signature: vec![0x80; 96],
    };
    let busy_vote = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(terminal_vote)),
        ))
        .expect("retain authenticated ingress across the Busy fence");
    assert_eq!(
        busy_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert_eq!(adapter.deferred_inputs.len(), 1);

    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody { subject, .. }] if *subject == decided_subject
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("service the retained completion first");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply { subject, .. }] if *subject == decided_subject
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(adapter.deferred_inputs.len(), 1);

    let applied = adapter
        .application_completed(decision_tag, decided_subject)
        .expect("acknowledge exact decision application");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(applied.effects().is_empty());
    assert!(adapter.reducer.ready_to_finish());
    assert!(adapter.deferred_work_is_serviceable());
    assert!(
        !adapter.ready_to_finish(),
        "adapter-owned Busy debt must block terminal height rollover"
    );

    assert!(
        adapter
            .drain_deferred()
            .expect("retire the authenticated terminal vote")
            .is_empty()
    );
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ready_to_finish());
}
