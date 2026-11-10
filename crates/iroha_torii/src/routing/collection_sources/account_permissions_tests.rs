//! A long effective-permission list under the budget of the smallest routed-read owner.
use super::*;
use crate::history_producer::HistoryProducerOwner;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::Account,
    permission::{Permission, Permissions},
};
use iroha_primitives::json::Json;
use iroha_torii_shared::list_query::field;

/// Cumulative decode budget of the smallest owner, five of its phase units.
const SMALLEST_OWNER_DECODE_BYTES: usize = 5 * crate::ROUTED_READ_OWNER_PHASE_UNITS[0];

fn read_grant(index: usize) -> Permission {
    Permission::new(
        "CanReadAccountData".to_owned(),
        Json::new(norito::json!({ "account": (format!("sorau{index:060}")) })),
    )
}

fn state_with_read_grants(count: usize) -> (Arc<CoreState>, AccountId) {
    let authority = AccountId::new(
        checked_routing_fixture_keypair(0xC7, Algorithm::Ed25519, "permission paging fixture key")
            .public_key()
            .clone(),
    );
    let mut world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    world.account_permissions_mut_for_testing().insert(
        authority.clone(),
        (0..count).map(read_grant).collect::<Permissions>(),
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    (Arc::new(state), authority)
}

/// A fresh owner of the smallest tier, with the phase ceilings a routed read derives from it.
fn smallest_owner() -> (HistoryProducerOwner, Limits) {
    let working = crate::query_fanout_fixed_overhead_bytes().unwrap()
        + crate::QUERY_FANOUT_PREBODY_UNITS * crate::ROUTED_READ_OWNER_PHASE_UNITS[0];
    let pool = crate::ByteWeightedMemoryPool::new(working).unwrap();
    let envelope = crate::QueryFanoutMemoryEnvelope::for_body_admission(working).unwrap();
    let reservation = crate::QueryFanoutMemoryReservation::from_admitted_fanout(
        pool.try_acquire_parts([working as u64]).unwrap(),
        envelope,
        pool.generation(),
    )
    .unwrap();
    let owner = HistoryProducerOwner::from_reservation(&reservation).unwrap();
    assert_eq!(
        owner.allocation_limit_for_test(),
        SMALLEST_OWNER_DECODE_BYTES
    );
    let mut limits = collection_limits();
    limits.bytes = collections::memory::BytePolicy::for_admitted_read(envelope, working);
    (owner, limits)
}

/// One read as the collection worker runs it: plan, source, projection and response
/// all spend the same owner. Returns the page and the decode budget it consumed.
fn read_page(
    state: &Arc<CoreState>,
    account: &AccountId,
    query: &ListQuery,
) -> (Result<RowPage>, u64) {
    let (owner, limits) = smallest_owner();
    let target = CollectionTarget::AccountPermissions(account.to_string());
    let page = owner.scope(|| -> Result<RowPage> {
        let prepared = collections::prepare(target.spec(), target.scope(), query, &limits)?;
        let page = execute_prepared_collection_local(
            None,
            state,
            (&target, query),
            &MaybeTelemetry::for_tests(),
            &DataspaceReadVisibility::all_for_tests(),
            &prepared,
            &limits,
        )?;
        let page = prepared.project(page)?;
        row_page_response_owned(page.clone(), limits.bytes, &owner)?;
        Ok(page)
    });
    (page, owner.allocation_context().consumed_allocated_bytes())
}

/// Consumption of the second page, which also decodes a cursor.
fn second_page_consumption(grants: usize, filtered: bool) -> u64 {
    let (state, account) = state_with_read_grants(grants);
    let mut query = ListQuery::new().limit(100);
    if filtered {
        query = query.filter(field("name").eq("CanReadAccountData"));
    }
    let (first, _) = read_page(&state, &account, &query);
    let cursor = first
        .expect("first page")
        .next_cursor
        .expect("a second page follows");
    let query = query.cursor(cursor);
    let (second, consumed) = read_page(&state, &account, &query);
    assert_eq!(second.expect("second page").items.len(), 100);
    consumed
}

#[test]
fn the_fixture_list_cannot_be_decoded_whole_by_the_smallest_owner() {
    let grants: Vec<_> = (0..2_400).map(read_grant).collect();
    let (owner, _) = smallest_owner();
    let refused = owner.scope(|| {
        grants
            .iter()
            .try_for_each(|grant| norito::json::from_str::<Value>(grant.payload().get()).map(drop))
    });
    assert!(
        refused.is_err_and(|error| error.is_decode_resource_limit()),
        "pages below would prove nothing if decoding every payload fit this budget"
    );
}

#[test]
fn a_page_spends_the_same_budget_whatever_the_list_length() {
    for filtered in [false, true] {
        let short = second_page_consumption(1_700, filtered);
        let long = second_page_consumption(6_800, filtered);
        assert_eq!(short, long, "filtered: {filtered}");
        assert!(
            (long as usize) < SMALLEST_OWNER_DECODE_BYTES / 4,
            "a page of 100 spent {long} bytes"
        );
    }
}
