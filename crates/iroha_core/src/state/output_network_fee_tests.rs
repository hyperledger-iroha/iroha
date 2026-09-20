//! Actual rejected Instructions/Batch fee ownership, rollback and block-gas boundaries.

use super::*;
use iroha_data_model::{
    asset::{AssetDefinitionId, AssetId},
    events::data::prelude::{AccountEventFilter, DataEventFilter},
    transaction::{FeeChargeKind, FeeChargeLimit},
};
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::numeric::Quantity;

fn priced_fixture(callback_bytes: Option<usize>) -> (State, AssetDefinitionId) {
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("network-fee", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let mut state = fixture_with_fee_asset(65_536, callback_bytes, Some(asset.clone()));
    state.nexus.get_mut().fees.per_instruction_fee = Quantity::from(1_u32);
    (state, asset)
}

fn payment(asset: &AssetDefinitionId, maximum: u32) -> FeePaymentIntent {
    FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            asset.clone(),
            Quantity::from(maximum),
        )],
        None,
    )
}

fn balance(
    block: &StateBlock<'_>,
    asset: &AssetDefinitionId,
    who: &iroha_data_model::account::AccountId,
) -> Quantity {
    block
        .world
        .assets()
        .get(&AssetId::of(asset.clone(), who.clone()))
        .map_or(Quantity::zero(), |value| value.0.clone())
}

fn write(key: &str) -> InstructionBox {
    SetKeyValue::account(ALICE_ID.clone(), key.parse().unwrap(), Json::new(1)).into()
}

fn failing_body() -> Vec<InstructionBox> {
    vec![
        write("rolled_back_fee_body"),
        Unregister::trigger("fee_missing_trigger".parse().unwrap()).into(),
    ]
}

#[test]
fn plain_and_batch_business_rejection_charge_the_same_actual_direct_basis() {
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    for batch in [false, true] {
        let (state, asset) = priced_fixture(None);
        let source = carrier(vec![input(
            &state,
            failing_body(),
            payment(&asset, 3),
            batch,
        )]);
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        assert!(network_row(&block, 0).result.is_err());
        assert_eq!(balance(&block, &asset, &ALICE_ID), Quantity::from(7_u32));
        assert_eq!(
            balance(&block, &asset, &iroha_test_samples::BOB_ID),
            Quantity::zero()
        );
        assert_eq!(
            block
                .world
                .asset_definition(&asset)
                .unwrap()
                .total_quantity(),
            &Quantity::from(7_u32)
        );
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("rolled_back_fee_body")
                .is_none()
        );
        assert_eq!(block.committed_fragment_count(), fragments + 1);
        assert!(
            network_row(&block, 0)
                .result
                .batch_transfer_outcomes()
                .is_empty()
        );
        assert!(network_row(&block, 0).completions.is_empty());
        assert!(block.gas_used_in_block > 0);
    }
}

#[test]
fn fee_only_settlement_does_not_charge_completed_work_again_at_the_block_limit() {
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    let body = failing_body();
    let gas = crate::gas::meter_instructions(&body);
    assert!(gas > 0);
    for exact in [false, true] {
        let (state, asset) = priced_fixture(None);
        let source = carrier(vec![input(&state, body.clone(), payment(&asset, 3), false)]);
        witness::start_block();
        let mut block = state.block(source.header());
        block.gas_limit_per_block = if exact { gas } else { gas - 1 };
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        assert!(network_row(&block, 0).result.is_err());
        assert_eq!(block.gas_used_in_block, if exact { gas } else { 0 });
        assert_eq!(
            balance(&block, &asset, &ALICE_ID),
            Quantity::from(if exact { 7_u32 } else { 10 })
        );
        assert_eq!(
            block.committed_fragment_count(),
            fragments + usize::from(exact)
        );
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("rolled_back_fee_body")
                .is_none()
        );
    }
}

#[test]
fn fee_admission_failure_has_no_business_fee_record_or_applied_fragment() {
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    let (state, asset) = priced_fixture(None);
    let source = carrier(vec![input(
        &state,
        failing_body(),
        payment(&asset, 2),
        false,
    )]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(network_row(&block, 0).result.is_err());
    assert_eq!(balance(&block, &asset, &ALICE_ID), Quantity::from(10_u32));
    assert_eq!(
        balance(&block, &asset, &iroha_test_samples::BOB_ID),
        Quantity::zero()
    );
    assert_eq!(block.gas_used_in_block, 0);
    assert_eq!(block.committed_fragment_count(), fragments);
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("rolled_back_fee_body")
            .is_none()
    );
}

#[test]
fn actual_data_callback_failure_rolls_back_business_but_preserves_root_fee_basis() {
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    let (state, asset) = priced_fixture(None);
    let callback: TriggerId = "fee_data_callback".parse().unwrap();
    let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    let mut transaction = setup.transaction();
    Register::trigger(Trigger::new(
        callback.clone(),
        Action::new(
            vec![
                write("rolled_back_callback"),
                Unregister::trigger("absent_in_callback".parse().unwrap()).into(),
            ],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            DataEventFilter::Account(AccountEventFilter::new().for_account(ALICE_ID.clone())),
        )
        .unwrap(),
    ))
    .execute(&ALICE_ID, &mut transaction)
    .unwrap();
    transaction.apply();
    setup.commit_world_overlay_for_testing().unwrap();
    let body = vec![write("data_callback_event")];
    let direct_gas = crate::gas::meter_instructions(&body);
    let source = carrier(vec![input(&state, body, payment(&asset, 2), false)]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(network_row(&block, 0).result.is_err());
    assert_eq!(balance(&block, &asset, &ALICE_ID), Quantity::from(8_u32));
    assert_eq!(
        balance(&block, &asset, &iroha_test_samples::BOB_ID),
        Quantity::zero()
    );
    assert_eq!(
        block
            .world
            .asset_definition(&asset)
            .unwrap()
            .total_quantity(),
        &Quantity::from(8_u32)
    );
    for key in ["data_callback_event", "rolled_back_callback"] {
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(key)
                .is_none()
        );
    }
    assert_eq!(
        block
            .world
            .triggers
            .data_triggers()
            .get(&callback)
            .unwrap()
            .repeats,
        Repeats::Exactly(1)
    );
    assert!(network_row(&block, 0).completions.is_empty());
    assert!(
        network_row(&block, 0)
            .result
            .batch_transfer_outcomes()
            .is_empty()
    );
    assert!(block.gas_used_in_block > direct_gas);
    assert_eq!(block.committed_fragment_count(), fragments + 1);
}

#[test]
fn healthy_output_overflow_drops_both_business_and_its_staged_fee() {
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    let (state, asset) = priced_fixture(Some(32_768));
    {
        let mut parameters = state.world.parameters.block();
        let mut policy = parameters.get().block().execution_output();
        policy.max_output_bytes = 16_384;
        parameters
            .get_mut()
            .set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
        parameters.commit();
    }
    let source = carrier(vec![input(
        &state,
        vec![ExecuteTrigger::new("network_callback".parse().unwrap()).into()],
        payment(&asset, 2),
        false,
    )]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(retained(&block).rows[0].is_output_limit_rejection());
    assert_eq!(balance(&block, &asset, &ALICE_ID), Quantity::from(10_u32));
    assert_eq!(
        balance(&block, &asset, &iroha_test_samples::BOB_ID),
        Quantity::zero()
    );
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("callback_write")
            .is_none()
    );
    assert_eq!(block.committed_fragment_count(), fragments);
    assert!(block.gas_used_in_block > 0);
}

fn bind_fee_context(
    transaction: &mut StateTransaction<'_, '_>,
    signed: &iroha_data_model::transaction::SignedTransaction,
) {
    transaction.current_entrypoint_index = Some(0);
    transaction.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    transaction.current_tx_hash = Some(signed.hash());
    transaction.current_lane_id = Some(iroha_model_base::topology::LaneId::SINGLE);
    transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
}

#[test]
fn actual_failed_execution_fee_authority_is_once_only_and_bound_to_its_source_context() {
    use crate::{
        executor::ExecutionFeeSettlementError, queue::RoutingDecision,
        smartcontracts::ivm::cache::IvmCache, tx::AcceptedTransaction,
    };
    use iroha_model_base::topology::LaneId;
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    for mutation in 0..11 {
        let (state, asset) = priced_fixture(None);
        let entry = input(&state, failing_body(), payment(&asset, 3), false);
        let TransactionEntrypoint::External(signed) = &entry else {
            unreachable!()
        };
        let foreign = input(
            &state,
            vec![write("another_signed_source")],
            payment(&asset, 3),
            false,
        );
        let TransactionEntrypoint::External(foreign_signed) = &foreign else {
            unreachable!()
        };
        let source = carrier(vec![entry.clone()]);
        witness::start_block();
        let mut block = state.block(source.header());
        let parameters = block.world.parameters.get();
        let accepted = AcceptedTransaction::accept_borrowed_entrypoint_at_time(
            &entry,
            &state.network_id,
            parameters.sumeragi().max_clock_drift(),
            parameters.transaction(),
            &block.crypto,
            source.header().creation_time(),
        )
        .unwrap();
        let mut cache = IvmCache::with_prepared_contract_cache(
            block.pipeline.cache_size,
            block.pipeline_ivm_prepared_cache.clone(),
        );
        let mut failed = block.transaction();
        bind_fee_context(&mut failed, signed);
        assert!(
            StateBlock::execute_accepted_transaction_in_overlay(
                accepted,
                &mut failed,
                &mut cache,
                Some(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
            )
            .is_err()
        );
        let completed_gas = failed.last_tx_gas_used;
        assert!(completed_gas > 0);
        let basis = failed
            .take_execution_fee_settlement()
            .unwrap()
            .expect("actual root fee basis");
        assert!(failed.take_execution_fee_settlement().unwrap().is_none());
        drop(failed);
        let mut fee = block.transaction();
        bind_fee_context(&mut fee, signed);
        // Price mutation here tests the immutable admitted snapshot, not on-chain
        // governance authorization. The attempted body above is real execution.
        fee.nexus.fees.base_fee = Quantity::from(100_u32);
        match mutation {
            0 => {}
            1 => fee.current_tx_hash = None,
            2 => fee.tx_call_hash = None,
            3 => fee.current_entrypoint_index = Some(1),
            4 => fee.current_lane_id = Some(LaneId::new(1)),
            5 => {
                fee.current_dataspace_id = Some(DataSpaceId::new(1));
                fee.world.current_dataspace_id = Some(DataSpaceId::new(1));
            }
            6 => fee.world.current_dataspace_id = None,
            7 => fee._curr_block = BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0),
            8 => fee.last_tx_gas_used = completed_gas,
            9 => {
                fee.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0).hash(),
                )
            }
            10 => {}
            _ => unreachable!(),
        }
        let result = basis.settle(
            &mut fee,
            if mutation == 10 {
                foreign_signed
            } else {
                signed
            },
        );
        if mutation == 0 {
            assert!(matches!(result, Ok(true)));
            assert_eq!(
                fee.last_tx_gas_used, 0,
                "charging a price does not execute work again"
            );
            fee.apply();
        } else {
            assert!(matches!(result, Err(ExecutionFeeSettlementError::Owner(_))));
            drop(fee);
        }
        assert_eq!(
            balance(&block, &asset, &ALICE_ID),
            Quantity::from(if mutation == 0 { 7_u32 } else { 10 })
        );
        assert_eq!(
            balance(&block, &asset, &iroha_test_samples::BOB_ID),
            Quantity::zero()
        );
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("rolled_back_fee_body")
                .is_none()
        );
    }
}

#[test]
fn actual_raw_vm_rejection_retains_and_charges_consumed_work_once() {
    use iroha_data_model::{
        ValidationFail,
        transaction::{IvmBytecode, error::TransactionRejectionReason},
    };
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    let (state, asset) = priced_fixture(None);
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000,
        ..Default::default()
    }
    .encode();
    for _ in 0..100 {
        program.extend_from_slice(
            &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 5, 5, 1)
                .to_le_bytes(),
        );
    }
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let gas_limit = 3;
    let fee = FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            asset.clone(),
            Quantity::from(1_u32),
        )],
        NonZeroU64::new(gas_limit),
    );
    let mut builder = TransactionBuilder::new(state.network_id, ALICE_ID.clone(), fee);
    builder.set_creation_time(Duration::from_millis(1));
    let signed = builder
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(ALICE_KEYPAIR.private_key());
    let source = carrier(vec![TransactionEntrypoint::External(signed)]);
    witness::start_block();
    let mut block = state.block(source.header());
    block.gas_limit_per_block = gas_limit;
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(matches!(network_row(&block, 0).result.as_ref(),
        Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)))
        if reason.contains("gas")));
    assert_eq!(block.gas_used_in_block, gas_limit);
    assert_eq!(balance(&block, &asset, &ALICE_ID), Quantity::from(9_u32));
    assert_eq!(
        balance(&block, &asset, &iroha_test_samples::BOB_ID),
        Quantity::zero()
    );
    assert_eq!(
        block
            .world
            .asset_definition(&asset)
            .unwrap()
            .total_quantity(),
        &Quantity::from(9_u32)
    );
    assert_eq!(block.committed_fragment_count(), fragments + 1);
    assert!(network_row(&block, 0).completions.is_empty());
}
