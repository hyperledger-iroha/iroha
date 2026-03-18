#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii-level E2E test for the P2P WebSocket fallback route `/p2p`.
//! Requires building with `--features p2p_ws`.

#[cfg(feature = "p2p_ws")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p2p_ws_route_accepts_and_handshakes() {
    use std::{net::SocketAddr, time::Duration};

    use axum::{
        Router,
        extract::{ConnectInfo, ws::WebSocketUpgrade},
        response::IntoResponse,
        routing::get,
    };
    use tokio::net::TcpListener;

    async fn route(
        ws: WebSocketUpgrade,
        ConnectInfo(remote): ConnectInfo<SocketAddr>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| async move {
            // We only assert that the route accepts a WS handshake and upgrades successfully.
            // The P2P node wires a real `IrohaNetwork` handle.
            iroha_torii::handle_p2p_ws(socket, None, remote).await;
        })
    }

    let app = Router::new().route("/p2p", get(route));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    let server_task = tokio::spawn(async move {
        let _ = server.await;
    });

    let endpoint = format!("{}:{}", addr.ip(), addr.port());
    let _duplex = tokio::time::timeout(
        Duration::from_secs(5),
        iroha_p2p::transport::ws::connect_ws(&endpoint),
    )
    .await
    .expect("ws connect timeout")
    .expect("ws connect");

    server_task.abort();
}
