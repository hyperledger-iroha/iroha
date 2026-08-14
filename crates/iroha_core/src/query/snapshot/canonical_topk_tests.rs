// Canonical fanout admission tests live here to keep the snapshot module's
// production implementation within its source-size ratchet.
use iroha_data_model::QueryOutputBatchBox;
#[cfg(not(feature = "fast_dsl"))]
use iroha_data_model::query::dsl::CompoundPredicate;
fn canonical_test_limits(max_items: u64) -> QueryLimits {
    use crate::smartcontracts::isi::query::{
        CANONICAL_QUERY_PREBOUNDED_SOURCE_BYTES, CanonicalQueryOutputLimits,
    };
    QueryLimits::new(16).with_canonical_output_limits(CanonicalQueryOutputLimits::new(
        max_items,
        CANONICAL_QUERY_PREBOUNDED_SOURCE_BYTES,
        1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    ))
}
#[cfg(not(feature = "fast_dsl"))]
fn find_role_ids_start(
    params: QueryParams,
    selector: iroha_data_model::query::dsl::SelectorTuple<RoleId>,
) -> QueryRequest {
    find_role_ids_start_with_predicate(
        params,
        iroha_data_model::query::dsl::CompoundPredicate::PASS,
        selector,
    )
}
#[cfg(not(feature = "fast_dsl"))]
fn find_role_ids_start_with_predicate(
    params: QueryParams,
    predicate: iroha_data_model::query::dsl::CompoundPredicate<RoleId>,
    selector: iroha_data_model::query::dsl::SelectorTuple<RoleId>,
) -> QueryRequest {
    let payload =
        norito::codec::Encode::encode(&iroha_data_model::query::role::prelude::FindRoleIds);
    let erased =
        iroha_data_model::query::ErasedIterQuery::<RoleId>::new(predicate, selector, payload);
    let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);
    QueryRequest::Start(iroha_data_model::query::QueryWithParams::new(&qbox, params))
}
#[cfg(not(feature = "fast_dsl"))]
#[test]
fn canonical_role_ids_rejects_large_filter_before_source_execution() {
    let predicate = CompoundPredicate::<RoleId>::build(|prototype| {
        prototype.exists("untrusted-filter-path-".repeat(64 * 1024))
    });
    let world = World::with_assets_and_roles(
        [],
        [alice_account()],
        [],
        [],
        [],
        [
            Role::new("canonical-role".parse().expect("role id"), ALICE_ID.clone())
                .build(&ALICE_ID),
        ],
    );
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-filter-rejection"),
    ));
    let error = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        find_role_ids_start_with_predicate(
            QueryParams::default(),
            predicate,
            SelectorTuple::default(),
        ),
        canonical_test_limits(1),
        QueryExecutionBudget::from_weighted_limit(1024 * 1024, 1, 1),
    )
    .expect_err("filtered role IDs must reject before source execution");
    assert!(matches!(error, SnapshotQueryError::Execution(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message)
        ) if message.contains("filtered") && message.contains("before source execution")));
}
#[cfg(not(feature = "fast_dsl"))]
#[test]
fn budgeted_arc_snapshot_canonical_mode_is_ephemeral_and_offset_bounded() {
    let domain = Domain::new(DomainId::try_new("canonical", "universal").expect("domain id"))
        .build(&ALICE_ID);
    let roles = ["canonical-z", "canonical-a", "canonical-m"]
        .map(|name| Role::new(name.parse().expect("role id"), ALICE_ID.clone()).build(&ALICE_ID));
    let world = World::with_assets_and_roles([domain], [alice_account()], [], [], [], roles);
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-snapshot"),
    ));
    let params = QueryParams {
        pagination: Pagination::new(Some(nonzero_ext::nonzero!(2_u64)), 1),
        ..QueryParams::default()
    };
    let response = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        find_role_ids_start(params, Default::default()),
        canonical_test_limits(3),
        QueryExecutionBudget::from_weighted_limit(16 * 1024 * 1024, 1, 1),
    )
    .expect("budgeted canonical snapshot query");
    let QueryResponse::Iterable(output) = response else {
        panic!("expected iterable response")
    };
    assert_eq!(output.batch.len(), 2);
    assert!(output.continue_cursor.is_none());
    let QueryOutputBatchBox::RoleId(role_ids) =
        output.batch.into_columns().pop().expect("one column")
    else {
        panic!("canonical role-id query changed output variant")
    };
    assert_eq!(role_ids.len(), 2);
}
#[cfg(not(feature = "fast_dsl"))]
#[test]
fn budgeted_arc_snapshot_canonical_mode_rejects_unbounded_domain_source() {
    let domain =
        Domain::new(DomainId::try_new("canonical-unbounded", "universal").expect("domain id"))
            .build(&ALICE_ID);
    let world = World::with([domain], [alice_account()], []);
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-source-rejection"),
    ));
    let error = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        find_domains_start(QueryParams::default()),
        canonical_test_limits(1),
        QueryExecutionBudget::from_weighted_limit(16 * 1024 * 1024, 1, 1),
    )
    .expect_err("unbounded domain rows must be rejected before query execution");
    let SnapshotQueryError::Execution(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
    ) = error
    else {
        panic!("unexpected canonical source rejection: {error:?}")
    };
    assert!(message.contains("FindDomains"));
    assert!(message.contains("before source execution"));
}
#[cfg(all(not(feature = "fast_dsl"), feature = "ids_projection"))]
#[test]
fn budgeted_arc_snapshot_canonical_mode_rejects_selector_before_source_execution() {
    let world = World::with_assets_and_roles(
        [],
        [alice_account()],
        [],
        [],
        [],
        [
            Role::new("canonical-role".parse().expect("role id"), ALICE_ID.clone())
                .build(&ALICE_ID),
        ],
    );
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-selector-rejection"),
    ));
    let error = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        find_role_ids_start(QueryParams::default(), SelectorTuple::ids_only()),
        canonical_test_limits(1),
        QueryExecutionBudget::from_weighted_limit(1024 * 1024, 1, 1),
    )
    .expect_err("selector must be rejected before source execution");
    assert!(matches!(error, SnapshotQueryError::Execution(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message)
        ) if message.contains("before source execution")));
}
#[cfg(not(feature = "fast_dsl"))]
#[test]
fn canonical_roles_by_large_multisig_rejects_before_concrete_payload_decode() {
    use iroha_data_model::account::{MultisigMember, MultisigPolicy};
    let members = (1_u8..=128)
        .map(|seed| {
            let keypair = iroha_crypto::KeyPair::try_from_seed(
                vec![seed; 32],
                iroha_crypto::Algorithm::Ed25519,
            )
            .expect("deterministic member key");
            MultisigMember::new(keypair.public_key().clone(), 1).expect("multisig member")
        })
        .collect();
    let account_id = AccountId::new_multisig(
        MultisigPolicy::new(1, members).expect("large valid multisig policy"),
    );
    let payload = norito::codec::Encode::encode(
        &iroha_data_model::query::role::prelude::FindRolesByAccountId::new(account_id),
    );
    let erased = iroha_data_model::query::ErasedIterQuery::<RoleId>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        payload,
    );
    let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);
    let request = QueryRequest::Start(iroha_data_model::query::QueryWithParams::new(
        &qbox,
        QueryParams::default(),
    ));
    let world = World::with([], [alice_account()], []);
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-multisig-rejection"),
    ));
    let error = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        request,
        canonical_test_limits(1),
        QueryExecutionBudget::from_weighted_limit(1024 * 1024, 1, 1),
    )
    .expect_err("parameterized multisig query must reject before payload decode");
    assert!(matches!(error, SnapshotQueryError::Execution(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message)
        ) if message.contains("FindRolesByAccountId")
            && message.contains("before source execution")));
}
#[cfg(feature = "fast_dsl")]
#[test]
fn canonical_fast_dsl_start_rejects_before_nested_component_decode() {
    let world = World::with([], [alice_account()], []);
    let store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        store.clone(),
        ChainId::from("canonical-fast-dsl-rejection"),
    ));
    let error = run_on_snapshot_ephemeral_with_budget_arc(
        &state,
        &store,
        &ALICE_ID,
        find_domains_start(QueryParams::default()),
        canonical_test_limits(1),
        QueryExecutionBudget::from_weighted_limit(1024 * 1024, 1, 1),
    )
    .expect_err("opaque fast-DSL canonical start must fail closed");
    assert!(matches!(error, SnapshotQueryError::Execution(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message)
        ) if message.contains("before nested payload")));
}
