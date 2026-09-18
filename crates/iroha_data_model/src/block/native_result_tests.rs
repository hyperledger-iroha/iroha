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
    crate::block::execution_output::ExecutionOutputV1,
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
    let native = native_model_transcript(call);
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
    let time = crate::block::output_test_support::simple_time(&block, 0);
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
    time: crate::block::execution_output::ExecutionOutputV1,
    results: Vec<crate::transaction::signed::TransactionResult>,
    transcripts: std::collections::BTreeMap<Hash, Vec<crate::fastpq::TransferTranscript>>,
) -> Result<(), crate::block::SetExecutionOutputsError> {
    if results.len() != 2 {
        return Err(crate::block::SetExecutionOutputsError::InvalidOutputs(
            "fixture requires exactly two actual results".into(),
        ));
    }
    let outputs = vec![
        crate::block::output_test_support::network(0, results[0].clone()),
        time,
    ];
    // The Time row already owns its actual root-first result, never a parallel synthetic input.
    block.set_execution_outputs(
        outputs,
        3,
        transcripts,
        vec![],
        Default::default(),
        Default::default(),
        vec![],
        &crate::block::output_test_support::limits(),
    )
}

#[test]
fn native_full_results_use_one_owner_with_time_indices_proofs_and_protocol_transcripts() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(false);
    let proposal = block.clone();
    let native = block.network_entrypoint_at(0).unwrap().clone();
    assert_eq!(block.external_entrypoint_count(), 0);
    attach_native_model_results(&mut block, time.clone(), results, transcripts.clone()).unwrap();
    block.validate_native_lane_results().unwrap();
    assert_eq!(block.header(), proposal.header());
    assert_eq!(block.hash(), proposal.hash());
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert_eq!(block.committed_fragment_count(), Some(3));
    assert_eq!(block.fastpq_transcripts(), &transcripts);
    assert_eq!(
        block.network_input_hashes().collect::<Vec<_>>(),
        [native.hash()]
    );
    let mut inputs = block.network_entrypoints();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs.next_back(), Some(&native));
    assert!(inputs.next().is_none());
    assert!(block.network_entrypoint_at(1).is_none());
    assert_eq!(block.execution_outputs()[1], time);
    let proof = block.network_execution_proof(&native.hash()).unwrap();
    assert_eq!(proof.entry_commitment.leaf_count().get(), 1);
    assert_eq!(proof.output_commitment.leaf_count().get(), 2);
    assert!(proof.entry_proof.verify(&proof.entry_commitment));
    assert!(proof.output_proof.verify(&proof.output_commitment));
    assert!(
        block
            .output_proof(1)
            .unwrap()
            .verify(&iroha_crypto::HashOf::new(&time), &proof.output_commitment)
    );
    let decoded = crate::block::decode_framed_signed_block(&block.encode_wire().unwrap()).unwrap();
    assert_eq!(decoded, block);
    decoded.validate_native_lane_results().unwrap();
}

#[test]
fn native_full_result_setter_rejects_output_owner_shape_and_index_mutations_atomically() {
    let (base, time, results, transcripts) = native_model_result_fixture(false);
    let call = Hash::from(base.network_entrypoint_at(0).unwrap().execution_call_hash());
    for mutation in 0..6 {
        let mut block = base.clone();
        let before = block.encode_wire().unwrap();
        let mut rows = vec![
            crate::block::output_test_support::network(0, results[0].clone()),
            time.clone(),
        ];
        let mut map = transcripts.clone();
        match mutation {
            0 => {
                rows.remove(0);
            }
            1 => {
                map.get_mut(&call).unwrap().clear();
            }
            2 => map.get_mut(&call).unwrap()[0].batch_hash = Hash::new(b"foreign"),
            3 => {
                map.insert(
                    Hash::new(b"foreign key"),
                    vec![native_model_transcript(call)],
                );
            }
            4 => rows[0] = crate::block::output_test_support::network(1, results[0].clone()),
            5 => rows.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(
            block
                .set_execution_outputs(
                    rows,
                    3,
                    map,
                    vec![],
                    Default::default(),
                    Default::default(),
                    vec![],
                    &crate::block::output_test_support::limits()
                )
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(block.encode_wire().unwrap(), before);
    }
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
    assert!(block.network_execution_proof(&entry.hash()).is_some());
    assert!(
        block
            .network_execution_proof(&entry.execution_call_hash())
            .is_none(),
        "inner call is not a second canonical entry/result leaf"
    );
}

#[test]
fn native_result_mutations_remain_structural_and_context_changes_invalidate_caches() {
    let (mut block, time, mut results, transcripts) = native_model_result_fixture(false);
    attach_native_model_results(
        &mut block,
        time.clone(),
        results.clone(),
        transcripts.clone(),
    )
    .unwrap();
    let before = block.encode_wire().unwrap();
    results[0] = crate::transaction::TransactionResult::new(Err(
        crate::transaction::error::TransactionRejectionReason::Validation(
            crate::ValidationFail::NotPermitted("different".into()),
        ),
    ));
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    assert_ne!(block.encode_wire().unwrap(), before);
    block.validate_native_lane_results().unwrap();
    let mut context = block.execution_context().unwrap().clone();
    context
        .native_lane_decisions
        .as_mut()
        .unwrap()
        .base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"new base"));
    block.set_execution_context(Some(context));
    assert!(!block.has_results());
    assert!(
        block
            .network_execution_proof(&block.network_entrypoint_at(0).unwrap().hash())
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
    let built = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    assert!(built.is_resultless_proposal());
    assert!(built.execution_outputs().is_empty());
    built.validate_proposal_commitments().unwrap();
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
    let retained = block.execution_outputs().to_vec();
    let mut removed = retained.clone();
    let crate::block::execution_output::ExecutionOutputV1::Network(row) = &mut removed[0] else {
        unreachable!()
    };
    row.result.set_batch_transfer_outcomes(vec![]);
    crate::block::output_test_support::install(&mut block, removed, 3).unwrap();
    assert!(block.batch_transfer_outcomes_for(&entry.hash()).is_empty());
    assert_ne!(block.encode_wire().unwrap(), before);
    // Reattach complete original metadata as well as outputs for exact wire equality.
    let call = Hash::from(entry.execution_call_hash());
    let protocol = Hash::new(b"typed protocol call authenticated later by Core");
    let map = std::collections::BTreeMap::from([
        (call, vec![native_model_transcript(call)]),
        (protocol, vec![native_model_transcript(protocol)]),
    ]);
    block
        .set_execution_outputs(
            retained,
            3,
            map,
            vec![],
            Default::default(),
            Default::default(),
            vec![],
            &crate::block::output_test_support::limits(),
        )
        .unwrap();
    assert_eq!(block.encode_wire().unwrap(), before);
}

#[test]
fn native_actual_fragment_count_is_explicit_and_inner_result_wrappers_are_refused() {
    let (mut block, _, _, transcripts) = native_model_result_fixture(false);
    let proposal = block.clone();
    block
        .set_execution_outputs(
            vec![crate::block::output_test_support::network(
                0,
                native_model_result(),
            )],
            3,
            transcripts,
            vec![],
            Default::default(),
            Default::default(),
            vec![],
            &crate::block::output_test_support::limits(),
        )
        .unwrap();
    assert_eq!(block.execution_outputs().len(), 1);
    assert_eq!(block.committed_fragment_count(), Some(3));
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert!(block.header().merkle_root().is_none());
}

#[test]
fn native_proofs_remain_structural_for_self_consistent_unfinalized_result_cache() {
    let (mut block, time, results, transcripts) = native_model_result_fixture(false);
    attach_native_model_results(&mut block, time, results, transcripts).unwrap();
    let entry = block.network_entrypoint_at(0).unwrap().hash();
    let state = block.result.as_mut().unwrap();
    state.outputs[0] = crate::block::output_test_support::network(
        0,
        Err(
            crate::transaction::error::TransactionRejectionReason::Validation(
                crate::ValidationFail::NotPermitted("raw divergent output".into()),
            ),
        ),
    );
    assert!(block.validate_output_merkle_cache().is_err());
    let state = block.result.as_mut().unwrap();
    state.output_merkle = state.outputs.iter().map(HashOf::new).collect();
    block.validate_output_merkle_cache().unwrap();
    assert!(block.network_execution_proof(&entry).is_some());
    // Structural proofs alone carry no global execution or finality authority.
}

#[test]
fn native_results_allow_equal_time_displays_with_distinct_untrusted_evidence_keys() {
    for sealed in [false, true] {
        let (mut block, time, _, mut transcripts) = native_model_result_fixture(sealed);
        let mut second = time.clone();
        let crate::block::execution_output::ExecutionOutputV1::Time(row) = &mut second else {
            unreachable!()
        };
        row.invocation.schedule_index = 1;
        assert_eq!(time.result(), second.result());
        assert_ne!(
            time.execution_call_hash(block.hash(), &block).unwrap(),
            second.execution_call_hash(block.hash(), &block).unwrap()
        );
        for output in [&time, &second] {
            let call = Hash::from(output.execution_call_hash(block.hash(), &block).unwrap());
            transcripts.insert(call, vec![native_model_transcript(call)]);
        }
        let rows = vec![
            crate::block::output_test_support::network(0, native_model_result()),
            time.clone(),
            second,
        ];
        block
            .set_execution_outputs(
                rows,
                5,
                transcripts.clone(),
                vec![],
                Default::default(),
                Default::default(),
                vec![],
                &crate::block::output_test_support::limits(),
            )
            .unwrap();
        assert_eq!(block.execution_outputs().len(), 3);
        assert_eq!(block.network_entrypoint_count(), 1);
        let before = block.encode_wire().unwrap();
        assert!(
            block
                .set_execution_outputs(
                    vec![
                        crate::block::output_test_support::network(0, native_model_result()),
                        time.clone(),
                        time
                    ],
                    5,
                    transcripts,
                    vec![],
                    Default::default(),
                    Default::default(),
                    vec![],
                    &crate::block::output_test_support::limits()
                )
                .is_err()
        );
        assert_eq!(block.encode_wire().unwrap(), before);
    }
}

#[test]
fn native_signed_builder_allows_repeated_time_display_positions() {
    let (base, _, _, _) = native_model_result_fixture(false);
    let mut builder = crate::block::builder::BlockBuilder::new(base.header());
    builder.set_execution_context(base.execution_context().cloned());
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x6d; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let mut block = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    assert!(block.is_resultless_proposal());
    assert_eq!(block.signatures().len(), 1);
    let proposal = block.clone();
    let rows = vec![
        crate::block::output_test_support::network(0, native_model_result()),
        crate::block::output_test_support::simple_time(&block, 0),
        crate::block::output_test_support::simple_time(&block, 1),
    ];
    crate::block::output_test_support::install(&mut block, rows, 3).unwrap();
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert_eq!(block.network_entrypoint_count(), 1);
    assert_eq!(block.execution_outputs().len(), 3);
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
    let trusted_context_id = artifact.context_id();
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        &artifact,
        trusted_context_id,
        &entry,
    )
    .unwrap();
    assert!(
        block
            .network_execution_proof(&entry)
            .unwrap()
            .verify(&anchor)
    );
    for mutation in 0..4 {
        let mut changed = block.clone();
        match mutation {
            0 => {
                let mut divergent = results.clone();
                divergent[0] = crate::transaction::TransactionResult::new(Err(
                    crate::transaction::error::TransactionRejectionReason::Validation(
                        crate::ValidationFail::NotPermitted("different executed result".into()),
                    ),
                ));
                attach_native_model_results(
                    &mut changed,
                    time.clone(),
                    divergent,
                    transcripts.clone(),
                )
                .unwrap();
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
            3 => {
                let rows = changed.execution_outputs().to_vec();
                changed
                    .set_execution_outputs(
                        rows,
                        4,
                        transcripts.clone(),
                        vec![],
                        Default::default(),
                        Default::default(),
                        vec![],
                        &crate::block::output_test_support::limits(),
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        changed.validate_native_lane_results().unwrap();
        changed.validate_proposal_commitments().unwrap();
        changed.validate_output_merkle_cache().unwrap();
        assert_eq!(changed.hash(), block.hash());
        assert_eq!(
            changed.canonical_proposal_wire_hash().unwrap(),
            block.canonical_proposal_wire_hash().unwrap()
        );
        assert_ne!(
            changed.executed_block_wire_hash().unwrap(),
            commitment.executed_block_wire_hash
        );
        let proof = changed.network_execution_proof(&entry).unwrap();
        assert!(
            !proof.verify(&anchor),
            "mutation {mutation} cannot use the original finalized output anchor"
        );
        assert!(matches!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &changed,
                &artifact,
                trusted_context_id,
                &entry
            ),
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
                header.creation_time_ms,
                header.view_change_index(),
            ),
            1 => BlockHeader::new(
                header.height(),
                None,
                None,
                header.creation_time_ms,
                header.view_change_index(),
            ),
            2 => BlockHeader::new(
                header.height(),
                header.prev_block_hash(),
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

#[test]
fn native_output_input_view_borrows_the_existing_admitted_body() {
    use crate::block::execution_output::{
        ExecutionInputs, ExecutionOutputV1, NetworkExecutionOutputV1, validate_execution_outputs_v1,
    };
    let (block, _, _, _) = native_model_result_fixture(false);
    assert_eq!(block.external_entrypoint_count(), 0);
    assert_eq!(block.input_count(), 1);
    let retained = &block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_ref()
        .unwrap()
        .groups[0]
        .payload
        .input
        .entrypoint;
    assert!(std::ptr::eq(block.input_at(0).unwrap(), retained));
    assert!(block.input_at(1).is_none());
    let output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: native_model_result(),
        completions: Vec::new(),
    });
    validate_execution_outputs_v1(
        &[output],
        block.hash(),
        block.header().height().get(),
        &block,
    )
    .unwrap();
}
