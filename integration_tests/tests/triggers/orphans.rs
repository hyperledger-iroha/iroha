#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Orphaned trigger cleanup scenarios.

use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{prelude::*, query::trigger::FindTriggers},
};
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

async fn set_up_trigger(iroha: &Client) -> eyre::Result<(DomainId, AccountId, TriggerId)> {
    let failand: DomainId = "failand".parse()?;
    let create_failand = Register::domain(Domain::new(failand.clone()));

    let (the_one_who_fails, _account_keypair) = gen_account_in(failand.name());
    let create_the_one_who_fails = Register::account(Account::new(the_one_who_fails.clone()));

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
    spawn_blocking({
        let client = iroha.clone();
        let create_failand: InstructionBox = create_failand.into();
        let create_the_one_who_fails: InstructionBox = create_the_one_who_fails.into();
        let register_fail_on_account_events: InstructionBox =
            register_fail_on_account_events.into();
        move || {
            client.submit_all_blocking::<InstructionBox>([
                create_failand,
                create_the_one_who_fails,
            ])?;
            client.submit_blocking::<InstructionBox>(register_fail_on_account_events)?;
            eyre::Result::<()>::Ok(())
        }
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
    let (_, the_one_who_fails, fail_on_account_events) = set_up_trigger(&iroha).await?;
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
async fn trigger_must_be_removed_on_action_authority_domain_removal() -> eyre::Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_must_be_removed_on_action_authority_domain_removal
    ))
    .await?
    else {
        return Ok(());
    };
    let iroha = network.client();
    let (failand, _, fail_on_account_events) = set_up_trigger(&iroha).await?;
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
    assert_eq!(find_trigger(&iroha, &fail_on_account_events).await?, None);
    Ok(())
}
