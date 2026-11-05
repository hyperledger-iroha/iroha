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

#[cfg(test)]
mod complete_role_controls {
    use super::*;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_data_model::prelude::*;
    use iroha_torii_shared::list_query::{SortKey, field};

    fn other_account() -> AccountId {
        AccountId::new(
            checked_routing_fixture_keypair(
                0xDA,
                iroha_crypto::Algorithm::Ed25519,
                "other role collection account",
            )
            .public_key()
            .clone(),
        )
    }

    fn fixture() -> (Arc<State>, AccountId, Vec<Role>) {
        let owner = AccountId::new(
            checked_routing_fixture_keypair(
                0xD9,
                iroha_crypto::Algorithm::Ed25519,
                "role collection fixture",
            )
            .public_key()
            .clone(),
        );
        let roles: Vec<_> = ["alpha", "bravo", "charlie"]
            .into_iter()
            .map(|id| {
                Role::new(id.parse().unwrap(), owner.clone())
                    .add_permission(Permission::new(
                        "can_inspect".into(),
                        Json::new(norito::json!({"scope": "full"})),
                    ))
                    .build(&owner)
            })
            .collect();
        let mut world = World::with_assets_and_roles(
            [],
            [
                Account::new(owner.clone()).build(&owner),
                Account::new(other_account()).build(&owner),
            ],
            [],
            [],
            [],
            roles.clone(),
        );
        for role in &roles {
            world.grant_role_for_tests(owner.clone(), role.id().clone());
        }
        (
            Arc::new(State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            )),
            owner,
            roles,
        )
    }

    fn run(
        state: &Arc<State>,
        target: &CollectionTarget,
        query: &ListQuery,
        limits: &Limits,
    ) -> Result<RowPage> {
        let prepared = collections::prepare(target.spec(), target.scope(), query, limits)?;
        let page = execute_prepared_collection_local(
            None,
            state,
            (target, query),
            &MaybeTelemetry::for_tests(),
            &DataspaceReadVisibility::all_for_tests(),
            &prepared,
            limits,
        )?;
        Ok(prepared.project(page)?)
    }

    #[test]
    fn role_objects_preserve_permissions_and_seek_pages_in_both_directions() {
        let (state, _, roles) = fixture();
        let limits = collection_limits();
        for descending in [false, true] {
            let query = ListQuery::new().limit(1).sort_by(if descending {
                SortKey::desc("id")
            } else {
                SortKey::asc("id")
            });
            let mut current = query.clone();
            let mut actual = Vec::new();
            loop {
                let page = run(&state, &CollectionTarget::Roles, &current, &limits).unwrap();
                actual.extend(
                    page.items
                        .into_iter()
                        .map(|row| norito::json::from_value::<Role>(Value::Object(row)).unwrap()),
                );
                let Some(cursor) = page.next_cursor else {
                    break;
                };
                current = query.clone().cursor(cursor);
            }
            let expected = if descending {
                roles.iter().rev().cloned().collect::<Vec<_>>()
            } else {
                roles.clone()
            };
            assert_eq!(actual, expected);
            for (actual, expected) in actual.iter().zip(&expected) {
                assert_eq!(
                    actual.permissions().collect::<Vec<_>>(),
                    expected.permissions().collect::<Vec<_>>()
                );
            }
        }
        let selected = run(
            &state,
            &CollectionTarget::Roles,
            &ListQuery::new()
                .filter(field("id").eq("bravo"))
                .select(["id"]),
            &limits,
        )
        .unwrap();
        assert_eq!(
            selected.items,
            [Map::from_iter([("id".to_owned(), Value::from("bravo"))])]
        );
    }

    #[test]
    fn assignment_cursor_binds_account_and_collection() {
        let (state, owner, _) = fixture();
        let target = CollectionTarget::AccountRoles(owner.to_string());
        let limits = collection_limits();
        let query = ListQuery::new().limit(1);
        let first = run(&state, &target, &query, &limits).unwrap();
        assert_eq!(first.items[0]["id"], Value::from("alpha"));
        let next = query.cursor(first.next_cursor.unwrap());
        let second = run(&state, &target, &next, &limits).unwrap();
        assert_eq!(second.items[0]["id"], Value::from("bravo"));
        assert!(run(&state, &CollectionTarget::Roles, &next, &limits).is_err());
        assert!(
            run(
                &state,
                &CollectionTarget::AccountRoles(other_account().to_string()),
                &next,
                &limits
            )
            .is_err()
        );
    }

    #[test]
    fn role_sources_refuse_private_visibility_before_materialization() {
        let (state, owner, _) = fixture();
        let limits = collection_limits();
        let query = ListQuery::new();
        for target in [
            CollectionTarget::Roles,
            CollectionTarget::AccountRoles(owner.to_string()),
        ] {
            let prepared =
                collections::prepare(target.spec(), target.scope(), &query, &limits).unwrap();
            let error = execute(
                &state,
                &target,
                &query,
                &MaybeTelemetry::for_tests(),
                &DataspaceReadVisibility::new(BTreeSet::new(), false),
                &prepared,
                &limits,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                Error::AppForbidden {
                    code: "role_read_denied",
                    ..
                }
            ));
        }
    }

    #[test]
    fn role_sources_enforce_frame_and_scan_budgets() {
        let (state, owner, _) = fixture();
        let mut limits = collection_limits();
        limits.bytes.source_frame_bytes = 16;
        let error = run(&state, &CollectionTarget::Roles, &ListQuery::new(), &limits).unwrap_err();
        assert!(
            matches!(error, Error::CollectionQuery(error) if error.code == "query_capacity_exceeded")
        );
        let mut limits = collection_limits();
        limits.max_scanned_rows = 1;
        let target = CollectionTarget::AccountRoles(owner.to_string());
        // Counting scans the entire collection; ordinary ordered reads instead
        // stop at their page scan budget and expose a bounded continuation.
        let counted = ListQuery::new().include_total();
        let error = run(&state, &target, &counted, &limits).unwrap_err();
        assert!(
            matches!(error, Error::CollectionQuery(error) if error.code == "query_scan_limit_exceeded")
        );
        limits.max_scanned_rows = 3;
        limits.ordered_page_scan_budget = 1;
        let page = run(&state, &target, &counted, &limits).unwrap();
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.total, Some(3));
        assert!(page.next_cursor.is_none());

        let query = ListQuery::new();
        let first = run(&state, &target, &query, &limits).unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0]["id"].as_str(), Some("alpha"));
        assert_eq!(first.total, None);
        let next = query.cursor(first.next_cursor.expect("bounded page continuation"));
        let second = run(&state, &target, &next, &limits).unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0]["id"].as_str(), Some("bravo"));
        assert!(second.next_cursor.is_some());
    }
}
