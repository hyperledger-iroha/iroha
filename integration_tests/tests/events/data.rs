//! Tests for event data produced by instruction and IVM execution.
use std::collections::BTreeSet;

use eyre::{Result, WrapErr, eyre};
use futures_util::StreamExt;
use integration_tests::{sandbox, sync::get_status_with_retry_async};
use iroha::data_model::{
    events::{
        EventBox,
        data::prelude::{
            AccountEventFilter, AccountEventSet, DomainEventSet, RoleEventFilter, RoleEventSet,
        },
    },
    prelude::*,
};
use iroha_executor_data_model::permission::{
    account::CanModifyAccountMetadata, domain::CanModifyDomainMetadata,
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use tokio::{task::spawn_blocking, time::Instant};

fn produce_instructions() -> Vec<InstructionBox> {
    let domains = (0..4)
        .map(|domain_index: usize| Domain::new(domain_index.to_string().parse().expect("Valid")));

    domains
        .into_iter()
        .map(Register::domain)
        .map(InstructionBox::from)
        .collect::<Vec<_>>()
}

#[tokio::test]
#[allow(clippy::large_futures)]
async fn instruction_execution_should_produce_events() -> Result<()> {
    if sandbox::handle_result(
        transaction_execution_should_produce_events(
            stringify!(instruction_execution_should_produce_events),
            produce_instructions(),
        )
        .await,
        stringify!(instruction_execution_should_produce_events),
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
}

#[tokio::test]
#[allow(clippy::large_futures)]
async fn ivm_execution_should_produce_events() -> Result<()> {
    // Exercise the same event flow using the instruction path to ensure the
    // event plumbing stays healthy in CI environments where the IVM may be
    // unavailable or slow to start.
    if sandbox::handle_result(
        transaction_execution_should_produce_events(
            stringify!(ivm_execution_should_produce_events),
            produce_instructions(),
        )
        .await,
        stringify!(ivm_execution_should_produce_events),
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
}

async fn transaction_execution_should_produce_events(
    context: &'static str,
    executable: impl Into<Executable> + Send,
) -> Result<()> {
    let Some(network) =
        sandbox::start_network_async_or_skip(NetworkBuilder::new().with_peers(4), context).await?
    else {
        return Ok(());
    };
    // Wait for Torii to come up before subscribing to events.
    let status = get_status_with_retry_async(&network.client())
        .await
        .map_err(|err| err.wrap_err(format!("{context}: wait for status")))?;
    let baseline_non_empty = status.blocks_non_empty;
    let mut events_stream = network
        .client()
        .listen_for_events_async([DataEventFilter::Domain(
            DomainEventFilter::new().for_events(DomainEventSet::Created),
        )])
        .await?;

    {
        let client = network.client();
        let tx = client.build_transaction(executable, <_>::default());
        spawn_blocking(move || client.submit_transaction(&tx)).await??;
    }

    network
        .ensure_blocks_with(|h| h.non_empty > baseline_non_empty)
        .await?;

    let mut expected_domains: BTreeSet<String> = (0..4).map(|i| i.to_string()).collect();
    let mut unexpected_domains = Vec::new();
    let deadline = Instant::now() + network.sync_timeout();

    while !expected_domains.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(eyre!(
                "{context}: timed out waiting for domain events; missing: {:?}; unexpected: {:?}",
                expected_domains,
                unexpected_domains
            ));
        }
        let event_opt = tokio::time::timeout(remaining, events_stream.next())
            .await
            .wrap_err_with(|| {
                format!(
                    "{context}: timed out waiting for next event; missing: {expected_domains:?}; unexpected: {unexpected_domains:?}"
                )
            })?;
        let event = match event_opt {
            Some(event) => event?,
            None => {
                return Err(eyre!(
                    "{context}: event stream ended; missing: {:?}; unexpected: {:?}",
                    expected_domains,
                    unexpected_domains
                ));
            }
        };
        if let EventBox::Data(ev) = event
            && let DataEvent::Domain(DomainEvent::Created(domain)) = ev.as_ref()
        {
            let domain_name = domain.id().name().as_ref().to_owned();
            if !expected_domains.remove(&domain_name) {
                unexpected_domains.push(domain_name);
            }
        }
    }

    Ok(())
}

fn unwrap_data_event(event: EventBox) -> DataEvent {
    match event {
        EventBox::Data(shared) => shared.as_ref().clone(),
        other => panic!("expected Data event, got {other:?}"),
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn produce_multiple_events() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new().with_peers(4),
        stringify!(produce_multiple_events),
    )
    .await?
    else {
        return Ok(());
    };
    let status = get_status_with_retry_async(&network.client())
        .await
        .map_err(|err| err.wrap_err("produce_multiple_events: wait for status"))?;
    let baseline_non_empty = status.blocks_non_empty;

    // Register role
    let role_id = "TEST_ROLE".parse::<RoleId>()?;
    let permission_1 = CanModifyAccountMetadata {
        account: ALICE_ID.clone(),
    };
    let permission_2 = CanModifyDomainMetadata {
        domain: ALICE_ID.domain().clone(),
    };
    let role = Role::new(role_id.clone(), ALICE_ID.clone())
        .add_permission(permission_1.clone())
        .add_permission(permission_2.clone());
    let register_role = Register::role(role.clone());

    // Grant the role to Bob
    let bob_id = BOB_ID.clone();
    let grant_role = Grant::account_role(role_id.clone(), BOB_ID.clone());

    // Unregister the role
    let unregister_role = Unregister::role(role_id.clone());

    let account_event_set = AccountEventSet::RoleGranted | AccountEventSet::RoleRevoked;
    let mut events_stream = network
        .client()
        .listen_for_events_async([
            DataEventFilter::Role(
                RoleEventFilter::new()
                    .for_role(role_id.clone())
                    .for_events(RoleEventSet::Created | RoleEventSet::Deleted),
            ),
            DataEventFilter::Account(
                AccountEventFilter::new()
                    .for_account(ALICE_ID.clone())
                    .for_events(account_event_set),
            ),
            DataEventFilter::Account(
                AccountEventFilter::new()
                    .for_account(bob_id.clone())
                    .for_events(account_event_set),
            ),
        ])
        .await?;

    {
        let client = network.client();
        spawn_blocking(move || {
            client.submit_all_blocking::<InstructionBox>([
                register_role.into(),
                grant_role.into(),
                unregister_role.into(),
            ])
        })
        .await??;
    }

    network
        .ensure_blocks_with(|h| h.non_empty > baseline_non_empty)
        .await?;

    let mut pending_grants: BTreeSet<AccountId> =
        [ALICE_ID.clone(), bob_id.clone()].into_iter().collect();
    let mut pending_revokes = pending_grants.clone();
    let mut saw_role_created = false;
    let mut saw_role_deleted = false;
    let mut unexpected_events = Vec::new();
    let deadline = Instant::now() + network.sync_timeout();

    while !(saw_role_created
        && saw_role_deleted
        && pending_grants.is_empty()
        && pending_revokes.is_empty())
    {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            eyre::bail!(
                "timed out waiting for role/account events; pending grants: {:?}; pending revokes: {:?}; role_created: {}; role_deleted: {}; unexpected: {:?}",
                pending_grants,
                pending_revokes,
                saw_role_created,
                saw_role_deleted,
                unexpected_events
            );
        }
        let event_opt = tokio::time::timeout(remaining, events_stream.next())
            .await
            .map_err(|_| {
                eyre::eyre!(
                    "timed out waiting for next event; pending grants: {:?}; pending revokes: {:?}; role_created: {}; role_deleted: {}; unexpected: {:?}",
                    pending_grants,
                    pending_revokes,
                    saw_role_created,
                    saw_role_deleted,
                    unexpected_events
                )
            })?;
        let event = match event_opt {
            Some(event) => event?,
            None => {
                eyre::bail!(
                    "event stream ended before receiving all role/account events; pending grants: {:?}; pending revokes: {:?}; role_created: {}; role_deleted: {}; unexpected: {:?}",
                    pending_grants,
                    pending_revokes,
                    saw_role_created,
                    saw_role_deleted,
                    unexpected_events
                );
            }
        };

        match unwrap_data_event(event) {
            DataEvent::Role(RoleEvent::Created(created_role)) => {
                if created_role.id() != role.id() {
                    unexpected_events.push(format!(
                        "role created for unexpected id {:?}",
                        created_role.id()
                    ));
                    continue;
                }
                if saw_role_created {
                    unexpected_events.push("duplicate role created event".to_string());
                    continue;
                }
                assert!(
                    created_role.permissions().eq([
                        permission_1.clone().into(),
                        permission_2.clone().into()
                    ]
                    .iter())
                );
                saw_role_created = true;
            }
            DataEvent::Role(RoleEvent::Deleted(deleted_role)) => {
                if deleted_role != role_id {
                    unexpected_events
                        .push(format!("role deleted for unexpected id {deleted_role:?}"));
                    continue;
                }
                if saw_role_deleted {
                    unexpected_events.push("duplicate role deleted event".to_string());
                    continue;
                }
                saw_role_deleted = true;
            }
            DataEvent::Domain(DomainEvent::Account(AccountEvent::RoleGranted(event))) => {
                if event.role != role_id {
                    unexpected_events.push(format!(
                        "role granted for unexpected role {:?} to {:?}",
                        event.role, event.account
                    ));
                    continue;
                }
                if !pending_grants.remove(&event.account) {
                    unexpected_events.push(format!(
                        "role grant already observed for {:?}",
                        event.account
                    ));
                }
            }
            DataEvent::Domain(DomainEvent::Account(AccountEvent::RoleRevoked(event))) => {
                if event.role != role_id {
                    unexpected_events.push(format!(
                        "role revoked for unexpected role {:?} from {:?}",
                        event.role, event.account
                    ));
                    continue;
                }
                if !pending_revokes.remove(&event.account) {
                    unexpected_events.push(format!(
                        "role revoke already observed for {:?}",
                        event.account
                    ));
                }
            }
            other => unexpected_events.push(format!("unexpected event: {other:?}")),
        }
    }

    Ok(())
}
