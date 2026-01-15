//! Smart contract query behaviour checks.

use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{client::QueryError, data_model::prelude::*};
use iroha_core::smartcontracts::ivm::gas_limit_for_meta;
use iroha_test_network::*;
use iroha_test_samples::load_sample_ivm;

fn metadata_with_gas_limit(bytecode: &IvmBytecode) -> Result<Metadata> {
    let parsed =
        ivm::ProgramMetadata::parse(bytecode.as_ref()).wrap_err("parse IVM program metadata")?;
    let mut metadata = Metadata::default();
    metadata.insert(
        "gas_limit".parse().expect("static gas_limit key"),
        gas_limit_for_meta(&parsed.metadata),
    );
    Ok(metadata)
}

#[test]
fn live_query_is_dropped_after_smart_contract_end() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(live_query_is_dropped_after_smart_contract_end),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    let result: Result<()> = (|| {
        let bytecode = load_sample_ivm("query_assets_and_save_cursor");
        let metadata = metadata_with_gas_limit(&bytecode)?;
        let transaction = client.build_transaction(bytecode, metadata);
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

        // here we are breaking the abstraction preventing us from using a cursor we pulled from the metadata
        let err = client
            .raw_continue_iterable_query(asset_cursor)
            .expect_err("Request with cursor from smart contract should fail");

        assert!(
            matches!(err, QueryError::Validation(ValidationFail::NotPermitted(_))),
            "unexpected query error: {err:?}"
        );
        Ok(())
    })();

    if sandbox::handle_result(
        result,
        stringify!(live_query_is_dropped_after_smart_contract_end),
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
}

#[test]
fn smart_contract_can_filter_queries() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(smart_contract_can_filter_queries),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let torii = client.torii_url.clone();
    let env_dir = network.env_dir().to_path_buf();

    let result: Result<()> = (|| {
        let bytecode = load_sample_ivm("smart_contract_can_filter_queries");
        let metadata = metadata_with_gas_limit(&bytecode)?;
        let transaction = client.build_transaction(bytecode, metadata);
        client
            .submit_transaction_blocking(&transaction)
            .wrap_err_with(|| {
                format!(
                    "submit smart_contract_can_filter_queries failed; torii={torii}, env_dir={}",
                    env_dir.display()
                )
            })?;
        Ok(())
    })();
    if sandbox::handle_result(result, stringify!(smart_contract_can_filter_queries))?.is_none() {
        return Ok(());
    }

    Ok(())
}
