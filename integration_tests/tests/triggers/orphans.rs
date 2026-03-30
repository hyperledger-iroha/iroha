#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Orphaned trigger cleanup scenarios.

use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{prelude::*, query::trigger::FindTriggers},
};
use iroha_executor_data_model::permission::trigger::CanRegisterTrigger;
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use tokio::task::spawn_blocking;

async fn start_network(context: &'static str) -> eyre::Result<Option<sandbox::SerializedNetwork>> {
    sandbox::start_network_async_or_skip(NetworkBuilder::new(), context).await
}

async fn find_trigger(iroha: &Client, trigger_id: &TriggerId) -> eyre::Result<Option<Trigger>> {
    let client = iroha.clone();
    let trigger_id = trigger_id.clone();
    spawn_blocking(move || {
        Ok(client
            .query(FindTriggers::new())
            .execute_all()
            .ok()
            .and_then(|triggers| {
                triggers
                    .into_iter()
                    .find(|trigger| trigger.id() == &trigger_id)
            }))
    })
    .await?
}

async fn set_up_trigger(
    network: &sandbox::SerializedNetwork,
) -> eyre::Result<(DomainId, AccountId, TriggerId)> {
    let iroha = network.client();
    let failand: DomainId = "failand".parse()?;
    let create_failand = Register::domain(Domain::new(failand.clone()));

    let (the_one_who_fails, account_keypair) = gen_account_in(failand.name());
    let create_the_one_who_fails = Register::account(Account::new_in_domain(
        the_one_who_fails.clone(),
        failand.clone(),
    ));

    let fail_on_account_events = "fail".parse::<TriggerId>()?;
    let fail_isi = Unregister::domain("dummy".parse().unwrap());
    let register_fail_on_account_events = Register::trigger(Trigger::new(
        fail_on_account_events.clone(),
        Action::new(
            [fail_isi],
            Repeats::Indefinitely,
            the_one_who_fails.clone(),
            AccountEventFilter::new(),
        ),
    ));
    let grant_register_trigger_permission = Grant::account_permission(
        CanRegisterTrigger {
            authority: the_one_who_fails.clone(),
        },
        the_one_who_fails.clone(),
    );
    let authority_client = network
        .peers()
        .first()
        .expect("test network should expose at least one peer")
        .client_for(&the_one_who_fails, account_keypair.private_key().clone());
    ensure_domain_registration_lease_for_network(network, &failand)?;
    spawn_blocking({
        let client = iroha.clone();
        let create_failand: InstructionBox = create_failand.into();
        let create_the_one_who_fails: InstructionBox = create_the_one_who_fails.into();
        let grant_register_trigger_permission: InstructionBox =
            grant_register_trigger_permission.into();
        move || {
            client.submit_all_blocking::<InstructionBox>([
                create_failand,
                create_the_one_who_fails,
                grant_register_trigger_permission,
            ])?;
            eyre::Result::<()>::Ok(())
        }
    })
    .await??;
    spawn_blocking({
        let client = authority_client.clone();
        let register_fail_on_account_events: InstructionBox =
            register_fail_on_account_events.into();
        move || client.submit_blocking::<InstructionBox>(register_fail_on_account_events)
    })
    .await??;
    Ok((failand, the_one_who_fails, fail_on_account_events))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_must_be_removed_on_action_authority_account_removal() -> eyre::Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_must_be_removed_on_action_authority_account_removal
    ))
    .await?
    else {
        return Ok(());
    };
    let iroha = network.client();
    let (_, the_one_who_fails, fail_on_account_events) = set_up_trigger(&network).await?;
    let trigger = find_trigger(&iroha, &fail_on_account_events).await?;
    assert_eq!(
        trigger.as_ref().map(Identifiable::id),
        Some(&fail_on_account_events.clone())
    );
    spawn_blocking({
        let client = iroha.clone();
        let the_one_who_fails = the_one_who_fails.clone();
        move || client.submit_blocking(Unregister::account(the_one_who_fails))
    })
    .await??;
    assert_eq!(find_trigger(&iroha, &fail_on_account_events).await?, None);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_must_survive_action_authority_domain_removal() -> eyre::Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_must_survive_action_authority_domain_removal
    ))
    .await?
    else {
        return Ok(());
    };
    let iroha = network.client();
    let (failand, _, fail_on_account_events) = set_up_trigger(&network).await?;
    let trigger = find_trigger(&iroha, &fail_on_account_events).await?;
    assert_eq!(
        trigger.as_ref().map(Identifiable::id),
        Some(&fail_on_account_events.clone())
    );
    spawn_blocking({
        let client = iroha.clone();
        let failand = failand.clone();
        move || client.submit_blocking(Unregister::domain(failand))
    })
    .await??;
    assert_eq!(
        find_trigger(&iroha, &fail_on_account_events)
            .await?
            .as_ref()
            .map(Identifiable::id),
        Some(&fail_on_account_events)
    );
    Ok(())
}
