#[cfg(test)]
mod recovery_tests {
    use super::super::schema::{CausalRoot, DurableContinuation, LifecycleStageKind, OwnerId};
    use super::*;
    #[cfg(feature = "bls")]
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    #[cfg(feature = "bls")]
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
    #[cfg(feature = "bls")]
    use tempfile::TempDir;
    #[cfg(feature = "bls")]
    fn empty_authenticated_payload_cut() -> (
        LifecycleContext,
        AuthenticatedCertifiedServePayloadRecoveryCut,
    ) {
        let mut keys = (0xC1_u8..=0xC4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic assembler BLS key")
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
            network_id: crate::sumeragi::synthetic_network_id(
                "storage-only-lifecycle-recovery-assembler-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"storage-only recovery AMX context"),
            execution_policy_hash: Hash::new(b"storage-only recovery execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0xC5; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(context, proofs)
            .expect("verify assembler height context");
        let root = TempDir::new().expect("temporary assembler storage root");
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.path().join("body"),
            verified.context().clone(),
        )
        .expect("open empty body store");
        let (_payload_store, recovered) =
            CertifiedServePayloadStoreV1::open(&root.path().join("payload"), verified.context())
                .expect("open empty payload store");
        let payloads = recovered
            .authenticate(&verified, &keys[0], &body_store)
            .expect("authenticate empty payload cut");
        (
            super::super::projection::lifecycle_context(verified.context()),
            payloads,
        )
    }
    #[cfg(feature = "bls")]
    fn sign_proposal_record(
        context: LifecycleContext,
        ordinal: u128,
        marker: u8,
        terminal: Option<TerminalOutcome>,
    ) -> LifecycleLedgerRecordV1 {
        let causal_root = CausalRoot::new(LifecycleDigest::new([marker; 32]));
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignProposal,
            u8::try_from(ordinal).expect("small SignProposal fixture ordinal"),
        );
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            terminal,
            causal_root.digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::None,
        )
        .expect("construct SignProposal record")
    }
    #[cfg(feature = "bls")]
    fn live_synthetic_serve_ledger(context: LifecycleContext) -> LifecycleLedgerV1 {
        let serve = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::CertifiedServe,
            0xC7,
        );
        let producer = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ProducerTurn,
            0xC7,
        );
        let causal_root = CausalRoot::new(LifecycleDigest::new([0xC8; 32]));
        let owner = OwnerId::new(causal_root, 1);
        let serve = LifecycleLedgerRecordV1::new(
            serve.key,
            owner,
            1,
            serve.work_class,
            serve.stage,
            None,
            causal_root.digest(),
            serve.payload,
            serve.authority,
            DurableContinuation::None,
        )
        .expect("construct synthetic live Serve row");
        let producer = LifecycleLedgerRecordV1::new(
            producer.key,
            owner,
            2,
            producer.work_class,
            producer.stage,
            None,
            causal_root.digest(),
            producer.payload,
            producer.authority,
            DurableContinuation::None,
        )
        .expect("construct synthetic live Producer row");
        LifecycleLedgerV1::new(context, 2, vec![serve, producer], BTreeMap::from([(1, 2)]))
            .expect("construct synthetic live Serve ledger")
    }
    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_serve_reconciliation_binds_the_exact_source_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = LifecycleLedgerV1::empty(context);
        let reconciliation = reconcile_complete_tip_serve_retirement(&ledger, payloads)
            .expect("empty final cut reconciles with the empty frame");
        assert!(reconciliation.authenticates_source(&ledger));
        assert!(reconciliation.is_drained());
        let stale = LifecycleLedgerV1::new(
            context,
            1,
            vec![sign_proposal_record(
                context,
                1,
                0xC9,
                Some(TerminalOutcome::Cancelled),
            )],
            BTreeMap::new(),
        )
        .expect("construct same-context stale frame");
        assert!(!reconciliation.authenticates_source(&stale));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_serve_reconciliation_rejects_missing_final_cut_coverage() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = live_synthetic_serve_ledger(context);
        assert!(reconcile_complete_tip_serve_retirement(&ledger, payloads).is_err());
    }
    fn classifier_record(
        ordinal: u128,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
        terminal: Option<TerminalOutcome>,
        continuation: DurableContinuation,
    ) -> LifecycleLedgerRecordV1 {
        let context = LifecycleContext::new(LifecycleDigest::new([0x91; 32]), 11);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            stage_kind,
            u8::try_from(ordinal).expect("small classifier view"),
        );
        let causal_root = CausalRoot::new(LifecycleDigest::new(
            [u8::try_from(ordinal).expect("small classifier marker"); 32],
        ));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            terminal,
            causal_root.digest(),
            replay.payload,
            replay.authority,
            continuation,
        )
        .expect("construct classifier-only durable record")
        .with_work_class_for_test(work_class)
    }
    fn ordinary_stage_inventory() -> [(LifecycleWorkClass, LifecycleStageKind); 20] {
        [
            (
                LifecycleWorkClass::SignProposal,
                LifecycleStageKind::SignProposal,
            ),
            (
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignPrepareVote,
            ),
            (
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignCommitVote,
            ),
            (
                LifecycleWorkClass::SignTimeout,
                LifecycleStageKind::SignTimeoutVote,
            ),
            (LifecycleWorkClass::Fetch, LifecycleStageKind::FetchBody),
            (LifecycleWorkClass::Store, LifecycleStageKind::StoreBody),
            (
                LifecycleWorkClass::Validate,
                LifecycleStageKind::ValidateBody,
            ),
            (LifecycleWorkClass::Apply, LifecycleStageKind::ApplyDecision),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastProposal,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastCommitVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastPrepareQc,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastCommitQc,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTc,
            ),
            (LifecycleWorkClass::EnterView, LifecycleStageKind::EnterView),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportProposalEquivocation,
            ),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportVoteEquivocation,
            ),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportTimeoutEquivocation,
            ),
            (
                LifecycleWorkClass::InvalidBodyReport,
                LifecycleStageKind::ReportInvalidBody,
            ),
        ]
    }
    #[test]
    fn storage_only_classifier_rejects_every_live_ordinary_stage_typed() {
        for (index, (work_class, stage_kind)) in ordinary_stage_inventory().into_iter().enumerate()
        {
            let ordinal = u128::try_from(index + 1).expect("small classifier ordinal");
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                None,
                DurableContinuation::None,
            );
            let Err(LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: observed_ordinal,
                work_class: observed_class,
                stage: observed_stage,
            }) = classify_storage_only_record(&record)
            else {
                panic!("live {work_class:?}/{stage_kind:?} did not fail with typed authority debt")
            };
            assert_eq!(observed_ordinal, ordinal);
            assert_eq!(observed_class, work_class);
            assert_eq!(observed_stage.kind(), stage_kind);
        }
    }
    #[test]
    fn storage_only_classifier_accepts_terminal_inventory_and_serve_pair_only() {
        for (ordinal, work_class, stage_kind) in [
            (
                1,
                LifecycleWorkClass::CertifiedServe,
                LifecycleStageKind::CertifiedServe,
            ),
            (
                2,
                LifecycleWorkClass::ProducerTurn,
                LifecycleStageKind::ProducerTurn,
            ),
        ] {
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                None,
                DurableContinuation::None,
            );
            assert!(classify_storage_only_record(&record).is_ok());
        }
        for (index, (work_class, stage_kind)) in ordinary_stage_inventory()
            .into_iter()
            .chain([
                (
                    LifecycleWorkClass::CertifiedServe,
                    LifecycleStageKind::CertifiedServe,
                ),
                (
                    LifecycleWorkClass::ProducerTurn,
                    LifecycleStageKind::ProducerTurn,
                ),
            ])
            .enumerate()
        {
            let ordinal = u128::try_from(index + 1).expect("small classifier ordinal");
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                Some(TerminalOutcome::Cancelled),
                DurableContinuation::None,
            );
            assert!(
                classify_storage_only_record(&record).is_ok(),
                "terminal {work_class:?}/{stage_kind:?} should need no physical carrier"
            );
        }
        assert_eq!(LifecycleWorkClass::ALL.len(), 13);
        assert_eq!(LifecycleStageKind::ALL.len(), 22);
    }
    #[test]
    fn storage_only_classifier_rejects_a_class_stage_mismatch() {
        let record = classifier_record(
            3,
            LifecycleWorkClass::SignProposal,
            LifecycleStageKind::FetchBody,
            Some(TerminalOutcome::Cancelled),
            DurableContinuation::None,
        );
        assert!(matches!(
            classify_storage_only_record(&record),
            Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecordShape {
                ordinal: 3,
                work_class: LifecycleWorkClass::SignProposal,
                stage,
            }) if stage.kind() == LifecycleStageKind::FetchBody
        ));
    }
    #[test]
    fn storage_only_classifier_checks_validate_no_successor_before_terminality() {
        let record = classifier_record(
            7,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
            Some(TerminalOutcome::Advanced),
            DurableContinuation::AdvancedNoSuccessor,
        );
        let Err(LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome {
            ordinal,
            stage,
        }) = classify_storage_only_record(&record)
        else {
            panic!("terminal Validate/no-successor lost its typed body-outcome debt")
        };
        assert_eq!(ordinal, 7);
        assert_eq!(stage.kind(), LifecycleStageKind::ValidateBody);
    }
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        storage_only_assembler_source_is_sealed_and_exhaustive
    );
    fn terminal_validate_no_successor_ledger()
    -> (LifecycleLedgerV1, AuthenticatedValidateNoSuccessorRecovery) {
        let context = LifecycleContext::new(LifecycleDigest::new([0x81; 32]), 9);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ValidateBody,
            4,
        );
        let key = replay.key;
        let causal_root = CausalRoot::new(LifecycleDigest::new([0x83; 32]));
        let owner = OwnerId::new(causal_root, 1);
        let stage = replay.stage;
        let payload = replay.payload;
        let record = LifecycleLedgerRecordV1::new(
            key,
            owner,
            1,
            replay.work_class,
            stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            payload,
            replay.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate ledger record");
        let ledger = LifecycleLedgerV1::new(context, 1, vec![record], BTreeMap::new())
            .expect("construct exact terminal Validate ledger");
        let proof = AuthenticatedValidateNoSuccessorRecovery {
            key,
            causal_root,
            reconstruction_source: causal_root.digest(),
            stage,
            payload,
        };
        (ledger, proof)
    }
    #[test]
    fn terminal_validate_no_successor_requires_exact_recovery_coverage() {
        let (ledger, proof) = terminal_validate_no_successor_ledger();
        assert!(
            validate_terminal_validate_no_successor_recovery(&ledger, &BTreeMap::new()).is_err()
        );
        let exact = BTreeMap::from([(proof.key, proof)]);
        assert!(validate_terminal_validate_no_successor_recovery(&ledger, &exact).is_ok());
        let mut foreign = proof;
        foreign.reconstruction_source = LifecycleDigest::new([0x86; 32]);
        assert!(
            validate_terminal_validate_no_successor_recovery(
                &ledger,
                &BTreeMap::from([(foreign.key, foreign)]),
            )
            .is_err()
        );
        let mut substituted = proof;
        let DurablePayloadReference::BodyFrame(mut frame) = substituted.payload else {
            panic!("terminal Validate proof must retain one body frame");
        };
        frame.frame = LifecycleDigest::new([0x87; 32]);
        substituted.payload = DurablePayloadReference::BodyFrame(frame);
        assert!(
            validate_terminal_validate_no_successor_recovery(
                &ledger,
                &BTreeMap::from([(substituted.key, substituted)]),
            )
            .is_err()
        );
    }
}
