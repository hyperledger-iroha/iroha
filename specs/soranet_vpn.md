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
- **Authenticated relay trust:** Enabling `network.soranet_vpn` requires an
  exact relay Ed25519 identity, a guard-directory snapshot path, and an
  independently provisioned exact snapshot digest. Torii authenticates those
  bytes once at startup, selects an exit-authorized VPN endpoint
  deterministically by priority and canonical multiaddr, and derives the TLS
  server name, leaf SPKI SHA-256, descriptor commitment, relay-certificate
  digest, and snapshot digest solely from that signed directory entry. There is
  no independent raw TLS-pin override. The first-release directory contract
  limits the encoded snapshot to 5 MiB, 16 issuers, 64 relays, 1,952 issuer
  ML-DSA-65 public-key bytes, and 64 KiB per embedded SRC bundle. Every local
  node, relay, and CLI consumer opens only a direct regular snapshot file
  without following its final path component, pins its identity through the
  read, reads at most the limit plus one byte, and decodes under an explicit
  Norito allocation budget. CLI HTTP fetches preflight `Content-Length` when
  present and stream at most the same limit plus one byte when the response is
  chunked or has no declared length. Complete-directory validation retains only
  the configured relay bundle, so certificates are not decoded into a second
  aggregate vector. Profile fields are empty and
  `available=false` when this trust cannot cover a complete configured lease;
  quote admission, the durable on-chain quote policy, relay helper-ticket
  admission, and restart reconstruction all reject trust that expires before
  the lease or differs from the currently authenticated entry.
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
- **Native XOR lease flow:** Torii now issues account-authenticated VPN quote
  responses before sessions.
  Each quote binds the account, exit class, relay, client metering public key,
  protocol-fixed XOR fee asset, deterministic per-lease custody account, and
  tariff, and returns exactly one required Norito-framed `OpenVpnLeaseEscrow`
  instruction as `open_lease_instruction`. Session
  creation only succeeds after the wallet submits that exact native lease-open
  transaction and provides the committed transaction hash. Native `vpn_leases`
  are the settlement source of truth: Torii process-local quote/session/receipt
  maps are live UX caches only. Active session lookups can reconstruct an
  unexpired active session from WSV after a Torii restart, and
  `/v1/vpn/receipts` rebuilds settlement context from WSV by lease id or relay
  receipt quote id within the on-chain grace window.
  The fee asset and custody account are consensus policy, not deployment
  profile inputs: Torii derives the canonical typed XOR definition and the
  custody account from the chain, lease id, and asset. Consequently neither
  value is configurable under `network.soranet_vpn` or advertised by the
  pre-quote profile; quote, session, and receipt records retain the resolved
  values needed to authorize and audit settlement.
- **Helper tickets:** Helper tickets are fixed 664-byte v1 frames. Each tariff
  component occupies a fixed slot containing a canonical exact `Quantity`
  frame, so no implicit integer nano-XOR unit crosses the helper boundary. The
  MAC covers the session, quote, account hash, relay id, payment hash,
  authorized Ed25519 metering public key, full deterministic tariff, and
  expiry. The client helper may structurally decode those fields to construct
  usage vouchers because it does not possess the relay MAC secret, but that
  path grants no authority. Relays always verify the MAC and expiry, reject
  old-length tickets, and reject vouchers signed by any key other than the
  ticket metering key. A successful redemption is committed synchronously to
  the namespace- and relay-identity-bound ledger configured by
  `vpn.helper_ticket_replay_store_path`; the relay rejects duplicates after a
  restart, never evicts an active redemption to admit another, and fails
  startup or admission closed on corruption, capacity exhaustion, lock
  contention, or persistence failure. The ledger retains exact millisecond
  expiry plus a durable monotonic wall-clock high-water mark, so a clock
  rollback cannot reopen a pruned ticket after restart, and rejects tickets
  whose remaining lifetime exceeds `vpn.lease_secs`.
- **SDK quote helpers:** JavaScript, C#, Swift, Python, Kotlin/JVM, and Java
  Android Torii clients expose quote-first VPN helpers plus typed
  `OpenVpnLeaseEscrow` / `SettleVpnLease` instruction DTOs. Callers should
  submit the returned native instructions as normal signed transactions; direct
  prepaid session creation is no longer the supported flow. Profile, quote, and
  session responses expose `lease_fee`, while receipts expose `lease_fee`,
  `earned_fee`, and `refunded_fee`; every fee is a canonical exact `Quantity`
  decimal string. JSON numbers and the retired integer `*_nanos` aliases are
  rejected rather than rounded or reinterpreted.
  Profile, quote, and session payloads carry the same required trust tuple:
  `relay_id_hex`, `descriptor_commit_hex`, `tls_server_name`,
  `relay_tls_spki_sha256_hex`, `relay_certificate_sha256_hex`, and
  `directory_snapshot_digest_hex`. Available quote/session values are exact
  lowercase 32-byte hex encodings plus the canonical DNS identity; clients
  must reject missing, null, malformed, or substituted fields.
- **Receipt/billing:** Exit gateways produce `VpnSessionReceiptV1` values
  and accept client-signed cumulative `VpnUsageVoucherV1` control cells. The
  relay verifies voucher/session/quote/relay binding, limits unvouched forwarding
  with `vpn.usage_voucher_debt_window_bytes`, and only mirrors the highest
  accepted voucher into settlement receipts. The earned fee is recomputed from
  the helper-ticket tariff; a client-supplied voucher envelope cannot raise or
  lower the settlement amount. Operator-submitted receipts return a Norito-framed
  optional `settle_lease_instruction` containing `SettleVpnLease` so earned XOR
  and refunds are split from native custody instead of trusting relay-supplied
  prepaid claims. Runtime
  operators can set `vpn.receipt_spool_dir` on the relay to persist the exact
  `/v1/vpn/receipts` request body (`relay_receipt_hex`, `client_voucher_hex`,
  and `lease_id_hex`) plus the audited top-level `earned_fee` as a canonical
  decimal string whenever a helper-authenticated session closes with an
  accepted voucher; sessions without a voucher intentionally do not produce a
  settlement artifact. `soranet-vpn-settlement` signs that artifact with
  runtime-only operator seed material and prints deterministic Torii headers/body
  or a ready `curl` command; do not edit the body after signing because Torii
  verifies the canonical body hash. Runtime counters still split data vs cover
  traffic for frames/bytes
  (`soranet_vpn_{data,cover}_{frames,bytes}_total`), where byte counters track
  payload bytes (derive on-wire bytes as `frames * 1024` when you need padding
  spend). Control/keepalive cell classes are tracked separately via
  `soranet_vpn_control_{frames,bytes}_total` and are excluded from VPN payload
  metrics and receipts.【tools/soranet-relay/src/runtime.rs:1984】【tools/soranet-relay/src/metrics.rs:744】【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **Privileged local backend:** Relay backend bridging is configured with
  `vpn.backend_endpoint`, not `vpn.backend_addr`. The default is a permissioned
  Unix socket (`unix:/tmp/sora-vpn-backend.sock`); TCP remains available as
  `tcp://host:port` only when both relay and backend configure
  `vpn.backend_bootstrap_secret_hex` / `SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_HEX`.
  Bootstrap frames are Norito envelopes with timestamp, nonce, and keyed MAC;
  the backend rejects bad MACs, stale timestamps, and replayed nonces, and Unix
  endpoints check peer credentials against the configured allowed uid/gid.
- **Local helper secrecy:** Hidden helper workers read their connect payloads
  from stdin instead of argv, and that stdin payload is a magic-prefixed Norito
  frame rather than JSON. The helper's private state file is also a
  magic-prefixed Norito frame; only the CLI status output remains JSON for local
  UX. Usage voucher signing derives the metering key and tariff from the helper
  ticket, and helper traffic counters are batched in memory with
  at-most-once-per-second state-file flushes plus a forced shutdown flush.
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
