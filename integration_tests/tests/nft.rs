#![allow(missing_docs)]

use eyre::Result;
use iroha::data_model::prelude::*;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{gen_account_in, ALICE_ID, BOB_ID};

#[test]
fn transfer_nft() {
    let (network, _rt) = NetworkBuilder::new().start_blocking().unwrap();
    let client = network.client();

    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let nft_id: NftId = "nft$wonderland".parse().expect("Valid");

    client
        .submit_blocking(Register::nft(Nft::new(nft_id.clone(), Metadata::default())))
        .expect("Failed to submit transaction");

    let nft = client
        .query(FindNfts::new())
        .filter_with(|nft| nft.id.eq(nft_id.clone()))
        .execute_single()
        .expect("Failed to execute Iroha Query");
    assert_eq!(nft.owned_by(), &alice_id);

    client
        .submit_blocking(Transfer::nft(alice_id, nft_id.clone(), bob_id.clone()))
        .expect("Failed to submit transaction");

    let nft = client
        .query(FindNfts::new())
        .filter_with(|nft| nft.id.eq(nft_id))
        .execute_single()
        .expect("Failed to execute Iroha Query");
    assert_eq!(nft.owned_by(), &bob_id);
}

#[test]
fn client_register_nft_second_time_should_fail() -> eyre::Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let nft_id: NftId = "test_nft$wonderland".parse()?;
    let mut metadata = Metadata::default();
    metadata.insert("key".parse()?, 1u32);
    let register_nft = Register::nft(Nft::new(nft_id.clone(), metadata.clone()));

    client.submit_blocking(register_nft.clone())?;

    let nft = client
        .query(FindNfts::new())
        .execute_all()?
        .into_iter()
        .find(|nft| *nft.id() == nft_id)
        .unwrap();
    assert_eq!(*nft.content(), metadata);

    assert!(client.submit_blocking(register_nft).is_err());

    Ok(())
}

#[test]
fn unregister_nft_should_remove_nft_from_account() -> eyre::Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    // Given
    let nft_id: NftId = "test_nft$wonderland".parse()?;
    let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
    let unregister_nft = Unregister::nft(nft_id.clone());

    // Check for NFT to be registered
    client.submit_blocking(register_nft)?;
    assert!(client
        .query(FindNfts::new())
        .execute_all()?
        .iter()
        .any(|nft| *nft.id() == nft_id));

    // ... and check that it is removed after Unregister
    client.submit_blocking(unregister_nft)?;
    assert!(client
        .query(FindNfts::new())
        .execute_all()?
        .iter()
        .all(|nft| *nft.id() != nft_id));

    Ok(())
}

#[test]
fn nft_owner_cant_modify_nft() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let (account_id, account_keypair) = gen_account_in("wonderland");
    let nft_id: NftId = "nft$wonderland".parse()?;

    let create_account = Register::account(Account::new(account_id.clone()));
    client.submit_blocking(create_account)?;

    let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
    client.submit_blocking(register_nft)?;

    let transfer_nft = Transfer::nft(ALICE_ID.clone(), nft_id.clone(), account_id.clone());
    client.submit_blocking(transfer_nft)?;

    let modify_nft = SetKeyValue::nft(nft_id, "foo".parse()?, "value");
    client
        .submit_blocking(modify_nft.clone())
        .expect("Owner of `nft.domain` can modify NFT");

    let modify_nft_tx = TransactionBuilder::new(network.chain_id(), account_id.clone())
        .with_instructions([modify_nft])
        .sign(account_keypair.private_key());
    let _ = client
        .submit_transaction_blocking(&modify_nft_tx)
        .expect_err("Owner of NFT can't modify NFT");

    Ok(())
}

#[test]
fn nft_owner_can_transfer_nft() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    let (account_id, account_keypair) = gen_account_in("wonderland");
    let nft_id: NftId = "nft$wonderland".parse()?;

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

    Ok(())
}
