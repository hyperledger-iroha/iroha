// Role collection transport authentication, grant checks and complete responses.

#[cfg(feature = "app_api")]
#[tokio::test]
async fn role_collections_require_signed_global_reader_and_preserve_assignments() {
    use axum::{Extension, Router, routing::post};
    use iroha_data_model::role::Role;
    use tower::ServiceExt as _;

    let key = checked_torii_test_ed25519_keypair(0xDB, "role collection reader");
    let account = AccountId::new(key.public_key().clone());
    let role = Role::new("auditor".parse().unwrap(), account.clone())
        .add_permission(CanReadAllLedgerData)
        .build(&account);
    // Use an empty role for the assignment so this binding does not itself grant
    // the global read permission exercised below.
    let empty_role = Role::new("member".parse().unwrap(), account.clone()).build(&account);
    let mut world = World::with_assets_and_roles(
        [],
        [Account::new(account.clone()).build(&account)],
        [],
        [],
        [],
        [role.clone(), empty_role.clone()],
    );
    world.grant_role_for_tests(account.clone(), empty_role.id().clone());
    let app = mk_app_state_for_tests_with_world(world);
    let router = Router::new()
        .route("/v1/roles/query", post(crate::role_collections::roles))
        .route(
            "/v1/accounts/{account}/roles/query",
            post(crate::role_collections::account_roles),
        )
        .with_state(app.clone())
        .layer(Extension(crate::loopback_connect_info()))
        .layer(axum::middleware::from_fn_with_state(
            CanonicalAccountBodyAuthState {
                app: app.clone(),
                max_body_bytes: 1024,
                missing_auth_code: "canonical_authentication_required",
                missing_auth_message: "canonical account request authentication is required",
            },
            enforce_canonical_account_body_authentication,
        ));
    let paths = [
        "/v1/roles/query".to_owned(),
        format!("/v1/accounts/{account}/roles/query"),
    ];
    for path in &paths {
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let uri: Uri = path.parse().unwrap();
        let mut request = axum::http::Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .body(Body::from("{}"))
            .unwrap();
        *request.headers_mut() = signed_app_headers(&account, &key, &Method::POST, &uri, b"{}");
        request
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    grant_account_permission_for_test(&app, &account, CanReadAllLedgerData.into());
    for (index, path) in paths.iter().enumerate() {
        let uri: Uri = path.parse().unwrap();
        let mut request = axum::http::Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .body(Body::from("{}"))
            .unwrap();
        *request.headers_mut() = signed_app_headers(&account, &key, &Method::POST, &uri, b"{}");
        request
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let body: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        let items = body["items"].as_array().unwrap();
        if index == 0 {
            assert_eq!(items.len(), 2);
            let decoded: Role = norito::json::from_value(items[0].clone()).unwrap();
            assert_eq!(decoded, role);
            assert_eq!(
                decoded.permissions().collect::<Vec<_>>(),
                role.permissions().collect::<Vec<_>>()
            );
        } else {
            assert_eq!(items, &[norito::json!({"id": empty_role.id()})]);
        }
    }
}

#[cfg(feature = "app_api")]
#[test]
fn role_collection_catalog_declares_mandatory_read_authentication() {
    use iroha_torii_shared::route_catalog::{AuthenticationPolicy, RouteEffect, application_api};
    for route in [
        application_api::ROLES_QUERY_POST,
        application_api::ACCOUNTS_BY_ACCOUNT_ID_ROLES_QUERY_POST,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
    }
}
