#[test]
fn replay_resigns_a_durable_proposal_before_prepare() {
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, _) = open_test_as_leader(&directory).expect("open leader");
        let subject = subject(10);
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
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }

    let (adapter, startup) = open_test_as_leader(&directory).expect("replay leader");
    assert!(adapter.ingress_ready());
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}

#[test]
fn proposal_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let proposal_signature = vec![0xD1; 96];
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposed_subject = subject(0xA8);
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            proposed_subject,
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal fixture")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let sign = adapter
            .local_proposal_ready(
                adapter.current_tag(),
                proposal.manifest,
                &durable,
                &validated,
            )
            .expect("persist proposal intent before signing");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(_),
                },
            ] => *tag,
            effects => panic!("unexpected proposal sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, proposal_signature.clone())
            .expect("complete proposal signature before simulated control loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        )));
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            retained,
            "a Signed callback is not a durable candidate tombstone"
        );
        // Drop both returned controls: the WAL contains ProposalIntent and
        // PrepareIntent, while neither broadcast reached transport.
    }

    let context = context();
    let leader = context.leader(0);
    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(2),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover proposal and Prepare intents");
    let proposal_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered proposal frontier: {effects:?}"),
    };
    let retained = recovered.serviced_candidate_count_for_test();
    let replayed = recovered
        .signature_completed(proposal_tag, proposal_signature)
        .expect("new generation accepts the replay-issued proposal callback");
    assert!(replayed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    )));
    let prepare_tag = replayed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Prepare => Some(*tag),
            _ => None,
        })
        .expect("recovered proposal releases its durable Prepare signature");
    assert_eq!(recovered.serviced_candidate_count_for_test(), retained);
    let prepare_signature = vec![0xD2; 96];
    let prepared = recovered
        .signature_completed(prepare_tag, prepare_signature.clone())
        .expect("complete replayed Prepare signature");
    assert!(prepared.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
    )));
    assert_eq!(
        recovered
            .signature_completed(prepare_tag, prepare_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn vote_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let vote_signature = vec![0xE1; 96];
    let prepared_subject = subject(0xA9);
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let proposer = adapter.status().expect("status").leader;
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, prepared_subject))
            .expect("accept remote proposal");
        let (tag, manifest) = match fetch.effects() {
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
            .expect("make remote body available");
        let receipt = durable_body_receipt(&adapter, round, prepared_subject);
        adapter
            .body_stored(tag, round, prepared_subject, &receipt)
            .expect("acknowledge durable body");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign = adapter
            .validation_succeeded(tag, round, prepared_subject, &validated)
            .expect("persist Prepare intent");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                },
            ] if vote.phase == wire::GlobalPhase::Prepare => *tag,
            effects => panic!("unexpected Prepare sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, vote_signature.clone())
            .expect("complete Prepare signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }

    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Prepare intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare && vote.subject == prepared_subject => *tag,
        effects => panic!("unexpected recovered Prepare frontier: {effects:?}"),
    };
    let validation_authority = recovered
        .recovered_validation_authority(&startup)
        .expect("WAL replay mints the exact bounded validation frontier");
    assert_eq!(validation_authority.len(), 1);
    assert!(validation_authority.authorizes(
        wire::ConsensusRound {
            context_id: context().id(),
            height: context().height,
            view: 0,
        },
        prepared_subject,
    ));
    let signed = recovered
        .signature_completed(sign_tag, vote_signature.clone())
        .expect("new generation accepts the replay-issued Prepare callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
            && vote.subject == prepared_subject
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, vote_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn recovered_validation_authority_uses_locked_proposal_round() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let proposal_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let certificate_round = wire::ConsensusRound {
        view: 2,
        ..proposal_round
    };
    let timeout = |view, marker| wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            view,
            ..proposal_round
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        }],
    };
    let timeout_zero = adapter
        .registry
        .tc_to_core(&timeout(0, 0xA8), &adapter.wire_context)
        .expect("register the view-zero timeout certificate");
    let timeout_one = adapter
        .registry
        .tc_to_core(&timeout(1, 0xA9), &adapter.wire_context)
        .expect("register the view-one timeout certificate");
    let locked_subject = subject(0xAA);
    let wire_prepare = wire::QuorumCertificate {
        round: certificate_round,
        proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xAA),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAA; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the carried durable PrepareQC");
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(3),
        reducer::WalRecord::LockAndCommit {
            vote: reducer::Vote::new_with_proposal_round(
                core_context.id(),
                prepare.round(),
                prepare.proposal_round(),
                reducer::Phase::Commit,
                prepare.subject(),
                local_validator,
            ),
            prepare,
        },
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(2),
        [
            reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::InstallTimeout(timeout_zero),
            ),
            reducer::WalEntry::new(
                reducer::PersistenceId::new(2),
                reducer::WalRecord::InstallTimeout(timeout_one),
            ),
            lock_entry,
        ],
    )
    .expect("recover the carried durable lock");

    let authority = adapter
        .recovered_validation_authority(&[])
        .expect("mint the recovered lock frontier");
    assert_eq!(authority.len(), 1);
    assert!(authority.authorizes(proposal_round, locked_subject));
    assert!(!authority.authorizes(certificate_round, locked_subject));
}

#[test]
fn timeout_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let timeout_signature = vec![0xF1; 96];
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let sign = adapter
            .timeout_elapsed(adapter.current_tag())
            .expect("persist Timeout intent");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected Timeout sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, timeout_signature.clone())
            .expect("complete Timeout signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }

    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Timeout intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered Timeout frontier: {effects:?}"),
    };
    let signed = recovered
        .signature_completed(sign_tag, timeout_signature.clone())
        .expect("new generation accepts the replay-issued Timeout callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, timeout_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn deferred_adapter_replay_with_startup_effects_publishes_no_status() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            subject(10),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }

    crate::sumeragi::status::clear_v2_status();
    let context = context();
    let leader = context.leader(0);
    let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(1),
        [0x22; 32],
        fingerprints(),
        deferred_admission_ordinals(),
    )
    .expect("replay leader without publishing status");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "nonempty startup work must not publish the prepared successor"
    );
    let prepared = adapter
        .successor_activation_status()
        .expect("prepare reducer-owned activation snapshot");
    assert_eq!(prepared.height, 1);
    assert!(matches!(
        prepared.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "snapshot construction must remain separate from publication"
    );
    crate::sumeragi::status::clear_v2_status();
}
