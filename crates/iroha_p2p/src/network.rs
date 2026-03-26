//! Network formed out of Iroha peers.
#![allow(
    clippy::unused_async,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]
#[cfg(feature = "quic")]
use std::sync::OnceLock;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Debug,
    io,
    net::{IpAddr, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake, SoranetPow as ActualSoranetPow,
};
use iroha_crypto::{
    KeyPair,
    soranet::{
        pow::{Parameters as PowParameters, TicketRevocationStore, TicketRevocationStoreLimits},
        puzzle,
    },
};
use iroha_data_model::{
    ChainId,
    prelude::{Peer, PeerId},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::prelude::*;
use iroha_primitives::addr::SocketAddr;
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};
use rand::Rng as _;
use soranet_pq::MlDsaSuite;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch},
};

#[cfg(feature = "p2p_tls")]
use crate::boilerplate;
use crate::{
    Broadcast, Error, NetworkMessage, OnlinePeers, Post, Priority, RelayRole, UpdatePeers,
    UpdateTopology, UpdateTrustedPeers,
    boilerplate::*,
    peer::{
        Connection, ConnectionId, SoranetHandshakeConfig,
        handles::{PeerHandle, PostError, connected_from, connecting},
        message::*,
    },
    sampler::LogSampler,
};

#[cfg(feature = "quic")]
static NEXT_QUIC_CONN_ID: OnceLock<AtomicU64> = OnceLock::new();

// Simple CIDR (IPv4/IPv6) representation for ACLs.
#[derive(Clone, Debug)]
struct IpNet {
    kind: IpKind,
}

#[derive(Clone, Debug)]
enum IpKind {
    V4 { net: u32, mask: u32 },
    V6 { net: [u8; 16], bits: u8 },
}

fn parse_cidr(s: &str) -> Option<IpNet> {
    if let Some((ip, bits_str)) = s.split_once('/') {
        let bits: u8 = bits_str.parse().ok()?;
        if let Ok(v4) = ip.parse::<std::net::Ipv4Addr>() {
            if bits > 32 {
                return None;
            }
            let n = u32::from(v4);
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            return Some(IpNet {
                kind: IpKind::V4 {
                    net: n & mask,
                    mask,
                },
            });
        }
        if let Ok(v6) = ip.parse::<std::net::Ipv6Addr>() {
            if bits > 128 {
                return None;
            }
            let mut net = [0u8; 16];
            net.copy_from_slice(&v6.octets());
            let full_bytes = (bits / 8) as usize;
            let rem_bits = bits % 8;
            if full_bytes < 16 {
                if rem_bits == 0 {
                    for b in net.iter_mut().skip(full_bytes) {
                        *b = 0;
                    }
                } else {
                    for b in net.iter_mut().skip(full_bytes + 1) {
                        *b = 0;
                    }
                    let mask = 0xFFu8 << (8 - rem_bits);
                    net[full_bytes] &= mask;
                }
            }
            return Some(IpNet {
                kind: IpKind::V6 { net, bits },
            });
        }
    }
    None
}

fn parse_cidrs(list: &[String]) -> Vec<IpNet> {
    list.iter().filter_map(|s| parse_cidr(s)).collect()
}

fn cidr_contains(nets: &[IpNet], ip: std::net::IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let x = u32::from(v4);
            nets.iter().any(|n| match &n.kind {
                IpKind::V4 { net, mask } => (x & mask) == *net,
                _ => false,
            })
        }
        IpAddr::V6(v6) => {
            let x = v6.octets();
            nets.iter().any(|n| match &n.kind {
                IpKind::V6 { net, bits } => {
                    let full = (*bits / 8) as usize;
                    let rem = *bits % 8;
                    (full == 0 || x[..full] == net[..full])
                        && (rem == 0 || {
                            let mask = 0xFFu8 << (8 - rem);
                            (x[full] & mask) == (net[full] & mask)
                        })
                }
                _ => false,
            })
        }
    }
}

fn runtime_from_handshake(
    handshake: ActualSoranetHandshake,
) -> Result<Arc<SoranetHandshakeConfig>, Error> {
    let ActualSoranetHandshake {
        descriptor_commit,
        client_capabilities,
        relay_capabilities,
        trust_gossip,
        kem_id,
        sig_id,
        resume_hash,
        pow,
    } = handshake;

    let ActualSoranetPow {
        required,
        difficulty,
        max_future_skew,
        min_ticket_ttl,
        ticket_ttl,
        revocation_store_capacity,
        revocation_max_ttl,
        revocation_store_path,
        puzzle,
        signed_ticket_public_key,
    } = pow;

    let pow_params = PowParameters::new(difficulty, max_future_skew, min_ticket_ttl);
    let puzzle_params = puzzle.map(|cfg| {
        puzzle::Parameters::new(
            cfg.memory_kib,
            cfg.time_cost,
            cfg.lanes,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
        )
    });

    let signed_ticket_public_key = signed_ticket_public_key
        .map(|key| {
            let expected = MlDsaSuite::MlDsa44.public_key_len();
            if key.len() != expected {
                return Err(Error::HandshakeSoranet(format!(
                    "invalid soranet signed_ticket_public_key_hex: expected {expected} bytes (ML-DSA-44), got {}",
                    key.len()
                )));
            }
            Ok(key)
        })
        .transpose()?;

    let revocation_limits =
        TicketRevocationStoreLimits::new(revocation_store_capacity, revocation_max_ttl).map_err(
            |err| {
                Error::HandshakeSoranet(format!("invalid soranet revocation configuration: {err}"))
            },
        )?;
    let revocation_store = TicketRevocationStore::load(
        revocation_store_path.as_ref(),
        revocation_limits,
        SystemTime::now(),
    )
    .unwrap_or_else(|err| {
        iroha_logger::warn!(
            path = %revocation_store_path,
            ?err,
            "failed to load soranet revocation store; falling back to in-memory cache"
        );
        TicketRevocationStore::in_memory(revocation_limits)
            .expect("revocation limits already validated")
    });
    let revocation_store = Some(Arc::new(Mutex::new(revocation_store)));

    let config = SoranetHandshakeConfig::new(
        descriptor_commit.into_value(),
        client_capabilities.into_value(),
        relay_capabilities.into_value(),
        trust_gossip,
        kem_id,
        sig_id,
        resume_hash.map(iroha_config::base::WithOrigin::into_value),
        required,
        pow_params,
        puzzle_params,
        ticket_ttl,
        signed_ticket_public_key,
        revocation_store,
        None,
    );
    Ok(Arc::new(config))
}

fn relay_role_from_mode(mode: iroha_config::parameters::actual::RelayMode) -> RelayRole {
    match mode {
        iroha_config::parameters::actual::RelayMode::Hub => RelayRole::Hub,
        iroha_config::parameters::actual::RelayMode::Spoke => RelayRole::Spoke,
        iroha_config::parameters::actual::RelayMode::Disabled
        | iroha_config::parameters::actual::RelayMode::Assist => RelayRole::Disabled,
    }
}

#[cfg(test)]
mod runtime_tests {
    use std::{fs, num::NonZeroU32, time::Duration};

    use iroha_config::parameters::actual::SoranetPuzzle as ConfigPuzzle;
    use rand::{SeedableRng, rngs::StdRng};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn runtime_from_handshake_preserves_puzzle_parameters() {
        let mut handshake = ActualSoranetHandshake::default();
        handshake.pow.required = true;
        handshake.pow.difficulty = 6;
        handshake.pow.max_future_skew = Duration::from_secs(300);
        handshake.pow.min_ticket_ttl = Duration::from_secs(60);
        handshake.pow.ticket_ttl = Duration::from_secs(240);
        handshake.pow.puzzle = Some(ConfigPuzzle {
            memory_kib: NonZeroU32::new(64 * 1024).expect("memory"),
            time_cost: NonZeroU32::new(3).expect("time_cost"),
            lanes: NonZeroU32::new(2).expect("lanes"),
        });
        let dir = tempdir().expect("tempdir");
        handshake.pow.revocation_store_path = dir
            .path()
            .join("revocations.norito")
            .to_string_lossy()
            .into_owned()
            .into();

        let runtime = runtime_from_handshake(handshake).expect("runtime");
        assert!(
            runtime.pow_required(),
            "puzzle-enabled handshake must require PoW"
        );
        let pow = runtime.pow_parameters();
        assert_eq!(pow.difficulty(), 6);
        let puzzle = runtime
            .puzzle_parameters()
            .expect("puzzle parameters should be present");
        assert_eq!(puzzle.memory_kib().get(), 64 * 1024);
        assert_eq!(puzzle.time_cost().get(), 3);
        assert_eq!(puzzle.lanes().get(), 2);
    }

    #[test]
    fn runtime_from_handshake_rejects_invalid_revocation_limits() {
        let mut handshake = ActualSoranetHandshake::default();
        handshake.pow.required = true;
        handshake.pow.revocation_store_capacity = 0;

        let err = runtime_from_handshake(handshake).expect_err("should fail");
        match err {
            Error::HandshakeSoranet(message) => {
                assert!(
                    message.contains("revocation"),
                    "expected revocation validation failure, got {message}"
                );
            }
            other => panic!("unexpected error type: {other:?}"),
        }
    }

    #[test]
    fn runtime_from_handshake_falls_back_on_corrupt_revocation_snapshot() {
        let mut handshake = ActualSoranetHandshake::default();
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        fs::write(&path, b"corrupt snapshot").expect("write corrupt revocation file");
        handshake.pow.required = true;
        handshake.pow.difficulty = 1;
        handshake.pow.revocation_store_path = path.to_string_lossy().into_owned().into();

        let runtime = runtime_from_handshake(handshake).expect("runtime should fall back");
        let mut rng = StdRng::from_seed([0x44; 32]);
        let minted = runtime
            .mint_challenge_ticket(&mut rng)
            .expect("mint ticket")
            .expect("ticket present");
        let ticket = minted.ticket.expect("ticket bytes");

        runtime
            .verify_challenge_ticket(&ticket)
            .expect("first verify succeeds");
        assert_eq!(runtime.active_revocations(), 1);
        let err = runtime
            .verify_challenge_ticket(&ticket)
            .expect_err("replay should be rejected via fallback store");
        assert!(matches!(err, crate::peer::ChallengeVerifyError::Replay));
    }
}

mod net_channel {
    use tokio::sync::mpsc;

    pub type Sender<T> = mpsc::Sender<T>;
    pub type Receiver<T> = mpsc::Receiver<T>;

    pub fn channel_with_capacity<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
        mpsc::channel(cap)
    }
}

/// Count of network posts dropped due to full bounded queue.
static DROPPED_POSTS: AtomicU64 = AtomicU64::new(0);
/// Count of network broadcasts dropped due to full bounded queue.
static DROPPED_BROADCASTS: AtomicU64 = AtomicU64::new(0);
/// High/Low split for bounded queue drops (posts)
static DROPPED_POSTS_HI: AtomicU64 = AtomicU64::new(0);
static DROPPED_POSTS_LO: AtomicU64 = AtomicU64::new(0);
/// High/Low split for bounded queue drops (broadcasts)
static DROPPED_BROADCASTS_HI: AtomicU64 = AtomicU64::new(0);
static DROPPED_BROADCASTS_LO: AtomicU64 = AtomicU64::new(0);
/// Latest observed depth for the high-priority network message queue.
static NETWORK_QUEUE_DEPTH_HIGH: AtomicU64 = AtomicU64::new(0);
/// Latest observed depth for the low-priority network message queue.
static NETWORK_QUEUE_DEPTH_LOW: AtomicU64 = AtomicU64::new(0);
/// Count of DNS interval-based hostname refreshes performed.
static DNS_REFRESHES: AtomicU64 = AtomicU64::new(0);
/// Count of DNS TTL-based hostname refreshes performed.
static DNS_TTL_REFRESHES: AtomicU64 = AtomicU64::new(0);
/// Count of hostname reconnect successes after a refresh.
static DNS_RECONNECT_SUCCESSES: AtomicU64 = AtomicU64::new(0);
/// Count of hostname resolution/connection failures for hostname peers.
static DNS_RESOLUTION_FAILURES: AtomicU64 = AtomicU64::new(0);
/// Count of scheduled per-address backoffs.
static BACKOFF_SCHEDULED: AtomicU64 = AtomicU64::new(0);
/// Total deferred outbound frames enqueued while peer session was missing.
static DEFERRED_SEND_ENQUEUED: AtomicU64 = AtomicU64::new(0);
/// Total deferred outbound frames dropped due to TTL expiry, stale generation, or queue cap.
static DEFERRED_SEND_DROPPED: AtomicU64 = AtomicU64::new(0);
/// Total reconnect attempts triggered because outbound frames were deferred for missing sessions.
static SESSION_RECONNECT_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Cumulative reconnect retry delay in milliseconds.
static CONNECT_RETRY_MILLIS_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Count of inbound WS connections accepted via Torii.
static WS_INBOUND_ACCEPTED: AtomicU64 = AtomicU64::new(0);
/// Count of outbound WS connections successfully established.
static WS_OUTBOUND_SUCCESSES: AtomicU64 = AtomicU64::new(0);
/// Count of inbound SCION connections accepted.
static SCION_INBOUND_ACCEPTED: AtomicU64 = AtomicU64::new(0);
/// Count of outbound SCION connections successfully established.
static SCION_OUTBOUND_SUCCESSES: AtomicU64 = AtomicU64::new(0);
/// Count of accepted connections dropped due to per-IP accept throttle.
static ACCEPT_THROTTLED: AtomicU64 = AtomicU64::new(0);
/// Count of accept throttle bucket evictions (idle or capacity).
static ACCEPT_BUCKET_EVICTIONS: AtomicU64 = AtomicU64::new(0);
/// Current number of active accept throttle buckets (prefix + per-IP).
static ACCEPT_BUCKETS_CURRENT: AtomicU64 = AtomicU64::new(0);
/// Count of prefix bucket cache hits.
static ACCEPT_PREFIX_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
/// Count of prefix bucket cache misses.
static ACCEPT_PREFIX_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
/// Count of prefix throttle rejections.
static ACCEPT_PREFIX_THROTTLED: AtomicU64 = AtomicU64::new(0);
/// Count of prefix throttle allowances.
static ACCEPT_PREFIX_ALLOWED: AtomicU64 = AtomicU64::new(0);
/// Count of per-IP throttle allowances.
static ACCEPT_IP_ALLOWED: AtomicU64 = AtomicU64::new(0);
/// Count of per-IP throttle rejections.
static ACCEPT_IP_THROTTLED: AtomicU64 = AtomicU64::new(0);
/// Count of accepted connections dropped due to `max_incoming` cap.
static INCOMING_CAP_REJECTS: AtomicU64 = AtomicU64::new(0);
/// Count of accepted connections dropped due to `max_total_connections` cap.
static TOTAL_CAP_REJECTS: AtomicU64 = AtomicU64::new(0);
/// Count of low-priority post messages throttled by per-peer token buckets.
static LOW_THROTTLED_POSTS: AtomicU64 = AtomicU64::new(0);
/// Count of low-priority broadcast deliveries throttled per peer.
static LOW_THROTTLED_BROADCASTS: AtomicU64 = AtomicU64::new(0);
/// Count of trust-gossip frames skipped because the capability is disabled locally or remotely.
static TRUST_GOSSIP_SKIPPED_CAP_OFF: AtomicU64 = AtomicU64::new(0);
/// Count of inbound messages dropped because subscriber queues are full.
static SUBSCRIBER_QUEUE_FULL: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_CONSENSUS_CHUNK: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_CONTROL: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_HEALTH: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_QUEUE_FULL_OTHER: AtomicU64 = AtomicU64::new(0);
/// Count of inbound frames dropped because no subscriber matches the topic.
static SUBSCRIBER_UNROUTED: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_CONSENSUS_CHUNK: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_CONTROL: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_HEALTH: AtomicU64 = AtomicU64::new(0);
static SUBSCRIBER_UNROUTED_OTHER: AtomicU64 = AtomicU64::new(0);
/// Count of per-peer post channel overflows (bounded per-topic channels).
static POST_OVERFLOWS: AtomicU64 = AtomicU64::new(0);
/// Per-topic frame cap violations
static CAP_VIOL_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_CONTROL: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_HEALTH: AtomicU64 = AtomicU64::new(0);
static CAP_VIOL_OTHER: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_CONTROL: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HEALTH: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_OTHER: AtomicU64 = AtomicU64::new(0);
// Per-priority breakdown (High/Low) per topic
static POST_OVERFLOWS_HI_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_CONTROL: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_HEALTH: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_HI_OTHER: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_CONSENSUS: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_CONTROL: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_BLOCK_SYNC: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_TX_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_PEER_GOSSIP: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_HEALTH: AtomicU64 = AtomicU64::new(0);
static POST_OVERFLOWS_LO_OTHER: AtomicU64 = AtomicU64::new(0);

// Jittered exponential backoff for reconnect attempts.
const BACKOFF_INITIAL: Duration = Duration::from_millis(100);
const BACKOFF_MAX: Duration = Duration::from_secs(5);
const INBOUND_PEER_HIGH_BUDGET: usize = 32;
/// Default hop limit for relay forwarding (origin hub hop + spoke hop).
#[cfg(test)]
const DEFAULT_RELAY_TTL: u8 = 8;
// Stagger delay between multi-address dial attempts for the same peer.
// Stagger for parallel dialing is configurable via node config per instance.

#[derive(Clone, Debug, Encode, Decode)]
enum RelayTarget {
    Broadcast,
    Direct(PeerId),
}

#[derive(Clone, Debug, Encode, Decode)]
struct RelayMessage<T> {
    origin: PeerId,
    target: RelayTarget,
    ttl: u8,
    priority: Priority,
    payload: T,
}

impl<'a, T> ncore::DecodeFromSlice<'a> for RelayMessage<T>
where
    T: ncore::NoritoSerialize + for<'de> ncore::NoritoDeserialize<'de>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let archived = ncore::archived_from_slice::<Self>(bytes)?;
        let archived_bytes = archived.bytes();
        let _guard = ncore::PayloadCtxGuard::enter(archived_bytes);
        let value = <Self as ncore::NoritoDeserialize>::try_deserialize(archived.archived())?;
        Ok((value, archived_bytes.len()))
    }
}

impl<T> RelayMessage<T> {
    fn new(origin: PeerId, target: RelayTarget, ttl: u8, priority: Priority, payload: T) -> Self {
        Self {
            origin,
            target,
            ttl,
            priority,
            payload,
        }
    }

    #[allow(dead_code)]
    fn decremented_ttl(&self) -> Option<u8> {
        self.ttl.checked_sub(1)
    }
}

/// Return the plaintext wire length of a P2P data frame containing `payload`.
///
/// This accounts for the relay envelope and the `Message::Data` wrapper but
/// excludes encryption overhead (use `frame_plaintext_cap` to apply caps).
pub fn data_frame_wire_len<T: Encode + Clone>(
    origin: &PeerId,
    target: Option<&PeerId>,
    ttl: u8,
    priority: message::Priority,
    payload: &T,
) -> usize {
    let target = target.map_or(RelayTarget::Broadcast, |peer_id| {
        RelayTarget::Direct(peer_id.clone())
    });
    let frame = RelayMessage::new(origin.clone(), target, ttl, priority, payload.clone());
    crate::peer::data_message_wire_len(frame)
}

type WireMessage<T> = RelayMessage<T>;

impl<T: message::ClassifyTopic> message::ClassifyTopic for RelayMessage<T> {
    fn topic(&self) -> message::Topic {
        self.payload.topic()
    }
}

fn peer_message_channel<T: Pload>(
    cap: core::num::NonZeroUsize,
) -> (
    mpsc::Sender<PeerMessage<WireMessage<T>>>,
    mpsc::Receiver<PeerMessage<WireMessage<T>>>,
) {
    mpsc::channel(cap.get())
}

#[derive(Clone, Debug)]
struct DeferredPeerFrame<T: Pload> {
    frame: RelayMessage<T>,
    topic: message::Topic,
    enqueued_at: tokio::time::Instant,
    generation: Option<ConnectionId>,
}

#[derive(Debug)]
struct DeferredPeerFrameQueue<T: Pload> {
    by_peer: HashMap<PeerId, VecDeque<DeferredPeerFrame<T>>>,
    max_per_peer: usize,
    ttl: Duration,
}

impl<T: Pload> DeferredPeerFrameQueue<T> {
    fn new(max_per_peer: usize, ttl: Duration) -> Self {
        Self {
            by_peer: HashMap::new(),
            max_per_peer: max_per_peer.max(1),
            ttl,
        }
    }

    fn prune_expired(
        entries: &mut VecDeque<DeferredPeerFrame<T>>,
        now: tokio::time::Instant,
        ttl: Duration,
    ) -> usize {
        if ttl.is_zero() {
            let dropped = entries.len();
            entries.clear();
            return dropped;
        }
        let mut dropped = 0usize;
        while entries
            .front()
            .is_some_and(|entry| now.saturating_duration_since(entry.enqueued_at) > ttl)
        {
            entries.pop_front();
            dropped = dropped.saturating_add(1);
        }
        dropped
    }

    fn enqueue(
        &mut self,
        peer_id: PeerId,
        frame: RelayMessage<T>,
        topic: message::Topic,
        generation: Option<ConnectionId>,
        now: tokio::time::Instant,
    ) -> (usize, usize) {
        let entries = self.by_peer.entry(peer_id).or_default();
        let expired = Self::prune_expired(entries, now, self.ttl);
        let mut overflow = 0usize;
        while entries.len() >= self.max_per_peer {
            entries.pop_front();
            overflow = overflow.saturating_add(1);
        }
        entries.push_back(DeferredPeerFrame {
            frame,
            topic,
            enqueued_at: now,
            generation,
        });
        (expired, overflow)
    }

    fn take_peer(
        &mut self,
        peer_id: &PeerId,
        now: tokio::time::Instant,
    ) -> (VecDeque<DeferredPeerFrame<T>>, usize) {
        let Some(mut entries) = self.by_peer.remove(peer_id) else {
            return (VecDeque::new(), 0);
        };
        let expired = Self::prune_expired(&mut entries, now, self.ttl);
        (entries, expired)
    }

    fn restore_peer(&mut self, peer_id: PeerId, entries: VecDeque<DeferredPeerFrame<T>>) {
        if entries.is_empty() {
            self.by_peer.remove(&peer_id);
            return;
        }
        self.by_peer.insert(peer_id, entries);
    }
}

#[cfg(test)]
mod data_frame_wire_len_tests {
    use iroha_crypto::KeyPair;
    use norito::codec::{Decode, Encode};

    use super::*;

    #[derive(Clone, Debug, Decode, Encode)]
    struct Dummy {
        tag: u8,
    }

    #[test]
    fn data_frame_wire_len_matches_manual_envelope() {
        let origin = PeerId::from(KeyPair::random().public_key().clone());
        let target = PeerId::from(KeyPair::random().public_key().clone());
        let payload = Dummy { tag: 7 };

        let direct =
            data_frame_wire_len(&origin, Some(&target), 8, message::Priority::High, &payload);
        let direct_frame = RelayMessage::new(
            origin.clone(),
            RelayTarget::Direct(target.clone()),
            8,
            message::Priority::High,
            payload.clone(),
        );
        let direct_expected = crate::peer::data_message_wire_len(direct_frame);
        assert_eq!(
            direct, direct_expected,
            "direct frame size should match envelope"
        );

        let broadcast = data_frame_wire_len(&origin, None, 8, message::Priority::Low, &payload);
        let broadcast_frame = RelayMessage::new(
            origin,
            RelayTarget::Broadcast,
            8,
            message::Priority::Low,
            payload,
        );
        let broadcast_expected = crate::peer::data_message_wire_len(broadcast_frame);
        assert_eq!(
            broadcast, broadcast_expected,
            "broadcast frame size should match envelope"
        );
    }

    #[test]
    fn relay_message_decode_from_slice_roundtrip() {
        let origin = PeerId::from(KeyPair::random().public_key().clone());
        let target = PeerId::from(KeyPair::random().public_key().clone());
        let payload = Dummy { tag: 42 };
        let frame = RelayMessage::new(
            origin.clone(),
            RelayTarget::Direct(target.clone()),
            5,
            message::Priority::High,
            payload.clone(),
        );

        let bytes = frame.encode();
        let (decoded, used) =
            <RelayMessage<Dummy> as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
                .expect("decode relay message");

        assert_eq!(used, bytes.len(), "should consume full payload");
        assert_eq!(decoded.origin, origin);
        assert_eq!(decoded.ttl, 5);
        assert_eq!(decoded.priority, message::Priority::High);
        assert_eq!(decoded.payload.tag, payload.tag);
        match decoded.target {
            RelayTarget::Direct(peer_id) => assert_eq!(peer_id, target),
            RelayTarget::Broadcast => panic!("expected direct relay target"),
        }
    }
}

/// Returns the number of dropped post messages (bounded queues only).
///
/// In unbounded mode this counter stays at 0.
pub fn dropped_post_count() -> u64 {
    DROPPED_POSTS.load(Ordering::Relaxed)
}

/// Returns the number of dropped broadcast messages (bounded queues only).
///
/// In unbounded mode this counter stays at 0.
pub fn dropped_broadcast_count() -> u64 {
    DROPPED_BROADCASTS.load(Ordering::Relaxed)
}

/// Returns the number of dropped post messages for High priority.
pub fn dropped_post_high_count() -> u64 {
    DROPPED_POSTS_HI.load(Ordering::Relaxed)
}
/// Returns the number of dropped post messages for Low priority.
pub fn dropped_post_low_count() -> u64 {
    DROPPED_POSTS_LO.load(Ordering::Relaxed)
}
/// Returns the number of dropped broadcast messages for High priority.
pub fn dropped_broadcast_high_count() -> u64 {
    DROPPED_BROADCASTS_HI.load(Ordering::Relaxed)
}
/// Returns the number of dropped broadcast messages for Low priority.
pub fn dropped_broadcast_low_count() -> u64 {
    DROPPED_BROADCASTS_LO.load(Ordering::Relaxed)
}

/// Returns the last observed depth of the high-priority network queue.
pub fn network_queue_depth_high() -> u64 {
    NETWORK_QUEUE_DEPTH_HIGH.load(Ordering::Relaxed)
}

/// Returns the last observed depth of the low-priority network queue.
pub fn network_queue_depth_low() -> u64 {
    NETWORK_QUEUE_DEPTH_LOW.load(Ordering::Relaxed)
}

// Update cached queue depths for telemetry metrics.
fn update_network_queue_depth_high(len: usize) {
    NETWORK_QUEUE_DEPTH_HIGH.store(len as u64, Ordering::Relaxed);
}

fn update_network_queue_depth_low(len: usize) {
    NETWORK_QUEUE_DEPTH_LOW.store(len as u64, Ordering::Relaxed);
}

/// Returns the number of inbound messages dropped because subscriber queues are full.
pub fn subscriber_queue_full_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic Consensus.
pub fn subscriber_queue_full_consensus_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_CONSENSUS.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic `ConsensusChunk`.
pub fn subscriber_queue_full_consensus_chunk_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_CONSENSUS_CHUNK.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic Control.
pub fn subscriber_queue_full_control_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_CONTROL.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic `BlockSync`.
pub fn subscriber_queue_full_block_sync_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic `TxGossip`.
pub fn subscriber_queue_full_tx_gossip_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic `PeerGossip`.
pub fn subscriber_queue_full_peer_gossip_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic Health.
pub fn subscriber_queue_full_health_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_HEALTH.load(Ordering::Relaxed)
}
/// Returns the number of subscriber-queue drops for topic Other.
pub fn subscriber_queue_full_other_count() -> u64 {
    SUBSCRIBER_QUEUE_FULL_OTHER.load(Ordering::Relaxed)
}

/// Returns the number of inbound messages dropped due to no matching subscriber.
pub fn subscriber_unrouted_count() -> u64 {
    SUBSCRIBER_UNROUTED.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic Consensus.
pub fn subscriber_unrouted_consensus_count() -> u64 {
    SUBSCRIBER_UNROUTED_CONSENSUS.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic `ConsensusChunk`.
pub fn subscriber_unrouted_consensus_chunk_count() -> u64 {
    SUBSCRIBER_UNROUTED_CONSENSUS_CHUNK.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic Control.
pub fn subscriber_unrouted_control_count() -> u64 {
    SUBSCRIBER_UNROUTED_CONTROL.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic `BlockSync`.
pub fn subscriber_unrouted_block_sync_count() -> u64 {
    SUBSCRIBER_UNROUTED_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic `TxGossip`.
pub fn subscriber_unrouted_tx_gossip_count() -> u64 {
    SUBSCRIBER_UNROUTED_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic `PeerGossip`.
pub fn subscriber_unrouted_peer_gossip_count() -> u64 {
    SUBSCRIBER_UNROUTED_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic Health.
pub fn subscriber_unrouted_health_count() -> u64 {
    SUBSCRIBER_UNROUTED_HEALTH.load(Ordering::Relaxed)
}
/// Returns the number of unrouted inbound messages for topic Other.
pub fn subscriber_unrouted_other_count() -> u64 {
    SUBSCRIBER_UNROUTED_OTHER.load(Ordering::Relaxed)
}

/// Testing helper: increment bounded queue drop counters directly.
///
/// - `priority_high`: true for High, false for Low.
/// - `broadcast`: true for Broadcast, false for Post.
/// - `n`: amount to add.
pub fn inc_queue_drop_for_test(priority_high: bool, broadcast: bool, n: u64) {
    use std::sync::atomic::Ordering::Relaxed;
    if broadcast {
        DROPPED_BROADCASTS.fetch_add(n, Relaxed);
        if priority_high {
            DROPPED_BROADCASTS_HI.fetch_add(n, Relaxed);
        } else {
            DROPPED_BROADCASTS_LO.fetch_add(n, Relaxed);
        }
    } else {
        DROPPED_POSTS.fetch_add(n, Relaxed);
        if priority_high {
            DROPPED_POSTS_HI.fetch_add(n, Relaxed);
        } else {
            DROPPED_POSTS_LO.fetch_add(n, Relaxed);
        }
    }
}

/// Testing helper: set the observed network queue depth for High/Low queues.
pub fn set_network_queue_depth_for_test(priority_high: bool, len: usize) {
    if priority_high {
        update_network_queue_depth_high(len);
    } else {
        update_network_queue_depth_low(len);
    }
}

/// Testing helper: increment subscriber-queue-full counters directly for a topic.
pub fn inc_subscriber_queue_full_for_test(topic: message::Topic, n: u64) {
    for _ in 0..n {
        inc_subscriber_queue_full_for(topic);
    }
}

/// Testing helper: increment subscriber unrouted counters directly for a topic.
pub fn inc_subscriber_unrouted_for_test(topic: message::Topic, n: u64) {
    for _ in 0..n {
        inc_subscriber_unrouted_for(topic);
    }
}

/// Returns the number of interval-based DNS refresh cycles performed.
pub fn dns_refresh_count() -> u64 {
    DNS_REFRESHES.load(Ordering::Relaxed)
}

/// Returns the number of TTL-based DNS refresh cycles performed.
pub fn dns_ttl_refresh_count() -> u64 {
    DNS_TTL_REFRESHES.load(Ordering::Relaxed)
}

/// Returns the number of hostname reconnect successes after refresh cycles.
pub fn dns_reconnect_success_count() -> u64 {
    DNS_RECONNECT_SUCCESSES.load(Ordering::Relaxed)
}

/// Returns the number of hostname resolution/connection failures for hostname peers.
pub fn dns_resolution_fail_count() -> u64 {
    DNS_RESOLUTION_FAILURES.load(Ordering::Relaxed)
}

/// Increment the hostname resolution failure counter.
pub fn inc_dns_resolution_fail() {
    DNS_RESOLUTION_FAILURES.fetch_add(1, Ordering::Relaxed);
}

/// Returns the number of scheduled per-address backoffs.
pub fn backoff_scheduled_count() -> u64 {
    BACKOFF_SCHEDULED.load(Ordering::Relaxed)
}

/// Returns total deferred outbound frames enqueued while peer session was missing.
pub fn deferred_send_enqueued_count() -> u64 {
    DEFERRED_SEND_ENQUEUED.load(Ordering::Relaxed)
}

/// Returns total deferred outbound frames dropped (TTL, stale generation, cap).
pub fn deferred_send_dropped_count() -> u64 {
    DEFERRED_SEND_DROPPED.load(Ordering::Relaxed)
}

/// Returns total reconnect attempts triggered because outbound frames were deferred.
pub fn session_reconnect_total() -> u64 {
    SESSION_RECONNECT_TOTAL.load(Ordering::Relaxed)
}

/// Returns cumulative reconnect retry delay in seconds (rounded up from milliseconds).
pub fn connect_retry_seconds_total() -> u64 {
    CONNECT_RETRY_MILLIS_TOTAL
        .load(Ordering::Relaxed)
        .div_ceil(1_000)
}

/// Increment WS inbound accepted counter (called by Torii `/p2p`).
pub fn inc_ws_inbound() {
    WS_INBOUND_ACCEPTED.fetch_add(1, Ordering::Relaxed);
}

/// Increment WS outbound success counter (called by WS dialer path).
pub fn inc_ws_outbound() {
    WS_OUTBOUND_SUCCESSES.fetch_add(1, Ordering::Relaxed);
}

/// Total accepted inbound WS connections.
pub fn ws_inbound_total() -> u64 {
    WS_INBOUND_ACCEPTED.load(Ordering::Relaxed)
}

/// Total successful outbound WS connections.
pub fn ws_outbound_total() -> u64 {
    WS_OUTBOUND_SUCCESSES.load(Ordering::Relaxed)
}

/// Increment SCION inbound accepted counter.
pub fn inc_scion_inbound() {
    SCION_INBOUND_ACCEPTED.fetch_add(1, Ordering::Relaxed);
}

/// Increment SCION outbound success counter.
pub fn inc_scion_outbound() {
    SCION_OUTBOUND_SUCCESSES.fetch_add(1, Ordering::Relaxed);
}

/// Total accepted inbound SCION connections.
pub fn scion_inbound_total() -> u64 {
    SCION_INBOUND_ACCEPTED.load(Ordering::Relaxed)
}

/// Total successful outbound SCION connections.
pub fn scion_outbound_total() -> u64 {
    SCION_OUTBOUND_SUCCESSES.load(Ordering::Relaxed)
}

/// Returns the number of connections rejected by the per‑IP accept throttle.
pub fn accept_throttled_count() -> u64 {
    ACCEPT_THROTTLED.load(Ordering::Relaxed)
}

/// Returns the number of accept throttle bucket evictions (idle or capacity).
pub fn accept_bucket_evictions_count() -> u64 {
    ACCEPT_BUCKET_EVICTIONS.load(Ordering::Relaxed)
}

/// Returns the current count of accept throttle buckets (prefix + per-IP).
pub fn accept_bucket_count() -> u64 {
    ACCEPT_BUCKETS_CURRENT.load(Ordering::Relaxed)
}

/// Returns the number of prefix throttle cache hits.
pub fn accept_prefix_hits_count() -> u64 {
    ACCEPT_PREFIX_CACHE_HITS.load(Ordering::Relaxed)
}

/// Returns the number of prefix throttle cache misses.
pub fn accept_prefix_misses_count() -> u64 {
    ACCEPT_PREFIX_CACHE_MISSES.load(Ordering::Relaxed)
}

/// Returns the number of connections allowed by the prefix throttle bucket.
pub fn accept_prefix_allowed_count() -> u64 {
    ACCEPT_PREFIX_ALLOWED.load(Ordering::Relaxed)
}

/// Returns the number of prefix throttle rejections.
pub fn accept_prefix_throttled_count() -> u64 {
    ACCEPT_PREFIX_THROTTLED.load(Ordering::Relaxed)
}

/// Returns the number of connections allowed by the per-IP throttle bucket.
pub fn accept_ip_allowed_count() -> u64 {
    ACCEPT_IP_ALLOWED.load(Ordering::Relaxed)
}

/// Returns the number of per-IP throttle rejections.
pub fn accept_ip_throttled_count() -> u64 {
    ACCEPT_IP_THROTTLED.load(Ordering::Relaxed)
}

/// Returns the number of connections rejected by the incoming cap.
pub fn incoming_cap_reject_count() -> u64 {
    INCOMING_CAP_REJECTS.load(Ordering::Relaxed)
}

/// Returns the number of connections rejected by the total connections cap.
pub fn total_cap_reject_count() -> u64 {
    TOTAL_CAP_REJECTS.load(Ordering::Relaxed)
}

/// Returns the number of low-priority post messages dropped by per-peer throttle.
pub fn low_post_throttled_count() -> u64 {
    LOW_THROTTLED_POSTS.load(Ordering::Relaxed)
}

/// Returns the number of low-priority broadcast deliveries skipped by per-peer throttle.
pub fn low_broadcast_throttled_count() -> u64 {
    LOW_THROTTLED_BROADCASTS.load(Ordering::Relaxed)
}

/// Returns the number of trust-gossip frames skipped because the capability is disabled.
pub fn trust_gossip_skipped_capability_off_count() -> u64 {
    TRUST_GOSSIP_SKIPPED_CAP_OFF.load(Ordering::Relaxed)
}

/// Returns true when trust gossip is permitted for the given topic/capability flag.
fn trust_gossip_allowed(topic: message::Topic, trust_gossip: bool) -> bool {
    !matches!(topic, message::Topic::TrustGossip) || trust_gossip
}

fn inc_subscriber_queue_full_for(topic: message::Topic) -> u64 {
    let total = SUBSCRIBER_QUEUE_FULL.fetch_add(1, Ordering::Relaxed) + 1;
    match topic {
        message::Topic::Consensus | message::Topic::ConsensusPayload => {
            SUBSCRIBER_QUEUE_FULL_CONSENSUS.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::ConsensusChunk => {
            SUBSCRIBER_QUEUE_FULL_CONSENSUS_CHUNK.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Control => {
            SUBSCRIBER_QUEUE_FULL_CONTROL.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::BlockSync => {
            SUBSCRIBER_QUEUE_FULL_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
            SUBSCRIBER_QUEUE_FULL_TX_GOSSIP.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::PeerGossip | message::Topic::TrustGossip => {
            SUBSCRIBER_QUEUE_FULL_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Health => {
            SUBSCRIBER_QUEUE_FULL_HEALTH.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Other => {
            SUBSCRIBER_QUEUE_FULL_OTHER.fetch_add(1, Ordering::Relaxed);
        }
    }
    total
}

fn inc_subscriber_unrouted_for(topic: message::Topic) -> u64 {
    let total = SUBSCRIBER_UNROUTED.fetch_add(1, Ordering::Relaxed) + 1;
    match topic {
        message::Topic::Consensus | message::Topic::ConsensusPayload => {
            SUBSCRIBER_UNROUTED_CONSENSUS.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::ConsensusChunk => {
            SUBSCRIBER_UNROUTED_CONSENSUS_CHUNK.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Control => {
            SUBSCRIBER_UNROUTED_CONTROL.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::BlockSync => {
            SUBSCRIBER_UNROUTED_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
            SUBSCRIBER_UNROUTED_TX_GOSSIP.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::PeerGossip | message::Topic::TrustGossip => {
            SUBSCRIBER_UNROUTED_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Health => {
            SUBSCRIBER_UNROUTED_HEALTH.fetch_add(1, Ordering::Relaxed);
        }
        message::Topic::Other => {
            SUBSCRIBER_UNROUTED_OTHER.fetch_add(1, Ordering::Relaxed);
        }
    }
    total
}

/// Returns the number of per-peer post channel overflows observed.
pub fn post_overflow_count() -> u64 {
    POST_OVERFLOWS.load(Ordering::Relaxed)
}

fn inc_trust_gossip_skipped(direction: &'static str, reason: &'static str) {
    use std::sync::atomic::Ordering::Relaxed;

    TRUST_GOSSIP_SKIPPED_CAP_OFF.fetch_add(1, Relaxed);
    #[cfg(feature = "telemetry")]
    {
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics
                .p2p_trust_gossip_skipped_total
                .with_label_values(&[direction, reason])
                .inc();
        }
    }
    iroha_logger::trace!(direction, reason, "trust gossip message skipped");
}

fn inc_post_overflow_for(topic: message::Topic) {
    match topic {
        message::Topic::Consensus | message::Topic::ConsensusPayload => {
            POST_OVERFLOWS_CONSENSUS.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::ConsensusChunk | message::Topic::BlockSync => {
            POST_OVERFLOWS_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::Control => POST_OVERFLOWS_CONTROL.fetch_add(1, Ordering::Relaxed),
        message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
            POST_OVERFLOWS_TX_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::PeerGossip | message::Topic::TrustGossip => {
            POST_OVERFLOWS_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::Health => POST_OVERFLOWS_HEALTH.fetch_add(1, Ordering::Relaxed),
        message::Topic::Other => POST_OVERFLOWS_OTHER.fetch_add(1, Ordering::Relaxed),
    };
}

fn inc_post_overflow_for_prio(topic: message::Topic, high: bool) {
    match (high, topic) {
        (true, message::Topic::Consensus | message::Topic::ConsensusPayload) => {
            POST_OVERFLOWS_HI_CONSENSUS.fetch_add(1, Ordering::Relaxed)
        }
        (true, message::Topic::ConsensusChunk | message::Topic::BlockSync) => {
            POST_OVERFLOWS_HI_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed)
        }
        (true, message::Topic::Control) => {
            POST_OVERFLOWS_HI_CONTROL.fetch_add(1, Ordering::Relaxed)
        }
        (true, message::Topic::TxGossip | message::Topic::TxGossipRestricted) => {
            POST_OVERFLOWS_HI_TX_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        (true, message::Topic::PeerGossip | message::Topic::TrustGossip) => {
            POST_OVERFLOWS_HI_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        (true, message::Topic::Health) => POST_OVERFLOWS_HI_HEALTH.fetch_add(1, Ordering::Relaxed),
        (true, message::Topic::Other) => POST_OVERFLOWS_HI_OTHER.fetch_add(1, Ordering::Relaxed),
        (false, message::Topic::Consensus | message::Topic::ConsensusPayload) => {
            POST_OVERFLOWS_LO_CONSENSUS.fetch_add(1, Ordering::Relaxed)
        }
        (false, message::Topic::ConsensusChunk | message::Topic::BlockSync) => {
            POST_OVERFLOWS_LO_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed)
        }
        (false, message::Topic::Control) => {
            POST_OVERFLOWS_LO_CONTROL.fetch_add(1, Ordering::Relaxed)
        }
        (false, message::Topic::TxGossip | message::Topic::TxGossipRestricted) => {
            POST_OVERFLOWS_LO_TX_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        (false, message::Topic::PeerGossip | message::Topic::TrustGossip) => {
            POST_OVERFLOWS_LO_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        (false, message::Topic::Health) => POST_OVERFLOWS_LO_HEALTH.fetch_add(1, Ordering::Relaxed),
        (false, message::Topic::Other) => POST_OVERFLOWS_LO_OTHER.fetch_add(1, Ordering::Relaxed),
    };
}

/// Count of post channel overflows for topic Consensus.
pub fn post_overflow_consensus_count() -> u64 {
    POST_OVERFLOWS_CONSENSUS.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic Control.
pub fn post_overflow_control_count() -> u64 {
    POST_OVERFLOWS_CONTROL.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic `BlockSync`.
pub fn post_overflow_block_sync_count() -> u64 {
    POST_OVERFLOWS_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic `TxGossip`.
pub fn post_overflow_tx_gossip_count() -> u64 {
    POST_OVERFLOWS_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic `PeerGossip`.
pub fn post_overflow_peer_gossip_count() -> u64 {
    POST_OVERFLOWS_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic Health.
pub fn post_overflow_health_count() -> u64 {
    POST_OVERFLOWS_HEALTH.load(Ordering::Relaxed)
}
/// Count of post channel overflows for topic Other.
pub fn post_overflow_other_count() -> u64 {
    POST_OVERFLOWS_OTHER.load(Ordering::Relaxed)
}

fn inc_cap_violation(topic: message::Topic) {
    match topic {
        message::Topic::Consensus | message::Topic::ConsensusPayload => {
            CAP_VIOL_CONSENSUS.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::ConsensusChunk | message::Topic::BlockSync => {
            CAP_VIOL_BLOCK_SYNC.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::Control => CAP_VIOL_CONTROL.fetch_add(1, Ordering::Relaxed),
        message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
            CAP_VIOL_TX_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::PeerGossip | message::Topic::TrustGossip => {
            CAP_VIOL_PEER_GOSSIP.fetch_add(1, Ordering::Relaxed)
        }
        message::Topic::Health => CAP_VIOL_HEALTH.fetch_add(1, Ordering::Relaxed),
        message::Topic::Other => CAP_VIOL_OTHER.fetch_add(1, Ordering::Relaxed),
    };
}

/// Total number of dropped inbound messages exceeding the Consensus topic cap.
pub fn cap_violations_consensus() -> u64 {
    CAP_VIOL_CONSENSUS.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the Control topic cap.
pub fn cap_violations_control() -> u64 {
    CAP_VIOL_CONTROL.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the `BlockSync` topic cap.
pub fn cap_violations_block_sync() -> u64 {
    CAP_VIOL_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the `TxGossip` topic cap.
pub fn cap_violations_tx_gossip() -> u64 {
    CAP_VIOL_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the `PeerGossip` topic cap.
pub fn cap_violations_peer_gossip() -> u64 {
    CAP_VIOL_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the Health topic cap.
pub fn cap_violations_health() -> u64 {
    CAP_VIOL_HEALTH.load(Ordering::Relaxed)
}
/// Total number of dropped inbound messages exceeding the Other topic cap.
pub fn cap_violations_other() -> u64 {
    CAP_VIOL_OTHER.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic Consensus.
pub fn post_overflow_consensus_high_count() -> u64 {
    POST_OVERFLOWS_HI_CONSENSUS.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic Control.
pub fn post_overflow_control_high_count() -> u64 {
    POST_OVERFLOWS_HI_CONTROL.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic `BlockSync`.
pub fn post_overflow_block_sync_high_count() -> u64 {
    POST_OVERFLOWS_HI_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic `TxGossip`.
pub fn post_overflow_tx_gossip_high_count() -> u64 {
    POST_OVERFLOWS_HI_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic `PeerGossip`.
pub fn post_overflow_peer_gossip_high_count() -> u64 {
    POST_OVERFLOWS_HI_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic Health.
pub fn post_overflow_health_high_count() -> u64 {
    POST_OVERFLOWS_HI_HEALTH.load(Ordering::Relaxed)
}
/// Count of High-priority post overflows for topic Other.
pub fn post_overflow_other_high_count() -> u64 {
    POST_OVERFLOWS_HI_OTHER.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic Consensus.
pub fn post_overflow_consensus_low_count() -> u64 {
    POST_OVERFLOWS_LO_CONSENSUS.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic Control.
pub fn post_overflow_control_low_count() -> u64 {
    POST_OVERFLOWS_LO_CONTROL.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic `BlockSync`.
pub fn post_overflow_block_sync_low_count() -> u64 {
    POST_OVERFLOWS_LO_BLOCK_SYNC.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic `TxGossip`.
pub fn post_overflow_tx_gossip_low_count() -> u64 {
    POST_OVERFLOWS_LO_TX_GOSSIP.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic `PeerGossip`.
pub fn post_overflow_peer_gossip_low_count() -> u64 {
    POST_OVERFLOWS_LO_PEER_GOSSIP.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic Health.
pub fn post_overflow_health_low_count() -> u64 {
    POST_OVERFLOWS_LO_HEALTH.load(Ordering::Relaxed)
}
/// Count of Low-priority post overflows for topic Other.
pub fn post_overflow_other_low_count() -> u64 {
    POST_OVERFLOWS_LO_OTHER.load(Ordering::Relaxed)
}

/// Testing helper: increment per-topic/per-priority overflow counters directly.
/// Increments overall total as well.
pub fn inc_post_overflow_for_test(priority_high: bool, topic: message::Topic, n: u64) {
    use std::sync::atomic::Ordering::Relaxed;
    POST_OVERFLOWS.fetch_add(n, Relaxed);
    for _ in 0..n {
        inc_post_overflow_for(topic);
        inc_post_overflow_for_prio(topic, priority_high);
    }
}

// LogSampler is provided by crate::sampler

/// A coarse bucket key for IPs used for throttling.
/// The prefix length used to derive the bucket is stored alongside the masked address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct IpBucketKey([u8; 16], u8);

fn ip_bucket_key(ip: std::net::IpAddr, prefix_v4_bits: u8, prefix_v6_bits: u8) -> IpBucketKey {
    match ip {
        IpAddr::V4(v4) => {
            let bits = prefix_v4_bits.min(32);
            let raw = u32::from_be_bytes(v4.octets());
            let masked = if bits == 0 {
                0
            } else {
                raw & (!0u32 << (32 - bits))
            };
            let mut key = [0u8; 16];
            key[..4].copy_from_slice(&masked.to_be_bytes());
            IpBucketKey(key, bits)
        }
        IpAddr::V6(v6) => {
            let bits = prefix_v6_bits.min(128);
            let raw = u128::from_be_bytes(v6.octets());
            let masked = if bits == 0 {
                0
            } else {
                raw & (!0u128 << (128 - bits))
            };
            IpBucketKey(masked.to_be_bytes(), bits)
        }
    }
}

/// Simple token-bucket limiter for accepts.
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: tokio::time::Instant,
    rate_per_sec: f64,
    burst: f64,
}

impl TokenBucket {
    fn new(rate_per_sec: f64, burst: f64) -> Self {
        let now = tokio::time::Instant::now();
        let tokens = burst;
        Self {
            tokens,
            last_refill: now,
            rate_per_sec,
            burst,
        }
    }

    fn allow(&mut self) -> bool {
        self.allow_at(tokio::time::Instant::now())
    }

    fn allow_at(&mut self, now: tokio::time::Instant) -> bool {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        self.last_refill = now;
        self.tokens = elapsed
            .mul_add(self.rate_per_sec, self.tokens)
            .min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Allow consuming an arbitrary amount from the bucket.
    fn allow_n(&mut self, amount: f64) -> bool {
        self.allow_n_at(amount, tokio::time::Instant::now())
    }

    /// Allow consuming an arbitrary amount from the bucket at a specific instant.
    fn allow_n_at(&mut self, amount: f64, now: tokio::time::Instant) -> bool {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        self.last_refill = now;
        self.tokens = elapsed
            .mul_add(self.rate_per_sec, self.tokens)
            .min(self.burst);
        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    fn update_shape(&mut self, rate_per_sec: f64, burst: f64) {
        self.rate_per_sec = rate_per_sec;
        self.burst = burst;
        self.tokens = self.tokens.min(self.burst);
    }
}

#[derive(Debug, Clone)]
struct AcceptBucket {
    bucket: TokenBucket,
    last_seen: tokio::time::Instant,
}

impl AcceptBucket {
    fn new(rate_per_sec: f64, burst: f64, now: tokio::time::Instant) -> Self {
        Self {
            bucket: TokenBucket::new(rate_per_sec, burst),
            last_seen: now,
        }
    }

    fn allow(&mut self, now: tokio::time::Instant) -> bool {
        self.last_seen = now;
        self.bucket.allow_at(now)
    }

    fn update_shape(&mut self, rate_per_sec: f64, burst: f64) {
        self.bucket.update_shape(rate_per_sec, burst);
    }
}

#[derive(Clone, Copy, Debug)]
struct AcceptThrottleParams {
    prefix_rate_per_sec: Option<f64>,
    prefix_burst: Option<f64>,
    prefix_v4_bits: u8,
    prefix_v6_bits: u8,
    ip_rate_per_sec: Option<f64>,
    ip_burst: Option<f64>,
    max_buckets: usize,
    bucket_idle: Duration,
}

impl AcceptThrottleParams {
    fn new(
        prefix_rate_per_sec: Option<f64>,
        prefix_burst: Option<f64>,
        prefix_v4_bits: u8,
        prefix_v6_bits: u8,
        ip_rate_per_sec: Option<f64>,
        ip_burst: Option<f64>,
        max_buckets: usize,
        bucket_idle: Duration,
    ) -> Self {
        Self {
            prefix_rate_per_sec,
            prefix_burst,
            prefix_v4_bits,
            prefix_v6_bits,
            ip_rate_per_sec,
            ip_burst,
            max_buckets: max_buckets.max(1),
            bucket_idle,
        }
    }
}

fn evict_oldest_bucket(map: &mut HashMap<IpBucketKey, AcceptBucket>) -> bool {
    let Some(oldest_key) = map
        .iter()
        .min_by_key(|(_, entry)| entry.last_seen)
        .map(|(k, _)| *k)
    else {
        return false;
    };
    map.remove(&oldest_key);
    true
}

fn prune_accept_buckets(
    buckets: &mut HashMap<IpBucketKey, AcceptBucket>,
    max_buckets: usize,
    bucket_idle: Duration,
    now: tokio::time::Instant,
) -> usize {
    let mut evicted = 0;
    if bucket_idle != Duration::ZERO {
        let before = buckets.len();
        buckets.retain(|_, entry| now.saturating_duration_since(entry.last_seen) < bucket_idle);
        evicted += before.saturating_sub(buckets.len());
    }
    while buckets.len() > max_buckets {
        if !evict_oldest_bucket(buckets) {
            break;
        }
        evicted += 1;
    }
    evicted
}

fn consume_accept_bucket(
    buckets: &mut HashMap<IpBucketKey, AcceptBucket>,
    key: IpBucketKey,
    rate_per_sec: f64,
    burst: f64,
    params: &AcceptThrottleParams,
    now: tokio::time::Instant,
) -> (bool, bool, usize) {
    let existed = buckets.contains_key(&key);
    let mut evicted = 0;
    if !existed && buckets.len() >= params.max_buckets && evict_oldest_bucket(buckets) {
        evicted += 1;
    }
    let entry = buckets
        .entry(key)
        .or_insert_with(|| AcceptBucket::new(rate_per_sec, burst, now));
    entry.update_shape(rate_per_sec, burst);
    (entry.allow(now), existed, evicted)
}

fn update_accept_bucket_gauge(
    prefix_buckets: &HashMap<IpBucketKey, AcceptBucket>,
    ip_buckets: &HashMap<IpBucketKey, AcceptBucket>,
) {
    ACCEPT_BUCKETS_CURRENT.store(
        (prefix_buckets.len() + ip_buckets.len()) as u64,
        Ordering::Relaxed,
    );
}

/// Evaluate IP gating rules and accept throttle buckets (prefix then per-IP).
fn allow_ip_with_policy(
    allow_nets: &[IpNet],
    deny_nets: &[IpNet],
    allowlist_only: bool,
    params: AcceptThrottleParams,
    prefix_buckets: &mut HashMap<IpBucketKey, AcceptBucket>,
    ip_buckets: &mut HashMap<IpBucketKey, AcceptBucket>,
    ip: std::net::IpAddr,
) -> bool {
    if cidr_contains(deny_nets, ip) {
        return false;
    }
    let allowlist_present = !allow_nets.is_empty();
    let allowlisted = cidr_contains(allow_nets, ip);
    if allowlist_present && !allowlisted {
        return false;
    }

    let now = tokio::time::Instant::now();
    let evicted = prune_accept_buckets(prefix_buckets, params.max_buckets, params.bucket_idle, now)
        + prune_accept_buckets(ip_buckets, params.max_buckets, params.bucket_idle, now);
    if evicted > 0 {
        ACCEPT_BUCKET_EVICTIONS.fetch_add(evicted as u64, Ordering::Relaxed);
    }

    if allowlisted {
        update_accept_bucket_gauge(prefix_buckets, ip_buckets);
        return true;
    }
    if allowlist_only && allowlist_present {
        update_accept_bucket_gauge(prefix_buckets, ip_buckets);
        return true;
    }

    if let Some(rate) = params.prefix_rate_per_sec {
        let burst = params.prefix_burst.unwrap_or_else(|| rate.max(1.0));
        let (allow, existed, evicted) = consume_accept_bucket(
            prefix_buckets,
            ip_bucket_key(ip, params.prefix_v4_bits, params.prefix_v6_bits),
            rate,
            burst,
            &params,
            now,
        );
        if evicted > 0 {
            ACCEPT_BUCKET_EVICTIONS.fetch_add(evicted as u64, Ordering::Relaxed);
        }
        if existed {
            ACCEPT_PREFIX_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        } else {
            ACCEPT_PREFIX_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        }
        if allow {
            ACCEPT_PREFIX_ALLOWED.fetch_add(1, Ordering::Relaxed);
        } else {
            ACCEPT_PREFIX_THROTTLED.fetch_add(1, Ordering::Relaxed);
            ACCEPT_THROTTLED.fetch_add(1, Ordering::Relaxed);
            update_accept_bucket_gauge(prefix_buckets, ip_buckets);
            return false;
        }
    } else {
        ACCEPT_PREFIX_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    }

    if let Some(rate) = params.ip_rate_per_sec {
        let burst = params.ip_burst.unwrap_or_else(|| rate.max(1.0));
        let (allow, _, evicted) = consume_accept_bucket(
            ip_buckets,
            ip_bucket_key(ip, 32, 128),
            rate,
            burst,
            &params,
            now,
        );
        if evicted > 0 {
            ACCEPT_BUCKET_EVICTIONS.fetch_add(evicted as u64, Ordering::Relaxed);
        }
        if allow {
            ACCEPT_IP_ALLOWED.fetch_add(1, Ordering::Relaxed);
        } else {
            ACCEPT_IP_THROTTLED.fetch_add(1, Ordering::Relaxed);
            ACCEPT_THROTTLED.fetch_add(1, Ordering::Relaxed);
            update_accept_bucket_gauge(prefix_buckets, ip_buckets);
            return false;
        }
    }

    update_accept_bucket_gauge(prefix_buckets, ip_buckets);
    true
}

/// Filter for peer-message subscriptions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubscriberFilter {
    /// Receive every incoming message.
    All,
    /// Receive messages whose topic matches one of the listed entries.
    Topics(Vec<message::Topic>),
}

impl SubscriberFilter {
    /// Build a filter from an iterable of topics.
    pub fn topics<I>(topics: I) -> Self
    where
        I: IntoIterator<Item = message::Topic>,
    {
        Self::Topics(topics.into_iter().collect())
    }

    fn matches(&self, topic: message::Topic) -> bool {
        match self {
            Self::All => true,
            Self::Topics(topics) => topics.iter().any(|t| *t == topic),
        }
    }
}

#[derive(Clone)]
struct Subscriber<T: Pload> {
    sender: mpsc::Sender<PeerMessage<T>>,
    filter: SubscriberFilter,
}

/// `NetworkBase` actor handle.
// NOTE: high/low network queues are now bounded by configuration to prevent
// memory blow-ups. Separate channels for consensus/control versus
// gossip/sync traffic reduce head-of-line blocking and provide coarse
// prioritisation.
#[derive(derive_more::Debug)]
#[debug("core::any::type_name::<Self>()")]
pub struct NetworkBaseHandle<T: Pload, K: Kex, E: Enc> {
    /// Sender to subscribe for messages received form other peers in the network
    subscribe_to_peers_messages_sender: mpsc::UnboundedSender<Subscriber<T>>,
    /// Receiver of `OnlinePeer` message
    online_peers_receiver: watch::Receiver<OnlinePeers>,
    /// Receiver of online peer transport capabilities.
    online_peer_capabilities_receiver: watch::Receiver<message::OnlinePeerCapabilities>,
    /// [`UpdateTopology`] message sender
    update_topology_sender: mpsc::UnboundedSender<UpdateTopology>,
    /// [`UpdatePeers`] message sender
    update_peers_sender: mpsc::UnboundedSender<UpdatePeers>,
    /// [`UpdatePeerCapabilities`] message sender
    update_peer_capabilities_sender: mpsc::UnboundedSender<message::UpdatePeerCapabilities>,
    /// Trusted peers update sender
    update_trusted_peers_sender: mpsc::UnboundedSender<UpdateTrustedPeers>,
    /// [`UpdateAcl`] message sender
    update_acl_sender: mpsc::UnboundedSender<message::UpdateAcl>,
    /// [`UpdateHandshake`] message sender
    update_handshake_sender: mpsc::UnboundedSender<message::UpdateHandshake>,
    /// [`UpdateConsensusCaps`] message sender
    update_consensus_caps_sender: mpsc::UnboundedSender<message::UpdateConsensusCaps>,
    /// Service channel into the network actor (for inbound accept helpers)
    service_message_sender: mpsc::Sender<ServiceMessage<WireMessage<T>>>,
    /// Sender of high priority messages
    network_message_high_sender: net_channel::Sender<NetworkMessage<T>>,
    /// Sender of low priority messages
    network_message_low_sender: net_channel::Sender<NetworkMessage<T>>,
    /// Configured capacity for subscriber queues.
    subscriber_queue_cap: core::num::NonZeroUsize,
    /// Key exchange used by network
    _key_exchange: core::marker::PhantomData<K>,
    /// Encryptor used by the network
    _encryptor: core::marker::PhantomData<E>,
}

impl<T: Pload, K: Kex, E: Enc> Clone for NetworkBaseHandle<T, K, E> {
    fn clone(&self) -> Self {
        Self {
            subscribe_to_peers_messages_sender: self.subscribe_to_peers_messages_sender.clone(),
            online_peers_receiver: self.online_peers_receiver.clone(),
            online_peer_capabilities_receiver: self.online_peer_capabilities_receiver.clone(),
            update_topology_sender: self.update_topology_sender.clone(),
            update_peers_sender: self.update_peers_sender.clone(),
            update_peer_capabilities_sender: self.update_peer_capabilities_sender.clone(),
            update_trusted_peers_sender: self.update_trusted_peers_sender.clone(),
            update_acl_sender: self.update_acl_sender.clone(),
            update_handshake_sender: self.update_handshake_sender.clone(),
            update_consensus_caps_sender: self.update_consensus_caps_sender.clone(),
            service_message_sender: self.service_message_sender.clone(),
            network_message_high_sender: self.network_message_high_sender.clone(),
            network_message_low_sender: self.network_message_low_sender.clone(),
            subscriber_queue_cap: self.subscriber_queue_cap,
            _key_exchange: core::marker::PhantomData::<K>,
            _encryptor: core::marker::PhantomData::<E>,
        }
    }
}

impl<T: Pload + message::ClassifyTopic, K: Kex + Sync, E: Enc + Sync> NetworkBaseHandle<T, K, E> {
    /// Start network peer and return handle to it
    ///
    /// # Errors
    /// - If binding to address fail
    #[log(skip(key_pair, shutdown_signal))]
    pub async fn start(
        key_pair: KeyPair,
        config: Config,
        chain_id: Option<ChainId>,
        consensus_caps: Option<crate::ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        shutdown_signal: ShutdownSignal,
    ) -> Result<(Self, Child), Error> {
        Self::start_with_crypto(
            key_pair,
            config,
            chain_id,
            consensus_caps,
            confidential_caps,
            None,
            shutdown_signal,
        )
        .await
    }

    /// Construct a closed network handle for tests that cannot bind sockets.
    ///
    /// The returned handle drops all outgoing messages and reports no online peers.
    #[must_use]
    pub fn closed_for_tests() -> Self {
        let (subscribe_tx, _subscribe_rx) = mpsc::unbounded_channel::<Subscriber<T>>();
        let (update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<message::UpdateTopology>();
        let (update_peers_tx, update_peers_rx) = mpsc::unbounded_channel::<message::UpdatePeers>();
        let (update_peer_capabilities_tx, update_peer_capabilities_rx) =
            mpsc::unbounded_channel::<message::UpdatePeerCapabilities>();
        let (update_trusted_tx, update_trusted_rx) =
            mpsc::unbounded_channel::<message::UpdateTrustedPeers>();
        let (update_acl_tx, update_acl_rx) = mpsc::unbounded_channel::<message::UpdateAcl>();
        let (update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<message::UpdateHandshake>();
        let (update_consensus_caps_tx, update_consensus_caps_rx) =
            mpsc::unbounded_channel::<message::UpdateConsensusCaps>();
        let (service_message_tx, _service_message_rx) =
            mpsc::channel::<ServiceMessage<WireMessage<T>>>(1);
        let (network_message_high_sender, _network_message_high_rx) =
            net_channel::channel_with_capacity(1);
        let (network_message_low_sender, _network_message_low_rx) =
            net_channel::channel_with_capacity(1);
        let (_online_peers_tx, online_peers_receiver) = watch::channel(HashSet::new());
        let (_online_peer_capabilities_tx, online_peer_capabilities_receiver) =
            watch::channel(HashMap::new());

        drop(update_topology_rx);
        drop(update_peers_rx);
        drop(update_peer_capabilities_rx);
        drop(update_trusted_rx);
        drop(update_acl_rx);
        drop(update_handshake_rx);
        drop(update_consensus_caps_rx);

        Self {
            subscribe_to_peers_messages_sender: subscribe_tx,
            online_peers_receiver,
            online_peer_capabilities_receiver,
            update_topology_sender: update_topology_tx,
            update_peers_sender: update_peers_tx,
            update_peer_capabilities_sender: update_peer_capabilities_tx,
            update_trusted_peers_sender: update_trusted_tx,
            update_acl_sender: update_acl_tx,
            update_handshake_sender: update_handshake_tx,
            update_consensus_caps_sender: update_consensus_caps_tx,
            service_message_sender: service_message_tx,
            network_message_high_sender,
            network_message_low_sender,
            subscriber_queue_cap: core::num::NonZeroUsize::new(1).expect("nonzero"),
            _key_exchange: core::marker::PhantomData::<K>,
            _encryptor: core::marker::PhantomData::<E>,
        }
    }

    /// Launch the P2P runtime with pluggable Noise/TLS capability overrides.
    ///
    /// Use this entrypoint when tests or specialised deployments need to force
    /// specific handshake capabilities (e.g., consensus/torii lanes, confidential
    /// transport) instead of relying on the defaults wired through [`Config`].
    /// The returned handle lets callers stream peer events, publish network
    /// messages, and coordinate shutdown for the spawned reactor.
    ///
    /// # Errors
    ///
    /// Returns an error if the listener cannot bind to the requested address, if
    /// the crypto handshake fails during bootstrap, or if the reactor tasks fail
    /// to initialise (for example, due to TLS key/cert issues or capability
    /// mismatches).
    #[log(skip(key_pair, shutdown_signal))]
    #[allow(clippy::too_many_lines, clippy::used_underscore_binding)]
    pub async fn start_with_crypto(
        key_pair: KeyPair,
        Config {
            address: listen_addr,
            public_address,
            relay_mode,
            relay_hub_addresses,
            relay_ttl,
            soranet_handshake,
            idle_timeout,
            connect_startup_delay,
            dial_timeout,
            deferred_send_ttl,
            deferred_send_max_per_peer,
            peer_gossip_period,
            trust_gossip,
            quic_enabled,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
            quic_datagram_receive_buffer_bytes,
            quic_datagram_send_buffer_bytes,
            scion,
            tls_enabled,
            tls_fallback_to_plain,
            prefer_ws_fallback,
            p2p_queue_cap_high,
            p2p_queue_cap_low,
            p2p_post_queue_cap,
            p2p_subscriber_queue_cap,
            dns_refresh_interval,
            dns_refresh_ttl,
            p2p_proxy,
            p2p_proxy_required,
            p2p_no_proxy,
            p2p_proxy_tls_verify,
            p2p_proxy_tls_pinned_cert_der_base64,
            happy_eyeballs_stagger: config_happy_eyeballs_stagger,
            addr_ipv6_first,
            max_incoming,
            max_total_connections,
            accept_rate_per_ip_per_sec,
            accept_burst_per_ip,
            max_accept_buckets,
            accept_bucket_idle,
            accept_prefix_v4_bits,
            accept_prefix_v6_bits,
            accept_rate_per_prefix_per_sec,
            accept_burst_per_prefix,
            low_priority_rate_per_sec,
            low_priority_burst,
            low_priority_bytes_per_sec,
            low_priority_bytes_burst,
            tls_listen_address,
            tls_inbound_only,
            allowlist_only,
            allow_keys,
            deny_keys,
            allow_cidrs,
            deny_cidrs,
            disconnect_on_post_overflow,
            max_frame_bytes,
            max_frame_bytes_consensus,
            max_frame_bytes_control,
            max_frame_bytes_block_sync,
            max_frame_bytes_tx_gossip,
            max_frame_bytes_peer_gossip,
            max_frame_bytes_health,
            max_frame_bytes_other,
            tcp_nodelay,
            tcp_keepalive,
            tls_only_v1_3: _tls_only_v1_3,
            quic_max_idle_timeout,
            ..
        }: Config,
        // Optional ChainId used to bind handshake to chain (feature-gated by callers)
        chain_id: Option<ChainId>,
        // Optional consensus capabilities for handshake gating (mode/proto/fingerprint)
        consensus_caps: Option<crate::ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        shutdown_signal: ShutdownSignal,
    ) -> Result<(Self, Child), Error> {
        let relay_role = relay_role_from_mode(relay_mode);
        let relay_ttl = relay_ttl;
        let self_id = PeerId::from(key_pair.public_key().clone());
        let trust_gossip_config = trust_gossip;
        let trust_gossip = trust_gossip_config && soranet_handshake.trust_gossip;
        let soranet_runtime = runtime_from_handshake(soranet_handshake)?;
        let connect_startup_delay_until = tokio::time::Instant::now() + connect_startup_delay;

        let proxy_is_https = p2p_proxy
            .as_deref()
            .is_some_and(|proxy| proxy.trim_start().starts_with("https://"));

        if p2p_proxy_required {
            if p2p_proxy.is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.p2p_proxy_required=true but network.p2p_proxy is not set",
                )
                .into());
            }
            if !p2p_no_proxy.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.p2p_proxy_required=true is incompatible with network.p2p_no_proxy; remove no-proxy exemptions to enforce the proxy",
                )
                .into());
            }
            // QUIC uses UDP and bypasses the TCP proxy dialer entirely.
            if cfg!(feature = "quic") && quic_enabled {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.p2p_proxy_required=true is incompatible with network.quic_enabled=true (QUIC bypasses the proxy); set network.quic_enabled=false",
                )
                .into());
            }
        }

        if proxy_is_https {
            if !cfg!(feature = "p2p_tls") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.p2p_proxy uses https:// but this build was compiled without iroha_p2p/p2p_tls",
                )
                .into());
            }
            if p2p_proxy_tls_verify {
                let pin_present = p2p_proxy_tls_pinned_cert_der_base64
                    .as_deref()
                    .is_some_and(|raw| !raw.trim().is_empty());
                if !pin_present {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "network.p2p_proxy_tls_verify=true requires network.p2p_proxy_tls_pinned_cert_der_base64 to be set when using an https:// proxy",
                    )
                    .into());
                }
            }
        }

        if tls_inbound_only {
            if !tls_enabled {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.tls_inbound_only=true requires network.tls_enabled=true",
                )
                .into());
            }
            if !cfg!(feature = "p2p_tls") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.tls_inbound_only=true requires a build with iroha_p2p/p2p_tls",
                )
                .into());
            }
            // Ensure peers can still dial us by requiring the advertised port to match the TLS bind port.
            let tls_port = tls_listen_address
                .as_ref()
                .map_or_else(|| listen_addr.value().port(), |addr| addr.value().port());
            if tls_port != public_address.value().port() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "network.tls_inbound_only=true binds inbound TLS on port {tls_port}, but network.public_address advertises port {}; set public_address to the TLS port",
                        public_address.value().port()
                    ),
                )
                .into());
            }
        }

        if tls_enabled && !cfg!(feature = "p2p_tls") {
            if !tls_fallback_to_plain {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "network.tls_enabled=true with tls_fallback_to_plain=false requires a build with iroha_p2p/p2p_tls",
                )
                .into());
            }
            if tls_listen_address.is_some() {
                iroha_logger::warn!(
                    "network.tls_listen_address is set but this build lacks iroha_p2p/p2p_tls; inbound TLS listener will not start"
                );
            }
            iroha_logger::warn!(
                "network.tls_enabled=true but this build lacks iroha_p2p/p2p_tls; outbound dials will be plaintext because tls_fallback_to_plain=true"
            );
        }

        if scion.enabled || scion.listen_endpoint.is_some() || !scion.routes.is_empty() {
            iroha_logger::warn!(
                "network.scion_* settings are ignored; SCION selection is automatic via peer capabilities"
            );
        }
        let local_scion_supported = quic_enabled && cfg!(feature = "quic");

        let proxy_policy = crate::transport::ProxyPolicy::from_config(p2p_proxy, p2p_no_proxy)?;
        let proxy_tls_pinned_cert_der: Option<std::sync::Arc<[u8]>> =
            if let Some(raw) = p2p_proxy_tls_pinned_cert_der_base64.as_deref() {
                let raw = raw.trim();
                if raw.is_empty() {
                    None
                } else {
                    let bytes = BASE64_STANDARD.decode(raw.as_bytes()).map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("network.p2p_proxy_tls_pinned_cert_der_base64: {e}"),
                        )
                    })?;
                    Some(std::sync::Arc::<[u8]>::from(bytes))
                }
            } else {
                None
            };

        let quic_dialer: Option<crate::transport::QuicDialer> = {
            #[cfg(feature = "quic")]
            {
                if quic_enabled {
                    // Reuse a single UDP socket for all outbound QUIC dials.
                    match crate::transport::quic::Dialer::bind(
                        "0.0.0.0:0".parse().expect("valid bind addr"),
                        crate::transport::quic::DialerConfig {
                            max_idle_timeout: quic_max_idle_timeout,
                            datagram_receive_buffer: quic_datagrams_enabled
                                .then_some(quic_datagram_receive_buffer_bytes),
                            datagram_send_buffer: if quic_datagrams_enabled {
                                quic_datagram_send_buffer_bytes
                            } else {
                                0
                            },
                            ..Default::default()
                        },
                    ) {
                        Ok(dialer) => Some(dialer),
                        Err(e) => {
                            iroha_logger::warn!(
                                %e,
                                "Failed to bind QUIC dialer endpoint; outbound QUIC disabled"
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            }
            #[cfg(not(feature = "quic"))]
            {
                let _ = quic_enabled;
                let _ = quic_datagrams_enabled;
                let _ = quic_datagram_max_payload_bytes;
                let _ = quic_datagram_receive_buffer_bytes;
                let _ = quic_datagram_send_buffer_bytes;
                let _ = quic_max_idle_timeout;
                None
            }
        };
        // Bind TCP listener with improved diagnostics that include the configured address.
        let listen_addr_repr = format!("{listen_addr:?}");
        let public_addr_repr = format!("{public_address:?}");
        let listener = if tls_inbound_only {
            // Bind a dummy TCP listener so the network actor's select loop can keep the accept
            // branch without exposing a plaintext listener on the configured P2P port.
            let dummy: std::net::SocketAddr = "127.0.0.1:0".parse().expect("valid bind addr");
            let listener = TcpListener::bind(dummy).await?;
            iroha_logger::info!(
                "Network started without plain TCP listener (tls_inbound_only=true)"
            );
            listener
        } else {
            let addrs: Vec<std::net::SocketAddr> = match listen_addr.value().to_socket_addrs() {
                Ok(iter) => iter.collect(),
                Err(e) => {
                    let error = std::sync::Arc::new(e);
                    iroha_logger::error!(
                        listen_addr = ?listen_addr,
                        public_address = ?public_address,
                        error = %error,
                        "Failed to resolve TCP listener address"
                    );
                    return Err(Error::BindListener {
                        listen_addr: listen_addr_repr.clone(),
                        public_address: public_addr_repr.clone(),
                        error,
                    });
                }
            };
            let listener = match TcpListener::bind(addrs.as_slice()).await {
                Ok(l) => l,
                Err(e) => {
                    let error = std::sync::Arc::new(e);
                    iroha_logger::error!(
                        listen_addr = ?listen_addr,
                        public_address = ?public_address,
                        error = %error,
                        "Failed to bind TCP listener"
                    );
                    return Err(Error::BindListener {
                        listen_addr: listen_addr_repr.clone(),
                        public_address: public_addr_repr.clone(),
                        error,
                    });
                }
            };
            iroha_logger::info!("Network bound to listener");
            listener
        };
        let (online_peers_sender, online_peers_receiver) = watch::channel(HashSet::new());
        let (online_peer_capabilities_sender, online_peer_capabilities_receiver) =
            watch::channel(HashMap::new());
        let (subscribe_to_peers_messages_sender, subscribe_to_peers_messages_receiver) =
            mpsc::unbounded_channel();
        let (update_topology_sender, update_topology_receiver) = mpsc::unbounded_channel();
        let (update_peers_sender, update_peers_receiver) = mpsc::unbounded_channel();
        let (update_peer_capabilities_sender, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel();
        let (update_trusted_peers_sender, update_trusted_peers_receiver) =
            mpsc::unbounded_channel();
        let (update_acl_sender, update_acl_receiver) = mpsc::unbounded_channel();
        let (update_handshake_sender, update_handshake_receiver) = mpsc::unbounded_channel();
        let (update_consensus_caps_sender, update_consensus_caps_receiver) =
            mpsc::unbounded_channel();
        // Bounded queue capacities are supplied from node configuration so the
        // default build enforces backpressure without relying on feature flags.
        let (network_message_high_sender, network_message_high_receiver) =
            net_channel::channel_with_capacity(p2p_queue_cap_high.get());
        let (network_message_low_sender, network_message_low_receiver) =
            net_channel::channel_with_capacity(p2p_queue_cap_low.get());
        let (peer_message_high_sender, peer_message_high_receiver) =
            peer_message_channel::<T>(p2p_queue_cap_high);
        let (peer_message_low_sender, peer_message_low_receiver) =
            peer_message_channel::<T>(p2p_queue_cap_low);
        let (service_message_sender, service_message_receiver) =
            mpsc::channel::<ServiceMessage<WireMessage<T>>>(1);
        // Clone a handle for the returned NetworkBaseHandle before moving the sender into the actor.
        let service_message_sender_handle = service_message_sender.clone();
        #[cfg(feature = "quic")]
        if quic_enabled {
            // Start QUIC listener on the same address (UDP). This is optional and will accept
            // incoming connections and spawn peer actors for each bidirectional stream.
            if let Err(e) = start_quic_listener::<WireMessage<T>, K, E>(
                &listen_addr.value().to_socket_addrs()?.as_slice()[0],
                key_pair.clone(),
                public_address.value().clone(),
                service_message_sender.clone(),
                idle_timeout,
                quic_max_idle_timeout,
                quic_datagrams_enabled,
                quic_datagram_max_payload_bytes,
                quic_datagram_receive_buffer_bytes,
                quic_datagram_send_buffer_bytes,
                chain_id.clone(),
                consensus_caps.clone(),
                confidential_caps.clone(),
                crypto_caps.clone(),
                p2p_post_queue_cap.get(),
                trust_gossip,
                max_frame_bytes,
                soranet_runtime.clone(),
                local_scion_supported,
                relay_role,
            )
            .await
            {
                iroha_logger::warn!(%e, "Failed to start QUIC listener; continuing with TCP only");
            }
        }
        #[cfg(feature = "p2p_tls")]
        if tls_enabled {
            let tls_bind_addr = if tls_inbound_only {
                Some(
                    tls_listen_address
                        .as_ref()
                        .map(|x| x.value().clone())
                        .unwrap_or_else(|| listen_addr.value().clone()),
                )
            } else {
                tls_listen_address.as_ref().map(|x| x.value().clone())
            };
            if let Some(tls_addr) = tls_bind_addr {
                if let Err(e) = start_tls_listener::<WireMessage<T>, K, E>(
                    tls_addr.to_socket_addrs()?.as_slice()[0],
                    key_pair.clone(),
                    public_address.value().clone(),
                    service_message_sender.clone(),
                    idle_timeout,
                    chain_id.clone(),
                    consensus_caps.clone(),
                    confidential_caps.clone(),
                    crypto_caps.clone(),
                    p2p_post_queue_cap.get(),
                    trust_gossip,
                    max_frame_bytes,
                    quic_datagrams_enabled,
                    quic_datagram_max_payload_bytes,
                    soranet_runtime.clone(),
                    local_scion_supported,
                    relay_role,
                    tcp_nodelay,
                    tcp_keepalive,
                )
                .await
                {
                    if tls_inbound_only {
                        return Err(e);
                    }
                    iroha_logger::warn!(
                        %e,
                        addr=%tls_addr,
                        "Failed to start TLS listener; continuing without inbound TLS"
                    );
                }
            }
        }
        let accept_params = AcceptThrottleParams::new(
            accept_rate_per_prefix_per_sec
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            accept_burst_per_prefix
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            accept_prefix_v4_bits,
            accept_prefix_v6_bits,
            accept_rate_per_ip_per_sec
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            accept_burst_per_ip
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            max_accept_buckets.get(),
            accept_bucket_idle,
        );
        let network = NetworkBase {
            listen_addr: listen_addr.into_value(),
            public_address: public_address.into_value(),
            relay_role,
            relay_mode,
            relay_hub_addresses,
            relay_hub_peer: None,
            relay_trusted_peers: HashSet::new(),
            relay_ttl,
            trust_gossip_config,
            trust_gossip,
            self_id,
            address_book: HashMap::new(),
            peer_reputations: PeerReputationBook::default(),
            soranet_handshake: soranet_runtime.clone(),
            listener,
            peers: HashMap::new(),
            connecting_peers: HashMap::new(),
            key_pair,
            subscribers_to_peers_messages: Vec::new(),
            subscribe_to_peers_messages_receiver,
            online_peers_sender,
            online_peer_capabilities_sender,
            update_topology_receiver,
            update_peers_receiver,
            update_peer_capabilities_receiver,
            update_trusted_peers_receiver,
            update_acl_receiver,
            update_handshake_receiver,
            update_consensus_caps_receiver,
            network_message_high_receiver,
            network_message_low_receiver,
            peer_message_high_receiver,
            peer_message_low_receiver,
            peer_message_high_sender,
            peer_message_low_sender,
            service_message_receiver,
            service_message_sender,
            current_conn_id: 0,
            current_topology: HashSet::new(),
            current_peers_addresses: Vec::new(),
            idle_timeout,
            dial_timeout,
            connect_startup_delay_until,
            chain_id,
            consensus_caps,
            confidential_caps,
            crypto_caps,
            peer_capabilities: HashMap::new(),
            post_queue_cap: p2p_post_queue_cap.get(),
            max_frame_bytes,
            cap_consensus: max_frame_bytes_consensus,
            cap_control: max_frame_bytes_control,
            cap_block_sync: max_frame_bytes_block_sync,
            cap_tx_gossip: max_frame_bytes_tx_gossip,
            cap_peer_gossip: max_frame_bytes_peer_gossip,
            cap_health: max_frame_bytes_health,
            cap_other: max_frame_bytes_other,
            dns_refresh_interval,
            dns_refresh_ttl,
            dns_last_refresh: HashMap::new(),
            topology_update_interval: peer_gossip_period.max(Duration::from_millis(1)),
            dns_pending_refresh: HashSet::new(),
            quic_enabled,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
            local_scion_supported,
            tls_enabled,
            tls_fallback_to_plain,
            prefer_ws_fallback,
            proxy_policy,
            proxy_tls_verify: p2p_proxy_tls_verify,
            proxy_tls_pinned_cert_der,
            quic_dialer,
            allowlist_only,
            allow_keys: allow_keys.into_iter().collect(),
            deny_keys: deny_keys.into_iter().collect(),
            allow_nets: parse_cidrs(&allow_cidrs),
            deny_nets: parse_cidrs(&deny_cidrs),
            retry_backoff: HashMap::new(),
            peer_session_generation: HashMap::new(),
            pending_connects: Vec::new(),
            deferred_send_queue: DeferredPeerFrameQueue::new(
                deferred_send_max_per_peer,
                deferred_send_ttl,
            ),
            happy_eyeballs_stagger: config_happy_eyeballs_stagger,
            addr_ipv6_first,
            last_active: HashMap::new(),
            incoming_pending: HashSet::new(),
            incoming_active: HashSet::new(),
            max_incoming: max_incoming.map(core::num::NonZeroUsize::get),
            max_total_connections: max_total_connections.map(core::num::NonZeroUsize::get),
            accept_params,
            accept_prefix_buckets: HashMap::new(),
            accept_ip_buckets: HashMap::new(),
            sampler_high_queue_warn: LogSampler::new(),
            sampler_low_queue_warn: LogSampler::new(),
            sampler_accept_err: LogSampler::new(),
            tcp_nodelay,
            tcp_keepalive,
            low_rate_per_sec: low_priority_rate_per_sec
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            low_burst: low_priority_burst
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            low_buckets: HashMap::new(),
            low_bytes_per_sec: low_priority_bytes_per_sec
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            low_bytes_burst: low_priority_bytes_burst
                .map(core::num::NonZeroU32::get)
                .map(f64::from),
            low_bytes_buckets: HashMap::new(),
            disconnect_on_post_overflow,
            _key_exchange: core::marker::PhantomData::<K>,
            _encryptor: core::marker::PhantomData::<E>,
        };
        let child = Child::new(
            tokio::task::spawn(network.run(shutdown_signal)),
            OnShutdown::Wait(Duration::from_secs(5)),
        );
        Ok((
            Self {
                subscribe_to_peers_messages_sender,
                online_peers_receiver,
                online_peer_capabilities_receiver,
                update_topology_sender,
                update_peers_sender,
                update_peer_capabilities_sender,
                update_trusted_peers_sender,
                update_acl_sender,
                update_handshake_sender,
                update_consensus_caps_sender,
                // Use the pre-cloned sender since the original was moved into the actor state
                service_message_sender: service_message_sender_handle,
                network_message_high_sender,
                network_message_low_sender,
                subscriber_queue_cap: p2p_subscriber_queue_cap,
                _key_exchange: core::marker::PhantomData,
                _encryptor: core::marker::PhantomData,
            },
            child,
        ))
    }

    /// Accept an inbound stream from an external server (e.g., Torii `/p2p` WebSocket upgrade).
    ///
    /// Performs the same caps/throttle checks as the TCP listener via an internal
    /// `InboundAsk`, and if accepted, hands the provided stream halves to the
    /// network actor to spawn a `connected_from` peer.
    ///
    /// Returns `Ok(true)` if the stream was accepted and the peer task was spawned,
    /// `Ok(false)` if rejected by caps/throttle/ACLs, or an error if the network
    /// actor is unavailable.
    ///
    /// # Errors
    /// Returns an I/O error if the network actor is unavailable or drops the reply channel.
    pub async fn accept_stream<R, W>(
        &self,
        read: R,
        write: W,
        remote_addr: std::net::SocketAddr,
    ) -> Result<bool, Error>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        // Allocate a large-range connection id to avoid collisions with TCP/QUIC id spaces.
        static NEXT_EXT_CONN_ID: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
            std::sync::OnceLock::new();
        let id_alloc = NEXT_EXT_CONN_ID.get_or_init(|| std::sync::atomic::AtomicU64::new(1 << 58));
        let conn_id = id_alloc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Ask the network actor to apply caps/throttle (same as TCP/QUIC paths).
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ask = ServiceMessage::InboundAsk {
            conn_id,
            remote_addr,
            reply: tx,
        };
        // If sending fails, the network actor has shut down.
        self.service_message_sender
            .send(ask)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "network actor down"))?;
        let allow = rx.await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "network actor dropped reply")
        })?;
        if !allow {
            return Ok(false);
        }

        // Hand off the stream halves to the network actor to spawn the peer.
        // Insert a pending marker first to align with other inbound code paths.
        let _ = self
            .service_message_sender
            .send(ServiceMessage::InboundPending(conn_id))
            .await;
        let _ = self
            .service_message_sender
            .send(ServiceMessage::InboundStream {
                conn_id,
                read: Box::new(read),
                write: Box::new(write),
            })
            .await;
        Ok(true)
    }

    /// Subscribe to messages received from other peers in the network.
    ///
    /// Returns `Ok(())` when the registration succeeds. If the underlying
    /// network task has already shut down, the original sender is returned so
    /// the caller may retry or decide how to handle the failure without
    /// triggering a panic.
    ///
    /// # Errors
    ///
    /// Returns the supplied `sender` when the network actor has already
    /// terminated and cannot accept new subscriptions.
    ///
    /// The supplied [`SubscriberFilter`] limits which topics are delivered to
    /// the subscriber queue.
    pub fn subscribe_to_peers_messages_with_filter(
        &self,
        sender: mpsc::Sender<PeerMessage<T>>,
        filter: SubscriberFilter,
    ) -> Result<(), mpsc::Sender<PeerMessage<T>>> {
        let subscriber = Subscriber { sender, filter };
        self.subscribe_to_peers_messages_sender
            .send(subscriber)
            .map_err(|err| {
                iroha_logger::warn!(
                    "P2P subscriber registration failed because the network actor has already shut down"
                );
                err.0.sender
            })
    }

    /// Subscribe to messages received from other peers using the default filter.
    ///
    /// # Errors
    ///
    /// Returns the supplied `sender` when the network actor has already
    /// terminated and cannot accept new subscriptions.
    pub fn subscribe_to_peers_messages(
        &self,
        sender: mpsc::Sender<PeerMessage<T>>,
    ) -> Result<(), mpsc::Sender<PeerMessage<T>>> {
        self.subscribe_to_peers_messages_with_filter(sender, SubscriberFilter::All)
    }

    /// Configured capacity for P2P subscriber queues.
    #[must_use]
    pub fn subscriber_queue_cap(&self) -> core::num::NonZeroUsize {
        self.subscriber_queue_cap
    }

    /// Send [`Post<T>`] message on network actor.
    #[allow(clippy::needless_pass_by_value)]
    pub fn post(&self, msg: Post<T>) {
        use tokio::sync::mpsc::error::TrySendError;

        let priority = msg.priority;
        let sender = match priority {
            Priority::High => &self.network_message_high_sender,
            Priority::Low => &self.network_message_low_sender,
        };
        if let Err(e) = sender.try_send(NetworkMessage::Post(msg)) {
            match e {
                TrySendError::Full(_) => {
                    iroha_logger::warn!("Network message queue is full, dropping post");
                    DROPPED_POSTS.fetch_add(1, Ordering::Relaxed);
                    match priority {
                        Priority::High => DROPPED_POSTS_HI.fetch_add(1, Ordering::Relaxed),
                        Priority::Low => DROPPED_POSTS_LO.fetch_add(1, Ordering::Relaxed),
                    };
                }
                TrySendError::Closed(_) => {
                    iroha_logger::debug!("Network actor is closed, dropping post");
                }
            }
        }
    }

    /// Send [`Broadcast<T>`] message on network actor.
    #[allow(clippy::needless_pass_by_value)]
    pub fn broadcast(&self, msg: Broadcast<T>) {
        use tokio::sync::mpsc::error::TrySendError;

        let priority = msg.priority;
        let sender = match priority {
            Priority::High => &self.network_message_high_sender,
            Priority::Low => &self.network_message_low_sender,
        };
        if let Err(e) = sender.try_send(NetworkMessage::Broadcast(msg)) {
            match e {
                TrySendError::Full(_) => {
                    iroha_logger::warn!("Network message queue is full, dropping broadcast");
                    DROPPED_BROADCASTS.fetch_add(1, Ordering::Relaxed);
                    match priority {
                        Priority::High => DROPPED_BROADCASTS_HI.fetch_add(1, Ordering::Relaxed),
                        Priority::Low => DROPPED_BROADCASTS_LO.fetch_add(1, Ordering::Relaxed),
                    };
                }
                TrySendError::Closed(_) => {
                    iroha_logger::debug!("Network actor is closed, dropping broadcast");
                }
            }
        }
    }

    /// Send [`UpdateTopology`] message on network actor.
    pub fn update_topology(&self, topology: UpdateTopology) {
        if self.update_topology_sender.send(topology).is_err() {
            iroha_logger::debug!("Network actor is closed, dropping topology update");
        }
    }

    /// Send [`UpdatePeers`] message on network actor.
    pub fn update_peers_addresses(&self, peers: UpdatePeers) {
        if self.update_peers_sender.send(peers).is_err() {
            iroha_logger::debug!("Network actor is closed, dropping peers update");
        }
    }

    /// Send [`UpdatePeerCapabilities`] message on network actor.
    pub fn update_peer_capabilities(&self, capabilities: message::UpdatePeerCapabilities) {
        if self
            .update_peer_capabilities_sender
            .send(capabilities)
            .is_err()
        {
            iroha_logger::debug!("Network actor is closed, dropping peer capability update");
        }
    }

    /// Update trusted peer list for reputation tracking.
    pub fn update_trusted_peers(&self, trusted: UpdateTrustedPeers) {
        if self.update_trusted_peers_sender.send(trusted).is_err() {
            iroha_logger::debug!("Network actor is closed, dropping trusted peers update");
        }
    }

    /// Update ACL configuration at runtime.
    pub fn update_acl(&self, acl: message::UpdateAcl) {
        if self.update_acl_sender.send(acl).is_err() {
            iroha_logger::debug!("Network actor is closed, dropping ACL update");
        }
    }

    /// Update `SoraNet` handshake configuration at runtime.
    pub fn update_soranet_handshake(&self, handshake: ActualSoranetHandshake) {
        if self
            .update_handshake_sender
            .send(message::UpdateHandshake { handshake })
            .is_err()
        {
            iroha_logger::debug!("Network actor is closed, dropping handshake update");
        }
    }

    /// Update consensus handshake capabilities at runtime and optionally reconnect peers.
    pub fn update_consensus_caps(
        &self,
        caps: crate::ConsensusHandshakeCaps,
        drop_existing_peers: bool,
    ) {
        if self
            .update_consensus_caps_sender
            .send(message::UpdateConsensusCaps {
                caps,
                drop_existing_peers,
            })
            .is_err()
        {
            iroha_logger::debug!("Network actor is closed, dropping consensus caps update");
        }
    }

    /// Receive latest update of [`OnlinePeers`]
    pub fn online_peers<P>(&self, f: impl FnOnce(&OnlinePeers) -> P) -> P {
        f(&self.online_peers_receiver.borrow())
    }

    /// Receive latest update of online peer transport capabilities.
    pub fn online_peer_capabilities<P>(
        &self,
        f: impl FnOnce(&message::OnlinePeerCapabilities) -> P,
    ) -> P {
        f(&self.online_peer_capabilities_receiver.borrow())
    }

    /// Get a receiver of [`OnlinePeers`]
    pub fn online_peers_receiver(&self) -> watch::Receiver<OnlinePeers> {
        self.online_peers_receiver.clone()
    }

    /// Wait for update of [`OnlinePeers`].
    ///
    /// # Errors
    /// Returns an error if the network actor has shut down and the watch channel is closed.
    pub async fn wait_online_peers_update<P>(
        &mut self,
        f: impl FnOnce(&OnlinePeers) -> P + Send,
    ) -> Result<P, watch::error::RecvError> {
        self.online_peers_receiver.changed().await?;
        Ok(self.online_peers(f))
    }
}

#[cfg(test)]
mod handle_update_tests {
    use std::collections::HashSet;

    use iroha_config::parameters::actual::SoranetHandshake as ActualSoranetHandshake;
    use iroha_crypto::{encryption::ChaCha20Poly1305, kex::X25519Sha256};
    use norito::codec::{Decode, DecodeAll, Encode};
    use tokio::sync::{mpsc, watch};

    use super::*;

    #[derive(Clone, Debug, Decode, Encode)]
    struct Dummy;

    impl message::ClassifyTopic for Dummy {}

    impl<'a> norito::core::DecodeFromSlice<'a> for Dummy {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let mut slice = bytes;
            let value = <Self as DecodeAll>::decode_all(&mut slice).map_err(|error| {
                norito::core::Error::Message(format!("codec decode error: {error}"))
            })?;
            Ok((value, bytes.len() - slice.len()))
        }
    }

    fn closed_handle() -> NetworkBaseHandle<Dummy, X25519Sha256, ChaCha20Poly1305> {
        let (subscribe_tx, _subscribe_rx) = mpsc::unbounded_channel::<Subscriber<Dummy>>();
        let (update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<message::UpdateTopology>();
        let (update_peers_tx, update_peers_rx) = mpsc::unbounded_channel::<message::UpdatePeers>();
        let (update_peer_capabilities_tx, update_peer_capabilities_rx) =
            mpsc::unbounded_channel::<message::UpdatePeerCapabilities>();
        let (update_trusted_tx, update_trusted_rx) =
            mpsc::unbounded_channel::<message::UpdateTrustedPeers>();
        let (update_acl_tx, update_acl_rx) = mpsc::unbounded_channel::<message::UpdateAcl>();
        let (update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<message::UpdateHandshake>();
        let (update_consensus_caps_tx, update_consensus_caps_rx) =
            mpsc::unbounded_channel::<message::UpdateConsensusCaps>();
        let (service_message_tx, _service_message_rx) =
            mpsc::channel::<ServiceMessage<WireMessage<Dummy>>>(1);
        let (network_message_high_sender, _network_message_high_rx) =
            net_channel::channel_with_capacity(1);
        let (network_message_low_sender, _network_message_low_rx) =
            net_channel::channel_with_capacity(1);
        let (_online_peers_tx, online_peers_receiver) = watch::channel(HashSet::new());
        let (_online_peer_capabilities_tx, online_peer_capabilities_receiver) =
            watch::channel(HashMap::new());

        drop(update_topology_rx);
        drop(update_peers_rx);
        drop(update_peer_capabilities_rx);
        drop(update_trusted_rx);
        drop(update_acl_rx);
        drop(update_handshake_rx);
        drop(update_consensus_caps_rx);

        NetworkBaseHandle {
            subscribe_to_peers_messages_sender: subscribe_tx,
            online_peers_receiver,
            online_peer_capabilities_receiver,
            update_topology_sender: update_topology_tx,
            update_peers_sender: update_peers_tx,
            update_peer_capabilities_sender: update_peer_capabilities_tx,
            update_trusted_peers_sender: update_trusted_tx,
            update_acl_sender: update_acl_tx,
            update_handshake_sender: update_handshake_tx,
            update_consensus_caps_sender: update_consensus_caps_tx,
            service_message_sender: service_message_tx,
            network_message_high_sender,
            network_message_low_sender,
            subscriber_queue_cap: core::num::NonZeroUsize::new(1).expect("nonzero"),
            _key_exchange: core::marker::PhantomData,
            _encryptor: core::marker::PhantomData,
        }
    }

    #[test]
    fn update_methods_ignore_closed_channels() {
        let handle = closed_handle();
        handle.update_topology(message::UpdateTopology(HashSet::new()));
        handle.update_peers_addresses(message::UpdatePeers(Vec::new()));
        handle.update_trusted_peers(message::UpdateTrustedPeers::default());
        handle.update_acl(message::UpdateAcl::default());
        handle.update_soranet_handshake(ActualSoranetHandshake::default());

        let caps = crate::ConsensusHandshakeCaps {
            mode_tag: "test".to_string(),
            proto_version: 1,
            consensus_fingerprint: [0u8; 32],
            config: crate::ConsensusConfigCaps {
                collectors_k: 0,
                redundant_send_r: 0,
                da_enabled: false,
                rbc_chunk_max_bytes: 0,
                rbc_session_ttl_ms: 0,
                rbc_store_max_sessions: 0,
                rbc_store_soft_sessions: 0,
                rbc_store_max_bytes: 0,
                rbc_store_soft_bytes: 0,
            },
        };
        handle.update_consensus_caps(caps, false);
    }

    #[test]
    fn closed_handle_reports_subscriber_queue_cap() {
        let handle = closed_handle();
        assert_eq!(handle.subscriber_queue_cap().get(), 1);
    }

    #[tokio::test]
    async fn wait_online_peers_update_reports_closed_channel() {
        let mut handle = closed_handle();
        let result = handle.wait_online_peers_update(HashSet::len).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod accept_stream_tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    };

    use iroha_config::parameters::actual::{
        LaneProfile, Network as NetCfg, RelayMode, SoranetHandshake as ActualSoranetHandshake,
        SoranetPow, SoranetPrivacy as ActualSoranetPrivacy,
    };
    use iroha_crypto::{
        KeyPair,
        encryption::ChaCha20Poly1305,
        kex::X25519Sha256,
        soranet::handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        },
    };
    use iroha_data_model::{
        ChainId,
        peer::{Peer, PeerId},
    };
    use iroha_primitives::addr::socket_addr;
    use norito::codec::{Decode, DecodeAll, Encode};
    #[cfg(feature = "quic")]
    #[allow(unused_imports)]
    use quinn::crypto::rustls::QuicClientConfig;
    use tokio::{
        net::TcpListener,
        sync::{mpsc, watch},
    };

    use super::*;
    use crate::{
        peer::{
            SoranetHandshakeConfig,
            test_support::{SpawnPath, snapshot},
        },
        sampler::LogSampler,
    };

    #[derive(Clone, Debug, Decode, Encode)]
    struct Dummy;

    impl crate::network::message::ClassifyTopic for Dummy {}

    macro_rules! impl_decode_from_slice_via_codec {
        ($($ty:ty),+ $(,)?) => {
            $(
                impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
                    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                        let mut slice = bytes;
                        let value = <Self as DecodeAll>::decode_all(&mut slice).map_err(|error| {
                            norito::core::Error::Message(format!("codec decode error: {error}"))
                        })?;
                        Ok((value, bytes.len() - slice.len()))
                    }
                }
            )+
        };
    }

    impl_decode_from_slice_via_codec!(Dummy);

    #[derive(Clone, Debug, Decode, Encode)]
    struct DummyConsensus;

    impl message::ClassifyTopic for DummyConsensus {
        fn topic(&self) -> message::Topic {
            message::Topic::Consensus
        }
    }

    impl_decode_from_slice_via_codec!(DummyConsensus);

    #[derive(Clone, Debug, Decode, Encode)]
    struct DummyConsensusPayload;

    impl message::ClassifyTopic for DummyConsensusPayload {
        fn topic(&self) -> message::Topic {
            message::Topic::ConsensusPayload
        }
    }

    impl_decode_from_slice_via_codec!(DummyConsensusPayload);

    #[derive(Clone, Debug, Decode, Encode)]
    struct DummyConsensusChunk;

    impl message::ClassifyTopic for DummyConsensusChunk {
        fn topic(&self) -> message::Topic {
            message::Topic::ConsensusChunk
        }
    }

    impl_decode_from_slice_via_codec!(DummyConsensusChunk);

    #[cfg(any(feature = "p2p_tls", feature = "quic"))]
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    #[cfg(any(feature = "p2p_tls", feature = "quic"))]
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    #[cfg(any(feature = "p2p_tls", feature = "quic"))]
    use rustls::{DigitallySignedStruct, Error as RustlsError, SignatureScheme};

    #[cfg(any(feature = "p2p_tls", feature = "quic"))]
    #[derive(Debug)]
    struct AcceptAllVerifier;

    #[cfg(any(feature = "p2p_tls", feature = "quic"))]
    impl ServerCertVerifier for AcceptAllVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PKCS1_SHA256,
            ]
        }
    }

    fn base_cfg() -> NetCfg {
        NetCfg {
            address: iroha_config_base::WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: iroha_config_base::WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: iroha_config::parameters::defaults::network::RELAY_TTL,
            soranet_handshake: ActualSoranetHandshake {
                descriptor_commit: iroha_config_base::WithOrigin::inline(
                    DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
                ),
                client_capabilities: iroha_config_base::WithOrigin::inline(
                    DEFAULT_CLIENT_CAPABILITIES.to_vec(),
                ),
                relay_capabilities: iroha_config_base::WithOrigin::inline(
                    DEFAULT_RELAY_CAPABILITIES.to_vec(),
                ),
                trust_gossip: true,
                kem_id: 1,
                sig_id: 1,
                resume_hash: None,
                pow: SoranetPow::default(),
            },
            soranet_privacy: ActualSoranetPrivacy::default(),
            soranet_vpn: iroha_config::parameters::actual::SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout: std::time::Duration::from_millis(200),
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            deferred_send_ttl: std::time::Duration::from_millis(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
            ),
            deferred_send_max_per_peer:
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
            peer_gossip_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            quic_datagrams_enabled:
                iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            scion: iroha_config::parameters::actual::ScionConfig::default(),
            tls_enabled: false,
            tls_fallback_to_plain: false,
            tls_listen_address: None,
            tls_inbound_only: false,
            prefer_ws_fallback: false,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: vec![],
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            p2p_queue_cap_high: core::num::NonZeroUsize::new(128).unwrap(),
            p2p_queue_cap_low: core::num::NonZeroUsize::new(128).unwrap(),
            p2p_post_queue_cap: core::num::NonZeroUsize::new(64).unwrap(),
            p2p_subscriber_queue_cap:
                iroha_config::parameters::defaults::network::P2P_SUBSCRIBER_QUEUE_CAP,
            consensus_ingress_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC,
            consensus_ingress_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BURST,
            consensus_ingress_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC,
            consensus_ingress_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_BURST,
            consensus_ingress_critical_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
            consensus_ingress_critical_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
            consensus_ingress_critical_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
            consensus_ingress_critical_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
            consensus_ingress_rbc_session_limit:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT,
            consensus_ingress_penalty_threshold:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
            consensus_ingress_penalty_window: Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
            ),
            consensus_ingress_penalty_cooldown: Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
            ),
            happy_eyeballs_stagger: std::time::Duration::from_millis(50),
            addr_ipv6_first: false,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
            accept_rate_per_prefix_per_sec: None,
            accept_burst_per_prefix: None,
            low_priority_rate_per_sec: None,
            low_priority_burst: None,
            low_priority_bytes_per_sec: None,
            low_priority_bytes_burst: None,
            allowlist_only: false,
            allow_keys: vec![],
            deny_keys: vec![],
            allow_cidrs: vec![],
            deny_cidrs: vec![],
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
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accept_stream_denied_by_incoming_cap() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.max_incoming = core::num::NonZeroUsize::new(1);
        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown.clone(),
        )
        .await;
        let (handle, _child) = match started {
            Ok(ok) => ok,
            Err(Error::Io(_) | Error::BindListener { .. }) => {
                // Likely running in a sandbox that forbids sockets; skip.
                return;
            }
            Err(e) => panic!("network start: {e:?}"),
        };

        let (a, _b) = tokio::io::duplex(64);
        let (read, write) = tokio::io::split(a);
        let remote: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let first = handle
            .accept_stream(read, write, remote)
            .await
            .expect("accept_stream should not error");
        assert!(first, "first inbound should be allowed");

        // Second concurrent inbound should be denied by the cap.
        let (a2, _b2) = tokio::io::duplex(64);
        let (read2, write2) = tokio::io::split(a2);
        let remote2: std::net::SocketAddr = "127.0.0.1:12346".parse().unwrap();
        let second = handle
            .accept_stream(read2, write2, remote2)
            .await
            .expect("accept_stream should not error");
        assert!(
            !second,
            "second inbound should be denied by max_incoming=1 cap already filled"
        );
        shutdown.send();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_proxy_required_without_proxy() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.p2p_proxy_required = true;
        cfg.p2p_proxy = None;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_proxy_required_with_no_proxy_exemptions() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.p2p_proxy_required = true;
        cfg.p2p_proxy = Some("http://proxy.invalid:8080".to_string());
        cfg.p2p_no_proxy = vec!["localhost".to_string()];

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_https_proxy_without_pin_when_verify_enabled() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.p2p_proxy = Some("https://proxy.invalid:443".to_string());
        cfg.p2p_proxy_tls_verify = true;
        cfg.p2p_proxy_tls_pinned_cert_der_base64 = None;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[cfg(not(feature = "p2p_tls"))]
    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_tls_without_feature_when_tls_only_outbound() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.tls_enabled = true;
        cfg.tls_fallback_to_plain = false;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[cfg(not(feature = "p2p_tls"))]
    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_tls_inbound_only_without_feature() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.tls_enabled = true;
        cfg.tls_inbound_only = true;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[cfg(feature = "p2p_tls")]
    #[tokio::test(flavor = "current_thread")]
    async fn start_accepts_tls_inbound_only_with_tls_feature() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.tls_enabled = true;
        cfg.tls_inbound_only = true;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown.clone(),
        )
        .await;

        let (_handle, _child) = match started {
            Ok(ok) => ok,
            Err(Error::Io(_) | Error::BindListener { .. }) => {
                // Likely running in a sandbox that forbids sockets; skip.
                return;
            }
            Err(e) => panic!("network start: {e:?}"),
        };

        shutdown.send();
    }

    #[cfg(feature = "quic")]
    #[tokio::test(flavor = "current_thread")]
    async fn start_rejects_proxy_required_with_quic_enabled() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.p2p_proxy_required = true;
        cfg.p2p_proxy = Some("http://proxy.invalid:8080".to_string());
        cfg.quic_enabled = true;

        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown,
        )
        .await;

        assert!(matches!(
            started,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accept_stream_allows_basic() {
        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.max_incoming = core::num::NonZeroUsize::new(1);
        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            Some(ChainId::from("test-chain".to_string())),
            None,
            None,
            shutdown.clone(),
        )
        .await;
        let (handle, _child) = match started {
            Ok(ok) => ok,
            Err(Error::Io(_) | Error::BindListener { .. }) => {
                // Likely running in a sandbox that forbids sockets; skip.
                return;
            }
            Err(e) => panic!("network start: {e:?}"),
        };

        let (a, _b) = tokio::io::duplex(64);
        let (read, write) = tokio::io::split(a);
        let remote: std::net::SocketAddr = "127.0.0.1:23456".parse().unwrap();
        let allowed = handle
            .accept_stream(read, write, remote)
            .await
            .expect("accept_stream should not error");
        assert!(allowed, "should be accepted by caps");
        shutdown.send();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn connect_peer_propagates_frame_cap() {
        use std::collections::HashSet;

        use iroha_primitives::addr::socket_addr;

        let baseline = snapshot().len();

        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.max_frame_bytes = 37_777;
        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            None,
            None,
            None,
            shutdown.clone(),
        )
        .await;
        let (handle, _child) = match started {
            Ok(ok) => ok,
            Err(Error::Io(_) | Error::BindListener { .. }) => {
                return;
            }
            Err(e) => panic!("network start: {e:?}"),
        };

        let peer_key = KeyPair::random();
        let peer_id = iroha_data_model::peer::PeerId::from(peer_key.public_key().clone());
        let addr = socket_addr!(127.0.0.1:9);
        handle.update_peers_addresses(UpdatePeers(vec![(peer_id.clone(), addr)]));
        let mut topology = HashSet::new();
        topology.insert(peer_id);
        handle.update_topology(UpdateTopology(topology));

        let mut observed = false;
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let records = snapshot();
            if records
                .iter()
                .skip(baseline)
                .any(|(path, cap)| *path == SpawnPath::Connecting && *cap == 37_777)
            {
                observed = true;
                break;
            }
        }

        shutdown.send();
        assert!(
            observed,
            "expected connecting spawn to record configured cap"
        );
    }

    #[cfg(feature = "p2p_tls")]
    #[tokio::test(flavor = "current_thread")]
    async fn tls_listener_propagates_frame_cap() {
        use std::sync::Arc;

        use tokio::sync::mpsc;
        use tokio_rustls::{
            TlsConnector,
            rustls::{self, ClientConfig},
        };

        let baseline = snapshot().len();
        let key_pair = KeyPair::random();
        let max_frame_bytes = 59_999usize;

        let (service_tx, mut service_rx) =
            mpsc::channel::<super::ServiceMessage<super::WireMessage<Dummy>>>(8);
        tokio::spawn(async move { while service_rx.recv().await.is_some() {} });

        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp bind failed: {e:?}"),
        };
        let addr = std_listener.local_addr().unwrap();
        drop(std_listener);

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        start_tls_listener::<super::WireMessage<Dummy>, X25519Sha256, ChaCha20Poly1305>(
            addr,
            key_pair.clone(),
            socket_addr!(127.0.0.1:1_337),
            service_tx,
            Duration::from_secs(1),
            None,
            None,
            None,
            None,
            8,
            true,
            max_frame_bytes,
            false,
            0,
            soranet.clone(),
            RelayRole::Disabled,
            true,
            None,
        )
        .await
        .expect("start_tls_listener");

        let tcp = match tokio::net::TcpStream::connect(addr).await {
            Ok(stream) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tls connect failed: {e:?}"),
        };

        let client_cfg = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(client_cfg));
        let server_name = rustls::pki_types::ServerName::try_from("iroha-tls")
            .unwrap()
            .to_owned();

        if connector.connect(server_name, tcp).await.is_err() {
            return;
        }

        let mut observed = false;
        for _ in 0..20 {
            if snapshot()
                .iter()
                .skip(baseline)
                .any(|(path, cap)| *path == SpawnPath::ConnectedFrom && *cap == max_frame_bytes)
            {
                observed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            observed,
            "expected tls listener to propagate configured frame cap"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accept_stream_propagates_frame_cap() {
        use std::net::SocketAddr as StdSocketAddr;

        let baseline = snapshot().len();

        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.max_incoming = core::num::NonZeroUsize::new(2);
        cfg.max_frame_bytes = 48_888;
        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let started = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair,
            cfg,
            None,
            None,
            None,
            shutdown.clone(),
        )
        .await;
        let (handle, _child) = match started {
            Ok(ok) => ok,
            Err(Error::Io(_) | Error::BindListener { .. }) => {
                return;
            }
            Err(e) => panic!("network start: {e:?}"),
        };

        let (inbound, _peer) = tokio::io::duplex(128);
        let (read, write) = tokio::io::split(inbound);
        let remote: StdSocketAddr = "127.0.0.1:34567".parse().unwrap();
        let allowed = handle
            .accept_stream(read, write, remote)
            .await
            .expect("accept_stream should not error");
        assert!(
            allowed,
            "connection should be accepted for propagation test"
        );

        let mut observed = false;
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let records = snapshot();
            if records
                .iter()
                .skip(baseline)
                .any(|(path, cap)| *path == SpawnPath::ConnectedFrom && *cap == 48_888)
            {
                observed = true;
                break;
            }
        }

        shutdown.send();
        assert!(
            observed,
            "expected connected_from spawn to record configured cap"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_provides_bind_listener_context_on_failure() {
        use iroha_primitives::addr::socket_addr;

        let key_pair = KeyPair::random();
        let mut cfg = base_cfg();
        cfg.address = iroha_config_base::WithOrigin::inline(socket_addr!(127.0.0.1:1));
        cfg.public_address = iroha_config_base::WithOrigin::inline(socket_addr!(127.0.0.1:1));
        let shutdown = iroha_futures::supervisor::ShutdownSignal::new();
        let result = super::NetworkBaseHandle::<Dummy, X25519Sha256, ChaCha20Poly1305>::start(
            key_pair, cfg, None, None, None, shutdown,
        )
        .await;

        match result {
            Err(Error::BindListener {
                listen_addr,
                public_address,
                ..
            }) => {
                assert!(
                    listen_addr.contains("127.0.0.1:1"),
                    "listen address should mention configured endpoint; got {listen_addr}"
                );
                assert!(
                    public_address.contains("127.0.0.1:1"),
                    "public address should mention configured endpoint; got {public_address}"
                );
            }
            Err(Error::Io(_)) => {
                // Likely running in a sandbox that forbids sockets; skip.
            }
            Ok(_) => panic!("expected bind failure due to privileged port"),
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    async fn accept_new_peer_propagates_frame_cap() {
        let baseline = snapshot().len();

        let key_pair = KeyPair::random();
        let max_frame_bytes = 59_999usize;

        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("listener bind failed: {e:?}"),
        };
        let listen_addr_std = std_listener.local_addr().unwrap();
        let listener_clone = std_listener.try_clone().unwrap();
        listener_clone.set_nonblocking(true).unwrap();
        let listener = match TcpListener::from_std(listener_clone) {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp listener from_std failed: {e:?}"),
        };

        let (_subscribe_tx, subscribe_rx) = mpsc::unbounded_channel::<super::Subscriber<Dummy>>();
        let (_update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<super::message::UpdateTopology>();
        let (_update_peers_tx, update_peers_rx) =
            mpsc::unbounded_channel::<super::message::UpdatePeers>();
        let (_update_trusted_tx, update_trusted_peers_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateTrustedPeers>();
        let (_update_acl_tx, update_acl_rx) =
            mpsc::unbounded_channel::<super::message::UpdateAcl>();
        #[allow(unused_variables)]
        let (_update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<super::message::UpdateHandshake>();
        let (_update_consensus_caps_tx, update_consensus_caps_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateConsensusCaps>();
        let (peer_message_hi_tx, peer_message_hi_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<Dummy>>>(1);
        let (peer_message_lo_tx, peer_message_lo_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<Dummy>>>(1);
        let (service_message_tx, service_message_rx) =
            mpsc::channel::<super::ServiceMessage<WireMessage<Dummy>>>(4);
        let (_hi_tx, network_message_high_rx) = super::net_channel::channel_with_capacity(1);
        let (_lo_tx, network_message_low_rx) = super::net_channel::channel_with_capacity(1);
        let (online_peers_tx, _online_peers_rx) = watch::channel(HashSet::new());
        let (online_peer_capabilities_tx, _online_peer_capabilities_rx) =
            watch::channel::<super::message::OnlinePeerCapabilities>(HashMap::new());
        let (_update_peer_capabilities_tx, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel::<super::message::UpdatePeerCapabilities>();

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        let mut network = super::NetworkBase::<Dummy, X25519Sha256, ChaCha20Poly1305> {
            listen_addr: listen_addr_std.into(),
            public_address: listen_addr_std.into(),
            relay_role: RelayRole::Disabled,
            relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_hub_peer: None,
            relay_trusted_peers: HashSet::new(),
            relay_ttl: DEFAULT_RELAY_TTL,
            trust_gossip_config: true,
            trust_gossip: true,
            self_id: PeerId::from(key_pair.public_key().clone()),
            address_book: HashMap::new(),
            peer_reputations: PeerReputationBook::default(),
            soranet_handshake: soranet.clone(),
            peers: HashMap::new(),
            connecting_peers: HashMap::new(),
            listener,
            key_pair,
            subscribers_to_peers_messages: Vec::new(),
            subscribe_to_peers_messages_receiver: subscribe_rx,
            online_peers_sender: online_peers_tx,
            online_peer_capabilities_sender: online_peer_capabilities_tx,
            update_topology_receiver: update_topology_rx,
            update_peers_receiver: update_peers_rx,
            update_peer_capabilities_receiver,
            update_trusted_peers_receiver,
            update_acl_receiver: update_acl_rx,
            network_message_high_receiver: network_message_high_rx,
            network_message_low_receiver: network_message_low_rx,
            peer_message_high_receiver: peer_message_hi_rx,
            peer_message_low_receiver: peer_message_lo_rx,
            peer_message_high_sender: peer_message_hi_tx,
            peer_message_low_sender: peer_message_lo_tx,
            service_message_receiver: service_message_rx,
            service_message_sender: service_message_tx,
            update_handshake_receiver: update_handshake_rx,
            update_consensus_caps_receiver,
            current_conn_id: 0,
            current_topology: HashSet::new(),
            current_peers_addresses: Vec::new(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            peer_capabilities: HashMap::new(),
            post_queue_cap: 4,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            dns_last_refresh: HashMap::new(),
            dns_pending_refresh: HashSet::new(),
            quic_enabled: false,
            quic_datagrams_enabled: false,
            quic_datagram_max_payload_bytes: 0,
            quic_dialer: None,
            local_scion_supported: false,
            tls_enabled: false,
            tls_fallback_to_plain: false,
            prefer_ws_fallback: false,
            proxy_policy: crate::transport::ProxyPolicy::disabled(),
            proxy_tls_verify: true,
            proxy_tls_pinned_cert_der: None,
            allowlist_only: false,
            allow_keys: HashSet::new(),
            deny_keys: HashSet::new(),
            allow_nets: Vec::new(),
            deny_nets: Vec::new(),
            idle_timeout: Duration::from_millis(50),
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            tcp_nodelay: true,
            tcp_keepalive: None,
            connect_startup_delay_until: tokio::time::Instant::now(),
            retry_backoff: HashMap::new(),
            peer_session_generation: HashMap::new(),
            pending_connects: Vec::new(),
            deferred_send_queue: DeferredPeerFrameQueue::new(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
                Duration::from_millis(
                    iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
                ),
            ),
            happy_eyeballs_stagger: Duration::from_millis(10),
            topology_update_interval:
                iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            addr_ipv6_first: false,
            last_active: HashMap::new(),
            incoming_pending: HashSet::new(),
            incoming_active: HashSet::new(),
            max_incoming: None,
            max_total_connections: None,
            accept_params: AcceptThrottleParams::new(
                None,
                None,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
                None,
                None,
                iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
                iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            ),
            accept_prefix_buckets: HashMap::new(),
            accept_ip_buckets: HashMap::new(),
            sampler_high_queue_warn: LogSampler::new(),
            sampler_low_queue_warn: LogSampler::new(),
            sampler_accept_err: LogSampler::new(),
            low_rate_per_sec: None,
            low_burst: None,
            low_buckets: HashMap::new(),
            low_bytes_per_sec: None,
            low_bytes_burst: None,
            low_bytes_buckets: HashMap::new(),
            max_frame_bytes,
            cap_consensus: max_frame_bytes,
            cap_control: max_frame_bytes,
            cap_block_sync: max_frame_bytes,
            cap_tx_gossip: max_frame_bytes,
            cap_peer_gossip: max_frame_bytes,
            cap_health: max_frame_bytes,
            cap_other: max_frame_bytes,
            disconnect_on_post_overflow: false,
            _key_exchange: core::marker::PhantomData,
            _encryptor: core::marker::PhantomData,
        };

        let connect_addr = listen_addr_std;
        let joiner = std::thread::spawn(move || -> std::io::Result<()> {
            let stream = std::net::TcpStream::connect(connect_addr)?;
            let _ = stream.set_nodelay(true);
            std::thread::sleep(std::time::Duration::from_millis(30));
            Ok(())
        });
        let accepted_std = loop {
            match std_listener.accept() {
                Ok((conn, _)) => break conn,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(e) => panic!("listener accept failed: {e:?}"),
            }
        };
        accepted_std.set_nonblocking(true).unwrap();
        let stream = match tokio::net::TcpStream::from_std(accepted_std) {
            Ok(stream) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp stream from_std failed: {e:?}"),
        };

        let conn_id = network.get_conn_id();
        network.accept_new_peer(stream, conn_id);

        match joiner.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Ok(Err(e)) => panic!("connector failed: {e:?}"),
            Err(payload) => std::panic::resume_unwind(payload),
        }

        let mut observed = false;
        for _ in 0..10 {
            if snapshot()
                .iter()
                .skip(baseline)
                .any(|(path, cap)| *path == SpawnPath::ConnectedFrom && *cap == max_frame_bytes)
            {
                observed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        assert!(
            observed,
            "expected accept_new_peer to propagate configured frame cap"
        );

        drop(network);
    }

    #[cfg(feature = "quic")]
    #[tokio::test(flavor = "current_thread")]
    async fn quic_listener_propagates_frame_cap() {
        use std::sync::Arc;

        use tokio::sync::mpsc;

        let baseline = snapshot().len();
        let key_pair = KeyPair::random();
        let max_frame_bytes = 61_111usize;

        let (service_tx, mut service_rx) =
            mpsc::channel::<super::ServiceMessage<super::WireMessage<Dummy>>>(8);
        tokio::spawn(async move { while service_rx.recv().await.is_some() {} });

        let udp = match std::net::UdpSocket::bind("127.0.0.1:0") {
            Ok(sock) => sock,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("udp bind failed: {e:?}"),
        };
        let addr = udp.local_addr().unwrap();
        drop(udp);

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        start_quic_listener::<super::WireMessage<Dummy>, X25519Sha256, ChaCha20Poly1305>(
            &addr,
            key_pair.clone(),
            socket_addr!(127.0.0.1:4_321),
            service_tx,
            Duration::from_secs(1),
            None,
            false,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
            8,
            true,
            max_frame_bytes,
            soranet.clone(),
            false,
            RelayRole::Disabled,
        )
        .await
        .expect("start_quic_listener");

        let bind_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut endpoint = match quinn::Endpoint::client(bind_addr) {
            Ok(ep) => ep,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    return;
                }
                panic!("quic endpoint failed: {err:?}");
            }
        };

        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
            .with_no_client_auth();
        let mut client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .expect("failed to configure rustls for quinn"),
        ));
        client_config.transport_config(Arc::new(quinn::TransportConfig::default()));
        endpoint.set_default_client_config(client_config);

        let connecting = match endpoint.connect(addr, "iroha-quic") {
            Ok(conn) => conn,
            Err(e) => {
                iroha_logger::warn!(%e, "quic connect failed; skipping test");
                return;
            }
        };

        let connection = match connecting.await {
            Ok(conn) => conn,
            Err(e) => {
                iroha_logger::warn!(%e, "quic handshake failed; skipping test");
                return;
            }
        };

        if connection.open_bi().await.is_err() {
            return;
        }

        let mut observed = false;
        for _ in 0..20 {
            if snapshot()
                .iter()
                .skip(baseline)
                .any(|(path, cap)| *path == SpawnPath::ConnectedFrom && *cap == max_frame_bytes)
            {
                observed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        connection.close(0u32.into(), b"done");
        endpoint.close(0u32.into(), b"done");

        assert!(
            observed,
            "expected quic listener to propagate configured frame cap"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    async fn peer_message_over_cap_increments_violation_counter() {
        use crate::network::cap_violations_consensus;
        let _guard = cap_violation_test_guard();

        let key_pair = KeyPair::random();

        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("listener bind failed: {e:?}"),
        };
        let listen_addr_std = std_listener.local_addr().unwrap();
        let listener_clone = std_listener.try_clone().unwrap();
        listener_clone.set_nonblocking(true).unwrap();
        let listener = match TcpListener::from_std(listener_clone) {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp stream from_std failed: {e:?}"),
        };

        let (_subscribe_tx, subscribe_rx): (
            mpsc::UnboundedSender<super::Subscriber<DummyConsensus>>,
            _,
        ) = mpsc::unbounded_channel();
        let (_update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<super::message::UpdateTopology>();
        let (_update_peers_tx, update_peers_rx) =
            mpsc::unbounded_channel::<super::message::UpdatePeers>();
        let (_update_trusted_tx, update_trusted_peers_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateTrustedPeers>();
        let (_update_acl_tx, update_acl_rx) =
            mpsc::unbounded_channel::<super::message::UpdateAcl>();
        let (_update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<super::message::UpdateHandshake>();
        let (_update_consensus_caps_tx, update_consensus_caps_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateConsensusCaps>();
        let (peer_message_hi_tx, peer_message_hi_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensus>>>(1);
        let (peer_message_lo_tx, peer_message_lo_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensus>>>(1);
        let (service_message_tx, service_message_rx) =
            mpsc::channel::<super::ServiceMessage<WireMessage<DummyConsensus>>>(4);
        let (_hi_tx, network_message_high_rx) = super::net_channel::channel_with_capacity(1);
        let (_lo_tx, network_message_low_rx) = super::net_channel::channel_with_capacity(1);
        let (online_peers_tx, _online_peers_rx) = watch::channel(HashSet::new());
        let (online_peer_capabilities_tx, _online_peer_capabilities_rx) =
            watch::channel::<super::message::OnlinePeerCapabilities>(HashMap::new());
        let (_update_peer_capabilities_tx, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel::<super::message::UpdatePeerCapabilities>();

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        let mut network = super::NetworkBase::<DummyConsensus, X25519Sha256, ChaCha20Poly1305> {
            listen_addr: listen_addr_std.into(),
            public_address: listen_addr_std.into(),
            relay_role: RelayRole::Disabled,
            relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_hub_peer: None,
            relay_trusted_peers: HashSet::new(),
            relay_ttl: DEFAULT_RELAY_TTL,
            trust_gossip_config: true,
            trust_gossip: true,
            self_id: PeerId::from(key_pair.public_key().clone()),
            address_book: HashMap::new(),
            peer_reputations: PeerReputationBook::default(),
            soranet_handshake: soranet.clone(),
            peers: HashMap::new(),
            connecting_peers: HashMap::new(),
            listener,
            key_pair: key_pair.clone(),
            subscribers_to_peers_messages: Vec::new(),
            subscribe_to_peers_messages_receiver: subscribe_rx,
            online_peers_sender: online_peers_tx,
            online_peer_capabilities_sender: online_peer_capabilities_tx,
            update_topology_receiver: update_topology_rx,
            update_peers_receiver: update_peers_rx,
            update_peer_capabilities_receiver,
            update_trusted_peers_receiver,
            update_acl_receiver: update_acl_rx,
            network_message_high_receiver: network_message_high_rx,
            network_message_low_receiver: network_message_low_rx,
            peer_message_high_receiver: peer_message_hi_rx,
            peer_message_low_receiver: peer_message_lo_rx,
            peer_message_high_sender: peer_message_hi_tx,
            peer_message_low_sender: peer_message_lo_tx,
            service_message_receiver: service_message_rx,
            service_message_sender: service_message_tx,
            update_handshake_receiver: update_handshake_rx,
            update_consensus_caps_receiver,
            current_conn_id: 0,
            current_topology: HashSet::new(),
            current_peers_addresses: Vec::new(),
            idle_timeout: Duration::from_millis(50),
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            tcp_nodelay: true,
            tcp_keepalive: None,
            connect_startup_delay_until: tokio::time::Instant::now(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            peer_capabilities: HashMap::new(),
            post_queue_cap: 4,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            dns_last_refresh: HashMap::new(),
            dns_pending_refresh: HashSet::new(),
            quic_enabled: false,
            quic_datagrams_enabled: false,
            quic_datagram_max_payload_bytes: 0,
            quic_dialer: None,
            local_scion_supported: false,
            tls_enabled: false,
            tls_fallback_to_plain: false,
            prefer_ws_fallback: false,
            proxy_policy: crate::transport::ProxyPolicy::disabled(),
            proxy_tls_verify: true,
            proxy_tls_pinned_cert_der: None,
            allowlist_only: false,
            allow_keys: HashSet::new(),
            deny_keys: HashSet::new(),
            allow_nets: Vec::new(),
            deny_nets: Vec::new(),
            retry_backoff: HashMap::new(),
            peer_session_generation: HashMap::new(),
            pending_connects: Vec::new(),
            deferred_send_queue: DeferredPeerFrameQueue::new(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
                Duration::from_millis(
                    iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
                ),
            ),
            happy_eyeballs_stagger: Duration::from_millis(10),
            topology_update_interval:
                iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            addr_ipv6_first: false,
            last_active: HashMap::new(),
            incoming_pending: HashSet::new(),
            incoming_active: HashSet::new(),
            max_incoming: None,
            max_total_connections: None,
            accept_params: AcceptThrottleParams::new(
                None,
                None,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
                None,
                None,
                iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
                iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            ),
            accept_prefix_buckets: HashMap::new(),
            accept_ip_buckets: HashMap::new(),
            sampler_high_queue_warn: LogSampler::new(),
            sampler_low_queue_warn: LogSampler::new(),
            sampler_accept_err: LogSampler::new(),
            low_rate_per_sec: None,
            low_burst: None,
            low_buckets: HashMap::new(),
            low_bytes_per_sec: None,
            low_bytes_burst: None,
            low_bytes_buckets: HashMap::new(),
            max_frame_bytes: 4096,
            cap_consensus: 128,
            cap_control: 128,
            cap_block_sync: 128,
            cap_tx_gossip: 128,
            cap_peer_gossip: 128,
            cap_health: 128,
            cap_other: 128,
            disconnect_on_post_overflow: false,
            _key_exchange: core::marker::PhantomData,
            _encryptor: core::marker::PhantomData,
        };

        let before = cap_violations_consensus();

        let peer = Peer::new(
            listen_addr_std.into(),
            PeerId::from(key_pair.public_key().clone()),
        );
        let oversized = super::PeerMessage {
            peer: peer.clone(),
            payload: RelayMessage::new(
                peer.id().clone(),
                RelayTarget::Direct(network.self_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyConsensus,
            ),
            payload_bytes: 512,
        };

        network.peer_message(oversized).await;

        let after = cap_violations_consensus();
        assert!(
            after >= before + 1,
            "expected cap violation counter to increase"
        );
        assert!(
            network.retry_backoff.contains_key(peer.id()),
            "cap violation should schedule backoff for offending peer"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    async fn peer_message_consensus_payload_uses_block_sync_cap() {
        use crate::network::cap_violations_consensus;
        let _guard = cap_violation_test_guard();

        let key_pair = KeyPair::random();

        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("listener bind failed: {e:?}"),
        };
        let listen_addr_std = std_listener.local_addr().unwrap();
        let listener_clone = std_listener.try_clone().unwrap();
        listener_clone.set_nonblocking(true).unwrap();
        let listener = match TcpListener::from_std(listener_clone) {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp stream from_std failed: {e:?}"),
        };

        let (_subscribe_tx, subscribe_rx): (
            mpsc::UnboundedSender<super::Subscriber<DummyConsensusPayload>>,
            _,
        ) = mpsc::unbounded_channel();
        let (_update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<super::message::UpdateTopology>();
        let (_update_peers_tx, update_peers_rx) =
            mpsc::unbounded_channel::<super::message::UpdatePeers>();
        let (_update_trusted_tx, update_trusted_peers_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateTrustedPeers>();
        let (_update_acl_tx, update_acl_rx) =
            mpsc::unbounded_channel::<super::message::UpdateAcl>();
        let (_update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<super::message::UpdateHandshake>();
        let (_update_consensus_caps_tx, update_consensus_caps_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateConsensusCaps>();
        let (peer_message_hi_tx, peer_message_hi_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensusPayload>>>(1);
        let (peer_message_lo_tx, peer_message_lo_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensusPayload>>>(1);
        let (service_message_tx, service_message_rx) =
            mpsc::channel::<super::ServiceMessage<WireMessage<DummyConsensusPayload>>>(4);
        let (_hi_tx, network_message_high_rx) = super::net_channel::channel_with_capacity(1);
        let (_lo_tx, network_message_low_rx) = super::net_channel::channel_with_capacity(1);
        let (online_peers_tx, _online_peers_rx) = watch::channel(HashSet::new());
        let (online_peer_capabilities_tx, _online_peer_capabilities_rx) =
            watch::channel::<super::message::OnlinePeerCapabilities>(HashMap::new());
        let (_update_peer_capabilities_tx, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel::<super::message::UpdatePeerCapabilities>();

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        let mut network =
            super::NetworkBase::<DummyConsensusPayload, X25519Sha256, ChaCha20Poly1305> {
                listen_addr: listen_addr_std.into(),
                public_address: listen_addr_std.into(),
                relay_role: RelayRole::Disabled,
                relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
                relay_hub_addresses: Vec::new(),
                relay_hub_peer: None,
                relay_trusted_peers: HashSet::new(),
                relay_ttl: DEFAULT_RELAY_TTL,
                trust_gossip_config: true,
                trust_gossip: true,
                self_id: PeerId::from(key_pair.public_key().clone()),
                address_book: HashMap::new(),
                peer_reputations: PeerReputationBook::default(),
                soranet_handshake: soranet.clone(),
                peers: HashMap::new(),
                connecting_peers: HashMap::new(),
                listener,
                key_pair: key_pair.clone(),
                subscribers_to_peers_messages: Vec::new(),
                subscribe_to_peers_messages_receiver: subscribe_rx,
                online_peers_sender: online_peers_tx,
                online_peer_capabilities_sender: online_peer_capabilities_tx,
                update_topology_receiver: update_topology_rx,
                update_peers_receiver: update_peers_rx,
                update_peer_capabilities_receiver,
                update_trusted_peers_receiver,
                update_acl_receiver: update_acl_rx,
                network_message_high_receiver: network_message_high_rx,
                network_message_low_receiver: network_message_low_rx,
                peer_message_high_receiver: peer_message_hi_rx,
                peer_message_low_receiver: peer_message_lo_rx,
                peer_message_high_sender: peer_message_hi_tx,
                peer_message_low_sender: peer_message_lo_tx,
                service_message_receiver: service_message_rx,
                service_message_sender: service_message_tx,
                update_handshake_receiver: update_handshake_rx,
                update_consensus_caps_receiver,
                current_conn_id: 0,
                current_topology: HashSet::new(),
                current_peers_addresses: Vec::new(),
                idle_timeout: Duration::from_millis(50),
                dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
                tcp_nodelay: true,
                tcp_keepalive: None,
                connect_startup_delay_until: tokio::time::Instant::now(),
                chain_id: None,
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                peer_capabilities: HashMap::new(),
                post_queue_cap: 4,
                dns_refresh_interval: None,
                dns_refresh_ttl: None,
                dns_last_refresh: HashMap::new(),
                dns_pending_refresh: HashSet::new(),
                quic_enabled: false,
                quic_datagrams_enabled: false,
                quic_datagram_max_payload_bytes: 0,
                quic_dialer: None,
                local_scion_supported: false,
                tls_enabled: false,
                tls_fallback_to_plain: false,
                prefer_ws_fallback: false,
                proxy_policy: crate::transport::ProxyPolicy::disabled(),
                proxy_tls_verify: true,
                proxy_tls_pinned_cert_der: None,
                allowlist_only: false,
                allow_keys: HashSet::new(),
                deny_keys: HashSet::new(),
                allow_nets: Vec::new(),
                deny_nets: Vec::new(),
                retry_backoff: HashMap::new(),
                peer_session_generation: HashMap::new(),
                pending_connects: Vec::new(),
                deferred_send_queue: DeferredPeerFrameQueue::new(
                    iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
                    Duration::from_millis(
                        iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
                    ),
                ),
                happy_eyeballs_stagger: Duration::from_millis(10),
                topology_update_interval:
                    iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
                addr_ipv6_first: false,
                last_active: HashMap::new(),
                incoming_pending: HashSet::new(),
                incoming_active: HashSet::new(),
                max_incoming: None,
                max_total_connections: None,
                accept_params: AcceptThrottleParams::new(
                    None,
                    None,
                    iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                    iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
                    None,
                    None,
                    iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
                    iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
                ),
                accept_prefix_buckets: HashMap::new(),
                accept_ip_buckets: HashMap::new(),
                sampler_high_queue_warn: LogSampler::new(),
                sampler_low_queue_warn: LogSampler::new(),
                sampler_accept_err: LogSampler::new(),
                low_rate_per_sec: None,
                low_burst: None,
                low_buckets: HashMap::new(),
                low_bytes_per_sec: None,
                low_bytes_burst: None,
                low_bytes_buckets: HashMap::new(),
                max_frame_bytes: 4096,
                cap_consensus: 128,
                cap_control: 128,
                cap_block_sync: 512,
                cap_tx_gossip: 128,
                cap_peer_gossip: 128,
                cap_health: 128,
                cap_other: 128,
                disconnect_on_post_overflow: false,
                _key_exchange: core::marker::PhantomData,
                _encryptor: core::marker::PhantomData,
            };

        let before = cap_violations_consensus();

        let peer = Peer::new(
            listen_addr_std.into(),
            PeerId::from(key_pair.public_key().clone()),
        );
        let within_block_sync_cap = super::PeerMessage {
            peer: peer.clone(),
            payload: RelayMessage::new(
                peer.id().clone(),
                RelayTarget::Direct(network.self_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyConsensusPayload,
            ),
            payload_bytes: 256,
        };
        network.peer_message(within_block_sync_cap).await;
        let after_within = cap_violations_consensus();
        assert!(
            after_within >= before,
            "cap violation counter should not decrease"
        );

        let oversized = super::PeerMessage {
            peer: peer.clone(),
            payload: RelayMessage::new(
                peer.id().clone(),
                RelayTarget::Direct(network.self_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyConsensusPayload,
            ),
            payload_bytes: 1024,
        };
        network.peer_message(oversized).await;
        let after_oversized = cap_violations_consensus();
        assert!(
            after_oversized >= after_within + 1,
            "oversized consensus payload should be dropped"
        );
        assert!(
            network.retry_backoff.contains_key(peer.id()),
            "cap violation should schedule backoff for offending peer"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    async fn peer_message_consensus_chunk_uses_block_sync_cap() {
        use crate::network::cap_violations_block_sync;
        let _guard = cap_violation_test_guard();

        let key_pair = KeyPair::random();

        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("listener bind failed: {e:?}"),
        };
        let listen_addr_std = std_listener.local_addr().unwrap();
        let listener_clone = std_listener.try_clone().unwrap();
        listener_clone.set_nonblocking(true).unwrap();
        let listener = match TcpListener::from_std(listener_clone) {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("tcp stream from_std failed: {e:?}"),
        };

        let (_subscribe_tx, subscribe_rx): (
            mpsc::UnboundedSender<super::Subscriber<DummyConsensusChunk>>,
            _,
        ) = mpsc::unbounded_channel();
        let (_update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<super::message::UpdateTopology>();
        let (_update_peers_tx, update_peers_rx) =
            mpsc::unbounded_channel::<super::message::UpdatePeers>();
        let (_update_trusted_tx, update_trusted_peers_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateTrustedPeers>();
        let (_update_acl_tx, update_acl_rx) =
            mpsc::unbounded_channel::<super::message::UpdateAcl>();
        let (_update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<super::message::UpdateHandshake>();
        let (_update_consensus_caps_tx, update_consensus_caps_receiver) =
            mpsc::unbounded_channel::<super::message::UpdateConsensusCaps>();
        let (peer_message_hi_tx, peer_message_hi_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensusChunk>>>(1);
        let (peer_message_lo_tx, peer_message_lo_rx) =
            mpsc::channel::<super::PeerMessage<WireMessage<DummyConsensusChunk>>>(1);
        let (service_message_tx, service_message_rx) =
            mpsc::channel::<super::ServiceMessage<WireMessage<DummyConsensusChunk>>>(4);
        let (_hi_tx, network_message_high_rx) = super::net_channel::channel_with_capacity(1);
        let (_lo_tx, network_message_low_rx) = super::net_channel::channel_with_capacity(1);
        let (online_peers_tx, _online_peers_rx) = watch::channel(HashSet::new());
        let (online_peer_capabilities_tx, _online_peer_capabilities_rx) =
            watch::channel::<super::message::OnlinePeerCapabilities>(HashMap::new());
        let (_update_peer_capabilities_tx, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel::<super::message::UpdatePeerCapabilities>();

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        let mut network = super::NetworkBase::<DummyConsensusChunk, X25519Sha256, ChaCha20Poly1305> {
            listen_addr: listen_addr_std.into(),
            public_address: listen_addr_std.into(),
            relay_role: RelayRole::Disabled,
            relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_hub_peer: None,
            relay_trusted_peers: HashSet::new(),
            relay_ttl: DEFAULT_RELAY_TTL,
            trust_gossip_config: true,
            trust_gossip: true,
            self_id: PeerId::from(key_pair.public_key().clone()),
            address_book: HashMap::new(),
            peer_reputations: PeerReputationBook::default(),
            soranet_handshake: soranet.clone(),
            peers: HashMap::new(),
            connecting_peers: HashMap::new(),
            listener,
            key_pair: key_pair.clone(),
            subscribers_to_peers_messages: Vec::new(),
            subscribe_to_peers_messages_receiver: subscribe_rx,
            online_peers_sender: online_peers_tx,
            online_peer_capabilities_sender: online_peer_capabilities_tx,
            update_topology_receiver: update_topology_rx,
            update_peers_receiver: update_peers_rx,
            update_peer_capabilities_receiver,
            update_trusted_peers_receiver,
            update_acl_receiver: update_acl_rx,
            network_message_high_receiver: network_message_high_rx,
            network_message_low_receiver: network_message_low_rx,
            peer_message_high_receiver: peer_message_hi_rx,
            peer_message_low_receiver: peer_message_lo_rx,
            peer_message_high_sender: peer_message_hi_tx,
            peer_message_low_sender: peer_message_lo_tx,
            service_message_receiver: service_message_rx,
            service_message_sender: service_message_tx,
            update_handshake_receiver: update_handshake_rx,
            update_consensus_caps_receiver,
            current_conn_id: 0,
            current_topology: HashSet::new(),
            current_peers_addresses: Vec::new(),
            idle_timeout: Duration::from_millis(50),
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            tcp_nodelay: true,
            tcp_keepalive: None,
            connect_startup_delay_until: tokio::time::Instant::now(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            peer_capabilities: HashMap::new(),
            post_queue_cap: 4,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            dns_last_refresh: HashMap::new(),
            dns_pending_refresh: HashSet::new(),
            quic_enabled: false,
            quic_datagrams_enabled: false,
            quic_datagram_max_payload_bytes: 0,
            quic_dialer: None,
            local_scion_supported: false,
            tls_enabled: false,
            tls_fallback_to_plain: false,
            prefer_ws_fallback: false,
            proxy_policy: crate::transport::ProxyPolicy::disabled(),
            proxy_tls_verify: true,
            proxy_tls_pinned_cert_der: None,
            allowlist_only: false,
            allow_keys: HashSet::new(),
            deny_keys: HashSet::new(),
            allow_nets: Vec::new(),
            deny_nets: Vec::new(),
            retry_backoff: HashMap::new(),
            peer_session_generation: HashMap::new(),
            pending_connects: Vec::new(),
            deferred_send_queue: DeferredPeerFrameQueue::new(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
                Duration::from_millis(
                    iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
                ),
            ),
            happy_eyeballs_stagger: Duration::from_millis(10),
            topology_update_interval:
                iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            addr_ipv6_first: false,
            last_active: HashMap::new(),
            incoming_pending: HashSet::new(),
            incoming_active: HashSet::new(),
            max_incoming: None,
            max_total_connections: None,
            accept_params: AcceptThrottleParams::new(
                None,
                None,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
                None,
                None,
                iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
                iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            ),
            accept_prefix_buckets: HashMap::new(),
            accept_ip_buckets: HashMap::new(),
            sampler_high_queue_warn: LogSampler::new(),
            sampler_low_queue_warn: LogSampler::new(),
            sampler_accept_err: LogSampler::new(),
            low_rate_per_sec: None,
            low_burst: None,
            low_buckets: HashMap::new(),
            low_bytes_per_sec: None,
            low_bytes_burst: None,
            low_bytes_buckets: HashMap::new(),
            max_frame_bytes: 4096,
            cap_consensus: 128,
            cap_control: 128,
            cap_block_sync: 512,
            cap_tx_gossip: 128,
            cap_peer_gossip: 128,
            cap_health: 128,
            cap_other: 128,
            disconnect_on_post_overflow: false,
            _key_exchange: core::marker::PhantomData,
            _encryptor: core::marker::PhantomData,
        };

        let before = cap_violations_block_sync();

        let peer = Peer::new(
            listen_addr_std.into(),
            PeerId::from(key_pair.public_key().clone()),
        );
        let within_block_sync_cap = super::PeerMessage {
            peer: peer.clone(),
            payload: RelayMessage::new(
                peer.id().clone(),
                RelayTarget::Direct(network.self_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyConsensusChunk,
            ),
            payload_bytes: 256,
        };
        network.peer_message(within_block_sync_cap).await;
        let after_within = cap_violations_block_sync();
        assert!(
            after_within >= before,
            "cap violation counter should not decrease"
        );

        let oversized = super::PeerMessage {
            peer: peer.clone(),
            payload: RelayMessage::new(
                peer.id().clone(),
                RelayTarget::Direct(network.self_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyConsensusChunk,
            ),
            payload_bytes: 1024,
        };
        network.peer_message(oversized).await;
        let after_oversized = cap_violations_block_sync();
        assert!(
            after_oversized >= after_within + 1,
            "oversized consensus chunk should be dropped"
        );
        assert!(
            network.retry_backoff.contains_key(peer.id()),
            "cap violation should schedule backoff for offending peer"
        );
    }

    #[test]
    fn overflow_counters_high_low_per_topic() {
        use super::message::Topic::*;
        // Capture base totals
        let base_total = super::post_overflow_count();
        let base_hi_cons = super::post_overflow_consensus_high_count();
        let base_lo_cons = super::post_overflow_consensus_low_count();
        // Simulate two overflows: one High/Consensus, one Low/Consensus
        super::POST_OVERFLOWS.fetch_add(2, std::sync::atomic::Ordering::Relaxed);
        super::inc_post_overflow_for(Consensus);
        super::inc_post_overflow_for_prio(Consensus, true);
        super::inc_post_overflow_for(Consensus);
        super::inc_post_overflow_for_prio(Consensus, false);

        assert_eq!(super::post_overflow_count(), base_total + 2);
        assert_eq!(
            super::post_overflow_consensus_high_count(),
            base_hi_cons + 1
        );
        assert_eq!(super::post_overflow_consensus_low_count(), base_lo_cons + 1);
    }

    #[test]
    fn overflow_counters_full_matrix() {
        use std::sync::atomic::Ordering::Relaxed;

        use super::message::Topic::*;

        // Snapshot bases
        let base_total = super::post_overflow_count();
        let b_hi = (
            super::post_overflow_consensus_high_count(),
            super::post_overflow_control_high_count(),
            super::post_overflow_block_sync_high_count(),
            super::post_overflow_tx_gossip_high_count(),
            super::post_overflow_peer_gossip_high_count(),
            super::post_overflow_health_high_count(),
            super::post_overflow_other_high_count(),
        );
        let b_lo = (
            super::post_overflow_consensus_low_count(),
            super::post_overflow_control_low_count(),
            super::post_overflow_block_sync_low_count(),
            super::post_overflow_tx_gossip_low_count(),
            super::post_overflow_peer_gossip_low_count(),
            super::post_overflow_health_low_count(),
            super::post_overflow_other_low_count(),
        );

        let topics = [
            Consensus,
            ConsensusChunk,
            Control,
            BlockSync,
            TxGossip,
            TxGossipRestricted,
            PeerGossip,
            TrustGossip,
            Health,
            Other,
        ];
        for &t in &topics {
            // High increment
            super::POST_OVERFLOWS.fetch_add(1, Relaxed);
            super::inc_post_overflow_for(t);
            super::inc_post_overflow_for_prio(t, true);
            // Low increment
            super::POST_OVERFLOWS.fetch_add(1, Relaxed);
            super::inc_post_overflow_for(t);
            super::inc_post_overflow_for_prio(t, false);
        }

        // Assert total grew by at least 16 (allowing for concurrent increments in other tests)
        assert!(super::post_overflow_count() >= base_total + 16);

        // Read back highs and lows
        let a_hi = (
            super::post_overflow_consensus_high_count(),
            super::post_overflow_control_high_count(),
            super::post_overflow_block_sync_high_count(),
            super::post_overflow_tx_gossip_high_count(),
            super::post_overflow_peer_gossip_high_count(),
            super::post_overflow_health_high_count(),
            super::post_overflow_other_high_count(),
        );
        let a_lo = (
            super::post_overflow_consensus_low_count(),
            super::post_overflow_control_low_count(),
            super::post_overflow_block_sync_low_count(),
            super::post_overflow_tx_gossip_low_count(),
            super::post_overflow_peer_gossip_low_count(),
            super::post_overflow_health_low_count(),
            super::post_overflow_other_low_count(),
        );

        // Each topic should have increased by at least 1 in both High and Low buckets
        assert!(a_hi.0 > b_hi.0);
        assert!(a_hi.1 > b_hi.1);
        assert!(a_hi.2 > b_hi.2);
        assert!(a_hi.3 > b_hi.3);
        assert!(a_hi.4 > b_hi.4);
        assert!(a_hi.5 > b_hi.5);
        assert!(a_hi.6 > b_hi.6);

        assert!(a_lo.0 > b_lo.0);
        assert!(a_lo.1 > b_lo.1);
        assert!(a_lo.2 > b_lo.2);
        assert!(a_lo.3 > b_lo.3);
        assert!(a_lo.4 > b_lo.4);
        assert!(a_lo.5 > b_lo.5);
        assert!(a_lo.6 > b_lo.6);
    }
}

#[cfg(test)]
mod reputation_tests {
    use std::collections::HashSet;

    use iroha_crypto::KeyPair;

    use super::*;

    #[test]
    fn trust_and_scores_update() {
        let id1 = PeerId::from(KeyPair::random().public_key().clone());
        let id2 = PeerId::from(KeyPair::random().public_key().clone());
        let mut rep = PeerReputationBook::default();

        rep.record_connected(&id1);
        rep.record_disconnected(&id2);

        let mut trusted = HashSet::new();
        trusted.insert(id1.clone());
        rep.set_trusted(&trusted);

        let trusted_ids: HashSet<_> = rep.trusted_peers().into_iter().collect();
        assert!(trusted_ids.contains(&id1));
        assert!(!trusted_ids.contains(&id2));

        let snap = rep.snapshot();
        let r1 = snap.get(&id1).expect("id1 present");
        assert!(r1.trusted);
        assert!(r1.score > 0);

        let r2 = snap.get(&id2).expect("id2 present");
        assert!(!r2.trusted);
        assert_eq!(r2.score, 0);
    }
}

#[cfg(feature = "quic")]
#[allow(clippy::too_many_arguments)]
async fn start_quic_listener<T, K, E>(
    addr: &std::net::SocketAddr,
    key_pair: iroha_crypto::KeyPair,
    public_address: iroha_primitives::addr::SocketAddr,
    service_message_sender: tokio::sync::mpsc::Sender<crate::peer::message::ServiceMessage<T>>,
    idle_timeout: std::time::Duration,
    quic_max_idle_timeout: Option<std::time::Duration>,
    quic_datagrams_enabled: bool,
    quic_datagram_max_payload_bytes: usize,
    quic_datagram_receive_buffer_bytes: usize,
    quic_datagram_send_buffer_bytes: usize,
    chain_id: Option<iroha_data_model::ChainId>,
    consensus_caps: Option<crate::ConsensusHandshakeCaps>,
    confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
    crypto_caps: Option<crate::CryptoHandshakeCaps>,
    post_capacity: usize,
    trust_gossip: bool,
    max_frame_bytes: usize,
    soranet_handshake: Arc<SoranetHandshakeConfig>,
    local_scion_supported: bool,
    relay_role: RelayRole,
) -> Result<(), Error>
where
    T: Pload + message::ClassifyTopic,
    K: Kex,
    E: Enc,
{
    use std::sync::Arc;

    use quinn::{
        IdleTimeout, TransportConfig, crypto::rustls::QuicServerConfig as QuinnRustlsServerConfig,
    };
    use rustls::pki_types::PrivatePkcs8KeyDer;

    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(["iroha-quic".to_owned()])
            .map_err(|e| Error::from(std::io::Error::other(format!("rcgen: {e}"))))?;
    let cert_der = cert.der().clone().into_owned();
    let transport_binding = crate::transport::certificate_fingerprint(cert_der.as_ref());
    let priv_key = PrivatePkcs8KeyDer::from(signing_key.serialize_der());

    let mut tls = rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], priv_key.into())
        .map_err(|e| Error::from(std::io::Error::other(format!("rustls server config: {e}"))))?;
    tls.max_early_data_size = u32::MAX;
    tls.alpn_protocols = vec![crate::transport::quic::P2P_ALPN.to_vec()];

    let crypto = QuinnRustlsServerConfig::try_from(Arc::new(tls))
        .map_err(|e| Error::from(std::io::Error::other(format!("quic rustls: {e}"))))?;

    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(crypto));

    // Align transport tuning with the outbound dialer defaults.
    let mut transport = TransportConfig::default();
    if let Some(timeout) = quic_max_idle_timeout {
        let idle = IdleTimeout::try_from(timeout)
            .map_err(|e| Error::from(std::io::Error::other(format!("quic idle timeout: {e}"))))?;
        transport.max_idle_timeout(Some(idle));
    }
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(10)));
    if quic_datagrams_enabled {
        transport.datagram_receive_buffer_size(Some(quic_datagram_receive_buffer_bytes));
        transport.datagram_send_buffer_size(quic_datagram_send_buffer_bytes);
    }
    server_config.transport_config(Arc::new(transport));

    let endpoint = quinn::Endpoint::server(server_config, *addr)
        .map_err(|e| Error::from(std::io::Error::other(format!("endpoint: {e}"))))?;

    // Allocate unique connection ids for QUIC streams starting from a high range
    // to minimize collision with TCP-generated ids.
    let id_alloc = NEXT_QUIC_CONN_ID.get_or_init(|| AtomicU64::new(1 << 60));

    let listen_addr = *addr;
    tokio::spawn(async move {
        iroha_logger::info!(addr=%listen_addr, "QUIC listener started");
        while let Some(incoming) = endpoint.accept().await {
            let service_message_sender = service_message_sender.clone();
            let key_pair = key_pair.clone();
            let public_address = public_address.clone();
            let chain_id = chain_id.clone();
            let consensus_caps = consensus_caps.clone();
            let confidential_caps = confidential_caps.clone();
            let crypto_caps = crypto_caps.clone();
            let idle_timeout = idle_timeout;
            let post_capacity = post_capacity;
            let soranet_handshake = soranet_handshake.clone();
            let relay_role = relay_role;
            let trust_gossip = trust_gossip;
            let transport_binding = transport_binding;
            tokio::spawn(async move {
                match incoming.accept() {
                    Ok(connecting) => {
                        let new_conn = match connecting.await {
                            Ok(conn) => conn,
                            Err(e) => {
                                iroha_logger::warn!(%e, "QUIC handshake failed");
                                return;
                            }
                        };
                        let remote = new_conn.remote_address();
                        let conn_id = id_alloc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        // Ask the network actor whether to accept this inbound under caps/throttle
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        let ask = ServiceMessage::InboundAsk {
                            conn_id,
                            remote_addr: remote,
                            reply: tx,
                        };
                        if service_message_sender.send(ask).await.is_err() {
                            iroha_logger::debug!(%remote, "Network dropped InboundAsk channel");
                            return;
                        }
                        let Ok(allow) = rx.await else {
                            return;
                        };
                        if !allow {
                            iroha_logger::debug!(%remote, "Dropping QUIC connection due to caps/throttle");
                            return;
                        }

                        let (send_hi, recv_hi) = match new_conn.accept_bi().await {
                            Ok((send, recv)) => (send, recv),
                            Err(e) => {
                                iroha_logger::warn!(%e, %remote, "Failed to accept QUIC bi-stream");
                                return;
                            }
                        };

                        // Optional low-priority bi-stream (not required; keep a short timeout).
                        let low = tokio::time::timeout(
                            std::time::Duration::from_millis(200),
                            new_conn.accept_bi(),
                        )
                        .await;
                        let (send_low, recv_low) = match low {
                            Ok(Ok((s, r))) => (Some(s), Some(r)),
                            Ok(Err(e)) => {
                                iroha_logger::debug!(%e, %remote, "Failed to accept low-priority QUIC stream; continuing with single stream");
                                (None, None)
                            }
                            Err(_) => (None, None),
                        };

                        let _ = service_message_sender
                            .send(ServiceMessage::InboundPending(conn_id))
                            .await;
                        connected_from::<T, K, E>(
                            public_address.clone(),
                            key_pair,
                            Connection::from_quic(
                                conn_id,
                                new_conn.clone(),
                                send_hi,
                                recv_hi,
                                send_low,
                                recv_low,
                                Some(remote),
                                Some(transport_binding),
                            ),
                            service_message_sender,
                            idle_timeout,
                            chain_id,
                            consensus_caps,
                            confidential_caps.clone(),
                            crypto_caps.clone(),
                            soranet_handshake.clone(),
                            local_scion_supported,
                            post_capacity,
                            relay_role,
                            trust_gossip,
                            max_frame_bytes,
                            quic_datagrams_enabled,
                            quic_datagram_max_payload_bytes,
                        );
                    }
                    Err(e) => {
                        iroha_logger::warn!(%e, "Failed to accept QUIC connection");
                    }
                }
            });
        }
    });
    Ok(())
}

#[cfg(all(test, feature = "quic"))]
mod quic_tests {
    use std::sync::Arc;

    use iroha_crypto::{KeyPair, encryption::ChaCha20Poly1305, kex::X25519Sha256};
    use iroha_primitives::addr::socket_addr;
    use norito::codec::{Decode, Encode};

    use super::*;

    #[derive(Clone, Debug, Decode, Encode)]
    struct Dummy;

    impl<'a> ncore::DecodeFromSlice<'a> for Dummy {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            ncore::decode_field_canonical::<Self>(bytes)
        }
    }

    impl crate::network::message::ClassifyTopic for Dummy {}

    #[tokio::test(flavor = "current_thread")]
    async fn quic_listener_scaffold_starts() {
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let kp = KeyPair::random();
        let (tx, _rx) = tokio::sync::mpsc::channel::<
            crate::peer::message::ServiceMessage<WireMessage<Dummy>>,
        >(1);
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        if let Err(err) = start_quic_listener::<WireMessage<Dummy>, X25519Sha256, ChaCha20Poly1305>(
            &addr,
            kp,
            socket_addr!(127.0.0.1:1337),
            tx,
            std::time::Duration::from_secs(1),
            None,
            false,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
            1,
            true,
            1_048_576,
            soranet,
            true,
            RelayRole::Disabled,
        )
        .await
        {
            if let Error::Io(io_err) = &err {
                if io_err.kind() == std::io::ErrorKind::PermissionDenied
                    || io_err.to_string().contains("Operation not permitted")
                {
                    return;
                }
            }
            panic!("scaffold should start without error: {err:?}");
        }
    }
}

#[cfg(feature = "p2p_tls")]
#[allow(clippy::too_many_arguments)]
async fn start_tls_listener<T, K, E>(
    addr: std::net::SocketAddr,
    key_pair: iroha_crypto::KeyPair,
    public_address: iroha_primitives::addr::SocketAddr,
    service_message_sender: tokio::sync::mpsc::Sender<crate::peer::message::ServiceMessage<T>>,
    idle_timeout: std::time::Duration,
    chain_id: Option<iroha_data_model::ChainId>,
    consensus_caps: Option<crate::ConsensusHandshakeCaps>,
    confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
    crypto_caps: Option<crate::CryptoHandshakeCaps>,
    post_capacity: usize,
    trust_gossip: bool,
    max_frame_bytes: usize,
    quic_datagrams_enabled: bool,
    quic_datagram_max_payload_bytes: usize,
    soranet_handshake: Arc<SoranetHandshakeConfig>,
    local_scion_supported: bool,
    relay_role: RelayRole,
    tcp_nodelay: bool,
    tcp_keepalive: Option<std::time::Duration>,
) -> Result<(), Error>
where
    T: boilerplate::Pload + message::ClassifyTopic,
    K: boilerplate::Kex,
    E: boilerplate::Enc,
{
    // Generate a self-signed certificate for the TLS server.
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(["iroha-tls".to_owned()])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("rcgen: {e}")))?;
    let cert_der = cert.der().clone();
    let transport_binding = crate::transport::certificate_fingerprint(cert_der.as_ref());

    let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der).into_owned()];
    let priv_key = rustls::pki_types::PrivateKeyDer::from(
        rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der()),
    )
    .clone_key();

    let mut server_cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, priv_key)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("tls config: {e}")))?;
    // Allow all ALPN; app-layer handshake authenticates peers
    server_cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(server_cfg));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Unique conn ids for TLS path
    static NEXT_TLS_CONN_ID: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
        std::sync::OnceLock::new();
    let id_alloc = NEXT_TLS_CONN_ID.get_or_init(|| std::sync::atomic::AtomicU64::new(1 << 59));

    tokio::spawn(async move {
        iroha_logger::info!(addr=%addr, "TLS listener started");
        loop {
            let Ok((tcp, remote)) = listener.accept().await else {
                break;
            };
            let service_message_sender = service_message_sender.clone();
            let key_pair = key_pair.clone();
            let public_address = public_address.clone();
            let chain_id = chain_id.clone();
            let consensus_caps = consensus_caps.clone();
            let confidential_caps = confidential_caps.clone();
            let crypto_caps = crypto_caps.clone();
            let acceptor = acceptor.clone();
            let idle_timeout = idle_timeout;
            let post_capacity = post_capacity;
            let soranet_handshake = soranet_handshake.clone();
            let relay_role = relay_role;
            let tcp_nodelay = tcp_nodelay;
            let tcp_keepalive = tcp_keepalive;
            let quic_datagrams_enabled = quic_datagrams_enabled;
            let quic_datagram_max_payload_bytes = quic_datagram_max_payload_bytes;
            let transport_binding = transport_binding;

            tokio::spawn(async move {
                let conn_id = id_alloc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let (tx, rx) = tokio::sync::oneshot::channel();
                let ask = ServiceMessage::InboundAsk {
                    conn_id,
                    remote_addr: remote,
                    reply: tx,
                };
                if service_message_sender.send(ask).await.is_err() {
                    iroha_logger::debug!(%remote, "Network dropped InboundAsk channel");
                    return;
                }
                let Ok(allow) = rx.await else {
                    return;
                };
                if !allow {
                    iroha_logger::debug!(%remote, "Dropping TLS connection due to caps/throttle");
                    return;
                }
                crate::transport::apply_tcp_socket_options(&tcp, tcp_nodelay, tcp_keepalive);
                match acceptor.accept(tcp).await {
                    Ok(tls_stream) => {
                        let (read_half, write_half) = tokio::io::split(tls_stream);
                        // Register pending
                        let _ = service_message_sender
                            .send(ServiceMessage::InboundPending(conn_id))
                            .await;
                        connected_from::<T, K, E>(
                            public_address,
                            key_pair,
                            Connection::from_split_with_binding(
                                conn_id,
                                read_half,
                                write_half,
                                Some(transport_binding),
                            ),
                            service_message_sender,
                            idle_timeout,
                            chain_id,
                            consensus_caps,
                            confidential_caps.clone(),
                            crypto_caps.clone(),
                            soranet_handshake.clone(),
                            local_scion_supported,
                            post_capacity,
                            relay_role,
                            trust_gossip,
                            max_frame_bytes,
                            quic_datagrams_enabled,
                            quic_datagram_max_payload_bytes,
                        );
                    }
                    Err(e) => {
                        iroha_logger::warn!(%e, %remote, "TLS accept failed");
                    }
                }
            });
        }
    });
    Ok(())
}

/// Base network layer structure, holding connections interacting with peers.
#[allow(clippy::struct_excessive_bools)]
struct NetworkBase<T: Pload, K: Kex, E: Enc> {
    /// Listening address for incoming connections. Must parse into [`std::net::SocketAddr`]
    listen_addr: SocketAddr,
    /// External address of the peer (as seen by other peers)
    public_address: SocketAddr,
    /// Local relay role advertised during handshake.
    relay_role: RelayRole,
    /// Relay mode configured for this node.
    relay_mode: iroha_config::parameters::actual::RelayMode,
    /// Relay hub addresses to dial when in `spoke` / `assist` mode (priority order).
    ///
    /// When multiple hubs are provided, the network may rotate through them to
    /// maintain reachability in the presence of firewalls/censorship.
    relay_hub_addresses: Vec<SocketAddr>,
    /// Hub peer id resolved from topology/address book (spoke mode).
    relay_hub_peer: Option<PeerId>,
    /// Trusted relay peers derived from `relay_hub_addresses` and peer address mapping.
    ///
    /// Peers in this set are allowed to send frames where `origin != incoming_peer_id`
    /// (i.e., forwarded/relayed frames).
    relay_trusted_peers: HashSet<PeerId>,
    /// Hop limit for forwarded frames.
    relay_ttl: u8,
    /// Configured trust-gossip capability for this node.
    trust_gossip_config: bool,
    /// Whether this node advertises trust-gossip support.
    trust_gossip: bool,
    /// Local peer identifier (derived from key pair).
    self_id: PeerId,
    /// Known peer addresses keyed by peer id.
    address_book: HashMap<PeerId, SocketAddr>,
    /// Local view of peer trust/score.
    peer_reputations: PeerReputationBook,
    /// `SoraNet` handshake runtime configuration shared across peers.
    soranet_handshake: Arc<SoranetHandshakeConfig>,
    /// Current [`Peer`]s in [`Peer::Ready`] state.
    peers: HashMap<PeerId, RefPeer<WireMessage<T>>>,
    /// [`Peer`]s in process of being connected.
    connecting_peers: HashMap<ConnectionId, Peer>,
    /// [`TcpListener`] that is accepting [`Peer`]s' connections
    listener: TcpListener,
    /// Our app-level key pair
    key_pair: KeyPair,
    /// Recipients of messages received from other peers in the network.
    subscribers_to_peers_messages: Vec<Subscriber<T>>,
    /// Receiver to subscribe for messages received from other peers in the network.
    subscribe_to_peers_messages_receiver: mpsc::UnboundedReceiver<Subscriber<T>>,
    /// Sender of `OnlinePeer` message
    online_peers_sender: watch::Sender<OnlinePeers>,
    /// Sender of online peer transport capabilities.
    online_peer_capabilities_sender: watch::Sender<message::OnlinePeerCapabilities>,
    /// [`UpdateTopology`] message receiver
    update_topology_receiver: mpsc::UnboundedReceiver<UpdateTopology>,
    /// [`UpdatePeers`] message receiver
    update_peers_receiver: mpsc::UnboundedReceiver<UpdatePeers>,
    /// [`UpdatePeerCapabilities`] message receiver
    update_peer_capabilities_receiver: mpsc::UnboundedReceiver<message::UpdatePeerCapabilities>,
    /// Trusted peers update receiver.
    update_trusted_peers_receiver: mpsc::UnboundedReceiver<UpdateTrustedPeers>,
    /// Receiver of high priority [`NetworkMessage`]
    network_message_high_receiver: net_channel::Receiver<NetworkMessage<T>>,
    /// Receiver of low priority [`NetworkMessage`]
    network_message_low_receiver: net_channel::Receiver<NetworkMessage<T>>,
    /// High-priority inbound peer messages (consensus/control).
    peer_message_high_receiver: mpsc::Receiver<PeerMessage<WireMessage<T>>>,
    /// Low-priority inbound peer messages (gossip/sync).
    peer_message_low_receiver: mpsc::Receiver<PeerMessage<WireMessage<T>>>,
    /// Sender for high-priority peer messages to provide clone inside peer.
    peer_message_high_sender: mpsc::Sender<PeerMessage<WireMessage<T>>>,
    /// Sender for low-priority peer messages to provide clone inside peer.
    peer_message_low_sender: mpsc::Sender<PeerMessage<WireMessage<T>>>,
    /// Channel to gather service messages from all peers
    service_message_receiver: mpsc::Receiver<ServiceMessage<WireMessage<T>>>,
    /// Sender for service peer messages to provide clone of sender inside peer
    service_message_sender: mpsc::Sender<ServiceMessage<WireMessage<T>>>,
    /// ACL update receiver
    update_acl_receiver: mpsc::UnboundedReceiver<message::UpdateAcl>,
    /// Handshake update receiver
    update_handshake_receiver: mpsc::UnboundedReceiver<message::UpdateHandshake>,
    /// Consensus handshake caps update receiver.
    update_consensus_caps_receiver: mpsc::UnboundedReceiver<message::UpdateConsensusCaps>,
    /// Current available connection id
    current_conn_id: ConnectionId,
    /// Current topology
    current_topology: HashSet<PeerId>,
    /// Peers which are not yet connected, but should.
    ///
    /// Can have two addresses for same `PeerId`.
    /// * One initially provided via config
    /// * Second received from other peers via gossiping
    ///
    /// Will try to establish connection via both addresses.
    current_peers_addresses: Vec<(PeerId, SocketAddr)>,
    /// Optional `ChainId` to include in handshake signature binding.
    chain_id: Option<ChainId>,
    /// Optional consensus handshake capabilities for gating connections.
    consensus_caps: Option<crate::ConsensusHandshakeCaps>,
    /// Optional confidential handshake capabilities for gating connections.
    confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
    /// Optional crypto handshake capabilities for gating connections.
    crypto_caps: Option<crate::CryptoHandshakeCaps>,
    /// Known peer transport capabilities keyed by peer id.
    peer_capabilities: HashMap<PeerId, message::PeerTransportCapabilities>,
    /// Per-peer post channel capacity (bounded mode).
    post_queue_cap: usize,
    /// Optional interval to refresh hostname-based peers.
    dns_refresh_interval: Option<Duration>,
    /// Optional TTL to refresh hostname-based peers individually.
    dns_refresh_ttl: Option<Duration>,
    /// Last refresh time per peer for TTL logic.
    dns_last_refresh: HashMap<PeerId, tokio::time::Instant>,
    /// Interval between topology refresh ticks.
    topology_update_interval: Duration,
    /// Enable QUIC transport based on config at runtime.
    #[allow(dead_code)]
    quic_enabled: bool,
    /// Enable QUIC DATAGRAM support for best-effort topics when using QUIC.
    quic_datagrams_enabled: bool,
    /// Upper bound (bytes) for QUIC datagram payloads.
    quic_datagram_max_payload_bytes: usize,
    /// Shared outbound QUIC dialer endpoint (feature-gated).
    #[allow(dead_code)]
    quic_dialer: Option<crate::transport::QuicDialer>,
    /// Whether this node can advertise/use SCION-preferred transport.
    local_scion_supported: bool,
    /// Enable TLS-over-TCP transport based on config at runtime (feature-gated).
    #[allow(dead_code)]
    tls_enabled: bool,
    /// When TLS-over-TCP is enabled, fall back to plain TCP if the TLS dial fails.
    tls_fallback_to_plain: bool,
    /// Prefer WS fallback for outbound (feature-gated via caller).
    #[allow(dead_code)]
    prefer_ws_fallback: bool,
    /// Proxy policy applied to outbound TCP dials.
    proxy_policy: crate::transport::ProxyPolicy,
    /// Whether to verify TLS certificates when dialing an `https://` proxy.
    proxy_tls_verify: bool,
    /// Optional pinned end-entity certificate for `https://` proxies (DER).
    proxy_tls_pinned_cert_der: Option<std::sync::Arc<[u8]>>,
    /// ACL: Allowlist-only switch and lists of keys and networks
    allowlist_only: bool,
    allow_keys: std::collections::HashSet<iroha_crypto::PublicKey>,
    deny_keys: std::collections::HashSet<iroha_crypto::PublicKey>,
    allow_nets: Vec<IpNet>,
    deny_nets: Vec<IpNet>,
    /// Peers pending reconnect after a DNS refresh.
    dns_pending_refresh: HashSet<PeerId>,
    /// Duration after which terminate connection with idle peer
    idle_timeout: Duration,
    /// Timeout applied to an individual outbound dial attempt.
    dial_timeout: Duration,
    /// Whether to enable `TCP_NODELAY` on TCP connections (best-effort).
    tcp_nodelay: bool,
    /// Optional TCP keepalive idle timeout (best-effort, platform-specific).
    tcp_keepalive: Option<Duration>,
    /// Outbound dial delay applied once at startup.
    connect_startup_delay_until: tokio::time::Instant,
    /// Per-address exponential backoff schedule per peer: next allowed retry time and current base delay.
    /// Keyed by peer id, then by address string.
    retry_backoff: HashMap<PeerId, HashMap<String, (tokio::time::Instant, Duration)>>,
    /// Last observed connection generation token per peer.
    peer_session_generation: HashMap<PeerId, ConnectionId>,
    /// Pending scheduled connect attempts with staggers
    pending_connects: Vec<(tokio::time::Instant, Peer)>,
    /// Deferred outbound frames queued while peer session is unavailable.
    deferred_send_queue: DeferredPeerFrameQueue<T>,
    /// Stagger delay between parallel address attempts for the same peer
    happy_eyeballs_stagger: Duration,
    /// Prefer IPv6 addresses first when ordering parallel dials
    addr_ipv6_first: bool,
    /// Track last-activity time per connected peer (updated on inbound peer messages)
    last_active: HashMap<PeerId, tokio::time::Instant>,
    /// Pending incoming accepts awaiting handshake (connection ids)
    incoming_pending: HashSet<ConnectionId>,
    /// Active incoming connections (connection ids)
    incoming_active: HashSet<ConnectionId>,
    /// Optional cap on number of incoming connections
    max_incoming: Option<usize>,
    /// Optional cap on total number of connections
    max_total_connections: Option<usize>,
    /// Accept throttle parameters (prefix + per-IP).
    accept_params: AcceptThrottleParams,
    /// Prefix-level accept throttle buckets.
    accept_prefix_buckets: HashMap<IpBucketKey, AcceptBucket>,
    /// Per-IP accept throttle buckets.
    accept_ip_buckets: HashMap<IpBucketKey, AcceptBucket>,
    /// Log sampling helpers to avoid repeated warnings flooding logs
    sampler_high_queue_warn: LogSampler,
    sampler_low_queue_warn: LogSampler,
    sampler_accept_err: LogSampler,
    /// Per-peer token buckets for low-priority messages
    low_rate_per_sec: Option<f64>,
    low_burst: Option<f64>,
    low_buckets: HashMap<PeerId, TokenBucket>,
    /// Optional bytes/sec limiter for Low-priority messages
    low_bytes_per_sec: Option<f64>,
    /// Optional bytes burst for Low-priority limiter
    low_bytes_burst: Option<f64>,
    /// Per-peer bytes token buckets
    low_bytes_buckets: HashMap<PeerId, TokenBucket>,
    /// Maximum allowed frame size (bytes)
    max_frame_bytes: usize,
    /// Per-topic frame caps
    cap_consensus: usize,
    cap_control: usize,
    cap_block_sync: usize,
    cap_tx_gossip: usize,
    cap_peer_gossip: usize,
    cap_health: usize,
    cap_other: usize,
    /// Whether to disconnect on per-peer post overflow (bounded channels)
    disconnect_on_post_overflow: bool,
    /// Key exchange used by network
    _key_exchange: core::marker::PhantomData<K>,
    /// Encryptor used by the network
    _encryptor: core::marker::PhantomData<E>,
}

impl<T: Pload + message::ClassifyTopic, K: Kex, E: Enc> NetworkBase<T, K, E> {
    fn update_soranet_handshake_config(
        &mut self,
        handshake: ActualSoranetHandshake,
    ) -> Result<(), Error> {
        self.soranet_handshake = runtime_from_handshake(handshake)?;
        self.trust_gossip = self.trust_gossip_config && self.soranet_handshake.trust_gossip();
        Ok(())
    }

    /// [`Self`] task.
    #[allow(clippy::too_many_lines)]
    #[log(skip(self, shutdown_signal), fields(listen_addr=%self.listen_addr, public_key=%self.key_pair.public_key()))]
    async fn run(mut self, shutdown_signal: ShutdownSignal) {
        let mut update_topology_interval =
            tokio::time::interval(topology_tick_interval(self.topology_update_interval));
        // Process pending staggered connects frequently to honor small staggers
        let mut pending_connects_interval = tokio::time::interval(Duration::from_millis(50));
        let mut dns_refresh_interval =
            self.dns_refresh_interval
                .map(tokio::time::interval)
                .map(|mut int| {
                    // Schedule first tick in the future to avoid immediate churn on startup
                    int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    int
                });
        // TTL check timer: coarse periodic check
        let mut dns_ttl_check = if self.dns_refresh_ttl.is_some() {
            Some(tokio::time::interval(Duration::from_secs(60)))
        } else {
            None
        };
        loop {
            let mut high_drained = 0usize;
            while high_drained < INBOUND_PEER_HIGH_BUDGET {
                match self.peer_message_high_receiver.try_recv() {
                    Ok(peer_message) => {
                        self.peer_message(peer_message).await;
                        high_drained = high_drained.saturating_add(1);
                    }
                    Err(
                        tokio::sync::mpsc::error::TryRecvError::Empty
                        | tokio::sync::mpsc::error::TryRecvError::Disconnected,
                    ) => break,
                }
            }
            tokio::select! {
                // Select is biased because we want to service messages to take priority over data messages.
                biased;
                // Subscribe messages is expected to exhaust at some point after starting network actor
                subscriber = self.subscribe_to_peers_messages_receiver.recv() => {
                    if let Some(subscriber) = subscriber {
                        self.subscribe_to_peers_messages(subscriber);
                    } else {
                        iroha_logger::warn!("unsubscribe channel closed; network actor shutting down");
                        break;
                    }
                }
                // Periodically refresh connections for hostname-based peers to re-resolve DNS
                _ = async {
                    match &mut dns_refresh_interval {
                        Some(int) => { int.tick().await; true }
                        None => false,
                    }
                }, if self.dns_refresh_interval.is_some() => {
                    self.refresh_hostnames();
                }
                // TTL-based selective refresh
                _ = async {
                    match &mut dns_ttl_check { Some(int) => { int.tick().await; true }, None => false }
                }, if self.dns_refresh_ttl.is_some() => {
                    self.refresh_hostnames_ttl();
                }
                // Update topology is relative low rate message (at most once every block)
                Some(update_topology) = self.update_topology_receiver.recv() => {
                    self.set_current_topology(update_topology);
                }
                Some(update_peers) = self.update_peers_receiver.recv() => {
                    self.set_current_peers_addresses(update_peers);
                }
                Some(update_capabilities) = self.update_peer_capabilities_receiver.recv() => {
                    self.set_peer_capabilities(update_capabilities);
                }
                // Apply ACL updates (hot reload)
                Some(acl) = self.update_acl_receiver.recv() => {
                    self.allowlist_only = acl.allowlist_only;
                    self.allow_keys = acl.allow_keys.into_iter().collect();
                    self.deny_keys = acl.deny_keys.into_iter().collect();
                    self.allow_nets = parse_cidrs(&acl.allow_cidrs);
                    self.deny_nets = parse_cidrs(&acl.deny_cidrs);
                    // Reconcile current topology with the new key rules
                    let updated: HashSet<_> = self
                        .current_topology
                        .iter()
                        .filter(|pid| {
                            let pk = pid.public_key();
                            !self.deny_keys.contains(pk)
                                && (!self.allowlist_only || self.allow_keys.contains(pk))
                        })
                        .cloned()
                        .collect();
                    self.current_topology = updated;
                }
                Some(handshake) = self.update_handshake_receiver.recv() => {
                    if let Err(err) = self.update_soranet_handshake_config(handshake.handshake) {
                        iroha_logger::error!(
                            error = %err,
                            "Failed to update SoraNet handshake configuration"
                        );
                    }
                }
                Some(consensus_caps) = self.update_consensus_caps_receiver.recv() => {
                    iroha_logger::info!(
                        mode_tag=?consensus_caps.caps.mode_tag,
                        drop_existing = consensus_caps.drop_existing_peers,
                        "Updating consensus handshake capabilities at runtime"
                    );
                    self.consensus_caps = Some(consensus_caps.caps);
                    if consensus_caps.drop_existing_peers {
                        let peers: Vec<_> = self.peers.keys().cloned().collect();
                        for peer_id in peers {
                            self.disconnect_peer(&peer_id);
                        }
                        self.update_topology();
                    }
                }
                // Frequency of update is relatively low, so it won't block other tasks from execution
                _ = update_topology_interval.tick() => {
                    self.update_topology()
                }
                Some(UpdateTrustedPeers(trusted)) = self.update_trusted_peers_receiver.recv() => {
                    self.peer_reputations.set_trusted(&trusted);
                    if self.apply_trusted_observers() {
                        self.update_topology();
                    }
                }
                // Process staggered connect attempts
                _ = pending_connects_interval.tick() => {
                    self.process_pending_connects();
                }
                // Every peer produce small amount of service messages so this shouldn't starve other tasks
                Some(service_message) = self.service_message_receiver.recv() => {
                    match service_message {
                        ServiceMessage::Terminated(terminated) => {
                            self.peer_terminated(terminated);
                        }
                        ServiceMessage::Connected(connected) => {
                            self.peer_connected(connected);
                        }
                        ServiceMessage::InboundPending(conn_id) => {
                            // Register a pending inbound connection (e.g., QUIC) so caps apply.
                            self.incoming_pending.insert(conn_id);
                        }
                        ServiceMessage::InboundAsk { conn_id, remote_addr, reply } => {
                            // Apply the same caps and per‑IP throttle as the TCP accept path.
                            let allow = if self.exceeds_caps() {
                                let evicted = self.evict_one_any();
                                if self.exceeds_caps() {
                                    TOTAL_CAP_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    iroha_logger::warn!(addr=%remote_addr, evicted, "Dropping QUIC connection due to total connections cap");
                                    false
                                } else { true }
                            } else if self.exceeds_incoming_cap() {
                                let evicted = self.evict_one_incoming();
                                if self.exceeds_incoming_cap() {
                                    INCOMING_CAP_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    iroha_logger::warn!(addr=%remote_addr, evicted, "Dropping QUIC connection due to max_incoming cap");
                                    false
                                } else { true }
                            } else if !self.allow_ip(remote_addr.ip()) {
                                ACCEPT_THROTTLED.fetch_add(1, Ordering::Relaxed);
                                iroha_logger::debug!(addr=%remote_addr, "Dropping QUIC connection due to per‑IP throttle");
                                false
                            } else {
                                true
                            };
                            if allow {
                                self.incoming_pending.insert(conn_id);
                            }
                            let _ = reply.send(allow);
                        }
                        ServiceMessage::InboundStream { conn_id, read, write } => {
                            // Spawn a peer task for the provided stream halves.
                            // Note: pending marker should have been inserted via InboundAsk/InboundPending.
                            let service_message_sender = self.service_message_sender.clone();
                            connected_from::<WireMessage<T>, K, E>(
                                self.public_address.clone(),
                                self.key_pair.clone(),
                                Connection::from_split(conn_id, read, write),
                                service_message_sender,
                                self.idle_timeout,
                                self.chain_id.clone(),
                                self.consensus_caps.clone(),
                                self.confidential_caps.clone(),
                                self.crypto_caps.clone(),
                                self.soranet_handshake.clone(),
                                self.local_scion_supported,
                                self.post_queue_cap,
                                self.relay_role,
                                self.trust_gossip,
                                self.max_frame_bytes,
                                self.quic_datagrams_enabled,
                                self.quic_datagram_max_payload_bytes,
                            );
                        }
                    }
                }
                // High-priority network messages (consensus/control)
                network_message = self.network_message_high_receiver.recv() => {
                    let Some(network_message) = network_message else {
                        iroha_logger::debug!("All handles to network actor are dropped. Shutting down...");
                        break;
                    };
                    let len = self.network_message_high_receiver.len();
                    update_network_queue_depth_high(len);
                    if len > 100 {
                        if let Some(supp) = self.sampler_high_queue_warn.should_log(tokio::time::Duration::from_secs(1)) {
                            iroha_logger::warn!(size=len, suppressed=supp, "High-priority messages are piling up in the queue");
                        }
                    }
                    match network_message {
                        NetworkMessage::Post(post) => self.post(post),
                        NetworkMessage::Broadcast(broadcast) => self.broadcast(broadcast),
                    }
                }
                // High-priority inbound peer messages (consensus/control)
                Some(peer_message) = self.peer_message_high_receiver.recv() => {
                    self.peer_message(peer_message).await;
                }
                // Low-priority network messages (gossip)
                network_message = self.network_message_low_receiver.recv() => {
                    let Some(network_message) = network_message else {
                        iroha_logger::debug!("All handles to network actor are dropped. Shutting down...");
                        break;
                    };
                    let len = self.network_message_low_receiver.len();
                    update_network_queue_depth_low(len);
                    if len > 100 {
                        if let Some(supp) = self.sampler_low_queue_warn.should_log(tokio::time::Duration::from_secs(1)) {
                            iroha_logger::warn!(size=len, suppressed=supp, "Low-priority messages are piling up in the queue");
                        }
                    }
                    match network_message {
                        NetworkMessage::Post(post) => self.post_low(post),
                        NetworkMessage::Broadcast(broadcast) => self.broadcast_low(broadcast),
                    }
                }
                // Accept incoming peer connections
                accept = self.listener.accept() => {
                    match accept {
                        Ok((stream, addr)) => {
                            iroha_logger::debug!(from_addr = %addr, "Accepted connection");
                            // Apply caps and per-IP throttle before spawning the peer task
                            let remote_ip = addr.ip();
                            if self.exceeds_caps() {
                                let evicted = self.evict_one_any();
                                if self.exceeds_caps() {
                                    TOTAL_CAP_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    iroha_logger::warn!(%addr, evicted, "Dropping incoming connection due to total connections cap");
                                    continue;
                                }
                            }
                            if self.exceeds_incoming_cap() {
                                let evicted = self.evict_one_incoming();
                                if self.exceeds_incoming_cap() {
                                    INCOMING_CAP_REJECTS.fetch_add(1, Ordering::Relaxed);
                                    iroha_logger::warn!(%addr, evicted, "Dropping incoming connection due to max_incoming cap");
                                    continue;
                                }
                            }
                            if !self.allow_ip(remote_ip) {
                                ACCEPT_THROTTLED.fetch_add(1, Ordering::Relaxed);
                                iroha_logger::debug!(%addr, "Dropping incoming connection due to per‑IP throttle");
                                continue;
                            }
                            let conn_id = self.get_conn_id();
                            self.incoming_pending.insert(conn_id);
                            // Handle creation of new peer with known connection id
                            self.accept_new_peer(stream, conn_id);
                        },
                        Err(error) => {
                            if let Some(supp) = self.sampler_accept_err.should_log(tokio::time::Duration::from_millis(500)) {
                                iroha_logger::warn!(%error, suppressed=supp, "Error accepting connection");
                            }
                        }
                    }
                }
                // Low-priority inbound peer messages (gossip/sync)
                Some(peer_message) = self.peer_message_low_receiver.recv() => {
                    self.peer_message(peer_message).await;
                }
                () = shutdown_signal.receive() => {
                    iroha_logger::debug!("Shutting down due to signal");
                    break
                }
                else => {
                    iroha_logger::debug!("All receivers are dropped, shutting down");
                    break
                },
            }
            tokio::task::yield_now().await;
        }
    }

    /// Disconnect and re-dial peers whose address is hostname-based to re-resolve DNS.
    fn refresh_hostnames(&mut self) {
        let ids_to_refresh: Vec<_> = self
            .peers
            .iter()
            .filter_map(|(peer_id, ref_peer)| match &ref_peer.p2p_addr {
                SocketAddr::Host(_) => Some(peer_id.clone()),
                _ => None,
            })
            .collect();
        for peer_id in ids_to_refresh {
            iroha_logger::debug!(%peer_id, "Refreshing DNS for hostname-based peer");
            self.dns_pending_refresh.insert(peer_id.clone());
            self.disconnect_peer(&peer_id);
        }
        DNS_REFRESHES.fetch_add(1, Ordering::Relaxed);
        // Connections will be re-established via the regular topology update path
        self.update_topology();
    }

    /// TTL-based selective refresh for hostname-based peers.
    fn refresh_hostnames_ttl(&mut self) {
        let Some(ttl) = self.dns_refresh_ttl else {
            return;
        };
        let now = tokio::time::Instant::now();
        let mut refreshed = Vec::new();
        for (peer_id, ref_peer) in &self.peers {
            if !matches!(ref_peer.p2p_addr, SocketAddr::Host(_)) {
                continue;
            }
            let last = self
                .dns_last_refresh
                .get(peer_id)
                .copied()
                .unwrap_or(now - ttl - Duration::from_secs(1));
            if now.duration_since(last) >= ttl {
                refreshed.push(peer_id.clone());
            }
        }
        for peer_id in &refreshed {
            iroha_logger::debug!(%peer_id, "TTL refresh of hostname-based peer");
            self.dns_last_refresh.insert(peer_id.clone(), now);
            self.disconnect_peer(peer_id);
        }
        if !refreshed.is_empty() {
            DNS_TTL_REFRESHES.fetch_add(1, Ordering::Relaxed);
        }
        self.update_topology();
    }

    fn accept_new_peer(&mut self, stream: TcpStream, conn_id: ConnectionId) {
        // Apply configured TCP socket options (best-effort)
        crate::transport::apply_tcp_socket_options(&stream, self.tcp_nodelay, self.tcp_keepalive);
        let service_message_sender = self.service_message_sender.clone();
        connected_from::<WireMessage<T>, K, E>(
            self.public_address.clone(),
            self.key_pair.clone(),
            Connection::new(conn_id, stream),
            service_message_sender,
            self.idle_timeout,
            self.chain_id.clone(),
            self.consensus_caps.clone(),
            self.confidential_caps.clone(),
            self.crypto_caps.clone(),
            self.soranet_handshake.clone(),
            self.local_scion_supported,
            self.post_queue_cap,
            self.relay_role,
            self.trust_gossip,
            self.max_frame_bytes,
            self.quic_datagrams_enabled,
            self.quic_datagram_max_payload_bytes,
        );
    }

    fn set_current_topology(&mut self, UpdateTopology(topology): UpdateTopology) {
        iroha_logger::debug!(?topology, "Network receive new topology");
        let mut topology: HashSet<_> = topology
            .into_iter()
            .filter(|peer_id| peer_id.public_key() != self.key_pair.public_key())
            .filter(|peer_id| {
                let pk = peer_id.public_key();
                if self.deny_keys.contains(pk) {
                    return false;
                }
                if self.allowlist_only && !self.allow_keys.contains(pk) {
                    return false;
                }
                true
            })
            .collect();
        match self.relay_mode {
            iroha_config::parameters::actual::RelayMode::Spoke => {
                // In spoke mode, we only dial the hub and rely on forwarding.
                let hub = self.ensure_hub_peer();
                topology.clear();
                if let Some(hub) = hub {
                    topology.insert(hub);
                } else {
                    iroha_logger::warn!(
                        relay_hub_addresses=?self.relay_hub_addresses,
                        "relay_mode=spoke requires at least one reachable hub peer id; topology is empty"
                    );
                }
            }
            iroha_config::parameters::actual::RelayMode::Assist => {
                // In assist mode, keep the provided topology but ensure we also connect to a hub
                // for relay fallback when a target is not directly connected.
                if let Some(hub) = self.ensure_hub_peer() {
                    topology.insert(hub);
                } else {
                    iroha_logger::warn!(
                        relay_hub_addresses=?self.relay_hub_addresses,
                        "relay_mode=assist requires at least one reachable hub peer id; retaining provided topology"
                    );
                }
            }
            iroha_config::parameters::actual::RelayMode::Disabled
            | iroha_config::parameters::actual::RelayMode::Hub => {}
        }
        self.current_topology = topology;
        self.apply_trusted_observers();
        self.peer_capabilities
            .retain(|peer_id, _| self.current_topology.contains(peer_id));
        self.update_topology()
    }

    fn allow_trusted_observers(&self) -> bool {
        self.is_permissioned_consensus()
            && !matches!(
                self.relay_mode,
                iroha_config::parameters::actual::RelayMode::Spoke
            )
    }

    fn apply_trusted_observers(&mut self) -> bool {
        if !self.allow_trusted_observers() {
            return false;
        }
        let mut updated = self.current_topology.clone();
        updated.retain(|peer_id| {
            let pk = peer_id.public_key();
            !self.deny_keys.contains(pk) && (!self.allowlist_only || self.allow_keys.contains(pk))
        });
        for peer_id in self.peer_reputations.trusted_peers() {
            if peer_id.public_key() == self.key_pair.public_key() {
                continue;
            }
            let pk = peer_id.public_key();
            if self.deny_keys.contains(pk) {
                continue;
            }
            if self.allowlist_only && !self.allow_keys.contains(pk) {
                continue;
            }
            updated.insert(peer_id);
        }
        if updated != self.current_topology {
            self.current_topology = updated;
            return true;
        }
        false
    }

    fn set_current_peers_addresses(&mut self, UpdatePeers(peers): UpdatePeers) {
        debug!(
            total = peers.len(),
            local_known = self.peers.len(),
            relay_mode = ?self.relay_mode,
            relay_hub_addresses = ?self.relay_hub_addresses,
            "Network receive new peers addresses",
        );
        let preserved_hub = if matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) {
            self.relay_hub_peer.as_ref().and_then(|hub| {
                self.address_book
                    .get(hub)
                    .cloned()
                    .map(|addr| (hub.clone(), addr))
            })
        } else {
            None
        };
        self.address_book.clear();
        for (pid, addr) in &peers {
            self.address_book.insert(pid.clone(), addr.clone());
        }
        self.current_peers_addresses = peers;
        self.peer_capabilities
            .retain(|peer_id, _| self.current_topology.contains(peer_id));
        if let Some((hub_id, hub_addr)) = preserved_hub {
            self.address_book
                .entry(hub_id.clone())
                .or_insert_with(|| hub_addr.clone());
            if !self
                .current_peers_addresses
                .iter()
                .any(|(id, _)| id == &hub_id)
            {
                self.current_peers_addresses.push((hub_id, hub_addr));
            }
        }
        // Recompute the set of peers allowed to relay frames (origin mismatch).
        self.relay_trusted_peers.clear();
        if !self.relay_hub_addresses.is_empty() {
            for (pid, addr) in &self.current_peers_addresses {
                if self.configured_hub_matches(addr) {
                    self.relay_trusted_peers.insert(pid.clone());
                }
            }
        }
        // Apply address updates immediately to reduce startup latency and
        // speed up recovery after gossip/DNS refresh. This keeps connection
        // attempts responsive instead of waiting for the periodic tick.
        self.update_topology();
    }

    fn set_peer_capabilities(
        &mut self,
        message::UpdatePeerCapabilities(capabilities): message::UpdatePeerCapabilities,
    ) {
        for (peer_id, caps) in capabilities {
            if peer_id.public_key() == self.key_pair.public_key() {
                continue;
            }
            if !self.current_topology.contains(&peer_id) {
                continue;
            }
            self.peer_capabilities.insert(peer_id, caps);
        }
    }

    fn update_topology(&mut self) {
        let now = tokio::time::Instant::now();
        // Keep hub selection fresh so spoke/assist modes can fail over between multiple hubs.
        self.refresh_relay_hub_peer(now);
        match self.relay_mode {
            iroha_config::parameters::actual::RelayMode::Spoke => {
                // Spokes should only keep a hub connection.
                let hub = self.relay_hub_peer.clone();
                self.current_topology.clear();
                if let Some(hub) = hub {
                    self.current_topology.insert(hub);
                }
            }
            iroha_config::parameters::actual::RelayMode::Assist => {
                // Ensure only one configured hub is kept in topology to avoid duplicate relay paths.
                if !self.relay_trusted_peers.is_empty() {
                    if let Some(selected) = self.relay_hub_peer.as_ref() {
                        self.current_topology
                            .retain(|id| !self.relay_trusted_peers.contains(id) || id == selected);
                    } else {
                        self.current_topology
                            .retain(|id| !self.relay_trusted_peers.contains(id));
                    }
                }
                if let Some(hub) = self.relay_hub_peer.clone() {
                    self.current_topology.insert(hub);
                }
            }
            iroha_config::parameters::actual::RelayMode::Disabled
            | iroha_config::parameters::actual::RelayMode::Hub => {}
        }
        // Group candidate addresses by peer id for staggered parallel attempts
        let mut by_peer: HashMap<PeerId, Vec<SocketAddr>> = HashMap::new();
        for (id, address) in &self.current_peers_addresses {
            if !self.current_topology.contains(id) {
                continue;
            }
            // Skip already connected or already connecting for the same address
            if self.peers.contains_key(id)
                || self
                    .connecting_peers
                    .values()
                    .any(|peer| (peer.id(), peer.address()) == (id, address))
            {
                continue;
            }
            if !self.ready_to_retry_addr(id, address, now) {
                continue;
            }
            by_peer.entry(id.clone()).or_default().push(address.clone());
        }

        // Order addresses by preference and schedule staggered attempts
        for (peer_id, mut addrs) in by_peer {
            addrs.sort_by_key(|a| self.addr_preference(a));
            for (i, addr) in addrs.into_iter().enumerate() {
                if self.is_scheduled(&peer_id, &addr) {
                    continue;
                }
                // Add small jitter to spread attempts in very large clusters
                let base = self
                    .happy_eyeballs_stagger
                    .saturating_mul(u32::try_from(i).unwrap_or(u32::MAX));
                let jitter_cap_ms =
                    u64::try_from(self.happy_eyeballs_stagger.as_millis() / 2).unwrap_or(u64::MAX);
                let jitter_ms = if jitter_cap_ms > 0 {
                    rand::rng().random_range(0..=jitter_cap_ms)
                } else {
                    0
                };
                let when = now + base + Duration::from_millis(jitter_ms);
                let when = apply_connect_startup_delay(when, self.connect_startup_delay_until);
                self.pending_connects
                    .push((when, Peer::new(addr, peer_id.clone())));
            }
        }

        let restrict_topology = self.is_permissioned_consensus()
            || matches!(
                self.relay_mode,
                iroha_config::parameters::actual::RelayMode::Spoke
            );
        let to_disconnect = if restrict_topology {
            self.peers
                .keys()
                // Peer is connected but shouldn't
                .filter(|&peer_id| !self.current_topology.contains(peer_id))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        // actual dialing is handled in process_pending_connects()

        for public_key in to_disconnect {
            self.disconnect_peer(&public_key)
        }
    }

    fn ensure_hub_peer(&mut self) -> Option<PeerId> {
        self.refresh_relay_hub_peer(tokio::time::Instant::now());
        self.relay_hub_peer.clone()
    }

    fn configured_hub_matches(&self, addr: &SocketAddr) -> bool {
        self.relay_hub_addresses.iter().any(|hub_addr| {
            addr == hub_addr
                || (addr.port() == hub_addr.port() && addr.host_str() == hub_addr.host_str())
        })
    }

    fn refresh_relay_hub_peer(&mut self, now: tokio::time::Instant) {
        if !matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) {
            self.relay_hub_peer = None;
            return;
        }
        if self.relay_hub_addresses.is_empty() {
            self.relay_hub_peer = None;
            return;
        }

        // If the selected hub is already connected or in-flight, keep it.
        if let Some(selected) = self.relay_hub_peer.as_ref() {
            if self.peers.contains_key(selected)
                || self.connecting_peers.values().any(|p| p.id() == selected)
            {
                return;
            }
        }

        let hub_addr_matches = |hub_addr: &SocketAddr, addr: &SocketAddr| {
            addr == hub_addr
                || (addr.port() == hub_addr.port() && addr.host_str() == hub_addr.host_str())
        };

        // Pick the first candidate hub that is connected, connecting, or ready to retry.
        for hub_addr in &self.relay_hub_addresses {
            // Resolve the configured hub address into a peer id via the current peer mapping.
            let Some((pid, addr)) = self
                .current_peers_addresses
                .iter()
                .find(|(_, addr)| hub_addr_matches(hub_addr, addr))
            else {
                continue;
            };

            // If we have a resolved allowlist of trusted relays, enforce it.
            if !self.relay_trusted_peers.is_empty() && !self.relay_trusted_peers.contains(pid) {
                continue;
            }

            if self.peers.contains_key(pid)
                || self
                    .connecting_peers
                    .values()
                    .any(|p| (p.id(), p.address()) == (pid, addr))
                || self.ready_to_retry_addr(pid, addr, now)
            {
                self.relay_hub_peer = Some(pid.clone());
                return;
            }
        }

        self.relay_hub_peer = None;
    }

    fn is_configured_hub_peer(&self, peer: &Peer, relay_role: RelayRole) -> bool {
        if !matches!(relay_role, RelayRole::Hub) {
            return false;
        }
        // Prefer peer-id based matching when we can resolve configured hubs into peer ids.
        if !self.relay_trusted_peers.is_empty() {
            return self.relay_trusted_peers.contains(peer.id());
        }
        self.configured_hub_matches(peer.address())
    }

    fn hub_handle(&mut self) -> Option<(&PeerId, &RefPeer<WireMessage<T>>)> {
        self.refresh_relay_hub_peer(tokio::time::Instant::now());
        let hub = self.relay_hub_peer.as_ref()?;
        self.peers.get_key_value(hub)
    }

    fn relay_route_for_unconnected_post_target(&mut self, target: &PeerId) -> Option<PeerId> {
        if !matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) {
            return None;
        }
        if self.peers.contains_key(target) {
            return None;
        }
        let hub_id = self.ensure_hub_peer()?;
        if &hub_id == target {
            return None;
        }
        Some(hub_id)
    }

    fn resolve_origin_peer(&self, origin: &PeerId, via: &Peer) -> Peer {
        self.address_book.get(origin).map_or_else(
            || Peer::new(via.address().clone(), origin.clone()),
            |addr| Peer::new(addr.clone(), origin.clone()),
        )
    }

    fn record_trust_gossip_skip(peer_id: &PeerId, direction: TrustDirection, reason: &'static str) {
        inc_trust_gossip_skipped(direction.as_label(), reason);
        iroha_logger::debug!(
            peer=%peer_id,
            ?direction,
            reason,
            "trust gossip skipped: capability off"
        );
    }

    fn current_generation_token(&self, peer_id: &PeerId) -> Option<ConnectionId> {
        self.peers
            .get(peer_id)
            .map(|peer| peer.conn_id)
            .or_else(|| self.peer_session_generation.get(peer_id).copied())
    }

    fn trigger_reconnect_for_peer(&mut self, peer_id: &PeerId) -> bool {
        if !self.current_topology.contains(peer_id) {
            return false;
        }
        if self.peers.contains_key(peer_id)
            || self
                .connecting_peers
                .values()
                .any(|peer| peer.id() == peer_id)
        {
            return false;
        }
        let now = tokio::time::Instant::now();
        let Some(addr) = self
            .current_peers_addresses
            .iter()
            .find_map(|(id, addr)| (id == peer_id).then_some(addr.clone()))
        else {
            return false;
        };
        if self.is_scheduled(peer_id, &addr) {
            return false;
        }
        let peer = Peer::new(addr.clone(), peer_id.clone());
        if self.ready_to_retry_addr(peer_id, &addr, now) {
            self.connect_peer(&peer);
        } else {
            let when = self
                .retry_backoff
                .get(peer_id)
                .and_then(|inner| inner.get(&addr.to_string()).map(|(when, _)| *when))
                .unwrap_or_else(|| now + Duration::from_millis(50));
            let when = apply_connect_startup_delay(when, self.connect_startup_delay_until);
            self.pending_connects.push((when, peer));
        }
        SESSION_RECONNECT_TOTAL.fetch_add(1, Ordering::Relaxed);
        true
    }

    fn defer_frame(
        &mut self,
        peer_id: &PeerId,
        frame: RelayMessage<T>,
        topic: message::Topic,
        generation: Option<ConnectionId>,
        trigger_reconnect: bool,
        reason: &'static str,
    ) -> bool {
        let now = tokio::time::Instant::now();
        let (expired, overflow) =
            self.deferred_send_queue
                .enqueue(peer_id.clone(), frame, topic, generation, now);
        DEFERRED_SEND_ENQUEUED.fetch_add(1, Ordering::Relaxed);
        let dropped = expired.saturating_add(overflow);
        if dropped > 0 {
            DEFERRED_SEND_DROPPED.fetch_add(dropped as u64, Ordering::Relaxed);
        }
        if trigger_reconnect {
            let _ = self.trigger_reconnect_for_peer(peer_id);
        }
        debug!(
            peer = %peer_id,
            ?generation,
            trigger_reconnect,
            expired_dropped = expired,
            overflow_dropped = overflow,
            reason,
            "deferred outbound frame while peer session unavailable"
        );
        true
    }

    fn flush_deferred_frames_for_peer(&mut self, peer_id: &PeerId) -> DeferredFlushOutcome {
        let now = tokio::time::Instant::now();
        let (mut queued, expired) = self.deferred_send_queue.take_peer(peer_id, now);
        if expired > 0 {
            DEFERRED_SEND_DROPPED.fetch_add(expired as u64, Ordering::Relaxed);
        }
        if queued.is_empty() {
            return DeferredFlushOutcome::Flushed;
        }
        while let Some(entry) = queued.pop_front() {
            let Some((conn_id, peer_addr, post_result)) = ({
                self.peers.get(peer_id).map(|ref_peer| {
                    if entry
                        .generation
                        .is_some_and(|generation| generation != ref_peer.conn_id)
                    {
                        return (
                            ref_peer.conn_id,
                            ref_peer.p2p_addr.clone(),
                            DeferredPostResult::StaleGeneration,
                        );
                    }
                    if !trust_gossip_allowed(
                        entry.topic,
                        ref_peer.trust_gossip && self.trust_gossip,
                    ) {
                        let reason = if self.trust_gossip {
                            "peer_capability_off"
                        } else {
                            "local_capability_off"
                        };
                        Self::record_trust_gossip_skip(peer_id, TrustDirection::Outbound, reason);
                        return (
                            ref_peer.conn_id,
                            ref_peer.p2p_addr.clone(),
                            DeferredPostResult::CapabilitySkipped,
                        );
                    }
                    (
                        ref_peer.conn_id,
                        ref_peer.p2p_addr.clone(),
                        match ref_peer.handle.post(entry.frame) {
                            Ok(()) => DeferredPostResult::Sent,
                            Err(PostError::Closed) => DeferredPostResult::Closed,
                            Err(PostError::Full) => DeferredPostResult::Full,
                        },
                    )
                })
            }) else {
                self.deferred_send_queue
                    .restore_peer(peer_id.clone(), queued);
                return DeferredFlushOutcome::PeerMissing;
            };

            match post_result {
                DeferredPostResult::Sent | DeferredPostResult::CapabilitySkipped => {}
                DeferredPostResult::StaleGeneration => {
                    DEFERRED_SEND_DROPPED.fetch_add(1, Ordering::Relaxed);
                }
                DeferredPostResult::Full => {
                    DEFERRED_SEND_DROPPED.fetch_add(1, Ordering::Relaxed);
                    self.deferred_send_queue
                        .restore_peer(peer_id.clone(), queued);
                    return DeferredFlushOutcome::Backpressured(conn_id);
                }
                DeferredPostResult::Closed => {
                    DEFERRED_SEND_DROPPED.fetch_add(1, Ordering::Relaxed);
                    let peer = Peer::new(peer_addr, peer_id.clone());
                    iroha_logger::warn!(
                        peer=%peer,
                        "Peer channel closed while flushing deferred frames; dropping peer"
                    );
                    self.peers.remove(peer_id);
                    Self::remove_online_peer(
                        &self.online_peers_sender,
                        &self.online_peer_capabilities_sender,
                        peer_id,
                    );
                    self.incoming_active.remove(&conn_id);
                    self.last_active.remove(peer_id);
                    self.clear_low_buckets(peer_id);
                    for deferred in &mut queued {
                        deferred.generation = None;
                    }
                    self.deferred_send_queue
                        .restore_peer(peer_id.clone(), queued);
                    return DeferredFlushOutcome::PeerMissing;
                }
            }
        }
        DeferredFlushOutcome::Flushed
    }

    fn send_frame_to_peer(
        &mut self,
        peer_id: &PeerId,
        frame: RelayMessage<T>,
        topic: message::Topic,
    ) -> bool {
        let is_high = matches!(frame.priority, Priority::High);
        let is_consensus = matches!(
            topic,
            message::Topic::Consensus
                | message::Topic::ConsensusPayload
                | message::Topic::ConsensusChunk
        );
        if matches!(topic, message::Topic::BlockSync) {
            iroha_logger::debug!(
                peer=%peer_id,
                high=is_high,
                "enqueueing block sync frame to peer"
            );
        }
        let Some(_) = self.peers.get(peer_id) else {
            if peer_id.public_key() == self.key_pair.public_key() {
                #[cfg(debug_assertions)]
                iroha_logger::trace!("Not sending message to myself");
                return false;
            }
            iroha_logger::warn!(
                peer=%peer_id,
                consensus=is_consensus,
                "Peer not found; deferring outbound frame"
            );
            return self.defer_frame(
                peer_id,
                frame,
                topic,
                self.current_generation_token(peer_id),
                true,
                "peer session missing",
            );
        };
        match self.flush_deferred_frames_for_peer(peer_id) {
            DeferredFlushOutcome::Flushed => {}
            DeferredFlushOutcome::PeerMissing => {
                return self.defer_frame(
                    peer_id,
                    frame,
                    topic,
                    self.current_generation_token(peer_id),
                    true,
                    "peer session missing after deferred flush",
                );
            }
            DeferredFlushOutcome::Backpressured(conn_id) => {
                return self.defer_frame(
                    peer_id,
                    frame,
                    topic,
                    Some(conn_id),
                    false,
                    "peer backpressured while flushing deferred frames",
                );
            }
        }

        let Some(ref_peer) = self.peers.get(peer_id) else {
            return self.defer_frame(
                peer_id,
                frame,
                topic,
                self.current_generation_token(peer_id),
                true,
                "peer session disappeared before post",
            );
        };
        if !trust_gossip_allowed(topic, ref_peer.trust_gossip && self.trust_gossip) {
            let reason = if self.trust_gossip {
                "peer_capability_off"
            } else {
                "local_capability_off"
            };
            Self::record_trust_gossip_skip(peer_id, TrustDirection::Outbound, reason);
            return false;
        }
        let (conn_id, p2p_addr) = (ref_peer.conn_id, ref_peer.p2p_addr.clone());
        let retry_frame = frame.clone();
        let outcome = match ref_peer.handle.post(frame) {
            Ok(()) => {
                if is_consensus {
                    iroha_logger::debug!(
                        peer=%peer_id,
                        high=is_high,
                        "consensus frame enqueued to peer"
                    );
                }
                return true;
            }
            Err(PostError::Closed) => {
                let peer = Peer::new(p2p_addr.clone(), peer_id.clone());
                iroha_logger::error!(peer=%peer, "Peer channel closed; dropping peer");
                Some(peer)
            }
            Err(PostError::Full) => {
                POST_OVERFLOWS.fetch_add(1, Ordering::Relaxed);
                inc_post_overflow_for(topic);
                inc_post_overflow_for_prio(topic, is_high);
                if self.disconnect_on_post_overflow {
                    let peer = Peer::new(p2p_addr.clone(), peer_id.clone());
                    iroha_logger::warn!(
                        peer=%peer,
                        consensus=is_consensus,
                        high=is_high,
                        "Per-peer post channel overflow; disconnecting per policy"
                    );
                    Some(peer)
                } else {
                    iroha_logger::warn!(
                        peer=%peer_id,
                        consensus=is_consensus,
                        high=is_high,
                        "Per-peer post channel overflow; dropping message per policy"
                    );
                    return self.defer_frame(
                        peer_id,
                        retry_frame,
                        topic,
                        Some(conn_id),
                        false,
                        "peer post queue full",
                    );
                }
            }
        };
        if outcome.is_some() {
            self.peers.remove(peer_id);
            Self::remove_online_peer(
                &self.online_peers_sender,
                &self.online_peer_capabilities_sender,
                peer_id,
            );
            self.incoming_active.remove(&conn_id);
            self.last_active.remove(peer_id);
            self.clear_low_buckets(peer_id);
            return self.defer_frame(
                peer_id,
                retry_frame,
                topic,
                None,
                true,
                "peer disconnected while posting frame",
            );
        }
        false
    }

    fn ready_to_retry_addr(
        &self,
        id: &PeerId,
        addr: &SocketAddr,
        now: tokio::time::Instant,
    ) -> bool {
        let key = addr.to_string();
        self.retry_backoff
            .get(id)
            .and_then(|m| m.get(&key))
            .is_none_or(|(when, _)| now >= *when)
    }

    fn schedule_backoff_addr(&mut self, id: &PeerId, addr: &SocketAddr) {
        let now = tokio::time::Instant::now();
        let key = addr.to_string();
        let base = self
            .retry_backoff
            .get(id)
            .and_then(|m| m.get(&key).map(|(_, b)| *b))
            .unwrap_or(BACKOFF_INITIAL);
        let next_base = core::cmp::min(BACKOFF_MAX, base.saturating_mul(2));
        let mut rng = rand::rng();
        let upper_ms = u64::try_from(next_base.as_millis()).unwrap_or(u64::MAX);
        let jitter_ms = if upper_ms == 0 {
            0
        } else {
            rng.random_range(0..=upper_ms)
        };
        CONNECT_RETRY_MILLIS_TOTAL.fetch_add(jitter_ms, Ordering::Relaxed);
        let when = now + Duration::from_millis(jitter_ms);
        self.retry_backoff
            .entry(id.clone())
            .or_default()
            .insert(key, (when, next_base));
        BACKOFF_SCHEDULED.fetch_add(1, Ordering::Relaxed);
        iroha_logger::debug!(peer=%id, addr=%addr, delay=?next_base, until=?when, "Scheduled reconnect backoff");
    }

    fn reset_backoff_addr(&mut self, id: &PeerId, addr: &SocketAddr) {
        let key = addr.to_string();
        if let Some(inner) = self.retry_backoff.get_mut(id) {
            inner.remove(&key);
            if inner.is_empty() {
                self.retry_backoff.remove(id);
            }
        }
    }

    fn is_permissioned_consensus(&self) -> bool {
        self.consensus_caps
            .as_ref()
            .map_or(true, |caps| caps.mode_tag.contains("permissioned"))
    }

    fn process_pending_connects(&mut self) {
        let now = tokio::time::Instant::now();
        let delay_until = self.connect_startup_delay_until;
        let mut rest = Vec::with_capacity(self.pending_connects.len());
        let mut due_peers = Vec::new();
        for (when, peer) in self.pending_connects.drain(..) {
            let when = apply_connect_startup_delay(when, delay_until);
            if when > now {
                rest.push((when, peer));
            } else {
                due_peers.push(peer);
            }
        }
        due_peers.sort_by(|a, b| {
            let ta = self.peer_reputations.is_trusted(a.id());
            let tb = self.peer_reputations.is_trusted(b.id());
            match tb.cmp(&ta) {
                std::cmp::Ordering::Equal => self
                    .peer_reputations
                    .score(b.id())
                    .cmp(&self.peer_reputations.score(a.id())),
                ord => ord,
            }
        });
        self.pending_connects = rest;

        for peer in due_peers {
            let id = peer.id().clone();
            let addr = peer.address().clone();
            if !self.current_topology.contains(&id) {
                continue;
            }
            if !self.peers.contains_key(&id)
                && !self
                    .connecting_peers
                    .values()
                    .any(|p| (p.id(), p.address()) == (&id, &addr))
                && self.ready_to_retry_addr(&id, &addr, now)
            {
                self.connect_peer(&peer);
            } else {
                // Not ready; reschedule shortly to avoid starvation
                let when = apply_connect_startup_delay(
                    now + Duration::from_millis(50),
                    self.connect_startup_delay_until,
                );
                self.pending_connects.push((when, peer));
            }
        }
    }

    fn is_scheduled(&self, id: &PeerId, addr: &SocketAddr) -> bool {
        self.pending_connects
            .iter()
            .any(|(_, p)| p.id() == id && p.address() == addr)
    }

    fn addr_preference(&self, addr: &SocketAddr) -> u8 {
        if self.addr_ipv6_first {
            return match addr {
                SocketAddr::Ipv6(_) => 0,
                SocketAddr::Host(_) => 1,
                SocketAddr::Ipv4(_) => 2,
            };
        }
        match addr {
            SocketAddr::Host(_) => 0,
            SocketAddr::Ipv6(_) => 1,
            SocketAddr::Ipv4(_) => 2,
        }
    }

    fn connect_peer(&mut self, peer: &Peer) {
        iroha_logger::trace!(
            listen_addr = %self.listen_addr, peer.id.address = %peer.address(),
            "Creating new peer actor",
        );

        let conn_id = self.get_conn_id();
        let prefer_scion = self.local_scion_supported
            && self
                .peer_capabilities
                .get(peer.id())
                .is_some_and(|caps| caps.scion_supported);
        self.connecting_peers.insert(conn_id, peer.clone());
        let service_message_sender = self.service_message_sender.clone();
        connecting::<WireMessage<T>, K, E>(
            // NOTE: we intentionally use peer's address and our public key, it's used during handshake
            peer.address().clone(),
            peer.id().clone(),
            self.public_address.clone(),
            self.key_pair.clone(),
            conn_id,
            service_message_sender,
            self.idle_timeout,
            self.dial_timeout,
            self.chain_id.clone(),
            self.consensus_caps.clone(),
            self.confidential_caps.clone(),
            self.crypto_caps.clone(),
            self.soranet_handshake.clone(),
            self.post_queue_cap,
            self.quic_enabled,
            self.tls_enabled,
            self.tls_fallback_to_plain,
            prefer_scion,
            self.local_scion_supported,
            self.prefer_ws_fallback,
            self.trust_gossip,
            self.max_frame_bytes,
            self.relay_role,
            self.happy_eyeballs_stagger,
            self.tcp_nodelay,
            self.tcp_keepalive,
            self.proxy_tls_verify,
            self.proxy_tls_pinned_cert_der.clone(),
            self.proxy_policy.clone(),
            self.quic_dialer.clone(),
            self.quic_datagrams_enabled,
            self.quic_datagram_max_payload_bytes,
        );
    }

    fn disconnect_peer(&mut self, peer_id: &PeerId) {
        let peer = match self.peers.remove(peer_id) {
            Some(peer) => peer,
            _ => return iroha_logger::warn!(?peer_id, "Not found peer to disconnect"),
        };
        iroha_logger::debug!(listen_addr = %self.listen_addr, %peer.conn_id, "Disconnecting peer");

        self.incoming_active.remove(&peer.conn_id);
        self.last_active.remove(peer_id);
        Self::remove_online_peer(
            &self.online_peers_sender,
            &self.online_peer_capabilities_sender,
            peer_id,
        );
        self.clear_low_buckets(peer_id);
    }

    #[log(skip_all, fields(peer=%peer, conn_id=connection_id, disambiguator=disambiguator))]
    fn peer_connected(
        &mut self,
        Connected {
            peer,
            connection_id,
            ready_peer_handle,
            peer_message_sender,
            disambiguator,
            relay_role,
            scion_supported,
            trust_gossip,
        }: Connected<WireMessage<T>>,
    ) {
        self.connecting_peers.remove(&connection_id);

        if !self.current_topology.contains(peer.id()) {
            if matches!(
                self.relay_mode,
                iroha_config::parameters::actual::RelayMode::Spoke
            ) {
                // Spokes only accept the configured hub(s). Do not expand topology from
                // inbound connections in this mode.
                if self.is_configured_hub_peer(&peer, relay_role) {
                    iroha_logger::debug!(
                        peer=%peer.id(),
                        "Accepting configured relay hub outside advertised topology due to spoke mode"
                    );
                    self.current_topology.insert(peer.id().clone());
                } else {
                    iroha_logger::warn!(
                        peer=%peer.id(),
                        role=?relay_role,
                        "Spoke mode only accepts configured hub peers; dropping peer"
                    );
                    return;
                }
            } else {
                let permissioned = self.is_permissioned_consensus();
                let trusted = self.peer_reputations.is_trusted(peer.id());
                if permissioned && !trusted {
                    iroha_logger::warn!(peer=%peer.id(), "Dropping untrusted observer in permissioned network");
                    return;
                }
                if !permissioned {
                    iroha_logger::debug!(peer=%peer.id(), "Accepting observer outside topology for public network");
                    self.current_topology.insert(peer.id().clone());
                }
                if matches!(
                    self.relay_mode,
                    iroha_config::parameters::actual::RelayMode::Assist
                ) && self.is_configured_hub_peer(&peer, relay_role)
                {
                    iroha_logger::debug!(
                        peer=%peer.id(),
                        "Accepting configured relay hub outside advertised topology due to relay mode"
                    );
                    self.current_topology.insert(peer.id().clone());
                } else if self.current_topology.is_empty() && !permissioned {
                    iroha_logger::debug!(peer=%peer.id(), "Bootstrapping topology from first inbound peer");
                    self.current_topology.insert(peer.id().clone());
                } else if permissioned && !trusted {
                    iroha_logger::warn!(peer=%peer.id(), topology=?self.current_topology, "Peer not present in topology is trying to connect");
                    return;
                }
            }
        }

        // Enforce key-based ACLs for inbound/outbound after handshake
        let pk = peer.id().public_key();
        if self.deny_keys.contains(pk) {
            iroha_logger::warn!(peer=%peer.id(), "Peer denied by key denylist; dropping connection");
            return;
        }
        if self.allowlist_only && !self.allow_keys.contains(pk) {
            iroha_logger::warn!(peer=%peer.id(), "Peer not in key allowlist; dropping connection");
            return;
        }

        if matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
        ) && !self.is_configured_hub_peer(&peer, relay_role)
        {
            iroha_logger::warn!(
                peer=%peer.id(),
                role=?relay_role,
                "Spoke mode only accepts configured hub connections; dropping peer"
            );
            return;
        }

        self.peer_reputations.record_connected(peer.id());

        if matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) && self.is_configured_hub_peer(&peer, relay_role)
        {
            let promote = self
                .relay_hub_peer
                .as_ref()
                .is_none_or(|selected| selected == peer.id() || !self.peers.contains_key(selected));
            if promote {
                self.relay_hub_peer = Some(peer.id().clone());
            }
            self.address_book
                .entry(peer.id().clone())
                .or_insert_with(|| peer.address().clone());
        }

        //  Insert peer if peer not in peers yet or replace peer if it's disambiguator value is smaller than new one (simultaneous connections resolution rule)
        match self.peers.get(peer.id()) {
            Some(peer) if peer.disambiguator > disambiguator => {
                iroha_logger::debug!(
                    "Peer is disconnected due to simultaneous connection resolution policy"
                );
                return;
            }
            Some(_) => {
                iroha_logger::debug!(
                    "New peer will replace previous one due to simultaneous connection resolution policy"
                );
            }
            None => {
                iroha_logger::debug!("Peer isn't in the peer set, inserting");
            }
        }

        // Successful connection: reset any existing backoff state for this peer.
        self.reset_backoff_addr(peer.id(), peer.address());

        // If this connection followed a DNS refresh request for this peer, treat it as a
        // reconnect success for telemetry and clear the pending marker.
        if self.dns_pending_refresh.remove(peer.id()) {
            DNS_RECONNECT_SUCCESSES.fetch_add(1, Ordering::Relaxed);
        }

        // Track initial last-active as now.
        self.last_active
            .insert(peer.id().clone(), tokio::time::Instant::now());

        // If this connection originated from an incoming accept, register as active incoming.
        if self.incoming_pending.remove(&connection_id) {
            self.incoming_active.insert(connection_id);
        }

        self.address_book
            .insert(peer.id().clone(), peer.address().clone());

        let transport_capabilities = message::PeerTransportCapabilities { scion_supported };
        self.peer_capabilities
            .insert(peer.id().clone(), transport_capabilities);

        let ref_peer = RefPeer {
            handle: ready_peer_handle,
            conn_id: connection_id,
            p2p_addr: peer.address().clone(),
            disambiguator,
            relay_role,
            trust_gossip,
        };
        let _ = peer_message_sender.send(crate::peer::message::PeerMessageSenders {
            high: self.peer_message_high_sender.clone(),
            low: self.peer_message_low_sender.clone(),
        });
        self.peers.insert(peer.id().clone(), ref_peer);
        self.peer_session_generation
            .insert(peer.id().clone(), connection_id);
        match self.flush_deferred_frames_for_peer(peer.id()) {
            DeferredFlushOutcome::Flushed | DeferredFlushOutcome::Backpressured(_) => {}
            DeferredFlushOutcome::PeerMissing => {
                let _ = self.trigger_reconnect_for_peer(peer.id());
            }
        }
        if self.dns_refresh_interval.is_some() || self.dns_refresh_ttl.is_some() {
            if self.dns_pending_refresh.remove(peer.id()) {
                DNS_RECONNECT_SUCCESSES.fetch_add(1, Ordering::Relaxed);
            }
            self.dns_last_refresh
                .insert(peer.id().clone(), tokio::time::Instant::now());
        }
        Self::add_online_peer(
            &self.online_peers_sender,
            &self.online_peer_capabilities_sender,
            peer,
            transport_capabilities,
        );
    }

    fn peer_terminated(&mut self, Terminated { peer, conn_id }: Terminated) {
        // If termination happened before handshake, the `peer` is None.
        // In that case use the pending `connecting_peers` map to find which peer failed.
        if let Some(peer) = peer {
            if let Some(ref_peer) = self.peers.get(peer.id()) {
                if ref_peer.conn_id == conn_id {
                    iroha_logger::debug!(conn_id, peer=%peer, "Peer terminated");
                    self.peer_reputations.record_disconnected(peer.id());
                    self.peers.remove(peer.id());
                    self.last_active.remove(peer.id());
                    Self::remove_online_peer(
                        &self.online_peers_sender,
                        &self.online_peer_capabilities_sender,
                        peer.id(),
                    );
                }
            }
            // Schedule backoff for this peer address.
            self.schedule_backoff_addr(peer.id(), peer.address());
            self.clear_low_buckets(peer.id());
            // Also drop any stale in-flight connecting entry for the same conn_id
            self.connecting_peers.remove(&conn_id);
            // If this was an incoming connection, update the incoming sets
            self.incoming_active.remove(&conn_id);
        } else {
            if let Some(pending_peer) = self.connecting_peers.remove(&conn_id) {
                // Pre-handshake failure to connect — back off this address.
                self.schedule_backoff_addr(pending_peer.id(), pending_peer.address());
            }
            // Could be a dropped incoming connection before handshake
            self.incoming_pending.remove(&conn_id);
        }
    }

    fn post(
        &mut self,
        Post {
            data,
            peer_id,
            priority,
        }: Post<T>,
    ) {
        iroha_logger::trace!(peer=%peer_id, "Post message");
        let topic = data.topic();
        if matches!(
            topic,
            message::Topic::Consensus
                | message::Topic::ConsensusPayload
                | message::Topic::ConsensusChunk
        ) {
            iroha_logger::debug!(
                peer = %peer_id,
                high = matches!(priority, Priority::High),
                "sending consensus frame to peer"
            );
        }
        if matches!(topic, message::Topic::TrustGossip) && !self.trust_gossip {
            iroha_logger::debug!(
                peer=%peer_id,
                "Skipping trust gossip post because local capability is disabled"
            );
            inc_trust_gossip_skipped("send", "local_capability_off");
            return;
        }
        let origin = self.self_id.clone();
        let relay_ttl = self.relay_ttl;
        let frame_for = |target: RelayTarget| {
            RelayMessage::new(origin.clone(), target, relay_ttl, priority, data.clone())
        };
        if let Some(hub_id) = self.relay_route_for_unconnected_post_target(&peer_id) {
            let frame = frame_for(RelayTarget::Direct(peer_id));
            let _ = self.send_frame_to_peer(&hub_id, frame, topic);
            return;
        }
        if self.send_frame_to_peer(
            &peer_id,
            frame_for(RelayTarget::Direct(peer_id.clone())),
            topic,
        ) {
            return;
        }
        if matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) {
            if let Some(hub_id) = self.hub_handle().map(|(id, _)| id.clone()) {
                let frame = frame_for(RelayTarget::Direct(peer_id));
                let _ = self.send_frame_to_peer(&hub_id, frame, topic);
            } else {
                iroha_logger::warn!(
                    peer=%peer_id,
                    "Relay mode could not route post because hub is unavailable"
                );
            }
        }
    }

    fn broadcast(&mut self, Broadcast { data, priority }: Broadcast<T>) {
        iroha_logger::trace!("Broadcast message");
        let topic = data.topic();
        if matches!(
            topic,
            message::Topic::Consensus
                | message::Topic::ConsensusPayload
                | message::Topic::ConsensusChunk
        ) {
            iroha_logger::debug!(
                high = matches!(priority, Priority::High),
                "broadcasting consensus frame to all peers"
            );
        }
        if matches!(topic, message::Topic::TrustGossip) && !self.trust_gossip {
            iroha_logger::debug!(
                "Skipping trust gossip broadcast because local capability is disabled"
            );
            inc_trust_gossip_skipped("send", "local_capability_off");
            return;
        }
        let peers: Vec<PeerId> = self.peers.keys().cloned().collect();
        for pid in peers {
            let frame = RelayMessage::new(
                self.self_id.clone(),
                RelayTarget::Broadcast,
                self.relay_ttl,
                priority,
                data.clone(),
            );
            if !self.send_frame_to_peer(&pid, frame, topic) {
                match topic {
                    message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
                        iroha_logger::warn!(
                            peer=%pid,
                            "Failed to enqueue tx gossip broadcast frame"
                        );
                    }
                    message::Topic::Consensus
                    | message::Topic::ConsensusPayload
                    | message::Topic::ConsensusChunk => {
                        iroha_logger::warn!(peer=%pid, "Failed to enqueue consensus broadcast frame");
                    }
                    _ => {}
                }
            }
        }
    }

    async fn peer_message(&mut self, msg: PeerMessage<WireMessage<T>>) {
        iroha_logger::trace!(peer=%msg.peer, "Received peer message");
        let topic = msg.payload.payload.topic();
        let size_bytes = msg.payload_bytes;
        let peer_id = msg.peer.id().clone();
        let peer_addr = msg.peer.address().clone();
        if matches!(topic, message::Topic::TrustGossip) {
            if !self.trust_gossip {
                Self::record_trust_gossip_skip(
                    &peer_id,
                    TrustDirection::Inbound,
                    "local_capability_off",
                );
                return;
            }
            if !self.peers.get(&peer_id).is_some_and(|p| p.trust_gossip) {
                Self::record_trust_gossip_skip(
                    &peer_id,
                    TrustDirection::Inbound,
                    "peer_capability_off",
                );
                return;
            }
        }
        let cap = match topic {
            message::Topic::Consensus => self.cap_consensus,
            // Payload-heavy consensus frames share the block-sync cap.
            message::Topic::ConsensusPayload
            | message::Topic::ConsensusChunk
            | message::Topic::BlockSync => self.cap_block_sync,
            message::Topic::Control => self.cap_control,
            message::Topic::TxGossip | message::Topic::TxGossipRestricted => self.cap_tx_gossip,
            message::Topic::PeerGossip | message::Topic::TrustGossip => self.cap_peer_gossip,
            message::Topic::Health => self.cap_health,
            message::Topic::Other => self.cap_other,
        };
        if size_bytes > cap {
            iroha_logger::warn!(peer=%msg.peer, topic=?topic, size=size_bytes, cap=cap, "Dropping inbound message exceeding topic cap");
            inc_cap_violation(topic);
            if let Some(ref_peer) = self.peers.remove(&peer_id) {
                self.peer_reputations.record_disconnected(&peer_id);
                self.schedule_backoff_addr(&peer_id, &ref_peer.p2p_addr);
                Self::remove_online_peer(
                    &self.online_peers_sender,
                    &self.online_peer_capabilities_sender,
                    &peer_id,
                );
                self.incoming_active.remove(&ref_peer.conn_id);
                self.last_active.remove(&peer_id);
                self.clear_low_buckets(&peer_id);
            } else {
                self.schedule_backoff_addr(&peer_id, &peer_addr);
                self.last_active.remove(&peer_id);
                self.clear_low_buckets(&peer_id);
            }
            return;
        }
        self.last_active
            .insert(peer_id.clone(), tokio::time::Instant::now());

        let incoming_peer = msg.peer;
        let RelayMessage {
            origin,
            target,
            ttl,
            priority,
            payload,
        } = msg.payload;
        // Most peers must send frames where `origin` matches their peer id. We only accept
        // relayed frames (origin mismatch) from explicitly trusted relay peers.
        let allow_origin_mismatch = match self.relay_mode {
            iroha_config::parameters::actual::RelayMode::Spoke
            | iroha_config::parameters::actual::RelayMode::Assist => self
                .relay_hub_peer
                .as_ref()
                .is_some_and(|hub| hub == incoming_peer.id()),
            iroha_config::parameters::actual::RelayMode::Hub => {
                self.relay_trusted_peers.contains(incoming_peer.id())
            }
            iroha_config::parameters::actual::RelayMode::Disabled => false,
        };
        if origin != *incoming_peer.id() && !allow_origin_mismatch {
            iroha_logger::warn!(
                peer = %incoming_peer,
                origin = %origin,
                "dropping relay frame with mismatched origin"
            );
            return;
        }
        if matches!(
            topic,
            message::Topic::Consensus
                | message::Topic::ConsensusPayload
                | message::Topic::ConsensusChunk
        ) {
            iroha_logger::debug!(
                from=%incoming_peer,
                origin=%origin,
                ?target,
                high=matches!(priority, Priority::High),
                size=size_bytes,
                topic=?topic,
                "received consensus frame"
            );
        }
        if matches!(topic, message::Topic::BlockSync) {
            iroha_logger::debug!(
                from=%incoming_peer,
                origin=%origin,
                ?target,
                high=matches!(priority, Priority::High),
                size=size_bytes,
                "received block sync frame"
            );
        }

        if ttl == 0
            && !matches!(&target, RelayTarget::Direct(id) if id == &self.self_id)
            && !matches!(target, RelayTarget::Broadcast)
        {
            iroha_logger::debug!(peer=%incoming_peer, "Dropping relay frame with expired ttl");
            return;
        }

        let deliver_local = matches!(target, RelayTarget::Broadcast)
            || matches!(&target, RelayTarget::Direct(id) if id == &self.self_id);

        // Forward first (only borrows `payload`) so we can move it into the local-delivery
        // message without cloning when hub-mode relay is enabled.
        if matches!(self.relay_role, RelayRole::Hub) {
            if let Some(next_ttl) = ttl.checked_sub(1) {
                match target {
                    RelayTarget::Broadcast => {
                        self.forward_broadcast(
                            &incoming_peer,
                            &origin,
                            &payload,
                            next_ttl,
                            priority,
                            topic,
                        );
                    }
                    RelayTarget::Direct(target_id) => {
                        if target_id != self.self_id {
                            self.forward_direct(
                                &incoming_peer,
                                &origin,
                                &target_id,
                                &payload,
                                next_ttl,
                                priority,
                                topic,
                            );
                        }
                    }
                }
            }
        }

        if deliver_local {
            let origin_peer = self.resolve_origin_peer(&origin, &incoming_peer);
            let deliver = PeerMessage {
                peer: origin_peer,
                payload,
                payload_bytes: msg.payload_bytes,
            };
            if matches!(
                topic,
                message::Topic::TxGossip | message::Topic::TxGossipRestricted
            ) {
                iroha_logger::debug!(
                    peer=%deliver.peer,
                    size_bytes,
                    "delivering tx gossip frame to subscribers"
                );
            } else if matches!(
                topic,
                message::Topic::Consensus
                    | message::Topic::ConsensusPayload
                    | message::Topic::ConsensusChunk
            ) {
                iroha_logger::debug!(
                    peer=%deliver.peer,
                    topic=?topic,
                    size_bytes,
                    "delivering consensus frame to subscribers"
                );
            }
            self.dispatch_to_subscribers(deliver);
        }
    }

    fn subscribe_to_peers_messages(&mut self, subscriber: Subscriber<T>) {
        self.subscribers_to_peers_messages.push(subscriber);
        iroha_logger::info!(
            subscribers = self.subscribers_to_peers_messages.len(),
            "registered peer message subscriber"
        );
    }

    fn dispatch_to_subscribers(&mut self, msg: PeerMessage<T>) {
        use tokio::sync::mpsc::error::TrySendError;

        let topic = msg.payload.topic();
        if self.subscribers_to_peers_messages.is_empty() {
            if matches!(
                topic,
                message::Topic::Consensus
                    | message::Topic::ConsensusPayload
                    | message::Topic::ConsensusChunk
            ) {
                iroha_logger::warn!(
                    peer = %msg.peer,
                    "dropping consensus frame because no subscribers are registered yet"
                );
            } else {
                iroha_logger::warn!("No subscribers to send message to");
            }
            return;
        }
        if matches!(topic, message::Topic::BlockSync) {
            iroha_logger::debug!(
                peer = %msg.peer,
                size_bytes = msg.payload_bytes,
                "dispatching block sync message to subscribers"
            );
        }
        let mut next = Vec::with_capacity(self.subscribers_to_peers_messages.len());
        let mut matched_any = false;
        for subscriber in self.subscribers_to_peers_messages.drain(..) {
            if !subscriber.filter.matches(topic) {
                next.push(subscriber);
                continue;
            }
            matched_any = true;
            match subscriber.sender.try_send(msg.clone()) {
                Ok(()) => next.push(subscriber),
                Err(TrySendError::Full(_)) => {
                    let drops = inc_subscriber_queue_full_for(topic);
                    if drops == 1 || drops % 1024 == 0 {
                        iroha_logger::warn!(
                            peer = %msg.peer,
                            topic = ?topic,
                            drops,
                            "subscriber queue full; dropping inbound message"
                        );
                    }
                    next.push(subscriber);
                }
                Err(TrySendError::Closed(_)) => {
                    iroha_logger::debug!("subscriber channel closed; dropping subscriber");
                }
            }
        }
        if !matched_any {
            let misses = inc_subscriber_unrouted_for(topic);
            if misses == 1 || misses % 1024 == 0 {
                iroha_logger::warn!(
                    peer = %msg.peer,
                    topic = ?topic,
                    misses,
                    "no subscribers registered for topic; dropping inbound message"
                );
            }
        }
        self.subscribers_to_peers_messages = next;
    }

    fn forward_broadcast(
        &mut self,
        incoming_peer: &Peer,
        origin: &PeerId,
        payload: &T,
        ttl: u8,
        priority: Priority,
        topic: message::Topic,
    ) {
        let targets: Vec<PeerId> = self.peers.keys().cloned().collect();
        for pid in targets {
            if pid == *incoming_peer.id() {
                continue;
            }
            let frame = RelayMessage::new(
                origin.clone(),
                RelayTarget::Broadcast,
                ttl,
                priority,
                payload.clone(),
            );
            self.send_frame_to_peer(&pid, frame, topic);
        }
    }

    fn forward_direct(
        &mut self,
        incoming_peer: &Peer,
        origin: &PeerId,
        target: &PeerId,
        payload: &T,
        ttl: u8,
        priority: Priority,
        topic: message::Topic,
    ) {
        if target == incoming_peer.id() {
            iroha_logger::debug!(%target, "Dropping relay frame targeted at sender");
            return;
        }
        let frame = RelayMessage::new(
            origin.clone(),
            RelayTarget::Direct(target.clone()),
            ttl,
            priority,
            payload.clone(),
        );
        let _ = self.send_frame_to_peer(target, frame, topic);
    }

    fn add_online_peer(
        online_peers_sender: &watch::Sender<OnlinePeers>,
        online_peer_capabilities_sender: &watch::Sender<message::OnlinePeerCapabilities>,
        peer: Peer,
        capabilities: message::PeerTransportCapabilities,
    ) {
        online_peers_sender.send_if_modified(|online_peers| {
            let inserted = online_peers.insert(peer.clone());
            if inserted {
                iroha_logger::info!(
                    peer=%peer.id(),
                    online_peers = online_peers.len(),
                    "peer connected"
                );
            }
            inserted
        });
        let peer_id = peer.id().clone();
        online_peer_capabilities_sender.send_if_modified(|online_caps| {
            online_caps.insert(peer_id.clone(), capabilities) != Some(capabilities)
        });
    }

    fn remove_online_peer(
        online_peers_sender: &watch::Sender<OnlinePeers>,
        online_peer_capabilities_sender: &watch::Sender<message::OnlinePeerCapabilities>,
        peer_id: &PeerId,
    ) {
        online_peers_sender.send_if_modified(|online_peers| online_peers.remove(peer_id));
        online_peer_capabilities_sender
            .send_if_modified(|online_caps| online_caps.remove(peer_id).is_some());
    }

    fn get_conn_id(&mut self) -> ConnectionId {
        let conn_id = self.current_conn_id;
        self.current_conn_id = conn_id.wrapping_add(1);
        conn_id
    }

    /// Whether total connection cap is exceeded.
    fn exceeds_caps(&self) -> bool {
        let Some(max_total) = self.max_total_connections else {
            return false;
        };
        // Total = active peers + outgoing connecting + pending incoming
        let total = self.peers.len() + self.connecting_peers.len() + self.incoming_pending.len();
        total >= max_total
    }

    /// Whether incoming cap is exceeded.
    fn exceeds_incoming_cap(&self) -> bool {
        let Some(max_in) = self.max_incoming else {
            return false;
        };
        let incoming = self.incoming_active.len() + self.incoming_pending.len();
        incoming >= max_in
    }

    /// Check and update per-IP accept throttle.
    fn allow_ip(&mut self, ip: std::net::IpAddr) -> bool {
        allow_ip_with_policy(
            &self.allow_nets,
            &self.deny_nets,
            self.allowlist_only,
            self.accept_params,
            &mut self.accept_prefix_buckets,
            &mut self.accept_ip_buckets,
            ip,
        )
    }

    /// Evict one incoming connection based on oldest last-activity; return true if any was evicted.
    fn evict_one_incoming(&mut self) -> bool {
        let mut candidate: Option<(PeerId, tokio::time::Instant)> = None;
        for (pid, ref_peer) in &self.peers {
            if self.incoming_active.contains(&ref_peer.conn_id) {
                let last = self
                    .last_active
                    .get(pid)
                    .copied()
                    .unwrap_or_else(tokio::time::Instant::now);
                match candidate {
                    None => candidate = Some((pid.clone(), last)),
                    Some((_, best)) => {
                        if last < best {
                            candidate = Some((pid.clone(), last));
                        }
                    }
                }
            }
        }
        if let Some((pid, _)) = candidate {
            iroha_logger::info!(%pid, "Evicting incoming connection due to caps (idle-first)");
            self.disconnect_peer(&pid);
            true
        } else {
            false
        }
    }

    /// Evict any connection based on oldest last-activity; return true if any was evicted.
    fn evict_one_any(&mut self) -> bool {
        let mut candidate: Option<(PeerId, tokio::time::Instant)> = None;
        for pid in self.peers.keys() {
            let last = self
                .last_active
                .get(pid)
                .copied()
                .unwrap_or_else(tokio::time::Instant::now);
            match candidate {
                None => candidate = Some((pid.clone(), last)),
                Some((_, best)) => {
                    if last < best {
                        candidate = Some((pid.clone(), last));
                    }
                }
            }
        }
        if let Some((pid, _)) = candidate {
            iroha_logger::info!(%pid, "Evicting connection due to total cap (idle-first)");
            self.disconnect_peer(&pid);
            true
        } else {
            false
        }
    }

    fn clear_low_buckets(&mut self, peer_id: &PeerId) {
        self.low_buckets.remove(peer_id);
        self.low_bytes_buckets.remove(peer_id);
    }
}

fn topology_tick_interval(configured: Duration) -> Duration {
    configured
}

#[cfg(test)]
fn cap_violation_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("cap violation test lock poisoned")
}

fn apply_connect_startup_delay(
    when: tokio::time::Instant,
    delay_until: tokio::time::Instant,
) -> tokio::time::Instant {
    if when < delay_until {
        delay_until
    } else {
        when
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    use iroha_crypto::{KeyPair, encryption::ChaCha20Poly1305, kex::X25519Sha256};
    use iroha_primitives::addr::socket_addr;
    use norito::codec::DecodeAll;
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::generate_mldsa_keypair;
    use tokio::sync::mpsc::error::TryRecvError;

    use super::*;

    #[derive(Clone, Debug, Decode, Encode)]
    struct DummyMsg;

    impl message::ClassifyTopic for DummyMsg {}

    #[derive(Clone, Copy, Debug, Decode, Encode)]
    struct TrustGossipMsg;

    impl message::ClassifyTopic for TrustGossipMsg {
        fn topic(&self) -> message::Topic {
            message::Topic::TrustGossip
        }
    }

    #[derive(Clone, Copy, Debug, Decode, Encode)]
    struct PeerGossipMsg;

    impl message::ClassifyTopic for PeerGossipMsg {
        fn topic(&self) -> message::Topic {
            message::Topic::PeerGossip
        }
    }

    #[derive(Clone, Copy, Debug, Decode, Encode)]
    enum TopicMsg {
        Trust,
        Peer,
    }

    impl message::ClassifyTopic for TopicMsg {
        fn topic(&self) -> message::Topic {
            match self {
                Self::Trust => message::Topic::TrustGossip,
                Self::Peer => message::Topic::PeerGossip,
            }
        }
    }

    macro_rules! impl_decode_from_slice_via_codec {
        ($($ty:ty),+ $(,)?) => {
            $(
                impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
                    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                        let mut slice = bytes;
                        let value = <Self as DecodeAll>::decode_all(&mut slice).map_err(|error| {
                            norito::core::Error::Message(format!("codec decode error: {error}"))
                        })?;
                        Ok((value, bytes.len() - slice.len()))
                    }
                }
            )+
        };
    }

    impl_decode_from_slice_via_codec!(DummyMsg, TrustGossipMsg, PeerGossipMsg, TopicMsg);

    #[test]
    fn trust_gossip_allowed_blocks_when_disabled() {
        assert!(trust_gossip_allowed(
            message::Topic::PeerGossip,
            /*trust_gossip=*/ false
        ));
        assert!(trust_gossip_allowed(
            message::Topic::PeerGossip,
            /*trust_gossip=*/ true
        ));
        assert!(trust_gossip_allowed(
            message::Topic::TrustGossip,
            /*trust_gossip=*/ true
        ));
        assert!(!trust_gossip_allowed(
            message::Topic::TrustGossip,
            /*trust_gossip=*/ false
        ));
    }

    #[test]
    fn subscriber_filter_routes_topics() {
        let Some(mut network) = bare_network_with::<TopicMsg>() else {
            return;
        };
        let (trust_tx, mut trust_rx) = mpsc::channel(1);
        let (peer_tx, mut peer_rx) = mpsc::channel(1);

        network.subscribe_to_peers_messages(Subscriber {
            sender: trust_tx,
            filter: SubscriberFilter::topics([message::Topic::TrustGossip]),
        });
        network.subscribe_to_peers_messages(Subscriber {
            sender: peer_tx,
            filter: SubscriberFilter::topics([message::Topic::PeerGossip]),
        });

        let peer = Peer::new(
            socket_addr!(127.0.0.1:202),
            KeyPair::random().public_key().clone(),
        );

        network.dispatch_to_subscribers(PeerMessage {
            peer: peer.clone(),
            payload: TopicMsg::Trust,
            payload_bytes: 1,
        });
        assert!(matches!(trust_rx.try_recv(), Ok(msg) if matches!(msg.payload, TopicMsg::Trust)));
        assert!(matches!(peer_rx.try_recv(), Err(TryRecvError::Empty)));

        network.dispatch_to_subscribers(PeerMessage {
            peer,
            payload: TopicMsg::Peer,
            payload_bytes: 1,
        });
        assert!(matches!(peer_rx.try_recv(), Ok(msg) if matches!(msg.payload, TopicMsg::Peer)));
        assert!(matches!(trust_rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn peer_message_channel_honors_capacity() {
        let cap = core::num::NonZeroUsize::new(2).expect("nonzero");
        let (tx, _rx) = peer_message_channel::<DummyMsg>(cap);

        let peer = Peer::new(
            socket_addr!(127.0.0.1:0),
            KeyPair::random().public_key().clone(),
        );
        let origin = PeerId::from(KeyPair::random().public_key().clone());
        let payload = RelayMessage::new(
            origin,
            RelayTarget::Broadcast,
            DEFAULT_RELAY_TTL,
            Priority::High,
            DummyMsg,
        );
        let msg = PeerMessage {
            peer,
            payload,
            payload_bytes: 1,
        };

        assert!(tx.try_send(msg.clone()).is_ok());
        assert!(tx.try_send(msg.clone()).is_ok());
        assert!(matches!(
            tx.try_send(msg),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_))
        ));
    }

    #[test]
    fn subscribe_with_filter_returns_error_when_closed() {
        let handle =
            NetworkBaseHandle::<DummyMsg, X25519Sha256, ChaCha20Poly1305>::closed_for_tests();
        let (tx, _rx) = mpsc::channel(1);
        assert!(
            handle
                .subscribe_to_peers_messages_with_filter(tx, SubscriberFilter::All)
                .is_err()
        );
    }

    #[test]
    fn runtime_from_handshake_accepts_signed_ticket_with_configured_key() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let mut handshake = ActualSoranetHandshake::default();
        handshake.pow.required = true;
        handshake.pow.difficulty = 1;
        handshake.pow.puzzle = None;
        handshake.pow.signed_ticket_public_key = Some(keypair.public_key().to_vec());

        let config = runtime_from_handshake(handshake).expect("runtime");

        let mut rng = StdRng::seed_from_u64(7);
        let minted = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint ticket")
            .expect("ticket bytes");
        let ticket_bytes = minted.ticket.expect("ticket bytes present");
        let ticket =
            iroha_crypto::soranet::pow::Ticket::parse(&ticket_bytes).expect("parse minted ticket");

        let signed = iroha_crypto::soranet::pow::SignedTicket::sign(
            ticket,
            &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
            None,
            keypair.secret_key(),
        )
        .expect("sign ticket");

        let admission = config
            .verify_challenge_ticket(&signed.encode())
            .expect("verify signed ticket")
            .expect("admission");
        assert_eq!(admission.pow.difficulty(), 1);
    }

    fn default_accept_params() -> AcceptThrottleParams {
        AcceptThrottleParams::new(
            None,
            None,
            iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
            None,
            None,
            iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
            iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        )
    }

    fn accept_params_with(
        max_buckets: usize,
        bucket_idle: Duration,
        prefix_rate: Option<f64>,
        ip_rate: Option<f64>,
    ) -> AcceptThrottleParams {
        AcceptThrottleParams::new(
            prefix_rate,
            None,
            iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
            ip_rate,
            None,
            max_buckets,
            bucket_idle,
        )
    }

    fn enter_test_runtime() -> Option<tokio::runtime::EnterGuard<'static>> {
        static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        if tokio::runtime::Handle::try_current().is_ok() {
            return None;
        }
        let rt = RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("test runtime should build")
        });
        Some(rt.enter())
    }

    fn trust_gossip_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("trust gossip test lock poisoned")
    }

    fn queue_depth_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("queue depth test lock poisoned")
    }

    fn deferred_send_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("deferred send test lock poisoned")
    }

    #[allow(clippy::too_many_lines)]
    fn bare_network() -> Option<NetworkBase<DummyMsg, X25519Sha256, ChaCha20Poly1305>> {
        bare_network_with::<DummyMsg>()
    }

    fn bare_network_with<T: Pload + message::ClassifyTopic>()
    -> Option<NetworkBase<T, X25519Sha256, ChaCha20Poly1305>> {
        let _guard = enter_test_runtime();
        let key_pair = KeyPair::random();
        let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(e) => panic!("listener bind failed: {e:?}"),
        };
        let listen_addr_std = std_listener.local_addr().unwrap();
        let listener_clone = std_listener.try_clone().unwrap();
        listener_clone.set_nonblocking(true).unwrap();
        let listener = match TcpListener::from_std(listener_clone) {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(e) => panic!("tcp listener from_std failed: {e:?}"),
        };

        let (_subscribe_tx, subscribe_rx) = mpsc::unbounded_channel::<Subscriber<T>>();
        let (_update_topology_tx, update_topology_rx) =
            mpsc::unbounded_channel::<message::UpdateTopology>();
        let (_update_peers_tx, update_peers_rx) = mpsc::unbounded_channel::<message::UpdatePeers>();
        let (_update_trusted_tx, update_trusted_peers_receiver) =
            mpsc::unbounded_channel::<message::UpdateTrustedPeers>();
        let (_update_acl_tx, update_acl_rx) = mpsc::unbounded_channel::<message::UpdateAcl>();
        let (_update_handshake_tx, update_handshake_rx) =
            mpsc::unbounded_channel::<message::UpdateHandshake>();
        let (_update_consensus_caps_tx, update_consensus_caps_receiver) =
            mpsc::unbounded_channel::<message::UpdateConsensusCaps>();
        let (peer_message_hi_tx, peer_message_hi_rx) =
            mpsc::channel::<PeerMessage<WireMessage<T>>>(1);
        let (peer_message_lo_tx, peer_message_lo_rx) =
            mpsc::channel::<PeerMessage<WireMessage<T>>>(1);
        let (service_message_tx, service_message_rx) =
            mpsc::channel::<ServiceMessage<WireMessage<T>>>(4);
        let (_network_hi_tx, network_message_high_rx) = net_channel::channel_with_capacity(1);
        let (_network_lo_tx, network_message_low_rx) = net_channel::channel_with_capacity(1);
        let (online_peers_tx, _online_peers_rx) = watch::channel(HashSet::new());
        let (online_peer_capabilities_tx, _online_peer_capabilities_rx) =
            watch::channel(HashMap::new());
        let (_update_peer_capabilities_tx, update_peer_capabilities_receiver) =
            mpsc::unbounded_channel::<message::UpdatePeerCapabilities>();

        let soranet = Arc::new(SoranetHandshakeConfig::defaults());

        Some(NetworkBase {
            listen_addr: listen_addr_std.into(),
            public_address: listen_addr_std.into(),
            relay_role: RelayRole::Disabled,
            relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_hub_peer: None,
            relay_trusted_peers: HashSet::new(),
            relay_ttl: DEFAULT_RELAY_TTL,
            trust_gossip_config: true,
            trust_gossip: true,
            self_id: PeerId::from(key_pair.public_key().clone()),
            address_book: HashMap::new(),
            peer_reputations: PeerReputationBook::default(),
            soranet_handshake: soranet,
            peers: HashMap::new(),
            connecting_peers: HashMap::new(),
            listener,
            key_pair,
            subscribers_to_peers_messages: Vec::new(),
            subscribe_to_peers_messages_receiver: subscribe_rx,
            online_peers_sender: online_peers_tx,
            online_peer_capabilities_sender: online_peer_capabilities_tx,
            update_topology_receiver: update_topology_rx,
            update_peers_receiver: update_peers_rx,
            update_peer_capabilities_receiver,
            update_trusted_peers_receiver,
            update_acl_receiver: update_acl_rx,
            update_handshake_receiver: update_handshake_rx,
            update_consensus_caps_receiver,
            network_message_high_receiver: network_message_high_rx,
            network_message_low_receiver: network_message_low_rx,
            peer_message_high_receiver: peer_message_hi_rx,
            peer_message_low_receiver: peer_message_lo_rx,
            peer_message_high_sender: peer_message_hi_tx,
            peer_message_low_sender: peer_message_lo_tx,
            service_message_receiver: service_message_rx,
            service_message_sender: service_message_tx,
            current_conn_id: 0,
            current_topology: HashSet::new(),
            current_peers_addresses: Vec::new(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            peer_capabilities: HashMap::new(),
            post_queue_cap: 4,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            dns_last_refresh: HashMap::new(),
            topology_update_interval:
                iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            dns_pending_refresh: HashSet::new(),
            idle_timeout: Duration::from_millis(50),
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            tcp_nodelay: true,
            tcp_keepalive: None,
            connect_startup_delay_until: tokio::time::Instant::now(),
            quic_enabled: false,
            quic_datagrams_enabled: false,
            quic_datagram_max_payload_bytes: 0,
            quic_dialer: None,
            local_scion_supported: false,
            tls_enabled: false,
            tls_fallback_to_plain: false,
            prefer_ws_fallback: false,
            proxy_policy: crate::transport::ProxyPolicy::disabled(),
            proxy_tls_verify: true,
            proxy_tls_pinned_cert_der: None,
            allowlist_only: false,
            allow_keys: HashSet::new(),
            deny_keys: HashSet::new(),
            allow_nets: Vec::new(),
            deny_nets: Vec::new(),
            retry_backoff: HashMap::new(),
            peer_session_generation: HashMap::new(),
            pending_connects: Vec::new(),
            deferred_send_queue: DeferredPeerFrameQueue::new(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
                Duration::from_millis(
                    iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
                ),
            ),
            happy_eyeballs_stagger: Duration::from_millis(10),
            addr_ipv6_first: false,
            last_active: HashMap::new(),
            incoming_pending: HashSet::new(),
            incoming_active: HashSet::new(),
            max_incoming: None,
            max_total_connections: None,
            accept_params: AcceptThrottleParams::new(
                None,
                None,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
                None,
                None,
                iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS.get(),
                iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            ),
            accept_prefix_buckets: HashMap::new(),
            accept_ip_buckets: HashMap::new(),
            sampler_high_queue_warn: LogSampler::new(),
            sampler_low_queue_warn: LogSampler::new(),
            sampler_accept_err: LogSampler::new(),
            low_rate_per_sec: None,
            low_burst: None,
            low_buckets: HashMap::new(),
            low_bytes_per_sec: None,
            low_bytes_burst: None,
            low_bytes_buckets: HashMap::new(),
            max_frame_bytes: 1024,
            cap_consensus: 1024,
            cap_control: 1024,
            cap_block_sync: 1024,
            cap_tx_gossip: 1024,
            cap_peer_gossip: 1024,
            cap_health: 1024,
            cap_other: 1024,
            disconnect_on_post_overflow: false,
            _key_exchange: core::marker::PhantomData,
            _encryptor: core::marker::PhantomData,
        })
    }

    fn trust_skip_count(direction: &str, reason: &str) -> u64 {
        #[cfg(feature = "telemetry")]
        {
            iroha_telemetry::metrics::global_or_default()
                .p2p_trust_gossip_skipped_total
                .get_metric_with_label_values(&[direction, reason])
                .map(|counter| counter.get())
                .unwrap_or(0)
        }
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = (direction, reason);
            trust_gossip_skipped_capability_off_count()
        }
    }

    #[test]
    fn ip_bucket_v4_groups_by_24() {
        let k1 = ip_bucket_key(IpAddr::from([192, 168, 1, 10]), 24, 64);
        let k2 = ip_bucket_key(IpAddr::from([192, 168, 1, 200]), 24, 64);
        let k3 = ip_bucket_key(IpAddr::from([192, 168, 2, 1]), 24, 64);
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn token_bucket_allows_burst_then_throttles() {
        let mut tb = TokenBucket::new(10.0, 3.0);
        // Allow immediately up to burst
        assert!(tb.allow());
        assert!(tb.allow());
        assert!(tb.allow());
        // Next one should be throttled without time passing
        assert!(!tb.allow());
    }

    #[test]
    fn topology_update_interval_uses_configured_period() {
        let Some(network) = bare_network() else {
            return;
        };
        assert_eq!(
            network.topology_update_interval,
            iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD
        );
    }

    #[test]
    fn topology_tick_interval_ignores_env_override() {
        let configured = Duration::from_millis(500);
        assert_eq!(topology_tick_interval(configured), configured);
    }

    #[test]
    fn connect_startup_delay_clamps_when_before_deadline() {
        let now = tokio::time::Instant::now();
        let delay_until = now + Duration::from_secs(5);
        let when = now + Duration::from_secs(1);
        assert_eq!(apply_connect_startup_delay(when, delay_until), delay_until);
    }

    #[test]
    fn update_topology_respects_startup_delay() {
        let Some(mut network) = bare_network() else {
            return;
        };
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let addr = socket_addr!(127.0.0.1:34567);
        let delay_until = tokio::time::Instant::now() + Duration::from_secs(5);
        network.connect_startup_delay_until = delay_until;
        network.current_topology.insert(peer_id.clone());
        network.current_peers_addresses.push((peer_id, addr));

        network.update_topology();

        assert!(
            !network.pending_connects.is_empty(),
            "pending connects should be scheduled for new topology peers"
        );
        assert!(
            network
                .pending_connects
                .iter()
                .all(|(when, _)| *when >= delay_until),
            "scheduled connect attempts must honor startup delay"
        );
    }

    #[test]
    fn trusted_observers_survive_topology_updates() {
        let Some(mut network) = bare_network() else {
            return;
        };
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let trusted_id = PeerId::from(KeyPair::random().public_key().clone());
        let mut trusted = HashSet::new();
        trusted.insert(trusted_id.clone());
        network.peer_reputations.set_trusted(&trusted);

        network.set_current_topology(UpdateTopology([peer_id.clone()].into_iter().collect()));

        assert!(
            network.current_topology.contains(&peer_id),
            "topology should retain peers from update"
        );
        assert!(
            network.current_topology.contains(&trusted_id),
            "trusted observers should remain connected across topology updates"
        );
    }

    #[test]
    fn process_pending_connects_respects_startup_delay() {
        let Some(mut network) = bare_network() else {
            return;
        };
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let addr = socket_addr!(127.0.0.1:45678);
        let delay_until = tokio::time::Instant::now() + Duration::from_secs(5);
        network.connect_startup_delay_until = delay_until;
        network.current_topology.insert(peer_id.clone());
        network
            .pending_connects
            .push((tokio::time::Instant::now(), Peer::new(addr, peer_id)));

        network.process_pending_connects();

        assert!(
            network.connecting_peers.is_empty(),
            "startup delay should prevent immediate dial attempts"
        );
        assert_eq!(
            network.pending_connects.len(),
            1,
            "connect attempt should be rescheduled"
        );
        assert!(
            network.pending_connects[0].0 >= delay_until,
            "rescheduled connect should honor startup delay"
        );
    }

    #[test]
    fn deferred_queue_preserves_order_and_generation_markers() {
        let _guard = deferred_send_test_guard();
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let now = tokio::time::Instant::now();
        let mut queue = DeferredPeerFrameQueue::<DummyMsg>::new(8, Duration::from_secs(1));

        let frame_one = RelayMessage::new(
            peer_id.clone(),
            RelayTarget::Direct(peer_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            DummyMsg,
        );
        let frame_two = RelayMessage::new(
            peer_id.clone(),
            RelayTarget::Direct(peer_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::High,
            DummyMsg,
        );
        let frame_three = RelayMessage::new(
            peer_id.clone(),
            RelayTarget::Direct(peer_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            DummyMsg,
        );

        let (expired_one, overflow_one) = queue.enqueue(
            peer_id.clone(),
            frame_one,
            message::Topic::Other,
            Some(11),
            now,
        );
        let (expired_two, overflow_two) = queue.enqueue(
            peer_id.clone(),
            frame_two,
            message::Topic::Other,
            Some(12),
            now + Duration::from_millis(1),
        );
        let (expired_three, overflow_three) = queue.enqueue(
            peer_id.clone(),
            frame_three,
            message::Topic::Other,
            None,
            now + Duration::from_millis(2),
        );
        assert_eq!((expired_one, overflow_one), (0, 0));
        assert_eq!((expired_two, overflow_two), (0, 0));
        assert_eq!((expired_three, overflow_three), (0, 0));

        let (queued, expired) = queue.take_peer(&peer_id, now + Duration::from_millis(3));
        assert_eq!(expired, 0);
        let observed_generations: Vec<Option<ConnectionId>> =
            queued.iter().map(|entry| entry.generation).collect();
        assert_eq!(observed_generations, vec![Some(11), Some(12), None]);
    }

    #[test]
    fn deferred_queue_ttl_and_cap_drop_oldest_deterministically() {
        let _guard = deferred_send_test_guard();
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let now = tokio::time::Instant::now();
        let mut queue = DeferredPeerFrameQueue::<DummyMsg>::new(2, Duration::from_millis(5));

        let mk_frame = || {
            RelayMessage::new(
                peer_id.clone(),
                RelayTarget::Direct(peer_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::Low,
                DummyMsg,
            )
        };

        let (_, overflow_one) = queue.enqueue(
            peer_id.clone(),
            mk_frame(),
            message::Topic::Other,
            Some(1),
            now,
        );
        let (_, overflow_two) = queue.enqueue(
            peer_id.clone(),
            mk_frame(),
            message::Topic::Other,
            Some(2),
            now + Duration::from_millis(1),
        );
        let (_, overflow_three) = queue.enqueue(
            peer_id.clone(),
            mk_frame(),
            message::Topic::Other,
            Some(3),
            now + Duration::from_millis(2),
        );
        assert_eq!(overflow_one, 0);
        assert_eq!(overflow_two, 0);
        assert_eq!(overflow_three, 1, "queue cap should evict oldest entry");

        let (queued_before_ttl, expired_before_ttl) =
            queue.take_peer(&peer_id, now + Duration::from_millis(3));
        assert_eq!(expired_before_ttl, 0);
        let kept_generations: Vec<Option<ConnectionId>> = queued_before_ttl
            .iter()
            .map(|entry| entry.generation)
            .collect();
        assert_eq!(kept_generations, vec![Some(2), Some(3)]);

        let mut ttl_queue = DeferredPeerFrameQueue::<DummyMsg>::new(2, Duration::from_millis(5));
        let _ = ttl_queue.enqueue(
            peer_id.clone(),
            mk_frame(),
            message::Topic::Other,
            Some(9),
            now,
        );
        let (queued_after_ttl, expired_after_ttl) =
            ttl_queue.take_peer(&peer_id, now + Duration::from_millis(20));
        assert_eq!(queued_after_ttl.len(), 0);
        assert_eq!(
            expired_after_ttl, 1,
            "expired entries should be dropped on flush"
        );
    }

    #[test]
    fn missing_session_defers_frame_and_schedules_reconnect() {
        let _guard = deferred_send_test_guard();
        let Some(mut network) = bare_network() else {
            return;
        };

        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let peer_addr = socket_addr!(127.0.0.1:45679);
        network.current_topology.insert(peer_id.clone());
        network
            .current_peers_addresses
            .push((peer_id.clone(), peer_addr.clone()));

        let now = tokio::time::Instant::now();
        network.retry_backoff.insert(
            peer_id.clone(),
            HashMap::from([(
                peer_addr.to_string(),
                (now + Duration::from_secs(1), Duration::from_millis(25)),
            )]),
        );

        let deferred_before = deferred_send_enqueued_count();
        let reconnect_before = session_reconnect_total();

        let frame = RelayMessage::new(
            network.self_id.clone(),
            RelayTarget::Direct(peer_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            DummyMsg,
        );
        assert!(
            network.send_frame_to_peer(&peer_id, frame, message::Topic::Other),
            "frame should be deferred when peer session is missing"
        );
        assert_eq!(
            network
                .deferred_send_queue
                .by_peer
                .get(&peer_id)
                .map(VecDeque::len),
            Some(1),
            "missing-session send should enqueue exactly one deferred frame"
        );
        assert!(
            !network.pending_connects.is_empty() || !network.connecting_peers.is_empty(),
            "missing-session defer should schedule reconnect work"
        );
        assert!(
            deferred_send_enqueued_count() >= deferred_before.saturating_add(1),
            "deferred send counter should increment"
        );
        assert!(
            session_reconnect_total() >= reconnect_before.saturating_add(1),
            "reconnect counter should increment"
        );
    }

    #[test]
    fn missing_session_deferred_burst_schedules_single_reconnect() {
        let _guard = deferred_send_test_guard();
        let Some(mut network) = bare_network() else {
            return;
        };

        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let peer_addr = socket_addr!(127.0.0.1:45680);
        network.current_topology.insert(peer_id.clone());
        network
            .current_peers_addresses
            .push((peer_id.clone(), peer_addr.clone()));

        let now = tokio::time::Instant::now();
        network.retry_backoff.insert(
            peer_id.clone(),
            HashMap::from([(
                peer_addr.to_string(),
                (now + Duration::from_secs(2), Duration::from_millis(50)),
            )]),
        );

        let reconnect_before = session_reconnect_total();
        for _ in 0..8 {
            let frame = RelayMessage::new(
                network.self_id.clone(),
                RelayTarget::Direct(peer_id.clone()),
                DEFAULT_RELAY_TTL,
                Priority::High,
                DummyMsg,
            );
            assert!(
                network.send_frame_to_peer(&peer_id, frame, message::Topic::Consensus),
                "missing-session send should defer outbound frame"
            );
        }

        assert!(
            network.pending_connects.len() <= 1,
            "deferred burst should not schedule duplicate reconnect attempts"
        );
        assert!(
            network
                .connecting_peers
                .values()
                .filter(|peer| peer.id() == &peer_id)
                .count()
                <= 1,
            "deferred burst should not spawn duplicate active reconnects"
        );
        assert_eq!(
            session_reconnect_total(),
            reconnect_before.saturating_add(1),
            "deferred burst should trigger reconnect once per unsatisfied peer session"
        );
    }

    #[test]
    fn allowlist_only_does_not_gate_ip_without_cidr_allowlist() {
        let ip = IpAddr::from([10, 0, 0, 1]);
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        assert!(
            allow_ip_with_policy(
                &[],
                &[],
                true,
                default_accept_params(),
                &mut prefix,
                &mut ip_buckets,
                ip
            ),
            "allowlist-only toggle must not block IPs when no CIDR allowlist is configured"
        );
    }

    #[test]
    fn cidr_allowlist_enforced_when_present() {
        let ip = IpAddr::from([10, 0, 0, 1]);
        let other = IpAddr::from([10, 0, 1, 1]);
        let allow = parse_cidrs(&vec!["10.0.0.0/24".to_string()]);
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        assert!(
            allow_ip_with_policy(
                &allow,
                &[],
                false,
                default_accept_params(),
                &mut prefix,
                &mut ip_buckets,
                ip
            ),
            "IP inside allowlist CIDR should be accepted"
        );
        assert!(
            !allow_ip_with_policy(
                &allow,
                &[],
                false,
                default_accept_params(),
                &mut prefix,
                &mut ip_buckets,
                other
            ),
            "IP outside allowlist CIDR should be rejected"
        );
    }

    #[test]
    fn ipv6_cidr_byte_boundary_is_respected() {
        let allow = parse_cidrs(&vec!["2001:db8::/64".to_string()]);
        let inside: IpAddr = "2001:db8::1".parse().expect("valid IPv6");
        let outside: IpAddr = "2001:db8:0:1::1".parse().expect("valid IPv6");
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();

        assert!(
            allow_ip_with_policy(
                &allow,
                &[],
                false,
                default_accept_params(),
                &mut prefix,
                &mut ip_buckets,
                inside
            ),
            "IPv6 address inside the /64 should be accepted"
        );
        assert!(
            !allow_ip_with_policy(
                &allow,
                &[],
                false,
                default_accept_params(),
                &mut prefix,
                &mut ip_buckets,
                outside
            ),
            "IPv6 address outside the /64 should be rejected"
        );
    }

    #[test]
    fn prefix_bucket_throttles_before_per_ip() {
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        let params = accept_params_with(8, Duration::from_secs(1), Some(1.0), Some(5.0));

        assert!(
            allow_ip_with_policy(
                &[],
                &[],
                false,
                params,
                &mut prefix,
                &mut ip_buckets,
                IpAddr::from([192, 168, 10, 1])
            ),
            "first connection in prefix should pass"
        );
        assert!(
            !allow_ip_with_policy(
                &[],
                &[],
                false,
                params,
                &mut prefix,
                &mut ip_buckets,
                IpAddr::from([192, 168, 10, 2])
            ),
            "second connection in prefix should be throttled by prefix bucket before per-IP bucket"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn accept_idle_buckets_are_evicted() {
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        let params = accept_params_with(4, Duration::from_millis(5), Some(10.0), Some(10.0));
        let ip_a = IpAddr::from([10, 0, 0, 1]);
        let ip_b = IpAddr::from([10, 0, 1, 1]);

        assert!(allow_ip_with_policy(
            &[],
            &[],
            false,
            params,
            &mut prefix,
            &mut ip_buckets,
            ip_a
        ));
        assert_eq!(prefix.len(), 1);
        tokio::time::advance(Duration::from_millis(10)).await;
        assert!(allow_ip_with_policy(
            &[],
            &[],
            false,
            params,
            &mut prefix,
            &mut ip_buckets,
            ip_b
        ));
        assert!(
            prefix.len() <= 1 && ip_buckets.len() <= 1,
            "idle buckets should be evicted before inserting new ones"
        );
    }

    #[test]
    fn allowlist_bypasses_throttle_state() {
        let allow = parse_cidrs(&vec!["10.1.0.0/24".to_string()]);
        let ip = IpAddr::from([10, 1, 0, 9]);
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        let params = accept_params_with(1, Duration::from_secs(30), Some(1.0), Some(1.0));

        assert!(allow_ip_with_policy(
            &allow,
            &[],
            false,
            params,
            &mut prefix,
            &mut ip_buckets,
            ip
        ));
        assert!(
            prefix.is_empty() && ip_buckets.is_empty(),
            "allowlisted IPs should bypass throttle buckets entirely"
        );
    }

    #[test]
    fn accept_bucket_cap_enforced_under_churn() {
        let mut prefix = HashMap::new();
        let mut ip_buckets = HashMap::new();
        let params = accept_params_with(1, Duration::from_secs(60), None, Some(5.0));
        let evicted_before = accept_bucket_evictions_count();

        for octet in 1..20 {
            let _ = allow_ip_with_policy(
                &[],
                &[],
                false,
                params,
                &mut prefix,
                &mut ip_buckets,
                IpAddr::from([172, 16, 0, octet]),
            );
        }

        assert!(
            prefix.len() <= 1 && ip_buckets.len() <= 1,
            "bucket map must stay within configured cap"
        );
        assert!(
            accept_bucket_evictions_count() > evicted_before,
            "evictions counter should increase when cap forces churn"
        );
    }

    #[test]
    fn trust_gossip_send_skips_when_disabled() {
        let _guard = trust_gossip_test_guard();
        let Some(mut network) = bare_network_with::<TrustGossipMsg>() else {
            return;
        };
        network.trust_gossip = false;

        let before = trust_skip_count("send", "local_capability_off");
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        network.post(Post {
            data: TrustGossipMsg,
            peer_id: peer_id.clone(),
            priority: Priority::Low,
        });
        network.broadcast(Broadcast {
            data: TrustGossipMsg,
            priority: Priority::Low,
        });

        let after = trust_skip_count("send", "local_capability_off");
        assert!(
            after >= before + 2,
            "post + broadcast should both be skipped when trust gossip is disabled (before={before}, after={after})"
        );
    }

    #[test]
    fn peer_gossip_not_counted_as_trust_skip() {
        let _guard = trust_gossip_test_guard();
        let Some(mut network) = bare_network_with::<PeerGossipMsg>() else {
            return;
        };
        network.trust_gossip = false;

        let before = trust_skip_count("send", "local_capability_off");
        network.broadcast(Broadcast {
            data: PeerGossipMsg,
            priority: Priority::Low,
        });
        let after = trust_skip_count("send", "local_capability_off");

        assert_eq!(
            before, after,
            "peer gossip should not bump trust-gossip skip counters"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn trust_gossip_recv_skips_when_local_capability_disabled() {
        let _guard = trust_gossip_test_guard();
        let Some(mut network) = bare_network_with::<TrustGossipMsg>() else {
            return;
        };
        network.trust_gossip = false;
        let peer = Peer::new(
            socket_addr!(127.0.0.1:200),
            KeyPair::random().public_key().clone(),
        );
        let payload = RelayMessage::new(
            peer.id().clone(),
            RelayTarget::Direct(network.self_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            TrustGossipMsg,
        );
        let msg = PeerMessage {
            peer,
            payload,
            payload_bytes: 1,
        };
        let before = trust_skip_count("recv", "local_capability_off");
        network.peer_message(msg).await;
        let after = trust_skip_count("recv", "local_capability_off");

        assert_eq!(
            after,
            before + 1,
            "local trust-gossip disablement should drop inbound frames"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn trust_gossip_recv_skips_when_peer_lacks_capability() {
        let _guard = trust_gossip_test_guard();
        let Some(mut network) = bare_network_with::<TrustGossipMsg>() else {
            return;
        };
        let peer = Peer::new(
            socket_addr!(127.0.0.1:201),
            KeyPair::random().public_key().clone(),
        );
        let payload = RelayMessage::new(
            peer.id().clone(),
            RelayTarget::Direct(network.self_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            TrustGossipMsg,
        );
        let msg = PeerMessage {
            peer,
            payload,
            payload_bytes: 1,
        };
        let before = trust_skip_count("recv", "peer_capability_off");
        network.peer_message(msg).await;
        let after = trust_skip_count("recv", "peer_capability_off");

        assert_eq!(
            after,
            before + 1,
            "trust gossip from peers without negotiated support should be rejected"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn peer_message_drops_mismatched_origin_without_hub() {
        let Some(mut network) = bare_network_with::<DummyMsg>() else {
            return;
        };
        let (tx, mut rx) = mpsc::channel(1);
        network.subscribe_to_peers_messages(Subscriber {
            sender: tx,
            filter: SubscriberFilter::All,
        });
        let incoming_peer = Peer::new(
            socket_addr!(127.0.0.1:202),
            KeyPair::random().public_key().clone(),
        );
        let origin = PeerId::from(KeyPair::random().public_key().clone());
        let payload = RelayMessage::new(
            origin,
            RelayTarget::Direct(network.self_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            DummyMsg,
        );
        let msg = PeerMessage {
            peer: incoming_peer,
            payload,
            payload_bytes: 1,
        };

        network.peer_message(msg).await;

        assert!(
            matches!(rx.try_recv(), Err(TryRecvError::Empty)),
            "mismatched origin should be dropped when not relaying from the hub"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn peer_message_allows_mismatched_origin_from_hub_in_spoke_mode() {
        let Some(mut network) = bare_network_with::<DummyMsg>() else {
            return;
        };
        network.relay_mode = iroha_config::parameters::actual::RelayMode::Spoke;
        let hub_peer = Peer::new(
            socket_addr!(127.0.0.1:203),
            KeyPair::random().public_key().clone(),
        );
        network.relay_hub_peer = Some(hub_peer.id().clone());
        let (tx, mut rx) = mpsc::channel(1);
        network.subscribe_to_peers_messages(Subscriber {
            sender: tx,
            filter: SubscriberFilter::All,
        });
        let origin = PeerId::from(KeyPair::random().public_key().clone());
        let payload = RelayMessage::new(
            origin.clone(),
            RelayTarget::Direct(network.self_id.clone()),
            DEFAULT_RELAY_TTL,
            Priority::Low,
            DummyMsg,
        );
        let msg = PeerMessage {
            peer: hub_peer,
            payload,
            payload_bytes: 1,
        };

        network.peer_message(msg).await;

        let received = rx.try_recv().expect("expected relay message");
        assert_eq!(
            received.peer.id(),
            &origin,
            "origin should be preserved when relaying through the hub"
        );
    }

    #[test]
    fn relay_routes_unconnected_spoke_posts_via_configured_hub() {
        let Some(mut network) = bare_network() else {
            return;
        };
        network.relay_mode = iroha_config::parameters::actual::RelayMode::Spoke;

        let hub_addr = socket_addr!(127.0.0.1:204);
        let hub_id = PeerId::from(KeyPair::random().public_key().clone());
        let target = PeerId::from(KeyPair::random().public_key().clone());

        network.relay_hub_addresses.push(hub_addr.clone());
        network
            .current_peers_addresses
            .push((hub_id.clone(), hub_addr));
        network.current_topology.insert(hub_id.clone());

        assert_eq!(
            network.relay_route_for_unconnected_post_target(&target),
            Some(hub_id),
            "spoke mode should route unknown direct targets through the selected hub"
        );
    }

    #[test]
    fn relay_keeps_direct_hub_posts_out_of_hub_reroute() {
        let Some(mut network) = bare_network() else {
            return;
        };
        network.relay_mode = iroha_config::parameters::actual::RelayMode::Spoke;

        let hub_addr = socket_addr!(127.0.0.1:205);
        let hub_id = PeerId::from(KeyPair::random().public_key().clone());

        network.relay_hub_addresses.push(hub_addr.clone());
        network
            .current_peers_addresses
            .push((hub_id.clone(), hub_addr));
        network.current_topology.insert(hub_id.clone());

        assert_eq!(
            network.relay_route_for_unconnected_post_target(&hub_id),
            None,
            "posts addressed to the hub itself should stay on the direct path"
        );
    }

    #[test]
    fn clear_low_buckets_removes_entries() {
        let peer_id = PeerId::from(KeyPair::random().public_key().clone());
        let mut low = HashMap::new();
        let mut low_bytes = HashMap::new();
        low.insert(peer_id.clone(), TokenBucket::new(1.0, 1.0));
        low_bytes.insert(peer_id.clone(), TokenBucket::new(1.0, 1.0));

        // Exercise the helper without requiring a full network instance.
        let Some(mut network) = bare_network() else {
            return;
        };
        network.low_buckets = low;
        network.low_bytes_buckets = low_bytes;
        network.clear_low_buckets(&peer_id);

        assert!(
            !network.low_buckets.contains_key(&peer_id),
            "per-peer token bucket should be removed"
        );
        assert!(
            !network.low_bytes_buckets.contains_key(&peer_id),
            "per-peer byte bucket should be removed"
        );
    }

    #[test]
    fn dispatch_to_subscribers_keeps_subscriber_on_full_channel() {
        let Some(mut network) = bare_network_with::<DummyMsg>() else {
            return;
        };
        let (tx, mut rx) = mpsc::channel(1);
        let peer = Peer::new(
            socket_addr!(127.0.0.1:2121),
            KeyPair::random().public_key().clone(),
        );
        let msg = PeerMessage {
            peer,
            payload: DummyMsg,
            payload_bytes: 1,
        };
        tx.try_send(msg.clone()).expect("fill channel");
        network.subscribers_to_peers_messages.push(Subscriber {
            sender: tx,
            filter: SubscriberFilter::All,
        });

        network.dispatch_to_subscribers(msg);

        assert_eq!(
            network.subscribers_to_peers_messages.len(),
            1,
            "subscriber should be retained when queue is full"
        );
        let first = rx.try_recv().expect("expected buffered message");
        let PeerMessage {
            payload: DummyMsg, ..
        } = first;
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn dispatch_to_subscribers_tracks_unmatched_topics() {
        let Some(mut network) = bare_network_with::<TopicMsg>() else {
            return;
        };
        let (tx, mut rx) = mpsc::channel(1);
        network.subscribe_to_peers_messages(Subscriber {
            sender: tx,
            filter: SubscriberFilter::topics([message::Topic::TrustGossip]),
        });
        let peer = Peer::new(
            socket_addr!(127.0.0.1:303),
            KeyPair::random().public_key().clone(),
        );

        let before = subscriber_unrouted_count();
        network.dispatch_to_subscribers(PeerMessage {
            peer,
            payload: TopicMsg::Peer,
            payload_bytes: 1,
        });
        let after = subscriber_unrouted_count();

        assert!(
            after >= before + 1,
            "unmatched topics should increment the unrouted counter"
        );
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn subscriber_queue_full_counts_increment_by_topic() {
        let before_total = subscriber_queue_full_count();
        let before_consensus = subscriber_queue_full_consensus_count();
        let before_chunks = subscriber_queue_full_consensus_chunk_count();
        inc_subscriber_queue_full_for_test(message::Topic::Consensus, 2);
        inc_subscriber_queue_full_for_test(message::Topic::ConsensusChunk, 1);
        let after_total = subscriber_queue_full_count();
        let after_consensus = subscriber_queue_full_consensus_count();
        let after_chunks = subscriber_queue_full_consensus_chunk_count();

        assert!(
            after_total >= before_total + 3,
            "queue-full counter should increase by at least the increment"
        );
        assert!(
            after_consensus >= before_consensus + 2,
            "queue-full per-topic counter should track consensus drops"
        );
        assert!(
            after_chunks >= before_chunks + 1,
            "queue-full per-topic counter should track consensus chunk drops"
        );
    }

    #[test]
    fn subscriber_unrouted_counts_increment_by_topic() {
        let before = subscriber_unrouted_count();
        let before_peer = subscriber_unrouted_peer_gossip_count();
        let before_chunks = subscriber_unrouted_consensus_chunk_count();
        inc_subscriber_unrouted_for_test(message::Topic::TrustGossip, 3);
        inc_subscriber_unrouted_for_test(message::Topic::ConsensusChunk, 1);
        let after = subscriber_unrouted_count();
        let after_peer = subscriber_unrouted_peer_gossip_count();
        let after_chunks = subscriber_unrouted_consensus_chunk_count();

        assert!(
            after >= before + 4,
            "increment helper should increase subscriber unrouted count"
        );
        assert!(
            after_peer >= before_peer + 3,
            "trust-gossip should be grouped under peer gossip counters"
        );
        assert!(
            after_chunks >= before_chunks + 1,
            "consensus chunk drops should be tracked separately"
        );
    }

    #[test]
    fn network_queue_depth_tracks_updates() {
        let _guard = queue_depth_test_guard();
        set_network_queue_depth_for_test(true, 0);
        set_network_queue_depth_for_test(false, 0);
        set_network_queue_depth_for_test(true, 12);
        set_network_queue_depth_for_test(false, 7);

        assert_eq!(network_queue_depth_high(), 12);
        assert_eq!(network_queue_depth_low(), 7);
    }

    #[test]
    fn pending_connects_drop_outside_topology() {
        let mut network = match bare_network() {
            Some(net) => net,
            None => return,
        };

        let peer = Peer::new(
            socket_addr!(127.0.0.1:9),
            PeerId::from(KeyPair::random().public_key().clone()),
        );
        network
            .pending_connects
            .push((tokio::time::Instant::now(), peer));

        network.process_pending_connects();

        assert!(
            network.pending_connects.is_empty(),
            "pending connects for peers outside topology should be dropped"
        );
        assert!(
            network.connecting_peers.is_empty(),
            "connect should not be attempted for peers outside topology"
        );
    }
}

pub mod message {
    //! Module for network messages

    use iroha_data_model::peer::Peer;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Priority for network messages.
    #[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
    pub enum Priority {
        /// Critical messages (consensus/control) that should be serviced first.
        High,
        /// Best-effort messages (gossip/sync) that can yield to high-priority ones.
        Low,
    }

    /// Logical topic for scheduling per-topic substreams.
    ///
    /// Topics are advisory tags used by the peer to route messages into
    /// separate internal queues. This avoids head-of-line blocking between
    /// unrelated flows (e.g., consensus vs. block sync vs. gossip) and allows
    /// basic prioritization.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Topic {
        /// Consensus data plane (votes, hints, and critical block-payload control signals).
        Consensus,
        /// Consensus payload chunks (RBC chunk data).
        ConsensusChunk,
        /// Consensus payload plane (block sync updates and proposal payloads).
        ConsensusPayload,
        /// Consensus control plane (view changes, coordination).
        Control,
        /// Block synchronization stream.
        BlockSync,
        /// Transaction gossip stream (public dataspaces).
        TxGossip,
        /// Transaction gossip stream for restricted dataspaces.
        TxGossipRestricted,
        /// Peer discovery gossip stream.
        PeerGossip,
        /// Signed trust gossip stream.
        TrustGossip,
        /// Health and diagnostics.
        Health,
        /// Any other traffic not classified explicitly.
        Other,
    }

    impl Topic {
        /// Whether this topic may be delivered as best-effort traffic.
        ///
        /// Best-effort topics may use QUIC DATAGRAM when available; reliable topics
        /// always stay on streams.
        pub const fn is_best_effort(self) -> bool {
            matches!(
                self,
                Topic::TxGossip
                    | Topic::TxGossipRestricted
                    | Topic::PeerGossip
                    | Topic::TrustGossip
                    | Topic::Health
            )
        }
    }

    /// Classification hook for payload types to indicate their logical topic.
    ///
    /// By default, all messages are classified as `Topic::Other`. Crates that
    /// define concrete network payloads (e.g., `iroha_core::NetworkMessage`)
    /// should implement this trait to provide useful classification.
    pub trait ClassifyTopic {
        /// Return the logical topic of the message for scheduling.
        fn topic(&self) -> Topic {
            Topic::Other
        }
    }

    /// Current online network peers
    pub type OnlinePeers = HashSet<Peer>;

    /// Transport capabilities observed/advertised for a peer.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode)]
    pub struct PeerTransportCapabilities {
        /// Whether the peer advertises SCION-preferred transport support.
        pub scion_supported: bool,
    }

    /// Current transport capabilities for online peers keyed by peer id.
    pub type OnlinePeerCapabilities = HashMap<PeerId, PeerTransportCapabilities>;

    /// The message that is sent to `NetworkBase` to update p2p topology of the network.
    #[derive(Clone, Debug)]
    pub struct UpdateTopology(pub HashSet<PeerId>);

    /// The message that is sent to `NetworkBase` to update peers addresses of the network.
    #[derive(Clone, Debug)]
    pub struct UpdatePeers(pub Vec<(PeerId, SocketAddr)>);

    /// The message that updates transport capabilities for peers.
    #[derive(Clone, Debug)]
    pub struct UpdatePeerCapabilities(pub Vec<(PeerId, PeerTransportCapabilities)>);

    /// Update ACL configuration at runtime (hot reload).
    #[derive(Clone, Debug, Default)]
    pub struct UpdateAcl {
        /// When true, only peers whose public keys appear in `allow_keys` are permitted.
        pub allowlist_only: bool,
        /// Allowlist of peer public keys.
        pub allow_keys: Vec<iroha_crypto::PublicKey>,
        /// Denylist of peer public keys.
        pub deny_keys: Vec<iroha_crypto::PublicKey>,
        /// CIDR allowlist (IPv4/IPv6), e.g., "192.168.1.0/24", `2001:db8::/32`.
        pub allow_cidrs: Vec<String>,
        /// CIDR denylist (checked before throttles).
        pub deny_cidrs: Vec<String>,
    }

    /// Update trusted peer list for lightweight reputation tracking.
    #[derive(Clone, Debug, Default)]
    pub struct UpdateTrustedPeers(pub HashSet<PeerId>);

    /// Update `SoraNet` handshake runtime configuration.
    #[derive(Clone, Debug)]
    pub struct UpdateHandshake {
        /// New handshake parameters to install.
        pub handshake: ActualSoranetHandshake,
    }

    /// Update consensus handshake capabilities (mode tag/fingerprint/runtime caps).
    #[derive(Clone, Debug)]
    pub struct UpdateConsensusCaps {
        /// New consensus handshake capabilities to install.
        pub caps: crate::ConsensusHandshakeCaps,
        /// When true, disconnect currently connected peers to force re-handshake under the new caps.
        pub drop_existing_peers: bool,
    }

    /// The message to be sent to the other [`Peer`].
    #[derive(Clone, Debug)]
    pub struct Post<T> {
        /// Data to be sent
        pub data: T,
        /// Destination peer
        pub peer_id: PeerId,
        /// Delivery priority
        pub priority: Priority,
    }

    /// The message to be send to the all connected [`Peer`]s.
    #[derive(Clone, Debug)]
    pub struct Broadcast<T> {
        /// Data to be send
        pub data: T,
        /// Delivery priority
        pub priority: Priority,
    }

    /// Message send to network by other actors.
    pub(crate) enum NetworkMessage<T> {
        Post(Post<T>),
        Broadcast(Broadcast<T>),
    }
}

/// Reference as a means of communication with a [`Peer`]
struct RefPeer<T: Pload> {
    handle: PeerHandle<T>,
    conn_id: ConnectionId,
    p2p_addr: SocketAddr,
    /// Disambiguator serves purpose of resolving situation when both peers are tying to connect to each other at the same time.
    /// Usually in Iroha network only one peer is trying to connect to another peer, but if peer is misbehaving it could be useful.
    ///
    /// Consider timeline:
    ///
    /// ```text
    /// [peer1 outgoing connection with peer2 completes first (A)] -> [peer1 incoming connection with peer2 completes second (B)]
    ///
    /// [peer2 outgoing connection with peer1 completes first (B)] -> [peer2 incoming connection with peer1 completes second (A)]
    /// ```
    ///
    /// Because it's meaningless for peer to have more than one connection with the same peer, peer must have some way of selecting what connection to preserve.
    ///
    /// In this case native approach where new connections will replace old ones won't work because it will result in peers not being connect at all.
    ///
    /// To solve this situation disambiguator value is used.
    /// It's equal for both peers and when peer receive connection for peer already present in peers set it just select connection with higher value.
    disambiguator: u64,
    #[allow(dead_code)]
    relay_role: RelayRole,
    trust_gossip: bool,
}

#[derive(Clone, Copy, Debug)]
enum TrustDirection {
    Inbound,
    Outbound,
}

impl TrustDirection {
    fn as_label(self) -> &'static str {
        match self {
            Self::Inbound => "recv",
            Self::Outbound => "send",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredFlushOutcome {
    Flushed,
    PeerMissing,
    Backpressured(ConnectionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredPostResult {
    Sent,
    Full,
    Closed,
    CapabilitySkipped,
    StaleGeneration,
}

#[derive(Clone, Debug, Default)]
struct PeerReputation {
    trusted: bool,
    score: i32,
}

#[derive(Default)]
struct PeerReputationBook {
    inner: HashMap<PeerId, PeerReputation>,
}

impl PeerReputationBook {
    #[allow(dead_code)]
    fn trusted_peers(&self) -> Vec<PeerId> {
        self.inner
            .iter()
            .filter_map(|(peer, rep)| rep.trusted.then_some(peer.clone()))
            .collect()
    }

    fn set_trusted(&mut self, trusted: &HashSet<PeerId>) {
        for id in trusted {
            self.inner
                .entry(id.clone())
                .or_insert_with(PeerReputation::default)
                .trusted = true;
        }
        // Clear trust flag for peers not present in the new set.
        for (peer_id, rep) in &mut self.inner {
            rep.trusted = trusted.contains(peer_id);
            if !rep.trusted {
                rep.score = 0;
            }
        }
    }

    fn record_connected(&mut self, peer: &PeerId) {
        let rep = self
            .inner
            .entry(peer.clone())
            .or_insert_with(PeerReputation::default);
        rep.score = rep.score.saturating_add(1);
    }

    fn record_disconnected(&mut self, peer: &PeerId) {
        let rep = self
            .inner
            .entry(peer.clone())
            .or_insert_with(PeerReputation::default);
        rep.score = rep.score.saturating_sub(1);
    }

    fn is_trusted(&self, peer: &PeerId) -> bool {
        self.inner
            .get(peer)
            .map_or(false, |rep| rep.trusted || rep.score > 0)
    }

    fn score(&self, peer: &PeerId) -> i32 {
        self.inner.get(peer).map_or(0, |rep| rep.score)
    }

    #[allow(dead_code)]
    fn snapshot(&self) -> HashMap<PeerId, PeerReputation> {
        self.inner.clone()
    }
}
// Low-priority helpers were accidentally emitted outside of the impl, which
// causes free functions with a `self` parameter (invalid) and missing generics.
// Wrap them into the NetworkBase impl where they belong.
impl<T: Pload + message::ClassifyTopic, K: Kex, E: Enc> NetworkBase<T, K, E> {
    fn low_allow(&mut self, id: &PeerId) -> bool {
        let Some(rate) = self.low_rate_per_sec else {
            return true;
        };
        let burst = self.low_burst.unwrap_or_else(|| rate.max(1.0));
        self.low_buckets
            .entry(id.clone())
            .or_insert_with(|| TokenBucket::new(rate, burst))
            .allow()
    }

    #[allow(clippy::cast_precision_loss)]
    fn low_allow_bytes(&mut self, id: &PeerId, bytes: usize) -> bool {
        let Some(rate) = self.low_bytes_per_sec else {
            return true;
        };
        let burst = self.low_bytes_burst.unwrap_or_else(|| rate.max(1.0));
        self.low_bytes_buckets
            .entry(id.clone())
            .or_insert_with(|| TokenBucket::new(rate, burst))
            .allow_n(bytes as f64)
    }

    fn post_low(
        &mut self,
        Post {
            data,
            peer_id,
            priority,
        }: Post<T>,
    ) {
        iroha_logger::trace!(peer=%peer_id, "Post message (low)");
        let topic = data.topic();
        let origin = self.self_id.clone();
        let relay_ttl = self.relay_ttl;
        let frame_for = |target: RelayTarget| {
            RelayMessage::new(origin.clone(), target, relay_ttl, priority, data.clone())
        };
        let send_with_throttle = |this: &mut Self, target_id: &PeerId, frame: RelayMessage<T>| {
            if !this.low_allow(target_id) {
                iroha_logger::debug!(peer=%target_id, "Low-priority post throttled by token bucket");
                LOW_THROTTLED_POSTS.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            let size_bytes = norito::codec::Encode::encode(&frame).len();
            if !this.low_allow_bytes(target_id, size_bytes) {
                iroha_logger::debug!(peer=%target_id, size=size_bytes, "Low-priority post throttled by bytes token bucket");
                LOW_THROTTLED_POSTS.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            this.send_frame_to_peer(target_id, frame, topic)
        };

        if let Some(hub_id) = self.relay_route_for_unconnected_post_target(&peer_id) {
            let _ = send_with_throttle(self, &hub_id, frame_for(RelayTarget::Direct(peer_id)));
            return;
        }
        if send_with_throttle(
            self,
            &peer_id,
            frame_for(RelayTarget::Direct(peer_id.clone())),
        ) {
            return;
        }
        if matches!(
            self.relay_mode,
            iroha_config::parameters::actual::RelayMode::Spoke
                | iroha_config::parameters::actual::RelayMode::Assist
        ) {
            if let Some(hub_id) = self.hub_handle().map(|(id, _)| id.clone()) {
                let _ = send_with_throttle(self, &hub_id, frame_for(RelayTarget::Direct(peer_id)));
            } else {
                iroha_logger::warn!(
                    peer=%peer_id,
                    "Relay mode could not route low post because hub is unavailable"
                );
            }
        }
    }

    fn broadcast_low(&mut self, Broadcast { data, priority }: Broadcast<T>) {
        iroha_logger::trace!("Broadcast message (low)");
        let topic = data.topic();
        let peers: Vec<PeerId> = self.peers.keys().cloned().collect();
        for pid in peers {
            let frame = RelayMessage::new(
                self.self_id.clone(),
                RelayTarget::Broadcast,
                self.relay_ttl,
                priority,
                data.clone(),
            );
            let size_bytes = norito::codec::Encode::encode(&frame).len();
            if !self.low_allow(&pid) || !self.low_allow_bytes(&pid, size_bytes) {
                LOW_THROTTLED_BROADCASTS.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            self.send_frame_to_peer(&pid, frame, topic);
        }
    }
}
