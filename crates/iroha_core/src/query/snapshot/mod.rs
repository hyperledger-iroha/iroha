//! Snapshot query execution helpers with telemetry instrumentation.
//!
//! This module centralises snapshot-lane execution so that server-facing
//! callers (Torii, pipeline harnesses, etc.) can reuse the same validation,
//! metrics, and policy enforcement.

#[cfg(feature = "telemetry")]
use std::time::Instant;

use iroha_data_model::{
    prelude::*,
    query::{QueryRequest, QueryResponse},
};

use crate::{
    query::store::LiveQueryStoreHandle,
    smartcontracts::isi::query::{QueryLimits, ValidQueryRequest},
    state::{State, StateReadOnly},
};

/// Error type for snapshot query lane execution.
#[derive(Debug)]
pub enum SnapshotQueryError {
    /// Validation failed in the executor before running the query.
    Validation(iroha_data_model::ValidationFail),
    /// Query execution failed while producing results.
    Execution(iroha_data_model::query::error::QueryExecutionFail),
}

/// Cursor handling mode for iterable queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMode {
    /// Return only the first batch; do not store a cursor in the `LiveQueryStore`.
    Ephemeral,
    /// Store a server-side cursor in the `LiveQueryStore` and allow `Continue`.
    Stored,
}

/// Execute a query against a point-in-time snapshot of the state with the provided cursor mode
/// and query limits.
///
/// Captures a lightweight query snapshot, validates the query, and executes it.
/// Stored cursor mode persists iterators inside the [`LiveQueryStore`] so
/// subsequent `Continue` requests can resume.
///
/// # Errors
/// Returns a validation error if the request is rejected by the executor, or an execution
/// error if producing results fails.
pub fn run_on_snapshot_with_mode(
    state: &State,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    request: QueryRequest,
    mode: CursorMode,
    limits: QueryLimits,
) -> Result<QueryResponse, SnapshotQueryError> {
    let view = state.query_view();

    if matches!(mode, CursorMode::Ephemeral) && matches!(request, QueryRequest::Continue(_)) {
        return Err(SnapshotQueryError::Validation(
            ValidationFail::NotPermitted(
                "cursor continuation requires stored cursor mode".to_owned(),
            ),
        ));
    }

    if let (CursorMode::Stored, QueryRequest::Continue(cursor)) = (&mode, &request) {
        let min_gas = view.pipeline().query_stored_min_gas_units;
        if min_gas > 0 && cursor.gas_budget.unwrap_or(0) < min_gas {
            return Err(SnapshotQueryError::Validation(
                ValidationFail::NotPermitted(format!(
                    "stored cursor continuation requires at least {min_gas} gas units"
                )),
            ));
        }
    }

    let validated = ValidQueryRequest::validate_for_client_parts(request, authority, &view, limits)
        .map_err(SnapshotQueryError::Validation)?;

    #[cfg(feature = "telemetry")]
    let telemetry_start = Instant::now();
    let response = match mode {
        CursorMode::Ephemeral => validated
            .execute_ephemeral(live_query_store, &view, authority)
            .map_err(SnapshotQueryError::Execution)?,
        CursorMode::Stored => validated
            .execute(live_query_store, &view, authority)
            .map_err(SnapshotQueryError::Execution)?,
    };

    #[cfg(feature = "telemetry")]
    {
        let telemetry = view.telemetry;
        if telemetry.is_enabled()
            && let QueryResponse::Iterable(ref output) = response
        {
            let elapsed_ms = telemetry_start.elapsed().as_secs_f64() * 1000.0;
            let mode_label = match mode {
                CursorMode::Ephemeral => "ephemeral",
                CursorMode::Stored => "stored",
            };
            telemetry.observe_snapshot_iterable(mode_label, elapsed_ms, output);
        }
    }

    Ok(response)
}

/// Convenience wrapper for ephemeral cursor semantics.
///
/// # Errors
/// Propagates validation and execution errors from [`run_on_snapshot_with_mode`].
pub fn run_on_snapshot(
    state: &State,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    request: QueryRequest,
    limits: QueryLimits,
) -> Result<QueryResponse, SnapshotQueryError> {
    run_on_snapshot_with_mode(
        state,
        live_query_store,
        authority,
        request,
        CursorMode::Ephemeral,
        limits,
    )
}

#[cfg(test)]
mod tests {
    use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
    };

    #[tokio::test]
    async fn snapshot_iterable_is_ephemeral() {
        let d1 = Domain::new("d1".parse().unwrap()).build(&ALICE_ID);
        let d2 = Domain::new("d2".parse().unwrap()).build(&ALICE_ID);
        let d3 = Domain::new("d3".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d1.clone(), d2.clone(), d3.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, store.clone(), ChainId::from("chain"));

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let erased = iroha_data_model::query::ErasedIterQuery::<Domain>::new(
            iroha_data_model::query::dsl::CompoundPredicate::PASS,
            iroha_data_model::query::dsl::SelectorTuple::default(),
            payload,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);
        let qreq = iroha_data_model::query::QueryRequest::Start(
            iroha_data_model::query::QueryWithParams::new(&qbox, params),
        );

        let resp = run_on_snapshot(&state, &store, &ALICE_ID, qreq, QueryLimits::default())
            .expect("query ok");
        let iroha_data_model::query::QueryResponse::Iterable(batch) = resp else {
            panic!("expected iterable")
        };
        let (_out, _rem, cursor) = batch.into_parts();
        assert!(cursor.is_none());
    }

    #[tokio::test]
    async fn snapshot_iterable_continuation_is_snapshot_consistent() {
        let d1 = Domain::new("d1".parse().unwrap()).build(&ALICE_ID);
        let d2 = Domain::new("d2".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d1.clone(), d2.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = LiveQueryStore::start_test();
        let state =
            State::new_with_chain(world, kura.clone(), store.clone(), ChainId::from("chain"));

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(nonzero_ext::nonzero!(1_u64).into()),
        };
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let erased = iroha_data_model::query::ErasedIterQuery::<Domain>::new(
            iroha_data_model::query::dsl::CompoundPredicate::PASS,
            iroha_data_model::query::dsl::SelectorTuple::default(),
            payload,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);
        let req = iroha_data_model::query::QueryRequest::Start(
            iroha_data_model::query::QueryWithParams::new(&qbox, params),
        );

        let snapshot_ids: std::collections::BTreeSet<_> = state
            .view()
            .world
            .domains
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        assert_eq!(snapshot_ids.len(), 2);

        let iroha_data_model::query::QueryResponse::Iterable(first) = run_on_snapshot_with_mode(
            &state,
            &store,
            &ALICE_ID,
            req,
            CursorMode::Stored,
            QueryLimits::default(),
        )
        .expect("query ok") else {
            panic!("expected iterable")
        };
        let (batch, _rem, mut cursor) = first.into_parts();
        let mut seen: std::collections::BTreeSet<DomainId> = std::collections::BTreeSet::new();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        for dom in v {
            seen.insert(dom.id().clone());
        }
        assert!(cursor.is_some(), "should have continuation");

        let header =
            crate::block::ValidBlock::new_dummy(iroha_test_samples::ALICE_KEYPAIR.private_key())
                .as_ref()
                .header();
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        let new_id: DomainId = "d3".parse().unwrap();
        Register::domain(Domain::new(new_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");
        stx.apply();
        let _ = sblock.commit();

        while let Some(cur) = cursor.take() {
            let next = store.handle_iter_continue(cur).expect("continue ok");
            let (batch, _rem, next_cur) = next.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
                other => panic!("unexpected batch variant: {other:?}"),
            };
            for dom in v {
                seen.insert(dom.id().clone());
            }
            cursor = next_cur;
        }

        assert_eq!(seen, snapshot_ids);
        assert!(!seen.contains(&"d3".parse::<DomainId>().unwrap()));
    }

    #[tokio::test]
    async fn snapshot_singular_find_parameters_smoke() {
        let d = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let a = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d], [a], []);
        let kura = Kura::blank_kura_for_testing();
        let store = LiveQueryStore::start_test();
        let state = State::new(world, kura, store.clone());

        let req = iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::prelude::FindParameters.into(),
        );

        let resp = run_on_snapshot(&state, &store, &ALICE_ID, req, QueryLimits::default())
            .expect("query ok");
        let iroha_data_model::query::QueryResponse::Singular(out) = resp else {
            panic!("expected singular")
        };
        match &out {
            iroha_data_model::query::SingularQueryOutputBox::Parameters(_p) => {}
            other => panic!("expected Parameters, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stored_cursor_requires_budget_on_continue() {
        let d = Domain::new("lane".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = LiveQueryStore::start_test();
        let mut state = State::new(world, kura, store.clone());
        state.pipeline.query_stored_min_gas_units = 10;

        let cursor = iroha_data_model::query::parameters::ForwardCursor {
            query: "q".to_owned(),
            cursor: nonzero_ext::nonzero!(1_u64),
            gas_budget: Some(1),
        };
        let req = QueryRequest::Continue(cursor);
        let err = run_on_snapshot_with_mode(
            &state,
            &store,
            &ALICE_ID,
            req,
            CursorMode::Stored,
            QueryLimits::default(),
        )
        .expect_err("validation should fail");
        match err {
            SnapshotQueryError::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("gas"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
