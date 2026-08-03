#[test]
fn enter_view_conversion_uses_effect_carried_lock_not_reducer_lock() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let carried_wire = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xE1),
        execution_commitment: execution_commitment(0xE1),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA1; 96],
    };
    let durable_wire = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xE2),
        execution_commitment: execution_commitment(0xE2),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA2; 96],
    };
    let wire_context = adapter.wire_context.clone();
    let carried_lock = adapter
        .registry
        .qc_to_core(&carried_wire, &wire_context)
        .expect("register the effect-carried PrepareQC");
    let durable_lock = adapter
        .registry
        .qc_to_core(&durable_wire, &wire_context)
        .expect("register the different durable PrepareQC");
    let carried_reference = carried_lock.reference();
    let durable_reference = durable_lock.reference();
    assert_ne!(carried_reference, durable_reference);

    let core_context = adapter.reducer.context().clone();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let durable_round = durable_lock.round();
    let durable_subject = durable_lock.subject();
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare: durable_lock,
            vote: reducer::Vote::new(
                core_context.id(),
                durable_round,
                reducer::Phase::Commit,
                durable_subject,
                local_validator,
            ),
        },
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(1),
        [lock_entry],
    )
    .expect("recover the different durable lock");
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .locked()
            .map(reducer::QuorumCertificate::reference),
        Some(durable_reference)
    );

    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC1; 96],
        }],
    };
    let timeout = adapter
        .registry
        .tc_to_core(&timeout, &wire_context)
        .expect("register the timeout certificate");

    adapter.registry.certificates.remove(&carried_reference);
    assert!(
        !adapter
            .registry
            .certificates
            .contains_key(&carried_reference)
    );
    let tag = adapter.current_tag();
    let converted = adapter
        .convert_effect(reducer::Effect::EnterView {
            tag,
            certificate: timeout,
            protected_lock: Some(carried_lock),
        })
        .expect("convert the effect-carried lock");

    let AdapterEffect::EnterView {
        tag: converted_tag,
        certificate,
        protected_body,
    } = converted
    else {
        panic!("expected EnterView adapter effect");
    };
    assert_eq!(converted_tag, tag);
    assert_eq!(certificate.round, wire_round);
    assert_eq!(
        protected_body,
        Some((carried_wire.round, carried_wire.subject))
    );
    assert_eq!(
        adapter.active_subject,
        Some((carried_reference.round(), carried_reference.subject()))
    );

    let converted_lock = adapter
        .registry
        .certificates
        .get(&carried_reference)
        .expect("conversion must materialize the carried PrepareQC");
    assert_eq!(converted_lock.round, carried_wire.round);
    assert_eq!(converted_lock.subject, carried_wire.subject);
    assert_eq!(
        converted_lock.execution_commitment,
        carried_wire.execution_commitment
    );
    assert_ne!(
        converted_lock.execution_commitment,
        durable_wire.execution_commitment
    );
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .locked()
            .map(reducer::QuorumCertificate::reference),
        Some(durable_reference),
        "conversion must neither re-read nor replace the reducer's different durable lock"
    );
}

#[test]
fn persisted_enter_view_clears_unlocked_subject_and_records_tc_progress() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let first_serviced = unowned_body_event(&adapter, 0xD3);
    adapter
        .step(first_serviced)
        .expect("service a view-zero candidate");
    let replacement = unowned_body_event(&adapter, 0xD4);
    adapter
        .step(replacement)
        .expect("service its equal-rank replacement");
    assert_eq!(
        adapter.serviced_candidate_views_for_test(),
        BTreeSet::from([0]),
    );

    let stale_subject = adapter
        .registry
        .register_subject(subject(0xD5))
        .expect("register stale active subject");
    let stale_round = reducer::Round::new(adapter.wire_context.height, 0);
    adapter.active_subject = Some((stale_round, stale_subject));
    adapter.last_progress = None;
    let timed_out_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let timeout = wire::TimeoutCertificate {
        round: timed_out_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC5; 96],
        }],
    };

    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install an unlocked timeout certificate");

    assert_eq!(installed.disposition(), reducer::StepDisposition::Applied);
    assert_eq!(adapter.reducer.durable_state().current_view(), 1);
    assert!(
        !adapter.serviced_candidate_views_for_test().contains(&0),
        "strict certified view advance reclaims every view-zero serviced identity"
    );
    assert!(adapter.reducer.durable_state().locked().is_none());
    assert!(
        adapter.active_subject.is_none(),
        "EnterView must not retain a stale proposal or certificate subject"
    );
    assert!(matches!(
        adapter.last_progress,
        Some((
            generation,
            _,
            wire::SumeragiV2ProgressTransition::TimeoutCertificateInstalled
        )) if generation == adapter.current_tag().generation()
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn strict_same_round_tc_preserves_and_retags_timeout_vote_owners() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let timed_out_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let initial_timeout = wire::TimeoutCertificate {
        round: timed_out_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC6; 96],
        }],
    };
    let initial_install = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                initial_timeout,
            )),
        ))
        .expect("install the first timeout certificate");
    assert_eq!(
        initial_install.disposition(),
        reducer::StepDisposition::Applied
    );
    assert_eq!(adapter.current_tag().view(), 1);

    let current_round = wire::ConsensusRound {
        view: 1,
        ..timed_out_round
    };
    let core_current_round = reducer::Round::new(current_round.height, current_round.view);
    let preserved_signer = adapter
        .registry
        .validator_id(1)
        .expect("fixture signer belongs to the frozen roster");
    let second_preserved_signer = adapter
        .registry
        .validator_id(3)
        .expect("fixture signer belongs to the frozen roster");
    let deferred_signer = adapter
        .registry
        .validator_id(2)
        .expect("fixture signer belongs to the frozen roster");
    let timeout_vote = |signer, marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(
            wire::TimeoutVote {
                round: current_round,
                highest_prepare_qc: None,
                signer,
                signature: vec![marker],
            },
        ))
    };

    let preserved_vote = timeout_vote(1, 0xD1);
    let first_consumed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            preserved_vote.clone(),
        ))
        .expect("consume the current-view TimeoutVote before the lock-only bump");
    assert_eq!(
        first_consumed.disposition(),
        reducer::StepDisposition::Applied
    );
    let second_preserved_vote = timeout_vote(3, 0xD5);
    let second_consumed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            second_preserved_vote,
        ))
        .expect("consume a second current-view TimeoutVote before the lock-only bump");
    assert_eq!(
        second_consumed.disposition(),
        reducer::StepDisposition::Applied
    );
    assert!(adapter.deferred_progress_inputs.is_empty());
    let preserved_pool = adapter
        .reducer
        .timeout_pool_snapshots()
        .into_iter()
        .find(|snapshot| snapshot.round == core_current_round)
        .expect("the consumed TimeoutVotes own a reducer pool entry");
    assert_eq!(
        preserved_pool.signers,
        vec![preserved_signer, second_preserved_signer]
    );
    assert!(!preserved_pool.certificate_formed);

    let promoted_prepare = wire::QuorumCertificate {
        round: timed_out_round,
        proposal_round: timed_out_round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xD2),
        execution_commitment: execution_commitment(0xD2),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD2; 96],
    };
    let alternate_timeout = wire::TimeoutCertificate {
        round: timed_out_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(promoted_prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD3; 96],
        }],
    };
    let wire_context = adapter.wire_context.clone();
    let alternate_timeout = adapter
        .registry
        .tc_to_core(&alternate_timeout, &wire_context)
        .expect("register the strict same-round lock upgrade");
    let pre_upgrade_tag = adapter.current_tag();
    let pending_install = adapter
        .reducer
        .step(reducer::Event::TimeoutCertificateReceived {
            tag: pre_upgrade_tag,
            certificate: alternate_timeout,
        })
        .expect("stage the real InstallTimeout persistence fence");
    assert_eq!(
        pending_install.disposition(),
        reducer::StepDisposition::Applied
    );
    let pending_install_effects = pending_install.into_effects();
    assert!(matches!(
        pending_install_effects.as_slice(),
        [reducer::Effect::Persist { entry, .. }]
            if matches!(entry.record(), reducer::WalRecord::InstallTimeout(_))
    ));

    let deferred_vote = timeout_vote(2, 0xD4);
    let busy = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            deferred_vote.clone(),
        ))
        .expect("retain authenticated TimeoutVote ownership behind InstallTimeout");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(busy.deferred_admission_ordinal().is_some());
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    assert!(matches!(
        adapter.deferred_progress_inputs.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { tag, vote },
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Progress,
            ..
        }) if *tag == pre_upgrade_tag
            && vote.vote().round() == core_current_round
            && vote.vote().signer() == deferred_signer
    ));
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                deferred_vote.clone(),
            ))
            .expect("coalesce the exact Busy-deferred occurrence")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);

    let install_effects = adapter
        .drive_effects(pending_install_effects)
        .expect("append and acknowledge the real strict same-round InstallTimeout");
    let post_upgrade_tag = adapter.current_tag();
    assert_eq!(post_upgrade_tag.view(), pre_upgrade_tag.view());
    assert!(post_upgrade_tag.generation() > pre_upgrade_tag.generation());
    assert!(install_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView { tag, certificate, .. }
            if *tag == post_upgrade_tag && certificate.round == timed_out_round
    )));

    let pool_after_upgrade = adapter
        .reducer
        .timeout_pool_snapshots()
        .into_iter()
        .find(|snapshot| snapshot.round == core_current_round)
        .expect("lock-only generation bump preserves the current timeout pool");
    assert_eq!(
        pool_after_upgrade.signers,
        vec![preserved_signer, second_preserved_signer]
    );
    assert!(!pool_after_upgrade.certificate_formed);
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                preserved_vote.clone(),
            ))
            .expect("suppress the already-consumed semantic duplicate after the bump")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(
        adapter
            .reducer
            .timeout_pool_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.round == core_current_round)
            .expect("duplicate suppression cannot erase the preserved pool")
            .signers,
        vec![preserved_signer, second_preserved_signer]
    );

    let wal_records_before_service = adapter.wal.recovered_records().len();
    let (service_effects, evidence) = adapter
        .drain_deferred_with_evidence()
        .expect("service the Busy-deferred TimeoutVote")
        .expect("one deferred TimeoutVote owner is serviceable");
    let successor_tag = adapter.current_tag();
    assert_eq!(successor_tag.view(), current_round.view + 1);
    assert!(successor_tag.strictly_advances(post_upgrade_tag));
    assert_eq!(successor_tag.generation(), reducer::Generation::INITIAL);
    assert_eq!(
        adapter.wal.recovered_records().len(),
        wal_records_before_service + 1,
        "the quorum-completing deferred vote must synchronously persist its InstallTimeout"
    );
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .last_timeout()
            .expect("the deferred third signer installs the next timeout certificate")
            .round(),
        core_current_round
    );
    let formed_timeout = service_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
                ..
            }) => Some(certificate),
            _ => None,
        })
        .expect("the locally formed timeout certificate is broadcast");
    assert_eq!(formed_timeout.round, current_round);
    assert_eq!(formed_timeout.groups.len(), 1);
    assert_eq!(formed_timeout.groups[0].signers, vec![1, 2, 3]);
    assert!(service_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView {
            tag,
            certificate,
            ..
        } if *tag == successor_tag && certificate == formed_timeout
    )));
    assert_eq!(evidence.event_kind, DeferredEventKind::TimeoutVoteReceived);
    assert_eq!(
        evidence.retag,
        DeferredRetagRelation::AuthenticatedIngress {
            from: pre_upgrade_tag,
            to: post_upgrade_tag,
        }
    );
    assert_eq!(evidence.total_len_before, 1);
    assert_eq!(evidence.total_len_after, 0);
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(
        adapter
            .drain_deferred_with_evidence()
            .expect("a consumed capability cannot receive a second service turn")
            .is_none()
    );

    assert!(
        adapter.reducer.timeout_pool_snapshots().is_empty(),
        "the view-advancing InstallTimeout retires the completed old-view pool"
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(deferred_vote))
            .expect("reject the exact old-view occurrence after its one service turn")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
    );
    assert!(
        adapter.reducer.timeout_pool_snapshots().is_empty(),
        "an old-view replay cannot resurrect the retired quorum"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn deferred_locked_commit_delivery_tracks_generation_after_tc() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xD8);
    let locked_execution_commitment = execution_commitment(0xD8);
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
        aggregate_signature: vec![0xA8; 96],
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
            .expect("append the durable lock"),
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

    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC8; 96],
        }],
    };
    let deferred_tc = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("defer the timeout certificate behind the signature fence");
    assert_eq!(
        deferred_tc.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );

    let locked_vote =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: 1,
            signature: vec![0xB8],
        }));
    let deferred_vote = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone()))
        .expect("defer the exact locked Commit vote behind the timeout certificate");
    assert_eq!(
        deferred_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 2);
    assert!(adapter.deferred_progress_inputs[1].admission.is_some());

    let duplicate_while_deferred = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone()))
        .expect("suppress an exact duplicate while the first delivery is deferred");
    assert_eq!(
        duplicate_while_deferred.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        2,
        "a same-generation duplicate cannot replace deferred ownership"
    );

    let tag_before_tc = adapter.current_tag();
    let completed_signature = adapter
        .signature_completed(replay_tag, vec![0xB6])
        .expect("complete the signature before draining the older timeout")
        .into_effects();
    assert!(
        completed_signature
            .iter()
            .all(|effect| !matches!(effect, AdapterEffect::EnterView { .. }))
    );
    let installed_effects = adapter
        .drain_deferred()
        .expect("service the timeout before the later locked vote");
    assert!(adapter.current_tag().strictly_advances(tag_before_tc));
    assert_eq!(
        adapter.current_tag().generation(),
        reducer::Generation::INITIAL
    );
    assert!(installed_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView {
            protected_body: Some((round, subject)),
            ..
        } if *round == wire_round && *subject == locked_subject
    )));
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
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "EnterView must return to its executor before servicing later deferred ownership"
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone(),))
            .expect("coalesce the exact vote across the EnterView boundary")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the deferred owner must not acquire a new-generation replacement"
    );
    adapter
        .signature_completed(commit_sign_tag, vec![0xB7])
        .expect("complete the reconstructed local vote");
    adapter
        .drain_deferred()
        .expect("service the separately scheduled deferred vote");
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
    let liveness = adapter.status().expect("build post-TC liveness status");
    assert!(liveness.liveness.commit_quorums.iter().any(|quorum| {
        quorum.round == wire_round && quorum.subject == locked_subject && quorum.signer_count == 2
    }));
    assert_eq!(adapter.active_subject, Some((round, core_subject)));

    let key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: 1,
    };
    assert_eq!(
        adapter
            .ingress_deliveries
            .get(&key)
            .expect("deferred delivery is recorded in its consumer generation")
            .generation,
        adapter.current_tag().generation()
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote))
            .expect("suppress a later duplicate in the TC generation")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
}
