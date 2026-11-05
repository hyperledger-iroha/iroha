#[cfg(feature = "app_api")]
mod full_trigger_rows {
    //! Complete Trigger source budgets and authenticated stored-cursor ownership.
    use super::*;
    use iroha_data_model::{HasMetadata as _, ValidationFail};
    fn trigger_rows_fixture(
        oversized: bool,
    ) -> (
        SharedAppState,
        AccountId,
        Vec<iroha_data_model::trigger::Trigger>,
    ) {
        use iroha_core::smartcontracts::Execute as _;
        use iroha_data_model::{
            account::Account,
            events::time::{ExecutionTime, TimeEventFilter},
            prelude::{Action, InstructionBox, Json, Register, Repeats, SetKeyValue, Trigger},
        };
        let authority = iroha_test_samples::ALICE_ID.clone();
        let world = iroha_core::state::World::with(
            [],
            [Account::new(authority.clone()).build(&authority)],
            [],
        );
        let expected = ["qtrig_iterable", "qtrig_second"]
            .into_iter()
            .map(|name| {
                let value = if oversized {
                    Json::new("x".repeat(8 * 1024))
                } else {
                    Json::new(true)
                };
                Trigger::new(
                    name.parse().unwrap(),
                    Action::new(
                        vec![InstructionBox::from(SetKeyValue::account(
                            authority.clone(),
                            "iterable_flag".parse().unwrap(),
                            value,
                        ))],
                        Repeats::Exactly(1),
                        authority.clone(),
                        TimeEventFilter::new(ExecutionTime::PreCommit),
                    )
                    .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let app = tests_runtime_handlers::mk_app_state_for_tests_with_world(world);
        let mut block = app.state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        transaction
            .world_mut_for_testing()
            .add_account_permission(&authority, CanReadAllLedgerData.into());
        for trigger in &expected {
            Register::trigger(trigger.clone())
                .execute(&authority, &mut transaction)
                .expect("owner registers the exact stored trigger action");
        }
        transaction.apply();
        block.commit_world_overlay_for_testing().unwrap();
        // Registration stores the block's height and creation time on the action.
        // Include both exact engine-owned fields in the complete-row byte oracle.
        let expected = expected
            .into_iter()
            .map(|trigger| {
                let mut metadata = trigger.action().metadata().clone();
                metadata.insert(
                    "__registered_block_height".parse().unwrap(),
                    Json::from(1u64),
                );
                metadata.insert("__registered_at_ms".parse().unwrap(), Json::from(0u64));
                Trigger::new(
                    trigger.id().clone(),
                    trigger.action().clone().with_metadata(metadata),
                )
            })
            .collect();
        (app, authority, expected)
    }
    fn trigger_rows_query(fetch_size: u64) -> iroha_data_model::query::QueryRequest {
        use iroha_data_model::{
            prelude::Trigger,
            query::{
                ErasedIterQuery, QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
                dsl::{CompoundPredicate, SelectorTuple},
                trigger::FindTriggers,
            },
        };
        let query: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Trigger>::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            norito::codec::Encode::encode(&FindTriggers),
        ));
        let mut params = iroha_data_model::query::parameters::QueryParams::default();
        params.fetch_size.fetch_size = NonZeroU64::new(fetch_size);
        QueryRequest::Start(QueryWithParams::new(&query, params).unwrap())
    }
    fn trigger_rows_page(
        response: iroha_data_model::query::QueryResponse,
    ) -> (
        Vec<iroha_data_model::trigger::Trigger>,
        Option<iroha_data_model::query::parameters::ForwardCursor>,
    ) {
        use iroha_data_model::query::{QueryOutputBatchBox, QueryResponse};
        let QueryResponse::Iterable(output) = response else {
            panic!("expected trigger page")
        };
        let mut batch = output.batch.into_iter();
        let Some(QueryOutputBatchBox::Trigger(rows)) = batch.next() else {
            panic!("expected complete Trigger rows")
        };
        assert!(batch.next().is_none());
        (rows, output.continue_cursor)
    }
    #[tokio::test]
    async fn full_trigger_rows_preserve_action_bytes_and_authenticated_stored_continuation() {
        use iroha_data_model::query::{QueryRequest, error::QueryExecutionFail};
        let (app, authority, expected) = trigger_rows_fixture(false);
        let before = app.query_fanout_inflight.available_bytes();
        let profile = app
            .ordinary_query_policy
            .trigger_limits
            .expect("default trigger profile");
        assert_eq!(profile.max_source_item_bytes(), 4 * 1024);
        assert_eq!(profile.max_page_items(), 100);
        assert_eq!(
            app.ordinary_query_policy.limits.max_source_item_bytes(),
            1024
        );
        assert!(
            profile.execution_headroom_bytes() + profile.max_cursor_retained_bytes()
                <= u64::try_from(app.query_fanout_working_set_bytes).unwrap()
        );
        let options = || QueryOptions {
            cursor_mode: Some("stored".into()),
            ..Default::default()
        };
        let first = execute_admitted_signed_query_with_opts(
            &app,
            authorize_query_for_test(trigger_rows_query(1), authority.clone()),
            options(),
        )
        .await
        .expect("original SetKeyValue action fits the full Trigger profile");
        let (response, lease) = first.into_parts();
        assert!(lease.reserved_bytes() >= profile.execution_headroom_bytes());
        let (mut actual, cursor) = trigger_rows_page(response);
        assert_eq!(actual.len(), 1);
        let cursor = cursor.expect("second full trigger retained in cursor");
        drop(lease);
        assert!(app.query_fanout_inflight.available_bytes() < before);
        assert_eq!(
            app.query_service
                .ordinary_cursor_execution_limits(&cursor, &authority)
                .unwrap(),
            profile
        );
        let mut stale_position = cursor.clone();
        stale_position.cursor = NonZeroU64::new(cursor.cursor.get() + 1).unwrap();
        assert_eq!(
            app.query_service
                .ordinary_cursor_execution_limits(&stale_position, &authority),
            Err(QueryExecutionFail::Expired),
        );
        let other = iroha_test_samples::BOB_ID.clone();
        assert_eq!(
            app.query_service
                .ordinary_cursor_execution_limits(&cursor, &other),
            Err(QueryExecutionFail::Expired)
        );
        let foreign = execute_admitted_signed_query_with_opts(
            &app,
            authorize_query_for_test(QueryRequest::Continue(cursor.clone()), other),
            options(),
        )
        .await;
        assert!(matches!(
            foreign,
            Err(Error::Query(ValidationFail::QueryFailed(
                QueryExecutionFail::Expired
            )))
        ));
        // A current-policy mismatch cannot adopt a cursor merely because its generation or shape looks similar.
        let mut unavailable = app.ordinary_query_policy;
        unavailable.trigger_limits = None;
        assert!(
            routing::ordinary_query_execution_plan(
                &app.state,
                &app.query_service,
                &authority,
                &QueryRequest::Continue(cursor.clone()),
                &options(),
                unavailable,
            )
            .is_err()
        );
        let (geometry, pool) = default_geometry();
        assert!(
            routing::ordinary_query_execution_plan(
                &app.state,
                &app.query_service,
                &authority,
                &QueryRequest::Continue(cursor.clone()),
                &options(),
                default_policy(geometry, &pool),
            )
            .is_err()
        );
        let second = execute_admitted_signed_query_with_opts(
            &app,
            authorize_query_for_test(QueryRequest::Continue(cursor.clone()), authority.clone()),
            options(),
        )
        .await
        .expect("continuation selects the original fully reserved Trigger geometry");
        let (response, lease) = second.into_parts();
        let (rows, next) = trigger_rows_page(response);
        assert_eq!(rows.len(), 1);
        assert!(next.is_none());
        actual.extend(rows);
        assert_eq!(
            norito::encode_canonical(&actual).unwrap(),
            norito::encode_canonical(&expected).unwrap()
        );
        drop(lease);
        assert_eq!(app.query_fanout_inflight.available_bytes(), before);
        assert_eq!(
            app.query_service
                .ordinary_cursor_execution_limits(&cursor, &authority),
            Err(QueryExecutionFail::Expired)
        );
    }
    #[tokio::test]
    async fn full_trigger_rows_refuse_oversized_sources_pages_and_unavailable_reservations() {
        use iroha_data_model::query::error::QueryExecutionFail;
        let (app, authority, expected) = trigger_rows_fixture(true);
        let limits = app.ordinary_query_policy.trigger_limits.unwrap();
        assert!(
            norito::encode_canonical(&expected[0]).unwrap().len()
                > usize::try_from(limits.max_source_item_bytes()).unwrap()
        );
        let before = app.query_fanout_inflight.available_bytes();
        let result = execute_admitted_signed_query_with_opts(
            &app,
            authorize_query_for_test(trigger_rows_query(1), authority.clone()),
            QueryOptions {
                cursor_mode: Some("ephemeral".into()),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(Error::Query(ValidationFail::QueryFailed(
                QueryExecutionFail::CapacityLimit
            )))
        ));
        assert_eq!(app.query_fanout_inflight.available_bytes(), before);
        // A valid small source makes this fail if the server silently clamps an oversized page.
        let (small_app, small_authority, _) = trigger_rows_fixture(false);
        let small_before = small_app.query_fanout_inflight.available_bytes();
        let result = execute_admitted_signed_query_with_opts(
            &small_app,
            authorize_query_for_test(
                trigger_rows_query(limits.max_page_items() + 1),
                small_authority,
            ),
            QueryOptions {
                cursor_mode: Some("ephemeral".into()),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(Error::Query(ValidationFail::QueryFailed(
                QueryExecutionFail::CapacityLimit
            )))
        ));
        assert_eq!(
            small_app.query_fanout_inflight.available_bytes(),
            small_before
        );
        let mut unavailable = app.ordinary_query_policy;
        unavailable.trigger_limits = None;
        assert!(
            routing::ordinary_query_execution_plan(
                &app.state,
                &app.query_service,
                &authority,
                &trigger_rows_query(1),
                &QueryOptions::default(),
                unavailable,
            )
            .is_err()
        );
        let held = app
            .query_fanout_inflight
            .try_acquire_parts([before])
            .unwrap();
        assert!(try_acquire_ordinary_query_memory(&app, limits, true, false).is_err());
        drop(held);
        assert_eq!(app.query_fanout_inflight.available_bytes(), before);
    }
}
