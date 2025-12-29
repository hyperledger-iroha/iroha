use std::{
    net::{SocketAddr, TcpListener},
    sync::atomic::{AtomicU16, Ordering},
};

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
