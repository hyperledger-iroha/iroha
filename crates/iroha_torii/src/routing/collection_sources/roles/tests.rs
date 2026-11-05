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
