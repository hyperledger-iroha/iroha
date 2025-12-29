//! Smart contract query behaviour checks.

use eyre::{Result, WrapErr};
use iroha::{client::QueryError, data_model::prelude::*};
use iroha_test_network::*;
use iroha_test_samples::load_sample_ivm;

use integration_tests::sandbox;

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
        let transaction = client.build_transaction(
            load_sample_ivm("query_assets_and_save_cursor"),
            Metadata::default(),
        );
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
        let transaction = client.build_transaction(
            load_sample_ivm("smart_contract_can_filter_queries"),
            Metadata::default(),
        );
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
