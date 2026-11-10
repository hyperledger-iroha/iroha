// A long effective-permission list through the real handlers and the owner ladder.
mod account_permission_page_tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header},
        middleware::{Next, from_fn, from_fn_with_state},
        response::Response,
    };
    use iroha_core::state::World;
    use iroha_data_model::{
        account::Account,
        permission::{Permission, Permissions},
        role::{Role, RoleId},
    };
    use iroha_primitives::json::Json;
    use iroha_torii_shared::{
        list_query::{FilterExpr, ListQuery, SortKey, field},
        route_catalog::RouteDescriptor,
    };

    /// More read grants than the 1,695 permissions the registry subject held when
    /// its listing stopped paging, and more than the smallest owner can decode.
    const READ_GRANTS: usize = 2_400;
    const READ_GRANT: &str = "CanReadAccountData";

    fn read_grant(index: usize) -> Permission {
        Permission::new(
            READ_GRANT.to_owned(),
            Json::new(norito::json!({ "account": (format!("sorau{index:060}")) })),
        )
    }

    fn named_grant(name: &str, index: usize) -> Permission {
        Permission::new(
            name.to_owned(),
            Json::new(norito::json!({ "target": (format!("target{index:03}")) })),
        )
    }

    /// The registry subject's shape: a long block of read grants between other names,
    /// and a role repeating one direct grant next to a grant only the role carries.
    fn world_holding(authority: &AccountId, direct: impl IntoIterator<Item = Permission>) -> World {
        let role_id: RoleId = "registry_readers".parse().unwrap();
        let role = Role::new(role_id.clone(), authority.clone())
            .add_permission(read_grant(0))
            .add_permission(Permission::new(
                "CanSetParameters".to_owned(),
                Json::new(()),
            ))
            .build(authority);
        let mut world = World::with_assets_and_roles(
            [],
            [Account::new(authority.clone()).build(authority)],
            [],
            [],
            [],
            [role],
        );
        world.grant_role_for_tests(authority.clone(), role_id);
        world.account_permissions_mut_for_testing().insert(
            authority.clone(),
            direct.into_iter().collect::<Permissions>(),
        );
        world
    }

    fn registry_like_grants() -> Vec<Permission> {
        (0..3)
            .map(|index| named_grant("CanBurnAssetWithDefinition", index))
            .chain((0..READ_GRANTS).map(read_grant))
            .chain((0..24).map(|index| named_grant("CanUseContractPermission", index)))
            .collect()
    }

    /// `(name, canonical payload)` of every effective row in the default order.
    fn effective_rows() -> Vec<(String, String)> {
        let mut rows: Vec<_> = registry_like_grants()
            .into_iter()
            .chain([Permission::new(
                "CanSetParameters".to_owned(),
                Json::new(()),
            )])
            .map(|permission| {
                (
                    permission.name().to_owned(),
                    permission.payload().get().clone(),
                )
            })
            .collect();
        rows.sort();
        rows
    }

    fn row_identity(row: &Value) -> (String, String) {
        (
            row["name"].as_str().expect("row name").to_owned(),
            norito::json::to_json(&row["payload"]).expect("row payload"),
        )
    }

    async fn insert_route(
        route: RouteDescriptor,
        mut request: Request<Body>,
        next: Next,
    ) -> Response {
        request
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(route));
        next.run(request).await
    }

    fn router_for(
        app: &SharedAppState,
        route: RouteDescriptor,
        handler: axum::routing::MethodRouter<SharedAppState>,
    ) -> Router {
        Router::new()
            .route(route.path(), handler)
            .with_state(app.clone())
            .layer(axum::Extension(crate::loopback_connect_info()))
            .layer(axum::Extension(ToriiAccountReadVisibility::None))
            .layer(from_fn_with_state(
                app.clone(),
                enforce_app_routed_read_http_admission,
            ))
            .layer(from_fn(move |request, next| {
                insert_route(route, request, next)
            }))
    }

    fn account_segment(account: &AccountId) -> String {
        url::form_urlencoded::byte_serialize(account.to_string().as_bytes()).collect()
    }

    struct Served {
        /// Pool credit held by the owner that answered; its response body retains it.
        owner_bytes: u64,
        page: Value,
    }

    async fn serve(app: &SharedAppState, router: Router, request: Request<Body>) -> Served {
        let before = app.query_fanout_inflight.available_bytes();
        let response = router.oneshot(request).await.unwrap();
        let owner_bytes = before - app.query_fanout_inflight.available_bytes();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        Served {
            owner_bytes,
            page: norito::json::from_slice(&body).expect("page JSON"),
        }
    }

    async fn get_page(app: &SharedAppState, account: &AccountId, query: &ListQuery) -> Served {
        let query_string = {
            let mut pairs = url::form_urlencoded::Serializer::new(String::new());
            for (key, value) in query.to_query_pairs().expect("query has a GET form") {
                pairs.append_pair(key, &value);
            }
            pairs.finish()
        };
        let route = route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_PERMISSIONS_GET;
        let request = Request::builder()
            .uri(format!(
                "/v1/accounts/{}/permissions?{}",
                account_segment(account),
                query_string
            ))
            .body(Body::empty())
            .unwrap();
        let router = router_for(app, route, axum::routing::get(handler_account_permissions));
        serve(app, router, request).await
    }

    async fn post_page(app: &SharedAppState, account: &AccountId, query: &ListQuery) -> Served {
        let body = norito::json::to_json(&query.to_json_value()).unwrap();
        let route = route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_PERMISSIONS_QUERY_POST;
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/v1/accounts/{}/permissions/query",
                account_segment(account)
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Body::from(body))
            .unwrap();
        let router = router_for(
            app,
            route,
            axum::routing::post(handler_account_permissions_query),
        );
        serve(app, router, request).await
    }

    fn smallest_owner_bytes(app: &SharedAppState) -> u64 {
        let smallest = routed_read_owner_working_sets(app).next().unwrap();
        routed_read_owner_weight(app, smallest).unwrap().0 as u64
    }

    fn fixture() -> (SharedAppState, AccountId) {
        let authority = routed_read_test_account(0xa7);
        let world = world_holding(&authority, registry_like_grants());
        (native_listener(world), authority)
    }

    /// A listener over a signed genesis, so it holds an authenticated root scope.
    fn native_listener(world: World) -> SharedAppState {
        crate::tests_runtime_handlers::native_ingress_app_with_world_for_test(world)
    }

    /// Follow `next_cursor` to the end; every page must come from the smallest owner.
    async fn walk_with_smallest_owner(
        app: &SharedAppState,
        account: &AccountId,
        query: ListQuery,
    ) -> Vec<(String, String)> {
        let smallest = smallest_owner_bytes(app);
        let mut rows = Vec::new();
        let mut current = query.clone();
        loop {
            let served = get_page(app, account, &current).await;
            assert_eq!(
                served.owner_bytes,
                smallest,
                "the page after {} rows outgrew the smallest owner",
                rows.len()
            );
            let items = served.page["items"].as_array().expect("items");
            rows.extend(items.iter().map(row_identity));
            match served.page["next_cursor"].as_str() {
                Some(cursor) => current = query.clone().cursor(cursor),
                None => return rows,
            }
        }
    }

    #[tokio::test]
    async fn every_page_of_a_long_list_fits_the_smallest_owner() {
        let (app, authority) = fixture();
        let rows = walk_with_smallest_owner(&app, &authority, ListQuery::new().limit(100)).await;
        assert_eq!(rows, effective_rows());
    }

    #[tokio::test]
    async fn descending_pages_keep_the_semantic_payload_order() {
        let (app, authority) = fixture();
        let rows = walk_with_smallest_owner(
            &app,
            &authority,
            ListQuery::new().sort_by(SortKey::desc("name")).limit(100),
        )
        .await;
        let mut expected = effective_rows();
        expected.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        assert_eq!(rows, expected);
    }

    #[tokio::test]
    async fn filters_page_the_block_they_select_and_skip_the_rest() {
        let (app, authority) = fixture();
        let selected: Vec<_> = effective_rows()
            .into_iter()
            .filter(|(name, _)| name == READ_GRANT)
            .collect();
        assert_eq!(selected.len(), READ_GRANTS);
        let rows = walk_with_smallest_owner(
            &app,
            &authority,
            ListQuery::new()
                .filter(field("name").eq(READ_GRANT))
                .limit(100),
        )
        .await;
        assert_eq!(rows, selected);

        let others: Vec<_> = effective_rows()
            .into_iter()
            .filter(|(name, _)| name != READ_GRANT)
            .collect();
        let rows = walk_with_smallest_owner(
            &app,
            &authority,
            ListQuery::new()
                .filter(FilterExpr::parse(&format!("name != \"{READ_GRANT}\"")).unwrap())
                .limit(10),
        )
        .await;
        assert_eq!(rows, others);
    }

    #[tokio::test]
    async fn payload_predicates_decide_object_payloads_without_decoding_them() {
        let (app, authority) = fixture();
        // The role's unit grant is the only row with a null payload.
        let objects: Vec<_> = effective_rows()
            .into_iter()
            .filter(|(_, payload)| payload != "null")
            .collect();
        let rows = walk_with_smallest_owner(
            &app,
            &authority,
            ListQuery::new()
                .filter(FilterExpr::parse("payload is not null").unwrap())
                .limit(100),
        )
        .await;
        assert_eq!(rows, objects);
        let served = get_page(
            &app,
            &authority,
            &ListQuery::new()
                .filter(FilterExpr::parse("payload = \"no scalar payload\"").unwrap())
                .limit(100)
                .include_total(),
        )
        .await;
        assert_eq!(served.owner_bytes, smallest_owner_bytes(&app));
        assert_eq!(served.page["total"].as_u64(), Some(0));
    }

    #[tokio::test]
    async fn totals_count_the_whole_list_on_every_page() {
        let (app, authority) = fixture();
        let query = ListQuery::new()
            .filter(field("name").eq(READ_GRANT))
            .limit(100)
            .include_total();
        let first = get_page(&app, &authority, &query).await;
        assert_eq!(first.owner_bytes, smallest_owner_bytes(&app));
        assert_eq!(first.page["total"].as_u64(), Some(READ_GRANTS as u64));
        let cursor = first.page["next_cursor"].as_str().expect("next page");
        let second = get_page(&app, &authority, &query.clone().cursor(cursor)).await;
        assert_eq!(second.owner_bytes, smallest_owner_bytes(&app));
        assert_eq!(second.page["total"].as_u64(), Some(READ_GRANTS as u64));
    }

    #[tokio::test]
    async fn post_query_pages_like_get() {
        let (app, authority) = fixture();
        let smallest = smallest_owner_bytes(&app);
        let query = ListQuery::new()
            .filter(field("name").eq(READ_GRANT))
            .limit(100);
        let mut rows = Vec::new();
        let mut current = query.clone();
        loop {
            let served = post_page(&app, &authority, &current).await;
            assert_eq!(served.owner_bytes, smallest);
            let items = served.page["items"].as_array().expect("items");
            rows.extend(items.iter().map(row_identity));
            match served.page["next_cursor"].as_str() {
                Some(cursor) => current = query.clone().cursor(cursor),
                None => break,
            }
        }
        assert_eq!(rows.len(), READ_GRANTS);
        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[tokio::test]
    async fn a_page_too_large_for_one_owner_moves_to_the_next() {
        let (app, authority) = fixture();
        let query = ListQuery::new().limit(500);
        let first = get_page(&app, &authority, &query).await;
        assert!(first.owner_bytes > smallest_owner_bytes(&app));
        assert_eq!(first.page["items"].as_array().unwrap().len(), 500);
        let cursor = first.page["next_cursor"].as_str().expect("next page");
        let second = get_page(&app, &authority, &query.clone().cursor(cursor)).await;
        assert_eq!(second.page["items"].as_array().unwrap().len(), 500);
    }

    /// Array payloads must be decoded for a scalar `payload` predicate, because a
    /// scalar literal matches any element. Running out of budget on them is an
    /// owner-limit refusal the ladder answers with a larger owner, not a client error.
    #[tokio::test]
    async fn a_decode_refused_by_one_owner_moves_to_the_next() {
        let authority = routed_read_test_account(0xa8);
        let padding = "x".repeat(1_024);
        let lists = (0..2_000).map(|index| {
            Permission::new(
                "CanHoldLists".to_owned(),
                Json::new(norito::json!([(format!("{padding}{index:04}"))])),
            )
        });
        let app = native_listener(world_holding(&authority, lists));
        let wanted = format!("{padding}1999");
        let served = get_page(
            &app,
            &authority,
            &ListQuery::new()
                .filter(field("payload").eq(wanted.as_str()))
                .limit(10),
        )
        .await;
        assert!(served.owner_bytes > smallest_owner_bytes(&app));
        let items = served.page["items"].as_array().expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["payload"][0].as_str(), Some(wanted.as_str()));
    }
}
