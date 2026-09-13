// Recovered signature FIFO tests share the canonical adapter test namespace.
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn recovered_signature_fifo_uses_latest_exact_owner_before_terminal_wal_frame() {
    let directory = TempDir::new().expect("temporary Proposal FIFO WAL");
    let (context, keys, proofs) = authenticated_context();
    let round_zero = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let round_one = wire::ConsensusRound {
        view: 1,
        ..round_zero
    };
    let local = context.leader(round_one.view);
    let subject = subject(0xC6);
    let commitment = execution_commitment(0xC6);
    let mut old_prepare = wire::QuorumCertificate {
        round: round_zero,
        proposal_round: round_zero,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut old_prepare, &keys);
    let timeout = authenticated_timeout_certificate(
        round_zero,
        Some(old_prepare.clone()),
        vec![0, 1, 2],
        &keys,
    );
    let chunks = wire::encode_payload_chunks(context.da_layout, b"recovered signature fifo")
        .expect("encode FIFO proposal payload");
    let manifest = wire::PayloadManifest::derive(
        &context,
        round_one,
        subject,
        u64::try_from(b"recovered signature fifo".len()).expect("small FIFO payload"),
        &chunks,
    )
    .expect("derive FIFO proposal manifest");
    let proposal = wire::Proposal {
        round: round_one,
        proposer: local,
        subject,
        manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout.clone(),
            highest_prepare_qc: Some(old_prepare.clone()),
        }),
        signature: Vec::new(),
    };
    let prepare_vote = wire::Vote {
        round: round_one,
        proposal_round: round_one,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let mut current_prepare = wire::QuorumCertificate {
        round: round_one,
        proposal_round: round_one,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut current_prepare, &keys);
    let current_commit = wire::Vote {
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let old_commit = wire::Vote {
        round: round_zero,
        proposal_round: round_zero,
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xC6; 32],
        vec![
            WalRecordV2::LockAndCommit {
                prepare: old_prepare,
                vote: old_commit,
            },
            WalRecordV2::InstallTimeout(timeout),
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare_vote.clone()),
            WalRecordV2::PrepareIntent(prepare_vote.clone()),
            WalRecordV2::LockAndCommit {
                prepare: current_prepare.clone(),
                vote: current_commit.clone(),
            },
            WalRecordV2::LockAndCommit {
                prepare: current_prepare.clone(),
                vote: current_commit.clone(),
            },
        ],
    );
    assert!(matches!(
        startup.effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(observed),
            ..
        }] if observed == &proposal
    ));
    assert_eq!(startup.adapter.reducer.queued_signatures().count(), 2);
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate FIFO Proposal owner: {error}"));
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated.authority else {
        panic!("the current FIFO head must be the Proposal control Sign")
    };
    let frames = authenticated.adapter.wal.recovered_records();
    assert!(control.wal_identity.exactly_matches_record(&frames[3]));
    assert!(
        !control
            .wal_identity
            .exactly_matches_record(frames.last().expect("terminal FIFO frame"))
    );
    let AuthenticatedRecoveredAdapterStartup {
        mut adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    drop(authority);
    let tag = adapter.current_tag();
    let mut after_proposal = adapter
        .signature_completed(tag, vec![0xA1; 96])
        .expect("complete recovered Proposal")
        .into_effects();
    let prepare_sign = take_current_sign(&mut after_proposal);
    assert!(matches!(
        prepare_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(ref vote),
            ..
        } if vote == &prepare_vote
    ));
    let mut current = vec![prepare_sign];
    let prepare_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate current recovered Prepare")
        .expect("Prepare has one WAL owner");
    assert!(current.is_empty());
    assert!(prepare_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[5]));
    let prepare_tag = prepare_owner.tag();
    drop(prepare_owner);
    let mut after_prepare = adapter
        .signature_completed(prepare_tag, vec![0xA2; 96])
        .expect("complete recovered Prepare")
        .into_effects();
    let commit_sign = take_current_sign(&mut after_prepare);
    assert!(matches!(
        commit_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(ref vote),
            ..
        } if vote == &current_commit
    ));
    let mut current = vec![commit_sign];
    let commit_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate current recovered Commit")
        .expect("Commit has one WAL owner");
    assert!(current.is_empty());
    assert!(commit_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[7]));
    assert_eq!(commit_owner.prepare_certificate(), Some(&current_prepare));
}
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn recovered_current_timeout_then_historical_commit_keeps_intrinsic_vote_round() {
    let directory = TempDir::new().expect("temporary Timeout/Commit FIFO WAL");
    let (context, keys, proofs) = authenticated_context();
    let locked_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let current_round = wire::ConsensusRound {
        view: 1,
        ..locked_round
    };
    let local = 0;
    let subject = subject(0xC7);
    let commitment = execution_commitment(0xC7);
    let mut locked_prepare = wire::QuorumCertificate {
        round: locked_round,
        proposal_round: locked_round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut locked_prepare, &keys);
    let historical_commit = wire::Vote {
        round: locked_round,
        proposal_round: locked_round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let installed_timeout =
        authenticated_timeout_certificate(locked_round, None, vec![0, 1, 2], &keys);
    let current_timeout = wire::TimeoutVote {
        round: current_round,
        highest_prepare_qc: Some(locked_prepare.clone()),
        signer: local,
        signature: Vec::new(),
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xC7; 32],
        vec![
            WalRecordV2::LockAndCommit {
                prepare: locked_prepare.clone(),
                vote: historical_commit.clone(),
            },
            WalRecordV2::InstallTimeout(installed_timeout),
            WalRecordV2::TimeoutIntent(current_timeout.clone()),
        ],
    );
    assert!(matches!(
        startup.effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(observed),
            ..
        }] if observed == &current_timeout
    ));
    assert_eq!(startup.adapter.reducer.queued_signatures().count(), 1);
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate current Timeout owner: {error}"));
    let recovery_authority = authenticated
        .leader_wire_recovery_authority()
        .expect("replay projects the exact durable lock into leader-wire recovery");
    let origin = context.roster[1].validator.clone();
    let phase = super::super::FairV2IngressLeaderWirePhase::CommitVote;
    let protected_commit = super::super::FairV2IngressLeaderWireToken {
        identity: super::super::FairV2IngressLeaderWireIdentity {
            context_id: context.id(),
            height: context.height,
            view: locked_round.view,
            subject_hash: Hash::new(subject.encode()),
            manifest_hash: None,
            phase,
            semantic_origin: origin.clone(),
            canonical_wire_hash: Hash::new(b"replayed historical Commit vote"),
            vote_statement_hash: Some(super::leader_wire_vote_statement_hash(
                locked_round,
                subject,
                &historical_commit.execution_commitment,
            )),
            timeout_prepare_view: None,
        },
        slot: super::super::FairV2IngressLeaderWireSlot {
            semantic_origin: origin,
            phase,
            chunk_index: None,
        },
        admission_ordinal: 1,
        scheduler_ordinal: 1,
        source_class: super::super::FairV2IngressLeaderWireSourceClass::Control,
    };
    assert!(
        !recovery_authority.retires(&protected_commit),
        "startup recovery must preserve peer votes for its replayed durable lock"
    );
    let mut wrong_subject_commit = protected_commit.clone();
    wrong_subject_commit.identity.subject_hash = Hash::new(b"wrong replayed Commit subject");
    assert!(recovery_authority.retires(&wrong_subject_commit));
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated.authority else {
        panic!("the current FIFO head must be the Timeout control Sign")
    };
    assert!(
        control.wal_identity.exactly_matches_record(
            authenticated
                .adapter
                .wal
                .recovered_records()
                .last()
                .expect("final TimeoutIntent frame")
        )
    );
    let AuthenticatedRecoveredAdapterStartup {
        mut adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    drop(authority);
    let tag = adapter.current_tag();
    let mut after_timeout = adapter
        .signature_completed(tag, vec![0xB1; 96])
        .expect("complete current recovered Timeout")
        .into_effects();
    let commit_sign = take_current_sign(&mut after_timeout);
    assert!(matches!(
        commit_sign,
        AdapterEffect::Sign {
            tag: commit_tag,
            request: SignRequest::Vote(ref vote),
        } if commit_tag.view() == current_round.view
            && vote == &historical_commit
            && vote.round == locked_round
    ));
    let mut current = vec![commit_sign];
    let commit_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate historical recovered Commit")
        .expect("historical Commit has one WAL owner");
    assert!(current.is_empty());
    assert!(commit_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[0]));
    assert_eq!(commit_owner.tag().view(), current_round.view);
    assert_eq!(commit_owner.vote().round, locked_round);
    assert_eq!(commit_owner.prepare_certificate(), Some(&locked_prepare));
}
