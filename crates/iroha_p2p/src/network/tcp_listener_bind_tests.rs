//! TCP listener binding and managed ownership transfer regressions.

use super::{bind_reusable_tcp_listener, prepare_tcp_listener};
use std::net::{Ipv4Addr, SocketAddr};
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

#[tokio::test(flavor = "current_thread")]
async fn prebound_tcp_listener_preserves_exact_bound_endpoint() {
    let original = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = original.local_addr().unwrap();
    let client = tokio::net::TcpStream::connect(address).await.unwrap();
    let listener = prepare_tcp_listener(address, Some(original)).await.unwrap();
    assert_eq!(listener.local_addr().unwrap(), address);
    assert_eq!(
        std::net::TcpListener::bind(address).unwrap_err().kind(),
        std::io::ErrorKind::AddrInUse,
    );
    let (_, remote) = tokio::time::timeout(std::time::Duration::from_secs(1), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(remote, client.local_addr().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn prebound_tcp_listener_rejects_endpoint_substitution() {
    let original = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let other = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let error = prepare_tcp_listener(other.local_addr().unwrap(), Some(original))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
