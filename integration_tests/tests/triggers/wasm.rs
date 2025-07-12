#![allow(missing_docs)]

use eyre::Result;
use iroha::data_model::prelude::*;
use iroha_data_model::isi::error::InstructionExecutionError;
use iroha_test_network::*;
use iroha_test_samples::{load_sample_wasm, ALICE_ID};
use mint_rose_trigger_data_model::MintRoseArgs;

#[test]
fn multiple_wasm_triggers_in_transaction() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let rose: AssetDefinitionId = "rose#wonderland".parse()?;

    let roses_before = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.account.eq(ALICE_ID.clone()))
        .filter_with(|asset| asset.id.definition.eq(rose.clone()))
        .execute_single()
        .unwrap();

    let trigger_id = "rose_farm".parse::<TriggerId>()?;
    let trigger_instructions = vec![
        WasmExecutable::binary(load_sample_wasm("mint_rose_trigger")),
        WasmExecutable::binary(load_sample_wasm("mint_rose_trigger")),
        WasmExecutable::binary(load_sample_wasm("mint_rose_trigger")),
    ];
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_instructions,
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        ),
    ));
    client.submit_blocking(register_trigger)?;

    // Args are shared between calls
    let args = &MintRoseArgs { val: 1 };
    client.submit_blocking(ExecuteTrigger::new(trigger_id).with_args(args))?;

    let roses_after = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.account.eq(ALICE_ID.clone()))
        .filter_with(|asset| asset.id.definition.eq(rose.clone()))
        .execute_single()
        .unwrap();

    assert_eq!(
        roses_before.value().checked_add(3_u32.into()).unwrap(),
        roses_after.value().clone()
    );

    Ok(())
}

#[test]
fn smartcontract_execute_fail_in_trigger() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let trigger_id = "rose_farm".parse::<TriggerId>()?;
    let trigger_instructions = vec![
        WasmExecutable::binary(load_sample_wasm("mint_rose_trigger")),
        WasmExecutable::binary(load_sample_wasm("mint_rose_smartcontract")),
    ];
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_instructions,
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        ),
    ));
    client.submit_blocking(register_trigger)?;

    // Args are shared between calls
    let args = &MintRoseArgs { val: 1 };
    let error = client
        .submit_blocking(ExecuteTrigger::new(trigger_id).with_args(args))
        .unwrap_err();

    let Some(InstructionExecutionError::InvariantViolation(msg)) = error
        .root_cause()
        .downcast_ref::<InstructionExecutionError>(
    ) else {
        panic!("unexpected error")
    };
    assert!(msg.contains("Validation failed"));

    Ok(())
}
