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
  interval, exit class, meter family). Enabling the surface also requires
  exactly one dedicated Ed25519 operator-key source
  (`operator_private_key` or the owner-private `operator_private_key_file`).
  The derived public key must exactly match the configured single-key
  `operator_account_id`. The client API summary exposes only public policy and
  never the signer or private-key source.
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
  transaction and provides its committed entrypoint hash. Direct and
  sealed-reveal entrypoints are only lookup handles: Torii derives the inner
  signed transaction hash, requires it to equal the consensus lease's
  `open_tx_hash`, and carries only that canonical payment identity into helper
  tickets and receipts. Native `vpn_leases`
  are the settlement source of truth: Torii process-local quote/session/receipt
  maps are live UX caches only. Active session lookups can reconstruct an
  unexpired active session from WSV after a Torii restart, and
  `/v1/vpn/receipts` rebuilds settlement context from WSV by lease id or relay
  receipt quote id within the on-chain grace window.
  V1 intentionally has no Torii or SDK session-deletion operation: removing a
  process-local cache entry cannot revoke an already issued relay ticket or an
  active on-chain lease. Clients disconnect the authenticated local helper;
  the relay then emits the usage artifact and the lease reaches its terminal
  state only through authenticated settlement, expiry, or refund.
  The fee asset and custody account are consensus policy, not deployment
  profile inputs: Torii derives the canonical typed XOR definition and the
  custody account from the chain, lease id, and asset. Consequently neither
  value is configurable under `network.soranet_vpn` or advertised by the
  pre-quote profile; quote, session, and receipt records retain the resolved
  values needed to authorize and audit settlement.
  Consensus preflights every signed quote before signature verification allocates
  its signing buffer: endpoints and TLS names use the relay-certificate canonical
  grammar, route and exclusion entries are canonical IP network CIDRs with cleared
  host bits and decimal prefixes, and DNS resolvers are canonical unicast IP
  literals (not unspecified, multicast, or IPv4 limited broadcast). Duplicate
  normalized CIDRs and the exact same CIDR in both route lists are rejected;
  a more-specific exclusion beneath a pushed default route remains intentional
  and valid. All string/list/signature bounds precede allocation-free encoded-size
  counting, and the same static validator runs during WSV reconstruction.
- **Helper tickets:** Helper tickets are fixed 788-byte v1 capabilities. Each tariff
  component occupies a fixed slot containing a canonical exact `Quantity`
  frame, so no implicit integer nano-XOR unit crosses the helper boundary.
  Torii signs every capability with the dedicated Ed25519 VPN operator key
  configured under `network.soranet_vpn`, which is also the sole signer for
  canonical VPN quotes. Torii's common node and proxy-bridge keys never issue
  either artifact, and a separate shared helper secret does not exist. The
  relay pins the corresponding public key through
  `vpn.helper_ticket_issuer_public_key_path`, whose target is read once as a
  single-link, owner-private, canonical 64-character lowercase-hex file.
  Operators provision the same public key at the helper's fixed trust-anchor
  path described below. The signature covers the session, quote, consensus
  lease id, account hash,
  relay id, payment hash,
  authorized Ed25519 metering public key, full deterministic tariff, the exact
  session-derived client IPv4 and IPv6 addresses, and the domain-separated
  canonical hash of the selected relay endpoint, identity,
  descriptor commitment, TLS name and SPKI pin, relay certificate and directory
  digests, padding budget, ordered route pushes, excluded routes, DNS servers,
  the exact derived tunnel-address plan, and the fixed 1,280-byte V1 MTU.
  The client helper and relay independently verify the signature and the
  half-open validity window. The helper also
  recomputes the network-policy hash and rejects any mismatch before stopping
  an existing worker, changing durable state, connecting to a relay, or
  mutating host networking. During QUIC admission it compares the live leaf
  certificate's SPKI SHA-256 with the signed pin before normal name, validity,
  and signature verification. `relay_certificate_sha256` identifies the
  canonical signed relay-certificate bundle and is deliberately not
  reinterpreted as a leaf-DER hash. The relay-authenticated SoraNet handshake
  binds that bundle digest, the rest of the exact transport trust tuple, and
  the ticket before the helper prepares the tunnel, so a bundle mismatch fails
  post-TLS authentication. Relays reject old-length tickets and vouchers signed by any key other
  than the ticket metering key. A successful redemption is committed synchronously to
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
  `directory_snapshot_digest_hex`. Trust keys and digests, quote ids, lease
  ids, and payment hashes use exact lowercase 32-byte hex encodings. The
  canonical VPN session id is 16 bytes (32 lowercase hexadecimal characters),
  matching `VpnHelperTicketV1` and the consensus lease record. Clients must
  reject missing, null, malformed, or substituted fields.
- **Receipt/billing:** Exit gateways produce `VpnSignedSessionReceiptV1`
  envelopes around `VpnSessionReceiptV1` values and accept client-signed
  cumulative prepaid `VpnUsageVoucherV1` control cells. The relay envelope is
  mandatory, signs every receipt field under a dedicated V1 domain, and uses
  the exact Ed25519 public key bytes in `relay_id`; wrong-key, malformed,
  unsigned, and algorithm-substituted receipts are rejected. Receipt hashes
  commit to the complete signed envelope rather than the unsigned body. The
  V1 voucher signature is always Ed25519 over a domain-separated, fixed-width,
  big-endian body; bare Norito-body signatures and algorithm substitution are
  rejected. VPN mutation requests have a protocol-specific 16 KiB HTTP limit,
  and Torii separately bounds the decoded signed-receipt and voucher fields
  before allocating their binary forms. Voucher, receipt, tariff-meter, and
  account commitments use distinct BLAKE3 domains. The
  relay verifies voucher/session/quote/relay binding, signed issuance time,
  cumulative user-IP-payload ceilings, active-time ceilings, and the tariff-derived
  `fee_ceiling`, which must not exceed the escrowed helper-ticket lease fee. The
  exact escrow boundary is accepted and any larger envelope is rejected before
  voucher/WAL/service state advances. The first valid voucher is mandatory before backend connection
  or service; only after that metering-key proof does the relay durably redeem the
  one-time helper ticket. `vpn.usage_voucher_setup_timeout_ms` bounds this
  pre-service wait. `vpn.usage_voucher_credit_window_bytes` caps each direction's
  authorized lead over observed payload (256 KiB through 16 MiB; default 1 MiB), and
  `vpn.usage_voucher_max_age_ms` caps both voucher age and active-time lead.
  The helper grants 256 KiB per direction and two seconds of active credit,
  refreshes every second, and requests an earlier refresh before its remaining
  byte credit falls below half a window. The relay reconstructs complete packet
  frames, verifies the packet and current monotonic time against the highest
  signed ceilings, and only then forwards it; length prefixes are never billed.
  The helper records relay-to-client bytes only after Linux accepts the complete
  packet in one TUN write; a short write closes the tunnel without billing the
  unwritten packet.
  Only the highest accepted voucher enters settlement receipts. Receipt counters
  and service time are the relay-observed values and must be no greater than the
  voucher ceilings. Service duration is measured on the monotonic clock and
  projected as `ended_at_ms = started_at_ms + elapsed_ms`, so a wall-clock
  rollback cannot erase billable active time. Before durable helper-ticket
  redemption, the relay fsyncs an owner-private, non-submit-ready per-session
  WAL containing a zero-usage reservation. After backend readiness, it
  atomically advances that WAL to the bounded prepaid byte/time ceilings before
  forwarding any packet; every refresh is persisted before its credit becomes
  usable. A graceful close replaces the reservation with actual observed usage.
  After process or host failure, the next exclusively locked relay instance
  promotes the last signed reservation instead, so lost volatile counters can
  never create free service; the maximum crash charge is the client's small,
  explicitly signed prepaid window. An accepted initial voucher still produces
  a zero-usage settlement artifact if backend setup fails.
  `meter_hash` is the domain-separated hash of the signed tariff and
  `cover_bytes` is zero because cover accounting is relay-local telemetry, not
  authenticated consensus evidence. The earned fee is recomputed from actual
  receipt usage under the helper-ticket tariff after consensus verifies all
  three prepaid ceilings, so neither the voucher envelope nor an over-ceiling
  relay receipt can raise settlement. Operator-submitted receipts
  return `status = settlement_pending` and a Norito-framed
  `settle_lease_instruction` containing `SettleVpnLease`; they do not remove the
  active session or publish a settled receipt before that instruction commits.
  Only the committed WSV lease projection reports `status = settled`, so earned
  XOR and refunds are split from native custody instead of trusting provisional
  relay state. Each settled lease retains the complete signed relay envelope
  and complete signed client voucher, alongside their hashes, so snapshot
  validation and auditors can independently reauthenticate both parties and
  recompute every deterministic tariff invariant. Active and refunded leases
  retain neither terminal artifact. Runtime
  enabled VPN relays must set `vpn.receipt_spool_dir` to an existing,
  canonical, absolute directory owned by the relay user with mode `0700`. The
  relay takes a process-lifetime exclusive spool lock, recovers orphan WALs
  before admitting service, revalidates custody at every transition, and uses
  owner-owned, single-link files with mode `0600`. Every create, replacement,
  promotion, or removal fsyncs both the file and directory; any failure poisons
  VPN persistence and closes service. Live WAL JSON is deliberately a distinct
  schema that `soranet-vpn-settlement` cannot submit. Only final promotion emits
  the exact
  `/v1/vpn/receipts` request body (`relay_receipt_hex`, `client_voucher_hex`,
  and `lease_id_hex`) plus the audited top-level `earned_fee` as a canonical
  decimal string whenever a helper-authenticated session closes with an
  accepted voucher. A session that fails to provide fresh vouchers is closed
  rather than retained without a settlement artifact.
  `soranet-vpn-settlement` signs that artifact with
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
  Unix socket (`unix:/run/sora-vpn-backend.sock`), and V1 accepts only
  `unix:/absolute/path` endpoints. An enabled relay must explicitly set
  `vpn.backend_expected_uid` and `vpn.backend_expected_gid` to the backend's
  effective identity. The relay canonicalizes and validates the socket parent
  chain, and on every connection requires a direct, single-link socket owned by
  that exact UID/GID with no permissions for other users. It pins the socket
  device/inode across connect and verifies the connected process credentials
  immediately, before sending any bootstrap byte. Filesystem custody and peer
  credentials therefore keep unprivileged local processes outside both sides
  of the backend admission boundary. Every
  session also requires both relay and backend to configure owner-private direct files selected by `vpn.backend_bootstrap_secret_path` /
  `SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_PATH`.
  Each file contains exactly 64 lowercase hexadecimal characters, with no
  whitespace and no all-zero placeholder.
  Bootstrap frames are Norito envelopes with timestamp, nonce, and keyed MAC;
  the backend rejects bad MACs, stale timestamps, and replayed nonces. Replay
  reservations and the accepted-time high-water mark are fsync-backed in the
  private backend replay directory before admission, preserving that guarantee
  across restarts and failing closed on clock rollback. Endpoints check peer
  credentials against the configured allowed uid/gid;
  peer identity does not replace the keyed bootstrap authentication.
  After validating the client's initial prepaid voucher, the relay authenticates
  and reserves the backend connection before durably redeeming the one-use
  helper ticket. It sends no backend protocol bytes until the settlement WAL
  and replay reservation commit, so a missing or spoofed local endpoint cannot
  burn an otherwise retryable paid session.
  The signed bootstrap carries each client's exact IPv4 and IPv6 assignment.
  Before crossing the TUN boundary the backend rejects malformed lengths,
  fragments, wrong client sources, and wrong client destinations. For each
  enabled forwarding family it resolves and pins one egress interface, permits
  only exact-client conntrack flows to that interface and established/related
  return traffic from it, and drops every other TUN FORWARD path. V1 is
  intentionally a public-Internet exit: userspace validation and independent
  kernel rules both reject loopback, unspecified, link-local, private/ULA,
  shared, benchmarking, documentation/test, multicast, reserved, IPv4-mapped,
  and local translation destinations before the egress allow. This also blocks
  private-LAN and metadata-service access when those routes share the default
  egress NIC. A disabled forwarding family remains drop-all even if the host
  sysctl was already enabled. MASQUERADE sources are limited to the assigned
  `/32` and `/128`.
- **Local helper secrecy:** Hidden helper workers read their connect payloads
  from stdin instead of argv, and that stdin payload is a magic-prefixed Norito
  frame rather than JSON. The helper's private state file is also a
  magic-prefixed Norito frame; only the CLI status output remains JSON for local
  UX. The only ticket trust anchor is the fixed
  `/etc/sora-vpn-controller/helper-ticket-issuer-public-key.hex` path. The
  helper accepts an exact canonical Ed25519 public key only from a root-owned,
  single-link, owner-private direct regular file under trusted parent
  directories; neither the caller, environment, nor payload can select another
  key. Both the parent and hidden worker verify the signed capability, and the
  worker checks expiry again before privileged tunnel preparation and while the
  tunnel is active. V1 requires 1–64 pushed routes, 0–64 exclusions, and 1–8
  DNS resolvers. Route entries are canonical network prefixes with cleared host
  bits; duplicates and exact include/exclude equality are rejected, while a
  more-specific exclusion under a pushed default remains valid. DNS literals
  reject unspecified, multicast, and IPv4 limited-broadcast addresses. The
  tunnel addresses must exactly equal the session-derived pair and the MTU is
  exactly 1,280 bytes. The privileged payload must carry the exact canonical
  metering private-key seed whose Ed25519 public key is signed into the ticket;
  omission or substitution fails before host networking changes. Usage voucher
  signing uses the signed tariff and one fixed one-second first-release cadence;
  the caller cannot select a timer interval, lease lifetime, or exit class in
  the privileged payload. Helper traffic counters are batched in memory with
  at-most-once-per-second state-file flushes plus a forced shutdown flush.
  First-release privileged mutations are Linux-only and fail closed unless the
  root-owned executable was entered directly through a set-user-ID transition:
  the real UID must be non-root while the effective and saved UIDs are root.
  Direct root, `sudo`, unprivileged, and capability-only invocation are refused
  until a private root daemon can authenticate clients with `SO_PEERCRED`.
  State persists that real UID, exact session id, and authenticated network
  policy hash. Connect cannot replace another UID's state, while disconnect and
  repair require both the same UID and an explicit matching `--session-id`.
  The set-user-ID process is only a supervisor: it owns the durable journal,
  signals, TUN creation, route/DNS mutations, exact child reaping, and cleanup.
  Both privileged child launches execute the already-running Linux inode through
  `/proc/self/exe`; neither re-resolves the installation pathname after the
  executable custody check, and child identity capture pins the same device and
  inode.
  It separately execs a network worker bound to the authenticated caller and a
  one-launch random token over an inherited nonblocking Unix `SOCK_SEQPACKET`
  descriptor. Before parsing the connect payload or doing DNS, QUIC, TLS,
  SoraNet handshake/record, voucher, or packet work, that child sets
  `no_new_privs`, disables retained/ambient capabilities, clears supplementary
  groups, sets all real/effective/saved GIDs and UIDs to the caller, and verifies
  the resulting IDs, empty group set, and zero capability sets. Because Linux
  clears `PDEATHSIG` on both set-user-ID exec and later credential changes, the
  worker installs and parent-PID-checks its kill-on-supervisor-death binding
  both before and after the permanent drop.
  The fixed 64-byte credential-bearing IPC sequence is `WORKER_READY`,
  `TUN_READY(fd)`, `TUN_ACK`, `START`, `STARTED`, cumulative `TRAFFIC`, `STOP`,
  and `WORKER_EXIT`. Every datagram carries kernel peer credentials and the
  inherited token; phases, reserved fields, values, and the single-descriptor
  transfer are exact. Launch authenticates the supervisor's effective root UID
  with `SO_PEERCRED`; subsequent `SCM_CREDENTIALS` frames pin the same parent
  PID and its real caller UID/GID, matching Linux credential semantics. The
  worker validates that descriptor as the expected
  read/write, nonblocking, close-on-exec `/dev/net/tun` device, with the exact
  interface name, `IFF_TUN|IFF_NO_PI`, and signed MTU. Connected state is not
  published until `STARTED` is received.
  Before the first host mutation the supervisor fsyncs a complete
  repair-required plan. Preparation advances that journal after TUN creation,
  link/route configuration, each pre-recorded exclusion mutation, and DNS
  intent. Teardown always stops and reaps the exact child, closes the final TUN
  custody, then restores global state. Cleanup durably advances after DNS
  revert, retains a `resolv.conf` backup through a persisted restored phase,
  removes it durably, and restores/pops each excluded route one at a time.
  Every external cleanup unit is idempotent, so a failure or crash resumes from
  the last durable phase without consuming the backup or skipping later routes.
  If the outer connect path observes an early supervisor exit or readiness
  timeout, it retains the action lock, terminates and reaps that exact direct
  child, and only then resumes progressive cleanup and publishes a terminal or
  repair-required state. Global routes or DNS are never restored while a child
  may still own the TUN descriptor.
  `install-check` is a static redacted health response. `status` authenticates
  the set-user-ID caller and returns session details only to the owning UID;
  neither observational command persists caller-triggered state changes.
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
