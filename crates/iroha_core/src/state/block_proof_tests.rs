use super::*;
use crate::kura::tests::CommittedNetworkProofFixture;
use iroha_crypto::MerkleTree as CanonMerkleTree;
use iroha_data_model::{
    block::{
        BlockPayload, BlockResult, BlockSignature, builder::BlockBuilder as ModelBlockBuilder,
        execution_output::*, proofs::TrustedBlockProofAnchor,
    },
    events::{
        time::{TimeEvent, TimeInterval},
        trigger_completed::TriggerCompletedOutcome,
    },
    transaction::{
        FeePaymentIntent,
        signed::{
            ExecutionStep, SealedTransactionReveal, TransactionBuilder, TransactionEntrypoint,
            TransactionResult,
        },
    },
    trigger::{DataTriggerStep, TriggerId},
};
use nonzero_ext::nonzero;
use norito::codec::DecodeAll as _;

#[derive(norito::NoritoSchema, norito::codec::Decode, norito::codec::Encode)]
#[norito_schema(name = "iroha_core::state::block_proof_tests::MutableSignedBlockWire")]
struct MutableSignedBlockWire {
    signatures: BTreeSet<BlockSignature>,
    payload: BlockPayload,
    result: Option<BlockResult>,
}

fn proof_limits() -> BlockProofLimits {
    BlockProofLimits {
        max_block_wire_bytes: 4 * 1024 * 1024,
        max_work_items: 128,
        max_response_bytes: 4 * 1024 * 1024,
    }
}

fn proof_target(parent: &SignedBlock, sealed_commitment: bool) -> SignedBlock {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let mut builder = ModelBlockBuilder::new(BlockHeader::new(
        nonzero!(2_u64),
        Some(parent.hash()),
        None,
        u64::try_from(parent.header().creation_time().as_millis()).unwrap() + 1,
        0,
    ));
    let signed = |millis| {
        let mut tx = TransactionBuilder::new(
            *DEFAULT_TEST_NETWORK_ID,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        tx.set_creation_time(std::time::Duration::from_millis(millis));
        tx.sign(keypair.private_key())
    };
    if sealed_commitment {
        use iroha_data_model::transaction::signed::{
            SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment,
            compute_sealed_transaction_commitment,
        };
        let tx = signed(7);
        let payload = SealedTransactionCommitmentPayload {
            network_id: *DEFAULT_TEST_NETWORK_ID,
            authority: authority.clone(),
            commitment: compute_sealed_transaction_commitment(
                &*DEFAULT_TEST_NETWORK_ID,
                &tx,
                [0xA5; 32],
                5,
            ),
            reveal_after_height: 3,
            reveal_deadline_height: 5,
            nonce: None,
        };
        builder.push_sealed_transaction_commitment(SignedSealedTransactionCommitment::sign(
            payload,
            keypair.private_key(),
        ));
    } else {
        builder.push_transaction(signed(7));
        builder.push_sealed_transaction_reveal(SealedTransactionReveal::new(
            Hash::new(b"proof sealed commitment"),
            signed(8),
            [0xA5; 32],
        ));
    }
    let mut block = builder.build_with_signature(0, keypair.private_key());
    let mut outputs = block
        .network_entrypoints()
        .enumerate()
        .map(|(index, _)| {
            let result =
                if index == 1 {
                    TransactionResult::new(Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted("alpha".into()))))
                } else {
                    TransactionResult::new(Ok(Vec::new()))
                };
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: u32::try_from(index).unwrap(),
                result,
                completions: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    let trigger = |name: &str| TriggerUseV1 {
        trigger_id: name.parse().unwrap(),
        registered_at_height: 1,
        action_hash: Hash::new(name.as_bytes()),
    };
    let trace = |id: &TriggerId| {
        TransactionResult::new(Ok(vec![DataTriggerStep {
            id: id.clone(),
            instructions: ExecutionStep(Vec::new().into()),
        }]))
    };
    let completion = |id: &TriggerId| {
        vec![InvocationCompletionV1 {
            callback_index: 0,
            trigger_id: id.clone(),
            outcome: TriggerCompletedOutcome::Success,
        }]
    };
    let pipeline = trigger("proof_pipeline");
    outputs.push(ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
        result: trace(&pipeline.trigger_id),
        completions: completion(&pipeline.trigger_id),
        invocation: PipelineInvocationV1 {
            event: PipelineEventPositionV1::BlockApproved,
            candidate_index: 0,
            trigger: pipeline,
        },
        failure_root: None,
    }));
    let timer = trigger("proof_timer");
    outputs.push(ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        result: trace(&timer.trigger_id),
        completions: completion(&timer.trigger_id),
        invocation: TimeInvocationV1 {
            schedule_index: 0,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 1,
                    length_ms: 1,
                },
            },
            trigger: timer,
        },
        failure_root: None,
    }));
    crate::kura::tests::install_network_index_test_outputs(&mut block, outputs);
    block.validate_output_merkle_cache().unwrap();
    block
}

fn proof_fixture() -> CommittedNetworkProofFixture {
    CommittedNetworkProofFixture::new(|parent| proof_target(parent, false), true)
}

fn proof_state(fixture: &CommittedNetworkProofFixture) -> Box<State> {
    let state = Box::new(
        State::try_new_with_chain_and_network_id(
            World::default(),
            Arc::clone(&fixture.kura),
            crate::query::store::LiveQueryStore::start_test(),
            (*DEFAULT_TEST_CHAIN_ID).clone(),
            fixture.artifacts[0].height_context.network_id,
            #[cfg(feature = "telemetry")]
            Default::default(),
        )
        .expect("actual fallible State startup over authenticated configured Kura"),
    );
    let mut hashes = state.block_hashes.block();
    for block in &fixture.blocks {
        hashes.push(block.hash());
    }
    hashes.commit();
    state
}

fn mutate_stored_block(
    block: &SignedBlock,
    mutate: impl FnOnce(&mut BlockPayload, &mut BlockResult),
) -> SignedBlock {
    let encoded = block.encode();
    let mut wire = MutableSignedBlockWire::decode_all(&mut encoded.as_slice()).unwrap();
    mutate(&mut wire.payload, wire.result.as_mut().unwrap());
    SignedBlock::decode_all(&mut wire.encode().as_slice()).unwrap()
}

fn assert_invalid_finalized_body(mutate: impl FnOnce(&mut BlockPayload, &mut BlockResult)) {
    let fixture = CommittedNetworkProofFixture::new(
        |parent| mutate_stored_block(&proof_target(parent, false), mutate),
        true,
    );
    let target = fixture.target();
    let entry = target.network_entrypoint_at(0).unwrap().hash();
    let error = proof_state(&fixture)
        .block_proofs_for_entry(nonzero!(2_u64), entry, proof_limits())
        .expect_err("malformed complete body must not yield any proof");
    assert!(
        matches!(error, BlockProofError::InvalidOutputs { block_height, .. }
        | BlockProofError::Storage { block_height, .. } if block_height == nonzero!(2_u64)),
        "complete structural/finality refusal, got {error:?}"
    );
}

#[test]
fn block_proofs_for_external_entry_use_distinct_input_and_output_trees() {
    let fixture = proof_fixture();
    let state = proof_state(&fixture);
    let block = fixture.target();
    let artifact = fixture.artifacts.last().unwrap();
    let fixture_context = artifact.context_id();
    for (index, input) in block.network_entrypoints().enumerate() {
        let proofs = state
            .block_proofs_for_entry(nonzero!(2_u64), input.hash(), proof_limits())
            .unwrap();
        assert_eq!(proofs.block_hash, block.hash());
        assert_eq!(
            proofs.executed_block_wire_hash,
            block.executed_block_wire_hash().unwrap()
        );
        assert_eq!(
            proofs.entry_commitment.root(),
            &block.header().merkle_root().unwrap()
        );
        assert_eq!(proofs.entry_commitment.leaf_count().get(), 2);
        assert_eq!(proofs.output_commitment.leaf_count().get(), 4);
        assert!(proofs.entry_proof.verify(&proofs.entry_commitment));
        assert!(proofs.output_proof.verify(&proofs.output_commitment));
        assert_eq!(
            proofs.output_proof.output(),
            &block.execution_outputs()[index]
        );
        let ExecutionOutputV1::Network(row) = proofs.output_proof.output() else {
            panic!("Network owner")
        };
        assert_eq!(usize::try_from(row.input_index).unwrap(), index);
        assert_eq!(row.result.is_err(), index == 1);
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            block,
            artifact,
            fixture_context,
            &input.hash(),
        )
        .unwrap();
        assert!(proofs.verify(&anchor));
    }
}

#[test]
fn block_proofs_do_not_reinterpret_sealed_inner_or_internal_calls_as_inputs() {
    let fixture = proof_fixture();
    let state = proof_state(&fixture);
    let block = fixture.target();
    let TransactionEntrypoint::SealedReveal(reveal) = block.network_entrypoint_at(1).unwrap()
    else {
        unreachable!()
    };
    let inner = reveal.signed_transaction().hash_as_entrypoint();
    assert_ne!(inner, block.network_entrypoint_at(1).unwrap().hash());
    let internal = block.execution_outputs()[2..].iter().map(|output| {
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            output.execution_call_hash(block.hash(), block).unwrap(),
        )
    });
    for hash in std::iter::once(inner).chain(internal) {
        assert!(
            matches!(state.block_proofs_for_entry(nonzero!(2_u64), hash, proof_limits()),
            Err(BlockProofError::EntrypointNotFound { entry_hash, block_height })
                if entry_hash == hash && block_height == nonzero!(2_u64))
        );
    }
}

#[test]
fn block_proofs_reject_kura_body_not_committed_by_wsv() {
    let fixture = proof_fixture();
    let expected =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"different committed WSV header"));
    assert_ne!(fixture.target().hash(), expected);
    let error = block_proofs_for_entry_from_kura(
        &fixture.kura,
        nonzero!(2_u64),
        expected,
        fixture.target().network_entrypoint_at(0).unwrap().hash(),
        proof_limits(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            BlockProofError::BlockHashMismatch { .. }
                | BlockProofError::BlockNotFound(_)
                | BlockProofError::Storage { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn block_proofs_reject_stored_input_root_drift() {
    assert_invalid_finalized_body(|payload, _| {
        let foreign =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(b"foreign input"));
        let tree: CanonMerkleTree<TransactionEntrypoint> = [foreign].into_iter().collect();
        assert_ne!(payload.header.merkle_root, tree.root());
        payload.header.merkle_root = tree.root();
    });
}

#[test]
fn block_proofs_reject_stored_input_count_drift() {
    assert_invalid_finalized_body(|payload, _| {
        payload.external_entrypoints.pop();
    });
}

#[test]
fn block_proofs_reject_stored_output_commitment_drift() {
    assert_invalid_finalized_body(|_, result| {
        let canonical = result.output_merkle.root();
        let mut leaves = result.outputs.iter().map(HashOf::new).collect::<Vec<_>>();
        leaves[2] = HashOf::from_untyped_unchecked(Hash::new(b"foreign internal output"));
        result.output_merkle = leaves.into_iter().collect();
        assert_eq!(result.output_merkle.leaf_count(), result.outputs.len());
        assert_ne!(canonical, result.output_merkle.root());
    });
}

#[test]
fn block_proofs_reject_stored_output_count_drift() {
    assert_invalid_finalized_body(|_, result| {
        let mut leaves = result.outputs.iter().map(HashOf::new).collect::<Vec<_>>();
        leaves.push(HashOf::from_untyped_unchecked(Hash::new(b"extra output")));
        result.output_merkle = leaves.into_iter().collect();
        assert_eq!(result.output_merkle.leaf_count(), result.outputs.len() + 1);
    });
}

#[test]
fn block_proofs_reject_self_consistent_network_count_misalignment() {
    assert_invalid_finalized_body(|_, result| {
        result.outputs.remove(1);
        result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
        assert_eq!(result.output_merkle.leaf_count(), 3);
    });
}

#[test]
fn block_proofs_validate_foreign_network_join_and_unrelated_internal_owner() {
    for internal in [false, true] {
        assert_invalid_finalized_body(|_, result| {
            if internal {
                let ExecutionOutputV1::Pipeline(row) = &mut result.outputs[2] else {
                    unreachable!()
                };
                row.invocation.trigger.registered_at_height = 2;
            } else {
                let ExecutionOutputV1::Network(row) = &mut result.outputs[1] else {
                    unreachable!()
                };
                row.input_index = 0;
            }
            result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
        });
    }
}

#[test]
fn block_proofs_reject_retired_context_even_with_exact_finality() {
    let fixture = CommittedNetworkProofFixture::new(
        |parent| {
            let mut block = proof_target(parent, false);
            let mut context = iroha_data_model::block::BlockExecutionContextBundle::new(Vec::new());
            context.version = 0;
            assert!(!context.has_current_version());
            block.set_execution_context(Some(context));
            block
        },
        true,
    );
    let entry = fixture.target().network_entrypoint_at(0).unwrap().hash();
    let error = proof_state(&fixture)
        .block_proofs_for_entry(nonzero!(2_u64), entry, proof_limits())
        .unwrap_err();
    assert!(
        matches!(
            error,
            BlockProofError::InvalidOutputs { .. } | BlockProofError::Storage { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn block_proofs_reject_requested_slot_header_height_mismatch() {
    let fixture = proof_fixture();
    let target = fixture.target();
    let error = block_proofs_for_entry_from_kura(
        &fixture.kura,
        nonzero!(1_u64),
        target.hash(),
        target.network_entrypoint_at(0).unwrap().hash(),
        proof_limits(),
    )
    .unwrap_err();
    assert!(
        matches!(error, BlockProofError::BlockHeightMismatch { requested, actual }
        if requested == nonzero!(1_u64) && actual == nonzero!(2_u64))
            || matches!(
                error,
                BlockProofError::Storage { .. } | BlockProofError::BlockHashMismatch { .. }
            ),
        "wrong slot must not reuse another height's authentic body: {error:?}"
    );
}

#[test]
fn block_proofs_require_published_finality_and_attached_outputs() {
    for resultless in [false, true] {
        let fixture = CommittedNetworkProofFixture::new(
            |parent| {
                let block = proof_target(parent, false);
                if resultless {
                    block.canonical_resultless_proposal()
                } else {
                    block
                }
            },
            resultless,
        );
        let target = fixture.target();
        let error = proof_state(&fixture)
            .block_proofs_for_entry(
                nonzero!(2_u64),
                target.network_entrypoint_at(0).unwrap().hash(),
                proof_limits(),
            )
            .unwrap_err();
        if resultless {
            assert!(
                matches!(
                    error,
                    BlockProofError::MissingResults(_)
                        | BlockProofError::InvalidOutputs { .. }
                        | BlockProofError::Storage { .. }
                ),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, BlockProofError::Storage { .. }),
                "{error:?}"
            );
        }
    }
}

#[test]
fn block_proofs_deny_cold_body_before_read_and_reject_finalized_wire_substitution() {
    for corrupt in [false, true] {
        let fixture = proof_fixture();
        let state = proof_state(&fixture);
        let block = fixture.target();
        let entry = block.network_entrypoint_at(0).unwrap().hash();
        let original = block.encode_wire().unwrap();
        if corrupt {
            let altered =
                mutate_stored_block(block, |_, result| {
                    let ExecutionOutputV1::Network(row) = &mut result.outputs[1] else {
                        unreachable!()
                    };
                    row.result = TransactionResult::new(Err(
                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted("bravo".into()))));
                    result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
                });
            altered.validate_output_merkle_cache().unwrap();
            assert_eq!(altered.header(), block.header());
            assert_eq!(altered.hash(), block.hash());
            let wire = altered.encode_wire().unwrap();
            assert_eq!(wire.len(), original.len());
            assert_ne!(wire, original);
            fixture.overwrite_target_wire(&wire);
        } else {
            fixture.make_target_cold();
        }
        let disk_before = fixture.target_disk_bytes();
        let index_before = fixture.index_image();
        let reads_before = fixture.kura.canonical_body_bytes_read_for_test();
        let actual = u64::try_from(original.len()).unwrap();
        let denied = BlockProofLimits {
            max_block_wire_bytes: actual - 1,
            ..proof_limits()
        };
        assert!(
            matches!(state.block_proofs_for_entry(nonzero!(2_u64), entry, denied),
            Err(BlockProofError::CapacityExceeded { block_height, resource: BlockProofResource::BlockWireBytes,
                actual: observed, limit }) if block_height == nonzero!(2_u64) && observed == actual && limit == actual - 1)
        );
        assert_eq!(
            fixture.kura.canonical_body_bytes_read_for_test(),
            reads_before
        );
        assert!(!fixture.target_cached());
        assert_eq!(fixture.index_image(), index_before);
        assert_eq!(fixture.target_disk_bytes(), disk_before);
        let admitted = state.block_proofs_for_entry(
            nonzero!(2_u64),
            entry,
            BlockProofLimits {
                max_block_wire_bytes: actual,
                ..proof_limits()
            },
        );
        assert_eq!(
            fixture.kura.canonical_body_bytes_read_for_test(),
            reads_before + actual
        );
        if corrupt {
            let expected = crate::kura::Error::CanonicalBlockWireMismatch { height: 2 }.to_string();
            assert!(
                matches!(admitted, Err(BlockProofError::Storage { reason, .. }) if reason == expected)
            );
        } else {
            assert!(admitted.is_ok());
        }
        assert!(!fixture.target_cached());
        assert_eq!(fixture.index_image(), index_before);
        assert_eq!(fixture.target_disk_bytes(), disk_before);
    }
}

#[test]
fn block_proofs_enforce_exact_work_and_response_limits() {
    let fixture = proof_fixture();
    let state = proof_state(&fixture);
    let entry = fixture.target().network_entrypoint_at(0).unwrap().hash();
    let proof = state
        .block_proofs_for_entry(nonzero!(2_u64), entry, proof_limits())
        .unwrap();
    let work = 6; // two source rows, all four outputs, no FASTPQ map/transcript rows.
    let response = u64::try_from(norito::canonical_frame_len(&proof).unwrap()).unwrap();
    for (resource, limits) in [
        (
            BlockProofResource::WorkItems,
            BlockProofLimits {
                max_work_items: work - 1,
                ..proof_limits()
            },
        ),
        (
            BlockProofResource::ResponseBytes,
            BlockProofLimits {
                max_response_bytes: response - 1,
                ..proof_limits()
            },
        ),
    ] {
        let error = state
            .block_proofs_for_entry(nonzero!(2_u64), entry, limits)
            .unwrap_err();
        assert!(
            matches!(error, BlockProofError::CapacityExceeded { resource: observed, .. }
            if observed == resource),
            "{error:?}"
        );
    }
    let exact = state
        .block_proofs_for_entry(
            nonzero!(2_u64),
            entry,
            BlockProofLimits {
                max_work_items: work,
                max_response_bytes: response,
                ..proof_limits()
            },
        )
        .unwrap();
    assert_eq!(exact, proof);
    fixture.make_target_cold();
    let reads_before = fixture.kura.canonical_body_bytes_read_for_test();
    let index_before = fixture.index_image();
    for (resource, limits) in [
        (
            BlockProofResource::BlockWireBytes,
            BlockProofLimits {
                max_block_wire_bytes: 0,
                ..proof_limits()
            },
        ),
        (
            BlockProofResource::WorkItems,
            BlockProofLimits {
                max_work_items: 0,
                ..proof_limits()
            },
        ),
        (
            BlockProofResource::ResponseBytes,
            BlockProofLimits {
                max_response_bytes: 0,
                ..proof_limits()
            },
        ),
    ] {
        assert!(
            matches!(state.block_proofs_for_entry(nonzero!(2_u64), entry, limits),
            Err(BlockProofError::CapacityExceeded { resource: observed, limit: 0, actual, .. })
                if observed == resource && actual > 0)
        );
        assert_eq!(
            fixture.kura.canonical_body_bytes_read_for_test(),
            reads_before
        );
        assert!(!fixture.target_cached());
        assert_eq!(fixture.index_image(), index_before);
    }
}

#[test]
fn executed_block_wire_returns_exact_finalized_bytes_and_enforces_admission() {
    let fixture = proof_fixture();
    let state = proof_state(&fixture);
    let expected = fixture.target().encode_wire().unwrap();
    let size = u64::try_from(expected.len()).unwrap();
    fixture.make_target_cold();
    let before = fixture.kura.canonical_body_bytes_read_for_test();
    for bound in [0, size - 1] {
        assert!(
            matches!(state.executed_block_wire(nonzero!(2_u64), BlockProofLimits {
            max_block_wire_bytes: bound, ..proof_limits()
        }), Err(BlockProofError::CapacityExceeded { resource: BlockProofResource::BlockWireBytes,
            actual, limit, .. }) if actual == if bound == 0 { 1 } else { size } && limit == bound)
        );
        assert_eq!(fixture.kura.canonical_body_bytes_read_for_test(), before);
    }
    assert_eq!(
        state
            .executed_block_wire(
                nonzero!(2_u64),
                BlockProofLimits {
                    max_block_wire_bytes: size,
                    max_response_bytes: size,
                    ..proof_limits()
                }
            )
            .unwrap(),
        expected
    );
    assert_eq!(
        fixture.kura.canonical_body_bytes_read_for_test(),
        before + size
    );
    assert!(!fixture.target_cached());
}

/// Keep the existing sealed-commitment State regression on genuine exact-wire finality.
pub(super) fn assert_sealed_commitment_proof_uses_distinct_trees() {
    let fixture = CommittedNetworkProofFixture::new(|parent| proof_target(parent, true), true);
    let state = proof_state(&fixture);
    let block = fixture.target();
    let sealed_hash = block.network_entrypoint_at(0).unwrap().hash();
    let proof = state
        .block_proofs_for_entry(nonzero!(2_u64), sealed_hash, proof_limits())
        .unwrap();
    assert_eq!(proof.block_hash, block.hash());
    assert_eq!(
        proof.executed_block_wire_hash,
        block.executed_block_wire_hash().unwrap()
    );
    assert_eq!(
        proof.entry_commitment.root(),
        &block.header().merkle_root().unwrap()
    );
    assert_eq!(proof.entry_commitment.leaf_count().get(), 1);
    assert_eq!(proof.output_commitment.leaf_count().get(), 3);
    assert!(proof.entry_proof.verify(&proof.entry_commitment));
    assert!(proof.output_proof.verify(&proof.output_commitment));
    assert_eq!(
        block.network_input_hashes().collect::<Vec<_>>(),
        vec![sealed_hash]
    );
    assert!(matches!(
        block.execution_outputs()[1],
        ExecutionOutputV1::Pipeline(_)
    ));
    assert!(matches!(
        block.execution_outputs()[2],
        ExecutionOutputV1::Time(_)
    ));
    let artifact = fixture.artifacts.last().unwrap();
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        block,
        artifact,
        artifact.context_id(),
        &sealed_hash,
    )
    .unwrap();
    assert!(proof.verify(&anchor));
}
