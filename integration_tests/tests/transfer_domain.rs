#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for domain permissions and transfers.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{crypto::KeyPair, data_model::prelude::*};
use iroha_executor_data_model::permission::{
    account::CanUnregisterAccount,
    asset::CanTransferAsset,
    asset_definition::CanUnregisterAssetDefinition,
    domain::CanUnregisterDomain,
    nft::{CanRegisterNft, CanUnregisterNft},
    trigger::{CanExecuteTrigger, CanUnregisterTrigger},
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in};
use tokio::runtime::Runtime;

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    sandbox::start_network_blocking_or_skip(builder, context)
}

#[test]
fn domain_owner_domain_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(domain_owner_domain_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, _bob_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::new("kingdom".parse()?, "coin".parse()?);
    let coin = AssetDefinition::numeric(coin_id.clone());

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id(kingdom_id.clone()));
    test_client.submit_blocking(Register::account(bob))?;

    // Asset-definition registration is issuer-owned in first-release semantics.
    test_client.submit_blocking(Register::asset_definition(coin.clone()))?;
    test_client.submit_blocking(Unregister::asset_definition(coin_id))?;

    // check that the canonical ALICE account as owner of domain can edit metadata in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit_blocking(SetKeyValue::domain(kingdom_id.clone(), key.clone(), value))?;
    test_client.submit_blocking(RemoveKeyValue::domain(kingdom_id.clone(), key))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke domain related permissions
    let permission = CanUnregisterDomain {
        domain: kingdom_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    // check that the canonical ALICE account as owner of domain can unregister her domain
    test_client.submit_blocking(Unregister::domain(kingdom_id))?;

    Ok(())
}

#[test]
fn domain_owner_account_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) =
        start_network(builder, stringify!(domain_owner_account_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let kingdom_id: DomainId = "kingdom".parse()?;
    let (mad_hatter_id, _mad_hatter_keypair) = gen_account_in("kingdom");

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id);
    test_client.submit_blocking(Register::domain(kingdom))?;

    let mad_hatter = Account::new(mad_hatter_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(mad_hatter))?;

    // check that the canonical ALICE account as owner of domain can edit metadata of account in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit_blocking(SetKeyValue::account(
        mad_hatter_id.clone(),
        key.clone(),
        value,
    ))?;
    test_client.submit_blocking(RemoveKeyValue::account(mad_hatter_id.clone(), key))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke account related permissions in her domain
    let bob_id = BOB_ID.clone();
    let permission = CanUnregisterAccount {
        account: mad_hatter_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    // check that the canonical ALICE account as owner of domain can unregister accounts in her domain
    test_client.submit_blocking(Unregister::account(mad_hatter_id))?;

    Ok(())
}

#[test]
fn domain_owner_asset_definition_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(
        builder,
        stringify!(domain_owner_asset_definition_permissions),
    )?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let (rabbit_id, _rabbit_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::new("kingdom".parse()?, "coin".parse()?);

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(bob))?;

    let rabbit = Account::new(rabbit_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(rabbit))?;

    // Register asset definition by "bob@kingdom" so he is owner of it.
    let coin = AssetDefinition::numeric(coin_id.clone());
    let transaction = TransactionBuilder::new(network.chain_id(), bob_id.clone())
        .with_instructions([Register::asset_definition(coin)])
        .sign(bob_keypair.private_key());
    test_client.submit_transaction_blocking(&transaction)?;

    // check that the canonical ALICE account as owner of domain can transfer asset definitions in her domain
    test_client.submit_blocking(Transfer::asset_definition(
        bob_id.clone(),
        coin_id.clone(),
        rabbit_id,
    ))?;

    // check that the canonical ALICE account as owner of domain can edit metadata of asset definition in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit_blocking(SetKeyValue::asset_definition(
        coin_id.clone(),
        key.clone(),
        value,
    ))?;
    test_client.submit_blocking(RemoveKeyValue::asset_definition(coin_id.clone(), key))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke asset definition related permissions in her domain
    let permission = CanUnregisterAssetDefinition {
        asset_definition: coin_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    // check that the canonical ALICE account as owner of domain can unregister asset definitions in her domain
    test_client.submit_blocking(Unregister::asset_definition(coin_id))?;

    Ok(())
}

#[test]
fn domain_owner_asset_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(domain_owner_asset_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let alice_id = ALICE_ID.clone();
    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::new("kingdom".parse()?, "coin".parse()?);

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(bob))?;

    // Register asset definition by "bob@kingdom" so he is owner of it.
    let coin = AssetDefinition::numeric(coin_id.clone());
    let transaction = TransactionBuilder::new(network.chain_id(), bob_id.clone())
        .with_instructions([Register::asset_definition(coin)])
        .sign(bob_keypair.private_key());
    test_client.submit_transaction_blocking(&transaction)?;

    // check that the canonical ALICE account as owner of domain can burn, mint and transfer assets in her domain
    let bob_coin_id = AssetId::new(coin_id, bob_id.clone());
    test_client.submit_blocking(Mint::asset_numeric(10u32, bob_coin_id.clone()))?;
    test_client.submit_blocking(Burn::asset_numeric(5u32, bob_coin_id.clone()))?;
    test_client.submit_blocking(Transfer::asset_numeric(bob_coin_id.clone(), 5u32, alice_id))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke asset related permissions in her domain
    let permission = CanTransferAsset { asset: bob_coin_id };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    Ok(())
}

#[test]
fn domain_owner_nft_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(domain_owner_nft_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let nft_id: NftId = "nft$kingdom".parse()?;

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(bob))?;

    // Grant permission to register NFT to "bob@kingdom"
    let permission = CanRegisterNft { domain: kingdom_id };
    test_client.submit_blocking(Grant::account_permission(permission, bob_id.clone()))?;

    // register NFT by "bob@kingdom" so he is owner of it
    let nft = Nft::new(nft_id.clone(), Metadata::default());
    let transaction = TransactionBuilder::new(network.chain_id(), bob_id.clone())
        .with_instructions([Register::nft(nft.clone())])
        .sign(bob_keypair.private_key());
    test_client.submit_transaction_blocking(&transaction)?;

    // check that the canonical ALICE account as owner of domain can edit metadata of NFT in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit_blocking(SetKeyValue::nft(nft_id.clone(), key.clone(), value))?;
    test_client.submit_blocking(RemoveKeyValue::nft(nft_id.clone(), key))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke NFT related permissions in her domain
    let permission = CanUnregisterNft {
        nft: nft_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    // check that the canonical ALICE account as owner of domain can unregister NFT in her domain
    test_client.submit_blocking(Unregister::nft(nft_id.clone()))?;

    Ok(())
}

#[test]
fn domain_owner_trigger_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) =
        start_network(builder, stringify!(domain_owner_trigger_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();

    let alice_id = ALICE_ID.clone();
    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, _bob_keypair) = gen_account_in("kingdom");

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id);
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(bob))?;

    let asset_definition_id = AssetDefinitionId::new("wonderland".parse()?, "rose".parse()?);
    let asset_id = AssetId::new(asset_definition_id, alice_id.clone());
    let trigger_id: TriggerId = "my_trigger".parse()?;

    let trigger_instructions = vec![Mint::asset_numeric(1u32, asset_id)];
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_instructions,
            Repeats::from(2_u32),
            bob_id.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
        ),
    ));
    test_client.submit_blocking(register_trigger)?;

    // check that the canonical ALICE account as owner of domain can edit repetitions of triggers in her domain
    test_client.submit_blocking(Mint::trigger_repetitions(1_u32, trigger_id.clone()))?;
    test_client.submit_blocking(Burn::trigger_repetitions(1_u32, trigger_id.clone()))?;

    // check that the canonical ALICE account as owner of domain can grant execute permission and call triggers in her domain
    let execute_permission = CanExecuteTrigger {
        trigger: trigger_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        execute_permission.clone(),
        alice_id.clone(),
    ))?;

    let execute_trigger = ExecuteTrigger::new(trigger_id.clone());
    test_client.submit_blocking(Instruction::into_instruction_box(Box::new(execute_trigger)))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        execute_permission,
        alice_id.clone(),
    )))?;

    // check that the canonical ALICE account as owner of domain can grant and revoke trigger related permissions in her domain
    let permission = CanUnregisterTrigger {
        trigger: trigger_id.clone(),
    };
    test_client.submit_blocking(Grant::account_permission(
        permission.clone(),
        bob_id.clone(),
    ))?;
    test_client.submit_blocking(RevokeBox::from(Revoke::account_permission(
        permission, bob_id,
    )))?;

    // check that the canonical ALICE account as owner of domain can unregister triggers in her domain
    test_client.submit_blocking(Unregister::trigger(trigger_id))?;

    Ok(())
}

#[test]
fn domain_owner_transfer() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(domain_owner_transfer))? else {
        return Ok(());
    };
    let test_client = network.client();

    let alice_id = ALICE_ID.clone();
    let kingdom_id: DomainId = "kingdom".parse()?;
    let (bob_id, _bob_keypair) = gen_account_in("kingdom");

    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    test_client.submit_blocking(Register::domain(kingdom))?;

    let bob = Account::new(bob_id.to_account_id("kingdom".parse()?));
    test_client.submit_blocking(Register::account(bob))?;

    let domain = test_client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .find(|domain| domain.id() == &kingdom_id)
        .expect("Failed to execute Iroha Query");

    assert_eq!(domain.owned_by(), &alice_id);

    test_client
        .submit_blocking(Transfer::domain(
            alice_id,
            kingdom_id.clone(),
            bob_id.clone(),
        ))
        .expect("Failed to submit transaction");

    let domain = test_client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .find(|domain| domain.id() == &kingdom_id)
        .expect("Failed to execute Iroha Query");
    assert_eq!(domain.owned_by(), &bob_id);

    Ok(())
}

#[test]
fn not_allowed_to_transfer_other_user_domain() -> Result<()> {
    let users_domain: DomainId = "users".parse()?;
    let foo_domain: DomainId = "foo".parse()?;
    let user1 = AccountId::new(KeyPair::random().into_parts().0);
    let user2 = AccountId::new(KeyPair::random().into_parts().0);
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();

    let builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::domain(Domain::new(users_domain.clone())))
        .with_genesis_instruction(Register::account(Account::new(
            user1.to_account_id(users_domain.clone()),
        )))
        .with_genesis_instruction(Register::account(Account::new(
            user2.to_account_id(users_domain.clone()),
        )))
        .with_genesis_instruction(Register::domain(Domain::new(foo_domain.clone())))
        .next_genesis_transaction()
        .with_genesis_instruction(Transfer::domain(
            genesis_account.clone(),
            foo_domain.clone(),
            user1.clone(),
        ))
        .with_genesis_instruction(Transfer::domain(
            genesis_account.clone(),
            users_domain.clone(),
            user1.clone(),
        ));
    let Some((network, _rt)) = start_network(
        builder,
        stringify!(not_allowed_to_transfer_other_user_domain),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    let domain = client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .find(|domain| domain.id() == &foo_domain)
        .expect("Failed to execute Iroha Query");
    assert_eq!(domain.owned_by(), &user1);
    let users = client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .find(|domain| domain.id() == &users_domain)
        .expect("Failed to execute Iroha Query");
    assert_eq!(users.owned_by(), &user1);

    // Client authority is "alice@wonderlang".
    // `foo_domain` is owned by `user1@users`.
    // Alice has no rights to `user1` or `foo_domain`.
    // Therefore transaction should be rejected.
    let transfer_domain = Transfer::domain(user1, foo_domain, user2);
    let result = client.submit_blocking(transfer_domain);
    assert!(result.is_err());

    Ok(())
}
