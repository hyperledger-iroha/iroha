---
lang: kk
direction: ltr
source: docs/source/p2p.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf09fce05f5cb039c5aa54fbe437c9f1c68d6e7edf04f18e3f6037036ff5a378
source_last_modified: "2026-02-03T18:28:41.133637+00:00"
translation_last_reviewed: 2026-02-07
---

## P2P Queues and Metrics

This section describes the peer-to-peer (P2P) queue capacities and the metrics exposed for monitoring.

### Queue Capacities ([network] settings)


- `p2p_queue_cap_high` (usize, default: 8192)
  - Capacity of the high-priority network message queue and inbound peer dispatch buffer
    (consensus/control messages).
- `p2p_queue_cap_low` (usize, default: 32768)
  - Capacity of the low-priority network message queue and inbound peer dispatch buffer
    (gossip/sync messages).
- `p2p_post_queue_cap` (usize, default: 2048)
  - Capacity of the per-peer post channel (outbound messages to a specific peer).
- `p2p_subscriber_queue_cap` (usize, default: 8192)
  - Capacity of each inbound subscriber queue feeding the node relay.

These defaults are tuned for blockchain workloads around 20,000 TPS: consensus/control traffic stays responsive, while gossip and synchronization get more headroom. Adjust these values based on your block size, block time, and network conditions.

Notes
- High-priority queues carry consensus/control traffic; low-priority queues handle gossip and sync paths.
- The relay registers separate high/low subscribers, so total relay buffering is `2 * p2p_subscriber_queue_cap` plus any additional subscribers (e.g., genesis bootstrap, Torii Connect).

### Low-Priority Rate Limiting ([network] settings)

- `low_priority_rate_per_sec` (optional; msgs/sec)
  - Enables per-peer token-bucket for Low-priority traffic (gossip/sync) on both ingress and egress. When unset, disabled.
- `low_priority_burst` (optional; msgs)
  - Bucket burst capacity; defaults to `low_priority_rate_per_sec` when unset.

When enabled, inbound Low-priority frames are dropped before relay dispatch (tx gossip, peer/trust gossip, health/time), and outbound Low-priority posts/broadcast deliveries are throttled per peer. Streaming control frames are also gated by the ingress limiter to prevent control-plane floods. High-priority consensus/control traffic is otherwise unaffected.

### DNS Hostname Refresh ([network] setting)

If your `P2P_PUBLIC_ADDRESS` is a hostname, you can optionally refresh connections on an interval to pick up IP changes:

- `dns_refresh_interval_ms` (optional; disabled if unset)
  - When set, the peer will periodically disconnect and re‑dial hostname‑based peers so the OS resolver can re‑resolve the host name.
  - Recommended values: 300000–600000 (5–10 minutes) depending on your DNS TTL and operational needs.

### P2P Telemetry Metrics

The following gauges are exposed via Prometheus when telemetry is enabled:

- `p2p_dropped_posts`: number of post messages dropped due to a full bounded queue (monotonic).
- `p2p_dropped_broadcasts`: number of broadcast messages dropped due to a full bounded queue (monotonic).
- `p2p_subscriber_queue_full_total`: number of inbound messages dropped because subscriber queues were full.
- `p2p_subscriber_queue_full_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: per-topic subscriber-queue drops.
- `p2p_subscriber_unrouted_total`: number of inbound messages dropped because no subscriber matches the topic.
- `p2p_subscriber_unrouted_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: per-topic unrouted inbound drops.
- `p2p_queue_depth{priority="High|Low"}`: bounded network actor queue depth by priority.
- `p2p_queue_dropped_total{priority="High|Low",kind="Post|Broadcast"}`: bounded network actor queue drops by priority/kind.
- `p2p_handshake_failures`: number of P2P handshake failures (timeouts, signature/verification errors).
- `soranet_pow_revocation_store_total{reason}`: count of SoraNet PoW revocation store fallbacks
  (e.g., load failures forcing an in-memory cache). Alert when this rises—replay protection relies
  on a healthy on-disk snapshot.
- `p2p_low_post_throttled_total`: number of Low-priority post messages throttled by per-peer token-buckets.
- `p2p_low_broadcast_throttled_total`: number of Low-priority broadcast deliveries throttled by per-peer token-buckets.
- `p2p_post_overflow_total`: number of per-peer post channel overflows (bounded per-topic channels).
- `p2p_dns_refresh_total`: number of DNS interval-based refresh cycles performed.
- `p2p_dns_ttl_refresh_total`: number of DNS TTL-based refresh cycles performed.
- `p2p_dns_resolution_fail_total`: number of DNS resolution/connection failures for hostname peers.
- `p2p_dns_reconnect_success_total`: number of reconnect successes after refresh cycles.
- `p2p_backoff_scheduled_total`: number of per-address connect backoffs scheduled.
- `p2p_accept_throttled_total`: number of incoming connections rejected by per-IP throttle.
- `p2p_accept_bucket_evictions_total`: number of accept bucket evictions (idle timeout or cap).
- `p2p_accept_buckets_current`: current number of active accept buckets (prefix + per-IP).
- `p2p_accept_prefix_cache_total{result="hit|miss"}`: prefix bucket cache utilisation.
- `p2p_accept_throttle_decisions_total{scope="prefix|ip",decision="allowed|throttled"}`: accept throttle outcomes split by prefix vs per-IP buckets.
- `p2p_incoming_cap_reject_total`: number of incoming connections rejected due to `max_incoming`.
- `p2p_total_cap_reject_total`: number of connections rejected due to `max_total_connections`.
- `p2p_ws_inbound_total`: accepted inbound WebSocket P2P connections.
- `p2p_ws_outbound_total`: successful outbound WebSocket P2P connections.

These metrics help identify saturation scenarios and networking issues. Drop counters remain at zero as long as queues keep up with traffic.

Example `/metrics` snippet (Prometheus):

```
# HELP p2p_dropped_posts Number of p2p post messages dropped due to backpressure
# TYPE p2p_dropped_posts gauge
p2p_dropped_posts 0

# HELP p2p_dropped_broadcasts Number of p2p broadcast messages dropped due to backpressure
# TYPE p2p_dropped_broadcasts gauge
p2p_dropped_broadcasts 12

# HELP p2p_subscriber_queue_full_total Number of inbound messages dropped because subscriber queues were full
# TYPE p2p_subscriber_queue_full_total gauge
p2p_subscriber_queue_full_total 3

# HELP p2p_subscriber_queue_full_by_topic_total Per-topic inbound drops caused by full subscriber queues
# TYPE p2p_subscriber_queue_full_by_topic_total gauge
p2p_subscriber_queue_full_by_topic_total{topic="Consensus"} 2

# HELP p2p_subscriber_unrouted_total Number of inbound messages dropped because no subscriber matches the topic
# TYPE p2p_subscriber_unrouted_total gauge
p2p_subscriber_unrouted_total 7

# HELP p2p_subscriber_unrouted_by_topic_total Per-topic inbound drops caused by no matching subscriber
# TYPE p2p_subscriber_unrouted_by_topic_total gauge
p2p_subscriber_unrouted_by_topic_total{topic="Consensus"} 1

# HELP p2p_handshake_failures Number of p2p handshake failures
# TYPE p2p_handshake_failures gauge
p2p_handshake_failures 1

# HELP p2p_dns_refresh_total Number of DNS interval-based refresh cycles performed
# TYPE p2p_dns_refresh_total gauge
p2p_dns_refresh_total 3

# HELP p2p_dns_ttl_refresh_total Number of DNS TTL-based refresh cycles performed
# TYPE p2p_dns_ttl_refresh_total gauge
p2p_dns_ttl_refresh_total 7

# HELP p2p_dns_resolution_fail_total Number of DNS resolution/connection failures for hostname peers
# TYPE p2p_dns_resolution_fail_total gauge
p2p_dns_resolution_fail_total 2

# HELP p2p_dns_reconnect_success_total Number of DNS reconnect successes after refresh cycles
# TYPE p2p_dns_reconnect_success_total gauge
p2p_dns_reconnect_success_total 5

# HELP p2p_backoff_scheduled_total Number of per-address connect backoffs scheduled
# TYPE p2p_backoff_scheduled_total gauge
p2p_backoff_scheduled_total 1

# HELP p2p_accept_throttled_total Number of inbound accepts rejected by per-IP throttle
# TYPE p2p_accept_throttled_total gauge
p2p_accept_throttled_total 0

# HELP p2p_accept_bucket_evictions_total Number of accept throttle bucket evictions (idle or cap)
# TYPE p2p_accept_bucket_evictions_total gauge
p2p_accept_bucket_evictions_total 0

# HELP p2p_accept_buckets_current Current number of active accept throttle buckets (prefix + per-IP)
# TYPE p2p_accept_buckets_current gauge
p2p_accept_buckets_current 0

# HELP p2p_accept_prefix_cache_total Prefix bucket cache hits/misses for accept throttle (label `result` = hit|miss)
# TYPE p2p_accept_prefix_cache_total gauge
p2p_accept_prefix_cache_total{result="hit"} 0
p2p_accept_prefix_cache_total{result="miss"} 4

# HELP p2p_incoming_cap_reject_total Number of inbound accepts rejected by incoming cap
# TYPE p2p_incoming_cap_reject_total gauge
p2p_incoming_cap_reject_total 0

# HELP p2p_total_cap_reject_total Number of connections rejected by total cap
# TYPE p2p_total_cap_reject_total gauge
p2p_total_cap_reject_total 0

# HELP p2p_ws_inbound_total Accepted inbound WebSocket P2P connections
# TYPE p2p_ws_inbound_total gauge
p2p_ws_inbound_total 0

# HELP p2p_ws_outbound_total Successful outbound WebSocket P2P connections
# TYPE p2p_ws_outbound_total gauge
p2p_ws_outbound_total 0
```

### Per-Topic Scheduling

- Outbound traffic is separated into logical topics to avoid head-of-line blocking and to prioritize consensus/control messages:
  - High: `Consensus`, `Control` (biased priority)
  - Low: `BlockSync`, `TxGossip`, `PeerGossip`, `Health`, `Other` (fair scheduling)
- Message payload types implement `iroha_p2p::network::message::ClassifyTopic` to supply their topic. `iroha_core::NetworkMessage` provides the mapping for core messages.
- A small fairness budget guarantees Low topics make progress during sustained High traffic.

### Proxy Support (HTTP CONNECT)

- The p2p dialer can use system proxy settings for outbound connections (HTTP CONNECT tunneling). No Iroha-specific environment toggles are required or recommended for operators.
- When a proxy is configured at the system level and the target host is not exempted, the connector tunnels via HTTP `CONNECT host:port`.
- Notes:
  - Basic authentication is not yet supported; prefer IP-allowlisted proxies.
  - Exemptions are matched as simple host suffixes (e.g., `.example.com`, `localhost`).
  - If no system proxy is configured, connections go direct.

### Dialing Strategy (Happy Eyeballs)

- Iroha dials multiple addresses per peer (e.g., hostname, IPv6, IPv4) in parallel with a small stagger so a reachable path wins quickly without waiting for slower/unreachable paths.
- Address preference (default): hostname first, then IPv6, then IPv4.
- Stagger interval: configurable via `[network]` as `happy_eyeballs_stagger_ms` (default 100ms). Increase if you have very large peer lists and want to further reduce burst dials; decrease for faster failover on slow networks.
- Per-address backoff: failed attempts back off independently per address with exponential jitter (up to 5s), avoiding stampedes.

### Example: `[network]` TOML and Feature Flags

Minimal `config.toml` snippet (values shown are defaults tuned for ~20k TPS):

```toml
[network]
# Internal bind address and advertised address
P2P_ADDRESS = "0.0.0.0:1337"
P2P_PUBLIC_ADDRESS = "peer1.example.com:1337"

# Bounded queue capacities
p2p_queue_cap_high = 8192     # consensus/control
p2p_queue_cap_low  = 32768    # gossip/sync
p2p_post_queue_cap = 2048     # per-peer post channel
p2p_subscriber_queue_cap = 8192  # inbound relay subscriber queue

# Other networking parameters (for reference)
block_gossip_size = 4        # fanout cap for block-sync gossip (peer samples, block sync updates, availability votes)
block_gossip_period_ms = 10000
block_gossip_max_period_ms = 30000
peer_gossip_period_ms = 1000
peer_gossip_max_period_ms = 30000
transaction_gossip_size = 500
transaction_gossip_period_ms = 1000
transaction_gossip_resend_ticks = 3
idle_timeout_ms = 60000
connect_startup_delay_ms = 0
# Trust decay/penalties for gossip senders (decays toward 0)
trust_decay_half_life_ms = 300000  # halve negative scores every 5 minutes
trust_penalty_bad_gossip = 5       # penalty applied per invalid trust gossip
trust_penalty_unknown_peer = 3     # penalty applied when gossip references peers outside topology
trust_min_score = -20              # drop trust gossip at or below this score
```

- Gossip/idle intervals are clamped to >=100ms to prevent zero-duration spin loops.
- Peer-address gossip is change-driven with exponential backoff up to `peer_gossip_max_period_ms`
  (and is throttled when the relay drops inbound frames); block-sync sampling similarly backs off
  up to `block_gossip_max_period_ms` when no progress is observed.
- Transaction gossip pauses when relay backpressure is active (recent subscriber-queue drops) and
  resumes after `transaction_gossip_period_ms * transaction_gossip_resend_ticks` to avoid
  flooding under load.
- `connect_startup_delay_ms` delays outbound dials immediately after startup to reduce
  connection-refused noise when peers come up in waves (localnet or orchestrated rollouts).
- NEW_VIEW votes are sent to the deterministic collector set for the current view and always
  include the leader; if the collector set is empty or local-only, the sender falls back to the
  full commit topology. This provides redundancy without relying on full broadcast in the steady state.
- Trust scoring is deterministic: scores start at 0, penalties subtract `trust_penalty_bad_gossip`,
  and the debt halves every `trust_decay_half_life_ms` until it reaches 0. In permissioned mode,
  trust gossip that advertises peers outside the current topology applies `trust_penalty_unknown_peer`;
  public (NPoS) mode accepts those reports without penalty. Gossip from peers at or below
  `trust_min_score` is ignored until decay lifts them above the floor. Metrics:
  `p2p_trust_score{peer_id}`, `p2p_trust_penalties_total{reason}`, and
  `p2p_trust_decay_ticks_total{peer_id}`.
- Unknown-peer trust gossip enforcement (permissioned mode): off-topology trust reports trigger a
  warning and increment `p2p_trust_penalties_total{reason="unknown_peer"}`. Scores decay using
  `trust_decay_half_life_ms`; once a sender climbs above `trust_min_score` it is reinstated and
  trust gossip resumes. Public/NPoS overlays skip the penalty so dial sets remain open.
- Trusted peers configured locally remain in the P2P topology even if they are not in the
  world-state topology (e.g., observers). They still receive gossip and block sync but do not
  change the consensus roster.
- Trust gossip capability: peers advertise `trust_gossip` during the handshake. When a peer sets
  `trust_gossip=false`, it will neither send nor accept trust gossip frames, but regular peer-address
  gossip continues unaffected. The default is `true`, and public (NPoS) deployments should leave it on
  so trust scores propagate network-wide.

#### Trust gossip capability and gating

The trust gossip plane is intentionally separable from peer-address gossip so permissioned networks
can opt out without starving connectivity updates, while public (NPoS) overlays keep trust exchange
wide open. The handshake advertises two booleans that must both be true for trust frames to flow:

- `network.trust_gossip` — local capability knob; defaults to `true`.
- `network.soranet_handshake.trust_gossip` — handshake advertisement; defaults to `true`.

Both flags are AND-ed during the handshake; the resulting capability is stored on the peer handle and
checked on every send/receive. Behaviour matrix:

- **Send:** `NetworkMessage::PeerTrustGossip` is classified into the `TrustGossip` topic. If either
  side disabled the capability, the frame is skipped before queueing, tagged with
  `p2p_trust_gossip_skipped_total{direction="send",reason="local_capability_off|peer_capability_off"}`,
  and a debug log explains why. Peer-address gossip remains on `PeerGossip` and is unaffected by the
  trust toggle.
- **Receive:** trust frames are dropped early when the local node disabled the capability or when the
  connected peer did not negotiate trust support. Drops increment the same skip counter with
  `direction="recv"` labels. Topic caps/backpressure stay unchanged because trust and peer gossip
  still share the existing Low queue and per-topic caps.
- **Relay:** hubs only forward trust frames for peers that negotiated trust support; `relay_ttl` and
  throttles are the same as peer gossip.

Operator guidance:

- Public/NPoS deployments should leave both knobs enabled so trust scores propagate across the overlay
  and observers can scrape `p2p_trust_score`/`p2p_trust_penalties_total`. The skip counter should stay
  at zero in this mode.
- Permissioned/air-gapped overlays can set `network.trust_gossip=false` to suppress trust exchange
  without affecting peer-address gossip or relay caps. Expect `p2p_trust_gossip_skipped_total` to tick
  up when trust frames are attempted; this is expected and indicates the capability gate is working.
- Topic classification and Low-queue throttles are unchanged by the capability: peer-address gossip
  still uses the `PeerGossip` topic and the same caps/backoffs, so disabling trust gossip does not
  starve peer updates.

Optional handshake hardening (chain-bound signatures):

```bash
# Build with chain-id included in handshake signatures
cargo build --workspace -F iroha_p2p/handshake_chain_id
```

Notes
- Enable `handshake_chain_id` when you want inbound and outbound peers to bind the signed handshake to a specific chain.

### ACL: Allow/Deny (Keys and CIDRs)

- Keys:
  - `allowlist_only` (bool, default false): when true, only peers whose public keys are listed in `allow_keys` are permitted (outbound dialing and inbound post‑handshake).
  - `allow_keys`: array of peer public keys.
  - `deny_keys`: array of peer public keys to always reject.
- Networks:
  - `allow_cidrs`: list of IPv4/IPv6 CIDRs that are permitted for inbound IPs (e.g., `192.168.1.0/24`, `2001:db8::/32`). Effective when `allowlist_only` is true.
  - `deny_cidrs`: list of IPv4/IPv6 CIDRs rejected for inbound IPs (checked before throttles).
- Precedence: `deny_*` takes precedence. CIDR checks apply before per‑IP throttling; key checks apply after handshake (and are also applied to topology for outbound).

### Accept throttle (prefix + per-IP)

- Knobs (`[network]`):
  - `accept_rate_per_prefix_per_sec` / `accept_burst_per_prefix` *(optional)*: prefix-level token bucket (default disabled). Applied before per-IP buckets.
  - `accept_prefix_v4_bits` / `accept_prefix_v6_bits` *(u8; defaults: 24 / 64)*: prefix width used for the prefix bucket key.
  - `accept_rate_per_ip_per_sec` / `accept_burst_per_ip` *(optional)*: per-IP token bucket keyed by full address (/32, /128). Disabled when unset.
  - `max_accept_buckets` *(usize, default: 4096)*: combined cap for active prefix + per-IP buckets (LRU eviction when exceeded).
  - `accept_bucket_idle_ms` *(default: 600000 / 10 minutes)*: idle timeout before buckets are evicted.
- Behaviour:
  - CIDR allowlists still gate access first; allowlisted IPs bypass both prefix and per-IP buckets.
  - Prefix bucket (when enabled) runs before per-IP buckets; a throttled prefix stops evaluation early.
  - Buckets prune idle entries on every evaluation and evict the least-recently-used entry when above `max_accept_buckets`.
- Telemetry:
  - `p2p_accept_buckets_current` gauges active bucket count; `p2p_accept_bucket_evictions_total` tracks idle/LRU evictions.
  - `p2p_accept_prefix_cache_total{result}` surfaces prefix cache hit/miss ratios.
  - `p2p_accept_throttle_decisions_total{scope,decision}` splits allow/throttle outcomes across prefix vs per-IP buckets; `p2p_accept_throttled_total` remains the aggregate throttle counter.

### QUIC Transport (experimental)

- Build-time: enable `iroha_p2p/quic` to include QUIC support.
- Runtime: set `[network].quic_enabled = true` to turn on the QUIC listener and allow outbound try‑QUIC dials. When a QUIC attempt fails, the dialer falls back to TCP automatically.
- Current status: inbound QUIC listener is implemented and spawns peers for accepted bidirectional streams. Outbound dialing can attempt QUIC to hostnames (with TCP fallback).

### TLS-over-TCP (camouflage)

- Build-time: enable `iroha_p2p/p2p_tls` to include TLS support.
- Runtime: set `[network].tls_enabled = true` to wrap outbound P2P connections in TLS 1.3 using rustls. Identity remains authenticated at the application layer by the signed handshake (address + optional `chain_id`).
- Behavior: the dialer connects to `host:port` over TCP and upgrades to TLS; if TLS fails, it falls back to plain TCP. This helps traversing L4 TLS proxies/LBs and makes traffic resemble HTTPS.
- Inbound: optionally enable a TLS listener on a separate address/port via `[network].tls_listen_address`. When set (and TLS is enabled), the node accepts inbound TLS connections on that address while still accepting plain TCP on `[network].address`. Certificates are self‑signed per process; authentication is enforced at the application handshake.

### Noise XX handshake (feature-gated)

- Build-time: enable `iroha_p2p/noise_handshake` to derive the session key from a Noise XX exchange.
- Behavior: peers still perform the SoraNet handshake (PoW + capability negotiation). After that, they run a Noise XX handshake over the same framing and derive a 32-byte session key from the handshake hash for message encryption.

### WebSocket Fallback (p2p_ws)

- `iroha_p2p::NetworkHandle::accept_stream(read, write, remote_addr)` allows accepting externally provided duplex streams and spawning a peer, applying the same caps/throttle as TCP accepts.
- Intended use: Torii `/p2p` WebSocket route upgrades to a raw duplex and forwards its halves to `accept_stream` (feature `p2p_ws`).
- Outbound fallback: the dialer attempts QUIC/TLS/TCP; if that fails it can fall back to WS/WSS (`ws://host:port/p2p` and `wss://host:port/p2p`).
- Preference knob: set `[network].prefer_ws_fallback = true` to try WS/WSS first for any peer address (useful for constrained environments and CI).
- Status: server-side route and outbound fallback implemented behind `p2p_ws`. An end‑to‑end test exercises the Torii `/p2p` route.

### Pre‑Handshake Header (defense‑in‑depth)

- Before the cryptographic handshake begins, peers exchange a 5‑byte preface (`"I2P2"` + version byte).
  This allows early rejection of garbage before incurring expensive crypto. Outbound writes then reads;
  inbound reads then writes, preventing deadlock.
  - Hello frames carry identity, consensus caps, and confidential caps (enabled/assume_valid/backend plus the `ConfidentialFeatureDigest`
  containing `vk_set_hash`, `poseidon_params_id`, `pedersen_params_id`, and `conf_rules_version`). The encrypted payload is length-prefixed with a `u16`,
  so metadata larger than `65_535` bytes is rejected with a deterministic
  `HandshakeMessageTooLarge` error rather than panicking.

#### Confidential Capability Outcomes

Validator nodes require `confidential.enabled=true`, `assume_valid=false`, the expected verifier backend, and a digest that matches their local registries. Outcomes mirror the matrix in [`confidential_assets.md`](confidential_assets.md#node-capability-negotiation):

| Remote advertisement | Result (validator role) | Operator action |
|----------------------|-------------------------|-----------------|
| `enabled=true`, `assume_valid=false`, backend matches, digest matches | Accepted | Peer enters rotation; no action needed. |
| `enabled=true`, `assume_valid=false`, backend matches, digest stale/missing | Rejected (`HandshakeConfidentialMismatch`) | Apply pending registry/parameter activations or wait for the scheduled `activation_height`. |
| `enabled=true`, `assume_valid=true` | Rejected (`HandshakeConfidentialMismatch`) | Configure the node as an observer or disable `assume_valid`. |
| `enabled=false`, missing fields, or backend differs | Rejected (`HandshakeConfidentialMismatch`) | Upgrade the peer and align backend + digest before reconnecting. |

Observers that intentionally skip verification (`assume_valid=true`) must avoid consensus connections; they can still ingest blocks via Torii/Web APIs but validators drop their P2P handshakes until capabilities match.

### Per-Peer Post Overflow Policy

- Knob: `[network].disconnect_on_post_overflow` (bool, default true)
  - When true (default), if a per-topic bounded post channel overflows for a peer, the connection is dropped.
  - When false, overflowed messages are dropped but the connection stays up. The counter `p2p_post_overflow_total` increments in both cases.

Per-topic metrics (when telemetry enabled):

- `p2p_post_overflow_total{priority="High|Low",topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`
- `p2p_subscriber_queue_full_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`
- `p2p_subscriber_unrouted_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`

Behavior matrix (bounded queues enabled):

| Setting                          | Effect on overflow                  | Metric updates                         |
|----------------------------------|-------------------------------------|----------------------------------------|
| disconnect_on_post_overflow=true | Disconnect peer; drop pending posts | `p2p_post_overflow_total{topic=..}↑`   |
| disconnect_on_post_overflow=false| Keep connection; drop overflowed    | `p2p_post_overflow_total{topic=..}↑`   |

Because queues are always bounded, overflow counters rise whenever a channel drops messages. Use `disconnect_on_post_overflow` to choose whether to drop the connection or just the overflowing messages.

### Frame Size Caps

- Global cap: `[network].max_frame_bytes` (default 16 MiB) rejects oversized frames early.
  The limit now applies uniformly to TCP, TLS, QUIC, and Torii `/p2p` WebSocket
  accepts as well as outbound dialers, with `p2p_post_overflow_by_topic`
  counters incremented whenever an inbound frame is dropped by the topic caps.
  This cap is enforced on encrypted frames, so AEAD overhead (nonce + tag) counts
  toward the limit (currently 28 bytes for ChaCha20-Poly1305).
- Topic caps (post-decode enforcement, tightened defaults) apply to decrypted payload sizes:
  - `[network].max_frame_bytes_consensus` (default 16 MiB; caps critical consensus frames such as `BlockCreated`, `FetchPendingBlock`, and `RbcInit`/`RbcReady`/`RbcDeliver`)
  - `[network].max_frame_bytes_control` (default 128 KiB)
  - `[network].max_frame_bytes_block_sync` (default = global cap; caps bulk consensus payloads such as `BlockSyncUpdate`, `Proposal`, and `RbcChunk`, plus BlockSync responses)
  - `[network].max_frame_bytes_tx_gossip` (default 256 KiB)
  - `[network].max_frame_bytes_peer_gossip` (default 64 KiB)
  - `[network].max_frame_bytes_health` (default 32 KiB)
  - `[network].max_frame_bytes_other` (default 128 KiB)

Recommended:
- Keep the global cap at or above the largest expected BlockSync frame.
- Tighten gossip/health caps to minimize attack surface.

### TCP Defaults

- `[network].tcp_nodelay = true` by default to minimize consensus latency.
- `[network].tcp_keepalive_ms = 60000` by default to keep long-lived connections healthy.

### QUIC/TLS Tuning

- TLS: default restricts to TLS 1.3 only when `[network].tls_only_v1_3 = true`.
- QUIC: configure idle timeout via `[network].quic_max_idle_timeout_ms`.
