# Connect Session Architecture (Swift / Android / JS)

This specification records the implemented Connect V1 contract shared by the
Swift, Android, and JavaScript SDKs.

## Goals

1. Align wallet ↔ dApp session lifecycle, including connection bootstrapping,
   approvals, signing requests, and teardown.
2. Define the Norito envelope schema (open/approve/sign/control) shared by all
   SDKs and ensure parity with `connect_norito_bridge`.
3. Split responsibilities between transport (WebSocket/WebRTC), encryption
   (Norito Connect frames + key exchange), and application layers (SDK facades).
4. Ensure deterministic behaviour across desktop/mobile platforms, including
   offline buffering and reconnection.

## Session Lifecycle (High-Level)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (app key, metadata, permissions, NetworkId)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Envelope/Norito Schema

All SDKs MUST use the canonical Norito schema defined in `connect_norito_bridge`:

- `ConnectControlV1` (open / approve / reject / close / heartbeat)
- `EnvelopeV1` (encrypted post-key payloads)
- `ConnectFrameV1` (ciphertext frames w/ AEAD payload)
- Control codes:
  - `open_ext` (metadata, permissions)
  - `approve_ext` (account, permissions, proofs, signature)
  - `reject`, `close`, `ping/pong`, `error`

Swift uses the Norito bridge and fails closed when the XCFramework is missing:

| Function | Description | Status |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp open frame | Implemented in bridge |
| `connect_norito_encode_control_approve_ext` | Wallet approval | Implemented |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Sign requests | Implemented |
| `connect_norito_encode_envelope_sign_result_ok/err` | Sign results | Implemented |
| `connect_norito_decode_*` | Parsing for wallets/dApps | Implemented |

### SDK parity contract

- Swift exposes typed `ConnectFrame` and `ConnectEnvelope` wrappers backed by
  the shared bridge.
- Android and JavaScript expose the same envelope families, error codes, and
  metadata keys.
- Cross-SDK fixtures and integration tests keep X25519 key exchange, AEAD,
  sequence, and Norito behaviour aligned.

## Transport Contract

- Primary transport: WebSocket (`/v1/connect/ws?sid=<session_id>&role=<app|wallet>`),
  authenticated with the role's one-shot token.
- Torii relay transport is in-node only: `CONNECT_RELAY_STRATEGY="broadcast"` uses Iroha peer-to-peer
  node transport, while `"local_only"` disables cross-node forwarding. These are the only accepted
  first-release spellings; aliases, case folding, whitespace normalization, and unknown values are
  rejected. There is no centralized relay gateway mode.
- Operator-only aggregate status reflects both configured and effective relay
  behavior under `/v1/connect/status/aggregate`: `policy.relay_strategy`
  (configured), `policy.relay_effective_strategy` (effective), and
  `policy.relay_p2p_attached` (P2P relay availability). The disjoint
  `/v1/connect/status?sid=...` protocol route returns only one session after
  management-token authentication.
- A session shadow learned from an authenticated Iroha peer retains the
  `ConnectSessionClaimV1.expires_at_ms` deadline as an absolute ceiling. At the
  deadline Torii rejects role-token attachment, management reads, and relay
  traffic even if the shadow was recently active; cleanup also removes an
  attached shadow and its WebSocket observes the standard TTL close. A repeated
  claim with the same SID but a different deadline is a conflict, not a lease
  extension.
- WebSocket is the only Connect V1 client transport; WebRTC is unsupported.
- Fresh-session retry strategy: exponential back-off with full jitter (base 5 s, max 60 s); shared constants across Swift, Android, and JS so retries remain predictable.
- Ping/pong cadence: 30 s heartbeat with tolerance for three missed pongs before the client abandons the SID and provisions a fresh session; JS clamps minimum interval to 15 s to satisfy browser throttling rules.
- Push hooks: Android wallet SDK exposes optional FCM integration for wake-ups, while JS stays polling-based (documented limitations for browser push permissions).
- SDK responsibilities:
  - Maintain ping/pong heartbeats (avoid draining batteries on mobile).
  - Buffer outgoing frames when offline (bounded queue, persisted for dApp).
- Provide event stream API (Swift Combine `AsyncStream`, Android Flow, JS async iter).
- Surface fresh-session retry hooks and allow manual re-subscribe with newly provisioned role tokens.
- Telemetry redaction: only emit session-level counters (`sid` hash, direction,
  sequence window, queue depth) with salts documented in the Connect telemetry
  guide; headers/keys must never appear in logs or debug strings.

## Encryption & Key Management

### Session identifiers & salts

- `sid` is exactly
  `BLAKE2b-256("iroha-connect|sid|" || network_id_raw_32 || app_ephemeral_pk || nonce16)`.
  `network_id_raw_32` is the exact genesis-derived `NetworkId` payload, never
  a display label or legacy chain identifier. DApps send all four committed
  fields to `/v1/connect/session`; Torii rejects noncanonical encodings,
  cross-network SIDs, duplicate registration, and response substitution.
- The same salt feeds every key-derivation step so SDKs never rely on entropy harvested from the host platform.

### Ephemeral key handling

- Every session uses fresh X25519 key material. Provider-neutral software-backed
  custody is valid on every platform. Applications may explicitly select
  Keychain/Secure Enclave custody through `ConnectCrypto` on Swift or
  StrongBox/TEE-backed keystores on Android; JS requires a secure-context
  WebCrypto instance or the native `iroha_js_host` plug-in.
- Open frames include the dApp ephemeral public key plus an optional attestation
  bundle. Wallet approvals return the wallet public key and any hardware
  attestation needed only when an application explicitly selects that
  compliance profile. Hardware custody and attestation are not ordinary Connect
  build, test, governance, deployment, or release gates.
- Attestation payloads follow the accepted schema:
  `attestation { platform, evidence_b64, statement_hash }`.
  Browsers may omit the block; native wallets include it whenever hardware-backed keys are in use.

### Directional keys & AEAD

- Shared secrets are expanded with HKDF-SHA256 (via the Rust bridge helpers) and domain-separated info strings:
  - `iroha-connect|k_app` → app→wallet traffic.
  - `iroha-connect|k_wallet` → wallet→app traffic.
- AEAD is ChaCha20-Poly1305 for the v1 envelope (`connect_norito_bridge` exposes helpers on every platform).
  Associated data equals `("connect:v1", sid, dir, seq_le, kind=ciphertext)` so tampering on headers is detected.
- Nonces are derived from the 64-bit sequence counter (`nonce[0..4]=0`, `nonce[4..12]=seq_le`). Shared helper tests ensure BigInt/UInt conversions behave identically across SDKs.

### Approval authentication

- Torii persists the first valid Open's exact application key, `NetworkId`
  constraint, and requested permissions. A second Open terminates the session.
- Approve carries a canonical account identifier and a detached account-key
  signature. The preimage binds the exact NetworkId and encoded constraints,
  SID, application and wallet X25519 keys, account identifier, accepted
  permissions, optional sign-in proof, and the session relay-auth hash.
- Torii verifies this account signature before delivering Approve. Wrong-network
  Open frames, signed-field substitution, approval-before-Open, and a second
  approval terminate the session. There is no legacy chain-label or unsigned
  approval path.
- Connect V1 has no key-rotation or resume control. Endpoints close the session
  before sequence exhaustion or after key loss and establish a fresh SID.

The maintained Swift, Kotlin, and JavaScript SDK facades implement the same
Connect V1 directional-key and AEAD contract. Public integration walkthroughs
live in the sibling `iroha-docs` repository at
<https://docs.iroha.tech/guide/tutorials/>.

## Permissions & Proofs

- Permission manifests must round-trip through the shared Norito struct exported by the bridge.
  Fields:
  - `methods` — verbs (`sign_transaction`, `sign_raw`, `submit_proof`, …).
  - `events` — subscriptions the dApp is allowed to attach to.
  - `resources` — optional account/asset filters so wallets can scope access.
  - Open `constraints` — the exact genesis-derived `NetworkId` enforced by the
    wallet and Torii before approval. Connect V1 has no label, TTL, or custom
    constraint compatibility fields.
- Compliance metadata rides alongside permissions:
  - Optional `attachments[]` contain Norito attachment references (KYC bundles, regulator receipts).
  - `compliance_manifest_id` ties the request to a previously approved manifest so operators can audit provenance.
- Wallet responses use the agreed codes:
  - `user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.
  Each may carry a `localized_message` for UI hints plus a machine-readable `reason_code`.
- Approval frames include the selected account/controller, permission echo, proof bundle (ZK proof or attestation), and any policy toggles (e.g., `deferred_queue_enabled`).
  Rejections mirror the same schema with empty `proof` but still record the `sid` for auditability.

## SDK Facades

| SDK | Proposed API | Notes |
|-----|--------------|-------|
| Swift | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | Replace placeholders with typed wrappers + async streams. |
| Android | Kotlin coroutines + sealed classes for frames | Align with Swift structure for portability. |
| JS | Async iterators + TypeScript enums for frame kinds | Provide bundler-friendly SDK (browser/node). |

### Common behaviours

- `ConnectSession` orchestrates lifecycle:
  1. Establish WebSocket, perform handshake.
  2. Exchange open/approve frames.
  3. Handle sign requests/responses.
  4. Emit events to application layer.
- Provide high-level helpers:
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` – emits a control frame acknowledged by the wallet.
- Error handling: map Norito error codes to SDK-specific errors; include
  domain-specific codes for UI using the shared taxonomy (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`). Swift's baseline implementation + telemetry guide lives in [`connect_error_taxonomy.md`](connect_error_taxonomy.md) and is the reference for Android/JS parity.
- Emit telemetry hooks for queue depth, reconnect counts, and request latency (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).

## Sequence Numbers & Flow Control

- Each direction keeps a dedicated 64-bit `sequence` counter whose first frame
  is `1`. Torii requires contiguous increments; gaps, wraparound, or a frame in
  the wrong role direction terminate the session.
- Nonces and associated data reference the sequence number, so duplicates can be rejected without parsing payloads. SDKs store `{sid, dir, seq, payload_hash}` in their journals to diagnose replay before discarding an interrupted session.
- Connect V1 defines no `FlowControl` or `Resume` compatibility control. Each
  role token is consumed atomically with its first successful WebSocket attach;
  transport loss therefore requires a fresh exact-network SID and new tokens.
- Conflicts (e.g., two payloads with the same `(sid, dir, seq)` but different hashes) escalate to `ConnectError.Internal` and force a new `sid` to avoid silent divergence.

## Threat model and data retention alignment

- **Surfaces considered:** WebSocket transport, Norito bridge encode/decode,
  journal persistence, telemetry exporters, and app-facing callbacks.
- **Primary goals:** protect session secrets (X25519 keys, derived AEAD keys,
  nonce/sequence counters) from leaks in logs/telemetry, prevent replay and
  downgrade attacks, and bound retention of journals and anomaly reports.
- **Mitigations codified:**
  - Journals carry ciphertext only; metadata stored is limited to hashes, length
    fields, timestamps, and sequence numbers.
  - Telemetry payloads redacts any header/payload content and includes only
    salted hashes of `sid` plus aggregate counters; redaction checklist shared
    between SDKs for audit parity.
  - Session logs are rotated and age out after 7 days by default. Wallets expose
    a `connectLogRetentionDays` knob (SDK default 7) and document the behaviour
    so regulated deployments can pin stricter windows.
  - Bridge API misuse (missing bindings, corrupt ciphertext, invalid sequence)
    returns typed errors without echoing raw payloads or keys.

SDK-specific parity evidence is recorded in
`specs/sdk/swift/connect_workshop.md`.

## Offline Buffering & Reconnections

### Journaling contract

Every SDK maintains an append-only journal per session so the dApp and wallet
can queue frames while offline, resume without data loss, and provide evidence
for telemetry. The contract mirrors the Norito bridge types so the same byte
representation survives across the mobile/JS stacks.

- Journals live under a hashed session identifier (`sha256(sid)`), producing two
  files per session: `app_to_wallet.queue` and `wallet_to_app.queue`. Swift uses
  a sandboxed file wrapper, Android stores the files via `Room`/`FileChannel`,
  and JS writes to IndexedDB; all formats are binary and endian-stable.
- Each record serialises as `ConnectJournalRecordV1`:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]` (Blake3 of ciphertext + headers)
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (exact Norito frame already AEAD-wrapped)
- Journals store ciphertext verbatim. We never re-encrypt the payload; AEAD
  headers already authenticate direction keys, so persistence reduces to
  fsyncing the appended record.
- A `ConnectQueueState` struct in memory mirrors the file metadata (depth,
  bytes used, oldest/newest seq). It feeds the telemetry exporters and the
  SDK-local queue limiter. This is not a Connect wire control.
- Journals cap at 32 frames / 1 MiB by default. Hitting the cap quarantines and
  discards the interrupted SID (`reason=overflow`) rather than retaining a
  sequence suffix with an unrecoverable gap. `ConnectFeatureConfig.max_queue_len`
  overrides these defaults per deployment.
- Journals retain data for 24 h (`expires_at_ms`). Background GC removes stale
  segments eagerly so the on-disk footprint stays bounded.
- Crash safety: append, fsync, and update the memory mirror _before_ notifying
  the caller. On startup, SDKs scan the directory, validate record checksums,
  and rebuild `ConnectQueueState`. Corruption causes the offending record to be
  skipped, flagged via telemetry, and optionally quarantined for support dumps.
- Because ciphertext already satisfies the Norito privacy envelope, the only
  additional metadata recorded is the hashed session id. Apps wanting extra
  privacy can opt into `telemetry_opt_in = false`, which stores journals but
  redacts queue-depth exports and disables sharing hashed `sid` in logs.
- SDKs expose `ConnectQueueObserver` so wallets/dApps can inspect queue depth,
  drains, and GC outcomes; this hook feeds status UIs without parsing logs.

### Replay and fresh-session semantics

Connect V1 has no Resume, ResumeAck, FlowControl, or key-rotation wire control.
Torii consumes each one-shot role token only when it atomically publishes that
role's first WebSocket endpoint. An upgrade that never attaches rolls its token
reservation back, but a transport that disconnects after attachment cannot
reuse the consumed token. Torii therefore removes that exact session
incarnation and sends `connect_transport_closed` to any surviving endpoint;
it never leaves the remaining role attached to a zombie SID. A duplicate, gap, wraparound, role/direction
substitution, delivery timeout, or buffer overflow terminates the session.
Clients then discard queued ciphertext and create a fresh app key, nonce, SID,
and session.

### Retry flow

1. A failed HTTP upgrade may retry the same reserved role token because no
   endpoint was attached and no token was consumed.
2. Any disconnect after attachment discards the interrupted SID and its local
   queue, then provisions a fresh exact-network session and role tokens.
3. Approve and Open remain one-shot controls and are never replayed.
4. Any uncertainty, duplicate, gap, terminal control, or server close follows
   the same fresh-session path.

### Failure modes

- If the session is considered stale (`offline_timeout_ms`, default 5 minutes),
  buffered frames are purged and the SDK raises `ConnectError.sessionExpired`.
- In case of journal corruption, SDKs attempt a single Norito decode repair; on
  failure they drop the journal and emit `connect.queue_repair_failed` telemetry.
- Sequence mismatch triggers `ConnectError.replayDetected` and forces a fresh
  handshake (session restart with new `sid`).

### Offline buffering plan & operator controls

The workshop deliverable requires a documented plan so every SDK ships the same
offline behaviour, remediation flow, and evidence surfaces. The plan below is
common across Swift (`ConnectSessionDiagnostics`), Android
(`ConnectDiagnosticsSnapshot`), and JS (`ConnectQueueInspector`).

| State | Trigger | Automatic response | Manual override | Telemetry flag |
|-------|---------|--------------------|-----------------|----------------|
| `Healthy` | Queue usage < `disk_watermark_warn` (default 60 %) and `ttl_ok` | None | N/A | `connect.queue_state=\"healthy\"` |
| `Throttled` | Usage ≥ `disk_watermark_warn` or retries > 5/min | Pause new sign requests locally | Apps may call `clearOfflineQueue(.app|.wallet)`; clearing requires a fresh session | `connect.queue_state=\"throttled\"`, `connect.queue_watermark` gauge |
| `Quarantined` | Usage ≥ `disk_watermark_drop` (default 85 %), corruption detected twice, or `offline_timeout_ms` exceeded | Stop buffering, raise `ConnectError.QueueQuarantined`, require operator acknowledgement | `ConnectSessionDiagnostics.forceReset()` deletes journals after exporting bundle | `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` counter |

- Thresholds live in `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). When a host
  omits a value, SDKs fall back to their defaults and log a warning so configs
  can be audited from telemetry.
- SDKs expose `ConnectQueueObserver` plus diagnostics helpers:
  - Swift: `ConnectSessionDiagnostics.snapshot()` yields `{state, depth, bytes,
    reason}` and `exportJournalBundle(url:)` persists both queues for support.
  - Android: `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` returns the same struct and a blob handle
    that UI code can upload to Torii support tools.
- When an app toggles `deferred_queue_enabled=false`, SDKs immediately drain and
  purge both journals, mark the state as `Disabled`, and emit a terminal
  telemetry event. This preference is local and is not added to Open or Approve.
- Operators run `connect queue inspect --sid <sid>` (CLI wrapper around the SDK
  diagnostics) during chaos tests; this command prints the state transitions,
  watermark history, and reconnect/termination evidence so governance reviews do not depend on
  platform-specific tooling.

### Evidence bundle workflow

Support and compliance teams rely on deterministic evidence when auditing
offline behaviour. Each SDK therefore implements the same three-step export:

1. `exportJournalBundle(..)` writes `{app_to_wallet,wallet_to_app}.queue` plus a
   manifest describing the build hash, feature flags, and disk watermarks.
2. `exportQueueMetrics(..)` emits the last 1 000 telemetry samples so dashboards
   can be reconstructed offline. Samples include the hashed session id when the
   user opted in.
3. The CLI helper zips both exports and attaches a signed Norito metadata file
   (`ConnectQueueEvidenceV1`) so Torii ingest can archive the bundle in SoraFS.

Bundles that fail validation are rejected with `connect.evidence_invalid`
telemetry so the SDK team can reproduce and patch the exporter.

## Telemetry & Diagnostics

- Emit Norito JSON events via shared OpenTelemetry exporters. Mandatory metrics:
  - `connect.queue_depth{direction}` (gauge) fed by `ConnectQueueState`.
  - `connect.queue_bytes{direction}` (gauge) for disk-backed footprint.
  - `connect.queue_dropped_total{reason}` (counter) for `overflow|ttl|repair`.
  - `connect.offline_flush_total{direction}` (counter) increments when queues
    drain without transport; failures increment `connect.offline_flush_failed`.
  - `connect.replay_error_total` for rejected duplicates or gaps.
  - `connect.reconnect_attempts_total` and `connect.session_restart_total`.
  - `connect.session_duration_ms` histogram (per completed session).
  - `connect.error` structured events with `code`, `fatal`, `telemetry_profile`.
- Exporters MUST attach `{platform, sdk_version, feature_hash}` labels so
  dashboards can split by SDK build. The hashed `sid` is optional and only
  emitted when telemetry opt-in is true.
- SDK-level hooks surface the same events so apps can export more detail:
  - Swift: `ConnectSession.addObserver(_:) -> ConnectEvent`.
  - Android: `Flow<ConnectEvent>`.
  - JS: async iterator or callback.
- CI gating: Swift jobs run `make swift-ci`, Android uses `./gradlew sdkConnectCi`,
  and JS runs `npm run test:connect` so telemetry/dashboards remain green before
  merging Connect changes.
- Structured logs include the hashed `sid`, `seq`, `queue_depth`, and `sid_epoch`
  values so operators can correlate client issues. Journals that fail repair emit
  `connect.queue_repair_failed{reason}` events plus an optional crash dump path.

### Telemetry hooks & governance evidence

- `connect.queue_state` is the primary offline-buffer risk signal. Dashboards group
  by `{platform, sdk_version}` and render time-in-state so governance can sample
  monthly drill evidence before approving staged rollouts.
- `connect.queue_watermark` and `connect.queue_bytes` feed the Connect risk score
  (`risk.connect.offline_buffer`), which automatically pages SRE when more than
  5 % of sessions spend >10 minutes in `Throttled`.
- Exporters attach `feature_hash` to every event so auditor tooling can confirm
  that the Norito codec + offline plan match the reviewed build. SDK CI fails
  fast when telemetry reports an unknown hash.
- The threat model and retention requirements are defined above. When metrics
  exceed policy thresholds, SDKs emit `connect.policy_violation` events summarising the
  offending sid (hashed), state, and resolved action (`drain|purge|quarantine`).
- Evidence captured via `exportQueueMetrics` lands in the same SoraFS namespace
  as the Connect runbook artefacts so council reviewers can trace every drill
  back to specific telemetry samples without requesting internal logs.

## Frame Ownership & Responsibilities

| Frame / Control | Owner | Sequence domain | Journal persisted? | Telemetry labels | Notes |
|-----------------|-------|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | One-shot; carries the exact app key, NetworkId constraint, metadata, and permissions. |
| `Control::Approve` | Wallet | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | One-shot; account signature binds the full approval transcript and relay authentication. |
| `Control::Reject` | Wallet | `seq_wallet` | ✅ | `event=reject`, `reason` | Optional localized message; dApp drops pending sign requests. |
| `Control::Close` | Either | sender direction | ✅ | `event=close`, `initiator` | Terminal; no acknowledgement or resume handshake is implied. |
| `Control::Ping` / `Pong` | Either | sender direction | No | `event=heartbeat` | Liveness nonce only; does not weaken contiguous sequence checks. |
| `Control::ServerEvent` | Torii | server sequence | No | `event=server_event` | Uses an independent server sequence and cannot advance either peer sequence. |
| `SignRequestRaw` / `SignRequestTx` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Encrypted payload; exact outer and inner sequence values must match. |
| `SignResultOk` / `SignResultErr` | Wallet | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Encrypted result bound to the wallet direction. |
| Encrypted `Control::Close` / `Reject` | Either | sender direction | ✅ | `event=close|reject` | Post-approval lifecycle control inside AEAD. |

- Directional cipher keys remain distinct per role (`app→wallet`,
  `wallet→app`) for the lifetime of the session. Key loss or exhaustion requires
  a fresh session.
- Metadata attachment is cached by the dApp only after the signed approval is
  accepted. Changing the approval transcript requires a fresh session; a second
  approval on the same SID is a replay violation.
- Ownership matrix above is referenced from SDK docs so CLI/web/automation
  clients follow the same contract and instrumentation defaults.
