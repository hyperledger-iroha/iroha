#[cfg(feature = "bls")]
#[test]
fn recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources() {
    let exact_directory = TempDir::new().expect("temporary exact Decision Fetch WAL");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let decision_subject = subject(0xC8);
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: decision_subject,
        execution_commitment: execution_commitment(0xC8),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let startup = write_and_reopen_authenticated_wal_startup(
        &exact_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let exact_effect = startup
        .effects
        .first()
        .expect("Decision replays one exact Fetch")
        .clone();
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate exact Decision Fetch: {error}"));
    assert!(authenticated.effects.is_empty());
    assert!(matches!(
        authenticated.authority,
        RecoveredWalStartupAuthorityV1::DecisionFetch(_)
    ));
    let pending_directory = TempDir::new().expect("temporary pending Kura Decision Fetch WAL");
    let expected_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height,
        decision.subject.block_hash,
    );
    let pending = write_and_reopen_authenticated_wal_startup(
        &pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    )
    .bind_pending_kura_apply(expected_pending)
    .unwrap_or_else(|(error, _)| panic!("bind exact pending Kura tip: {error}"))
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|error| panic!("authenticate pending Kura Decision Fetch: {error}"));
    assert!(pending.retains_decision_fetch_for_test());
    assert_eq!(pending.expected_for_test(), expected_pending);

    let mismatched_pending_directory =
        TempDir::new().expect("temporary mismatched pending Kura Decision Fetch WAL");
    let mismatched_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign pending Kura block")),
    );
    let Err(mismatched) = write_and_reopen_authenticated_wal_startup(
        &mismatched_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    )
    .bind_pending_kura_apply(mismatched_pending)
    .unwrap_or_else(|(error, _)| panic!("bind same-height pending Kura tip: {error}"))
    .authenticate_final_wal_startup_authority() else {
        panic!("a same-height foreign Kura block must fail before owner launch")
    };
    assert!(matches!(
        mismatched,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));
    drop(pending);

    let direct_pending_directory =
        TempDir::new().expect("temporary direct pending Kura Decision Apply WAL");
    let direct_storage = TempDir::new().expect("temporary direct pending Kura stores");
    let (direct_startup, body_store) = write_decision_startup_with_body_marker(
        &direct_pending_directory,
        &direct_storage.path().join("body"),
        0xCA,
        DecisionBodyMarkerFixture::Validated,
    );
    let direct_expected = match direct_startup.effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                ..
            },
        ] => crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
            round.context_id,
            tag.height(),
            subject.block_hash,
        ),
        _ => panic!("validated Decision must replay one exact Fetch"),
    };
    let direct_pending = direct_startup
        .bind_pending_kura_apply(direct_expected)
        .unwrap_or_else(|(error, _)| panic!("bind direct pending Kura tip: {error}"))
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|error| panic!("authenticate direct pending Kura Decision Fetch: {error}"));
    assert!(direct_pending.retains_decision_fetch_for_test());
    assert_eq!(direct_pending.expected_for_test(), direct_expected);
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal the validated pending Kura body outcome");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic direct pending Kura signer");
    let ledger_root = direct_storage.path().join("ledger");
    let serve_root = direct_storage.path().join("serve");
    let owner = direct_pending
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            body_store,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("open direct pending Kura Apply owner: {error}"));
    let (row_count, apply_ordinal) = owner
        .recovered_decision_apply_summary_for_test()
        .expect("pending Kura owner retains the exact recovered Apply carrier");
    assert_eq!(row_count, 4);
    assert!(apply_ordinal > 0);
    drop(owner);

    let empty_pending_directory =
        TempDir::new().expect("temporary pending Kura startup without a Decision");
    let empty_pending = write_and_reopen_authenticated_wal_startup(
        &empty_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        Vec::new(),
    )
    .bind_pending_kura_apply(expected_pending)
    .unwrap_or_else(|(error, _)| panic!("bind empty pending Kura startup: {error}"))
    .authenticate_final_wal_startup_authority();
    let Err(empty_error) = empty_pending else {
        panic!("pending Kura startup without a Decision Fetch must fail closed")
    };
    assert!(matches!(
        empty_error,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));

    let foreign_pending_directory =
        TempDir::new().expect("temporary foreign pending Kura Decision Fetch WAL");
    let foreign_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height + 1,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign pending Kura block")),
    );
    let foreign_startup = write_and_reopen_authenticated_wal_startup(
        &foreign_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let Err((error, retained)) = foreign_startup.bind_pending_kura_apply(foreign_pending) else {
        panic!("a foreign pending Kura height must not bind recovered startup")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));
    assert_eq!(retained.effects.len(), 1);

    let manifest_directory = TempDir::new().expect("temporary mutated-manifest WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &manifest_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let AdapterEffect::FetchBody { manifest, .. } = &mut retained.effects[0] else {
        panic!("Decision replay effect must remain FetchBody")
    };
    let chunks = wire::encode_payload_chunks(context.da_layout, b"forbidden guessed manifest")
        .expect("encode mutation payload");
    let guessed = wire::PayloadManifest::derive(
        &context,
        round,
        decision_subject,
        u64::try_from(b"forbidden guessed manifest".len()).expect("small mutation payload"),
        &chunks,
    )
    .expect("derive mutation manifest");
    *manifest = Some(guessed);
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a guessed Decision manifest must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let certificate_directory = TempDir::new().expect("temporary foreign-certificate WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &certificate_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let mut foreign_certificate = decision.clone();
    foreign_certificate.subject = subject(0xC9);
    foreign_certificate.execution_commitment = execution_commitment(0xC9);
    authenticate_qc(&mut foreign_certificate, &keys);
    let AdapterEffect::FetchBody {
        round: effect_round,
        subject: effect_subject,
        certificate,
        ..
    } = &mut retained.effects[0]
    else {
        panic!("Decision replay effect must remain FetchBody")
    };
    *effect_round = foreign_certificate.proposal_round;
    *effect_subject = foreign_certificate.subject;
    *certificate = Some(foreign_certificate);
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a foreign Decision certificate must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let sources_directory = TempDir::new().expect("temporary mutated-sources WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &sources_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    retained.effects[0] = exact_effect;
    let AdapterEffect::FetchBody {
        certified_sources, ..
    } = &mut retained.effects[0]
    else {
        panic!("Decision replay effect must remain FetchBody")
    };
    certified_sources.pop();
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a truncated Decision source roster must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let locator_directory = TempDir::new().expect("temporary foreign-locator WAL");
    let authenticated = write_and_reopen_authenticated_wal_startup(
        &locator_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision)],
    )
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("authenticate locator fixture: {error}"));
    let AuthenticatedRecoveredAdapterStartup {
        adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    let RecoveredWalStartupAuthorityV1::DecisionFetch(mut fetch) = authority else {
        panic!("locator fixture must retain Decision Fetch")
    };
    fetch.wal_identity = RecoveredWalFrameIdentity::for_test(0, 1, [0xE8; 32]);
    let verified = VerifiedHeightContext {
        context: adapter.wire_context.clone(),
        proofs_of_possession: adapter.proofs_of_possession.clone(),
        parent_verification: adapter.parent_verification.clone(),
    };
    assert!(
        crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(&verified, fetch)
            .is_err(),
        "a substituted exact-shaped WAL locator must not project Decision Fetch authority"
    );
}
#[test]
fn bls_control_classifier_rejects_action_tag_extra_and_dual_residuals_pre_store() {
    let proposal_safety = TempDir::new().expect("temporary ProposalIntent classifier WAL");
    let timeout_safety = TempDir::new().expect("temporary TimeoutIntent classifier WAL");
    persist_proposal_intent_for_control_recovery(&proposal_safety, 0xC3);
    persist_timeout_intent_for_control_recovery(&timeout_safety);
    let authenticated_proposal = open_recovered_leader_startup_test(&proposal_safety)
        .expect("open exact ProposalIntent startup for local-attempt ownership")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate exact ProposalIntent startup: {error}")
        });
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated_proposal.authority
    else {
        panic!("ProposalIntent must retain one exact control Sign")
    };
    let recovered_attempt = RecoveredLifecycleLocalProposalAttemptV1::from_control(control)
        .expect("ProposalIntent control Sign mints one opaque local-attempt owner");
    let directive = authenticated_proposal
        .adapter
        .local_proposal_directive()
        .expect("read exact recovered Proposal directive");
    assert!(recovered_attempt.exactly_matches_directive(directive));
    drop(authenticated_proposal);

    let authenticated_timeout = open_recovered_startup_test(&timeout_safety)
        .expect("open exact TimeoutIntent startup for local-attempt exclusion")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate exact TimeoutIntent startup: {error}")
        });
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated_timeout.authority
    else {
        panic!("TimeoutIntent must retain one exact control Sign")
    };
    assert!(RecoveredLifecycleLocalProposalAttemptV1::from_control(control).is_none());
    drop(authenticated_timeout);

    let mut proposal =
        open_recovered_leader_startup_test(&proposal_safety).expect("open ProposalIntent startup");
    let mut timeout =
        open_recovered_startup_test(&timeout_safety).expect("open TimeoutIntent startup");
    let proposal_effect = proposal.effects.pop().expect("one Proposal control effect");
    let timeout_effect = timeout.effects.pop().expect("one Timeout control effect");
    proposal.effects.push(timeout_effect);
    timeout.effects.push(proposal_effect);
    for swapped in [proposal, timeout] {
        let Err((error, retained)) = swapped.authenticate_final_wal_startup_authority() else {
            panic!("Proposal/Timeout owner-frame action swaps must fail")
        };
        assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
        assert_eq!(retained.effects.len(), 1);
    }
    let mut wrong_tag = open_recovered_leader_startup_test(&proposal_safety)
        .expect("reopen ProposalIntent for tag mutation");
    let AdapterEffect::Sign { tag, .. } = &mut wrong_tag.effects[0] else {
        unreachable!("ProposalIntent replays one Sign")
    };
    *tag = reducer::EventTag::new(
        tag.height(),
        tag.view(),
        reducer::Generation::new(tag.generation().get().saturating_add(1)),
    );
    let Err((error, retained)) = wrong_tag.authenticate_final_wal_startup_authority() else {
        panic!("mutated recovered control tag must fail")
    };
    assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
    assert_eq!(retained.effects.len(), 1);
    let mut extra = open_recovered_leader_startup_test(&proposal_safety)
        .expect("reopen ProposalIntent for residual mutation");
    let duplicate = extra.effects[0].clone();
    extra.effects.push(duplicate);
    let Err((error, retained)) = extra.authenticate_final_wal_startup_authority() else {
        panic!("an extra residual effect must fail before authority removal")
    };
    assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
    assert_eq!(retained.effects.len(), 2);
    let phase_safety = TempDir::new().expect("temporary phase/control exclusivity WAL");
    let (mut phase, _vote, _proposal, _manifest, _validated) =
        reopen_with_prepare_intent(&phase_safety, 0xC4);
    let control = open_recovered_startup_test(&timeout_safety)
        .expect("reopen TimeoutIntent control for dual residual")
        .effects
        .into_iter()
        .next()
        .expect("one Timeout control effect");
    phase.effects.push(control);
    let Err((error, retained)) = phase.authenticate_final_wal_startup_authority() else {
        panic!("phase-vote and control authority must be mutually exclusive")
    };
    assert!(matches!(error, AdapterError::RecoveredVoteSignAmbiguous));
    assert_eq!(retained.effects.len(), 2);
    let unopened = TempDir::new().expect("unopened classification store root");
    assert!(!unopened.path().join("ledger").exists());
    assert!(!unopened.path().join("serve").exists());
}
#[test]
fn bls_foreign_control_replay_row_is_never_repaired_or_published() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary foreign control replay WAL");
    let storage = TempDir::new().expect("temporary foreign control lifecycle stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let wire_context = context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(wire_context.id().0.as_ref());
    assert!(substitute_recovered_control_replay_authority_for_test(
        &storage.path().join("ledger"),
        LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height),
    ));
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let foreign_frame = std::fs::read(&ledger_path).expect("read foreign replay frame");
    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen foreign replay control startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("WAL authority itself remains exact: {error}"));
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS control-startup signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_from_roots_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                &storage.path().join("body"),
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                &local_signer,
            )
            .is_err(),
        "a structurally valid foreign replay row must never be repaired in place"
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread rejected foreign replay frame"),
        foreign_frame,
        "rejection performs no LedgerV1 fsync"
    );
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[test]
fn bls_same_owner_foreign_terminal_control_row_is_rejected_without_rewrite() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary same-owner control WAL");
    let storage = TempDir::new().expect("temporary same-owner lifecycle stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let wire_context = context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(wire_context.id().0.as_ref());
    assert!(append_same_owner_foreign_terminal_for_test(
        &storage.path().join("ledger"),
        LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height),
    ));
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let foreign_frame = std::fs::read(&ledger_path).expect("read same-owner foreign frame");
    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen same-owner control startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("WAL authority itself remains exact: {error}"));
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic same-owner control signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_from_roots_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                &storage.path().join("body"),
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                &local_signer,
            )
            .is_err(),
        "a foreign terminal row cannot share the standalone control owner"
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread rejected same-owner frame"),
        foreign_frame,
        "same-owner rejection performs no LedgerV1 rewrite"
    );
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[test]
fn bls_mutated_control_frame_identity_fails_before_serve_or_ledger_open() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary mutated control frame WAL");
    let storage = TempDir::new().expect("temporary unopened control stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    let mut authenticated = open_recovered_startup_test(&safety)
        .expect("open exact TimeoutIntent before identity mutation")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate exact frame first: {error}"));
    let (frame_sequence, mut foreign_hash) = {
        let frame = authenticated
            .adapter
            .wal
            .recovered_records()
            .last()
            .expect("retained TimeoutIntent frame");
        (frame.sequence(), frame.frame_hash())
    };
    foreign_hash[0] ^= 1;
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &mut authenticated.authority else {
        panic!("TimeoutIntent owns one control Sign")
    };
    control.wal_identity = RecoveredWalFrameIdentity::for_test(
        frame_sequence,
        frame_sequence
            .checked_add(1)
            .expect("fixture persistence id"),
        foreign_hash,
    );
    let body_root = storage.path().join("body");
    let body_store = super::super::v2_body_store::V2BodyStore::open(
        &body_root,
        authenticated.adapter.wire_context.clone(),
    )
    .expect("open the runner-owned empty body store");
    let body_entries_before = std::fs::read_dir(&body_root)
        .expect("read preflight body directory")
        .count();
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal the empty runner-owned body store");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS control-startup signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_with_store_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                body_store,
                &local_signer,
            )
            .is_err()
    );
    assert!(body_root.exists());
    assert_eq!(
        std::fs::read_dir(&body_root)
            .expect("reread unchanged preflight body directory")
            .count(),
        body_entries_before
    );
    assert!(!storage.path().join("ledger").exists());
    assert!(!storage.path().join("serve").exists());
    assert!(crate::sumeragi::status::v2_status().is_none());
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    recovered_wal_first_release_source_is_closed_and_store_ordered
);
