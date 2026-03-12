#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for account-domain links and receive-path implicit account creation.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{isi::domain_link::*, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR, gen_account_in};
use tokio::runtime::Runtime;

fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_pipeline_time(std::time::Duration::from_secs(2)),
        context,
    )
    .unwrap()
}

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        ..base_client.clone()
    }
}

#[test]
fn domain_links_roundtrip_without_account_registration() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        domain_links_roundtrip_without_account_registration
    )) else {
        return Ok(());
    };
    let client = network.client();

    let domain: DomainId = "domain_links_query".parse()?;
    client.submit_blocking(Register::domain(Domain::new(domain.clone())))?;

    let (probe_account, _) = gen_account_in("ghost");
    client.submit_blocking::<InstructionBox>(
        LinkAccountDomain {
            account: probe_account.clone(),
            domain: domain.clone(),
        }
        .into(),
    )?;

    let linked_domains: Vec<DomainId> =
        client.query_single(FindDomainsByAccountId::new(probe_account.clone()))?;
    assert_eq!(linked_domains, vec![domain.clone()]);

    let linked_accounts: Vec<AccountId> =
        client.query_single(FindAccountIdsByDomainId::new(domain.clone()))?;
    assert_eq!(linked_accounts, vec![probe_account.clone()]);

    client.submit_blocking::<InstructionBox>(
        UnlinkAccountDomain {
            account: probe_account.clone(),
            domain: domain.clone(),
        }
        .into(),
    )?;

    let linked_domains_after: Vec<DomainId> =
        client.query_single(FindDomainsByAccountId::new(probe_account.clone()))?;
    assert!(linked_domains_after.is_empty());

    let linked_accounts_after: Vec<AccountId> =
        client.query_single(FindAccountIdsByDomainId::new(domain))?;
    assert!(linked_accounts_after.is_empty());

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

    let domain: DomainId = "receive_without_preregister".parse()?;
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    client.submit_all_blocking(register_domain_and_transfer)?;
    let bob_client = alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &client);

    let destination_asset = gen_account_in(&domain).0;
    let destination_nft = gen_account_in(&domain).0;

    let asset_definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(domain.clone(), "coin".parse()?);
    bob_client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    ))?;
    let source_asset_id = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
    bob_client.submit_blocking(Mint::asset_numeric(10u32, source_asset_id.clone()))?;
    bob_client.submit_blocking(Transfer::asset_numeric(
        source_asset_id,
        4u32,
        destination_asset.clone(),
    ))?;

    let destination_asset_id = AssetId::new(asset_definition_id, destination_asset.clone());
    let destination_asset_state = client.query_single(FindAssetById::new(destination_asset_id))?;
    assert_eq!(*destination_asset_state.value(), Numeric::from(4u32));

    let nft_id: NftId = format!("nft_receive${domain}").parse()?;
    bob_client.submit_blocking(Register::nft(Nft::new(nft_id.clone(), Metadata::default())))?;
    bob_client.submit_blocking(Transfer::nft(
        BOB_ID.clone(),
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

#[test]
fn domain_links_allow_subject_authority_for_link_and_unlink() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        domain_links_allow_subject_authority_for_link_and_unlink
    )) else {
        return Ok(());
    };
    let client = network.client();

    let target_domain: DomainId = "subject_link_target".parse()?;
    let controller_domain: DomainId = "subject_link_controller".parse()?;
    let register_domains: [InstructionBox; 2] = [
        Register::domain(Domain::new(target_domain.clone())).into(),
        Register::domain(Domain::new(controller_domain.clone())).into(),
    ];
    client.submit_all_blocking(register_domains)?;

    let (subject_account, subject_keypair) = gen_account_in(&controller_domain);
    client.submit_blocking(Register::account(Account::new(
        subject_account.to_account_id(controller_domain.clone()),
    )))?;
    let subject_client = alt_client((subject_account.clone(), subject_keypair), &client);

    subject_client.submit_blocking::<InstructionBox>(
        LinkAccountDomain {
            account: subject_account.clone(),
            domain: target_domain.clone(),
        }
        .into(),
    )?;

    let linked_domains: Vec<DomainId> =
        client.query_single(FindDomainsByAccountId::new(subject_account.clone()))?;
    assert!(
        linked_domains.contains(&target_domain),
        "subject authority should be able to add a domain link for itself"
    );

    subject_client.submit_blocking::<InstructionBox>(
        UnlinkAccountDomain {
            account: subject_account.clone(),
            domain: target_domain.clone(),
        }
        .into(),
    )?;

    let linked_domains_after: Vec<DomainId> =
        client.query_single(FindDomainsByAccountId::new(subject_account))?;
    assert!(
        !linked_domains_after.contains(&target_domain),
        "subject authority should be able to remove its own explicit domain link"
    );

    Ok(())
}

#[test]
fn domain_links_reject_unrelated_authority() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(domain_links_reject_unrelated_authority))
    else {
        return Ok(());
    };
    let client = network.client();

    let target_domain: DomainId = "reject_unrelated_authority".parse()?;
    client.submit_blocking(Register::domain(Domain::new(target_domain.clone())))?;

    let (probe_account, _) = gen_account_in("probe_subject");
    client.submit_blocking::<InstructionBox>(
        LinkAccountDomain {
            account: probe_account.clone(),
            domain: target_domain.clone(),
        }
        .into(),
    )?;

    let attacker_domain: DomainId = "attacker_controller".parse()?;
    let (attacker_account, attacker_keypair) = gen_account_in(&attacker_domain);
    let register_attacker: [InstructionBox; 2] = [
        Register::domain(Domain::new(attacker_domain.clone())).into(),
        Register::account(Account::new(
            attacker_account.to_account_id(attacker_domain),
        ))
        .into(),
    ];
    client.submit_all_blocking(register_attacker)?;
    let attacker_client = alt_client((attacker_account, attacker_keypair), &client);

    let err = attacker_client
        .submit_blocking::<InstructionBox>(
            UnlinkAccountDomain {
                account: probe_account,
                domain: target_domain,
            }
            .into(),
        )
        .expect_err("unrelated authority must be rejected");
    assert!(
        err.to_string().contains("not permitted"),
        "unexpected rejection reason: {err}"
    );

    Ok(())
}

#[test]
fn unlink_domain_link_preserves_materialized_asset_ownership() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        unlink_domain_link_preserves_materialized_asset_ownership
    )) else {
        return Ok(());
    };
    let client = network.client();

    let domain: DomainId = "unlink_keeps_ownership".parse()?;
    client.submit_blocking(Register::domain(Domain::new(domain.clone())))?;

    let destination = gen_account_in(&domain).0;
    let definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(domain.clone(), "coin".parse()?);
    client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(definition_id.clone()).with_name(definition_id.name().to_string()),
    ))?;

    let source_asset_id = AssetId::new(definition_id.clone(), ALICE_ID.clone());
    client.submit_blocking(Mint::asset_numeric(11u32, source_asset_id.clone()))?;
    client.submit_blocking(Transfer::asset_numeric(
        source_asset_id,
        4u32,
        destination.clone(),
    ))?;

    let destination_asset_id = AssetId::new(definition_id, destination.clone());
    let before = client.query_single(FindAssetById::new(destination_asset_id.clone()))?;
    assert_eq!(*before.value(), Numeric::from(4u32));

    client.submit_blocking::<InstructionBox>(
        LinkAccountDomain {
            account: destination.clone(),
            domain: domain.clone(),
        }
        .into(),
    )?;

    client.submit_blocking::<InstructionBox>(
        UnlinkAccountDomain {
            account: destination.clone(),
            domain: domain.clone(),
        }
        .into(),
    )?;

    let after = client.query_single(FindAssetById::new(destination_asset_id))?;
    assert_eq!(
        *after.value(),
        Numeric::from(4u32),
        "unlink should not drop already materialized ownership state"
    );

    let linked_domains: Vec<DomainId> =
        client.query_single(FindDomainsByAccountId::new(destination.clone()))?;
    assert!(
        !linked_domains.contains(&domain),
        "unlink should remove explicit subject-domain association"
    );

    Ok(())
}
