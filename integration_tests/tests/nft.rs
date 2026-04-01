#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for non-fungible token lifecycle operations.

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
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

#[test]
fn nft_lifecycle_scenarios() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(nft_lifecycle_scenarios)) else {
        return Ok(());
    };
    let client = network.client();

    // transfer_nft
    {
        let alice_id = ALICE_ID.clone();
        let bob_id = BOB_ID.clone();
        let nft_id: NftId = "nft_transfer$wonderland".parse()?;

        client.submit_blocking(Register::nft(Nft::new(nft_id.clone(), Metadata::default())))?;

        let nft = client
            .query(FindNfts::new())
            .execute_all()?
            .into_iter()
            .find(|nft| nft.id() == &nft_id)
            .expect("nft should exist");
        assert_eq!(nft.owned_by(), &alice_id);

        client.submit_blocking(Transfer::nft(alice_id, nft_id.clone(), bob_id.clone()))?;

        let nft = client
            .query(FindNfts::new())
            .execute_all()?
            .into_iter()
            .find(|nft| nft.id() == &nft_id)
            .expect("nft should exist");
        assert_eq!(nft.owned_by(), &bob_id);
    }

    // client_register_nft_second_time_should_fail
    {
        let nft_id: NftId = "nft_register_twice$wonderland".parse()?;
        let mut metadata = Metadata::default();
        metadata.insert("key".parse()?, 1u32);
        let register_nft = Register::nft(Nft::new(nft_id.clone(), metadata.clone()));

        client.submit_blocking(register_nft.clone())?;

        let nft = client
            .query(FindNfts::new())
            .execute_all()?
            .into_iter()
            .find(|nft| *nft.id() == nft_id)
            .expect("nft should exist");
        assert_eq!(*nft.content(), metadata);

        assert!(client.submit_blocking(register_nft).is_err());
    }

    // unregister_nft_should_remove_nft_from_account
    {
        let nft_id: NftId = "nft_unregister$wonderland".parse()?;
        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        let unregister_nft = Unregister::nft(nft_id.clone());

        client.submit_blocking(register_nft)?;
        assert!(
            client
                .query(FindNfts::new())
                .execute_all()?
                .iter()
                .any(|nft| *nft.id() == nft_id)
        );

        client.submit_blocking(unregister_nft)?;
        assert!(
            client
                .query(FindNfts::new())
                .execute_all()?
                .iter()
                .all(|nft| *nft.id() != nft_id)
        );
    }

    // nft_owner_cant_modify_nft
    {
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let nft_id: NftId = "nft_owner_modify$wonderland".parse()?;

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
        let nft_id: NftId = "nft_owner_transfer$wonderland".parse()?;

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
