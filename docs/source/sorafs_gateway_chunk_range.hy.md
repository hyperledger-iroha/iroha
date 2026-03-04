---
lang: hy
direction: ltr
source: docs/source/sorafs_gateway_chunk_range.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad6f50f19b1098353fed40bb8b9e722d8f2487d72789f12321a6356e4c6a077b
source_last_modified: "2026-01-05T09:28:12.084433+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Chunk-Range & Scheduler Integration
---

# SoraFS Gateway Chunk-Range & Scheduler Integration

## Goals

- Implement deterministic HTTP range endpoints honouring `dag-scope` semantics.
- Enforce per-peer stream tokens tied to admission policy and capacity declarations.
- Emit telemetry for range requests (`Sora-Chunk-Range`) to feed orchestrator and observability.

## API Requirements

| Method | Path | Required Headers | Notes |
|--------|------|------------------|-------|
| `GET`  | `/car/{manifest_id}` | `Range`, `dag-scope=block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce`, `X-SoraFS-Stream-Token`, `Sora-Name` (optional alias) | Returns an aligned CAR slice; alias header is validated against the manifest envelope. Stream token must be base64-encoded Norito. |
| `GET`  | `/chunk/{manifest_id}/{chunk_digest}` | `X-SoraFS-Nonce`, `X-SoraFS-Stream-Token` | Single chunk retrieval with deterministic headers. |
| `POST` | `/token` | `X-SoraFS-Client`, `X-SoraFS-Nonce`, signed manifest envelope | Issues per-peer stream token with TTL & rate limits. |

CAR responses MUST include:
- `Content-Range: bytes start-end/total`
- `X-Sora-Chunk-Range: start={start};end={end};chunks={count}`
- `X-SoraFS-Chunker` echo
- Echoed `X-SoraFS-Stream-Token`/`X-SoraFS-Nonce`

Chunk responses MUST include:
- `Content-Type: application/octet-stream`
- `X-Sora-Chunk-Range: start={offset};end={offset+len-1};chunks=1`
- `X-SoraFS-Chunk-Digest`
- Echoed nonce / stream token when supplied

## Stream Token Enforcement

- Token metadata:
  - Provider ID
  - Manifest digest / chunker handle
  - Max concurrent streams
  - Expiration (epoch seconds)
  - Rate-limit budget (req/min, bytes/s)
- Verification path checks token signature + admission envelope.
- On violation (expired, over budget) respond `429` with reason `stream_token_exhausted`.

## Telemetry

Metrics:
- `sorafs_gateway_chunk_range_requests_total{result,chunker}`
- `sorafs_gateway_stream_tokens_active`
- `sorafs_gateway_stream_token_denials_total{reason}`
- `sorafs_gateway_chunk_range_latency_ms_bucket`

Logs:
- Structured events when token issued/revoked
- Correlation ID linking token to orchestrator fetch (future SF-6b integration)

## Token Signing & Rotation

- **Key storage.** Gateways write the Ed25519 signing secret to
  `sorafs_gateway_secrets/token_signing_sk` (read-only for the gateway process). A corresponding public key record is published in the admission manifest under `gateway.token_signing_pk`.
- **Distribution.** Providers and orchestrators load the public key during bootstrap and cache it for signature verification; manifests include a `token_pk_version` so hot swaps are coordinated.
- **Rotation cadence.** Keys rotate quarterly. Rotation is driven by the `SF-6b-ROTATE-KEYS` runbook:
  1. Generate new keypair on the gateway host (`sorafs-gateway key rotate --kind token-signing`).
  2. Update the admission manifest and publish to Pin Registry.
  3. Notify orchestrators via telemetry topic `sorafs.gateway.token_pk_update`.
  4. Keep the previous key available for 24 h to honour in-flight tokens.
- **Audit trail.** Gateways emit `sorafs_gateway_token_key_rotation_total` and record the new key fingerprint in `ops/sorafs_gateway_tokens.md`.

## Canonical Token Schema

- **Field set.** Tokens are serialized as Norito JSON with the following fields:
  - `token_id` (ULID string)
  - `manifest_cid`
  - `provider_id`
  - `profile_handle`
  - `max_streams`
  - `ttl_epoch`
  - `rate_limit_bytes`
  - `issued_at`
  - `requests_per_minute`
  - `signature` (Ed25519 hex)
- **Normalization rules.** Fields are sorted alphabetically before signing to ensure canonical serialization. Numeric values are represented as integers; time values use Unix epoch seconds.
- **Scoreboard alignment.** Orchestrator scoreboard ingests the above fields directly, mapping `max_streams`, `ttl_epoch`, and `rate_limit_bytes` into availability and penalty factors. Additional scoreboard signals (e.g., token health) derive from issuance telemetry using `token_id`.
- **Validation helpers.** Shared Rust crate `sorafs_token_schema` will expose `Token::sign` / `Token::verify` and schema validation to minimize duplication across gateway and orchestrator binaries.

## Secure Token Issuance API

- **Authentication.** `/token` requires mutual TLS. Clients must present certificates signed by the admission CA; the gateway validates the certificate subject against the admission manifest.
- **Request flow.**
  1. Client submits `POST /token` with headers `X-SoraFS-Client`, `X-SoraFS-Nonce`, and a signed admission manifest envelope in the body.
  2. Gateway authenticates the client, validates the manifest, and applies per-client rate limits using `X-SoraFS-Client`.
  3. Gateway mints a token, signs it using the Ed25519 key, and returns JSON:

     ```json
     {
       "token": { /* canonical fields */ },
       "signature": "hex",
       "expires_at": 1738368000
     }
     ```

  4. Response headers include `X-SoraFS-Token-Id`, `X-SoraFS-Client-Quota-Remaining`, and the echoed `X-SoraFS-Nonce`. `X-SoraFS-Client-Quota-Remaining` reports how many issuance requests remain in the current 60 s window; when the quota is unbounded the header is set to `unlimited`. Once the budget is exhausted the gateway returns `429` together with `Retry-After` indicating when the next token request is permitted.
- **Telemetry.** Gateway records issuance metrics:
  - `sorafs_gateway_token_issuance_total{client,result}`
  - `sorafs_gateway_token_issuance_latency_ms_bucket`
  - `sorafs_gateway_token_denials_total{reason}`
- **Abuse protection.** Clients exceeding their `requests_per_minute` budget receive `429 stream_token_rate_limited`. Nonce replays are rejected with `409` and emitted via audit logs.

## Documentation & Rollout

- **Protocol documentation.** Expand `docs/source/sorafs_node_client_protocol.md` with:
  - `/token` request/response examples.
  - Token schema definitions and signature verification steps.
  - Error matrix describing `401`, `403`, `409`, `429`, and `5xx` cases.
- **SDK updates.** Coordinate with SDK teams to add helpers:
  - Rust: `sorafs_sdk::TokenClient::request_token`.
  - TypeScript: `requestToken(manifestCid, profileHandle, options)`.
  - Go: `client.RequestToken(ctx, manifestCID, opts)`.
- **Change management.** Initial rollout targets SF-5d milestone:
  1. Implement gateway token controller with schema crate.
  2. Update orchestrator to validate tokens using the shared crate.
  3. Land documentation updates and announce via release notes (`RLS-105`).
  4. Enable telemetry dashboards tracking issuance and denials before GA.
