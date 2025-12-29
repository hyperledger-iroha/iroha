//! Integration tests for governance parameter updates.

use eyre::Result;
use iroha::data_model::{
    parameter::{BlockParameter, Parameter, Parameters, SmartContractParameter},
    prelude::*,
};
use iroha_test_network::*;
use nonzero_ext::nonzero;

use integration_tests::sandbox;

#[test]
fn can_change_parameter_value() -> Result<()> {
    let builder = NetworkBuilder::new().with_genesis_instruction(SetParameter::new(
        Parameter::Block(BlockParameter::MaxTransactions(nonzero!(16u64))),
    ));
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(can_change_parameter_value))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let old_params: Parameters = test_client.query_single(FindParameters::new())?;
    assert_eq!(old_params.block().max_transactions(), nonzero!(16u64));

    let new_value = nonzero!(32u64);
    test_client.submit_blocking(SetParameter::new(Parameter::Block(
        BlockParameter::MaxTransactions(new_value),
    )))?;

    let params = test_client.query_single(FindParameters::new())?;
    assert_eq!(params.block().max_transactions(), new_value);

    Ok(())
}

#[test]
fn can_change_executor_execution_depth_on_fresh_network() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(can_change_executor_execution_depth_on_fresh_network),
    )?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let initial_params: Parameters = test_client.query_single(FindParameters::new())?;
    let current_depth = initial_params.executor().execution_depth();
    let new_depth = if current_depth == u8::MAX {
        current_depth - 1
    } else {
        current_depth + 1
    };

    test_client.submit_blocking(SetParameter::new(Parameter::Executor(
        SmartContractParameter::ExecutionDepth(new_depth),
    )))?;

    let params: Parameters = test_client.query_single(FindParameters::new())?;
    assert_eq!(params.executor().execution_depth(), new_depth);

    Ok(())
}
