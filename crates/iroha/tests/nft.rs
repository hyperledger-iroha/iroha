use iroha::data_model::prelude::*;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID};

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
