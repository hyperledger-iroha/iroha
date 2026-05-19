#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Data-trigger execution and rollback scenarios.

use std::time::{Duration, Instant};

use eyre::Result;
use integration_tests::sandbox;
use iroha::{client, data_model::prelude::*};
use iroha_data_model::{
    account::{
        AccountAddress,
        rekey::{AccountAlias, AccountAliasDomain},
    },
    nexus::DataSpaceId,
    sns::{
        ACCOUNT_ALIAS_SUFFIX_ID, NameControllerV1, NameSelectorV1, NameStatus, PaymentProofV1,
        RegisterNameRequestV1,
    },
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use iroha_primitives::json::Json;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use tokio::task::spawn_blocking;

const TEST_SNS_LEASE_PAYMENT_NANOS: u64 = 500_000_000;
const ASSET_VALUE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ASSET_VALUE_TIMEOUT: Duration = Duration::from_secs(30);

async fn start_network(context: &'static str) -> Result<Option<sandbox::SerializedNetwork>> {
    sandbox::start_network_async_or_skip(NetworkBuilder::new(), context).await
}

async fn start_custom_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Result<Option<sandbox::SerializedNetwork>> {
    sandbox::start_network_async_or_skip(builder, context).await
}

async fn run_or_skip<F, Fut>(context: &'static str, test: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    if sandbox::handle_result(test().await, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}

fn test_account_alias_name_controller(account: &AccountId) -> Result<NameControllerV1> {
    let address = AccountAddress::from_account_id(account)?;
    Ok(NameControllerV1::account(&address))
}

fn test_account_alias_register_request(
    alias_literal: &str,
    owner: &AccountId,
) -> Result<RegisterNameRequestV1> {
    Ok(RegisterNameRequestV1 {
        selector: NameSelectorV1 {
            version: NameSelectorV1::VERSION,
            suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
            label: alias_literal.to_owned(),
        },
        owner: owner.clone(),
        controllers: vec![test_account_alias_name_controller(owner)?],
        term_years: 1,
        pricing_class_hint: None,
        payment: PaymentProofV1 {
            asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
            gross_amount: TEST_SNS_LEASE_PAYMENT_NANOS,
            net_amount: TEST_SNS_LEASE_PAYMENT_NANOS,
            settlement_tx: Json::from("mock-settlement"),
            payer: owner.clone(),
            signature: Json::from("mock-signature"),
        },
        governance: None,
        metadata: Metadata::default(),
    })
}

fn ensure_account_alias_registration_lease(
    client: &client::Client,
    alias_literal: &str,
) -> Result<()> {
    match client
        .sns()
        .get_name(iroha::sns::SnsNamespacePath::AccountAlias, alias_literal)
    {
        Ok(record) if record.owner == client.account && record.status == NameStatus::Active => {
            Ok(())
        }
        Ok(record) => Err(eyre::eyre!(
            "account alias `{alias_literal}` requires an active SNS lease owned by `{}`; found owner `{}` with status {:?}",
            client.account,
            record.owner,
            record.status
        )),
        Err(_) => {
            let request = test_account_alias_register_request(alias_literal, &client.account)?;
            client.sns().register(&request)?;
            Ok(())
        }
    }
}

fn asset_value(client: &client::Client, asset_id: &AssetId) -> Result<Numeric> {
    let assets = client.query(FindAssets::new()).execute_all()?;
    let asset = assets
        .into_iter()
        .find(|asset| asset.id() == asset_id)
        .ok_or_else(|| eyre::eyre!("asset {asset_id} not found"))?;

    Ok(asset.value().clone())
}

fn wait_for_asset_value(
    client: &client::Client,
    asset_id: &AssetId,
    expected: &Numeric,
    context: &str,
) -> Result<Numeric> {
    let deadline = Instant::now() + ASSET_VALUE_TIMEOUT;
    let mut last_observed = "asset was not queried".to_owned();

    while Instant::now() < deadline {
        match asset_value(client, asset_id) {
            Ok(value) => {
                last_observed = value.to_string();
                if &value == expected {
                    return Ok(value);
                }
            }
            Err(error) => {
                last_observed = format!("query failed: {error}");
            }
        }

        std::thread::sleep(ASSET_VALUE_POLL_INTERVAL);
    }

    Err(eyre::eyre!(
        "timed out waiting for asset {asset_id} to equal {expected} after {context}; last_observed={last_observed}"
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_non_intersecting_execution_paths() -> Result<()> {
    let Some(network) = start_network(stringify!(two_non_intersecting_execution_paths)).await?
    else {
        return Ok(());
    };
    let test_client = network.client();

    run_or_skip(stringify!(two_non_intersecting_execution_paths), || async {
        let account_id = ALICE_ID.clone();
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let asset_id = AssetId::new(asset_definition_id, account_id.clone());

        let prev_value = spawn_blocking({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || asset_value(&client, &asset_id)
        })
        .await??;

        let instruction = Mint::asset_numeric(1u32, asset_id.clone());
        let alias_domain = DomainId::try_new("wonderland", "universal")?;
        let account_alias = AccountAlias::new(
            "mintrose".parse()?,
            Some(AccountAliasDomain::new("wonderland".parse()?)),
            DataSpaceId::UNIVERSAL,
        );
        let account_alias_literal = "mintrose@wonderland.universal";
        spawn_blocking({
            let client = test_client.clone();
            let alias_domain = alias_domain.clone();
            move || -> Result<()> {
                client.submit_blocking(Grant::account_permission(
                    Permission::from(CanManageAccountAlias {
                        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
                    }),
                    ALICE_ID.clone(),
                ))?;
                client.submit_blocking(Grant::account_permission(
                    Permission::from(CanManageAccountAlias {
                        scope: AccountAliasPermissionScope::Domain(alias_domain),
                    }),
                    ALICE_ID.clone(),
                ))?;
                Ok(())
            }
        })
        .await??;
        spawn_blocking({
            let client = test_client.clone();
            move || ensure_account_alias_registration_lease(&client, account_alias_literal)
        })
        .await??;
        let register_trigger = Register::trigger(Trigger::new(
            "mint_rose_1".parse()?,
            Action::new(
                [instruction.clone()],
                Repeats::Indefinitely,
                account_id.clone(),
                AccountEventFilter::new().for_events(AccountEventSet::Created),
            ),
        ));
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(register_trigger)
        })
        .await??;

        let register_trigger = Register::trigger(Trigger::new(
            "mint_rose_2".parse()?,
            Action::new(
                [instruction],
                Repeats::Indefinitely,
                account_id,
                DomainEventFilter::new().for_events(DomainEventSet::Created),
            ),
        ));
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(register_trigger)
        })
        .await??;

        spawn_blocking({
            let client = test_client.clone();
            let account_alias = account_alias.clone();
            move || {
                client.submit_blocking(Register::account(
                    Account::new(gen_account_in("wonderland").0.clone())
                        .with_label(Some(account_alias)),
                ))
            }
        })
        .await??;

        let expected_new_value = prev_value.checked_add(numeric!(1)).unwrap();
        let new_value = spawn_blocking({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || {
                wait_for_asset_value(
                    &client,
                    &asset_id,
                    &expected_new_value,
                    "account-created trigger",
                )
            }
        })
        .await??;

        let neverland: DomainId = DomainId::try_new("neverland", "universal")?;
        ensure_domain_registration_lease_for_network(&network, &neverland)?;
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(Register::domain(Domain::new(neverland)))
        })
        .await??;

        let expected_newer_value = new_value.checked_add(numeric!(1)).unwrap();
        let expected_newer_value_for_wait = expected_newer_value.clone();
        let newer_value = spawn_blocking({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || {
                wait_for_asset_value(
                    &client,
                    &asset_id,
                    &expected_newer_value_for_wait,
                    "domain-created trigger",
                )
            }
        })
        .await??;
        assert_eq!(newer_value, expected_newer_value);

        Ok(())
    })
    .await
}

/// # Scenario
///
/// 1. Capture the current maximum execution depth.
/// 2. Bump the maximum allowed depth via a `SetParameter` instruction.
/// 3. After the change, the maximum allowed depth remains elevated.
///
/// Note: the current execution depth cannot be inspected.
///
/// # Implications
///
/// This test illustrates a potential loophole rather than a legitimate use case.
/// Under `Repeats::Indefinitely`, the trigger would loop indefinitely.
/// Such behavior must be prevented by enforcing:
/// - permissions for executable calls (#5441) and event subscriptions (#5439)
/// - quotas or fee-based consumption (#5440)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cat_depth_and_mouse_depth() -> Result<()> {
    let Some(network) =
        start_custom_network(NetworkBuilder::new(), stringify!(cat_depth_and_mouse_depth)).await?
    else {
        return Ok(());
    };
    let test_client = network.client();

    run_or_skip(stringify!(cat_depth_and_mouse_depth), || async {
        let mut parameters = spawn_blocking({
            let client = test_client.clone();
            move || client.query_single(FindParameters)
        })
        .await??;
        let base_depth = parameters.smart_contract().execution_depth();
        assert!(base_depth > 0, "execution depth should be positive");

        let new_depth = base_depth
            .checked_add(110)
            .expect("execution depth increase should fit in u8");
        spawn_blocking({
            let client = test_client.clone();
            move || {
                client.submit_blocking(SetParameter::new(Parameter::SmartContract(
                    iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(new_depth),
                )))
            }
        })
        .await??;

        parameters = spawn_blocking({
            let client = test_client.clone();
            move || client.query_single(FindParameters)
        })
        .await??;
        assert_eq!(new_depth, parameters.smart_contract().execution_depth());
        Ok(())
    })
    .await
}
