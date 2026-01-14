//! Integration tests covering Torii query APIs.

use std::time::Duration;

use integration_tests::sandbox;
use iroha::{
    client::{Client, QueryError},
    data_model::{
        prelude::*,
        query::{error::QueryExecutionFail, parameters::MAX_FETCH_SIZE},
    },
};
use iroha_test_network::*;

const QUERY_TX_STATUS_TIMEOUT: Duration = Duration::from_secs(30);

fn query_network_builder() -> NetworkBuilder {
    NetworkBuilder::new()
}

fn query_client(network: &Network) -> Client {
    let mut client = network.client();
    client.transaction_status_timeout = QUERY_TX_STATUS_TIMEOUT;
    client
}

/// Account query scenarios.
mod account;
/// Asset query regression coverage.
mod asset;
/// Metadata query workflow tests.
mod metadata;
/// Proof record query validation.
mod proof;
/// Query error surface verification.
mod query_errors;
/// Role-related queries.
mod role;
/// Smart contract query scenarios.
mod smart_contract;

#[test]
fn too_big_fetch_size_is_not_allowed() {
    if let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        query_network_builder(),
        stringify!(too_big_fetch_size_is_not_allowed),
    )
    .unwrap()
    {
        let client = query_client(&network);

        let result: eyre::Result<()> = {
            let err = client
                .query(FindAssets::new())
                .with_fetch_size(FetchSize::new(Some(MAX_FETCH_SIZE.checked_add(1).unwrap())))
                .execute()
                .expect_err("Should fail");

            assert!(matches!(
                err,
                QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::FetchSizeTooBig
                ))
            ));
            Ok(())
        };

        let _ =
            sandbox::handle_result(result, stringify!(too_big_fetch_size_is_not_allowed)).unwrap();
    }
}

#[test]
fn find_blocks_reversed() -> eyre::Result<()> {
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        query_network_builder(),
        stringify!(find_blocks_reversed),
    )?
    else {
        return Ok(());
    };
    let client = query_client(&network);

    // Force the chain to advance by submitting two transactions; idle peers may not emit empty blocks.
    let register_first_domain = Register::domain(Domain::new("domain1".parse()?));
    client.submit_blocking(register_first_domain)?;
    rt.block_on(async { network.ensure_blocks(2).await })?;

    let register_second_domain = Register::domain(Domain::new("domain2".parse()?));
    client.submit_blocking(register_second_domain)?;
    rt.block_on(async { network.ensure_blocks(3).await })?;

    let blocks = client.query(FindBlocks).execute_all()?;
    assert!(
        blocks.len() >= 3,
        "expected at least genesis plus two committed blocks"
    );
    assert_eq!(
        blocks[blocks.len() - 1].header().prev_block_hash(),
        None,
        "genesis block should be last"
    );
    for pair in blocks.windows(2) {
        assert_eq!(
            pair[0].header().prev_block_hash(),
            Some(pair[1].header().hash())
        );
    }

    Ok(())
}

#[test]
fn find_transactions_reversed() -> eyre::Result<()> {
    let result: eyre::Result<()> = {
        let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
            query_network_builder(),
            stringify!(find_transactions_reversed),
        )?
        else {
            return Ok(());
        };
        let client = query_client(&network);

        let register_domain = Register::domain(Domain::new("domain1".parse()?));
        client.submit_blocking(register_domain.clone())?;

        let txs = client.query(FindTransactions).execute_all()?;

        // check that latest transaction is register domain
        let TransactionEntrypoint::External(entrypoint) = txs[0].entrypoint() else {
            eyre::bail!("entrypoint should be external transaction");
        };
        let Executable::Instructions(instructions) = entrypoint.instructions() else {
            eyre::bail!("entrypoint should be builtin instructions");
        };
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0], register_domain.into());

        Ok(())
    };

    let _ = sandbox::handle_result(result, stringify!(find_transactions_reversed))?;
    Ok(())
}
