## Norito-RPC Transport (NRPC-1)

Status: Accepted 2026-03-21  
Owners: Torii Platform, SDK Program Lead  
Roadmap reference: NRPC-1 -- Norito-RPC transport RFC publication  
Canonical specification: `docs/source/torii/nrpc_spec.md`

### 1. Goals & Scope

JSON `/v2/pipeline` routes. Every message is framed with the Norito header so
clients get deterministic layouts, schema hashing, and CRC protection by
default. This RFC specifies the wire contract, negotiated encodings, control
parity helpers without reverse-engineering Torii internals.

The scope covers synchronous HTTP request/response exchanges. Streaming and
relay tunnelling reuse the same Norito frame format; the SNNet-11 SoraNet
bridge now ships with blinded channel ids, padding, and GAR category rules
documented alongside the streaming codec (`norito_streaming.md`) and
SoraNet handshake guide (`docs/source/soranet_handshake.md`).

### 2. Transport Summary

- **Endpoint**: identical host/port as `/v2/pipeline`; TLS requirements and
  reverse-proxy guidance remain unchanged.
- **HTTP version**: Torii accepts HTTP/1.1 and HTTP/2. The `Content-Type`
  determines whether a request is treated as Norito-RPC.
- **Authentication**: Bearer tokens travel via the standard `Authorization`
  header. When present, `X-API-Token` continues to override the rate-limit
  identity (`crates/iroha_torii/src/limits.rs:120`).
- **Connection gating**: Pre-auth permits and telemetry counters track Norito
  sessions via `ConnScheme::NoritoRpc`, which is selected whenever the inbound
  request advertises `Content-Type: application/x-norito`
  (`crates/iroha_torii/src/lib.rs:714`).

### 3. Media Types & Negotiation

- **Requests** MUST set `Content-Type: application/x-norito`. Torii will attempt
  to decode invalid payloads as Norito first, falling back to Norito-backed JSON
  only if no `Content-Type` is provided (`crates/iroha_torii/src/utils.rs:576`).
- **Responses** honour the `Accept` header via
  `negotiate_response_format` (`crates/iroha_torii/src/utils.rs:63`):
  - If `application/x-norito` (optionally with parameters) is present with the
    highest quality factor, Torii replies with Norito bytes.
  - Otherwise Torii falls back to Norito JSON (`application/json`) so clients
    without binary support can interoperate.
- Media type parameters are ignored during negotiation. JSON callers may send
  `application/json`, `text/json`, or any `application/*+json` media type.
- Unsupported media types yield `415 Unsupported Media Type` with a plain-text
  explanation.

### 4. Norito Envelope

Each payload begins with the Norito header defined in
`crates/norito/src/core.rs`. The header is 39 bytes long and encodes integrity
and layout metadata as shown below:

| Offset | Field        | Type     | Notes                                     |
| ------ | ----------- | -------- | ----------------------------------------- |
| 0      | Magic       | `[u8;4]` | ASCII `NRT0`; rejects malformed payloads. |
| 4      | Major       | `u8`     | Currently `1`; mismatches return 400.     |
| 5      | Minor       | `u8`     | Bitmask of negotiated layout flags.       |
| 6      | Schema hash | `[u8;16]`| Deterministic hash of the message type.   |
| 22     | Compression | `u8`     | `0 = none`, `1 = zstd`.                   |
| 23     | Length      | `u64`    | Uncompressed payload length in bytes.     |
| 31     | CRC64       | `u64`    | CRC64-XZ (ECMA polynomial, reflected, init/xor all ones) over the payload. |
| 39     | Flags       | `u8`     | Layout flags: packed sequences, compact lengths, field bitset (`crates/norito/src/core.rs:2460`,`crates/norito/src/core.rs:2474`,`crates/norito/src/core.rs:267`). |

Torii validates the header before decoding:

- The checksum must match the payload bytes. A mismatch raises
  `checksum mismatch`.
- Unsupported compression values or layout flags produce a
  `415 Unsupported Media Type` equivalent.
- Schema hashes are compared against the expected response/request type; a
  mismatch yields `400 Bad Request`.

Compression is optional; clients may send uncompressed payloads and indicate
`Compression = 0`. When `Compression = 1` the payload must be zstd compressed;
Torii re-computes the checksum after decompression.

### 5. Request Semantics

1. Encode the request body using `norito::to_bytes(&payload)` or the equivalent
   helper in the target language.
2. POST (or PUT) to the desired Torii route with:
   - `Content-Type: application/x-norito`
   - `Accept: application/x-norito` (optional but recommended for binary
     responses)
   - `Authorization: Bearer <token>` if the endpoint requires auth.
3. Optional query parameters follow standard URL encoding. Torii decodes them
   into strongly typed Norito models using `NoritoQuery`
  (`crates/iroha_torii/src/utils.rs:705`).
4. Some routes accept versioned envelopes. When the route uses
   `NoritoVersioned<T>`, Torii first attempts to decode the incoming bytes using
   `iroha_version` framing before falling back to plain Norito
  (`crates/iroha_torii/src/utils.rs:380`). Clients should prefer the versioned
   framing when invoking versioned data-model types (e.g., transactions and
   blocks) so future schema upgrades can be negotiated.

Example submission (payload prepared separately):

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v2/transactions/submit
```

### 6. Response Semantics

- Successful responses honour the negotiated format and reuse the matching
  schema hash so clients can decode into the same data-model types they would
  receive over `/v2/pipeline`.
- Plain-text errors (status `4xx`/`5xx`) are emitted when extraction fails
  before Torii reaches a typed handler, e.g. "invalid Norito body: checksum
  mismatch". These responses use UTF-8 text and the default Axum error content
  type.
- Structured error payloads returned by handlers (e.g., business-logic failures)
  participate in the same negotiation as successful responses and are Norito
  encoded when permitted by `Accept`.
- When a handler emits a dynamic JSON value (`NoritoJsonBody`), the
  `application/x-norito` response wraps a UTF-8 JSON string in a Norito frame
  (schema hash for `String`). Decode the Norito string, then parse JSON. The
  `application/json` form remains a plain JSON object/array.

### 7. Message Catalogue

| Group | Representative routes | Request types | Response shape | Notes |
| --- | --- | --- | --- | --- |
| Signed queries | `POST /query` (aliased by the pipeline router) | `SignedQuery` via `NoritoVersioned`[`crates/iroha_torii/src/lib.rs:5387`](/crates/iroha_torii/src/lib.rs#L5387) | Norito or JSON `QueryResultBox` depending on `Accept` | The handler streams results through `handle_queries`/`handle_queries_with_opts`, honouring pagination and cursor overrides (`crates/iroha_torii/src/routing.rs:9472`). |
| Contracts & verifying keys | `POST /v2/contracts/{code,deploy,instance,*}`; `POST /v2/zk/vk/{register,update,deprecate}` | `RegisterContractCodeDto`, `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`, `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`[`crates/iroha_torii/src/routing.rs:3307`](/crates/iroha_torii/src/routing.rs#L3307)[`crates/iroha_torii/src/routing.rs:4336`](/crates/iroha_torii/src/routing.rs#L4336) | Norito acknowledgement envelope mirroring `/v2/pipeline/transactions` status fields | Each DTO is decoded through `NoritoJson<T>` so callers may send Norito or Norito-backed JSON. Successful calls enqueue a signed transaction via `handle_transaction_with_metrics`. |
| ZK proof orchestration | `POST /v2/zk/{roots,verify,submit-proof,vote/tally}`; `GET /v2/zk/proofs{,/count}` | `ZkRootsGetRequestDto`, `ZkVoteGetTallyRequestDto`, batch envelopes for proof submission[`crates/iroha_torii/src/routing.rs:2441`](/crates/iroha_torii/src/routing.rs#L2441)[`crates/iroha_torii/src/routing.rs:2497`](/crates/iroha_torii/src/routing.rs#L2497) | Norito structs (`ProofRootsResponse`, `ProofVoteTallyDto`, etc.) selected by `Accept` | NORITO requests reject malformed payloads before reaching business logic (`crates/iroha_torii/src/zk_prover.rs:985`), preserving deterministic proof verification. |
| SoraFS storage APIs | `/v2/sorafs/{pin,capacity,deal,replication,por}/*` | `RegisterPinManifestDto`, `RegisterCapacityDeclarationDto`, `RecordDealUsageDto`, `RecordPorChallengeDto`, `RecordPorProofDto`, etc.[`crates/iroha_torii/src/routing.rs:5624`](/crates/iroha_torii/src/routing.rs#L5624)[`crates/iroha_torii/src/routing.rs:6385`](/crates/iroha_torii/src/routing.rs#L6385) | Norito acknowledgements with policy enforcement metadata | All admissions run through shared helpers that emit deterministic telemetry and queue transactions when on-chain mutations are required. |
| Confidential ledger helpers | `POST /v2/confidential/derive-keyset` | `ConfidentialKeyRequest`[`crates/iroha_torii/src/routing.rs:1328`](/crates/iroha_torii/src/routing.rs#L1328) | Norito `ConfidentialKeyResponse` containing derived key material | Input seeds accept hex or base64 encodings; invalid lengths are rejected before key derivation. |
| Connect pairing | `POST /v2/connect/session` (plus `DELETE/GET` companions) | `ConnectSessionRequest`[`crates/iroha_torii/src/routing.rs:1613`](/crates/iroha_torii/src/routing.rs#L1613) | Norito `ConnectSessionResponse` / status DTOs | Sessions inherit the same Norito negotiation so wallets and dApps can reuse binary codecs while bridging to WebSocket upgrades. |

The catalogue above captures every handler that extracts or emits `NoritoJson`/`NoritoVersioned` payloads today. JSON callers continue to function via the same surfaces, but SDKs should default to the Norito form to gain schema hashing and stricter validation.

### 8. Error Semantics

- **Decode & negotiation failures.** Schema mismatches, CRC failures, or unsupported layout bits trigger `400 Bad Request` or `415 Unsupported Media Type` before business logic runs. The response body is a UTF-8 diagnostic string emitted by the Norito extractor so clients can surface actionable errors.
- **Validation and permission errors.** Domain-level failures propagate through `Error::Query` and reuse the `ValidationFail` → status mapping in `Error::query_status_code`[`crates/iroha_torii/src/lib.rs:9647`](/crates/iroha_torii/src/lib.rs#L9647). Examples: `NotPermitted` → `403`, `Find(_)` → `404`, `Expired` → `410`, `CapacityLimit` → `429`.
- **Queue backpressure.** Admission failures bubble through `Error::PushIntoQueue`, which selects status codes (`429`, `400`, `409`, `500`) via `status_code_for_queue_error`[`crates/iroha_torii/src/lib.rs:9717`](/crates/iroha_torii/src/lib.rs#L9717) and returns a Norito `QueueErrorEnvelope` containing queue depth, capacity, and saturation flags[`crates/iroha_torii/src/lib.rs:9730`](/crates/iroha_torii/src/lib.rs#L9730). Applicable responses include `X-Iroha-Queue-Depth`, `X-Iroha-Queue-Capacity`, `X-Iroha-Queue-State`, and a `Retry-After: 1` hint for transient saturation.
- **Generic envelopes.** All other errors are serialized as Norito `ErrorEnvelope { code, message }`[`crates/iroha_torii/src/lib.rs:8445`](/crates/iroha_torii/src/lib.rs#L8445). Clients should surface the machine-readable `code` first (`queue_full`, `already_enqueued`, etc.) and fall back to the message for human diagnostics.
- **Plain-text fallbacks.** When extraction fails before a typed handler is selected, Axum returns a plain-string body (e.g., “invalid Norito body: checksum mismatch”). SDKs SHOULD treat unknown content types as fatal and relay the payload verbatim to avoid swallowing diagnostics.

### 9. Control Channels & Observability

- **Pre-auth gate**: Every request consumes a permit from the pre-auth limiter.
  Permits are accounted per scheme (`norito_rpc`) for telemetry and rate-limit
  tuning (`crates/iroha_torii/src/lib.rs:746`).
- **Rate limiting**: The limiter key prefers `X-API-Token` and then the remote
  IP, exactly like JSON requests (`crates/iroha_torii/src/limits.rs:120`). Norito
  callers should continue supplying `X-API-Token` where available to avoid
  aggregated anonymous buckets.
- **Tracing**: The middleware populates `trace_id` and `span_id` fields
  uniformly across transports; Norito-RPC does not change the tracing surface.
- **Metrics**: Telemetry exports counters with the `scheme="norito_rpc"` label
  for active connections and pre-auth rejections. Dashboards already break down
  throughput using this label.
- **Configuration**: Operators control the rollout via `torii.transport.norito_rpc`
  in `config.toml`. The block exposes `enabled`, `require_mtls`, `stage`
  (`disabled`, `canary`, `ga`), and `allowed_clients` (used during canary).
  When `stage = "canary"`, Torii only accepts Norito-RPC calls that include an
  `X-API-Token` header present in the allowlist. See
  `docs/source/config/client_api.md` for a detailed reference.
- **Control endpoints**: `/rpc/capabilities` and `/rpc/ping` are public,
  cacheable helpers that report the active Norito-RPC configuration and a simple
  liveness timestamp. SDKs should hit `/rpc/capabilities` during transport
  negotiation to honour `stage` and `require_mtls`, and `/rpc/ping` before
  switching transports in health checks.

### 10. Coexistence With `/v2/pipeline`

- Every Norito-enabled endpoint shares its path with the JSON pipeline. Clients
  select the transport by setting `Content-Type` and `Accept`.
- JSON callers continue to work unchanged; Torii uses the Norito JSON codec
  to ensure deterministic layouts even over the JSON interface.
- Norito-RPC does **not** introduce new routes. Feature parity is enforced via
  CI fixtures that run the same request bodies across both transports (see
  `python/iroha_python/scripts/run_norito_rpc_smoke.sh`).
- SDKs should expose helpers that map existing high-level calls onto the Norito
  transport so call-sites can opt in without duplicating route knowledge.

### 11. Client Support Matrix

- **Rust**: `iroha_client::client::Client` auto-negotiates Norito responses when
  the caller sets `Accept: application/x-norito`, returning native data-model
  types.
- **Python**: `NoritoRpcClient` wraps the requests session and injects the
  Norito headers automatically (`python/iroha_python/src/iroha_python/norito_rpc.py:32`).
- **Android**: `NoritoRpcClient` and `NoritoRpcRequestOptions` mirror the Python
  helper to keep mobile parity (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcClient.java:1`).
- **JavaScript**: The JS SDK reuses the Torii REST stack but can switch to
  binary responses by setting the `Accept` header. Native Norito encoding helpers
  will land with NRPC-3.

### 13. Future Work

- Document streaming (`norito-stream`) and relay encapsulation (SNNet-11).
- Automate OpenAPI-to-Norito fixture generation (`cargo xtask openapi`) to keep
  the schema catalogue in sync with Torii releases.

### 14. Stakeholder Brief & Distribution

- **Torii Platform (NRPC-2 owners)**: review the transport RFC in the 2026-03-21 platform sync. Agenda circulated via `docs/source/torii/norito_rpc_sync_notes.md`, covering ingress/auth/telemetry rollout requirements and open questions on hybrid streaming.
- **SDK Program (Swift, JS, Python, Android leads)**: consume the summary deck at `docs/source/torii/norito_rpc_brief.md` and log adoption checkpoints (client helpers, fixture parity) before the April SDK alignment review. Shared action item tracker lives in `docs/source/torii/norito_rpc_tracker.md`.
- **Docs/DevRel**: integrate the RFC highlights into the developer portal (`docs/portal/docs/devportal/torii-rpc-overview.md`) and refresh the Try-It console guidance to include Norito payload examples by 2026-03-25.
- **Status Broadcasting**: status.md entry references this RFC so weekly triage sees NRPC-1 closed and downstream follow-ups assigned.
- **Observability**: implement the Norito-RPC telemetry and alert suite described in `docs/source/torii/norito_rpc_telemetry.md`, landing dashboards/alerts ahead of production rollout.

### 15. Appendix -- Header Validation Reference

The `norito::core::Header` implementation (`crates/norito/src/core.rs:2632`)
performs the following checks that clients should mirror in diagnostics:

1. Verify magic bytes (`NRT0`).
2. Reject unexpected major versions.
3. Ensure all minor-version feature bits are supported by the current build.
4. Parse compression and ensure it is in the supported set (`None`, `Zstd`).
5. Validate payload length and checksum.
6. Apply layout flags; unsupported bits raise `Unsupported feature: layout flag`.

Error variants map onto human-readable strings, making it easier to bubble
decode failures up to CLI tools and SDKs.
