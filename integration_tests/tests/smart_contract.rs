#![allow(missing_docs)]

use eyre::Result;
use iroha::data_model::prelude::*;
use iroha_data_model::isi::error::WasmExecutionError;
use iroha_test_network::*;
use iroha_test_samples::{load_sample_wasm, ALICE_ID};

#[test]
fn multiple_smartcontracts_in_transaction() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let rose: AssetDefinitionId = "rose#wonderland".parse()?;

    let roses_before = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.account.eq(ALICE_ID.clone()))
        .filter_with(|asset| asset.id.definition.eq(rose.clone()))
        .execute_single()
        .unwrap();

    let transaction = client.build_transaction(
        vec![
            WasmExecutable::binary(load_sample_wasm("mint_rose_smartcontract")),
            WasmExecutable::binary(load_sample_wasm("mint_rose_smartcontract")),
            WasmExecutable::binary(load_sample_wasm("mint_rose_smartcontract")),
        ],
        Metadata::default(),
    );
    client.submit_transaction_blocking(&transaction)?;

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
fn trigger_wasm_execute_fail_outside_of_trigger() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let transaction = client.build_transaction(
        vec![
            WasmExecutable::binary(load_sample_wasm("mint_rose_smartcontract")),
            WasmExecutable::binary(load_sample_wasm("mint_rose_trigger")),
        ],
        Metadata::default(),
    );
    let error = client
        .submit_transaction_blocking(&transaction)
        .unwrap_err();

    assert!(error
        .root_cause()
        .downcast_ref::<WasmExecutionError>()
        .unwrap()
        .reason()
        .contains("Export error"));

    Ok(())
}
