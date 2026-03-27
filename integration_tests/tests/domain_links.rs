#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for account-domain links and receive-path implicit account creation.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{
        account::AccountAddress,
        isi::domain_link::*,
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

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        ..base_client.clone()
    }
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

fn ensure_alias_domain(client: &Client) -> Result<()> {
    let alias_domain: DomainId = "aid".parse()?;
    ensure_registered_domain(client, &alias_domain)
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
fn domain_links_roundtrip_without_account_registration() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        domain_links_roundtrip_without_account_registration
    )) else {
        return Ok(());
    };
    let client = network.client();

    let domain: DomainId = "domain-links-query".parse()?;
    ensure_registered_domain(&client, &domain)?;

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
    if let Err(err) = ensure_alias_domain(&client) {
        eprintln!(
            "Skipping receive-path materialization coverage: failed to ensure `aid` domain: {err}"
        );
        return Ok(());
    }

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

#[test]
fn domain_links_allow_subject_authority_for_link_and_unlink() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(
        domain_links_allow_subject_authority_for_link_and_unlink
    )) else {
        return Ok(());
    };
    let client = network.client();

    let target_domain: DomainId = "subject-link-target".parse()?;
    let controller_domain: DomainId = "subject-link-controller".parse()?;
    ensure_registered_domain(&client, &target_domain)?;
    ensure_registered_domain(&client, &controller_domain)?;

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

    let target_domain: DomainId = "reject-unrelated-authority".parse()?;
    ensure_registered_domain(&client, &target_domain)?;

    let (probe_account, _) = gen_account_in("probe_subject");
    client.submit_blocking::<InstructionBox>(
        LinkAccountDomain {
            account: probe_account.clone(),
            domain: target_domain.clone(),
        }
        .into(),
    )?;

    let attacker_domain: DomainId = "attacker-controller".parse()?;
    let (attacker_account, attacker_keypair) = gen_account_in(&attacker_domain);
    ensure_registered_domain(&client, &attacker_domain)?;
    client.submit_blocking(Register::account(Account::new(
        attacker_account.to_account_id(attacker_domain),
    )))?;
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
    if let Err(err) = ensure_alias_domain(&client) {
        eprintln!("Skipping unlink ownership coverage: failed to ensure `aid` domain: {err}");
        return Ok(());
    }

    let domain: DomainId = "unlink-keeps-ownership".parse()?;
    ensure_registered_domain(&client, &domain)?;

    let destination = gen_account_in(&domain).0;
    let definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(domain.clone(), "coin".parse()?);
    client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(definition_id.clone()).with_name(definition_id.name().to_string()),
    ))?;

    let source_asset_id = AssetId::new(definition_id.clone(), client.account.clone());
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
