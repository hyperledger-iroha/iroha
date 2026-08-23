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
fn conn_scheme_flags_websocket_upgrade() {
    let mut request = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::CONNECTION, "keep-alive, Upgrade")
        .header(axum::http::header::UPGRADE, "websocket")
        .header(axum::http::header::SEC_WEBSOCKET_VERSION, "13")
        .header(
            axum::http::header::SEC_WEBSOCKET_KEY,
            "dGhlIHNhbXBsZSBub25jZQ==",
        )
        .body(())
        .unwrap();
    request
        .extensions_mut()
        .insert(MatchedRouteMetadata::from_descriptor(
            route_catalog::streaming::SUBSCRIPTION_WS,
        ));
    assert!(matches!(ConnScheme::from_request(&request), ConnScheme::Ws));
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
