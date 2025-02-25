use iroha::{
    client::QueryError,
    data_model::{
        prelude::*,
        query::{error::QueryExecutionFail, parameters::MAX_FETCH_SIZE},
    },
};
use iroha_test_network::*;

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
fn find_blocks_reversed() -> eyre::Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    // Waiting for empty block to be committed
    std::thread::sleep(network.pipeline_time());

    client.submit_blocking(Register::domain(Domain::new("domain1".parse()?)))?;

    // Waiting for empty block to be committed
    std::thread::sleep(network.pipeline_time());

    let blocks = client.query(FindBlocks).execute_all()?;
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[blocks.len() - 1].header().prev_block_hash(), None);
    for i in 0..blocks.len() - 1 {
        assert_eq!(
            blocks[i].header().prev_block_hash(),
            Some(blocks[i + 1].header().hash())
        );
    }
    assert!(blocks[0].is_empty());

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
    let Executable::Instructions(instructions) = txs[0].as_ref().instructions() else {
        panic!("Expected instructions");
    };
    assert_eq!(instructions.len(), 1);
    assert_eq!(
        instructions[0],
        InstructionBox::Register(register_domain.into())
    );

    Ok(())
}
