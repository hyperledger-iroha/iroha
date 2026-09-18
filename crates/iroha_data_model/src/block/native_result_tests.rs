// Pure wire/output fixtures; neither signatures nor these claims mint State authority.

fn native_model_transcript(call: Hash) -> crate::fastpq::TransferTranscript {
    crate::fastpq::TransferTranscript {
        batch_hash: call,
        deltas: vec![],
        authority_digest: Hash::new(b"model-only authority digest"),
        poseidon_preimage_digest: None,
    }
}

fn native_model_result() -> crate::transaction::signed::TransactionResult {
    crate::transaction::signed::TransactionResult::new(Ok(Default::default()))
}

fn native_model_result_fixture(
    sealed: bool,
) -> (
    crate::block::SignedBlock,
    crate::trigger::TimeTriggerEntrypoint,
    Vec<crate::transaction::signed::TransactionResult>,
    std::collections::BTreeMap<Hash, Vec<crate::fastpq::TransferTranscript>>,
) {
    let mut batch = decision_batch_fixture();
    if sealed {
        let payload = &mut batch.groups[0].payload;
        let TransactionEntrypoint::External(signed) = payload.input.entrypoint.clone() else {
            unreachable!()
        };
        payload.input.entrypoint = TransactionEntrypoint::SealedReveal(
            crate::transaction::signed::SealedTransactionReveal::new(
                Hash::new(b"model sealed commitment"),
                signed,
                [7; 32],
            ),
        );
        let binding = &mut payload.input.certificate.binding;
        binding.entrypoint_hash = payload.input.entrypoint.hash();
        binding.request_id =
            queue_plan_synced_request_id(&binding_fixture().0, binding.entrypoint_hash);
        payload.descriptor.admitted_input_hash =
            Hash::new(norito::encode_canonical(&payload.input).unwrap());
        batch.groups[0] = decision_group_fixture(payload.clone());
    }
    let entry = &batch.groups[0].payload.input.entrypoint;
    let call = Hash::from(entry.execution_call_hash());
    let authority = entry.authority().clone();
    let native = native_model_transcript(call);
    let time = crate::trigger::TimeTriggerEntrypoint {
        id: "native-model-maintenance".parse().unwrap(),
        instructions: crate::transaction::ExecutionStep(
            iroha_primitives::const_vec::ConstVec::new_empty(),
        ),
        authority,
    };
    let protocol = Hash::new(b"typed protocol call authenticated later by Core");
    let transcripts = std::collections::BTreeMap::from([
        (call, vec![native]),
        (protocol, vec![native_model_transcript(protocol)]),
    ]);
    let mut builder = crate::block::builder::BlockBuilder::new(decision_batch_header(&batch));
    builder.set_execution_context(Some(
        crate::block::BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    let block = builder.build(Default::default());
    assert!(block.is_resultless_proposal());
    (
        block,
        time,
        vec![native_model_result(), native_model_result()],
        transcripts,
    )
}

fn attach_native_model_results(
    block: &mut crate::block::SignedBlock,
    time: crate::trigger::TimeTriggerEntrypoint,
    results: Vec<crate::transaction::signed::TransactionResult>,
    transcripts: std::collections::BTreeMap<Hash, Vec<crate::fastpq::TransferTranscript>>,
) -> Result<(), crate::block::SetTransactionResultsError> {
    let hashes = block
        .network_entrypoints()
        .map(TransactionEntrypoint::hash)
        .chain(std::iter::once(time.hash_as_entrypoint()))
        .collect::<Vec<_>>();
    block.set_full_transaction_results_with_transcripts(
        vec![time],
        &hashes,
        results,
        3,
        transcripts,
        vec![],
        Default::default(),
    )
}

#[test]
fn native_full_results_use_one_owner_with_time_indices_proofs_and_protocol_transcripts() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(false);
    let proposal_hash = block.hash();
    assert!(block.header().merkle_root().is_none());
    let native = block.network_entrypoint_at(0).unwrap().clone();
    let native_hash = native.hash();
    let time_hash = time.hash_as_entrypoint();
    assert_eq!(
        block.external_entrypoint_count(),
        0,
        "native input is not duplicated into external storage"
    );
    assert_eq!(block.network_entrypoint_count(), 1);
    attach_native_model_results(
        &mut block,
        time.clone(),
        results.clone(),
        transcripts.clone(),
    )
    .unwrap();
    block.validate_native_lane_results().unwrap();
    assert!(
        block.header().merkle_root().is_none(),
        "native source never enters the physical-external root"
    );
    assert_eq!(
        block.hash(),
        proposal_hash,
        "actual results do not change consensus header identity"
    );
    assert_eq!(
        block.canonical_resultless_proposal().hash(),
        proposal_hash,
        "output attachment preserves exact proposal identity"
    );
    assert_eq!(
        block.committed_fragment_count(),
        Some(3),
        "actual fragment count is not successful result count"
    );
    assert_eq!(block.results().cloned().collect::<Vec<_>>(), results);
    assert_eq!(
        block.fastpq_transcripts(),
        &transcripts,
        "additional protocol call remains structural evidence, not invented model authority"
    );
    assert_eq!(
        block.entrypoint_hashes().collect::<Vec<_>>(),
        [native_hash, time_hash]
    );
    assert_eq!(block.entrypoint_cloned_at(0), Some(native.clone()));
    assert_eq!(
        block.entrypoint_cloned_at(1),
        Some(TransactionEntrypoint::Time(time.clone()))
    );
    assert!(block.entrypoint_cloned_at(2).is_none());
    let mut mixed = block.entrypoints_cloned();
    assert_eq!(mixed.len(), 2);
    assert_eq!(mixed.size_hint(), (2, Some(2)));
    assert_eq!(mixed.next_back(), Some(TransactionEntrypoint::Time(time)));
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed.size_hint(), (1, Some(1)));
    assert_eq!(mixed.next(), Some(native));
    assert_eq!(mixed.len(), 0);
    assert!(mixed.next_back().is_none());
    assert!(mixed.next().is_none());
    for (index, entry, result) in block.entrypoint_results() {
        assert_eq!(&results[index], result);
        let proofs = block.proofs_for_entry_hash(&entry.hash()).unwrap();
        assert!(proofs.entry_proof.verify(&proofs.entry_commitment));
        assert!(proofs.result_proof.verify(&proofs.result_commitment));
        assert_eq!(
            block.entrypoint_proof(index as u32),
            Some(proofs.entry_proof.proof().clone())
        );
    }
    let wire = block.encode_wire().unwrap();
    let decoded = crate::block::decode_framed_signed_block(&wire).unwrap();
    assert_eq!(decoded, block);
    decoded.validate_native_lane_results().unwrap();
    assert_eq!(
        block
            .canonical_resultless_proposal()
            .network_entrypoint_count(),
        1
    );
}

#[test]
fn native_full_result_setter_rejects_output_owner_shape_and_index_mutations_atomically() {
    let (base, time, results, transcripts) = native_model_result_fixture(false);
    let call = Hash::from(base.network_entrypoint_at(0).unwrap().execution_call_hash());
    for mutation in 0..4 {
        let mut block = base.clone();
        let before = block.encode_wire().unwrap();
        let mut values = results.clone();
        let mut map = transcripts.clone();
        match mutation {
            0 => {
                values.pop();
            }
            1 => {
                map.get_mut(&call).unwrap().clear();
            }
            2 => {
                map.get_mut(&call).unwrap()[0].batch_hash = Hash::new(b"wrong call");
            }
            3 => {
                let foreign = Hash::new(b"malformed protocol row");
                map.insert(foreign, vec![native_model_transcript(call)]);
            }
            _ => unreachable!(),
        }
        assert!(
            attach_native_model_results(&mut block, time.clone(), values, map).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(
            block.encode_wire().unwrap(),
            before,
            "mutation {mutation} cannot partially update header/result"
        );
    }
    let mut block = base.clone();
    let before = block.encode_wire().unwrap();
    let hashes = [
        time.hash_as_entrypoint(),
        base.network_entrypoint_at(0).unwrap().hash(),
    ];
    assert!(
        block
            .set_full_transaction_results_with_transcripts(
                vec![time],
                &hashes,
                results,
                3,
                transcripts,
                vec![],
                Default::default()
            )
            .is_err()
    );
    assert_eq!(block.encode_wire().unwrap(), before);
}

#[test]
fn native_sealed_outputs_keep_inner_call_map_and_outer_entry_proof_identity() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(true);
    let entry = block.network_entrypoint_at(0).unwrap().clone();
    let outer = Hash::from(entry.hash());
    let inner = Hash::from(entry.execution_call_hash());
    assert_ne!(outer, inner);
    let mut bad = transcripts.clone();
    let values = bad.remove(&inner).unwrap();
    bad.insert(outer, values);
    assert!(attach_native_model_results(&mut block, time.clone(), results.clone(), bad).is_err());
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    assert!(block.fastpq_transcripts().contains_key(&inner));
    assert!(!block.fastpq_transcripts().contains_key(&outer));
    assert!(block.proofs_for_entry_hash(&entry.hash()).is_some());
    assert!(
        block
            .proofs_for_entry_hash(&entry.execution_call_hash())
            .is_none(),
        "inner call is not a second canonical entry/result leaf"
    );
}

#[test]
fn native_result_mutations_remain_structural_and_context_changes_invalidate_caches() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(false);
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    let before = block.encode_wire().unwrap();
    let changed = Err(
        crate::transaction::error::TransactionRejectionReason::Validation(
            crate::ValidationFail::NotPermitted("wrong native output".into()),
        ),
    );
    assert!(block.update_transaction_result(0, &changed));
    assert_ne!(block.encode_wire().unwrap(), before);
    block.validate_native_lane_results().unwrap();
    assert!(
        block.update_transaction_result(1, &changed),
        "Time owns its own ordinary mutable result leaf"
    );
    block.validate_native_lane_results().unwrap();
    let mut context = block.execution_context().unwrap().clone();
    context
        .native_lane_decisions
        .as_mut()
        .unwrap()
        .base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"new applying pre-State"));
    block.set_execution_context(Some(context));
    assert!(!block.has_results());
    assert!(block.header().result_merkle_root().is_none());
    assert!(
        block
            .proofs_for_entry_hash(&block.network_entrypoint_at(0).unwrap().hash())
            .is_none()
    );
}

#[test]
fn native_mixed_carrier_rejects_shape_and_signed_builder_preserves_structural_scope() {
    let (base, time, results, transcripts) = native_model_result_fixture(false);
    let key = KeyPair::from_seed(vec![0x81; 32], Algorithm::Ed25519);
    let mut mixed = base.clone();
    mixed.set_external_entrypoints(vec![base.network_entrypoint_at(0).unwrap().clone()]);
    assert_eq!(
        mixed.network_entrypoint_count(),
        2,
        "invalid mixed inputs are not concealed"
    );
    assert!(
        attach_native_model_results(
            &mut mixed,
            time.clone(),
            results.clone(),
            transcripts.clone()
        )
        .is_err()
    );
    let mut invented_root = base.clone();
    let mut header = invented_root.header();
    header.merkle_root = base
        .network_entrypoints()
        .map(TransactionEntrypoint::hash)
        .collect::<iroha_crypto::MerkleTree<_>>()
        .root();
    invented_root.replace_header_for_testing(header);
    let before = invented_root.encode_wire().unwrap();
    assert!(
        attach_native_model_results(&mut invented_root, time.clone(), results, transcripts)
            .is_err(),
        "native input root must not be copied into the physical-external header root"
    );
    assert_eq!(invented_root.encode_wire().unwrap(), before);
    let mut builder = crate::block::builder::BlockBuilder::new(base.header());
    builder.set_execution_context(base.execution_context().cloned());
    assert_eq!(builder.push_time_trigger(time), 1);
    builder.push_result(Ok(Default::default()));
    builder.push_result(Ok(Default::default()));
    let built = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    built.validate_native_lane_results().unwrap();
    assert!(
        built.fastpq_transcripts().is_empty(),
        "builder cannot infer required evidence from input; execution must check it"
    );
}

#[test]
fn native_full_result_setter_preserves_batch_receipts_and_mutations_change_executed_wire() {
    use crate::events::data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferOutcome};
    let (mut block, time, mut results, transcripts) = native_model_result_fixture(false);
    let entry = block.network_entrypoint_at(0).unwrap().clone();
    let authority = entry.authority().clone();
    let outcome = AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "native-output-model-leg".into(),
        asset: crate::asset::AssetId::new(
            crate::asset::AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("native-output", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            authority.clone(),
        ),
        destination: authority,
        amount: iroha_primitives::numeric::Quantity::from(1u32),
        status: AssetBatchTransferLegStatus::Applied,
    };
    results[0].set_batch_transfer_outcomes(vec![outcome.clone()]);
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    assert_eq!(
        block.batch_transfer_outcomes_for(&entry.hash()),
        &[outcome.clone()]
    );
    let before = block.encode_wire().unwrap();
    block
        .set_batch_transfer_outcomes(Default::default())
        .unwrap();
    assert!(block.batch_transfer_outcomes_for(&entry.hash()).is_empty());
    assert_ne!(
        block.encode_wire().unwrap(),
        before,
        "structurally valid output changes still alter the globally certified executed wire"
    );
    block
        .set_batch_transfer_outcomes(std::collections::BTreeMap::from([(
            entry.hash(),
            vec![outcome],
        )]))
        .unwrap();
    assert_eq!(
        block.encode_wire().unwrap(),
        before,
        "same exact assignment is idempotent"
    );
}

#[test]
fn native_actual_fragment_count_is_explicit_and_inner_result_wrappers_are_refused() {
    let (base, _, _, transcripts) = native_model_result_fixture(false);
    let hashes = [base.network_entrypoint_at(0).unwrap().hash()];
    let mut block = base.clone();
    let before = block.encode_wire().unwrap();
    assert!(
        block
            .set_transaction_results(vec![], &hashes, vec![Ok(Default::default())])
            .is_err()
    );
    assert!(
        block
            .set_transaction_results_with_transcripts(
                vec![],
                &hashes,
                vec![Ok(Default::default())],
                transcripts.clone(),
                vec![],
                Default::default()
            )
            .is_err()
    );
    assert_eq!(block.encode_wire().unwrap(), before);
    block
        .set_full_transaction_results_with_transcripts(
            vec![],
            &hashes,
            vec![native_model_result()],
            3,
            transcripts,
            vec![],
            Default::default(),
        )
        .unwrap();
    assert_eq!(block.results().len(), 1);
    assert_eq!(block.committed_fragment_count(), Some(3));
    assert_eq!(block.canonical_resultless_proposal().hash(), base.hash());
    assert!(block.header().merkle_root().is_none());
}

#[test]
fn native_proofs_remain_structural_for_self_consistent_unfinalized_result_cache() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(false);
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    let entry_hash = block.network_entrypoint_at(0).unwrap().hash();
    // Raw decoded bytes can be internally Merkle-consistent. Proof paths alone
    // authenticate no output; a verified global execution commitment is required.
    let result = block.result.as_mut().unwrap();
    result.transaction_results[0] = crate::transaction::signed::TransactionResult::new(Err(
        crate::transaction::error::TransactionRejectionReason::Validation(
            crate::ValidationFail::NotPermitted("raw divergent native result".into()),
        ),
    ));
    result.result_merkle = result
        .transaction_results
        .iter()
        .map(crate::transaction::signed::TransactionResult::hash)
        .collect();
    block.payload.header.result_merkle_root = result.result_merkle.root();
    block.validate_entrypoint_merkle_cache().unwrap();
    block.validate_result_merkle_cache().unwrap();
    assert!(block.proofs_for_entry_hash(&entry_hash).is_some());
}

#[test]
fn native_results_allow_equal_time_displays_with_distinct_untrusted_evidence_keys() {
    for sealed in [false, true] {
        let (mut block, time, _, mut transcripts) = native_model_result_fixture(sealed);
        let native = block.network_entrypoint_at(0).unwrap().clone();
        let native_call = Hash::from(native.execution_call_hash());
        let display = time.hash_as_entrypoint();
        let invocations = [
            Hash::new(b"model-only actual invocation position one"),
            Hash::new(b"model-only actual invocation position two"),
        ];
        assert_ne!(invocations[0], invocations[1]);
        assert_ne!(native_call, Hash::from(display));
        for invocation in invocations {
            assert_ne!(invocation, native_call);
            assert_ne!(invocation, Hash::from(display));
            assert!(
                transcripts
                    .insert(invocation, vec![native_model_transcript(invocation)])
                    .is_none()
            );
        }
        // These are structurally distinct evidence keys, not authenticated Time
        // ownership. Core must join its actual executed invocation/capture map.
        let hashes = [native.hash(), display, display];
        let proposal = block.hash();
        let results = vec![
            native_model_result(),
            native_model_result(),
            native_model_result(),
        ];
        block
            .set_full_transaction_results_with_transcripts(
                vec![time.clone(), time.clone()],
                &hashes,
                results.clone(),
                5,
                transcripts.clone(),
                vec![],
                Default::default(),
            )
            .unwrap();
        block.validate_native_lane_results().unwrap();
        block.validate_entrypoint_merkle_cache().unwrap();
        block.validate_result_merkle_cache().unwrap();
        assert_eq!(block.hash(), proposal);
        assert_eq!(block.canonical_resultless_proposal().hash(), proposal);
        assert!(block.header().merkle_root().is_none());
        assert_eq!(block.committed_fragment_count(), Some(5));
        assert_eq!(block.entrypoint_hashes().collect::<Vec<_>>(), hashes);
        assert_eq!(block.results().cloned().collect::<Vec<_>>(), results);
        assert_eq!(block.fastpq_transcripts(), &transcripts);
        let entries = block.full_entry_merkle_commitment().unwrap();
        let outputs = block.result_merkle_commitment().unwrap();
        for index in [1u32, 2u32] {
            assert_eq!(
                block.entrypoint_cloned_at(index as usize),
                Some(TransactionEntrypoint::Time(time.clone()))
            );
            let proof = block.entrypoint_proof(index).unwrap();
            assert_eq!(proof.leaf_index(), index);
            assert!(proof.verify(&display, &entries));
            let result_proof = block.result_proof(index).unwrap();
            assert_eq!(result_proof.leaf_index(), index);
            assert!(result_proof.verify(&results[index as usize].hash(), &outputs));
        }
        assert!(block.proofs_for_entry_hash(&native.hash()).is_some());
        if sealed {
            assert_ne!(native.hash(), native.execution_call_hash());
            assert!(
                block
                    .proofs_for_entry_hash(&native.execution_call_hash())
                    .is_none()
            );
            assert!(
                !block
                    .fastpq_transcripts()
                    .contains_key(&Hash::from(native.hash()))
            );
        }
        let decoded =
            crate::block::decode_framed_signed_block(&block.encode_wire().unwrap()).unwrap();
        assert_eq!(decoded, block);
        decoded.validate_native_lane_results().unwrap();
        // Removing a purported invocation's extra row remains structurally valid:
        // the model does not authenticate Time availability/capture completeness.
        let mut missing_time = transcripts.clone();
        missing_time.remove(&invocations[0]);
        let mut structurally_valid = block.clone();
        structurally_valid
            .set_full_transaction_results_with_transcripts(
                vec![time.clone(), time.clone()],
                &hashes,
                results.clone(),
                5,
                missing_time,
                vec![],
                Default::default(),
            )
            .unwrap();
        // Native transcript completeness also belongs to execution, not shape.
        let mut missing_native = transcripts.clone();
        missing_native.remove(&native_call);
        let before = block.encode_wire().unwrap();
        block
            .set_full_transaction_results_with_transcripts(
                vec![time.clone(), time],
                &hashes,
                results,
                5,
                missing_native,
                vec![],
                Default::default(),
            )
            .unwrap();
        assert_ne!(block.encode_wire().unwrap(), before);
    }
}

#[test]
fn native_signed_builder_allows_repeated_time_display_positions() {
    let batch = decision_batch_fixture();
    let native = &batch.groups[0].payload.input.entrypoint;
    let native_hash = native.hash();
    let time = crate::trigger::TimeTriggerEntrypoint {
        id: "native-builder-repeated-time".parse().unwrap(),
        instructions: crate::transaction::ExecutionStep(
            iroha_primitives::const_vec::ConstVec::new_empty(),
        ),
        authority: native.authority().clone(),
    };
    let display = time.hash_as_entrypoint();
    let mut builder = crate::block::builder::BlockBuilder::new(decision_batch_header(&batch));
    builder.set_execution_context(Some(
        crate::block::BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    assert_eq!(builder.push_time_trigger(time.clone()), 1);
    assert_eq!(builder.push_time_trigger(time), 2);
    for index in 0..3 {
        assert_eq!(builder.push_result(Ok(Default::default())), index);
    }
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x6d; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let block = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    block.validate_native_lane_results().unwrap();
    assert_eq!(
        block.entrypoint_hashes().collect::<Vec<_>>(),
        [native_hash, display, display]
    );
    assert!(block.header().merkle_root().is_none());
    assert_eq!(block.results().len(), 3);
    // Signature construction is not Core execution, capture, finality or publication authority.
    assert_eq!(block.signatures().len(), 1);
}

#[test]
fn native_output_authenticity_belongs_to_exact_global_execution_finality() {
    use crate::block::{
        consensus_v2::ExecutionCommitment,
        proofs::{TrustedBlockProofAnchor, TrustedBlockProofAnchorError},
    };
    let (mut block, time, results, transcripts) = native_model_result_fixture(true);
    attach_native_model_results(
        &mut block,
        time.clone(),
        results.clone(),
        transcripts.clone(),
    )
    .unwrap();
    let entry = block.network_entrypoint_at(0).unwrap().hash();
    let wire = block.encode_wire().unwrap();
    let commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"actual model pre-State"),
        Hash::new(b"actual model post-State"),
        Hash::new(b"actual model writes"),
        wire.len() as u64,
        Hash::new(&wire),
    );
    let artifact =
        crate::block::proofs::finalized_native_output_artifact_for_test(&block, &commitment);
    artifact.verify().unwrap();
    let bootstrap = artifact.height_context.snapshot_bootstrap.as_ref().unwrap();
    let batch = block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_deref()
        .unwrap();
    assert_eq!(bootstrap.snapshot_height, batch.base_state_height);
    assert_eq!(bootstrap.snapshot_height + 1, block.header().height().get());
    assert_eq!(
        Some(bootstrap.snapshot_block_hash),
        block.header().prev_block_hash()
    );
    assert_eq!(
        bootstrap.snapshot_state_hash,
        Hash::from(batch.base_state_hash)
    );
    assert!(bootstrap.snapshot_block_creation_time_ms < block.header().creation_time_ms);
    assert!(artifact.height_context.parent_commit_qc.is_none());
    assert_eq!(artifact.commit_qc.signers.len(), 3);
    assert_eq!(artifact.validator_set_pops.len(), 4);
    let anchor =
        TrustedBlockProofAnchor::from_untrusted_finality_artifact(&block, &artifact, &entry)
            .unwrap();
    assert!(block.proofs_for_entry_hash(&entry).unwrap().verify(&anchor));
    for mutation in 0..4 {
        let mut changed = block.clone();
        match mutation {
            0 => {
                assert!(changed.update_transaction_result(
                    0,
                    &Err(
                        crate::transaction::error::TransactionRejectionReason::Validation(
                            crate::ValidationFail::NotPermitted("different executed result".into())
                        )
                    )
                ));
            }
            1 => {
                let mut altered = transcripts.clone();
                let call = Hash::from(
                    changed
                        .network_entrypoint_at(0)
                        .unwrap()
                        .execution_call_hash(),
                );
                altered.get_mut(&call).unwrap()[0].authority_digest =
                    Hash::new(b"different executed evidence");
                attach_native_model_results(&mut changed, time.clone(), results.clone(), altered)
                    .unwrap();
            }
            2 => {
                let mut omitted = transcripts.clone();
                omitted.remove(&Hash::from(
                    changed
                        .network_entrypoint_at(0)
                        .unwrap()
                        .execution_call_hash(),
                ));
                attach_native_model_results(&mut changed, time.clone(), results.clone(), omitted)
                    .unwrap();
            }
            3 => changed.set_committed_fragment_count(4),
            _ => unreachable!(),
        }
        changed.validate_native_lane_results().unwrap();
        changed.validate_entrypoint_merkle_cache().unwrap();
        changed.validate_result_merkle_cache().unwrap();
        assert_eq!(changed.hash(), block.hash());
        assert_eq!(
            changed.canonical_proposal_wire_hash().unwrap(),
            block.canonical_proposal_wire_hash().unwrap()
        );
        assert_ne!(
            changed.executed_block_wire_hash().unwrap(),
            commitment.executed_block_wire_hash
        );
        let proof = changed.proofs_for_entry_hash(&entry).unwrap();
        assert!(
            !proof.verify(&anchor),
            "mutation {mutation} cannot use the original finalized output anchor"
        );
        assert!(matches!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(&changed, &artifact, &entry),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        ));
    }
}

#[test]
fn native_source_header_checks_base_height_parent_time_and_context_binding() {
    let (base, time, results, transcripts) = native_model_result_fixture(false);
    for mutation in 0..4 {
        let mut changed = base.clone();
        let header = changed.header();
        let mut replacement = match mutation {
            0 => BlockHeader::new(
                (header.height().get() + 1).try_into().unwrap(),
                header.prev_block_hash(),
                None,
                None,
                header.creation_time_ms,
                header.view_change_index(),
            ),
            1 => BlockHeader::new(
                header.height(),
                None,
                None,
                None,
                header.creation_time_ms,
                header.view_change_index(),
            ),
            2 => BlockHeader::new(
                header.height(),
                header.prev_block_hash(),
                None,
                None,
                0,
                header.view_change_index(),
            ),
            _ => header,
        };
        replacement.set_execution_context_hash(if mutation == 3 {
            Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign bundle")))
        } else {
            header.execution_context_hash()
        });
        changed.replace_header_for_testing(replacement);
        let before = changed.encode_wire().unwrap();
        assert!(
            attach_native_model_results(
                &mut changed,
                time.clone(),
                results.clone(),
                transcripts.clone()
            )
            .is_err()
        );
        assert_eq!(changed.encode_wire().unwrap(), before);
    }
}
