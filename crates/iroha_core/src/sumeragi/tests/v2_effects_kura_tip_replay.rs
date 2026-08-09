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
        staged.stage = staged
            .transition_for_effect(effect)
            .expect("exact recovery stage transition");
    }
    assert_eq!(
        staged.stage(),
        PendingKuraApplyRecoveryStage::ApplicationDispatched
    );

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
    apply_stage.stage = PendingKuraApplyRecoveryStage::Apply;
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
