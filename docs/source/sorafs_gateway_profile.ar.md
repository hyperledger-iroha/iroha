---
lang: ar
direction: rtl
source: docs/source/sorafs_gateway_profile.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 065a894f4a420fd361477390c9f494b86f6cfe0d313caba761aba77b6de2444f
source_last_modified: "2026-01-04T10:50:53.673706+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway Trustless Profile
summary: Normative profile for trustless HTTP delivery of SoraFS objects.
---

# SoraFS Gateway Trustless Profile (Draft)

This document specifies the **trustless delivery profile** required for
SoraFS gateways and clients that participate in the SF-5 rollout. It captures
the HTTP request/response matrix, proof formats, and verification rules needed
to stream CAR objects deterministically over untrusted transport. Future
iterations of the conformance suite, load tests, and self-certification tooling
will follow the definitions here.

> **Status:** Draft. Feedback should target the Networking TL and QA Guild.

## 1. Scope

The profile applies to HTTP/1.1 and HTTP/2 HTTPS endpoints broadcasting SoraFS
chunks encoded as CARv2. Gateways MAY offer other APIs, but **every** endpoint
covered here MUST adhere to the normative behaviour so clients can safely
interleave multiple providers without out-of-band trust.

## 2. Request Matrix

| Method | Path Pattern                    | Description                                | Required Headers                            |
|--------|---------------------------------|--------------------------------------------|---------------------------------------------|
| `GET`  | `/car/{manifest_cid}`           | Full-object retrieval                      | `Accept: application/vnd.ipld.car; dag-scope=full` |
| `GET`  | `/car/{manifest_cid}`           | Range retrieval (byte ranges)              | `Accept: application/vnd.ipld.car; dag-scope=block`, `Range` |
| `HEAD` | `/car/{manifest_cid}`           | Metadata probe                             | `Accept: application/vnd.ipld.car`           |
| `GET`  | `/chunk/{chunk_digest}`         | Single chunk retrieval                     | `Accept: application/octet-stream`, `X-SoraFS-Chunk-Index` |
| `GET`  | `/proof/{manifest_cid}`         | PoR proof payload (`PorProofV1`)           | `Accept: application/json`                   |

### 2.1. Required Request Headers

* `X-SoraFS-Version`: `sf1` for the initial rollout, bump on future profiles.
* `X-SoraFS-Client`: opaque identifier (useful for telemetry). OPTIONAL but
  **RECOMMENDED**.
* `X-SoraFS-Nonce`: 32-byte hex nonce echoed in the response signature to prevent replay.

Clients MUST attach the Norito-signed manifest envelope (`manifest_signatures.json`)
using `X-SoraFS-Manifest-Envelope` when requesting a full CAR to allow gateways to
perform policy checks (GAR, PDP/PoR status).

## 3. Response Requirements

### 3.1. Common Headers

All successful responses (2xx) MUST include:

* `Content-Type`: `application/vnd.ipld.car` for CAR streams, `application/json`
  for proof bundles, `application/octet-stream` for raw chunks.
* `X-SoraFS-Nonce`: echo of the request nonce.
* `X-SoraFS-Chunker`: canonical chunker handle (`sorafs.sf1@1.0.0`).
* `X-SoraFS-Proof-Digest`: hex digest of the accompanying PoR proof payload
  (`PorProofV1.proof_digest`, BLAKE3-256 over the proof fields excluding the signature).
* `X-SoraFS-PoR-Root`: hex digest of the PoR Merkle root for the requested scope.

Gateways MUST fail requests with `428 Precondition Required` when the manifest
envelope is missing or does not match the locally cached admission record.

### 3.2. Range Support

* Implement byte ranges (`Range: bytes=start-end`). The response MUST include
  `Content-Range` and align chunks to the CAR block boundaries advertised in the
  manifest’s chunk plan.
* Enforce the deterministic `dag-scope` semantics:
  * `full`: entire DAG rooted at `manifest_cid`.
  * `block`: subset containing the requested byte range, plus parent blocks
    required for validation.
* Status codes: full CAR responses return `200 OK`; aligned range responses
  return `206 Partial Content` with `Content-Range`, while invalid ranges
  surface `416 Range Not Satisfiable`.

### 3.3. Conditional Requests

* Support `If-None-Match` with the manifest digest.
* Respond with `304 Not Modified` when the provided digest matches the current
  manifest to allow clients to skip redundant downloads.

## 4. Proof Formats

### 4.1. CAR Integrity

* Each CAR response MUST contain a Norito-encoded proof envelope with:
  * `manifest_digest`: BLAKE3-256 of `ManifestV1`.
  * `chunk_plan_digest`: SHA3-256 of the ordered chunk metadata.
  * `range_proof`: optional list of chunk indices covered by the response.

Clients MUST verify:

1. BLAKE3 digest of streamed bytes equals `manifest_digest`.
2. The BLAKE3 proof decompresses to the chunk plan recorded in the manifest.
3. CAR sections align with the chunk plan offsets.

### 4.2. Proof-of-Retrievability (PoR)

* Gateways must serve `GET /proof/{manifest_cid}` returning a Norito JSON `PorProofV1` payload:
  * `manifest_digest`, `provider_id`, `samples[]`, `auth_path`, `signature`, `submitted_at`.
  * `auth_path` is the ordered list of chunk roots for the PoR tree.
  * `signature` is Ed25519 over `proof_digest` (BLAKE3-256 of the proof fields excluding
    the signature), and `signature.public_key` is the gateway's Ed25519 public key.
* The signing key is configured at `torii.sorafs.storage.stream_tokens.signing_key_path`
  and is shared with stream token issuance.

Clients MUST verify the signature and ensure the public key matches the provider's
admitted gateway key before trusting the `X-SoraFS-PoR-Root` header.

### 4.3. Streaming Receipts

When chunk streaming completes successfully, gateways SHOULD emit a signed receipt:

```json
{
  "version": 1,
  "provider_id": "hex",
  "manifest_digest": "hex",
  "range_start": 0,
  "range_end": 1048575,
  "duration_ms": 1200,
  "bytes_sent": 1048576,
  "por_root": "hex",
  "nonce": "hex",
  "signature": "hex"
}
```

Receipts enable PoTR-Lite deadline proofs (SF-14). The exact schema will stabilise
in a follow-up iteration.

## 5. Negative Behaviour

Gateways MUST return deterministic error codes for refusal paths:

| Condition                                            | Status | Body                                                     |
|------------------------------------------------------|--------|----------------------------------------------------------|
| Unsupported chunker profile                          | 406    | `{ "error": "unsupported_chunker", "handle": "..." }`    |
| Manifest envelope missing/invalid                    | 428    | `{ "error": "manifest_envelope_required" }`              |
| Manifest not admitted / admission registry unavailable | 412  | `{ "error": "provider_not_admitted" \| "admission_unavailable" }` |
| Missing provider identifier for admission/capability | 428    | `{ "error": "provider_id_missing" }`                     |
| Proof verification failure                           | 422    | `{ "error": "proof_verification_failed" }`               |
| Downgrade attempt (missing headers)                  | 428    | `{ "error": "required_headers_missing" }`                |
| Rate limit / GAR policy violation                    | 429    | `{ "error": "rate_limited", "reason": "..." }`           |
| Denylist (provider/manifest/CID/perceptual family)   | 451    | `{ "error": "denylisted", "kind": "..." }`               |
| Internal errors                                      | 500    | `{ "error": "internal", "request_id": "..." }`           |

Clients MUST treat any non-2xx response as a refusal and exclude the gateway
from the multi-source schedule until an operator review occurs.

### 5.1. Policy Matrix Surface (Fixtures)

The conformance fixtures under `fixtures/sorafs_gateway/1.0.0/scenarios.json`
exercise the canonical status matrix:

- Success paths: `200` (full CAR) and `206` (aligned range replay).
- Header/manifest enforcement: `428` for missing manifest envelopes or required
  headers (`B2`), `412` for admission failures (`B5`).
- GAR/denylist enforcement: `451` for governance or compliance refusals (`D1`).

Capability refusal fixtures (`fixtures/sorafs_gateway/capability_refusal`) mirror
the same codes and are used by SDK and gateway self-certification suites to
guard against drift.

## 6. Telemetry Expectations

Gateways SHOULD emit the following Prometheus metrics and HTTP headers:

| Metric / Header                | Description                                         |
|--------------------------------|-----------------------------------------------------|
| `sorafs_gateway_requests_total{result}` | Success/err buckets                            |
| `sorafs_gateway_range_bytes_total`      | Bytes served per chunker handle                |
| `sorafs_gateway_proof_failures_total`   | Count of failed PoR/manifest validations       |
| `sorafs_gateway_latency_ms_bucket`      | Request latency histogram                      |
| `X-SoraFS-Telemetry-Nonce`              | 16-byte nonce correlated with telemetry events |

Telemetry MUST NOT leak user-identifying information. Aggregate per-client
statistics using pseudonymous identifiers derived from the request nonce.

## 7. Conformance Targets

The QA Guild will publish a replay harness that:

1. Replays canonical fixtures (full and ranged CARs) and ensures BLAKE3/PoR checks pass.
2. Issues negative requests (unsupported chunker, corrupted proof) and expects refusal codes.
3. Launches ≥1,000 concurrent range streams with seeded randomness to confirm gateways maintain
   deterministic throughput and proof integrity under load.

Gateways MUST satisfy the harness before onboarding. Operators will self-certify using
a signed attestation containing the conformance run hash, proof receipts, and
gateway build metadata.

## 8. Open Questions

* Finalise receipt signature scheme (Ed25519 vs multi-sig council keys).
* Determine whether chunk-range endpoints require GREASE capabilities.
* Align PoR sampling defaults with SF-13 PDP upgrades.
* Evaluate feasibility of HTTP/3 support within the same profile or publish a
  dedicated QUIC supplement.

Contributions and feedback are tracked via the SF-5a task in `roadmap.md`.
