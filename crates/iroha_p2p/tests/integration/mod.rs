use std::{
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    sync::{
        OnceLock,
        atomic::{AtomicU16, Ordering},
    },
};

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{NetworkId, block::BlockHeader};
use iroha_p2p::P2pIdentityKeys;

fn test_network_id(seed: &str) -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
        iroha_crypto::Hash::new(seed.as_bytes()),
    ))
}

/// Generate the only supported first-release node identity for P2P tests.
fn random_node_key_pair() -> KeyPair {
    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
}

/// Assign an independently generated Ed25519 identity to the SoraNet transport role.
fn p2p_identity_keys(node: KeyPair) -> P2pIdentityKeys {
    let soranet_transport = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    P2pIdentityKeys::new(node, soranet_transport)
        .expect("BLS-normal node and Ed25519 SoraNet identities must be valid")
}

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
    static NEXT_PORT: OnceLock<AtomicU16> = OnceLock::new();
    // Cargo/nextest can run several integration binaries concurrently. A
    // process-specific starting point prevents every binary from probing the
    // same fixed port range while retaining deterministic monotonic allocation
    // within one test process.
    let next_port = NEXT_PORT.get_or_init(|| {
        const BASE: u32 = 12_000;
        const PROCESS_SPAN: u32 = 40_000;
        let start = BASE + std::process::id() % PROCESS_SPAN;
        AtomicU16::new(u16::try_from(start).expect("test port seed fits u16"))
    });

    let mut attempts = 0u32;
    let mut last_err = None;
    loop {
        let port = next_port.fetch_add(1, Ordering::Relaxed);
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
            "exhausted process-local test port range; last bind error: {last_err:?}"
        );
    }
}

/// Allocate a concrete local socket address for tests that advertise peer endpoints.
fn next_addr() -> iroha_primitives::addr::SocketAddr {
    iroha_primitives::addr::SocketAddr::from(([127, 0, 0, 1], next_port()))
}

#[test]
fn next_addr_never_advertises_ephemeral_port() {
    assert_ne!(next_addr().port(), 0);
}

mod p2p;
mod p2p_caps;
mod p2p_consensus_caps;
mod p2p_puzzle;
mod p2p_trust_gossip;
#[cfg(feature = "p2p_ws")]
mod ws_io;
