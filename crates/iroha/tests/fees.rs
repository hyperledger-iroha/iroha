#![allow(unused_imports)]
#![allow(unused_variables)]

use eyre::Result;
use iroha::{
    client, crypto::KeyPair, data_model::{
        asset::{AssetId, AssetType, AssetValue},
        isi::error::{InstructionEvaluationError, InstructionExecutionError, TypeError},
        prelude::*,
        transaction::error::TransactionRejectionReason,
    }
};
use iroha_data_model::parameter::{CustomParameter, CustomParameterId, Parameter};
use iroha_executor_data_model::{permission::asset::CanTransferAsset, parameter::Parameter as ExecutorParameter};
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, ALICE_ID, BOB_ID, load_sample_wasm};
use fees_executor_data_model::parameters::*;

#[test]
fn fees_example_test() -> Result<()> {
    let upgrade_executor = Upgrade::new(Executor::new(load_sample_wasm("fees_executor")));

    let (network, _rt) = NetworkBuilder::new()
    .with_genesis_instruction(upgrade_executor)
    .start_blocking()?;

    let test_client = network.client();

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    test_client.submit(SetParameter::new(Parameter::Custom(CustomParameter::new("123".parse()?, "123"))))?;
     
    println!("{}", fees_options.receiver);

    Ok(())
}
