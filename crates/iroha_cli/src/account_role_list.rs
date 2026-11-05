//! Preserve role-ID list output while reading authenticated account-assignment pages.

use std::collections::BTreeSet;

use eyre::{Result, WrapErr as _, ensure};
use iroha::{
    blocking::Client as BlockingClient,
    client::Client,
    collections::{Collection, ListQuery},
    data_model::{account::AccountId, role::RoleId},
};

const DEFAULT_PAGE_SIZE: u32 = 100;

#[derive(norito::derive::JsonDeserialize)]
struct Assignment {
    id: RoleId,
}

/// Read the requested ordered window without interpreting a server-owned cursor.
pub(crate) fn list(
    client: Client,
    account: AccountId,
    limit: Option<u64>,
    mut offset: u64,
    fetch_size: Option<u64>,
) -> Result<Vec<RoleId>> {
    ensure!(limit != Some(0), "--limit must be greater than zero");
    let page_size = fetch_size
        .map(u32::try_from)
        .transpose()
        .wrap_err("--fetch-size exceeds the collection's u32 page-size range")?
        .unwrap_or(DEFAULT_PAGE_SIZE);
    ensure!(page_size > 0, "--fetch-size must be greater than zero");
    let client = BlockingClient::from_client(client)?;
    let collection = Collection::AccountRoles(account);
    let mut query = ListQuery::new().limit(page_size);
    let mut cursors = BTreeSet::new();
    let mut previous = None;
    let mut remaining = limit;
    let mut roles = Vec::new();
    loop {
        // The cursor binds identity/filter/order, not page size. Bound the last
        // fetch to the requested prefix instead of reading unwanted role rows.
        let requested = remaining.map_or(u64::from(page_size), |left| {
            u64::from(page_size).min(offset.saturating_add(left))
        });
        let requested = u32::try_from(requested)?;
        query = query.limit(requested);
        let page = client
            .list_page(&collection, &query)
            .wrap_err("failed to read account role assignments")?;
        ensure!(
            u64::try_from(page.items.len())? <= u64::from(requested) && page.total.is_none(),
            "account role page exceeds its bound or supplies an unrequested total"
        );
        let next = query.next_page(&page);
        if let Some(cursor) = &page.next_cursor {
            ensure!(
                !cursor.is_empty() && cursors.insert(cursor.clone()),
                "account role continuation is empty or repeated"
            );
        }
        let assignments = page
            .items
            .into_iter()
            .map(norito::json::from_value::<Assignment>)
            .collect::<Result<Vec<_>, _>>()?;
        for Assignment { id } in assignments {
            ensure!(
                previous.as_ref().is_none_or(|previous| previous < &id),
                "account role assignments are duplicated or out of order"
            );
            previous = Some(id.clone());
            if offset > 0 {
                offset -= 1;
                continue;
            }
            roles.push(id);
            if let Some(left) = remaining.as_mut() {
                *left -= 1;
                if *left == 0 {
                    return Ok(roles);
                }
            }
        }
        let Some(next) = next else {
            return Ok(roles);
        };
        query = next;
    }
}

#[cfg(test)]
#[path = "account_role_list_tests.rs"]
mod tests;
