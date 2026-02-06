#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Trigger execution log coverage.
#![cfg(feature = "fault_injection")]

use eyre::{Ok, Result};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::{
    crypto::MerkleProof,
    data_model::{events::pipeline::BlockEventFilter, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;
use tokio::{task::spawn_blocking, time::timeout};

/// # Scenario
///
/// 1. Client queries a transaction by its entrypoint hash.
/// 2. Node returns the transaction with both entrypoint and result, each accompanied by its Merkle proof.
/// 3a. If proofs are valid, the client accepts the data.
/// 3b. If proofs are invalid or tampered with, the client rejects the data.
#[tokio::test]
async fn client_verifies_transaction_entrypoint_and_result_proofs() -> Result<()> {
    let rose_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset definition");
    let alice_rose = AssetId::new(rose_def.clone(), ALICE_ID.clone());
    let bob_rose = AssetId::new(rose_def, BOB_ID.clone());
    let user_instruction = Transfer::asset_numeric(alice_rose.clone(), 5u32, BOB_ID.clone());
    let data_trigger_instruction =
        Transfer::asset_numeric(bob_rose.clone(), 3u32, ALICE_ID.clone());

    // Initialize a network with a registered data-trigger at genesis.
    let builder = NetworkBuilder::new()
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
        )));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(client_verifies_transaction_entrypoint_and_result_proofs),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();

    // Subscribe to committed block headers.
    let mut events = timeout(
        network.sync_timeout(),
        test_client.listen_for_events_async([BlockEventFilter::new()
            .for_height(nonzero!(2u64))
            .for_status(BlockStatus::Committed)]),
    )
    .await
    .map_err(|_| {
        eyre::eyre!(
            "client_verifies_transaction_entrypoint_and_result_proofs: timed out opening block event stream"
        )
    })??;

    // Submit the user transaction and derive its entrypoint hash.
    let tx_hash: HashOf<SignedTransaction> =
        spawn_blocking(move || test_client.submit(user_instruction))
            .await
            .unwrap()?;
    let entrypoint_hash: HashOf<TransactionEntrypoint> =
        HashOf::from_untyped_unchecked(tx_hash.into());

    // Wait for the block to be committed and retrieve its header.
    // NOTE: header verification is out of scope for this test.
    let event_timeout = network.sync_timeout();
    let header = timeout(event_timeout, async {
        let EventBox::Pipeline(PipelineEventBox::Block(event)) =
            events.next().await.unwrap().unwrap()
        else {
            panic!("expected block event");
        };
        *event.header()
    })
    .await?;
    events.close().await;
    let block_hash = header.hash();

    // Query the committed transaction by its entrypoint hash.
    let test_client = network.client();
    let committed_tx: CommittedTransaction = spawn_blocking(move || {
        test_client
            .query(FindTransactions::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .find(|tx| tx.entrypoint_hash() == &entrypoint_hash)
            .unwrap()
    })
    .await
    .unwrap();

    println!("{committed_tx:#?}");
    assert_eq!(*committed_tx.block_hash(), block_hash);

    // Verify inclusion proof for the transaction entrypoint.
    let leaf = committed_tx.entrypoint().hash();
    let proof: MerkleProof<TransactionEntrypoint> = committed_tx.entrypoint_proof().clone();
    let root = header
        .merkle_root()
        .expect("non-empty block should have a Merkle root");
    assert!(proof.verify(&leaf, &root, 9)); // Assumes up to 2^9 (512) transactions per block.

    // Fault injection: proof should now fail for a tampered entrypoint.
    let mut tampered_tx = committed_tx.clone();
    let self_transfer = Transfer::asset_numeric(alice_rose, 100u32, ALICE_ID.clone());
    // Inject a zero-net-effect, self-neutralizing instruction.
    tampered_tx.inject_instructions([self_transfer]);
    let leaf = tampered_tx.entrypoint().hash();
    let bad_proof = tampered_tx.entrypoint_proof().clone();
    assert!(!bad_proof.verify(&leaf, &root, 9));

    // Verify inclusion proof for the transaction result.
    let leaf = committed_tx.result().hash();
    let proof: MerkleProof<TransactionResult> = committed_tx.result_proof().clone();
    let root = header
        .result_merkle_root()
        .expect("non-empty block should have a Merkle root");
    assert!(proof.verify(&leaf, &root, 9));

    // Fault injection: proof should fail for a tampered result.
    let mut tampered_tx = committed_tx.clone();
    tampered_tx.swap_result();
    let leaf = tampered_tx.result().hash();
    let bad_proof = tampered_tx.result_proof().clone();
    assert!(!bad_proof.verify(&leaf, &root, 9));

    Ok(())
}
