#[cfg(test)]
mod recovery_tests {
    use super::super::authority;
    #[cfg(feature = "bls")]
    use super::super::schema::DurableContinuationEdge;
    use super::super::schema::{CausalRoot, DurableContinuation, LifecycleStageKind, OwnerId};
    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::{
        v2_body_store::{V2BodyStore, ValidatedBodyReceipt},
        v2_chunks::encode_payload,
    };
    #[cfg(feature = "bls")]
    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    #[cfg(feature = "bls")]
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
        peer::PeerId,
    };
    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;
    #[cfg(feature = "bls")]
    use tempfile::TempDir;
    #[cfg(feature = "bls")]
    struct EmptyAuthenticatedPayloadFixture {
        context: LifecycleContext,
        verified: VerifiedHeightContext,
        root: TempDir,
        payload_store: CertifiedServePayloadStoreV1,
        payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: V2BodyStore,
        keys: Vec<KeyPair>,
    }
    #[cfg(feature = "bls")]
    fn empty_authenticated_payload_fixture() -> EmptyAuthenticatedPayloadFixture {
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
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_048_576,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 2,
            },
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
        let (payload_store, recovered) =
            CertifiedServePayloadStoreV1::open(&root.path().join("payload"), verified.context())
                .expect("open empty payload store");
        let payloads = recovered
            .authenticate(&verified, &keys[0], &body_store)
            .expect("authenticate empty payload cut");
        EmptyAuthenticatedPayloadFixture {
            context: super::super::projection::lifecycle_context(verified.context()),
            verified,
            root,
            payload_store,
            payloads,
            body_store,
            keys,
        }
    }
    #[cfg(feature = "bls")]
    fn empty_authenticated_payload_cut() -> (
        LifecycleContext,
        AuthenticatedCertifiedServePayloadRecoveryCut,
    ) {
        let EmptyAuthenticatedPayloadFixture {
            context, payloads, ..
        } = empty_authenticated_payload_fixture();
        (context, payloads)
    }
    #[cfg(feature = "bls")]
    fn terminal_validate_record_with_body_outcome(
        fixture: &mut EmptyAuthenticatedPayloadFixture,
        ordinal: u128,
        view: u64,
        rejected: bool,
    ) -> LifecycleLedgerRecordV1 {
        let wire_context = fixture.verified.context();
        let round = wire::ConsensusRound {
            context_id: wire_context.id(),
            height: wire_context.height,
            view,
        };
        let leader = wire_context.leader(view);
        let leader_index = usize::try_from(leader).expect("small fixture leader index");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000_u64.saturating_add(view),
            view,
        );
        let signature =
            SignatureOf::try_from_hash(fixture.keys[leader_index].private_key(), header.hash())
                .expect("sign terminal Validate body");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("encode terminal Validate body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(wire_context, round, subject, &canonical_wire)
            .expect("encode terminal Validate payload")
            .manifest()
            .clone();
        let receipt = fixture
            .body_store
            .store(manifest.clone(), canonical_wire)
            .expect("persist terminal Validate body");
        let replay = super::super::replay_authority::exact_local_body_record_fixture(
            fixture.context,
            crate::sumeragi::v2_core::EventTag::new(
                round.height,
                round.view,
                crate::sumeragi::v2_core::Generation::new(0),
            ),
            manifest,
            &receipt,
            LifecycleStageKind::ValidateBody,
        )
        .expect("exact stored body mints one canonical Validate fixture");
        if rejected {
            let _rejected = fixture
                .body_store
                .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        "deterministic terminal Validate rejection".to_owned(),
                    )
                })
                .expect("persist terminal Validate rejection");
        } else {
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = fixture
                .body_store
                .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                    Ok::<_, String>(commitment)
                })
                .expect("persist terminal Validate success");
        }
        let causal_root = CausalRoot::new(LifecycleDigest::new(
            [u8::try_from(ordinal).expect("small fixture ordinal"); 32],
        ));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate body-outcome record")
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
    fn live_sign_proposal_ledger(context: LifecycleContext) -> LifecycleLedgerV1 {
        let record = sign_proposal_record(context, 1, 0xC6, None);
        LifecycleLedgerV1::new(context, 1, vec![record], BTreeMap::new())
            .expect("construct live SignProposal ledger")
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
    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_seals_an_empty_exact_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = LifecycleLedgerV1::empty(context);
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger.clone(), payloads)
                .expect("empty storage census assembles exactly");
        assert_eq!(recovery.context, context);
        assert_eq!(recovery.authenticated_ledger, ledger);
        assert!(recovery.candidates.is_empty());
        assert!(recovery.validate_no_successor.is_empty());
        assert!(recovery.authenticates_opened_ledger(&ledger));
        let foreign = LifecycleLedgerV1::empty(LifecycleContext::new(
            LifecycleDigest::new([0xC7; 32]),
            context.height(),
        ));
        assert!(!recovery.authenticates_opened_ledger(&foreign));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_consumes_exact_success_and_rejection_outcomes() {
        let mut fixture = empty_authenticated_payload_fixture();
        let validated = terminal_validate_record_with_body_outcome(&mut fixture, 1, 0, false);
        let rejected = terminal_validate_record_with_body_outcome(&mut fixture, 2, 1, true);
        let ledger = LifecycleLedgerV1::new(
            fixture.context,
            2,
            vec![validated, rejected],
            BTreeMap::new(),
        )
        .expect("construct two-outcome terminal Validate ledger");
        let strict = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(
            ledger.clone(),
            fixture.payloads,
        )
        .expect_err("the body-free factory must reject terminal Validate tombstones");
        assert!(matches!(
            strict.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { .. }
        ));
        let LifecycleRecoveryAssemblyError {
            _serve_payloads: payloads,
            ..
        } = strict;
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_body_validation_outcomes(
                ledger.clone(),
                payloads,
                &mut fixture.body_store,
            )
            .expect("consume both exact terminal Validate outcomes");
        assert_eq!(recovery.authenticated_ledger, ledger);
        assert_eq!(recovery.validate_no_successor.len(), 2);
        assert!(recovery.candidates.is_empty());
        assert!(fixture.body_store.validated_recovery_catalog().is_empty());
    }
    #[cfg(feature = "bls")]
    #[test]
    fn terminal_validate_catalog_failure_restores_prior_exact_selection() {
        let mut fixture = empty_authenticated_payload_fixture();
        let first = terminal_validate_record_with_body_outcome(&mut fixture, 1, 0, false);
        let second = terminal_validate_record_with_body_outcome(&mut fixture, 2, 1, true);
        assert!(
            first.key().expect("decode first key") < second.key().expect("decode second key"),
            "the exact first claim must be selected before the substituted second claim"
        );
        let exact = LifecycleLedgerV1::new(
            fixture.context,
            2,
            vec![first.clone(), second.clone()],
            BTreeMap::new(),
        )
        .expect("construct exact two-outcome ledger");
        let DurablePayloadReference::BodyFrame(mut substituted_frame) = second
            .durable_payload()
            .expect("decode second terminal body frame")
        else {
            panic!("terminal Validate must retain a BodyFrame")
        };
        substituted_frame.frame = LifecycleDigest::new([0xCE; 32]);
        let substituted = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            second.key().expect("decode second key"),
            second.owner(),
            second.ordinal(),
            second.work_class().expect("decode second class"),
            second.stage().expect("decode second stage"),
            second.terminal().expect("decode second terminal"),
            second.reconstruction_source(),
            DurablePayloadReference::BodyFrame(substituted_frame),
            second.continuation().expect("decode second continuation"),
        )
        .expect("construct checksum-valid substituted body frame");
        assert!(matches!(
            LifecycleLedgerV1::new(
                fixture.context,
                2,
                vec![first, substituted],
                BTreeMap::new(),
            ),
            Err(super::super::ledger::LifecycleLedgerError::InvalidLedger(_))
        ));
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_body_validation_outcomes(
                exact,
                fixture.payloads,
                &mut fixture.body_store,
            )
            .expect("structural replay rejection leaves every exact outcome available");
        assert_eq!(recovery.validate_no_successor.len(), 2);
    }
    #[cfg(feature = "bls")]
    #[test]
    fn repaired_wal_sign_and_terminal_validate_outcome_assemble_together() {
        let mut fixture = empty_authenticated_payload_fixture();
        let terminal = terminal_validate_record_with_body_outcome(&mut fixture, 3, 3, true);
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                fixture.context,
                0xCF,
            )
            .expect("construct repaired WAL ledger fixture");
        let mut records = repaired.records().to_vec();
        records.push(terminal);
        let combined = LifecycleLedgerV1::new(fixture.context, 3, records, BTreeMap::new())
            .expect("construct repaired WAL plus terminal Validate ledger");
        let recovery = AuthenticatedLifecycleRecoveryCut::
            assemble_storage_only_with_recovered_wal_sign_and_body_validation_outcomes(
                combined.clone(),
                fixture.payloads,
                &mut fixture.body_store,
                &projection,
            )
            .expect("assemble repaired Sign and terminal outcome atomically");
        assert_eq!(recovery.authenticated_ledger, combined);
        assert_eq!(recovery.candidates.len(), 1);
        assert_eq!(recovery.validate_no_successor.len(), 1);
        assert!(recovery.owns_recovered_wal_sign(&projection));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_failure_retains_frame_and_payload_authority() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = live_sign_proposal_ledger(context);
        let expected = ledger.clone();
        let error = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger, payloads)
            .expect_err("live SignProposal lacks storage-only replay authority");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 1,
                work_class: LifecycleWorkClass::SignProposal,
                stage,
            } if stage.kind() == LifecycleStageKind::SignProposal
        ));
        assert_eq!(error._authenticated_ledger, expected);
        assert!(error._serve_payloads.is_empty());
        assert_eq!(
            digest_bytes(error._serve_payloads.context_id().0.as_ref()),
            context.id()
        );
    }
    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_still_rejects_repaired_wal_sign_child() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (_projection, ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD0,
            )
            .expect("construct repaired WAL ledger fixture");
        let error = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger, payloads)
            .expect_err("unqualified storage-only recovery must reject the live Sign child");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 2,
                work_class: LifecycleWorkClass::SignVote,
                stage,
            } if stage.kind() == LifecycleStageKind::SignPrepareVote
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_seals_exact_repaired_child_and_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD1,
            )
            .expect("construct repaired WAL ledger fixture");
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger.clone(),
                payloads,
                &projection,
            )
            .expect("exact repaired Sign child must assemble");
        assert_eq!(recovery.authenticated_ledger, ledger);
        assert!(projection.owns_spliced_candidates(&recovery.candidates));
        assert!(recovery.authenticates_opened_ledger(&ledger));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_foreign_live_sign_child() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, _own_ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD2,
            )
            .expect("construct installed WAL projection fixture");
        let (_foreign_projection, foreign_ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD3,
            )
            .expect("construct foreign repaired WAL ledger fixture");
        let expected = foreign_ledger.clone();
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                foreign_ledger,
                payloads,
                &projection,
            )
            .expect_err("foreign live Sign must not consume installed WAL authority");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 2,
                work_class: LifecycleWorkClass::SignVote,
                ..
            }
        ));
        assert_eq!(error._authenticated_ledger, expected);
        assert!(error._serve_payloads.is_empty());
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_exact_child_at_wrong_durable_ordinal() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD9,
            )
            .expect("construct repaired WAL ledger fixture");
        let parent = repaired.records().first().expect("repaired fixture parent");
        let child = repaired.records().get(1).expect("repaired fixture child");
        let displaced_parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            parent.key().expect("decode parent key"),
            parent.owner(),
            parent.ordinal(),
            parent.work_class().expect("decode parent class"),
            parent.stage().expect("decode parent stage"),
            parent.terminal().expect("decode parent terminal"),
            parent.reconstruction_source(),
            parent.durable_payload().expect("decode parent payload"),
            DurableContinuation::successor(DurableContinuationEdge::ValidateToSignPrepare, 3),
        )
        .expect("construct displaced repaired parent");
        let filler = sign_proposal_record(context, 2, 0xDA, Some(TerminalOutcome::Cancelled));
        let displaced_child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            child.key().expect("decode child key"),
            child.owner(),
            3,
            child.work_class().expect("decode child class"),
            child.stage().expect("decode child stage"),
            child.terminal().expect("decode child terminal"),
            child.reconstruction_source(),
            child.durable_payload().expect("decode child payload"),
            child.continuation().expect("decode child continuation"),
        )
        .expect("construct displaced repaired child");
        let ledger = LifecycleLedgerV1::new(
            context,
            3,
            vec![displaced_parent, filler, displaced_child],
            BTreeMap::new(),
        )
        .expect("construct valid wrong-ordinal repaired frame");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("semantic child at a foreign durable address must fail closed");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(message)
                if message.contains("owner, ordinal")
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_an_extra_live_ordinary_row() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD4,
            )
            .expect("construct repaired WAL ledger fixture");
        let mut records = repaired.records().to_vec();
        records.push(sign_proposal_record(context, 3, 0xD5, None));
        let ledger = LifecycleLedgerV1::new(context, 3, records, BTreeMap::new())
            .expect("construct repaired ledger with extra live work");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("one opaque WAL projection cannot authorize extra live work");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 3,
                work_class: LifecycleWorkClass::SignProposal,
                ..
            }
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_exact_child_with_foreign_first_owner_row() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xDB,
            )
            .expect("construct repaired WAL ledger fixture");
        let child = repaired
            .records()
            .get(1)
            .expect("repaired fixture Sign child")
            .clone();
        let owner = child.owner();
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignProposal,
            1,
        );
        let foreign_parent = LifecycleLedgerRecordV1::new(
            replay.key,
            owner,
            1,
            replay.work_class,
            replay.stage,
            Some(TerminalOutcome::Cancelled),
            owner.causal_root().digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::None,
        )
        .expect("construct same-owner foreign first row");
        let ledger = LifecycleLedgerV1::new(
            context,
            child.ordinal(),
            vec![foreign_parent, child],
            BTreeMap::new(),
        )
        .expect("construct structurally valid child-only repaired impostor");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("exact Sign child cannot replace its typed Validate parent edge");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(message)
                if message.contains("terminal Validate parent")
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_pre_repair_live_validate() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD6,
            )
            .expect("construct repaired WAL ledger fixture");
        let repaired_parent = repaired
            .records()
            .first()
            .expect("repaired fixture retains its Validate parent");
        let live_parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            repaired_parent.key().expect("decode parent key"),
            repaired_parent.owner(),
            repaired_parent.ordinal(),
            repaired_parent.work_class().expect("decode parent class"),
            repaired_parent.stage().expect("decode parent stage"),
            None,
            repaired_parent.reconstruction_source(),
            repaired_parent
                .durable_payload()
                .expect("decode parent payload"),
            DurableContinuation::None,
        )
        .expect("construct pre-repair live Validate parent");
        let ledger = LifecycleLedgerV1::new(context, 1, vec![live_parent], BTreeMap::new())
            .expect("construct pre-repair WAL ledger");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("post-repair factory must not authorize the old live Validate parent");
        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 1,
                work_class: LifecycleWorkClass::Validate,
                ..
            }
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembled_cut_rejects_same_context_stale_reread() {
        let EmptyAuthenticatedPayloadFixture {
            context,
            verified,
            root,
            payload_store,
            payloads,
            ..
        } = empty_authenticated_payload_fixture();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD7,
            )
            .expect("construct repaired WAL ledger fixture");
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                repaired.clone(),
                payloads,
                &projection,
            )
            .expect("assemble exact production-shaped repaired cut");
        let mut stale_records = repaired.records().to_vec();
        stale_records.push(sign_proposal_record(
            context,
            3,
            0xD8,
            Some(TerminalOutcome::Cancelled),
        ));
        let stale = LifecycleLedgerV1::new(context, 3, stale_records, BTreeMap::new())
            .expect("construct valid same-context stale frame");
        assert!(!recovery.authenticates_opened_ledger(&stale));
        let ledger_root = root.path().join("ledger");
        let (ledger_store, opened) = LifecycleLedgerStoreV1::open(&ledger_root, context)
            .expect("open stale-reread ledger store");
        assert_eq!(opened, LifecycleLedgerV1::empty(context));
        ledger_store
            .persist(&stale)
            .expect("persist same-context stale frame");
        let authority = authority::recovered_wal_test_authority(&verified)
            .expect("construct focused recovered-WAL authority");
        let prepared = LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            &ledger_root,
            &payload_store,
            &recovery,
        );
        let Err(LifecycleOpenError(LifecycleOpenErrorKind::InvalidRecovery(message))) = prepared
        else {
            panic!("durable open did not reject the changed authenticated ledger frame")
        };
        assert_eq!(
            message,
            "lifecycle ledger changed after recovery-cut authentication"
        );
    }
    #[cfg(feature = "bls")]
    #[test]
    fn prepared_open_rejects_same_store_drift_without_overwrite() {
        let EmptyAuthenticatedPayloadFixture {
            context,
            verified,
            root,
            mut payload_store,
            payloads,
            ..
        } = empty_authenticated_payload_fixture();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD9,
            )
            .expect("construct prepared-open repaired WAL fixture");
        let mut recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                repaired.clone(),
                payloads,
                &projection,
            )
            .expect("assemble exact prepared-open recovery cut");
        let ledger_root = root.path().join("ledger");
        let (ledger_store, opened) = LifecycleLedgerStoreV1::open(&ledger_root, context)
            .expect("open prepared-open ledger store");
        assert_eq!(opened, LifecycleLedgerV1::empty(context));
        ledger_store
            .persist(&repaired)
            .expect("persist prepared-open predecessor frame");
        let authority = authority::recovered_wal_test_authority(&verified)
            .expect("construct prepared-open recovered-WAL authority");
        let prepared = LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            &ledger_root,
            &payload_store,
            &recovery,
        )
        .expect("prepare against the exact predecessor frame");
        let drift = LifecycleLedgerV1::empty(context);
        ledger_store
            .persist(&drift)
            .expect("replace the predecessor after preparation");
        let error = prepared
            .commit(&mut payload_store, &mut recovery)
            .expect_err("commit must not overwrite a changed predecessor frame")
            .into_error();
        let LifecycleOpenError(LifecycleOpenErrorKind::Ledger(
            LifecycleLedgerError::InvalidLedger(message),
        )) = error
        else {
            panic!("prepare-to-commit drift returned the wrong error")
        };
        assert_eq!(
            message,
            "attached lifecycle ledger changed before successor publication"
        );
        assert_eq!(
            ledger_store
                .load()
                .expect("reload the externally changed predecessor"),
            drift,
            "failed exact-successor publication must not overwrite the changed frame"
        );
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
        assert_eq!(replay.work_class, work_class);
        let causal_root = CausalRoot::new(LifecycleDigest::new(
            [u8::try_from(ordinal).expect("small classifier marker"); 32],
        ));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            work_class,
            replay.stage,
            terminal,
            causal_root.digest(),
            replay.payload,
            replay.authority,
            continuation,
        )
        .expect("construct classifier-only durable record")
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
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(storage_only_assembler_source_is_sealed_and_exhaustive);
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
