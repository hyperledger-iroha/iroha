---
title: Proof Streaming
summary: Stream live PoR witnesses or read exact finalized PDP and PoTR outcomes.
---

# SoraFS Proof Streaming

The `sorafs_cli` binary requests live Proof-of-Retrievability (PoR) witnesses or
reads one exact terminal Proof-of-Data-Possession (PDP) or Proof-of-Timed
Retrieval (PoTR) outcome from finalized ledger state. PDP and PoTR responses
are not process-local scheduler status, receipt-cache scans, or pending work:
Torii executes an exact `FindSorafsProofOutcome` query against its finalized
view and returns one canonical NDJSON row.

The HTTP interface uses the schema-closed `ProofStreamHttpRequestV1` envelope.
A PDP request binds the non-zero challenge identifier admitted by the provider
protocol. A PoTR request binds a non-zero 16-byte orchestrator job identifier;
Torii derives the authoritative request-scope identity from the manifest
digest, provider ID, and job ID. Clients cannot synthesize PDP sampling inputs
or request a broad PoTR receipt history.

## CLI Usage

```bash
export TORII_URL="https://gateway.example/"
export PROVIDER_ID_HEX="1111111111111111111111111111111111111111111111111111111111111111"
# Inject this value from the runtime secret store; do not pass it in argv.
export SORAFS_PROOF_BEARER_TOKEN="..."

sorafs_cli proof stream \
  --manifest=artifacts/manifest.to \
  --torii-url="${TORII_URL}" \
  --provider-id-hex="${PROVIDER_ID_HEX}" \
  --proof-kind=por \
  --samples=128 \
  --bearer-token-env=SORAFS_PROOF_BEARER_TOKEN \
  --summary-out=artifacts/proof_stream_summary.json \
  --governance-evidence-dir=artifacts/proof_stream_evidence
```

- The CLI accepts a bare HTTPS `--torii-url` origin or an exact HTTPS
  `--gateway-url` ending in `/v1/sorafs/proof/stream`. It rejects HTTP,
  userinfo, queries, fragments, redirects, and path aliases.
- Authentication is mandatory. `--bearer-token-env` names an uppercase runtime
  environment variable; the credential itself never enters argv, output, or
  evidence. The retired `--stream-token` form is rejected.
- Before streaming, the CLI authenticates an exact native
  `GET /v1/sorafs/pin/{manifest_digest_hex}` against the same origin. The
  record must be `Approved` and must match the local exact canonical manifest's
  digest, root CID, chunker, chunk-plan digest, PoR root, content length, and
  pin policy. The returned finalized height and block hash must both be
  non-zero.
- The command then POSTs canonical Norito JSON matching
  `ProofStreamHttpRequestV1`. Every proof kind carries both
  `expected_finalized_height` and `expected_finalized_block_hash_hex` from that
  readback. Each response row must echo the exact request digest and finalized
  cursor.
- PoR `sample_count` is bounded to `1..=500`; oversized requests fail before
  manifest lookup so gateways do not perform unbounded sampling work.
- The shared transport verifier authenticates canonical rows, exact
  cardinality, request-derived PoR order, duplicates, truncation, and extra
  rows through EOF before the CLI publishes anything.
- With `--emit-events=true`, each NDJSON event is a payload-free projection:
  digests, identifiers, indices, finalized anchors, result, and timing only.
  It never contains leaf bytes, Merkle paths, signed receipts, nonces, or
  credentials. Use `--emit-events=false` when a consumer expects only the final
  summary.
- `--summary-out` writes the aggregated metrics to disk so CI pipelines can
  archive results alongside manifests, signatures, and CAR summaries.
- `--governance-evidence-dir=<dir>` copies the manifest, writes `metadata.json`
  (CLI version, redacted HTTPS origin/path, manifest digest, capture timestamp),
  and persists the summary JSON. Redirect targets and URL credentials can
  never enter this evidence.
- Any gateway failure or local verification failure rejects the command. V1
  deliberately has no failure-budget override.
- `--samples` defaults to `32` for PoR and must not exceed `500`. For PDP pass
  `--proof-kind=pdp --challenge-id-hex=<64-lowercase-hex>` and omit sampling
  and deadline options. For PoTR pass `--proof-kind=potr`,
  `--deadline-ms=<millis>`, and
  `--orchestrator-job-id-hex=<32-lowercase-hex>`; Torii returns only the exact
  finalized outcome whose signed receipt carries that job ID.

Successful end-to-end CLI coverage must use a real authenticated TLS Torii
fixture that exposes both native pin readback and proof streaming on the same
origin. The retired local HTTP mock success cases are not a supported
compatibility path, and the CLI has no insecure test bypass.

## V1 response semantics

- **PoR is generated live.** The current Torii PoR branch samples the requested
  manifest from the configured local `sorafs_node` storage and emits one
  successful row per witness. Each row includes `leaf_index_flat`,
  `chunk_index`, `segment_index`, `leaf_index`, and `proof`. The client rejects
  a row unless the outer indices equal the witness indices, its request digest
  and finalized cursor match the request, its witness verifies against the
  ledger-authoritative manifest root, and its position matches the
  request-derived sample schedule. PoR rows carry the finalized pin-manifest
  cursor, not finalized-outcome provenance.
- **PDP is an exact finalized lookup.** The lookup key is
  `(pdp, challenge_id)`. The response is one terminal `success` or `failure`
  row; there is no `pending` result. `outcome_identity_hex` equals
  `challenge_id_hex`, and the row includes the outcome digest, admission
  envelope digest, finalized block height/hash, and commit timestamp.
- **PoTR is an exact finalized lookup.** The lookup key is
  `(potr, BLAKE3("sorafs.potr.request-scope.v1\0" || manifest_digest ||
  provider_id || orchestrator_job_id))`. The single terminal row contains the
  exact canonical final signed receipt in `receipt_b64` plus the same complete
  committed provenance. The JSON manifest, provider, identity, digest,
  result/reason, deadline, latency, tier, trace, and recorded timestamp must
  agree with that receipt.

Unknown, unfinalized, or mismatched PDP/PoTR scopes return `404`; corrupt or
unavailable authoritative state returns `503`. Manifest/provider/deadline/tier/
job mismatches never fall back to a local cache or a broader scan.

### PoTR HTTP headers

- Clients issue ranged fetches (`GET /v1/sorafs/storage/car/{manifest}` or
  `GET /v1/sorafs/storage/chunk/{manifest}/{digest}`) with
  `Sora-PoTR-Request: deadline=<value>;tier=<hot|warm|archive>` alongside the
  existing gateway headers. The final V1 receipt requires a non-zero
  `request-id=<32-lowercase-hex>`; `trace-id=<32-lowercase-hex>` remains
  optional.
- Gateways respond with `Sora-PoTR-Receipt` (base64-encoded Norito
  `PotrReceiptV1`) and `Sora-PoTR-Status` so clients can verify signed latency
  receipts without issuing a separate API call. Receipts include the requested
  byte range, observed/request timestamps, deterministic request IDs, and the
  gateway’s Ed25519 signature and the admitted provider’s governed ML-DSA
  signature.
- The capture path validates the Ed25519 gateway signature and ML-DSA-65
  provider signature. An authorized retry-safe transaction forwarder must
  submit the exact receipt for ledger commitment, where the native instruction
  rechecks the signatures against the active governed policy. Invalid,
  unsigned, or self-advertised-key receipts are rejected. Every finalized PoTR
  row includes `receipt_b64`, the canonical Norito bytes of that final signed
  receipt. The CLI verifies both signatures and rejects any JSON identity,
  result, timing, tier, trace, or provenance projection that differs from the
  signed and committed object.

### Summary structure

The CLI writes a summary only after a complete, successful, request-bound
sequence. A representative PoR summary is:

```json
{
  "endpoint": "https://gateway.example/v1/sorafs/proof/stream",
  "manifest_digest_hex": "1111111111111111111111111111111111111111111111111111111111111111",
  "provider_id_hex": "2222222222222222222222222222222222222222222222222222222222222222",
  "proof_kind": "por",
  "request_digest_hex": "3333333333333333333333333333333333333333333333333333333333333333",
  "finalized_block_height": 12345,
  "finalized_block_hash_hex": "4444444444444444444444444444444444444444444444444444444444444444",
  "nonce_digest_hex": "5555555555555555555555555555555555555555555555555555555555555555",
  "metrics": {
    "item_total": 32,
    "success_total": 32,
    "failure_total": 0,
    "failure_by_reason": {}
  }
}
```

The summary never includes the bearer token, raw nonce, proof payload, leaf
bytes, signed PoTR receipt, or a URL query/userinfo component. A failed stream
produces no events, summary file, or evidence directory.

This data maps directly onto the metrics documented in
`specs/sorafs_proof_streaming_plan.md`:

- `metrics.success_total` / `metrics.failure_total` feed into
  `torii_sorafs_proof_stream_events_total{result="success|failure"}`.
- `metrics.failure_by_reason` mirrors the `reason` label carried by the same
  counter and allows quick post-processing without scraping Prometheus.
- `metrics.latency_ms` reflects `torii_sorafs_proof_stream_latency_ms_bucket`
  for the requested proof kind.

Instrumentation in `iroha_telemetry` already exposes these Prometheus counters
and histograms (`kind` distinguishes `por`, `pdp`, and `potr`). The CLI summary
provides a deterministic blob for CI gating when metrics export is disabled.

## Dashboard Example

A Grafana skeleton that tracks outcome totals and latency quantiles is
available under
`fixtures/documentation/sorafs_proof_streaming_dashboard.json`. It assumes the metrics
names listed above and can be imported directly with the Prometheus exporter enabled. The main panels include:

1. Proof outcomes (`success_total` vs `failure_total`)
2. Failure reasons split by taxonomy (timeout, invalid_proof, etc.)
3. Latency p50/p95 derived from `sorafs_proof_stream_latency_ms_bucket`

## Operational note

- **Event volume.** The CLI can print one payload-free projection per verified
  item; set `--emit-events=false` when you only need the final summary blob for
  CI.
- **Promotion boundary.** Finalized proof-outcome queries, retry-safe
  terminal-outcome forwarding, and exact-chain native repair handoff do not by
  themselves close the readiness lane. The competing local repair
  manager/checkpoint consumers are removed; promotion still requires
  cross-peer restart reconciliation with one terminal outcome and genuine
  multi-provider deployment evidence.
