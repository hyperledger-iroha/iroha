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
    let request = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::UPGRADE, "websocket")
        .body(())
        .unwrap();
    assert!(matches!(ConnScheme::from_request(&request), ConnScheme::Ws));
}
