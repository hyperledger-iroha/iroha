//! Bounded stored-role and account-assignment sources over their ordered State indexes.
use super::*;
use iroha_core::state::range_bounds::RoleIdByAccountBounds;
use iroha_data_model::role::{Role, RoleId, RoleIdWithOwner};

/// Seek one account's assignments without scanning other accounts or the cursor's prefix.
fn assignments<'a>(
    world: &'a impl WorldReadOnly,
    account: &AccountId,
    candidates: Option<BTreeSet<RoleId>>,
    after: Option<&'a RoleId>,
    descending: bool,
) -> Box<dyn Iterator<Item = &'a RoleId> + 'a> {
    if let Some(candidates) = candidates {
        let account = account.clone();
        let rows = candidates.into_iter().filter_map(move |id| {
            if !entry_after_cursor(&id, after, descending) {
                return None;
            }
            world
                .account_roles()
                .get_key_value(&RoleIdWithOwner::new(account.clone(), id))
                .map(|(key, ())| &key.id)
        });
        return if descending {
            Box::new(rows.rev())
        } else {
            Box::new(rows)
        };
    }
    let bounds = RoleIdByAccountBounds::new(account);
    let bounds = match (after, descending) {
        (Some(id), true) => bounds.before(id),
        (Some(id), false) => bounds.after(id),
        (None, _) => bounds,
    };
    let rows = world.account_roles().range(bounds).map(|(key, ())| &key.id);
    if descending {
        Box::new(rows.rev())
    } else {
        Box::new(rows)
    }
}

/// Execute under the same row, scan, retained-byte and response ceilings as other collections.
pub(super) fn execute(
    state: &Arc<CoreState>,
    target: &CollectionTarget,
    query: &ListQuery,
    telemetry: &MaybeTelemetry,
    visibility: &DataspaceReadVisibility,
    prepared: &collections::Prepared<'_>,
    limits: &Limits,
) -> Result<RowPage> {
    let world = state.world_view();
    let scan = prepared.ordered_scan();
    let descending = scan.is_some_and(|scan| scan.descending);
    let after = cursor_key(scan.and_then(|scan| scan.after), |id| {
        id.parse::<RoleId>().ok()
    })?;
    let seek_after = after.as_ref().filter(|_| !query.include_total);
    let candidates = id_candidates(query, |id| id.parse::<RoleId>().ok());
    let rows: Box<dyn Iterator<Item = (&RoleId, Option<&Role>)> + '_> = match target {
        CollectionTarget::Roles => {
            if !visibility.can_read_all() {
                return Err(Error::AppForbidden {
                    code: "role_read_denied",
                    message: "CanReadAllLedgerData is required to read stored role definitions"
                        .into(),
                });
            }
            Box::new(
                seek_keys(world.roles(), candidates, seek_after, descending)
                    .map(|(id, role)| (id, Some(role))),
            )
        }
        CollectionTarget::AccountRoles(literal) => {
            let (account, _) = parse_account_path_segment_with_state(
                state,
                literal,
                telemetry,
                target.endpoint(),
            )?;
            // The route owner authenticates the exact account grant, including its
            // revocation, before minting this scope. Public dataspace visibility alone
            // never authorizes disclosure of account role assignments.
            if !visibility.can_read_all() && visibility.exact_account_id() != Some(&account) {
                return Err(Error::AppForbidden {
                    code: "role_read_denied",
                    message: "an authenticated exact-account read scope is required".into(),
                });
            }
            if visibility.allows_account(&world, &account) {
                Box::new(
                    assignments(&world, &account, candidates, seek_after, descending)
                        .map(|id| (id, None)),
                )
            } else {
                Box::new(std::iter::empty())
            }
        }
        _ => return Err(conversion_error("not a role collection".into())),
    };
    let rows = rows.map(|(id, role)| {
        let row = source_filtered_row(
            prepared,
            || limits.bytes.row_fields([("id", id)]),
            || match role {
                Some(role) => limits.bytes.row(role).map_err(Into::into),
                None => limits.bytes.row_fields([("id", id)]).map_err(Into::into),
            },
        )?;
        Ok((
            entry_after_cursor(id, after.as_ref(), descending),
            Some(row),
        ))
    });
    let mut failure = None;
    let rows = until_error(rows, &mut failure);
    let page = if scan.is_some() {
        prepared.execute_ordered(rows, limits)
    } else {
        prepared.execute_entries(rows.map(|(_, row)| row), limits)
    };
    if let Some(error) = failure {
        return Err(error);
    }
    page.map_err(Into::into)
}

#[cfg(test)]
mod tests;
