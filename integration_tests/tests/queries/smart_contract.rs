#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smart contract query behaviour checks.
use std::num::NonZeroU64;
use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{
    client::QueryError,
    data_model::{prelude::*, query::error::QueryExecutionFail},
};
use iroha_core::smartcontracts::ivm::gas_limit_for_meta;
use iroha_test_network::*;
use iroha_test_samples::load_sample_ivm;
fn fee_payment_with_gas_limit(bytecode: &IvmBytecode) -> Result<FeePaymentIntent> {
    let parsed =
        ivm::ProgramMetadata::parse(bytecode.as_ref()).wrap_err("parse IVM program metadata")?;
    let gas_limit = gas_limit_for_meta(&parsed.metadata)
        .map_err(|error| eyre::eyre!("invalid IVM cycle limit: {error:?}"))?;
    Ok(FeePaymentIntent::authority(
        Vec::new(),
        NonZeroU64::new(gas_limit),
    ))
}
#[test]
fn smart_contract_query_scenarios() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_config_layer(|layer| {
            layer.write(["pipeline", "query_default_cursor_mode"], "stored");
        }),
        stringify!(smart_contract_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let torii = client.torii_url.clone();
    let env_dir = network.env_dir().to_path_buf();
    // live_query_is_dropped_after_smart_contract_end
    {
        let bytecode = load_sample_ivm("query_assets_and_save_cursor");
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let transaction = client.build_transaction(bytecode, fee_payment, Metadata::default());
        client.submit_transaction_blocking(&transaction)?;
        let cursor_key: Name = "cursor".parse().unwrap();
        let asset_cursor = client
            .query(FindAccounts)
            .execute_all()? // lightweight DSL: filter/select on client
            .into_iter()
            .find(|account| account.id() == &client.account)
            .and_then(|account| account.metadata().get(&cursor_key).cloned())
            .expect("account metadata must contain cursor")
            .try_into_any_norito()?;
        let err = client
            .raw_continue_iterable_query(asset_cursor)
            .expect_err("Request with cursor from smart contract should fail");
        // Continuation must fail; the exact error depends on cursor mode/config.
        let allowed = matches!(
            &err,
            QueryError::Validation(ValidationFail::NotPermitted(_))
                | QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::Expired
                        | QueryExecutionFail::NotFound
                        | QueryExecutionFail::CursorMismatch
                        | QueryExecutionFail::CursorDone
                ))
        ) || err
            .to_string()
            .contains("cursor continuation requires stored cursor mode");
        assert!(allowed, "unexpected query error: {err:?}");
    }
    // smart_contract_can_filter_queries
    {
        let bytecode = load_sample_ivm("smart_contract_can_filter_queries");
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let transaction = client.build_transaction(bytecode, fee_payment, Metadata::default());
        client
            .submit_transaction_blocking(&transaction)
            .wrap_err_with(|| {
                format!(
                    "submit smart_contract_can_filter_queries failed; torii={torii}, env_dir={}",
                    env_dir.display()
                )
            })?;
    }
    Ok(())
}
