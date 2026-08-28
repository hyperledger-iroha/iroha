#[cfg(feature = "bls")]
fn resign_recovered_replay_certificate(
    fixture: &CertifiedServeRecoveredReplayFixture,
    mut certificate: wire::QuorumCertificate,
) -> wire::QuorumCertificate {
    let preimage = wire::Vote {
        round: certificate.round,
        proposal_round: certificate.proposal_round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = certificate
        .signers
        .iter()
        .map(|signer| {
            Signature::new(
                fixture.keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate recovered replay certificate");
    certificate
}

#[cfg(feature = "bls")]
#[test]
fn protected_lock_validate_reseed_enters_local_admission_without_local_handoff() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let context = fixture.verified.context();
    let certificate = fixture.authenticated.request().certificate.clone();
    let round = certificate.round;
    let subject = certificate.subject;
    let tag = EventTag::new(
        round.height,
        round.view.saturating_add(1),
        Generation::new(12),
    );
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate.clone()),
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 0xDB)],
    )
    .expect("bind the authenticated Prepare owner")
    .pop()
    .expect("one authenticated Prepare owner");
    let validate_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("inherit the full Prepare statement at Validate");
    let receipt =
        DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&fixture.manifest));
    let prepared = super::super::work_registry::PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_lock_validate(
        validate_effect.clone(),
        validate_ownership.clone(),
        fixture.manifest.clone(),
        receipt,
        certificate,
    )
    .expect("the exact protected-lock body reseals one Validate owner");
    let pending = prepared.into_pending_durable_validate_admission();
    assert!(pending.exactly_retains_for_test(&validate_effect, false));
    assert!(pending.exactly_matches_retry(&validate_effect, &validate_ownership));
    assert!(
        !pending.projects_local_proposal_handoff_for_test(),
        "protected-lock replay must not become a local proposal producer",
    );
    assert!(
        pending
            .prepare(replay_context(round), &fixture.verified)
            .is_ok(),
        "lifecycle admission reauthenticates the retained full PrepareQC",
    );
}

#[cfg(feature = "bls")]
#[test]
fn protected_lock_validate_reseed_rejects_a_foreign_valid_prepare_qc() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let context = fixture.verified.context();
    let certificate = fixture.authenticated.request().certificate.clone();
    let round = certificate.round;
    let subject = certificate.subject;
    let tag = EventTag::new(
        round.height,
        round.view.saturating_add(1),
        Generation::new(13),
    );
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate.clone()),
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 0xDC)],
    )
    .expect("bind the canonical Prepare owner")
    .pop()
    .expect("one canonical Prepare owner");
    let validate_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("inherit the canonical Prepare statement at Validate");
    let receipt =
        DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&fixture.manifest));

    let foreign_subject = self::subject(0xDE);
    let mut foreign = certificate.clone();
    foreign.subject = foreign_subject;
    let foreign = resign_recovered_replay_certificate(&fixture, foreign);
    assert!(fixture.verified.verify_quorum_certificate(&foreign).is_ok());
    assert!(
        super::super::work_registry::PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_lock_validate(
            validate_effect.clone(),
            validate_ownership.clone(),
            fixture.manifest.clone(),
            receipt.clone(),
            foreign,
        )
        .is_err(),
        "a valid QC for another subject cannot reseal this durable body",
    );
    assert!(
        super::super::work_registry::PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_lock_validate(
            validate_effect,
            validate_ownership,
            fixture.manifest,
            receipt,
            certificate,
        )
        .is_ok(),
        "the rejected foreign QC leaves the canonical reseed available",
    );
}

#[cfg(feature = "bls")]
#[test]
fn protected_decision_validate_reseed_enters_local_admission_without_local_handoff() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let context = fixture.verified.context();
    let mut certificate = fixture.authenticated.request().certificate.clone();
    certificate.phase = wire::GlobalPhase::Commit;
    let certificate = resign_recovered_replay_certificate(&fixture, certificate);
    assert!(
        fixture
            .verified
            .verify_quorum_certificate(&certificate)
            .is_ok()
    );

    let round = certificate.proposal_round;
    let subject = certificate.subject;
    let tag = EventTag::new(
        round.height,
        round.view.saturating_add(1),
        Generation::new(14),
    );
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate.clone()),
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 0xDD)],
    )
    .expect("bind the authenticated Commit owner")
    .pop()
    .expect("one authenticated Commit owner");
    let validate_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("inherit the full Commit statement at Validate");
    let receipt =
        DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&fixture.manifest));
    let validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
        receipt.clone(),
        certificate.execution_commitment,
    );
    let prepared = super::super::work_registry::PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_decision_validate(
        validate_effect.clone(),
        validate_ownership.clone(),
        fixture.manifest,
        receipt,
        &validated_receipt,
        certificate,
    )
    .expect("the exact protected-Decision body reseals one Validate owner");
    let pending = prepared.into_pending_durable_validate_admission();
    assert!(pending.exactly_retains_for_test(&validate_effect, false));
    assert!(pending.exactly_matches_retry(&validate_effect, &validate_ownership));
    assert!(
        !pending.projects_local_proposal_handoff_for_test(),
        "protected-Decision replay must not become a local proposal producer",
    );
    assert!(
        pending
            .prepare(replay_context(round), &fixture.verified)
            .is_ok(),
        "lifecycle admission reauthenticates the retained full CommitQC",
    );
}
