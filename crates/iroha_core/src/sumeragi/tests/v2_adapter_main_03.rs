#[test]
fn serviced_candidate_snapshot_is_bound_to_the_local_validator_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let owner_a_wal = directory.path().join("owner-a.wal");
    let owner_a_snapshot;
    {
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            &owner_a_wal,
            verified_genesis(context.clone()),
            Some(0),
            reducer::Generation::new(1),
            [0xA1; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open owner-A adapter");
        assert!(startup.is_empty());
        durably_retire_unowned_body_event(&mut adapter, 0xA1);
        owner_a_snapshot = adapter
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
    }
    let owner_b_wal = directory.path().join("owner-b.wal");
    let owner_b_snapshot = directory.path().join("owner-b.wal.serviced-candidates");
    std::fs::copy(&owner_a_snapshot, &owner_b_snapshot)
        .expect("transplant owner-A sidecar onto owner-B path");
    let mut owner_b_fingerprints = fingerprints();
    owner_b_fingerprints.node = Hash::new(b"owner-b node");
    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            owner_b_wal,
            verified_genesis(context),
            Some(1),
            reducer::Generation::new(1),
            [0xB2; 32],
            owner_b_fingerprints,
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
}
#[test]
#[allow(clippy::too_many_lines)]
fn aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let signer_subsets = [
        vec![0, 1, 2],
        vec![0, 1, 3],
        vec![0, 2, 3],
        vec![1, 2, 3],
        vec![0, 1, 2, 3],
    ];
    let marker_count = adapter.serviced_candidate_count_for_test();
    let mut qc_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC1),
            execution_commitment: execution_commitment(0xC1),
            signers: signers.clone(),
            aggregate_signature: vec![0xC0 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
        )
        .encode();
        let certificate = adapter
            .registry
            .qc_to_core(&certificate, &adapter.wire_context)
            .expect("convert valid same-reference QC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::QuorumCertificateReceived { tag, certificate },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("QC has a service identity");
        assert_eq!(
            candidate.0.class(),
            ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
            "scheduler priority is excluded from the logical key"
        );
        match qc_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "valid quorum subset and aggregate replacement is not a new QC owner"
            ),
            None => qc_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce QC carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 1,
        "all valid QC carrier variants share one transient identity"
    );
    let mut tc_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: signers.clone(),
                aggregate_signature: vec![0xD0 | marker; 96],
            }],
        };
        let carrier = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate.clone()),
        )
        .encode();
        let certificate = adapter
            .registry
            .tc_to_core(&certificate, &adapter.wire_context)
            .expect("convert valid same-reference TC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::TimeoutCertificateReceived { tag, certificate },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("TC has a service identity");
        match tc_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "valid timeout quorum subset and aggregate replacement is not a new owner"
            ),
            None => tc_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce TC carrier variant");
    }
    assert_ne!(qc_key, tc_key);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 2
    );
    let mut timeout_vote_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let highest_prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC2),
            execution_commitment: execution_commitment(0xC2),
            signers: signers.clone(),
            aggregate_signature: vec![0xE0 | marker; 96],
        };
        let vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(highest_prepare),
            signer: 0,
            signature: vec![0x70 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(
            vote.clone(),
        ))
        .encode();
        let vote = adapter
            .registry
            .timeout_vote_to_core(&vote, &adapter.wire_context)
            .expect("convert TimeoutVote with alternate high-QC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::TimeoutVoteReceived { tag, vote },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("TimeoutVote has a service identity");
        match timeout_vote_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "nested high-QC signer and signature variants are one TimeoutVote owner"
            ),
            None => timeout_vote_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce nested TimeoutVote carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 3
    );
    let proposal_round = wire::ConsensusRound { view: 1, ..round };
    let proposal_subject = subject(0xC3);
    let proposal_payload = [0xC3, 2];
    let manifest = encode_payload(
        &adapter.wire_context,
        proposal_round,
        proposal_subject,
        &proposal_payload,
    )
    .expect("encode proposal payload")
    .manifest()
    .clone();
    let mut proposal_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: signers.clone(),
                aggregate_signature: vec![0x50 | marker; 96],
            }],
        };
        let proposal = wire::Proposal {
            round: proposal_round,
            proposer: adapter.wire_context.leader(proposal_round.view),
            subject: proposal_subject,
            manifest: manifest.clone(),
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: certificate,
                highest_prepare_qc: None,
            }),
            signature: vec![0x60 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
            proposal.clone(),
        ))
        .encode();
        let proposal = adapter
            .registry
            .proposal_to_core(&proposal, &adapter.wire_context)
            .expect("convert proposal with alternate TC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::ProposalReceived { tag, proposal },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("proposal has a service identity");
        match proposal_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "nested TC and proposal-signature variants are one proposal owner"
            ),
            None => proposal_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce nested proposal carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 4
    );
    let mut vote_key = None;
    for variant in 0_u8..5 {
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC4),
            execution_commitment: execution_commitment(0xC4),
            signer: 1,
            signature: vec![0x20 | variant; 96],
        };
        let carrier =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone()))
                .encode();
        let vote = adapter
            .registry
            .vote_to_core(&vote, &adapter.wire_context)
            .expect("convert alternate vote signature carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::VoteReceived { tag, vote },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("vote has a service identity");
        match vote_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "authenticated signature replacements are one vote owner"
            ),
            None => vote_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce vote carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 5
    );
}
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
fn persistence_macro_step_budgets_have_exact_four_effect_maximum() {
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
            PersistenceMacroStepBudget::new(1, 4),
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
        "local TC formation is the four-effect persistence witness"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn exact_live_wal_cut_seals_all_six_real_persisted_continuations() {
    {
        let directory = TempDir::new().expect("temporary proposal WAL directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader adapter");
        assert!(startup.is_empty());
        let leader = adapter.wire_context.leader(0);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) =
            proposal(&adapter.wire_context, leader, subject(0xD1)).payload
        else {
            unreachable!("proposal fixture")
        };
        let (_, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let context = adapter.wire_context.clone();
        let manifest = adapter
            .registry
            .manifest_to_core(&proposal.manifest, &context)
            .expect("register local proposal manifest");
        let round = adapter
            .registry
            .round_to_core(proposal.round, &context)
            .expect("convert local proposal round");
        adapter
            .registry
            .register_execution_commitment(
                round,
                manifest.subject(),
                validated.execution_commitment(),
            )
            .expect("register local proposal execution result");
        adapter.active_subject = Some((round, manifest.subject()));
        let tag = adapter.current_tag();
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::LocalProposalReady { tag, manifest })
                .expect("stage real ProposalIntent"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::SignProposal);
    }
    {
        let directory = TempDir::new().expect("temporary Prepare WAL directory");
        let (mut adapter, startup) = open_test(&directory).expect("open Prepare adapter");
        assert!(startup.is_empty());
        let (tag, manifest, _durable, validated) =
            advance_direct_validation_fixture_to_durable(&mut adapter, 0xD2);
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        adapter
            .registry
            .register_execution_commitment(round, subject, validated.execution_commitment())
            .expect("register Prepare execution result");
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::ValidationCompleted {
                    tag,
                    round,
                    subject,
                    valid: true,
                })
                .expect("stage real PrepareIntent"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::SignPrepare);
    }
    {
        let directory = TempDir::new().expect("temporary Commit WAL directory");
        let (mut adapter, startup) = open_test(&directory).expect("open Commit adapter");
        assert!(startup.is_empty());
        let (tag, manifest, _durable, validated) =
            advance_direct_validation_fixture_to_durable(&mut adapter, 0xD3);
        let prepare = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Prepare,
            subject: manifest.subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD3; 96],
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    prepare,
                )),
            ))
            .expect("durably observe exact PrepareQC");
        assert!(observed.effects().is_empty());
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        adapter
            .registry
            .register_execution_commitment(round, subject, validated.execution_commitment())
            .expect("register Commit execution result");
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::ValidationCompleted {
                    tag,
                    round,
                    subject,
                    valid: true,
                })
                .expect("stage real LockAndCommit"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::SignCommit);
    }
    {
        let directory = TempDir::new().expect("temporary timeout WAL directory");
        let (mut adapter, startup) = open_test(&directory).expect("open timeout adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::TimeoutElapsed { tag })
                .expect("stage real TimeoutIntent"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::SignTimeout);
    }
    {
        let directory = TempDir::new().expect("temporary EnterView WAL directory");
        let (mut adapter, startup) = open_test(&directory).expect("open EnterView adapter");
        assert!(startup.is_empty());
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: adapter.current_tag().view(),
        };
        let certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xD4; 96],
            }],
        };
        let context = adapter.wire_context.clone();
        let certificate = adapter
            .registry
            .tc_to_core(&certificate, &context)
            .expect("convert exact timeout certificate");
        let tag = adapter.current_tag();
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::TimeoutCertificateReceived { tag, certificate })
                .expect("stage real InstallTimeout"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::EnterView);
    }
    {
        let directory = TempDir::new().expect("temporary Apply WAL directory");
        let (mut adapter, startup) = open_test(&directory).expect("open Apply adapter");
        assert!(startup.is_empty());
        let (tag, manifest, _durable, validated) =
            advance_direct_validation_fixture_to_durable(&mut adapter, 0xD5);
        let sign = adapter
            .validation_succeeded(tag, manifest.round, manifest.subject, &validated)
            .expect("durably validate exact Apply body");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            }] if vote.phase == wire::GlobalPhase::Prepare
        ));
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: manifest.subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD5; 96],
        };
        let context = adapter.wire_context.clone();
        let certificate = adapter
            .registry
            .qc_to_core(&decision, &context)
            .expect("convert exact CommitQC Decision");
        let persist = only_pending_persist(
            adapter
                .reducer
                .step(reducer::Event::QuorumCertificateReceived { tag, certificate })
                .expect("stage real Decision"),
        );
        drive_live_wal_fixture(&mut adapter, persist, LiveWalOwnedStage::Apply);
    }
}
#[cfg(feature = "bls")]
#[test]
fn recovered_timeout_signature_preview_is_exact_and_drop_inert() {
    let directory = TempDir::new().expect("temporary recovered Sign preview directory");
    let (mut adapter, startup) = open_test(&directory).expect("open recovered Sign adapter");
    assert!(startup.is_empty());
    let sign = adapter
        .timeout_elapsed(adapter.current_tag())
        .expect("persist exact TimeoutIntent and expose its Sign")
        .into_effects();
    let [AdapterEffect::Sign { tag, request }] = sign.as_slice() else {
        panic!("TimeoutIntent must expose exactly one timeout Sign")
    };
    assert!(matches!(request, SignRequest::TimeoutVote(_)));
    let tag = *tag;
    let request = request.clone();
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_before = adapter.wal.recovered_records().len();
    let invalid =
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request.clone(),
            vec![0xA5; 96],
            None,
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlTimeout,
        );
    assert!(
        adapter
            .prepare_recovered_lifecycle_sign_completion(invalid)
            .is_err()
    );
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_before);
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let signature = Signature::new(keys[0].private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let exact =
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request,
            signature,
            None,
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlTimeout,
        );
    let preview = adapter
        .prepare_recovered_lifecycle_sign_completion(exact)
        .expect("exact recovered timeout signature previews its successor");
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast)
    );
    assert!(matches!(
        preview.broadcast_effect(),
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
            ..
        }) if !vote.signature.is_empty()
    ));
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_before);
}
#[cfg(feature = "bls")]
#[test]
fn recovered_proposal_broadcast_and_sign_seals_exact_wal_body_and_successor() {
    let directory = TempDir::new().expect("temporary recovered FIFO directory");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let local = context.leader(round.view);
    let body_subject = subject(0xD6);
    let outbound = encode_payload(&context, round, body_subject, b"recovered proposal FIFO")
        .expect("encode recovered proposal payload");
    let manifest = outbound.manifest().clone();
    let proposal = wire::Proposal {
        round,
        proposer: local,
        subject: body_subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    let (_, validated) = validated_receipts_for_manifest(&context, &proposal.manifest);
    let body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("next-vote-body-owner"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open exact next-Vote body-store owner");
    let body_store_identity = body_store.instance_identity();
    let prepare = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: body_subject,
        execution_commitment: validated.execution_commitment(),
        signer: local,
        signature: Vec::new(),
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare.clone()),
        ],
    );
    let RecoveredAdapterStartup {
        mut adapter,
        effects,
    } = startup;
    let [AdapterEffect::Sign { tag, request }] = effects.as_slice() else {
        panic!("recovered Proposal/Prepare FIFO must expose the Proposal Sign first")
    };
    assert_eq!(request, &SignRequest::Proposal(proposal.clone()));
    let tag = *tag;
    let request = request.clone();
    let proposal_identity = adapter
        .authenticate_recovered_wal_frame(&adapter.wal.recovered_records()[0])
        .expect("authenticate ProposalIntent frame")
        .0;
    let prepare_identity = adapter
        .authenticate_recovered_wal_frame(&adapter.wal.recovered_records()[1])
        .expect("authenticate PrepareIntent frame")
        .0;
    let local_index = usize::try_from(local).expect("fixture signer index fits usize");
    let signature = Signature::new(
        keys[local_index].private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let completion =
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request,
            signature,
            Some(outbound),
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlProposal,
        );
    let mut preview = adapter
        .prepare_recovered_lifecycle_sign_completion(completion)
        .expect("preview exact recovered Proposal signature");
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign)
    );
    let broadcast = preview.broadcast_effect().clone();
    let next_sign = preview
        .next_sign_effect()
        .expect("recovered FIFO retains its Prepare Sign")
        .clone();
    assert!(matches!(
        &next_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } if vote == &prepare
    ));
    let mut late_prepare_sign = next_sign.clone();
    let AdapterEffect::Sign { tag, .. } = &mut late_prepare_sign else {
        unreachable!("fixture successor is a Vote Sign")
    };
    *tag = reducer::EventTag::new(
        round.height,
        round.view.saturating_add(1),
        reducer::Generation::new(round.height),
    );
    assert!(
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            broadcast.clone(),
            late_prepare_sign,
        )
        .is_none(),
        "a Prepare successor cannot move to a later EventTag view"
    );
    let mut signed_prepare = prepare.clone();
    signed_prepare.signature = Signature::new(
        keys[local_index].private_key(),
        &SignRequest::Vote(prepare.clone()).signature_preimage(),
    )
    .payload()
    .to_vec();
    let prepare_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(signed_prepare),
    ));
    let mut commit = prepare.clone();
    commit.phase = wire::GlobalPhase::Commit;
    let later_commit_sign = AdapterEffect::Sign {
        tag: reducer::EventTag::new(
            round.height,
            round.view.saturating_add(1),
            reducer::Generation::new(round.height),
        ),
        request: SignRequest::Vote(commit),
    };
    let proposal_broadcast = preview.broadcast.clone();
    let proposal_next_sign = preview.next_sign.clone();
    preview.broadcast = prepare_broadcast.clone();
    preview.next_sign = Some(later_commit_sign.clone());
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign)
    );
    let Some(AdapterEffect::Sign {
        request: SignRequest::Vote(malformed_commit),
        ..
    }) = preview.next_sign.as_mut()
    else {
        unreachable!("fixture successor remains a Vote Sign")
    };
    malformed_commit.subject = subject(0xD7);
    assert_eq!(
        preview.settlement_family(),
        None,
        "a mismatched combined relation has no settlement family"
    );
    preview.broadcast = proposal_broadcast;
    preview.next_sign = proposal_next_sign;
    assert!(
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            prepare_broadcast,
            later_commit_sign,
        )
        .is_some(),
        "a Commit successor may retain a later EventTag view than its vote round"
    );
    let expected_manifest_hash = Some(HashOf::new(&manifest));
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let exact_body_lookup = preview
        .project_broadcast_and_sign_body_lookup_for_test(
            body_store_identity.clone(),
            Arc::clone(&output_guard),
        )
        .expect("bind exact reducer body lookup to the preview/store owner");
    let foreign_durable =
        DurableBodyReceipt::for_test(context.id(), round, subject(0xD7), HashOf::new(&manifest));
    let foreign_body = ValidatedBodyReceipt::for_test(foreign_durable);
    assert!(
        RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
            RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash,)
                .expect("project exact reducer body lookup"),
            foreign_body.clone(),
            body_store_identity.clone(),
        )
        .is_none(),
        "a substituted validated body cannot mint exact-owner authority"
    );
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.payload_size_bytes = foreign_manifest
        .payload_size_bytes
        .checked_add(1)
        .expect("fixture payload length has headroom");
    let foreign_manifest_durable = DurableBodyReceipt::for_test(
        context.id(),
        round,
        body_subject,
        HashOf::new(&foreign_manifest),
    );
    let foreign_manifest_body = ValidatedBodyReceipt::for_test_with_commitment(
        foreign_manifest_durable,
        validated.execution_commitment(),
    );
    assert!(
        RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
            RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash,)
                .expect("project exact reducer body lookup"),
            foreign_manifest_body,
            body_store_identity.clone(),
        )
        .is_none(),
        "same-coordinate foreign manifest cannot mint exact-owner authority"
    );
    let mut substituted_sign = next_sign.clone();
    let AdapterEffect::Sign {
        request: SignRequest::Vote(vote),
        ..
    } = &mut substituted_sign
    else {
        unreachable!("fixture successor is a Vote Sign")
    };
    vote.subject = subject(0xD8);
    let substituted_sign_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash)
            .expect("project exact reducer body lookup"),
        validated.clone(),
        body_store_identity.clone(),
    )
    .expect("exact body mints opaque test authority");
    assert!(
        preview
            .project_broadcast_and_substituted_sign_for_test(
                &substituted_sign,
                substituted_sign_body,
            )
            .is_err(),
        "a substituted next Sign cannot consume exact body authority"
    );
    let dispatch_key = preview.dispatch_key();
    let exact_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        exact_body_lookup,
        validated.clone(),
        body_store_identity.clone(),
    )
    .expect("exact body mints opaque test authority");
    assert!(exact_body.exactly_matches_for_test(&validated, &body_store_identity));
    let combined = preview
        .project_broadcast_and_sign_authority(exact_body)
        .expect("seal exact recovered Broadcast and next Sign");
    let duplicate_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash)
            .expect("project exact reducer body lookup"),
        validated.clone(),
        body_store_identity,
    )
    .expect("exact body mints a separate opaque test authority");
    assert!(
        preview
            .project_broadcast_and_sign_authority(duplicate_body)
            .is_err(),
        "one preview cannot duplicate its combined successor authority"
    );
    assert!(combined.exactly_matches_for_test(
        dispatch_key,
        &broadcast,
        prepare_identity,
        &next_sign,
        &validated,
    ));
    let mut substituted_broadcast = broadcast.clone();
    let AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
        ..
    }) = &mut substituted_broadcast
    else {
        unreachable!("fixture Broadcast is the signed Proposal")
    };
    proposal.signature.push(0xD9);
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &substituted_broadcast,
            prepare_identity,
            &next_sign,
            &validated,
        ),
        "the combined authority retains the exact signed Broadcast"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            proposal_identity,
            &next_sign,
            &validated,
        ),
        "a different authenticated WAL frame cannot own the next Sign"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            prepare_identity,
            &substituted_sign,
            &validated,
        ),
        "the combined authority retains the exact next Sign"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            prepare_identity,
            &next_sign,
            &foreign_body,
        ),
        "the combined authority retains the exact validated body"
    );
    let cold_adapter = RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
        broadcast,
        next_sign,
    )
    .expect("the exact durable pair mints one cold-adapter authority");
    drop(combined);
    drop(preview);
    let verified = VerifiedHeightContext::genesis(context, proofs)
        .expect("reverify the exact recovered Proposal context");
    let confirmed = ProductionLifecycleAdapterStartupV1::recovered(adapter, Vec::new())
        .advance_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_adapter)
        .expect("cold adapter replays the exact fsynced Broadcast-and-Sign pair");
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        adapter: confirmed,
        effects: confirmed_effects,
        leader_wire_launch_prepared: false,
        ..
    } = confirmed.state
    else {
        panic!("confirmed production startup remains in the recovered state")
    };
    assert!(confirmed_effects.is_empty());
    let Some(reducer::SignableMessage::Vote(awaiting)) = confirmed.reducer.awaiting_signature()
    else {
        panic!("confirmed adapter must await the exact next Vote Sign")
    };
    assert_eq!(
        confirmed
            .registry
            .unsigned_vote_to_wire(*awaiting)
            .expect("reconstruct confirmed next Vote"),
        prepare
    );
}
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn production_recovered_proposal_sign_joins_exact_next_vote_body_store() {
    let directory = TempDir::new().expect("temporary production recovered-Sign directory");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let local = context.leader(round.view);
    let local_index = usize::try_from(local).expect("fixture signer index fits usize");
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("fixture height is non-zero"),
        None,
        None,
        None,
        8_214,
        round.view,
    );
    let block_signature =
        SignatureOf::try_from_hash(keys[local_index].private_key(), header.hash())
            .expect("sign exact recovered-Sign body");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(local), block_signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block
        .encode_wire()
        .expect("encode exact recovered-Sign body");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let outbound = encode_payload(&context, round, subject, &canonical_wire)
        .expect("encode exact recovered-Sign payload");
    let manifest = outbound.manifest().clone();
    let proposal = wire::Proposal {
        round,
        proposer: local,
        subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    let commitment = execution_commitment(0xD6);
    let prepare = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let RecoveredAdapterStartup { adapter, effects } = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![WalRecordV2::ProposalIntent(proposal.clone())],
    );
    let RecoveredAdapterStartup {
        adapter: cold_adapter,
        effects: cold_effects,
    } = write_and_reopen_authenticated_wal_startup_at_path(
        directory.path().join("cold-pair-preview-safety.wal"),
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare.clone()),
        ],
    );
    assert_eq!(cold_effects, effects);
    let [AdapterEffect::Sign { tag, request }] = effects.as_slice() else {
        panic!("recovered Proposal/Prepare FIFO must expose the Proposal Sign first")
    };
    assert_eq!(request, &SignRequest::Proposal(proposal.clone()));
    let tag = *tag;
    let request = request.clone();
    let prepare_identity = cold_adapter
        .authenticate_recovered_wal_frame(&cold_adapter.wal.recovered_records()[1])
        .expect("authenticate exact PrepareIntent frame")
        .0;
    let signature = Signature::new(
        keys[local_index].private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let completion = || {
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request.clone(),
            signature.clone(),
            Some(outbound.clone()),
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlProposal,
        )
    };
    let mut body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("exact-next-vote-body"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open exact next-Vote body store");
    let durable = body_store
        .store(manifest.clone(), canonical_wire)
        .expect("persist exact next-Vote body");
    let validated = body_store
        .validate(&durable, |_| Ok::<_, String>(commitment))
        .expect("persist exact next-Vote validation");
    let body_store_identity = body_store.instance_identity();
    let [
        AdapterEffect::Sign {
            tag: cold_tag,
            request: cold_request,
        },
    ] = cold_effects.as_slice()
    else {
        panic!("cold recovered Proposal/Prepare FIFO retains one Proposal Sign")
    };
    let SignRequest::Proposal(mut signed_proposal) = cold_request.clone() else {
        panic!("cold recovered control request is a Proposal")
    };
    signed_proposal.signature.clone_from(&signature);
    let cold_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(signed_proposal),
    ));
    let mut substituted_cold_broadcast = cold_broadcast.clone();
    let AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::Proposal(substituted_proposal),
        ..
    }) = &mut substituted_cold_broadcast
    else {
        unreachable!("cold fixture Broadcast is a signed Proposal")
    };
    substituted_proposal.subject.payload_hash = Hash::new(b"substituted cold Proposal payload");
    assert!(
        RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            *cold_tag,
            cold_request.clone(),
            substituted_cold_broadcast,
        )
        .is_none(),
        "a substituted signed Broadcast cannot mint cold preview authority"
    );
    let cold_authority =
        RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            *cold_tag,
            cold_request.clone(),
            cold_broadcast.clone(),
        )
        .expect("WAL-authenticated Proposal Broadcast mints one cold preview authority");
    drop(cold_effects);
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
        .expect("reverify cold recovered Proposal context");
    let recovered_local_proposal =
        RecoveredLifecycleLocalProposalAttemptV1::for_test(tag, proposal.round, proposal.subject);
    let mut cold_preview =
        ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
            cold_adapter,
            Vec::new(),
            Some(recovered_local_proposal),
        )
        .prepare_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_authority)
        .expect("cold adapter previews exact Broadcast and next Sign");
    let cold_body = body_store
        .authenticate_recovered_lifecycle_next_vote_body(&mut cold_preview)
        .expect("exact revalidated body store authenticates the cold next Vote");
    assert!(cold_body.exactly_matches_for_test(&validated, &body_store_identity,));
    assert!(matches!(
        body_store.authenticate_recovered_lifecycle_next_vote_body(&mut cold_preview),
        Err(super::super::v2_body_store::V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)
    ));
    let cold_seal = cold_preview
        .seal_recovered_lifecycle_next_wal_vote(cold_body)
        .expect("cold preview seals its exact WAL and body-owned next Vote");
    let (cold_startup, sealed_broadcast, sealed_next_sign, sealed_output) = cold_seal
        .consume_for_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
        );
    let expected_next_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(prepare.clone()),
    };
    assert_eq!(sealed_broadcast, cold_broadcast);
    assert!(
        sealed_output
            .as_ref()
            .is_some_and(|output| output.matches_broadcast(&sealed_broadcast))
    );
    assert!(sealed_next_sign.exactly_matches(prepare_identity, &expected_next_sign, &validated,));
    let cold_adapter_authority =
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            sealed_broadcast,
            expected_next_sign,
        )
        .expect("sealed cold pair retains the exact adapter replay relation");
    drop(sealed_next_sign);
    let cold_startup = cold_startup
        .advance_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_adapter_authority)
        .expect("sealed cold pair advances the retained original startup");
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        adapter: advanced_cold_adapter,
        effects: advanced_cold_effects,
        local_proposal_attempt: Some(recovered_local_proposal),
        leader_wire_launch_prepared: false,
        ..
    } = cold_startup.state
    else {
        panic!("advanced cold preview retains one recovered adapter startup")
    };
    assert!(advanced_cold_effects.is_empty());
    assert!(
        recovered_local_proposal.exactly_matches_directive(
            advanced_cold_adapter
                .local_proposal_directive()
                .expect("read the advanced cold Proposal directive"),
        ),
        "cold Broadcast-and-Sign replay must preserve its opaque local-attempt owner"
    );
    let Some(reducer::SignableMessage::Vote(advanced_vote)) =
        advanced_cold_adapter.reducer.awaiting_signature()
    else {
        panic!("advanced cold preview must await its exact next Vote")
    };
    assert_eq!(
        advanced_cold_adapter
            .registry
            .unsigned_vote_to_wire(*advanced_vote)
            .expect("reconstruct advanced cold next Vote"),
        prepare,
    );
    let now = Instant::now();
    let (mut runtime, startup_effects) = super::super::v2_runtime::SerializedV2Runtime::new(
        adapter,
        effects,
        now,
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap recovered adapter in the serialized runtime");
    let _startup_ownership = runtime
        .take_effect_ownership(startup_effects.len())
        .expect("transfer the recovered Sign's startup ownership");
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let requester = context.roster[local_index].validator.clone();
    let (mut executor, body_store) =
        super::super::v2_effects::V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            context.clone(),
            requester,
            Some(local),
            Arc::clone(&output_guard),
            super::super::v2_effects::EffectQueueConfig::default(),
        )
        .expect("open executor with exact recovered body catalogs");
    let (mut services, _) = super::super::v2_worker::tests::fixture();
    let service_io =
        super::super::v2_worker::tests::install_lifecycle_planner_io_for_validator_for_test(
            &mut services,
            context.clone(),
            local,
            tag,
            Arc::clone(&output_guard),
            body_store,
            body_store_identity.clone(),
            1,
        );
    super::super::v2_worker::tests::install_local_signer_for_test(
        &mut services,
        &keys[local_index],
    );
    let foreign_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("foreign-next-vote-body"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open foreign next-Vote body store");
    let foreign_store_identity = foreign_store.instance_identity();
    assert!(!foreign_store_identity.same_instance(&body_store_identity));
    let (mut foreign_services, _) = super::super::v2_worker::tests::fixture();
    let foreign_service_io =
        super::super::v2_worker::tests::install_lifecycle_planner_io_for_validator_for_test(
            &mut foreign_services,
            context,
            local,
            tag,
            Arc::clone(&output_guard),
            foreign_store,
            foreign_store_identity,
            1,
        );
    let status_before = executor.status();
    let tag_before = executor.current_tag();
    let error = match foreign_services
        .prepare_recovered_lifecycle_sign_completion_with_body(&mut executor, completion())
    {
        Err(error) => error,
        Ok(_) => panic!("a foreign body-store service must fail before adapter preview"),
    };
    assert!(error.contains("foreign service owner"));
    let mut status_after = executor.status();
    status_after.captured_at = status_before.captured_at;
    assert_eq!(status_after, status_before);
    assert_eq!(executor.current_tag(), tag_before);
    assert!(!output_guard.restart_required());
    let (mut preview, body_authority) = services
        .prepare_recovered_lifecycle_sign_completion_with_body(&mut executor, completion())
        .expect("the exact production service authenticates the next-Vote body");
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal)
    );
    assert!(body_authority.exactly_matches_for_test(&validated, &body_store_identity));
    let dispatch_key = preview.dispatch_key();
    let broadcast = preview.broadcast_effect().clone();
    assert!(matches!(
        &broadcast,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(signed),
            ..
        }) if signed.manifest == manifest && signed.signature == signature
    ));
    assert!(preview.next_sign_effect().is_none());
    let proposal_output = preview
        .project_proposal_exact_output_authority()
        .expect("seal the signed Proposal and exact recovered payload");
    assert!(
        preview.project_proposal_exact_output_authority().is_err(),
        "one adapter preview cannot duplicate Proposal output authority"
    );
    let proposal_output = match services
        .capture_recovered_lifecycle_proposal_exact_output(proposal_output)
        .expect("exact launched service reserves Proposal control and chunks")
    {
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            reservation,
        ) => reservation.abort_before_publication(),
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty exact-output corridor must retain the complete Proposal batch")
        }
    };
    let mut output = match services
        .capture_recovered_lifecycle_proposal_exact_output(proposal_output)
        .expect("typed abort returns the exact retry authority")
    {
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            reservation,
        ) => reservation,
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("retry against the unchanged empty corridor must remain reservable")
        }
    };
    let wal_permit = output
        .prepare_wal_append_permit()
        .expect("the armed Proposal output owns the initial WAL append");
    preview
        .append_recovered_lifecycle_proposal_prepare_wal(wal_permit)
        .expect("fsync the preflighted PrepareIntent before child publication");
    assert!(
        output.prepare_wal_append_permit().is_none(),
        "a successful WAL append irreversibly closes the retry permit"
    );
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign)
    );
    assert_eq!(preview.adapter.wal.recovered_records().len(), 2);
    assert_eq!(preview.adapter.pending_persistence_id, Some(2));
    let appended_prepare_identity = preview
        .adapter
        .authenticate_recovered_wal_frame(&preview.adapter.wal.recovered_records()[1])
        .expect("authenticate the just-fsynced PrepareIntent")
        .0;
    assert_eq!(appended_prepare_identity, prepare_identity);
    let next_sign = preview
        .next_sign_effect()
        .expect("the fsynced PrepareIntent retains its exact Prepare Sign")
        .clone();
    assert!(matches!(
        &next_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } if vote == &prepare
    ));
    let combined = preview
        .project_broadcast_and_sign_authority(body_authority)
        .expect("seal the exact production-authenticated successor pair");
    assert!(combined.exactly_matches_for_test(
        dispatch_key,
        &broadcast,
        prepare_identity,
        &next_sign,
        &validated,
    ));
    drop(combined);
    preview.commit_after_durable_broadcast_and_sign();
    output.commit_after_publication();
    foreign_service_io.detach(&mut foreign_services);
    service_io.detach(&mut services);
}
fn reviewed_v2_adapter_source_for_test() -> String {
    const RECOVERED_STARTUP_INCLUDE: &str =
        "include!(\"v2_adapter_recovered_startup_branches.rs\");";
    let parent = include_str!("../v2.rs");
    assert_eq!(
        parent.matches(RECOVERED_STARTUP_INCLUDE).count(),
        1,
        "the adapter must include the recovered-startup provider exactly once"
    );
    parent.replacen(
        RECOVERED_STARTUP_INCLUDE,
        include_str!("../v2_adapter_recovered_startup_branches.rs"),
        1,
    )
}
include!("v2_adapter_04_wal_recovery.rs");
include!("v2_adapter_04b_lifecycle_startup.rs");
include!("v2_adapter_05_direct_lifecycle.rs");
#[test]
fn recovered_wal_sign_status_publication_is_exact_last_and_unwired() {
    let source = reviewed_v2_adapter_source_for_test();
    let body_store_source = include_str!("../v2_body_store.rs");
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    let publication = production
        .split_once("// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_BEGIN")
        .expect("recovered Sign publication begins")
        .1
        .split_once("// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_END")
        .expect("recovered Sign publication ends")
        .0;
    assert!(
        publication.contains("#[cfg(test)]"),
        "the superseded parts-based publication remains test-only"
    );
    for required in [
        "struct PublishedRecoveredWalLifecycleStartup<'registry>",
        "struct RecoveredWalLifecycleOpenPublicationError<'registry>",
        "OpenedRecoveredWalSignLifecycleCut<'registry>",
        "RecoveredWalSignLifecycleOpenError<'registry>",
        "fn publish_open_result(",
        "let opened = match opened",
        "if let Err(error) = adapter.publish_status()",
        "RecoveredWalLifecycleOpenPublicationFailure::Status",
        "fn open_coordinator_and_publish(",
        "installed.open_coordinator_from_verified(",
        "fn open_coordinator_and_publish_for_test(",
        "installed.open_coordinator_for_test(",
    ] {
        assert!(
            publication.contains(required),
            "status-last publication omitted {required}"
        );
    }
    assert_eq!(publication.matches("adapter.publish_status()").count(), 1);
    let opened = publication
        .find("let opened = match opened")
        .expect("inner exact open is classified");
    let status = publication
        .find("adapter.publish_status()")
        .expect("adapter status is published");
    assert!(opened < status, "status must follow the exact open result");
    let owner_factory = production
        .split_once("pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(")
        .expect("locate the sole production owner factory")
        .1
        .split_once("/// Open an empty-marker test body store")
        .expect("locate the end of the production owner factory")
        .0;
    let canonical_factory = owner_factory
        .split_once("fn open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("locate the private authenticated-root implementation")
        .0;
    let factory_inputs = canonical_factory
        .find("factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1")
        .expect("factory consumes the adapter-bound execution/storage seal");
    assert!(canonical_factory.contains("body_store: super::v2_body_store::QuarantinedV2BodyStore"));
    assert!(!canonical_factory.contains("body_store: super::v2_body_store::V2BodyStore"));
    assert!(
        !canonical_factory.contains("body_store: super::v2_body_store::RevalidatedV2BodyStore")
    );
    let residual = canonical_factory
        .find("if !self.effects.is_empty()")
        .expect("factory rejects residual effects before marker replay");
    let startup_binding = canonical_factory
        .find("Arc::ptr_eq(&adapter_owner, &self.factory_owner)")
        .expect("factory input remains bound to this exact authenticated startup");
    let context_binding = canonical_factory
        .find("storage.context_id != context.id()")
        .expect("factory binds the storage authority to the recovered context");
    let body_root = canonical_factory
        .find("body_store.matches_lifecycle_storage_root(")
        .expect("factory joins the body store to the sealed root and policy");
    let wal_path = canonical_factory
        .find("self.adapter.wal.matches_path(&storage.wal_path)")
        .expect("factory joins the adapter to the recovery-sealed WAL path");
    let apply_service = canonical_factory
        .find("let apply_service = super::v2_apply::V2ApplyService::new(")
        .expect("factory constructs one exact marker/live Apply service");
    let replay = canonical_factory
        .find(".into_revalidated_lifecycle_startup(")
        .expect("factory consumes the fixed marker replay cut");
    let sealed_parts = canonical_factory
        .find("let RecoveredLifecycleStorageAuthorityV1 {")
        .expect("factory opens the storage authority only after validation");
    let authenticated_roots = canonical_factory
        .find("self.open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("factory enters the private implementation after target checks");
    let kura_binding = canonical_factory
        .find("owner.with_recovered_kura_binding_and_apply_service(")
        .expect("factory retains the Kura and replay service in one owner transition");
    assert!(factory_inputs < residual);
    assert!(residual < startup_binding);
    assert!(startup_binding < context_binding);
    assert!(context_binding < body_root);
    assert!(body_root < wal_path);
    assert!(wal_path < apply_service);
    assert!(apply_service < replay);
    assert!(replay < sealed_parts);
    assert!(sealed_parts < authenticated_roots);
    assert!(authenticated_roots < kura_binding);
    for forbidden in [
        "kura: &Kura",
        "ledger_root: &std::path::Path",
        "serve_payload_root: &std::path::Path",
        "body_root: &std::path::Path",
        "body_signature_policy:",
    ] {
        assert!(
            !canonical_factory.contains(forbidden),
            "production owner factory accepts forbidden raw target {forbidden}"
        );
    }
    let control_projection = owner_factory
        .find("project_recovered_wal_control_sign")
        .expect("factory projects the sealed control authority");
    let decision_projection = owner_factory
        .find("project_recovered_wal_decision_fetch")
        .expect("factory projects the sealed Decision authority");
    let decision_body_preflight = owner_factory
        .find("detach_recovered_decision_apply_body")
        .expect("factory preflights an opaque same-store Decision body");
    let body_handoff = owner_factory
        .find("into_lifecycle_owner_store")
        .expect("factory consumes the revalidated same-store handoff");
    let serve_open = owner_factory
        .find("CertifiedServePayloadStoreV1::open(")
        .expect("factory opens the Serve store");
    let owner_open = owner_factory
        .find(".into_owner(registry, payload_store, body_store)")
        .expect("factory constructs the recovered owner");
    assert!(residual < control_projection);
    assert!(control_projection < body_handoff);
    assert!(decision_projection < decision_body_preflight);
    assert!(decision_body_preflight < body_handoff);
    assert!(body_handoff < serve_open);
    assert!(serve_open < owner_open);
    assert!(!owner_factory.contains("publish_recovered_adapter_status"));
    assert!(!owner_factory.contains("recovery: AuthenticatedLifecycleRecoveryCut"));
    assert!(
        owner_factory.contains("ProductionLifecycleOwnerV1::open_recovered_decision_apply_startup")
    );
    assert!(
        !owner_factory.contains("restart-closed Decision Apply publication is not implemented")
    );
    assert!(!owner_factory.contains("V2BodyStore::open_with_policy("));
    assert!(!owner_factory.contains("body_root:"));
    let quarantine = body_store_source
        .split_once("impl QuarantinedV2BodyStore {")
        .expect("locate quarantined recovered-startup cut")
        .1
        .split_once("impl RevalidatedV2BodyStore {")
        .expect("locate end of quarantined recovered-startup cut")
        .0;
    assert!(quarantine.contains("fn into_revalidated_lifecycle_startup("));
    assert!(!quarantine.contains("fn retain_recovered_markers_for_subject("));
    assert!(!quarantine.contains("fn retain_recovered_markers_for_authority("));
    assert!(!quarantine.contains("fn revalidate_recovered_markers<"));
    assert!(!quarantine.contains("fn into_revalidated_startup("));
    let finality = quarantine
        .find("apply_service.recovered_finality_subject(context)")
        .expect("fixed replay derives the recovered-finality marker subject");
    let subject_filter = quarantine
        .find(".retain_recovered_markers_for_subject(subject)")
        .expect("fixed replay filters markers to recovered finality first");
    let authority_filter = quarantine
        .find(".retain_recovered_markers_for_authority(validation_authority)")
        .expect("fixed replay then filters markers to authenticated WAL authority");
    let semantic_replay = quarantine
        .find(".revalidate_recovered_markers(|body|")
        .expect("fixed replay semantically validates retained markers");
    let seal = quarantine
        .find("self.0.into_revalidated_startup()")
        .expect("fixed replay seals only replayed marker state");
    assert!(finality < subject_filter);
    assert!(subject_filter < authority_filter);
    assert!(authority_filter < semantic_replay);
    assert!(semantic_replay < seal);
    for forbidden in [
        "CandidateAdmission",
        "PendingRuntimeEffectBinding",
        "RuntimeEffectOwnership",
        "into_parts",
        "pub(crate) fn coordinator(",
        "pub(crate) fn effect(",
        "pub(crate) fn receipt(",
    ] {
        assert!(
            !publication.contains(forbidden),
            "status publication exposes forbidden surface {forbidden}"
        );
    }
    for runner_source in [
        include_str!("../v2_runner.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_effects.rs"),
    ] {
        assert!(!runner_source.contains("open_coordinator_and_publish("));
        assert!(!runner_source.contains("PublishedRecoveredWalLifecycleStartup"));
    }
}
