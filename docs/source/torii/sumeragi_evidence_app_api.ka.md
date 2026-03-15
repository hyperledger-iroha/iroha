---
lang: ka
direction: ltr
source: docs/source/torii/sumeragi_evidence_app_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fddacf8c904bcd43ca2024ce081595ce33711b079fa2c9e9958a051968e765
source_last_modified: "2026-01-22T14:35:37.756307+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii Sumeragi Evidence & Proof Streaming (TORII-APP-3)

Status: Completed 2026-03-21  
Owners: Torii Platform, Integration Tests WG  
Roadmap reference: TORII-APP-3 — Evidence endpoints & SSE parity

This note documents the `/v2/sumeragi/evidence/*` HTTP surfaces and the proof
signals emitted over `/v2/events/sse`. The handlers already ship in Torii, but
the contract was tracked in the roadmap until we captured the DTO shapes,
filters, and sample payloads for SDK parity.

## Evidence Endpoints

The handlers live in `crates/iroha_torii/src/routing.rs:2863-3099` and are wired
under `Torii::add_sumeragi_routes` when the `app_api` feature is enabled.【crates/iroha_torii/src/routing.rs:2863】【crates/iroha_torii/src/lib.rs:6295】
Requests accept Norito (`application/x-norito`) or JSON payloads via the shared
`NoritoJson` extractor, and responses can be negotiated with the `Accept` header
(`application/x-norito` or `application/json`). Query parameters use standard
URL encoding and are decoded through the Norito JSON codec, so booleans,
integers, and strings are parsed without additional quoting.

### `GET /v2/sumeragi/evidence/count`

- Returns a monotonic count of unique evidence records observed by the node
  during the retention horizon.
- Response body (JSON):

```json
{
  "count": 12
}
```

- Binary parity: `CountResponse` (`norito::derive::NoritoSerialize`) is sent
  when `Accept: application/x-norito` is supplied.【crates/iroha_torii/src/routing.rs:263】【crates/iroha_torii/src/routing.rs:2855】

### `GET /v2/sumeragi/evidence`

Lists recent evidence records from the in-memory snapshot. Supported query
parameters (`EvidenceListQuery`):

| Parameter | Type | Default | Notes |
|-----------|------|---------|-------|
| `limit`   | `usize` | 50 | Clamped to `1..=1000`. |
| `offset`  | `usize` | `0` | Offset into the ordered snapshot. |
| `kind`    | `string` | _none_ | One of `DoublePrepare`, `DoubleCommit`, `InvalidQc`, `InvalidProposal`, `Censorship`. |

Response JSON is a Norito JSON object:

```json5
{
  "total": 4,
  "items": [
    {
      "kind": "DoublePrepare",
      "phase": "Prevote",
      "height": 1024,
      "view": 8,
      "epoch": 0,
      "signer": 3,
      "block_hash_1": "2c5a…",
      "block_hash_2": "8f0d…",
      "recorded_height": 2048,
      "recorded_view": 16,
      "recorded_ms": 1731883656123
    },
    {
      "kind": "InvalidProposal",
      "height": 1050,
      "view": 12,
      "epoch": 0,
      "subject_block_hash": "4de1…",
      "payload_hash": "f10a…",
      "reason": "missing qc payload commitment",
      "recorded_height": 2050,
      "recorded_view": 18,
      "recorded_ms": 1731883657451
    }
  ]
}
```

The keys vary per `EvidenceKind` and mirror the JSON produced by
`evidence_to_json`. When `Accept: application/x-norito` the response is a binary
`EvidenceListWire` payload (`total: u64`, `items: Vec<EvidenceRecord>`).【crates/iroha_torii/src/routing.rs:2915】【crates/iroha_torii/src/routing.rs:2954】

### `POST /v2/sumeragi/evidence/submit`

Submits slashing evidence to the running Sumeragi instance.

- Request body:

```json
{
  "evidence_hex": "0x010000000200…" // hex of Norito-framed `ConsensusEvidence` bytes
}
```

- The handler validates the hex payload, decodes it as `ConsensusEvidence`, and
  forwards it to Sumeragi. Bad hex or invalid payloads result in
  `400 Bad Request` with a Norito `ValidationFail` error.
- Successful submissions return `202 Accepted` with:

```json
{
  "status": "accepted",
  "kind": "DoublePrepare"
}
```

Norito bodies are also accepted (`Content-Type: application/x-norito`).【crates/iroha_torii/src/routing.rs:3078】【crates/iroha_torii/src/routing.rs:3085】

## Proof & Pipeline SSE (`GET /v2/events/sse`)

The SSE handler and DTO live at
`crates/iroha_torii/src/routing.rs:14376-14571` and expose the shared
`EventsSender` broadcast stream.【crates/iroha_torii/src/routing.rs:14376】【crates/iroha_torii/src/lib.rs:6602】

- Endpoint: `GET /v2/events/sse`
- Protocol: `text/event-stream`; each `data:` line is a single JSON document.
- Query: optional `filter` query parameter containing a JSON-encoded Norito
  `FilterExpr`. Standard pipeline filters (`tx_status`, `tx_hash`,
  `block_status`, etc.) are supported, and the handler adds proof-specific
  selectors:
  - `proof_backend` — string equality or `IN` array.
  - `proof_call_hash` — 64 hex characters (decoded to `[u8; 32]`).
  - `proof_envelope_hash` — 64 hex characters.
  - Invalid or unsupported filters return `400 Bad Request`.

Example request filtering proof verifications for a specific backend/call hash:

```sh
curl -N "$TORII/v2/events/sse?filter=$(python3 - <<'PY'
import json, urllib.parse
expr = {
  "op": "and",
  "args": [
    {"op": "eq", "args": ["proof_backend", "halo2/ipa"]},
    {"op": "eq", "args": ["proof_call_hash", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]}
  ]
}
print(urllib.parse.quote(json.dumps(expr)))
PY
)"
```

Sample SSE payloads generated by `event_to_json_value`:

```json5
// Proof verification
{
  "category": "Data",
  "event": "ProofVerified",
  "backend": "halo2/ipa",
  "proof_hash": "33…00",
  "call_hash": "aa…aa",
  "envelope_hash": "10…10",
  "vk_ref": "halo2/ipa::transfer_v3",
  "vk_commitment": "55…55"
}

// Proof rejection
{
  "category": "Data",
  "event": "ProofRejected",
  "backend": "halo2/ipa",
  "proof_hash": "22…00",
  "call_hash": null,
  "envelope_hash": null,
  "vk_ref": null,
  "vk_commitment": null
}
```

Pipeline events (`category: "Pipeline"`) share the same stream. Events that do
not match the supplied filters are dropped. Lagged subscribers receive `: lagged`
comments until they catch up.

## Validation & Tooling

- Integration tests exercise the evidence list/count endpoints and SSE filters:
  - `crates/iroha_torii/tests/sumeragi_evidence_list_endpoint.rs`
  - `crates/iroha_torii/tests/sse_proof_verified_fields.rs`
  - `crates/iroha_torii/tests/sse_proof_rejected_fields.rs`
  - `integration_tests/tests/events/sse_smoke.rs`
- CLI helpers mirror the API (`iroha ops sumeragi evidence list|count|submit`), and
  SDKs consume the same DTOs (`javascript/iroha_js`, `IrohaSwift`, Python).

With the schema captured here the roadmap item moves to `status.md`, and SDK
teams can rely on a stable reference for evidence monitoring and proof event
streaming.
