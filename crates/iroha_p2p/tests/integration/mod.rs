use iroha_config::parameters::{
    actual::{LaneProfile, Network, RelayMode, SoranetHandshake, SoranetPrivacy, SoranetVpn},
    defaults::network as network_defaults,
};
use iroha_config_base::WithOrigin;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{NetworkId, block::BlockHeader};
use iroha_p2p::P2pIdentityKeys;
use iroha_primitives::addr::SocketAddr as IrohaSocketAddr;
use std::{
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    num::NonZeroUsize,
    sync::{
        OnceLock,
        atomic::{AtomicU16, Ordering},
    },
    time::Duration,
};

/// Build the exact fully populated baseline shared by P2P integration cases.
///
/// Callers use struct update syntax for case-specific deviations, keeping the
/// required network field set explicit in one place without collapsing cases.
#[expect(
    clippy::too_many_lines,
    reason = "the shared integration fixture deliberately lists every network field explicitly"
)]
fn test_network_config(
    address: IrohaSocketAddr,
    public_address: IrohaSocketAddr,
    idle_timeout: Duration,
    soranet_handshake: SoranetHandshake,
    trust_gossip: bool,
) -> Network {
    Network {
        address: WithOrigin::inline(address),
        public_address: WithOrigin::inline(public_address),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: network_defaults::RELAY_TTL,
        soranet_handshake,
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        reply_writer_flush_timeout: network_defaults::REPLY_WRITER_FLUSH_TIMEOUT,
        connect_startup_delay: network_defaults::CONNECT_STARTUP_DELAY,
        dial_timeout: network_defaults::DIAL_TIMEOUT,
        deferred_send_ttl: Duration::from_millis(network_defaults::DEFERRED_SEND_TTL_MS),
        deferred_send_max_per_peer: network_defaults::DEFERRED_SEND_MAX_PER_PEER,
        deferred_send_max_bytes_per_peer: network_defaults::DEFERRED_SEND_MAX_BYTES_PER_PEER,
        deferred_send_max_bytes_total: network_defaults::DEFERRED_SEND_MAX_BYTES_TOTAL,
        peer_gossip_period: network_defaults::PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: network_defaults::PEER_GOSSIP_PERIOD,
        trust_decay_half_life: network_defaults::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip: network_defaults::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer: network_defaults::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: network_defaults::TRUST_MIN_SCORE,
        trust_gossip,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: Vec::new(),
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
        addr_ipv6_first: false,
        dns_refresh_interval: None,
        dns_refresh_ttl: None,
        quic_enabled: false,
        quic_datagrams_enabled: network_defaults::QUIC_DATAGRAMS_ENABLED,
        quic_datagram_max_payload_bytes: network_defaults::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
        quic_datagram_receive_buffer_bytes: network_defaults::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES
            .get(),
        quic_datagram_send_buffer_bytes: network_defaults::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
        p2p_outbound_frame_queue_max_high_bytes:
            network_defaults::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES,
        p2p_outbound_frame_queue_max_low_bytes:
            network_defaults::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_BYTES,
        p2p_outbound_frame_queue_max_high_frames:
            network_defaults::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_FRAMES,
        p2p_outbound_frame_queue_max_low_frames:
            network_defaults::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_FRAMES,
        p2p_subscriber_queue_cap: network_defaults::P2P_SUBSCRIBER_QUEUE_CAP,
        consensus_ingress_rate_per_sec: network_defaults::CONSENSUS_INGRESS_RATE_PER_SEC,
        consensus_ingress_burst: network_defaults::CONSENSUS_INGRESS_BURST,
        consensus_ingress_bytes_per_sec: network_defaults::CONSENSUS_INGRESS_BYTES_PER_SEC,
        consensus_ingress_bytes_burst: network_defaults::CONSENSUS_INGRESS_BYTES_BURST,
        consensus_ingress_critical_rate_per_sec:
            network_defaults::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
        consensus_ingress_critical_burst: network_defaults::CONSENSUS_INGRESS_CRITICAL_BURST,
        consensus_ingress_critical_bytes_per_sec:
            network_defaults::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
        consensus_ingress_critical_bytes_burst:
            network_defaults::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
        consensus_ingress_penalty_threshold: network_defaults::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
        consensus_ingress_penalty_window: Duration::from_millis(
            network_defaults::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
        ),
        consensus_ingress_penalty_cooldown: Duration::from_millis(
            network_defaults::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
        ),
        max_incoming: None,
        max_total_connections: None,
        accept_rate_per_ip_per_sec: None,
        accept_burst_per_ip: None,
        max_accept_buckets: network_defaults::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: network_defaults::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: network_defaults::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: network_defaults::ACCEPT_PREFIX_V6_BITS,
        accept_rate_per_prefix_per_sec: None,
        accept_burst_per_prefix: None,
        low_priority_rate_per_sec: None,
        low_priority_burst: None,
        low_priority_bytes_per_sec: None,
        low_priority_bytes_burst: None,
        allowlist_only: false,
        allow_keys: Vec::new(),
        deny_keys: Vec::new(),
        allow_cidrs: Vec::new(),
        deny_cidrs: Vec::new(),
        disconnect_on_post_overflow: true,
        max_frame_bytes: 1_048_576,
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        quic_max_idle_timeout: None,
    }
}

fn test_network_id(seed: &str) -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
        iroha_crypto::Hash::new(seed.as_bytes()),
    ))
}
/// Generate the only supported first-release node identity for P2P tests.
fn random_node_key_pair() -> KeyPair {
    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
}
/// Assign an independently generated Ed25519 identity to the `SoraNet` transport role.
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
