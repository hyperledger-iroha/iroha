// Exact-wire proof transport fixtures. These certificates authenticate test outputs;
// they do not claim State execution or SCCP/native producer activation.

use crate::test_utils::torii_proof_finality_for_block;

fn committed_network_proof_app_for_test() -> (SharedAppState, Arc<SignedBlock>, V2FinalityArtifact)
{
    use iroha_data_model::{
        block::{
            builder::BlockBuilder as ModelBlockBuilder, execution_output::*,
            output_budget::ExecutionOutputLimits,
        },
        events::{
            time::{TimeEvent, TimeInterval},
            trigger_completed::TriggerCompletedOutcome,
        },
        transaction::signed::{ExecutionStep, TransactionResult},
        trigger::{DataTriggerStep, TriggerId},
    };
    let app = mk_app_state_for_tests();
    let key = checked_torii_test_ed25519_keypair(0x39, "proof fixture input signer");
    let authority = AccountId::new(key.public_key().clone());
    let mut builder = ModelBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        1,
        0,
    ));
    for millis in [1_u64, 2] {
        let mut tx = TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        tx.set_creation_time(std::time::Duration::from_millis(millis));
        builder.push_transaction(checked_torii_test_transaction(
            tx,
            &key,
            "proof fixture input",
        ));
    }
    let mut block = builder.build_with_signature(0, key.private_key());
    let proposal = block.canonical_resultless_proposal();
    block.validate_proposal_commitments().unwrap();
    let mut outputs = (0..2).map(|input_index| ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index,
        result: if input_index == 0 { TransactionResult::new(Ok(Vec::new())) } else {
            TransactionResult::new(Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted("transport fixture rejection".into()))))
        }, completions: Vec::new(),
    })).collect::<Vec<_>>();
    let action = |id: &str| TriggerUseV1 {
        trigger_id: id.parse().unwrap(),
        registered_at_height: 0,
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
    let pipeline = action("torii_proof_pipeline");
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
    let timer = action("torii_proof_timer");
    outputs.push(ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        result: trace(&timer.trigger_id),
        completions: completion(&timer.trigger_id),
        invocation: TimeInvocationV1 {
            schedule_index: 0,
            trigger: timer,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 0,
                    length_ms: 1,
                },
            },
        },
        failure_root: None,
    }));
    let limits = ExecutionOutputLimits {
        max_outputs: 8,
        max_output_bytes: 1024 * 1024,
        max_total_output_bytes: 2 * 1024 * 1024,
        max_executed_wire_bytes: 4 * 1024 * 1024,
    };
    block
        .set_execution_outputs(
            outputs,
            3,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &limits,
        )
        .unwrap();
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert!(block.header().sccp_commitment_root().is_none());
    let artifact = torii_proof_finality_for_block(&block, *app.state.network_id_ref(), None);
    let block = Arc::new(block);
    app.kura.store_block(Arc::clone(&block)).unwrap();
    let receipt = app.kura.store_v2_finality_artifact(&artifact).unwrap();
    assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
    assert_eq!(receipt.context_id(), artifact.context_id());
    record_committed_block_hash_for_test(&app, block.header(), block.hash());
    (app, block, artifact)
}
