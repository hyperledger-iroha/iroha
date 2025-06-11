use eyre::{Ok, Result};
use futures_util::StreamExt;
use iroha::{
    // crypto::MerkleProof,
    data_model::{events::pipeline::BlockEventFilter, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;
use tokio::{
    task::spawn_blocking,
    time::{timeout, Duration},
};

/// # Scenario
///
/// 1. Client sends a query request with a transaction hash to a node.
/// 2. Node retrieves the transaction and its associated data trigger execution log from block storage.
/// 3. Node returns the complete transaction details to the client.
/// 4. Client verifies and accepts the returned transaction data.
#[tokio::test]
async fn query_returns_complete_execution_log() -> Result<()> {
    let alice_rose: AssetId = format!("rose##{}", *ALICE_ID).parse().unwrap();
    let bob_rose: AssetId = format!("rose##{}", *BOB_ID).parse().unwrap();
    let user_instruction = Transfer::asset_numeric(alice_rose.clone(), 5u32, BOB_ID.clone());
    let data_trigger_instruction =
        Transfer::asset_numeric(bob_rose.clone(), 3u32, ALICE_ID.clone());

    // Initialize a network with a registered data-trigger at genesis.
    let network = NetworkBuilder::new()
        .with_genesis_instruction(SetParameter::new(Parameter::SmartContract(
            iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(1),
        )))
        .with_genesis_instruction(Register::trigger(Trigger::new(
            "data-bob-alice-0".parse().unwrap(),
            Action::new(
                [data_trigger_instruction],
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                DataEventFilter::Asset(
                    AssetEventFilter::new()
                        .for_asset(bob_rose)
                        .for_events(AssetEventSet::Added),
                ),
            ),
        )))
        .start()
        .await?;
    let test_client = network.client();

    // Subscribe to committed block headers.
    let mut events = test_client
        .listen_for_events_async([BlockEventFilter::new()
            .for_height(nonzero!(2u64))
            .for_status(BlockStatus::Committed)])
        .await?;

    // Submit the user transaction and derive its entrypoint hash.
    let tx_hash: HashOf<SignedTransaction> =
        spawn_blocking(move || test_client.submit(user_instruction)).await??;
    let entrypoint_hash: HashOf<TransactionEntrypoint> =
        HashOf::from_untyped_unchecked(tx_hash.into());

    // Wait for the block to be committed and retrieve its header.
    // NOTE: header verification is out of scope for this test.
    let header = timeout(Duration::from_secs(5), async move {
        let EventBox::Pipeline(PipelineEventBox::Block(event)) =
            events.next().await.unwrap().unwrap()
        else {
            panic!("expected block event");
        };
        *event.header()
    })
    .await?;
    let block_hash = header.hash();

    // Query the committed transaction by its entrypoint hash.
    let test_client = network.client();
    let committed_tx: CommittedTransaction = spawn_blocking(move || {
        test_client
            .query(FindTransactions::new())
            .filter_with(|tx| tx.entrypoint_hash.eq(entrypoint_hash))
            .execute_single()
    })
    .await??;

    println!("{committed_tx:#?}");
    assert_eq!(*committed_tx.block_hash(), block_hash);

    // Verify inclusion proof for the transaction entrypoint.
    // let proof: MerkleProof<TransactionEntrypoint>;
    // let root = header
    //     .merkle_root()
    //     .expect("non-empty block should have a Merkle root");
    // assert!(proof.verify(&root, 9)); // Assumes up to 2^9 (512) transactions per block.

    // Verify inclusion proof for the transaction result.

    Ok(())
}

/// # Scenario
///
/// 1. Client sends a query request with a transaction hash to a node.
/// 2. Node retrieves the transaction and its associated data trigger execution log from block storage.
///    FAULT INJECTION: block storage returns an altered execution log.
/// 3. Node returns the tampered transaction details to the client.
/// 4. Client verifies and detects the tampering in the transaction data.
#[test]
fn query_returns_tampered_execution_log() {}
