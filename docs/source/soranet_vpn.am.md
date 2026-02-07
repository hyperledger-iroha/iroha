---
lang: am
direction: ltr
source: docs/source/soranet_vpn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b7e738790d81688d947760a65575f7f20e57539a93d77b37c9e84b0e41b224a
source_last_modified: "2026-01-05T09:28:12.096235+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet native VPN bridge

The native VPN bridge wraps IP traffic into fixed 1,024-byte SoraNet cells so
PacketTunnel clients, exit gateways, and the governance/billing surfaces share
the same deterministic framing.

- **Cell format:** `crates/iroha_data_model/src/soranet/vpn.rs` pins the header
  (version, class, flags, circuit id, flow label, seq/ack, padding budget,
  payload length) and exposes helpers for padding (`VpnCellV1::into_padded_frame`)
  and control-plane/billing payloads. Payload capacity is `1024 - 42 = 982`
  bytes; headers carry a padding budget in milliseconds.
- **Control plane:** `crates/iroha_config/src/parameters/{defaults,user,actual}.rs`
  adds `network.soranet_vpn.*` knobs (cell size, flow label width, cover ratio,
  burst, heartbeat, jitter, padding budget, guard refresh, lease, DNS push
  interval, exit class, meter family). The client API summary now exposes the
  same fields to SDKs.
- **Cover scheduling:** `xtask/src/soranet_vpn.rs` builds deterministic cover/data
  plans from the config using a BLAKE3 XOF seeded by all 32 seed bytes, clamps
  bursts, frames payloads with the configured padding budget, and emits billing
  receipts keyed by the exit class.
- **Cover ratio + seeding:** `cover_to_data_per_mille` accepts 0-1000; an explicit
  `0` disables cover even when `vpn.cover.enabled=true`, and burst caps insert
  data slots while resetting the cover streak. `VpnBridge` derives a per-circuit
  default cover seed from the circuit id + flow label (override with
  `set_cover_seed`).【crates/iroha_config/src/parameters/user.rs:6380】【tools/soranet-relay/src/config.rs:740】【crates/iroha_data_model/src/soranet/vpn.rs:509】【tools/soranet-relay/src/vpn_adapter.rs:224】
- **Flow-label enforcement:** `flow_label_bits` now clamps to 1–24 bits (default
  24) on config/client inputs. Frame builders validate the configured width and
  parsing helpers reject frames whose flow label exceeds the allowed width so
  runtimes cannot silently accept oversized labels.
- **Exit/lease validation:** Exit class labels are restricted to the
  `standard`/`low-latency`/`high-security` allowlist (hyphen/underscore
  variants accepted) and are canonicalised before they reach the wire; unknown
  labels now error in client/config parsing and xtask helpers. Control-plane
  leases must fit in `u32` seconds and are rejected early in config parsing,
  client summaries, and control-plane builders instead of being truncated.【crates/iroha_data_model/src/soranet/vpn.rs:548】【crates/iroha_config/src/parameters/user.rs:5529】【crates/iroha_config/src/client_api.rs:1549】【xtask/src/soranet_vpn.rs:42】
- **Client surface:** `IrohaSwift/Sources/IrohaSwift/SoranetVpnTunnel.swift`
  provides a PacketTunnel-friendly framer that pads to 1,024 bytes, enforces the
  header layout, and offers a small `NEPacketTunnelNetworkSettings` helper for
  DNS/route pushes. Unit tests (`IrohaSwift/Tests/IrohaSwiftTests/`) mirror the
  Rust layout.
- **Receipt/billing:** Exit gateways produce `VpnSessionReceiptV1` values
  (ingress/egress/cover bytes, uptime, exit class, meter hash) using the helper
  in `xtask/src/soranet_vpn.rs`, keeping Norito payloads consistent with the
  config defaults and governance meters. The relay runtime now emits receipts on
  circuit teardown with Prometheus counters (`soranet_vpn_session_receipts_total`,
  `soranet_vpn_receipt_*_bytes_total`) so billing/telemetry pipelines can scrape
  live usage alongside adapter/unit tests. Runtime counters now split data vs
  cover traffic for frames/bytes (`soranet_vpn_{data,cover}_{frames,bytes}_total`),
  where byte counters track payload bytes (derive on-wire bytes as
  `frames * 1024` when you need padding spend). Control/keepalive cell classes
  (for example, route-open control frames) are tracked separately via
  `soranet_vpn_control_{frames,bytes}_total` and are excluded from VPN payload
  metrics and receipts.【tools/soranet-relay/src/runtime.rs:1984】【tools/soranet-relay/src/metrics.rs:744】【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **End-to-end metrics harness:** The adapter suite now includes a paced
  bridge→adapter round-trip that pumps data and cover cells over a duplex link
  and asserts ingress/egress counters for cover/data frames and bytes on both
  ends. It also verifies payload delivery to the exit side, tightening the
  cover/data accounting promised in SNNet-18f7 without spinning the full relay
  runtime.【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **Frame I/O + padding enforcement:** Relay builders rewrite padding budgets
  from config, enforce the pinned 1,024-byte frame size and flag allowlist, and
  async read/write helpers drop truncated frames while counting ingress/egress
  bytes. Overlay/adapter tests guard zero padding, payload-length limits, and
  truncated stream rejection to keep framing deterministic.【tools/soranet-relay/src/vpn.rs:1】【tools/soranet-relay/tests/vpn_overlay.rs:1】【tools/soranet-relay/tests/vpn_adapter.rs:1】【xtask/src/soranet_vpn.rs:1】
- **Pacing + cover injection:** `schedule_frames` applies `pacing_millis` to
  interleave cover/data frames derived from the BLAKE3-seeded plan (burst/jitter
  caps) and `send_scheduled_frames` emits at the computed cadence with async
  helpers and regression tests asserting send-time spacing.【tools/soranet-relay/src/vpn.rs:303】【tools/soranet-relay/tests/vpn_runtime.rs:1】
- **Runtime guard & telemetry:** Frame I/O, pacing, and receipt emission now run
  in the relay runtime while exit-bridge/control-plane wiring proceeds. The
  Prometheus gauge `soranet_vpn_runtime_status{state="disabled|active|stubbed"}`
  (tagged with `vpn_session_meter`/`vpn_byte_meter` labels) plus receipt counters
  keep operators aware when VPN handling is active vs stubbed/disabled.【tools/soranet-relay/src/runtime.rs:1】【tools/soranet-relay/src/config.rs:1】【tools/soranet-relay/src/metrics.rs:1】

Use `network.soranet_vpn` to tune the heartbeat/cover budget for deployments and
`xtask/src/soranet_vpn.rs` to generate reproducible schedules and receipts for
acceptance evidence.
