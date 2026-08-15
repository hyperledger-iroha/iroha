use super::{SocketAdmission, WriteTimeoutIo, bind_torii_tcp_listener, serve_torii_http};
use axum::{Router, routing::get};
use iroha_config::parameters::actual::ToriiHttpTransport;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_primitives::addr::SocketAddr;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr as StdSocketAddr, SocketAddrV4},
    num::NonZeroUsize,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
#[tokio::test(flavor = "current_thread")]
async fn torii_reusable_tcp_listener_binds_loopback() {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
    let listener = bind_torii_tcp_listener(SocketAddr::Ipv4(addr.into()))
        .await
        .expect("Torii reusable listener should bind");
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
async fn torii_reusable_tcp_listener_rejects_active_listener() {
    let active =
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("active listener should bind");
    let addr = match active.local_addr().expect("active listener has local addr") {
        StdSocketAddr::V4(addr) => addr,
        StdSocketAddr::V6(_) => unreachable!("test binds IPv4 loopback"),
    };
    let err = bind_torii_tcp_listener(SocketAddr::Ipv4(addr.into()))
        .await
        .expect_err("active listener must not be shadowed by reusable bind");
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
}
#[test]
fn socket_admission_enforces_global_and_canonical_source_limits() {
    let admission = SocketAdmission::new(
        NonZeroUsize::new(2).expect("non-zero global limit"),
        NonZeroUsize::new(1).expect("non-zero per-IP limit"),
    );
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mapped_loopback = IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped());
    let other = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    let loopback_permit = admission
        .try_acquire(loopback)
        .expect("first source socket is admitted");
    assert!(
        admission.try_acquire(mapped_loopback).is_none(),
        "IPv4-mapped IPv6 must share the IPv4 source limit"
    );
    let other_permit = admission
        .try_acquire(other)
        .expect("second source socket is admitted");
    assert!(
        admission
            .try_acquire(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)))
            .is_none(),
        "global socket limit must apply across source IPs"
    );
    drop(loopback_permit);
    let replacement = admission
        .try_acquire(mapped_loopback)
        .expect("dropping a permit immediately releases both counters");
    drop(replacement);
    drop(other_permit);
    let state = admission.state.lock();
    assert_eq!(state.active, 0);
    assert!(state.active_by_ip.is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn stalled_socket_write_hits_progress_deadline() {
    let (writer, _unread_peer) = tokio::io::duplex(1);
    let mut writer = WriteTimeoutIo::new(writer, Duration::from_millis(20));
    let error = writer
        .write_all(&[0xA5, 0x5A])
        .await
        .expect_err("an unread full socket must hit the write-progress deadline");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}
#[tokio::test(flavor = "current_thread")]
async fn partial_http_head_is_closed_at_listener_deadline() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("test listener address");
    let mut config = ToriiHttpTransport::default();
    config.header_read_timeout = Duration::from_millis(25);
    config.write_timeout = Duration::from_millis(100);
    let shutdown = ShutdownSignal::new();
    let server_shutdown = shutdown.clone();
    let server = tokio::spawn(async move {
        serve_torii_http(
            listener,
            Router::new().route("/", get(|| async { "ok" })),
            config,
            server_shutdown,
        )
        .await
    });
    let mut client = TcpStream::connect(address)
        .await
        .expect("test client should connect");
    client
        .write_all(b"GET / HTTP/1.1\r\nHost:")
        .await
        .expect("partial request head should reach Torii");
    let mut response = Vec::new();
    let read_result =
        tokio::time::timeout(Duration::from_secs(1), client.read_to_end(&mut response))
            .await
            .expect("partial request head must not retain a socket past its deadline");
    if let Err(error) = read_result {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            ),
            "unexpected read failure after header timeout: {error}"
        );
    }
    shutdown.send();
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .expect("Torii test server should stop")
        .expect("Torii test task should not panic")
        .expect("Torii test server should exit cleanly");
}
