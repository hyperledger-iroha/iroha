#[test]
fn pending_kura_tip_requires_exact_decision_body_and_validation_replay() {
    let fixture = Fixture::new();
    let directory = TempDir::new().expect("body-store directory");
    let mut store = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("open body store");
    let durable = store
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist exact body");
    let validated = store
        .validate(&durable, |_| {
            Ok::<_, &'static str>(
                ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment(),
            )
        })
        .expect("persist validation marker");
    drop(store);
    let mut reopened = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("reopen exact body store");
    reopened
        .revalidate_recovered_markers(|_| Ok::<_, String>(validated.execution_commitment()))
        .expect("semantically replay recovered validation marker");
    let recovered = reopened.recovery_catalog().expect("recovery catalog");
    let validations = reopened.validated_recovery_catalog();
    assert_eq!(
        validations
            .get(&(fixture.manifest.round, fixture.manifest.subject))
            .map(ValidatedBodyReceipt::durable),
        Some(&durable)
    );
    let expected = PendingKuraApply::for_test(
        fixture.context.id(),
        fixture.context.height,
        fixture.block.hash(),
    );
    let decision = Some((
        fixture.manifest.round,
        fixture.manifest.round,
        fixture.manifest.subject,
        validated.execution_commitment(),
    ));
    let mut certificate = fixture.qc(wire::GlobalPhase::Commit);
    certificate.execution_commitment = validated.execution_commitment();
    let (authenticated_genesis_context, evidence) = verify_pending_kura_apply_parts(
        &fixture.context,
        decision,
        &recovered,
        &validations,
        expected,
        tag(0),
        tag(0),
        certificate.clone(),
        Some(&fixture.manifest),
    )
    .expect("exact replay binding");
    let authenticated_genesis_context = authenticated_genesis_context
        .expect("height-one replay mints a genesis projection capability");
    assert_eq!(
        authenticated_genesis_context.hash(),
        fixture.context.nexus_amx_context_hash
    );
    assert_eq!(evidence.expected(), expected);
    assert_eq!(evidence.expected().state_height(), 0);
    assert_eq!(evidence.frozen_context_id(), fixture.context.id());
    assert_eq!(evidence.frozen_height(), fixture.context.height);
    assert_eq!(evidence.replay_tag(), tag(0));
    assert_eq!(evidence.owner_tag(), tag(0));
    assert_eq!(evidence.replay_generation(), tag(0).generation().get());
    assert_eq!(evidence.commit_qc(), &certificate);
    assert_eq!(evidence.commit_round(), certificate.round);
    assert_eq!(evidence.commit_phase(), wire::GlobalPhase::Commit);
    assert_eq!(evidence.commit_subject(), certificate.subject);
    assert_eq!(
        evidence.execution_commitment(),
        certificate.execution_commitment
    );
    assert_eq!(evidence.commit_signers(), certificate.signers.as_slice());
    assert_eq!(
        evidence.commit_aggregate_signature(),
        certificate.aggregate_signature.as_slice()
    );
    assert_eq!(evidence.manifest(), &fixture.manifest);
    assert_eq!(evidence.manifest_hash(), HashOf::new(&fixture.manifest));
    assert_eq!(evidence.durable_receipt(), &durable);
    assert_eq!(
        evidence.durable_receipt().frame_hash(),
        durable.frame_hash()
    );
    assert_eq!(evidence.validated_receipt(), &validated);
    assert_eq!(
        evidence.stage(),
        PendingKuraApplyRecoveryStage::CertifiedFetch
    );
    let mut missing_signature = certificate.clone();
    missing_signature.aggregate_signature.clear();
    assert!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &validations,
            expected,
            tag(0),
            tag(0),
            missing_signature,
            Some(&fixture.manifest),
        )
        .is_err()
    );
    let (_, delayed_evidence) = verify_pending_kura_apply_parts(
        &fixture.context,
        decision,
        &recovered,
        &validations,
        expected,
        tag(3),
        tag(3),
        certificate.clone(),
        Some(&fixture.manifest),
    )
    .expect("historical CommitQC remains replayable by the current owner");
    assert_eq!(delayed_evidence.replay_tag(), tag(3));
    assert_eq!(delayed_evidence.owner_tag(), tag(3));
    let mut later_certificate = certificate.clone();
    later_certificate.round.view = fixture
        .manifest
        .round
        .view
        .checked_add(2)
        .expect("fixture reproposal view increment");
    later_certificate.proposal_round = later_certificate.round;
    let later_decision = Some((
        later_certificate.round,
        later_certificate.proposal_round,
        later_certificate.subject,
        later_certificate.execution_commitment,
    ));
    let alias_round = wire::ConsensusRound {
        view: fixture
            .manifest
            .round
            .view
            .checked_add(1)
            .expect("fixture alias view increment"),
        ..fixture.manifest.round
    };
    let alias_manifest = canonical_payload_manifest(
        &fixture.context,
        alias_round,
        fixture.manifest.subject,
        &fixture.body,
    );
    let alias_durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        alias_round,
        fixture.manifest.subject,
        HashOf::new(&alias_manifest),
    );
    let alias_validated = ValidatedBodyReceipt::for_test_with_commitment(
        alias_durable.clone(),
        validated.execution_commitment(),
    );
    let later_manifest = canonical_payload_manifest(
        &fixture.context,
        later_certificate.round,
        fixture.manifest.subject,
        &fixture.body,
    );
    let later_durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        later_certificate.round,
        fixture.manifest.subject,
        HashOf::new(&later_manifest),
    );
    let later_validated = ValidatedBodyReceipt::for_test_with_commitment(
        later_durable.clone(),
        validated.execution_commitment(),
    );
    let mut recovered_with_alias = recovered.clone();
    recovered_with_alias.insert(
        (alias_round, fixture.manifest.subject),
        (alias_manifest, alias_durable),
    );
    recovered_with_alias.insert(
        (later_certificate.round, fixture.manifest.subject),
        (later_manifest.clone(), later_durable),
    );
    let mut validations_with_alias = validations.clone();
    validations_with_alias.insert((alias_round, fixture.manifest.subject), alias_validated);
    validations_with_alias.insert(
        (later_certificate.round, fixture.manifest.subject),
        later_validated,
    );
    let (_, later_finality_evidence) = verify_pending_kura_apply_parts(
        &fixture.context,
        later_decision,
        &recovered_with_alias,
        &validations_with_alias,
        expected,
        tag(0),
        tag(0),
        later_certificate.clone(),
        Some(&later_manifest),
    )
    .expect("reproposal CommitQC selects its same-round body among exact aliases");
    assert_eq!(
        later_finality_evidence.durable_round(),
        later_certificate.round
    );
    assert_eq!(
        later_finality_evidence.commit_round(),
        later_certificate.round
    );
    assert!(later_finality_evidence.is_exact(&fixture.context));
    let mut conflicting_body = fixture.body.clone();
    let first_byte = conflicting_body
        .first_mut()
        .expect("fixture body is non-empty");
    *first_byte ^= 0xff;
    let conflicting_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        alias_round,
        fixture.manifest.subject,
        &conflicting_body,
    );
    let conflicting_durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        alias_round,
        fixture.manifest.subject,
        HashOf::new(&conflicting_manifest),
    );
    let mut recovered_with_conflict = recovered_with_alias.clone();
    recovered_with_conflict.insert(
        (alias_round, fixture.manifest.subject),
        (conflicting_manifest, conflicting_durable),
    );
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            later_decision,
            &recovered_with_conflict,
            &validations_with_alias,
            expected,
            tag(0),
            tag(0),
            later_certificate.clone(),
            Some(&later_manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("aliases conflict")
    ));
    let later_sources = certified_sources(&fixture, &later_certificate);
    assert_eq!(
        later_finality_evidence
            .transition_for_effect(&AdapterEffect::FetchBody {
                tag: tag(0),
                round: later_certificate.round,
                subject: fixture.manifest.subject,
                manifest: Some(later_manifest),
                certified_sources: later_sources,
                certificate: Some(later_certificate),
            })
            .expect("same-round reproposal Fetch is authorized by its CommitQC"),
        PendingKuraApplyRecoveryStage::DurableStore
    );
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &validations,
            expected,
            tag(3),
            tag(2),
            certificate.clone(),
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("frozen reducer incarnation")
    ));
    let mut altered_generation = evidence.clone();
    altered_generation.replay_generation = altered_generation
        .replay_generation
        .checked_add(1)
        .expect("fixture generation increment");
    assert!(!altered_generation.is_exact(&fixture.context));
    let mut altered_manifest = evidence.clone();
    altered_manifest.manifest.chunk_root = Hash::new(b"altered recovery manifest root");
    assert!(!altered_manifest.is_exact(&fixture.context));
    let mut altered_frame = evidence.clone();
    altered_frame.durable_receipt = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert_ne!(
        altered_frame.durable_receipt().frame_hash(),
        altered_frame.durable_frame_hash()
    );
    assert!(!altered_frame.is_exact(&fixture.context));
    let apply_effect = AdapterEffect::Apply {
        tag: tag(0),
        subject: fixture.manifest.subject,
        certificate: certificate.clone(),
    };
    let certified_sources = certified_sources(&fixture, &certificate);
    let recovery_sequence = [
        AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: None,
            certified_sources,
            certificate: Some(certificate.clone()),
        },
        AdapterEffect::StoreBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        },
        AdapterEffect::ValidateBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        },
        apply_effect.clone(),
    ];
    let mut staged = evidence.clone();
    for effect in &recovery_sequence {
        staged.advance_stage_for_test(effect);
    }
    assert_eq!(
        staged.stage(),
        PendingKuraApplyRecoveryStage::ApplicationDispatched
    );

    let mut clock_fixture = ProductionTransportFixture::new();
    clock_fixture.executor.pending_tip_recovery = Some(evidence.clone());
    assert_eq!(
        clock_fixture.executor.arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            Instant::now(),
        ),
        Err(RuntimeClockError::PendingKuraRecovery),
        "pending Kura recovery must keep the ordinary pacemaker sealed",
    );
    let mut non_local_executor = fixture.executor(EffectQueueConfig::default());
    non_local_executor.pending_tip_recovery = Some(evidence.clone());
    let mut non_local_services = fixture.services();
    assert!(matches!(
        non_local_executor.consume_pending_tip_recovery_effects(
            vec![AdapterEffect::Broadcast(proposal(&fixture))],
            &mut non_local_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("non-local consensus effect")
    ));
    let mut direct_apply_executor = fixture.executor(EffectQueueConfig::default());
    direct_apply_executor.validated_bodies = validations.clone();
    direct_apply_executor.pending_tip_recovery = Some(evidence.clone());
    let mut direct_apply_services = fixture.services();
    assert!(matches!(
        direct_apply_executor.consume_pending_tip_recovery_effects(
            vec![apply_effect.clone()],
            &mut direct_apply_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("does not match its exact authenticated stage")
    ));
    let mut apply_stage = evidence.clone();
    apply_stage.enter_apply_stage_for_test();
    assert_eq!(
        apply_stage
            .transition_for_effect(&apply_effect)
            .expect("exact Apply advances the closed-ingress stage"),
        PendingKuraApplyRecoveryStage::ApplicationDispatched
    );
    let mut altered_signers = certificate.clone();
    altered_signers.signers.swap(0, 1);
    assert!(
        apply_stage
            .transition_for_effect(&AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: altered_signers,
            })
            .is_err(),
        "recovery must compare the complete canonical signer evidence"
    );
    let mut altered_signature = certificate.clone();
    altered_signature.aggregate_signature.push(0xA5);
    assert!(
        apply_stage
            .transition_for_effect(&AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: altered_signature,
            })
            .is_err(),
        "recovery must compare the complete aggregate-signature evidence"
    );
    for effects in [Vec::new(), vec![apply_effect.clone(), apply_effect.clone()]] {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.validated_bodies = validations.clone();
        executor.pending_tip_recovery = Some(apply_stage.clone());
        let mut services = fixture.services();
        assert!(matches!(
            executor.consume_pending_tip_recovery_effects(effects, &mut services),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("must emit exactly one effect")
        ));
    }
    let mut wrong_context = fixture.context.clone();
    wrong_context.nexus_amx_context_hash = Hash::new(b"different frozen Nexus/AMX context");
    assert_ne!(
        wrong_context.id(),
        fixture.context.id(),
        "height-context identity must bind the Nexus/AMX projection"
    );
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &wrong_context,
            decision,
            &recovered,
            &validations,
            expected,
            tag(0),
            tag(0),
            certificate.clone(),
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("different frozen height context")
    ));
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            None,
            &recovered,
            &validations,
            expected,
            tag(0),
            tag(0),
            certificate.clone(),
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("no complete durable Decision")
    ));
    let wrong_tip = PendingKuraApply::for_test(
        fixture.context.id(),
        fixture.context.height,
        HashOf::from_untyped_unchecked(Hash::new(b"different Kura tip")),
    );
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &validations,
            wrong_tip,
            tag(0),
            tag(0),
            certificate.clone(),
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("does not identify the canonical")
    ));
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &BTreeMap::new(),
            expected,
            tag(0),
            tag(0),
            certificate.clone(),
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("no matching durable validation marker")
    ));
    let mismatched_execution_commitment = fixture_execution_commitment();
    assert_ne!(
        mismatched_execution_commitment,
        validated.execution_commitment(),
        "the adversarial Decision fixture must change the consensus-bound execution result"
    );
    let mut mismatched_certificate = certificate.clone();
    mismatched_certificate.execution_commitment = mismatched_execution_commitment;
    assert!(matches!(
        verify_pending_kura_apply_parts(
            &fixture.context,
            Some((
                fixture.manifest.round,
                fixture.manifest.round,
                fixture.manifest.subject,
                mismatched_execution_commitment,
            )),
            &recovered,
            &validations,
            expected,
            tag(0),
            tag(0),
            mismatched_certificate,
            Some(&fixture.manifest),
        ),
        Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
            if reason.contains("Decision commitment differs")
    ));
    assert_eq!(validated.durable(), &durable);
}

#[test]
fn mismatched_kura_completion_fails_closed_before_application_ack() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor
        .consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: commit.clone(),
            }],
            &mut services,
        )
        .expect("begin apply");
    let work_id = services.apply_tasks[0].id();
    let mut artifact = wire::finality::V2FinalityArtifact::new(
        fixture.context.clone(),
        fixture.manifest.subject,
        commit,
        vec![vec![0x5D]; fixture.context.roster.len()],
    );
    artifact.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong block"));
    let receipt = KuraV2CommitReceipt::for_test(&artifact);
    let completions_before = executor.runtime.completions.len();
    assert!(matches!(
        executor.complete_application(
            DurableApplyCompletion::new(work_id, receipt, artifact),
            &mut services,
        ),
        Err(EffectExecutorError::InvalidApplyCompletion)
    ));
    assert_eq!(executor.runtime.completions.len(), completions_before);
    assert!(executor.status().fail_closed);
}
#[test]
fn service_runtime_body_store_and_status_failures_close_executor() {
    let fixture = Fixture::new();
    let mut runtime_executor = fixture.executor(EffectQueueConfig::default());
    let mut runtime_services = fixture.services();
    runtime_executor
        .runtime
        .steps
        .push_back(Err("driver failed".to_owned()));
    assert!(matches!(
        runtime_executor.step(Instant::now(), &mut runtime_services),
        Err(EffectExecutorError::Runtime(_))
    ));
    let mut body_executor = fixture.executor(EffectQueueConfig::default());
    let mut body_services = fixture.services();
    body_executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut body_services,
        )
        .expect("queue asynchronous body store");
    let store_id = body_services.store_tasks[0].id();
    assert!(matches!(
        body_executor.body_service_failed(store_id, "fsync failed", &mut body_services,),
        Err(EffectExecutorError::BodyStore(_))
    ));
    let mut status_executor = fixture.executor(EffectQueueConfig::default());
    let mut status_services = fixture.services();
    status_services.fail_on = Some("status");
    assert!(matches!(
        status_executor.consume_effects(Vec::new(), &mut status_services),
        Err(EffectExecutorError::Service(_))
    ));
    assert!(status_executor.status().fail_closed);
    assert!(
        status_services.fail_on.is_none(),
        "failure injection was not consumed"
    );
}
#[test]
fn proposal_fanout_retires_active_producer_only_after_service_acceptance() {
    let fixture = Fixture::new();
    let message = proposal(&fixture);
    let mut failed = fixture.executor(EffectQueueConfig::default());
    failed.runtime.active_view_producer_retained = true;
    let mut failed_services = fixture.services();
    failed_services.fail_on = Some("broadcast");
    failed
        .consume_effects(
            vec![AdapterEffect::Broadcast(message.clone())],
            &mut failed_services,
        )
        .expect("Proposal fanout stops at lifecycle admission before service I/O");
    assert!(
        failed.runtime.active_view_producer_retained,
        "an admitted but unexecuted fanout must retain the exact producer fence"
    );
    assert!(failed.runtime.completed_proposal_fanouts.is_empty());
    assert_eq!(failed.pending_lifecycle_output_admissions.len(), 1);
    assert_eq!(failed_services.fail_on, Some("broadcast"));
    assert!(failed_services.broadcast_attempts.is_empty());
    assert!(failed_services.broadcasts.is_empty());
    let mut retained = fixture.executor(EffectQueueConfig::default());
    retained.runtime.active_view_producer_retained = true;
    let mut retained_services = fixture.services();
    retained_services
        .broadcast_dispositions
        .push_back(ConsensusBroadcastDisposition::SourceRetained);
    retained
        .consume_effects(
            vec![AdapterEffect::Broadcast(message.clone())],
            &mut retained_services,
        )
        .expect("Proposal source reaches the lifecycle-owned service boundary");
    assert!(
        retained.runtime.active_view_producer_retained,
        "a source-retained Proposal must keep the active producer fence"
    );
    assert!(retained.runtime.completed_proposal_fanouts.is_empty());
    assert_eq!(retained.pending_lifecycle_output_admissions.len(), 1);
    assert!(retained_services.broadcast_attempts.is_empty());
    assert!(retained_services.broadcasts.is_empty());
    assert!(retained.retained_effect_batch.is_none());
    assert!(!retained.status().fail_closed);
    retained
        .consume_effects(
            vec![AdapterEffect::Broadcast(message.clone())],
            &mut retained_services,
        )
        .expect("periodic Proposal retransmission stutters behind the exact owner");
    assert!(retained.runtime.active_view_producer_retained);
    assert_eq!(retained.pending_lifecycle_output_admissions.len(), 1);
    assert!(retained_services.broadcast_attempts.is_empty());
    assert!(retained.runtime.completed_proposal_fanouts.is_empty());
    let effect = AdapterEffect::Broadcast(message.clone());
    let ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&effect),
        vec![retained.runtime.test_effect_ownership(&effect)],
    )
    .expect("reconstruct the exact one-effect Proposal occurrence")
    .pop()
    .expect("one Proposal output owner");
    let key = *retained
        .pending_lifecycle_output_admissions
        .keys()
        .next()
        .expect("one parked Proposal owner");
    let pending = retained
        .pending_lifecycle_output_admissions
        .remove(&key)
        .expect("transfer Proposal ownership into lifecycle service settlement");
    assert_eq!(
        retained
            .execute_lifecycle_output_service(&effect, &ownership, &mut retained_services)
            .expect("source-retained Proposal service result"),
        LifecycleOutputServiceDispositionV1::SourceRetained
    );
    assert!(retained.runtime.active_view_producer_retained);
    assert!(retained.runtime.completed_proposal_fanouts.is_empty());
    assert!(
        retained
            .pending_lifecycle_output_admissions
            .insert(key, pending)
            .is_none()
    );
    let _accepted_pending = retained
        .pending_lifecycle_output_admissions
        .remove(&key)
        .expect("retry the same Proposal owner after service capacity changes");
    assert_eq!(
        retained
            .execute_lifecycle_output_service(&effect, &ownership, &mut retained_services)
            .expect("accepted Proposal service result"),
        LifecycleOutputServiceDispositionV1::Accepted
    );
    assert!(!retained.runtime.active_view_producer_retained);
    assert_eq!(retained.runtime.completed_proposal_fanouts.len(), 1);
    assert_eq!(retained_services.broadcast_attempts.len(), 2);
    let mut accepted = fixture.executor(EffectQueueConfig::default());
    accepted.runtime.active_view_producer_retained = true;
    let mut accepted_services = fixture.services();
    accepted
        .consume_effects(
            vec![AdapterEffect::Broadcast(message.clone())],
            &mut accepted_services,
        )
        .expect("guarded Proposal fanout transfers the exact source owner");
    assert!(accepted.runtime.active_view_producer_retained);
    assert_eq!(accepted.pending_lifecycle_output_admissions.len(), 1);
    assert!(accepted_services.broadcast_attempts.is_empty());
    assert!(accepted_services.broadcasts.is_empty());
    assert!(accepted.runtime.completed_proposal_fanouts.is_empty());
}
#[test]
fn source_retained_non_proposal_control_remains_retransmittable() {
    let fixture = Fixture::new();
    let control =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote(&fixture)));
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    services
        .broadcast_dispositions
        .push_back(ConsensusBroadcastDisposition::SourceRetained);
    executor
        .consume_effects(
            vec![AdapterEffect::Broadcast(control.clone())],
            &mut services,
        )
        .expect("ordinary control reaches lifecycle admission");
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(services.broadcast_attempts.is_empty());
    assert!(services.broadcasts.is_empty());
    assert!(executor.runtime.completed_proposal_fanouts.is_empty());
    assert!(!executor.status().fail_closed);
    executor
        .consume_effects(
            vec![AdapterEffect::Broadcast(control.clone())],
            &mut services,
        )
        .expect("periodic control retransmission stutters behind the exact owner");
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(services.broadcast_attempts.is_empty());
    assert!(services.broadcasts.is_empty());
    assert!(executor.runtime.completed_proposal_fanouts.is_empty());
    assert!(!executor.status().fail_closed);
}
fn leader_wire_runtime_terminal_fixture(
    fixture: &Fixture,
    scheduler_ordinal: u128,
) -> (TempDir, LeaderWireRuntimeTerminal) {
    let directory = TempDir::new().expect("temporary leader-wire terminal directory");
    let origin = fixture.context.roster[0].validator.clone();
    let phase = super::super::FairV2IngressLeaderWirePhase::PrepareVote;
    let token = super::super::FairV2IngressLeaderWireToken {
        identity: super::super::FairV2IngressLeaderWireIdentity {
            context_id: fixture.context.id(),
            height: fixture.context.height,
            view: fixture.manifest.round.view,
            subject_hash: Hash::new(b"effect-dispatch leader-wire subject"),
            manifest_hash: None,
            phase,
            semantic_origin: origin.clone(),
            canonical_wire_hash: Hash::new(b"effect-dispatch leader-wire bytes"),
        },
        slot: super::super::FairV2IngressLeaderWireSlot {
            semantic_origin: origin,
            phase,
            chunk_index: None,
        },
        admission_ordinal: 1,
        scheduler_ordinal,
        source_class: super::super::FairV2IngressLeaderWireSourceClass::Control,
    };
    let owner =
        [u8::try_from(scheduler_ordinal).expect("leader-wire fixture ordinal fits one byte"); 32];
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            fixture.context.roster.len(),
            fixture.context.da_layout.max_chunk_count,
        )
        .expect("finite leader-wire effect fixture capacity");
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            fixture.context.id(),
            fixture.context.height,
            owner,
            fixture.manifest.round.view,
            false,
        );
    let (gate, _) = super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &directory.path().join("effect-dispatch.wal"),
        fixture.context.id(),
        fixture.context.height,
        owner,
        fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        capacity,
        fixture.context.da_layout.max_chunk_count,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open leader-wire effect fixture");
    gate.reserve(token.clone())
        .expect("reserve leader-wire effect owner");
    gate.mark_ingress(&token)
        .expect("transfer leader-wire effect owner to ingress");
    let runtime_owner = super::super::serviced_candidate_store::LeaderWireRuntimeOwner::new(
        token.identity_hash(),
        token.scheduler_ordinal(),
    )
    .expect("construct leader-wire effect runtime owner");
    let receipt = gate
        .mark_runtime(&token, runtime_owner)
        .expect("transfer leader-wire effect owner to runtime");
    (directory, LeaderWireRuntimeTerminal::Volatile(receipt))
}
#[test]
fn effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains() {
    let fixture = Fixture::new();
    let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 97);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    executor
        .runtime
        .leader_wire_terminal_batches
        .push_back(Vec::new());
    executor
        .runtime
        .leader_wire_terminal_batches
        .push_back(vec![terminal.clone()]);
    let mut services = fixture.services();
    assert_eq!(
        executor
            .consume_effects(
                vec![AdapterEffect::ReportEquivocation {
                    evidence: vote_equivocation_evidence(&fixture, 1),
                }],
                &mut services,
            )
            .expect("dispatch batch and consume its late terminal"),
        1
    );
    assert_eq!(services.leader_wire_terminals, vec![terminal]);
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("a consumed terminal cannot fail-close the next scheduler turn"),
        EffectExecutorStep::Idle
    );
}
#[test]
fn lock_reconciliation_consumes_retirement_terminal_before_the_next_turn() {
    let fixture = Fixture::new();
    let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 96);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    executor.runtime.leader_wire_terminal_after_lock = Some(terminal.clone());
    let mut services = fixture.services();
    executor
        .reconcile_locked_body_for_recovery(
            tag(fixture.manifest.round.view),
            (fixture.manifest.round, fixture.manifest.subject),
            &mut services,
        )
        .expect("lock retirement transfers its terminal in the same synchronous call");
    assert_eq!(services.leader_wire_terminals, vec![terminal]);
    assert!(executor.runtime.leader_wire_terminal_after_lock.is_none());
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("the next scheduler turn cannot overtake a lock-retirement terminal"),
        EffectExecutorStep::Idle
    );
}
#[test]
fn leader_wire_terminal_batch_attempts_every_owner_after_one_transfer_fails() {
    let fixture = Fixture::new();
    let (_first_directory, first) = leader_wire_runtime_terminal_fixture(&fixture, 95);
    let (_second_directory, second) = leader_wire_runtime_terminal_fixture(&fixture, 96);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    executor
        .runtime
        .leader_wire_terminal_batches
        .push_back(vec![first, second.clone()]);
    let mut services = fixture.services();
    services.fail_on = Some("leader-wire-terminal");
    assert!(
        executor.consume_effects(Vec::new(), &mut services).is_err(),
        "the first injected terminal-transfer failure must fail closed"
    );
    assert_eq!(
        services.leader_wire_terminals,
        vec![second],
        "a failed first transfer cannot drop later independent runtime owners"
    );
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn retained_live_retry_consumes_decision_retirement_terminal_same_cycle() {
    let fixture = Fixture::new();
    let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 98);
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    assert_eq!(
        executor
            .consume_effects(
                vec![timeout_sign(&fixture, 0), timeout_sign(&fixture, 1)],
                &mut services,
            )
            .expect("retain the second timeout-sign effect at pending-work capacity"),
        1
    );
    assert!(executor.retained_effect_batch.is_some());
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("Decision retires the retained suffix and transfers its runtime terminal"),
        EffectExecutorStep::Idle
    );
    assert_eq!(services.leader_wire_terminals, vec![terminal]);
    assert!(
        executor
            .runtime
            .leader_wire_terminal_after_decision
            .is_none()
    );
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_signatures.is_empty());
    assert!(!executor.status().fail_closed);
}
#[test]
fn retained_drain_failure_transfers_decision_terminal_before_fail_close() {
    let fixture = Fixture::new();
    let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 100);
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    assert_eq!(
        executor
            .consume_effects(
                vec![timeout_sign(&fixture, 0), timeout_sign(&fixture, 1)],
                &mut services,
            )
            .expect("retain the second timeout-sign effect at pending-work capacity"),
        1
    );
    assert!(executor.retained_effect_batch.is_some());
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());
    services.fail_on = Some("cancel-sign");
    assert!(
        executor.step(Instant::now(), &mut services).is_err(),
        "the injected retained-suffix cancellation failure must fail closed"
    );
    assert_eq!(
        services.leader_wire_terminals,
        vec![terminal],
        "the earlier Decision terminal must cross its gate before fail-close teardown"
    );
    assert!(
        executor
            .runtime
            .leader_wire_terminal_after_decision
            .is_none()
    );
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle() {
    let fixture = Fixture::new();
    let (_directory, terminal) = leader_wire_runtime_terminal_fixture(&fixture, 99);
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    assert_eq!(
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0), fetch], &mut services)
            .expect("retain the exact recovery fetch at pending-work capacity"),
        1
    );
    assert!(executor.retained_effect_batch.is_some());
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    executor.recovered_bodies.insert(
        (fixture.manifest.round, fixture.manifest.subject),
        (fixture.manifest.clone(), durable),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    executor.runtime.leader_wire_terminal_after_decision = Some(terminal.clone());
    assert_eq!(
        executor
            .step_pending_tip_recovery(Instant::now(), &mut services)
            .expect("recovery retry consumes the Decision retirement terminal"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.leader_wire_terminals, vec![terminal]);
    assert!(
        executor
            .runtime
            .leader_wire_terminal_after_decision
            .is_none()
    );
    assert!(executor.runtime.leader_wire_terminal_batches.is_empty());
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_signatures.is_empty());
    assert_eq!(
        executor.status().pending_tip_recovery_last_result,
        Some(PendingTipRecoveryAttemptResult::Advanced)
    );
    assert!(!executor.status().fail_closed);
}
#[test]
fn body_fetch_authority_upgrades_monotonically_in_both_orders() {
    let fixture = Fixture::new();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("proposal starts ordinary acquisition");
    let work_id = services.fetch_tasks[0].id();
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources.clone(),
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("PrepareQC adds certified authority");
    let upgraded = services.fetch_tasks.last().expect("upgraded task");
    assert_eq!(upgraded.id(), work_id);
    assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
    assert_eq!(
        upgraded
            .certified_request()
            .map(|request| &request.certificate),
        Some(&prepare)
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
    let first_request = upgraded
        .certified_request()
        .expect("first certified authority")
        .clone();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources.clone(),
                certificate: Some(commit),
            }],
            &mut services,
        )
        .expect("later same-subject QC retransmits first authority");
    assert_eq!(
        services
            .fetch_tasks
            .last()
            .and_then(BodyFetchTask::certified_request),
        Some(&first_request)
    );
    assert_eq!(executor.outstanding_requests.len(), 1);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources: sources.clone(),
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("PrepareQC starts certified acquisition");
    let work_id = services.fetch_tasks[0].id();
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("proposal adds manifest authority");
    let upgraded = services.fetch_tasks.last().expect("upgraded task");
    assert_eq!(upgraded.id(), work_id);
    assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
    assert_eq!(
        upgraded
            .certified_request()
            .map(|request| &request.certificate),
        Some(&prepare)
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
}
#[test]
fn hybrid_reconstruction_wins_and_retires_certified_request() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("start hybrid acquisition");
    let task = services.fetch_tasks[0].clone();
    assert_eq!(
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("authenticated reconstruction wins"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
}
#[test]
fn authenticated_genesis_satisfies_later_view_fetch_through_normal_body_pipeline() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .install_authenticated_genesis_body_for_test(&fixture.block)
        .expect("retain authenticated staged genesis");
    let manifest = manifest_at_view(&fixture, 5);
    let round = manifest.round;
    let subject = manifest.subject;
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(5),
                round,
                subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("derive the later-view manifest from authenticated genesis");
    assert!(services.fetch_tasks.is_empty());
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(executor.ready_bodies[&(round, subject)].manifest, manifest);
    assert_eq!(
        executor.ready_bodies[&(round, subject)].bytes.as_ref(),
        fixture.body.as_slice()
    );
    assert!(executor.durable_bodies.is_empty());
    assert!(executor.validated_bodies.is_empty());
    assert_eq!(
        executor.runtime.completions,
        vec![RuntimeCompletion::BodyAvailable(tag(5), manifest.clone())]
    );
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(5),
                round,
                subject,
            }],
            &mut services,
        )
        .expect("enter the ordinary durable-store stage");
    assert_eq!(services.store_tasks.len(), 1);
    assert_eq!(services.store_tasks[0].manifest(), &manifest);
    assert_eq!(
        services.store_tasks[0].canonical_wire(),
        fixture.body.as_slice()
    );
    let store_id = services.store_tasks[0].id();
    let store_completion = services.execute_store(store_id);
    executor
        .complete_body_store(store_completion, &mut services)
        .expect("complete the current-round durable store");
    assert_eq!(executor.durable_bodies[&(round, subject)].round(), round);
    executor
        .consume_effects(
            vec![AdapterEffect::ValidateBody {
                tag: tag(5),
                round,
                subject,
            }],
            &mut services,
        )
        .expect("enter ordinary deterministic validation");
    assert_eq!(executor.pending_durable_validate_admissions.len(), 1);
    assert!(
        executor
            .pending_durable_validate_admissions
            .contains_key(&(round, subject))
    );
    assert!(executor.validated_bodies.is_empty());
}
#[test]
fn authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    executor.runtime.retain_body_available_effect_ownership = true;
    let mut services = fixture.services();
    executor
        .install_authenticated_genesis_body_for_test(&fixture.block)
        .expect("retain authenticated staged genesis");
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let sources = certified_sources(&fixture, &certificate);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources: sources,
                certificate: Some(certificate),
            }],
            &mut services,
        )
        .expect("consume certified Decision from authenticated local genesis");
    assert!(services.fetch_tasks.is_empty());
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(
        executor.ready_bodies[&(fixture.manifest.round, fixture.manifest.subject)].manifest,
        fixture.manifest
    );
    assert_eq!(
        executor.runtime.completions,
        vec![RuntimeCompletion::BodyAvailable(
            tag(0),
            fixture.manifest.clone()
        )]
    );
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("advance authenticated genesis through Store");
    let store_id = services.store_tasks[0].id();
    let completion = services.execute_store(store_id);
    executor
        .complete_body_store(completion, &mut services)
        .expect("fsync authenticated genesis body");
    executor
        .consume_effects(
            vec![AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("admit authenticated genesis Validate through the closed LocalBody cut");
    assert_eq!(executor.pending_durable_validate_admissions.len(), 1);
    assert!(
        executor
            .pending_durable_validate_admissions
            .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
    );
    assert!(
        !executor.pending_durable_validate_admissions
            [&(fixture.manifest.round, fixture.manifest.subject)]
            .projects_local_proposal_handoff_for_test(),
        "certified genesis enters the closed LocalBody admission surface without becoming a local proposal"
    );
    assert!(executor.authenticated_genesis_replay.is_empty());
}
#[test]
fn authenticated_genesis_cache_does_not_satisfy_a_different_subject() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .install_authenticated_genesis_body_for_test(&fixture.block)
        .expect("retain authenticated staged genesis");
    let proposal_round = round(&fixture.context, 4);
    let (subject, body) = distinct_body(&fixture);
    let manifest = canonical_payload_manifest(&fixture.context, proposal_round, subject, &body);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(4),
                round: proposal_round,
                subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("unrelated proposal uses network acquisition");
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(services.fetch_tasks[0].manifest(), Some(&manifest));
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.runtime.completions.is_empty());
}
#[test]
fn retained_exact_body_pipeline_prevents_reacquisition_at_every_stage() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    executor
        .consume_effects(vec![fetch.clone()], &mut services)
        .expect("start one exact acquisition");
    let task = services.fetch_tasks[0].clone();
    executor
        .complete_body_reconstruction(
            &task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("retain reconstructed body");
    assert_eq!(executor.runtime.queued_commands(), 1);
    executor
        .consume_effects(vec![fetch.clone()], &mut services)
        .expect("ready body makes FetchBody idempotent");
    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(executor.runtime.queued_commands(), 1);
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("advance body into exact store ownership");
    executor
        .consume_effects(vec![fetch.clone()], &mut services)
        .expect("pending store makes FetchBody idempotent");
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(executor.pending_stores.len(), 1);
    assert_eq!(executor.runtime.queued_commands(), 1);
    let store_id = services.store_tasks[0].id();
    let completion = services.execute_store(store_id);
    executor
        .complete_body_store(completion, &mut services)
        .expect("advance body into durable ownership");
    assert_eq!(executor.runtime.queued_commands(), 2);
    executor
        .consume_effects(vec![fetch], &mut services)
        .expect("durable receipt makes FetchBody idempotent");
    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.durable_bodies.len(), 1);
    assert_eq!(executor.runtime.queued_commands(), 2);
    let mut conflicting_manifest = fixture.manifest.clone();
    conflicting_manifest.payload_size_bytes = conflicting_manifest
        .payload_size_bytes
        .checked_add(1)
        .expect("small fixture body");
    let conflicting_result = executor.consume_effects(
        vec![AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(conflicting_manifest),
            certified_sources: Vec::new(),
            certificate: None,
        }],
        &mut services,
    );
    assert!(
        matches!(conflicting_result, Err(EffectExecutorError::Contract(_))),
        "conflicting retained manifest must fail closed: {conflicting_result:?}"
    );
    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(executor.status().fail_closed);
}
#[test]
fn uncertified_fetch_rejects_spurious_certified_sources() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: vec![fixture.context.roster[0].validator.clone()],
                certificate: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert!(services.fetch_tasks.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn fetch_retransmissions_reuse_one_work_slot_and_one_signed_request() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let expected_sources = fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    assert_eq!(sources, expected_sources);
    assert!(
        !prepare.signers.contains(&3) && sources.contains(&fixture.context.roster[3].validator),
        "the immutable archive fanout includes a frozen-roster non-QC signer"
    );
    let effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(prepare),
    };
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("admit the exact certified fetch");
    let lifecycle_ordinal_after_first = executor.runtime.next_lifecycle_ordinal;
    assert_eq!(
        executor
            .consume_effects(vec![effect], &mut services)
            .expect("redispatch one same-owner retransmission"),
        1
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(services.fetch_tasks.len(), 2);
    let first_id = services.fetch_tasks[0].id();
    let first_request = services.fetch_tasks[0]
        .certified_request()
        .expect("certified request")
        .clone();
    assert!(services.fetch_tasks.iter().all(|task| {
        task.id() == first_id
            && task.certified_request() == Some(&first_request)
            && task.sources() == sources.as_slice()
    }));
    let first_lifecycle_ordinal = services.fetch_tasks[0].lifecycle_ordinal();
    assert!(
        services
            .fetch_tasks
            .iter()
            .all(|task| task.lifecycle_ordinal() == first_lifecycle_ordinal),
        "every retry must retain the incumbent fetch's original owner"
    );
    assert_eq!(
        executor.runtime.next_lifecycle_ordinal, lifecycle_ordinal_after_first,
        "same-owner retry cannot mint another lifecycle ordinal"
    );
    assert_eq!(executor.status().effect_dispatch_queue.depth, 0);
    assert!(executor.retained_effect_batch.is_none());
}
#[test]
fn conflicting_fetch_retransmission_fails_closed() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    executor
        .consume_effects(vec![effect], &mut services)
        .expect("first fetch");
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: EventTag::new(1, 0, Generation::new(8)),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(executor.status().fail_closed);
}
#[test]
fn apply_retransmissions_reuse_one_work_slot() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let effect = AdapterEffect::Apply {
        tag: tag(0),
        subject: fixture.manifest.subject,
        certificate: certificate.clone(),
    };
    let lifecycle_ordinal_before = executor.runtime.next_lifecycle_ordinal;
    for _ in 0..8 {
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("redispatch the exact Apply through its incumbent lifecycle");
    }
    assert_eq!(executor.pending_applications.len(), 1);
    assert_eq!(services.apply_tasks.len(), 8);
    let id = services.apply_tasks[0].id();
    assert!(services.apply_tasks.iter().all(|task| task.id() == id));
    let lifecycle_ordinal = services.apply_tasks[0].lifecycle_ordinal();
    assert!(
        services
            .apply_tasks
            .iter()
            .all(|task| task.lifecycle_ordinal() == lifecycle_ordinal),
        "every retry must retain the incumbent task's original owner"
    );
    assert_eq!(
        executor.runtime.next_lifecycle_ordinal,
        lifecycle_ordinal_before + 1,
        "eight retries retain the first immutable Apply owner"
    );
    let mut alternate_evidence = fixture.qc(wire::GlobalPhase::Commit);
    alternate_evidence.aggregate_signature = vec![2];
    executor
        .consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: alternate_evidence,
            }],
            &mut services,
        )
        .expect("coalesce alternate valid evidence for the same committed decision");
    assert_eq!(services.apply_tasks.len(), 9);
    assert_eq!(services.apply_tasks[8].certificate(), &certificate);
    assert!(!executor.status().fail_closed);
    executor.runtime.effect_owners.clear();
    executor
        .consume_effects(vec![effect], &mut services)
        .expect("an exact duplicate decision carrier retains the live Apply owner");
    assert_eq!(services.apply_tasks.len(), 10);
    assert_eq!(services.apply_tasks[9].id(), id);
    assert_eq!(
        services.apply_tasks[9].lifecycle_ordinal(),
        lifecycle_ordinal
    );
    assert!(!executor.status().fail_closed);
    let mut conflicting = fixture.qc(wire::GlobalPhase::Commit);
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"conflicting terminal parent state"),
        Hash::new(b"conflicting terminal post state"),
        Hash::new(b"conflicting terminal ordinary writes"),
        1,
        Hash::new(b"conflicting terminal executed block"),
    );
    executor.runtime.effect_owners.clear();
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: conflicting,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason == "conflicting Apply retransmission for one height"
    ));
    assert_eq!(services.apply_tasks.len(), 10);
    assert!(executor.status().fail_closed);
}
#[test]
fn apply_retransmission_after_durable_finality_does_not_schedule_a_second_write() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let effect = AdapterEffect::Apply {
        tag: tag(0),
        subject: fixture.manifest.subject,
        certificate: certificate.clone(),
    };
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("begin application");
    let work_id = services.apply_tasks[0].id();
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.context.clone(),
        fixture.manifest.subject,
        certificate.clone(),
        vec![vec![0x5C]; fixture.context.roster.len()],
    );
    let receipt = KuraV2CommitReceipt::for_test(&artifact);
    executor
        .complete_application(
            DurableApplyCompletion::new(work_id, receipt, artifact.clone()),
            &mut services,
        )
        .expect("durable application");
    assert!(executor.pending_applications.is_empty());
    assert_eq!(services.apply_tasks.len(), 1);
    let completions_before = executor.runtime.completions.clone();
    // Keep ApplicationCompleted queued in the fake runtime and reproduce
    // the production timer/CommitQC race which rediscovered Apply first.
    // The runtime retains its authoritative incarnation after finality;
    // the durable completion itself remains the exact retry tombstone.
    executor.runtime.effect_owners.clear();
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("coalesce a foreign-owner exact Apply after durable finality");
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(executor.runtime.completions, completions_before);
    for _ in 0..7 {
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("coalesce post-finality Apply retransmission");
    }
    assert!(executor.pending_applications.is_empty());
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(executor.runtime.completions, completions_before);
    assert_eq!(
        executor.durable_finality().expect("durable finality").1,
        &artifact
    );
    assert!(!executor.status().fail_closed);
    let conflicting_apply_ownership = bound_test_apply_ownership(
        tag(1),
        fixture.manifest.subject,
        &certificate,
        tag(1),
        u128::MAX - 1,
    );
    assert!(matches!(
        executor.begin_apply(
            tag(1),
            fixture.manifest.subject,
            certificate,
            conflicting_apply_ownership,
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason == "conflicting Apply retransmission after durable finality"
    ));
    assert!(!executor.status().fail_closed);
    let mut alternate_evidence = fixture.qc(wire::GlobalPhase::Commit);
    alternate_evidence.aggregate_signature = vec![2];
    executor
        .consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: alternate_evidence,
            }],
            &mut services,
        )
        .expect("coalesce alternate evidence for the durable committed decision");
    assert_eq!(services.apply_tasks.len(), 1);
    assert!(!executor.status().fail_closed);
    let mut conflicting = fixture.qc(wire::GlobalPhase::Commit);
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"conflicting terminal parent state"),
        Hash::new(b"conflicting terminal post state"),
        Hash::new(b"conflicting terminal ordinary writes"),
        1,
        Hash::new(b"conflicting terminal executed block"),
    );
    executor.runtime.effect_owners.clear();
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: conflicting,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("conflicting Apply retransmission after durable finality")
    ));
    assert_eq!(services.apply_tasks.len(), 1);
    assert!(executor.status().fail_closed);
}
#[test]
fn tc_body_rebind_preserves_the_exact_fetch_until_reconstruction_completes() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    let high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
    let sources = certified_sources(&fixture, &high_prepare);
    let fetch = |view| AdapterEffect::FetchBody {
        tag: consumer_tag(view),
        round: high_prepare.round,
        subject: high_prepare.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(high_prepare.clone()),
    };
    executor
        .consume_effects(vec![fetch(0)], &mut services)
        .expect("begin exact high-QC fetch");
    let work_id = services.fetch_tasks[0].id();
    for view in 0..3 {
        let mut timeout = timeout_at_view(&fixture, view);
        timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(view + 1),
                    certificate: timeout,
                    protected_lock: Some(high_prepare.clone()),
                }],
                &mut services,
            )
            .expect("rebind protected fetch across certified view");
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(
            executor.pending_fetches[&work_id].task.tag,
            consumer_tag(view + 1)
        );
        assert_eq!(
            services.fetch_tasks.last().map(BodyFetchTask::id),
            Some(work_id)
        );
        assert_eq!(
            services.fetch_tasks.last().map(|task| task.tag),
            Some(consumer_tag(view + 1))
        );
        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_work(), 1);
    }
    let same_view_tag = EventTag::new(1, 3, Generation::new(11));
    let mut timeout_upgrade = timeout_at_view(&fixture, 2);
    timeout_upgrade.groups[0].highest_prepare_qc = Some(high_prepare.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: same_view_tag,
                certificate: timeout_upgrade,
                protected_lock: Some(high_prepare.clone()),
            }],
            &mut services,
        )
        .expect("rebind the protected fetch across a same-view generation upgrade");
    let task = executor.pending_fetches[&work_id].task.clone();
    assert_eq!(task.tag, same_view_tag);
    assert_eq!(
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("complete once after repeated TC rebinding"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.ready_bodies.len(), 1);
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == same_view_tag && manifest == &fixture.manifest
    ));
    assert!(!executor.status().fail_closed);
}
#[test]
fn tc_body_rebind_uses_the_effective_local_lock_when_the_tc_omits_or_lowers_it() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    let manifest = manifest_at_view(&fixture, 1);
    let mut local_lock = fixture.qc(wire::GlobalPhase::Prepare);
    local_lock.round = manifest.round;
    local_lock.proposal_round = manifest.round;
    local_lock.subject = manifest.subject;
    let sources = certified_sources(&fixture, &local_lock);
    let consumer_tag = |view| EventTag::new(1, view, Generation::new(20 + view));
    let fetch = |view| AdapterEffect::FetchBody {
        tag: consumer_tag(view),
        round: local_lock.round,
        subject: local_lock.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(local_lock.clone()),
    };
    executor
        .consume_effects(vec![fetch(1)], &mut services)
        .expect("begin local-lock fetch");
    let work_id = services.fetch_tasks[0].id();
    let omitted = timeout_at_view(&fixture, 1);
    executor.runtime.round_tag = Some(consumer_tag(2));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: consumer_tag(2),
                certificate: omitted,
                protected_lock: Some(local_lock.clone()),
            }],
            &mut services,
        )
        .expect("an omitted TC high cannot lower the effective local lock");
    let mut lowered = timeout_at_view(&fixture, 2);
    lowered.groups[0].highest_prepare_qc = Some(fixture.qc(wire::GlobalPhase::Prepare));
    executor.runtime.round_tag = Some(consumer_tag(3));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: consumer_tag(3),
                certificate: lowered,
                protected_lock: Some(local_lock.clone()),
            }],
            &mut services,
        )
        .expect("a lower TC high cannot replace the effective local lock");
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_fetches[&work_id].task.tag, consumer_tag(3));
    assert!(services.fetch_tasks.iter().all(|task| task.id() == work_id));
    assert!(services.cancelled_fetches.is_empty());
    let task = executor.pending_fetches[&work_id].task.clone();
    assert_eq!(
        executor
            .complete_body_reconstruction(&task, manifest, fixture.body.clone(), &mut services,)
            .expect("the once-rebound local-lock work completes"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, _)]
            if *completion_tag == consumer_tag(3)
    ));
}
#[test]
fn enter_view_rejects_a_tc_high_without_an_effective_protected_lock() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let mut timeout = timeout_at_view(&fixture, 0);
    timeout.groups[0].highest_prepare_qc = Some(fixture.qc(wire::GlobalPhase::Prepare));
    executor.runtime.round_tag = Some(tag(1));
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("omitted the lock selected")
    ));
    assert!(executor.status().fail_closed);
}
#[test]
fn enter_view_rejects_a_protected_lock_with_a_conflicting_execution_commitment() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let highest = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_at_view(&fixture, 0);
    timeout.groups[0].highest_prepare_qc = Some(highest.clone());
    let mut conflicting = highest;
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"conflicting EnterView parent state"),
        Hash::new(b"conflicting EnterView post state"),
        Hash::new(b"conflicting EnterView ordinary writes"),
        1,
        Hash::new(b"conflicting EnterView executed block"),
    );
    executor.runtime.round_tag = Some(tag(1));

    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: Some(conflicting),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("conflicts with its highest PrepareQC")
    ));
    assert!(executor.status().fail_closed);
    assert!(services.entered_views.is_empty());
}
#[test]
fn tc_body_rebind_retags_a_queued_body_available_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    let high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
    let sources = certified_sources(&fixture, &high_prepare);
    let fetch = |view| AdapterEffect::FetchBody {
        tag: consumer_tag(view),
        round: high_prepare.round,
        subject: high_prepare.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(high_prepare.clone()),
    };
    executor
        .consume_effects(vec![fetch(0)], &mut services)
        .expect("begin exact high-QC fetch");
    let task = services.fetch_tasks[0].clone();
    executor
        .complete_body_reconstruction(
            &task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("queue old-view body completion");
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.ready_bodies.len(), 1);
    for view in 0..3 {
        let mut timeout = timeout_at_view(&fixture, view);
        timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(view + 1),
                    certificate: timeout,
                    protected_lock: Some(high_prepare.clone()),
                }],
                &mut services,
            )
            .expect("rebind protected terminal completion");
        executor
            .consume_effects(vec![fetch(view + 1)], &mut services)
            .expect("new reducer incarnation adopts the ready body");
        assert_eq!(executor.ready_bodies.len(), 1);
        assert!(executor.pending_fetches.is_empty());
        assert!(services.cancelled_fetches.is_empty());
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == consumer_tag(view + 1)
                    && manifest == &fixture.manifest
        ));
    }
    assert_eq!(executor.ready_body_bytes, fixture.body.len() as u64);
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
}
#[test]
fn tc_body_rebind_retires_a_superseded_completion_and_releases_capacity() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 1, 1_048_576, 1));
    let mut services = fixture.services();
    let original = fixture.qc(wire::GlobalPhase::Prepare);
    let original_sources = certified_sources(&fixture, &original);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: EventTag::new(1, 0, Generation::new(30)),
                round: original.round,
                subject: original.subject,
                manifest: None,
                certified_sources: original_sources,
                certificate: Some(original.clone()),
            }],
            &mut services,
        )
        .expect("start original fetch");
    let original_task = services.fetch_tasks[0].clone();
    executor
        .complete_body_reconstruction(
            &original_task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("queue original BodyAvailable");
    assert_eq!(executor.runtime.completions.len(), 1);
    assert_eq!(executor.ready_bodies.len(), 1);
    let replacement_manifest = manifest_at_view(&fixture, 1);
    let mut replacement = original;
    replacement.round = replacement_manifest.round;
    replacement.proposal_round = replacement_manifest.round;
    replacement.subject = replacement_manifest.subject;
    let mut timeout = timeout_at_view(&fixture, 1);
    timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: EventTag::new(1, 2, Generation::new(32)),
                certificate: timeout,
                protected_lock: Some(replacement.clone()),
            }],
            &mut services,
        )
        .expect("supersede the old completion with a higher exact lock");
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.ready_bodies.is_empty());
    assert_eq!(executor.ready_body_bytes, 0);
    assert!(executor.body_pipeline_owners.is_empty());
    let replacement_sources = certified_sources(&fixture, &replacement);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: EventTag::new(1, 2, Generation::new(32)),
                round: replacement.round,
                subject: replacement.subject,
                manifest: None,
                certified_sources: replacement_sources,
                certificate: Some(replacement),
            }],
            &mut services,
        )
        .expect("the replacement claims the released one-item work capacity");
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_work(), 1);
}
#[test]
fn tc_retires_unprotected_retryable_body_token_before_the_next_fetch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare_a = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(&fixture, &prepare_a),
                certificate: Some(prepare_a),
            }],
            &mut services,
        )
        .expect("begin the fetch later superseded by a different lock");
    let task_a = services.fetch_tasks[0].clone();
    let request_hash_a = HashOf::new(
        task_a
            .certified_request()
            .expect("the first Fetch owns its signed certified request"),
    );
    let response_a = signed_certified_response(
        &fixture,
        &task_a,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let responder = fixture.context.roster[0].validator.clone();
    assert!(matches!(
        executor.probe_certified_response_priority(&response_a, &responder),
        Ok(CertifiedResponsePriorityProbe::PreflightRequired(_))
    ));
    let retryable = executor
        .runtime
        .reserve_body_available_with_owner(task_a.tag, fixture.manifest.clone(), task_a.ownership())
        .expect("reserve A's unpublished BodyAvailable completion");
    assert!(retryable.owns_new_slot());
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(
        executor.certified_work.get(&request_hash_a),
        Some(&task_a.id())
    );
    assert_eq!(executor.runtime.reserved_body_available, Some(retryable));
    assert!(executor.runtime.completions.is_empty());

    let timeout = timeout_at_view(&fixture, 0);
    executor.runtime.round_tag = Some(tag(1));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("the TC retires the unprotected stale fetch and its token");
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(services.cancelled_fetches, vec![task_a.id()]);
    assert!(
        executor
            .body_ownership_projection()
            .runtime_body_reservation
            .is_none(),
        "retiring the fetch must release its unpublished Completion owner",
    );
    assert!(executor.runtime.completions.is_empty());
    assert!(matches!(
        executor.probe_certified_response_priority(&response_a, &responder),
        Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
            CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
        )) if request_hash == request_hash_a
    ));

    let (subject_b, body_b) = distinct_body(&fixture);
    let manifest_b = canonical_payload_manifest(
        &fixture.context,
        round(&fixture.context, 1),
        subject_b,
        &body_b,
    );
    let mut prepare_b = fixture.qc(wire::GlobalPhase::Prepare);
    prepare_b.round = manifest_b.round;
    prepare_b.proposal_round = manifest_b.round;
    prepare_b.subject = manifest_b.subject;
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: manifest_b.round,
                subject: manifest_b.subject,
                manifest: Some(manifest_b.clone()),
                certified_sources: certified_sources(&fixture, &prepare_b),
                certificate: Some(prepare_b),
            }],
            &mut services,
        )
        .expect("the successor body acquires the released pipeline");
    let task_b = services
        .fetch_tasks
        .last()
        .expect("replacement fetch")
        .clone();
    assert_eq!(
        executor
            .complete_body_reconstruction(&task_b, manifest_b.clone(), body_b, &mut services)
            .expect("the next body publishes after stale-token retirement"),
        CompletionDisposition::Accepted,
    );
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == tag(1) && manifest == &manifest_b
    ));
    assert!(!executor.output_guard.restart_required());
    assert!(!executor.status().fail_closed);
}
#[test]
fn serialized_runtime_rebinds_busy_deferred_body_completion_before_service() {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS validator key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = wire::HeightContext {
        network_id: crate::sumeragi::synthetic_network_id("serialized-body-rebind-test"),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"serialized rebind nexus context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: wire::MAX_DA_CHUNK_SIZE_BYTES,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: u64::from(wire::MAX_DA_CHUNK_SIZE_BYTES),
            max_chunk_count: 2,
        },
        leader_seed: [0x44; 32],
    };
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified =
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
    let directory = TempDir::new().expect("temporary runtime directory");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        directory.path().join("serialized-rebind-safety.wal"),
        verified,
        None,
        Generation::new(1),
        [0x55; 32],
        AdapterFingerprints {
            node: Hash::new(b"serialized rebind node"),
            build: Hash::new(b"serialized rebind build"),
            config: Hash::new(b"serialized rebind config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        3_000,
        0,
    );
    let block_signature = SignatureOf::try_from_hash(keys[0].private_key(), header.hash())
        .expect("canonical body signature");
    let block = SignedBlock::presigned(BlockSignature::new(0, block_signature), header, Vec::new());
    let body = block.encode_wire().expect("canonical SignedBlockWire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    let manifest = canonical_payload_manifest(&context, round, subject, &body);
    let execution_commitment = fixture_execution_commitment();
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
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
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate PrepareQC"),
    };
    let signed_timeout = |signer: wire::ValidatorIndex| {
        let mut vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
    };
    for signer in 0_u32..2 {
        let authenticated = adapter
            .authenticate(signed_timeout(signer))
            .expect("authenticate timeout vote");
        adapter
            .receive_authenticated(authenticated)
            .expect("admit timeout share before quorum");
    }
    let original_tag = adapter.current_tag();
    adapter
        .defer_body_available_for_test(original_tag, &manifest)
        .expect("stage Busy-deferred body completion");
    let authenticated = adapter
        .authenticate(signed_timeout(2))
        .expect("authenticate quorum timeout vote");
    let final_effects = adapter
        .receive_authenticated(authenticated)
        .expect("form and install TC before draining the old completion")
        .into_effects();
    let rebound_tag = final_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_lock: Some(protected),
                ..
            } if protected == &prepare => Some(*tag),
            _ => None,
        })
        .expect("effective-lock EnterView effect");
    let started = Instant::now();
    let (runtime, startup_effects) = SerializedV2Runtime::new(
        adapter,
        final_effects.clone(),
        started,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("serialized production runtime");
    assert_eq!(startup_effects, final_effects);
    let mut executor = V2EffectExecutor::with_runtime(
        runtime,
        BTreeMap::new(),
        context,
        PeerId::new(keys[3].public_key().clone()),
        None,
        EffectQueueConfig::default(),
    )
    .expect("serialized production executor");
    executor.ready_body_bytes = u64::try_from(body.len()).expect("body length");
    executor.ready_bodies.insert(
        (round, subject),
        ReadyBody {
            manifest: manifest.clone(),
            bytes: body.into(),
        },
    );
    executor.body_pipeline_owners.insert(
        (round, subject),
        BodyPipelineOwner {
            tag: original_tag,
            manifest_hash: Some(HashOf::new(&manifest)),
        },
    );
    let mut services = FakeServices::default();
    executor
        .consume_effects(final_effects, &mut services)
        .expect("executor rebinds the deferred completion before later service");
    assert!(services.fetch_tasks.is_empty());
    assert_eq!(
        executor.body_pipeline_owners[&(round, subject)].tag,
        rebound_tag
    );
    executor
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            started,
        )
        .expect("arm clocks after startup effects");
    assert!(matches!(
        executor
            .step(started + Duration::from_secs(2), &mut services)
            .expect("periodic service drains the rebound completion"),
        EffectExecutorStep::Advanced { .. }
    ));
    assert_eq!(services.store_tasks.len(), 1);
    assert_eq!(services.store_tasks[0].tag(), rebound_tag);
    assert_eq!(services.store_tasks[0].manifest(), &manifest);
    assert!(!executor.status().fail_closed);
}
