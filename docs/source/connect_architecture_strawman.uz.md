---
lang: uz
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-05T09:28:12.003461+00:00"
translation_last_reviewed: 2026-02-07
---

# Connect Session Architecture Strawman (Swift / Android / JS)

This strawman proposal outlines the shared design for Nexus Connect workflows
across the Swift, Android, and JavaScript SDKs. It is intended to support the
Feb 2026 cross-SDK workshop and capture open questions before implementation.

> Last updated: 2026-01-29  
> Authors: Swift SDK Lead, Android Networking TL, JS Lead  
> Status: Draft for council review (threat model + data retention alignment added 2026-03-12)

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
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
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

- `EnvelopeV1` (open / approve / sign / control)
- `ConnectFrameV1` (ciphertext frames w/ AEAD payload)
- Control codes:
  - `open_ext` (metadata, permissions)
  - `approve_ext` (account, permissions, proofs, signature)
  - `reject`, `close`, `ping/pong`, `error`

Swift previously shipped placeholder JSON encoders (`ConnectCodec.swift`). As of Apr 2026 the SDK
always uses the Norito bridge and fails closed when the XCFramework is missing, but this strawman
still captures the mandate that led to the bridge integration:

| Function | Description | Status |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp open frame | Implemented in bridge |
| `connect_norito_encode_control_approve_ext` | Wallet approval | Implemented |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Sign requests | Implemented |
| `connect_norito_encode_envelope_sign_result_ok/err` | Sign results | Implemented |
| `connect_norito_decode_*` | Parsing for wallets/dApps | Implemented |

### Required Work

- Swift: Replace placeholder `ConnectCodec` JSON helpers with bridge calls and surface
  typed wrappers (`ConnectFrame`, `ConnectEnvelope`) using the shared Norito types. ✅ (Apr 2026)
- Android/JS: Ensure the same wrappers exist; align error codes and metadata keys.
- Shared: Document encryption (X25519 key exchange, AEAD) with consistent key derivation
  per Norito spec, and provide sample integration tests using the Rust bridge.

## Transport Contract

- Primary transport: WebSocket (`/v1/connect/ws?sid=<session_id>`).
- Optional future: WebRTC (TBD) – out of scope for initial strawman.
- Reconnect strategy: exponential back-off with full jitter (base 5 s, max 60 s); shared constants across Swift, Android, and JS so retries remain predictable.
- Ping/pong cadence: 30 s heartbeat with tolerance for three missed pongs before reconnect; JS clamps minimum interval to 15 s to satisfy browser throttling rules.
- Push hooks: Android wallet SDK exposes optional FCM integration for wake-ups, while JS stays polling-based (documented limitations for browser push permissions).
- SDK responsibilities:
  - Maintain ping/pong heartbeats (avoid draining batteries on mobile).
  - Buffer outgoing frames when offline (bounded queue, persisted for dApp).
- Provide event stream API (Swift Combine `AsyncStream`, Android Flow, JS async iter).
- Surface reconnect hooks and allow manual re-subscribe.
- Telemetry redaction: only emit session-level counters (`sid` hash, direction,
  sequence window, queue depth) with salts documented in the Connect telemetry
  guide; headers/keys must never appear in logs or debug strings.

## Encryption & Key Management

### Session identifiers & salts

- `sid` is a 32-byte identifier derived from `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)`.  
  DApps compute it before calling `/v1/connect/session`; wallets echo it in `approve` frames so both sides can key journals and telemetry consistently.
- The same salt feeds every key-derivation step so SDKs never rely on entropy harvested from the host platform.

### Ephemeral key handling

- Every session uses fresh X25519 key material.  
  Swift stores it in the Keychain/Secure Enclave via `ConnectCrypto`, Android wallets default to StrongBox (falling back to TEE-backed keystores), and JS requires a secure-context WebCrypto instance or the native `iroha_js_host` plug-in.
- Open frames include the dApp ephemeral public key plus an optional attestation bundle. Wallet approvals return the wallet public key and any hardware attestation needed for compliance flows.
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

### Rotation & recovery handshake

- Rotation remains optional but the protocol is defined: dApps emit a `Control::RotateKeys` frame when sequence counters approach the wrap guard, wallets respond with the new public key plus a signed acknowledgement, and both sides immediately derive new directional keys without closing the session.
- Wallet-side key loss triggers the same handshake followed by a `resume` control so dApps know to flush cached ciphertext that targeted the retired key.

For historic CryptoKit fallbacks see `docs/connect_swift_ios.md`; Kotlin and JS have matching references under `docs/connect_kotlin_ws*.md`.

## Permissions & Proofs

- Permission manifests must round-trip through the shared Norito struct exported by the bridge.  
  Fields:
  - `methods` — verbs (`sign_transaction`, `sign_raw`, `submit_proof`, …).  
  - `events` — subscriptions the dApp is allowed to attach to.  
  - `resources` — optional account/asset filters so wallets can scope access.  
  - `constraints` — chain ID, TTL, or custom policy knobs that the wallet enforces before signing.
- Compliance metadata rides alongside permissions:
  - Optional `attachments[]` contain Norito attachment references (KYC bundles, regulator receipts).  
  - `compliance_manifest_id` ties the request to a previously approved manifest so operators can audit provenance.
- Wallet responses use the agreed codes:
  - `user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Each may carry a `localized_message` for UI hints plus a machine-readable `reason_code`.
- Approval frames include the selected account/controller, permission echo, proof bundle (ZK proof or attestation), and any policy toggles (e.g., `offline_queue_enabled`).  
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

- Each direction keeps a dedicated 64-bit `sequence` counter that starts at zero when the session opens. The shared helper types clamp increments and trigger a `ConnectError.sequenceOverflow` + key-rotation handshake well before the counter would wrap.
- Nonces and associated data reference the sequence number, so duplicates can be rejected without parsing payloads. SDKs must store `{sid, dir, seq, payload_hash}` in their journals to make deduplication deterministic across reconnects.
- Wallets advertise back-pressure via a logical window (`FlowControl` control frames). DApps dequeue only when a window token is available; wallets emit new tokens after processing ciphertext to keep pipelines bounded.
- Resume negotiation is explicit: both sides emit `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` after reconnecting so observers can verify how much data was re-sent and whether journals contain gaps.
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

Pending questions from review are tracked in `docs/source/sdk/swift/connect_workshop.md`
and will be resolved in the council minutes; once closed the strawman will be
promoted from draft to accepted.

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
  `FlowControl` helper.
- Journals cap at 32 frames / 1 MiB by default; hitting the cap evicts the
  oldest entries (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
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

### Replay & resume semantics

1. When reconnecting, SDKs emit `Control::Resume` with `{seq_app_max,
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. The hash is the
   Blake3 digest of the append-only journal so mismatched peers can detect drift.
2. The receiving peer compares the resume payload with its state, requests
   retransmission when gaps exist, and acknowledges replayed frames via
   `Control::ResumeAck`.
3. Replayed frames always respect insertion order (`sequence` then write-time).
   Wallet SDKs MUST apply back-pressure by issuing `FlowControl` tokens (also
   journaled) so dApps cannot flood the queue while offline.
4. Journals store ciphertext verbatim, so replay simply pumps the recorded bytes
   back through the transport and decoder. No per-SDK re-encoding is allowed.

### Reconnection flow

1. Transport re-establishes WebSocket and negotiates new ping interval.
2. dApp replays queued frames in order, respecting back-pressure from wallet
   (`ConnectSession.nextControlFrame()` yields `FlowControl` tokens).
3. Wallet decrypts buffered results, verifies sequence monotonicity, and
   replays pending approvals/results.
4. Both sides emit a `resume` control summarising `seq_app_max`, `seq_wallet_max`,
   and queue depths for telemetry.
5. Duplicate frames (matching `sequence` + `payload_hash`) are acknowledged and dropped; conflicts raise `ConnectError.Internal` and trigger a forced session restart.

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
| `Throttled` | Usage ≥ `disk_watermark_warn` or retries > 5/min | Pause new sign requests, emit flow-control tokens at half rate | Apps may call `clearOfflineQueue(.app|.wallet)`; SDK re-hydrates state from peer once online | `connect.queue_state=\"throttled\"`, `connect.queue_watermark` gauge |
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
- When an app toggles `offline_queue_enabled=false`, SDKs immediately drain and
  purge both journals, mark the state as `Disabled`, and emit a terminal
  telemetry event. The user-facing preference is mirrored in the Norito
  approval frame so peers know whether they can resume buffered frames.
- Operators run `connect queue inspect --sid <sid>` (CLI wrapper around the SDK
  diagnostics) during chaos tests; this command prints the state transitions,
  watermark history, and resume evidence so governance reviews do not depend on
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
  - `connect.replay_success_total`/`connect.replay_error_total`.
  - `connect.resume_latency_ms` histogram (time between reconnect and steady
    state) plus `connect.resume_attempts_total`.
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

- `connect.queue_state` doubles as the roadmap risk indicator. Dashboards group
  by `{platform, sdk_version}` and render time-in-state so governance can sample
  monthly drill evidence before approving staged rollouts.
- `connect.queue_watermark` and `connect.queue_bytes` feed the Connect risk score
  (`risk.connect.offline_buffer`), which automatically pages SRE when more than
  5 % of sessions spend >10 minutes in `Throttled`.
- Exporters attach `feature_hash` to every event so auditor tooling can confirm
  that the Norito codec + offline plan match the reviewed build. SDK CI fails
  fast when telemetry reports an unknown hash.
- The strawman still requires a threat-model appendix; when metrics exceed the
  policy thresholds, SDKs emit `connect.policy_violation` events summarising the
  offending sid (hashed), state, and resolved action (`drain|purge|quarantine`).
- Evidence captured via `exportQueueMetrics` lands in the same SoraFS namespace
  as the Connect runbook artefacts so council reviewers can trace every drill
  back to specific telemetry samples without requesting internal logs.

## Frame Ownership & Responsibilities

| Frame / Control | Owner | Sequence domain | Journal persisted? | Telemetry labels | Notes |
|-----------------|-------|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Carries metadata + permission bitmap; wallets replay the latest open before prompts. |
| `Control::Approve` | Wallet | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Includes account, proofs, signatures. Metadata version increments recorded here. |
| `Control::Reject` | Wallet | `seq_wallet` | ✅ | `event=reject`, `reason` | Optional localized message; dApp drops pending sign requests. |
| `Control::Close` (init) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | Wallet acknowledges with its own `Close`. |
| `Control::Close` (ack) | Wallet | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Confirms teardown; GC removes journals once both sides persist the frame. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Payload hash recorded for replay conflict detection. |
| `SignResult` | Wallet | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Includes BLAKE3 hash of signed bytes; failures raise `ConnectError.Signing`. |
| `Control::Error` | Wallet (most) / dApp (transport) | matching owner domain | ✅ | `event=error`, `code` | Fatal errors force session restart; telemetry marks `fatal=true`. |
| `Control::RotateKeys` | Wallet | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Announces new direction keys; dApp replies with `RotateKeysAck` (journaled on app side). |
| `Control::Resume` / `ResumeAck` | Both | local domain only | ✅ | `event=resume`, `direction=app|wallet` | Summarises queue depth + seq state; hashed journal digest aids diagnosis. |

- Directional cipher keys remain symmetric per role (`app→wallet`, `wallet→app`).
  Wallet rotation proposals are advertised via `Control::RotateKeys`, and dApps
  acknowledge by emitting `Control::RotateKeysAck`; both frames must hit disk
  before keys swap to avoid replay gaps.
- Metadata attachment (icons, localized names, compliance proofs) is signed by
  the wallet and cached by the dApp; updates require a fresh approval frame with
  incremented `metadata_version`.
- Ownership matrix above is referenced from SDK docs so CLI/web/automation
  clients follow the same contract and instrumentation defaults.

## Open Questions

1. **Session discovery**: Do we need QR codes / out-of-band handshake like WalletConnect? (Future work.)
2. **Multisig**: How are multi-sign approvals represented? (Extend sign result to support multiple signatures.)
3. **Compliance**: Which fields are mandatory for regulated flows (per roadmap)? (Await compliance team guidance.)
4. **SDK packaging**: Should we factor shared code (e.g., Norito Connect codecs) into a cross-platform crate? (TBD.)

## Next Steps

- Circulate this strawman to the SDK council (Feb 2026 meeting).
- Collect feedback on open questions and update doc accordingly.
- Schedule implementation breakdown per SDK (Swift IOS7, Android AND7, JS Connect milestones).
- Track progress via roadmap hot list; update `status.md` once strawman is ratified.
