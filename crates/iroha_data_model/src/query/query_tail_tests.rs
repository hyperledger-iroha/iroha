#[cfg(test)]
mod certified_merge_inclusion_tests {
    use super::*;
    use crate::{
        account::AccountId,
        block::CertifiedMergeLedgerReference,
        merge::{MergeQuorumCertificate, MergeSignerProof},
        transaction::{
            TransactionBuilder,
            signed::{TransactionEntrypoint, TransactionResult},
        },
        trigger::DataTriggerSequence,
    };
    use iroha_crypto::{Hash, HashOf, KeyPair, MerkleProof, MerkleTree};
    use iroha_model_base::peer::PeerId;
    use norito::codec::DecodeAll as _;
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
                test_network_id(),
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

    #[cfg(feature = "transparent_api")]
    fn authenticated_execution_fixture(merged: bool) -> (SignedBlock, CommittedTransaction) {
        let (reference, mut committed) = certified_merge_fixture();
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(5).expect("carrier height"),
            Some(reference.merge_qc.carrier_parent_hash),
            None,
            None,
            10,
            0,
        );
        let mut builder = crate::block::builder::BlockBuilder::new(header);
        if merged {
            builder.set_execution_context(Some(
                crate::block::BlockExecutionContextBundle::new(Vec::new())
                    .with_merge_entry(reference),
            ));
        } else {
            let TransactionEntrypoint::External(signed) = committed.entrypoint.clone() else {
                panic!("fixture external transaction");
            };
            builder.push_transaction(signed);
            builder.push_result((*committed.result).clone());
            committed.merge_inclusion = None;
        }
        let block = builder.build(std::collections::BTreeSet::default());
        committed.block_hash = block.hash();
        (block, committed)
    }

    #[cfg(feature = "transparent_api")]
    fn synthetic_execution_commitment(
        block: &SignedBlock,
    ) -> crate::block::consensus_v2::ExecutionCommitment {
        let wire = block.encode_wire().expect("fixture canonical wire");
        // Test-only supplied trust input; no consensus verification is claimed here.
        let mut commitment = crate::block::consensus_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"fixture parent state"),
            Hash::new(b"fixture post state"),
            Hash::new(b"fixture ordinary writes"),
            wire.len() as u64,
            Hash::new(&wire),
        );
        commitment.merge_carrier = block
            .execution_context()
            .and_then(|context| context.merge_entry.as_ref())
            .map(|reference| {
                crate::block::consensus_v2::MergeCarrierCommitmentV1::new(reference.entry_hash)
            });
        commitment
    }

    #[cfg(feature = "transparent_api")]
    fn committed_at(block: &SignedBlock, index: u32) -> CommittedTransaction {
        let entrypoint = block
            .entrypoint_cloned_at(index as usize)
            .expect("entrypoint");
        let result = block
            .results()
            .nth(index as usize)
            .cloned()
            .expect("result");
        CommittedTransaction {
            block_hash: block.hash(),
            entrypoint_hash: entrypoint.hash(),
            entrypoint_proof: block.entrypoint_proof(index).expect("entry proof"),
            entrypoint,
            result_hash: result.hash(),
            result_proof: block.result_proof(index).expect("result proof"),
            result,
            merge_inclusion: None,
        }
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn authenticated_execution_inclusion_binds_ordinary_and_merge_carriers() {
        for merged in [false, true] {
            let (block, committed) = authenticated_execution_fixture(merged);
            assert!(committed.verify_inclusion_in_authenticated_execution(
                &block,
                &synthetic_execution_commitment(&block)
            ));
            assert_committed_transaction_roundtrip(&committed);
            let wire = block.encode_wire().expect("canonical carrier");
            let decoded = crate::block::decode_framed_signed_block(&wire).expect("decode carrier");
            assert!(committed.verify_inclusion_in_authenticated_execution(
                &decoded,
                &synthetic_execution_commitment(&block)
            ));
            assert_eq!(block.external_entrypoint_count(), usize::from(!merged));
            let mut wrong_length = synthetic_execution_commitment(&block);
            wrong_length.executed_block_wire_len += 1;
            assert!(!committed.verify_inclusion_in_authenticated_execution(&block, &wrong_length));
            let mut wrong_hash = synthetic_execution_commitment(&block);
            wrong_hash.executed_block_wire_hash = Hash::new(b"different executed wire");
            assert!(!committed.verify_inclusion_in_authenticated_execution(&block, &wrong_hash));
            let mut wrong_merge = synthetic_execution_commitment(&block);
            wrong_merge.merge_carrier = if merged {
                None
            } else {
                Some(crate::block::consensus_v2::MergeCarrierCommitmentV1::new(
                    HashOf::from_untyped_unchecked(Hash::new(b"unexpected merge carrier")),
                ))
            };
            assert!(!committed.verify_inclusion_in_authenticated_execution(&block, &wrong_merge));
        }
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn authenticated_execution_inclusion_rejects_unbound_wire_and_header_material() {
        for merged in [false, true] {
            let (original, _) = authenticated_execution_fixture(merged);
            let (mut substituted, mut committed) = authenticated_execution_fixture(merged);
            substituted.replace_header_for_testing(original.header());
            committed.block_hash = original.hash();
            assert_eq!(substituted.hash(), original.hash());
            assert!(substituted.validate_entrypoint_merkle_cache().is_ok());
            assert!(substituted.validate_result_merkle_cache().is_ok());
            assert!(committed.verify_inclusion_in_block(&substituted));
            assert!(!committed.verify_inclusion_in_authenticated_execution(
                &substituted,
                &synthetic_execution_commitment(&original)
            ));
            assert!(
                !committed.verify_inclusion_in_authenticated_execution(
                    &substituted,
                    &synthetic_execution_commitment(&substituted)
                ),
                "rebinding synthetic wire does not admit invalid header commitments"
            );
        }

        let (original, committed) = authenticated_execution_fixture(false);
        let rejected = TransactionResult::from(Err(
            crate::transaction::error::TransactionRejectionReason::Validation(
                crate::ValidationFail::NotPermitted("substituted result".into()),
            ),
        ));
        let result_tree: MerkleTree<TransactionResult> = [rejected.hash()].into_iter().collect();
        let mut value = norito::json::to_value(&original).expect("carrier JSON");
        let result = value
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap();
        result.insert(
            "transaction_results".into(),
            norito::json::to_value(&vec![rejected]).unwrap(),
        );
        result.insert(
            "result_merkle".into(),
            norito::json::to_value(&result_tree).unwrap(),
        );
        let substituted: SignedBlock =
            norito::json::from_value(value).expect("substituted result carrier");
        assert_eq!(substituted.header(), original.header());
        assert!(substituted.validate_result_merkle_cache().is_ok());
        assert!(committed.verify_inclusion_in_block(&substituted));
        assert!(!committed.verify_inclusion_in_authenticated_execution(
            &substituted,
            &synthetic_execution_commitment(&original)
        ));
        assert!(!committed.verify_inclusion_in_authenticated_execution(
            &substituted,
            &synthetic_execution_commitment(&substituted)
        ));

        let mut rewritten_result = original.clone();
        let entry_hashes = rewritten_result.entrypoint_hashes().collect::<Vec<_>>();
        rewritten_result
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![Err(
                    crate::transaction::error::TransactionRejectionReason::Validation(
                        crate::ValidationFail::NotPermitted(
                            "rewritten result and header root".into(),
                        ),
                    ),
                )],
            )
            .expect("structurally valid alternate execution result");
        let rewritten_committed = committed_at(&rewritten_result, 0);
        assert_eq!(
            rewritten_result.hash(),
            original.hash(),
            "consensus hash excludes result root"
        );
        assert_ne!(
            rewritten_result.header().result_merkle_root(),
            original.header().result_merkle_root()
        );
        assert!(rewritten_committed.verify_inclusion_in_block(&rewritten_result));
        assert!(
            !rewritten_committed.verify_inclusion_in_authenticated_execution(
                &rewritten_result,
                &synthetic_execution_commitment(&original)
            ),
            "matching header hash cannot substitute for authenticated executed wire"
        );

        let (merge, merge_committed) = authenticated_execution_fixture(true);
        let mut context = merge.execution_context().unwrap().clone();
        context.version += 1;
        let mut builder = crate::block::builder::BlockBuilder::new(merge.header());
        builder.set_execution_context(Some(context));
        let unsupported = builder.build(std::collections::BTreeSet::default());
        let mut unsupported_committed = merge_committed.clone();
        unsupported_committed.block_hash = unsupported.hash();
        assert!(unsupported_committed.verify_inclusion_in_block(&unsupported));
        assert!(
            !unsupported_committed.verify_inclusion_in_authenticated_execution(
                &unsupported,
                &synthetic_execution_commitment(&unsupported)
            )
        );

        for mut block in [original.clone(), merge.clone()] {
            let mut header = block.header();
            header.set_execution_context_hash(if block.execution_context().is_some() {
                None
            } else {
                merge.header().execution_context_hash()
            });
            block.replace_header_for_testing(header);
            let mut evidence = if block.execution_context().is_some() {
                merge_committed.clone()
            } else {
                committed.clone()
            };
            evidence.block_hash = block.hash();
            assert!(!evidence.verify_inclusion_in_authenticated_execution(
                &block,
                &synthetic_execution_commitment(&block)
            ));
        }

        let mut broken_cache = norito::json::to_value(&original).unwrap();
        broken_cache
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                "merkle".into(),
                norito::json::to_value(&MerkleTree::<TransactionEntrypoint>::default()).unwrap(),
            );
        let broken_cache: SignedBlock = norito::json::from_value(broken_cache).unwrap();
        assert!(broken_cache.validate_entrypoint_merkle_cache().is_err());
        assert!(!committed.verify_inclusion_in_authenticated_execution(
            &broken_cache,
            &synthetic_execution_commitment(&broken_cache)
        ));
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn authenticated_execution_inclusion_binds_time_and_exact_indices() {
        let (mut block, original) = authenticated_execution_fixture(false);
        let scheduled = crate::trigger::TimeTriggerEntrypoint {
            id: "header_proof_trigger".parse().expect("trigger id"),
            instructions: crate::transaction::ExecutionStep(
                Vec::<crate::isi::InstructionBox>::new().into(),
            ),
            authority: match &original.entrypoint {
                TransactionEntrypoint::External(transaction) => transaction.authority().clone(),
                _ => unreachable!(),
            },
        };
        let entry_hashes = [original.entrypoint_hash, scheduled.hash_as_entrypoint()];
        block
            .set_transaction_results(
                vec![scheduled],
                &entry_hashes,
                vec![
                    Ok(DataTriggerSequence::default()),
                    Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("native mixed external/time results");
        let external = committed_at(&block, 0);
        let time = committed_at(&block, 1);
        assert!(external.verify_inclusion_in_authenticated_execution(
            &block,
            &synthetic_execution_commitment(&block)
        ));
        assert!(
            time.verify_inclusion_in_block(&block),
            "trusted-full-wire generic capability remains available"
        );
        assert!(
            time.verify_inclusion_in_authenticated_execution(
                &block,
                &synthetic_execution_commitment(&block)
            ),
            "authenticated full wire binds scheduled entrypoints too"
        );
        assert_ne!(block.full_entry_merkle_root(), block.header().merkle_root());

        let mut wrong_index = external;
        wrong_index.entrypoint_proof = time.entrypoint_proof;
        wrong_index.result_proof = time.result_proof;
        assert!(!wrong_index.verify_inclusion_in_authenticated_execution(
            &block,
            &synthetic_execution_commitment(&block)
        ));
        let mut no_results = block.canonical_resultless_proposal();
        wrong_index.block_hash = no_results.hash();
        assert!(!wrong_index.verify_inclusion_in_authenticated_execution(
            &no_results,
            &synthetic_execution_commitment(&block)
        ));
        no_results.replace_header_for_testing(block.header());
        assert!(!original.verify_inclusion_in_authenticated_execution(
            &no_results,
            &synthetic_execution_commitment(&block)
        ));
    }
}
#[cfg(all(test, feature = "fault_injection"))]
mod fault_injection_tests {
    use super::*;
    use crate::{
        AssetDefinitionId, Level,
        events::data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferOutcome},
        isi::{InstructionBox, Log},
        prelude::{DataTriggerSequence, Quantity, TimeTriggerEntrypoint, TransactionResult},
        trigger::TriggerId,
    };
    use iroha_crypto::{Hash, HashOf, MerkleProof};
    use std::str::FromStr;
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
        let original_result_hash = tx.result_hash;
        let original_result_proof = tx.result_proof.clone();
        tx.swap_result();
        assert!(tx.result.0.is_err());
        assert_eq!(tx.result.batch_transfer_outcomes(), &[outcome]);
        assert_eq!(tx.result_hash, tx.result.hash());
        assert_ne!(tx.result_hash, original_result_hash);
        assert_eq!(tx.result_proof, original_result_proof);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use norito::json;
    use std::num::NonZeroU64;
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
            sorafs::prelude::FindSorafsCitizenBondBySerialCommitment::new([0x21; 32]).into(),
            sorafs::prelude::FindSorafsCitizenBondSnapshot.into(),
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
