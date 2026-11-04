//! Integration tests for non-fungible token lifecycle operations.
use eyre::{Result, ensure};
use integration_tests::sandbox;
use iroha::collections::{Collection, ListQuery, field};
use iroha::{blocking::Client, data_model::prelude::*};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID, gen_account_in};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_block_cadence(std::time::Duration::from_secs(2)),
        context,
    )
    .unwrap()
}
fn wait_for_nft(
    client: &Client,
    nft_id: &NftId,
    predicate: impl Fn(&Nft) -> bool,
    context: &str,
) -> Result<Nft> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "nft was not queried".to_owned();
    while Instant::now() < deadline {
        match client
            .client()
            .query_single(FindNftById::new(nft_id.clone()))
        {
            Ok(nft) => {
                ensure!(nft.id() == nft_id, "singular NFT query changed identity");
                last_observed = format!("owner={:?}, content={:?}", nft.owned_by(), nft.content());
                if predicate(&nft) {
                    return Ok(nft);
                }
            }
            Err(error) => return Err(error.into()),
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    panic!("timed out waiting for nft after {context}; id={nft_id}; last_observed={last_observed}");
}
fn wait_for_nft_owner(
    client: &Client,
    nft_id: &NftId,
    expected_owner: &AccountId,
    context: &str,
) -> Result<()> {
    wait_for_nft(
        client,
        nft_id,
        |nft| nft.owned_by() == expected_owner,
        context,
    )?;
    Ok(())
}
fn wait_for_nft_absent(client: &Client, nft_id: &NftId, context: &str) -> Result<()> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "nft was not queried".to_owned();
    let literal = nft_id.to_string();
    let query = ListQuery::new()
        .filter(field("id").eq(literal.clone()))
        .limit(1);
    while Instant::now() < deadline {
        // A successful complete, account-signed page establishes absence. A
        // generic HTTP 404 (including a missing route) never counts as removal.
        let page = client.list_page(&Collection::Nfts, &query)?;
        ensure!(
            page.next_cursor.is_none() && page.items.len() <= 1,
            "exact NFT absence query returned an incomplete or non-unique page"
        );
        if page.items.is_empty() {
            return Ok(());
        }
        ensure!(
            page.items[0]
                .get("id")
                .and_then(norito::json::Value::as_str)
                == Some(literal.as_str()),
            "exact NFT absence query changed identity"
        );
        "nft still present".clone_into(&mut last_observed);
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
        client.submit(
            Register::nft(Nft::new(nft_id.clone(), Metadata::default())),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        wait_for_nft_owner(&client, &nft_id, &alice_id, "nft registration")?;
        client.submit(
            Transfer::nft(alice_id, nft_id.clone(), bob_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        wait_for_nft_owner(&client, &nft_id, &bob_id, "nft transfer")?;
    }
    // client_register_nft_second_time_should_fail
    {
        let nft_id = NftId::new(wonderland.clone(), "nft_register_twice".parse()?);
        let mut metadata = Metadata::default();
        metadata.insert("key".parse()?, 1u32);
        let register_nft = Register::nft(Nft::new(nft_id.clone(), metadata.clone()));
        client.submit(
            register_nft.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let nft = wait_for_nft(
            &client,
            &nft_id,
            |nft| nft.content() == &metadata,
            "nft duplicate registration seed",
        )?;
        assert_eq!(*nft.content(), metadata);
        assert!(
            client
                .submit(
                    register_nft,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None)
                )
                .is_err()
        );
    }
    // unregister_nft_should_remove_nft_from_account
    {
        let nft_id = NftId::new(wonderland.clone(), "nft_unregister".parse()?);
        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        let unregister_nft = Unregister::nft(nft_id.clone());
        client.submit(
            register_nft,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        wait_for_nft(
            &client,
            &nft_id,
            |_| true,
            "nft unregister registration seed",
        )?;
        client.submit(
            unregister_nft,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        wait_for_nft_absent(&client, &nft_id, "nft unregister")?;
    }
    // nft_owner_cant_modify_nft
    {
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let nft_id = NftId::new(wonderland.clone(), "nft_owner_modify".parse()?);
        let create_account = Register::account(Account::new(account_id.clone()));
        client.submit(
            create_account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let fee_asset = AssetId::new(
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
            ALICE_ID.clone(),
        );
        client.submit(
            Transfer::asset_quantity(fee_asset, 100_u32, account_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let owner_client =
            network.peers()[0].client_for(&account_id, account_keypair.private_key().clone());
        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        client.submit(
            register_nft,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let transfer_nft = Transfer::nft(ALICE_ID.clone(), nft_id.clone(), account_id.clone());
        client.submit(
            transfer_nft,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let modify_nft = SetKeyValue::nft(nft_id.clone(), "foo".parse()?, "value");
        client
            .submit(
                modify_nft.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect("Owner of `nft.domain` can modify NFT");
        let error = owner_client
            .submit(
                modify_nft,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err("Owner of NFT can't modify NFT");
        assert!(matches!(error.downcast_ref::<iroha_data_model::transaction::error::TransactionRejectionReason>(),
            Some(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_)))),
            "NFT modification must fail authorization after fee admission: {error:?}");
    }
    // nft_owner_can_transfer_nft
    {
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let nft_id = NftId::new(wonderland.clone(), "nft_owner_transfer".parse()?);
        let create_account = Register::account(Account::new(account_id.clone()));
        client.submit(
            create_account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let fee_asset = AssetId::new(
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
            ALICE_ID.clone(),
        );
        client.submit(
            Transfer::asset_quantity(fee_asset, 100_u32, account_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let owner_client =
            network.peers()[0].client_for(&account_id, account_keypair.private_key().clone());
        let register_nft = Register::nft(Nft::new(nft_id.clone(), Metadata::default()));
        client.submit(
            register_nft,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let transfer_nft1 = Transfer::nft(ALICE_ID.clone(), nft_id.clone(), account_id.clone());
        client.submit(
            transfer_nft1,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let transfer_nft2 = Transfer::nft(account_id.clone(), nft_id.clone(), ALICE_ID.clone());
        owner_client
            .submit(
                transfer_nft2,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect("Owner of NFT can transfer NFT");
        wait_for_nft_owner(&client, &nft_id, &ALICE_ID, "owner transfers NFT back")?;
    }
    Ok(())
}
