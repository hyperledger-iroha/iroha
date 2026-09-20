// Included in the adapter tests; these checks retain real successor WAL custody.

#[cfg(feature = "bls")]
fn complete_tip_decision_authority_adapter(
    path: PathBuf,
    verified: &VerifiedHeightContext,
    certificate: Option<wire::QuorumCertificate>,
) -> SumeragiV2Adapter {
    let open = |path: PathBuf| {
        SumeragiV2Adapter::open_with_aggregator_and_publication(
            path,
            verified.clone(),
            Some(0),
            reducer::Generation::new(73),
            [0xE8; 32],
            fingerprints(),
            Box::new(TestAggregator),
            false,
            deferred_admission_ordinals(),
        )
        .expect("open exact CompleteTip successor WAL")
    };
    let (mut writer, initial) = open(path.clone());
    assert!(initial.is_empty());
    if let Some(certificate) = certificate {
        let payload = WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: 1,
            record: WalRecordV2::Decision(certificate),
        }
        .encode();
        let receipt = writer
            .wal
            .append(&payload)
            .expect("fsync successor Decision");
        assert_eq!(receipt.sequence(), 0);
    }
    drop(writer);
    let (adapter, _recovered_effects) = open(path);
    adapter
}

#[cfg(feature = "bls")]
fn complete_tip_decision_authority_certificate(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    marker: u8,
) -> wire::QuorumCertificate {
    let context = verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: wire::BlockSubject {
            parent_block_hash: Some(
                context
                    .parent_commit_qc
                    .as_ref()
                    .unwrap()
                    .subject
                    .block_hash,
            ),
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
            payload_hash: Hash::new([marker, 2]),
        },
        execution_commitment: execution_commitment(marker),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut certificate, keys);
    verified
        .verify_quorum_certificate(&certificate)
        .expect("authenticate successor Decision");
    certificate
}

#[cfg(feature = "bls")]
fn complete_tip_decision_authority_rejects_changed_projection(
    authority: &RecoveredSuccessorDecisionActivationAuthorityV1,
    verified: &VerifiedHeightContext,
    status: &wire::SumeragiV2Status,
) {
    let parent = verified.verified_predecessor_context().unwrap();
    let parent_qc = verified.context().parent_commit_qc.as_ref().unwrap();
    let mutations: [fn(&mut wire::SumeragiV2Status); 11] = [
        |value| value.last_committed_subject = Some(subject(0xFA)),
        |value| {
            value.height_context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
                Hash::new(b"foreign activation"),
            ))
        },
        |value| value.phase = wire::SumeragiV2StatusPhase::AwaitingProposal,
        |value| value.body_state = wire::SumeragiV2BodyState::Applied,
        |value| {
            value
                .last_commit_qc
                .as_mut()
                .unwrap()
                .certificate
                .execution_commitment = execution_commitment(0xFB)
        },
        |value| value.last_commit_qc.as_mut().unwrap().certificate.subject = subject(0xFC),
        |value| value.last_commit_qc = None,
        |value| value.height += 1,
        |value| value.last_committed_height -= 1,
        |value| value.pending_persistence_id = Some(2),
        |value| value.restart_required = true,
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut changed = status.clone();
        mutate(&mut changed);
        assert!(
            !authority.authorizes(parent, parent_qc, verified.context().id(), &changed),
            "status mutation {index} cannot reuse the exact Decision authority"
        );
    }
    let mut foreign_parent = parent.clone();
    foreign_parent.height += 1;
    assert!(!authority.authorizes(&foreign_parent, parent_qc, verified.context().id(), status));
    let mut foreign_parent_qc = parent_qc.clone();
    foreign_parent_qc.execution_commitment = execution_commitment(0xFD);
    assert!(!authority.authorizes(parent, &foreign_parent_qc, verified.context().id(), status));
    assert!(!authority.authorizes(parent, parent_qc, parent.id(), status));
}

#[cfg(feature = "bls")]
#[test]
fn complete_tip_decision_activation_requires_exact_replayed_wal() {
    run_lifecycle_fixture_on_large_stack(
        "complete_tip_decision_activation_requires_exact_replayed_wal",
        complete_tip_decision_activation_requires_exact_replayed_wal_body,
    );
}

#[cfg(feature = "bls")]
fn complete_tip_decision_activation_requires_exact_replayed_wal_body() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    let (_kura, _state, verified, _storage, _signer, _retirement) =
        super::super::v2_recovery::production_genesis_complete_tip_fixture_for_test();
    let (_, keys, _) = authenticated_context();
    let directory = TempDir::new().expect("owned CompleteTip WAL tests");
    let decision = complete_tip_decision_authority_certificate(&verified, &keys, 0xE1);
    let mut adapter = complete_tip_decision_authority_adapter(
        directory.path().join("original.wal"),
        &verified,
        Some(decision.clone()),
    );
    let authority = adapter
        .recovered_successor_decision_activation_authority()
        .expect("mint from real replay")
        .expect("durable successor Decision");
    let status = adapter.status().expect("replayed Decision status");
    assert!(authority.authorizes(
        verified.verified_predecessor_context().unwrap(),
        verified.context().parent_commit_qc.as_ref().unwrap(),
        verified.context().id(),
        &status,
    ));
    complete_tip_decision_authority_rejects_changed_projection(&authority, &verified, &status);

    let mut empty = complete_tip_decision_authority_adapter(
        directory.path().join("empty.wal"),
        &verified,
        None,
    );
    let cached = empty
        .registry
        .qc_to_core(&decision, verified.context())
        .expect("cache a valid QC only");
    assert_eq!(empty.reducer.durable_state().last_id().get(), 0);
    assert!(empty.reducer.durable_state().decision().is_none());
    assert!(
        reducer::Reducer::recover(
            empty.reducer.context().clone(),
            empty.reducer.local_validator(),
            empty.reducer.generation(),
            [reducer::WalEntry::new(
                reducer::PersistenceId::new(0),
                reducer::WalRecord::Decision(cached),
            )],
        )
        .is_err(),
        "a bare Decision at persistence id zero cannot construct durable reducer authority"
    );
    assert!(
        empty
            .recovered_successor_decision_activation_authority()
            .unwrap()
            .is_none(),
        "a valid cached CommitQC cannot invent a durable Decision"
    );
    std::mem::swap(&mut adapter.wal, &mut empty.wal);
    assert!(
        adapter
            .recovered_successor_decision_activation_authority()
            .is_err(),
        "reducer Decision cannot borrow an empty WAL"
    );
    std::mem::swap(&mut adapter.wal, &mut empty.wal);

    let foreign_decision = complete_tip_decision_authority_certificate(&verified, &keys, 0xE2);
    let mut foreign = complete_tip_decision_authority_adapter(
        directory.path().join("foreign.wal"),
        &verified,
        Some(foreign_decision),
    );
    let foreign_status = foreign
        .status()
        .expect("another real Decision with the same parent");
    assert!(
        !authority.authorizes(
            verified.verified_predecessor_context().unwrap(),
            verified.context().parent_commit_qc.as_ref().unwrap(),
            verified.context().id(),
            &foreign_status,
        ),
        "a later status from a different Decision cannot borrow the original mint"
    );
    assert_eq!(
        adapter.reducer.durable_state().last_id(),
        foreign.reducer.durable_state().last_id()
    );
    std::mem::swap(&mut adapter.wal, &mut foreign.wal);
    adapter
        .authenticate_recovered_wal_frontier()
        .expect("same-length foreign WAL has valid tail");
    assert!(
        matches!(
            adapter.recovered_successor_decision_activation_authority(),
            Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch)
        ),
        "authenticated tail alone cannot replace the exact Decision frame"
    );
}

#[cfg(feature = "bls")]
#[test]
fn complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state() {
    run_lifecycle_fixture_on_large_stack(
        "complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state",
        complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state_body,
    );
}

#[cfg(feature = "bls")]
fn complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state_body() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    let (_kura, _state, verified, _storage, _signer, _retirement) =
        super::super::v2_recovery::production_genesis_complete_tip_fixture_for_test();
    let (_, keys, _) = authenticated_context();
    let directory = TempDir::new().expect("owned CompleteTip state tests");
    let decision = complete_tip_decision_authority_certificate(&verified, &keys, 0xE3);
    let mut adapter = complete_tip_decision_authority_adapter(
        directory.path().join("state.wal"),
        &verified,
        Some(decision),
    );
    adapter.replay_complete = false;
    assert!(matches!(
        adapter.recovered_successor_decision_activation_authority(),
        Err(AdapterError::ReplayNotComplete)
    ));
    adapter.replay_complete = true;
    adapter.fail_closed = true;
    assert!(matches!(
        adapter.recovered_successor_decision_activation_authority(),
        Err(AdapterError::FailClosed)
    ));
    adapter.fail_closed = false;
    adapter.pending_persistence_id = Some(2);
    assert!(matches!(
        adapter.recovered_successor_decision_activation_authority(),
        Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch)
    ));
    adapter.pending_persistence_id = None;
    assert!(
        adapter
            .recovered_successor_decision_activation_authority()
            .unwrap()
            .is_some()
    );

    let decided = adapter.reducer.durable_state().decision().unwrap().clone();
    let round = decided.proposal_round();
    let subject = decided.subject();
    let tag = adapter.reducer.current_tag();
    for event in [
        reducer::Event::BodyAvailable {
            tag,
            round,
            subject,
        },
        reducer::Event::BodyStored {
            tag,
            round,
            subject,
        },
        reducer::Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        },
        reducer::Event::ApplicationCompleted { tag, subject },
    ] {
        let outcome = adapter
            .reducer
            .step(event)
            .expect("advance trusted local body completion");
        assert_eq!(outcome.disposition(), reducer::StepDisposition::Applied);
    }
    assert_eq!(adapter.reducer.applied_subject(), Some(subject));
    assert!(
        matches!(
            adapter.recovered_successor_decision_activation_authority(),
            Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch)
        ),
        "already applied successor cannot borrow pending-Decision activation"
    );
}

#[cfg(feature = "bls")]
#[test]
fn complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache() {
    run_lifecycle_fixture_on_large_stack(
        "complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache",
        complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache_body,
    );
}

#[cfg(feature = "bls")]
fn complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache_body() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    let (_kura, _state, verified, _storage, _signer, _retirement) =
        super::super::v2_recovery::production_genesis_complete_tip_fixture_for_test();
    let (_, keys, _) = authenticated_context();
    let directory = TempDir::new().expect("owned CompleteTip exact-quorum tests");
    let decision = complete_tip_decision_authority_certificate(&verified, &keys, 0xE4);
    let mut alternate = decision.clone();
    alternate.signers = vec![1, 2, 3];
    authenticate_qc(&mut alternate, &keys);
    verified
        .verify_quorum_certificate(&alternate)
        .expect("alternate real quorum");
    assert_eq!(decision.as_ref(), alternate.as_ref());
    assert_ne!(decision, alternate);
    let mut adapter = complete_tip_decision_authority_adapter(
        directory.path().join("original.wal"),
        &verified,
        Some(decision),
    );
    adapter
        .registry
        .qc_to_core(&alternate, verified.context())
        .expect("cache alternate quorum");
    assert!(
        adapter
            .recovered_successor_decision_activation_authority()
            .unwrap()
            .is_some(),
        "reference-cache replacement cannot substitute for original replayed QC"
    );
    let mut other = complete_tip_decision_authority_adapter(
        directory.path().join("alternate.wal"),
        &verified,
        Some(alternate.clone()),
    );
    std::mem::swap(&mut adapter.wal, &mut other.wal);
    adapter
        .registry
        .qc_to_core(&alternate, verified.context())
        .expect("reinstall alternate cache");
    adapter
        .authenticate_recovered_wal_frontier()
        .expect("alternate QC tail is authentic");
    assert!(
        matches!(
            adapter.recovered_successor_decision_activation_authority(),
            Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch)
        ),
        "same-reference QC with different quorum cannot replace exact Decision custody"
    );
}
