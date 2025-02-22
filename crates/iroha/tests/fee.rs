#![allow(unused_imports)]
#![allow(unused_variables)]

use eyre::Result;
use iroha::{
    client,
    crypto::KeyPair,
    data_model::{
        asset::{AssetId, AssetType, AssetValue},
        isi::error::{InstructionEvaluationError, InstructionExecutionError, TypeError},
        prelude::*,
        transaction::error::TransactionRejectionReason,
    },
};
use iroha_data_model::fee::FeeReceiverDefinition;
use iroha_executor_data_model::permission::asset::CanTransferAsset;
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, ALICE_ID, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID};

#[test]
fn fee_receiver_declared_in_genesis() -> Result<()> {
    let alice_id = ALICE_ID.clone();
    let cabbage_id: AssetDefinitionId = "cabbage#garden_of_live_flowers".parse()?;

    let declare_fee_receiver = Declare::fee_receiver(FeeReceiverDefinition::new(
        alice_id.clone(),
        cabbage_id.clone(),
    ));

    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(declare_fee_receiver)
        .start_blocking()?;

    let test_client = network.client();

    let result = test_client.query_single(FindFeeReceiverAccount::new())?;
    let fees_recipient = result.expect("Fee receiver must be declared");
    assert_eq!(fees_recipient, alice_id);

    let result = test_client.query_single(FindFeePaymentAsset::new())?;
    let fees_asset = result.expect("Fee asset must be declared");
    assert_eq!(fees_asset, cabbage_id);

    Ok(())
}

#[test]
fn fee_receiver_absent_in_default_genesis() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();

    let result = test_client.query_single(FindFeeReceiverAccount::new())?;
    assert!(result.is_none());

    let result = test_client.query_single(FindFeePaymentAsset::new())?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn fee_receiver_cannot_change() -> Result<()> {
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let cabbage_id: AssetDefinitionId = "cabbage#garden_of_live_flowers".parse()?;
    let rose_id: AssetDefinitionId = "rose#wonderland".parse()?;

    let declare_fee_receiver = Declare::fee_receiver(FeeReceiverDefinition::new(
        alice_id.clone(),
        cabbage_id.clone(),
    ));
    let redeclare_fee_receiver =
        Declare::fee_receiver(FeeReceiverDefinition::new(bob_id.clone(), rose_id.clone()));

    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(declare_fee_receiver)
        .with_genesis_instruction(redeclare_fee_receiver)
        .start_blocking()?;

    let test_client = network.client();

    let result = test_client.query_single(FindFeeReceiverAccount::new())?;
    let fees_recipient = result.expect("Fee receiver must be declared");
    assert_eq!(fees_recipient, alice_id);

    let result = test_client.query_single(FindFeePaymentAsset::new())?;
    let fees_asset = result.expect("Fee asset must be declared");
    assert_eq!(fees_asset, cabbage_id);

    Ok(())
}

#[test]
fn fee_receiver_cannot_declare_outside_genesis() -> Result<()> {
    let alice_id = ALICE_ID.clone();
    let cabbage_id: AssetDefinitionId = "cabbage#garden_of_live_flowers".parse()?;

    let declare_fee_receiver = Declare::fee_receiver(FeeReceiverDefinition::new(
        alice_id.clone(),
        cabbage_id.clone(),
    ));

    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();

    let result = test_client.submit_blocking(declare_fee_receiver);

    assert!(result.is_err());

    Ok(())
}

#[test]
fn fee_receiver_shared() -> Result<()> {
    let alice_id = ALICE_ID.clone();
    let cabbage_id: AssetDefinitionId = "cabbage#garden_of_live_flowers".parse()?;

    let declare_fee_receiver = Declare::fee_receiver(FeeReceiverDefinition::new(
        alice_id.clone(),
        cabbage_id.clone(),
    ));

    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(declare_fee_receiver)
        .with_peers(3)
        .start_blocking()?;

    let mut peers = network.peers().iter();
    let peer_a = peers.next().unwrap();
    let peer_b = peers.next().unwrap();

    let test_client = peer_a.client();

    let result = test_client.query_single(FindFeeReceiverAccount::new())?;
    let fees_recipient = result.expect("Fee receiver must be declared");
    assert_eq!(fees_recipient, alice_id);

    let result = test_client.query_single(FindFeePaymentAsset::new())?;
    let fees_asset = result.expect("Fee asset must be declared");
    assert_eq!(fees_asset, cabbage_id);

    let test_client = peer_b.client();

    let result = test_client.query_single(FindFeeReceiverAccount::new())?;
    let fees_recipient = result.expect("Fee receiver must be declared");
    assert_eq!(fees_recipient, alice_id);

    let result = test_client.query_single(FindFeePaymentAsset::new())?;
    let fees_asset = result.expect("Fee asset must be declared");
    assert_eq!(fees_asset, cabbage_id);

    Ok(())
}
