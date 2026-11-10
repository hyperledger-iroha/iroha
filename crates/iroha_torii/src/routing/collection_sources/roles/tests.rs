//! State-backed role pagination, authorization, and source-budget regressions.
use super::*;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{account::Account, permission::Permission};
use iroha_primitives::json::Json;
use iroha_torii_shared::list_query::{SortKey, field};

fn fixture() -> (Arc<CoreState>, AccountId, AccountId, Vec<Role>) {
    let account = |seed| {
        AccountId::new(
            checked_routing_fixture_keypair(
                seed,
                iroha_crypto::Algorithm::Ed25519,
                "role collection fixture",
            )
            .public_key()
            .clone(),
        )
    };
    let alice = account(0xC1);
    let bob = account(0xC2);
    let permission = Permission::new(
        "fixture_permission".parse().unwrap(),
        Json::new(norito::json!({"value": 1})),
    );
    let roles: Vec<_> = (0..7)
        .map(|index| {
            Role::new(format!("role_{index}").parse().unwrap(), alice.clone())
                .add_permission(permission.clone())
                .build(&alice)
        })
        .collect();
    let mut world = World::with_assets_and_roles(
        [],
        [
            Account::new(alice.clone()).build(&alice),
            Account::new(bob.clone()).build(&bob),
        ],
        [],
        [],
        [],
        roles.clone(),
    );
    for role in &roles {
        world.grant_role_for_tests(alice.clone(), role.id().clone());
    }
    world.grant_role_for_tests(bob.clone(), roles[3].id().clone());
    (
        Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )),
        alice,
        bob,
        roles,
    )
}
fn run(
    state: &Arc<CoreState>,
    target: &CollectionTarget,
    query: &ListQuery,
    visibility: &DataspaceReadVisibility,
    limits: &Limits,
) -> Result<RowPage> {
    let prepared = collections::prepare(target.spec(), target.scope(), query, limits)?;
    execute(
        state,
        target,
        query,
        &MaybeTelemetry::for_tests(),
        visibility,
        &prepared,
        limits,
    )
}
#[test]
fn roles_and_assignments_seek_both_directions_without_cross_account_rows() {
    let (state, alice, bob, roles) = fixture();
    let limits = collection_limits();
    for target in [
        CollectionTarget::Roles,
        CollectionTarget::AccountRoles(alice.to_string()),
    ] {
        for descending in [false, true] {
            let mut query = ListQuery::new().limit(2).sort_by(if descending {
                SortKey::desc("id")
            } else {
                SortKey::asc("id")
            });
            let mut ids = Vec::new();
            let mut cursors = BTreeSet::new();
            for _ in 0..8 {
                let page = run(
                    &state,
                    &target,
                    &query,
                    &DataspaceReadVisibility::all(),
                    &limits,
                )
                .unwrap();
                assert!(page.items.len() <= 2);
                for row in page.items {
                    if target == CollectionTarget::Roles {
                        let actual: Role =
                            norito::json::from_value(Value::Object(row.clone())).unwrap();
                        let expected = roles.iter().find(|role| role.id() == actual.id()).unwrap();
                        assert_eq!(
                            actual.permissions().collect::<Vec<_>>(),
                            expected.permissions().collect::<Vec<_>>()
                        );
                    }
                    ids.push(row["id"].as_str().unwrap().to_owned());
                }
                let Some(cursor) = page.next_cursor else {
                    break;
                };
                assert!(cursors.insert(cursor.clone()));
                query = query.cursor(cursor);
            }
            let mut expected: Vec<_> = roles.iter().map(|role| role.id().to_string()).collect();
            if descending {
                expected.reverse();
            }
            assert_eq!(ids, expected);
        }
    }
    let page = run(
        &state,
        &CollectionTarget::AccountRoles(bob.to_string()),
        &ListQuery::new(),
        &DataspaceReadVisibility::all(),
        &limits,
    )
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0]["id"].as_str(), Some("role_3"));
}
#[test]
fn public_or_wrong_account_visibility_cannot_disclose_roles() {
    let (state, alice, bob, _) = fixture();
    let limits = collection_limits();
    let public = DataspaceReadVisibility::new(BTreeSet::from([DataSpaceId::UNIVERSAL]), false);
    let exact_alice = public
        .clone()
        .with_exact_account(BTreeSet::new(), alice.clone());
    for visibility in [&public, &exact_alice] {
        assert!(
            run(
                &state,
                &CollectionTarget::Roles,
                &ListQuery::new(),
                visibility,
                &limits
            )
            .is_err()
        );
        assert!(
            run(
                &state,
                &CollectionTarget::AccountRoles(bob.to_string()),
                &ListQuery::new(),
                visibility,
                &limits
            )
            .is_err()
        );
    }
    assert_eq!(
        run(
            &state,
            &CollectionTarget::AccountRoles(alice.to_string()),
            &ListQuery::new(),
            &exact_alice,
            &limits
        )
        .unwrap()
        .items
        .len(),
        7
    );
}
#[test]
fn role_cursor_binds_collection_scope_filter_and_order() {
    let (state, alice, bob, _) = fixture();
    let limits = collection_limits();
    let target = CollectionTarget::AccountRoles(alice.to_string());
    let query = ListQuery::new().limit(1);
    let page = run(
        &state,
        &target,
        &query,
        &DataspaceReadVisibility::all(),
        &limits,
    )
    .unwrap();
    let continuation = query.cursor(page.next_cursor.unwrap());
    assert!(
        run(
            &state,
            &CollectionTarget::AccountRoles(bob.to_string()),
            &continuation,
            &DataspaceReadVisibility::all(),
            &limits
        )
        .is_err()
    );
    assert!(
        run(
            &state,
            &CollectionTarget::Roles,
            &continuation,
            &DataspaceReadVisibility::all(),
            &limits
        )
        .is_err()
    );
    for altered in [
        continuation.clone().filter(field("id").eq("role_2")),
        continuation.sort_by(SortKey::desc("id")),
    ] {
        assert!(
            run(
                &state,
                &target,
                &altered,
                &DataspaceReadVisibility::all(),
                &limits
            )
            .is_err()
        );
    }
}
#[test]
fn exact_role_lookup_does_not_scan_the_prefix_and_source_bytes_remain_bounded() {
    let (state, alice, _, _) = fixture();
    let mut limits = collection_limits();
    limits.max_scanned_rows = 1;
    let exact = ListQuery::new()
        .filter(field("id").eq("role_6"))
        .include_total();
    for target in [
        CollectionTarget::Roles,
        CollectionTarget::AccountRoles(alice.to_string()),
    ] {
        let page = run(
            &state,
            &target,
            &exact,
            &DataspaceReadVisibility::all(),
            &limits,
        )
        .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, Some(1));
        assert!(
            run(
                &state,
                &target,
                &ListQuery::new().include_total(),
                &DataspaceReadVisibility::all(),
                &limits
            )
            .is_err()
        );
    }
    limits.bytes.source_frame_bytes = 8;
    assert!(
        run(
            &state,
            &CollectionTarget::Roles,
            &exact,
            &DataspaceReadVisibility::all(),
            &limits
        )
        .is_err()
    );
}

#[test]
fn role_projections_and_metrics_preserve_the_actual_query_output_shapes() {
    let (state, alice, bob, roles) = fixture();
    let limits = collection_limits();
    let visibility = DataspaceReadVisibility::all();
    for target in [
        CollectionTarget::Roles,
        CollectionTarget::AccountRoles(alice.to_string()),
        CollectionTarget::AccountRoles(bob.to_string()),
    ] {
        let page = run(
            &state,
            &target,
            &ListQuery::new().select(["id"]).include_total(),
            &visibility,
            &limits,
        )
        .unwrap();
        let expected = if target == CollectionTarget::AccountRoles(bob.to_string()) {
            1
        } else {
            7
        };
        assert_eq!(page.total, Some(expected));
        assert!(
            page.items
                .iter()
                .all(|row| row.len() == 1 && row.contains_key("id"))
        );

        let query: ListQuery = norito::json::from_str(
            r#"{"aggregate":{"metrics":[{"alias":"n","fn":"count"},{"alias":"id","fn":"distinct_count","field":"id"}]},"include_total":true}"#,
        )
        .unwrap();
        let page = run(&state, &target, &query, &visibility, &limits).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].len(), 2);
        assert_eq!(page.items[0]["n"].as_u64(), Some(expected));
        // An ungrouped metric alias may be id; it is then a metric, not a role name.
        assert_eq!(page.items[0]["id"].as_u64(), Some(expected));
        assert_eq!(page.total, Some(1));
        assert!(page.next_cursor.is_none());

        let empty = query.filter(field("id").eq("missing_role"));
        let page = run(&state, &target, &empty, &visibility, &limits).unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.total, Some(0));
    }

    let projected = run(
        &state,
        &CollectionTarget::Roles,
        &ListQuery::new().select(["permissions"]).limit(1),
        &visibility,
        &limits,
    )
    .unwrap();
    assert_eq!(projected.items[0].len(), 1);
    let full = norito::json::to_value(&roles[0]).unwrap();
    assert_eq!(projected.items[0]["permissions"], full["permissions"]);
    let query: ListQuery = norito::json::from_str(
        r#"{"aggregate":{"metrics":[{"alias":"sets","fn":"distinct_count","field":"permissions"},{"alias":"sum","fn":"sum","field":"permissions"},{"alias":"min","fn":"min","field":"permissions"},{"alias":"max","fn":"max","field":"permissions"},{"alias":"avg","fn":"avg","field":"permissions"}]}}"#,
    )
    .unwrap();
    let page = run(
        &state,
        &CollectionTarget::Roles,
        &query,
        &visibility,
        &limits,
    )
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].len(), 5);
    assert_eq!(page.items[0]["sets"].as_u64(), Some(1));
    for alias in ["sum", "min", "max", "avg"] {
        assert!(page.items[0][alias].is_null());
    }
}

#[test]
fn role_aggregate_filter_having_and_pagination_stay_in_the_exact_account_scope() {
    let (state, alice, bob, _) = fixture();
    let limits = collection_limits();
    let visibility = DataspaceReadVisibility::all();
    let query: ListQuery = norito::json::from_str(
        r#"{"filter":"id in [\"role_2\", \"role_3\", \"role_4\"] and exists(id) and id is not null","aggregate":{"group_by":["id"],"metrics":[{"alias":"n","fn":"count"}],"having":"n = 1 and id != \"role_4\""},"sort":["id"],"limit":1,"include_total":true}"#,
    )
    .unwrap();
    for target in [
        CollectionTarget::Roles,
        CollectionTarget::AccountRoles(alice.to_string()),
    ] {
        let first = run(&state, &target, &query, &visibility, &limits).unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0]["id"].as_str(), Some("role_2"));
        assert_eq!(first.items[0]["n"].as_u64(), Some(1));
        assert_eq!(first.total, Some(2));
        let next = query.clone().cursor(first.next_cursor.unwrap());
        let second = run(&state, &target, &next, &visibility, &limits).unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0]["id"].as_str(), Some("role_3"));
        assert_eq!(second.total, Some(2));
        assert!(second.next_cursor.is_none());
        assert!(
            run(
                &state,
                &CollectionTarget::AccountRoles(bob.to_string()),
                &next,
                &visibility,
                &limits
            )
            .is_err()
        );
        let mut changed = next;
        let metric = &mut changed.aggregate.as_mut().unwrap().metrics[0];
        metric.r#fn = iroha_torii_shared::list_query::AggregateFn::DistinctCount;
        metric.field = Some("id".into());
        assert!(run(&state, &target, &changed, &visibility, &limits).is_err());
    }
    let bob_page = run(
        &state,
        &CollectionTarget::AccountRoles(bob.to_string()),
        &query,
        &visibility,
        &limits,
    )
    .unwrap();
    assert_eq!(bob_page.items.len(), 1);
    assert_eq!(bob_page.items[0]["id"].as_str(), Some("role_3"));
    assert_eq!(bob_page.total, Some(1));
    assert!(bob_page.next_cursor.is_none());
}

#[test]
fn role_queries_refuse_unknown_fields_empty_projection_and_conflicting_controls() {
    let (state, alice, _, _) = fixture();
    let limits = collection_limits();
    for target in [
        CollectionTarget::Roles,
        CollectionTarget::AccountRoles(alice.to_string()),
    ] {
        for body in [
            r#"{"filter":"owned_by = \"role_1\""}"#,
            r#"{"sort":["permissions"]}"#,
            r#"{"select":[]}"#,
            r#"{"select":["id"],"aggregate":{"metrics":[{"alias":"n","fn":"count"}]}}"#,
            r#"{"aggregate":{"group_by":["permissions"],"metrics":[{"alias":"n","fn":"count"}]}}"#,
            r#"{"aggregate":{"metrics":[{"alias":"n","fn":"sum","field":"id"}]}}"#,
            r#"{"aggregate":{"group_by":["id"],"metrics":[{"alias":"id","fn":"count"}]}}"#,
        ] {
            // Shape/control conflicts fail while decoding; collection-specific fields
            // and aggregate types are then checked by the common planner.
            let refused = match norito::json::from_str::<ListQuery>(body) {
                Ok(query) => run(
                    &state,
                    &target,
                    &query,
                    &DataspaceReadVisibility::all(),
                    &limits,
                )
                .is_err(),
                Err(_) => true,
            };
            assert!(refused, "{body}");
        }
    }
}
