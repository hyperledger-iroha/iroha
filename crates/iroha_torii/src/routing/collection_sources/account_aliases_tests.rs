//! Alias fields joined onto account rows (`alias_scopes`, `aliases`).
//!
//! Include as a child of `routing::collection_sources`. The fixtures bind real SNS leases through
//! the same helper the by-account route tests use, then read the accounts collection through its
//! production row producer.

use super::account_aliases::{AccountAliasJoin, query_reads_alias_fields};
use super::*;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::Algorithm;
use iroha_data_model::prelude as dm;
use iroha_primitives::json::Json;
use iroha_torii_shared::list_query::{FieldPath, FilterExpr};

const MIBANK: &str = "mibank.universal";
const OTHER: &str = "otherbank.universal";

fn authority(seed: u8) -> dm::AccountId {
    dm::AccountId::new(
        checked_routing_fixture_keypair(seed, Algorithm::Ed25519, "account alias fixture key")
            .public_key()
            .clone(),
    )
}

fn domain_id(name: &str) -> DomainId {
    DomainId::try_new(name, "universal").expect("fixture domain")
}

struct Fixture {
    state: Arc<CoreState>,
    alice: dm::AccountId,
    bob: dm::AccountId,
    carol: dm::AccountId,
    dave: dm::AccountId,
}

/// alice: primary `alice@mibank` and `shop@otherbank`; bob: `bob@otherbank`; carol: no alias;
/// dave: two bindings in the MiBank scope.
fn fixture() -> Fixture {
    let [alice, bob, carol, dave] = [0xD1, 0xD2, 0xD3, 0xD4].map(authority);
    let domains =
        ["mibank", "otherbank"].map(|name| dm::Domain::new(domain_id(name)).build(&alice));
    let accounts = [&alice, &bob, &carol, &dave].map(|id| dm::Account::new(id.clone()).build(id));
    let state = Arc::new(State::new_for_testing(
        World::with(domains, accounts, []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    for (account, literal) in [
        (&alice, "alice@mibank.universal"),
        (&alice, "shop@otherbank.universal"),
        (&bob, "bob@otherbank.universal"),
        (&dave, "dave@mibank.universal"),
        (&dave, "dave2@mibank.universal"),
    ] {
        bind_account_alias_for_test(&state, account, literal);
    }
    set_primary_for_test(&state, &alice, "alice@mibank.universal");
    Fixture {
        state,
        alice,
        bob,
        carol,
        dave,
    }
}

fn set_primary_for_test(state: &Arc<CoreState>, account: &dm::AccountId, literal: &str) {
    let catalog = state.nexus_snapshot().dataspace_catalog;
    let label = iroha_data_model::account::rekey::AccountAlias::from_literal(literal, &catalog)
        .expect("fixture alias");
    let header =
        iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.world_mut_for_testing()
        .account_mut(account)
        .expect("fixture account")
        .set_label(Some(label));
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit primary label");
}

fn read(fixture: &Fixture, query: &ListQuery, visibility: &DataspaceReadVisibility) -> RowPage {
    let target = CollectionTarget::Accounts;
    let limits = collection_limits();
    let prepared = collections::prepare(target.spec(), target.scope(), query, &limits).unwrap();
    let page = execute_prepared_collection_local(
        None,
        &fixture.state,
        (&target, query),
        &MaybeTelemetry::for_tests(),
        visibility,
        &prepared,
        &limits,
    )
    .expect("accounts read");
    prepared.project(page).expect("projection")
}

fn ids(page: &RowPage) -> Vec<String> {
    page.items
        .iter()
        .map(|row| row["id"].as_str().expect("row id").to_owned())
        .collect()
}

fn sorted(ids: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut ids: Vec<String> = ids.into_iter().collect();
    ids.sort();
    ids
}

fn scope_filter(scope: &str) -> FilterExpr {
    FilterExpr::Eq(FieldPath::new("alias_scopes"), Value::from(scope))
}

#[tokio::test]
async fn ordinary_account_reads_do_not_join_aliases() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    for query in [
        ListQuery::new(),
        ListQuery::new().select(["id", "uaid"]),
        ListQuery::new().filter(FilterExpr::IsNull(FieldPath::new("uaid"))),
    ] {
        assert!(!query_reads_alias_fields(&query));
        assert!(
            AccountAliasJoin::for_query(&fixture.state, &query, &all, &collection_limits())
                .is_none()
        );
        let page = read(&fixture, &query, &all);
        assert_eq!(page.items.len(), 4);
        assert!(
            page.items
                .iter()
                .all(|row| !row.contains_key("aliases") && !row.contains_key("alias_scopes")),
            "rows keep today's shape: {:?}",
            page.items
        );
    }
}

#[tokio::test]
async fn any_use_of_a_joined_field_asks_for_the_join() {
    let all = DataspaceReadVisibility::all_for_tests();
    let state = fixture().state;
    for query in [
        ListQuery::new().filter(scope_filter(MIBANK)),
        ListQuery::new().filter(FilterExpr::Not(Box::new(FilterExpr::Exists(
            FieldPath::new("aliases"),
        )))),
        ListQuery::new().filter(FilterExpr::Or(vec![
            FilterExpr::IsNull(FieldPath::new("uaid")),
            scope_filter(OTHER),
        ])),
        ListQuery::new().select(["id", "aliases"]),
        ListQuery::new().select(["alias_scopes"]),
    ] {
        assert!(query_reads_alias_fields(&query), "{query:?}");
        assert!(AccountAliasJoin::for_query(&state, &query, &all, &collection_limits()).is_some());
    }
    assert!(!query_reads_alias_fields(&ListQuery::new().filter(
        FilterExpr::Eq(FieldPath::new("metadata.aliases"), Value::from("x"))
    )));
}

#[tokio::test]
async fn the_scope_filter_keeps_exactly_the_accounts_with_a_binding_in_that_scope() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    let page = read(
        &fixture,
        &ListQuery::new().filter(scope_filter(MIBANK)),
        &all,
    );
    assert_eq!(
        sorted(ids(&page)),
        sorted([fixture.alice.to_string(), fixture.dave.to_string()])
    );
    let other = read(
        &fixture,
        &ListQuery::new().filter(scope_filter(OTHER)),
        &all,
    );
    assert_eq!(
        sorted(ids(&other)),
        sorted([fixture.alice.to_string(), fixture.bob.to_string()])
    );
    let none = read(
        &fixture,
        &ListQuery::new().filter(scope_filter("mibank")),
        &all,
    );
    assert!(
        none.items.is_empty(),
        "the scope is the whole text after `@`"
    );
}

#[tokio::test]
async fn full_rows_of_a_filtered_read_carry_the_aliases_in_by_account_order() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    let page = read(
        &fixture,
        &ListQuery::new().filter(scope_filter(MIBANK)),
        &all,
    );
    let alice = page
        .items
        .iter()
        .find(|row| row["id"] == Value::from(fixture.alice.to_string()))
        .expect("alice");
    assert_eq!(alice["alias_scopes"], norito::json!([MIBANK, OTHER]));
    assert_eq!(
        alice["aliases"],
        norito::json!([
            {"alias": "alice@mibank.universal", "dataspace": "universal", "domain": "mibank", "is_primary": true},
            {"alias": "shop@otherbank.universal", "dataspace": "universal", "domain": "otherbank", "is_primary": false},
        ])
    );
    let dave = page
        .items
        .iter()
        .find(|row| row["id"] == Value::from(fixture.dave.to_string()))
        .expect("dave");
    assert_eq!(
        dave["alias_scopes"],
        norito::json!([MIBANK]),
        "two bindings in one scope list it once"
    );
    assert_eq!(dave["aliases"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn a_projected_read_returns_exactly_the_selected_fields() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    let query = ListQuery::new()
        .filter(scope_filter(MIBANK))
        .select(["id", "aliases"]);
    let page = read(&fixture, &query, &all);
    assert_eq!(page.items.len(), 2);
    for row in &page.items {
        assert_eq!(
            row.keys().map(String::as_str).collect::<Vec<_>>(),
            ["aliases", "id"]
        );
    }
    let carol = read(
        &fixture,
        &ListQuery::new().select(["id", "alias_scopes", "aliases"]),
        &all,
    )
    .items
    .into_iter()
    .find(|row| row["id"] == Value::from(fixture.carol.to_string()))
    .expect("carol");
    assert_eq!(carol["alias_scopes"], norito::json!([]));
    assert_eq!(carol["aliases"], norito::json!([]));
}

#[tokio::test]
async fn negated_filters_see_the_same_fields_including_accounts_without_aliases() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    let not_mibank = read(
        &fixture,
        &ListQuery::new().filter(FilterExpr::Ne(
            FieldPath::new("alias_scopes"),
            Value::from(MIBANK),
        )),
        &all,
    );
    assert_eq!(
        sorted(ids(&not_mibank)),
        sorted([fixture.bob.to_string(), fixture.carol.to_string()])
    );
    let nobody = read(
        &fixture,
        &ListQuery::new().filter(FilterExpr::Not(Box::new(FilterExpr::Exists(
            FieldPath::new("alias_scopes"),
        )))),
        &all,
    );
    assert!(
        nobody.items.is_empty(),
        "every joined row carries the field, even when empty"
    );
}

#[tokio::test]
async fn invisible_accounts_stay_invisible_and_unjoined() {
    let fixture = fixture();
    let nothing = DataspaceReadVisibility::default();
    let page = read(
        &fixture,
        &ListQuery::new().filter(scope_filter(MIBANK)),
        &nothing,
    );
    assert!(page.items.is_empty());
}

#[tokio::test]
async fn aliases_of_other_dataspaces_need_the_callers_resolve_permission() {
    let fixture = fixture();
    let world = fixture.state.world_view();
    let details = |id: &dm::AccountId| world.accounts().get(id).expect("account").clone();
    let alice_details = details(&fixture.alice);
    // The universal dataspace is not public in this join: only a permitted caller reads it.
    let private = |resolver| AccountAliasJoin {
        now_ms: 0,
        public_dataspaces: BTreeSet::new(),
        resolver,
        remaining_visits: std::cell::Cell::new(10_000),
    };
    let bindings = |join: &AccountAliasJoin| {
        join.bindings(
            &world,
            &fixture.alice,
            alice_details.as_ref(),
            collection_limits().bytes,
        )
        .expect("bindings")
        .into_iter()
        .map(|item| item.alias)
        .collect::<Vec<_>>()
    };
    assert!(bindings(&private(None)).is_empty());
    assert!(
        bindings(&private(Some(fixture.bob.clone()))).is_empty(),
        "a caller without a grant reads nothing"
    );
    drop(world);
    grant_resolve_for_test(&fixture.state, &fixture.bob, "alice@mibank.universal");
    let world = fixture.state.world_view();
    let granted = private(Some(fixture.bob.clone()));
    assert_eq!(
        granted
            .bindings(
                &world,
                &fixture.alice,
                alice_details.as_ref(),
                collection_limits().bytes
            )
            .expect("bindings")
            .into_iter()
            .map(|item| item.alias)
            .collect::<Vec<_>>(),
        ["alice@mibank.universal"],
        "an exact or domain grant opens only the aliases it covers"
    );
}

fn grant_resolve_for_test(state: &Arc<CoreState>, grantee: &dm::AccountId, literal: &str) {
    use iroha_data_model::permission::Permission;
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanResolveAccountAlias,
    };
    let catalog = state.nexus_snapshot().dataspace_catalog;
    let resolved =
        iroha_data_model::alias_setup::ResolvedAccountAliasV1::resolve_catalog(literal, &catalog)
            .expect("fixture alias");
    let header =
        iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.world_mut_for_testing().add_account_permission(
        grantee,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Alias(resolved),
        }),
    );
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias resolve permission");
}

#[tokio::test]
async fn a_binding_the_caller_may_not_read_adds_no_scope_either() {
    let fixture = fixture();
    let world = fixture.state.world_view();
    let alice = world.accounts().get(&fixture.alice).expect("alice").clone();
    let hidden = AccountAliasJoin {
        now_ms: 0,
        public_dataspaces: BTreeSet::new(),
        resolver: None,
        remaining_visits: std::cell::Cell::new(10_000),
    };
    let row = hidden
        .fields(
            &world,
            (&fixture.alice, alice.as_ref()),
            collection_limits().bytes,
        )
        .expect("join");
    assert_eq!(row["alias_scopes"], norito::json!([]));
    assert_eq!(row["aliases"], norito::json!([]));
}

#[tokio::test]
async fn pages_follow_the_cursor_when_the_filter_is_selective() {
    let fixture = fixture();
    let all = DataspaceReadVisibility::all_for_tests();
    let mut limits = collection_limits();
    limits.ordered_page_scan_budget = 1;
    let target = CollectionTarget::Accounts;
    let query = ListQuery::new().filter(scope_filter(MIBANK)).limit(1);
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..16 {
        let query = match cursor.take() {
            Some(cursor) => query.clone().cursor(cursor),
            None => query.clone(),
        };
        let prepared =
            collections::prepare(target.spec(), target.scope(), &query, &limits).unwrap();
        let page = execute_prepared_collection_local(
            None,
            &fixture.state,
            (&target, &query),
            &MaybeTelemetry::for_tests(),
            &all,
            &prepared,
            &limits,
        )
        .expect("accounts page");
        seen.extend(ids(&page));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(cursor.is_none(), "the scan reaches the end");
    assert_eq!(
        sorted(seen),
        sorted([fixture.alice.to_string(), fixture.dave.to_string()])
    );
}

#[tokio::test]
async fn an_account_with_more_aliases_than_the_by_account_route_lists_is_refused() {
    let fixture = fixture();
    // The route lists at most EXACT_ALIAS_LOOKUP_MAX_ITEMS bindings; dave already holds two.
    for index in 0..crate::EXACT_ALIAS_LOOKUP_MAX_ITEMS {
        bind_account_alias_for_test(
            &fixture.state,
            &fixture.dave,
            &format!("extra{index:03}@mibank.universal"),
        );
    }
    let all = DataspaceReadVisibility::all_for_tests();
    let query = ListQuery::new().filter(scope_filter(MIBANK));
    let target = CollectionTarget::Accounts;
    let limits = collection_limits();
    let prepared = collections::prepare(target.spec(), target.scope(), &query, &limits).unwrap();
    let error = execute_prepared_collection_local(
        None,
        &fixture.state,
        (&target, &query),
        &MaybeTelemetry::for_tests(),
        &all,
        &prepared,
        &limits,
    )
    .unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded"),
        "{error:?}"
    );
    // An ordinary read of the same account is unaffected: the join is paid only on request.
    let page = read(&fixture, &ListQuery::new().select(["id"]), &all);
    assert_eq!(page.items.len(), 4);
}

#[tokio::test]
async fn an_account_the_alias_filter_rejects_never_serializes_its_metadata() {
    let member = authority(0xD5);
    let heavy = authority(0xD6);
    // Serialized, this metadata exceeds the source frame quota below.
    let mut metadata = Metadata::default();
    metadata.insert(
        "dense".parse().unwrap(),
        Json::new(Value::Array(vec![Value::Null; 1024])),
    );
    let domain = dm::Domain::new(domain_id("mibank")).build(&member);
    let accounts = [
        dm::Account::new(member.clone()).build(&member),
        dm::Account::new(heavy.clone())
            .with_metadata(metadata)
            .build(&heavy),
    ];
    let state = Arc::new(State::new_for_testing(
        World::with([domain], accounts, []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    bind_account_alias_for_test(&state, &member, "alice@mibank.universal");
    let mut limits = collection_limits();
    limits.bytes = collections::memory::BytePolicy {
        source_frame_bytes: 4 * 1024,
        row_bytes: 16 * 1024,
        retained_bytes: 64 * 1024,
        scratch_bytes: 64 * 1024,
        response_bytes: 64 * 1024,
    };
    let all = DataspaceReadVisibility::all_for_tests();
    let target = CollectionTarget::Accounts;
    let run = |query: &ListQuery| {
        let prepared = collections::prepare(target.spec(), target.scope(), query, &limits).unwrap();
        execute_prepared_collection_local(
            None,
            &state,
            (&target, query),
            &MaybeTelemetry::for_tests(),
            &all,
            &prepared,
            &limits,
        )
    };
    let page = run(&ListQuery::new().filter(scope_filter(MIBANK)))
        .expect("the filter rejects the heavy account before its metadata is read");
    assert_eq!(ids(&page), [member.to_string()]);
    let error = run(&ListQuery::new()).unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded"),
        "an unfiltered read does serialize it: {error:?}"
    );
}

#[tokio::test]
async fn unrelated_overfull_alias_output_does_not_break_a_scope_filtered_inventory() {
    let fixture = fixture();
    for index in 0..crate::EXACT_ALIAS_LOOKUP_MAX_ITEMS {
        bind_account_alias_for_test(
            &fixture.state,
            &fixture.bob,
            &format!("extra{index:03}@otherbank.universal"),
        );
    }
    let all = DataspaceReadVisibility::all_for_tests();
    let query = ListQuery::new()
        .filter(scope_filter(MIBANK))
        .select(["id", "uaid", "aliases"])
        .limit(500);
    let page = read(&fixture, &query, &all);
    assert_eq!(
        sorted(ids(&page)),
        sorted([fixture.alice.to_string(), fixture.dave.to_string()])
    );
    // Partial filters must retain ordinary OR/NOT semantics: an account whose
    // other predicate matches still needs its complete, bounded output.
    let query = ListQuery::new()
        .filter(FilterExpr::Or(vec![
            scope_filter(MIBANK),
            FilterExpr::Eq(FieldPath::new("id"), Value::from(fixture.bob.to_string())),
        ]))
        .select(["id", "aliases"]);
    let target = CollectionTarget::Accounts;
    let limits = collection_limits();
    let prepared = collections::prepare(target.spec(), target.scope(), &query, &limits).unwrap();
    let error = execute_prepared_collection_local(
        None,
        &fixture.state,
        (&target, &query),
        &MaybeTelemetry::for_tests(),
        &all,
        &prepared,
        &limits,
    )
    .unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded")
    );
    let page = read(
        &fixture,
        &ListQuery::new().filter(FilterExpr::Not(Box::new(scope_filter(OTHER)))),
        &all,
    );
    assert_eq!(
        sorted(ids(&page)),
        sorted([fixture.carol.to_string(), fixture.dave.to_string()])
    );
}

#[tokio::test]
async fn inactive_alias_index_consumes_one_bounded_query_scan_allowance() {
    let fixture = fixture();
    let labels = (0..2048)
        .map(|index| {
            iroha_data_model::account::rekey::AccountAlias::new(
                format!("inactive{index:04}").parse().unwrap(),
                Some(iroha_data_model::account::rekey::AccountAliasDomain(
                    "otherbank".parse().unwrap(),
                )),
                DataSpaceId::new(0),
            )
        })
        .collect();
    let header =
        iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
    let mut block = fixture.state.block(header);
    let mut tx = block.transaction();
    tx.world_mut_for_testing()
        .account_aliases_by_account_mut_for_testing()
        .insert(fixture.bob.clone(), labels);
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
    let world = fixture.state.world_view();
    let query = ListQuery::new().filter(scope_filter(MIBANK));
    let mut limits = collection_limits();
    limits.max_scanned_rows = 8;
    let join = AccountAliasJoin::for_query(
        &fixture.state,
        &query,
        &DataspaceReadVisibility::all_for_tests(),
        &limits,
    )
    .unwrap();
    let error = join
        .scope_fields(&world, &fixture.bob, limits.bytes)
        .unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded")
    );
    assert_eq!(
        join.remaining_visits.get(),
        0,
        "hidden or inactive entries are work too"
    );
    let error = join
        .scope_fields(&world, &fixture.alice, limits.bytes)
        .unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded"),
        "the allowance must not restart for another account"
    );
}

#[tokio::test]
async fn hidden_active_aliases_are_bounded_and_scratch_is_checked_before_visiting() {
    let fixture = fixture();
    let world = fixture.state.world_view();
    let join = AccountAliasJoin {
        now_ms: 0,
        public_dataspaces: BTreeSet::new(),
        resolver: None,
        remaining_visits: std::cell::Cell::new(1),
    };
    let mut bytes = collection_limits().bytes;
    bytes.scratch_bytes = 1;
    assert!(join.scope_fields(&world, &fixture.alice, bytes).is_err());
    assert_eq!(
        join.remaining_visits.get(),
        1,
        "scratch admission precedes any index work"
    );
    let error = join
        .scope_fields(&world, &fixture.alice, collection_limits().bytes)
        .unwrap_err();
    assert!(
        matches!(error, Error::CollectionQuery(ref error) if error.code == "query_capacity_exceeded")
    );
    assert_eq!(
        join.remaining_visits.get(),
        0,
        "even undisclosed active aliases consume scan work"
    );
}

fn admitted_alias_owner() -> (
    crate::QueryFanoutMemoryReservation,
    crate::history_producer::HistoryProducerOwner,
) {
    const WORKING: usize = 48 * 1024 * 1024;
    let pool = crate::ByteWeightedMemoryPool::new(WORKING).unwrap();
    let reservation = crate::QueryFanoutMemoryReservation::from_admitted_fanout(
        pool.try_acquire_parts([WORKING as u64]).unwrap(),
        crate::QueryFanoutMemoryEnvelope::for_body_admission(WORKING).unwrap(),
        pool.generation(),
    )
    .unwrap();
    let owner =
        crate::history_producer::HistoryProducerOwner::from_reservation(&reservation).unwrap();
    (reservation, owner)
}

#[tokio::test]
async fn sns_alias_decode_exhaustion_marks_the_original_collection_owner() {
    let fixture = fixture();
    let (reservation, owner) = admitted_alias_owner();
    let all = DataspaceReadVisibility::all_for_tests();
    let query = ListQuery::new().filter(scope_filter(MIBANK));
    let limits = collection_limits();
    let join = AccountAliasJoin::for_query(&fixture.state, &query, &all, &limits).unwrap();
    let world = fixture.state.world_view();
    let error = owner.scope(|| {
        // Spend the original cumulative decode grant as earlier scanned rows do.
        // Refused increments do not spend credit; stop at the exact remaining byte.
        for quantum in [1024 * 1024, 1024, 1] {
            while norito::core::reserve_decode_allocation(quantum).is_ok() {}
        }
        assert!(
            !reservation.owner_limit_refused(),
            "raw decode accounting does not signal HTTP owner growth"
        );
        join.scope_fields(&world, &fixture.alice, limits.bytes)
            .unwrap_err()
    });
    assert!(
        matches!(
            error,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::GasBudgetExceeded
            ))
        ),
        "{error:?}"
    );
    assert!(
        reservation.owner_limit_refused(),
        "the HTTP admission must retry this same read with a larger owner"
    );
}

#[tokio::test]
async fn malformed_sns_alias_record_does_not_request_a_larger_owner() {
    let fixture = fixture();
    let catalog = fixture.state.nexus_snapshot().dataspace_catalog;
    let label = iroha_data_model::account::rekey::AccountAlias::from_literal(
        "alice@mibank.universal",
        &catalog,
    )
    .unwrap();
    let selector = iroha_core::sns::selector_for_account_alias(&label, &catalog).unwrap();
    let storage_key = iroha_core::sns::record_storage_key(&selector);
    let header =
        iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
    let mut block = fixture.state.block(header);
    let mut tx = block.transaction();
    let world = tx.world_mut_for_testing();
    let mut record = world
        .smart_contract_state()
        .get(&storage_key)
        .unwrap()
        .clone();
    record.push(0); // A canonical SNS record must consume its complete stored frame.
    world
        .smart_contract_state_mut_for_testing()
        .insert(storage_key, record);
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
    let (reservation, owner) = admitted_alias_owner();
    let all = DataspaceReadVisibility::all_for_tests();
    let query = ListQuery::new().filter(scope_filter(MIBANK));
    let limits = collection_limits();
    let join = AccountAliasJoin::for_query(&fixture.state, &query, &all, &limits).unwrap();
    let world = fixture.state.world_view();
    let error = owner
        .scope(|| join.scope_fields(&world, &fixture.alice, limits.bytes))
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::Query(iroha_data_model::ValidationFail::InternalError(_))
        ),
        "{error:?}"
    );
    assert!(
        !reservation.owner_limit_refused(),
        "invalid authoritative state is not an owner-size refusal"
    );
}

#[test]
fn sns_allocator_and_semantic_errors_do_not_request_a_larger_owner() {
    use super::account_aliases::alias_producer_sns_error;
    let (reservation, owner) = admitted_alias_owner();
    owner.scope(|| {
        let allocator = norito::core::classify_decode_attempt::<()>(|| {
            Err(norito::Error::AllocationFailed { bytes: 1024 })
        })
        .unwrap_err()
        .into_local_refusal()
        .unwrap();
        let error = alias_producer_sns_error(iroha_core::sns::SnsError::Deferred(allocator.into()));
        assert!(matches!(
            error,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::GasBudgetExceeded
            ))
        ));
        assert!(!reservation.owner_limit_refused());
        let error = alias_producer_sns_error(iroha_core::sns::SnsError::Conflict(
            "conflicting dataspace mapping".into(),
        ));
        assert!(matches!(error, Error::AppConflict { .. }));
        assert!(!reservation.owner_limit_refused());
    });
}
