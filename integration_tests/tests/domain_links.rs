#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for receive-path implicit account creation and domain SNS helpers.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        account::AccountAddress,
        metadata::Metadata,
        prelude::*,
        sns::{
            DOMAIN_NAME_SUFFIX_ID, NameControllerV1, NameSelectorV1, PaymentProofV1,
            RegisterNameRequestV1,
        },
    },
    sns::SnsNamespacePath,
};
use iroha_primitives::json::Json;
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use tokio::runtime::Runtime;

fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_pipeline_time(std::time::Duration::from_secs(2)),
        context,
    )
    .unwrap()
}

fn account_controller(account: &AccountId) -> Result<NameControllerV1> {
    let address = AccountAddress::from_account_id(account)?;
    Ok(NameControllerV1::account(&address))
}

fn stub_payment_proof(payer: &AccountId) -> PaymentProofV1 {
    PaymentProofV1 {
        asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
        gross_amount: 120,
        net_amount: 120,
        settlement_tx: Json::from("mock-settlement"),
        payer: payer.clone(),
        signature: Json::from("mock-signature"),
    }
}

fn build_domain_register_request(
    domain: &DomainId,
    owner: &AccountId,
) -> Result<RegisterNameRequestV1> {
    Ok(RegisterNameRequestV1 {
        selector: NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.name().as_ref())?,
        owner: owner.clone(),
        controllers: vec![account_controller(owner)?],
        term_years: 1,
        pricing_class_hint: Some(0),
        payment: stub_payment_proof(owner),
        governance: None,
        metadata: Metadata::default(),
    })
}

fn ensure_registered_domain(client: &Client, domain: &DomainId) -> Result<()> {
    let domain_exists = client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .any(|existing| existing.id() == domain);
    if domain_exists {
        return Ok(());
    }

    // Runtime domain registration now requires a matching active SNS domain
    // lease owned by the same authority.
    if client
        .sns()
        .get_name(SnsNamespacePath::Domain, domain.name().as_ref())
        .is_err()
    {
        client
            .sns()
            .register(&build_domain_register_request(domain, &client.account)?)?;
    }

    client.submit_blocking(Register::domain(Domain::new(domain.clone())))?;
    Ok(())
}

#[test]
fn build_domain_register_request_uses_domain_label_and_owner_controller() -> Result<()> {
    let domain: DomainId = "helperdomain".parse()?;
    let (owner, _) = gen_account_in("helper_owner");
    let request = build_domain_register_request(&domain, &owner)?;

    assert_eq!(request.selector.suffix_id, DOMAIN_NAME_SUFFIX_ID);
    assert_eq!(request.selector.normalized_label(), domain.name().as_ref());
    assert_eq!(request.owner, owner);
    assert_eq!(request.controllers, vec![account_controller(&owner)?]);
    assert_eq!(request.payment.payer, owner);
    assert_eq!(request.pricing_class_hint, Some(0));
    assert_eq!(request.term_years, 1);

    Ok(())
}

#[test]
fn receive_paths_materialize_unregistered_accounts_for_assets_and_nfts() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        receive_paths_materialize_unregistered_accounts_for_assets_and_nfts
    )) else {
        return Ok(());
    };
    let client = network.client();

    let domain: DomainId = "receive-without-preregister".parse()?;
    ensure_registered_domain(&client, &domain)?;
    let source_account = client.account.clone();

    let destination_asset = gen_account_in(&domain).0;
    let destination_nft = gen_account_in(&domain).0;

    let asset_definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(domain.clone(), "coin".parse()?);
    client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    ))?;
    let source_asset_id = AssetId::new(asset_definition_id.clone(), source_account.clone());
    client.submit_blocking(Mint::asset_numeric(10u32, source_asset_id.clone()))?;
    client.submit_blocking(Transfer::asset_numeric(
        source_asset_id,
        4u32,
        destination_asset.clone(),
    ))?;

    let destination_asset_id = AssetId::new(asset_definition_id, destination_asset.clone());
    let destination_asset_state = client.query_single(FindAssetById::new(destination_asset_id))?;
    assert_eq!(*destination_asset_state.value(), Numeric::from(4u32));

    let nft_id: NftId = format!("nft_receive${domain}").parse()?;
    client.submit_blocking(Register::nft(Nft::new(nft_id.clone(), Metadata::default())))?;
    client.submit_blocking(Transfer::nft(
        source_account,
        nft_id.clone(),
        destination_nft.clone(),
    ))?;

    let nft = client
        .query(FindNfts::new())
        .execute_all()?
        .into_iter()
        .find(|nft| nft.id() == &nft_id)
        .expect("nft should exist after transfer");
    assert_eq!(nft.owned_by(), &destination_nft);

    let accounts = client.query(FindAccounts::new()).execute_all()?;
    assert!(
        accounts
            .iter()
            .any(|account| account.id() == &destination_asset)
    );

    Ok(())
}
