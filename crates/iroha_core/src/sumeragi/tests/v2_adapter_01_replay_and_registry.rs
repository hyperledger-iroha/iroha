    #[test]
    fn replay_resigns_only_an_acknowledged_intent() {
        let directory = TempDir::new().expect("temporary directory");
        {
            let (mut adapter, _) = open_test(&directory).expect("open adapter");
            let proposer = adapter.status().expect("status").leader;
            let subject = subject(9);
            let proposal = proposal(&adapter.wire_context, proposer, subject);
            let effects = adapter
                .receive_verified(proposal)
                .expect("accept proposal")
                .into_effects();
            let (tag, manifest) = match effects.as_slice() {
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
                .expect("body available");
            let receipt = durable_body_receipt(&adapter, round, subject);
            adapter
                .body_stored(tag, round, subject, &receipt)
                .expect("body stored");
            let validated = ValidatedBodyReceipt::for_test(receipt);
            let sign = adapter
                .validation_succeeded(tag, round, subject, &validated)
                .expect("body valid");
            assert!(matches!(sign.effects(), [AdapterEffect::Sign { .. }]));
        }

        let (adapter, startup) = open_test(&directory).expect("replay adapter");
        assert!(adapter.ingress_ready());
        assert!(matches!(startup.as_slice(), [AdapterEffect::Sign { .. }]));
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn replayed_decision_key_survives_incomplete_tail_and_rejects_key_drift() {
        let directory = TempDir::new().expect("temporary directory");
        let expected;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"pending Kura block")),
                payload_hash: Hash::new(b"pending exact body"),
            };
            let round = wire::ConsensusRound {
                context_id: adapter.wire_context.id(),
                height: adapter.wire_context.height,
                view: 0,
            };
            let commitment = execution_commitment(0xD4);
            let mut decision = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: Vec::new(),
            };
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS-normal key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let preimage = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: commitment,
                signer: 0,
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = keys[..3]
                .iter()
                .map(|key| {
                    Signature::new(key.private_key(), &preimage)
                        .payload()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            decision.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate fixture CommitQC");
            let record = WalEnvelopeV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                persistence_id: 1,
                record: WalRecordV2::Decision(decision),
            }
            .encode();
            adapter
                .wal
                .append(&record)
                .expect("append acknowledged Decision record");
            expected = (round, round, subject, commitment);
        }
        OpenOptions::new()
            .append(true)
            .open(directory.path().join("safety.wal"))
            .expect("open WAL tail")
            .write_all(b"S2FR\x01\x00")
            .expect("model incomplete next frame");

        let (mut adapter, startup) = open_test(&directory).expect("replay durable Decision");
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::FetchBody {
                certificate: Some(_),
                ..
            }]
        ));
        assert_eq!(
            adapter
                .replayed_decision_key()
                .expect("map replayed Decision"),
            Some(expected)
        );
        let (active_round, active_subject) = adapter
            .active_subject
            .expect("durable Decision owns the recovery body pipeline");
        assert_eq!(adapter.registry.round_to_wire(active_round), expected.1);
        assert_eq!(
            adapter
                .registry
                .subject(active_subject)
                .expect("map active decision subject"),
            expected.2
        );
        let status = adapter.status().expect("first decision recovery snapshot");
        assert_eq!(
            status.liveness.work.candidate,
            wire::SumeragiV2LocalWorkStage::Complete
        );
        assert_eq!(
            status.liveness.work.body_recovery,
            wire::SumeragiV2LocalWorkStage::Queued
        );
        assert_eq!(
            status.liveness.work.application,
            wire::SumeragiV2LocalWorkStage::Queued
        );
        assert!(matches!(
            status.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
                ..
            })
        ));
        drop(adapter);

        assert!(matches!(
            SumeragiV2Adapter::open_with_aggregator(
                directory.path().join("safety.wal"),
                verified_genesis(context()),
                Some(0),
                reducer::Generation::new(1),
                [0x99; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            ),
            Err(AdapterError::SafetyWal(SafetyWalError::IdentityMismatch {
                field: "consensus key hash",
                ..
            }))
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn replay_rejects_checksummed_wal_decision_without_quorum_authority() {
        let directory = TempDir::new().expect("temporary directory");
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let round = wire::ConsensusRound {
                context_id: adapter.wire_context.id(),
                height: adapter.wire_context.height,
                view: 0,
            };
            let decision = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject: subject(0xD5),
                execution_commitment: execution_commitment(0xD5),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xD5; 48],
            };
            let record = WalEnvelopeV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                persistence_id: 1,
                record: WalRecordV2::Decision(decision),
            }
            .encode();
            adapter
                .wal
                .append(&record)
                .expect("append a fully checksummed but unauthenticated Decision");
        }

        assert!(matches!(
            open_test(&directory),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn replay_rejects_forged_lock_before_resigning_the_commit_intent() {
        let directory = TempDir::new().expect("temporary directory");
        let wal_path = directory.path().join("forged-lock-safety.wal");
        let (context, _keys, proofs) = authenticated_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let locked_subject = subject(0xDB);
        let commitment = execution_commitment(0xDB);
        let forged_prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xDB; 48],
        };
        let commit_intent = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: commitment,
            signer: 0,
            signature: Vec::new(),
        };
        {
            let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
                .expect("verified genesis context");
            let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
                wal_path.clone(),
                verified,
                Some(0),
                reducer::Generation::new(1),
                [0x22; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            )
            .expect("open adapter");
            assert!(startup.is_empty());
            let record = WalEnvelopeV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                persistence_id: 1,
                record: WalRecordV2::LockAndCommit {
                    prepare: forged_prepare,
                    vote: commit_intent,
                },
            }
            .encode();
            adapter
                .wal
                .append(&record)
                .expect("append checksummed forged lock");
        }

        let verified =
            VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context");
        assert!(matches!(
            SumeragiV2Adapter::open_with_aggregator(
                wal_path,
                verified,
                Some(0),
                reducer::Generation::new(1),
                [0x22; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            ),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn wal_record_authority_rejects_forged_certificates_in_every_record_variant() {
        let (context, _keys, proofs) = authenticated_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let certified_subject = subject(0xD7);
        let commitment = execution_commitment(0xD7);
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: certified_subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD7; 48],
        };
        let commit = wire::QuorumCertificate {
            phase: wire::GlobalPhase::Commit,
            ..prepare.clone()
        };
        let timeout = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xD7; 48],
            }],
        };
        let proposal_round = wire::ConsensusRound { view: 1, ..round };
        let proposal_subject = subject(0xD8);
        let proposal_payload = [0xD8, 2];
        let proposal_manifest =
            encode_payload(&context, proposal_round, proposal_subject, &proposal_payload)
                .expect("encode fixture proposal payload")
                .manifest()
                .clone();
        let proposal = wire::Proposal {
            round: proposal_round,
            proposer: context.leader(proposal_round.view),
            subject: proposal_subject,
            manifest: proposal_manifest,
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: timeout.clone(),
                highest_prepare_qc: None,
            }),
            signature: Vec::new(),
        };
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: certified_subject,
            execution_commitment: commitment,
            signer: 0,
            signature: Vec::new(),
        };
        let timeout_vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer: 0,
            signature: Vec::new(),
        };
        let records = [
            (
                "ProposalIntent timeout",
                WalRecordV2::ProposalIntent(proposal),
            ),
            (
                "ObservePrepare",
                WalRecordV2::ObservePrepare(prepare.clone()),
            ),
            (
                "LockAndCommit",
                WalRecordV2::LockAndCommit {
                    prepare: prepare.clone(),
                    vote,
                },
            ),
            ("TimeoutIntent", WalRecordV2::TimeoutIntent(timeout_vote)),
            ("InstallTimeout", WalRecordV2::InstallTimeout(timeout)),
            ("Decision", WalRecordV2::Decision(commit)),
        ];
        for (kind, record) in records {
            assert!(
                matches!(
                    verify_wal_record_authority(&context, None, &record, &proofs),
                    Err(AdapterError::Cryptography(_))
                ),
                "{kind} must reauthenticate every embedded certificate"
            );
        }

        let forged_parent = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: certified_subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD9; 48],
        };
        let mut successor = context.clone();
        successor.height = context.height + 1;
        successor.parent_commit_qc = Some(forged_parent.clone());
        let successor_round = wire::ConsensusRound {
            context_id: successor.id(),
            height: successor.height,
            view: 0,
        };
        let successor_subject = subject(0xD9);
        let successor_payload = [0xD9, 2];
        let successor_manifest = encode_payload(
            &successor,
            successor_round,
            successor_subject,
            &successor_payload,
        )
        .expect("encode successor fixture payload")
        .manifest()
        .clone();
        let parent_proposal = wire::Proposal {
            round: successor_round,
            proposer: successor.leader(0),
            subject: successor_subject,
            manifest: successor_manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification {
                    certificate: Some(forged_parent),
                },
            ),
            signature: Vec::new(),
        };
        let parent_verification = ParentVerificationContext {
            context,
            proofs_of_possession: proofs.clone(),
        };
        assert!(matches!(
            verify_wal_record_authority(
                &successor,
                Some(&parent_verification),
                &WalRecordV2::ProposalIntent(parent_proposal),
                &proofs,
            ),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[test]
    fn wal_unsigned_intents_reject_ignored_signature_bytes() {
        let context = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
            proposal(&context, context.leader(0), subject(0xDA)).payload
        else {
            unreachable!("proposal fixture")
        };
        proposal.signature = vec![0xDA];
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xDA),
            execution_commitment: execution_commitment(0xDA),
            signer: 0,
            signature: vec![0xDA],
        };
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: vote.subject,
            execution_commitment: vote.execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xDA; 48],
        };
        let timeout_vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0xDA],
        };
        let records = [
            WalRecordV2::ProposalIntent(proposal),
            WalRecordV2::PrepareIntent(vote.clone()),
            WalRecordV2::LockAndCommit {
                prepare,
                vote: wire::Vote {
                    phase: wire::GlobalPhase::Commit,
                    ..vote
                },
            },
            WalRecordV2::TimeoutIntent(timeout_vote),
        ];
        for record in records {
            assert!(matches!(
                verify_wal_record_authority(&context, None, &record, &[]),
                Err(AdapterError::WalDecode(_))
            ));
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn replay_authenticates_the_exact_decision_not_a_same_reference_cache_alias() {
        let directory = TempDir::new().expect("temporary directory");
        let wal_path = directory.path().join("exact-decision-safety.wal");
        let (context, keys, proofs) = authenticated_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let decision_subject = subject(0xD6);
        let commitment = execution_commitment(0xD6);
        let forged = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: decision_subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD6; 48],
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: decision_subject,
            execution_commitment: commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let valid_signers = [0_usize, 1, 3];
        let valid_shares = valid_signers
            .iter()
            .map(|index| {
                Signature::new(keys[*index].private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let valid = wire::QuorumCertificate {
            signers: valid_signers
                .into_iter()
                .map(|index| u32::try_from(index).expect("small fixture signer index"))
                .collect(),
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &valid_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate valid same-reference CommitQC"),
            ..forged.clone()
        };
        verify_quorum_certificate(&context, &valid, &proofs)
            .expect("cache-alias fixture must be cryptographically valid");

        {
            let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
                .expect("verified genesis context");
            let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
                wal_path.clone(),
                verified,
                Some(0),
                reducer::Generation::new(1),
                [0x22; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            )
            .expect("open adapter");
            assert!(startup.is_empty());
            for (persistence_id, certificate) in [(1, forged), (2, valid)] {
                let record = WalEnvelopeV2 {
                    protocol_version: wire::PROTOCOL_VERSION,
                    persistence_id,
                    record: WalRecordV2::Decision(certificate),
                }
                .encode();
                adapter
                    .wal
                    .append(&record)
                    .expect("append checksummed Decision record");
            }
        }

        let verified =
            VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context");
        assert!(matches!(
            SumeragiV2Adapter::open_with_aggregator(
                wal_path,
                verified,
                Some(0),
                reducer::Generation::new(1),
                [0x22; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            ),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[test]
    fn verified_aggregate_qc_roundtrips_without_reaggregation() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let subject = subject(3);
        let certificate = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(3),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xAA; 96],
        };
        let core = registry
            .qc_to_core(&certificate, &context)
            .expect("convert verified QC");
        let roundtrip = registry
            .qc_to_wire(&core, &TestAggregator)
            .expect("convert QC to wire");
        assert_eq!(roundtrip, certificate);
    }

    #[test]
    fn registry_preserves_exact_qc_when_one_reference_has_distinct_signer_quorums() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let first = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x31),
            execution_commitment: execution_commitment(0x31),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA1; 96],
        };
        let second = wire::QuorumCertificate {
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0xB2; 96],
            ..first.clone()
        };

        let first_core = registry
            .qc_to_core(&first, &context)
            .expect("register first signer quorum");
        let second_core = registry
            .qc_to_core(&second, &context)
            .expect("register second signer quorum for the same reference");

        assert_eq!(
            registry
                .qc_to_wire(&first_core, &TestAggregator)
                .expect("recover first exact certificate"),
            first
        );
        assert_eq!(
            registry
                .qc_to_wire(&second_core, &TestAggregator)
                .expect("recover second exact certificate"),
            second
        );
    }

    #[test]
    fn aggregate_reconstruction_rejects_mixed_or_disagreeing_verified_tokens() {
        let mixed = vec![
            reducer::SignatureShare::new(
                validator_token(0),
                reducer::OpaqueSignature::new(vec![0xA0; 96]),
            ),
            reducer::SignatureShare::new(validator_token(1), aggregate_token(&[0xA1; 96])),
        ];
        assert!(matches!(
            aggregate_core_shares(&mixed, &TestAggregator),
            Err(AdapterError::SignatureAggregation(_))
        ));

        let disagreeing = vec![
            reducer::SignatureShare::new(validator_token(0), aggregate_token(&[0xA2; 96])),
            reducer::SignatureShare::new(validator_token(1), aggregate_token(&[0xA3; 96])),
        ];
        assert!(matches!(
            aggregate_core_shares(&disagreeing, &TestAggregator),
            Err(AdapterError::SignatureAggregation(_))
        ));
    }

    #[test]
    fn registry_rejects_vote_or_qc_execution_commitment_drift_for_one_body() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let subject = subject(0xEC);
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let canonical_commitment = execution_commitment(0xEC);
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: canonical_commitment,
            signer: 0,
            signature: vec![1],
        };
        registry
            .vote_to_core(&vote, &context)
            .expect("first commitment binds body");
        vote.signer = 1;
        vote.execution_commitment = execution_commitment(0xED);
        assert!(matches!(
            registry.vote_to_core(&vote, &context),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(0xED),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![2],
        };
        assert!(matches!(
            registry.qc_to_core(&certificate, &context),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let reproposal_round = wire::ConsensusRound { view: 1, ..round };
        let mut reproposal_certificate = wire::QuorumCertificate {
            round: reproposal_round,
            proposal_round: reproposal_round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: execution_commitment(0xEF),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![3],
        };
        assert!(matches!(
            registry.qc_to_core(&reproposal_certificate, &context),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        reproposal_certificate.execution_commitment = canonical_commitment;
        registry
            .qc_to_core(&reproposal_certificate, &context)
            .expect("an unchanged re-proposal retains the deterministic execution result");
    }

    #[test]
    fn registry_rejects_split_round_vote_and_qc_reference() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let certified_round = wire::ConsensusRound {
            view: 1,
            ..proposal_round
        };
        let subject = subject(0xEE);
        let commitment = execution_commitment(0xEE);
        let vote = wire::Vote {
            round: certified_round,
            proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: commitment,
            signer: 0,
            signature: vec![1],
        };
        assert!(matches!(
            registry.vote_to_core(&vote, &context),
            Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound
            ))
        ));

        let reference = wire::QuorumCertificateRef {
            round: certified_round,
            proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: commitment,
        };
        assert!(matches!(
            registry.qc_reference_to_core(&reference),
            Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound
            ))
        ));
    }

    #[test]
    fn self_contained_grouped_timeout_certificate_roundtrips() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let subject = subject(5);
        let prepare = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(5),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xAB; 96],
        };
        let certificate = wire::TimeoutCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 3,
            },
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xBC; 96],
            }],
        };
        let core = registry
            .tc_to_core(&certificate, &context)
            .expect("convert verified TC");
        let roundtrip = registry
            .tc_to_wire(&core, &TestAggregator)
            .expect("convert TC to wire");
        assert_eq!(roundtrip, certificate);
    }

    #[test]
    fn registry_preserves_distinct_timeout_certificates_for_one_round() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let first = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xC1; 96],
            }],
        };
        let second = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 3],
                aggregate_signature: vec![0xC2; 96],
            }],
        };
        let first_core = registry
            .tc_to_core(&first, &context)
            .expect("register first timeout quorum");
        let second_core = registry
            .tc_to_core(&second, &context)
            .expect("register second timeout quorum for the same round");

        assert_eq!(
            registry
                .tc_to_wire(&first_core, &TestAggregator)
                .expect("recover first exact timeout certificate"),
            first
        );
        assert_eq!(
            registry
                .tc_to_wire(&second_core, &TestAggregator)
                .expect("recover second exact timeout certificate"),
            second
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn equivocation_flood_is_bounded_and_cannot_starve_commit_qc() {
        fn flood_subject(counter: u64) -> wire::BlockSubject {
            let mut bytes = [0_u8; 9];
            bytes[..8].copy_from_slice(&counter.to_le_bytes());
            bytes[8] = 0;
            let parent_block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
            bytes[8] = 1;
            let block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
            bytes[8] = 2;
            wire::BlockSubject {
                parent_block_hash: Some(parent_block_hash),
                block_hash,
                payload_hash: Hash::new(bytes),
            }
        }

        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        // Drive the local node to an outstanding Prepare signature. Authenticated
        // network inputs now exercise the adapter's deferred queues.
        let proposer = adapter.status().expect("status").leader;
        let decided_subject = subject(0xD0);
        let proposal = proposal(&adapter.wire_context, proposer, decided_subject);
        let fetch = adapter
            .receive_verified(proposal)
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match fetch.as_slice() {
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
            .expect("body available");
        let receipt = durable_body_receipt(&adapter, round, decided_subject);
        adapter
            .body_stored(tag, round, decided_subject, &receipt)
            .expect("body stored");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let decided_execution_commitment = validated.execution_commitment();
        let sign = adapter
            .validation_succeeded(tag, round, decided_subject, &validated)
            .expect("body valid")
            .into_effects();
        let _sign_tag = match sign.as_slice() {
            [AdapterEffect::Sign { tag, .. }] => *tag,
            effects => panic!("unexpected validation effects: {effects:?}"),
        };

        let first_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: flood_subject(0),
            execution_commitment: execution_commitment(0x41),
            signer: 1,
            signature: vec![0x41],
        };
        let first = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(first_vote),
            ))
            .expect("defer first vote");
        assert_eq!(
            first.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), 1);

        let mut evidence_reports = 0_usize;
        let flood_size = u64::try_from(MAX_DEFERRED_INPUTS).expect("queue bound fits u64") + 128;
        for counter in 1..=flood_size {
            let vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: flood_subject(counter),
                execution_commitment: execution_commitment(0x42),
                signer: 1,
                signature: vec![0x42],
            };
            let outcome = adapter
                .receive_verified(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(vote),
                ))
                .expect("equivocation admission stays live");
            evidence_reports += outcome
                .effects()
                .iter()
                .filter(|effect| matches!(effect, AdapterEffect::ReportEquivocation { .. }))
                .count();
        }
        assert_eq!(evidence_reports, 1, "evidence is capped per semantic key");
        assert_eq!(adapter.deferred_inputs.len(), 1);
        assert_eq!(adapter.ingress_equivocations.len(), 2);
        assert_eq!(adapter.ingress_deliveries.len(), 2);
        assert!(adapter.registry.subjects.len() <= 2);

        // A valid CommitQC supersedes the outstanding local signer immediately;
        // it must not join ordinary or PrepareQC Busy-deferred ownership.
        let commit_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: decided_execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC0; 96],
        };
        let commit = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(commit_qc),
            ))
            .expect("apply CommitQC through the signature fence");
        assert_eq!(commit.disposition(), reducer::StepDisposition::Applied);
        let decided = commit.into_effects();
        assert!(decided.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Apply { subject, .. } if *subject == decided_subject
        )));
        let decided_subject = adapter
            .registry
            .register_subject(decided_subject)
            .expect("subject");
        assert_eq!(
            adapter
                .reducer
                .durable_state()
                .decision()
                .map(reducer::QuorumCertificate::subject),
            Some(decided_subject)
        );
        assert!(adapter.deferred_progress_inputs.is_empty());
        assert_eq!(adapter.deferred_inputs.len(), 1);
        assert!(
            adapter
                .drain_deferred()
                .expect("service the remaining normal deferred input")
                .is_empty()
        );
        assert!(adapter.deferred_inputs.is_empty());
    }

    #[test]
    fn unsafe_proposal_admission_preserves_duplicate_and_equivocation_semantics() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let locked_subject = subject(0xC4);
        let locked_execution_commitment = execution_commitment(0xC4);
        let core_context = adapter.reducer.context().clone();
        let core_round = reducer::Round::new(core_context.height(), 0);
        let core_subject = adapter
            .registry
            .register_subject(locked_subject)
            .expect("register locked subject");
        adapter
            .registry
            .register_execution_commitment(core_round, core_subject, locked_execution_commitment)
            .expect("register locked execution commitment");
        let shares = (0_u32..3)
            .map(|index| {
                reducer::SignatureShare::new(
                    adapter
                        .registry
                        .validator_id(index)
                        .expect("fixture validator"),
                    reducer::OpaqueSignature::new(vec![
                        0xC4,
                        u8::try_from(index).expect("small validator index"),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        let prepare = reducer::QuorumCertificate::new(
            reducer::CertificateRef::new(
                core_context.id(),
                core_round,
                reducer::Phase::Prepare,
                core_subject,
            ),
            shares,
        );
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("local fixture validator");
        adapter.reducer = reducer::Reducer::recover(
            core_context.clone(),
            Some(local_validator),
            reducer::Generation::new(2),
            [reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::LockAndCommit {
                    prepare,
                    vote: reducer::Vote::new(
                        core_context.id(),
                        core_round,
                        reducer::Phase::Commit,
                        core_subject,
                        local_validator,
                    ),
                },
            )],
        )
        .expect("recover durable lock without resuming reducer delivery");

        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let proposer = adapter.wire_context.leader(wire_round.view);
        let unsafe_proposal = proposal(&adapter.wire_context, proposer, subject(0xC5));
        let conflicting_proposal = proposal(&adapter.wire_context, proposer, subject(0xC6));
        let reducer_before = adapter.reducer.clone();
        let registry_before = (
            adapter.registry.subjects.len(),
            adapter.registry.manifests.len(),
            adapter.registry.execution_commitments.len(),
            adapter.registry.proposals.len(),
        );
        let active_subject_before = adapter.active_subject;

        let first = adapter
            .receive_verified(unsafe_proposal.clone())
            .expect("reject the first unsafe proposal at admission");
        assert_eq!(
            first.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::UnsafeProposal)
        );
        assert!(first.effects().is_empty());

        let retransmit = adapter
            .receive_verified(unsafe_proposal)
            .expect("coalesce the exact unsafe proposal retransmission");
        assert_eq!(
            retransmit.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert!(retransmit.effects().is_empty());

        let conflict = adapter
            .receive_verified(conflicting_proposal)
            .expect("report the conflicting proposal fingerprint");
        assert_eq!(conflict.disposition(), reducer::StepDisposition::Applied);
        assert_eq!(
            conflict.effects(),
            &[AdapterEffect::ReportEquivocation {
                offender: adapter.wire_context.roster
                    [usize::try_from(proposer).expect("small proposer index")]
                .validator
                .clone(),
                round: wire_round,
                kind: reducer::EquivocationKind::Proposal,
            }]
        );

        assert_eq!(
            adapter.reducer, reducer_before,
            "unsafe proposal admission must not reach reducer delivery"
        );
        assert_eq!(
            (
                adapter.registry.subjects.len(),
                adapter.registry.manifests.len(),
                adapter.registry.execution_commitments.len(),
                adapter.registry.proposals.len(),
            ),
            registry_before,
            "unsafe proposal admission must not stage registry conversion"
        );
        assert_eq!(adapter.active_subject, active_subject_before);
        assert!(!adapter.fail_closed);
    }

    #[test]
    fn admission_keeps_only_the_exact_locked_commit_vote_beyond_one_rotation() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let locked_subject = subject(0xD4);
        let core_subject = reducer::Subject::new(Hash::new(locked_subject.encode()).into());
        let core_context = adapter.reducer.context().clone();
        let round = reducer::Round::new(core_context.height(), 0);
        assert_eq!(
            adapter
                .registry
                .register_subject(locked_subject)
                .expect("register locked subject"),
            core_subject
        );
        adapter
            .registry
            .register_execution_commitment(round, core_subject, execution_commitment(0xD4))
            .expect("register locked execution commitment");
        let shares = |marker| {
            (0_u32..3)
                .map(|index| {
                    reducer::SignatureShare::new(
                        adapter
                            .registry
                            .validator_id(index)
                            .expect("fixture validator"),
                        reducer::OpaqueSignature::new(vec![
                            marker,
                            u8::try_from(index).expect("small fixture validator index"),
                        ]),
                    )
                })
                .collect::<Vec<_>>()
        };
        let prepare = reducer::QuorumCertificate::new(
            reducer::CertificateRef::new(
                core_context.id(),
                round,
                reducer::Phase::Prepare,
                core_subject,
            ),
            shares(0xA1),
        );
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("local fixture validator");
        let timeout_round = reducer::Round::new(
            core_context.height(),
            u64::try_from(adapter.wire_context.roster.len()).expect("small roster") + 1,
        );
        let timeout = reducer::TimeoutCertificate::new(
            core_context.id(),
            timeout_round,
            vec![reducer::TimeoutSignatureGroup::new(
                Some(prepare.clone()),
                shares(0xA2),
            )],
        );
        adapter.reducer = reducer::Reducer::recover(
            core_context.clone(),
            Some(local_validator),
            reducer::Generation::new(2),
            [
                reducer::WalEntry::new(
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
                ),
                reducer::WalEntry::new(
                    reducer::PersistenceId::new(2),
                    reducer::WalRecord::InstallTimeout(timeout),
                ),
            ],
        )
        .expect("recover a lock older than one complete leader rotation");
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
        adapter
            .reducer
            .step(reducer::Event::Signed {
                tag: replay_tag,
                signature: reducer::OpaqueSignature::new(vec![0xB0]),
            })
            .expect("restore the local locked CommitVote");
        assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);

        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let locked_commit = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: execution_commitment(0xD4),
            signer: 1,
            signature: vec![0xB1],
        });
        let (outcome, admission) = adapter
            .admit_authenticated_payload(&locked_commit)
            .expect("exact locked CommitVote remains admissible");
        assert!(outcome.is_none());
        assert!(admission.is_some());
        let (outcome, admission) = adapter
            .admit_authenticated_payload(&locked_commit)
            .expect("pre-delivery admission does not consume the generation");
        assert!(outcome.is_none());
        assert!(admission.is_some());
        let received = adapter
            .receive_verified(wire::ConsensusMessageV2::new(locked_commit.clone()))
            .expect("locked CommitVote reaches the freshly cleared reducer pool");
        assert_eq!(received.disposition(), reducer::StepDisposition::Applied);
        assert!(received.effects().is_empty());
        assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
        let duplicate = adapter
            .receive_verified(wire::ConsensusMessageV2::new(locked_commit.clone()))
            .expect("same-generation duplicate is harmless");
        assert_eq!(
            duplicate.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );

        let mut quorum_vote = match locked_commit.clone() {
            wire::ConsensusMessageV2Payload::Vote(vote) => vote,
            _ => unreachable!("fixture is a CommitVote"),
        };
        quorum_vote.signer = 2;
        quorum_vote.signature = vec![0xB2];
        let quorum_vote = adapter
            .registry
            .vote_to_core(&quorum_vote, &adapter.wire_context)
            .expect("convert the final locked-round CommitVote");
        let quorum = adapter
            .reducer
            .step(reducer::Event::VoteReceived {
                tag: adapter.reducer.current_tag(),
                vote: quorum_vote,
            })
            .expect("a third locked-round CommitVote rebuilds the cleared quorum");
        assert!(matches!(
            quorum.effects(),
            [reducer::Effect::Persist { entry, .. }]
                if matches!(
                    entry.record(),
                    reducer::WalRecord::Decision(certificate)
                        if certificate.round() == round
                            && certificate.phase() == reducer::Phase::Commit
                            && certificate.subject() == core_subject
                )
        ));

        for rejected in [
            wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Prepare,
                subject: locked_subject,
                execution_commitment: execution_commitment(0xD4),
                signer: 1,
                signature: vec![0xB2],
            }),
            wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Commit,
                subject: subject(0xD5),
                execution_commitment: execution_commitment(0xD5),
                signer: 1,
                signature: vec![0xB3],
            }),
        ] {
            let (outcome, admission) = adapter
                .admit_authenticated_payload(&rejected)
                .expect("irrelevant historical vote is harmless");
            assert!(matches!(
                outcome.map(|outcome| outcome.disposition()),
                Some(reducer::StepDisposition::Ignored(
                    reducer::IgnoreReason::IrrelevantView
                ))
            ));
            assert!(admission.is_none());
        }
    }
