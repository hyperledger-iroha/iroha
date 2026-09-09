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
| `POST` | `/token` | `X-Iroha-Operator-{Public-Key,Timestamp-Ms,Nonce,Signature}`, `X-SoraFS-Client`, `X-SoraFS-Nonce`, signed manifest envelope | Issues per-peer stream token with TTL & rate limits. The operator signature binds the exact network and complete request. |

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
- An already expired token returns `401`; authenticated request, byte-rate or
  concurrency exhaustion returns `429` with its specific reason. Admission or
  current-custody unavailability returns `503` and `Retry-After: 1`.
- CAR and chunk leases stay owned by physical storage work and then by the
  application response body until EOF, error or cancellation. Cancelling an HTTP
  waiter does not release a lease while its storage worker is still running.
  Query/heavy permits cover the actual admission and storage operation; response
  bodies retain the separate bounded cleanup ticket.
- The exact accepted lease expiry is the earlier of signed token expiry and
  `validated_at_unix_ms + qualified lease_ttl_ms`. Body production checks that
  authenticated deadline and the fixed monotonic deadline anchored before
  admission. Host-clock rollback before the validation timestamp fails closed.
  Expiry and shutdown wake a previously polled body and stop subsequent frames.
  Expiry detected before response production returns `503`; after headers, it
  terminates the body with an error. The full response must not be reported as
  successfully delivered when its body terminates early.
- This ownership boundary covers application body consumption. It cannot retract
  bytes already passed to Hyper or the socket. An unpolled body conservatively
  holds its cleanup ticket until the HTTP owner drops it. Physical worker
  completion, cleanup acknowledgement, transport buffering and deployment
  throughput are distinct measurements; these source rules do not qualify a
  hard real-time network deadline or finite arbitrary-provider shutdown.

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

- **Configuration and custody.** Use the complete
  [public-pin template](sorafs/snippets/stream_token_hardware_binding.toml) and
  [hardware custody contract](sorafs/stream_token_hardware_custody.md). The nested
  `hardware` group requires signer, independent attester and independent observer
  pins; `hardware.key_revision` supplies the sole checked token generation.
  There is no environment-variable enablement or signing-seed path. The key is
  generated inside qualified hardware and must never be exportable or previously
  exported. Credentials and sessions remain runtime-only.
- **Startup and issuance.** Separate hardware and observer clients and an independent
  approved full custody anchor are mandatory. Fresh signed current observations
  bind the provider-scoped identity, phase, challenge and finalized floor; the
  issuer retains the exact body association privately. Completed observations
  additionally bind the exact body, original custody, durable operation and receipt.
  Only final `BeforeRelease` evidence, local Core finality and current
  expiry checks allow a token to leave. An ambiguous Sign permits one read-only
  recovery of the retained operation and never another Sign.
- **Distribution and pinning.** Each provider descriptor pins one strong Ed25519
  public key from authenticated deployment inventory and verifies its token before
  HTTP. Compare `X-SoraFS-Verifying-Key` with that approved key; a key returned
  beside a token is not an independent trust anchor.
- **Rotation and audit.** Follow the contract's governed hardware activation and
  terminal revocation fences. Switch the descriptor's `gateway-key` and token
  atomically. Preserve public fingerprints, generations and approved custody/policy
  digests; do not retain key material or invent a multi-key fallback.

The source contract and signed simulations do not qualify physical devices,
authoritative durable state or genuinely current observers. Those deployment
qualifications and coherent native execution remain required.

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
  `POST /v1/sorafs/storage/token`. It requires one fresh exact-network operator
  signature over the complete canonical request. `X-SoraFS-Client` is only a
  diagnostic label and `X-SoraFS-Nonce` is only an echoed correlation value;
  neither authenticates the caller.
- **CORS preflight.** When CORS is enabled, a catalog-declared `OPTIONS`
  preflight may complete without signature headers; it performs no manifest
  lookup, quota reservation, or token issuance. The actual `POST` still
  requires all four `X-Iroha-Operator-*` headers. Browser deployments must
  explicitly allow those headers, `X-SoraFS-Client`, `X-SoraFS-Nonce`, and
  `Content-Type` in `torii.cors.allowed_headers`.
- **Request flow.**
  1. Client signs and submits the exact request plus the two SoraFS diagnostic
     headers and JSON containing
     `manifest_id_hex`, `provider_id_hex`, and any approved TTL, stream,
     byte-rate, or issuance-quota overrides.
  2. Gateway verifies signature freshness, exact NetworkId, and replay nonce,
     derives a domain-separated opaque quota subject from the authenticated
     operator key, resolves the manifest from local storage, and applies its
     configured issuance quota. Rotating
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
     `X-SoraFS-Issuance-Quota-Remaining` reports the authenticated operator's
     remaining 60-second issuance allowance; exhaustion returns `429` plus
     `Retry-After`.
- **Telemetry.** Gateway records issuance metrics:
  - `sorafs_gateway_token_issuance_total{client,result}`
  - `sorafs_gateway_token_issuance_latency_ms_bucket`
  - `sorafs_gateway_token_denials_total{reason}`
- **Abuse protection.** Treat the returned base64 stream token as a bearer
  credential; operator request signatures are fresh and replay-protected.
  Do not log provider descriptors without redacting `stream-token`. Clients
  exceeding their issuance budget receive `429`; the operator-signature replay
  cache rejects a reused nonce inside the accepted freshness window.

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
