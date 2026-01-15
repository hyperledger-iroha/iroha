## Norito-RPC Protocol Specification (NRPC-1)

Status: Accepted — 2026-03-21  
Owners: Torii Platform TL, SDK Program Lead  
Source of truth: This document + `docs/source/torii/norito_rpc.md`  
Related roadmap items: NRPC-1 (spec), NRPC-2 (rollout), NRPC-3 (SDK adoption)

### 0. Normative References
- [`docs/source/torii/norito_rpc.md`](norito_rpc.md) — background RFC and design rationale.  
- [`docs/source/torii/norito_rpc_rollout_plan.md`](norito_rpc_rollout_plan.md) — deployment/runbook guardrails.  
- [`docs/source/torii/norito_rpc_telemetry.md`](norito_rpc_telemetry.md) — observability contract.  
- [`docs/source/torii/norito_rpc_tracker.md`](norito_rpc_tracker.md) — action tracker.  
- `crates/norito/src/core.rs`, `crates/iroha_torii/src/utils.rs`, `crates/iroha_torii/src/lib.rs` — authoritative implementation.

### 1. Transport Overview
| Property | Requirement |
|----------|-------------|
| Endpoint | Same host/port as `/v1/pipeline`; HTTPS/TLS mandatory. |
| HTTP versions | 1.1 and 2.0 supported; ALPN `h2` preferred when available. |
| Media Type | Requests *must* set `Content-Type: application/x-norito`; responses honour `Accept` negotiation (`crates/iroha_torii/src/utils.rs:63`). |
| Auth | Standard `Authorization: Bearer …` or Torii `X-API-Token`; mTLS optional per rollout stage (see NRPC-2A). |
| Connection tagging | Requests with the Norito content-type are tracked under `ConnScheme::NoritoRpc` for rate limits and telemetry (`crates/iroha_torii/src/lib.rs:714`). |
| Routes | No new endpoints. Every existing Torii handler accepts Norito payloads identical to the JSON DTOs enumerated in `docs/source/torii/app_api_parity_audit.md`. |

### 2. Envelope & Framing
Every Norito-RPC payload begins with the canonical header defined in `crates/norito/src/core.rs`.

| Offset | Field        | Type      | Notes |
|--------|-------------|-----------|-------|
| 0      | Magic       | `[u8;4]`  | ASCII `NRT0`; rejects malformed payloads. |
| 4      | Major       | `u8`      | Current value `1`; mismatches return HTTP 400. |
| 5      | Minor       | `u8` set  | Bitmask of negotiated layout flags. |
| 6      | Schema hash | `[u8;16]` | Deterministic hash of the DTO (see Section 3). |
| 22     | Compression | `u8`      | `0 = none`, `1 = zstd`. |
| 23     | Length      | `u64`     | Uncompressed payload length in bytes. |
| 31     | CRC64       | `u64`     | CRC64-XZ (ECMA polynomial, reflected, init/xor all ones) over the uncompressed bytes. |
| 39     | Flags       | `u8`      | Layout bits (packed sequences, compact lengths, varint offsets, field bitset). |

Validation rules:
1. Magic + major version must match or Torii returns `400 schema mismatch`.
2. Unsupported flags/compression values trigger `415 Unsupported Media Type`.
3. CRC64 must match the decoded payload; failures are logged as `torii_norito_decode_failures_total`.  
4. Schema hash *must* equal the expected DTO hash; mismatches raise `X-Iroha-Error-Code: schema_mismatch`.

### 3. Message Taxonomy

| Category | Routes | Request DTO | Response DTO | Notes |
|----------|--------|-------------|--------------|-------|
| Pipeline submission | `/pipeline/submit`, `/pipeline/submit_batch` | `SignedTransaction` | `SubmitResponse` | Norito header ensures deterministic TX canonicalisation. |
| Query & status | `/query/*`, `/status/*`, `/block/*` | Various `QueryBox` variants | Domain DTOs (e.g., `Block`, `PeersStatus`) | Schema hashes align with the Norito JSON equivalents; pagination fields are unchanged. |
| Governance / Sumeragi | `/sumeragi/*`, `/governance/*` | `GovernanceInstruction`, `SumeragiEvidence*` | `GovernanceResponse`, `EvidenceReceipt` | Required for ZK/SNARK payloads where JSON is insufficient. |
| SoraFS / SoraNet | `/sorafs/*`, `/soranet/*` | `SorafsRequest`, `GuardDirectoryRequest` | `SorafsResponse`, `GuardDirectoryResponse` | Multi-source orchestrator + guard selection rely on binary payload integrity. |
| Norito RPC control | `/rpc/capabilities`, `/rpc/ping` | `NoritoRpcPing` | `NoritoRpcPong` | Lightweight control plane for health checks; optional but recommended by SDKs. |

All DTO hashes are catalogued under `fixtures/norito_rpc/schema_hashes.json`
(`type_name`, `alias`, and the 16-byte `schema_hash` hex), regenerated via
`cargo xtask norito-rpc-fixtures`, and verified across SDK copies with
`cargo xtask norito-rpc-verify`.

### 4. Error Semantics

| Error | HTTP Status | `X-Iroha-Error-Code` | Description |
|-------|-------------|---------------------|-------------|
| `Unsupported feature: layout flag` | 415 | `unsupported_layout` | Client advertised a flag not recognised by the current Torii binary. |
| `Checksum mismatch` | 400 | `checksum_mismatch` | CRC64 mismatch after decompression; payload discarded. |
| `Schema mismatch` | 400 | `schema_mismatch` | Header hash does not match DTO hash compiled into Torii. |
| `Decode` | 400 | `decode_error` | Generic Norito decode failure (invalid enum/tag). |
| `Forbidden` | 403 | `permission_denied` | Reused across JSON/Norito; still relies on bearer tokens/mTLS auth. |
| `RateLimited` | 429 | `rate_limited` | Per-scheme throttling; clients should honour `Retry-After`. |

Clients must surface both HTTP status and `X-Iroha-Error-Code` to the caller. SDKs SHOULD retry idempotent requests on 429/503 with backoff.

### 5. Negotiation, Compression & Streaming
- Set `Accept: application/x-norito` to request binary responses. Torii treats
  `application/json`, `text/json`, and `application/*+json` as JSON and ignores
  media-type parameters. JSON fallback remains available for clients without
  binary support.  
- When a handler emits a dynamic JSON value, the `application/x-norito` payload
  is a Norito-framed UTF-8 JSON string (schema hash for `String`); decode the
  string, then parse JSON.  
- Requests may omit compression (`Compression = 0`). If compression is enabled, the payload MUST be zstd framed with content length equal to the *uncompressed* size; Torii recomputes the checksum after decompression.  
- Clients may pipeline requests over HTTP/2; each frame is self-contained and independent.

### 6. Client Behaviour Requirements
1. Always serialise DTOs using the official Norito codecs (`norito::to_bytes`, `norito::json::to_vec`) to guarantee schema hashes.  
2. Preserve the header when proxying responses (e.g., CLI helpers writing `.norito` files).  
3. When downgrading to JSON, propagate schema mismatch errors so operators can see which DTO changed.  
4. Record `NoritoRpcClient` metrics (`torii.request.duration`, `torii.request.bytes`) alongside the regular HTTP stats so telemetry parity can be verified.  
5. Honor the `transport.norito_rpc.stage` config flag; SDK smoke tests must refuse to flip to Norito when the server advertises `stage=disabled` via the `/rpc/capabilities` endpoint.

### 7. Connection Pooling, Pre-Auth Limits & Backpressure
- Torii applies the same pre-auth gate to every transport. Norito-RPC requests are tagged as `ConnScheme::NoritoRpc`, so they consume a dedicated bucket alongside the global/IP caps enforced by `PreAuthGate` (`crates/iroha_torii/src/lib.rs:709`, `crates/iroha_torii/src/lib.rs:760`, `crates/iroha_torii/src/limits.rs:270`). Configure `torii.preauth_max_connections`, `torii.preauth_max_connections_per_ip`, and `torii.preauth_scheme_limits[].{scheme,max_connections}` (`crates/iroha_config/src/parameters/user.rs:6173`, `crates/iroha_config/src/parameters/user.rs:6504`) so the Norito pool cannot evict JSON/WS callers.
- Active connections per scheme are exported via `torii_active_connections_total{scheme="norito_rpc"}` and rejections are counted in `torii_pre_auth_reject_total{reason}` (`crates/iroha_core/src/telemetry.rs:3423`, `docs/source/torii/norito_rpc_telemetry.md:32`). Dashboards and the canary runbook already alert on these gauges (`docs/source/runbooks/torii_norito_rpc_canary.md:75`); clients must never open best-effort connections beyond the configured budget.
- Client expectations:
  1. Reuse a single HTTP/2 connection (or a very small pool) per Torii host. Opening a new TLS session per request risks tripping `scheme_cap` throttles before the allowlist can be expanded.
  2. Bound in-flight Norito calls. SDKs SHOULD gate concurrency locally and surface `429/503` responses that include `Retry-After` or `X-Iroha-Error-Code: preauth_rate_limited|preauth_scheme_cap`.
  3. When a pre-auth rejection occurs, drop the offending connection immediately and retry on JSON only after the cooldown elapses; otherwise the ban timer (`torii.preauth_ban_duration_ms`) will extend the outage.
  4. Report pool depth and rejection counters in client telemetry so operators can reconcile them with `torii_active_connections_total`.

### 8. Dual-Stack Behaviour & `/v1/pipeline` Coexistence
- `/rpc/capabilities` and `/rpc/ping` advertise the live Norito settings (`enabled`, `require_mtls`, rollout `stage`, and the canary allowlist size) pulled straight from `NoritoRpcTransport` (`crates/iroha_torii/src/lib.rs:1042`, `crates/iroha_config/src/parameters/actual.rs:1876`). Clients MUST call `/rpc/capabilities` during startup and before flipping transports.
- Gate enforcement happens before routing: `evaluate_norito_rpc_gate` blocks requests when `enabled = false`, `stage = disabled`, or the supplied API token is not listed in the canary allowlist, returning `norito_rpc_disabled` or `norito_rpc_canary_denied` envelopes (`crates/iroha_torii/src/lib.rs:1058`, `crates/iroha_torii/src/lib.rs:1090`, `crates/iroha_torii/src/lib.rs:7130`). SDKs must treat these as soft failures and fall back to JSON immediately.
- Negotiation rules:
  1. Send `/rpc/capabilities`; if `enabled = false` or `stage = "disabled"`, pin the JSON transport until the next poll interval.
  2. When `stage = "canary"`, include an `X-API-Token` that appears in `torii.transport.norito_rpc.allowed_clients`. Missing or stale tokens MUST downgrade the connection and emit a warning.
  3. Honour the `require_mtls` flag. If Torii advertises mTLS but the client cannot supply certificates, stay on JSON.
  4. Even after switching to Norito, keep the JSON `/v1/pipeline` path hot for retries. Any `415 unsupported_layout`, `norito_rpc_disabled`, or TLS failure requires downgrading the affected request and recording the failure for operators (the rollout plan and canary runbook describe the evidence bundle: `docs/source/torii/norito_rpc_rollout_plan.md`, `docs/source/runbooks/torii_norito_rpc_canary.md`).

### 9. Dual-Stack Rollout Review (Scheduled)
- **Review slot**: 2025-06-19 15:00 UTC (Torii Platform weekly sync).  
- **Agenda**:  
  1. Confirm this specification (`nrpc_spec.md`) is frozen for NRPC-1.  
  2. Walk through NRPC-2 ingress/auth telemetry changes and sign off the `transport.norito_rpc` config knobs.  
  3. Validate NRPC-3 SDK commitments (Android mock harness parity demo + JS/Python fixture publication).  
  4. Capture action items for AND4 (Android networking preview) and AND7 (telemetry runbook).  
- **Participants**: Torii Platform TL, SDK Program Lead, Android Networking TL, Observability liaison, DevRel docs owner, Release Engineering delegate.  
- Meeting artifact: notes recorded in `docs/source/torii/norito_rpc_sync_notes.md` (June entry) plus a sign-off checklist stored in `docs/source/torii/norito_rpc_tracker.md` (new row NRPC-2R).

### 10. Fixture & Compliance Requirements
- All schema hashes, request/response fixtures, and regression tests live under `fixtures/norito_rpc/`. Running `cargo xtask norito-rpc-fixtures` regenerates the catalogue and updates the manifest consumed by SDK CI, and `cargo xtask norito-rpc-verify` ensures the Android/Python/Swift copies remain bit-for-bit identical.  
- QA team maintains the dual-transport smoke script (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) which must pass before releases flip `transport.norito_rpc.stage` to `canary`.  
- Operator docs (`docs/source/runbooks/torii_norito_rpc_canary.md`) must link back to this spec to ensure ingress teams use the correct headers and error codes.

### 11. Transport Configuration & Stage Gates
- Operators steer NRPC via the `torii.transport.norito_rpc` block in `iroha_config`. The nested struct lives in `crates/iroha_config/src/parameters/user.rs:6383` and resolves to the actual runtime enum in `crates/iroha_config/src/parameters/actual.rs:1889`.

  | Key | Default | Notes |
  |-----|---------|-------|
  | `torii.transport.norito_rpc.enabled` | `true` (`crates/iroha_config/src/parameters/defaults.rs:552`) | Master switch. When `false`, the gate rejects every Norito request before it reaches the router. |
  | `torii.transport.norito_rpc.stage` | `"disabled"` (`crates/iroha_config/src/parameters/defaults.rs:556`) | Parsed into `NoritoRpcStage::{Disabled,Canary,Ga}` (`crates/iroha_config/src/parameters/actual.rs:1899`). The label is surfaced verbatim via `/rpc/capabilities`. |
  | `torii.transport.norito_rpc.require_mtls` | `false` (`crates/iroha_config/src/parameters/defaults.rs:555`) | When `true`, Torii rejects Norito-RPC requests that do not present an mTLS marker header (e.g. `X-Forwarded-Client-Cert`). Clients must surface the flag and operators must configure their proxies to propagate the header. |
  | `torii.transport.norito_rpc.allowed_clients[]` | `[]` (`crates/iroha_config/src/parameters/defaults.rs:559`) | Canonical list of API tokens that are allowed to use the transport while the stage is `canary`. The list size is mirrored in `/rpc/capabilities`. |

- **Stage semantics (wired through `NoritoRpcStage`)** (`crates/iroha_config/src/parameters/actual.rs:1889`):  
  - `disabled` — all requests fail with `norito_rpc_disabled`, even if `enabled = true`.  
  - `canary` — requests must send an `X-API-Token` that appears in `allowed_clients`; everything else receives `norito_rpc_canary_denied`.  
  - `ga` — any authenticated caller may use Norito-RPC; the allowlist is ignored.

- **Admission path**: the pre-auth middleware tags requests whose `Content-Type` contains `application/x-norito` as `ConnScheme::NoritoRpc` (`crates/iroha_torii/src/lib.rs:723`) and runs the gate before routing (`crates/iroha_torii/src/lib.rs:1148`). `evaluate_norito_rpc_gate` enforces the `enabled` flag, the mTLS requirement (`crates/iroha_torii/src/lib.rs:1040`), and the stage logic (`crates/iroha_torii/src/lib.rs:1057`), reads the allowlist via `HEADER_API_TOKEN`/`X-API-Token` (`crates/iroha_torii/src/lib.rs:1079`, `crates/iroha_torii/src/lib.rs:7129`), and returns `403` responses stamped with `X-Iroha-Error-Code: norito_rpc_disabled|norito_rpc_mtls_required|norito_rpc_canary_denied` (`crates/iroha_torii/src/lib.rs:1086`, `crates/iroha_torii/src/lib.rs:7130`). SDKs MUST treat those errors as downgrade signals.
- **Retry hints**: each gate rejection now includes `Retry-After: 300` so SDKs have a deterministic backoff window before probing the stage again or logging the downgrade for operators.

- **Capability discovery**: `/rpc/capabilities` and `/rpc/ping` embed `RpcNoritoRpcCapability { enabled, require_mtls, stage, canary_allowlist_size }` so clients can log what Torii advertises at runtime (`crates/iroha_torii/src/lib.rs:794`, `crates/iroha_torii/src/lib.rs:800`, `crates/iroha_torii/src/lib.rs:1042`). The responses are backed by the same config structure that guards ingress, ensuring telemetry and SDK behaviour stay in sync.

### 12. Change Control
- Minor updates (typos, clarifications) may merge with Torii Platform approval.  
- Additive protocol changes (new layout flag, compression mode, schema hash policy) require an RFC update and a new spec version recorded in this document.  
- Any change affecting schema hashes must regenerate `fixtures/norito_rpc/schema_hashes.json` and trigger an announcement on the Torii SDK mailing list.  
- Breaking changes require simultaneous updates to `docs/source/torii/norito_rpc_rollout_plan.md` and `status.md`, plus a minimum of one release cycle notice.

This specification locks the Norito-RPC contract for NRPC-1 and unblocks the NRPC-2 rollout and AND4 Android networking workstreams.
