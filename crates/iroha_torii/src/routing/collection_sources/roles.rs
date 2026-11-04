//! Bounded role and role-assignment reads from the current authoritative World.

use super::*;

/// Stream borrowed role sources through the common admitted collection engine.
pub(super) fn execute(
    state: &Arc<CoreState>,
    target: &CollectionTarget,
    query: &ListQuery,
    telemetry: &MaybeTelemetry,
    visibility: &DataspaceReadVisibility,
    prepared: &collections::Prepared<'_>,
    limits: &Limits,
) -> Result<RowPage> {
    if !visibility.can_read_all() {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "CanReadAllLedgerData permission is required for role collections".to_owned(),
            ),
        ));
    }
    let world = state.world_view();
    let mut failure = None;
    let page = match target {
        CollectionTarget::Roles => {
            let scan = prepared.ordered_scan();
            let after = cursor_key(scan.and_then(|scan| scan.after), |id| id.parse().ok())?;
            let candidates = id_candidates(query, |id| id.parse().ok());
            let rows = seek_keys(
                world.roles(),
                candidates,
                after.as_ref().filter(|_| !query.include_total),
                scan.is_some_and(|scan| scan.descending),
            )
            .map(|(id, role)| {
                Ok((
                    entry_after_cursor(
                        id,
                        after.as_ref(),
                        scan.is_some_and(|scan| scan.descending),
                    ),
                    Some(limits.bytes.row(role)?),
                ))
            });
            let rows = until_error(rows, &mut failure);
            if scan.is_some() {
                prepared.execute_ordered(rows, limits)
            } else {
                prepared.execute_entries(rows.map(|(_, row)| row), limits)
            }
        }
        CollectionTarget::AccountRoles(literal) => {
            let (account, _) = parse_account_path_segment_with_state(
                state,
                literal,
                telemetry,
                target.endpoint(),
            )?;
            let rows = world
                .account_roles_iter(&account)
                .map(|id| Ok(limits.bytes.row_fields([("id", id)])?));
            prepared.execute(until_error(rows, &mut failure), limits)
        }
        _ => unreachable!("role producer receives only role collections"),
    };
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(page?)
}

#[cfg(test)]
mod tests {
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
                Error::Query(iroha_data_model::ValidationFail::NotPermitted(_))
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
        let error = run(&state, &target, &ListQuery::new(), &limits).unwrap_err();
        assert!(
            matches!(error, Error::CollectionQuery(error) if error.code == "query_scan_limit_exceeded")
        );
    }
}
