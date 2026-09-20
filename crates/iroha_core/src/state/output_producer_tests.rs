//! Real State rollback and linear-owner controls for the private output kernel.
//! These are unit controls, not qualification of the unfinished production producer.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    smartcontracts::isi::triggers::set::SetReadOnly,
    state::{State, TransactionsBlockError, World},
    sumeragi::witness,
};
use iroha_data_model::{
    Registrable,
    block::{BlockHeader, builder::BlockBuilder},
    isi::SetParameter,
    parameter::{BlockParameter, ExecutionOutputPolicyV1, Parameter},
    prelude::{InstructionBox, Level, Log, TransactionBuilder},
    transaction::{
        FeePaymentIntent,
        signed::{ExecutionStep, TransactionResult},
    },
    trigger::data::DataTriggerStep,
};
use iroha_model_base::name::Name;
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use mv::storage::StorageReadOnly;
use std::num::{NonZeroU32, NonZeroU64};

fn state(row_bytes: u64) -> State {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut parameters = state.world.parameters.block();
    let mut policy = ExecutionOutputPolicyV1::bootstrap();
    policy.max_output_bytes = row_bytes;
    policy.max_pipeline_triggers = 0;
    policy.max_time_invocations = 1;
    policy.validate().expect("feasible fixture policy");
    parameters
        .get_mut()
        .set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
    parameters.get_mut().set_parameter(Parameter::Block(
        BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::MIN),
    ));
    parameters.commit();
    state
}

fn source(state: &State, count: u32) -> SignedBlock {
    let mut builder = BlockBuilder::new(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    for index in 0..count {
        let signed = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([Log::new(Level::INFO, format!("source {index}"))])
        .sign(ALICE_KEYPAIR.private_key());
        builder.push_transaction(signed);
    }
    builder.build_with_signature(0, ALICE_KEYPAIR.private_key())
}

fn row(index: u32, trace_bytes: usize) -> NetworkExecutionOutputV1 {
    let steps = if trace_bytes == 0 {
        vec![]
    } else {
        vec![DataTriggerStep {
            id: "output_fixture".parse().unwrap(),
            instructions: ExecutionStep(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "x".repeat(trace_bytes),
                ))]
                .into(),
            ),
        }]
    };
    NetworkExecutionOutputV1 {
        input_index: index,
        result: TransactionResult::new(Ok(steps)),
        completions: vec![],
    }
}

fn write_state(transaction: &mut StateTransaction<'_, '_>, count: u64) {
    SetParameter::new(Parameter::Block(BlockParameter::MaxTransactions(
        NonZeroU64::new(count).unwrap(),
    )))
    .execute(&ALICE_ID, transaction)
    .unwrap();
    let key: Name = "output_write".parse().unwrap();
    witness::record_read_account_kv(&ALICE_ID, &key, Some(&Json::new(1)));
    witness::record_write_account_kv(&ALICE_ID, &key, &Json::new(count));
    #[cfg(feature = "zk-preverify")]
    assert!(transaction.zk_dedup.check_and_insert(&proof()));
}

#[cfg(feature = "zk-preverify")]
fn proof() -> iroha_data_model::proof::ProofBox {
    iroha_data_model::proof::ProofBox::new("output-fixture".into(), vec![7, 8, 9])
}

fn finish_empty_internal(producer: &mut ExecutionOutputProducer<'_, '_, '_>) -> Result<(), String> {
    // This fixture's actual registry has no Pipeline or Time actions.
    assert!(producer.state.world.triggers.pipeline_triggers().is_empty());
    assert!(producer.state.world.triggers.time_triggers().is_empty());
    producer.skip_uninvoked(ExecutionOutputPhase::Time, 1)
}

fn retained<'a>(block: &'a StateBlock<'_>) -> &'a RetainedExecutionOutputs {
    match block.execution_output_plan.as_ref().unwrap() {
        ExecutionOutputPlanState::Retained(outputs) => outputs,
        _ => panic!("producer did not retain its completed rows"),
    }
}

#[test]
fn fitting_complete_output_applies_state_and_retains_exact_row_once() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    let fragments = block.committed_fragment_count();
    let expected = row(0, 1024);
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            assert!(matches!(
                producer.state.execution_output_plan,
                Some(ExecutionOutputPlanState::Running)
            ));
            let capacity = producer.rows.capacity();
            assert_eq!(
                producer.try_apply_network_success(0, |input, transaction| {
                    assert_eq!(input, source.network_entrypoint_at(0).unwrap());
                    assert_eq!(
                        transaction.tx_call_hash,
                        Some(Hash::from(input.execution_call_hash()))
                    );
                    write_state(transaction, 7);
                    Ok(expected.clone())
                })?,
                NetworkSuccessDisposition::Applied
            );
            assert_eq!(producer.rows.capacity(), capacity);
            finish_empty_internal(producer)
        })
        .unwrap();
    assert_eq!(
        block
            .world
            .parameters
            .get()
            .block()
            .max_transactions()
            .get(),
        7
    );
    assert_eq!(block.committed_fragment_count(), fragments + 1);
    let outputs = retained(&block);
    assert_eq!(outputs.rows, [ExecutionOutputV1::Network(expected)]);
    assert_eq!(
        outputs.row_bytes,
        norito::canonical_frame_len(&outputs.rows[0]).unwrap() as u64
    );
    assert_eq!(witness::snapshot_exec_witness().writes.len(), 1);
    #[cfg(feature = "zk-preverify")]
    assert!(!block.zk_dedup.check_and_insert(&proof()));
    assert!(block.reserve_ordinary_execution_outputs(&source).is_err());
    assert!(
        block
            .produce_ordinary_execution_outputs(&source, |_| panic!("cannot execute twice"))
            .is_err()
    );
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn oversized_success_rolls_back_world_events_witness_and_dedup_before_terminal() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    let parameters = block.world.parameters.get().clone();
    let events = block.world.external_event_buf.len();
    let fragments = block.committed_fragment_count();
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            assert_eq!(
                producer.try_apply_network_success(0, |_, tx| {
                    write_state(tx, 7);
                    Ok(row(0, 32_768))
                })?,
                NetworkSuccessDisposition::OutputLimit
            );
            finish_empty_internal(producer)
        })
        .unwrap();
    assert_eq!(*block.world.parameters.get(), parameters);
    assert_eq!(block.world.external_event_buf.len(), events);
    assert_eq!(block.committed_fragment_count(), fragments);
    assert_eq!(
        retained(&block).rows,
        [ExecutionOutputV1::network_output_limit_rejection(0)]
    );
    let witness = witness::snapshot_exec_witness();
    assert!(witness.reads.is_empty());
    assert!(witness.writes.is_empty());
    #[cfg(feature = "zk-preverify")]
    assert!(block.zk_dedup.check_and_insert(&proof()));
}

#[test]
fn canonical_exact_row_limit_accepts_and_one_byte_less_rolls_back() {
    let actual = row(0, 16_384);
    let bytes =
        norito::canonical_frame_len(&ExecutionOutputV1::Network(actual.clone())).unwrap() as u64;
    for (limit, expected) in [
        (bytes, NetworkSuccessDisposition::Applied),
        (bytes - 1, NetworkSuccessDisposition::OutputLimit),
    ] {
        let state = state(limit);
        let source = source(&state, 1);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        block
            .produce_ordinary_execution_outputs(&source, |producer| {
                assert_eq!(
                    producer.try_apply_network_success(0, |_, _| Ok(actual.clone()))?,
                    expected
                );
                finish_empty_internal(producer)
            })
            .unwrap();
    }
}

#[test]
fn actual_execution_order_preserves_canonical_network_output_positions() {
    let state = state(16_384);
    let source = source(&state, 3);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            for index in [2, 0, 1] {
                assert_eq!(
                    producer.try_apply_network_success(index, |_, _| Ok(row(index, 0)))?,
                    NetworkSuccessDisposition::Applied
                );
            }
            finish_empty_internal(producer)
        })
        .unwrap();
    assert_eq!(
        retained(&block).rows,
        (0..3)
            .map(|i| ExecutionOutputV1::Network(row(i, 0)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn duplicate_or_foreign_network_index_never_executes_and_poison_is_sticky() {
    for index in [0, 1, u32::MAX] {
        let state = state(16_384);
        let source = source(&state, 1);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        assert!(
            block
                .produce_ordinary_execution_outputs(&source, |producer| {
                    producer.try_apply_network_success(0, |_, _| Ok(row(0, 0)))?;
                    assert!(
                        producer
                            .try_apply_network_success(index, |_, _| panic!(
                                "foreign/repeated work must not execute"
                            ))
                            .is_err()
                    );
                    // Deliberately swallow refusal: finish must still poison the owner.
                    Ok(())
                })
                .is_err()
        );
        assert!(matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Poisoned)
        ));
        assert!(block.reserve_ordinary_execution_outputs(&source).is_err());
        assert!(matches!(
            block.commit().unwrap_err(),
            TransactionsBlockError::ExecutionOutputCapacity
        ));
    }
}

#[test]
fn changed_source_cannot_take_reserved_plan_or_run_body() {
    let state = state(16_384);
    let source = source(&state, 1);
    let foreign = super::tests::source(&state, 2);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    assert!(
        block
            .produce_ordinary_execution_outputs(&foreign, |_| panic!("foreign source"))
            .is_err()
    );
    assert_eq!(block.reserved_output_input_count_for_test(), Some(1));
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            producer.try_apply_network_success(0, |_, _| Ok(row(0, 0)))?;
            finish_empty_internal(producer)
        })
        .unwrap();
}

#[test]
fn recursive_producer_entry_cannot_replace_running_owner() {
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            assert!(
                producer
                    .state
                    .produce_ordinary_execution_outputs(&source, |_| panic!("recursive execution"))
                    .is_err()
            );
            assert!(
                producer
                    .state
                    .reserve_ordinary_execution_outputs(&source)
                    .is_err()
            );
            producer.try_apply_network_success(0, |_, _| Ok(row(0, 0)))?;
            finish_empty_internal(producer)
        })
        .unwrap();
}

#[test]
fn unfinished_network_or_callback_obligation_cannot_finish() {
    for skip_network in [false, true] {
        let state = state(16_384);
        let source = source(&state, 1);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        assert!(
            block
                .produce_ordinary_execution_outputs(&source, |producer| {
                    if !skip_network {
                        producer.try_apply_network_success(0, |_, _| Ok(row(0, 0)))?;
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Poisoned)
        ));
    }
}

#[test]
fn local_refusal_rolls_back_and_never_becomes_a_canonical_rejection() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    let before = block.world.parameters.get().clone();
    let error = block
        .produce_ordinary_execution_outputs(&source, |producer| {
            producer.try_apply_network_success(0, |_, tx| {
                write_state(tx, 7);
                Err("fixture local serializer refusal".into())
            })?;
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error, "fixture local serializer refusal");
    assert_eq!(*block.world.parameters.get(), before);
    assert!(witness::snapshot_exec_witness().writes.is_empty());
    assert!(matches!(
        block.execution_output_plan,
        Some(ExecutionOutputPlanState::Poisoned)
    ));
    #[cfg(feature = "zk-preverify")]
    assert!(block.zk_dedup.check_and_insert(&proof()));
}

#[test]
fn unwind_rolls_back_side_channels_and_cannot_reopen_publication() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    let before = block.world.parameters.get().clone();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = block.produce_ordinary_execution_outputs(&source, |producer| {
            producer.try_apply_network_success(0, |_, tx| {
                write_state(tx, 7);
                panic!("fixture interruption before output fit");
            })?;
            Ok(())
        });
    }));
    assert!(caught.is_err());
    assert_eq!(*block.world.parameters.get(), before);
    assert!(witness::snapshot_exec_witness().writes.is_empty());
    assert!(matches!(
        block.execution_output_plan,
        Some(ExecutionOutputPlanState::Poisoned)
    ));
    #[cfg(feature = "zk-preverify")]
    assert!(block.zk_dedup.check_and_insert(&proof()));
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn origin_substitution_and_real_rejection_refuse_before_state_application() {
    for case in 0..4 {
        let state = state(16_384);
        let source = source(&state, 1);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        let fragments = block.committed_fragment_count();
        assert!(
            block
                .produce_ordinary_execution_outputs(&source, |producer| {
                    producer.try_apply_network_success(0, |_, transaction| {
                        let mut actual = row(0, 0);
                        match case {
                            0 => actual.input_index = 1,
                            1 => transaction.tx_call_hash = Some(Hash::new(b"foreign call")),
                            2 => transaction.current_tx_hash = None,
                            _ => {
                                actual.result = ExecutionOutputV1::network_output_limit_rejection(0)
                                    .result()
                                    .clone()
                            }
                        }
                        Ok(actual)
                    })?;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(block.committed_fragment_count(), fragments);
        assert!(matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Poisoned)
        ));
    }
}

#[test]
fn invalid_skip_cannot_erase_network_obligation_even_if_error_is_ignored() {
    let state = state(16_384);
    let source = source(&state, 1);
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    assert!(
        block
            .produce_ordinary_execution_outputs(&source, |producer| {
                assert!(
                    producer
                        .skip_uninvoked(ExecutionOutputPhase::Network, 1)
                        .is_err()
                );
                assert!(
                    producer
                        .try_apply_network_success(0, |_, _| panic!("poisoned producer"))
                        .is_err()
                );
                Ok(())
            })
            .is_err()
    );
    assert!(matches!(
        block.execution_output_plan,
        Some(ExecutionOutputPlanState::Poisoned)
    ));
}

// This fixture executes the real independent transfer ISI directly under the
// private transaction kernel. It does not claim signature/admission, fee,
// callback, full-wire or finalized-block execution qualification.
fn receipt_source(
    row_bytes: u64,
) -> (
    State,
    SignedBlock,
    iroha_data_model::asset::AssetId,
    iroha_data_model::asset::AssetId,
) {
    use iroha_data_model::{
        account::Account,
        asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
        domain::Domain,
        isi::{TransferAssetBatch, TransferAssetBatchEntry},
    };
    use iroha_model_base::domain::DomainId;
    use iroha_test_samples::BOB_ID;

    let domain = DomainId::try_new("output-receipts", "universal").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    let source_asset = AssetId::new(definition.clone(), ALICE_ID.clone());
    let destination_asset = AssetId::new(definition.clone(), BOB_ID.clone());
    let world = World::with_assets(
        [Domain::new(domain).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [AssetDefinition::numeric(
            definition.clone(),
            "output receipt coin",
            AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID)],
        [Asset::new(source_asset.clone(), 10u32)],
        [],
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut parameters = state.world.parameters.block();
    let mut policy = ExecutionOutputPolicyV1::bootstrap();
    policy.max_output_bytes = row_bytes;
    policy.max_pipeline_triggers = 0;
    policy.max_time_invocations = 1;
    policy.validate().expect("finite receipt fixture policy");
    parameters
        .get_mut()
        .set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
    parameters.get_mut().set_parameter(Parameter::Block(
        BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::MIN),
    ));
    parameters.commit();

    // A legal long leg identity makes actual receipts exceed the minimum
    // terminal ceiling without inventing an executed callback trace.
    let batch = TransferAssetBatch::independent(vec![
        TransferAssetBatchEntry::with_leg_id(
            "a".repeat(16_384),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            definition.clone(),
            3u32,
        ),
        TransferAssetBatchEntry::with_leg_id(
            "insufficient",
            ALICE_ID.clone(),
            BOB_ID.clone(),
            definition,
            1000u32,
        ),
    ]);
    let header = BlockHeader::new(NonZeroU64::MIN, None, None, 7, 0);
    let mut transaction = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    );
    transaction.set_creation_time(header.creation_time());
    let signed = transaction
        .with_instructions([batch])
        .sign(ALICE_KEYPAIR.private_key());
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(signed);
    let source = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    (state, source, source_asset, destination_asset)
}

#[test]
fn real_independent_receipts_are_joined_before_exact_row_fit_and_rollback() {
    use iroha_data_model::{
        events::data::prelude::AssetBatchTransferLegStatus, transaction::Executable,
    };
    use iroha_primitives::numeric::Quantity;

    let _guard = witness::exec_witness_guard();
    let mut exact = None::<ExecutionOutputV1>;
    // First obtain the actual successful row, then test its exact measured
    // boundary and one byte less on fresh independent State overlays.
    for case in 0..3 {
        witness::start_block();
        let full_bytes = exact
            .as_ref()
            .map(|row| u64::try_from(norito::canonical_frame_len(row).unwrap()).unwrap());
        let limit = match case {
            0 => 65_536,
            1 => full_bytes.unwrap(),
            _ => full_bytes.unwrap() - 1,
        };
        let applies = case != 2;
        let (state, source, source_asset, destination_asset) = receipt_source(limit);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        let fragments = block.committed_fragment_count();
        let events = block.world.external_event_buf.len();
        let before_witness = witness::snapshot_exec_witness();
        let mut actual_receipts = None;
        block
            .produce_ordinary_execution_outputs(&source, |producer| {
                assert_eq!(
                    producer.try_apply_network_success(0, |input, transaction| {
                        let TransactionEntrypoint::External(signed) = input else {
                            panic!("fixture is one external independent batch");
                        };
                        let Executable::Instructions(instructions) = signed.instructions() else {
                            panic!("fixture contains actual native instructions");
                        };
                        for instruction in instructions.iter() {
                            instruction
                                .clone()
                                .execute(signed.authority(), transaction)
                                .unwrap();
                        }
                        let key = input.execution_call_hash();
                        assert_eq!(transaction.pending_batch_transfer_outcomes.len(), 1);
                        let receipts = &transaction.pending_batch_transfer_outcomes[&key];
                        assert_eq!(receipts.len(), 2);
                        assert!(matches!(
                            receipts[0].status,
                            AssetBatchTransferLegStatus::Applied
                        ));
                        assert!(matches!(
                            receipts[1].status,
                            AssetBatchTransferLegStatus::Rejected(_)
                        ));
                        actual_receipts = Some(receipts.clone());
                        assert_eq!(
                            transaction.pending_transfer_transcript_count_for_testing(),
                            1
                        );
                        // The closure returns no receipts. Only the kernel may
                        // join its actual transaction-owned map before sizing.
                        let without_receipts = row(0, 0);
                        assert!(
                            u64::try_from(
                                norito::canonical_frame_len(&ExecutionOutputV1::Network(
                                    without_receipts.clone()
                                ))
                                .unwrap()
                            )
                            .unwrap()
                                < limit
                        );
                        Ok(without_receipts)
                    })?,
                    if applies {
                        NetworkSuccessDisposition::Applied
                    } else {
                        NetworkSuccessDisposition::OutputLimit
                    }
                );
                finish_empty_internal(producer)
            })
            .unwrap();
        let result = &retained(&block).rows[0];
        assert!(
            block.batch_transfer_outcomes.is_empty(),
            "receipts cannot remain for a second post-fit attachment"
        );
        if applies {
            assert_eq!(
                result.result().batch_transfer_outcomes(),
                actual_receipts.as_ref().unwrap()
            );
            assert_eq!(block.committed_fragment_count(), fragments + 1);
            assert_eq!(
                block.world.assets.get(&source_asset).unwrap().as_ref(),
                &Quantity::from(7u32)
            );
            assert_eq!(
                block.world.assets.get(&destination_asset).unwrap().as_ref(),
                &Quantity::from(3u32)
            );
            assert!(block.world.external_event_buf.len() > events);
            assert_eq!(block.fastpq_transcripts.len(), 1);
            assert_eq!(block.captured_fastpq_transcript_sources().unwrap().len(), 1);
            if let Some(expected) = &exact {
                assert_eq!(result, expected);
            } else {
                exact = Some(result.clone());
            }
        } else {
            assert_eq!(
                result,
                &ExecutionOutputV1::network_output_limit_rejection(0)
            );
            assert!(result.result().batch_transfer_outcomes().is_empty());
            assert_eq!(block.committed_fragment_count(), fragments);
            assert_eq!(
                block.world.assets.get(&source_asset).unwrap().as_ref(),
                &Quantity::from(10u32)
            );
            assert!(block.world.assets.get(&destination_asset).is_none());
            assert_eq!(block.world.external_event_buf.len(), events);
            assert!(block.fastpq_transcripts.is_empty());
            assert!(
                block
                    .captured_fastpq_transcript_sources()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(witness::snapshot_exec_witness(), before_witness);
        }
        assert_eq!(
            retained(&block).row_bytes,
            u64::try_from(norito::canonical_frame_len(result).unwrap()).unwrap()
        );
        drop(block);
        assert_eq!(
            state
                .world
                .assets
                .view()
                .get(&source_asset)
                .unwrap()
                .as_ref(),
            &Quantity::from(10u32)
        );
        assert!(state.world.assets.view().get(&destination_asset).is_none());
    }
}

#[test]
fn unowned_callbacks_or_receipts_refuse_before_real_batch_application() {
    use iroha_data_model::{
        block::execution_output::InvocationCompletionV1,
        events::trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
        transaction::Executable,
    };
    use iroha_primitives::numeric::Quantity;

    let _guard = witness::exec_witness_guard();
    for case in 0..4 {
        witness::start_block();
        let (state, source, source_asset, destination_asset) = receipt_source(65_536);
        let mut block = state.block(source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        let fragments = block.committed_fragment_count();
        let events = block.world.external_event_buf.len();
        assert!(
            block
                .produce_ordinary_execution_outputs(&source, |producer| {
                    producer.try_apply_network_success(0, |input, transaction| {
                        let TransactionEntrypoint::External(signed) = input else {
                            unreachable!()
                        };
                        let Executable::Instructions(instructions) = signed.instructions() else {
                            unreachable!()
                        };
                        for instruction in instructions.iter() {
                            instruction
                                .clone()
                                .execute(signed.authority(), transaction)
                                .unwrap();
                        }
                        let mut actual = row(0, 0);
                        let id = "unowned_callback".parse().unwrap();
                        match case {
                            0 => actual.completions.push(InvocationCompletionV1 {
                                callback_index: 0,
                                trigger_id: id,
                                outcome: TriggerCompletedOutcome::Success,
                            }),
                            1 => transaction.world.external_event_buf.push(
                                TriggerCompletedEvent::new(
                                    id,
                                    input.execution_call_hash(),
                                    0,
                                    TriggerCompletedOutcome::Success,
                                )
                                .into(),
                            ),
                            2 => actual.result.set_batch_transfer_outcomes(
                                transaction.pending_batch_transfer_outcomes
                                    [&input.execution_call_hash()]
                                    .clone(),
                            ),
                            _ => {
                                let receipts = transaction
                                    .pending_batch_transfer_outcomes
                                    .remove(&input.execution_call_hash())
                                    .unwrap();
                                transaction.pending_batch_transfer_outcomes.insert(
                                    HashOf::from_untyped_unchecked(Hash::new(
                                        b"foreign receipt call",
                                    )),
                                    receipts,
                                );
                            }
                        }
                        Ok(actual)
                    })?;
                    Ok(())
                })
                .is_err()
        );
        assert!(matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Poisoned)
        ));
        assert_eq!(
            block.world.assets.get(&source_asset).unwrap().as_ref(),
            &Quantity::from(10u32)
        );
        assert!(block.world.assets.get(&destination_asset).is_none());
        assert_eq!(block.world.external_event_buf.len(), events);
        assert_eq!(block.committed_fragment_count(), fragments);
        assert!(block.batch_transfer_outcomes.is_empty());
        assert!(block.fastpq_transcripts.is_empty());
        assert!(
            block
                .captured_fastpq_transcript_sources()
                .unwrap()
                .is_empty()
        );
        assert!(witness::snapshot_exec_witness().writes.is_empty());
    }
}

#[path = "output_callback_tests.rs"]
mod callbacks;

#[path = "output_time_tests.rs"]
mod scheduled_time;

#[test]
fn completed_work_survives_healthy_output_overflow_and_bounds_the_next_overlay() {
    let _guard = witness::exec_witness_guard();
    for oversized in [false, true] {
        let state = state(16_384);
        let source = source(&state, 1);
        witness::start_block();
        let mut block = state.block(source.header());
        block.zk.max_confidential_ops_per_block = 1;
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        block
            .produce_ordinary_execution_outputs(&source, |producer| {
                let disposition = producer.try_apply_network_success(0, |_, transaction| {
                    // Exercise the real work-registration APIs; this is budget
                    // ownership coverage, not cryptographic verifier qualification.
                    transaction
                        .register_confidential_proof(13)
                        .map_err(|e| e.to_string())?;
                    transaction
                        .charge_trigger_work_gas(7, "completed test work")
                        .map_err(|e| e.to_string())?;
                    transaction.record_confidential_gas_delta(19);
                    write_state(transaction, 77);
                    Ok(row(0, if oversized { 32_768 } else { 0 }))
                })?;
                assert_eq!(
                    disposition,
                    if oversized {
                        NetworkSuccessDisposition::OutputLimit
                    } else {
                        NetworkSuccessDisposition::Applied
                    }
                );
                finish_empty_internal(producer)
            })
            .unwrap();
        assert_eq!(block.gas_used_in_block, 7);
        assert_eq!(block.zk_confidential_ops_in_block, 1);
        assert_eq!(block.zk_verify_calls_in_block, 1);
        assert_eq!(block.zk_proof_bytes_in_block, 13);
        assert_eq!(block.confidential_gas_used_in_block, 19);
        assert_eq!(
            block
                .world
                .parameters
                .get()
                .block()
                .max_transactions()
                .get()
                == 77,
            !oversized
        );
        let mut next = block.transaction();
        assert!(
            next.register_confidential_proof(1).is_err(),
            "completed work consumes the next overlay's shared budget even after business rollback"
        );
    }
}

#[path = "output_network_tests.rs"]
mod network;
