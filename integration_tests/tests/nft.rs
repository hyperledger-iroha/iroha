#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for non-fungible token lifecycle operations.

use std::time::{Duration, Instant};

use eyre::Result;
use integration_tests::sandbox;
use iroha::{client::Client, data_model::prelude::*};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID, gen_account_in};
use tokio::runtime::Runtime;

fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_pipeline_time(std::time::Duration::from_secs(2)),
        context,
    )
    .unwrap()
}

fn wait_for_nft(
    client: &Client,
    nft_id: &NftId,
    predicate: impl Fn(&Nft) -> bool,
    context: &str,
) -> Nft {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "nft was not queried".to_owned();

    while Instant::now() < deadline {
        match client.query(FindNfts::new()).execute_all() {
            Ok(nfts) => {
                let matching = nfts.into_iter().find(|nft| nft.id() == nft_id);
                if let Some(nft) = matching {
                    last_observed =
                        format!("owner={:?}, content={:?}", nft.owned_by(), nft.content());
                    if predicate(&nft) {
                        return nft;
                    }
                } else {
                    last_observed = "nft missing".to_owned();
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    panic!("timed out waiting for nft after {context}; id={nft_id}; last_observed={last_observed}");
}

fn wait_for_nft_owner(client: &Client, nft_id: &NftId, expected_owner: &AccountId, context: &str) {
    wait_for_nft(
        client,
        nft_id,
        |nft| nft.owned_by() == expected_owner,
        context,
    );
}

fn wait_for_nft_absent(client: &Client, nft_id: &NftId, context: &str) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "nft was not queried".to_owned();

    while Instant::now() < deadline {
        match client.query(FindNfts::new()).execute_all() {
            Ok(nfts) => {
                if nfts.iter().all(|nft| nft.id() != nft_id) {
                    return;
                }
                last_observed = "nft still present".to_owned();
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    panic!(
        "timed out waiting for nft removal after {context}; id={nft_id}; last_observed={last_observed}"
    );
}

#[test]
fn nft_lifecycle_scenarios() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(nft_lifecycle_scenarios)) else {
        return Ok(());
    };
    let client = network.client();
    let wonderland = DomainId::try_new("wonderland", "universal")?;

    // transfer_nft
    {
        let alice_id = ALICE_ID.clone();
        let bob_id = BOB_ID.clone();
        let nft_id = NftId::new(wonderland.clone(), "nft_transfer".parse()?);

        client.submit_blocking(Register::nft(Nft::new(nft_id.clone(), Metadata::default())))?;

        wait_for_nft_owner(&client, &nft_id, &alice_id, "nft registration");

        client.submit_blocking(Transfer::nft(alice_id, nft_id.clone(), bob_id.clone()))?;

        wait_for_nft_owner(&client, &nft_id, &bob_id, "nft transfer");
    }

    // client_register_nft_second_time_should_fail
    {
        let nft_id = NftId::new(wonderland.clone(), "nft_register_twice".parse()?);
        let mut metadata = Metadata::default();
        metadata.insert("key".parse()?, 1u32);
        let register_nft = Register::nft(Nft::new(nft_id.clone(), metadata.clone()));

        client.submit_blocking(register_nft.clone())?;

        let nft = wait_for_nft(
            &client,
            &nft_id,
            |nft| nft.content() == &metadata,
            "nft duplicate registration seed",
        );
        assert_eq!(*nft.content(), metadata);

        assert!(client.submit_blocking(register_nft).is_err());
    }

    // unregister_nft_should_remove_nft_from_account
    {
        let nft_id = NftId::new(wonderland.clone(), "nft_unregister".parse()?);
        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        let unregister_nft = Unregister::nft(nft_id.clone());

        client.submit_blocking(register_nft)?;
        wait_for_nft(
            &client,
            &nft_id,
            |_| true,
            "nft unregister registration seed",
        );

        client.submit_blocking(unregister_nft)?;
        wait_for_nft_absent(&client, &nft_id, "nft unregister");
    }

    // nft_owner_cant_modify_nft
    {
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let nft_id = NftId::new(wonderland.clone(), "nft_owner_modify".parse()?);

        let create_account = Register::account(Account::new(account_id.clone()));
        client.submit_blocking(create_account)?;

        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        client.submit_blocking(register_nft)?;

        let transfer_nft = Transfer::nft(ALICE_ID.clone(), nft_id.clone(), account_id.clone());
        client.submit_blocking(transfer_nft)?;

        let modify_nft = SetKeyValue::nft(nft_id.clone(), "foo".parse()?, "value");
        client
            .submit_blocking(modify_nft.clone())
            .expect("Owner of `nft.domain` can modify NFT");

        let modify_nft_tx = TransactionBuilder::new(network.chain_id(), account_id.clone())
            .with_instructions([modify_nft])
            .sign(account_keypair.private_key());
        let _ = client
            .submit_transaction_blocking(&modify_nft_tx)
            .expect_err("Owner of NFT can't modify NFT");
    }

    // nft_owner_can_transfer_nft
    {
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let nft_id = NftId::new(wonderland.clone(), "nft_owner_transfer".parse()?);

        let create_account = Register::account(Account::new(account_id.clone()));
        client.submit_blocking(create_account)?;

        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        client.submit_blocking(register_nft)?;

        let transfer_nft1 = Transfer::nft(ALICE_ID.clone(), nft_id.clone(), account_id.clone());
        client.submit_blocking(transfer_nft1)?;

        let transfer_nft2 = Transfer::nft(account_id.clone(), nft_id.clone(), ALICE_ID.clone());
        let transfer_nft2_tx = TransactionBuilder::new(network.chain_id(), account_id.clone())
            .with_instructions([transfer_nft2])
            .sign(account_keypair.private_key());
        client
            .submit_transaction_blocking(&transfer_nft2_tx)
            .expect("Owner of NFT can transfer NFT");
    }

    Ok(())
}
