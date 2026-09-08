// Durable recovery regressions for Proposal signatures whose next Vote advanced.
#[cfg(feature = "bls")]
fn recovered_proposal_continuation_case(commit_state: u8, body_state: u8) {
    use crate::sumeragi::v2_lifecycle_coordinator::{
        ProductionLifecycleOwnerV1, RecoveredLifecycleNextWalVoteSealV1,
    };
    let directory = TempDir::new().expect("continuation crash fixture");
    let (context, keys, proofs) = authenticated_context();
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone()).unwrap();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let local = context.leader(0);
    let signer = &keys[usize::try_from(local).unwrap()];
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).unwrap(),
        None,
        None,
        None,
        8_214,
        0,
    );
    let block = SignedBlock::presigned(
        BlockSignature::new(
            u64::from(local),
            SignatureOf::try_from_hash(signer.private_key(), header.hash()).unwrap(),
        ),
        header,
        Vec::new(),
    );
    let bytes = block.encode_wire().unwrap();
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&bytes),
    };
    let manifest = encode_payload(&context, round, subject, &bytes)
        .unwrap()
        .manifest()
        .clone();
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
    let commitment = execution_commitment(0xE7);
    let prepare = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let commit = wire::Vote {
        phase: wire::GlobalPhase::Commit,
        ..prepare.clone()
    };
    let mut records = vec![
        WalRecordV2::ProposalIntent(proposal.clone()),
        WalRecordV2::PrepareIntent(prepare.clone()),
    ];
    if commit_state != 0 {
        let mut qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut qc, &keys);
        records.push(WalRecordV2::LockAndCommit {
            prepare: qc,
            vote: commit.clone(),
        });
    }
    let wal_path = directory.path().join("authenticated-fifo-safety.wal");
    let initial = write_and_reopen_authenticated_wal_startup(
        &directory, &context, &proofs, local, [0xE7; 32], records,
    );
    let tag = initial.adapter.current_tag();
    let mut body = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("body"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .unwrap();
    let durable = body.store(manifest, bytes).unwrap();
    let validated = body
        .validate(&durable, |_| Ok::<_, String>(commitment))
        .unwrap();
    let vote_seal = |index, vote| {
        let identity = initial
            .adapter
            .authenticate_recovered_wal_frame(&initial.adapter.wal.recovered_records()[index])
            .unwrap()
            .0;
        RecoveredLifecycleNextWalVoteSealV1::for_test(identity, tag, vote, validated.clone())
            .unwrap()
    };
    let signed_vote = |mut vote: wire::Vote| {
        vote.signature = Signature::new(signer.private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(vote),
        ))
    };
    let mut votes = vec![(vote_seal(1, prepare.clone()), Some(signed_vote(prepare)))];
    if commit_state != 0 {
        votes.push((
            vote_seal(2, commit.clone()),
            (commit_state == 2).then(|| signed_vote(commit)),
        ));
    }
    if body_state == 3 {
        votes.clear();
    }
    if body_state == 5 {
        votes[0].1 = None;
    }
    let authenticated = initial
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate original WAL: {error}"));
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = authenticated.authority else {
        panic!("Proposal remains first WAL authority")
    };
    let projection =
        super::super::v2_runtime::project_recovered_wal_control_sign(&verified, control)
            .unwrap_or_else(|_| panic!("control projection"));
    let mut signed_proposal = proposal;
    signed_proposal.signature =
        Signature::new(signer.private_key(), &signed_proposal.signature_preimage())
            .payload()
            .to_vec();
    if body_state == 4 {
        signed_proposal.signature[0] ^= 1;
    }
    let ledger_root = directory.path().join("ledger");
    projection.persist_advanced_continuation_for_test(
        &verified,
        &ledger_root,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(signed_proposal),
        )),
        votes,
    );
    drop(projection);
    drop(authenticated.adapter);
    drop(body);
    if body_state == 1 {
        std::fs::remove_dir_all(directory.path().join("body")).unwrap();
    }
    let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
    let ledger_before = std::fs::read(&ledger_path).unwrap();
    let wal_before = std::fs::read(&wal_path).unwrap();
    for _ in 0..2 {
        let startup = SumeragiV2Adapter::open_recovered_startup_with_aggregator(
            wal_path.clone(),
            verified.clone(),
            Some(local),
            reducer::Generation::new(50),
            [0xE7; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .unwrap();
        let authenticated = startup
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|(error, _)| panic!("authenticate cold WAL: {error}"));
        assert!(authenticated.effects.is_empty());
        let RecoveredWalStartupAuthorityV1::ControlSign(control) = authenticated.authority else {
            panic!("cold Proposal authority")
        };
        let projection =
            super::super::v2_runtime::project_recovered_wal_control_sign(&verified, control)
                .unwrap_or_else(|_| panic!("cold control projection"));
        let local_attempt = RecoveredLifecycleLocalProposalAttemptV1::for_test(tag, round, subject);
        let startup = ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
            authenticated.adapter,
            Vec::new(),
            Some(local_attempt),
        );
        let mut body = super::super::v2_body_store::V2BodyStore::open_with_policy(
            directory.path().join("body"),
            context.clone(),
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
        )
        .unwrap();
        let revalidated = body.revalidate_recovered_markers(|_| {
            Ok::<_, String>(if body_state == 2 {
                execution_commitment(0xE8)
            } else {
                commitment
            })
        });
        if body_state == 2 {
            assert!(matches!(revalidated, Err(super::super::v2_body_store::V2BodyStoreError::RecoveredValidationCommitmentMismatch)));
        } else {
            revalidated.unwrap();
        }
        let (payload_store, recovered) =
            super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &directory.path().join("serve"),
                &context,
            )
            .unwrap();
        let payloads = recovered.authenticate(&verified, signer, &body).unwrap();
        let result = ProductionLifecycleOwnerV1::open_recovered_control_startup(
            verified.clone(),
            projection,
            &ledger_root,
            body,
            &lifecycle_owner_config(),
            4,
            payload_store,
            payloads,
            startup,
        );
        if body_state == 0 || body_state == 5 {
            let mut owner = result.unwrap_or_else(|error| {
                panic!(
                    "reopen advanced Proposal continuation {commit_state}: {}",
                    error.reason()
                )
            });
            assert!(
                owner.exact_recovered_body_pipeline_join_for_test(),
                "every live carrier must remain exactly owned"
            );
        } else {
            let error = match result {
                Ok(_) => panic!("missing/substituted body cannot authorize a continuation"),
                Err(error) => error,
            };
            if body_state <= 2 {
                assert!(
                    error.reason().contains("body-store authority"),
                    "{}",
                    error.reason()
                );
            }
        }
        assert_eq!(
            std::fs::read(&ledger_path).unwrap(),
            ledger_before,
            "recovery never rewrites the durable crash frame"
        );
        assert_eq!(
            std::fs::read(&wal_path).unwrap(),
            wal_before,
            "recovery never appends a duplicate WAL intent"
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn production_recovered_proposal_advanced_prepare_reopens_exactly() {
    run_lifecycle_fixture_on_large_stack("advanced Prepare continuation", || {
        recovered_proposal_continuation_case(0, 0);
        recovered_proposal_continuation_case(0, 5);
    });
}
#[cfg(feature = "bls")]
#[test]
fn production_recovered_proposal_advanced_commit_reopens_exactly() {
    run_lifecycle_fixture_on_large_stack("advanced Commit continuation", || {
        recovered_proposal_continuation_case(1, 0);
        recovered_proposal_continuation_case(2, 0);
    });
}
#[cfg(feature = "bls")]
#[test]
fn production_recovered_proposal_advanced_continuation_rejects_missing_or_substituted_body() {
    run_lifecycle_fixture_on_large_stack("invalid continuation body", || {
        recovered_proposal_continuation_case(0, 1);
        recovered_proposal_continuation_case(0, 2);
    });
}

#[cfg(feature = "bls")]
#[test]
fn production_recovered_proposal_advanced_continuation_rejects_incomplete_or_forged_lineage() {
    run_lifecycle_fixture_on_large_stack("invalid continuation lineage", || {
        recovered_proposal_continuation_case(0, 3);
        recovered_proposal_continuation_case(0, 4);
    });
}
