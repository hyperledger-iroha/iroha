---
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
| `POST` | `/token` | `X-API-Token`, `X-SoraFS-Client`, `X-SoraFS-Nonce`, signed manifest envelope | Issues per-peer stream token with TTL & rate limits. |

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

- **Configuration and custody.** Enable issuance only in the node TOML and pin
  the runtime signer by a non-secret handle and its exact Ed25519 public key:

  ```toml
  [sorafs.storage.stream_tokens]
  enabled = true
  signer_handle = "pkcs11:prod/stream-token/v4"
  signer_public_key_hex = "<64-lowercase-hex-characters>"
  signer_revision = 4
  signer_policy_digest_hex = "<64-lowercase-nonzero-hex-characters>"
  admission_provider_handle = "sealed-cas:prod/stream-token/admission/v1"
  admission_provider_revision = 7
  admission_provider_policy_digest_hex = "<64-lowercase-nonzero-hex-characters>"
  key_version = 4
  ```

  There is no environment-variable enablement or signing-seed path. The private
  key remains encrypted and runtime-only in the external software signer, and
  its authenticated session is supplied only to the runtime-injected adapter. It must never
  appear in configuration, files, logs, or readiness artefacts.
- **Startup binding.** An enabled issuer requires all four configured public
  signer fields and an injected signer. Startup probes twice and fails closed
  unless the adapter reports the exact configured handle, public key, non-zero
  revision, and non-zero public-policy digest without drift. It also rejects
  malformed or weak Ed25519 keys and stale, substituted, revoked, or test-marked
  providers. A handle is an identifier, not a place to embed credentials.
  Disabling issuance in TOML does not permit an injected signer to activate it.
- **Signing boundary.** Torii revalidates all four public identity fields before
  and after sending the canonical domain-separated payload to the injected
  signer and accepts only a raw 64-byte Ed25519 signature. It assembles
  `StreamTokenV1` and strictly verifies the returned signature against
  `signer_public_key_hex` before releasing the token. Qualification drift, an
  unavailable/refusing signer, or any malformed, wrong-key, or non-verifying
  output fails closed and must produce only a bounded, payload-free failure
  class.
- **Distribution.** Orchestrators receive the corresponding 32-byte public key
  through authenticated provider deployment inventory and pass its 64-character
  hex encoding as `gateway-key`. The issuance response also reports
  `X-SoraFS-Verifying-Key`, but a key delivered beside the token it verifies is
  not a trust anchor; compare that header with the approved inventory value.
- **Pinning.** Each provider descriptor pins exactly one key. The client rejects
  malformed/weak Ed25519 keys and verifies the token before making an HTTP
  request. It never falls back to a key embedded in an untrusted response.
- **Rotation.** Create the replacement key inside the independently administered software signer without
  exporting it. In one controlled rollout, inject the adapter for its new
  non-secret handle, update `signer_handle`, `signer_public_key_hex`,
  `signer_revision`, `signer_policy_digest_hex`, and `key_version`, and restart
  the issuer. Require both startup qualification probes and a
  strictly verified probe token before publishing the new public key through
  authenticated inventory. Atomically deploy a matching `gateway-key` and token.
  For overlap, use separately named old/new provider descriptors; remove the old
  descriptor by its final token expiry, then revoke the old software-signing key. There
  is no implicit multi-key acceptance window or path-based fallback.
- **Audit trail.** Record old/new public-key fingerprints, key versions,
  non-secret signer handles, activation and final-expiry times, approver
  identity, and negative-test evidence showing that old-key, cross-key, and
  wrong-handle tokens fail after cutover. Never record signing material or
  signer credentials.

## Canonical Token Schema

- **Wire format.** `StreamTokenV1` is canonical Norito binary transported as
  standard padded base64. It contains a `body: StreamTokenBodyV1` and a 64-byte
  Ed25519 `signature`; JSON returned by the issuance endpoint is only a
  diagnostic projection plus the canonical `encoded` token.
- **Field set.** The signed body contains `token_id`, `manifest_cid`,
  `provider_id`, `profile_handle`, `max_streams`, `ttl_epoch`,
  `rate_limit_bytes`, `issued_at`, `requests_per_minute`, and
  `token_pk_version`.
- **Signature input.** Sign exactly
  `b"sorafs.stream-token.signature.v1\0" || norito::to_bytes(body)`. The NUL is
  part of the domain separator. Signing the body bytes alone, signing a JSON
  projection, adding a length prefix, or using another SoraFS signature domain
  produces an invalid token.
- **Strict validation.** Clients reject non-Norito or oversized tokens,
  malformed/weak keys and signatures, body-only legacy signatures,
  `issued_at > ttl_epoch`, expired tokens, issuance more than 60 seconds in the
  future, empty/oversized identifiers and CIDs, zero stream capacity, and any
  provider/profile/manifest binding mismatch.
- **Scoreboard alignment.** Orchestrator scoreboard ingests the above fields directly, mapping `max_streams`, `ttl_epoch`, and `rate_limit_bytes` into availability and penalty factors. Additional scoreboard signals (e.g., token health) derive from issuance telemetry using `token_id`.
- **Validation helpers.** Use `sorafs_manifest::{StreamTokenBodyV1,
  StreamTokenV1}` to construct the canonical signing payload, assemble the
  externally returned signature, and verify the result. Do not implement a
  second token codec or signature preimage in clients or signer adapters.

## Secure Token Issuance API

- **Authentication.** The canonical route is
  `POST /v1/sorafs/storage/token`. It always requires exactly one valid
  `X-API-Token` from `torii.api_tokens`, independently of the listener-wide
  `torii.require_api_token` setting. `X-SoraFS-Client` is only a diagnostic
  label and `X-SoraFS-Nonce` is only an echoed correlation value; neither
  authenticates the caller.
- **CORS preflight.** When CORS is enabled, a catalog-declared `OPTIONS`
  preflight may complete without `X-API-Token`; it performs no manifest lookup,
  quota reservation, or token issuance. The actual `POST` still requires the
  credential. Browser deployments must explicitly allow `X-API-Token`,
  `X-SoraFS-Client`, `X-SoraFS-Nonce`, and `Content-Type` in
  `torii.cors.allowed_headers`.
- **Request flow.**
  1. Client submits the three required headers and JSON containing
     `manifest_id_hex`, `provider_id_hex`, and any approved TTL, stream,
     byte-rate, or issuance-quota overrides.
  2. Gateway authenticates `X-API-Token`, derives a domain-separated opaque
     quota subject from that credential, resolves the manifest from local
     storage, and applies its configured issuance quota. Rotating
     `X-SoraFS-Client` labels does not create a fresh budget.
  3. Gateway mints and domain-separates a token, then returns JSON containing
     `token.body`, `token.signature_hex`, `token.encoded`, and
     `token_base64`. The last two values are the canonical header token.

     ```json
     {
       "token": {
         "body": { "token_pk_version": 4 },
         "signature_hex": "...",
         "encoded": "..."
       },
       "token_base64": "..."
     }
     ```

  4. Response headers include `X-SoraFS-Token-Id`,
     `X-SoraFS-Verifying-Key`, `X-SoraFS-Issuance-Quota-Remaining`, and the echoed
     nonce/client identifiers. `Cache-Control: no-store` is mandatory.
     `X-SoraFS-Issuance-Quota-Remaining` reports the authenticated credential's
     remaining 60-second issuance allowance; exhaustion returns `429` plus
     `Retry-After`.
- **Telemetry.** Gateway records issuance metrics:
  - `sorafs_gateway_token_issuance_total{client,result}`
  - `sorafs_gateway_token_issuance_latency_ms_bucket`
  - `sorafs_gateway_token_denials_total{reason}`
- **Abuse protection.** Treat both the Torii API token and returned base64
  stream token as bearer credentials.
  Do not log provider descriptors without redacting `stream-token`. Clients
  exceeding their issuance budget receive `429`; nonce uniqueness must be
  enforced by the authenticated perimeter until the route has a durable replay
  ledger.

## Documentation & Rollout

- **Protocol documentation.** Expand `specs/sorafs_node_client_protocol.md` with:
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
