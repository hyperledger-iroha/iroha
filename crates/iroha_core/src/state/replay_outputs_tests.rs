//! Structural replay-parity controls; genuine replay authority stays in strict fixtures.

use super::*;
use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    asset::{AssetDefinitionId, AssetId},
    block::{
        BlockHeader, BlockPayload, BlockResult, BlockSignature, builder::BlockBuilder,
        execution_output::*, output_budget::ExecutionOutputLimits,
    },
    events::{
        data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferOutcome},
        time::{TimeEvent, TimeInterval},
        trigger_completed::TriggerCompletedOutcome,
    },
    fastpq::TransferTranscript,
    transaction::{
        FeePaymentIntent, TransactionBuilder,
        signed::{ExecutionStep, SealedTransactionReveal, TransactionResult},
    },
    trigger::{DataTriggerStep, TriggerId},
};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use norito::codec::{DecodeAll, Encode};
use std::{collections::BTreeSet, num::NonZeroU64, time::Duration};

fn limits() -> ExecutionOutputLimits {
    ExecutionOutputLimits {
        max_outputs: 32,
        max_output_bytes: 64 * 1024,
        max_total_output_bytes: 256 * 1024,
        max_executed_wire_bytes: 1024 * 1024,
    }
}

fn install(block: &mut SignedBlock, outputs: Vec<ExecutionOutputV1>) {
    let fragments = block.committed_fragment_count().unwrap_or(3);
    let transcripts = if block.has_results() {
        block.fastpq_transcripts().clone()
    } else {
        Default::default()
    };
    let envelopes = block.axt_envelopes().unwrap_or_default().to_vec();
    let policy = block.axt_policy_snapshot().cloned().unwrap_or_default();
    let transitions = block
        .axt_transitioned_dataspaces()
        .cloned()
        .unwrap_or_default();
    let statements = block.lane_finality_statements().to_vec();
    block
        .set_execution_outputs(
            outputs,
            fragments,
            transcripts,
            envelopes,
            policy,
            transitions,
            statements,
            &limits(),
        )
        .expect("structural output fixture fits explicit limits");
}

fn fixture() -> SignedBlock {
    let signed = |millis| {
        let mut transaction = TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"structural replay parity fixture"),
            )),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        transaction.set_creation_time(Duration::from_millis(millis));
        transaction.sign(ALICE_KEYPAIR.private_key())
    };
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"structural replay parent",
        ))),
        None,
        2,
        0,
    ));
    builder.push_transaction(signed(1));
    builder.push_sealed_transaction_reveal(SealedTransactionReveal::new(
        Hash::new(b"structural sealed commitment"),
        signed(2),
        [7; 32],
    ));
    let mut block = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    let mut outputs = (0..2)
        .map(|input_index| {
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index,
                result: TransactionResult::new(Ok(Vec::new())),
                completions: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    let trigger = |id: &str| TriggerUseV1 {
        trigger_id: id.parse().unwrap(),
        registered_at_height: 1,
        action_hash: Hash::new(id.as_bytes()),
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
    let pipeline = trigger("replay_pipeline");
    outputs.push(ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
        invocation: PipelineInvocationV1 {
            event: PipelineEventPositionV1::BlockApproved,
            candidate_index: 0,
            trigger: pipeline.clone(),
        },
        result: trace(&pipeline.trigger_id),
        failure_root: None,
        completions: completion(&pipeline.trigger_id),
    }));
    let timer = trigger("replay_timer");
    for schedule_index in [0, 1] {
        outputs.push(ExecutionOutputV1::Time(TimeExecutionOutputV1 {
            invocation: TimeInvocationV1 {
                schedule_index,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: 1,
                        length_ms: 1,
                    },
                },
                trigger: timer.clone(),
            },
            result: trace(&timer.trigger_id),
            failure_root: None,
            completions: completion(&timer.trigger_id),
        }));
    }
    install(&mut block, outputs);
    block
}

#[derive(norito::NoritoSchema, norito::codec::Decode, norito::codec::Encode)]
#[norito_schema(name = "iroha_core::state::replay_outputs::tests::MutableReplayBlock")]
struct MutableReplayBlock {
    signatures: BTreeSet<BlockSignature>,
    payload: BlockPayload,
    result: Option<BlockResult>,
}

fn mutate(block: &SignedBlock, change: impl FnOnce(&mut MutableReplayBlock)) -> SignedBlock {
    let mut raw = MutableReplayBlock::decode_all(&mut block.encode().as_slice()).unwrap();
    change(&mut raw);
    SignedBlock::decode_all(&mut raw.encode().as_slice()).unwrap()
}

fn receipt() -> AssetBatchTransferOutcome {
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("replay-parity", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "retained-leg".into(),
        asset: AssetId::new(definition, ALICE_ID.clone()),
        destination: ALICE_ID.clone(),
        amount: 1_u32.into(),
        status: AssetBatchTransferLegStatus::Applied,
    }
}

#[test]
fn full_parity_keeps_unequal_source_output_counts_and_distinct_time_calls() {
    let block = fixture();
    assert_eq!(block.network_entrypoint_count(), 2);
    assert_eq!(block.execution_outputs().len(), 5);
    assert_eq!(block.network_output_at(1).unwrap().1.input_index, 1);
    assert!(block.network_output_at(2).is_none());
    let outputs = block.execution_outputs();
    assert_eq!(
        outputs[3].result(),
        outputs[4].result(),
        "equal displayed Time programs"
    );
    assert_ne!(
        outputs[3]
            .execution_call_hash(block.hash(), &block)
            .unwrap(),
        outputs[4]
            .execution_call_hash(block.hash(), &block)
            .unwrap()
    );
    ensure_replayed_results_match_committed(2, &block, &block).unwrap();
    log_replayed_signed_sources(2, &block).unwrap();
}

#[test]
fn stale_cache_and_malformed_joins_fail_on_both_comparison_sides() {
    let block = fixture();
    for attack in 0..5 {
        let corrupted = mutate(&block, |raw| {
            let result = raw.result.as_mut().unwrap();
            match attack {
                0 => {
                    let ExecutionOutputV1::Network(row) = &mut result.outputs[0] else {
                        unreachable!()
                    };
                    row.result =
                        TransactionResult::new(Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted("stale leaf".into()),
                        )));
                }
                1 => {
                    result.outputs.remove(0);
                }
                2 => {
                    result.outputs.swap(1, 2);
                }
                3 => {
                    let ExecutionOutputV1::Network(row) = &mut result.outputs[1] else {
                        unreachable!()
                    };
                    row.input_index = 0;
                }
                _ => raw.payload.external_entrypoints.swap(0, 1),
            }
        });
        for (committed, replayed) in [(&corrupted, &block), (&block, &corrupted)] {
            let error =
                ensure_replayed_results_match_committed(2, committed, replayed).unwrap_err();
            assert!(format!("{error:#}").contains("output/source/cache validation failed"));
        }
        assert!(replayed_signed_sources(&corrupted).is_err());
    }
}

#[test]
fn checked_changed_rows_preserve_proposal_but_fail_complete_output_parity() {
    let original = fixture();
    for attack in 0..5 {
        let mut changed = original.clone();
        let mut outputs = changed.execution_outputs().to_vec();
        match attack {
            0 => {
                let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
                    unreachable!()
                };
                row.result.set_batch_transfer_outcomes(vec![receipt()]);
            }
            1 => {
                let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
                    unreachable!()
                };
                row.completions.push(InvocationCompletionV1 {
                    callback_index: 0,
                    trigger_id: "changed_callback".parse().unwrap(),
                    outcome: TriggerCompletedOutcome::Success,
                });
            }
            2 => {
                let ExecutionOutputV1::Pipeline(row) = &mut outputs[2] else {
                    unreachable!()
                };
                row.invocation.trigger.action_hash = Hash::new(b"different action");
            }
            3 => {
                outputs.pop();
            }
            _ => {
                let ExecutionOutputV1::Time(row) = &mut outputs[4] else {
                    unreachable!()
                };
                row.invocation.schedule_index = 2;
            }
        }
        install(&mut changed, outputs);
        changed.validate_output_merkle_cache().unwrap();
        assert_eq!(changed.header(), original.header());
        assert_eq!(
            changed.canonical_resultless_proposal(),
            original.canonical_resultless_proposal()
        );
        assert_eq!(changed.hash(), original.hash());
        assert_ne!(
            changed.encode_wire().unwrap(),
            original.encode_wire().unwrap()
        );
        let error = ensure_replayed_results_match_committed(2, &original, &changed).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("typed execution output mismatch")
        );
        if attack == 3 {
            assert!(error.to_string().contains("first_mismatch=Some(4)"));
        }
    }
}

#[test]
fn metadata_outside_output_merkle_is_part_of_replay_parity() {
    let original = fixture();
    for (attack, expected) in [
        (0, "committed fragment count"),
        (1, "FASTPQ transcripts"),
        (2, "AXT transitioned dataspaces"),
    ] {
        let changed = mutate(&original, |raw| {
            let result = raw.result.as_mut().unwrap();
            match attack {
                0 => result.committed_fragment_count += 1,
                1 => {
                    let call = Hash::new(b"structurally valid additional protocol call");
                    result.fastpq_transcripts.insert(
                        call,
                        vec![TransferTranscript {
                            batch_hash: call,
                            deltas: Vec::new(),
                            authority_digest: Hash::new(b"changed"),
                            poseidon_preimage_digest: None,
                        }],
                    );
                }
                _ => {
                    result
                        .axt_transitioned_dataspaces
                        .insert(DataSpaceId::new(9));
                }
            }
        });
        changed.validate_output_merkle_cache().unwrap();
        assert_eq!(
            changed.output_merkle_commitment(),
            original.output_merkle_commitment()
        );
        assert_ne!(
            changed.encode_wire().unwrap(),
            original.encode_wire().unwrap()
        );
        assert!(
            ensure_replayed_results_match_committed(2, &original, &changed)
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
    }
}

#[test]
fn diagnostics_separate_internal_failures_from_explicit_sealed_network_join() {
    let mut block = fixture();
    let mut outputs = block.execution_outputs().to_vec();
    let ExecutionOutputV1::Network(network) = &mut outputs[1] else {
        unreachable!()
    };
    network.result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted("sealed rejection".into()),
    )));
    let ExecutionOutputV1::Pipeline(pipeline) = &outputs[2] else {
        unreachable!()
    };
    outputs[2] = ExecutionOutputV1::pipeline_output_limit_rejection(pipeline.invocation.clone());
    let ExecutionOutputV1::Time(timer) = &outputs[3] else {
        unreachable!()
    };
    outputs[3] = ExecutionOutputV1::time_output_limit_rejection(timer.invocation.clone());
    install(&mut block, outputs);
    let sources = replayed_signed_sources(&block).unwrap();
    assert_eq!(sources.len(), 2);
    assert!(sources[0].rejection.is_none());
    assert_eq!(sources[0].entrypoint_hash, sources[0].execution_call_hash);
    assert_eq!(sources[1].input_index, 1);
    assert_ne!(sources[1].entrypoint_hash, sources[1].execution_call_hash);
    assert_eq!(
        sources[1].entrypoint_hash,
        block.network_entrypoint_at(1).unwrap().hash()
    );
    assert!(sources[1].rejection.is_some());
    let diagnostics = replay_validation_output_errors(&block);
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].starts_with("output#1 network#1:"));
    assert!(diagnostics[0].contains("sealed rejection"));
    assert!(diagnostics[1].starts_with("output#2 pipeline "));
    assert!(diagnostics[2].starts_with("output#3 time#0 "));
    log_replayed_signed_sources(2, &block).unwrap();
    assert!(replay_validation_output_errors(&block.canonical_resultless_proposal()).is_empty());
    assert!(replayed_signed_sources(&block.canonical_resultless_proposal()).is_err());
}

#[test]
fn parity_refuses_different_headers_and_signatures_even_with_equal_output_rows() {
    let original = fixture();
    let wrong_header = mutate(&original, |raw| raw.payload.header.creation_time_ms += 1);
    wrong_header.validate_output_merkle_cache().unwrap();
    assert_eq!(
        wrong_header.execution_outputs(),
        original.execution_outputs()
    );
    assert!(
        ensure_replayed_results_match_committed(2, &original, &wrong_header)
            .unwrap_err()
            .to_string()
            .contains("proposal header mismatch")
    );
    let mut wrong_signatures = original.clone();
    wrong_signatures
        .replace_signatures(BTreeSet::from([BlockSignature::new(
            1,
            iroha_crypto::SignatureOf::try_from_hash(ALICE_KEYPAIR.private_key(), original.hash())
                .unwrap(),
        )]))
        .unwrap();
    assert!(
        ensure_replayed_results_match_committed(2, &original, &wrong_signatures)
            .unwrap_err()
            .to_string()
            .contains("proposal signature sequence mismatch")
    );
    assert!(
        ensure_replayed_results_match_committed(3, &original, &original)
            .unwrap_err()
            .to_string()
            .contains("height differs")
    );
}
