use std::{
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    sync::{
        OnceLock,
        atomic::{AtomicU16, Ordering},
    },
};

fn tcp_bind_permitted() -> bool {
    static PERMITTED: OnceLock<bool> = OnceLock::new();
    *PERMITTED.get_or_init(
        || match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) {
            Ok(listener) => {
                drop(listener);
                true
            }
            Err(err) => err.kind() != ErrorKind::PermissionDenied,
        },
    )
}

/// Return `true` if tests that require binding local TCP sockets should be skipped.
///
/// Some sandbox environments prohibit `bind(2)` entirely. Skipping in that case keeps
/// `cargo test` useful, while CI and real developer environments still run the suite.
fn skip_if_no_tcp_bind() -> bool {
    if tcp_bind_permitted() {
        false
    } else {
        eprintln!("skipping integration tests: TCP bind is not permitted in this environment");
        true
    }
}

/// Allocate a local TCP port for tests to avoid clashes when they run in parallel.
fn next_port() -> u16 {
    static NEXT_PORT: AtomicU16 = AtomicU16::new(12_000);

    let mut attempts = 0u32;
    let mut last_err = None;
    loop {
        let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match TcpListener::bind(addr) {
            Ok(listener) => {
                drop(listener);
                // Release probe socket; the test will bind immediately after.
                return port;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                // Try the next candidate.
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                panic!("TCP bind is not permitted in this environment: {err}");
            }
            Err(err) => {
                last_err = Some(err);
            }
        }
        attempts = attempts.wrapping_add(1);
        assert!(
            u16::try_from(attempts).is_ok(),
            "exhausted test ports starting at 12000; last bind error: {last_err:?}"
        );
    }
}

mod p2p;
mod p2p_caps;
mod p2p_puzzle;
mod p2p_trust_gossip;
