//! Puzzle-gated handshake edge cases for P2P.
use super::next_port;
use iroha_config::parameters::{
    actual::{Network as Config, SoranetHandshake as ActualSoranetHandshake, SoranetPuzzle},
    defaults::network::{DEFAULT_AEAD_FRAME_OVERHEAD_BYTES, TRUST_GOSSIP},
};
use iroha_data_model::prelude::{Peer, PeerId};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{
    NetworkHandle,
    network::message::{ClassifyTopic, Topic, UpdatePeers, UpdateTopology},
    peer,
};
use iroha_primitives::addr::socket_addr;
use norito::codec::{Decode, Encode};
use std::{
    collections::HashSet,
    num::NonZeroU32,
    time::{Duration, Instant},
};
const PUZZLE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
#[derive(Clone, Debug, Decode, Encode)]
struct EmptyMsg;
impl ClassifyTopic for EmptyMsg {
    fn topic(&self) -> Topic {
        Topic::Other
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for EmptyMsg {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}
fn puzzle_handshake(difficulty: u8, memory_kib: u32) -> ActualSoranetHandshake {
    let mut handshake = super::low_cost_test_soranet_handshake();
    handshake.pow.difficulty = difficulty;
    handshake.pow.max_future_skew = Duration::from_secs(300);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    handshake.pow.ticket_ttl = Duration::from_secs(120);
    handshake.pow.puzzle = SoranetPuzzle {
        memory_kib: NonZeroU32::new(memory_kib).expect("non-zero puzzle memory"),
        time_cost: NonZeroU32::new(2).expect("non-zero time cost"),
        lanes: NonZeroU32::new(1).expect("non-zero lanes"),
    };
    handshake
}
fn config(addr: iroha_primitives::addr::SocketAddr, handshake: ActualSoranetHandshake) -> Config {
    let public_addr = addr.clone();
    Config {
        happy_eyeballs_stagger: Duration::from_millis(50),
        p2p_queue_cap_high: core::num::NonZeroUsize::new(1024).unwrap(),
        p2p_queue_cap_low: core::num::NonZeroUsize::new(1024).unwrap(),
        p2p_post_queue_cap: core::num::NonZeroUsize::new(256).unwrap(),
        // `max_frame_bytes` is the encrypted ceiling, while topic caps are
        // plaintext. Reserve the fixed ChaCha20-Poly1305 nonce and tag.
        max_frame_bytes: 1_048_576 + DEFAULT_AEAD_FRAME_OVERHEAD_BYTES,
        ..super::test_network_config(
            addr,
            public_addr,
            Duration::from_secs(5),
            handshake,
            TRUST_GOSSIP,
        )
    }
}
async fn assert_exact_peers_connect(
    network: &NetworkHandle<EmptyMsg>,
    expected_peers: &HashSet<PeerId>,
) {
    let mut online = network.online_peers_receiver();
    tokio::time::timeout(
        PUZZLE_CONNECT_TIMEOUT,
        online.wait_for(|peers| {
            peers.len() == expected_peers.len()
                && expected_peers.iter().all(|peer| peers.contains(peer))
        }),
    )
    .await
    .unwrap_or_else(|_| {
        panic!(
            "expected exact matching-puzzle peer set {expected_peers:?} did not connect within {PUZZLE_CONNECT_TIMEOUT:?}"
        )
    })
    .expect("online peers channel closed while waiting for required-puzzle handshake");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn matching_mandatory_puzzle_parameters_connect() {
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let chain = super::test_network_id("puzzle_match");
    let key_pairs = std::array::from_fn::<_, 4, _>(|_| super::random_node_key_pair());
    let addresses = std::array::from_fn::<_, 4, _>(|_| socket_addr!(127.0.0.1: {next_port()}));
    // Exercise the real Argon2 admission path with a small but valid memory
    // cost so the positive case remains reliable on loaded CI workers.
    let shutdown = ShutdownSignal::new();
    let mut networks = Vec::with_capacity(key_pairs.len());
    let mut children = Vec::with_capacity(key_pairs.len());
    for (index, (key_pair, address)) in key_pairs.iter().zip(&addresses).enumerate() {
        let handshake = puzzle_handshake(1, 4 * 1024);
        assert_eq!(handshake.pow.puzzle.memory_kib.get(), 4 * 1024);
        let (network, child) = NetworkHandle::<EmptyMsg>::start(
            super::p2p_identity_keys(key_pair.clone()),
            config(address.clone(), handshake),
            chain,
            None,
            None,
            shutdown.clone(),
        )
        .await
        .unwrap_or_else(|error| panic!("required-puzzle peer {index} should start: {error}"));
        networks.push(network);
        children.push(child);
    }
    let peers = addresses
        .into_iter()
        .zip(&key_pairs)
        .map(|(address, key_pair)| Peer::new(address, key_pair.public_key().clone()))
        .collect::<Vec<_>>();
    // Use a star with peer 0 as the sole dialer. This exercises three
    // independent puzzle-gated handshakes without simultaneous-connection
    // replacement races. Each leaf authorizes only the hub as an inbound peer.
    let hub_expected = peers[1..]
        .iter()
        .map(|peer| peer.id().clone())
        .collect::<HashSet<_>>();
    networks[0].update_topology(UpdateTopology(hub_expected.clone()));
    networks[0].update_peers_addresses(UpdatePeers(
        peers[1..]
            .iter()
            .map(|peer| (peer.id().clone(), peer.address().clone()))
            .collect(),
    ));
    let leaf_expected = HashSet::from([peers[0].id().clone()]);
    for network in &networks[1..] {
        network.update_topology(UpdateTopology(leaf_expected.clone()));
    }
    tokio::join!(
        assert_exact_peers_connect(&networks[0], &hub_expected),
        assert_exact_peers_connect(&networks[1], &leaf_expected),
        assert_exact_peers_connect(&networks[2], &leaf_expected),
        assert_exact_peers_connect(&networks[3], &leaf_expected),
    );
    shutdown.send();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn puzzle_mismatch_rejects_handshake() {
    let chain = super::test_network_id("puzzle_mismatch");
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let baseline_failures = peer::handshake_failure_count();
    let kp1 = super::random_node_key_pair();
    let kp2 = super::random_node_key_pair();
    let addr1 = socket_addr!(127.0.0.1: {next_port()});
    let addr2 = socket_addr!(127.0.0.1: {next_port()});
    // Keep the puzzle work small so the mismatch is observed reliably even under CPU load.
    let handshake_entry = puzzle_handshake(2, 8 * 1024);
    let handshake_exit = puzzle_handshake(3, 8 * 1024);
    let started1 = NetworkHandle::<EmptyMsg>::start(
        super::p2p_identity_keys(kp1.clone()),
        config(addr1.clone(), handshake_entry),
        chain,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _child1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let started2 = NetworkHandle::<EmptyMsg>::start(
        super::p2p_identity_keys(kp2.clone()),
        config(addr2.clone(), handshake_exit),
        chain,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _child2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let peer1 = Peer::new(addr1, kp1.public_key().clone());
    let peer2 = Peer::new(addr2, kp2.public_key().clone());
    net1.update_topology(UpdateTopology([peer2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(
        peer2.id().clone(),
        peer2.address().clone(),
    )]));
    net2.update_topology(UpdateTopology([peer1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(
        peer1.id().clone(),
        peer1.address().clone(),
    )]));
    // Allow enough time for the slower side to mint a ticket when tests are running in parallel.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut observed_failure = false;
    while Instant::now() < deadline {
        if peer::handshake_failure_count() > baseline_failures {
            observed_failure = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Avoid races where the counter increments between the last poll and the loop exit.
    observed_failure |= peer::handshake_failure_count() > baseline_failures;
    assert!(
        observed_failure,
        "expected puzzle mismatch to cause handshake failure"
    );
    assert_eq!(
        net1.online_peers(HashSet::len),
        0,
        "entry relay must not mark the mismatched peer online"
    );
    assert_eq!(
        net2.online_peers(HashSet::len),
        0,
        "exit relay must not mark the mismatched peer online"
    );
}
