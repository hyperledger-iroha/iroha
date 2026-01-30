---
lang: ru
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-01-30
---

# Light Client Data Availability Sampling

The Light Client Sampling API allows authenticated operators to retrieve
Merkle-authenticated RBC chunk samples for an in-flight block. Light clients
can issue random sampling requests, verify the returned proofs against the
advertised chunk root, and build confidence that data is available without
fetching the entire payload.

## Endpoint

```
POST /v1/sumeragi/rbc/sample
```

The endpoint requires an `X-API-Token` header matching one of the configured
Torii API tokens. Requests are additionally rate-limited and subject to a daily
per-caller byte budget; exceeding either returns HTTP 429.

### Request Body

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – target block hash in hex.
* `height`, `view` – identifying tuple for the RBC session.
* `count` – desired number of samples (defaults to 1, capped by configuration).
* `seed` – optional deterministic RNG seed for reproducible sampling.

### Response Body

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

Each sample entry contains the chunk index, payload bytes (hex), SHA-256 leaf
digest, and a Merkle inclusion proof (with optional siblings encoded as hex
strings). Clients can verify proofs using the `chunk_root` field.

## Limits and Budgets

* **Max samples per request** – configurable via `torii.rbc_sampling.max_samples_per_request`.
* **Max bytes per request** – enforced using `torii.rbc_sampling.max_bytes_per_request`.
* **Daily byte budget** – tracked per caller through `torii.rbc_sampling.daily_byte_budget`.
* **Rate limiting** – enforced using a dedicated token bucket (`torii.rbc_sampling.rate_per_minute`).

Requests exceeding any limit return HTTP 429 (CapacityLimit). When the chunk
store is unavailable or the session is missing payload bytes the endpoint
returns HTTP 404.

## SDK Integration

### JavaScript

`@iroha/iroha-js` exposes the `ToriiClient.sampleRbcChunks` helper so data
availability verifiers can call the endpoint without rolling their own fetch
logic. The helper validates the hex payloads, normalises integers, and returns
typed objects that mirror the response schema above:

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

The helper throws when the server returns malformed data, helping JS-04 parity
tests detect regressions alongside the Rust and Python SDKs. Rust
(`iroha_client::ToriiClient::sample_rbc_chunks`) and Python
(`IrohaToriiClient.sample_rbc_chunks`) ship equivalent helpers; use whichever
matches your sampling harness.
