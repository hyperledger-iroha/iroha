use iroha::{
    client::QueryError,
    data_model::{
        prelude::*,
        query::{error::QueryExecutionFail, parameters::MAX_FETCH_SIZE},
    },
};
use iroha_test_network::*;
use nonzero_ext::nonzero;

mod account;
mod asset;
mod metadata;
mod query_errors;
mod role;
mod smart_contract;

#[test]
fn too_big_fetch_size_is_not_allowed() {
    let (network, _rt) = NetworkBuilder::new().start_blocking().unwrap();
    let client = network.client();

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
}

#[test]
fn find_blocks() -> eyre::Result<()> {
    let (network, rt) = NetworkBuilder::new()
        .with_config_layer(|cfg| {
            cfg.write(["logger", "level"], "DEBUG");
        })
        .start_blocking()?;
    let client = network.client();

    // Waiting for empty block to be committed
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 2).await })?;

    client.submit_blocking(Register::domain(Domain::new("domain1".parse()?)))?;

    // Waiting for empty block to be committed
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 4).await })?;

    let blocks = client
        .query(FindBlocks::new(Order::Descending))
        .execute_all()?;
    assert_eq!(blocks.len(), 4);
    assert!(blocks
        .windows(2)
        .all(|wnd| wnd[0].header() > wnd[1].header()
            && wnd[0].header().prev_block_hash().unwrap() == wnd[1].header().hash()));
    assert!(blocks[0].is_empty());

    let blocks_asc = client
        .query(FindBlocks::new(Order::Ascending))
        .execute_all()?;
    assert_eq!(blocks_asc, {
        let mut blocks = blocks.clone();
        blocks.reverse();
        blocks
    });

    // Covering the use case from #5454: deterministically fetching the genesis block
    let genesis = client
        .query(FindBlocks::new(Order::Ascending))
        .with_pagination(Pagination::new(Some(nonzero!(1u64)), 0))
        .execute_single()?;
    assert!(genesis.header().is_genesis());

    Ok(())
}

#[test]
fn find_transactions_reversed() -> eyre::Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let register_domain = Register::domain(Domain::new("domain1".parse()?));
    client.submit_blocking(register_domain.clone())?;

    let txs = client.query(FindTransactions).execute_all()?;

    // check that latest transaction is register domain
    let TransactionEntrypoint::External(entrypoint) = txs[0].entrypoint() else {
        eyre::bail!("entrypoint should be external transaction");
    };
    let Executable::Instructions(instructions) = entrypoint.instructions();
    assert_eq!(instructions.len(), 1);
    assert_eq!(
        instructions[0],
        InstructionBox::Register(register_domain.into())
    );

    Ok(())
}
