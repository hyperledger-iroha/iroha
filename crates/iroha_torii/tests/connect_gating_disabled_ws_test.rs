#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_handshake_fails_when_disabled() {
    use tokio::net::TcpListener;
    // Build disabled config and Torii router
    let cfg = minimal_actual_config(false);
    let torii = build_torii(&cfg);
    let app = torii
        .api_router_for_tests()
        .expect("test Torii router initializes");
    // Serve
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_handshake_fails_when_disabled: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);
    // Attempt WS connect directly; expect failure
    let url = format!(
        "ws://{}/v1/connect/ws?sid={}&role=app",
        addr, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
    let err = tokio_tungstenite::connect_async(&url)
        .await
        .expect_err("ws handshake should fail when connect disabled");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(response) => response.status(),
        other => panic!("unexpected WebSocket error: {other:?}"),
    };
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
