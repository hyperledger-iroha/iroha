#[cfg(test)]
mod certified_merge_inclusion_tests {
    use iroha_crypto::{Hash, HashOf, KeyPair, MerkleProof, MerkleTree};
    use norito::codec::DecodeAll as _;

    use super::*;
    use crate::{
        account::AccountId,
        block::CertifiedMergeLedgerReference,
        merge::{MergeQuorumCertificate, MergeSignerProof},
        peer::PeerId,
        transaction::{
            TransactionBuilder,
            signed::{TransactionEntrypoint, TransactionResult},
        },
        trigger::DataTriggerSequence,
    };

    fn assert_committed_transaction_roundtrip(committed: &CommittedTransaction) {
        let encoded = committed.encode();
        let decoded = CommittedTransaction::decode_all(&mut encoded.as_slice())
            .expect("canonical committed transaction must decode");
        assert_eq!(decoded, *committed);
    }

    fn test_network_id() -> crate::NetworkId {
        crate::NetworkId::from_genesis_hash(
            HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x15; Hash::LENGTH],
            )),
        )
    }

    fn certified_merge_fixture() -> (CertifiedMergeLedgerReference, CommittedTransaction) {
        let key_pair = KeyPair::random();
        let authority = AccountId::new(key_pair.public_key().clone());
        let signed = TransactionBuilder::new(
            test_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions::<crate::isi::InstructionBox>([])
        .sign(key_pair.private_key());
        let entrypoint = TransactionEntrypoint::External(signed);
        let result = TransactionResult::from(Ok(DataTriggerSequence::default()));
        let entrypoint_hash = entrypoint.hash();
        let result_hash = result.hash();
        let entrypoint_tree: MerkleTree<TransactionEntrypoint> =
            [entrypoint_hash].into_iter().collect();
        let result_tree: MerkleTree<TransactionResult> = [result_hash].into_iter().collect();
        let entrypoint_merkle_root = entrypoint_tree.root().expect("non-empty entrypoint tree");
        let result_merkle_root = result_tree.root().expect("non-empty result tree");
        let merge_entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"merge-entry"));
        let execution_batch_hash = Hash::new(b"merge-batch");
        let validators = Vec::<PeerId>::new();
        let reference = CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: merge_entry_hash,
            encoded_len: 1,
            epoch_id: 7,
            execution_batch_hash: Some(execution_batch_hash),
            entrypoint_count: Some(1),
            entrypoint_merkle_root: Some(entrypoint_merkle_root),
            result_merkle_root: Some(result_merkle_root),
            base_state_height: Some(4),
            base_state_hash: Some(HashOf::from_untyped_unchecked(Hash::new(b"base-state"))),
            merge_qc: MergeQuorumCertificate::new(
                0,
                7,
                5,
                HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent")),
                Hash::new(b"chain"),
                1,
                HashOf::new(&validators),
                validators,
                Vec::new(),
                Vec::<MergeSignerProof>::new(),
                Vec::new(),
                Hash::new(b"message"),
            ),
        };
        let inclusion = CertifiedMergeTransactionInclusion {
            version: 1,
            merge_entry_hash,
            merge_epoch_id: 7,
            execution_batch_hash,
            entrypoint_count: 1,
            entrypoint_merkle_root,
            result_merkle_root,
        };
        let committed = CommittedTransaction {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"carrier-block")),
            entrypoint_hash,
            entrypoint_proof: entrypoint_tree.get_proof(0).expect("entrypoint proof"),
            entrypoint,
            result_hash,
            result_proof: result_tree.get_proof(0).expect("result proof"),
            result,
            merge_inclusion: Some(inclusion),
        };
        (reference, committed)
    }

    #[test]
    fn certified_merge_inclusion_verifies_exact_reference_and_parallel_proofs() {
        let (reference, committed) = certified_merge_fixture();

        assert!(committed.verify_certified_merge_inclusion(&reference));
        assert_committed_transaction_roundtrip(&committed);

        let mut ordinary = committed.clone();
        ordinary.merge_inclusion = None;
        assert_committed_transaction_roundtrip(&ordinary);
        assert!(
            !ordinary.verify_certified_merge_inclusion(&reference),
            "ordinary transactions must not verify as certified merge inclusions"
        );

        #[cfg(feature = "transparent_api")]
        {
            let carrier_header = BlockHeader::new(
                core::num::NonZeroU64::new(5).expect("non-zero carrier height"),
                Some(HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent"))),
                None,
                None,
                10,
                0,
            );
            let mut carrier_builder = crate::block::builder::BlockBuilder::new(carrier_header);
            carrier_builder.set_execution_context(Some(
                crate::block::BlockExecutionContextBundle::new(Vec::new())
                    .with_merge_entry(reference.clone()),
            ));
            let carrier = carrier_builder.build(std::collections::BTreeSet::default());
            let mut block_bound = committed.clone();
            block_bound.block_hash = carrier.hash();
            assert!(block_bound.verify_certified_merge_inclusion_in_block(&carrier));
            assert!(block_bound.verify_inclusion_in_block(&carrier));

            let other_header = BlockHeader::new(
                core::num::NonZeroU64::new(5).expect("non-zero carrier height"),
                Some(HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent"))),
                None,
                None,
                11,
                0,
            );
            let mut other_builder = crate::block::builder::BlockBuilder::new(other_header);
            other_builder.set_execution_context(Some(
                crate::block::BlockExecutionContextBundle::new(Vec::new())
                    .with_merge_entry(reference.clone()),
            ));
            let other_carrier = other_builder.build(std::collections::BTreeSet::default());
            assert!(
                !block_bound.verify_certified_merge_inclusion_in_block(&other_carrier),
                "a valid proof and copied reference must not verify against a different block hash"
            );
            assert!(!block_bound.verify_inclusion_in_block(&other_carrier));
        }

        let mut wrong_reference = reference.clone();
        wrong_reference.entrypoint_count = Some(2);
        assert!(!committed.verify_certified_merge_inclusion(&wrong_reference));

        let mut ambiguous_count_reference = reference.clone();
        ambiguous_count_reference.entrypoint_count = Some(2);
        let mut ambiguous_count = committed.clone();
        ambiguous_count
            .merge_inclusion
            .as_mut()
            .expect("merge inclusion")
            .entrypoint_count = 2;
        assert!(
            !ambiguous_count.verify_certified_merge_inclusion(&ambiguous_count_reference),
            "a one-leaf proof must not be rebound to a two-leaf certified count"
        );

        let oversized_leaf_count = (1_u64 << u32::BITS) + 1;
        let mut oversized_reference = reference.clone();
        oversized_reference.entrypoint_count = Some(oversized_leaf_count);
        let mut oversized = committed.clone();
        oversized
            .merge_inclusion
            .as_mut()
            .expect("merge inclusion")
            .entrypoint_count = oversized_leaf_count;
        assert!(
            !oversized.verify_certified_merge_inclusion(&oversized_reference),
            "certified merge proofs must reject counts outside the u32 block-proof index space"
        );

        let mut wrong_version = reference.clone();
        wrong_version.version = 2;
        assert!(!committed.verify_certified_merge_inclusion(&wrong_version));

        let mut misaligned = committed;
        misaligned.result_proof = MerkleProof::from_audit_path(1, Vec::new());
        assert!(!misaligned.verify_certified_merge_inclusion(&reference));
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn ordinary_committed_transaction_verifies_against_exact_carrier_block() {
        let (_, fixture) = certified_merge_fixture();
        let TransactionEntrypoint::External(signed) = fixture.entrypoint else {
            panic!("fixture must contain an external transaction");
        };
        let result_inner = (*fixture.result).clone();
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            10,
            0,
        );
        let mut builder = crate::block::builder::BlockBuilder::new(header);
        builder.push_transaction(signed);
        builder.push_result(result_inner);
        let carrier = builder.build(std::collections::BTreeSet::default());
        let ordinary = CommittedTransaction {
            block_hash: carrier.hash(),
            entrypoint_hash: carrier.entrypoint_hashes().next().expect("entrypoint hash"),
            entrypoint_proof: carrier
                .entrypoint_proofs()
                .next()
                .expect("entrypoint proof"),
            entrypoint: carrier.entrypoints_cloned().next().expect("entrypoint"),
            result_hash: carrier.result_hashes().next().expect("result hash"),
            result_proof: carrier.result_proofs().next().expect("result proof"),
            result: carrier.results().next().cloned().expect("result"),
            merge_inclusion: None,
        };

        assert!(ordinary.verify_inclusion_in_block(&carrier));

        let mut wrong_hash = ordinary.clone();
        wrong_hash.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong-entrypoint"));
        assert!(!wrong_hash.verify_inclusion_in_block(&carrier));

        let other_header = BlockHeader::new(
            core::num::NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            11,
            0,
        );
        let other_carrier = crate::block::builder::BlockBuilder::new(other_header)
            .build(std::collections::BTreeSet::default());
        assert!(!ordinary.verify_inclusion_in_block(&other_carrier));
    }
}

#[cfg(all(test, feature = "fault_injection"))]
mod fault_injection_tests {
    use std::str::FromStr;

    use iroha_crypto::{Hash, HashOf, MerkleProof};

    use super::*;
    use crate::{
        AssetDefinitionId, Level,
        events::data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferOutcome},
        isi::{InstructionBox, Log},
        prelude::{DataTriggerSequence, Quantity, TimeTriggerEntrypoint, TransactionResult},
        trigger::TriggerId,
    };

    fn zero_hash<T>() -> HashOf<T> {
        let zero = [0u8; 32];
        HashOf::from_untyped_unchecked(Hash::prehashed(zero))
    }

    fn make_time_committed_tx() -> CommittedTransaction {
        let entry = TransactionEntrypoint::Time(TimeTriggerEntrypoint {
            id: TriggerId::from_str("fault_trigger").expect("valid trigger id"),
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority: AccountId::parse_encoded(
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("valid authority"),
        });

        let result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        CommittedTransaction {
            block_hash: zero_hash(),
            entrypoint_hash: entry.hash(),
            entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
            entrypoint: entry,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, vec![]),
            result,
            merge_inclusion: None,
        }
    }

    #[test]
    fn time_entrypoint_injection_appends_instructions() {
        let mut tx = make_time_committed_tx();
        let original_hash = tx.entrypoint_hash;

        let injected: InstructionBox = Log {
            level: Level::WARN,
            msg: "timer tamper".into(),
        }
        .into();

        tx.inject_instructions([injected.clone()]);

        assert_ne!(
            tx.entrypoint_hash, original_hash,
            "entrypoint hash must reflect injected instructions"
        );

        let instructions = match &tx.entrypoint {
            TransactionEntrypoint::Time(entry) => entry.instructions.0.clone().into_vec(),
            _ => panic!("expected time entrypoint"),
        };
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0], injected);
    }

    #[test]
    fn result_swap_preserves_independent_batch_receipts() {
        let mut tx = make_time_committed_tx();
        let authority = match &tx.entrypoint {
            TransactionEntrypoint::Time(entrypoint) => entrypoint.authority.clone(),
            _ => panic!("expected time entrypoint"),
        };
        let outcome = AssetBatchTransferOutcome {
            leg_index: 0,
            leg_id: "fault-injection-leg".to_owned(),
            asset: AssetId::new(
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    Name::from_str("rose").expect("asset name"),
                ),
                authority.clone(),
            ),
            destination: authority,
            amount: Quantity::from(1_u32),
            status: AssetBatchTransferLegStatus::Applied,
        };
        tx.result.set_batch_transfer_outcomes(vec![outcome.clone()]);
        tx.result_hash = tx.result.hash();

        let original_result_hash = tx.result_hash.clone();
        let original_result_proof = tx.result_proof.clone();
        tx.swap_result();

        assert!(tx.result.0.is_err());
        assert_eq!(tx.result.batch_transfer_outcomes(), &[outcome]);
        assert_eq!(tx.result_hash, tx.result.hash());
        assert_ne!(tx.result_hash, original_result_hash);
        assert_eq!(tx.result_proof, original_result_proof);
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::KeyPair;
    use norito::json;

    use super::*;

    #[test]
    fn proof_backend_query_payload_roundtrips() {
        use norito::codec::{Decode, Encode};

        let query =
            proof::prelude::FindProofRecordsByBackend::new("test/nonexistent-proof-backend".into());
        let encoded = query.encode();
        assert!(
            !encoded.is_empty(),
            "backend query payload must carry the backend identifier"
        );

        let mut bytes = encoded.as_slice();
        let decoded =
            proof::prelude::FindProofRecordsByBackend::decode(&mut bytes).expect("decode query");
        assert!(bytes.is_empty(), "decoder must consume the whole payload");
        assert_eq!(decoded.backend, query.backend);
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one authoritative SoraFS vector keeps every singular V1 query payload roundtrip in registry order"
    )]
    fn sorafs_authoritative_singular_query_payloads_roundtrip() {
        use norito::codec::{Decode, Encode};

        let juror = AccountId::new(KeyPair::random().public_key().clone());
        let orderbook_cursor = crate::sorafs::orderbook::OrderbookFinalizedCursorV1 {
            height: 7,
            block_hash: [0xA7; 32],
        };
        let reserve_cursor = crate::sorafs::reserve::ReserveFinalizedCursorV1 {
            height: 7,
            block_hash: [0xB7; 32],
        };
        let pin_cursor = crate::sorafs::pin_registry::PinManifestFinalizedCursorV1 {
            height: 7,
            block_hash: [0xC7; 32],
        };
        let repair_cursor = crate::sorafs::moderation_ledger::RepairFinalizedCursorV1 {
            height: 8,
            block_hash: [0xA8; 32],
        };
        let proof_outcome_cursor = crate::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1 {
            height: 9,
            block_hash: [0xA9; 32],
        };
        let reputation_cursor = crate::sorafs::reputation::ReputationJournalFinalizedCursorV1 {
            height: 10,
            block_hash: [0xAA; 32],
            finalized_at_unix_ms: 1_700_000_010_000,
        };
        let queries: Vec<SingularQueryBox> = vec![
            sorafs::prelude::FindSorafsOrderbookPolicy.into(),
            sorafs::prelude::FindSorafsOrderbookOrderById::new([0x11; 32]).into(),
            sorafs::prelude::FindSorafsOrderbookCancellationByOrderId::new([0x12; 32]).into(),
            sorafs::prelude::FindSorafsOrderbookReceiptById::new([0x13; 32]).into(),
            sorafs::prelude::FindSorafsOrderbookTradeById::new([0x17; 32]).into(),
            sorafs::prelude::FindSorafsOrderbookChannelById::new([0x18; 32]).into(),
            sorafs::prelude::FindSorafsOrderbookStatus.into(),
            sorafs::prelude::FindSorafsOrderbookOrders::new(
                Some(orderbook_cursor),
                Some(crate::sorafs::orderbook::OrderbookOrderStatusV1::Open),
                Some([0x14; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsOrderbookReceipts::new(
                Some(orderbook_cursor),
                Some([0x15; 32]),
                Some([0x16; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsOrderbookTrades::new(
                Some(orderbook_cursor),
                Some([0x19; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsOrderbookChannels::new(
                Some(orderbook_cursor),
                Some(crate::sorafs::orderbook::OrderbookSettlementChannelStatusV1::Open),
                Some([0x1A; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsOrderbookEvents::new(
                Some(orderbook_cursor),
                Some(crate::sorafs::orderbook::OrderbookFinalizedEventCursorV1 {
                    sequence: 2,
                    block_height: 7,
                    block_hash: [0xA7; 32],
                    event_index: 1,
                }),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsReservePolicy.into(),
            sorafs::prelude::FindSorafsReserveProviderById::new(
                crate::sorafs::capacity::ProviderId::new([0x1B; 32]),
            )
            .into(),
            sorafs::prelude::FindSorafsReserveMovementById::new([0x1C; 32]).into(),
            sorafs::prelude::FindSorafsReserveAppealById::new([0x1D; 32]).into(),
            sorafs::prelude::FindSorafsReserveProviders::new(
                Some(reserve_cursor),
                Some(crate::sorafs::capacity::ProviderId::new([0x1E; 32])),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsReserveMovements::new(
                Some(reserve_cursor),
                Some([0x1F; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsReserveAppeals::new(
                Some(reserve_cursor),
                Some([0x20; 32]),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsReserveEvents::new(
                Some(reserve_cursor),
                Some(crate::sorafs::reserve::ReserveFinalizedEventCursorV1 {
                    sequence: 3,
                    block_height: 7,
                    block_hash: reserve_cursor.block_hash,
                    event_index: 2,
                }),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsPopIssuerPolicy.into(),
            sorafs::prelude::FindSorafsPopCredentialCommitmentByDigest::new([0x21; 32]).into(),
            sorafs::prelude::FindSorafsPopCommitmentRootByVersion::new(2).into(),
            sorafs::prelude::FindSorafsPopRevocationPublicationByVersion::new(3).into(),
            sorafs::prelude::FindSorafsPopRevocationByNonceCommitment::new([0x22; 32]).into(),
            sorafs::prelude::FindSorafsPopAuditDigestBySequence::new(4).into(),
            sorafs::prelude::FindSorafsPopRegistryStatus.into(),
            sorafs::prelude::FindSorafsPinManifest::new(
                crate::sorafs::pin_registry::ManifestDigest::new([0x24; 32]),
                Some(pin_cursor),
            )
            .into(),
            sorafs::prelude::FindSorafsPinManifests::new(
                Some(pin_cursor),
                Some(crate::sorafs::pin_registry::PinStatusKindV1::Approved),
                Some(crate::sorafs::pin_registry::ManifestDigest::new([0x25; 32])),
                25,
                16 * 1024,
            )
            .into(),
            sorafs::prelude::FindSorafsRepairTask::new("REP-1".to_owned(), Some(repair_cursor))
                .into(),
            sorafs::prelude::FindSorafsRepairTasks::new(Some(repair_cursor), Some([0x23; 32]), 25)
                .into(),
            sorafs::prelude::FindSorafsRepairStatus::new(Some(repair_cursor)).into(),
            sorafs::prelude::FindSorafsRepairEvents::new(
                Some(repair_cursor),
                Some(
                    crate::sorafs::moderation_ledger::RepairFinalizedEventCursorV1 {
                        sequence: 6,
                        block_height: 8,
                        block_hash: [0xA8; 32],
                        event_index: 2,
                    },
                ),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsProofOutcome::new(
                crate::sorafs::proof_ledger::ProofOutcomeKindV1::Pdp,
                [0x24; 32],
                Some(proof_outcome_cursor),
            )
            .into(),
            sorafs::prelude::FindSorafsProofOutcomeEvents::new(
                Some(proof_outcome_cursor),
                Some(
                    crate::sorafs::proof_ledger::ProofOutcomeFinalizedEventCursorV1 {
                        sequence: 7,
                        block_height: 9,
                        block_hash: [0xA9; 32],
                        event_index: 0,
                    },
                ),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsReputationJournalAuthorityPolicy.into(),
            sorafs::prelude::FindSorafsReputationJournalEventBySourceId::new(
                crate::sorafs::reputation::ReputationJournalSourceIdV1([0x25; 32]),
                Some(reputation_cursor),
            )
            .into(),
            sorafs::prelude::FindSorafsReputationJournalEvents::new(
                Some(reputation_cursor),
                Some(
                    crate::sorafs::reputation::ReputationJournalFinalizedEventCursorV1 {
                        sequence: 8,
                        block_height: 10,
                        block_hash: reputation_cursor.block_hash,
                        event_index: 1,
                    },
                ),
                25,
            )
            .into(),
            sorafs::prelude::FindSorafsModerationPolicy.into(),
            sorafs::prelude::FindSorafsModerationAppeal::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationJurorEligibility::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                juror.clone(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationCase::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationCommit::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                juror.clone(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationReveal::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                juror.clone(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationChallenge::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationOutcome::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
            )
            .into(),
            sorafs::prelude::FindSorafsModerationNoShow::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                juror,
            )
            .into(),
            sorafs::prelude::FindSorafsModerationStatus.into(),
            sorafs::prelude::FindSorafsModerationSnapshot::new(64, 128).into(),
            sorafs::prelude::FindSorafsModerationEvents::new(
                crate::sorafs::moderation_ledger::ModerationFinalizedCursorV1 {
                    height: 9,
                    block_hash: [0x31; 32],
                },
                Some(
                    crate::sorafs::moderation_ledger::ModerationFinalizedEventCursorV1 {
                        sequence: 12,
                        block_height: 8,
                        block_hash: [0x32; 32],
                        event_index: 2,
                    },
                ),
                25,
            )
            .into(),
        ];

        for query in queries {
            let encoded = query.encode();
            let mut bytes = encoded.as_slice();
            let decoded = SingularQueryBox::decode(&mut bytes).expect("decode orderbook query");
            assert!(bytes.is_empty(), "decoder must consume the whole query");
            assert_eq!(decoded, query);
        }
    }

    #[test]
    fn query_output_batch_box_json_roundtrip() {
        let batch = QueryOutputBatchBox::String(vec!["hello".to_owned()]);

        let as_value = json::to_value(&batch).expect("serialize batch");
        assert_eq!(
            as_value,
            norito::json!({ "kind": "String", "content": ["hello"] })
        );

        let decoded: QueryOutputBatchBox = json::from_value(as_value).expect("deserialize batch");
        assert_eq!(decoded, batch);
    }

    #[test]
    fn query_response_iterable_json_roundtrip() {
        let cursor = parameters::ForwardCursor {
            query: "query-id".to_owned(),
            cursor: NonZeroU64::new(1).expect("nonzero"),
            gas_budget: None,
        };
        let output = QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![
                Numeric::from(42_u32),
            ])),
            remaining_items: Some(0),
            has_more: true,
            continue_cursor: Some(cursor),
        };
        let response = QueryResponse::Iterable(output.clone());

        let as_value = json::to_value(&response).expect("serialize response");
        let decoded: QueryResponse =
            json::from_value(as_value.clone()).expect("deserialize response");
        assert_eq!(decoded, response);

        // Ensure JSON structure exposes iterable wrapper with batch payload.
        match as_value {
            json::Value::Object(map) => {
                assert_eq!(map.get("kind"), Some(&norito::json!("Iterable")));
                assert!(map.contains_key("content"));
            }
            other => panic!("expected object for iterable response, got {other:?}"),
        }
    }
}
