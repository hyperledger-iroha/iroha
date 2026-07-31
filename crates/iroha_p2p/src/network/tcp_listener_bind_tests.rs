use std::net::{Ipv4Addr, SocketAddr};

use super::bind_reusable_tcp_listener;

#[tokio::test(flavor = "current_thread")]
async fn reusable_tcp_listener_binds_loopback() {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = bind_reusable_tcp_listener(&[addr]).expect("reusable listener should bind");

    assert_ne!(
        listener
            .local_addr()
            .expect("listener has local addr")
            .port(),
        0,
        "OS should assign an ephemeral port"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reusable_tcp_listener_rejects_active_listener() {
    let active =
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("active listener should bind");
    let addr = active.local_addr().expect("active listener has local addr");

    let err = bind_reusable_tcp_listener(&[addr])
        .expect_err("active listener must not be shadowed by reusable bind");

    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
}
