//! Block-summary and smoke lookup parity across lifecycle output carriers.

use super::*;
use iroha_data_model::block::execution_output::{
    NetworkExecutionOutputV2, PipelineEventPositionV1, PipelineExecutionOutputV2,
    PipelineInvocationV1, TimeExecutionOutputV2, TriggerLifecycleEffectsV1,
};
use iroha_data_model::transaction::error::TransactionLimitError;

#[test]
fn lifecycle_block_summary_preserves_counts_and_network_smoke_outcome() {
    for lifecycle in [false, true] {
        for rejected in [false, true] {
            let result = if rejected {
                Err(TransactionRejectionReason::LimitCheck(
                    TransactionLimitError {
                        reason: "summary rejection".into(),
                    },
                ))
            } else {
                Ok(iroha_data_model::transaction::DataTriggerSequence::default())
            };
            let mut block = sample_block_with_result(result);
            let tx_hash = block.external_transactions().next().unwrap().hash();
            let trigger = TriggerUseV1 {
                trigger_id: "summary_callback".parse().unwrap(),
                registered_at_height: 0,
                action_hash: Hash::new(b"summary callback action"),
            };
            let mut outputs = block.execution_outputs().to_vec();
            outputs.push(ExecutionOutputV1::pipeline_output_limit_rejection(
                PipelineInvocationV1 {
                    event: PipelineEventPositionV1::BlockApproved,
                    candidate_index: 0,
                    trigger: trigger.clone(),
                },
            ));
            outputs.push(ExecutionOutputV1::time_output_limit_rejection(
                TimeInvocationV1 {
                    schedule_index: 0,
                    event: TimeEvent {
                        interval: TimeInterval {
                            since_ms: 41_999,
                            length_ms: 1,
                        },
                    },
                    trigger,
                },
            ));
            if lifecycle {
                outputs = outputs
                    .into_iter()
                    .map(|output| {
                        let trigger_lifecycle = TriggerLifecycleEffectsV1::Complete {
                            effects: Vec::new(),
                        };
                        match output {
                            ExecutionOutputV1::Network(execution) => {
                                ExecutionOutputV1::NetworkWithLifecycle(NetworkExecutionOutputV2 {
                                    execution,
                                    trigger_lifecycle,
                                })
                            }
                            ExecutionOutputV1::Pipeline(execution) => {
                                ExecutionOutputV1::PipelineWithLifecycle(
                                    PipelineExecutionOutputV2 {
                                        execution,
                                        trigger_lifecycle,
                                    },
                                )
                            }
                            ExecutionOutputV1::Time(execution) => {
                                ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                                    execution,
                                    trigger_lifecycle,
                                })
                            }
                            _ => unreachable!("fixture starts with uncovered outputs"),
                        }
                    })
                    .collect();
            }
            block
                .set_execution_outputs(
                    outputs,
                    u64::from(!rejected),
                    Default::default(),
                    Vec::new(),
                    Default::default(),
                    Default::default(),
                    &SAMPLE_OUTPUT_LIMITS,
                )
                .expect("attach explicit Network, Pipeline and Time rows");
            let original_hash = block.hash();
            let summary = BlockSummary::from_block(&block);
            assert_eq!(summary.transaction_count, 1);
            assert_eq!(summary.rejected_transaction_count, usize::from(rejected));
            assert_eq!(summary.time_trigger_count, 1);
            assert_eq!(summary.hash_hex, original_hash.to_string());
            let outcome = smoke_transaction_result_in_block(&block, &tx_hash)
                .expect("explicit Network output is present");
            if rejected {
                assert!(
                    matches!(outcome, Err(ToriiError::SmokeRejected { reason, .. })
                    if reason.ends_with(": summary rejection"))
                );
            } else {
                assert_eq!(outcome.expect("Network succeeded"), summary.height);
            }
            assert_eq!(
                block.hash(),
                original_hash,
                "projection preserves commitments"
            );
        }
    }
}
