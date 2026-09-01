#[test]
fn conn_scheme_detects_norito_rpc() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .header(
            axum::http::header::CONTENT_TYPE,
            crate::utils::NORITO_MIME_TYPE,
        )
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::NoritoRpc
    ));
}
#[test]
fn conn_scheme_marks_transaction_path_as_norito_rpc() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri(iroha_torii_shared::uri::TRANSACTION)
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::NoritoRpc
    ));
}
#[test]
fn conn_scheme_labels_use_norito_rpc_name() {
    assert_eq!(ConnScheme::NoritoRpc.label(), "norito_rpc");
}
#[test]
fn conn_scheme_defaults_to_http_for_json() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::Http
    ));
}
#[test]
fn conn_scheme_flags_every_catalogued_websocket_upgrade() {
    let websocket_routes = [
        route_catalog::streaming::SUBSCRIPTION_WS,
        route_catalog::streaming::BLOCKS_WS,
        route_catalog::connect::WEBSOCKET,
        route_catalog::sorafs::REPUTATION_EVENTS_WEBSOCKET,
        route_catalog::application_api::SORAFS_ORDERBOOK_EVENTS_WS_GET,
        route_catalog::application_api::SORAFS_RESERVE_EVENTS_WS_GET,
    ];
    for descriptor in websocket_routes {
        let mut request = complete_websocket_upgrade_request();
        request
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(descriptor));
        assert_eq!(
            ConnScheme::from_request(&request),
            ConnScheme::Ws,
            "{} must be classified from catalog transport metadata",
            descriptor.stable_route_id()
        );
    }
}

fn complete_websocket_upgrade_request() -> axum::http::Request<()> {
    axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::CONNECTION, "keep-alive, Upgrade")
        .header(axum::http::header::UPGRADE, "websocket")
        .header(axum::http::header::SEC_WEBSOCKET_VERSION, "13")
        .header(
            axum::http::header::SEC_WEBSOCKET_KEY,
            "dGhlIHNhbXBsZSBub25jZQ==",
        )
        .body(())
        .unwrap()
}

#[test]
fn conn_scheme_rejects_incomplete_or_uncatalogued_websocket_upgrades() {
    let mut incomplete = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::UPGRADE, "websocket")
        .body(())
        .unwrap();
    incomplete
        .extensions_mut()
        .insert(MatchedRouteMetadata::from_descriptor(
            route_catalog::streaming::SUBSCRIPTION_WS,
        ));
    assert!(matches!(
        ConnScheme::from_request(&incomplete),
        ConnScheme::Http
    ));

    let uncatalogued = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::CONNECTION, "Upgrade")
        .header(axum::http::header::UPGRADE, "websocket")
        .header(axum::http::header::SEC_WEBSOCKET_VERSION, "13")
        .header(
            axum::http::header::SEC_WEBSOCKET_KEY,
            "dGhlIHNhbXBsZSBub25jZQ==",
        )
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&uncatalogued),
        ConnScheme::Http
    ));

    for descriptor in [
        route_catalog::streaming::EVENTS_SSE,
        route_catalog::core::HEALTH,
    ] {
        let mut wrong_transport = complete_websocket_upgrade_request();
        wrong_transport
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(descriptor));
        assert_eq!(
            ConnScheme::from_request(&wrong_transport),
            ConnScheme::Http,
            "{} must not be classified as a WebSocket",
            descriptor.stable_route_id()
        );
    }
}

#[test]
fn conn_scheme_rejects_noncanonical_norito_content_types() {
    for content_type in [
        "application/x-norito-evil",
        "text/plain; note=application/x-norito",
        "application/x-norito; charset=utf-8",
    ] {
        let request = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .header(axum::http::header::CONTENT_TYPE, content_type)
            .body(())
            .unwrap();
        assert!(matches!(
            ConnScheme::from_request(&request),
            ConnScheme::Http
        ));
    }

    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .header(
            axum::http::header::CONTENT_TYPE,
            crate::utils::NORITO_MIME_TYPE,
        )
        .header(
            axum::http::header::CONTENT_TYPE,
            crate::utils::NORITO_MIME_TYPE,
        )
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::Http
    ));
}
