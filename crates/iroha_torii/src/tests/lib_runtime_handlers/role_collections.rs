// Role collection transport authentication, grant checks and complete responses.

#[cfg(feature = "app_api")]
#[tokio::test]
async fn role_collections_require_signed_authorized_reader_and_preserve_assignments() {
    use axum::{Extension, middleware::from_fn_with_state};
    use iroha_torii_shared::route_catalog::{RouteCatalog, application_api};
    use iroha_data_model::{Identifiable as _, role::Role};
    use tower::ServiceExt as _;

    let key = checked_torii_test_ed25519_keypair(0xDB, "role collection reader");
    let account = AccountId::new(key.public_key().clone());
    let target = AccountId::new(
        checked_torii_test_ed25519_keypair(0xDC, "role assignment target")
            .public_key().clone(),
    );
    let role = Role::new("auditor".parse().unwrap(), account.clone())
        .add_permission(CanReadAllLedgerData)
        .build(&account);
    // Use an empty role for the assignment so this binding does not itself grant
    // the global read permission exercised below.
    let empty_role = Role::new("member".parse().unwrap(), account.clone()).build(&account);
    let mut world = World::with_assets_and_roles(
        [],
        [
            Account::new(account.clone()).build(&account),
            Account::new(target.clone()).build(&account),
        ],
        [],
        [],
        [],
        [role.clone(), empty_role.clone()],
    );
    // A different target preserves the pre-grant denial under the current
    // self/exact-account authorization model.
    world.grant_role_for_tests(target.clone(), empty_role.id().clone());
    let app = mk_native_app_state_for_tests_with_world(world);
    let mut builder = RouterBuilder::new(
        app.clone(),
        RouteCatalog::new(&[
            application_api::ROLES_GET,
            application_api::ROLES_QUERY_POST,
            application_api::ACCOUNTS_BY_ACCOUNT_ID_ROLES_GET,
            application_api::ACCOUNTS_BY_ACCOUNT_ID_ROLES_QUERY_POST,
        ]),
        compiled_route_features(),
    ).expect("unique current role descriptors");
    crate::role_collection_routes::mount(&mut builder, app.clone(), 1024);
    let (router, manifest) = builder.finish().expect("actual role handlers match their catalog");
    let router = router
        .with_state(app.clone())
        .layer(from_fn_with_state(
            app.clone(),
            enforce_app_routed_read_http_admission,
        ))
        .layer(from_fn_with_state(
            manifest.route_index(),
            attach_matched_route_metadata,
        ))
        .layer(Extension(crate::loopback_connect_info()));
    let paths = [
        "/v1/roles/query".to_owned(),
        format!("/v1/accounts/{target}/roles/query"),
    ];
    for path in &paths {
        for body in ["{}", "{"] {
            let response = router.clone().oneshot(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body)).unwrap(),
            ).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let uri: Uri = path.parse().unwrap();
        let mut malformed = axum::http::Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .body(Body::from("{")).unwrap();
        *malformed.headers_mut() = signed_network_app_headers(
            app.state.network_id_ref(), &account, &key, &Method::POST, &uri, b"{",
        );
        malformed.headers_mut().insert("content-type", HeaderValue::from_static("application/json"));
        let malformed = router.clone().oneshot(malformed).await.unwrap();
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
        let mut request = axum::http::Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .body(Body::from("{}"))
            .unwrap();
        *request.headers_mut() = signed_network_app_headers(
            app.state.network_id_ref(),
            &account,
            &key,
            &Method::POST,
            &uri,
            b"{}",
        );
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
        *request.headers_mut() = signed_network_app_headers(
            app.state.network_id_ref(),
            &account,
            &key,
            &Method::POST,
            &uri,
            b"{}",
        );
        request
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        let response = router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&bytes)
        );
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
            assert_eq!(items, &[norito::json!({"id": (empty_role.id())})]);
        }
    }
}

#[cfg(feature = "app_api")]
#[test]
fn role_collection_catalog_declares_canonical_read_authentication() {
    use iroha_torii_shared::route_catalog::{AuthenticationPolicy, RouteEffect, application_api};
    for route in [
        application_api::ROLES_GET,
        application_api::ROLES_QUERY_POST,
        application_api::ACCOUNTS_BY_ACCOUNT_ID_ROLES_GET,
        application_api::ACCOUNTS_BY_ACCOUNT_ID_ROLES_QUERY_POST,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
    }
}
