---
lang: am
direction: ltr
source: docs/source/p2p_trust_gossip.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19912eba5930ce21483d018a6dd846fb38a4f2060013e5cdcf8dd1173a0a9aa7
source_last_modified: "2025-12-29T18:16:36.006735+00:00"
translation_last_reviewed: 2026-02-07
---

# Trust Gossip Capability

The trust-gossip plane carries signed trust hints about peers (scores, bad-gossip penalties,
unknown-peer penalties) and is intentionally separable from peer-address gossip. This note captures
the capability semantics, gating rules, and operational expectations for both public (NPoS) overlays
and permissioned deployments.

## Decision

We keep the unknown-peer penalty enforced (rather than removing the surface) so permissioned
deployments can reject off-topology trust spam without affecting peer-address gossip. Public (NPoS)
overlays remain penalty-free for unknown peers so trust exchange stays open. Operators should watch
`p2p_trust_penalties_total{reason="unknown_peer"}` and the `trust_min_score` floor for unexpected
drops; scores decay toward zero per `trust_decay_half_life_ms`, allowing recovery without manual
intervention.

## Capability model

- Two configuration flags participate in capability negotiation:
  - `network.trust_gossip` – local intent to send/receive trust gossip; defaults to `true`.
  - `network.soranet_handshake.trust_gossip` – handshake advertisement; defaults to `true`.
- The handshake ANDs the two flags from each side. Trust frames flow only when **both peers** set both
  flags to `true`. If either side disables the capability, trust gossip is skipped end-to-end while
  peer-address gossip remains unaffected.
- Topic classification is explicit: `NetworkMessage::PeerTrustGossip` maps to `Topic::TrustGossip`,
  and peer-address gossip stays on `Topic::PeerGossip`. Caps/backoffs/relay TTLs are unchanged and
  still shared with peer gossip.

## Send/receive behaviour

- **Send path**
  - Posts/broadcasts on `Topic::TrustGossip` are rejected immediately when the local capability is
    off, emitting `p2p_trust_gossip_skipped_total{direction="send",reason="local_capability_off"}`.
  - When the remote peer did not negotiate trust support, the frame is skipped before queueing,
    tagged with `reason="peer_capability_off"`, and logged at debug level. Peer-address gossip
    continues on the existing Low queue.
- **Receive path**
  - Trust frames are dropped before cap/size enforcement when the local node disabled the capability
    or when the connection was negotiated without trust support. Drops increment the same skip
    counter with `direction="recv"`.
  - Topic caps/backpressure stay intact: trust and peer gossip still share the Low queue and
    `max_frame_bytes_peer_gossip`, so capability gating cannot starve peer gossip.
- **Relay**
  - Relay TTL and forwarding rules mirror peer gossip. Hubs forward trust frames only for peers that
    negotiated trust support; otherwise frames are skipped with the same telemetry labels.

## Configuration checklist

- Public/NPoS overlays: leave both knobs enabled so trust scores propagate network-wide. The skip
  counter should remain at zero; investigate any increments as potential misconfigurations.
- Permissioned overlays: set `network.trust_gossip=false` to suppress trust exchange without touching
  peer-address gossip. Expect `p2p_trust_gossip_skipped_total` to tick up when peers attempt trust
  frames; this is expected and confirms the capability gate.
- Templates: `docs/source/references/peer.template.toml` now exposes `trust_gossip` under `[network]`
  alongside trust decay/penalties. The config reference documents both knobs and the skip counter.

## Telemetry and diagnostics

- Skip counter: `p2p_trust_gossip_skipped_total{direction,reason}` increments on both send/receive
  drops (`local_capability_off` and `peer_capability_off`). Use it to alarm on unexpected trust
  suppression in public mode and to verify capability gates in permissioned mode.
- Trust scoring metrics remain unchanged: `p2p_trust_score{peer_id}`,
  `p2p_trust_penalties_total{reason}`, and `p2p_trust_decay_ticks_total{peer_id}`.
- Logs: debug-level entries accompany capability drops; peer gossip continues, so drops should not
  correlate with address-gossip stalls.

## Non-goals

- No throttling changes: the capability does **not** introduce new rate limits or Low-queue caps.
  Peer-address gossip uses the existing `PeerGossip` topic and caps.
- No peer-address suppression: disabling trust gossip never affects address gossip or relay target
  selection. Mixed deployments (one trust-enabled, one disabled) still exchange addresses and stay
  connected.
- No ABI toggles: trust-gossip opcodes/syscalls remain shipped unconditionally; the capability only
  gates whether frames are sent/accepted on the wire.

## Test surface

- Unit: `trust_gossip_allowed` rejects trust frames when the capability is off.
- Integration: `crates/iroha_p2p/tests/integration/p2p_trust_gossip.rs` covers:
  - Trust disabled → trust frames dropped both directions while peer gossip still flows.
  - Trust enabled → trust frames delivered end-to-end under existing caps/fanout.

These behaviours are deterministic and hardware-independent; skips are counted and logged, not
retried.
