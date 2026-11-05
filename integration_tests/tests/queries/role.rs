//! Tests queries for retrieving roles and their identifiers.
use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    collections::{Collection, ListQuery, Page, field},
    data_model::prelude::*,
};
use iroha_executor_data_model::permission::account::CanModifyAccountMetadata;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use std::{
    collections::HashSet,
    thread::sleep,
    time::{Duration, Instant},
};
/// Walk small signed pages, rejecting truncation, repeated cursors and duplicate identities.
fn bounded_role_rows(
    query: ListQuery,
    mut fetch: impl FnMut(&ListQuery) -> Result<Page<norito::json::Value>>,
) -> Result<Vec<norito::json::Value>> {
    let mut query = query.limit(2);
    let mut rows = Vec::new();
    let mut cursors = HashSet::new();
    let mut previous_id: Option<RoleId> = None;
    for _ in 0..64 {
        let page = fetch(&query)?;
        eyre::ensure!(
            page.items.len() <= 2 && page.total.is_none(),
            "role page exceeds its bound or supplies an unrequested total"
        );
        for row in page.items {
            let id: RoleId = norito::json::from_value(
                row.get("id")
                    .cloned()
                    .ok_or_else(|| eyre::eyre!("role row has no id"))?,
            )?;
            eyre::ensure!(
                previous_id.as_ref().is_none_or(|previous| previous < &id),
                "role rows are duplicated or out of order"
            );
            previous_id = Some(id);
            rows.push(row);
            eyre::ensure!(rows.len() <= 64, "role fixture exceeds its row bound");
        }
        let Some(cursor) = page.next_cursor else {
            return Ok(rows);
        };
        eyre::ensure!(cursors.insert(cursor.clone()), "role cursor repeated");
        query = query.cursor(cursor);
    }
    eyre::bail!("role fixture exceeds its page bound")
}

fn roles(client: &iroha::blocking::Client, query: ListQuery) -> Result<Vec<Role>> {
    bounded_role_rows(query, |query| {
        Ok(client.list_page(&Collection::Roles, query)?)
    })?
    .into_iter()
    .map(|row| norito::json::from_value(row).map_err(Into::into))
    .collect()
}

#[test]
fn bounded_role_rows_preserves_pages_and_rejects_invalid_envelopes() -> Result<()> {
    let row = |id: &str| norito::json!({"id": id, "permissions": []});
    let page = |items, next_cursor| Page {
        items,
        next_cursor,
        total: None,
    };
    let expected = vec![row("a"), row("b"), row("c")];
    let mut pages = [
        page(expected[..2].to_vec(), Some("next".to_owned())),
        page(Vec::new(), Some("last".to_owned())),
        page(expected[2..].to_vec(), None),
    ]
    .into_iter();
    assert_eq!(
        bounded_role_rows(ListQuery::new(), |_| Ok(pages.next().unwrap()))?,
        expected
    );
    assert!(bounded_role_rows(ListQuery::new(), |_| Ok(page(vec![row("a"); 3], None))).is_err());
    assert!(
        bounded_role_rows(ListQuery::new(), |_| Ok(page(
            vec![row("a"), row("a")],
            None
        )))
        .is_err()
    );
    assert!(
        bounded_role_rows(ListQuery::new(), |_| Ok(page(
            Vec::new(),
            Some("repeat".to_owned())
        )))
        .is_err()
    );
    assert!(
        bounded_role_rows(ListQuery::new(), |_| Ok(Page {
            items: Vec::new(),
            next_cursor: None,
            total: Some(0)
        }))
        .is_err()
    );
    Ok(())
}

fn create_role_ids() -> [RoleId; 5] {
    [
        "a".parse().expect("Valid"),
        "b".parse().expect("Valid"),
        "c".parse().expect("Valid"),
        "d".parse().expect("Valid"),
        "e".parse().expect("Valid"),
    ]
}
fn start_network(
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, tokio::runtime::Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}
#[test]
fn find_roles() -> Result<()> {
    let Some((network, rt)) = start_network(stringify!(find_roles)) else {
        return Ok(());
    };
    let test_client = network.client();
    let role_ids = create_role_ids();
    // Registering roles
    let register_roles = role_ids
        .iter()
        .cloned()
        .map(|role_id| Register::role(Role::new(role_id, ALICE_ID.clone())))
        .collect::<Vec<_>>();
    if sandbox::handle_result(
        test_client.submit_all(
            register_roles,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        stringify!(find_roles),
    )?
    .is_none()
    {
        return Ok(());
    }
    rt.block_on(async { network.ensure_blocks(1).await })?;
    let role_ids = HashSet::from(role_ids);
    // Checking results
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let found_role_ids = match sandbox::handle_result(
            roles(&test_client, ListQuery::new()),
            stringify!(find_roles),
        )? {
            Some(found_role_ids) => found_role_ids
                .into_iter()
                .map(|role| role.id().clone())
                .collect::<HashSet<_>>(),
            None => return Ok(()),
        };
        if role_ids.is_subset(&found_role_ids) {
            break;
        }
        if Instant::now() >= deadline {
            assert!(
                role_ids.is_subset(&found_role_ids),
                "expected registered roles {role_ids:?} to be included in {found_role_ids:?}"
            );
        }
        sleep(Duration::from_millis(250));
    }
    Ok(())
}
#[test]
fn find_role_ids() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(find_role_ids)) else {
        return Ok(());
    };
    let test_client = network.client();
    let role_ids = create_role_ids();
    // Registering roles
    let register_roles = role_ids
        .iter()
        .cloned()
        .map(|role_id| Register::role(Role::new(role_id, ALICE_ID.clone())))
        .collect::<Vec<_>>();
    if sandbox::handle_result(
        test_client.submit_all(
            register_roles,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        stringify!(find_role_ids),
    )?
    .is_none()
    {
        return Ok(());
    }
    let role_ids = HashSet::from(role_ids);
    // Checking results
    let found_role_ids = match sandbox::handle_result(
        roles(&test_client, ListQuery::new()),
        stringify!(find_role_ids),
    )? {
        Some(found_roles) => found_roles,
        None => return Ok(()),
    };
    let found_role_ids = found_role_ids
        .into_iter()
        .map(|role| role.id().clone())
        .collect::<HashSet<_>>();
    assert!(role_ids.is_subset(&found_role_ids));
    Ok(())
}
#[test]
fn find_role_by_id() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(find_role_by_id)) else {
        return Ok(());
    };
    let test_client = network.client();
    let role_id: RoleId = "root".parse().expect("Valid");
    let new_role = Role::new(role_id.clone(), ALICE_ID.clone());
    // Registering role
    let register_role = Register::role(new_role.clone());
    if sandbox::handle_result(
        test_client.submit(
            register_role,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        stringify!(find_role_by_id),
    )?
    .is_none()
    {
        return Ok(());
    }
    let found_role = match sandbox::handle_result(
        roles(
            &test_client,
            ListQuery::new().filter(field("id").eq(role_id.to_string())),
        ),
        stringify!(find_role_by_id),
    )? {
        Some(mut found_roles) => {
            assert_eq!(
                found_roles.len(),
                1,
                "exact role lookup must return one row"
            );
            found_roles.pop().expect("role should exist")
        }
        None => return Ok(()),
    };
    assert_eq!(found_role.id(), new_role.id());
    assert!(found_role.permissions().next().is_none());
    Ok(())
}
#[test]
fn find_unregistered_role_by_id() {
    let Some((network, _rt)) = start_network(stringify!(find_unregistered_role_by_id)) else {
        return;
    };
    let test_client = network.client();
    let role_id: RoleId = "root".parse().expect("Valid");
    let found_roles = roles(
        &test_client,
        ListQuery::new().filter(field("id").eq(role_id.to_string())),
    )
    .expect("query missing role through a complete bounded page");
    // Not found: a complete exact lookup must contain no rows.
    assert!(found_roles.is_empty());
}
#[test]
fn find_roles_by_account_id() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(find_roles_by_account_id)) else {
        return Ok(());
    };
    let test_client = network.client();
    let role_ids = create_role_ids();
    let alice_id = ALICE_ID.clone();
    // Registering roles
    let register_roles = role_ids
        .iter()
        .cloned()
        .map(|role_id| {
            Register::role(Role::new(role_id, alice_id.clone()).add_permission(
                CanModifyAccountMetadata {
                    account: alice_id.clone(),
                },
            ))
        })
        .collect::<Vec<_>>();
    if sandbox::handle_result(
        test_client.submit_all(
            register_roles,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        stringify!(find_roles_by_account_id),
    )?
    .is_none()
    {
        return Ok(());
    }
    let role_ids = HashSet::from(role_ids);
    // Checking results
    let found_role_ids = match sandbox::handle_result(
        bounded_role_rows(ListQuery::new(), |query| {
            Ok(test_client.list_page(&Collection::AccountRoles(alice_id.clone()), query)?)
        }),
        stringify!(find_roles_by_account_id),
    )? {
        Some(ids) => ids,
        None => return Ok(()),
    };
    #[derive(norito::derive::JsonDeserialize)]
    struct Assignment {
        id: RoleId,
    }
    let found_role_ids = found_role_ids
        .into_iter()
        .map(|row| norito::json::from_value::<Assignment>(row).map(|row| row.id))
        .collect::<Result<HashSet<_>, _>>()?;
    assert!(role_ids.is_subset(&found_role_ids));
    Ok(())
}
