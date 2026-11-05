//! Integration tests for domain permissions and transfers.
use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::query::QueryError;
use iroha::{blocking::Client, crypto::KeyPair, data_model::prelude::*};
use iroha_data_model::{
    ValidationFail,
    alias_setup::{AccountAliasRoleV1, AccountProvisionV1},
    query::error::{FindError, QueryExecutionFail},
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanUnregisterAccount},
    asset::{
        CanBurnAssetWithDefinition, CanMintAssetWithDefinition, CanTransferAsset,
        CanTransferAssetWithDefinition,
    },
    asset_definition::{CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition},
    domain::CanUnregisterDomain,
    nft::{CanRegisterNft, CanUnregisterNft},
    trigger::{CanModifyTrigger, CanUnregisterTrigger},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
const DOMAIN_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(30);
const DOMAIN_VISIBILITY_POLL: Duration = Duration::from_millis(100);
fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    sandbox::start_network_blocking_or_skip(builder, context)
}
/// Fund the selected signer from the original signed fee asset and retain the same endpoint.
fn funded_account_client(
    network: &Network,
    client: &Client,
    account: &AccountId,
    keypair: &KeyPair,
) -> Result<Client> {
    ensure!(
        account == &AccountId::new(keypair.public_key().clone()),
        "fixture account must match its signing key"
    );
    let mut builder = client.client().to_builder();
    let asset = iroha_genesis::signed_genesis_root_fee_policy(&network.genesis().0)?.fee_asset_id;
    client.submit(
        Transfer::asset_quantity(
            AssetId::new(asset, builder.account.clone()),
            25_000_u32,
            account.clone(),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    builder.account = account.clone();
    builder.key_pair = keypair.clone();
    Client::from_client(builder.build()?)
}

/// Require the authenticated permission rejection, rather than an unrelated fee or transport error.
fn assert_permission_denied(error: &eyre::Report, expected: &str) {
    use iroha_data_model::transaction::error::TransactionRejectionReason;

    assert!(
        error.chain().any(|cause| matches!(
            cause.downcast_ref::<TransactionRejectionReason>(),
            Some(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message)))
                if message.contains(expected)
        )),
        "expected permission rejection containing {expected:?}, got {error:?}"
    );
}

#[test]
fn permission_assertions_read_wrapped_authenticated_rejections() {
    let reason = iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted("authority cannot modify this metadata".into()),
    );
    let error = eyre::Report::new(reason).wrap_err("transaction reached Rejected finality");
    assert_permission_denied(&error, "authority cannot modify this metadata");
}

#[test]
#[should_panic(expected = "expected permission rejection")]
fn permission_assertions_refuse_unauthenticated_matching_text() {
    let error = eyre!("authority cannot modify this metadata");
    assert_permission_denied(&error, "authority cannot modify this metadata");
}

fn submit_as<I: Into<InstructionBox>>(
    client: &Client,
    authority: &AccountId,
    keypair: &KeyPair,
    instructions: impl IntoIterator<Item = I>,
) -> Result<()> {
    let signer = integration_tests::sync::rebind_blocking_client(client, |builder| {
        builder.account = authority.clone();
        builder.key_pair = keypair.clone();
    });
    signer.submit_all(instructions, FeePaymentIntent::authority(Vec::new(), None))?;
    Ok(())
}
fn fund_signer(client: &Client, authority: AccountId) -> Result<()> {
    let fee_definition = iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
    client.submit(
        Transfer::asset_quantity(
            AssetId::new(fee_definition, ALICE_ID.clone()),
            100u32,
            authority,
        ),
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    Ok(())
}
fn checked_random_account_id() -> AccountId {
    AccountId::new(
        KeyPair::try_random()
            .expect("generate checked transfer-domain account keypair")
            .into_parts()
            .0,
    )
}
#[test]
fn transfer_domain_account_fixture_uses_checked_randomness() {
    let _account_id = checked_random_account_id();
}
fn wait_for_domain_owner(
    client: &Client,
    domain_id: &DomainId,
    expected_owner: &AccountId,
    context: &str,
) -> Result<Domain> {
    let deadline = Instant::now() + DOMAIN_VISIBILITY_TIMEOUT;
    let mut last_owner = None;
    loop {
        match client
            .client()
            .query_single(FindDomainById::new(domain_id.clone()))
        {
            Ok(domain) => {
                ensure!(
                    domain.id() == domain_id,
                    "exact domain query returned another domain"
                );
                if domain.owned_by() == expected_owner {
                    return Ok(domain);
                }
                last_owner = Some(domain.owned_by().clone());
            }
            Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                FindError::Domain(missing),
            )))) if &missing == domain_id => {}
            Err(error) => return Err(error.into()),
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for domain owner after {context}: expected {expected_owner}, last observed {last_owner:?}"
            ));
        }
        std::thread::sleep(DOMAIN_VISIBILITY_POLL);
    }
}
#[test]
fn domain_owner_domain_permissions() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(domain_owner_domain_permissions))?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, _bob_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("kingdom", "universal")?,
        "coin".parse()?,
    );
    let coin = AssetDefinition::numeric(
        coin_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(kingdom_id.clone()),
    );
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Asset-definition registration is issuer-owned in first-release semantics.
    test_client.submit(
        Register::asset_definition(coin.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        Unregister::asset_definition(coin_id),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can edit metadata in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit(
        SetKeyValue::domain(kingdom_id.clone(), key.clone(), value),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        RemoveKeyValue::domain(kingdom_id.clone(), key),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can grant and revoke domain related permissions
    let permission = CanUnregisterDomain {
        domain: kingdom_id.clone(),
    };
    test_client.submit(
        Grant::account_permission(permission.clone(), bob_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        RevokeBox::from(Revoke::account_permission(permission, bob_id)),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can unregister her domain
    test_client.submit(
        Unregister::domain(kingdom_id),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
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
    let kingdom_id = DomainId::try_new("kingdom", "universal")?;
    let (mad_hatter_id, mad_hatter_keypair) = gen_account_in("kingdom");
    submit_ensure_domain_for_network(&network, &test_client, Domain::new(kingdom_id))?;
    test_client.submit(
        Register::account(Account::new(mad_hatter_id.clone())),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let owner = funded_account_client(&network, &test_client, &mad_hatter_id, &mad_hatter_keypair)?;
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    // Domain ownership grants no authority over another canonical, domainless account.
    let set = SetKeyValue::account(mad_hatter_id.clone(), key.clone(), value);
    let error = test_client
        .submit(
            set.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not edit another account's metadata");
    assert_permission_denied(&error, "authority cannot modify this metadata");
    owner.submit(
        set,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let remove = RemoveKeyValue::account(mad_hatter_id.clone(), key);
    let error = test_client
        .submit(
            remove.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not remove another account's metadata");
    assert_permission_denied(&error, "authority cannot remove this metadata");
    owner.submit(
        remove,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let permission = CanUnregisterAccount {
        account: mad_hatter_id.clone(),
    };
    let grant = Grant::account_permission(permission.clone(), BOB_ID.clone());
    let error = test_client
        .submit(
            grant.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not grant another account's permissions");
    assert_permission_denied(&error, "CanUnregisterAccount");
    owner.submit(
        grant,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let revoke = Revoke::account_permission(permission.clone(), BOB_ID.clone());
    let error = test_client
        .submit(
            revoke.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not revoke another account's permissions");
    assert_permission_denied(&error, "CanUnregisterAccount");
    owner.submit(
        revoke,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let unregister = Unregister::account(mad_hatter_id);
    let error = test_client
        .submit(
            unregister.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not unregister another account without a grant");
    assert_permission_denied(&error, "authority cannot remove this resource");
    owner.submit(
        Grant::account_permission(permission, ALICE_ID.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        unregister,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
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
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let (rabbit_id, _rabbit_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("kingdom", "universal")?,
        "coin".parse()?,
    );
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let rabbit = Account::new(rabbit_id.clone());
    test_client.submit(
        Register::account(rabbit),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let owner = funded_account_client(&network, &test_client, &bob_id, &bob_keypair)?;
    // Alice registers in her domain, then transfers the definition to Bob so
    // domain ownership and definition ownership remain separate for the assertions.
    let coin = AssetDefinition::numeric(
        coin_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(kingdom_id.clone()),
    );
    test_client.submit_all::<InstructionBox>(
        [
            Register::asset_definition(coin).into(),
            Transfer::asset_definition(ALICE_ID.clone(), coin_id.clone(), bob_id.clone()).into(),
        ],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let definition = test_client
        .client()
        .query_single(FindAssetDefinitionById::new(coin_id.clone()))?;
    assert_eq!(definition.owned_by(), &bob_id);
    // Asset definitions are issuer-owned in first-release semantics.
    let err = test_client
        .submit(
            Transfer::asset_definition(bob_id.clone(), coin_id.clone(), rabbit_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not transfer another account's asset definition");
    assert!(err.chain().any(|cause| {
        cause
            .to_string()
            .contains("Can't transfer asset definition of another account")
    }));
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    let set = SetKeyValue::asset_definition(coin_id.clone(), key.clone(), value);
    let error = test_client
        .submit(
            set.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not modify another issuer's definition metadata");
    assert_permission_denied(&error, "authority cannot modify this metadata");
    owner.submit(
        set,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let remove = RemoveKeyValue::asset_definition(coin_id.clone(), key);
    let error = test_client
        .submit(
            remove.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not remove another issuer's definition metadata");
    assert_permission_denied(&error, "authority cannot remove this metadata");
    owner.submit(
        remove,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let permission = CanUnregisterAssetDefinition {
        asset_definition: coin_id.clone(),
    };
    let grant = Grant::account_permission(permission.clone(), BOB_ID.clone());
    let error = test_client
        .submit(
            grant.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not grant another issuer's definition permissions");
    assert_permission_denied(&error, "CanUnregisterAssetDefinition");
    owner.submit(
        grant,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let revoke = Revoke::account_permission(permission, BOB_ID.clone());
    let error = test_client
        .submit(
            revoke.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not revoke another issuer's definition permissions");
    assert_permission_denied(&error, "CanUnregisterAssetDefinition");
    owner.submit(
        revoke,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let unregister = Unregister::asset_definition(coin_id.clone());
    let error = test_client
        .submit(
            unregister.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not unregister another issuer's definition");
    assert_permission_denied(&error, "authority cannot remove this resource");
    // Explicit issuer grants are tested only after every domain-only refusal above.
    owner.submit_all::<InstructionBox>(
        [
            Grant::account_permission(
                CanModifyAssetDefinitionMetadata {
                    asset_definition: coin_id.clone(),
                },
                ALICE_ID.clone(),
            )
            .into(),
            Grant::account_permission(
                CanUnregisterAssetDefinition {
                    asset_definition: coin_id.clone(),
                },
                ALICE_ID.clone(),
            )
            .into(),
        ],
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let delegated_key: Name = "delegated_metadata".parse()?;
    test_client.submit_all::<InstructionBox>(
        [
            SetKeyValue::asset_definition(
                coin_id.clone(),
                delegated_key.clone(),
                Json::new("granted"),
            )
            .into(),
            RemoveKeyValue::asset_definition(coin_id, delegated_key).into(),
            unregister.into(),
        ],
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
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
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let coin_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("kingdom", "universal")?,
        "coin".parse()?,
    );
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let owner = funded_account_client(&network, &test_client, &bob_id, &bob_keypair)?;
    // Alice registers in her domain, then transfers the definition to Bob so
    // domain ownership and definition ownership remain separate for the assertions.
    let coin = AssetDefinition::numeric(
        coin_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(kingdom_id.clone()),
    );
    test_client.submit_all::<InstructionBox>(
        [
            Register::asset_definition(coin).into(),
            Transfer::asset_definition(ALICE_ID.clone(), coin_id.clone(), bob_id.clone()).into(),
        ],
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let stored = test_client
        .client()
        .query_single(FindAssetDefinitionById::new(coin_id.clone()))?;
    assert_eq!(stored.owned_by(), &bob_id);
    // Domain ownership does not grant issuer mint/burn or source-account transfer rights.
    let bob_coin_id = AssetId::new(coin_id, bob_id.clone());
    let denied = test_client
        .submit(
            Mint::asset_quantity(20u32, bob_coin_id.clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain ownership must not mint another issuer's asset");
    assert!(
        denied.chain().any(|cause| cause.to_string().contains(
            "Can't mint an asset without owning its definition or an exact mint permission"
        )),
        "unexpected mint rejection: {denied:?}"
    );
    submit_as(
        &test_client,
        &bob_id,
        &bob_keypair,
        [Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: bob_coin_id.definition().clone(),
            },
            ALICE_ID.clone(),
        )],
    )?;
    test_client.submit(
        Mint::asset_quantity(20u32, bob_coin_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let denied = test_client
        .submit(
            Burn::asset_quantity(5u32, bob_coin_id.clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain ownership must not burn another account's asset");
    assert!(
        denied.chain().any(|cause| cause
            .to_string()
            .contains("authority cannot burn this resource")),
        "unexpected burn rejection: {denied:?}"
    );
    submit_as(
        &test_client,
        &bob_id,
        &bob_keypair,
        [Grant::account_permission(
            CanBurnAssetWithDefinition {
                asset_definition: bob_coin_id.definition().clone(),
            },
            ALICE_ID.clone(),
        )],
    )?;
    test_client.submit(
        Burn::asset_quantity(5u32, bob_coin_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let err = test_client
        .submit(
            Transfer::asset_quantity(bob_coin_id.clone(), 5u32, alice_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not transfer another account asset without explicit grant");
    assert!(err.chain().any(|cause| {
        cause
            .to_string()
            .contains("source asset owner must sign the transaction")
    }));
    let alice_id = ALICE_ID.clone();
    let exact_permission = CanTransferAsset {
        asset: bob_coin_id.clone(),
    };
    owner.submit(
        Grant::account_permission(exact_permission.clone(), alice_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        Transfer::asset_quantity(bob_coin_id.clone(), 5u32, alice_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    owner.submit(
        Revoke::account_permission(exact_permission, alice_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let revoked = test_client
        .submit(
            Transfer::asset_quantity(bob_coin_id.clone(), 1u32, alice_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("revoking the asset-specific permission must immediately close access");
    assert_permission_denied(&revoked, "source asset owner must sign the transaction");
    let definition_permission = CanTransferAssetWithDefinition {
        asset_definition: bob_coin_id.definition().clone(),
    };
    owner.submit(
        Grant::account_permission(definition_permission, alice_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        Transfer::asset_quantity(bob_coin_id, 5u32, alice_id),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    Ok(())
}
#[test]
fn active_alias_domain_owner_cannot_transfer_the_aliased_accounts_assets() -> Result<()> {
    let manage_aliases: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        ),
    }
    .into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_genesis_instruction(Grant::account_permission(manage_aliases, ALICE_ID.clone()));
    let Some((network, _rt)) = start_network(
        builder,
        stringify!(active_alias_domain_owner_cannot_transfer_the_aliased_accounts_assets),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let alias_owner = ALICE_ID.clone();
    let (definition_owner, definition_owner_keypair) = gen_account_in("assets");
    let (source, _source_keypair) = gen_account_in("fi");
    let destination = checked_random_account_id();
    let alias_domain = DomainId::try_new("fi", "universal")?;
    let asset_domain = DomainId::try_new("assets", "universal")?;
    let asset_definition =
        AssetDefinitionId::derive_from_components(asset_domain.clone(), "alias_safe_coin".parse()?);
    let source_asset = AssetId::new(asset_definition.clone(), source.clone());
    client.submit_all::<InstructionBox>(
        [
            domain_setup_instruction(&alias_domain, &alias_owner)?,
            domain_setup_instruction(&asset_domain, &alias_owner)?,
            Register::account(Account::new(definition_owner.clone())).into(),
            Register::account(Account::new(source.clone())).into(),
            Register::account(Account::new(destination.clone())).into(),
        ],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    fund_signer(&client, definition_owner.clone())?;
    client.submit(
        Transfer::domain(
            alias_owner.clone(),
            asset_domain.clone(),
            definition_owner.clone(),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    client.submit(
        account_alias_setup_instruction(
            "customer@fi.universal",
            &source,
            AccountProvisionV1::Existing,
            AccountAliasRoleV1::Primary,
        )?,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let issuer = funded_account_client(
        &network,
        &client,
        &definition_owner,
        &definition_owner_keypair,
    )?;
    issuer.submit_all::<InstructionBox>(
        [
            Register::asset_definition(AssetDefinition::numeric(
                asset_definition,
                "alias-safe coin".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(asset_domain),
            ))
            .into(),
            Mint::asset_quantity(10_u32, source_asset.clone()).into(),
        ],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let error = client
        .submit(
            Transfer::asset_quantity(source_asset, 1_u32, destination),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err(
            "owning an active alias domain must not authorize transfers from the aliased account",
        );
    assert!(
        error.chain().any(|cause| {
            cause
                .to_string()
                .contains("source asset owner must sign the transaction")
        }),
        "unexpected alias-domain transfer rejection: {error:?}"
    );
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
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    let nft_id = NftId::new(DomainId::try_new("kingdom", "universal")?, "nft".parse()?);
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Grant the canonical Bob account permission to register NFTs in this domain.
    let permission = CanRegisterNft { domain: kingdom_id };
    test_client.submit(
        Grant::account_permission(permission, bob_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Bob signs the registration and becomes the NFT owner.
    let nft = Nft::new(nft_id.clone(), Metadata::default());
    let owner = funded_account_client(&network, &test_client, &bob_id, &bob_keypair)?;
    owner.submit(
        Register::nft(nft.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can edit metadata of NFT in her domain
    let key: Name = "key".parse()?;
    let value = Json::new("value");
    test_client.submit(
        SetKeyValue::nft(nft_id.clone(), key.clone(), value),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        RemoveKeyValue::nft(nft_id.clone(), key),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can grant and revoke NFT related permissions in her domain
    let permission = CanUnregisterNft {
        nft: nft_id.clone(),
    };
    test_client.submit(
        Grant::account_permission(permission.clone(), bob_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        RevokeBox::from(Revoke::account_permission(permission, bob_id)),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // check that the canonical ALICE account as owner of domain can unregister NFT in her domain
    test_client.submit(
        Unregister::nft(nft_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
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
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, bob_keypair) = gen_account_in("kingdom");
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id);
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    let asset_id = AssetId::new(asset_definition_id, alice_id.clone());
    let trigger_id: TriggerId = "my_trigger".parse()?;
    let trigger_instructions = vec![Mint::asset_quantity(1u32, asset_id)];
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_instructions,
            Repeats::from(2_u32),
            bob_id.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    ));
    let err = test_client
        .submit(
            register_trigger.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not register a trigger owned by another account");
    assert!(
        err.chain()
            .any(|cause| cause.to_string().contains("Missing CanRegisterTrigger"))
    );
    let owner = funded_account_client(&network, &test_client, &bob_id, &bob_keypair)?;
    owner.submit(
        Grant::account_permission(
            iroha_executor_data_model::permission::trigger::CanRegisterTrigger {
                authority: bob_id.clone(),
            },
            bob_id.clone(),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    owner.submit(
        register_trigger,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let mint = Mint::trigger_repetitions(1_u32, trigger_id.clone());
    let error = test_client
        .submit(
            mint.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not modify another account's trigger repetitions");
    assert_permission_denied(&error, "authority cannot modify trigger repetitions");
    owner.submit(
        mint,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let burn = Burn::trigger_repetitions(1_u32, trigger_id.clone());
    let error = test_client
        .submit(
            burn.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not burn another account's trigger repetitions");
    assert_permission_denied(&error, "authority cannot burn this resource");
    owner.submit(
        burn,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let execute_permission = CanExecuteTrigger {
        trigger: trigger_id.clone(),
    };
    let execute_trigger = ExecuteTrigger::new(trigger_id.clone());
    let err = test_client
        .submit(
            Instruction::into_instruction_box(Box::new(execute_trigger)),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not execute another account's trigger without a grant");
    assert_permission_denied(&err, "authority cannot execute this trigger");
    owner.submit(
        Grant::account_permission(execute_permission, alice_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let permission = CanUnregisterTrigger {
        trigger: trigger_id.clone(),
    };
    let error = test_client
        .submit(
            Unregister::trigger(trigger_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("domain owner must not unregister another account's trigger without a grant");
    assert_permission_denied(&error, "authority cannot remove this resource");
    owner.submit(
        Grant::account_permission(
            CanModifyTrigger {
                trigger: trigger_id.clone(),
            },
            alice_id.clone(),
        ),
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit_all::<InstructionBox>(
        [
            Mint::trigger_repetitions(1_u32, trigger_id.clone()).into(),
            Burn::trigger_repetitions(1_u32, trigger_id.clone()).into(),
        ],
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    owner.submit(
        Grant::account_permission(permission, alice_id),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    test_client.submit(
        Unregister::trigger(trigger_id),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
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
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal")?;
    let (bob_id, _bob_keypair) = gen_account_in("kingdom");
    // the canonical ALICE account is owner of "kingdom" domain
    let kingdom = Domain::new(kingdom_id.clone());
    submit_ensure_domain_for_network(&network, &test_client, kingdom)?;
    let bob = Account::new(bob_id.clone());
    test_client.submit(
        Register::account(bob),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let domain =
        wait_for_domain_owner(&test_client, &kingdom_id, &alice_id, "domain registration")?;
    assert_eq!(domain.owned_by(), &alice_id);
    test_client
        .submit(
            Transfer::domain(alice_id, kingdom_id.clone(), bob_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to submit transaction");
    let domain = wait_for_domain_owner(&test_client, &kingdom_id, &bob_id, "domain transfer")?;
    assert_eq!(domain.owned_by(), &bob_id);
    Ok(())
}
#[test]
fn not_allowed_to_transfer_other_user_domain() -> Result<()> {
    let users_domain: DomainId = DomainId::try_new("users", "universal")?;
    let foo_domain: DomainId = DomainId::try_new("foo", "universal")?;
    let user1 = checked_random_account_id();
    let user2 = checked_random_account_id();
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::domain(Domain::new(users_domain.clone())))
        .with_genesis_instruction(Register::account(Account::new(user1.clone())))
        .with_genesis_instruction(Register::account(Account::new(user2.clone())))
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
        .client()
        .query_single(FindDomainById::new(foo_domain.clone()))?;
    assert_eq!(domain.id(), &foo_domain);
    assert_eq!(domain.owned_by(), &user1);
    let users_domain_record = client
        .client()
        .query_single(FindDomainById::new(users_domain.clone()))?;
    assert_eq!(users_domain_record.id(), &users_domain);
    assert_eq!(users_domain_record.owned_by(), &user1);
    // Client authority is "alice@wonderlang".
    // `foo_domain` is owned by `user1@users`.
    // Alice has no rights to `user1` or `foo_domain`.
    // Therefore transaction should be rejected.
    let transfer_domain = Transfer::domain(user1, foo_domain, user2);
    let result = client.submit(
        transfer_domain,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    assert!(result.is_err());
    Ok(())
}
